//! Source-language compilation only. Unknown methods/conditions stay explicit;
//! imported order, authored policies and stable source keys remain authoritative.
use crate::{
    action_blocks, command_families, event_instructions, guide_prepare,
    import_catalog::{Index, regex},
    informal_targets, political_binding, production_binding, production_milestones,
    production_revisions, reference_identity, timing_compiler, word_sources,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
type Result<T> = std::result::Result<T, String>;
fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn arr(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn change(row: &Value, keys: Value, actions: usize, unknown: bool) -> Value {
    json!({"key":row["key"],"actions":actions,"step_keys":keys,"unverifiable_timing":unknown})
}
fn base_key(key: &str) -> String {
    regex(r"(?:--operation-\d+)+$").replace(key, "").to_string()
}
fn generated(guide: &str, key: &str, text: &str) -> Vec<Value> {
    let base = base_key(key);
    let mut keys = vec![key];
    if base != key {
        keys.push(&base);
    }
    keys.into_iter().map(|source|{let hash=format!("{:x}",Sha256::digest(format!("{guide}\0{source}\0{text}").as_bytes()));json!({"op":"fact","key":format!("guide.conditions.{}",&hash[..24]),"test":"eq","value":true})}).collect()
}
fn policies(row: &mut Value, values: &Value) {
    if let Some(m) = values.as_object() {
        for (k, v) in m {
            if row.get(k).is_none() {
                row[k] = v.clone();
            }
        }
    }
}
fn instruction(row: &Value) -> Option<&str> {
    let instructions = arr(&row["action"]["params"]["instructions"]);
    (instructions.len() == 1)
        .then(|| instructions[0].as_str())
        .flatten()
}
struct Context<'a> {
    index: &'a Index,
    id: String,
    country: Value,
    scope: Value,
    focuses: HashSet<String>,
}
impl Context<'_> {
    fn condition(&self, text: &Value, params: &Value) -> Result<Option<Value>> {
        if let Some(value) = timing_compiler::compile(
            text,
            Some(self.index),
            &self.country,
            &self.scope,
            s(&params["cost_currency"]),
            0,
        )? {
            return Ok(Some(value));
        }
        let Some(hit) = text
            .as_str()
            .and_then(|text| regex(r"(?i)^(on|after|once)\s+(.+?)(?:…|\.{3})$").captures(text))
        else {
            return Ok(None);
        };
        let needle = hoi4companion_catalog_core::normal_name(&hit[2]);
        let found = self
            .index
            .rows()
            .iter()
            .filter(|r| {
                r["kind"] == "focus"
                    && self.focuses.contains(s(&r["id"]))
                    && (std::iter::once(&r["name"]).chain(arr(&r["aliases"])))
                        .filter_map(Value::as_str)
                        .any(|n| hoi4companion_catalog_core::normal_name(n).starts_with(&needle))
            })
            .map(|r| s(&r["id"]).to_owned())
            .collect::<HashSet<_>>();
        if found.len() != 1 {
            return Ok(None);
        }
        let target = found.into_iter().next().unwrap();
        let done = json!({"op":"fact","key":"focus.completed","test":"contains","value":target});
        Ok(Some(if hit[1].eq_ignore_ascii_case("on") {
            json!({"op":"any","args":[{"op":"fact","key":"focus.active","test":"contains","value":target},done]})
        } else {
            done
        }))
    }
}
struct Pieces<'a, 'b> {
    items: Vec<Value>,
    row: &'a Value,
    params: &'a Value,
    timing: Option<String>,
    context: &'a Context<'b>,
}
impl Pieces<'_, '_> {
    fn push(&mut self, text: &str, action: Option<&Value>, cost: Option<&Value>) -> Result<usize> {
        let mut item = self.row.clone();
        item["title"] = json!(text);
        item["action"] = json!({"kind":"manual","params":self.params});
        item["action"]["params"]["instructions"] = json!([text]);
        if let Some(action) = action {
            item.as_object_mut().unwrap().remove("tracking");
            item["action"]["params"]
                .as_object_mut()
                .ok_or("Invalid operation parameters")?
                .extend(
                    action
                        .as_object()
                        .ok_or("Invalid command parameters")?
                        .clone(),
                );
            if let Some(cost) = cost
                && item.get("political_power_cost").is_none()
            {
                item["political_power_cost"] = cost.clone();
            }
        }
        if let Some(timing) = &self.timing {
            if item["action"]["params"].get("timing").is_none() {
                item["action"]["params"]["timing"] = json!(timing);
            }
            let placeholders = generated(&self.context.id, s(&self.row["key"]), timing);
            if item.get("when").is_none() || placeholders.contains(&item["when"]) {
                item["when"] = self
                    .context
                    .condition(&json!(timing), &item["action"]["params"])?
                    .unwrap_or_else(|| placeholders[0].clone());
            }
            let mut notes = vec![];
            for note in [s(&self.row["note"]), timing.as_str()] {
                if !note.is_empty() && !notes.contains(&note) {
                    notes.push(note);
                }
            }
            item["note"] = json!(notes.join("\n"));
        }
        let index = self.items.len();
        self.items.push(item);
        Ok(index)
    }
    fn auxiliary(&mut self, index: usize) {
        self.items[index]["_auxiliary_vocabulary_note"] = json!(true);
    }
}
fn quotes(
    row: &Value,
    sources: Option<&HashSet<String>>,
    contains: Option<&str>,
) -> HashSet<(String, String)> {
    arr(&row["evidence"])
        .iter()
        .filter_map(|e| {
            let (id, quote) = (e["source_id"].as_str()?, e["quote"].as_str()?);
            (sources.is_none_or(|sources| sources.contains(id))
                && contains.is_none_or(|i| quote.contains(i)))
            .then(|| (id.into(), quote.into()))
        })
        .collect()
}
fn repair_residuals(
    guide: &mut Value,
    index: &Index,
    library: &Value,
    variants: &Value,
    changes: &mut Vec<Value>,
) -> Result<()> {
    let sources = arr(&guide["sources"])
        .iter()
        .filter(|r| r["type"] == "file" && s(&r["text"]).contains("[Table row]"))
        .map(|r| s(&r["id"]).to_owned())
        .collect::<HashSet<_>>();
    let country = guide["applies_to"]["country"].clone();
    for lane in guide["tracks"]
        .as_object_mut()
        .ok_or("Missing guide tracks")?
        .values_mut()
    {
        let rows = lane.as_array_mut().ok_or("Invalid guide track")?;
        let mut parents = vec![];
        for row in rows.iter() {
            if row["action"]["params"]["category"] != "production" {
                continue;
            }
            let Some(text) = instruction(row) else {
                continue;
            };
            let quotes = quotes(row, Some(&sources), Some(text));
            if quotes.is_empty() {
                continue;
            }
            if let Some(parsed) =
                informal_targets::production(text, index, &country, library, variants)?
            {
                parents.push((row["key"].clone(), quotes, parsed));
            }
        }
        let original = std::mem::take(rows);
        for mut row in original {
            let params = &row["action"]["params"];
            let Some(text) = instruction(&row) else {
                rows.push(row);
                continue;
            };
            if row["tracking"] != "reminder"
                || !regex(r"--operation-\d+$").is_match(s(&row["key"]))
                || !regex(r"(?i)^(MIL|NAV|Production)\s*:").is_match(text)
            {
                rows.push(row);
                continue;
            }
            let evidence = quotes(&row, None, None);
            let candidates = parents
                .iter()
                .filter(|(key, quotes, _)| {
                    *key != row["key"]
                        && base_key(s(key)) == base_key(s(&row["key"]))
                        && !evidence.is_empty()
                        && evidence.is_subset(quotes)
                        && evidence.iter().all(|(_, q)| !q.contains(text))
                })
                .collect::<Vec<_>>();
            if candidates.len() != 1 {
                rows.push(row);
                continue;
            }
            let (parent, _, parsed) = candidates[0];
            if parsed["remainder"].as_str().is_none_or(|s| s.is_empty()) {
                changes.push(change(&row, json!([parent]), 0, false));
                continue;
            }
            let remainder = &parsed["remainder"];
            if params["instructions"] != json!([remainder])
                || params["production_residual_for"] != *parent
            {
                row["action"]["params"]["instructions"] = json!([remainder]);
                row["action"]["params"]["production_residual_for"] = parent.clone();
                row["title"] = remainder.clone();
                row["name"] = remainder.clone();
                changes.push(change(&row, json!([row["key"]]), 0, false));
            }
            rows.push(row);
        }
    }
    Ok(())
}
fn production_signature(params: &Value) -> Value {
    let mut result = json!({});
    for key in [
        "production_selector",
        "requested_factories",
        "assigned_factories",
        "quantity_mode",
        "production_priority",
        "assignment_test",
    ] {
        result[key] = params[key].clone();
    }
    guide_prepare::canonical(&result)
}
pub fn promote(guide: &Value, index: &Index, library: &Value) -> Result<Value> {
    if guide["dependency_policy"] != "explicit" {
        return Ok(json!({"guide":guide,"changes":[]}));
    }
    let blocks = action_blocks::annotate(guide)?;
    let mut guide = blocks["guide"].clone();
    let mut changes = arr(&blocks["changes"]).to_vec();
    let variants = reference_identity::bindings(&guide, index, library)?;
    let context = Context {
        index,
        id: s(&guide["id"]).into(),
        country: guide["applies_to"]["country"].clone(),
        scope: guide["applies_to"]["tree_id"].clone(),
        focuses: guide["tracks"]
            .as_object()
            .ok_or("Missing guide tracks")?
            .values()
            .flat_map(arr)
            .filter(|r| r["action"]["kind"] == "select_focus")
            .map(|r| s(&r["action"]["entity"]["id"]).to_owned())
            .collect(),
    };
    let mut all_keys = guide["tracks"]
        .as_object()
        .unwrap()
        .values()
        .flat_map(arr)
        .map(|r| s(&r["key"]).to_owned())
        .collect::<HashSet<_>>();
    repair_residuals(&mut guide, index, library, &variants, &mut changes)?;
    for lane in guide["tracks"].as_object_mut().unwrap().values_mut() {
        let rows = lane.as_array_mut().ok_or("Invalid guide track")?;
        let original = std::mem::take(rows);
        for mut row in original {
            if row["action"]["params"]
                .get("production_residual_for")
                .is_some_and(crate::state::truth)
            {
                rows.push(row);
                continue;
            }
            if row["action"]["params"]["category"] == "production" && row["tracking"] != "reminder"
            {
                let bound =
                    production_binding::bind(&row, index, &context.country, library, &variants)?;
                if [
                    "production_layout",
                    "production_live_assignment",
                    "production_remaining_assignment",
                    "production_relative_assignment",
                ]
                .contains(&s(&bound["reason"]))
                    && row != bound["step"]
                {
                    row = bound["step"].clone();
                    changes.push(change(&row, json!([row["key"]]), 0, false));
                }
                if let Some(text) = instruction(&row)
                    && row["action"]["params"]["production_targets"].is_array()
                    && let Some(parsed) = informal_targets::production(
                        text,
                        index,
                        &context.country,
                        library,
                        &variants,
                    )?
                {
                    let old = arr(&row["action"]["params"]["production_targets"]);
                    let new = arr(&parsed["params"]["production_targets"]);
                    let signatures = new.iter().map(production_signature).collect::<Vec<_>>();
                    if new.len() > old.len()
                        && old
                            .iter()
                            .all(|p| signatures.contains(&production_signature(p)))
                    {
                        row["action"]["params"]["production_targets"] = json!(new);
                        changes.push(change(&row, json!([row["key"]]), 0, false));
                    }
                }
            }
            if let Some(known) =
                context.condition(&row["action"]["params"]["timing"], &row["action"]["params"])?
            {
                let previous = generated(
                    &context.id,
                    s(&row["key"]),
                    s(&row["action"]["params"]["timing"]),
                );
                if (row.get("when").is_none() || previous.contains(&row["when"]))
                    && row["when"] != known
                {
                    row["when"] = known;
                    changes.push(change(&row, json!([row["key"]]), 0, false));
                }
            }
            let params = row["action"]["params"].clone();
            let instructions = arr(&params["instructions"]);
            if row["tracking"] != "reminder"
                || row["action"]["kind"] != "manual"
                || instructions.is_empty()
                || instructions.iter().any(|v| !v.is_string())
            {
                rows.push(row);
                continue;
            }
            let mut pieces = Pieces {
                items: vec![],
                row: &row,
                params: &params,
                timing: None,
                context: &context,
            };
            let mut promoted = 0;
            for instruction in instructions {
                for raw in
                    regex(r"\r\n|[\n\r\x0B\x0C\x1C-\x1E\u0085\u2028\u2029]").split(s(instruction))
                {
                    let mut text = regex(r"^\s*[-•]\s*").replace(raw, "").trim().to_owned();
                    if text.is_empty() {
                        continue;
                    }
                    let original_text = text.clone();
                    if let Some(event) =
                        event_instructions::command(&json!(text), index, &context.country)?
                    {
                        let i = pieces.push(&text, Some(&event["params"]), None)?;
                        policies(&mut pieces.items[i], &event["policies"]);
                        promoted += 1;
                        continue;
                    }
                    let timing = regex(
                        r"(?i)^((?:on|after|before|shortly before|when|once|at)\b[^:]+):\s*(.*)$",
                    );
                    let hit = timing.captures(&text);
                    if hit.is_none()&&regex(r"(?i)^(?:on|after|before|shortly before|when|once)\s+[^:\n]+(?:…|\.{3})$").is_match(&text){pieces.timing=Some(text);continue;}
                    if let Some(hit) = hit {
                        pieces.timing = Some(hit[1].trim().into());
                        text = hit[2].trim().into();
                        if text.is_empty() {
                            continue;
                        }
                        if let Some(event) =
                            event_instructions::command(&json!(text), index, &context.country)?
                        {
                            let i = pieces.push(&text, Some(&event["params"]), None)?;
                            policies(&mut pieces.items[i], &event["policies"]);
                            promoted += 1;
                            continue;
                        }
                    }
                    let foreign = command_families::foreign_prefix(&text, index)?;
                    if !arr(&foreign[0]).is_empty() {
                        let base = pieces.items.len();
                        for (j, command) in arr(&foreign[0]).iter().enumerate() {
                            let i =
                                pieces.push(s(&command["text"]), Some(&command["params"]), None)?;
                            if j > 0 {
                                pieces.auxiliary(i);
                            }
                            pieces.items[i]["_foreign_after"] = json!(
                                arr(&command["after_keys"])
                                    .iter()
                                    .map(|v| base + v.as_u64().unwrap_or(0) as usize)
                                    .collect::<Vec<_>>()
                            );
                        }
                        if !s(&foreign[1]).is_empty() {
                            let i = pieces.push(s(&foreign[1]), None, None)?;
                            pieces.auxiliary(i);
                        }
                        promoted += arr(&foreign[0]).len();
                        continue;
                    }
                    if let Some(fortified) = command_families::fortification(&text, index) {
                        pieces.push(
                            text.split(',').next().unwrap_or(""),
                            Some(&fortified["params"]),
                            None,
                        )?;
                        if !s(&fortified["remainder"]).is_empty() {
                            let i = pieces.push(s(&fortified["remainder"]), None, None)?;
                            pieces.auxiliary(i);
                        }
                        promoted += 1;
                        continue;
                    }
                    if let Some(commands) =
                        informal_targets::sequence(&text, index, &context.country)?
                    {
                        let mut previous: Option<Value> = None;
                        for command in arr(&commands) {
                            let i =
                                pieces.push(s(&command["text"]), Some(&command["params"]), None)?;
                            policies(&mut pieces.items[i], &command["policies"]);
                            if command["after_previous"] == true {
                                pieces.items[i]["_sequence_after"] = json!([
                                    i.saturating_sub(1),
                                    previous.as_ref().ok_or("Missing sequence predecessor")?["milestone"]
                                ]);
                            }
                            previous = Some(command.clone());
                            promoted += 1;
                        }
                        continue;
                    }
                    if let Some(law) =
                        word_sources::explicit_action(&text).filter(|a| a["type"] == "law")
                    {
                        let bound = political_binding::bind(
                            &json!({"action":{"kind":"manual","params":{"category":"politics","law_name":law["target"]}}}),
                            index,
                            &context.country,
                            false,
                        )?;
                        if bound["reason"] == "politics_live_law" {
                            pieces.push(
                                &text,
                                Some(&bound["step"]["action"]["params"]),
                                law.get("political_power_cost"),
                            )?;
                            promoted += 1;
                        } else {
                            pieces.push(&original_text, None, None)?;
                        }
                        continue;
                    }
                    let agency = regex(
                        r"(?i)^(?:create|form)\s+(?:an?\s+)?(?:intelligence\s+)?agency(?:\s*(&\s*setup\s+done)|\s*\(([^()]+)\))?$",
                    );
                    if let Some(agency) = agency.captures(&text) {
                        pieces.push("Create intelligence agency",Some(&json!({"category":"projects","operation_id":"create_intelligence_agency"})),None)?;
                        promoted += 1;
                        if let Some(tail) = agency.get(1) {
                            pieces.push(
                                tail.as_str().trim_start_matches(['&', ' ']).trim(),
                                None,
                                None,
                            )?;
                        }
                        if let Some(tail) = agency.get(2) {
                            for clause in tail.as_str().split(',') {
                                if let Some(commands) =
                                    command_families::foreign(clause.trim(), index)?
                                {
                                    for command in arr(&commands) {
                                        let i = pieces.push(
                                            s(&command["text"]),
                                            Some(&command["params"]),
                                            None,
                                        )?;
                                        pieces.auxiliary(i);
                                        promoted += 1;
                                    }
                                } else {
                                    let i = pieces.push(clause.trim(), None, None)?;
                                    pieces.auxiliary(i);
                                }
                            }
                        }
                        continue;
                    }
                    if let Some(production) = informal_targets::production(
                        &text,
                        index,
                        &context.country,
                        library,
                        &variants,
                    )? {
                        pieces.push(&text, Some(&production["params"]), None)?;
                        promoted += 1;
                        if !s(&production["remainder"]).is_empty() {
                            pieces.push(s(&production["remainder"]), None, None)?;
                        }
                        continue;
                    }
                    if let Some(doctrine) =
                        command_families::doctrine(&text, index, &context.country)?
                    {
                        if !doctrine["prefix"].is_null() {
                            let prefix = &doctrine["prefix"];
                            let i =
                                pieces.push(s(&prefix["text"]), Some(&prefix["params"]), None)?;
                            policies(&mut pieces.items[i], &prefix["policies"]);
                        }
                        let i =
                            pieces.push(s(&doctrine["text"]), Some(&doctrine["params"]), None)?;
                        policies(&mut pieces.items[i], &doctrine["policies"]);
                        if !s(&doctrine["remainder"]).is_empty() {
                            let i = pieces.push(s(&doctrine["remainder"]), None, None)?;
                            pieces.auxiliary(i);
                        }
                        promoted += 1;
                        continue;
                    }
                    if let Some(named) = informal_targets::named(&text, index, &context.country)? {
                        let previous = pieces.timing.clone();
                        if !s(&named["timing"]).is_empty() {
                            pieces.timing = Some(
                                previous
                                    .as_deref()
                                    .into_iter()
                                    .chain(std::iter::once(s(&named["timing"])))
                                    .filter(|s| !s.is_empty())
                                    .collect::<Vec<_>>()
                                    .join("; "),
                            );
                        }
                        let i = pieces.push(&text, Some(&named["params"]), None)?;
                        policies(&mut pieces.items[i], &named["policies"]);
                        pieces.timing = previous;
                        promoted += 1;
                        continue;
                    }
                    pieces.push(&original_text, None, None)?;
                }
            }
            if promoted == 0 {
                rows.push(row);
                continue;
            }
            let unknown = pieces.timing.as_ref().is_some_and(|t| !t.is_empty())
                && context.condition(&json!(pieces.timing), &params)?.is_none()
                && row.get("when").is_none();
            let mut keys = vec![];
            let (mut core, mut notes) = (0, 0);
            for item in &mut pieces.items {
                if item
                    .as_object_mut()
                    .unwrap()
                    .remove("_auxiliary_vocabulary_note")
                    == Some(json!(true))
                {
                    notes += 1;
                    while all_keys.contains(&format!("{}--note-{notes}", s(&row["key"]))) {
                        notes += 1;
                    }
                    keys.push(format!("{}--note-{notes}", s(&row["key"])));
                } else {
                    core += 1;
                    keys.push(if core == 1 {
                        s(&row["key"]).to_owned()
                    } else {
                        format!("{}--operation-{core}", s(&row["key"]))
                    });
                }
            }
            if pieces.items.len() > 512 || keys.iter().skip(1).any(|k| all_keys.contains(k)) {
                rows.push(row);
                continue;
            }
            for (item, key) in pieces.items.iter_mut().zip(&keys) {
                item["key"] = json!(key);
                let foreign_after = item
                    .as_object_mut()
                    .unwrap()
                    .remove("_foreign_after")
                    .unwrap_or(json!([]));
                if !arr(&foreign_after).is_empty() {
                    let mut dependencies = arr(&item["depends_on"]).to_vec();
                    for index in arr(&foreign_after) {
                        let key = keys
                            .get(index.as_u64().ok_or("Invalid source dependency")? as usize)
                            .ok_or("Missing source dependency")?;
                        if !dependencies.contains(&json!(key)) {
                            dependencies.push(json!(key));
                        }
                    }
                    item["depends_on"] = json!(dependencies);
                }
                if let Some(dependency) = item.as_object_mut().unwrap().remove("_sequence_after") {
                    let field = if dependency[1] == "start" {
                        "depends_on_started"
                    } else {
                        "depends_on"
                    };
                    let mut dependencies = arr(&item[field]).to_vec();
                    let key = keys
                        .get(
                            dependency[0]
                                .as_u64()
                                .ok_or("Invalid sequence dependency")?
                                as usize,
                        )
                        .ok_or("Missing sequence dependency")?;
                    if !dependencies.contains(&json!(key)) {
                        dependencies.push(json!(key));
                    }
                    item[field] = json!(dependencies);
                }
            }
            rows.extend(pieces.items);
            all_keys.extend(keys.iter().cloned());
            changes.push(change(&row, json!(keys), promoted, unknown));
        }
    }
    let attached = reference_identity::attach_images(&guide, library)?;
    guide = attached["guide"].clone();
    changes.extend_from_slice(arr(&attached["changes"]));
    let revisions = production_revisions::annotate(&guide)?;
    guide = revisions["guide"].clone();
    changes.extend_from_slice(arr(&revisions["changes"]));
    changes.extend(production_milestones::annotate(&mut guide)?);
    Ok(json!({"guide":guide,"changes":changes}))
}
