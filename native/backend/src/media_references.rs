//! Source/time reference selection only. A visual citation is authored evidence,
//! never proof of the current campaign, and distinct videos are not guessed.
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashMap, HashSet};
fn a(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn time(v: &Value) -> bool {
    v.as_f64()
        .is_some_and(|n| n.is_finite() && (0.0..14400.0).contains(&n))
}
fn decoded(input: &str) -> Option<String> {
    let raw = input.as_bytes();
    let mut result = vec![];
    let mut pos = 0;
    while pos < raw.len() {
        if raw[pos] == b'%' && pos + 2 < raw.len() {
            let digit = |b: u8| match b {
                b'0'..=b'9' => Some(b - b'0'),
                b'a'..=b'f' => Some(b - b'a' + 10),
                b'A'..=b'F' => Some(b - b'A' + 10),
                _ => None,
            };
            result.push(digit(raw[pos + 1])? * 16 + digit(raw[pos + 2])?);
            pos += 3
        } else {
            result.push(if raw[pos] == b'+' { b' ' } else { raw[pos] });
            pos += 1
        }
    }
    String::from_utf8(result).ok()
}
pub fn youtube(url: &str) -> Result<String, String> {
    let invalid = || "URL YouTube non valido".to_owned();
    let tail = url.strip_prefix("https://").ok_or_else(invalid)?;
    let host_end = tail.find(['/', '?', '#']).unwrap_or(tail.len());
    let host = &tail[..host_end];
    if ![
        "youtube.com",
        "www.youtube.com",
        "m.youtube.com",
        "youtu.be",
    ]
    .contains(&host.to_lowercase().as_str())
        || url.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        return Err(invalid());
    }
    let remainder = &tail[host_end..];
    let no_fragment = remainder.split('#').next().unwrap_or("");
    let (path, query) = no_fragment.split_once('?').unwrap_or((no_fragment, ""));
    let key = if host.eq_ignore_ascii_case("youtu.be") {
        path.trim_matches('/').to_owned()
    } else if path.starts_with("/shorts/") || path.starts_with("/live/") {
        path.split('/').nth(2).unwrap_or("").to_owned()
    } else {
        query
            .split('&')
            .find_map(|pair| {
                let (k, v) = pair.split_once('=')?;
                (decoded(k)?.as_str() == "v" && !v.is_empty())
                    .then(|| decoded(v))
                    .flatten()
            })
            .unwrap_or_default()
    };
    if key.len() != 11
        || !key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err("Indica un singolo video YouTube".into());
    }
    Ok(format!("https://www.youtube.com/watch?v={key}"))
}
fn canonical(source: &Value) -> Option<String> {
    youtube(s(&source["url"])).ok()
}
struct Context<'a> {
    step: &'a Value,
    sources: HashMap<&'a str, &'a Value>,
    urls: BTreeSet<String>,
    direct: Vec<&'a Value>,
    inventory: Vec<&'a Value>,
}
impl<'a> Context<'a> {
    fn new(guide: &'a Value, step: &'a Value) -> Self {
        let sources: HashMap<_, _> = a(&guide["sources"])
            .iter()
            .filter_map(|v| Some((v["id"].as_str()?, v)))
            .collect();
        let urls = sources
            .values()
            .copied()
            .chain(std::iter::once(&guide["source"]))
            .filter_map(canonical)
            .collect();
        let direct = a(&step["evidence"])
            .iter()
            .filter(|v| time(&v["timestamp_seconds"]))
            .collect();
        let inventory = a(&guide["source_inventory"]["items"])
            .iter()
            .filter(|v| {
                a(&v["step_keys"]).contains(&step["key"])
                    && !crate::state::truth(&v["excluded_reason"])
                    && time(&v["timestamp_seconds"])
            })
            .collect();
        Self {
            step,
            sources,
            urls,
            direct,
            inventory,
        }
    }
    fn source(&self, item: &Value) -> &Value {
        self.sources
            .get(s(&item["source_id"]))
            .copied()
            .unwrap_or(&Value::Null)
    }
    fn select(&self, items: &[&Value], seconds: Option<&Value>) -> Option<Value> {
        if items.is_empty() {
            return None;
        }
        let mut urls = BTreeSet::new();
        for item in items {
            let source = self.source(item);
            let url = canonical(source);
            if crate::state::truth(&source["url"]) && url.is_none() {
                return None;
            }
            if let Some(url) = url {
                urls.insert(url);
            }
        }
        if urls.is_empty() {
            urls = self.urls.clone()
        }
        if urls.len() != 1 {
            return None;
        }
        Some(
            json!({"url":urls.first().unwrap(),"seconds":seconds.unwrap_or(&items[0]["timestamp_seconds"])}),
        )
    }
    fn preferred(&self) -> Option<Value> {
        if let Some(seconds) = self.step["action"]["params"].get("source_seconds") {
            if !time(seconds) {
                return None;
            }
            let all = self
                .direct
                .iter()
                .chain(&self.inventory)
                .copied()
                .collect::<Vec<_>>();
            let matching = all
                .iter()
                .copied()
                .filter(|e| e["timestamp_seconds"] == *seconds)
                .collect::<Vec<_>>();
            return if !matching.is_empty() {
                self.select(&matching, Some(seconds))
            } else if !all.is_empty() {
                self.select(&all, Some(seconds))
            } else {
                self.select(&[&json!({"timestamp_seconds":seconds})], Some(seconds))
            };
        }
        for items in [
            self.direct
                .iter()
                .copied()
                .filter(|e| e["basis"] == "visual")
                .collect::<Vec<_>>(),
            self.inventory
                .iter()
                .copied()
                .filter(|e| e["basis"] == "visual")
                .collect(),
            self.direct.clone(),
            self.inventory.clone(),
        ] {
            if !items.is_empty() {
                return self.select(&items, None);
            }
        }
        None
    }
}
pub fn frame(guide: &Value, step: &Value) -> Value {
    Context::new(guide, step).preferred().unwrap_or(Value::Null)
}
pub fn references(guide: &Value, step: &Value) -> Value {
    let context = Context::new(guide, step);
    let preferred = context.preferred();
    let mut references = vec![];
    let mut seen = HashSet::new();
    let mut quotes = HashMap::<String, Value>::new();
    let mut add = |item: &Value, origin: &str, override_url: Option<&str>| {
        let source_id = &item["source_id"];
        if !source_id.is_null() && !context.sources.contains_key(s(source_id)) {
            return;
        }
        let source = context.source(item);
        let mut url = override_url
            .map(str::to_owned)
            .or_else(|| canonical(source));
        if url.is_none() && crate::state::truth(&source["url"]) {
            return;
        }
        if url.is_none()
            && !source.is_null()
            && !source["type"].is_null()
            && source["type"] != "youtube"
        {
            return;
        }
        if url.is_none() && context.urls.len() == 1 {
            url = context.urls.first().cloned()
        }
        let Some(url) = url else { return };
        let seconds = &item["timestamp_seconds"];
        let basis = item
            .get("basis")
            .cloned()
            .unwrap_or_else(|| json!("unverified"));
        let quote = &item["quote"];
        let quotation = quote
            .as_str()
            .filter(|q| !q.is_empty())
            .map(|_| json!([url, seconds, source_id, quote]).to_string());
        if basis == "unverified"
            && quotation
                .as_ref()
                .and_then(|k| quotes.get(k))
                .is_some_and(|v| v == "explicit" || v == "visual")
        {
            return;
        }
        let identity = json!([url, seconds, basis, source_id, quote]).to_string();
        if !seen.insert(identity) {
            return;
        }
        if let Some(key) = quotation {
            quotes.insert(key, basis.clone());
        }
        let title = source
            .get("title")
            .filter(|v| crate::state::truth(v))
            .or_else(|| {
                guide["source"]
                    .get("title")
                    .filter(|v| crate::state::truth(v))
            })
            .cloned()
            .unwrap_or_else(|| json!("Video"));
        let n = seconds.as_f64().unwrap_or(0.);
        let mut row = json!({"id":format!("r{}",references.len()),"basis":basis,"origin":origin,"source_id":source_id,"source_title":title,"url":url,"seconds":seconds,"clip_start":(n-5.).max(0.),"clip_end":(n+10.).min(14400.)});
        if time(&item["end_seconds"]) && item["end_seconds"].as_f64().unwrap() >= n {
            row["end_seconds"] = item["end_seconds"].clone();
        }
        if let Some(quote) = quote.as_str() {
            row["quote"] = json!(quote.chars().take(4096).collect::<String>());
        }
        references.push(row);
    };
    let explicit = &step["action"]["params"]["source_seconds"];
    let unknown =
        context.direct.iter().chain(&context.inventory).any(|e| {
            !e["source_id"].is_null() && !context.sources.contains_key(s(&e["source_id"]))
        });
    if time(explicit)
        && !unknown
        && let Some(preferred) = &preferred
    {
        let matching = context.direct.iter().chain(&context.inventory).find(|e| {
            e["timestamp_seconds"] == *explicit
                && canonical(context.source(e)).as_deref() == preferred["url"].as_str()
        });
        let mut item = json!({"timestamp_seconds":explicit,"basis":"declared"});
        if let Some(matching) = matching {
            item["source_id"] = matching["source_id"].clone();
        }
        add(&item, "declared", preferred["url"].as_str());
    }
    let mut items = context
        .direct
        .iter()
        .map(|v| (*v, "step"))
        .chain(context.inventory.iter().map(|v| (*v, "inventory")))
        .collect::<Vec<_>>();
    items.sort_by_key(|(v, _)| match s(&v["basis"]) {
        "visual" => 0,
        "explicit" => 1,
        _ => 2,
    });
    for (item, origin) in items {
        add(item, origin, None)
    }
    let default = references
        .iter()
        .find(|r| {
            preferred
                .as_ref()
                .is_some_and(|p| p["url"] == r["url"] && p["seconds"] == r["seconds"])
        })
        .map(|r| r["id"].clone());
    let truncated = references.len() > 256;
    references.truncate(256);
    let default = default
        .filter(|id| references.iter().any(|r| r["id"] == *id))
        .unwrap_or(Value::Null);
    json!({"references":references,"default_reference":default,"truncated":truncated})
}
