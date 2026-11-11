//! Owns campaign transitions and durable SQLite commit as one service. A failed
//! disk/conflict write rolls back the in-memory transaction before returning.
use hoi4companion_storage::{progress, request as database};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};
type Result<T> = std::result::Result<T, String>;
pub struct Runs {
    store: progress::Store,
    path: Option<PathBuf>,
    revision: i64,
}
pub(crate) fn new_id() -> Result<String> {
    #[cfg(windows)]
    {
        let id =
            unsafe { windows::Win32::System::Com::CoCreateGuid() }.map_err(|e| e.to_string())?;
        Ok(format!("{:032x}", id.to_u128()))
    }
    #[cfg(not(windows))]
    {
        Err("Native campaign ID generation unavailable on this platform".into())
    }
}
impl Runs {
    pub fn open(path: Option<PathBuf>) -> Result<Self> {
        let mut result = Self {
            store: progress::Store::default(),
            path,
            revision: 0,
        };
        if let Some(path) = &result.path {
            let name = path.to_string_lossy();
            database(json!({"schema_version":1,"op":"init","path":name}))?;
            let saved = database(json!({"schema_version":1,"op":"load","path":name}))?;
            if !saved.is_null() {
                let snapshot: Value = serde_json::from_str(
                    saved["payload"]
                        .as_str()
                        .ok_or("Invalid progress payload")?,
                )
                .map_err(|e| e.to_string())?;
                result.store.request(progress::Request {
                    schema_version: 1,
                    op: "load".into(),
                    args: json!({"snapshot":snapshot}),
                    ids: vec![],
                })?;
                result.revision = saved["revision"]
                    .as_i64()
                    .ok_or("Invalid storage revision")?;
            }
        }
        Ok(result)
    }
    fn raw(&mut self, op: &str, args: Value, ids: Vec<Value>) -> Result<Value> {
        self.store.request(progress::Request {
            schema_version: 1,
            op: op.into(),
            args,
            ids,
        })
    }
    pub fn call(&mut self, op: &str, args: Value) -> Result<Value> {
        if ["accept", "rollback"].contains(&op) {
            return Err("Storage transaction boundary is owned by the backend".into());
        }
        let mut ids = vec![];
        let result = loop {
            let result = self.raw(op, args.clone(), ids.clone())?;
            if result["needs_id"] != true {
                break result;
            }
            if ids.len() >= 20000 {
                return Err("Progress ID limit exceeded".into());
            }
            ids.push(json!(new_id()?));
        };
        if result["changed"] == true {
            if let Some(path) = &self.path {
                let saved = database(
                    json!({"schema_version":1,"op":"save","path":path.to_string_lossy(),"expected_revision":self.revision,"payload":result["snapshot"].to_string()}),
                );
                match saved {
                    Ok(value) => {
                        self.revision = value["revision"]
                            .as_i64()
                            .ok_or("Invalid committed revision")?
                    }
                    Err(error) => {
                        self.raw("rollback", json!({}), vec![])?;
                        return Err(error);
                    }
                }
            }
            self.raw("accept", json!({}), vec![])?;
        }
        let mut value=result["value"].clone();
        if op=="get" && args["include_decision_history"]==true && value["campaign_id"].is_string() {
            let history=self.raw("decision_history",json!({"campaign_id":value["campaign_id"],"country":value["country"]}),vec![])?;
            if history["value"].is_object() {value["decision_history"]=history["value"].clone();}
        }
        Ok(value)
    }
    pub fn observe_decisions(&mut self, state: &Value) -> Result<bool> {
        self.call("observe_decisions",json!({"state":crate::state::normalize(state)})).map(|v|v==true)
    }
    pub fn observe(&mut self, guide: &Value, state: &Value, allow_research: bool) -> Result<bool> {
        let record = self.call("get", json!({"guide_id":guide["id"],"include_decision_history":true}))?;
        let records = self.call("records", json!({}))?;
        let result = crate::guide_observe::observe(
            guide,
            state,
            &record,
            &records,
            allow_research,
        )?;
        if result["changed"] != true {
            return Ok(false);
        }
        let mut records = json!({});
        let mut durable=result["record"].clone();
        durable.as_object_mut().unwrap().remove("decision_history");
        records[record["run_id"].as_str().ok_or("Missing run ID")?] = durable;
        self.call("observe_replace", json!({"records":records,"value":true}))?;
        Ok(true)
    }
}
static REGISTRY: OnceLock<Mutex<HashMap<u64, Arc<Mutex<Runs>>>>> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
fn registry() -> &'static Mutex<HashMap<u64, Arc<Mutex<Runs>>>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}
pub fn execute(op: &str, args: &Value) -> Result<Value> {
    if op == "runs_open" {
        let path = args["path"].as_str().map(PathBuf::from);
        let store = Runs::open(path)?;
        let mut entries = registry()
            .lock()
            .map_err(|_| "Progress registry unavailable")?;
        if entries.len() >= 512 {
            return Err("Progress registry limit exceeded".into());
        }
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        entries.insert(id, Arc::new(Mutex::new(store)));
        return Ok(json!(id));
    }
    let id = args["handle"]
        .as_u64()
        .ok_or("Missing progress owner handle")?;
    if op == "runs_release" {
        return Ok(json!(
            registry()
                .lock()
                .map_err(|_| "Progress registry unavailable")?
                .remove(&id)
                .is_some()
        ));
    }
    let store = registry()
        .lock()
        .map_err(|_| "Progress registry unavailable")?
        .get(&id)
        .cloned()
        .ok_or("Unknown progress owner handle")?;
    let mut store = store.lock().map_err(|_| "Progress store unavailable")?;
    match op {
        "runs_status" => Ok(json!({"revision":store.revision,"path":store.path})),
        "runs_call" => store.call(
            args["operation"]
                .as_str()
                .ok_or("Missing progress operation")?,
            args["args"].clone(),
        ),
        "runs_observe" => {
            let guide = crate::guide_schema::normalize(&args["guide"])?;
            store
                .observe(
                    &guide,
                    &args["state"],
                    args["allow_research"].as_bool().unwrap_or(true),
                )
                .map(|v| json!(v))
        }
        _ => Err("Unknown progress owner operation".into()),
    }
}
