//! Deterministic campaign/run transitions. No process access, clocks or IDs
//! inferred from country/time; callers supply qualified identity and fresh IDs.
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::collections::HashSet;

type Object = Map<String, Value>;
type Result<T> = std::result::Result<T, String>;
fn object(v: &Value) -> Result<&Object> {
    v.as_object()
        .ok_or_else(|| "Archivio avanzamento non valido".into())
}
fn string(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn nonempty(v: &Value, maximum: Option<usize>, optional: bool) -> bool {
    optional && v.is_null()
        || v.as_str()
            .is_some_and(|s| !s.trim().is_empty() && maximum.is_none_or(|n| s.chars().count() <= n))
}
fn identifier(v: &Value, label: &str, optional: bool, maximum: Option<usize>) -> Result<()> {
    if nonempty(v, maximum, optional) {
        Ok(())
    } else {
        Err(format!("{label} non valido"))
    }
}
fn country(v: &Value) -> bool {
    v.as_str().is_some_and(|s| {
        s.len() == 3
            && s.bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    })
}
fn integer(v: &Value, minimum: u64) -> bool {
    v.as_u64()
        .is_some_and(|n| n >= minimum && n <= 9_007_199_254_740_991)
}
pub fn campaign(v: &Value) -> bool {
    let Some(s) = v.as_str() else { return false };
    if !s.is_ascii() || s.len() != 50 || !s.starts_with("hoi4:") || &s[41..42] != ":" {
        return false;
    }
    let uuid = &s[5..41];
    uuid.bytes().enumerate().all(|(i, b)| {
        if [8, 13, 18, 23].contains(&i) {
            b == b'-'
        } else {
            b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
        }
    }) && uuid.bytes().any(|b| b != b'0' && b != b'-')
        && s[42..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn checks(v: &Value) -> Result<()> {
    if v.as_object()
        .is_some_and(|m| m.values().all(Value::is_boolean))
    {
        Ok(())
    } else {
        Err("Conferme della partita non valide".into())
    }
}
fn baselines(v: &Value) -> Result<()> {
    let valid = v.as_object().is_some_and(|m| {
        m.len() <= 4096
            && m.iter().all(|(k, e)| {
                !k.is_empty()
                    && e.as_object().is_some_and(|x| {
                        x.len() == 5
                            && ["fact_key", "value", "country", "quantity", "source"]
                                .iter()
                                .all(|k| x.contains_key(*k))
                    })
                    && ["fact_key", "country", "source"]
                        .iter()
                        .all(|k| e[*k].as_str().is_some_and(|s| !s.is_empty()))
                    && integer(&e["value"], 0)
                    && integer(&e["quantity"], 1)
            })
    });
    if valid {
        Ok(())
    } else {
        Err("Conteggio iniziale della partita non valido".into())
    }
}
fn observations(v: &Value) -> Result<()> {
    let Some(m) = v.as_object() else {
        return Err("Osservazioni campagna non valide".into());
    };
    if m.is_empty() {
        return Ok(());
    }
    let valid = m
        .keys()
        .all(|k| ["country", "war_seen", "milestones"].contains(&k.as_str()))
        && country(&v["country"])
        && (!m.contains_key("war_seen") || v["war_seen"] == true)
        && (m.contains_key("war_seen")
            || v["milestones"].as_object().is_some_and(|m| !m.is_empty()))
        && (!m.contains_key("milestones")
            || v["milestones"].as_object().is_some_and(|m| {
                !m.is_empty()
                    && m.len() <= 4096
                    && m.iter()
                        .all(|(k, v)| !k.is_empty() && k.chars().count() <= 2048 && v == true)
            }));
    if valid {
        Ok(())
    } else {
        Err("Osservazioni campagna non valide".into())
    }
}
/// Legacy reason for runs whose original definition was never archived.
pub const LEGACY_REASON: &str = "original_definition_unavailable";
fn pinned_ref(v: &Value) -> bool {
    v.as_object().is_some_and(|m| m.len() == 3)
        && v["status"] == "pinned"
        && v["archive_format"] == 1
        && v["id"].as_str().is_some_and(crate::definition_id)
}
/// Typed, immutable run-to-definition binding. Absent = legacy run that has
/// not yet been written by a pinning-aware binary (still legacy_unresolved).
fn definition_ref(v: Option<&Value>) -> Result<()> {
    let Some(v) = v else { return Ok(()) };
    let legacy = v.as_object().is_some_and(|m| {
        m.keys().all(|k| ["status", "reason", "observed_definition_id"].contains(&k.as_str()))
    }) && v["status"] == "legacy_unresolved"
        && v["reason"].as_str().is_some_and(|s| !s.is_empty() && s.len() <= 128)
        && (v.get("observed_definition_id").is_none()
            || v["observed_definition_id"].as_str().is_some_and(crate::definition_id));
    if legacy || pinned_ref(v) {
        Ok(())
    } else {
        Err("Definizione della partita non valida".into())
    }
}
fn legacy_marker(reason: &str) -> Value {
    json!({"status":"legacy_unresolved","reason":reason})
}
fn validate_record(key: &str, row: &Value) -> Result<()> {
    if !row.is_object() || row["schema_version"].as_u64() != Some(1) || row["run_id"] != key {
        return Err("Identità avanzamento non valida".into());
    }
    definition_ref(row.get("definition_ref"))?;
    identifier(&json!(key), "Partita", false, Some(160))?;
    identifier(&row["guide_id"], "Guida", false, Some(160))?;
    identifier(&row["session_id"], "Sessione", true, None)?;
    checks(&row["checks"])?;
    checks(&row["started"])?;
    // Steps the player set aside (absent in runs written before it existed).
    if let Some(v) = row.get("set_aside") {
        checks(v)?;
    }
    // Campaign date of each confirmation (absent when it is unknown).
    if let Some(v) = row.get("checked_at")
        && !v.as_object().is_some_and(|m| m.len() <= 20000 && m.values().all(|t| t.as_u64().is_some()))
    {
        return Err("Date delle conferme non valide".into());
    }
    baselines(row.get("baselines").unwrap_or(&json!({})))?;
    observations(row.get("campaign_conditions").unwrap_or(&json!({})))?;
    for field in ["research_bindings", "research_history"] {
        if let Some(v) = row.get(field)
            && !v.as_object().is_some_and(|m| {
                m.len() <= 4096
                    && m.iter().all(|(k, v)| {
                        !k.is_empty()
                            && v.as_str()
                                .is_some_and(|s| !s.is_empty() && s.chars().count() <= 512)
                    })
            }) {
                return Err("Associazioni ricerca archiviate non valide".into());
            }
    }
    if let Some(v) = row.get("research_options")
        && !v.as_object().is_some_and(|m| {
            m.len() <= 4096
                && m.iter().all(|(k, v)| {
                    !k.is_empty()
                        && v.as_array().is_some_and(|a| {
                            !a.is_empty()
                                && a.len() <= 8
                                && a.iter().all(|v| {
                                    v.as_str()
                                        .is_some_and(|s| !s.is_empty() && s.chars().count() <= 512)
                                })
                                && a.iter().map(string).collect::<HashSet<_>>().len() == a.len()
                        })
                })
        }) {
            return Err("Alternative ricerca archiviate non valide".into());
        }
    if row
        .get("run_revision")
        .is_some_and(|v| v.as_u64().is_none())
    {
        return Err("Revisione avanzamento non valida".into());
    }
    if !row["campaign_id"].is_null() {
        if !campaign(&row["campaign_id"]) || !country(&row["country"]) {
            return Err("Campagna archiviata non valida".into());
        }
        if row["last_tick"].as_u64().is_none() {
            return Err("Data campagna non valida".into());
        }
    }
    Ok(())
}
fn campaign_key(a: &Value, b: &Value) -> String {
    json!([a, b]).to_string()
}

#[derive(Clone)]
pub struct State {
    runs: Object,
    active: Object,
    campaigns: Object,
    decision_history: Object,
    restored: HashSet<String>,
    binding: Option<[Value; 3]>,
    tick: Option<u64>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            runs: Object::new(),
            active: Object::new(),
            campaigns: Object::new(),
            decision_history: Object::new(),
            restored: HashSet::new(),
            binding: None,
            tick: None,
        }
    }
}
impl State {
    fn snapshot(&self) -> Value {
        let mut value=json!({"schema_version":1,"runs":self.runs,"active":self.active,"campaign_active":self.campaigns});
        if !self.decision_history.is_empty() {value["decision_history"]=json!(self.decision_history);}
        value
    }
    fn load(&mut self, v: Value) -> Result<()> {
        if !v.as_object().is_some_and(|m| {
            (m.len() == 4 || m.len()==5 && m.contains_key("decision_history"))
                && ["schema_version", "runs", "active", "campaign_active"]
                    .iter()
                    .all(|k| m.contains_key(*k))
        }) || v["schema_version"].as_u64() != Some(1)
        {
            return Err("Formato archivio avanzamento non supportato".into());
        }
        let runs = object(&v["runs"])?;
        let active = object(&v["active"])?;
        let campaigns = object(&v["campaign_active"])?;
        let histories=v.get("decision_history").cloned().unwrap_or_else(||json!({}));
        let histories=object(&histories)?;
        if histories.len()>10000 {return Err("Archivio storico decisioni non valido".into());}
        for (key,history) in histories {
            crate::decision_history::validate(history)?;
            if key!=&campaign_key(&history["campaign_id"],&history["country"]) {return Err("Identità storico decisioni non valida".into());}
        }
        if runs.len() > 10000 {
            return Err("Archivio avanzamento non valido".into());
        }
        for (k, row) in runs {
            validate_record(k, row)?
        }
        for (guide, key) in active {
            if !runs
                .get(string(key))
                .is_some_and(|r| r["guide_id"] == guide.as_str())
            {
                return Err("Partita attiva non valida".into());
            }
        }
        for (guide, bindings) in campaigns {
            for (identity, key) in object(bindings)? {
                let row = runs
                    .get(string(key))
                    .ok_or("Associazione campagna non valida")?;
                if row["guide_id"] != guide.as_str()
                    || identity != &campaign_key(&row["campaign_id"], &row["country"])
                {
                    return Err("Associazione campagna non valida".into());
                }
            }
        }
        self.runs = runs.clone();
        self.active = active.clone();
        self.campaigns = campaigns.clone();
        self.decision_history=histories.clone();
        self.restored = self.runs.keys().cloned().collect();
        for row in self.runs.values_mut() {
            if row["campaign_id"].is_null() {
                row["campaign_required"] = json!(true)
            }
        }
        Ok(())
    }
    fn get(&self, guide: &Value, key: Option<&Value>) -> Result<Value> {
        identifier(guide, "Identità guida", false, None)?;
        let key = key
            .filter(|k| !k.is_null())
            .unwrap_or_else(|| self.active.get(string(guide)).unwrap_or(&Value::Null));
        self.runs
            .get(string(key))
            .filter(|r| r["guide_id"] == *guide)
            .cloned()
            .ok_or_else(|| "Partita della guida non trovata".into())
    }
    /// Dates each newly confirmed step with the current campaign date when
    /// the row belongs to the bound campaign; keeps earlier dates and drops
    /// the dates of steps no longer confirmed.
    fn date_confirmations(&self, row: &mut Value, previous: &Value) {
        let bound = self.binding.as_ref().is_some_and(|b| row["campaign_id"] == b[0] && row["country"] == b[1]);
        let mut dates = row.get("checked_at").and_then(Value::as_object).cloned().unwrap_or_default();
        dates.retain(|k, _| row["checks"][k.as_str()] == true);
        if let (true, Some(tick), Some(checks)) = (bound, self.tick, row["checks"].as_object()) {
            for (k, v) in checks {
                if *v == true && previous[k.as_str()] != true && !dates.contains_key(k) {
                    dates.insert(k.clone(), json!(tick));
                }
            }
        }
        if let Some(m) = row.as_object_mut() {
            if dates.is_empty() {
                m.remove("checked_at");
            } else {
                m.insert("checked_at".into(), Value::Object(dates));
            }
        }
    }
    fn insert(&mut self, row: Value) {
        let guide = string(&row["guide_id"]).to_owned();
        let key = string(&row["run_id"]).to_owned();
        self.active.insert(guide.clone(), json!(key));
        if !row["campaign_id"].is_null() {
            self.campaigns.entry(guide).or_insert_with(|| json!({}))
                [campaign_key(&row["campaign_id"], &row["country"])] = json!(key);
        }
        self.runs.insert(key, row);
    }
    fn new_run(
        &self,
        guide: &Value,
        session: &Value,
        checks: Value,
        ids: &mut impl Iterator<Item = Value>,
    ) -> Result<Value> {
        identifier(guide, "Identità guida", false, None)?;
        identifier(session, "Sessione", true, None)?;
        let id = ids.next().ok_or("__needs_id__")?;
        identifier(&id, "Identità partita", false, Some(160))?;
        if self.runs.contains_key(string(&id)) {
            return Err("Identità partita duplicata".into());
        }
        Ok(
            json!({"schema_version":1,"guide_id":guide,"run_id":id,"session_id":session,"checks":checks,"started":{},"run_revision":0}),
        )
    }
    /// Operations that only read the state, or None for every other one.
    /// `Store::request` answers them without copying the whole state.
    fn read(&self, op: &str, args: &Value) -> Option<Result<Value>> {
        let guide = &args["guide_id"];
        Some(match op {
            "snapshot" => Ok(self.snapshot()),
            "records" => Ok(json!(self.runs)),
            "decision_history" => Ok(self.decision_history.get(&campaign_key(&args["campaign_id"],&args["country"])).cloned().unwrap_or(Value::Null)),
            "get" => self.get(guide, args.get("run_id")),
            "history" => Ok(Value::Array(
                self.runs
                    .values()
                    .filter(|r| r["guide_id"] == *guide)
                    .map(metadata)
                    .collect(),
            )),
            "metadata" => Ok(Value::Object(
                self.active
                    .iter()
                    .map(|(g, k)| (g.clone(), metadata(&self.runs[string(k)])))
                    .collect(),
            )),
            _ => return None,
        })
    }
    fn raw(
        &mut self,
        op: &str,
        args: &Value,
        ids: &mut impl Iterator<Item = Value>,
    ) -> Result<Value> {
        if let Some(value) = self.read(op, args) {
            return value;
        }
        let guide = &args["guide_id"];
        let session = &args["session_id"];
        match op {
            "observe_decisions" => {
                let Some(binding)=self.binding.as_ref() else {return Ok(json!(false));};
                let tick=self.tick.ok_or("Data campagna non qualificata")?;
                let key=campaign_key(&binding[0],&binding[1]);
                let history=self.decision_history.get_mut(&key).ok_or("Storico campagna non associato")?;
                let state=&args["state"];
                let before=history.clone();
                if state["facts"]["campaign.id"]["status"]!="observed" || state["facts"]["campaign.id"]["value"]!=binding[0]
                    || state["facts"]["country"]["value"]!=binding[1] || state["session_id"]!=binding[2] || state["usable"]!=true {
                    crate::decision_history::interrupt(history);return Ok(json!(*history!=before));
                }
                crate::decision_history::observe(history,state,tick)?;
                Ok(json!(*history!=before))
            }
            "load" => {
                self.load(args["snapshot"].clone())?;
                Ok(Value::Null)
            }
            "ensure" => {
                identifier(guide, "Identità guida", false, None)?;
                if self.active.contains_key(string(guide)) {
                    return self.get(guide, None);
                }
                let checks = if args["checks"].is_null() {
                    json!({})
                } else {
                    Value::Object(
                        args["checks"]
                            .as_object()
                            .ok_or("Conferme legacy non valide")?
                            .iter()
                            .filter(|(_, v)| v.is_boolean())
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect(),
                    )
                };
                let imported = checks.as_object().is_some_and(|m| !m.is_empty());
                let mut row = self.new_run(guide, &Value::Null, checks, ids)?;
                if imported {
                    // Checks from an older format never belong to today's definition.
                    row["definition_ref"] = legacy_marker(LEGACY_REASON);
                }
                self.insert(row.clone());
                Ok(row)
            }
            "create" => {
                let mut row = self.new_run(guide, session, json!({}), ids)?;
                if let Ok(old) = self.get(guide, None) {
                    for k in ["campaign_id", "country", "last_tick"] {
                        if let Some(v) = old.get(k) {
                            row[k] = v.clone()
                        }
                    }
                }
                self.insert(row.clone());
                Ok(row)
            }
            "select" => {
                let mut row = self.get(guide, args.get("run_id"))?;
                let current = self.get(guide, None)?;
                if !current["campaign_id"].is_null()
                    && (row["campaign_id"] != current["campaign_id"]
                        || row["country"] != current["country"])
                {
                    return Err("Questa checklist appartiene a un’altra campagna o nazione".into());
                }
                if !row["campaign_id"].is_null()
                    && self
                        .tick
                        .is_some_and(|t| row["last_tick"].as_u64().unwrap_or(0) > t)
                {
                    return Err(
                        "Questa checklist contiene avanzamento successivo alla partita caricata"
                            .into(),
                    );
                }
                if !row["campaign_id"].is_null() {
                    row["session_id"] = current["session_id"].clone()
                }
                self.insert(row.clone());
                Ok(row)
            }
            "update" => {
                checks(&args["checks"])?;
                if !args["started"].is_null() {
                    checks(&args["started"])?
                }
                if !args["set_aside"].is_null() {
                    checks(&args["set_aside"])?
                }
                let mut row = self.get(guide, None)?;
                if !args["started"].is_null() {
                    row["started"] = args["started"].clone()
                }
                if !args["set_aside"].is_null() {
                    row["set_aside"] = args["set_aside"].clone()
                }
                let previous = std::mem::replace(&mut row["checks"], args["checks"].clone());
                self.date_confirmations(&mut row, &previous);
                self.insert(row.clone());
                Ok(row)
            }
            // Moves the active run onto the current guide definition with its
            // progress already translated by the caller. The previous binding
            // and progress stay in the row (last 8) so nothing is lost.
            "upgrade" => {
                if !pinned_ref(&args["definition"]) {
                    return Err("Definizione corrente non valida".into());
                }
                checks(&args["checks"])?;
                checks(&args["started"])?;
                let mut row = self.get(guide, None)?;
                if row["run_id"] != args["run_id"] {
                    return Err("Partita della guida non trovata".into());
                }
                if row["definition_ref"] == args["definition"] {
                    return Ok(row);
                }
                let mut previous = row
                    .get("previous_definitions")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                previous.push(json!({"definition_ref":row.get("definition_ref").cloned().unwrap_or_else(|| legacy_marker(LEGACY_REASON)),
                    "checks":row["checks"],"started":row["started"]}));
                if previous.len() > 8 {
                    previous.drain(..previous.len() - 8);
                }
                row["previous_definitions"] = Value::Array(previous);
                row["definition_ref"] = args["definition"].clone();
                row["checks"] = args["checks"].clone();
                row["started"] = args["started"].clone();
                // Translated keys have no known date; only kept keys keep theirs.
                let unchanged = row["checks"].clone();
                self.date_confirmations(&mut row, &unchanged);
                self.insert(row.clone());
                Ok(row)
            }
            "bind" => {
                if session.is_null() {
                    return Ok(json!(false));
                }
                identifier(session, "Sessione", false, None)?;
                let mut changed = false;
                for (g, k) in self.active.clone() {
                    let mut row = self.runs[string(&k)].clone();
                    if !row["campaign_id"].is_null() || row["session_id"] == *session {
                        continue;
                    }
                    if row["session_id"].is_null() {
                        row["session_id"] = session.clone()
                    } else {
                        row = self.new_run(&json!(g), session, json!({}), ids)?
                    }
                    self.insert(row);
                    changed = true;
                }
                Ok(json!(changed))
            }
            "bind_campaign" => {
                let a = &args["campaign_id"];
                let c = &args["country"];
                let tick = args["tick"]
                    .as_u64()
                    .ok_or("Data campagna non qualificata")?;
                if !campaign(a) || !country(c) {
                    return Err("Identità campagna non qualificata".into());
                }
                identifier(session, "Sessione", false, None)?;
                let binding = [a.clone(), c.clone(), session.clone()];
                if let Some(previous)=&self.binding && (previous[0]!=*a || previous[1]!=*c)
                    && let Some(history)=self.decision_history.get_mut(&campaign_key(&previous[0],&previous[1])) {crate::decision_history::interrupt(history);}
                let index = campaign_key(a, c);
                let history=self.decision_history.entry(index.clone()).or_insert_with(||crate::decision_history::empty(a,c,session,tick));
                if tick<history["last_tick"].as_u64().unwrap_or(tick) || self.binding.as_ref().is_some_and(|b|b[0]==*a && b[1]==*c) && self.tick.is_some_and(|last|tick<last) {
                    *history=crate::decision_history::empty(a,c,session,tick);
                } else if history["session_id"]!=*session {
                    history["session_id"]=session.clone();
                    history["coverage"]["gaps_observed"]=json!(true);
                    for row in history["entries"].as_object_mut().unwrap().values_mut() {
                        row["armed"]=json!(false); row["completion_pending"]=json!(false); row["phase"]=json!("unknown");
                    }
                }
                if self.binding.as_ref() == Some(&binding)
                    && self.tick.is_some_and(|t| tick >= t)
                    && self.active.values().all(|k| {
                        self.runs[string(k)]["campaign_id"] == *a
                            && self.runs[string(k)]["country"] == *c
                    })
                {
                    self.tick = Some(tick);
                    return Ok(json!(false));
                }
                let mut changed = false;
                for (g, k) in self.active.clone() {
                    let current = self.runs[string(&k)].clone();
                    let mut row = if let Some(key) = self
                        .campaigns
                        .get(&g)
                        .and_then(|v| v.get(&index))
                        .and_then(Value::as_str)
                    {
                        self.runs[key].clone()
                    } else if current["campaign_id"].is_null()
                        && !self.restored.contains(string(&current["run_id"]))
                        && (current["session_id"].is_null() || current["session_id"] == *session)
                    {
                        current.clone()
                    } else {
                        self.new_run(&json!(g), session, json!({}), ids)?
                    };
                    let last = if self
                        .binding
                        .as_ref()
                        .is_some_and(|b| b[0] == *a && b[1] == *c)
                    {
                        self.tick
                    } else {
                        row["last_tick"].as_u64()
                    };
                    if last.is_some_and(|last| tick < last) {
                        let mut history = row
                            .get("research_history")
                            .and_then(Value::as_object)
                            .cloned()
                            .unwrap_or_default();
                        if let Some(bindings) = row["research_bindings"].as_object() {
                            for (k, v) in bindings {
                                history.entry(k.clone()).or_insert_with(|| v.clone());
                            }
                        }
                        // A rewind continues the same playthrough: it keeps the
                        // old run's definition (or its legacy status), never the
                        // current one.
                        let inherited = row
                            .get("definition_ref")
                            .cloned()
                            .unwrap_or_else(|| legacy_marker(LEGACY_REASON));
                        // Confirmations dated no later than the loaded date
                        // still hold in it; later or undated ones stay unknown.
                        // Set-aside steps are the player's choice, not game facts.
                        let dated: Object = row
                            .get("checked_at")
                            .and_then(Value::as_object)
                            .into_iter()
                            .flatten()
                            .filter(|(k, t)| row["checks"][k.as_str()] == true && t.as_u64().is_some_and(|t| t <= tick))
                            .map(|(k, t)| (k.clone(), t.clone()))
                            .collect();
                        let checks: Object = dated.keys().map(|k| (k.clone(), json!(true))).collect();
                        let aside = row.get("set_aside").cloned();
                        row = self.new_run(&json!(g), session, Value::Object(checks), ids)?;
                        if !dated.is_empty() {
                            row["checked_at"] = Value::Object(dated)
                        }
                        if let Some(aside) = aside.filter(|v| v.as_object().is_some_and(|m| !m.is_empty())) {
                            row["set_aside"] = aside
                        }
                        row["definition_ref"] = inherited;
                        row["branch_reason"] = json!("timeline_rewind");
                        if !history.is_empty() {
                            row["research_history"] = Value::Object(history)
                        }
                    }
                    row["campaign_id"] = a.clone();
                    row["country"] = c.clone();
                    row["session_id"] = session.clone();
                    row["last_tick"] = json!(tick);
                    changed |= row != current;
                    self.insert(row);
                }
                self.binding = Some(binding);
                self.tick = Some(tick);
                Ok(json!(changed))
            }
            "observe_replace" => {
                for (key, row) in object(&args["records"])? {
                    validate_record(key, row)?;
                    let old = self
                        .runs
                        .get(key)
                        .ok_or("Partita della guida non trovata")?;
                    for k in [
                        "guide_id",
                        "run_id",
                        "campaign_id",
                        "country",
                        "session_id",
                        "run_revision",
                        "definition_ref",
                    ] {
                        if row.get(k) != old.get(k) {
                            return Err("Identità osservazione avanzamento non valida".into());
                        }
                    }
                    self.runs.insert(key.clone(), row.clone());
                }
                Ok(args["value"].clone())
            }
            _ => Err("Operazione avanzamento non valida".into()),
        }
    }
}
fn metadata(row: &Value) -> Value {
    Value::Object(
        row.as_object()
            .expect("validated run")
            .iter()
            .filter(|(k, _)| !["checks", "checked_at", "started", "set_aside", "baselines", "previous_definitions"].contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
    )
}

#[derive(Deserialize)]
pub struct Request {
    pub schema_version: u32,
    pub op: String,
    #[serde(default)]
    pub args: Value,
    #[serde(default)]
    pub ids: Vec<Value>,
}
#[derive(Default)]
pub struct Store {
    state: State,
    pending: Option<State>,
}
impl Store {
    pub fn request(&mut self, r: Request) -> Result<Value> {
        if r.schema_version != 1 || r.ids.len() > 20000 {
            return Err("Versione avanzamento non supportata".into());
        }
        if r.op == "accept" {
            self.pending = None;
            return Ok(Value::Null);
        }
        if r.op == "rollback" {
            if let Some(state) = self.pending.take() {
                self.state = state
            }
            return Ok(Value::Null);
        }
        if self.pending.is_some() {
            return Err("Transazione avanzamento non confermata".into());
        }
        // Reads change nothing: answer them from the state as it is. Copying
        // the whole state (thousands of runs) for each one cost ~8 ms a call.
        if let Some(value) = self.state.read(&r.op, &r.args) {
            return Ok(json!({"changed":false,"value":value?}));
        }
        let mut next = self.state.clone();
        let mut value = match next.raw(&r.op, &r.args, &mut r.ids.into_iter()) {
            Err(e) if e == "__needs_id__" => return Ok(json!({"needs_id":true})),
            result => result?,
        };
        if r.op == "load" {
            self.state = next;
            return Ok(json!({"changed":false,"value":value}));
        }
        // Bind every written run to a definition before commit. New runs take
        // the caller's current definition (rewinds already inherited theirs);
        // legacy runs only record which current definition was first observed,
        // as evidence, never as their recovered original. A missing entry asks
        // the caller; an explicit null leaves the run unbound. Callers that send
        // no `definitions` object (older ABI users) keep the previous behavior.
        let aware = r.args.get("definitions").is_some_and(Value::is_object);
        for (key, row) in next.runs.iter_mut().filter(|_| aware) {
            let old = self.state.runs.get(key);
            if old == Some(&*row) {
                continue;
            }
            let current = row.get("definition_ref");
            let legacy_unobserved = current.is_some_and(|v| {
                v["status"] == "legacy_unresolved" && v.get("observed_definition_id").is_none()
            });
            if current.is_some() && !legacy_unobserved {
                continue;
            }
            let guide = string(&row["guide_id"]).to_owned();
            let Some(supplied) = r.args.get("definitions").and_then(|d| d.get(&guide)) else {
                return Ok(json!({"needs_definition":guide}));
            };
            if supplied.is_null() {
                continue;
            }
            if !pinned_ref(supplied) {
                return Err("Definizione corrente non valida".into());
            }
            if legacy_unobserved {
                row["definition_ref"]["observed_definition_id"] = supplied["id"].clone();
            } else if old.is_none() {
                row["definition_ref"] = supplied.clone();
            } else {
                row["definition_ref"] = legacy_marker(LEGACY_REASON);
                row["definition_ref"]["observed_definition_id"] = supplied["id"].clone();
            }
        }
        let changed = next.runs != self.state.runs
            || next.active != self.state.active
            || next.campaigns != self.state.campaigns
            || next.decision_history != self.state.decision_history;
        if changed {
            for (key, row) in &mut next.runs {
                let old = self.state.runs.get(key);
                if old != Some(row) {
                    row["run_revision"] = json!(match old {
                        Some(v) => v["run_revision"]
                            .as_u64()
                            .unwrap_or(0)
                            .checked_add(1)
                            .ok_or("Revisione avanzamento oltre limite")?,
                        None => 0,
                    });
                    if ["update", "create", "select", "observe_replace"].contains(&r.op.as_str())
                        && self.state.binding.as_ref().is_some_and(|b| {
                            row["campaign_id"] == b[0]
                                && row["country"] == b[1]
                                && row["session_id"] == b[2]
                        })
                    {
                        row["last_tick"] = json!(self.state.tick)
                    }
                }
            }
            if let Some(id) = value.get("run_id").and_then(Value::as_str)
                && let Some(row) = next.runs.get(id) {
                    value = row.clone()
                }
            self.pending = Some(std::mem::replace(&mut self.state, next));
        } else {
            self.state = next;
        }
        Ok(if changed {
            json!({"changed":true,"snapshot":self.state.snapshot(),"value":value})
        } else {
            json!({"changed":false,"value":value})
        })
    }
}
