//! Revisioned HUD projection over the same application packet as the web UI.
//! Only immutable guide/event preparation is cached; geometry is sampled fresh.
use crate::{hud_events, hud_targets, import_catalog::Index};
use serde_json::{Value, json};
type Result<T> = std::result::Result<T, String>;
#[derive(Default)]
pub(crate) struct Hud {
    version: u64,
    key: Value,
    event_key: Value,
    event_plan: Value,
    /// Quick-reference sections of the guide (`guide_sections`), per definition.
    sections: Value,
    snapshot: Value,
}
impl Hud {
    pub(crate) fn project(
        &mut self,
        packet: &Value,
        index: &Index,
        generation: u64,
    ) -> Result<Value> {
        let guide = &packet["guide"];
        let runtime = &packet["evaluation"];
        let state = &packet["state"];
        let event_key = json!([
            generation,
            packet["guide_id"],
            packet["definition_revision"]
        ]);
        if self.event_key != event_key {
            self.event_plan = hud_events::plan(guide, index)?;
            self.sections = crate::guide_sections::card_sections(guide);
            self.event_key = event_key;
        }
        let mut proposed = hud_targets::targets(guide, runtime, index, state, false, &self.event_plan)?;
        let card = step_card(guide,runtime);
        let context=&runtime["action_context"];
        if runtime["mode"]=="live" && state["usable"]==true && runtime["guide_id"]==guide["id"]
            && state["sample"]["country"]==runtime["country"]
            && (context["session_id"].is_null()||context["session_id"]==state["sample"]["session_id"])
            && (context["campaign_id"].is_null()||context["campaign_id"]==state["sample"]["campaign_identity"]["id"]) {
            let step=guide["tracks"].as_object().into_iter().flat_map(|m|m.values()).flat_map(crate::hud_geometry::rows).find(|r|r["key"]==card["key"]);
            let mut seen=std::collections::HashSet::new();
            if let Some(step)=step {
                for target in crate::hud_geometry::rows(&step["action"]["params"]["construction_targets"]) {
                    let Some(id)=target["state_id"].as_u64().filter(|id|seen.insert(*id)) else {continue};
                    let overview=&state["sample"]["country_overview"];
                    if overview["country"]!=runtime["country"] || overview["states"]["status"]!="observed"
                        || !crate::hud_geometry::rows(&overview["states"]["items"]).iter().any(|r|r["id"]==id&&(r["owned"]==true||r["controlled"]==true)) {continue;}
                    let entries=index.rows().iter().filter(|r|r["kind"]=="state"&&r["id"]==id.to_string()).collect::<Vec<_>>();
                    if entries.len()!=1 || !entries[0]["geography"]["map_anchor"].is_array() {continue;}
                    let entry=entries[0];
                    proposed.as_array_mut().ok_or("Invalid HUD proposals")?.push(json!({"key":format!("{}:state:{id}",card["key"].as_str().unwrap_or("")),"guide_step":card["key"],"panel":"construction_map","identity":id.to_string(),"state_id":id,"title":entry.get("localized_name").unwrap_or(&entry["name"]),"world_point":entry["geography"]["map_anchor"],"country":runtime["country"],"status":"ready"}));
                }
            }
        }
        let error = if runtime["mode"] == "live" {
            ""
        } else {
            "Partita non valida per questa guida: evidenziazioni sospese"
        };
        let key = json!([
            generation,
            packet["server_id"],
            packet["guide_id"],
            packet["definition_revision"],
            runtime["run_id"],
            context["session_id"],
            context["campaign_id"],
            proposed,
            error
        ]);
        if self.key != key {
            self.version = self.version.wrapping_add(1);
            self.key = key;
        }
        let usable = error.is_empty() && proposed.as_array().is_some_and(|p| !p.is_empty());
        let production = card_production(&proposed);
        self.snapshot = json!({"version":self.version,"proposed":if usable{proposed}else{json!([])},"usable":usable,"title":guide["title"],"error":if error.is_empty() && !usable{"Nessuna indicazione live attuale"}else{error},"age_ms":0,"guide_id":packet["guide_id"],"guide_server_id":packet["server_id"],"source_generation":generation,"idle_explanation":null});
        if error.is_empty() && !usable {
            self.snapshot["idle_explanation"] = notice(runtime);
        }
        // Shown by the HUD whether or not a control can be highlighted.
        self.snapshot["step_card"] = if error.is_empty() { card } else { Value::Null };
        if !self.snapshot["step_card"].is_null() {
            self.snapshot["step_card"]["production"] = production;
            // Army setup, war plans and events, checkable per checklist run.
            self.snapshot["step_card"]["references"] = self.sections.clone();
            self.snapshot["step_card"]["references_scope"] = json!(format!("{}:{}", packet["guide_id"].as_str().unwrap_or(""), runtime["run_id"].as_str().unwrap_or("")));
        }
        Ok(self.snapshot.clone())
    }
    pub(crate) fn unavailable(&mut self, error: &str) -> Value {
        let key = json!(["unavailable", error]);
        if self.key != key {
            self.version = self.version.wrapping_add(1);
            self.key = key;
        }
        self.snapshot = json!({"version":self.version,"proposed":[],"usable":false,"title":"","error":error,"age_ms":null,"guide_id":null,"guide_server_id":null,"idle_explanation":null});
        self.snapshot.clone()
    }
}
/// The guide step the player should do now, as authored, for the HUD card:
/// the first not-yet-done step of the main (operations) track, else of any
/// track. Text is the author's (plus a recognized full name); status comes
/// from the Rust evaluation only.
fn step_card(guide: &Value, runtime: &Value) -> Value {
    let tracks = &runtime["tracks"];
    let statuses: std::collections::HashMap<_, _> = crate::hud_geometry::rows(&runtime["steps"])
        .iter().filter_map(|r| r["key"].as_str().map(|k| (k, &r["status"]))).collect();
    // Steps the player set aside are left out of the card until resumed.
    let aside: std::collections::HashSet<&str> = crate::hud_geometry::rows(&runtime["steps"])
        .iter().filter(|r| r["set_aside"] == true).filter_map(|r| r["key"].as_str()).collect();
    let names: Vec<&str> = std::iter::once("operations")
        .chain(tracks.as_object().into_iter().flat_map(|m| m.keys().map(String::as_str)).filter(|k| *k != "operations"))
        .collect();
    let Some((track, key)) = names
        .iter()
        .find_map(|t| crate::hud_geometry::rows(&guide["tracks"][*t]).iter()
            .find(|r| r["tracking"] != "reminder" && !aside.contains(r["key"].as_str().unwrap_or("")) && !statuses.get(r["key"].as_str().unwrap_or(""))
                .is_some_and(|s| **s == "completed" || **s == "not_applicable"))
            .and_then(|r| r["key"].as_str()).map(|k| (*t,k)))
    else {
        return Value::Null;
    };
    let rows = guide["tracks"][track].as_array().cloned().unwrap_or_default();
    let Some(position) = rows.iter().position(|r| r["key"] == key) else {
        return Value::Null;
    };
    let step = &rows[position];
    let params = &step["action"]["params"];
    let clean = |v: &Value| v.as_str().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
    let instructions: Vec<String> = params["instructions"].as_array().into_iter().flatten().filter_map(clean).collect();
    let resolved = clean(&params["reference_resolution"]["option_text"]).or_else(|| clean(&params["event_option_name"]));
    let title = clean(&step["title"]).or_else(|| instructions.first().cloned()).unwrap_or_else(|| key.to_owned());
    let heading = resolved.clone().map(|r| format!("«{r}»")).unwrap_or_else(|| title.clone());
    let abbreviation = resolved.as_ref().and_then(|_| instructions.first().cloned().or_else(|| Some(title.clone())));
    // A line the heading already says (`Build: X` above `X`) is not repeated.
    let repeats = |line: &str| [&heading, &title].iter().any(|t| *t == line || t.ends_with(&format!(": {line}")));
    let mut body: Vec<String> = instructions
        .iter()
        .filter(|l| Some(*l) != abbreviation.as_ref() && !repeats(l))
        .cloned()
        .collect();
    let mut length = 0;
    body.retain(|l| { length += l.chars().count(); length <= 900 });
    let status = crate::hud_geometry::rows(&runtime["steps"]).into_iter().find(|r| r["key"] == key).map(|r| r["status"].clone()).unwrap_or(Value::Null);
    let total = rows.iter().filter(|r| r["tracking"] != "reminder").count();
    let index = rows[..=position].iter().filter(|r| r["tracking"] != "reminder").count();
    // The HUD offers "Done" only where the suite offers a manual confirmation
    // of this very step; observed steps complete from the game alone.
    let checkable = status != "completed" && crate::application::manual_keys(guide).contains(key)
        && confirmation_completes(step);
    // Guided mode (UX R9): the line "Category · When · Verification" and the
    // readable title, as in the suite, then the next step and the nearest
    // waiting one. Presentation only, from the prepared step and the evaluation.
    let status_of = |r: &Value| statuses.get(r["key"].as_str().unwrap_or("")).map(|s| (*s).clone()).unwrap_or(Value::Null);
    let brief = |r: &Value| json!({"key":r["key"],"heading":clean(&r["title"]).or_else(|| clean(&r["name"])).or_else(|| r["action"]["params"]["instructions"].as_array().into_iter().flatten().find_map(clean)).unwrap_or_default(),"reading_title":reading_title(r)});
    let later: Vec<&Value> = rows[position + 1..].iter()
        .filter(|r| r["tracking"] != "reminder" && !aside.contains(r["key"].as_str().unwrap_or("")) && !matches!(status_of(r).as_str(), Some("completed" | "not_applicable")))
        .collect();
    let then = later.iter().find(|r| matches!(status_of(r).as_str(), Some("ready" | "unverifiable"))).map(|r| brief(r)).unwrap_or(Value::Null);
    let waiting = later.iter().find(|r| status_of(r) == "waiting").map(|r| {
        let mut out = brief(r);
        out["reason"] = waiting_reason(guide, runtime, r, &brief);
        out
    }).unwrap_or(Value::Null);
    json!({"key":key,"track":track,"index":index,"total":total,"heading":heading,"abbreviation":abbreviation,
        "body":body,"status":status,"event_answer":resolved.is_some(),"checkable":checkable,"images":card_images(params),
        "construction":card_construction(runtime, key),
        "category":hoi4companion_guides::display_category::category(step),
        "verification":hoi4companion_guides::import_reports::verification(step),
        "started_when":step.get("started_when").is_some_and(|v| !v.is_null()),
        "reading_title":reading_title(step),"original":step["reading"]["original"],
        // The author's condition for this line ("At 24% paranoia"), as written.
        "timing":clean(&params["timing"]).or_else(|| clean(&params["decomposition"]["timing"])),
        "then":then,"waiting":waiting})
}
/// Whether the step's own manual check can complete it, as guide-core reads it:
/// a manual or special-project action without a declared completion, or a
/// declared completion that names this step's check. Otherwise the check is
/// stored but ignored, and a "Done" button would do nothing.
fn confirmation_completes(step: &Value) -> bool {
    fn names(node: &Value, key: &str) -> bool {
        node["op"] == "check" && node["key"] == key
            || node["args"].as_array().is_some_and(|a| a.iter().any(|n| names(n, key)))
            || node.get("arg").is_some_and(|n| names(n, key))
    }
    let key = step["key"].as_str().unwrap_or("");
    match step.get("completion").filter(|c| !c.is_null()) {
        Some(completion) => names(completion, key),
        None => matches!(step["action"]["kind"].as_str(), Some("manual" | "special_project")),
    }
}
/// Why a waiting step waits, as the suite's guided line says it (UX R10): the
/// first prerequisite step, else the author's trigger as Rust recognized it,
/// else the evaluation's reason code (read through `runtime_reasons` in
/// translations.json by the HUD). Null when the evaluation gives none.
fn waiting_reason(guide: &Value, runtime: &Value, step: &Value, brief: &dyn Fn(&Value) -> Value) -> Value {
    let key = step["key"].as_str().unwrap_or("");
    let evaluated = crate::hud_geometry::rows(&runtime["steps"]).iter().find(|r| r["key"] == key).cloned().unwrap_or(Value::Null);
    let all: Vec<&Value> = guide["tracks"].as_object().into_iter().flat_map(|m| m.values()).flat_map(|t| crate::hud_geometry::rows(t)).collect();
    if let Some(dep) = crate::hud_geometry::rows(&evaluated["blocked_by"]).iter().find_map(|k| all.iter().find(|r| r["key"] == *k)) {
        return json!({"kind":"step","step":brief(dep)});
    }
    let trig = &step["action"]["params"]["trigger_resolution"];
    if trig["status"] == "compiled" && trig["entity"].is_object() {
        let entity = &trig["entity"];
        let name = entity["name"].as_str().filter(|s| !s.is_empty()).or(entity["id"].as_str());
        return json!({"kind":"entity","entity":entity["kind"],"name":name,"relation":trig["relation"]});
    }
    if let Some(text) = trig["text"].as_str().filter(|s| !s.is_empty()) {
        return json!({"kind":"text","text":text});
    }
    match evaluated["reason"].as_str().filter(|s| !s.is_empty()) {
        Some(code) => json!({"kind":"reason","code":code}),
        None => Value::Null,
    }
}
/// The title Rust composed for a line understood in full (`reading`, UX R8):
/// sentence templates and localized names, translated by the HUD. Null otherwise.
fn reading_title(step: &Value) -> Value {
    let reading = &step["reading"];
    let parts = reading["title"]["parts"].as_array().filter(|p| !p.is_empty());
    if reading["understood"] == true && parts.is_some() { reading["title"].clone() } else { Value::Null }
}
/// Most regions the HUD card lists for one construction step.
const CARD_REGIONS: usize = 12;
/// What a construction step asks, region by region, from the Rust
/// evaluation's construction targets (built level, declared target, queue
/// membership), so the card can say it in words instead of the author's
/// shorthand. Regions keep the guide's order; an unknown level stays null.
fn card_construction(runtime: &Value, key: &str) -> Value {
    let Some(step) = crate::hud_geometry::rows(&runtime["steps"]).into_iter().find(|r| r["key"] == key) else {
        return Value::Null;
    };
    let targets = crate::hud_geometry::rows(&step["construction"]["targets"]);
    if targets.is_empty() {
        return Value::Null;
    }
    let mut regions: Vec<(Value, Value, Vec<Value>)> = Vec::new();
    let mut buildings: Vec<Value> = Vec::new();
    for t in targets.iter() {
        if !buildings.contains(&t["building_name"]) {
            buildings.push(t["building_name"].clone());
        }
        let place = if t["province_id"].is_null() { t["state_id"].clone() } else { t["province_id"].clone() };
        let item = json!({"building":t["building_name"],"current":t["current"],"target":t["target"],"queued":t["queued"]});
        match regions.iter_mut().find(|(id, _, _)| *id == place) {
            Some((_, _, items)) => items.push(item),
            None => regions.push((place, t["state_name"].clone(), vec![item])),
        }
    }
    let count = regions.len();
    json!({"buildings":buildings,"region_count":count,"regions":regions.into_iter().take(CARD_REGIONS)
        .map(|(_, name, items)| json!({"name":name,"items":items})).collect::<Vec<_>>()})
}
/// Most production lines the HUD card lists.
const CARD_PRODUCTION: usize = 12;
/// The guide's current production plan for the card, so the player sees how
/// many factories to assign without opening the production panel: the HUD's
/// own production proposals (`hud_production::proposals`, admitted from the
/// Rust evaluation), in guide order, with the live requested/assigned counts.
/// Same field names as the proposals, so the HUD words both alike.
fn card_production(proposed: &Value) -> Value {
    let fields = ["key", "guide_step", "title", "quantity", "quantity_mode", "requested", "assigned", "goal_satisfied", "production_priority"];
    Value::Array(
        crate::hud_geometry::rows(proposed)
            .iter()
            .filter(|row| row["panel"] == "production")
            // A production step already done is not listed again and again.
            .filter(|row| row["status"] != "completed")
            .take(CARD_PRODUCTION)
            .map(|row| {
                let mut line: serde_json::Map<String, Value> = fields.iter().map(|f| ((*f).to_owned(), row[*f].clone())).collect();
                // Military factories or naval dockyards, so the card says which.
                line.insert("category".into(), row["production_selector"]["category"].clone());
                Value::Object(line)
            })
            .collect(),
    )
}
/// Most images the HUD card shows for one step.
const CARD_IMAGES: usize = 3;
/// The reference-library images the walkthrough shows for this step (web
/// `guide.js`): the step's reference images, then its design references and
/// those of its production targets. Deduplicated by id and capped; the HUD
/// fetches each one from `/api/reference-library/image`.
fn card_images(params: &Value) -> Value {
    let designs = std::iter::once(&params["design_reference"])
        .chain(params["production_targets"].as_array().into_iter().flatten().map(|t| &t["design_reference"]))
        .map(|d| (&d["image_id"], &d["name"]));
    let references = params["reference_images"].as_array().into_iter().flatten().map(|r| (&r["id"], &r["name"]));
    let mut images: Vec<Value> = Vec::new();
    for (id, name) in references.chain(designs) {
        let Some(id) = id.as_str().map(str::trim).filter(|s| !s.is_empty()) else { continue };
        if images.iter().any(|i| i["id"] == id) {
            continue;
        }
        images.push(json!({"id":id,"name":name.as_str().map(str::trim).unwrap_or("")}));
        if images.len() == CARD_IMAGES {
            break;
        }
    }
    Value::Array(images)
}
fn notice(runtime: &Value) -> Value {
    let capacity = &runtime["parallel"]["research"]["capacity"];
    let key = &runtime["action_sequence"]["political"];
    let rows = crate::hud_geometry::rows(&runtime["steps"]);
    let row = rows.iter().find(|r| r["key"] == *key).or_else(|| {
        rows.iter().find(|r| {
            crate::hud_geometry::rows(&runtime["action_sequence"]["projects"]).contains(&r["key"])
                && r["status"] != "active"
        })
    });
    if let Some(row) = row {
        let details = &row["action_readiness"];
        let insufficient: Vec<_> = crate::hud_geometry::rows(&details["resources"])
            .iter()
            .filter(|r| r["affordable"] == false)
            .collect();
        let s = |v: &Value| v.as_str().unwrap_or("").to_owned();
        if !insufficient.is_empty() {
            return json!({"code":"insufficient_resource","message":format!("HUD in attesa delle risorse per {}. {}",s(&row["key"]),insufficient.iter().map(|r|format!("{}: {} / {}",s(&r["key"]),r["available"],r["cost"])).collect::<Vec<_>>().join("; "))});
        }
        let unknown = crate::hud_geometry::rows(&details["unknown"]);
        if unknown
            .iter()
            .any(|r| r == "project_cost_unknown" || r == "resource_balance_unknown")
        {
            return json!({"code":"resource_unverifiable","message":format!("Costo o saldo dei punti non qualificato per {}: nessuna cornice HUD finché il dato resta sconosciuto.",s(&row["key"]))});
        }
        if crate::hud_geometry::rows(&details["blocked"])
            .iter()
            .any(|r| r == "insufficient_political_power")
        {
            return json!({"code":"insufficient_political_power","message":format!("Prima azione politica della guida: {}. Potere politico {} / {} indicati dalla guida; attendo senza saltare alla nomina successiva.",s(key),details["political_power"]["available"],details["political_power"]["cost"])});
        }
        if unknown
            .iter()
            .any(|r| r == "political_cost_unknown" || r == "political_power_unknown")
        {
            return json!({"code":"political_resource_unverifiable","message":format!("Potere politico o costo non qualificato per {}: HUD in attesa, senza proporre una nomina successiva.",s(&row["key"]))});
        }
        if row["reason"] == "dependency_unknown" || row["reason"] == "dependency_not_completed" {
            return json!({"code":row["reason"],"message":format!("La prossima azione politica attende il prerequisito dichiarato dalla guida: {}",crate::hud_geometry::rows(&row["blocked_by"]).iter().map(s).collect::<Vec<_>>().join(", "))});
        }
        if row["status"] == "waiting" || row["status"] == "unverifiable" {
            return json!({"code":row["reason"],"message":format!("La prossima azione politica non è ancora verificabile o disponibile secondo i criteri della guida. La sequenza conserva {}.",s(key))});
        }
    }
    json!({"code":if capacity==&json!(0){"slots_occupied"}else{"no_targets"},"message":if capacity==&json!(0){"Nessuna nuova ricerca da avviare: gli slot rilevati sono occupati."}else{"Nessuna indicazione attuale ammessa dalla guida; questo non equivale a un errore HUD."}})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn construction_markers_use_current_step_state_ids_and_reject_foreign_state() {
        let index=Index::new(&json!({"entries":[{"kind":"state","id":"219","name":"Moscow","localized_name":"Mosca","geography":{"map_anchor":[3428.,10.4,1630.]}}]})).unwrap();
        let guide=json!({"id":"map-test","tracks":{"operations":[{"key":"build","title":"Any translated title","action":{"kind":"manual","params":{"category":"construction","construction_targets":[{"state_id":219},{"state_id":219}]}}}]}});
        let runtime=json!({"guide_id":"map-test","mode":"live","country":"SOV","tracks":{"operations":{"next":"build"}},"steps":[{"key":"build","status":"active"}]});
        let mut packet=json!({"guide_id":"map-test","guide":guide,"evaluation":runtime,"state":{"usable":true,"sample":{"country":"SOV","country_overview":{"country":"SOV","states":{"status":"observed","items":[{"id":219,"owned":true,"controlled":true}]}}}}});
        let mut hud=Hud::default();
        let result=hud.project(&packet,&index,1).unwrap();
        let markers=crate::hud_geometry::rows(&result["proposed"]).iter().filter(|p|p["panel"]=="construction_map").collect::<Vec<_>>();
        assert_eq!(markers.len(),1);assert_eq!(markers[0]["state_id"],219);assert_eq!(markers[0]["title"],"Mosca");
        packet["state"]["sample"]["country"]=json!("GER");
        assert!(crate::hud_geometry::rows(&hud.project(&packet,&index,1).unwrap()["proposed"]).iter().all(|p|p["panel"]!="construction_map"));
    }
    #[test]
    fn construction_card_says_the_shorthand_once_and_lists_regions_with_levels() {
        let guide = json!({"tracks":{"operations":[{"key":"build","title":"Build: INFRA+MILs in: Moscow, Kharkov",
            "action":{"kind":"manual","params":{"instructions":["INFRA+MILs in: Moscow, Kharkov"]}}}]}});
        let target = |building: &str, state: u64, name: &str, current: Value, goal: Value, queued: bool|
            json!({"building_name":building,"state_id":state,"state_name":name,"province_id":null,"current":current,"target":goal,"queued":queued});
        let runtime = json!({"tracks":{"operations":{"next":"build"}},"steps":[{"key":"build","status":"active","construction":{"targets":[
            target("Infrastruttura",219,"Mosca",json!(4),json!(5),true),target("Fabbrica militare",219,"Mosca",json!(7),Value::Null,false),
            target("Infrastruttura",221,"Kharkov",Value::Null,json!(5),false)]}}]});
        let card = step_card(&guide, &runtime);
        assert_eq!(card["body"], json!([]), "the instruction repeats the heading");
        let construction = &card["construction"];
        assert_eq!(construction["buildings"], json!(["Infrastruttura","Fabbrica militare"]));
        assert_eq!(construction["region_count"], 2);
        assert_eq!(construction["regions"][0], json!({"name":"Mosca","items":[
            {"building":"Infrastruttura","current":4,"target":5,"queued":true},
            {"building":"Fabbrica militare","current":7,"target":null,"queued":false}]}));
        assert!(construction["regions"][1]["items"][0]["current"].is_null(), "an unobserved level stays unknown");
    }
    #[test]
    fn step_card_skips_set_aside_steps() {
        let guide = json!({"tracks":{"operations":[
            {"key":"a","title":"First","action":{"kind":"manual"}},
            {"key":"b","title":"Second","action":{"kind":"manual"}}]}});
        let runtime = json!({"tracks":{"operations":{"next":"b"}},"steps":[
            {"key":"a","status":"unverifiable","set_aside":true},{"key":"b","status":"unverifiable"}]});
        let card = step_card(&guide, &runtime);
        assert_eq!(card["key"], "b");
        assert!(card["then"].is_null(), "a set-aside step is not offered as next either");
    }
    #[test]
    fn step_card_is_the_next_operation_with_its_recognized_answer() {
        let guide = json!({"tracks":{"operations":[
            {"key":"word-0001","title":"Done","action":{"kind":"manual","params":{"instructions":["Done"]}}},
            {"key":"word-0002","tracking":"reminder","title":"Memo","action":{"kind":"manual"}},
            {"key":"word-0003","title":"Mobilize The…","action":{"kind":"manual","params":{
                "instructions":["Mobilize The…","Queue civilian factories"],
                "reference_resolution":{"option_text":"Mobilize the Armed Forces immediately!"}}}}]}});
        let runtime = json!({"tracks":{"operations":{"next":"word-0003"}},"steps":[{"key":"word-0001","status":"completed"},{"key":"word-0003","status":"ready"}]});
        let card = step_card(&guide, &runtime);
        assert_eq!(card["heading"], "«Mobilize the Armed Forces immediately!»");
        assert_eq!(card["abbreviation"], "Mobilize The…");
        assert_eq!(card["body"], json!(["Queue civilian factories"]));
        assert_eq!((card["index"].clone(), card["total"].clone()), (json!(2), json!(2)), "reminders are not numbered");
        assert_eq!(card["status"], "ready");
        assert_eq!(card["checkable"], false, "a step without a manual confirmation is never checked from the HUD");
        let mut gated = guide.clone();
        gated["tracks"]["operations"][2]["kind"] = json!("gate");
        assert_eq!(step_card(&gated, &runtime)["checkable"], true);
        assert!(step_card(&guide, &json!({"tracks":{"operations":{"next":null}},"steps":[{"key":"word-0001","status":"completed"},{"key":"word-0003","status":"completed"}]})).is_null());
        assert_eq!(card["images"], json!([]));
    }
    #[test]
    fn step_card_carries_the_guided_line_readable_title_then_and_waiting() {
        let title = json!({"parts":[{"template":"Justify a war goal on {0}","args":["Ungheria (HUN)"]}],"joiner":"and"});
        let guide = json!({"tracks":{"operations":[
            {"key":"a","title":"Justify HUN","action":{"kind":"manual","params":{"source_action":{},"instructions":["Justify HUN"]}},
                "reading":{"understood":true,"original":"Justify HUN","title":title}},
            {"key":"memo","tracking":"reminder","title":"Never cancel","action":{"kind":"manual"}},
            {"key":"b","title":"On Revise…: appoint X","action":{"kind":"manual","params":{"category":"characters"}}},
            {"key":"c","title":"Build 2 civs in Mun","action":{"kind":"manual","params":{"category":"construction"}},
                "reading":{"understood":false,"original":"Build 2 civs in Mun","title":null}},
            {"key":"d","title":"Later","action":{"kind":"manual"}}]}});
        let runtime = json!({"tracks":{"operations":{"next":"a"}},"steps":[
            {"key":"a","status":"ready"},{"key":"b","status":"waiting"},{"key":"c","status":"ready"},{"key":"d","status":"waiting"}]});
        let card = step_card(&guide, &runtime);
        assert_eq!(card["category"], "diplomacy");
        assert_eq!(card["verification"], "uninterpreted", "same method as the import report");
        assert_eq!(card["reading_title"], title);
        assert_eq!(card["original"], "Justify HUN");
        assert!(card["timing"].is_null(), "no condition in the guide: none shown");
        let mut timed = guide.clone();
        timed["tracks"]["operations"][0]["action"]["params"]["decomposition"] = json!({"timing":"At 24% paranoia"});
        assert_eq!(step_card(&timed, &runtime)["timing"], "At 24% paranoia", "the author's condition, verbatim");
        assert_eq!(card["then"], json!({"key":"c","heading":"Build 2 civs in Mun","reading_title":null}), "a reminder and a waiting step are not 'then'");
        assert_eq!(card["waiting"]["key"], "b", "the nearest waiting step");
        assert!(card["waiting"]["reason"].is_null(), "no reason from the evaluation: none invented");
        // The player's interpretation (UX R13), attached by the projection, gives
        // the card the same category and verification as the suite.
        let mut interpreted = guide.clone();
        interpreted["tracks"]["operations"][0]["interpretation"] = json!({"category":"army","target":null});
        let card = step_card(&interpreted, &runtime);
        assert_eq!((card["category"].as_str(), card["verification"].as_str()), (Some("army"), Some("manual")));
        assert_eq!(card["status"], "ready", "status stays the evaluation's");
        // UX R10: the reason, in the suite's order of preference.
        let mut why = runtime.clone();
        why["steps"][1]["reason"] = json!("insufficient_political_power");
        assert_eq!(step_card(&guide, &why)["waiting"]["reason"], json!({"kind":"reason","code":"insufficient_political_power"}));
        let mut trig = guide.clone();
        trig["tracks"]["operations"][2]["action"]["params"]["trigger_resolution"] = json!({"status":"compiled","relation":"reached",
            "entity":{"kind":"focus","id":"ROM_revise_the_constitution","name":"Revise the Constitution"}});
        assert_eq!(step_card(&trig, &why)["waiting"]["reason"],
            json!({"kind":"entity","entity":"focus","name":"Revise the Constitution","relation":"reached"}));
        trig["tracks"]["operations"][2]["action"]["params"]["trigger_resolution"] = json!({"status":"unknown","text":"On Revise…"});
        assert_eq!(step_card(&trig, &why)["waiting"]["reason"], json!({"kind":"text","text":"On Revise…"}));
        why["steps"][1]["blocked_by"] = json!(["d"]);
        assert_eq!(step_card(&trig, &why)["waiting"]["reason"], json!({"kind":"step","step":{"key":"d","heading":"Later","reading_title":null}}));
        let mut unknown = runtime.clone();
        unknown["steps"][2] = json!({"key":"x","status":"ready"});
        assert!(step_card(&guide, &unknown)["then"].is_null(), "a step without a Rust status is never 'then'");
        let mut done = runtime.clone();
        done["steps"][1]["status"] = json!("completed");
        done["steps"][2]["status"] = json!("completed");
        done["steps"][3]["status"] = json!("completed");
        let card = step_card(&guide, &done);
        assert!(card["then"].is_null() && card["waiting"].is_null(), "nothing left after the card");
    }
    #[test]
    fn step_card_keeps_the_first_incomplete_operation_even_when_next_skips_it() {
        let guide=json!({"tracks":{"operations":[
            {"key":"one","title":"Finished construction"},
            {"key":"two","title":"Construction in progress"},
            {"key":"three","title":"Production in progress"},
            {"key":"four","title":"Next order"}
        ]}});
        let mut runtime=json!({"tracks":{"operations":{"next":"four"}},"steps":[
            {"key":"one","status":"completed"},{"key":"two","status":"active"},
            {"key":"three","status":"active"},{"key":"four","status":"ready"}
        ]});
        let card=step_card(&guide,&runtime);
        assert_eq!(card["key"],"two");assert_eq!(card["index"],2);assert_eq!(card["status"],"active");
        runtime["steps"][1]["status"]=json!("completed");
        assert_eq!(step_card(&guide,&runtime)["key"],"three");
        runtime["steps"][1]["status"]=json!("unverifiable");
        assert_eq!(step_card(&guide,&runtime)["key"],"two");
    }
    #[test]
    fn step_card_offers_done_only_where_the_check_completes_the_step() {
        let fact=json!({"op":"fact","key":"country.states.items.1.infrastructure","test":"gte","value":5});
        let card=|completion:Value|{
            let guide=json!({"tracks":{"operations":[{"key":"g","kind":"gate","title":"Build","action":{"kind":"manual"},"completion":completion}]}});
            step_card(&guide,&json!({"steps":[{"key":"g","status":"active"}]}))["checkable"].clone()
        };
        // A declared completion without this step's check ignores the check.
        assert_eq!(card(fact.clone()),false);
        assert_eq!(card(json!({"op":"all","args":[fact,{"op":"any","args":[{"op":"check","key":"g"}]}]})),true);
        assert_eq!(card(Value::Null),true);
    }
    #[test]
    fn step_card_lists_the_walkthrough_reference_images_once() {
        let guide = json!({"tracks":{"operations":[{"key":"word-0001","title":"Change I-16 to F1ca","action":{"kind":"manual","params":{
            "reference_images":[{"id":"aa","name":"F1ca"},{"id":" ","name":"blank"}],
            "design_reference":{"name":"F1ca","image_id":"aa","modules":{}},
            "production_targets":[{"design_reference":{"name":"Tank","image_id":"bb"}},{"design_reference":{"name":"No image"}},
                {"design_reference":{"name":"C","image_id":"cc"}},{"design_reference":{"name":"D","image_id":"dd"}}]}}}]}});
        let runtime = json!({"tracks":{"operations":{"next":"word-0001"}},"steps":[]});
        assert_eq!(step_card(&guide, &runtime)["images"], json!([{"id":"aa","name":"F1ca"},{"id":"bb","name":"Tank"},{"id":"cc","name":"C"}]));
    }
    #[test]
    fn card_lists_the_production_proposals_only() {
        let proposed = json!([
            {"key":"p1.target-1","panel":"production","guide_step":"p1","title":"F1ca","quantity":15,"quantity_mode":"total","requested":10,"assigned":8,"goal_satisfied":false,"production_priority":null,"production_selector":{"category":"military"},"rect":[1,2,3,4]},
            {"key":"f1","panel":"focus","title":"Five-Year Plan"},
            {"key":"p2","panel":"production","guide_step":"p2","title":"Infantry Equipment","quantity":null,"quantity_mode":"remaining","requested":null,"assigned":null,"goal_satisfied":null,"production_priority":"last"}]);
        let rows = card_production(&proposed);
        assert_eq!(rows.as_array().unwrap().len(), 2);
        assert_eq!(rows[0], json!({"key":"p1.target-1","guide_step":"p1","title":"F1ca","quantity":15,"quantity_mode":"total","requested":10,"assigned":8,"goal_satisfied":false,"production_priority":null,"category":"military"}));
        assert_eq!(rows[1]["quantity_mode"], "remaining");
        let done = json!([{"key":"p3","panel":"production","guide_step":"p3","title":"Truck","status":"completed","goal_satisfied":true}]);
        assert_eq!(card_production(&done), json!([]), "satisfied production is not shown forever");
        assert_eq!(card_production(&json!([])), json!([]));
    }
    #[test]
    fn errors_never_keep_usable_targets_or_previous_identity() {
        let mut hud = Hud::default();
        let first = hud.unavailable("closed");
        assert_eq!(first["proposed"], json!([]));
        let version = first["version"].clone();
        assert_eq!(hud.unavailable("closed")["version"], version);
        assert_ne!(hud.unavailable("foreign")["version"], version);
        assert_eq!(hud.snapshot["usable"], false);
        assert!(hud.snapshot["guide_id"].is_null());
    }
}
