//! Editorial source accounting and structural barriers, separate from live progress.
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
type Observations = HashMap<(String, String, Option<u64>), (String, BTreeSet<String>, usize)>;
fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn arr(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn issue(report: &mut Value, code: &str, message: String, item: Option<&Value>, keys: &Value) {
    report["issues"].as_array_mut().unwrap().push(json!({"code":code,"message":message,"item_id":item.map(|i|i["id"].clone()),"step_keys":keys}));
}
pub fn coverage(guide: &Value, sources: &Value) -> Result<Value, String> {
    let mut report = json!({"schema_version":1,"status":"unavailable","total":0,"linked":0,"excluded":0,"issues":[],"scope":"declared_instructions_only"});
    let Some(items) = crate::import_document::inventory(guide)? else {
        return Ok(report);
    };
    let rows = guide["tracks"]
        .as_object()
        .ok_or("Missing guide tracks")?
        .values()
        .flat_map(arr)
        .map(|r| (s(&r["key"]).to_owned(), r))
        .collect::<BTreeMap<_, _>>();
    let mut covered = BTreeSet::new();
    let mut observations: Observations = HashMap::new();
    let mut excluded = 0;
    let mut linked = 0;
    for item in items {
        let keys = item.get("step_keys").cloned().unwrap_or(json!([]));
        let source = &sources[s(&item["source_id"])];
        let quote = s(&item["quote"]);
        if !source.is_object() || !s(&source["text"]).contains(quote) {
            issue(
                &mut report,
                "unsupported_quote",
                "Citazione non presente nella fonte allegata.".into(),
                Some(item),
                &keys,
            );
        }
        if let Some(timestamp) = item.get("timestamp_seconds")
            && !arr(&source["segments"]).iter().any(|segment| {
                segment["start"]
                    .as_f64()
                    .is_some_and(|t| (t - timestamp.as_f64().unwrap()).abs() <= 1.)
                    && s(&segment["text"]).contains(quote)
            })
        {
            issue(
                &mut report,
                "unsupported_timestamp",
                "Tempo non riscontrato nei segmenti della fonte.".into(),
                Some(item),
                &keys,
            );
        }
        let has_excluded = item["excluded_reason"]
            .as_str()
            .is_some_and(|s| !s.is_empty());
        if has_excluded {
            if !arr(&keys).is_empty() {
                issue(
                    &mut report,
                    "conflicting_disposition",
                    "Scegli collegamento oppure esclusione motivata.".into(),
                    Some(item),
                    &keys,
                );
            } else {
                excluded += 1;
            }
        } else if arr(&keys).is_empty() {
            issue(
                &mut report,
                "unmapped",
                "Istruzione estratta senza uno step o una esclusione motivata.".into(),
                Some(item),
                &json!([]),
            );
        } else if arr(&keys).iter().any(|key| !rows.contains_key(s(key))) {
            issue(
                &mut report,
                "unknown_step",
                "Il collegamento indica uno step inesistente.".into(),
                Some(item),
                &keys,
            );
        } else {
            linked += 1;
            covered.extend(arr(&keys).iter().map(|k| s(k).to_owned()));
        }
        let time = item["timestamp_seconds"]
            .as_f64()
            .map(|t| if t == 0. { 0 } else { t.to_bits() });
        let identity = (s(&item["source_id"]).to_owned(), quote.to_owned(), time);
        if !has_excluded {
            let key_set = arr(&keys)
                .iter()
                .map(|k| s(k).to_owned())
                .collect::<BTreeSet<_>>();
            if let Some((id, previous, count)) = observations.get_mut(&identity) {
                let repeated = item.get("timestamp_seconds").is_none()
                    && *count >= s(&source["text"]).matches(quote).count();
                if !previous.is_disjoint(&key_set) || repeated || key_set.is_empty() {
                    issue(
                        &mut report,
                        "duplicate_observation",
                        format!("Stesso riscontro di {id}: verifica il duplicato."),
                        Some(item),
                        &keys,
                    );
                }
                previous.extend(key_set);
                *count += 1;
            } else {
                observations.insert(identity, (s(&item["id"]).into(), key_set, 1));
            }
        }
    }
    for key in rows.keys().filter(|k| !covered.contains(*k)) {
        issue(
            &mut report,
            "untracked_step",
            format!("Step senza collegamento all’inventario: {key}"),
            None,
            &json!([key]),
        );
    }
    report["total"] = json!(items.len());
    report["linked"] = json!(linked);
    report["excluded"] = json!(excluded);
    report["status"] = json!(if arr(&report["issues"]).is_empty() {
        "accounted"
    } else {
        "needs_review"
    });
    Ok(report)
}
pub fn sequence(guide: &Value) -> Result<Value, String> {
    let policy = guide
        .get("dependency_policy")
        .cloned()
        .unwrap_or(json!("legacy"));
    let tracks = guide["tracks"]
        .as_object()
        .ok_or("Missing guide tracks")?
        .iter()
        .collect::<BTreeMap<_, _>>();
    let mut affected = vec![];
    let (mut edges, mut unspecified, mut steps) = (0, 0, 0);
    for (track, rows) in tracks {
        let mut barriers = 0;
        for step in arr(rows) {
            steps += 1;
            let specified =
                step.get("depends_on").is_some() || step.get("depends_on_started").is_some();
            if !specified {
                unspecified += 1;
                if policy == "legacy" && barriers > 0 {
                    edges += barriers;
                    affected.push(
                        json!({"key":step["key"],"track":track,"implicit_barriers":barriers}),
                    );
                }
            }
            if step["action"]["kind"] == "manual" && step["tracking"] != "reminder" {
                barriers += 1;
            }
        }
    }
    Ok(
        json!({"policy":policy,"step_count":steps,"unspecified_dependency_steps":unspecified,"implicit_barrier_steps":affected.len(),"implicit_barrier_edges":edges,"examples":affected.into_iter().take(10).collect::<Vec<_>>()}),
    )
}
