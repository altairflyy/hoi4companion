//! Static scenario research context. Not a live observation or campaign history.
use crate::{import_catalog::regex, installation, script_document as script, source_eligibility};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
type Result<T> = std::result::Result<T, String>;
type Research = BTreeMap<String, Option<bool>>;
fn date(text: &str) -> Result<[u32; 4]> {
    if !regex(r"^\d{4}\.\d{1,2}\.\d{1,2}(?:\.\d{1,2})?$").is_match(text) {
        return Err("Data iniziale ricerca non valida".into());
    }
    let mut result = [0; 4];
    for (i, p) in text.split('.').enumerate() {
        result[i] = p.parse().map_err(|_| "Data iniziale ricerca non valida")?;
    }
    if !(1..=12).contains(&result[1]) || !(1..=31).contains(&result[2]) {
        return Err("Data iniziale ricerca non valida".into());
    }
    Ok(result)
}
fn limits(node: &Value) -> Value {
    json!({"entries":script::entries(node).map(|(k,v)|{let upper=k.to_uppercase();json!([if ["AND","OR","NOT","NOR","NAND"].contains(&upper.as_str()){upper}else{k.into()},if v["entries"].is_array(){limits(v)}else{v.clone()}])}).collect::<Vec<_>>()})
}
struct Context<'a> {
    country: &'a str,
    dlcs: &'a Value,
    scripts: &'a Value,
}
impl Context<'_> {
    fn apply(&self, node: &Value, state: &mut Research, depth: usize) -> Result<()> {
        if depth > 32 {
            return Err("Storia ricerca troppo profonda".into());
        }
        for (key, value) in script::entries(node) {
            let block = value["entries"].is_array();
            if key == "set_technology" && block {
                for (technology, level) in script::entries(value) {
                    if technology == "popup" {
                        continue;
                    }
                    let number = level
                        .as_str()
                        .and_then(|s| s.trim().parse::<f64>().ok())
                        .filter(|n| n.is_finite());
                    state.insert(technology.into(), number.map(|n| n > 0.));
                }
            } else if key == "if" && block {
                let answer = script::first(value, "limit")
                    .filter(|n| n["entries"].is_array())
                    .and_then(|n| {
                        source_eligibility::condition(
                            &limits(n),
                            self.country,
                            self.dlcs,
                            self.scripts,
                        )
                    });
                let yes = json!({"entries":script::entries(value).filter(|(k,_)|!["limit","else","else_if"].contains(k)).map(|(k,v)|json!([k,v])).collect::<Vec<_>>()});
                let mut no = script::first(value, "else")
                    .filter(|n| n["entries"].is_array())
                    .cloned()
                    .unwrap_or_else(|| json!({"entries":[]}));
                let alternatives = script::blocks(value, "else_if").collect::<Vec<_>>();
                for alternate in alternatives.into_iter().rev() {
                    let mut entries = alternate["entries"].as_array().unwrap().clone();
                    entries.push(json!(["else", no]));
                    no = json!({"entries":[["if",{"entries":entries}]]});
                }
                match answer {
                    Some(true) => self.apply(&yes, state, depth + 1)?,
                    Some(false) => self.apply(&no, state, depth + 1)?,
                    None => {
                        let (mut left, mut right) = (state.clone(), state.clone());
                        self.apply(&yes, &mut left, depth + 1)?;
                        self.apply(&no, &mut right, depth + 1)?;
                        let keys = left
                            .keys()
                            .chain(right.keys())
                            .cloned()
                            .collect::<BTreeSet<_>>();
                        for key in keys {
                            let a = left.get(&key).copied().unwrap_or(Some(false));
                            let b = right.get(&key).copied().unwrap_or(Some(false));
                            state.insert(key, if a == b { a } else { None });
                        }
                    }
                }
            } else if block
                && !["limit", "else"].contains(&key)
                && !regex(r"^\d{4}\.").is_match(key)
            {
                let mut possible = Research::new();
                self.apply(value, &mut possible, depth + 1)?;
                for key in possible.into_keys() {
                    state.insert(key, None);
                }
            }
        }
        Ok(())
    }
}
pub fn read(args: &Value) -> Result<Value> {
    let Some(country) = args["country"]
        .as_str()
        .filter(|s| regex(r"^[A-Z0-9]{3}$").is_match(s))
    else {
        return Ok(Value::Null);
    };
    let game = Path::new(args["game"].as_str().ok_or("Missing game directory")?);
    let mods = args["mods"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|v| {
            v.as_str()
                .map(PathBuf::from)
                .ok_or("Invalid mod path".into())
        })
        .collect::<Result<Vec<_>>>()?;
    let files = installation::collect(game, &mods, "history/countries", ".txt")?;
    let prefix = format!("{country} - ");
    let files = files["files"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|p| {
            Path::new(p)
                .file_name()
                .is_some_and(|s| s.to_string_lossy().starts_with(&prefix))
        })
        .collect::<Vec<_>>();
    if files.len() != 1 {
        return Ok(Value::Null);
    }
    let start = if args["start_date"].is_null() {
        let bookmarks = installation::collect(game, &mods, "common/bookmarks", ".txt")?;
        let mut dates = vec![];
        for file in bookmarks["files"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            let node = script::read(Path::new(file))?;
            for group in script::blocks(&node, "bookmarks") {
                for bookmark in script::blocks(group, "bookmark") {
                    let text = script::scalar(bookmark, "date");
                    if !text.is_empty() {
                        dates.push((date(text)?, text.to_owned()));
                    }
                }
            }
        }
        dates.sort_by_key(|(date, _)| *date);
        let Some((_, date)) = dates.first() else {
            return Ok(Value::Null);
        };
        date.clone()
    } else {
        args["start_date"]
            .as_str()
            .ok_or("Data iniziale ricerca non valida")?
            .to_owned()
    };
    let start_date = date(&start)?;
    let root = script::read(Path::new(files[0]))?;
    let context = Context {
        country,
        dlcs: &args["dlcs"],
        scripts: &args["scripts"],
    };
    let mut state = Research::new();
    context.apply(&root, &mut state, 0)?;
    let mut dated = vec![];
    for (key, node) in script::entries(&root) {
        if node["entries"].is_array() && regex(r"^\d{4}\.\d{1,2}\.\d{1,2}$").is_match(key) {
            dated.push((date(key)?, node));
        }
    }
    dated.sort_by_key(|(date, _)| *date);
    for (date, node) in dated {
        if date <= start_date {
            context.apply(node, &mut state, 0)?;
        }
    }
    Ok(
        json!({"country":country,"date":start,"researched":state.iter().filter(|(_,v)|**v==Some(true)).map(|(k,_)|k).collect::<Vec<_>>(),"unknown":state.iter().filter(|(_,v)|v.is_none()).map(|(k,_)|k).collect::<Vec<_>>(),"basis":"country_history","source":files[0]}),
    )
}
