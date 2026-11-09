//! Explanations describe available evidence. They never admit targets or mark
//! instructions completed. Grouping retains every affected guide step key.
use crate::hud_geometry::rows;
use serde_json::{Value, json};
/// Diagnostics are descriptive only. Never reuse a different guide/session or
/// accept an unbounded/status file older than the existing four-second window.
pub(crate) fn read_status(path: &std::path::Path, guide: &Value, server: &Value) -> Value {
    fn unavailable(code: &str, message: &str) -> Value {
        json!({"schema_version":1,"status":"unavailable","code":code,"message":message,"items":[]})
    }
    fn shortened(value: &Value, limit: usize) -> String {
        value.as_str().unwrap_or("").chars().take(limit).collect()
    }
    let missing = || unavailable("not_observed", "Stato HUD non disponibile: avvialo per verificarne le indicazioni.");
    let Ok(file) = std::fs::File::open(path) else { return missing(); };
    let Ok(meta) = file.metadata() else { return missing(); };
    if meta.modified().ok().and_then(|t| std::time::SystemTime::now().duration_since(t).ok()).is_none_or(|age| age.as_secs_f64()>4.0) {
        return unavailable("stale", "Stato HUD non aggiornato; non mostro indicazioni precedenti.");
    }
    if meta.len()>262144 { return missing(); }
    use std::io::Read;
    let mut bytes=Vec::new();
    if file.take(262145).read_to_end(&mut bytes).is_err() || bytes.len()>262144 { return missing(); }
    let Ok(value)=serde_json::from_slice::<Value>(&bytes) else { return missing(); };
    if !value.is_object() { return missing(); }
    if guide.is_null() || server.is_null() || value["guide_id"]!=*guide || value["guide_server_id"]!=*server {
        return unavailable("context_mismatch", "Diagnostica HUD riferita a un’altra guida o sessione del programma.");
    }
    if value["status_message"].as_str().is_none_or(str::is_empty) { return missing(); }
    let items:Vec<_>=rows(&value["explanations"]).iter().take(8).filter(|v|v["code"].is_string() && v["message"].is_string()).map(|v| {
        let keys:Vec<_>=rows(&v["step_keys"]).iter().take(64).filter(|k|k.is_string()).map(|k|shortened(k,160)).collect();
        let mut item=json!({"code":shortened(&v["code"],80),"message":shortened(&v["message"],500),"step_keys":keys});
        for name in ["panel","folder"] { if v[name].is_string() { item[name]=json!(shortened(&v[name],160)); } }
        item
    }).collect();
    json!({"schema_version":1,"status":"observed","code":value.get("status_code").unwrap_or(&json!("unknown")),"message":shortened(&value["status_message"],500),"items":items,"guide_id":guide})
}
fn text(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
pub(crate) fn explain(raw: &Value, proposals: &Value, matches: &[Value]) -> Value {
    let mut groups: Vec<Value> = vec![];
    for p in rows(proposals) {
        let panel = text(&p["panel"]);
        if ![
            "focus",
            "research",
            "event",
            "politics",
            "projects",
            "production",
        ]
        .contains(&panel)
        {
            continue;
        }
        let info = &raw["panels"][panel];
        let folder = &p["folder"];
        let matched = matches.iter().any(|m| {
            m["key"] == p["key"]
                || m["panel"] == "production"
                    && m["navigation_stage"] == "add"
                    && rows(&m["step_keys"]).contains(&p["key"])
        });
        let tab = matches
            .iter()
            .any(|m| m["panel"] == "research_tab" && rows(&m["step_keys"]).contains(&p["key"]));
        let (code, message) = if raw["country"] != p["country"] {
            (
                "identity_mismatch",
                "Identità della nazione diversa: indicazione sospesa.".into(),
            )
        } else if rows(&raw["event_windows"])
            .iter()
            .any(|w| w["visible"] == true)
            && panel != "event"
        {
            (
                "event_foreground",
                "Una finestra evento è in primo piano: le indicazioni degli alberi sono sospese."
                    .into(),
            )
        } else if matched {
            let stage = matches
                .iter()
                .find(|m| m["key"] == p["key"])
                .map(|m| text(&m["navigation_stage"]))
                .unwrap_or("");
            let mut message=if ["access","slot","facility"].contains(&stage){"Evidenziato il punto di accesso; l’entità verrà indicata solo nel pannello e contesto qualificati."}else{"Pulsante individuato ed evidenziato."}.to_owned();
            if panel == "production" {
                let stages: Vec<_> = matches
                    .iter()
                    .filter(|m| m["key"] == p["key"] || rows(&m["step_keys"]).contains(&p["key"]))
                    .map(|m| text(&m["navigation_stage"]))
                    .collect();
                message=if stages.contains(&"equipment"){if stages.contains(&"add"){"Production category and exact product highlighted."}else{"Prodotto esatto individuato: selezionalo per creare la linea."}}
                else if stages.contains(&"design_candidate"){
                    let status=text(&p["production_design_check"]["status"]);
                    if stages.contains(&"add"){match status{"different"=>"Production category highlighted. The current design differs from the required recipe.","unknown"=>"Production category highlighted. The current design configuration is not verifiable.",_=>"Production category highlighted. Create or load the required design."}}
                    else{match status{"different"=>"La configurazione live differisce dalla ricetta richiesta: confronta moduli e potenziamenti nel walkthrough.","unknown"=>"Configurazione live non verificabile: nessuna corrispondenza dedotta dal nome.",_=>"Carica o crea il progetto richiesto nel progettista dei mezzi."}}
                }else if stages.contains(&"add"){"Evidenziata la categoria per aggiungere i prodotti mancanti."}
                else if p["goal_satisfied"]==true{"Quota della guida raggiunta: linea evidenziata in azzurro."}else{&message}.to_owned();
            }
            if crate::state::truth(&p["action_readiness"]["unknown"]) {
                message += " Restano requisiti non verificabili; la cornice non certifica disponibilità o esecuzione.";
            }
            ("highlighted", message)
        } else if panel == "production" {
            let production = &raw["production_ui"];
            if production["status"] != "observed" {
                (
                    "production_gui_unqualified",
                    format!(
                        "Contesto GUI della produzione non qualificato: {}",
                        production["reason"]
                            .as_str()
                            .unwrap_or("copertura incompleta")
                    ),
                )
            } else if !crate::state::truth(&production["visible"]) {
                ("panel_closed", "Apri Produzione nel gioco.".into())
            } else if rows(&p["production_line_ids"]).is_empty() {
                if p["production_missing"] != true {
                    ("production_evidence_unknown","Presenza della linea non verificabile: non deduco che il prodotto sia assente.".into())
                } else if crate::state::truth(&production["picker_open"]) {
                    ("production_product_unqualified","Prodotto richiesto non individuato in modo univoco: verifica modello, ricerche e filtri del selettore.".into())
                } else {
                    (
                        "production_line_missing",
                        format!(
                            "Prodotto da aggiungere: {}",
                            p["title"].as_str().unwrap_or(text(&p["identity"]))
                        ),
                    )
                }
            } else if rows(&p["production_line_ids"]).len() > 1 {
                (
                    "widget_ambiguous",
                    "Più linee corrispondenti: nessuna riga scelta arbitrariamente.".into(),
                )
            } else {
                let controls: Vec<_> = rows(&production["lines"])
                    .iter()
                    .filter(|c| rows(&p["production_line_ids"]).contains(&c["line_id"]))
                    .collect();
                if controls.len() == 1 && controls[0]["visible"] == false {
                    (
                        "offscreen",
                        "Linea fuori dall’area visibile: scorri Produzione.".into(),
                    )
                } else {
                    ("production_row_unqualified","Identità o posizione della linea non qualificata; nessuna evidenziazione arbitraria.".into())
                }
            }
        } else if ["politics", "projects"].contains(&panel) {
            let message = if ["partial", "unavailable"].contains(&text(&raw["action_ui"]["status"]))
            {
                format!(
                    "Contesto GUI delle azioni non qualificato: {}",
                    raw["action_ui"]["reason"]
                        .as_str()
                        .unwrap_or("copertura incompleta")
                )
            } else {
                if panel=="politics"{"Slot o ruolo non individuato in modo univoco nella GUI qualificata."}else{"Impianto e specializzazione non ancora qualificati: nessun progetto scelto arbitrariamente."}.into()
            };
            ("action_gui_unqualified", message)
        } else if info["visible"] != true {
            (
                "panel_closed",
                match panel {
                    "research" => "Apri l’albero tecnologico nel gioco.",
                    "focus" => "Apri l’albero dei focus nel gioco.",
                    _ => "L’evento previsto non è visibile.",
                }
                .into(),
            )
        } else if panel == "focus" && crate::state::truth(&p["scope"]) && p["scope"] != raw["scope"]
        {
            (
                "identity_mismatch",
                "Albero nazionale diverso: indicazione sospesa.".into(),
            )
        } else if panel == "research" && *folder != info["folder"] {
            let name = p["folder_name"]
                .as_str()
                .filter(|s| !s.is_empty())
                .or_else(|| folder.as_str().filter(|s| !s.is_empty()))
                .unwrap_or("ricerche");
            (
                "different_folder",
                format!(
                    "Ricerca prevista in {name}. {}",
                    if tab {
                        "La scheda è evidenziata nel gioco."
                    } else {
                        "Apri questa scheda nel gioco."
                    }
                ),
            )
        } else {
            let nodes: Vec<_> = rows(&raw["nodes"])
                .iter()
                .filter(|n| {
                    n["panel"] == panel
                        && n["identity"] == p["identity"]
                        && (panel != "research" || n["folder"] == *folder)
                })
                .collect();
            if nodes.len() > 1 {
                (
                    "widget_ambiguous",
                    "Più pulsanti corrispondenti: nessuna posizione scelta arbitrariamente.".into(),
                )
            } else if nodes.is_empty() {
                ("widget_unresolved","Pulsante non individuato nella GUI corrente; la causa non è ancora verificata.".into())
            } else if nodes[0]["clip"].as_array().is_some_and(|c| {
                c.len() == 4
                    && (c[2].as_f64().is_some_and(|v| v <= 0.)
                        || c[3].as_f64().is_some_and(|v| v <= 0.))
            }) {
                (
                    "offscreen",
                    "Pulsante fuori dall’area visibile: scorri l’albero.".into(),
                )
            } else if nodes[0]["visible"] != true {
                (
                    "widget_hidden",
                    "Il widget corrispondente non è visibile; non ne deduco la disponibilità."
                        .into(),
                )
            } else {
                (
                    "geometry_unqualified",
                    "Pulsante individuato, ma il rettangolo visibile non è qualificato.".into(),
                )
            }
        };
        if let Some(group) = groups
            .iter_mut()
            .find(|g| g["code"] == code && g["panel"] == panel && g["folder"] == *folder)
        {
            group["step_keys"]
                .as_array_mut()
                .unwrap()
                .push(p["key"].clone());
        } else {
            groups.push(json!({"code":code,"message":message,"panel":panel,"folder":folder,"step_keys":[p["key"]]}));
        }
    }
    json!(groups)
}
