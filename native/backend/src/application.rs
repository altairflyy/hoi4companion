//! Coherent application ownership, independent of HTTP, DOM and native reader.
//! Definitions, observed state and per-campaign progression remain separate.
use crate::{guide_runtime, guide_schema, progress_store::Runs, state};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};
type Result<T> = std::result::Result<T, String>;

fn text(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn rows(guide: &Value) -> impl Iterator<Item = &Value> {
    guide["tracks"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
        .flat_map(|v| v.as_array().into_iter().flatten())
}
fn sorted(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .map(|(k, v)| (k.clone(), sorted(v)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(sorted).collect()),
        v => v.clone(),
    }
}
fn check_keys(node: &Value, keys: &mut HashSet<String>) {
    if node["op"] == "check" {
        keys.insert(text(&node["key"]).into());
    }
    if let Some(a) = node["args"].as_array() {
        for child in a {
            check_keys(child, keys);
        }
    }
    if node.get("arg").is_some() {
        check_keys(&node["arg"], keys);
    }
}
fn manual_keys(guide: &Value) -> HashSet<String> {
    let mut keys = HashSet::new();
    for row in rows(guide) {
        if ["gate", "project"].contains(&text(&row["kind"])) && row["tracking"] != "reminder" {
            keys.insert(text(&row["key"]).into());
        }
        for field in [
            "when",
            "priority_when",
            "skip_when",
            "completion",
            "started_when",
        ] {
            check_keys(&row[field], &mut keys);
        }
        for field in ["gate", "available_gate"] {
            if let Some(key) = row[field].as_str() {
                keys.insert(key.into());
            }
        }
    }
    for row in guide["milestones"].as_array().into_iter().flatten() {
        keys.insert(text(&row["key"]).into());
    }
    for row in guide["phases"].as_array().into_iter().flatten() {
        check_keys(&row["when"], &mut keys);
    }
    keys
}
fn campaign(state: &Value) -> Option<&str> {
    if state["usable"] != true {
        return None;
    }
    let id = &state["sample"]["campaign_identity"];
    if id["status"] == "observed"
        && id["source"] == "native_game_unique_id_and_seed"
        && hoi4companion_storage::progress::campaign(&id["id"])
    {
        id["id"].as_str()
    } else {
        None
    }
}

pub struct Application {
    guides: Map<String, Value>,
    context: Option<Arc<crate::runtime_context::Context>>,
    sources: Map<String, Value>,
    definition_generation: u64,
    association_directory: Option<PathBuf>,
    runs: Runs,
    live: Value,
    server: String,
    active: Option<String>,
    default: Option<String>,
    selection_path: Option<PathBuf>,
    active_revision: u64,
    revision: u64,
    observation_revision: u64,
    persistent: bool,
}
pub(crate) struct PreparedContext {
    generation: u64,
    guides: Map<String, Value>,
    context: Arc<crate::runtime_context::Context>,
}
impl Application {
    pub fn open(args: &Value) -> Result<Self> {
        let path = args["path"].as_str().map(PathBuf::from);
        let mut result = Self {
            guides: Map::new(),
            context: None,
            sources: Map::new(),
            definition_generation: 0,
            association_directory: args["association_directory"].as_str().map(PathBuf::from),
            runs: Runs::open(path.clone())?,
            live: json!({"schema_version":1,"usable":false,"facts":{}}),
            server: crate::progress_store::new_id()?,
            active: None,
            default: None,
            selection_path: args["selection_path"].as_str().map(PathBuf::from),
            active_revision: 0,
            revision: 0,
            observation_revision: 0,
            persistent: path.is_some(),
        };
        for guide in args["guides"].as_array().into_iter().flatten() {
            if result.guides.contains_key(text(&guide["id"])) {
                return Err("Identità guida duplicata".into());
            }
            result.put_guide(guide)?;
        }
        if args["guide_directories"].is_array() {
            let loaded = crate::guide_repository::load(&args["guide_directories"], None)?;
            for guide in loaded["guides"].as_array().into_iter().flatten() {
                if result.guides.contains_key(text(&guide["id"])) {
                    return Err("Identità guida duplicata".into());
                }
                result.put_guide(guide)?;
            }
        }
        let defaults: Vec<_> = result
            .guides
            .values()
            .filter(|g| g["default"] == true)
            .collect();
        if defaults.len() > 1 {
            return Err("Multiple default guides".into());
        }
        result.active = defaults
            .first()
            .map(|g| text(&g["id"]).to_owned())
            .or_else(|| result.guides.keys().next().cloned());
        result.default = result.active.clone();
        if let Some(path) = &result.selection_path {
            match std::fs::read(path) {
                Ok(data) => {
                    if let Ok(saved) = serde_json::from_slice::<Value>(&data)
                        && result.guides.contains_key(text(&saved["guide_id"]))
                    {
                        result.active = Some(text(&saved["guide_id"]).into());
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(format!("Guide selection unavailable: {e}")),
            }
        }
        Ok(result)
    }
    pub fn put_guide(&mut self, guide: &Value) -> Result<Value> {
        let mut guide = guide_schema::normalize(guide)?;
        let id = text(&guide["id"]).to_owned();
        if id.is_empty()
            || id.len() > 160
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err("Invalid guide identity".into());
        }
        guide
            .as_object_mut()
            .unwrap()
            .remove("runtime_definition_revision");
        guide.as_object_mut().unwrap().remove("runtime_engine");
        let source = guide.clone();
        if let Some(directory) = &self.association_directory {
            guide = crate::guide_repository::apply(
                &guide,
                &crate::guide_repository::associations(&id, directory)?,
            )?;
        }
        if let Some(context) = &self.context {
            guide = context.present(&context.prepare(&guide)?)?;
        }
        let guide = Self::revision(guide)?;
        if self.guides.len() >= 4096 && !self.guides.contains_key(&id) {
            return Err("Guide registry limit exceeded".into());
        }
        self.definition_generation += 1;
        self.sources.insert(id.clone(), source);
        self.guides.insert(id, guide.clone());
        Ok(guide)
    }
    fn revision(mut guide: Value) -> Result<Value> {
        guide
            .as_object_mut()
            .ok_or("Invalid guide definition")?
            .remove("runtime_definition_revision");
        guide.as_object_mut().unwrap().remove("runtime_engine");
        guide["runtime_definition_revision"] = json!(format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&sorted(&guide)).map_err(|e| e.to_string())?)
        ));
        guide["runtime_engine"] = json!("rust-guide-core");
        Ok(guide)
    }
    pub(crate) fn context_definitions(&self) -> Result<Vec<Value>> {
        self.sources
            .values()
            .map(|source| {
                if let Some(directory) = &self.association_directory {
                    crate::guide_repository::apply(
                        source,
                        &crate::guide_repository::associations(text(&source["id"]), directory)?,
                    )
                } else {
                    Ok(source.clone())
                }
            })
            .collect()
    }
    pub(crate) fn context_input(&self) -> Result<Value> {
        Ok(
            json!({"generation":self.definition_generation,"definitions":self.context_definitions()?}),
        )
    }
    pub(crate) fn prepare_context(
        input: &Value,
        context: Arc<crate::runtime_context::Context>,
    ) -> Result<PreparedContext> {
        let guides = input["definitions"]
            .as_array()
            .ok_or("Missing guide definitions")?
            .iter()
            .map(|source| {
                let guide = Self::revision(context.present(&context.prepare(source)?)?)?;
                Ok((text(&guide["id"]).to_owned(), guide))
            })
            .collect::<Result<Map<String, Value>>>()?;
        Ok(PreparedContext {
            generation: input["generation"]
                .as_u64()
                .ok_or("Missing source generation")?,
            guides,
            context,
        })
    }
    pub(crate) fn check_context(&self, prepared: &PreparedContext) -> Result<()> {
        if self.definition_generation != prepared.generation {
            return Err("Guide cambiate durante la compilazione: ripeti il ricaricamento".into());
        }
        Ok(())
    }
    pub(crate) fn apply_context(&mut self, prepared: PreparedContext) {
        self.guides = prepared.guides;
        self.context = Some(prepared.context);
        self.revision += 1;
    }
    pub(crate) fn set_context(
        &mut self,
        context: Arc<crate::runtime_context::Context>,
    ) -> Result<()> {
        let prepared = Self::prepare_context(&self.context_input()?, context)?;
        self.check_context(&prepared)?;
        self.apply_context(prepared);
        Ok(())
    }
    pub(crate) fn can_add_guide(&self, id: &str) -> Result<()> {
        if self.guides.contains_key(id) {
            return Err("Guida già presente: scegli un nuovo ID per una revisione".into());
        }
        if self.guides.len() >= 4096 {
            return Err("Guide registry limit exceeded".into());
        }
        Ok(())
    }
    fn identity(&self, id: Option<&str>) -> Result<String> {
        let id = id.or(self.active.as_deref()).ok_or("No guide available")?;
        if !self.guides.contains_key(id) {
            return Err("Guide not found".into());
        }
        Ok(id.into())
    }
    fn storage(&self) -> Value {
        json!({"status":if self.persistent{"persistent"}else{"memory"},"error":""})
    }
    fn bind(&mut self) -> Result<()> {
        let changed = if let Some(id) = campaign(&self.live) {
            self.runs.call("bind_campaign",json!({"campaign_id":id,"country":self.live["sample"]["country"],"session_id":self.live["sample"]["session_id"],"tick":self.live["sample"]["native_tick"]}))?
        } else {
            let state = state::normalize(&self.live);
            self.runs.call("bind",json!({"session_id":if self.live["usable"]==true {state["session_id"].clone()}else{Value::Null}}))?
        };
        if changed == true {
            self.revision += 1;
        }
        if self.live["usable"] == true {
            let ids = self.runs.call("metadata", json!({}))?;
            for id in ids.as_object().into_iter().flat_map(|m| m.keys()) {
                if let Some(guide) = self.guides.get(id)
                    && self
                        .runs
                        .observe(guide, &self.live, self.active.as_ref() == Some(id))?
                {
                    self.revision += 1;
                }
            }
        }
        Ok(())
    }
    pub fn accept(&mut self, value: Value) -> Result<Value> {
        self.observation_revision = self.observation_revision.wrapping_add(1);
        self.live = value;
        self.bind()?;
        Ok(self.live.clone())
    }
    pub fn invalidate_observation(&mut self) {
        self.live = json!({"usable":false,"status":"disconnected","sample":null,"error":"Installation generation changed; awaiting a qualified observation"});
        self.revision += 1;
    }
    pub fn live_packet(&mut self) -> Result<Value> {
        let mut value = self.live.clone();
        let metadata = self.runs.call("metadata", json!({}))?;
        let (mut checks, mut started, mut baselines) = (json!({}), json!({}), json!({}));
        for id in metadata.as_object().into_iter().flat_map(|m| m.keys()) {
            let record = self.runs.call("get", json!({"guide_id":id}))?;
            checks[id] = record["checks"].clone();
            started[id] = record["started"].clone();
            baselines[id] = record["baselines"].clone();
        }
        value["guide_id"] = json!(self.default);
        value["guide_checks"] = self
            .default
            .as_ref()
            .and_then(|id| checks.get(id))
            .cloned()
            .unwrap_or(json!({}));
        value["active_guide_id"] = json!(self.active);
        value["active_guide_revision"] = json!(self.active_revision);
        value["guide_states"] = checks;
        value["guide_started"] = started;
        value["guide_baselines"] = baselines;
        value["guide_runs"] = metadata;
        value["guide_server_id"] = json!(self.server);
        value["guide_storage"] = self.storage();
        value["guide_revision"] = json!(self.revision);
        value["overlay"] = Value::Null;
        Ok(value)
    }
    pub fn active(&mut self, update: Option<&Value>) -> Result<Value> {
        if let Some(update) = update {
            if update.as_object().is_none_or(|m| m.len() != 1) {
                return Err("Invalid guide selection".into());
            }
            let id = self.identity(Some(
                update["guide_id"]
                    .as_str()
                    .ok_or("Invalid guide selection")?,
            ))?;
            if self.active.as_ref() != Some(&id) {
                if let Some(path) = &self.selection_path {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                    }
                    let temporary = path.with_extension(format!("{}.tmp", self.server));
                    let save = (|| {
                        use std::io::Write;
                        let mut file =
                            std::fs::File::create(&temporary).map_err(|e| e.to_string())?;
                        file.write_all(json!({"guide_id":id}).to_string().as_bytes())
                            .map_err(|e| e.to_string())?;
                        file.sync_all().map_err(|e| e.to_string())?;
                        drop(file);
                        std::fs::rename(&temporary, path).map_err(|e| e.to_string())
                    })();
                    if let Err(error) = save {
                        let _ = std::fs::remove_file(temporary);
                        return Err(error);
                    }
                }
                self.active = Some(id);
                self.active_revision += 1;
            }
        }
        let id = self.identity(None)?;
        Ok(
            json!({"schema_version":1,"guide_id":id,"title":self.guides[&id]["title"],"revision":self.active_revision,"server_id":self.server}),
        )
    }
    fn ensure(&mut self, id: &str) -> Result<()> {
        self.runs
            .call("ensure", json!({"guide_id":id,"checks":{}}))?;
        self.bind()
    }
    fn guard(&self, update: &Value, current: &Value) -> Result<Value> {
        let mut update = update.as_object().cloned().ok_or("Invalid run update")?;
        if current["campaign_id"].as_str().is_some()
            && (campaign(&self.live) != current["campaign_id"].as_str()
                || self.live["sample"]["country"] != current["country"])
        {
            return Err("Campaign identity not verified: progress suspended".into());
        }
        if update.get("server_id").is_some_and(|v| *v != self.server) {
            return Err("Server restarted: reload progress".into());
        }
        if update
            .get("expected_run_id")
            .is_some_and(|v| *v != current["run_id"])
        {
            return Err("Campaign or checklist changed: reload progress".into());
        }
        if update
            .get("expected_revision")
            .is_some_and(|v| v.as_u64().is_none() || *v != current["run_revision"])
        {
            return Err("Progress updated by another view: reload progress".into());
        }
        for key in ["server_id", "expected_run_id", "expected_revision"] {
            update.remove(key);
        }
        Ok(Value::Object(update))
    }
    pub fn progress(
        &mut self,
        id: Option<&str>,
        run: Option<&str>,
        update: Option<&Value>,
    ) -> Result<Value> {
        let id = self.identity(id)?;
        self.ensure(&id)?;
        if let Some(update) = update {
            let current = self.runs.call("get", json!({"guide_id":id}))?;
            let update = self.guard(update, &current)?;
            let map = update.as_object().unwrap();
            if map.is_empty() {
                self.runs.call(
                    "create",
                    json!({"guide_id":id,"session_id":current["session_id"]}),
                )?;
            } else if map.len() == 1 && map.contains_key("run_id") {
                self.runs
                    .call("select", json!({"guide_id":id,"run_id":update["run_id"]}))?;
            } else if map.contains_key("run_id")
                && map
                    .keys()
                    .all(|k| ["run_id", "checks", "started"].contains(&k.as_str()))
                && (map.contains_key("checks") || map.contains_key("started"))
            {
                if update["run_id"] != current["run_id"] {
                    return Err("Checklist belongs to an inactive run".into());
                }
                let checks = update.get("checks").unwrap_or(&current["checks"]);
                let allowed = manual_keys(&self.guides[&id]);
                if checks
                    .as_object()
                    .is_none_or(|m| m.keys().any(|k| !allowed.contains(k)))
                {
                    return Err("Checks not declared by the guide".into());
                }
                let started = update.get("started").unwrap_or(&current["started"]);
                let operations: HashSet<_> = rows(&self.guides[&id])
                    .filter(|s| {
                        ["manual", "special_project"].contains(&text(&s["action"]["kind"]))
                            && s["tracking"] != "reminder"
                    })
                    .map(|s| text(&s["key"]))
                    .collect();
                if started
                    .as_object()
                    .is_none_or(|m| m.keys().any(|k| !operations.contains(k.as_str())))
                {
                    return Err("Starts not declared by the guide".into());
                }
                self.runs.call(
                    "update",
                    json!({"guide_id":id,"checks":checks,"started":started}),
                )?;
            } else {
                return Err("Unknown run operation".into());
            }
            self.revision += 1;
        }
        let mut current = self.runs.call("get", json!({"guide_id":id,"run_id":run}))?;
        current["revision"] = json!(self.revision);
        current["server_id"] = json!(self.server);
        current["history"] = self.runs.call("history", json!({"guide_id":id}))?;
        current["storage"] = self.storage();
        Ok(current)
    }
    pub fn projection(&mut self, id: Option<&str>, live: bool) -> Result<Value> {
        let id = self.identity(id)?;
        let progress = self.progress(Some(&id), None, None)?;
        let state = if live {
            self.live.clone()
        } else {
            json!({"schema_version":1,"usable":false,"session_id":null,"facts":{}})
        };
        let guide = self.project_definition(&id, &state, &progress)?;
        Ok(
            json!({"schema_version":1,"engine":"rust-guide-core","server_id":self.server,"guide_id":id,"definition_revision":guide["runtime_definition_revision"],"guide":guide,"state":state,"progress":progress,"evaluation":guide_runtime::evaluate(&guide,&state,&progress)?}),
        )
    }
    fn project_definition(&self, id: &str, state: &Value, progress: &Value) -> Result<Value> {
        let guide = &self.guides[id];
        if !rows(guide).any(|s| {
            s["relative_research"].is_object()
                || s["relative_research_unresolved"] == true
                || s["research_choice"].is_object()
        }) {
            return Ok(guide.clone());
        }
        let assessment = guide_runtime::evaluate(guide, state, progress)?;
        let empty = json!({});
        let bindings =
            if assessment["mode"] == "live" && assessment["progress_status"] == "accepted" {
                &progress["research_bindings"]
            } else {
                &empty
            };
        let mut projected = crate::research_projection::project(guide, bindings)?;
        if let Some(context) = &self.context {
            projected = context.present(&projected)?;
        }
        projected
            .as_object_mut()
            .unwrap()
            .remove("runtime_definition_revision");
        projected.as_object_mut().unwrap().remove("runtime_engine");
        let revision = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&sorted(&projected)).map_err(|e| e.to_string())?)
        );
        projected["runtime_definition_revision"] = json!(revision);
        projected["runtime_engine"] = json!("rust-guide-core");
        Ok(projected)
    }
    fn guide(&mut self, id: Option<&str>) -> Result<Value> {
        let id = self.identity(id)?;
        let progress = self.progress(Some(&id), None, None)?;
        self.project_definition(&id, &self.live, &progress)
    }
    pub fn checks(&mut self, id: Option<&str>, update: Option<&Value>) -> Result<Value> {
        let id = self.identity(id)?;
        self.ensure(&id)?;
        let current = self.runs.call("get", json!({"guide_id":id}))?;
        if let Some(update) = update {
            self.guard(&json!({"checks":{}}), &current)?;
            let allowed = manual_keys(&self.guides[&id]);
            fn apply(current: &mut Value, update: &Value, allowed: &HashSet<String>) -> Result<()> {
                if update.as_object().is_none_or(|m| m.len() != 2)
                    || update["key"].as_str().is_none()
                    || !update["checked"].is_boolean()
                {
                    return Err("Conferma manuale non valida".into());
                }
                let key = update["key"].as_str().unwrap();
                if !allowed.contains(key) {
                    return Err("Questa voce non è una conferma manuale della guida".into());
                }
                if update["checked"] == true {
                    current[key] = json!(true);
                } else {
                    current.as_object_mut().unwrap().remove(key);
                }
                Ok(())
            }
            let mut checks = current["checks"].clone();
            if *update == json!({"reset":true}) {
                checks = json!({});
            } else if update.as_object().is_some_and(|m| m.len() == 1)
                && let Some(values) = update["checks"].as_object()
            {
                checks = json!({});
                for (key, value) in values {
                    apply(&mut checks, &json!({"key":key,"checked":value}), &allowed)?;
                }
            } else {
                apply(&mut checks, update, &allowed)?;
            }
            self.runs.call("update",json!({"guide_id":id,"checks":checks,"started":if *update==json!({"reset":true}){json!({})}else{current["started"].clone()}}))?;
            self.revision += 1;
        }
        Ok(
            json!({"checks":self.runs.call("get",json!({"guide_id":id}))?["checks"],"revision":self.revision}),
        )
    }
    pub fn request(&mut self, op: &str, args: &Value) -> Result<Value> {
        match op {
        "app_live_packet"=>self.live_packet(),
        "app_overlay_packet"=>{let id=self.identity(None)?;let packet=self.projection(Some(&id),true)?;Ok(packet)},
        "app_overlay_stamp"=>Ok(json!({"server":self.server,"guide":self.active,"selection":self.active_revision,"definition":self.definition_generation,"progress":self.revision,"observation":self.observation_revision})),
        "app_state"=>self.accept(args["value"].clone()),
        "app_active"=>self.active(args.get("update")),
        "app_progress"=>self.progress(args["guide_id"].as_str(),args["run_id"].as_str(),args.get("update")),
        "app_checks"=>self.checks(args["guide_id"].as_str(),args.get("update")),
        "app_projection"=>self.projection(args["guide_id"].as_str(),args["live"].as_bool().unwrap_or(true)),
        "app_guide"=>self.guide(args["guide_id"].as_str()),
        "app_source"=>{let id=self.identity(args["guide_id"].as_str())?;Ok(self.sources[&id].clone())},
        "app_definition"=>{let id=self.identity(args["guide_id"].as_str())?;Ok(self.guides[&id].clone())},
        "app_put_guide"=>self.put_guide(&args["guide"]),
        "app_guides"=>Ok(json!(self.guides.values().map(|g|json!({"id":g["id"],"title":g["title"],"schema_version":1,"applies_to":g["applies_to"]})).collect::<Vec<_>>())),
        _=>Err("Unknown application operation".into())
    }
    }
}
type Owner = Arc<Mutex<Application>>;
static REGISTRY: OnceLock<Mutex<HashMap<u64, Owner>>> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
fn registry() -> &'static Mutex<HashMap<u64, Owner>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}
pub fn execute(op: &str, args: &Value) -> Result<Value> {
    if op == "app_open" {
        let owner = Application::open(args)?;
        let mut entries = registry()
            .lock()
            .map_err(|_| "Application registry unavailable")?;
        if entries.len() >= 64 {
            return Err("Application registry limit exceeded".into());
        }
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        entries.insert(id, Arc::new(Mutex::new(owner)));
        return Ok(json!(id));
    }
    let id = args["handle"]
        .as_u64()
        .ok_or("Missing application handle")?;
    if op == "app_release" {
        return Ok(json!(
            registry()
                .lock()
                .map_err(|_| "Application registry unavailable")?
                .remove(&id)
                .is_some()
        ));
    }
    let owner = registry()
        .lock()
        .map_err(|_| "Application registry unavailable")?
        .get(&id)
        .cloned()
        .ok_or("Unknown application handle")?;
    owner
        .lock()
        .map_err(|_| "Application unavailable")?
        .request(op, args)
}
