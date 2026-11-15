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
        let mut value=self.presentation.enrich(guide)?;
        value["advanced_references"]=json!(self.library["documents"].as_array().into_iter().flatten().map(|d|json!({"id":d["id"],"name":d["name"],"category":d["category"]})).collect::<Vec<_>>());
        Ok(value)
    }
    pub fn prepare_presented(&self,guide:&Value)->Result<Value,String>{
        let build=||self.present(&self.prepare(guide)?);
        if let Some(cache)=&self.prepared_cache{cache.resolve(guide,build)}else{build()}
    }
    /// Current compiler contract for NEW checklists, from the associated source:
    /// contract-4 preparation (uncached; contract-4 parsing such as production
    /// pairs), presentation, withdrawal of catalog-contradicted bindings, then
    /// decomposition and grounded triggers. The contract-1 registry is untouched.
    pub fn compile_new(&self, source: &Value) -> Result<Value, String> {
        let prepared = crate::informal_targets::with_contract4_parsing(|| {
            runtime_definition::prepare(source, &self.index, &self.source["continuous_focuses"], &self.library)
        })?;
        let mut presented = self.present(&prepared)?;
        crate::decomposition::drop_duplicate_production(&mut presented)?;
        crate::decomposition::withdraw_incompatible_decisions(&mut presented, &self.index)?;
        // Present again so withdrawn bindings and new children get fresh names.
        self.present(&self.decompose(&presented)?)
    }
    /// Contract-2 compile of an already prepared+presented (contract-1) guide:
    /// headers use promote's catalog condition compiler; new children are
    /// presented with the same catalog. A guide without containers is unchanged.
    pub fn decompose(&self, guide: &Value) -> Result<Value, String> {
        let compile = |text: &str| crate::operation_language::compile_condition(guide, &self.index, text);
        // Catalog focuses of the guide's own tree add grounded candidates.
        let extra = crate::entity_resolution::catalog_candidates(self.index.rows(), guide);
        let result = crate::decomposition::decompose_with(guide, &compile, &extra)?;
        if result["report"].as_array().is_some_and(|r| r.iter().any(|e| e["status"] == "decomposed")) {
            self.present(&result["guide"])
        } else {
            Ok(result["guide"].clone())
        }
    }
    pub fn compare(&self, guide: &Value) -> Result<Value, String> {
        let prepared = self.prepare(guide)?;
        Ok(json!({"definition":prepared,"presentation":self.present(&prepared)?}))
    }
}
