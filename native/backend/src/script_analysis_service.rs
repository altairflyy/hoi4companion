//! Static CWTools contracts. No live facts and no execution of game scripts.
use crate::{
    installation,
    script_document::{entries, first, scalar},
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::{Path, PathBuf},
};
type Result<T> = std::result::Result<T, String>;
pub const ENGINE_REVISION: &str = "b377453dee803f9258be92cfc49896d09039702d";
fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn array(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
pub fn provenance(root: &Path) -> Result<Value> {
    let result = crate::guide_repository::json_file(&root.join("provenance.json"), 16_000_000)?;
    for (file, digest) in result["files"]
        .as_object()
        .ok_or("Invalid provenance manifest")?
    {
        if Path::new(file).is_absolute()
            || file
                .replace('\\', "/")
                .split('/')
                .any(|p| p == ".." || p.contains(':'))
        {
            return Err("Invalid provenance path".into());
        }
        let mut input = std::fs::File::open(root.join(file))
            .map_err(|_| format!("CWTools: file mancante o modificato: {file}"))?;
        let mut hash = Sha256::new();
        let mut chunk = [0u8; 65536];
        let mut total = 0u64;
        loop {
            let n = input.read(&mut chunk).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            total += n as u64;
            if total > 1_000_000_000 {
                return Err("CWTools file exceeds limit".into());
            }
            hash.update(&chunk[..n]);
        }
        if format!("{:x}", hash.finalize()) != s(digest) {
            return Err(format!("CWTools: file mancante o modificato: {file}"));
        }
    }
    Ok(result)
}
fn schema(node: &Value) -> Value {
    if node.get("entries").is_some() {
        json!(
            entries(node)
                .map(|(k, v)| json!([k, schema(v)]))
                .collect::<Vec<_>>()
        )
    } else {
        node.clone()
    }
}
fn cwt(source: &str) -> Result<Value> {
    let atoms =
        crate::import_catalog::regex(r##""(?:\\.|[^"\\])*"|#[^\r\n]*|[{}]|[<>!]?=|<>|[^\s{}=]+"##);
    let text = atoms
        .find_iter(source)
        .map(|m| m.as_str())
        .filter(|t| !t.starts_with('#'))
        .map(|t| {
            if t.starts_with('"')
                || ["{", "}", "=", "<", ">", ">=", "<=", "!=", "<>", "=="].contains(&t)
            {
                t.to_owned()
            } else {
                json!(t).to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    serde_json::to_value(hoi4companion_parser::parse(text.as_bytes())?).map_err(|e| e.to_string())
}
fn scopes(doc: &Value) -> Value {
    json!(
        array(&doc["supported_scope"])
            .iter()
            .map(|v| s(v).to_lowercase())
            .collect::<Vec<_>>()
    )
}
pub fn rulebook(root: &Path) -> Result<Value> {
    let provenance = provenance(root).map_err(|e| {
        e.replace(
            "CWTools: file mancante o modificato: ",
            "Regole CWTools modificate o incomplete: ",
        )
    })?;
    let documentation =
        crate::guide_repository::json_file(&root.join("script_documentation.json"), 32_000_000)?;
    let mut rules = json!({"trigger":{},"effect":{}});
    let mut links = Map::new();
    let mut enums = Map::new();
    let mut errors = vec![];
    let mut files = installation::files(root, ".cwt")?;
    files.sort();
    let alias = crate::import_catalog::regex(r"^alias\[(trigger|effect):([^\]]+)\]$");
    for file in files {
        let relative = file
            .strip_prefix(root)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let mut text = String::new();
        std::fs::File::open(&file)
            .map_err(|e| e.to_string())?
            .take(16_000_001)
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        if text.len() > 16_000_000 {
            return Err("CWT source exceeds limit".into());
        }
        let node = match cwt(text.trim_start_matches('\u{feff}')) {
            Ok(v) => v,
            Err(error) => {
                errors.push(json!({"file":relative,"error":error}));
                continue;
            }
        };
        for (key, value) in entries(&node) {
            if let Some(c) = alias.captures(key) {
                let kind = &c[1];
                let name = &c[2];
                if name.contains('<') {
                    continue;
                }
                let doc = &documentation[format!("{kind}s")][name];
                let entry = json!({"schema":schema(value),"scopes":scopes(doc),"description":s(&doc["description"]).chars().take(2000).collect::<String>(),"file":relative});
                rules[kind]
                    .as_object_mut()
                    .unwrap()
                    .entry(name.to_lowercase())
                    .or_insert_with(|| json!([]))
                    .as_array_mut()
                    .unwrap()
                    .push(entry);
            } else if key == "links" && value.get("entries").is_some() {
                for (name, link) in entries(value) {
                    if link.get("entries").is_none() {
                        continue;
                    }
                    let input = first(link, "input_scopes").unwrap_or(&Value::Null);
                    let inputs = if input.get("entries").is_some() {
                        entries(input)
                            .filter(|(k, v)| k.is_empty() && v.is_string())
                            .map(|(_, v)| s(v).to_lowercase())
                            .collect::<Vec<_>>()
                    } else if input.is_string() {
                        vec![s(input).to_lowercase()]
                    } else {
                        vec![]
                    };
                    let output = scalar(link, "output_scope");
                    links.insert(name.to_lowercase(),json!({"output":if output.is_empty(){Value::Null}else{json!(output)},"inputs":inputs}));
                }
            } else if key == "enums" && value.get("entries").is_some() {
                for (name, items) in entries(value) {
                    if name.starts_with("enum[") && items.get("entries").is_some() {
                        let values: BTreeSet<_> = entries(items)
                            .filter(|(k, v)| k.is_empty() && v.is_string())
                            .map(|(_, v)| s(v))
                            .collect();
                        enums.insert(name[5..name.len() - 1].into(), json!(values));
                    }
                }
            }
        }
    }
    for kind in ["trigger", "effect"] {
        for (name, doc) in documentation[format!("{kind}s")]
            .as_object()
            .into_iter()
            .flatten()
        {
            rules[kind].as_object_mut().unwrap().entry(name.to_lowercase()).or_insert_with(||json!([{"schema":null,"scopes":scopes(doc),"description":s(&doc["description"]).chars().take(2000).collect::<String>(),"file":"script_documentation.json"} ]));
        }
    }
    Ok(
        json!({"provenance":provenance,"rules":rules,"links":links,"enums":enums,"load_errors":errors}),
    )
}
pub fn describe(book: &Value, kind: &str, name: &str) -> Value {
    let variants = book["rules"][kind]
        .get(name.to_lowercase())
        .cloned()
        .unwrap_or_else(|| json!([]));
    let scopes: BTreeSet<_> = array(&variants)
        .iter()
        .flat_map(|v| array(&v["scopes"]))
        .filter_map(Value::as_str)
        .collect();
    json!({"schema_version":1,"kind":kind,"name":name,"variants":variants,"scopes":scopes,"revision":book["provenance"]["revision"],"source":"cwtools_rules","runtime_evaluated":false})
}
pub fn files(game: &Path, mods: &[PathBuf]) -> Result<Value> {
    const FOLDERS: [&str; 11] = [
        "common",
        "country_metadata",
        "events",
        "gfx",
        "interface",
        "localisation",
        "history",
        "map",
        "music",
        "portraits",
        "sound",
    ];
    const EXT: [&str; 17] = [
        "txt", "gui", "gfx", "sfx", "asset", "map", "yml", "csv", "dds", "tga", "shader", "lua",
        "png", "mesh", "ttf", "otf", "wav",
    ];
    let mut files = BTreeMap::new();
    let mut warnings = vec![];
    for folder in FOLDERS {
        let selected = installation::collect(game, mods, folder, "")?;
        warnings.extend(array(&selected["warnings"]).iter().cloned());
        for file in array(&selected["files"]).iter().filter_map(Value::as_str) {
            let path = Path::new(file);
            if !path.is_file()
                || !path
                    .extension()
                    .is_some_and(|e| EXT.contains(&e.to_string_lossy().to_lowercase().as_str()))
            {
                continue;
            }
            let relative = mods
                .iter()
                .map(PathBuf::as_path)
                .rev()
                .chain(std::iter::once(game))
                .find_map(|r| path.strip_prefix(r).ok())
                .ok_or("CWTools source outside installation roots")?;
            files.insert(
                relative.to_string_lossy().replace('\\', "/"),
                file.to_owned(),
            );
        }
    }
    Ok(json!({"files":files,"warnings":warnings}))
}
pub fn normalize(
    raw: &Value,
    files: &Value,
    warnings: &Value,
    root: &Path,
    limit: usize,
) -> Result<Value> {
    let mut counts = BTreeMap::<String, u64>::new();
    let mut diagnostics = vec![];
    let ignored: Vec<_> = array(&raw["parser_errors"])
        .iter()
        .filter(|e| e["classification"] == "rejected_by_both")
        .cloned()
        .collect();
    for item in array(&raw["diagnostics"]) {
        for key in ["total", s(&item["category"]), s(&item["severity"])] {
            *counts.entry(key.into()).or_default() += 1
        }
        let mut row = item.clone();
        row["source"] = json!("static_analysis");
        row["runtime_evaluated"] = json!(false);
        row["rule"] = item["code"].clone();
        row["value"] = json!("");
        row["path"] = json!(format!("{}:{}", item["line"], item["column"]));
        row["position"] = json!({"line":item["line"],"column":item["column"],"end_line":item["end_line"],"end_column":item["end_column"]});
        if diagnostics.len() < limit {
            diagnostics.push(row)
        }
    }
    let summary = raw
        .get("diagnostic_counts")
        .cloned()
        .unwrap_or_else(|| json!(counts));
    let provenance = crate::guide_repository::json_file(&root.join("provenance.json"), 16_000_000)?;
    let mut file_warnings = array(warnings).to_vec();
    file_warnings.extend(
        array(&raw["parser_errors"])
            .iter()
            .filter(|e| e["classification"] != "rejected_by_both")
            .map(|e| {
                json!(format!(
                    "{}:{}: {}",
                    s(&e["file"]),
                    e["line"],
                    s(&e["message"])
                ))
            }),
    );
    let empty = json!([]);
    let recoveries = raw.get("source_recoveries").unwrap_or(&empty);
    Ok(
        json!({"schema_version":1,"source":"static_analysis","runtime_evaluated":false,
      "status":if !array(&raw["parser_errors"]).is_empty()||!array(warnings).is_empty()||!array(recoveries).is_empty(){"partial"}else{"analyzed"},
      "rules":{"repository":provenance["repository"],"revision":provenance["revision"],"adapter":"cwtools-engine-v1","engine":"cwtools","engine_revision":raw["engine_revision"],"parser_compatibility":raw.get("parser_compatibility").unwrap_or(&json!(0)),"reviewed_recovery":raw.get("reviewed_recovery").unwrap_or(&json!(0)),"game_version_compatibility":"not_certified","load_errors":[]},
      "coverage":{"documents":raw["documents"],"entities":raw["entities"],"recognized":null,"unsupported":null,"unknown_scope":null,"unhandled":{},"metrics_available":false,"type_names":raw["type_names"],"localisation_sources":raw.get("localisation_sources").unwrap_or(&empty),"parser_errors":array(&raw["parser_errors"]).len(),"ignored_parser_errors":ignored.len(),"recovered_sources":array(recoveries).len()},
      "summary":summary,"diagnostics":diagnostics,"truncated":summary["total"].as_u64().unwrap_or(0)>diagnostics.len()as u64,
      "parser_error_details":raw["parser_errors"],"ignored_source_errors":ignored,"source_recoveries":recoveries,
      "input_adjustments":raw.get("input_adjustments").unwrap_or(&empty),"non_script_files":raw.get("non_script_files").unwrap_or(&empty),"file_warnings":file_warnings,"sources":files,"elapsed_ms":raw.get("elapsed_ms").unwrap_or(&Value::Null)}),
    )
}
pub fn analyze(manifest: &Value, rules: &Path, host: &Path, parser: &Path) -> Result<Value> {
    if provenance(host)?["revision"] != ENGINE_REVISION {
        return Err("Versione host CWTools non supportata".into());
    }
    provenance(rules)?;
    let request = json!({"schema_version":1,"rules_dir":rules,"parser_library":parser,"files":manifest["files"]});
    let bytes = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
    let reply = hoi4companion_parser::analysis::run(
        &host.join(if cfg!(windows) {
            "Companion.Cwtools.exe"
        } else {
            "Companion.Cwtools"
        }),
        &bytes,
        std::time::Duration::from_secs(300),
    )?;
    let raw: Value = serde_json::from_slice(&reply).map_err(|e| e.to_string())?;
    if raw["parser_compatibility"] != 1 || raw["reviewed_recovery"] != 1 {
        return Err(
            "Contratto motore CWTools non supportato: ricompila i componenti nativi".into(),
        );
    }
    normalize(&raw, &manifest["files"], &manifest["warnings"], rules, 2000)
}
/// Keep unreadable source history separately from diagnostics and live state.
/// A damaged/unwritable log is preserved and reported as a partial analysis.
pub fn record_errors(report: &Value, path: &Path, now: &str) -> Value {
    if report.get("ignored_source_errors").is_none() {
        return report.clone();
    }
    let mut result = report.clone();
    if result.get("file_warnings").is_none() {
        result["file_warnings"] = json!([]);
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    result["source_error_log"] = json!({"status":"saved","path":installation::display(&absolute),"count":array(&report["ignored_source_errors"]).len(),"recovered":array(&report["source_recoveries"]).len()});
    let saved = (|| -> Result<()> {
        let registry = if path.exists() {
            crate::guide_repository::json_file(path, 32_000_000)?
        } else {
            json!({"schema_version":1,"entries":[]})
        };
        if registry["schema_version"] != 1
            || !registry["entries"].is_array()
            || array(&registry["entries"])
                .iter()
                .any(|e| !e.is_object() || !e["id"].is_string())
        {
            return Err("Formato del registro non supportato".into());
        }
        let mut existing: BTreeMap<String, Value> = array(&registry["entries"])
            .iter()
            .map(|e| (s(&e["id"]).into(), e.clone()))
            .collect();
        let scanned: BTreeSet<_> = report["sources"]
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(_, v)| v.as_str())
            .collect();
        for entry in existing.values_mut() {
            if scanned.contains(s(&entry["source_path"])) {
                entry["active"] = json!(false);
            }
        }
        let mut outcomes = array(&report["ignored_source_errors"])
            .iter()
            .cloned()
            .map(|e| (e, "skipped_unreadable_source"))
            .collect::<Vec<_>>();
        for item in array(&report["source_recoveries"]) {
            let evidence = &item["evidence"];
            outcomes.push((json!({"file":item["file"],"source_sha256":item["source_sha256"],"classification":"recovered_by_both","jomini_error":evidence["original_error"],"recovery":evidence}),"recovered_in_memory"));
        }
        for (error, disposition) in outcomes {
            let file = s(&error["file"]);
            let source = report["sources"]
                .get(file)
                .and_then(Value::as_str)
                .unwrap_or("");
            let identity = json!([
                if source.is_empty() { file } else { source },
                error["source_sha256"]
            ]);
            let key = format!(
                "{:x}",
                Sha256::digest(crate::reference_library::legacy_json(&identity).as_bytes())
            );
            let mut entry = existing.get(&key).cloned().unwrap_or_else(|| json!({}));
            for (k, v) in error.as_object().ok_or("Invalid parser error")? {
                entry[k] = v.clone();
            }
            let first = entry
                .get("first_seen")
                .cloned()
                .unwrap_or_else(|| json!(now));
            for(k,v)in json!({"id":key,"source_path":source,"first_seen":first,"last_seen":now,"active":true,"disposition":disposition,"engine_revision":report["rules"]["engine_revision"],"rules_revision":report["rules"]["revision"]}).as_object().unwrap(){entry[k]=v.clone();}
            existing.insert(key, entry);
        }
        let registry = json!({"schema_version":1,"updated_at":now,"meaning":"Parser rejection is not proof of a gameplay defect. Sources are retried on each analysis.","entries":existing.into_values().collect::<Vec<_>>()});
        let data = serde_json::to_vec_pretty(&registry).map_err(|e| e.to_string())?;
        if data.len() > 32_000_000 {
            return Err("Source error registry exceeds limit".into());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        crate::reference_library::atomic_write(path, &data)
    })();
    if let Err(error) = saved {
        result["source_error_log"]["status"] = json!("error");
        result["source_error_log"]["error"] = json!(error);
        result["status"] = json!("partial");
        result["file_warnings"]
            .as_array_mut()
            .unwrap()
            .push(json!(format!(
                "Registro dei sorgenti accantonati non salvato: {error}"
            )));
    }
    result
}
