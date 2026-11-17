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
    /// Scenario research at the start (country history), by country and date.
    research_start: Mutex<HashMap<String, Value>>,
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
            research_start: Mutex::new(HashMap::new()),
        }))
    }
    /// Technologies a guide's nation has when its campaign starts, from the
    /// installed country history (`{researched, unknown}`); null without game
    /// files, a single country history file or a valid nation. Static data only.
    pub fn research_start(&self, guide: &Value) -> Value {
        let country = guide["applies_to"]["country"].as_str().or(guide["country"].as_str()).unwrap_or("");
        let start = &guide["start_date"];
        if self.source["game_root"].is_null() || !(start.is_null() || start.is_string()) {
            return Value::Null;
        }
        let key = json!([country, start]).to_string();
        if let Some(found) = self.research_start.lock().ok().and_then(|c| c.get(&key).cloned()) {
            return found;
        }
        let value = crate::research_start::read(&json!({"game":self.source["game_root"],"mods":self.source["mod_roots"],
            "country":country,"start_date":start,"dlcs":self.source["dlcs"],"scripts":self.source["scripts"]})).unwrap_or(Value::Null);
        if let Ok(mut cache) = self.research_start.lock() {
            if cache.len() >= 64 {
                cache.clear();
            }
            cache.insert(key, value.clone());
        }
        value
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
        name_production_targets(&mut value, self.index.rows());
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
        let mut bound = crate::construction_instructions::bind(&result["guide"], &self.index)?;
        crate::informal_targets::bind_production_children(&mut bound, &self.index, &self.library)?;
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
/// The game's own name for each production target, so the walkthrough says
/// "Equipaggiamento di supporto" where the author wrote "Sup Equip": a design
/// or national model name first ("I-16"), else the one catalog equipment (or
/// family) the selector names, localized. The author's text is unchanged.
fn name_production_targets(guide: &mut Value, catalog: &[Value]) {
    for step in guide["tracks"].as_object_mut().into_iter().flat_map(|t| t.values_mut()).flat_map(|v| v.as_array_mut().into_iter().flatten()) {
        // Never create a missing params object (indexing would insert null).
        let Some(params) = step.get_mut("action").and_then(|a| a.get_mut("params")) else { continue };
        if params["category"] != "production" {
            continue;
        }
        let targets: Vec<&mut Value> = match params["production_targets"].as_array_mut() {
            Some(list) => list.iter_mut().collect(),
            None => vec![params],
        };
        for target in targets {
            let selector = &target["production_selector"];
            let ids: Vec<&str> = selector["equipment_ids"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
            if ids.is_empty() {
                continue;
            }
            let variant = selector["variant_names"].as_array().and_then(|v| v.first()).and_then(Value::as_str).filter(|v| !v.contains('_'));
            let entry = catalog.iter().filter(|e| e["kind"] == "equipment").find(|e| {
                let own: Vec<&str> = e["equipment_ids"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                if ids.len() == 1 { e["id"] == ids[0] } else { crate::state::truth(&e["is_archetype"]) && own.len() == ids.len() && ids.iter().all(|id| own.contains(id)) }
            });
            let name = variant.map(str::to_owned).or_else(|| entry.and_then(|e| {
                [&e["localized_name"], &e["name"]].into_iter().find_map(|n| n.as_str().filter(|n| !n.trim().is_empty()).map(str::to_owned))
            }));
            if let Some(name) = name {
                target["display_name"] = json!(name);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn production_targets_carry_the_game_name() {
        let catalog = vec![
            json!({"kind":"equipment","id":"support_equipment_1","name":"Support Equipment","localized_name":"Equipaggiamento di supporto","equipment_ids":["support_equipment_1"]}),
            json!({"kind":"equipment","id":"ship_hull_heavy","name":"Heavy Ship","localized_name":"Nave pesante","is_archetype":true,"equipment_ids":["ship_hull_heavy_1","ship_hull_heavy_2"]})];
        let mut guide = json!({"tracks":{"operations":[{"key":"p","action":{"kind":"manual","params":{"category":"production","production_targets":[
            {"equipment_name":"Sup Equip","production_selector":{"equipment_ids":["support_equipment_1"],"variant_names":[]}},
            {"equipment_name":"battleship","production_selector":{"equipment_ids":["ship_hull_heavy_2","ship_hull_heavy_1"],"variant_names":[]}},
            {"equipment_name":"I-16","production_selector":{"equipment_ids":["fighter_equipment_1","small_plane_airframe_1"],"variant_names":["I-16","fighter_equipment_1"]}},
            {"equipment_name":"unknown","production_selector":{"equipment_ids":["nothing"],"variant_names":[]}}]}}}]}});
        let mut focus = json!({"tracks":{"focus":[{"key":"f","action":{"kind":"select_focus","entity":{"kind":"focus","id":"F"}}}]}});
        name_production_targets(&mut focus, &catalog);
        assert!(focus["tracks"]["focus"][0]["action"].get("params").is_none(), "a step without params keeps none");
        name_production_targets(&mut guide, &catalog);
        let targets = &guide["tracks"]["operations"][0]["action"]["params"]["production_targets"];
        assert_eq!(targets[0]["display_name"], "Equipaggiamento di supporto");
        assert_eq!(targets[1]["display_name"], "Nave pesante");
        assert_eq!(targets[2]["display_name"], "I-16");
        assert!(targets[3].get("display_name").is_none());
        assert_eq!(targets[0]["equipment_name"], "Sup Equip", "the author's text stays");
    }
    #[test]
    fn presentation_cache_keys_full_definition_and_returns_independent_values() {
        let source=Arc::new(json!({"operation_entities":[]}));
        let context=Context {
            index:Index::new(&json!({"entries":[]})).unwrap(),
            presentation:guide_enrich::Presentation::new(source.clone(),crate::import_catalog::language()),
            source,library:json!({"documents":[]}),compiled:Mutex::new(HashMap::new()),
            presented:Mutex::new(HashMap::new()),prepared_cache:None,research_start:Mutex::new(HashMap::new()),
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
