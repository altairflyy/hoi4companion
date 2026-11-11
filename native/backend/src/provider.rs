//! Process-owned acquisition cache. Failed probes discard all pointer caches;
//! only a new complete qualified read can restore a usable sample.
use crate::{acquisition, game_process::Game, profile::Profile, watch};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicU64, Ordering},
    },
    time::{Instant, SystemTime},
};
type Result<T> = std::result::Result<T, String>;
struct Cached {
    sample: acquisition::Sample,
    packet: watch::Packet,
    audited: f64,
}
struct Provider {
    executable: PathBuf,
    manifest: String,
    audit: f64,
    neighbors: Value,
    shared_focuses: Value,
    building_scopes: Value,
    political_cost_ids: Value,
    stat: Option<(u64, SystemTime)>,
    profile: Option<Arc<Profile>>,
    game: Option<Arc<Game>>,
    cached: Option<Cached>,
    metrics: Value,
    gui: Arc<Mutex<crate::gui_registry::Registry>>,
}
impl Provider {
    fn set_political_cost_ids(&mut self,targets:&Value)->Result<()> {
        if !targets.is_array() {return Err("Political cost targets must be an array".into());}
        if self.political_cost_ids!=*targets {
            self.political_cost_ids=targets.clone();
            self.cached=None;
        }
        Ok(())
    }
    fn new(args: &Value) -> Result<Self> {
        let audit = args["audit_seconds"].as_f64().unwrap_or(5.);
        if !audit.is_finite() || audit < 0. {
            return Err("Invalid integrity audit interval".into());
        }
        Ok(Self {
            executable: PathBuf::from(args["path"].as_str().ok_or("Executable path missing")?),
            manifest: args["manifest_sha256"]
                .as_str()
                .ok_or("Profile manifest missing")?
                .into(),
            audit,
            neighbors: args["province_neighbors"].clone(),
            shared_focuses: args["shared_focuses"].clone(),
            building_scopes: args["building_scopes"].clone(),
            political_cost_ids: args["political_cost_ids"].clone(),
            stat: None,
            profile: None,
            game: None,
            cached: None,
            metrics: json!({"probes":0,"full_reads":0,"changes":0,"last_read_ms":null,"watch_ranges":0,"watch_bytes":0,"probe_read_calls":0,"probe_read_bytes":0}),
            gui: Arc::new(Mutex::new(crate::gui_registry::Registry::default())),
        })
    }
    fn close(&mut self) {
        self.gui = Arc::new(Mutex::new(crate::gui_registry::Registry::default()));
        self.game = None;
        self.cached = None;
    }
    fn increment(&mut self, key: &str) {
        let value = self.metrics[key].as_u64().unwrap_or(0);
        self.metrics[key] = json!(value + 1);
    }
    fn collect_inner(&mut self, now: f64) -> Result<Value> {
        if !now.is_finite() || now < 0. {
            return Err("Invalid provider clock".into());
        }
        let started = Instant::now();
        let stat = std::fs::metadata(&self.executable).map_err(|e| e.to_string())?;
        let key = (stat.len(), stat.modified().map_err(|e| e.to_string())?);
        if self.profile.is_none() || self.stat != Some(key) {
            self.close();
            self.profile = Some(Arc::new(Profile::open(&self.executable, &self.manifest)?));
            self.stat = Some(key);
        }
        if self.game.is_none() {
            self.game = Some(Arc::new(Game::open(self.profile.as_ref().unwrap().clone())?))
        }
        let game = self.game.as_ref().unwrap();
        if game.identity()? != game.created {
            return Err("Sessione di processo cambiata".into());
        }
        let mut changed = false;
        let mut full = self
            .cached
            .as_ref()
            .is_none_or(|v| now - v.audited >= self.audit);
        if !full {
            self.increment("probes");
            changed = acquisition::compare(
                self.game.as_ref().unwrap(),
                &self.cached.as_ref().unwrap().packet,
            )?;
            full = changed;
        }
        if full {
            let game = self.game.as_ref().unwrap();
            let cache = self
                .cached
                .as_ref()
                .map(|v| v.sample.event_cache.clone())
                .unwrap_or(Value::Null);
            let sample = acquisition::collect(game, &self.neighbors, &cache, &self.shared_focuses, &self.building_scopes, &self.political_cost_ids)?;
            let packet = watch::compile(&sample.watch)?;
            self.metrics["watch_ranges"] = json!(sample.watch.len());
            self.metrics["watch_bytes"] =
                json!(sample.watch.keys().map(|(_, n)| *n).sum::<usize>());
            self.metrics["probe_read_calls"] = json!(packet.ranges * 2);
            self.metrics["probe_read_bytes"] = json!(packet.read_bytes * 2);
            self.cached = Some(Cached {
                sample,
                packet,
                audited: now + started.elapsed().as_secs_f64(),
            });
            self.increment("full_reads");
            if changed {
                self.increment("changes")
            }
        }
        let game = self.game.as_ref().unwrap();
        let cached = self.cached.as_ref().unwrap();
        let tick = i32::from_le_bytes(
            game.read(cached.sample.world + 0x468, 4)?
                .try_into()
                .unwrap(),
        );
        if game.identity()? != game.created {
            return Err("Sessione di processo cambiata".into());
        }
        let mut sample = cached.sample.value.clone();
        sample["native_tick"] = json!(tick);
        self.metrics["last_read_ms"] =
            json!((started.elapsed().as_secs_f64() * 1_000_000.).round_ties_even() / 1000.);
        Ok(sample)
    }
    fn collect(&mut self, now: f64) -> Result<Value> {
        match self.collect_inner(now) {
            Ok(value) => Ok(value),
            Err(error) => {
                self.close();
                Err(error)
            }
        }
    }
    fn geometry(&mut self, args: &Value) -> Result<Value> {
        self.gui_lane()?.geometry(args)
    }
    fn gui_lane(&self) -> Result<GuiLane> {
        Ok(GuiLane {
            game: Arc::downgrade(self.game.as_ref().ok_or("No qualified live game session")?),
            gui: self.gui.clone(),
        })
    }
}
/// A separate observation lane over the SAME qualified handle/profile. It
/// neither owns a second process handle nor waits for full-state acquisition.
/// Closing/replacing the provider invalidates the weak session immediately.
#[derive(Clone)]
pub(crate) struct GuiLane {
    game: Weak<Game>,
    gui: Arc<Mutex<crate::gui_registry::Registry>>,
}
impl GuiLane {
    pub(crate) fn matches_pid(&self, pid: Option<u64>) -> bool {
        self.game.upgrade().is_some_and(|g| Some(g.pid as u64) == pid)
    }
    pub(crate) fn geometry(&self, args: &Value) -> Result<Value> {
        let game = self.game.upgrade().ok_or("No qualified live game session")?;
        if game.identity()? != game.created {
            return Err("Sessione di processo cambiata".into());
        }
        if args["view"][1].as_u64() != Some(game.pid as u64) {
            return Err("Processo GUI non corrispondente".into());
        }
        let mut profile = crate::live_process::profile_value(&game.profile.metadata()["selected"]);
        profile["image_base"] = game.profile.metadata()["image_base"].clone();
        profile["image_size"] = game.profile.metadata()["image_size"].clone();
        profile["sha256"] = game.profile.metadata()["sha256"].clone();
        if ([
            "b6650c85002374218e60598bd04aaa26cbad996fedce77126fcf92fc6bd6d888",
            "cf781ed178cd801643b233b7e3fcd1c0efd19bd01aecd24bf07a35f69f10476f",
        ]
        .contains(&profile["sha256"].as_str().unwrap_or("")) || profile["layout_id"] == "hoi4-x64-abi-v1")
            && !profile["gui_layout"].is_object()
        {
            return Err("Geometria GUI non verificata per questa versione di HOI4".into());
        }
        let proposals = &args["proposals"];
        let ids = |panel: &str| {
            crate::hud_geometry::rows(proposals)
                .iter()
                .filter(|p| p["panel"] == panel)
                .map(|p| p["identity"].clone())
                .collect::<Vec<_>>()
        };
        let request = json!({"profile":profile,"base":game.base,"pid":game.pid,"created":game.created,
            "focus_ids":ids("focus"),"technology_ids":ids("research"),"event_ids":crate::hud_geometry::rows(proposals).iter().filter(|p|p["panel"]=="event").map(|p|p["event_id"].clone()).collect::<Vec<_>>(),
            "action_ui":crate::hud_geometry::rows(proposals).iter().any(|p|p["panel"]=="politics" || p["panel"]=="projects"),"production_ui":crate::hud_geometry::rows(proposals).iter().any(|p|p["panel"]=="production")});
        let mut request = request;
        request["event_instances"] = json!(
            crate::hud_geometry::rows(proposals)
                .iter()
                .filter(|p| p["panel"] == "event")
                .flat_map(|p| crate::hud_geometry::rows(&p["instances"]))
                .cloned()
                .collect::<Vec<_>>()
        );
        let mut memory = crate::live_process::Memory::new(&game, 4 * 1024 * 1024)?;
        let raw = self.gui.lock().map_err(|_|"GUI observation lane unavailable")?.observe(&mut memory, &request)?;
        let returned = Instant::now();
        if !same_session(args.get("session_id"),&raw["session_id"]) {
            return Err("Sessione GUI diversa dallo stato della guida".into());
        }
        if game.identity()? != game.created {
            return Err("Sessione di processo cambiata".into());
        }
        let mut result = crate::hud_geometry::admit(&raw, proposals, &args["view"])?;
        result["sample_age_ms"] = json!(raw["sample_age_ms"].as_f64().ok_or("Missing native GUI witness age")?
            + returned.elapsed().as_secs_f64()*1000.);
        Ok(result)
    }
}
fn same_session(expected:Option<&Value>,observed:&Value)->bool {
    expected.is_none_or(|expected|expected.as_str().is_some_and(|s|!s.is_empty()) && expected==observed)
}
static REGISTRY: OnceLock<Mutex<HashMap<u64, Arc<Mutex<Provider>>>>> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
fn registry() -> &'static Mutex<HashMap<u64, Arc<Mutex<Provider>>>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}
pub(crate) fn gui_lane(handle: u64) -> Result<GuiLane> {
    let provider=registry().lock().map_err(|_|"Provider registry unavailable")?
        .get(&handle).cloned().ok_or("Unknown provider handle")?;
    let lane=provider.lock().map_err(|_|"Provider unavailable")?.gui_lane()?;
    Ok(lane)
}
pub fn execute(op: &str, args: &Value) -> Result<Value> {
    if op == "provider_open" {
        let provider = Provider::new(args)?;
        let mut entries = registry()
            .lock()
            .map_err(|_| "Provider registry unavailable")?;
        if entries.len() >= 64 {
            return Err("Provider registry limit exceeded".into());
        }
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        entries.insert(id, Arc::new(Mutex::new(provider)));
        return Ok(json!(id));
    }
    let id = args["handle"].as_u64().ok_or("Provider handle missing")?;
    if op == "provider_release" {
        return Ok(json!(
            registry()
                .lock()
                .map_err(|_| "Provider registry unavailable")?
                .remove(&id)
                .is_some()
        ));
    }
    let provider = registry()
        .lock()
        .map_err(|_| "Provider registry unavailable")?
        .get(&id)
        .cloned()
        .ok_or("Unknown provider handle")?;
    let mut provider = provider.lock().map_err(|_| "Provider unavailable")?;
    match op {
        "provider_collect" => {
            if let Some(targets)=args.get("political_cost_ids") {provider.set_political_cost_ids(targets)?;}
            provider.collect(args["now"].as_f64().ok_or("Provider clock missing")?)
        }
        "provider_inspect" => Ok(
            json!({"metrics":provider.metrics,"cached":provider.cached.is_some(),"connected":provider.game.is_some()}),
        ),
        "provider_geometry" => provider.geometry(args),
        "provider_close" => {
            provider.close();
            Ok(Value::Null)
        }
        _ => Err("Unknown provider operation".into()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn guide_purchase_target_updates_invalidate_only_changed_acquisition_proofs() {
        let old=json!([{"id":"old","group":"decision","countries":["NOR"]}]);
        let mut p=Provider::new(&json!({"path":"unused","manifest_sha256":"unused","political_cost_ids":old})).unwrap();
        p.cached=Some(Cached{sample:acquisition::Sample{value:Value::Null,watch:Default::default(),world:0,event_cache:Value::Null},packet:watch::Packet{bytes:vec![],ranges:0,read_bytes:0},audited:1.});
        p.set_political_cost_ids(&old).unwrap();assert!(p.cached.is_some());
        assert!(p.set_political_cost_ids(&Value::Null).is_err());assert!(p.cached.is_some());
        let next=json!([{"id":"new","group":"decision","countries":["GER"]}]);
        p.set_political_cost_ids(&next).unwrap();assert!(p.cached.is_none());assert_eq!(p.political_cost_ids,next);
    }
    #[test]
    fn concurrent_gui_rejects_an_old_world_or_country_even_in_the_same_process() {
        let current=json!("123:456:aaa:bbb");
        assert!(same_session(Some(&current),&current));
        for old in [json!("124:456:aaa:bbb"),json!("123:457:aaa:bbb"),json!("123:456:ccc:bbb"),json!("123:456:aaa:ddd"),Value::Null,json!("")] {
            assert!(!same_session(Some(&old),&current));
        }
        assert!(same_session(None,&current),"Legacy direct geometry interface has no coarse-state context");
    }
    #[test]
    fn warm_gui_lane_uses_the_same_handle_without_waiting_for_collector_lock() {
        let mut provider=Provider::new(&json!({"path":"unused","manifest_sha256":"unused"})).unwrap();
        let game=Game::owned_fixture();let pid=game.pid;
        provider.game=Some(game.clone());
        let lane=provider.gui_lane().unwrap();
        assert!(Arc::ptr_eq(&game,&lane.game.upgrade().unwrap()));
        drop(game);
        let collector=Mutex::new(provider);
        let guard=collector.lock().unwrap();
        let (send,receive)=std::sync::mpsc::channel();let worker_lane=lane.clone();
        let worker=std::thread::spawn(move || {
            send.send(worker_lane.geometry(&json!({"view":[1,0,0,0,1280,720]}))).unwrap();
        });
        let result=receive.recv_timeout(std::time::Duration::from_secs(1));
        drop(guard);worker.join().unwrap();
        assert_eq!(result.unwrap().unwrap_err(),"Processo GUI non corrispondente");
        assert!(lane.matches_pid(Some(pid as u64)));
        collector.lock().unwrap().close();
        assert!(!lane.matches_pid(Some(pid as u64)));
        assert!(lane.geometry(&json!({})).is_err());
    }
    #[test]
    fn missing_executable_never_keeps_a_cached_or_usable_session() {
        let mut provider=Provider::new(&json!({"path":"Z:\\missing-native-provider-test\\hoi4.exe","manifest_sha256":"invalid"})).unwrap();
        assert!(provider.collect(1.).is_err());
        assert!(provider.cached.is_none());
        assert!(provider.game.is_none());
        assert_eq!(provider.metrics["full_reads"], 0);
        assert!(provider.collect(f64::NAN).is_err());
        provider.close();
        assert!(provider.cached.is_none());
    }
}
