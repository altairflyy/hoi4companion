//! Immutable identity context shared by import compilers. No live state.
use hoi4companion_catalog_core::{Catalog, Query};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};
pub fn regex(pattern: &str) -> regex::Regex {
    static CACHE: OnceLock<Mutex<HashMap<String, regex::Regex>>> = OnceLock::new();
    let mut cache = CACHE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if let Some(re) = cache.get(pattern) {
        return re.clone();
    }
    let re = regex::Regex::new(pattern).expect("checked-in import pattern");
    if cache.len() < 128 {
        cache.insert(pattern.into(), re.clone());
    }
    re
}
pub fn language() -> &'static Value {
    static LANGUAGE: OnceLock<Value> = OnceLock::new();
    LANGUAGE.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../src/hoi4companion/guide_language.json"
        ))
        .expect("checked-in guide language")
    })
}
#[derive(Clone)]
pub struct Index {
    inner: std::sync::Arc<Catalog>,
}
impl Index {
    pub fn new(args: &Value) -> Result<Self, String> {
        if let Some(handle) = args["identity_handle"].as_u64() {
            return Ok(Self {
                inner: hoi4companion_catalog_core::borrow_catalog(handle)?,
            });
        }
        let rows = args["entries"]
            .as_array()
            .ok_or("Missing import catalog")?
            .clone();
        let build = json!({"schema_version":1,"entries":rows,"country_names":args.get("country_names").unwrap_or(&json!({})),"adjectives":args.get("adjectives").unwrap_or(&json!([])),"vocabulary":args.get("vocabulary").unwrap_or(language())});
        Ok(Self {
            inner: std::sync::Arc::new(Catalog::compile(
                serde_json::from_value(build).map_err(|e| e.to_string())?,
            )?),
        })
    }
    pub fn rows(&self) -> &[Value] {
        self.inner.entries()
    }
    pub fn vocabulary(&self) -> &Value {
        self.inner.vocabulary()
    }
    pub fn query(&self, mut request: Value) -> Result<Value, String> {
        request["schema_version"] = json!(1);
        self.inner
            .query(serde_json::from_value::<Query>(request).map_err(|e| e.to_string())?)
    }
    pub fn entries(&self, request: Value) -> Result<Vec<Value>, String> {
        self.query(request)?
            .as_array()
            .ok_or("Invalid catalog query")?
            .iter()
            .map(|v| {
                v.as_u64()
                    .and_then(|i| self.rows().get(i as usize))
                    .cloned()
                    .ok_or_else(|| "Invalid catalog index".into())
            })
            .collect()
    }
    pub fn matches(
        &self,
        q: &str,
        kinds: &[&str],
        country: &Value,
        scope: &Value,
        typos: bool,
    ) -> Result<Vec<Value>, String> {
        self.entries(json!({"op":"matches","query":q,"kinds":kinds,"country":country,"scope":scope,"typos":typos}))
    }
}
