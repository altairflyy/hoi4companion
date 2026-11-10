//! HUD eligibility uses the qualified reader packet, never saved preferences or
//! the active guide's nation. Static compatibility is supplied by the catalog.
use serde_json::{Value, json};

pub(crate) fn select(packet: &Value, catalog: &Value) -> Value {
    let sample = &packet["sample"];
    let country = sample["country"].as_str().filter(|tag| {
        tag.len() == 3 && tag.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    });
    let qualified = packet["usable"] == true
        && packet["status"] == "observed"
        && packet["source"] == "process_memory"
        && sample["session_id"].as_str().is_some_and(|s| !s.is_empty());
    let Some(country) = country.filter(|_| qualified) else {
        return json!({"guides":[],"guide_filter":{"status":"unavailable","country":null}});
    };
    let tree = sample["focus_tree_id"].as_str().filter(|s| !s.is_empty());
    let guides = catalog.as_array().into_iter().flatten().filter(|guide| {
        let scope = &guide["applies_to"];
        let country_matches = scope["country"].as_str().is_none_or(|s| s.is_empty() || s == country);
        let tree_matches = scope["tree_id"].as_str().is_none_or(|s| s.is_empty() || tree == Some(s));
        country_matches && tree_matches && guide["compatibility"]["blocked"] != true
    }).map(|guide| json!({"id":guide["id"],"title":guide["title"],"applies_to":guide["applies_to"]})).collect::<Vec<_>>();
    json!({"guides":guides,"guide_filter":{"status":"observed","country":country,"tree_id":tree,"session_id":sample["session_id"]}})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn packet(country: &str, tree: &str) -> Value {
        json!({"usable":true,"status":"observed","source":"process_memory","sample":{"country":country,"focus_tree_id":tree,"session_id":"campaign-one"}})
    }
    fn catalog() -> Value {
        json!([
            {"id":"hun-monarchy","title":"Hungarian monarchy","applies_to":{"country":"HUN","tree_id":"hungary"}},
            {"id":"hun-democratic","title":"Hungarian democracy","applies_to":{"country":"HUN","tree_id":"hungary"}},
            {"id":"nor","title":"Norway","applies_to":{"country":"NOR"}},
            {"id":"hun-old-tree","applies_to":{"country":"HUN","tree_id":"old-hungary"}},
            {"id":"hun-dlc","applies_to":{"country":"HUN"},"compatibility":{"blocked":true}},
            {"id":"universal","applies_to":{}}
        ])
    }
    #[test]
    fn all_matching_routes_survive_without_selecting_a_guide() {
        let output = select(&packet("HUN", "hungary"), &catalog());
        let ids = output["guides"].as_array().unwrap().iter().map(|g|g["id"].as_str().unwrap()).collect::<Vec<_>>();
        assert_eq!(ids, ["hun-monarchy", "hun-democratic", "universal"]);
        assert!(output.get("guide_id").is_none());
        assert_eq!(output["guide_filter"]["country"], "HUN");
    }
    #[test]
    fn stale_missing_or_unqualified_identity_never_reuses_a_country() {
        let good = packet("HUN", "hungary");
        for (key, value) in [("usable",json!(false)),("status",json!("stale")),("status",json!("disconnected")),("source",json!("snapshot"))] {
            let mut invalid=good.clone(); invalid[key]=value;
            assert_eq!(select(&invalid,&catalog())["guides"],json!([]));
        }
        for (key,value) in [("country",json!(null)),("country",json!("HUNGER")),("session_id",json!(""))] {
            let mut invalid=good.clone();invalid["sample"][key]=value;
            assert_eq!(select(&invalid,&catalog())["guides"],json!([]));
        }
        assert_eq!(select(&json!({"country":"HUN","usable":true}),&catalog())["guides"],json!([]));
    }
    #[test]
    fn nation_campaign_registry_and_tree_changes_take_effect_immediately() {
        let mut live = packet("HUN", "hungary");
        assert_eq!(select(&live,&catalog())["guides"].as_array().unwrap().len(),3);
        live["sample"] = packet("NOR","norway")["sample"].clone();
        live["sample"]["session_id"] = json!("campaign-two");
        let output=select(&live,&catalog());
        assert_eq!(output["guides"][0]["id"],"nor");
        assert_eq!(output["guides"].as_array().unwrap().len(),2);
        assert_eq!(output["guide_filter"]["session_id"],"campaign-two");
        assert_eq!(select(&live,&json!([]))["guides"],json!([]));
        live["sample"]["focus_tree_id"]=Value::Null;
        assert_eq!(select(&live,&catalog())["guides"].as_array().unwrap().len(),2);
    }
}
