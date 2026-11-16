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
    presented: Mutex<HashMap<String, Value>>,
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
            presented: Mutex::new(HashMap::new()),
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
        // Research projection can request the same static presentation on every
        // delivery. Cache by the WHOLE projected definition, including bindings;
        // the owning context fixes catalog, vocabulary and reference library.
        // Live facts, evaluation and completion are never stored here.
        let signature=format!("{:x}",Sha256::digest(serde_json::to_vec(guide).map_err(|e|e.to_string())?));
        if let Some(value)=self.presented.lock().map_err(|_|"Guide presentation unavailable")?.get(&signature) {
            return Ok(value.clone());
        }
        let mut value=self.presentation.enrich(guide)?;
        value["advanced_references"]=json!(self.library["documents"].as_array().into_iter().flatten().map(|d|json!({"id":d["id"],"name":d["name"],"category":d["category"]})).collect::<Vec<_>>());
        let mut cache=self.presented.lock().map_err(|_|"Guide presentation unavailable")?;
        if cache.len()>=64 {cache.clear();}
        cache.insert(signature,value.clone());
        Ok(value)
    }
    pub fn prepare_presented(&self,guide:&Value)->Result<Value,String>{
        let build=||self.present(&self.prepare(guide)?);
        if let Some(cache)=&self.prepared_cache{cache.resolve(guide,build)}else{build()}
    }
    /// Current compiler contract for NEW checklists, from the associated source:
    /// preparation (the same production parsing as the registry), presentation,
    /// withdrawal of catalog-contradicted bindings, then decomposition and
    /// grounded triggers.
    pub fn compile_new(&self, source: &Value) -> Result<Value, String> {
        let prepared = runtime_definition::prepare(source, &self.index, &self.source["continuous_focuses"], &self.library)?;
        let mut presented = self.present(&prepared)?;
        crate::decomposition::withdraw_incompatible_decisions(&mut presented, &self.index)?;
        // Children split from multi-line blocks carry their own event answers.
        let mut decomposed = self.decompose(&presented)?;
        crate::reference_candidates::ground_references(&mut decomposed, &self.index)?;
        // Present again so withdrawn bindings and new children get fresh names.
        self.present(&decomposed)
    }
    /// Contract-2 compile of an already prepared+presented (contract-1) guide:
    /// headers use promote's catalog condition compiler; new children are
    /// presented with the same catalog. A guide without containers is unchanged.
    pub fn decompose(&self, guide: &Value) -> Result<Value, String> {
        let compile = |text: &str| crate::operation_language::compile_condition(guide, &self.index, text);
        // Catalog focuses of the guide's own tree add grounded candidates.
        let extra = crate::entity_resolution::catalog_candidates(self.index.rows(), guide);
        let result = crate::decomposition::decompose_with(guide, &compile, &extra)?;
        let bound = crate::construction_instructions::bind(&result["guide"], &self.index)?;
        if result["report"].as_array().is_some_and(|r| r.iter().any(|e| e["status"] == "decomposed")) {
            self.present(&bound)
        } else {
            Ok(bound)
        }
    }
    pub fn compare(&self, guide: &Value) -> Result<Value, String> {
        let prepared = self.prepare(guide)?;
        Ok(json!({"definition":prepared,"presentation":self.present(&prepared)?}))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presentation_cache_keys_full_definition_and_returns_independent_values() {
        let source=Arc::new(json!({"operation_entities":[]}));
        let context=Context {
            index:Index::new(&json!({"entries":[]})).unwrap(),
            presentation:guide_enrich::Presentation::new(source.clone(),crate::import_catalog::language()),
            source,library:json!({"documents":[]}),compiled:Mutex::new(HashMap::new()),
            presented:Mutex::new(HashMap::new()),prepared_cache:None,
        };
        let mut guide=crate::guide_schema::normalize(&json!({"id":"cache","title":"First","tracks":{"operations":[{"key":"one","action":{"kind":"manual"}}]}})).unwrap();
        let expected=context.present(&guide).unwrap();
        let mut copy=context.present(&guide).unwrap();copy["title"]=json!("caller mutation");
        assert_eq!(context.present(&guide).unwrap(),expected);
        assert_eq!(context.presented.lock().unwrap().len(),1);
        guide["title"]=json!("Second");
        assert_eq!(context.present(&guide).unwrap()["title"],"Second");
        assert_eq!(context.presented.lock().unwrap().len(),2);
    }
}
