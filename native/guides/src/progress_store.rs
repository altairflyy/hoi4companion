//! Owns campaign transitions and durable SQLite commit as one service. A failed
//! disk/conflict write rolls back the in-memory transaction before returning.
use crate::definition_archive::{self, Archive};
use hoi4companion_storage::{progress, referenced_definitions, request as database};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};
type Result<T> = std::result::Result<T, String>;
/// Supplies the archive of the definition currently evaluated for a guide, or
/// None when that guide is unknown (its new runs then stay unbound).
pub type Provider<'a> = dyn FnMut(&str) -> Result<Option<Archive>> + 'a;
pub struct Runs {
    store: progress::Store,
    path: Option<PathBuf>,
    revision: i64,
    pub generation: u64,
    /// Archive payloads by ID: loaded from SQLite or awaiting publication.
    archives: HashMap<String, Arc<str>>,
    /// IDs already committed in `guide_definitions` (memory stores: none).
    persisted: HashSet<String>,
}
pub fn new_id() -> Result<String> {
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
            generation: 0,
            archives: HashMap::new(),
            persisted: HashSet::new(),
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
                // Verified lazily per run (definition_archive::open); a missing
                // or corrupt archive makes only its runs unverifiable.
                for (id, payload) in saved["definitions"].as_object().into_iter().flatten() {
                    if let Some(payload) = payload.as_str() {
                        result.archives.insert(id.clone(), Arc::from(payload));
                        result.persisted.insert(id.clone());
                    }
                }
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
    /// Payload of an archived definition known to this store, if any.
    pub fn archive(&self, id: &str) -> Option<Arc<str>> {
        self.archives.get(id).cloned()
    }
    /// Re-pin the active run of `guide` to `archive`, with progress already
    /// translated to that definition; the archive is published in the same
    /// SQLite transaction.
    pub fn upgrade(&mut self, guide: &str, run_id: &Value, archive: &Archive, checks: Value, started: Value) -> Result<Value> {
        self.archives
            .entry(archive.id.clone())
            .or_insert_with(|| Arc::from(archive.payload.as_str()));
        self.transact(
            "upgrade",
            json!({"guide_id":guide,"run_id":run_id,"definition":definition_archive::pin(&archive.id),"checks":checks,"started":started}),
            None,
        )
    }
    /// Definition-unaware call (older ABI/test callers): runs stay unbound.
    pub fn call(&mut self, op: &str, args: Value) -> Result<Value> {
        self.transact(op, args, None)
    }
    /// Pinning-aware call: every written run is bound to a definition from
    /// `provider` before the same SQLite transaction commits it.
    pub fn call_with(&mut self, op: &str, mut args: Value, provider: &mut Provider<'_>) -> Result<Value> {
        if !args.is_object() {
            args = json!({});
        }
        if !args["definitions"].is_object() {
            args["definitions"] = json!({});
        }
        self.transact(op, args, Some(provider))
    }
    fn transact(&mut self, op: &str, mut args: Value, mut provider: Option<&mut Provider<'_>>) -> Result<Value> {
        if ["accept", "rollback"].contains(&op) {
            return Err("Storage transaction boundary is owned by the backend".into());
        }
        let mut ids = vec![];
        let mut asked = 0;
        let result = loop {
            let result = self.raw(op, args.clone(), ids.clone())?;
            if let Some(guide) = result["needs_definition"].as_str() {
                let provider = provider.as_mut().ok_or("Definition provider unavailable")?;
                asked += 1;
                if asked > 4096 || args["definitions"].get(guide).is_some() {
                    return Err("Guide definition binding did not converge".into());
                }
                args["definitions"][guide] = match provider(guide)? {
                    Some(archive) => {
                        self.archives
                            .entry(archive.id.clone())
                            .or_insert_with(|| Arc::from(archive.payload.as_str()));
                        definition_archive::pin(&archive.id)
                    }
                    None => Value::Null,
                };
                continue;
            }
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
                // Publish exactly the archives this snapshot references and the
                // database does not hold yet, in the progress transaction.
                let mut definitions = serde_json::Map::new();
                let mut missing = false;
                for id in referenced_definitions(&result["snapshot"]) {
                    if self.persisted.contains(&id) {
                        continue;
                    }
                    match self.archives.get(&id) {
                        Some(payload) => {
                            definitions.insert(id, json!(&**payload));
                        }
                        None => missing = true,
                    }
                }
                let saved = if missing {
                    Err("Progress references an unarchived guide definition".to_string())
                } else {
                    database(
                        json!({"schema_version":1,"op":"save","path":path.to_string_lossy(),"expected_revision":self.revision,"version":hoi4companion_storage::PROGRESS_VERSION,"definitions":definitions,"payload":result["snapshot"].to_string()}),
                    )
                };
                match saved {
                    Ok(value) => {
                        self.revision = value["revision"]
                            .as_i64()
                            .ok_or("Invalid committed revision")?;
                        self.persisted.extend(definitions.keys().cloned());
                    }
                    Err(error) => {
                        self.raw("rollback", json!({}), vec![])?;
                        return Err(error);
                    }
                }
            }
            self.raw("accept", json!({}), vec![])?;
            self.generation = self.generation.wrapping_add(1);
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
        self.observe_with(guide, state, allow_research, None)
    }
    /// Observe with the definition the active run is evaluated against. With a
    /// provider, a legacy run also records its first observed definition.
    pub fn observe_with(
        &mut self,
        guide: &Value,
        state: &Value,
        allow_research: bool,
        provider: Option<&mut Provider<'_>>,
    ) -> Result<bool> {
        // Scope needs only nation/tree, not tens of thousands of provincial
        // facts. The admitted observation is still normalized in full below.
        let scope_input = if state.get("facts").is_some() {
            json!({"schema_version":state["schema_version"],"usable":state["usable"],"session_id":state["session_id"],"facts":{"country":state["facts"]["country"],"focus.tree":state["facts"]["focus.tree"]}})
        } else {
            json!({"usable":state["usable"],"source":state["source"],"sample":{"country":state["sample"]["country"],"focus_tree_id":state["sample"]["focus_tree_id"]}})
        };
        let normalized = crate::state::normalize(&scope_input);
        // Foreign/unqualified guides cannot record an observation. Apply the
        // existing admission rule before cloning campaign records and history.
        if crate::guide_prepare::mode(guide, &normalized) != "live" {
            return Ok(false);
        }
        let record = self.call("get", json!({"guide_id":guide["id"],"include_decision_history":true}))?;
        let records = if allow_research { self.call("records", json!({}))? } else { json!({}) };
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
        let args = json!({"records":records,"value":true});
        match provider {
            Some(provider) => self.call_with("observe_replace", args, provider)?,
            None => self.call("observe_replace", args)?,
        };
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
        "runs_status" => Ok(json!({"revision":store.revision,"path":store.path,"archives":store.persisted.len()})),
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
