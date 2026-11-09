use super::*;
const FOREIGN: [&str; 5] = [
    "diplomacy.countries",
    "diplomacy.improving_relations",
    "diplomacy.non_aggression_pacts",
    "intelligence.countries",
    "intelligence.active_missions",
];
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
        "events.visible",
        "events.visible_options",
        "decisions.listed",
        "decisions.executed",
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
                                    !id(&r["character_id"])
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
