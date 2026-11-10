use super::*;
const FOREIGN: [&str; 7] = [
    "diplomacy.countries",
    "diplomacy.improving_relations",
    "diplomacy.non_aggression_pacts",
    "diplomacy.outgoing",
    "diplomacy.incoming",
    "intelligence.countries",
    "intelligence.active_missions",
];
const RELATION_KINDS: [&str; 18] = ["war","licence_production_access","embargo","military_access","offer_military_access","improve_relations","attache","boost_party_popularity","guarantee","non_aggression_pact","lend_lease","subject","air_base_access","offer_air_base_access","docking_rights","offer_docking_rights","naval_blockade","market_access_rights"];
const MISSIONS: [&str; 9] = [
    "no_mission",
    "build_intel_network",
    "quiet_network",
    "counter_intelligence",
    "root_out_resistance",
    "boost_ideology",
    "control_trade",
    "diplomatic_pressure",
    "propaganda",
];
const STATES: [&str; 6] = [
    "captured",
    "cooldown",
    "disbanding",
    "active",
    "operation",
    "killed",
];
fn tags(value: &Value) -> bool {
    value
        .as_array()
        .is_some_and(|a| a.len() <= 8192 && a.iter().all(tag) && unique(a))
}
pub(super) fn valid(key: &str, value: &Value) -> bool {
    if !FOREIGN.contains(&key) {
        return true;
    }
    if ["diplomacy.outgoing", "diplomacy.incoming"].contains(&key) {
        return value.as_array().is_some_and(|rows| rows.len() <= 8192 && unique(rows) && rows.iter().all(|v| {
            v.as_str().and_then(|s| s.split_once(':')).is_some_and(|(kind, target)| RELATION_KINDS.contains(&kind) && tag(&json!(target)))
        }));
    }
    if key != "intelligence.active_missions" {
        return tags(value);
    }
    value.as_array().is_some_and(|a| {
        a.len() <= 4096
            && unique(a)
            && a.iter().all(|v| {
                v.as_str()
                    .and_then(|s| s.split_once(':'))
                    .is_some_and(|(mission, target)| {
                        MISSIONS[1..].contains(&mission) && tag(&json!(target))
                    })
            })
    })
}
fn publish(
    facts: &mut Map<String, Value>,
    key: &str,
    values: impl IntoIterator<Item = String>,
    source: &str,
) {
    facts.insert(
        key.into(),
        known(
            json!(values.into_iter().collect::<BTreeSet<_>>()),
            "calculated",
            source,
        ),
    );
}
pub(super) fn foreign(overview: &Value) -> Map<String, Value> {
    let mut facts = FOREIGN
        .into_iter()
        .map(|k| (k.into(), unknown("Foreign interaction not qualified")))
        .collect::<Map<_, _>>();
    let dip = &overview["diplomacy"];
    let d = &dip["interactions"];
    let countries = &d["countries"];
    let relations = &d["relations"];
    if dip["status"] == "observed" && d["status"] == "observed" && tags(countries)
        && relations["status"] == "observed" && relations["source"] == "native_relation_base_serializer"
        && relations["items"].as_array().is_some_and(|rows| rows.len() <= 4096
            && rows.iter().map(|r| text(&r["id"])).collect::<BTreeSet<_>>().len() == rows.len()
            && rows.iter().all(|r| id(&r["id"]) && RELATION_KINDS.contains(&text(&r["kind"]))
                && ["from_country","to_country"].iter().all(|k| tag(&r[k]) && list(countries).contains(&r[k]))
                && r["from_country"] != r["to_country"]
                && match text(&r["direction"]) {
                    "outgoing" => r["from_country"] == overview["country"],
                    "incoming" => r["to_country"] == overview["country"],
                    _ => false,
                }
                && tags(&r["other_countries"]) && !list(&r["other_countries"]).is_empty()
                && list(&r["other_countries"]).contains(if r["direction"] == "outgoing" {&r["to_country"]} else {&r["from_country"]})
                && !list(&r["other_countries"]).contains(&overview["country"])
                && list(&r["other_countries"]).iter().all(|v| list(countries).contains(v))))
    {
        for direction in ["outgoing", "incoming"] {
            publish(&mut facts, &format!("diplomacy.{direction}"), list(&relations["items"]).iter()
                .filter(|r| r["direction"] == direction)
                .flat_map(|r| list(&r["other_countries"]).iter().map(|t| format!("{}:{}", text(&r["kind"]), text(t)))), "native_relation_adapter");
        }
    }
    if dip["status"] == "observed"
        && d["status"] == "observed"
        && tags(countries)
        && ["improving_relations", "non_aggression_pacts"]
            .iter()
            .all(|k| tags(&d[k]) && list(&d[k]).iter().all(|v| list(countries).contains(v)))
    {
        for key in ["countries", "improving_relations", "non_aggression_pacts"] {
            publish(
                &mut facts,
                &format!("diplomacy.{key}"),
                list(&d[key]).iter().map(|v| text(v).to_owned()),
                "foreign_interaction_adapter",
            );
        }
    }
    let intel = &overview["intelligence"];
    let m = &intel["missions"];
    let countries = &m["countries"];
    if intel["status"] == "observed"
        && m["status"] == "observed"
        && tags(countries)
        && m["items"].as_array().is_some_and(|rows| {
            rows.len() <= 4096
                && rows.iter().all(|r| {
                    r.is_object()
                        && r["id"]
                            .as_str()
                            .is_some_and(|s| !s.is_empty() && s.chars().count() <= 160)
                        && MISSIONS.contains(&text(&r["mission_id"]))
                        && STATES.contains(&text(&r["state"]))
                        && if r["mission_id"] == "no_mission" {
                            r["target_country"].is_null()
                        } else {
                            list(countries).contains(&r["target_country"])
                        }
                })
                && rows
                    .iter()
                    .map(|r| r["id"].to_string())
                    .collect::<BTreeSet<_>>()
                    .len()
                    == rows.len()
        })
    {
        publish(
            &mut facts,
            "intelligence.countries",
            list(countries).iter().map(|v| text(v).to_owned()),
            "foreign_interaction_adapter",
        );
        publish(
            &mut facts,
            "intelligence.active_missions",
            list(&m["items"])
                .iter()
                .filter(|r| r["state"] == "active" && r["mission_id"] != "no_mission")
                .map(|r| format!("{}:{}", text(&r["mission_id"]), text(&r["target_country"]))),
            "foreign_interaction_adapter",
        );
    }
    facts
}
pub(super) fn political(overview: &Value) -> Map<String, Value> {
    let mut result = [
        "doctrines.selected",
        "politics.ideas",
        "politics.advisors",
        "politics.characters",
        "politics.unit_leaders",
        "politics.scientists",
        "events.visible",
        "events.visible_options",
        "decisions.listed",
        "decisions.executed",
        "decisions.active",
        "decisions.completed",
    ]
    .into_iter()
    .map(|k| (k.into(), unknown("Sezione politica non qualificata")))
    .collect::<Map<_, _>>();
    let doctrine = &overview["doctrines"];
    if doctrine["status"] == "observed"
        && doctrine["items"].as_array().is_some_and(|a| {
            a.len() <= 4096
                && a.iter().all(|r| r.is_object() && id(&r["id"]))
                && a.iter()
                    .map(|r| r["id"].to_string())
                    .collect::<BTreeSet<_>>()
                    .len()
                    == a.len()
        })
    {
        publish(
            &mut result,
            "doctrines.selected",
            list(&doctrine["items"])
                .iter()
                .map(|r| text(&r["id"]).to_owned()),
            "politics_adapter",
        );
    }
    let politics = &overview["politics"];
    let characters = &politics["characters"];
    if ["observed", "partial"].contains(&text(&politics["status"]))
        && characters["status"] == "observed" && characters["source"] == "native_character_status_roles"
        && characters["items"].as_array().is_some_and(|rows| rows.len() <= 4096
            && rows.iter().all(|r| id(&r["id"]) && ["country_leader", "advisor", "unit_leader", "scientist"].iter().all(|k| r[k].is_boolean()))
            && rows.iter().map(|r| text(&r["id"])).collect::<BTreeSet<_>>().len() == rows.len())
    {
        for (key, role) in [("politics.characters", None), ("politics.unit_leaders", Some("unit_leader")), ("politics.scientists", Some("scientist"))] {
            publish(&mut result, key, list(&characters["items"]).iter().filter(|r| role.is_none_or(|k| r[k] == true)).map(|r| text(&r["id"]).to_owned()), "native_character_role_adapter");
        }
    }
    if ["observed", "partial"].contains(&text(&politics["status"])) {
        for part in ["ideas", "advisors"] {
            let section = &politics[part];
            let rows = &section["items"];
            if section["status"] != "observed"
                || rows.as_array().is_none_or(|a| {
                    a.len() > 4096
                        || a.iter().any(|r| {
                            !r.is_object()
                                || !id(&r["id"])
                                || if part == "ideas" {
                                    !id(&r["group"])
                                        || !id(&r["category"])
                                        || !r["is_law"].is_boolean()
                                } else {
                                    !(id(&r["character_id"])
                                        || (r.get("character_id") == Some(&Value::Null)
                                            && r["character_identity_status"] == "unavailable"
                                            && r["character_name"].as_str().is_some_and(|name| {
                                                !name.is_empty() && name.chars().count() <= 512
                                                    && !name.chars().any(char::is_control)
                                            })))
                                        || !integer_range(&r["slot"], 0, 2_000_000_000)
                                }
                        })
                        || a.iter()
                            .map(|r| r["id"].to_string())
                            .collect::<BTreeSet<_>>()
                            .len()
                            != a.len()
                })
            {
                continue;
            }
            publish(
                &mut result,
                &format!("politics.{part}"),
                list(rows).iter().map(|r| text(&r["id"]).to_owned()),
                "politics_adapter",
            );
        }
    }
    let events = &overview["events"];
    let rows = &events["visible"];
    if events["status"] == "observed"
        && rows.as_array().is_some_and(|a| {
            a.len() <= 256
                && a.iter()
                    .all(|r| r.is_object() && id(&r["instance_id"]) && id(&r["event_id"]))
                && a.iter()
                    .map(|r| r["instance_id"].to_string())
                    .collect::<BTreeSet<_>>()
                    .len()
                    == a.len()
        })
    {
        publish(
            &mut result,
            "events.visible",
            list(rows).iter().map(|r| text(&r["event_id"]).to_owned()),
            "politics_adapter",
        );
        if list(rows).iter().all(|r| {
            r["option_ids"]
                .as_array()
                .is_some_and(|a| a.len() <= 256 && a.iter().all(id) && unique(a))
        }) {
            publish(
                &mut result,
                "events.visible_options",
                list(rows).iter().flat_map(|r| {
                    list(&r["option_ids"])
                        .iter()
                        .map(move |v| format!("{}:{}", text(&r["event_id"]), text(v)))
                }),
                "politics_adapter",
            );
        }
    }
    let decisions = &overview["decisions"];
    let history = &decisions["execution_history"];
    if decisions["status"] == "observed" && history["status"] == "observed"
        && history["source"] == "native_decisions_taken_and_timed_state"
        && history["history_scope"] == "current_native_records"
    {
        for (field, key) in [("executed_ids", "decisions.executed"), ("active_ids", "decisions.active"), ("completed_ids", "decisions.completed")] {
            if history[field].as_array().is_some_and(|a| a.len() <= 4096 && a.iter().all(id) && unique(a)) {
                publish(&mut result, key, list(&history[field]).iter().map(|v| text(v).to_owned()), "native_decision_membership_adapter");
            }
        }
    }
    if decisions["status"] == "observed"
        && decisions["listed_ids"]
            .as_array()
            .is_some_and(|a| a.len() <= 4096 && a.iter().all(id) && unique(a))
    {
        publish(
            &mut result,
            "decisions.listed",
            list(&decisions["listed_ids"])
                .iter()
                .map(|v| text(v).to_owned()),
            "politics_adapter",
        );
    }
    result
}
