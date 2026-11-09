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
            self.event_key = event_key;
        }
        let proposed = hud_targets::targets(guide, runtime, index, state, false, &self.event_plan)?;
        let error = if runtime["mode"] == "live" {
            ""
        } else {
            "Partita non valida per questa guida: evidenziazioni sospese"
        };
        let context = &runtime["action_context"];
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
        self.snapshot = json!({"version":self.version,"proposed":if usable{proposed}else{json!([])},"usable":usable,"title":guide["title"],"error":if error.is_empty() && !usable{"Nessuna indicazione live attuale"}else{error},"age_ms":0,"guide_id":packet["guide_id"],"guide_server_id":packet["server_id"],"source_generation":generation,"idle_explanation":null});
        if error.is_empty() && !usable {
            self.snapshot["idle_explanation"] = notice(runtime);
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
