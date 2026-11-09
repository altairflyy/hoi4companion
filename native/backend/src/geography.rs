//! Static installed province relations. No live owner/controller inference.
use hoi4companion_catalog_core::normal_name;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
type Set = BTreeSet<u64>;
type Places = BTreeSet<(u64, Option<u64>)>;
fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn arr(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn id(v: &Value) -> Option<u64> {
    v.as_u64()
        .or_else(|| {
            v.as_str()
                .filter(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()))
                .and_then(|s| s.parse().ok())
        })
        .filter(|n| *n > 0)
}
pub struct Geography {
    pub states: BTreeMap<u64, Vec<Value>>,
    names: HashMap<String, Places>,
    pub provinces: BTreeMap<u64, Set>,
    neighbors: BTreeMap<u64, Option<Vec<u64>>>,
}
impl Geography {
    pub fn new(rows: &[Value]) -> Self {
        let mut result = Self {
            states: BTreeMap::new(),
            names: HashMap::new(),
            provinces: BTreeMap::new(),
            neighbors: BTreeMap::new(),
        };
        for row in rows {
            if row["kind"] != "state" {
                continue;
            }
            let Some(state) = id(&row["id"]) else {
                continue;
            };
            result.states.entry(state).or_default().push(row.clone());
            for name in [s(&row["id"]), s(&row["name"]), s(&row["localized_name"])]
                .into_iter()
                .chain(arr(&row["aliases"]).iter().map(s))
            {
                if !name.is_empty() {
                    result
                        .names
                        .entry(normal_name(name))
                        .or_default()
                        .insert((state, None));
                }
            }
            let geography = &row["geography"];
            let provinces = arr(&geography["province_ids"])
                .iter()
                .filter_map(|p| p.as_u64().filter(|p| *p > 0))
                .collect::<Set>();
            for p in &provinces {
                result.provinces.entry(*p).or_default().insert(state);
            }
            for (key, items) in geography["province_neighbors"]
                .as_object()
                .into_iter()
                .flatten()
            {
                let Some(province) = id(&json!(key)) else {
                    continue;
                };
                if !provinces.contains(&province)
                    || !items.as_array().is_some_and(|items| {
                        items.len() <= 512
                            && items.iter().all(|n| n.as_u64().is_some_and(|n| n > 0))
                    })
                {
                    continue;
                }
                let values = arr(items)
                    .iter()
                    .filter_map(Value::as_u64)
                    .collect::<Set>()
                    .into_iter()
                    .collect::<Vec<_>>();
                if result
                    .neighbors
                    .get(&province)
                    .is_some_and(|old| old.as_ref() != Some(&values))
                {
                    result.neighbors.insert(province, None);
                } else {
                    result.neighbors.insert(province, Some(values));
                }
            }
            for city in arr(&geography["cities"]) {
                let Some(p) = city["province_id"]
                    .as_u64()
                    .filter(|p| provinces.contains(p))
                else {
                    continue;
                };
                for name in [s(&city["name"]), s(&city["localized_name"])]
                    .into_iter()
                    .chain(arr(&city["aliases"]).iter().map(s))
                {
                    if !name.is_empty() {
                        result
                            .names
                            .entry(normal_name(name))
                            .or_default()
                            .insert((state, Some(p)));
                    }
                }
            }
        }
        result
    }
    pub fn export(&self) -> Value {
        json!({"states":self.states,"names":self.names,"provinces":self.provinces,"neighbors":self.neighbors})
    }
    pub fn resolve(&self, params: &Value, provincial: bool) -> Value {
        let (mut state, mut province) = (id(&params["state_id"]), id(&params["province_id"]));
        macro_rules! fail{($reason:expr)=>{return json!({"reason":$reason})}}
        if !params["state_id"].is_null() && state.is_none()
            || !params["province_id"].is_null() && province.is_none()
        {
            fail!("location_unresolved");
        }
        if state.is_some_and(|s| !self.states.contains_key(&s)) {
            fail!("location_unresolved");
        }
        if let Some(p) = province {
            let Some(owners) = self.provinces.get(&p).filter(|s| !s.is_empty()) else {
                fail!("location_unresolved");
            };
            if owners.len() != 1 {
                fail!("location_ambiguous");
            }
            if state.is_some_and(|s| !owners.contains(&s)) {
                fail!("location_conflict");
            }
            state = owners.first().copied();
        }
        let mut candidates = params["state_name"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .and_then(|s| self.names.get(&normal_name(s)))
            .cloned()
            .unwrap_or_default();
        if let Some(state) = state {
            if candidates.is_empty() {
                candidates.insert((state, province));
            } else {
                candidates.retain(|(s, p)| {
                    *s == state && (province.is_none() || p.is_none() || *p == province)
                });
                if candidates.is_empty() {
                    fail!("location_conflict");
                }
            }
        } else if candidates.is_empty() {
            fail!("location_unresolved");
        }
        let states = candidates.iter().map(|(s, _)| *s).collect::<Set>();
        if states.len() != 1 {
            fail!("location_ambiguous");
        }
        let state = *states.first().unwrap();
        let regions = &self.states[&state];
        if regions.len() != 1 {
            fail!("location_ambiguous");
        }
        let cities = candidates.iter().filter_map(|(_, p)| *p).collect::<Set>();
        if cities
            .iter()
            .any(|p| self.provinces.get(p).map_or(0, Set::len) != 1)
        {
            fail!("location_ambiguous");
        }
        if province.is_none() && cities.len() == 1 {
            province = cities.first().copied();
        }
        if provincial {
            let Some(p) = province else {
                fail!(if cities.is_empty() {
                    "province_required"
                } else {
                    "location_ambiguous"
                });
            };
            if self.provinces.get(&p).map_or(0, Set::len) != 1 {
                fail!("location_ambiguous");
            }
        }
        let region = &regions[0];
        json!({"state_id":state,"state_name":region.get("localized_name").unwrap_or(&region["name"]),"province_id":province,"via":if !cities.is_empty(){"city"}else if province.is_some(){"province"}else{"state"}})
    }
}
pub fn execute(op: &str, args: &Value) -> Result<Value, String> {
    use std::sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    };
    static ITEMS: OnceLock<Mutex<HashMap<u64, Arc<Geography>>>> = OnceLock::new();
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let registry = ITEMS.get_or_init(Default::default);
    if op == "geography_open" {
        let geography = if let Some(handle) = args["identity_handle"].as_u64() {
            Geography::new(hoi4companion_catalog_core::borrow_catalog(handle)?.entries())
        } else {
            Geography::new(
                args["entries"]
                    .as_array()
                    .ok_or("Missing geography catalog")?,
            )
        };
        let mut entries = registry
            .lock()
            .map_err(|_| "Geography registry unavailable")?;
        if entries.len() >= 128 {
            return Err("Geography handle limit exceeded".into());
        }
        let handle = NEXT.fetch_add(1, Ordering::Relaxed);
        entries.insert(handle, Arc::new(geography));
        return Ok(json!({"handle":handle}));
    }
    let handle = args["handle"].as_u64().ok_or("Missing geography handle")?;
    if op == "geography_release" {
        return Ok(json!(
            registry
                .lock()
                .map_err(|_| "Geography registry unavailable")?
                .remove(&handle)
                .is_some()
        ));
    }
    let owner = registry
        .lock()
        .map_err(|_| "Geography registry unavailable")?
        .get(&handle)
        .cloned()
        .ok_or("Unknown geography handle")?;
    match op {
        "geography_export" => Ok(owner.export()),
        "geography_resolve" => Ok(owner.resolve(
            &args["params"],
            args["provincial"].as_bool().unwrap_or(false),
        )),
        _ => Err("Unknown geography operation".into()),
    }
}
