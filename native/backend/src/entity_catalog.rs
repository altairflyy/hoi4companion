//! Resolver identities derived from one installation generation. English names
//! identify entities; installed translations remain local display aliases.
use crate::{
    focus_text, operational_catalog::unique, recipient, script_document::entries,
    source_eligibility,
};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
fn adjective_tag(key: &str) -> Option<&str> {
    let tag = key.get(..3)?;
    let rest = &key[3..];
    (recipient::tag(tag) && rest.starts_with('_') && rest.ends_with("_ADJ")).then_some(tag)
}
pub fn adjectives(labels: &Value, localise: bool) -> BTreeMap<String, BTreeSet<String>> {
    let mut result: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (key, text) in labels.as_object().into_iter().flatten() {
        if let (Some(tag), Some(text)) = (adjective_tag(key), text.as_str()) {
            let value = if localise {
                focus_text::localise(text, labels)
            } else {
                text.into()
            };
            if !value.is_empty() && value.chars().count() <= 128 && !value.contains(['[', ']']) {
                result.entry(tag.into()).or_default().insert(value);
            }
        }
    }
    result
}
fn excluded_governments(node: &Value) -> Vec<String> {
    if node.get("entries").is_none() {
        return vec![];
    }
    fn gather(node: &Value, result: &mut BTreeSet<String>) {
        for (key, value) in entries(node) {
            if key == "has_government"
                && let Some(value) = value.as_str()
            {
                result.insert(value.into());
            } else if value.get("entries").is_some() {
                gather(value, result);
            }
        }
    }
    fn project(node: &Value, government: &str) -> Value {
        json!({"entries":entries(node).map(|(key,value)|{
        if key=="has_government"&&value.is_string(){json!(["always",if value==government{"yes"}else{"no"}])}
        else if ["AND","OR","NOT","NAND","NOR"].contains(&key.to_uppercase().as_str())&&value.get("entries").is_some(){json!([key.to_uppercase(),project(value,government)])}
        else if key=="always"{json!([key,value])}else{json!(["unobserved_runtime_predicate","unknown"])}
    }).collect::<Vec<_>>()})
    }
    let mut governments = BTreeSet::from([
        "communism".into(),
        "fascism".into(),
        "neutrality".into(),
        "democratic".into(),
    ]);
    gather(node, &mut governments);
    governments
        .into_iter()
        .filter(|g| {
            source_eligibility::condition(&project(node, g), "", &Value::Null, &json!({}))
                == Some(false)
        })
        .collect()
}
fn successors(tech: &Value, technologies: &Map<String, Value>) -> Vec<Value> {
    let targets = unique(
        tech["paths"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| p["target"].as_str())
            .map(|v| json!(v)),
    );
    let aligned: Vec<_> = targets
        .iter()
        .filter(|key| {
            technologies
                .get(key.as_str().unwrap())
                .is_some_and(|other| {
                    tech["folders"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|left| {
                            let x = &left["layout"];
                            let axis = match x["direction"].as_str() {
                                Some("UP") => "anchor_x",
                                Some("LEFT") => "anchor_y",
                                _ => return false,
                            };
                            if x["tree_id"].as_str().is_none_or(str::is_empty)
                                || !x[axis].is_number()
                            {
                                return false;
                            }
                            other["folders"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .any(|right| {
                                    let y = &right["layout"];
                                    left["id"] == right["id"]
                                        && x["tree_id"] == y["tree_id"]
                                        && x["direction"] == y["direction"]
                                        && x[axis].as_f64() == y[axis].as_f64()
                                })
                        })
                })
        })
        .cloned()
        .collect();
    if aligned.is_empty() { targets } else { aligned }
}
struct Names<'a> {
    labels: &'a Value,
    reference: &'a Value,
    technologies: Map<String, Value>,
    aliases: HashMap<String, Vec<Value>>,
}
impl Names<'_> {
    fn names(&self, key: &str, display: &str, text_key: &str) -> Value {
        let equipment: Vec<_> = self
            .technologies
            .get(key)
            .into_iter()
            .flat_map(|t| t["unlocks"].as_array().into_iter().flatten())
            .filter(|item| item["kind"] == "Equipaggiamento")
            .filter_map(|item| item["id"].as_str())
            .filter_map(|id| self.reference[id].as_str())
            .collect();
        let name = focus_text::localise(
            self.reference[if text_key.is_empty() { key } else { text_key }]
                .as_str()
                .unwrap_or(equipment.first().copied().unwrap_or(display)),
            self.reference,
        );
        let aliases = unique(
            std::iter::once(json!(display))
                .chain(self.labels.get(key).cloned())
                .chain(self.aliases.get(key).into_iter().flatten().cloned())
                .chain(
                    equipment
                        .iter()
                        .map(|text| json!(focus_text::localise(text, self.reference))),
                )
                .filter(|v| v.as_str().is_some_and(|s| !s.is_empty() && s != name)),
        );
        json!({"name":name,"localized_name":display,"aliases":aliases})
    }
}
pub fn entities(args: &Value) -> Result<Value, String> {
    let mut technologies = args["technologies"]
        .as_object()
        .cloned()
        .ok_or("Missing technology identities")?;
    let mut excluded: HashSet<String> = technologies
        .iter()
        .filter(|(_, t)| t["catalog_eligible"] == false)
        .map(|(k, _)| k.clone())
        .collect();
    excluded.extend(
        args["unavailable"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned),
    );
    technologies.retain(|k, _| !excluded.contains(k));
    let labels = &args["labels"];
    let reference = &args["reference"];
    let mut equipment = HashMap::new();
    for (key, tech) in &technologies {
        for item in tech["unlocks"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|i| i["kind"] == "Equipaggiamento")
        {
            if let Some(id) = item["id"].as_str() {
                equipment.insert(id, key);
            }
        }
    }
    let mut aliases: HashMap<String, Vec<Value>> = HashMap::new();
    for (english, texts) in [(true, reference), (false, labels)] {
        for (key, text) in texts.as_object().into_iter().flatten() {
            if key.as_bytes().get(3) != Some(&b'_')
                || !key.get(..3).is_some_and(recipient::tag)
                || key.len() < 5
            {
                continue;
            }
            let mut name = &key[4..];
            if name.ends_with("_short")
                && !technologies.contains_key(name)
                && !equipment.contains_key(name)
            {
                name = &name[..name.len() - 6];
            }
            let target = technologies
                .get_key_value(name)
                .map(|(k, _)| k)
                .or_else(|| equipment.get(name).copied());
            if let (Some(target), Some(text)) = (target, text.as_str()) {
                let items = aliases.entry(target.clone()).or_default();
                items.push(json!(focus_text::localise(text, texts)));
                let role = if name.starts_with("fighter_equipment_") {
                    Some("Fighter")
                } else if name.starts_with("CAS_equipment_") {
                    Some("CAS")
                } else {
                    None
                };
                let nickname = text
                    .split(|c| !hoi4companion_catalog_core::unicode_alnum(c))
                    .rfind(|s| !s.is_empty());
                if let (true, Some(role), Some(nickname)) = (english, role, nickname)
                    && nickname.chars().count() >= 4
                    && nickname
                        .chars()
                        .all(hoi4companion_catalog_core::unicode_alpha)
                    && text
                        .chars()
                        .any(hoi4companion_catalog_core::unicode_decimal)
                {
                    items.push(json!(format!("{role} {nickname}")));
                }
            }
        }
    }
    let names = Names {
        labels,
        reference,
        technologies,
        aliases,
    };
    let adjectives = adjectives(reference, true);
    let mut result = vec![];
    let trees: BTreeMap<_, _> = args["trees"]
        .as_object()
        .ok_or("Missing focus identities")?
        .iter()
        .collect();
    for (scope, tree) in trees {
        let focuses: BTreeMap<_, _> = tree["focuses"].as_object().into_iter().flatten().collect();
        for (key, focus) in focuses {
            let countries = tree.get("eligible_countries").cloned().unwrap_or(json!([]));
            let mut row = names.names(
                key,
                focus["name"].as_str().unwrap_or(key),
                focus["localization_key"].as_str().unwrap_or(""),
            );
            if row["name"]
                .as_str()
                .is_some_and(|s| s.contains("[ROOT.GetAdjective]"))
            {
                let variants: BTreeSet<_> = countries
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .flat_map(|tag| adjectives.get(tag).into_iter().flatten())
                    .map(|a| {
                        row["name"]
                            .as_str()
                            .unwrap()
                            .replace("[ROOT.GetAdjective]", a)
                    })
                    .collect();
                row["aliases"] = json!(unique(
                    row["aliases"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .cloned()
                        .chain(variants.into_iter().map(|s| json!(s)))
                ));
            }
            row.as_object_mut().unwrap().extend(json!({"kind":"focus","id":key,"scope":scope,"countries":countries,"prerequisites":focus.get("prerequisites").cloned().unwrap_or(json!([])),"mutually_exclusive":focus.get("mutually_exclusive").cloned().unwrap_or(json!([])),"bypass_possible":focus["bypass_possible"],"sequence_effects_safe":focus["sequence_effects_safe"],"external_completion_possible":focus["external_completion_possible"],"repeatable":focus.get("repeatable").cloned().unwrap_or(json!(false)),"excluded_governments":excluded_governments(&tree["branch_conditions"][key])}).as_object().unwrap().clone());
            result.push(row);
        }
    }
    let techs: BTreeMap<_, _> = names.technologies.iter().collect();
    for (key, tech) in techs {
        let mut row = names.names(key, tech["name"].as_str().unwrap_or(key), "");
        row.as_object_mut().unwrap().extend(json!({"kind":"technology","id":key,"upgrade_successors":successors(tech,&names.technologies).into_iter().filter(|v|v.as_str().is_some_and(|s|!excluded.contains(s))).collect::<Vec<_>>()}).as_object().unwrap().clone());
        result.push(row);
    }
    let continuous: BTreeMap<_, _> = args["continuous"]
        .as_object()
        .into_iter()
        .flatten()
        .collect();
    for (key, focus) in continuous {
        let mut row = names.names(key, focus["name"].as_str().unwrap_or(key), "");
        row.as_object_mut().unwrap().extend(json!({"kind":"focus","id":key,"countries":[],"scope_independent":true,"continuous":true,"bypass_possible":false,"sequence_effects_safe":true,"prerequisites":[],"mutually_exclusive":[]}).as_object().unwrap().clone());
        result.push(row);
    }
    Ok(json!(result))
}
pub fn export(entries: &Value, reference: &Value) -> Result<Value, String> {
    let mut english: HashSet<String> = reference
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    english.extend(
        english
            .iter()
            .filter(|s| s.contains('$') && s.chars().count() <= 4096)
            .map(|s| focus_text::localise(s, reference))
            .collect::<Vec<_>>(),
    );
    let normalized = |value: &str| {
        hoi4companion_catalog_core::casefold(
            &value
                .split(hoi4companion_catalog_core::unicode_space)
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" "),
        )
    };
    let mut result = vec![];
    for entry in entries.as_array().ok_or("Missing export identities")? {
        let mut row = entry.clone();
        let map = row.as_object_mut().ok_or("Invalid export identity")?;
        for field in [
            "geography",
            "production_ui_category",
            "bypass_possible",
            "sequence_effects_safe",
            "external_completion_possible",
            "repeatable",
        ] {
            map.remove(field);
        }
        let display = map.remove("localized_name");
        let name = map
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let same = normalized(&name);
        let keep = |value: &Value| {
            value
                .as_str()
                .is_some_and(|s| !s.is_empty() && normalized(s) != same && english.contains(s))
        };
        let aliases = unique(
            map.remove("aliases")
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default()
                .into_iter()
                .filter(keep),
        );
        if !aliases.is_empty() {
            map.insert("aliases".into(), json!(aliases));
        }
        if let Some(values) = map.remove("country_aliases") {
            let country: Map<_, _> = values
                .as_object()
                .into_iter()
                .flatten()
                .filter_map(|(k, v)| {
                    let values = unique(
                        v.as_array()
                            .into_iter()
                            .flatten()
                            .filter(|v| keep(v))
                            .cloned(),
                    );
                    (!values.is_empty()).then(|| (k.clone(), json!(values)))
                })
                .collect();
            if !country.is_empty() {
                map.insert("country_aliases".into(), json!(country));
            }
        }
        if let Some(Value::String(display)) = display
            && !display.is_empty()
            && display != name
            && !english.contains(&name)
            && (map.get("id") == Some(&json!(name))
                || !name.is_empty()
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_'))
        {
            map.insert("fallback_name".into(), json!(display));
        }
        result.push(row);
    }
    Ok(json!(result))
}

impl crate::catalog_generation::Catalog {
    pub fn identity_data(&mut self) -> Result<Value, String> {
        if let Some(value) = &self.identities {
            return Ok(value.clone());
        }
        let source = &self.sources;
        let entries = entities(
            &json!({"trees":source["trees"],"technologies":source["technologies"]["technologies"],"labels":source["labels"],"reference":source["reference_labels"],"unavailable":source["technologies"]["excluded_technologies"],"continuous":source["continuous_focuses"]}),
        )?;
        let mut entries = entries.as_array().cloned().unwrap_or_default();
        entries.extend(
            source["operation_entities"]
                .as_array()
                .into_iter()
                .flatten()
                .cloned(),
        );
        let excluded: HashSet<_> = source["excluded_focuses"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|r| Some((r["scope"].as_str()?, r["id"].as_str()?)))
            .collect();
        entries.retain(|row| {
            row["kind"] != "focus"
                || !excluded.contains(&(
                    row["scope"].as_str().unwrap_or(""),
                    row["id"].as_str().unwrap_or(""),
                ))
        });
        let expander = crate::scripted_names::Expander::new(
            &source["scripted_names"],
            &source["reference_labels"],
        );
        for row in &mut entries {
            let alternatives = expander.expand(
                row["name"].as_str().unwrap_or(""),
                &row["countries"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>(),
            );
            if !alternatives.is_empty() {
                row["aliases"] = json!(unique(
                    row["aliases"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .cloned()
                        .chain(alternatives.into_iter().map(|s| json!(s)))
                ));
            }
        }
        let mut countries: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut adjective_values = vec![];
        for (key, text) in source["reference_labels"].as_object().into_iter().flatten() {
            if let Some(text) = text.as_str() {
                if key.ends_with("_ADJ") {
                    adjective_values.push(text);
                }
                let Some(tag) = key.get(..3).filter(|s| recipient::tag(s)) else {
                    continue;
                };
                if !["", "_democratic", "_fascism", "_neutrality", "_communism"]
                    .contains(&&key[3..])
                    || text.contains(['[', '$'])
                {
                    continue;
                }
                let names = countries
                    .entry(tag.into())
                    .or_insert_with(|| BTreeSet::from([tag.into()]));
                names.insert(text.into());
                let words: Vec<_> = text
                    .split(|c: char| !c.is_ascii_alphabetic())
                    .filter(|s| !s.is_empty())
                    .collect();
                if (2..=4).contains(&words.len()) {
                    names.insert(
                        words
                            .iter()
                            .filter_map(|s| s.chars().next())
                            .collect::<String>()
                            .to_ascii_uppercase(),
                    );
                }
            }
        }
        let result =
            json!({"entries":entries,"country_names":countries,"adjectives":adjective_values});
        self.identities = Some(result.clone());
        Ok(result)
    }
}
