//! Membership and start/completion observations, without inferred eligibility.
use super::{Reader, Result};
use crate::live::Memory;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
fn profiles() -> &'static Value {
    static P: OnceLock<Value> = OnceLock::new();
    P.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../src/hoi4companion/country-functions-v1.json"
        ))
        .expect("bundled action profiles")
    })
}
struct BreakthroughTree {
    head: u64,
    count: usize,
    seen: HashSet<u64>,
    rows: Vec<Value>,
}
impl<M: Memory> Reader<'_, M> {
    fn action_signatures(&mut self, key: &str, error: &str) -> Result<()> {
        let resolved = if self.attributes["layout_id"] == "hoi4-x64-abi-v1" {
            &self.attributes["abi_action_signatures"][key]
        } else {
            &profiles()[key]
        };
        for row in resolved.as_array().ok_or("Missing action signatures")? {
            let text = row[1].as_str().ok_or("Invalid action signature")?;
            let expected = (0..text.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("bundled hex"))
                .collect::<Vec<_>>();
            if self
                .d
                .memory
                .executable_bytes(row[0].as_u64().unwrap(), expected.len())?
                != expected
            {
                return Err(error.into());
            }
        }
        Ok(())
    }
    fn ideas(&mut self) -> Result<Value> {
        let current = self.attributes["sha256"] == profiles()["sha256"]
            || self.attributes["layout_id"] == "hoi4-x64-abi-v1";
        if current {
            self.action_signatures(
                "ideas",
                "Idea membership signature does not match the executable.",
            )?
        }
        let manager = self.object_pointer(self.country + 0xf90, "CPoliticalStatus")?;
        if current && self.pointer(manager + 0xc8)? != self.country {
            return Err("Idea membership belongs to another country.".into());
        }
        let mut rows = vec![];
        let mut identities = HashSet::new();
        let mut law_groups = HashSet::new();
        for idea in self.array(manager + 0x50, 4096)? {
            self.typed(idea, "CIdea")?;
            let key = self.d.string(idea + 0x18)?;
            let group = self.object_pointer(idea + 0xaa0, "CIdeaGroupType")?;
            let category = self.object_pointer(group + 0x60, "CIdeaCategory")?;
            let group_id = self.d.string(group + 0x38)?;
            let category_id = self.d.string(category + 0x10)?;
            let flag = self.d.read(group + 0x58, 1)?[0];
            if flag > 1 || !identities.insert(key.clone()) {
                return Err("Identità o flag legge non valido".into());
            }
            if flag == 1 && !law_groups.insert(group_id.clone()) {
                return Err("Più leggi selezionate nello stesso gruppo".into());
            }
            rows.push(json!({"id":key,"group":group_id,"category":category_id,"is_law":flag==1}));
        }
        Ok(json!({"status":"observed","items":rows}))
    }
    fn advisors(&mut self) -> Result<Value> {
        if self.attributes["sha256"] == profiles()["sha256"]
            || self.attributes["layout_id"] == "hoi4-x64-abi-v1"
        {
            self.action_signatures(
                "advisors",
                "Active advisor membership signature does not match the executable.",
            )?
        }
        let manager = self.object_pointer(self.country + 0xff0, "CCountryCharacters")?;
        if self.pointer(manager + 8)? != self.country {
            return Err("Incarichi di un’altra nazione".into());
        }
        let mut rows = vec![];
        let mut identities = HashSet::new();
        for role in self.array(manager + 0x58, 4096)? {
            self.typed(role, "CAdvisor")?;
            let character = self.object_pointer(role + 0x10, "CCharacter")?;
            if self.integer(character + 0x68, 0, 8191)? != self.tag as i64 {
                return Err("Personaggio di un’altra nazione".into());
            }
            let key = self.d.string(role + 0x30)?;
            // CCharacter stores a display name or localization key here, not
            // necessarily an identifier. Appointment identity comes from CAdvisor.
            let character_name = self.name(character + 0x48)?;
            if character_name.is_empty() || character_name.chars().any(char::is_control) {
                return Err("Invalid advisor character label".into());
            }
            if !identities.insert((key.clone(), character)) {
                return Err("Identità incarico duplicata".into());
            }
            let mut row = json!({"id":key,"character_id":null,"slot":self.integer(role+0x28,0,2_000_000_000)?});
            if character_name.len() <= 160 && character_name.bytes().all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c)) {
                row["character_id"] = json!(character_name);
            } else {
                row["character_name"] = json!(character_name);
                row["character_identity_status"] = json!("unavailable");
            }
            rows.push(row);
        }
        Ok(json!({"status":"observed","items":rows}))
    }
    fn characters(&mut self) -> Result<Value> {
        self.function_signatures("characters", "Character role state is not qualified")?;
        if !self.attributes["abi_functions"]["characters"].as_array().is_some_and(|s| s.len() == 3) {
            return Err("Character serializer evidence is incomplete".into());
        }
        let manager = self.object_pointer(self.country + 0xff0, "CCountryCharacters")?;
        if self.pointer(manager + 8)? != self.country { return Err("Character manager owner mismatch".into()); }
        let mut items = vec![];
        let mut identities = HashSet::new();
        for item in self.inline(manager + 0x10, 16, 4096)? {
            let character = self.object_pointer(item, "CCharacter")?;
            if self.integer(character + 0x68, 1, 8191)? != self.tag as i64 {
                return Err("Character role belongs to another country".into());
            }
            let token = self.integer(character + 0x18, 1, 1_000_000)?;
            let id = self.token_name(token)?;
            if !identities.insert(id.clone()) { return Err("Duplicate character role identity".into()); }
            let flags = self.track(item + 8, 4)?;
            if flags.iter().any(|v| *v > 1) { return Err("Invalid character role flags".into()); }
            items.push(json!({"id":id,"name":self.name(character + 0x48)?,
                "country_leader":flags[0]==1,"advisor":flags[1]==1,"unit_leader":flags[2]==1,"scientist":flags[3]==1}));
        }
        Ok(json!({"status":"observed","source":"native_character_status_roles","items":items,
            "scope":"current_country_role_status_not_hiring_availability_or_appointment"}))
    }
    fn political_power(&mut self) -> Result<Value> {
        if self.attributes["layout_id"] == "hoi4-x64-abi-v1" {
            self.action_signatures("power", "Political power code is not qualified")?;
        }
        let manager = self.object_pointer(self.country + 0xf90, "CPoliticalStatus")?;
        if self.attributes["layout_id"] == "hoi4-x64-abi-v1"
            && self.pointer(manager + 0xc8)? != self.country
        {
            return Err("Political power belongs to another country".into());
        }
        let amount = i64::from_le_bytes(
            self.track(
                manager
                    + self.attributes["political_power_offset"]
                        .as_u64()
                        .unwrap_or(0xe0),
                8,
            )?
            .try_into()
            .unwrap(),
        );
        if !(-100_000_000_000..=100_000_000_000).contains(&amount) {
            return Err("Potere politico fuori intervallo".into());
        }
        Ok(json!({"status":"observed","available":amount as f64/100000.}))
    }
    pub(super) fn politics(&mut self) -> Result<Value> {
        let mut result = json!({});
        let mut observed = 0;
        result["characters"] = match self.characters() {
            Ok(value) => value,
            Err(reason) => json!({"status":"unavailable","reason":reason}),
        };
        for key in ["ideas", "advisors", "power"] {
            let build = self.attributes["sha256"].clone();
            let value = if build == profiles()["updated_sha256"] && key != "ideas"
                || (build == profiles()["sha256"]
                    && self.attributes["layout_id"] != "hoi4-x64-abi-v1"
                    || self.attributes["layout_id"] == "hoi4-x64-abi-v1"
                        && self.attributes["political_power_offset"].is_null())
                    && key == "power"
            {
                json!({"status":"unavailable","reason":"Sezione non ancora verificata per questa versione del gioco."})
            } else {
                let read = (|| {
                    if self.integer(self.country + 8, 0, 8191)? != self.tag as i64 {
                        return Err("Identità della nazione cambiata".into());
                    }
                    match key {
                        "ideas" => self.ideas(),
                        "advisors" => self.advisors(),
                        _ => self.political_power(),
                    }
                })();
                match read {
                    Ok(value) => value,
                    Err(reason) => json!({"status":"unavailable","reason":reason}),
                }
            };
            if value["status"] == "observed" {
                observed += 1
            }
            result[key] = value;
        }
        result["status"] = json!(if observed == 3 {
            "observed"
        } else if observed > 0 || result["characters"]["status"] == "observed" {
            "partial"
        } else {
            "unavailable"
        });
        result["reason"] = json!(
            "Idee e leggi selezionate; incarichi attivi del paese. Nessuna disponibilità o costo inferito."
        );
        Ok(result)
    }
    fn breakthrough_visit(
        &mut self,
        node: u64,
        parent: u64,
        bounds: (i64, i64),
        tree: &mut BreakthroughTree,
    ) -> Result<()> {
        let (lower, upper) = bounds;
        if node == tree.head {
            return Ok(());
        }
        if tree.seen.len() >= tree.count || !tree.seen.insert(node) {
            return Err("Albero scoperte ciclico o duplicato".into());
        }
        if self.track(node + 0x19, 1)? != [0] || self.pointer(node + 8)? != parent {
            return Err("Proprietà nodo scoperte incoerente".into());
        }
        let raw = self.track(node + 0x1c, 8)?;
        let token = i32::from_le_bytes(raw[..4].try_into().unwrap()) as i64;
        let progress = i32::from_le_bytes(raw[4..].try_into().unwrap());
        if !(lower < token && token < upper && (0..=1_000_000_000).contains(&progress)) {
            return Err("Punti scoperta fuori intervallo".into());
        }
        let key = self.token_name(token)?;
        let left = self.pointer(node)?;
        self.breakthrough_visit(left, node, (lower, token), tree)?;
        tree.rows.push(
            json!({"id":key,"available":progress/10000,"progress":(progress%10000) as f64/10000.}),
        );
        let right = self.pointer(node + 0x10)?;
        self.breakthrough_visit(right, node, (token, upper), tree)
    }
    fn breakthroughs(&mut self, status: u64) -> Result<Value> {
        self.typed(status + 0x50, "CBreakthroughProgress@NProject")?;
        let head = self.pointer(status + 0x58)?;
        let count = u64::from_le_bytes(self.track(status + 0x60, 8)?.try_into().unwrap());
        if count > 64 || self.track(head + 0x19, 1)? != [1] {
            return Err("Contatore scoperte non qualificato".into());
        }
        let root = self.pointer(head + 8)?;
        let mut tree = BreakthroughTree {
            head,
            count: count as usize,
            seen: HashSet::new(),
            rows: vec![],
        };
        self.breakthrough_visit(root, head, (-1, 1_000_001), &mut tree)?;
        let rows = tree.rows;
        if rows.len() != count as usize
            || rows
                .iter()
                .map(|v| v["id"].as_str())
                .collect::<HashSet<_>>()
                .len()
                != rows.len()
        {
            return Err("Elenco scoperte incompleto".into());
        }
        Ok(
            json!({"status":"observed","items":rows,"reason":"Punti spendibili separati dal progresso frazionario; contatore della nazione corrente."}),
        )
    }
    pub(super) fn projects(&mut self) -> Result<Value> {
        let status = self.object_pointer(self.country + 0xfa8, "CProgramStatus@NProject")?;
        if self.integer(status + 0x10, 0, 8191)? != self.tag as i64 {
            return Err("Programmi di ricerca di una nazione diversa".into());
        }
        let pool = self.object_pointer(status + 0x18, "CProjectPool@NProject")?;
        if self.integer(pool + 8, 0, 8191)? != self.tag as i64 {
            return Err("Progetti di una nazione diversa".into());
        }
        let mut programs = HashMap::new();
        for p in self.array(status + 0x20, 256)? {
            self.typed(p, "CProgram@NProject")?;
            if self.integer(p + 0x18, 0, 8191)? != self.tag as i64 {
                return Err("Impianto di un’altra nazione".into());
            }
            let handle = self.track(p + 8, 8)?;
            if handle[..4] == [0; 4] || handle[4..] == [0; 4] || programs.contains_key(&handle) {
                return Err("Identità impianto non valida".into());
            }
            // CProgram serialization emits +50 as an atom and +54 as its
            // optional-value flag, not an eight-byte project object handle.
            let selected = self.track(p + 0x54, 1)?[0];
            if selected > 1 { return Err("Invalid facility project selection flag".into()); }
            let token = self.integer(p + 0x50, 0, 1_000_000)?;
            programs.insert(handle, (self.d.string(p + 0x20)?, (token, selected == 1)));
        }
        let raw = self.track(pool + 0x10, 16)?;
        let data = u64::from_le_bytes(raw[..8].try_into().unwrap());
        let capacity = i32::from_le_bytes(raw[8..12].try_into().unwrap());
        let count = i32::from_le_bytes(raw[12..].try_into().unwrap());
        if count < 0 || count > capacity || capacity > 1024 || count > 0 && data == 0 {
            return Err("Catalogo progetti nativo non valido".into());
        }
        let mut rows = vec![];
        let mut seen = HashSet::new();
        for index in 0..count {
            let project = self.typed(data + index as u64 * 0x1c0, "CProject@NProject")?;
            let template = self
                .pointer(project + 0x20)?
                .checked_sub(0x18)
                .ok_or("Invalid project template")?;
            self.typed(template, "CProjectTemplate@NProject")?;
            let key = self.d.string(template + 0x238)?;
            if !seen.insert(key.clone()) {
                return Err("ID progetto duplicato".into());
            }
            let machine = self.object_pointer(project + 0x28, "CProjectStateMachine@NProject")?;
            let current = self.pointer(machine + 8)?;
            let pending = self.track(machine + 0x10, 1)?[0];
            if pending > 1 {
                return Err("Transizione progetto non valida".into());
            }
            let completed = pending == 0 && current == machine + 0xe8;
            if completed {
                self.typed(current, "CSimpleProjectState@NProject")?;
                if self.d.string(current + 0x58)? != "RESEARCH_COMPLETED" {
                    return Err("Stato finale progetto non riconosciuto".into());
                }
            }
            let handle = self.track(project + 0x60, 8)?;
            let linked = handle != [0; 8];
            let active = linked && !completed;
            if active && !programs.contains_key(&handle) {
                return Err("Impianto associato al progetto non verificabile".into());
            }
            let token = self.integer(template + 8, 1, 1_000_000)?;
            if active && programs[&handle].1 != (token, true) {
                return Err("Progetto e impianto non corrispondono".into());
            }
            let mut row = json!({"id":key,"active":active,"completed":completed});
            if active {
                row["facility"] = json!(programs[&handle].0)
            }
            rows.push(row);
        }
        let breakthroughs = match self.breakthroughs(status) {
            Ok(v) => v,
            Err(reason) => json!({"status":"unavailable","reason":reason}),
        };
        Ok(
            json!({"status":"observed","items":rows,"breakthroughs":breakthroughs,"reason":"Progetti della nazione corrente; impianto associato distinto dallo stato finale di ricerca."}),
        )
    }
    fn decision_id(&mut self, pointer: u64) -> Result<String> {
        self.typed(pointer, "CDecision")?;
        self.d.string(pointer + 0x120)
    }
    fn decision_memberships(&mut self, manager: u64, taken: &[String]) -> Result<Value> {
        // Independently qualified serializers name +40 `decisions_taken`.
        // They are not SDecisionRandomCountItem counters or GUI click evidence.
        self.function_signatures("decisions", "Decision membership layout is not qualified")?;
        if !self.attributes["abi_functions"]["decisions"].as_array().is_some_and(|s| s.len() == 3) {
            return Err("Decision serializer evidence is incomplete".into());
        }
        let mut active = std::collections::BTreeSet::new();
        let mut executed = taken.iter().cloned().collect::<std::collections::BTreeSet<_>>();
        let mut completed = std::collections::BTreeSet::new();
        let mut timed = vec![];
        for item in self.array(manager + 0xa0, 4096)? {
            self.typed(item, "CTimedDecision")?;
            let definition = self.pointer(item + 0x10)?;
            let key = self.decision_id(definition)?;
            let state = self.integer(item + 0x1c, 0, 4)? as usize;
            let days = self.integer(item + 0x18, -1_000_000, 1_000_000)?;
            let names = ["active", "completed", "failed", "aborted", "cooldown"];
            if state == 0 { active.insert(key.clone()); }
            if state == 1 { completed.insert(key.clone()); }
            // A timed record proves activation, including a later failure or
            // cancellation. Only its completed state proves completion.
            executed.insert(key.clone());
            timed.push(json!({"id":key,"state":names[state],"days":days}));
        }
        // Targeted choices need a qualified target identity before their outcome
        // can satisfy an untargeted guide condition. Preserve them as unknown.
        Ok(json!({"status":"observed","source":"native_decisions_taken_and_timed_state",
            "taken_ids":taken,"executed_ids":executed,"active_ids":active,"completed_ids":completed,"timed":timed,
            "history_scope":"current_native_records","targeted_history_status":"unavailable"}))
    }
    pub(super) fn decisions(&mut self) -> Result<Value> {
        let manager = self.object_pointer(self.country + 0xfa0, "CDecisionStatus")?;
        if self.pointer(manager + 8)? != self.country {
            return Err("Decisioni di un’altra nazione".into());
        }
        let mut listed = vec![];
        for pointer in self.array(manager + 0x40, 4096)? {
            listed.push(self.decision_id(pointer)?)
        }
        if listed.iter().collect::<HashSet<_>>().len() != listed.len() {
            return Err("Identità decisione duplicata".into());
        }
        listed.sort();
        let memberships = match self.decision_memberships(manager, &listed) {
            Ok(value) => value,
            Err(reason) => json!({"status":"unavailable","reason":reason}),
        };
        let raw = self.track(manager + 0x148, 16)?;
        let data = u64::from_le_bytes(raw[..8].try_into().unwrap());
        let capacity = i32::from_le_bytes(raw[8..12].try_into().unwrap());
        let count = i32::from_le_bytes(raw[12..].try_into().unwrap());
        if count < 0 || count > capacity || capacity > 2048 {
            return Err("Contatori decisioni fuori intervallo".into());
        }
        let raw = if count > 0 {
            self.track(data, count as usize * 24)?
        } else {
            vec![]
        };
        let mut counters = vec![];
        let mut seen = HashSet::new();
        for row in raw.as_chunks::<24>().0 {
            let vtable = u64::from_le_bytes(row[..8].try_into().unwrap());
            let decision = u64::from_le_bytes(row[8..16].try_into().unwrap());
            let amount = i32::from_le_bytes(row[16..20].try_into().unwrap());
            let target = u32::from_le_bytes(row[20..].try_into().unwrap());
            let structural = self.attributes["layout_id"] == "hoi4-x64-abi-v1";
            let qualified = if structural {
                vtable.checked_sub(self.d.base).is_some_and(|rva| {
                    self.d
                        .memory
                        .class_name(rva)
                        .is_ok_and(|name| name == ".?AUSDecisionRandomCountItem@CDecisionStatus@@")
                })
            } else {
                vtable == self.d.base + 0x27eb570
            };
            if !qualified || amount < 0 {
                return Err("Contatore decisione non qualificato".into());
            }
            let key = self.decision_id(decision)?;
            if !seen.insert((key.clone(), target)) {
                return Err("Contatore decisione duplicato".into());
            }
            counters.push(if structural {
                json!({"id":key,"native_target_tag":target,"native_random_counter":amount})
            } else {
                json!({"id":key,"native_target_tag":target,"native_execution_counter":amount})
            });
        }
        if self.attributes["layout_id"] == "hoi4-x64-abi-v1" {
            return Ok(
                json!({"status":"observed","listed_ids":listed,"execution_counters":[],"random_counters":counters,
                "execution_history":memberships,
                "reason":"Appartenenza al manager e contatori casuali nativi. L’esecuzione delle decisioni non è dedotta da questi contatori."}),
            );
        }
        Ok(
            json!({"status":"observed","listed_ids":listed,"execution_counters":counters,"reason":"Appartenenza al manager e contatori nativi. Disponibilità, avvio, scelta ed effetti non ancora qualificati."}),
        )
    }
}
