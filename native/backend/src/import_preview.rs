//! One deterministic import preview: source integrity, catalog identities,
//! operational compilation and schema validation. No strategy or live reads.
use crate::{
    focus_import, focus_routes, guide_schema,
    import_catalog::{Index, regex},
    import_document, import_reports, operation_binding, operation_language, research_import,
    state::truth,
};
use hoi4companion_catalog_core::optional_focus;
use serde_json::{Value, json};
use std::collections::BTreeSet;
type Result<T> = std::result::Result<T, String>;
fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn arr(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn push(report: &mut Value, field: &str, text: impl Into<String>) {
    report[field]
        .as_array_mut()
        .unwrap()
        .push(json!(text.into()));
}
fn rows(guide: &Value) -> Vec<(String, usize)> {
    guide["tracks"]
        .as_object()
        .into_iter()
        .flatten()
        .flat_map(|(track, rows)| {
            arr(rows)
                .iter()
                .enumerate()
                .map(move |(i, _)| (track.clone(), i))
        })
        .collect()
}
fn caption(item: &Value, source: &Value) -> bool {
    let Some(timestamp) = item["timestamp_seconds"].as_f64() else {
        return false;
    };
    let Some(quote) = item["quote"].as_str() else {
        return false;
    };
    arr(&source["segments"]).iter().any(|segment| {
        segment["start"]
            .as_f64()
            .is_some_and(|start| (start - timestamp).abs() <= 1.)
            && segment["text"]
                .as_str()
                .is_some_and(|text| text.contains(quote))
    })
}
fn supported_visual(item: &Value, sources: &Value) -> bool {
    let source = &sources[s(&item["source_id"])];
    let quote = s(&item["quote"]);
    item["basis"] == "visual"
        && source["acquisition"] == "external_ai"
        && !quote.trim().is_empty()
        && s(&source["text"]).contains(quote)
        && if item.get("timestamp_seconds").is_none() {
            source["type"] != "youtube"
        } else {
            caption(item, source)
        }
}
fn suggestions(
    index: &Index,
    query: &str,
    kind: &str,
    country: &Value,
    scope: &Value,
) -> Result<Vec<Value>> {
    let result = index.query(
        json!({"op":"suggest","query":query,"kinds":[kind],"country":country,"scope":scope}),
    )?;
    arr(&result)
        .iter()
        .map(|r| {
            let mut row = index
                .rows()
                .get(r["index"].as_u64().ok_or("Invalid catalog suggestion")? as usize)
                .cloned()
                .ok_or("Invalid catalog suggestion")?;
            row["suggestion"] = r["suggestion"].clone();
            Ok(row)
        })
        .collect()
}
fn excluded(
    index: &Index,
    query: &str,
    kind: &str,
    country: &Value,
    scope: &Value,
) -> Result<Value> {
    let result = index.query(
        json!({"op":"excluded","query":query,"kinds":[kind],"country":country,"scope":scope}),
    )?;
    Ok(json!(
        arr(&result)
            .iter()
            .map(|r| {
                let row = index
                    .rows()
                    .get(r["index"].as_u64().ok_or("Invalid catalog exclusion")? as usize)
                    .ok_or("Invalid catalog exclusion")?;
                Ok(json!({"entity":row,"reason":r["reason"]}))
            })
            .collect::<Result<Vec<_>>>()?
    ))
}
pub fn preview(
    input: &Value,
    sources: &Value,
    index: &Index,
    library: &Value,
    research: &Value,
) -> Value {
    let mut report = json!({"import_schema_version":1,"valid":false,"guide":null,"draft":if input.is_object(){input.clone()}else{Value::Null},"errors":[],"warnings":[],"resolutions":[],"visual_review":{"count":0,"step_keys":[]}});
    if let Err(error) = compile(input, sources, index, library, research, &mut report) {
        push(
            &mut report,
            "errors",
            if error.is_empty() {
                "Documento non valido".into()
            } else {
                error
            },
        );
    }
    report
}
fn compile(
    input: &Value,
    sources: &Value,
    index: &Index,
    library: &Value,
    research: &Value,
    report: &mut Value,
) -> Result<()> {
    let acquired = import_document::validate_sources(sources)?;
    if !input.is_object()
        || !input["id"]
            .as_str()
            .is_some_and(|id| regex(r"^[A-Za-z0-9_-]{1,160}$").is_match(id))
    {
        return Err("ID guida necessario: usa lettere, numeri, trattino o underscore".into());
    }
    if !input["title"]
        .as_str()
        .is_some_and(|s| !s.trim().is_empty())
    {
        return Err("Titolo guida necessario".into());
    }
    let mut guide = guide_schema::normalize(input)?;
    if !truth(&guide["applies_to"]["country"]) {
        let country =
            index.query(json!({"op":"country_hint","query":guide["title"],"guide":guide}))?;
        if truth(&country) {
            guide["applies_to"]["country"] = country.clone();
            push(
                report,
                "warnings",
                format!(
                    "Nazione {} riscontrata nel titolo e nei focus della guida; non dedotta dalla partita corrente.",
                    s(&country)
                ),
            );
        }
    }
    let scope = focus_routes::infer_scope(&guide, index)?;
    if truth(&scope) {
        guide["applies_to"]["tree_id"] = scope.clone();
        push(
            report,
            "warnings",
            format!(
                "Albero focus dedotto dai nomi presenti nella guida: {}. Verifica che corrisponda alla nazione e alla versione della guida.",
                s(&scope)
            ),
        );
    }
    if !guide["subtitle"].is_string()
        || !guide["assumptions"]
            .as_array()
            .is_some_and(|a| a.iter().all(Value::is_string))
    {
        return Err("Sottotitolo o elenco dei limiti non valido".into());
    }
    if guide["milestones"] != json!([]) {
        return Err(
            "Importa i checkpoint come passaggi operativi con evidenza, non come milestones legacy"
                .into(),
        );
    }
    if let Some(source) = guide.get("source")
        && (!source.is_object()
            || source.get("chapters").is_some_and(|v| {
                !v.as_array().is_some_and(|a| {
                    a.iter().all(|item| {
                        item.is_object()
                            && item["title"].is_string()
                            && item["start_time"].is_number()
                    })
                })
            }))
    {
        return Err("Metadati fonte legacy non validi".into());
    }
    let positions = rows(&guide);
    if positions.is_empty() {
        return Err("La guida non contiene istruzioni".into());
    }
    let declared = arr(&guide["sources"])
        .iter()
        .map(|r| s(&r["id"]).to_owned())
        .collect::<BTreeSet<_>>();
    if declared.is_empty() || !declared.iter().all(|id| acquired.get(id).is_some()) {
        return Err("La guida cita fonti non acquisite".into());
    }
    guide["sources"] = json!(
        acquired
            .as_object()
            .unwrap()
            .iter()
            .filter(|(id, _)| declared.contains(*id))
            .map(|(_, src)| {
                let fields = if src["type"] == "file" {
                    &["id", "type", "title", "url", "content_sha256", "text"][..]
                } else {
                    &["id", "type", "title", "url", "content_sha256"][..]
                };
                let mut result = json!({});
                for field in fields {
                    if let Some(v) = src.get(*field) {
                        result[*field] = v.clone();
                    }
                }
                result
            })
            .collect::<Vec<_>>()
    );
    guide.as_object_mut().unwrap().remove("default");
    let mut visual = BTreeSet::new();
    for (track, i) in &positions {
        let row = &mut guide["tracks"][track][*i];
        let action = &row["action"];
        let query = s(&action["entity"]["id"]);
        let params = &action["params"];
        if action["kind"] == "research"
            && !query.contains('_')
            && params.get("upgrade_from").is_none()
            && params["interpretation"] != "next_upgrade"
            && index
                .matches(query, &["technology"], &Value::Null, &Value::Null, false)?
                .is_empty()
        {
            let projects = index.matches(query, &["project"], &Value::Null, &Value::Null, false)?;
            if projects.len() == 1 {
                let query = query.to_owned();
                row["action"]["kind"] = json!("special_project");
                row["action"]["entity"] = json!({"kind":"project","id":projects[0]["id"]});
                row["kind"] = json!("project");
                row["target"] = projects[0]["id"].clone();
                push(
                    report,
                    "warnings",
                    format!(
                        "{}: {query} è un progetto speciale nel catalogo, non una tecnologia ordinaria.",
                        s(&row["key"])
                    ),
                );
            }
        }
    }
    let conditional = focus_import::conditionals(&guide, index)?;
    guide = conditional["guide"].clone();
    let conditionals = &conditional["resolutions"];
    for (key, item) in conditionals.as_object().into_iter().flatten() {
        if item["condition_basis"] == "source_text_unverifiable" {
            push(
                report,
                "warnings",
                format!(
                    "{key}: Conditional alternative resolved; source condition \"{}\" has no measurable criterion and remains unverifiable. No automatic recommendation is made from this phrase.",
                    s(&item["condition_text"])
                ),
            );
        }
    }
    let upgrades = research_import::compile(&guide, index, research)?;
    guide = upgrades["guide"].clone();
    let upgrades = &upgrades["resolutions"];
    if truth(research) {
        report["research_context"] = research.clone();
    }
    let routes = focus_routes::resolve(&guide, index)?;
    let focus_matches = &routes["resolutions"];
    report["focus_route"] = routes["route"].clone();
    report["focus_sequence"] = routes["route"]["sequence"].clone();
    report["errors"]
        .as_array_mut()
        .unwrap()
        .extend_from_slice(arr(&routes["errors"]));
    let default_scope = guide["applies_to"]["tree_id"].clone();
    let default_country = guide["applies_to"]["country"].clone();
    for (track, i) in &positions {
        let row = &mut guide["tracks"][track][*i];
        let key = s(&row["key"]).to_owned();
        let params = &row["action"]["params"];
        if let Some(hint) = params["category_hint"].as_str()
            && params["category"] != hint
        {
            push(
                report,
                "warnings",
                format!(
                    "{key}: category \"{hint}\" grouped under \"{}\"; the original label is retained and does not define live monitoring.",
                    params["category"].as_str().unwrap_or("checkpoints")
                ),
            );
        }
        let evidence = arr(&row["evidence"]);
        if evidence.is_empty() {
            push(
                report,
                "errors",
                format!("{key}: manca il riscontro nella fonte"),
            );
        }
        for item in evidence {
            let src = &acquired[s(&item["source_id"])];
            let quote = s(&item["quote"]);
            let is_visual = item["basis"] == "visual" && src["acquisition"] == "external_ai";
            if (item["basis"] != "explicit" && !is_visual)
                || quote.trim().is_empty()
                || src.is_null()
                || !s(&src["text"]).contains(quote)
            {
                push(
                    report,
                    "errors",
                    format!("{key}: citazione non esplicita o non presente nel testo acquisito"),
                );
            }
            if is_visual {
                push(
                    report,
                    "warnings",
                    format!(
                        "{key}: osservazione visiva dichiarata dall’IA esterna, da confrontare con la fonte"
                    ),
                );
                if supported_visual(item, &acquired) {
                    visual.insert(key.clone());
                }
                if src["type"] == "youtube" && item.get("timestamp_seconds").is_none() {
                    push(
                        report,
                        "errors",
                        format!("{key}: osservazione visiva del video priva di timestamp"),
                    );
                }
            }
            if item.get("timestamp_seconds").is_some() && !caption(item, src) {
                push(
                    report,
                    "errors",
                    format!("{key}: timestamp non riscontrato nei sottotitoli acquisiti"),
                );
            }
        }
        if !["select_focus", "research", "special_project"].contains(&s(&row["action"]["kind"])) {
            continue;
        }
        let kind = s(&row["action"]["entity"]["kind"]).to_owned();
        let scope = if truth(&row["action"]["entity"]["scope"]) {
            row["action"]["entity"]["scope"].clone()
        } else if kind == "focus" {
            default_scope.clone()
        } else {
            Value::Null
        };
        let country = if kind == "focus" {
            default_country.clone()
        } else {
            Value::Null
        };
        let upgrade = upgrades.get(&key);
        let route = focus_matches.get(&key);
        if let Some(binding) = route
            && binding["locked"] != true
        {
            row["action"]["entity"]["id"] = binding["source_query"].clone();
        }
        let original = s(&row["action"]["entity"]["id"]).to_owned();
        let exact = index.matches(&original, &[&kind], &country, &scope, false)?;
        let mut matches = if exact.is_empty() {
            index.matches(&original, &[&kind], &country, &scope, true)?
        } else {
            exact.clone()
        };
        let mut basis = if exact.is_empty() {
            "minor_typo"
        } else {
            "catalog_name_or_id"
        };
        if truth(&row["research_choice"]) {
            basis = "research_branch_choice";
        }
        let mut supporting = json!([]);
        if let Some(route) = route {
            matches = arr(&route["matches"]).to_vec();
            supporting = route["supporting_focus_ids"].clone();
            if truth(&route["inferred"]) {
                basis = if truth(&route["recovery"]) {
                    "guide_required_ancestor"
                } else {
                    "guide_prerequisites"
                };
            }
            if truth(&route["government_context"]) {
                basis = "guide_government_branch";
            }
            if truth(&route["word_variant"]) {
                basis = "focus_one_word_variant";
            }
            if route["recovery"]["method"] == "unique_global_sequence" {
                basis = "focus_sequence_constraints";
            }
        }
        if upgrade.is_some_and(|u| !["upgrade_resolved", "upgrade_live"].contains(&s(&u["reason"])))
        {
            matches.clear();
        }
        let mut resolution;
        if matches.len() == 1 {
            let matched = &matches[0];
            row["action"]["entity"]["id"] = matched["id"].clone();
            if truth(&matched["continuous"]) {
                row["action"]["params"]["focus_mode"] = json!("continuous");
                if optional_focus(&original).is_some() {
                    row["action"]["params"]["focus_optional"] = json!(true);
                }
            } else if row["action"]["kind"] == "select_focus"
                && let Some(params) = row
                    .get_mut("action")
                    .and_then(|a| a.get_mut("params"))
                    .and_then(Value::as_object_mut)
            {
                params.remove("focus_mode");
                params.remove("focus_optional");
            }
            if truth(&matched["scope"]) {
                row["action"]["entity"]["scope"] = matched["scope"].clone();
            }
            row["target"] = matched["id"].clone();
            row["name"] = matched
                .get("localized_name")
                .unwrap_or(&matched["name"])
                .clone();
            resolution = json!({"key":key,"status":"resolved","entity":row["action"]["entity"],"candidates":matches,"matching_basis":basis});
            if let Some(conditional) = conditionals.get(&key) {
                resolution["matching_basis"] = json!("conditional_focus");
                resolution["association_evidence"] = conditional.clone();
            }
            if basis == "focus_sequence_constraints" {
                push(
                    report,
                    "warnings",
                    format!(
                        "{key}: reconstructed at source position {}: {original} → {} ({}); identity evidence only, not live completion.",
                        route.unwrap()["source_position"],
                        s(&matched["name"]),
                        s(&matched["id"])
                    ),
                );
            } else if exact.is_empty() {
                push(
                    report,
                    "warnings",
                    format!(
                        "{key}: refuso riconosciuto: {original} → {} ({})",
                        s(&matched["name"]),
                        s(&matched["id"])
                    ),
                );
            }
            if truth(&supporting) {
                resolution["supporting_focus_ids"] = supporting;
            }
            if let Some(route) = route {
                for field in ["recovery", "government_context", "word_variant"] {
                    if truth(&route[field]) {
                        resolution["association_evidence"] = route[field].clone();
                    }
                }
            }
        } else {
            let mut suggested = if !matches.is_empty() {
                matches.clone()
            } else if upgrade.is_some_and(|u| truth(&u["candidates"])) {
                arr(&upgrade.unwrap()["candidates"]).to_vec()
            } else {
                suggestions(index, &original, &kind, &country, &scope)?
            };
            if let Some(route) = route {
                if truth(&route["probable_candidates"]) {
                    suggested = arr(&route["probable_candidates"]).to_vec();
                }
                if route["has_name_matches"] == true {
                    suggested = arr(&route["viable_candidates"]).to_vec();
                }
            }
            resolution = json!({"key":key,"status":if matches.is_empty(){"unresolved"}else{"ambiguous"},"query":row["action"]["entity"]["id"],"kind":kind,"context":{"country":country,"scope":scope},"candidates":suggested.into_iter().take(12).collect::<Vec<_>>()});
            if route.is_some_and(|r| r["unresolved_route"] == true) {
                resolution["status"] = json!("unresolved_route");
            }
            push(
                report,
                "errors",
                if resolution["status"] == "unresolved_route" {
                    format!(
                        "{key}: percorso focus non verificabile: prerequisiti mancanti o ciclici ({})",
                        s(&row["action"]["entity"]["id"])
                    )
                } else {
                    format!(
                        "{key}: ID {kind} ambiguo o non risolto ({})",
                        s(&row["action"]["entity"]["id"])
                    )
                },
            );
        }
        if let Some(route) = route {
            if truth(&route["excluded_candidates"]) {
                resolution["excluded_candidates"] = route["excluded_candidates"].clone();
            }
            for (field, target) in [
                ("classification", "classification"),
                ("source_position", "source_position"),
                ("source_query", "source_query"),
            ] {
                resolution[target] = route[field].clone();
            }
            row["focus_resolution"] = json!({"schema_version":1,"position":route["source_position"],"query":route["source_query"],"classification":route["classification"],"entity":row["action"]["entity"],"locked":route["locked"]});
        }
        let context_exclusions = excluded(index, &original, &kind, &country, &scope)?;
        if truth(&context_exclusions) {
            resolution["context_exclusions"] = context_exclusions;
        }
        if let Some(upgrade) = upgrade {
            for field in ["query", "interpretation", "upgrade_from", "reason"] {
                resolution[field] = upgrade[field].clone();
            }
            let reason = s(&upgrade["reason"]);
            if reason == "upgrade_live" {
                push(
                    report,
                    "warnings",
                    format!(
                        "{key}: ramo UP identificato; il livello verrà associato ai dati di ricerca della partita, non allo scenario iniziale."
                    ),
                );
            }
            if !["upgrade_resolved", "upgrade_live"].contains(&reason) {
                let explanation = match reason {
                    "upgrade_baseline_unknown" => "specifica il livello di partenza o upgrade_from",
                    "upgrade_end_of_line" => "il ramo non ha un livello successivo nel catalogo",
                    "upgrade_branch_ambiguous" => {
                        "il ramo ha più successori: specifica quello previsto"
                    }
                    "upgrade_target_missing" => "il successore non è nel catalogo compatibile",
                    "upgrade_cycle_or_repeated_target" => {
                        "il successore ripete un livello precedente"
                    }
                    _ => return Err("Unknown research upgrade reason".into()),
                };
                let last = report["errors"]
                    .as_array_mut()
                    .unwrap()
                    .last_mut()
                    .ok_or("Missing unresolved upgrade diagnostic")?;
                *last = json!(format!("{key}: UP = prossimo livello; {explanation}"));
            }
        }
        report["resolutions"]
            .as_array_mut()
            .unwrap()
            .push(resolution);
    }
    let scope = guide["applies_to"]["tree_id"].clone();
    let country = guide["applies_to"]["country"].clone();
    let selected = positions
        .iter()
        .filter_map(|(t, i)| {
            let r = &guide["tracks"][t][*i];
            (r["action"]["kind"] == "select_focus").then(|| r["target"].clone())
        })
        .collect::<Vec<_>>();
    for (track, i) in &positions {
        let row = &mut guide["tracks"][track][*i];
        if !truth(&row["action"]["params"]["focus_optional"]) || truth(&row["optional_before"]) {
            continue;
        }
        let text = format!("{} {}", s(&row["note"]), s(&row["title"]));
        if let Some(hit) = regex(r"(?i)\buntil\s+(.+?)\s+(?:is\s+)?available\b").captures(&text) {
            let name = hit[1].trim_matches([' ', '"', '“', '”', '\'']);
            let matches = index.matches(name, &["focus"], &country, &scope, false)?;
            if matches.len() == 1 && selected.contains(&matches[0]["id"]) {
                row["optional_before"] = matches[0]["id"].clone();
            }
        }
    }
    let promoted = operation_language::promote(&guide, index, library)?;
    guide = promoted["guide"].clone();
    for item in arr(&promoted["changes"]) {
        if item["unverifiable_timing"] == true {
            push(
                report,
                "warnings",
                format!(
                    "{}: Source actions resolved, but the informal timing condition remains unverifiable; no phase is inferred from missing data.",
                    s(&item["key"])
                ),
            );
        }
    }
    let bound = operation_binding::bind(&guide, index, library)?;
    guide = bound["guide"].clone();
    report["operations"] = bound["reports"].clone();
    for source in arr(sources) {
        report["warnings"]
            .as_array_mut()
            .unwrap()
            .extend_from_slice(arr(&source["warnings"]));
    }
    report["coverage"] = import_reports::coverage(&guide, &acquired)?;
    let keys = rows(&guide)
        .iter()
        .map(|(t, i)| s(&guide["tracks"][t][*i]["key"]).to_owned())
        .collect::<BTreeSet<_>>();
    for item in import_document::inventory(&guide)?.into_iter().flatten() {
        if !truth(&item["excluded_reason"]) && supported_visual(item, &acquired) {
            for key in arr(&item["step_keys"]) {
                if keys.contains(s(key)) {
                    visual.insert(s(key).to_owned());
                }
            }
        }
    }
    report["visual_review"] =
        json!({"count":visual.len(),"step_keys":visual.into_iter().collect::<Vec<_>>()});
    for item in arr(&report["coverage"]["issues"]).to_vec() {
        push(
            report,
            "errors",
            format!("Copertura: {}", s(&item["message"])),
        );
    }
    report["guide"] = guide_schema::normalize(&guide)?;
    report["sequence_report"] = import_reports::sequence(&report["guide"])?;
    report["valid"] = json!(arr(&report["errors"]).is_empty());
    Ok(())
}
