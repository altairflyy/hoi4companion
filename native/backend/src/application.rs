//! Coherent application ownership, independent of HTTP, DOM and native reader.
//! Definitions, observed state and per-campaign progression remain separate.
use crate::{
    definition_archive::{self, Archive},
    guide_runtime, guide_schema,
    progress_store::Runs,
    state,
};
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

fn same_observation(left: &Value, right: &Value) -> bool {
    let (Some(left), Some(right)) = (left.as_object(), right.as_object()) else {
        return left == right;
    };
    let significant = |key: &str| !matches!(key,
        "observed_at" | "age_seconds" | "metrics" | "transport" | "overlay");
    left.iter().filter(|(key, _)| significant(key))
        .all(|(key, value)| right.get(key) == Some(value))
        && right.keys().filter(|key| significant(key)).all(|key| left.contains_key(key))
}

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
pub(crate) fn manual_keys(guide: &Value) -> HashSet<String> {
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
fn started_keys(guide: &Value) -> HashSet<String> {
    rows(guide)
        .filter(|s| {
            ["manual", "special_project"].contains(&text(&s["action"]["kind"]))
                && s["tracking"] != "reminder"
        })
        .map(|s| text(&s["key"]).to_owned())
        .collect()
}
/// Memo of `current_archive`: guide ID -> (key, compiled archive or its error).
/// A failed compile is kept too, so a broken guide is not recompiled on every
/// observation; a new revision or catalog context retries it.
type ArchiveMemo = HashMap<String, (Value, Result<Archive>)>;
fn archive_key(guide: &Value, context: Option<&Arc<crate::runtime_context::Context>>) -> Value {
    json!([guide["runtime_definition_revision"], context.map(|c| Arc::as_ptr(c) as usize)])
}
/// Archive that NEW runs of `id` are pinned to: the current compiler contract
/// (`Context::compile_new` from the associated source when a catalog is
/// loaded, else decomposition of the registry definition), memoized by the
/// registry revision and catalog context. The contract-1 registry is unchanged.
fn current_archive(
    guides: &Map<String, Value>,
    sources: &Map<String, Value>,
    associations: Option<&PathBuf>,
    context: Option<&Arc<crate::runtime_context::Context>>,
    memo: &mut ArchiveMemo,
    id: &str,
) -> Result<Option<Archive>> {
    let Some(guide) = guides.get(id) else {
        return Ok(None);
    };
    let seen_key = archive_key(guide, context);
    if let Some((seen, archive)) = memo.get(id)
        && *seen == seen_key
    {
        return archive.clone().map(Some);
    }
    let archive = compile_archive(id, guide, sources.get(id), associations, context);
    memo.insert(id.to_owned(), (seen_key, archive.clone()));
    archive.map(Some)
}
/// The compile `current_archive` memoizes. It needs no application state, so
/// `ArchiveJob` can run it without holding the application lock.
fn compile_archive(
    id: &str,
    guide: &Value,
    source: Option<&Value>,
    associations: Option<&PathBuf>,
    context: Option<&Arc<crate::runtime_context::Context>>,
) -> Result<Archive> {
    let compiled = match (context, source) {
        (Some(context), Some(source)) => {
            let input = match associations {
                Some(directory) => crate::guide_repository::apply(
                    source,
                    &crate::guide_repository::associations(id, directory)?,
                )?,
                None => source.clone(),
            };
            context.compile_new(&input)?
        }
        _ => {
            // No catalog: only catalog-free timing rules can compile headers.
            let compile = |text: &str| {
                crate::timing_compiler::compile(&json!(text), None, &guide["applies_to"]["country"],
                    &guide["applies_to"]["tree_id"], "", 0)
            };
            crate::decomposition::decompose(guide, &compile)?["guide"].clone()
        }
    };
    definition_archive::build(id, &compiled, source.unwrap_or(&Value::Null))
}
/// A current definition that a campaign binding may need and that is not
/// compiled for the current catalog yet, captured with everything its compile
/// needs. The live read loop never compiles one: see `Application::cold_archives`.
pub(crate) struct ArchiveJob {
    id: String,
    key: Value,
    guide: Value,
    source: Option<Value>,
    associations: Option<PathBuf>,
    context: Option<Arc<crate::runtime_context::Context>>,
}
impl ArchiveJob {
    pub(crate) fn compile(&self) -> Result<Archive> {
        compile_archive(&self.id, &self.guide, self.source.as_ref(), self.associations.as_ref(), self.context.as_ref())
    }
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
    political_cost_ids: Value,
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
    bound_context: Option<(u64, u64, u64, u64, u64)>,
    persistent: bool,
    /// guide ID -> (runtime revision and catalog, current definition archive).
    current_archives: ArchiveMemo,
    /// Set by the live service, which compiles current definitions in the
    /// background: a campaign binding then waits for them instead of compiling
    /// them inside the read loop (minutes for dozens of guides).
    background_archives: bool,
    observation_scopes: HashMap<String, Value>,
    /// Move checklists onto the current definition only once the game catalog
    /// is applied (default), so startup never moves a run twice.
    upgrade_awaits_catalog: bool,
    /// Verified archived definitions by archive ID, ready for evaluation.
    archived_guides: HashMap<String, Value>,
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
            political_cost_ids: json!([]),
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
            bound_context: None,
            persistent: path.is_some(),
            current_archives: HashMap::new(),
            background_archives: false,
            observation_scopes: HashMap::new(),
            upgrade_awaits_catalog: args["upgrade_without_catalog"] != true,
            archived_guides: HashMap::new(),
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
        self.refresh_political_cost_ids();
        Ok(guide)
    }
    fn refresh_political_cost_ids(&mut self) {
        self.political_cost_ids=if let Some(context)=&self.context {context.political_cost_ids(self.guides.values())}
            else {crate::operational_catalog::political_cost_ids(&Value::Null,self.guides.values())};
    }
    pub(crate) fn political_cost_ids(&self)->&Value {&self.political_cost_ids}
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
                let guide = Self::revision(context.prepare_presented(source)?)?;
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
        self.refresh_political_cost_ids();
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
    /// Run operations that can write: every written run is bound atomically to
    /// the definition the registry evaluates now (new runs) or, for legacy
    /// runs, records it as first-observed evidence only.
    fn runs_call(&mut self, op: &str, args: Value) -> Result<Value> {
        let (guides, sources, memo, context, associations) = (&self.guides, &self.sources, &mut self.current_archives, self.context.as_ref(), self.association_directory.as_ref());
        self.runs
            .call_with(op, args, &mut |id| current_archive(guides, sources, associations, context, memo, id))
    }
    /// Guides whose current definition a campaign binding may need (the active
    /// guide first, then every guide with a checklist) and that are not compiled
    /// for the current catalog yet. Empty until the catalog context exists.
    fn cold_archive_ids(&mut self) -> Result<Vec<String>> {
        let Some(context) = self.context.clone() else {
            return Ok(vec![]);
        };
        let mut ids: Vec<String> = self.active.iter().cloned().collect();
        let metadata = self.runs.call("metadata", json!({}))?;
        for id in metadata.as_object().into_iter().flat_map(|m| m.keys()) {
            if !ids.contains(id) {
                ids.push(id.clone());
            }
        }
        Ok(ids.into_iter().filter(|id| {
            self.guides.get(id).is_some_and(|guide| {
                let key = archive_key(guide, Some(&context));
                !self.current_archives.get(id).is_some_and(|(seen, _)| *seen == key)
            })
        }).collect())
    }
    /// Compiles needed by a campaign binding, for the background service.
    pub(crate) fn cold_archives(&mut self) -> Result<Vec<ArchiveJob>> {
        Ok(self.cold_archive_ids()?.into_iter().map(|id| ArchiveJob {
            key: archive_key(&self.guides[&id], self.context.as_ref()),
            guide: self.guides[&id].clone(),
            source: self.sources.get(&id).cloned(),
            associations: self.association_directory.clone(),
            context: self.context.clone(),
            id,
        }).collect())
    }
    /// Keeps a background compile unless its guide or catalog changed meanwhile.
    pub(crate) fn store_archive(&mut self, job: &ArchiveJob, archive: Result<Archive>) {
        if self.guides.get(&job.id).map(|guide| archive_key(guide, self.context.as_ref())).as_ref() == Some(&job.key) {
            self.current_archives.insert(job.id.clone(), (job.key.clone(), archive));
        }
    }
    pub(crate) fn compile_archives_in_background(&mut self) {
        self.background_archives = true;
    }
    fn current_definition_id(&mut self, id: &str) -> Result<Option<String>> {
        Ok(current_archive(&self.guides, &self.sources, self.association_directory.as_ref(), self.context.as_ref(), &mut self.current_archives, id)?
            .map(|a| a.id))
    }
    /// Updating the app updates open checklists: the active run of `id` is
    /// moved onto the current definition, carrying its manual progress where
    /// the new definition holds the same instruction (`run_upgrade`). Waits for
    /// the game catalog when the guide has a source, so a run is not moved
    /// twice (catalog-free, then catalog) during startup.
    fn upgrade_run(&mut self, id: &str) -> Result<()> {
        if self.upgrade_awaits_catalog && self.context.is_none() && self.sources.contains_key(id) {
            return Ok(());
        }
        let Some(archive) = self.current_definition_archive(id)? else {
            return Ok(());
        };
        let run = self.runs.call("get", json!({"guide_id":id}))?;
        let reference = &run["definition_ref"];
        if reference["status"] == "pinned" && reference["id"] == archive.id.as_str() {
            return Ok(());
        }
        let old = self.definition_for_run(id, &run).ok().map(|(guide, _)| guide);
        let envelope = definition_archive::open(&archive.id, &archive.payload)?;
        let new = Self::revision(envelope["definition"].clone())?;
        let (checks, started, _) = crate::run_upgrade::translate(
            old.as_ref(),
            &new,
            &run["checks"],
            &run["started"],
            &manual_keys(&new),
            &started_keys(&new),
        );
        self.runs.upgrade(id, &run["run_id"], &archive, checks, started)?;
        self.revision += 1;
        Ok(())
    }
    fn current_definition_archive(&mut self, id: &str) -> Result<Option<Archive>> {
        current_archive(&self.guides, &self.sources, self.association_directory.as_ref(), self.context.as_ref(), &mut self.current_archives, id)
    }
    /// The single source of the definition a run is evaluated, observed and
    /// validated against. Pinned runs use only their verified archive, never
    /// the current registry; a missing/corrupt/unsupported archive fails closed.
    /// Legacy runs use the current definition only while the legacy compiler
    /// contract applies, and report evidence of definition changes.
    pub(crate) fn definition_for_run(&mut self, id: &str, run: &Value) -> Result<(Value, Value)> {
        let reference = &run["definition_ref"];
        let current = self.current_definition_id(id)?;
        if reference["status"] == "pinned" {
            let archive_id = text(&reference["id"]).to_owned();
            let (guide, manifest, contract) = if let Some(entry) = self.archived_guides.get(&archive_id) {
                (entry["guide"].clone(), entry["manifest"].clone(), entry["contract"].clone())
            } else {
                let payload = self.runs.archive(&archive_id).ok_or(
                    "Archived guide definition unavailable: this checklist is unverifiable",
                )?;
                let envelope = definition_archive::open(&archive_id, &payload)?;
                if envelope["guide_id"] != id {
                    return Err("Archived guide definition belongs to another guide".into());
                }
                let guide = Self::revision(envelope["definition"].clone())?;
                if self.archived_guides.len() >= 64 {
                    self.archived_guides.clear();
                }
                let manifest = envelope["occurrence_manifest"].clone();
                let contract = envelope["compiler_contract"].clone();
                self.archived_guides.insert(
                    archive_id.clone(),
                    json!({"guide":guide,"manifest":manifest,"contract":contract}),
                );
                (guide, manifest, contract)
            };
            let info = json!({"status":"pinned","id":archive_id,"run_id":run["run_id"],
                "compiler_contract":contract,
                "current_definition_id":current,
                "newer_definition_available":current.as_ref().is_some_and(|c| *c != archive_id),
                "manifest":if manifest.is_null() {"unavailable"} else {"bound"},
                "operations":manifest["operations"].as_array().map(Vec::len)});
            return Ok((guide, info));
        }
        if definition_archive::REGISTRY_COMPILER_CONTRACT != definition_archive::LEGACY_COMPILER_CONTRACT {
            return Err("Legacy checklist cannot be evaluated by this guide compiler: progress is unverifiable".into());
        }
        let guide = self.guides.get(id).cloned().ok_or("Guide not found")?;
        // Detection only: undeclared saved keys and a changed current definition
        // are reported; nothing is migrated, dropped or reassigned.
        let undeclared = |field: &str, allowed: &HashSet<String>| {
            let mut keys: Vec<_> = run[field]
                .as_object()
                .into_iter()
                .flat_map(|m| m.keys())
                .filter(|k| !allowed.contains(*k))
                .cloned()
                .collect();
            keys.sort();
            keys
        };
        let observed = reference["observed_definition_id"].as_str();
        let info = json!({"status":"legacy_unresolved","run_id":run["run_id"],
            "reason":reference["reason"].as_str().unwrap_or(hoi4companion_storage::progress::LEGACY_REASON),
            "current_definition_id":current,"observed_definition_id":observed,
            "changed_since_observed":observed.map(|o| current.as_deref() != Some(o)),
            "undeclared_checks":undeclared("checks",&manual_keys(&guide)),
            "undeclared_started":undeclared("started",&started_keys(&guide)),
            // Legacy progress is keyed by positional step keys only; it is never
            // bound to operation identities of any (current or future) manifest.
            "manifest":"unbound","compiler_contract":definition_archive::REGISTRY_COMPILER_CONTRACT});
        Ok((guide, info))
    }
    /// Occurrence manifest of the run's own definition, with inactive shadow
    /// decomposition and, when a newer definition exists, the conservative
    /// correspondence a future explicit upgrade would be limited to.
    pub fn manifest(&mut self, id: Option<&str>, run: Option<&str>) -> Result<Value> {
        let id = self.identity(id)?;
        let progress = self.progress(Some(&id), run, None)?;
        let (definition, info) = self.definition_for_run(&id, &progress)?;
        let current = current_archive(&self.guides, &self.sources, self.association_directory.as_ref(), self.context.as_ref(), &mut self.current_archives, &id)?;
        let current_manifest = match &current {
            Some(archive) => serde_json::from_str::<Value>(&archive.payload)
                .map_err(|e| e.to_string())?["occurrence_manifest"]
                .clone(),
            None => Value::Null,
        };
        let manifest = if info["status"] == "pinned" {
            self.archived_guides
                .get(text(&info["id"]))
                .map(|e| e["manifest"].clone())
                .unwrap_or(Value::Null)
        } else {
            Value::Null
        };
        let shadow = if manifest.is_null() {
            Value::Null
        } else {
            crate::occurrence_manifest::shadow(&definition, &manifest)?
        };
        let correspondence = if !manifest.is_null()
            && !current_manifest.is_null()
            && info["newer_definition_available"] == true
        {
            crate::occurrence_manifest::correspondence(&manifest, &current_manifest)
        } else {
            Value::Null
        };
        Ok(json!({"schema_version":1,"guide_id":id,"run_id":progress["run_id"],"definition":info,
            "manifest":manifest,"shadow_decomposition":shadow,"correspondence_to_current":correspondence}))
    }
    fn storage(&self) -> Value {
        json!({"status":if self.persistent{"persistent"}else{"memory"},"error":""})
    }
    fn binding_context(&self) -> (u64, u64, u64, u64, u64) {
        (self.active_revision, self.definition_generation, self.revision,
            self.observation_revision, self.runs.generation)
    }
    fn bind(&mut self) -> Result<()> {
        // Reading progress/projections repeatedly must not observe the same
        // campaign again. Every committed run mutation and qualified observation
        // invalidates this stamp; failures never mark a binding as complete.
        if self.bound_context == Some(self.binding_context()) {
            return Ok(());
        }
        // Binding a campaign can create a checklist for every guide and pin it
        // to its current definition. While those are still compiling in the
        // background, wait: progress stays unverified meanwhile, and the read
        // loop keeps delivering observations instead of stalling.
        if self.background_archives && !self.cold_archive_ids()?.is_empty() {
            return Ok(());
        }
        let changed = if let Some(id) = campaign(&self.live) {
            let args = json!({"campaign_id":id,"country":self.live["sample"]["country"],"session_id":self.live["sample"]["session_id"],"tick":self.live["sample"]["native_tick"]});
            self.runs_call("bind_campaign", args)?
        } else {
            let state = state::normalize(&self.live);
            let args = json!({"session_id":if self.live["usable"]==true {state["session_id"].clone()}else{Value::Null}});
            self.runs_call("bind", args)?
        };
        if changed == true {
            self.revision += 1;
        }
        if self.runs.observe_decisions(&self.live)? {
            self.revision += 1;
        }
        if self.live["usable"] == true {
            let identity = state::normalize(&json!({"usable":self.live["usable"],"source":self.live["source"],"sample":{"country":self.live["sample"]["country"],"focus_tree_id":self.live["sample"]["focus_tree_id"]}}));
            let ids = self.runs.call("metadata", json!({}))?;
            for id in ids.as_object().into_iter().flat_map(|m| m.keys()) {
                if !self.guides.contains_key(id) {
                    continue;
                }
                // Other guides move onto their current definition when opened.
                if self.active.as_deref() == Some(id.as_str())
                    && let Err(error) = self.upgrade_run(id)
                {
                    eprintln!("Checklist update to the current guide failed for {id}: {error}");
                }
                // Observe against the run's own definition. An unverifiable
                // run (missing/corrupt archive) is not observed at all.
                let record = self.runs.call("get", json!({"guide_id":id}))?;
                // Determine admission from the run's immutable scope before
                // compiling current definitions for unrelated countries. A
                // cached scope is tied to the archive hash, never its title.
                let scope = if record["definition_ref"]["status"] == "pinned" {
                    let archive_id = text(&record["definition_ref"]["id"]);
                    if let Some(scope) = self.observation_scopes.get(archive_id) {
                        scope.clone()
                    } else {
                        let Some(payload) = self.runs.archive(archive_id) else {continue};
                        let Ok(envelope) = definition_archive::open(archive_id, &payload) else {continue};
                        if envelope["guide_id"] != *id {continue;}
                        let scope = envelope["definition"]["applies_to"].clone();
                        self.observation_scopes.insert(archive_id.to_owned(),scope.clone());
                        scope
                    }
                } else { self.guides[id]["applies_to"].clone() };
                if crate::guide_prepare::mode(&json!({"applies_to":scope}), &identity) != "live" {continue;}
                let Ok((guide, _)) = self.definition_for_run(id, &record) else {
                    continue;
                };
                let active = self.active.as_ref() == Some(id);
                let (guides, sources, memo, context, associations) =
                    (&self.guides, &self.sources, &mut self.current_archives, self.context.as_ref(), self.association_directory.as_ref());
                if self.runs.observe_with(
                    &guide,
                    &self.live,
                    active,
                    Some(&mut |g| current_archive(guides, sources, associations, context, memo, g)),
                )? {
                    self.revision += 1;
                }
            }
        }
        self.bound_context = Some(self.binding_context());
        Ok(())
    }
    pub fn accept(&mut self, value: Value) -> Result<Value> {
        // A successful fresh read need not rerun every guide or invalidate HUD
        // targets when only delivery timestamps and diagnostic counters changed.
        // All game data, identity, validity and unknown fields remain significant.
        let changed = !same_observation(&self.live, &value);
        self.live = value;
        if changed {
            self.observation_revision = self.observation_revision.wrapping_add(1);
        }
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
        self.runs_call("ensure", json!({"guide_id":id,"checks":{}}))?;
        if let Err(error) = self.upgrade_run(id) {
            eprintln!("Checklist update to the current guide failed for {id}: {error}");
        }
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
        // Definition CAS: null means a legacy (unpinned) run.
        if update
            .get("expected_definition_id")
            .is_some_and(|v| *v != current["definition_ref"]["id"])
        {
            return Err("Guide definition changed: reload progress".into());
        }
        for key in ["server_id", "expected_run_id", "expected_revision", "expected_definition_id"] {
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
                self.runs_call(
                    "create",
                    json!({"guide_id":id,"session_id":current["session_id"]}),
                )?;
            } else if map.len() == 1 && map.contains_key("run_id") {
                self.runs_call("select", json!({"guide_id":id,"run_id":update["run_id"]}))?;
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
                // Keys are validated against the run's own definition.
                let (definition, _) = self.definition_for_run(&id, &current)?;
                let allowed = manual_keys(&definition);
                if checks
                    .as_object()
                    .is_none_or(|m| m.keys().any(|k| !allowed.contains(k)))
                {
                    return Err("Checks not declared by the guide".into());
                }
                let started = update.get("started").unwrap_or(&current["started"]);
                let operations = started_keys(&definition);
                if started
                    .as_object()
                    .is_none_or(|m| m.keys().any(|k| !operations.contains(k.as_str())))
                {
                    return Err("Starts not declared by the guide".into());
                }
                let args = json!({"guide_id":id,"checks":checks,"started":started});
                self.runs_call("update", args)?;
            } else {
                return Err("Unknown run operation".into());
            }
            self.revision += 1;
        }
        let mut current = self.runs.call("get", json!({"guide_id":id,"run_id":run,"include_decision_history":true}))?;
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
        let (definition, info) = self.definition_for_run(&id, &progress)?;
        let guide = self.project_definition(&definition, &state, &progress)?;
        Ok(
            json!({"schema_version":1,"engine":"rust-guide-core","server_id":self.server,"guide_id":id,"definition_revision":guide["runtime_definition_revision"],"definition":info,"guide":guide,"state":state,"progress":progress,"evaluation":guide_runtime::evaluate(&guide,&state,&progress)?}),
        )
    }
    fn project_definition(&self, guide: &Value, state: &Value, progress: &Value) -> Result<Value> {
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
        let (definition, _) = self.definition_for_run(&id, &progress)?;
        self.project_definition(&definition, &self.live, &progress)
    }
    pub fn checks(&mut self, id: Option<&str>, update: Option<&Value>) -> Result<Value> {
        let id = self.identity(id)?;
        self.ensure(&id)?;
        let current = self.runs.call("get", json!({"guide_id":id}))?;
        if let Some(update) = update {
            self.guard(&json!({"checks":{}}), &current)?;
            let allowed = manual_keys(&self.definition_for_run(&id, &current)?.0);
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
            self.runs_call("update",json!({"guide_id":id,"checks":checks,"started":if *update==json!({"reset":true}){json!({})}else{current["started"].clone()}}))?;
            self.revision += 1;
        }
        Ok(
            json!({"checks":self.runs.call("get",json!({"guide_id":id}))?["checks"],"revision":self.revision}),
        )
    }
    pub fn request(&mut self, op: &str, args: &Value) -> Result<Value> {
        match op {
        "app_live_packet"=>self.live_packet(),
        // The display-cadence HUD needs identity, not every campaign's stored
        // checklist/history. Keep all qualification witnesses, without DB reads.
        "app_hud_context"=>Ok(json!({"usable":self.live["usable"],"status":self.live["status"],"source":self.live["source"],"sample":{"country":self.live["sample"]["country"],"focus_tree_id":self.live["sample"]["focus_tree_id"],"session_id":self.live["sample"]["session_id"]}})),
        "app_overlay_packet"=>{let id=self.identity(None)?;let packet=self.projection(Some(&id),true)?;Ok(packet)},
        "app_overlay_stamp"=>Ok(json!({"server":self.server,"guide":self.active,"selection":self.active_revision,"definition":self.definition_generation,"progress":self.revision,"observation":self.observation_revision})),
        "app_state"=>self.accept(args["value"].clone()),
        "app_active"=>self.active(args.get("update")),
        "app_progress"=>self.progress(args["guide_id"].as_str(),args["run_id"].as_str(),args.get("update")),
        "app_checks"=>self.checks(args["guide_id"].as_str(),args.get("update")),
        "app_projection"=>self.projection(args["guide_id"].as_str(),args["live"].as_bool().unwrap_or(true)),
        "app_guide"=>self.guide(args["guide_id"].as_str()),
        "app_manifest"=>self.manifest(args["guide_id"].as_str(),args["run_id"].as_str()),
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

#[cfg(test)]
mod observation_tests {
    use super::*;
    #[test]
    fn foreign_archived_runs_do_not_compile_current_definitions_on_live_delivery() {
        let make=|id:&str,country:&str|json!({"id":id,"applies_to":{"country":country},"tracks":{"operations":[{"key":"note","action":{"kind":"manual"}}]}});
        let mut app=Application::open(&json!({"guides":[make("foreign","FIN"),make("current","SOV")]})).unwrap();
        app.progress(Some("foreign"),None,None).unwrap();
        app.active(Some(&json!({"guide_id":"current"}))).unwrap();
        app.progress(Some("current"),None,None).unwrap();
        app.current_archives.clear();
        app.accept(json!({"usable":true,"source":"process_memory","sample":{"country":"SOV","session_id":"one","country_overview":{"country":"SOV"}}})).unwrap();
        assert!(!app.current_archives.contains_key("foreign"));
        assert!(app.current_archives.contains_key("current"));
    }
    #[test]
    fn progress_reads_reuse_binding_but_run_guide_and_live_changes_invalidate_it() {
        let make = |id: &str| json!({"id":id,"tracks":{"operations":[{"key":"note","action":{"kind":"manual"}}]}});
        let mut app = Application::open(&json!({"guides":[make("one"),make("two")]})).unwrap();
        let live = json!({"usable":false,"status":"disconnected","sample":null});
        app.accept(live.clone()).unwrap();
        app.progress(Some("one"),None,None).unwrap();
        let bound = app.bound_context.unwrap();
        for _ in 0..3 {
            app.progress(Some("one"),None,None).unwrap();
            app.accept(live.clone()).unwrap();
            assert_eq!(app.bound_context,Some(bound));
        }
        app.runs.call("update",json!({"guide_id":"one","checks":{"operations:note":true}})).unwrap();
        assert_ne!(app.binding_context(),bound);
        app.accept(live.clone()).unwrap();
        assert_eq!(app.bound_context,Some(app.binding_context()));
        let bound = app.bound_context;
        app.active(Some(&json!({"guide_id":"two"}))).unwrap();
        app.accept(live.clone()).unwrap();
        assert_ne!(app.bound_context,bound);
        let bound = app.bound_context;
        app.put_guide(&make("two")).unwrap();
        app.accept(live.clone()).unwrap();
        assert_ne!(app.bound_context,bound);
        let bound = app.bound_context;
        app.accept(json!({"usable":false,"status":"stale","sample":null})).unwrap();
        assert_ne!(app.bound_context,bound);
    }
    #[test]
    fn hud_identity_is_qualified_without_serializing_campaign_progress() {
        let mut app=Application::open(&json!({})).unwrap();
        let catalog=json!([{"id":"hun","applies_to":{"country":"HUN","tree_id":"hungary"}}]);
        app.live=json!({"usable":true,"status":"observed","source":"process_memory","sample":{"country":"HUN","session_id":"one","focus_tree_id":"hungary","country_overview":{"large":"not needed"}}});
        let identity=app.request("app_hud_context",&json!({})).unwrap();
        assert!(identity["sample"]["country_overview"].is_null());
        assert!(identity["guide_runs"].is_null());
        assert_eq!(crate::hud_guides::select(&identity,&catalog),crate::hud_guides::select(&app.live,&catalog));
        for (key,value) in [("usable",json!(false)),("status",json!("stale")),("source",json!("snapshot"))] {
            let old=app.live[key].clone();app.live[key]=value;
            let identity=app.request("app_hud_context",&json!({})).unwrap();
            assert_eq!(crate::hud_guides::select(&identity,&catalog)["guides"],json!([]));app.live[key]=old;
        }
        app.live["sample"]["session_id"]=json!("");
        assert_eq!(crate::hud_guides::select(&app.request("app_hud_context",&json!({})).unwrap(),&catalog)["guides"],json!([]));
    }
    #[test]
    fn guide_purchase_cache_refreshes_after_import_and_replacement() {
        let make=|id:&str,country:&str,law:&str|json!({"id":id,"applies_to":{"country":country},"tracks":{"operations":[{"key":"buy","action":{"kind":"manual","params":{"category":"politics","law_id":law,"law_group":"trade_laws"}}}]}});
        let mut app=Application::open(&json!({"guides":[make("nor-guide","NOR","free_trade")]})).unwrap();
        assert_eq!(app.political_cost_ids().as_array().unwrap().len(),1);
        app.put_guide(&make("ger-guide","GER","limited_exports")).unwrap();
        assert_eq!(app.political_cost_ids().as_array().unwrap().len(),2);
        app.put_guide(&make("nor-guide","NOR","closed_economy")).unwrap();
        assert_eq!(app.political_cost_ids().as_array().unwrap().len(),2);
        assert!(!app.political_cost_ids().to_string().contains("free_trade"));
        assert!(app.political_cost_ids().to_string().contains("closed_economy"));
    }
    #[test]
    fn only_delivery_metadata_can_skip_revaluation() {
        let original=json!({"usable":true,"sample":{"country":"HUN","native_tick":1},"error":""});
        let mut delivery=original.clone();delivery["metrics"]=json!({"probes":2});delivery["observed_at"]=json!("later");
        assert!(same_observation(&original,&delivery));
        for (key,value) in [("usable",json!(false)),("sample",json!({"country":"LUX","native_tick":1})),("sample",json!({"country":"HUN","native_tick":2})),("error",json!("stale")),("validation",json!("rejected"))] {
            let mut changed=delivery.clone();changed[key]=value;assert!(!same_observation(&original,&changed));
        }
    }
}
