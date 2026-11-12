//! One immutable source generation for static adaptation and presentation.
//! Compiled guide cache contains no state, run progress or completion evidence.
use crate::{catalog_generation::Catalog, guide_enrich, import_catalog::Index, runtime_definition};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
pub struct Context {
    index: Index,
    source: Arc<Value>,
    library: Value,
    presentation: guide_enrich::Presentation,
    compiled: Mutex<HashMap<String, (String, Value)>>,
    prepared_cache: Option<crate::compiled_cache::Cache>,
}
impl Context {
    pub(crate) fn political_cost_ids<'a>(&self,guides:impl IntoIterator<Item=&'a Value>)->Value {
        crate::operational_catalog::political_cost_ids(&self.source["operation_entities"],guides)
    }
    pub fn new(catalog: &mut Catalog, library: Value) -> Result<Arc<Self>, String> {
        let source = catalog.sources.clone();
        let index = catalog.import_index()?;
        let presentation = guide_enrich::Presentation::new(source.clone(), &catalog.vocabulary);
        let prepared_cache=catalog.compiled_cache_root.as_ref().and_then(|root|crate::compiled_cache::Cache::open(root,&source,&library,&catalog.vocabulary));
        Ok(Arc::new(Self {
            index,
            source,
            library,
            presentation,
            compiled: Mutex::new(HashMap::new()),
            prepared_cache,
        }))
    }
    pub fn prepare(&self, guide: &Value) -> Result<Value, String> {
        let id = guide["id"].as_str().ok_or("Missing guide identity")?;
        let signature = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(guide).map_err(|e| e.to_string())?)
        );
        {
            let cached = self
                .compiled
                .lock()
                .map_err(|_| "Guide compiler unavailable")?;
            if let Some((previous, compiled)) = cached.get(id)
                && *previous == signature
            {
                return Ok(compiled.clone());
            }
        }
        let result = runtime_definition::prepare(
            guide,
            &self.index,
            &self.source["continuous_focuses"],
            &self.library,
        )?;
        let mut cache = self
            .compiled
            .lock()
            .map_err(|_| "Guide compiler unavailable")?;
        if cache.len() >= 256 {
            cache.clear()
        }
        cache.insert(id.into(), (signature, result.clone()));
        Ok(result)
    }
    pub fn present(&self, guide: &Value) -> Result<Value, String> {
        self.presentation.enrich(guide)
    }
    pub fn prepare_presented(&self,guide:&Value)->Result<Value,String>{
        let build=||self.present(&self.prepare(guide)?);
        if let Some(cache)=&self.prepared_cache{cache.resolve(guide,build)}else{build()}
    }
    pub fn compare(&self, guide: &Value) -> Result<Value, String> {
        let prepared = self.prepare(guide)?;
        Ok(json!({"definition":prepared,"presentation":self.present(&prepared)?}))
    }
}
