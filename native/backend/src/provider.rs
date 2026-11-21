//! Process-owned acquisition cache. Failed probes discard all pointer caches;
//! only a new complete qualified read can restore a usable sample. The
//! periodic integrity audit (a full read, ~1-1.5 s) runs on its own thread
//! while probes keep answering from the cached read, so an audit never stops
//! the delivery loop; a probe that sees a change still reads in full at once.
use crate::{acquisition, game_process::Game, profile::Profile, watch};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::{Instant, SystemTime},
};
type Result<T> = std::result::Result<T, String>;
/// A failed collect with the stage it failed in (`Provider::collect` closes
/// the session on any of them; the stage says why, HUD plan part 2 step 2).
type Staged<T> = std::result::Result<T, (&'static str, String)>;
trait At<T> {
    fn at(self, stage: &'static str) -> Staged<T>;
}
impl<T, E: Into<String>> At<T> for std::result::Result<T, E> {
    fn at(self, stage: &'static str) -> Staged<T> {
        self.map_err(|error| (stage, error.into()))
    }
}
/// Close events kept in `metrics.closes`, newest last.
const CLOSES_KEPT: usize = 16;
struct Cached {
    sample: acquisition::Sample,
    packet: watch::Packet,
    audited: f64,
}
type AuditResult = Result<(acquisition::Sample, watch::Packet, f64)>;
/// An integrity audit in flight: started at provider time `started` over the
/// full read of `generation`; its result replaces that read only.
struct Audit {
    generation: u64,
    started: f64,
    result: mpsc::Receiver<AuditResult>,
}
struct Provider {
    executable: PathBuf,
    manifest: String,
    audit: f64,
    neighbors: Value,
    shared_focuses: Value,
    building_scopes: Value,
    political_cost_ids: Value,
    /// Provinces whose buildings are read; null reads every province.
    province_ids: Value,
    stat: Option<(u64, SystemTime)>,
    profile: Option<Arc<Profile>>,
    game: Option<Arc<Game>>,
    cached: Option<Cached>,
    /// Bumped by every full read installed and by `close`.
    generation: u64,
    audit_job: Option<Audit>,
    /// False once the session is closed, even while an audit thread still
    /// holds the game: the GUI lane stops at once.
    open: Arc<AtomicBool>,
    metrics: Value,
    gui: Arc<Mutex<crate::gui_registry::Registry>>,
    /// The GUI lane of the open session, shared with the registry entry so
    /// `gui_lane(handle)` never waits on this provider's lock: a full read
    /// after a reopen holds it ~1-1.5 s (geometry spikes 878-1516 ms, HUD
    /// plan part 2 step 3b). Set when a session opens, cleared on `close`.
    lane: Arc<Mutex<Option<GuiLane>>>,
}
impl Provider {
    fn set_political_cost_ids(&mut self,targets:&Value)->Result<()> {
        if !targets.is_array() {return Err("Political cost targets must be an array".into());}
        if self.political_cost_ids!=*targets {
            self.political_cost_ids=targets.clone();
            self.cached=None;
            self.audit_job=None;
        }
        Ok(())
    }
    fn set_province_ids(&mut self,ids:&Value)->Result<()> {
        if !ids.as_array().is_some_and(|a|a.len()<=20000&&a.iter().all(|v|v.as_u64().is_some_and(|n|n>0&&n<=100000))) {
            return Err("Province filter must be an array of province ids".into());
        }
        if self.province_ids!=*ids {
            self.province_ids=ids.clone();
            self.cached=None;
            self.audit_job=None;
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
            province_ids: Value::Null,
            stat: None,
            profile: None,
            game: None,
            cached: None,
            generation: 0,
            audit_job: None,
            open: Arc::new(AtomicBool::new(false)),
            metrics: json!({"probes":0,"full_reads":0,"audits":0,"changes":0,"last_read_ms":null,"watch_ranges":0,"watch_bytes":0,"probe_read_calls":0,"probe_read_bytes":0,"close_count":0,"closes":[]}),
            gui: Arc::new(Mutex::new(crate::gui_registry::Registry::deferred())),
            lane: Arc::new(Mutex::new(None)),
        })
    }
    fn close(&mut self) {
        self.open.store(false, Ordering::Release);
        self.gui = Arc::new(Mutex::new(crate::gui_registry::Registry::deferred()));
        self.game = None;
        self.cached = None;
        self.audit_job = None;
        self.generation += 1;
        self.publish_lane();
    }
    /// Publishes the current session's GUI lane (or none) to the shared slot.
    fn publish_lane(&self) {
        let lane = self.gui_lane().ok();
        match self.lane.lock() {
            Ok(mut slot) => *slot = lane,
            Err(poisoned) => *poisoned.into_inner() = lane,
        }
    }
    /// Takes a finished audit: its read replaces the cached one (a probe then
    /// compares against it). A failed audit fails the collect like any read.
    fn finish_audit(&mut self) -> Result<()> {
        let Some(job) = self.audit_job.as_ref() else { return Ok(()) };
        let result = match job.result.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return Ok(()),
            Err(mpsc::TryRecvError::Disconnected) => Err("Integrity audit stopped".into()),
        };
        let job = self.audit_job.take().unwrap();
        let (sample, packet, seconds) = result?;
        if job.generation != self.generation || self.cached.is_none() {
            return Ok(());
        }
        self.install(sample, packet, job.started + seconds);
        self.increment("audits");
        Ok(())
    }
    fn install(&mut self, sample: acquisition::Sample, packet: watch::Packet, audited: f64) {
        self.metrics["watch_ranges"] = json!(sample.watch.len());
        self.metrics["watch_bytes"] = json!(sample.watch.keys().map(|(_, n)| *n).sum::<usize>());
        self.metrics["probe_read_calls"] = json!(packet.ranges * 2);
        self.metrics["probe_read_bytes"] = json!(packet.read_bytes * 2);
        self.cached = Some(Cached { sample, packet, audited });
        self.generation += 1;
        self.increment("full_reads");
    }
    /// Starts the integrity audit of the cached read on its own thread.
    fn start_audit(&mut self, now: f64) -> Result<()> {
        let game = self.game.clone().ok_or("No qualified live game session")?;
        let cache = self.cached.as_ref().map(|v| v.sample.event_cache.clone()).unwrap_or(Value::Null);
        let inputs = (self.neighbors.clone(), self.shared_focuses.clone(), self.building_scopes.clone(),
            self.political_cost_ids.clone(), self.province_ids.clone());
        let (send, result) = mpsc::channel();
        std::thread::Builder::new()
            .name("hoi4-integrity-audit".into())
            .spawn(move || {
                let started = Instant::now();
                let (neighbors, shared, scopes, costs, provinces) = &inputs;
                let read = acquisition::collect(&game, neighbors, &cache, shared, scopes, costs, provinces)
                    .and_then(|sample| {
                        let packet = watch::compile(&sample.watch)?;
                        Ok((sample, packet, started.elapsed().as_secs_f64()))
                    });
                drop(game);
                let _ = send.send(read);
            })
            .map_err(|e| e.to_string())?;
        self.audit_job = Some(Audit { generation: self.generation, started: now, result });
        Ok(())
    }
    /// Keeps why a collect failed and closed the session. A failure that
    /// repeats the last one (same stage and error, e.g. HOI4 not running) only
    /// updates its count and time, so the bounded list keeps the real causes.
    /// `seq` numbers the events, so a reader sees each new one once.
    fn record_close(&mut self, stage: &str, error: &str, now: f64) {
        let unix_ms = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0., |d| d.as_secs_f64() * 1000.).round();
        let had_session = self.game.is_some();
        let seq = self.metrics["close_count"].as_u64().unwrap_or(0) + 1;
        let closes = self.metrics["closes"].as_array_mut().expect("closes is a list");
        if let Some(last) = closes.last_mut()
            && last["stage"] == stage && last["error"] == error
        {
            last["repeats"] = json!(last["repeats"].as_u64().unwrap_or(1) + 1);
            last["last_unix_ms"] = json!(unix_ms);
            return;
        }
        closes.push(json!({"seq":seq,"stage":stage,"error":error,"at":now,"unix_ms":unix_ms,"had_session":had_session,"repeats":1}));
        if closes.len() > CLOSES_KEPT {
            closes.remove(0);
        }
        self.metrics["close_count"] = json!(seq);
    }
    fn increment(&mut self, key: &str) {
        let value = self.metrics[key].as_u64().unwrap_or(0);
        self.metrics[key] = json!(value + 1);
    }
    fn collect_inner(&mut self, now: f64) -> Staged<Value> {
        if !now.is_finite() || now < 0. {
            return Err(("clock", "Invalid provider clock".into()));
        }
        let started = Instant::now();
        let stat = std::fs::metadata(&self.executable).map_err(|e| e.to_string()).at("executable")?;
        let key = (stat.len(), stat.modified().map_err(|e| e.to_string()).at("executable")?);
        if self.profile.is_none() || self.stat != Some(key) {
            self.close();
            self.profile = Some(Arc::new(Profile::open(&self.executable, &self.manifest).at("profile")?));
            self.stat = Some(key);
        }
        if self.game.is_none() {
            self.game = Some(Arc::new(Game::open(self.profile.as_ref().unwrap().clone()).at("open")?));
            self.open = Arc::new(AtomicBool::new(true));
            self.publish_lane();
        }
        let game = self.game.as_ref().unwrap();
        if game.identity().at("identity")? != game.created {
            return Err(("identity", "Sessione di processo cambiata".into()));
        }
        let mut changed = false;
        self.finish_audit().at("audit")?;
        let mut full = self.cached.is_none();
        if !full {
            self.increment("probes");
            changed = acquisition::compare(
                self.game.as_ref().unwrap(),
                &self.cached.as_ref().unwrap().packet,
            ).at("probe")?;
            full = changed;
        }
        if full {
            let game = self.game.as_ref().unwrap();
            let cache = self
                .cached
                .as_ref()
                .map(|v| v.sample.event_cache.clone())
                .unwrap_or(Value::Null);
            let sample = acquisition::collect(game, &self.neighbors, &cache, &self.shared_focuses, &self.building_scopes, &self.political_cost_ids, &self.province_ids).at("full_read")?;
            let packet = watch::compile(&sample.watch).at("full_read")?;
            // An audit begun before this read would only bring older bytes.
            self.audit_job = None;
            self.install(sample, packet, now + started.elapsed().as_secs_f64());
            if changed {
                self.increment("changes")
            }
        } else if self.audit_job.is_none()
            && self.cached.as_ref().is_some_and(|v| now - v.audited >= self.audit)
        {
            self.start_audit(now).at("audit_start")?;
        }
        let game = self.game.as_ref().unwrap();
        let cached = self.cached.as_ref().unwrap();
        let tick = i32::from_le_bytes(
            game.read(cached.sample.world + 0x468, 4).at("tick")?
                .try_into()
                .unwrap(),
        );
        if game.identity().at("identity")? != game.created {
            return Err(("identity", "Sessione di processo cambiata".into()));
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
            Err((stage, error)) => {
                self.record_close(stage, &error, now);
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
            open: self.open.clone(),
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
    open: Arc<AtomicBool>,
}
impl GuiLane {
    fn session(&self) -> Option<Arc<Game>> {
        self.game.upgrade().filter(|_| self.open.load(Ordering::Acquire))
    }
    pub(crate) fn matches_pid(&self, pid: Option<u64>) -> bool {
        self.session().is_some_and(|g| Some(g.pid as u64) == pid)
    }
    /// Runs the registry discovery the last request queued, if any, on its
    /// own thread: a scan takes 634-918 ms (HUD trace 2026-11-21) and the
    /// requests answer `DISCOVERY_PENDING` (every panel unknown) meanwhile.
    /// The scan is bound to `game`, the session of the request that queued
    /// it: another process is never read at that session's base and root.
    fn start_discovery(&self, gui: &mut crate::gui_registry::Registry, game: &Game) {
        let Some(target) = gui.take_discovery() else { return };
        let lane = self.clone();
        let owner = (game.pid, game.created);
        let spawned = std::thread::Builder::new()
            .name("hoi4-gui-discovery".into())
            .spawn(move || {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| lane.discover(&target, owner)))
                    .unwrap_or_else(|_| Err("GUI registry discovery failed".into()));
                if let Ok(mut gui) = lane.gui.lock() {
                    gui.install(outcome);
                }
            });
        if let Err(error) = spawned {
            gui.install(Err(format!("GUI registry discovery not started: {error}")));
        }
    }
    fn discover(&self, target: &crate::gui_registry::DiscoveryTarget, owner: (u32, u64)) -> Result<crate::gui_registry::Discovered> {
        let game = self.session().ok_or("No qualified live game session")?;
        if (game.pid, game.created) != owner || game.identity()? != game.created {
            return Err("Sessione di processo cambiata".into());
        }
        let mut memory = crate::live_process::Memory::new(&game, 4 * 1024 * 1024)?;
        let found = crate::gui_registry::discover_target(&mut memory, target)?;
        if game.identity()? != game.created {
            return Err("Sessione di processo cambiata".into());
        }
        Ok(found)
    }
    pub(crate) fn geometry(&self, args: &Value) -> Result<Value> {
        let game = self.session().ok_or("No qualified live game session")?;
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
        let raw = {
            let mut gui = self.gui.lock().map_err(|_|"GUI observation lane unavailable")?;
            let observed = gui.observe(&mut memory, &request);
            self.start_discovery(&mut gui, &game);
            observed?
        };
        let returned = Instant::now();
        if !same_session(args.get("session_id"),&raw["session_id"]) {
            return Err("Sessione GUI diversa dallo stato della guida".into());
        }
        if game.identity()? != game.created {
            return Err("Sessione di processo cambiata".into());
        }
        let mut result = crate::hud_geometry::admit(&raw, proposals, &args["view"])?;
        if let Some(panels)=result["handled_panels"].as_array_mut() {panels.push(json!("construction_map"));}
        if raw["available"]==true && raw["resolution"]==json!([args["view"][4],args["view"][5]])
            && raw["active_panel"].as_str().is_none_or(str::is_empty)
            && crate::hud_geometry::rows(proposals).iter().any(|p|p["panel"]=="construction_map") {
            match crate::map_markers::geometry(&mut memory,game.base,&profile,proposals,&args["view"]) {
                Ok(map)=>{if let (Some(rows),Some(points))=(result["matches"].as_array_mut(),map["matches"].as_array()){rows.extend(points.iter().cloned());}result["diagnostics"]["map_bytes"]=map["bytes"].clone();},
                Err(reason)=>result["diagnostics"]["map_unavailable"]=json!(reason),
            }
        }
        result["sample_age_ms"] = json!(raw["sample_age_ms"].as_f64().ok_or("Missing native GUI witness age")?
            + returned.elapsed().as_secs_f64()*1000.);
        Ok(result)
    }
}
fn same_session(expected:Option<&Value>,observed:&Value)->bool {
    expected.is_none_or(|expected|expected.as_str().is_some_and(|s|!s.is_empty()) && expected==observed)
}
/// A provider and its published GUI lane (read without the provider's lock).
type Entry = (Arc<Mutex<Provider>>, Arc<Mutex<Option<GuiLane>>>);
static REGISTRY: OnceLock<Mutex<HashMap<u64, Entry>>> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
fn registry() -> &'static Mutex<HashMap<u64, Entry>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}
/// The GUI lane of the provider's open session. Never waits on the provider
/// lock, so geometry answers while a collect (a full read) is running.
pub(crate) fn gui_lane(handle: u64) -> Result<GuiLane> {
    let slot=registry().lock().map_err(|_|"Provider registry unavailable")?
        .get(&handle).map(|(_, lane)| lane.clone()).ok_or("Unknown provider handle")?;
    let lane=slot.lock().map_err(|_|"Provider unavailable")?.clone();
    lane.ok_or_else(|| "No qualified live game session".into())
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
        let lane = provider.lane.clone();
        entries.insert(id, (Arc::new(Mutex::new(provider)), lane));
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
        .map(|(provider, _)| provider.clone())
        .ok_or("Unknown provider handle")?;
    let mut provider = provider.lock().map_err(|_| "Provider unavailable")?;
    match op {
        "provider_collect" => {
            if let Some(targets)=args.get("political_cost_ids") {provider.set_political_cost_ids(targets)?;}
            if let Some(ids)=args.get("province_ids") {provider.set_province_ids(ids)?;}
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
    fn read(value:Value)->acquisition::Sample{
        acquisition::Sample{value,watch:Default::default(),world:0,event_cache:Value::Null}
    }
    fn packet()->watch::Packet{watch::Packet{bytes:vec![],ranges:0,read_bytes:0}}
    /// A provider with a cached read and an audit whose result is `result`.
    fn auditing(result:Option<AuditResult>)->(Provider,mpsc::Sender<AuditResult>){
        let mut p=Provider::new(&json!({"path":"unused","manifest_sha256":"unused"})).unwrap();
        p.install(read(json!("old")),packet(),1.);
        let (send,receive)=mpsc::channel();
        if let Some(result)=result {send.send(result).unwrap();}
        p.audit_job=Some(Audit{generation:p.generation,started:10.,result:receive});
        (p,send)
    }
    #[test]
    fn a_running_audit_leaves_the_cached_read_and_a_finished_one_replaces_it() {
        let (mut p,send)=auditing(None);
        p.finish_audit().unwrap();
        assert_eq!(p.cached.as_ref().unwrap().sample.value,"old","still running: probes answer from the cached read");
        assert!(p.audit_job.is_some());
        send.send(Ok((read(json!("new")),packet(),1.5))).unwrap();
        let generation=p.generation;
        p.finish_audit().unwrap();
        let cached=p.cached.as_ref().unwrap();
        assert_eq!(cached.sample.value,"new");
        assert_eq!(cached.audited,11.5,"audited when the audit's read ended");
        assert!(p.audit_job.is_none()&&p.generation>generation);
        assert_eq!((p.metrics["audits"].clone(),p.metrics["full_reads"].clone()),(json!(1),json!(2)));
    }
    #[test]
    fn an_audit_older_than_the_cached_read_is_dropped_and_a_failed_one_fails_the_collect() {
        let (mut p,_send)=auditing(Some(Ok((read(json!("older")),packet(),1.))));
        p.install(read(json!("probe change")),packet(),12.);
        p.finish_audit().unwrap();
        assert_eq!(p.cached.as_ref().unwrap().sample.value,"probe change");
        assert!(p.audit_job.is_none());
        let (mut p,_send)=auditing(Some(Err("Lettura incoerente".into())));
        assert_eq!(p.finish_audit().unwrap_err(),"Lettura incoerente");
        let (mut p,send)=auditing(None);drop(send);
        assert!(p.finish_audit().is_err(),"a vanished audit thread is a failed read");
    }
    #[test]
    fn closing_stops_the_gui_lane_even_while_an_audit_still_holds_the_game() {
        let mut provider=Provider::new(&json!({"path":"unused","manifest_sha256":"unused"})).unwrap();
        let game=Game::owned_fixture();let pid=game.pid;
        provider.game=Some(game.clone());provider.open=Arc::new(AtomicBool::new(true));
        let lane=provider.gui_lane().unwrap();
        assert!(lane.matches_pid(Some(pid as u64)));
        provider.close();
        assert!(Arc::strong_count(&game)>=1,"the audit thread's handle");
        assert!(!lane.matches_pid(Some(pid as u64)));
        assert!(lane.geometry(&json!({"view":[1,pid,0,0,1280,720]})).is_err());
        assert!(provider.audit_job.is_none());
    }
    #[test]
    fn a_gui_registry_discovery_runs_on_its_own_thread_and_always_installs_an_outcome() {
        let mut provider=Provider::new(&json!({"path":"unused","manifest_sha256":"unused"})).unwrap();
        let game=Game::owned_fixture();
        provider.game=Some(game.clone());provider.open=Arc::new(AtomicBool::new(true));
        let lane=provider.gui_lane().unwrap();
        let state=|| lane.gui.lock().unwrap().discovery_state_for_test();
        let run=|| {
            let mut gui=lane.gui.lock().unwrap();
            // An unreadable root: the scan fails at the registry headers.
            gui.queue_discovery_for_test(crate::gui_registry::DiscoveryTarget{base:game.base,profile:json!({}),root:0x10});
            lane.start_discovery(&mut gui, &game);
            assert!(gui.discovery_state_for_test().0,"taken while the request holds the registry");
            drop(gui);
            let began=Instant::now();
            while state().0 {
                assert!(began.elapsed().as_secs()<10,"the discovery thread never installed its outcome");
                std::thread::yield_now();
            }
            state().1.expect("failed discovery is kept as the answer")
        };
        assert!(!run().is_empty());
        let target=crate::gui_registry::DiscoveryTarget{base:game.base,profile:json!({}),root:0x10};
        assert_eq!(lane.discover(&target,(game.pid+1,game.created)).err().as_deref(),Some("Sessione di processo cambiata"),
            "a target queued by another session is never scanned");
        provider.close();
        assert_eq!(run(),"No qualified live game session","a closed session is never scanned");
    }
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
    fn a_new_province_filter_drops_the_cached_read_and_invalid_filters_are_refused() {
        let mut p=Provider::new(&json!({"path":"unused","manifest_sha256":"unused"})).unwrap();
        assert!(p.province_ids.is_null(),"no filter reads every province");
        let cached=||Some(Cached{sample:acquisition::Sample{value:Value::Null,watch:Default::default(),world:0,event_cache:Value::Null},packet:watch::Packet{bytes:vec![],ranges:0,read_bytes:0},audited:1.});
        p.cached=cached();
        p.set_province_ids(&json!([6380,10])).unwrap();assert!(p.cached.is_none());
        p.cached=cached();
        p.set_province_ids(&json!([6380,10])).unwrap();assert!(p.cached.is_some(),"same filter keeps the read");
        for bad in [Value::Null,json!([0]),json!(["6380"]),json!({})] {assert!(p.set_province_ids(&bad).is_err());}
        assert_eq!(p.province_ids,json!([6380,10]));
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
        provider.game=Some(game.clone());provider.open=Arc::new(AtomicBool::new(true));
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
    fn the_registry_lane_answers_while_a_collect_holds_the_provider_and_closing_clears_it() {
        let handle=execute("provider_open",&json!({"path":"unused","manifest_sha256":"unused"})).unwrap().as_u64().unwrap();
        assert_eq!(gui_lane(handle).err().as_deref(),Some("No qualified live game session"));
        let provider=registry().lock().unwrap().get(&handle).unwrap().0.clone();
        let game=Game::owned_fixture();let pid=game.pid;
        {
            let mut p=provider.lock().unwrap();
            // What `collect_inner` does when it opens a session, before its full read.
            p.game=Some(game.clone());p.open=Arc::new(AtomicBool::new(true));p.publish_lane();
        }
        let collecting=provider.lock().unwrap();
        let (send,receive)=mpsc::channel();
        let worker=std::thread::spawn(move || {send.send(gui_lane(handle).map(|lane|lane.matches_pid(Some(pid as u64)))).unwrap();});
        let result=receive.recv_timeout(std::time::Duration::from_secs(1));
        drop(collecting);worker.join().unwrap();
        assert_eq!(result.expect("gui_lane waited on the provider lock"),Ok(true));
        let lane=gui_lane(handle).unwrap();
        provider.lock().unwrap().close();
        assert!(!lane.matches_pid(Some(pid as u64)));
        assert_eq!(gui_lane(handle).err().as_deref(),Some("No qualified live game session"));
        assert_eq!(execute("provider_release",&json!({"handle":handle})).unwrap(),true);
        assert_eq!(gui_lane(handle).err().as_deref(),Some("Unknown provider handle"));
    }
    #[test]
    fn a_failed_collect_records_its_stage_and_error_and_repeats_are_counted_not_listed() {
        let mut provider=Provider::new(&json!({"path":"Z:\\missing-native-provider-test\\hoi4.exe","manifest_sha256":"invalid"})).unwrap();
        for _ in 0..3 {assert!(provider.collect(1.).is_err());}
        let closes=provider.metrics["closes"].as_array().unwrap().clone();
        assert_eq!(closes.len(),1,"the same failure again is one event: {closes:?}");
        assert_eq!((closes[0]["seq"].clone(),closes[0]["stage"].clone(),closes[0]["repeats"].clone(),closes[0]["had_session"].clone()),
            (json!(1),json!("executable"),json!(3),json!(false)));
        assert!(closes[0]["error"].as_str().is_some_and(|e|!e.is_empty()));
        assert!(provider.collect(f64::NAN).is_err());
        let last=provider.metrics["closes"].as_array().unwrap().last().unwrap().clone();
        assert_eq!((last["seq"].clone(),last["stage"].clone(),last["error"].clone()),(json!(2),json!("clock"),json!("Invalid provider clock")));
        assert_eq!(provider.metrics["close_count"],2);
    }
    #[test]
    fn a_failed_audit_is_recorded_with_the_open_session_and_the_list_is_bounded() {
        let mut provider=Provider::new(&json!({"path":"unused","manifest_sha256":"unused"})).unwrap();
        provider.game=Some(Game::owned_fixture());
        provider.record_close("audit","Lettura incoerente",5.);
        assert_eq!(provider.metrics["closes"][0]["had_session"],true);
        for n in 0..40 {provider.record_close("probe",&format!("error {n}"),6.);}
        let closes=provider.metrics["closes"].as_array().unwrap();
        assert_eq!(closes.len(),CLOSES_KEPT);
        assert_eq!((closes.last().unwrap()["seq"].clone(),provider.metrics["close_count"].clone()),(json!(41),json!(41)));
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
