//! Small authoring v1 compiler. No strategy inference, observations or I/O.
use crate::guide_schema;
use regex::Regex;
use serde_json::{Map, Value, json};
use std::{collections::BTreeSet, sync::OnceLock};
type Result<T> = std::result::Result<T, String>;
fn specs() -> &'static Value {
    static SPECS: OnceLock<Value> = OnceLock::new();
    SPECS.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../src/hoi4companion/authoring-actions-v1.json"
        ))
        .expect("packaged authoring contract")
    })
}
fn set(node: &Value) -> BTreeSet<String> {
    node.as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}
fn keys(node: &Value) -> BTreeSet<String> {
    node.as_object()
        .into_iter()
        .flatten()
        .map(|(k, _)| k.clone())
        .collect()
}
fn err<T>(key: &str, message: impl AsRef<str>) -> Result<T> {
    Err(format!("{key}: {}", message.as_ref()))
}
fn text<'a>(value: &'a Value, key: &str, field: &str) -> Result<&'a str> {
    value
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.chars().count() <= 4096)
        .ok_or_else(|| format!("{key}: {field} mancante o non valido"))
}
fn integer(value: &Value, min: u64) -> bool {
    value
        .as_u64()
        .is_some_and(|n| (min..=9007199254740991).contains(&n))
}
fn reference(value: &Value, key: &str, field: &str) -> Result<(bool, String, Option<String>)> {
    if value.is_string() {
        return Ok((false, text(value, key, field)?.into(), None));
    }
    if !value.is_object()
        || keys(value)
            .iter()
            .any(|s| !["id", "name", "scope"].contains(&s.as_str()))
        || (value.get("id").is_some() == value.get("name").is_some())
    {
        return err(
            key,
            format!("{field}: usa un nome oppure un riferimento con id o name"),
        );
    }
    let explicit = value.get("id").is_some();
    let target = &value[if explicit { "id" } else { "name" }];
    let target = if field == "state" && explicit && integer(target, 1) {
        target.to_string()
    } else {
        text(target, key, field)?.into()
    };
    let scope = value
        .get("scope")
        .map(|v| text(v, key, &format!("{field}.scope")).map(str::to_owned))
        .transpose()?;
    Ok((explicit, target, scope))
}
fn put(
    params: &mut Map<String, Value>,
    value: &Value,
    key: &str,
    field: &str,
    id: &str,
    name: &str,
) -> Result<()> {
    let (explicit, target, scope) = reference(value, key, field)?;
    if scope.is_some() {
        return err(key, format!("{field}: scope ammesso solo per focus"));
    }
    params.insert(if explicit { id } else { name }.into(), json!(target));
    Ok(())
}
fn ui_category(value: &Value, key: &str) -> Result<(String, Option<String>)> {
    let label = text(value, key, "category")?;
    if label.chars().count() > 128 {
        return err(key, "category troppo lunga");
    }
    let mut token = String::new();
    for c in label.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            token.push(c);
        } else if !token.ends_with('_') {
            token.push('_');
        }
    }
    let token = token.trim_matches('_');
    let group = if [
        "construction",
        "production",
        "politics",
        "characters",
        "army",
        "templates",
        "projects",
        "checkpoints",
    ]
    .contains(&token)
    {
        token
    } else {
        specs()["category_aliases"][token]
            .as_str()
            .unwrap_or("checkpoints")
    };
    Ok((group.into(), (label != group).then(|| label.to_owned())))
}
fn details(step: &Value, key: &str, generic: bool) -> Result<Map<String, Value>> {
    let mut value = step.get("details").cloned().unwrap_or(json!({}));
    if !value.is_object() {
        return err(key, "details deve essere un oggetto");
    }
    for (alias, field) in specs()["detail_aliases"][step["type"].as_str().unwrap_or("")]
        .as_object()
        .into_iter()
        .flatten()
    {
        if let Some(original) = value.get(alias).cloned() {
            let field = field.as_str().unwrap();
            if let Some(existing) = value.get(field)
                && *existing != original
            {
                return err(key, format!("details: {alias} e {field} in conflitto"));
            }
            value.as_object_mut().unwrap().remove(alias);
            value[field] = original;
        }
    }
    let mut forbidden = set(&specs()["generated"]);
    forbidden.extend(
        [
            "category",
            "timing",
            "instructions",
            "source_seconds",
            "game_date",
        ]
        .map(str::to_owned),
    );
    if !generic {
        forbidden.extend(set(&specs()["identities"]));
    }
    let conflict: Vec<_> = keys(&value).intersection(&forbidden).cloned().collect();
    if !conflict.is_empty() || keys(&value).iter().any(|k| k.starts_with("guide.")) {
        return err(
            key,
            format!(
                "details contiene campi riservati o in conflitto: {}",
                conflict.join(", ")
            ),
        );
    }
    fn visit(value: &Value) -> bool {
        match value {
            Value::Object(map) => {
                map.keys().any(|s| {
                    ["verification", "production_selector", "political_binding"]
                        .contains(&s.as_str())
                }) || map.values().any(visit)
            }
            Value::Array(a) => a.iter().any(visit),
            _ => false,
        }
    }
    if visit(&value) {
        return err(key, "details contiene campi interni generati");
    }
    Ok(value.as_object().unwrap().clone())
}
fn factories(value: &Value, key: &str) -> Result<(&'static str, Option<Value>, &'static str)> {
    if value.is_object() && value["mode"] == "remaining" {
        if keys(value)
            .iter()
            .any(|s| !["mode", "basis"].contains(&s.as_str()))
            || value.get("basis").unwrap_or(&json!("requested")) != "requested"
        {
            return err(key, "factories remaining: senza value, basis requested");
        }
        return Ok(("requested_factories", None, "remaining"));
    }
    let basis = value
        .get("basis")
        .and_then(Value::as_str)
        .unwrap_or("requested");
    let mode = value.get("mode").and_then(Value::as_str).unwrap_or("total");
    if !value.is_object()
        || keys(value)
            .iter()
            .any(|s| !["value", "basis", "mode"].contains(&s.as_str()))
        || !integer(&value["value"], 0)
        || !["requested", "assigned"].contains(&basis)
        || !["total", "add"].contains(&mode)
        || value.get("basis").is_some_and(|v| !v.is_string())
        || value.get("mode").is_some_and(|v| !v.is_string())
    {
        return err(
            key,
            "factories: richiesti value intero, basis requested/assigned e mode total/add",
        );
    }
    if mode == "add" && value["value"] == 0 {
        return err(key, "factories: un incremento deve essere positivo");
    }
    Ok((
        if basis == "requested" {
            "requested_factories"
        } else {
            "assigned_factories"
        },
        Some(value["value"].clone()),
        if mode == "add" {
            "additional"
        } else {
            "target_total"
        },
    ))
}
fn production(value: &Value, key: &str) -> Result<Value> {
    let allowed = [
        "equipment",
        "factories",
        "variant",
        "production_category",
        "production_priority",
    ];
    if !value.is_object()
        || keys(value).iter().any(|s| !allowed.contains(&s.as_str()))
        || value.get("equipment").is_none()
        || value.get("factories").is_none()
    {
        return err(
            key,
            "produzione: servono equipment e factories; campi ammessi equipment, factories, production_category, production_priority, variant",
        );
    }
    let mut params = Map::new();
    put(
        &mut params,
        &value["equipment"],
        key,
        "equipment",
        "equipment_id",
        "equipment_name",
    )?;
    let (field, count, mode) = factories(&value["factories"], key)?;
    if mode == "remaining" {
        params.insert("quantity_mode".into(), json!("remaining"));
        params.insert("production_category".into(), json!("military"));
    } else {
        params.insert(field.into(), count.unwrap());
    }
    if mode == "additional" {
        params.insert("quantity_mode".into(), json!(mode));
    }
    if let Some(value) = value.get("variant") {
        params.insert("variant_name".into(), json!(text(value, key, "variant")?));
    }
    if let Some(category) = value.get("production_category") {
        if ![Some("military"), Some("naval")].contains(&category.as_str()) {
            return err(key, "production_category: military oppure naval");
        }
        if mode == "remaining" && category != "military" {
            return err(key, "remaining richiede produzione military");
        }
        params.insert("production_category".into(), category.clone());
    }
    if let Some(priority) = value.get("production_priority") {
        if ![Some("first"), Some("last")].contains(&priority.as_str())
            || params
                .get("production_category")
                .is_some_and(|v| v != "military")
            || mode == "additional"
        {
            return err(
                key,
                "production_priority: first oppure last per totali military",
            );
        }
        params.insert("production_priority".into(), priority.clone());
        params.insert("production_category".into(), json!("military"));
    }
    Ok(Value::Object(params))
}
fn step(step: &Value, index: usize) -> Result<(&'static str, Value)> {
    if !step.is_object() {
        return err(&index.to_string(), "step non valido");
    }
    let fallback = json!(format!("step-{:04}", index + 1));
    let key = text(
        step.get("key").unwrap_or(&fallback),
        &index.to_string(),
        "key",
    )?;
    let typ = step["type"].as_str().unwrap_or("");
    let Some(spec) = specs()["types"].get(typ) else {
        return err(
            key,
            format!(
                "type sconosciuto: {}",
                step.get("type")
                    .map(|v| v
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| v.to_string()))
                    .unwrap_or("None".into())
            ),
        );
    };
    let kind = &spec["runtime_kind"];
    let category = spec["category"].as_str();
    let mut allowed = set(&specs()["common"]);
    allowed.extend(set(&spec["fields"]));
    let unknown: Vec<_> = keys(step).difference(&allowed).cloned().collect();
    if !unknown.is_empty() {
        return err(
            key,
            format!("campi non riconosciuti: {}", unknown.join(", ")),
        );
    }
    for field in spec["required"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        if typ == "build"
            && field == "state"
            && ["province", "provinces", "border"]
                .iter()
                .any(|f| step.get(f).is_some_and(|v| !v.is_null()))
        {
            continue;
        }
        if step.get(field).is_none_or(Value::is_null) {
            return err(key, format!("manca {field}"));
        }
    }
    if typ == "advanced_v1" {
        if keys(step)
            .iter()
            .any(|s| !["type", "step", "key", "title", "evidence"].contains(&s.as_str()))
        {
            return err(key, "advanced_v1: conserva le regole nello step v1");
        }
        let mut original = step["step"].clone();
        if !original.is_object() || !original["action"].is_object() {
            return err(key, "step v1 non valido");
        }
        for field in ["key", "title", "evidence"] {
            if let Some(value) = step.get(field) {
                if let Some(existing) = original.get(field)
                    && existing != value
                {
                    return err(key, format!("advanced_v1: {field} in conflitto"));
                }
                original[field] = value.clone();
            }
        }
        if original.get("key").is_none() {
            original["key"] = json!(key);
        }
        let lane = match original["action"]["kind"].as_str() {
            Some("select_focus") => "focus",
            Some("research" | "free") => "research",
            _ => "operations",
        };
        return Ok((lane, original));
    }
    let mut params = details(step, key, ["instruction", "reminder"].contains(&typ))?;
    if let Some(category) = category {
        params.insert("category".into(), json!(category));
    }
    if let Some(value) = step.get("category") {
        let (group, hint) = ui_category(value, key)?;
        params.insert("category".into(), json!(category.unwrap_or(&group)));
        if hint.is_some() || category.is_some_and(|c| value != c) {
            params.insert("category_hint".into(), value.clone());
        }
    }
    for field in ["timing", "source_seconds", "game_date"] {
        if let Some(value) = step.get(field) {
            if field == "source_seconds" {
                if value.as_f64().is_none_or(|n| !n.is_finite() || n < 0.) {
                    return err(key, "source_seconds deve essere un tempo positivo o zero");
                }
            } else {
                text(value, key, field)?;
            }
            params.insert(field.into(), value.clone());
        }
    }
    if let Some(value) = step.get("instructions") {
        let value = if value.is_string() {
            json!([value])
        } else {
            value.clone()
        };
        if !value.as_array().is_some_and(|a| {
            !a.is_empty()
                && a.iter()
                    .all(|v| v.as_str().is_some_and(|s| !s.trim().is_empty()))
        }) {
            return err(key, "instructions deve contenere frasi non vuote");
        }
        params.insert("instructions".into(), value);
    }
    let policies = set(&specs()["policies"]);
    let mut row: Value = Value::Object(
        step.as_object()
            .unwrap()
            .iter()
            .filter(|(k, _)| policies.contains(k.as_str()) || k.as_str() == "evidence")
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
    );
    if row["evidence"].is_object() {
        row["evidence"] = json!([row["evidence"]]);
    }
    row["key"] = json!(key);
    let mut action = json!({"kind":kind});
    match typ {
        "focus" | "research" | "special_project" => {
            let (_, target, scope) = reference(&step["target"], key, "target")?;
            if scope.is_some() && typ != "focus" {
                return err(key, "scope ammesso solo per focus");
            }
            action["entity"] = json!({"kind":match typ{"focus"=>"focus","research"=>"technology",_=>"project"},"id":target});
            if let Some(scope) = scope.filter(|s| !s.is_empty()) {
                action["entity"]["scope"] = json!(scope);
            }
            if let Some(value) = step.get("upgrade_from") {
                params.insert(
                    "upgrade_from".into(),
                    json!(text(value, key, "upgrade_from")?),
                );
            }
        }
        "build" => {
            put(
                &mut params,
                &step["building"],
                key,
                "building",
                "building_id",
                "building_name",
            )?;
            if let Some(value) = step.get("state") {
                put(&mut params, value, key, "state", "state_id", "state_name")?;
            }
            if ["province", "provinces", "border"]
                .iter()
                .filter(|f| step.get(f).is_some())
                .count()
                > 1
            {
                return err(key, "choose province, provinces or border");
            }
            if let Some(value) = step.get("border") {
                if step.get("state").is_some() {
                    return err(key, "border and state are alternative locations");
                }
                if !value.as_str().is_some_and(crate::recipient::tag) {
                    return err(key, "border requires a country tag");
                }
                params.insert(
                    "province_selector".into(),
                    json!({"kind":"border","country":value}),
                );
            }
            if let Some(value) = step.get("provinces") {
                if !value.as_array().is_some_and(|a| {
                    !a.is_empty()
                        && a.len() <= 512
                        && a.iter().all(|v| integer(v, 1))
                        && a.iter()
                            .map(Value::to_string)
                            .collect::<BTreeSet<_>>()
                            .len()
                            == a.len()
                }) {
                    return err(key, "provinces requires unique positive IDs");
                }
                params.insert("province_ids".into(), value.clone());
            }
            if !["state", "province", "provinces", "border"]
                .iter()
                .any(|f| step.get(f).is_some())
            {
                return err(key, "build requires a location");
            }
            if let Some(value) = params.get("state_id") {
                let number = value
                    .as_str()
                    .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
                    .and_then(|s| s.parse::<u64>().ok());
                if number.is_none_or(|n| !(1..=9007199254740991).contains(&n)) {
                    return err(key, "state.id deve essere intero positivo");
                }
                params.insert("state_id".into(), json!(number.unwrap()));
            }
            let q = &step["quantity"];
            if keys(q) != BTreeSet::from(["mode".into(), "value".into()])
                || ![Some("add"), Some("total")].contains(&q["mode"].as_str())
                || !integer(&q["value"], 1)
            {
                return err(
                    key,
                    "quantity: servono mode add/total e value intero positivo",
                );
            }
            params.insert("quantity".into(), q["value"].clone());
            params.insert(
                "quantity_mode".into(),
                json!(if q["mode"] == "add" {
                    "additional"
                } else {
                    "target_total"
                }),
            );
            for (src, dst, min) in [
                ("baseline", "baseline_level", 0),
                ("province", "province_id", 1),
                ("queue_priority", "queue_priority", 1),
            ] {
                if let Some(value) = step.get(src) {
                    if !integer(value, min) {
                        return err(key, format!("{src} deve essere intero valido"));
                    }
                    params.insert(dst.into(), value.clone());
                }
            }
        }
        "production" => {
            if let Some(lines) = step.get("lines") {
                if [
                    "equipment",
                    "factories",
                    "variant",
                    "production_category",
                    "production_priority",
                ]
                .iter()
                .any(|k| step.get(k).is_some())
                {
                    return err(key, "produzione: scegli lines oppure una singola linea");
                }
                let Some(lines) = lines.as_array().filter(|a| !a.is_empty() && a.len() <= 64)
                else {
                    return err(key, "lines: richieste da 1 a 64 linee");
                };
                let lines = lines
                    .iter()
                    .enumerate()
                    .map(|(i, v)| production(v, &format!("{key}.lines[{i}]")))
                    .collect::<Result<Vec<_>>>()?;
                if lines.iter().any(|v| v["quantity_mode"] == "additional") {
                    return err(key, "lines richiede totali, non incrementi");
                }
                let remaining: Vec<_> = lines
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| v["quantity_mode"] == "remaining")
                    .map(|(i, _)| i)
                    .collect();
                if !remaining.is_empty() && remaining != vec![lines.len() - 1] {
                    return err(key, "remaining ammesso solo per l’ultima linea");
                }
                params.insert("production_targets".into(), json!(lines));
            } else {
                let line: Value = Value::Object(
                    step.as_object()
                        .unwrap()
                        .iter()
                        .filter(|(k, _)| {
                            [
                                "equipment",
                                "factories",
                                "variant",
                                "production_category",
                                "production_priority",
                            ]
                            .contains(&k.as_str())
                        })
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect(),
                );
                params.extend(production(&line, key)?.as_object().unwrap().clone());
            }
        }
        "law" | "idea" | "decision" => {
            put(
                &mut params,
                &step["target"],
                key,
                "target",
                &format!("{typ}_id"),
                &format!("{typ}_name"),
            )?;
            if let Some(group) = step.get("group") {
                params.insert("law_group".into(), json!(text(group, key, "group")?));
            }
        }
        "appoint" => {
            if !["person", "advisor", "trait"].iter().any(|f| {
                step.get(f)
                    .is_some_and(|v| !v.is_null() && v != false && v != "")
            }) {
                return err(key, "manca person, advisor o trait");
            }
            if step.get("person").is_some() && step.get("advisor").is_some() {
                return err(key, "scegli person oppure advisor");
            }
            for field in ["person", "advisor"] {
                if let Some(value) = step.get(field) {
                    put(
                        &mut params,
                        value,
                        key,
                        field,
                        if field == "person" {
                            "person_id"
                        } else {
                            "advisor_id"
                        },
                        "person_name",
                    )?;
                }
            }
            for (src, dst) in [("trait", "trait_name"), ("role", "role")] {
                if let Some(value) = step.get(src) {
                    params.insert(dst.into(), json!(text(value, key, src)?));
                }
            }
            if step.get("trait").is_some()
                && !["person", "advisor", "role"]
                    .iter()
                    .any(|f| step.get(f).is_some())
            {
                return err(key, "manca role per la scelta tramite trait");
            }
        }
        "event" => {
            put(
                &mut params,
                &step["event"],
                key,
                "event",
                "event_id",
                "event_name",
            )?;
            put(
                &mut params,
                &step["option"],
                key,
                "option",
                "event_option_id",
                "event_option",
            )?;
        }
        "create_agency" => {
            params.insert("operation_id".into(), json!("create_intelligence_agency"));
        }
        "agency_upgrade" => {
            params.insert("operation_id".into(), json!("upgrade_intelligence_agency"));
            params.insert(
                "upgrade_name".into(),
                json!(text(&step["target"], key, "target")?),
            );
        }
        "intelligence_operation" => {
            params.insert(
                "operation_name".into(),
                json!(text(&step["operation"], key, "operation")?),
            );
        }
        "mio" => {
            params.insert(
                "mio_name".into(),
                json!(text(&step["organization"], key, "organization")?),
            );
            if let Some(value) = step.get("equipment") {
                put(
                    &mut params,
                    value,
                    key,
                    "equipment",
                    "equipment_id",
                    "equipment_name",
                )?;
            }
        }
        "template" => {
            params.insert(
                "template_name".into(),
                json!(text(&step["template"], key, "template")?),
            );
        }
        "design" => {
            params.insert(
                "design_name".into(),
                json!(text(&step["name"], key, "name")?),
            );
        }
        "army" | "diplomacy" => {
            params.insert("task".into(), json!(text(&step["task"], key, "task")?));
        }
        "instruction" | "reminder" => {
            params
                .entry("category".to_owned())
                .or_insert(json!("checkpoints"));
            if typ == "reminder" {
                row["tracking"] = json!("reminder");
            }
        }
        _ => {}
    }
    if let Some(value) = step.get("country") {
        params.insert("target_country".into(), json!(text(value, key, "country")?));
    }
    if !params.is_empty() {
        action["params"] = json!(params);
    }
    row["action"] = action;
    let lane = match typ {
        "focus" => "focus",
        "research" => "research",
        "free" => match step
            .get("lane")
            .map(Value::as_str)
            .unwrap_or(Some("research"))
        {
            Some("focus") => "focus",
            Some("research") => "research",
            _ => return err(key, "lane: focus oppure research"),
        },
        _ => "operations",
    };
    Ok((lane, row))
}
pub fn action_reference() -> Value {
    json!(specs()["types"].as_object().unwrap().iter().map(|(name,spec)|{
        let mut row=json!({"type":name,"fields":spec["fields"],"required":spec["required"].as_array().unwrap().iter().filter(|f|name!="build"||**f!="state").collect::<Vec<_>>(),"category":spec["category"],"runtime_kind":spec["runtime_kind"]});if name=="build"{row["location_one_of"]=json!(["state","province","provinces","border"]);}row
    }).collect::<Vec<_>>())
}
pub fn compile(document: &Value) -> Result<Value> {
    if !document.is_object()
        || document["format"] != "hoi4-guide-authoring"
        || document["authoring_schema_version"].as_u64() != Some(1)
    {
        return Err("Formato authoring o versione non supportata".into());
    }
    if keys(document)
        .iter()
        .any(|s| !["format", "authoring_schema_version", "sources", "guide"].contains(&s.as_str()))
    {
        return Err("Campi authoring non riconosciuti".into());
    }
    let value = &document["guide"];
    if !value.is_object() || value.get("tracks").is_some() {
        return Err("Authoring: usa guide.steps, senza tracks".into());
    }
    let allowed = [
        "id",
        "title",
        "subtitle",
        "applies_to",
        "sources",
        "phases",
        "assumptions",
        "steps",
        "start_date",
        "research_baseline",
        "source_inventory",
        "dependency_policy",
        "focus_sequence",
    ];
    let unknown: Vec<_> = keys(value)
        .into_iter()
        .filter(|k| !allowed.contains(&k.as_str()))
        .collect();
    if !unknown.is_empty() {
        return Err(format!(
            "Metadati authoring non riconosciuti: {}",
            unknown.join(", ")
        ));
    }
    for field in ["id", "title"] {
        text(&value[field], "guide", field)?;
    }
    let steps = value["steps"]
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 4096)
        .ok_or("Authoring: richiesti da 1 a 4096 step")?;
    let sources = document["sources"]
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 8)
        .ok_or("Authoring: richieste da 1 a 8 fonti")?;
    let mut guide = value.clone();
    guide.as_object_mut().unwrap().remove("steps");
    if guide.get("schema_version").is_none() {
        guide["schema_version"] = json!(1);
    }
    static RANGE: OnceLock<Regex> = OnceLock::new();
    let range=RANGE.get_or_init(||Regex::new(r"(?i)^\s*Video\s+(\d{1,3}:[0-5]\d(?::[0-5]\d)?)\s*[–—-]\s*(\d{1,3}:[0-5]\d(?::[0-5]\d)?)(?:;\s*not a campaign date)?\s*$").unwrap());
    if let Some(phases) = guide.get_mut("phases").and_then(Value::as_array_mut) {
        for phase in phases {
            if !phase.is_object() || !phase["when"].is_string() {
                continue;
            }
            let label = phase["when"].as_str().unwrap();
            let key = phase["id"].as_str().unwrap_or("phase");
            let seconds = |s: &str| {
                s.split(':')
                    .fold(0u64, |n, p| n * 60 + p.parse::<u64>().unwrap_or(0))
            };
            if !range
                .captures(label)
                .is_some_and(|c| seconds(&c[1]) <= seconds(&c[2]))
            {
                return err(
                    key,
                    "when richiede una condizione dichiarativa; usa timing per il testo descrittivo",
                );
            }
            if phase.get("timing").is_some_and(|v| v != label) {
                return err(key, "timing e when in conflitto");
            }
            let label = phase.as_object_mut().unwrap().remove("when").unwrap();
            phase["timing"] = label;
        }
    }
    if guide.get("sources").is_none() {
        guide["sources"] = json!(
            sources
                .iter()
                .filter(|s| s.is_object())
                .map(|s| Value::Object(
                    s.as_object()
                        .unwrap()
                        .iter()
                        .filter(|(k, _)| ["id", "type", "title", "url"].contains(&k.as_str()))
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect()
                ))
                .collect::<Vec<_>>()
        );
    }
    guide["tracks"] = json!({"focus":[],"research":[],"operations":[]});
    let mut errors = vec![];
    for (index, value) in steps.iter().enumerate() {
        match step(value, index) {
            Ok((lane, row)) => guide["tracks"][lane].as_array_mut().unwrap().push(row),
            Err(e) => errors.push(e),
        }
    }
    if !errors.is_empty() {
        let mut message = errors
            .iter()
            .take(32)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        if errors.len() > 32 {
            message.push_str(&format!("\n{} ulteriori errori.", errors.len() - 32));
        }
        return Err(message);
    }
    Ok(
        json!({"format":"hoi4-guide-import","import_schema_version":1,"sources":sources,"guide":guide_schema::normalize(&guide)?}),
    )
}
pub fn prompt() -> String {
    let mut table = vec![
        "| type | Type-specific fields | Required |".to_owned(),
        "| --- | --- | --- |".into(),
    ];
    for row in action_reference().as_array().unwrap() {
        let join = |v: &Value| {
            v.as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        };
        let mut required = join(&row["required"]);
        if required.is_empty() {
            required = match row["type"].as_str() {
                Some("production") => "equipment + factories or lines",
                Some("appoint") => "person, advisor or trait + role",
                _ => "—",
            }
            .into();
        }
        let fields = join(&row["fields"]);
        table.push(format!(
            "| {} | {} | {required} |",
            row["type"].as_str().unwrap(),
            if fields.is_empty() { "—" } else { &fields }
        ));
    }
    include_str!("../../../src/hoi4companion/guide-import-prompt.md")
        .replace("\r\n", "\n")
        .replace("<!-- tipi-authoring-v1 -->", &table.join("\n"))
}
