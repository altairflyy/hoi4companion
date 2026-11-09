use super::*;
struct Adapter {
    facts: Map<String, Value>,
    source: String,
}
impl Adapter {
    fn put(&mut self, key: &str, data: Value, known: bool, calculated: bool, reason: &str) {
        let value = if known && !data.is_null() && valid_fact(key, &data) {
            super::known(
                data,
                if calculated { "calculated" } else { "observed" },
                if calculated {
                    "legacy_adapter"
                } else {
                    &self.source
                },
            )
        } else {
            unknown(reason)
        };
        self.facts.insert(key.into(), value);
    }
    fn field(&mut self, key: &str, value: &Value, known: bool, calculated: bool) {
        self.put(key, value.clone(), known, calculated, "Campo non osservato")
    }
    fn flatten(&mut self, path: &str, data: &Value, known: bool, calculated: bool) {
        if [
            "country.diplomacy.interactions",
            "country.intelligence.missions",
        ]
        .contains(&path)
        {
            return;
        }
        match data {
            Value::Object(map) => {
                let known = known
                    && (map.get("status").is_none()
                        || ["observed", "partial"].contains(&text(&data["status"])));
                for (key, child) in map {
                    if !["status", "reason"].contains(&key.as_str()) {
                        self.flatten(
                            &format!("{path}.{key}"),
                            child,
                            known,
                            calculated
                                || [
                                    "by_type",
                                    "by_template",
                                    "total",
                                    "effective_factories",
                                    "output_per_day",
                                    "assigned_factories",
                                    "category",
                                    "balance",
                                    "used",
                                    "owned",
                                    "controlled",
                                    "name",
                                ]
                                .contains(&key.as_str()),
                        );
                    }
                }
            }
            Value::Array(rows) => {
                self.field(&format!("{path}.count"), &json!(rows.len()), known, true);
                for (index, child) in rows.iter().enumerate() {
                    let identifier = child
                        .get("id")
                        .map(|v| match v {
                            Value::String(s) => s.clone(),
                            Value::Null => "None".into(),
                            Value::Bool(b) => if *b { "True" } else { "False" }.into(),
                            v => v.to_string(),
                        })
                        .unwrap_or_else(|| index.to_string());
                    self.flatten(&format!("{path}.{identifier}"), child, known, calculated);
                }
            }
            _ => {
                let stockpile =
                    path.starts_with("country.equipment.items.") && path.ends_with(".stockpile");
                let line = path.starts_with("country.production.lines.") && path.ends_with(".id");
                let construction = path.starts_with("country.construction.lines.")
                    && ["position", "location_status", "state_name"]
                        .contains(&path.rsplit('.').next().unwrap_or(""));
                self.field(
                    path,
                    data,
                    known,
                    calculated || stockpile || line || construction,
                );
            }
        }
    }
}
fn queued(section: &Value) -> Option<Value> {
    if section["status"] != "observed" {
        return None;
    }
    let rows = section["lines"].as_array()?;
    let integer =
        |v: &Value, min: f64| number(v) && v.as_f64().is_some_and(|n| n.fract() == 0. && n >= min);
    let mut keys = BTreeSet::new();
    let mut identities = BTreeSet::new();
    for row in rows {
        if !row.is_object()
            || !["id", "building_id"].iter().all(|k| nonempty(&row[k]))
            || !identities.insert(text(&row["id"]))
            || row["location_status"] != "observed"
            || !integer(&row["state_id"], 1.)
            || row.get("province_id").is_none()
            || !row["province_id"].is_null() && !integer(&row["province_id"], 1.)
            || ["amount", "active_factories", "built_level", "repair_levels"]
                .iter()
                .any(|k| !integer(&row[k], 0.))
            || !row["conversion"].is_boolean()
        {
            return None;
        }
        if row["amount"].as_f64()? > 0.
            && row["repair_levels"].as_f64()? == 0.
            && row["conversion"] == false
        {
            let key = text(&row["building_id"]);
            keys.insert(format!("building:{key}"));
            keys.insert(format!("state:{}:{key}", row["state_id"].as_f64()? as i64));
            if !row["province_id"].is_null() {
                keys.insert(format!(
                    "province:{}:{key}",
                    row["province_id"].as_f64()? as i64
                ));
            }
        }
    }
    Some(json!(keys))
}
fn campaign_id(sample: &Value) -> Option<&str> {
    let identity = &sample["campaign_identity"];
    let id = identity["id"].as_str()?;
    if identity["status"] != "observed"
        || identity["source"] != "native_game_unique_id_and_seed"
        || id.len() != 50
        || !id.starts_with("hoi4:")
        || id.as_bytes()[41] != b':'
    {
        return None;
    }
    let raw = &id[5..41];
    let seed = &id[42..];
    if raw.bytes().enumerate().any(|(i, b)| {
        if [8, 13, 18, 23].contains(&i) {
            b != b'-'
        } else {
            !b.is_ascii_digit() && !(b'a'..=b'f').contains(&b)
        }
    }) || raw.bytes().all(|b| b == b'0' || b == b'-')
        || !seed
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    Some(id)
}
fn project_rows(projects: &Value, matched: bool) -> bool {
    matched
        && projects["status"] == "observed"
        && projects["items"].as_array().is_some_and(|rows| {
            rows.len() <= 1024
                && rows.iter().all(|r| {
                    r.is_object()
                        && nonempty(&r["id"])
                        && r["active"].is_boolean()
                        && r["completed"].is_boolean()
                        && !(r["active"] == true && r["completed"] == true)
                })
                && rows
                    .iter()
                    .map(|r| text(&r["id"]))
                    .collect::<BTreeSet<_>>()
                    .len()
                    == rows.len()
        })
}
pub(super) fn normalize(value: &Value) -> Value {
    let empty = json!({});
    let sample = if value["usable"] == true && value["sample"].is_object() {
        &value["sample"]
    } else {
        &empty
    };
    let usable = sample.as_object().is_some_and(|v| !v.is_empty());
    let source = value["source"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or("process_memory");
    let mut adapter = Adapter {
        facts: Map::new(),
        source: source.into(),
    };
    adapter.field("country", &sample["country"], true, false);
    if let Some(date) = crate::events::calendar(sample) {
        adapter.facts.insert(
            "campaign.date".into(),
            known(json!(date), "calculated", "paradox_calendar_adapter"),
        );
    }
    if let Some(id) = campaign_id(sample) {
        adapter.facts.insert(
            "campaign.id".into(),
            known(json!(id), "observed", "native_game_unique_id_and_seed"),
        );
    }
    adapter.field("focus.tree", &sample["focus_tree_id"], true, true);
    adapter.field(
        "focus.completed",
        &sample["completed_focuses"],
        sample["focus_complete"] != false,
        false,
    );
    adapter.field(
        "research.completed",
        &sample["researched_technologies"],
        sample["research_complete"] != false,
        false,
    );
    for (key, field) in [
        ("focus.active", "current_focus"),
        ("focus.continuous", "continuous_focus"),
    ] {
        let active = if truth(&sample[field]) {
            json!([sample[field]])
        } else {
            json!([])
        };
        adapter.field(key, &active, sample.get(field).is_some(), false);
    }
    let slots = &sample["research_slots"];
    let valid_slots = valid_fact("research.slots", slots);
    adapter.field("research.slots", slots, valid_slots, false);
    adapter.field(
        "research.active",
        &json!(
            list(slots)
                .iter()
                .filter_map(|s| s["technology_id"].as_str().filter(|s| !s.is_empty()))
                .collect::<Vec<_>>()
        ),
        valid_slots,
        false,
    );
    adapter.field("focus.ids", &sample["focus_ids"], true, false);
    adapter.field("research.ids", &sample["technology_ids"], true, false);
    let overview = &sample["country_overview"];
    let country_known = nonempty(&sample["country"]);
    let matched = country_known && overview["country"] == sample["country"];
    let construction = if matched {
        queued(&overview["construction"])
    } else {
        None
    };
    adapter.facts.insert(
        "construction.queued".into(),
        construction
            .map(|q| known(q, "calculated", "construction_adapter"))
            .unwrap_or_else(|| unknown("Coda costruzioni non qualificata")),
    );
    let production = &overview["production"];
    let lines = &production["lines"];
    adapter.facts.insert(
        "production.lines".into(),
        if matched && production["status"] == "observed" && valid_lines(lines) {
            known(lines.clone(), "calculated", "production_adapter")
        } else {
            unknown("Linee di produzione non qualificate")
        },
    );
    let projects = &overview["projects"];
    for field in ["active", "completed"] {
        adapter.facts.insert(
            format!("projects.{field}"),
            if project_rows(projects, matched) {
                known(
                    json!(
                        list(&projects["items"])
                            .iter()
                            .filter(|r| r[field] == true)
                            .map(|r| &r["id"])
                            .collect::<Vec<_>>()
                    ),
                    "calculated",
                    "projects_adapter",
                )
            } else {
                unknown("Progetti speciali non qualificati")
            },
        );
    }
    let breakthroughs = &projects["breakthroughs"];
    let points = &breakthroughs["items"];
    if matched
        && breakthroughs["status"] == "observed"
        && points.as_array().is_some_and(|rows| {
            rows.len() <= 64
                && rows.iter().all(|r| {
                    r.is_object() && nonempty(&r["id"]) && integer_range(&r["available"], 0, 100000)
                })
                && rows
                    .iter()
                    .map(|r| text(&r["id"]))
                    .collect::<BTreeSet<_>>()
                    .len()
                    == rows.len()
        })
    {
        for row in list(points) {
            adapter.facts.insert(
                format!("projects.points.{}", text(&row["id"])),
                known(
                    row["available"].clone(),
                    "calculated",
                    "breakthrough_points_adapter",
                ),
            );
        }
    }
    let power = &overview["politics"]["power"];
    adapter.facts.insert(
        "country.politics.political_power".into(),
        if matched && power["status"] == "observed" && number(&power["available"]) {
            known(power["available"].clone(), "observed", source)
        } else {
            unknown("Potere politico non qualificato")
        },
    );
    adapter.facts.extend(memberships::political(if matched {
        overview
    } else {
        &empty
    }));
    if overview["country"] == sample["country"]
        && let Some(sections) = overview.as_object()
    {
        for (section, data) in sections {
            let Some(map) = data.as_object() else {
                continue;
            };
            if section == "manpower" {
                for (key, child) in map {
                    if !["status", "reason"].contains(&key.as_str()) {
                        adapter.field(
                            &format!("country.manpower.{key}"),
                            child,
                            ["observed", "partial"].contains(&text(&data["status"])),
                            key == "available",
                        );
                    }
                }
            } else if section == "fortifications" {
                let rows = &data["items"];
                let valid = data["status"] == "observed"
                    && rows.as_array().is_some_and(|a| {
                        a.len() <= 20000
                            && a.iter().all(|r| {
                                r.is_object()
                                    && integer_range(&r["id"], 1, 100000)
                                    && ["bunker", "coastal_bunker"]
                                        .iter()
                                        .all(|k| r.get(k).is_none_or(|v| integer_range(v, 0, 1000)))
                            })
                            && a.iter()
                                .map(|r| r["id"].to_string())
                                .collect::<BTreeSet<_>>()
                                .len()
                                == a.len()
                    });
                adapter.flatten(
                    "country.fortifications.items",
                    data.get("items").unwrap_or(&json!([])),
                    valid,
                    false,
                );
                if valid
                    && data["borders_status"] == "observed"
                    && let Some(borders) = data["borders"].as_object()
                {
                    for (key, provinces) in borders {
                        if tag(&json!(key))
                            && provinces.as_array().is_some_and(|a| {
                                a.len() <= 512
                                    && a.iter().all(|v| integer_range(v, 1, 100000))
                                    && unique(a)
                            })
                        {
                            adapter.facts.insert(
                                format!("fortifications.borders.{key}"),
                                known(
                                    provinces.clone(),
                                    "calculated",
                                    "native_controllers_and_map_topology",
                                ),
                            );
                        }
                    }
                }
            } else {
                let known = if ["experience", "diplomacy", "production"].contains(&section.as_str())
                {
                    data["status"] == "observed"
                } else {
                    ["observed", "partial"].contains(&text(&data["status"]))
                };
                adapter.flatten(&format!("country.{section}"), data, known, false);
            }
        }
    }
    adapter.facts.extend(memberships::foreign(if matched {
        overview
    } else {
        &empty
    }));
    let metrics = &overview["metrics"];
    for metric in ["war_support", "stability"] {
        let key = format!("country.metrics.{metric}");
        adapter.facts.insert(
            key.clone(),
            if matched && metrics["status"] == "observed" && valid_fact(&key, &metrics[metric]) {
                known(
                    metrics[metric].clone(),
                    "calculated",
                    "native_effective_country_metric",
                )
            } else {
                unknown("Country metric not qualified")
            },
        );
    }
    json!({"schema_version":1,"usable":usable,"session_id":sample["session_id"],"facts":adapter.facts})
}
