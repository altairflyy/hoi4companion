//! Read-only installation inventory and deterministic mod/localization overlays.
use crate::script_document::{all, read as document, scalar};
use serde_json::{Map, Value, json};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};
type Result<T> = std::result::Result<T, String>;
fn path(value: &Value) -> Result<PathBuf> {
    Ok(PathBuf::from(
        value.as_str().ok_or("Missing installation path")?,
    ))
}
pub fn display(path: &Path) -> String {
    let normalized = path.components().collect::<PathBuf>();
    let value = normalized.to_string_lossy();
    if let Some(tail) = value.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{tail}")
    } else {
        value.strip_prefix(r"\\?\").unwrap_or(&value).to_string()
    }
}
fn resolve(path: &Path) -> Result<PathBuf> {
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    Ok(PathBuf::from(display(&path)))
}
fn json_file(path: &Path) -> Result<Value> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    if data.len() > 16 * 1024 * 1024 {
        return Err("Installation profile exceeds limit".into());
    }
    serde_json::from_slice(data.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&data))
        .map_err(|e| e.to_string())
}
pub fn files(root: &Path, suffix: &str) -> Result<Vec<PathBuf>> {
    fn walk(root: &Path, out: &mut Vec<PathBuf>, suffix: &str, depth: usize) -> Result<()> {
        if depth > 256 || out.len() > 1_000_000 {
            return Err("Installation file traversal exceeds limit".into());
        }
        for item in std::fs::read_dir(root).map_err(|e| e.to_string())? {
            let item = item.map_err(|e| e.to_string())?;
            let kind = item.file_type().map_err(|e| e.to_string())?;
            let path = item.path();
            if kind.is_dir() {
                walk(&path, out, suffix, depth + 1)?;
            } else if (kind.is_file() || kind.is_symlink() && path.is_file())
                && item
                    .file_name()
                    .to_string_lossy()
                    .to_lowercase()
                    .ends_with(suffix)
            {
                out.push(path);
            }
        }
        Ok(())
    }
    let mut result = vec![];
    if root.is_dir() {
        walk(root, &mut result, &suffix.to_lowercase(), 0)?;
    }
    result.sort_by_key(|p| p.to_string_lossy().to_lowercase());
    Ok(result)
}
pub fn collect(game: &Path, mods: &[PathBuf], subdir: &str, extension: &str) -> Result<Value> {
    let mut result = Map::new();
    let mut warnings = vec![];
    for (i, root) in std::iter::once(game)
        .chain(mods.iter().map(PathBuf::as_path))
        .enumerate()
    {
        let descriptor = root.join("descriptor.mod");
        if i > 0 && descriptor.exists() {
            match document(&descriptor) {
                Ok(node) => {
                    let replacements: Vec<_> = all(&node, "replace_path")
                        .filter_map(Value::as_str)
                        .map(|s| s.replace('\\', "/").trim_end_matches('/').to_owned())
                        .collect();
                    if replacements.iter().any(|r| {
                        subdir == r
                            || subdir.starts_with(&(r.clone() + "/"))
                            || r.starts_with(&(subdir.to_owned() + "/"))
                    }) {
                        result.retain(|key, _| {
                            !replacements
                                .iter()
                                .any(|r| key == r || key.starts_with(&(r.clone() + "/")))
                        });
                    }
                }
                Err(_) => warnings.push(format!(
                    "Descrittore mod non valido: {}",
                    display(&descriptor)
                )),
            }
        }
        let base = root.join(subdir);
        for file in files(&base, extension)? {
            let relative =
                Path::new(subdir).join(file.strip_prefix(&base).map_err(|e| e.to_string())?);
            let key = relative.to_string_lossy().replace('\\', "/").to_lowercase();
            result.insert(key, json!(display(&file)));
        }
    }
    Ok(json!({"files":result.into_values().collect::<Vec<_>>(),"warnings":warnings}))
}
pub fn mods(docs: &Path) -> Result<Value> {
    let config = docs.join("dlc_load.json");
    let mut result = json!({"status":"unavailable","source":display(&config),"basis":"launcher_profile","items":[],"roots":[],"warnings":[]});
    let source = json_file(&config).and_then(|v| {
        v["enabled_mods"]
            .as_array()
            .cloned()
            .ok_or_else(|| "enabled_mods deve essere un elenco".into())
    });
    let items = match source {
        Ok(items) => items,
        Err(error) => {
            result["warnings"] = json!([format!(
                "Impossibile leggere le mod da {}: {error}",
                display(&config)
            )]);
            return Ok(result);
        }
    };
    let mut rows = vec![];
    for item in items {
        let name = if item.is_null() {
            "None".into()
        } else {
            item.as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| item.to_string())
        };
        let mut row =
            json!({"descriptor":name,"name":name,"path":null,"status":"unavailable","reason":""});
        let Some(reference) = item.as_str().filter(|s| !s.trim().is_empty()) else {
            row["reason"] = json!("Riferimento mod non valido nel profilo");
            rows.push(row);
            continue;
        };
        let descriptor = docs.join(reference.replace('\\', "/"));
        let outcome = (|| -> Result<()> {
            let node = document(&descriptor)?;
            let name = scalar(&node, "name");
            if !name.is_empty() {
                row["name"] = json!(name);
            }
            let location = scalar(&node, "path");
            if location.is_empty() {
                row["reason"] = json!(if scalar(&node, "archive").is_empty() {
                    "Percorso mod assente"
                } else {
                    "Mod archivio ZIP non supportata"
                });
                return Ok(());
            }
            let mut folder = PathBuf::from(location.replace('\\', "/"));
            if let Some(rest) = folder.to_str().and_then(|s| s.strip_prefix("~/"))
                && let Some(home) =
                    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))
            {
                folder = PathBuf::from(home).join(rest);
            }
            if !folder.is_absolute() {
                let mut candidates = vec![];
                for base in [docs, descriptor.parent().unwrap_or(docs)] {
                    let candidate = base.join(&folder);
                    if candidate.is_dir() {
                        let candidate = resolve(&candidate)?;
                        if !candidates.contains(&candidate) {
                            candidates.push(candidate);
                        }
                    }
                }
                if candidates.len() != 1 {
                    row["reason"] = json!(if candidates.is_empty() {
                        format!("Cartella mod non trovata: {location}")
                    } else {
                        "Percorso mod relativo ambiguo".into()
                    });
                    return Ok(());
                }
                folder = candidates.remove(0);
            }
            if folder.is_dir() {
                row["path"] = json!(display(&resolve(&folder)?));
                row["status"] = json!("loaded");
            } else {
                row["reason"] = json!(format!("Cartella mod non trovata: {}", display(&folder)));
            }
            Ok(())
        })();
        if let Err(error) = outcome {
            row["reason"] = json!(format!("Mod non leggibile: {error}"));
        }
        rows.push(row);
    }
    let warnings: Vec<_> = rows
        .iter()
        .filter(|r| r["reason"] != "")
        .map(|r| {
            format!(
                "{}: {}",
                r["descriptor"].as_str().unwrap_or(""),
                r["reason"].as_str().unwrap_or("")
            )
        })
        .collect();
    let mut roots = vec![];
    for row in &rows {
        if row["status"] == "loaded" {
            roots.retain(|p| p != &row["path"]);
            roots.push(row["path"].clone());
        }
    }
    result["status"] = json!(if warnings.is_empty() {
        "observed"
    } else {
        "partial"
    });
    result["items"] = json!(rows);
    result["roots"] = json!(roots);
    result["warnings"] = json!(warnings);
    Ok(result)
}
fn localization_line(line: &str) -> Option<(&str, String)> {
    let line =
        line.trim_start_matches(|c: char| c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c));
    let colon = line.find(':')?;
    let key = &line[..colon];
    if key.is_empty() || key.chars().any(|c| c == '#' || c.is_whitespace()) {
        return None;
    }
    let rest = &line[colon + 1..];
    let rest = rest.trim_start_matches(char::is_numeric);
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let rest = rest
        .trim_start_matches(char::is_whitespace)
        .strip_prefix('"')?;
    let mut escaped = false;
    for (i, c) in rest.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
        } else if c == '"' {
            return Some((key, rest[..i].replace("\\n", "\n").replace("\\\"", "\"")));
        }
    }
    None
}
pub fn localisations(game: &Path, mods: &[PathBuf], language: &str) -> Result<Value> {
    let mut result = Map::new();
    let mut reference = Map::new();
    let mut languages = vec!["english"];
    if language != "english" {
        languages.push(language);
    }
    for language in languages {
        for root in std::iter::once(game).chain(mods.iter().map(PathBuf::as_path)) {
            let mut english = Map::new();
            for file in files(&root.join("localisation"), &format!("_l_{language}.yml"))? {
                let Ok(data) = std::fs::read(&file) else {
                    continue;
                };
                let data =
                    String::from_utf8_lossy(data.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&data));
                for line in data.lines() {
                    if let Some((key, text)) = localization_line(line) {
                        result.insert(key.into(), json!(text));
                        if language == "english" {
                            english.insert(key.into(), json!(text));
                        }
                    }
                }
            }
            if language == "english" {
                for (key, text) in english {
                    if root == game || !reference.contains_key(&key) {
                        reference.insert(key, text);
                    }
                }
            }
        }
    }
    Ok(json!([result, reference]))
}
pub fn dlcs(game: &Path, docs: Option<&Path>) -> Result<Value> {
    let mut warnings = vec![];
    let mut known = true;
    let mut disabled = HashSet::new();
    let config = docs.map(|d| d.join("dlc_load.json"));
    if config.as_ref().is_none_or(|p| !p.is_file()) {
        known = false;
        warnings.push("Configurazione DLC del launcher assente: attivazione dei DLC non integrati da verificare.".into());
    } else {
        match json_file(config.as_ref().unwrap()).and_then(|v| {
            v["disabled_dlcs"]
                .as_array()
                .filter(|a| a.iter().all(Value::is_string))
                .cloned()
                .ok_or_else(|| "disabled_dlcs non è una lista di stringhe".into())
        }) {
            Ok(items) => {
                for item in items {
                    disabled.insert(hoi4companion_catalog_core::casefold(
                        &item.as_str().unwrap().replace('\\', "/"),
                    ));
                }
            }
            Err(error) => {
                known = false;
                warnings.push(format!(
                    "Configurazione DLC del launcher non leggibile: {error}"
                ));
            }
        }
    }
    let mut complete = known;
    let mut items = vec![];
    for group in ["dlc", "integrated_dlc"] {
        let descriptors = match files(&game.join(group), ".dlc") {
            Ok(v) => v,
            Err(error) => {
                complete = false;
                warnings.push(format!("Inventario DLC incompleto: {error}"));
                continue;
            }
        };
        for descriptor in descriptors {
            let item = (|| -> Result<Value> {
                let node = document(&descriptor)?;
                let name = scalar(&node, "name");
                if name.is_empty() {
                    return Err("nome DLC assente".into());
                }
                let rel = descriptor
                    .strip_prefix(game)
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .replace('\\', "/");
                let identifiers = [
                    rel.clone(),
                    descriptor
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    descriptor
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    name.into(),
                    scalar(&node, "steam_id").into(),
                    scalar(&node, "pops_id").into(),
                ];
                let integrated = group == "integrated_dlc";
                let archive = scalar(&node, "archive");
                let payload = if archive.is_empty() {
                    scalar(&node, "path")
                } else {
                    archive
                };
                let present = if integrated {
                    descriptor.parent().is_some_and(Path::is_dir)
                } else {
                    !payload.is_empty()
                        && if archive.is_empty() {
                            game.join(payload.replace('\\', "/")).is_dir()
                        } else {
                            game.join(payload.replace('\\', "/")).is_file()
                        }
                };
                let status = if !present {
                    "missing"
                } else if integrated {
                    "integrated"
                } else if identifiers
                    .iter()
                    .any(|s| disabled.contains(&hoi4companion_catalog_core::casefold(s)))
                {
                    "disabled"
                } else if known {
                    "enabled"
                } else {
                    "unknown"
                };
                Ok(
                    json!({"name":name,"status":status,"descriptor":rel,"steam_id":scalar(&node,"steam_id")}),
                )
            })();
            match item {
                Ok(item) => items.push(item),
                Err(error) => {
                    complete = false;
                    warnings.push(format!(
                        "DLC non leggibile ({}): {error}",
                        descriptor.file_name().unwrap_or_default().to_string_lossy()
                    ));
                }
            }
        }
    }
    Ok(json!({"items":items,"complete":complete,"warnings":warnings}))
}
pub fn execute(op: &str, args: &Value) -> Result<Value> {
    if op == "installation_mods" {
        return mods(&path(&args["docs"])?);
    }
    let game = path(&args["game"])?;
    if op == "installation_dlcs" {
        let docs = args["docs"].as_str().map(PathBuf::from);
        return dlcs(&game, docs.as_deref());
    }
    let mods = args["mods"]
        .as_array()
        .into_iter()
        .flatten()
        .map(path)
        .collect::<Result<Vec<_>>>()?;
    match op {
        "installation_files" => collect(
            &game,
            &mods,
            args["subdir"].as_str().ok_or("Missing subdirectory")?,
            args["extension"].as_str().ok_or("Missing extension")?,
        ),
        "installation_localisations" => localisations(
            &game,
            &mods,
            args["language"].as_str().ok_or("Missing language")?,
        ),
        _ => Err("Unknown installation operation".into()),
    }
}
