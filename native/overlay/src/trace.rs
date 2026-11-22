//! Bounded responsiveness trace of the HUD loop (`hud-trace.jsonl`, next to the
//! status file). One JSON line per loop iteration and per show/hide change, so
//! `scripts/hud-trace-report.cjs` can tell where the time goes and why the HUD
//! went blank. It records timings and counts only: no guide text, no images,
//! no game memory. The file rotates at `ROTATE_BYTES`, keeping `KEEP` older
//! files (`hud-trace.1.jsonl`, `hud-trace.2.jsonl`), so it never grows unbounded.
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

pub const ROTATE_BYTES: u64 = 5 * 1024 * 1024;
pub const KEEP: usize = 2;
const FLUSH_EVERY: Duration = Duration::from_millis(500);

pub fn path_for(status: &Path) -> PathBuf {
    status.parent().unwrap_or(Path::new(".")).join("hud-trace.jsonl")
}
fn rotated(path: &Path, index: usize) -> PathBuf {
    path.with_extension(format!("{index}.jsonl"))
}
fn unix_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0., |d| (d.as_secs_f64() * 1000.).round())
}
/// Round a duration in milliseconds to 0.1 ms, keeping the lines short.
pub fn ms(duration: Duration) -> f64 {
    (duration.as_secs_f64() * 10_000.).round() / 10.
}
/// Shift `path` to `.1`, `.1` to `.2` …, dropping the oldest beyond `keep`.
fn rotate(path: &Path, keep: usize) {
    if keep == 0 {
        let _ = std::fs::remove_file(path);
        return;
    }
    let _ = std::fs::remove_file(rotated(path, keep));
    for index in (1..keep).rev() {
        let _ = std::fs::rename(rotated(path, index), rotated(path, index + 1));
    }
    let _ = std::fs::rename(path, rotated(path, 1));
}

/// Why the HUD shows nothing in an iteration, while it was showing before.
pub fn blank_cause(paused: bool, foreground: bool, connection_lost: bool, delivery_known: bool, capture_error: bool) -> &'static str {
    if paused {
        "paused"
    } else if !foreground {
        "game_background"
    } else if connection_lost {
        "connection_lost"
    } else if capture_error {
        "capture_error"
    } else if !delivery_known {
        "delivery_unknown"
    } else {
        "no_card_or_marks"
    }
}

/// A backend message, at most 120 characters (never guide text: the error
/// and native error fields carry the backend's own status messages).
fn short_text(value: &Value) -> Option<String> {
    value.as_str().filter(|s| !s.is_empty()).map(|s| s.chars().take(120).collect())
}

/// One loop iteration as the trace stores it.
pub struct Iteration<'a> {
    pub branch: &'static str,
    pub foreground: bool,
    pub observe: Option<Duration>,
    pub reply: &'a Value,
    pub reply_bytes: usize,
    pub card: bool,
    pub marks: usize,
    /// Construction map markers among the drawn rows (HUD quality plan F1).
    pub map: usize,
    pub capture_error: bool,
    /// Longest pause of the loop itself since the previous recorded sample:
    /// the time drawing, card drag and buttons could not be served. Since the
    /// loop no longer waits on HTTP, `loop_ms` is the gap between answers.
    pub gap: Option<Duration>,
}
impl<'a> Iteration<'a> {
    pub fn new(branch: &'static str, foreground: bool, reply: &'a Value) -> Self {
        Self { branch, foreground, observe: None, reply, reply_bytes: 0, card: false, marks: 0, map: 0, capture_error: false, gap: None }
    }
}

pub struct Trace {
    path: PathBuf,
    out: Option<BufWriter<File>>,
    written: u64,
    limit: u64,
    flushed: Instant,
    last_iteration: Option<Instant>,
    shown: Option<bool>,
    last_state_age: Option<f64>,
    /// `seq` of the last session close written (`provider_close`).
    last_close: Option<(u64, f64)>,
}

impl Trace {
    pub fn open(path: PathBuf) -> Self {
        Self::with_limit(path, ROTATE_BYTES)
    }
    fn with_limit(path: PathBuf, limit: u64) -> Self {
        let mut trace = Self { path, out: None, written: 0, limit, flushed: Instant::now(), last_iteration: None, shown: None, last_state_age: None, last_close: None };
        trace.reopen();
        trace
    }
    fn reopen(&mut self) {
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        self.written = std::fs::metadata(&self.path).map_or(0, |m| m.len());
        self.out = OpenOptions::new().create(true).append(true).open(&self.path).ok().map(BufWriter::new);
    }
    pub fn event(&mut self, mut line: Value) {
        line["t"] = json!(unix_ms());
        let Ok(mut bytes) = serde_json::to_vec(&line) else { return };
        bytes.push(b'\n');
        if self.written > 0 && self.written + bytes.len() as u64 > self.limit {
            self.out = None;
            rotate(&self.path, KEEP);
            self.reopen();
        }
        let Some(out) = self.out.as_mut() else { return };
        if out.write_all(&bytes).is_ok() {
            self.written += bytes.len() as u64;
        }
        if self.flushed.elapsed() >= FLUSH_EVERY || line["ev"] != "iter" {
            let _ = out.flush();
            self.flushed = Instant::now();
        }
    }
    /// Record one iteration, and a `show`/`hide` line when visibility changes.
    pub fn iteration(&mut self, sample: Iteration<'_>, paused: bool) {
        let now = Instant::now();
        let reply = sample.reply;
        let loop_ms = self.last_iteration.map(|last| ms(now.duration_since(last)));
        self.last_iteration = Some(now);
        let known = reply["live_delivery_fresh"] != false;
        let state_age = reply["state_age_ms"].as_f64();
        let mut line = json!({"ev":"iter","branch":sample.branch,"fg":sample.foreground,"loop_ms":loop_ms,
            // As the backend said it: true, false, or null when absent.
            "fresh":reply.get("live_delivery_fresh").cloned().unwrap_or(Value::Null),"usable":reply["usable"]==true,"card":sample.card,"marks":sample.marks});
        if let Some(gap) = sample.gap {
            line["gap_ms"] = json!(ms(gap));
        }
        if let Some(observe) = sample.observe {
            line["rt_ms"] = json!(ms(observe));
            line["bytes"] = json!(sample.reply_bytes);
            for (key, field) in [("handling_ms", "handling_ms"), ("geometry_ms", "geometry_ms"), ("state_age_ms", "state_age_ms")] {
                if let Some(value) = reply[field].as_f64() {
                    line[key] = json!((value * 10.).round() / 10.);
                }
            }
        }
        if reply["connection_lost"] == true {
            line["lost"] = json!(true);
        }
        // Why a delivery is unusable, and what the GUI geometry cost: a
        // registry rebuild (`disc`; `disc_bg` when it ran off the request
        // path), the event scan (`refresh_ms`), retries.
        if reply["usable"] != true && let Some(error) = short_text(&reply["error"]) {
            line["error"] = json!(error);
        }
        if let Some(error) = short_text(&reply["native_error"]) {
            line["native_error"] = json!(error);
        }
        let diagnostics = &reply["diagnostics"];
        let tenth = |value: f64| (value * 10.).round() / 10.;
        if let Some(value) = diagnostics["snapshot_ms"].as_f64() {
            line["snap_ms"] = json!(tenth(value));
        }
        if let Some(count) = diagnostics["discovered"].as_u64().filter(|n| *n > 0) {
            line["disc"] = json!(count);
            line["disc_ms"] = json!(tenth(diagnostics["discovery_ms"].as_f64().unwrap_or(0.)));
            if diagnostics["discovery_background"] == true {
                line["disc_bg"] = json!(true);
            }
        }
        if let Some(value) = diagnostics["refresh_ms"].as_f64().filter(|v| *v >= 1.) {
            line["refresh_ms"] = json!(tenth(value));
        }
        if let Some(count) = diagnostics["retry_count"].as_u64().filter(|n| *n > 0) {
            line["retry"] = json!(count);
        }
        // Construction map markers (HUD quality plan F1 step 1), only while the
        // delivery proposes some: proposed (`map_n`), drawn (`map`), projected
        // in view (`map_pts`), the camera signature, and why none was projected.
        let proposed = reply["proposed"].as_array().map_or(0, |rows| rows.iter().filter(|row| row["panel"] == "construction_map").count());
        if proposed > 0 {
            line["map_n"] = json!(proposed);
            line["map"] = json!(sample.map);
            if let Some(points) = diagnostics["map_points"].as_u64() {
                line["map_pts"] = json!(points);
            }
            if diagnostics["map_camera"].is_array() {
                line["map_cam"] = diagnostics["map_camera"].clone();
            }
            if let Some(error) = short_text(&diagnostics["map_unavailable"]) {
                line["map_err"] = json!(error);
            }
            if let Some(reason) = short_text(&diagnostics["map_skipped"]) {
                line["map_skip"] = json!(reason);
            }
        }
        // A younger delivery than the previous sample is a new one: record how
        // long the backend took to build it.
        if let Some(age) = state_age {
            if self.last_state_age.is_none_or(|previous| age < previous) && reply["delivery_pipeline_ms"].is_object() {
                line["delivery"] = reply["delivery_pipeline_ms"].clone();
            }
        }
        self.last_state_age = state_age;
        // Why the backend last closed the qualified game session: written on
        // the first sample that carries a new close event (HUD plan part 2,
        // step 2). The backend keeps sending the last one until another comes.
        let close = &reply["provider_close"];
        if let (Some(seq), Some(at)) = (close["seq"].as_u64(), close["unix_ms"].as_f64())
            && self.last_close != Some((seq, at))
        {
            self.last_close = Some((seq, at));
            line["close_seq"] = json!(seq);
            line["close_at"] = json!(at);
            line["close_stage"] = close["stage"].clone();
            line["close_error"] = json!(short_text(&close["error"]).unwrap_or_default());
            line["close_had_session"] = json!(close["had_session"] == true);
        }
        self.event(line);
        let shown = sample.card || sample.marks > 0;
        if self.shown != Some(shown) {
            let mut change = json!({"ev": if shown {"show"} else {"hide"},"fg":sample.foreground,"card":sample.card,"marks":sample.marks});
            if !shown {
                change["cause"] = json!(blank_cause(paused, sample.foreground, reply["connection_lost"] == true, known, sample.capture_error));
                if let Some(error) = short_text(&reply["error"]) {
                    change["error"] = json!(error);
                }
            }
            // The first sample of a process is a state, not a change.
            if self.shown.is_some() || shown {
                self.event(change);
            }
            self.shown = Some(shown);
        }
    }
    /// Round trip of a control request (Done, Undo, select…) on its worker.
    pub fn action(&mut self, kind: &str, took: Duration) {
        self.event(json!({"ev":"action","kind":kind,"rt_ms":ms(took)}));
    }
}
impl Drop for Trace {
    fn drop(&mut self) {
        if let Some(out) = self.out.as_mut() {
            let _ = out.flush();
        }
    }
}
/// Append one line without a live `Trace` (process start and fatal errors).
pub fn append(path: &Path, line: Value) {
    let mut trace = Trace::open(path.to_path_buf());
    trace.event(line);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hoi4-hud-trace-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("hud-trace.jsonl")
    }
    fn lines(path: &Path) -> Vec<Value> {
        std::fs::read_to_string(path).unwrap_or_default().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
    }

    #[test]
    fn trace_sits_next_to_the_status_file() {
        assert_eq!(path_for(Path::new("data/overlay-status.json")), Path::new("data").join("hud-trace.jsonl"));
    }

    #[test]
    fn iterations_record_timings_and_blank_changes_with_cause() {
        let path = temp("iter");
        {
            let mut trace = Trace::open(path.clone());
            let shown = json!({"usable":true,"handling_ms":12.34,"geometry_ms":3.21,"state_age_ms":400.0,"delivery_pipeline_ms":{"acquisition":900.0}});
            trace.iteration(Iteration { observe: Some(Duration::from_millis(20)), reply_bytes: 5000, card: true, marks: 2, gap: Some(Duration::from_millis(17)), ..Iteration::new("native", true, &shown) }, false);
            let unknown = json!({"live_delivery_fresh":false,"usable":false});
            trace.iteration(Iteration { observe: Some(Duration::from_millis(5)), ..Iteration::new("idle", true, &unknown) }, false);
            trace.iteration(Iteration::new("idle", false, &unknown), false);
        }
        let lines = lines(&path);
        let kinds: Vec<&str> = lines.iter().map(|l| l["ev"].as_str().unwrap()).collect();
        assert_eq!(kinds, ["iter", "show", "iter", "hide", "iter"]);
        assert_eq!(lines[0]["rt_ms"], 20.0);
        assert_eq!(lines[0]["handling_ms"], 12.3);
        assert_eq!(lines[0]["bytes"], 5000);
        assert_eq!(lines[0]["gap_ms"], 17.0);
        assert!(lines[2].get("gap_ms").is_none(), "no gap measured, none written");
        assert_eq!(lines[0]["delivery"]["acquisition"], 900.0, "first delivery seen is recorded");
        assert_eq!(lines[3]["cause"], "delivery_unknown");
        assert!(lines[2]["loop_ms"].is_number());
        assert!(lines[4].get("rt_ms").is_none(), "no request, no round trip");
        assert!(lines.iter().all(|l| l["t"].is_number()));
    }

    #[test]
    fn a_session_close_is_written_once_with_its_stage_and_error() {
        let path = temp("close");
        {
            let mut trace = Trace::open(path.clone());
            let close = json!({"seq":3,"unix_ms":1000.0,"stage":"audit","error":"Lettura incoerente","had_session":true,"repeats":1});
            let reply = json!({"usable":false,"live_delivery_fresh":false,"provider_close":close});
            for _ in 0..2 {
                trace.iteration(Iteration::new("idle", true, &reply), false);
            }
            let later = json!({"usable":false,"provider_close":{"seq":4,"unix_ms":2000.0,"stage":"probe","error":"e".repeat(300),"had_session":false}});
            trace.iteration(Iteration::new("idle", true, &later), false);
            trace.iteration(Iteration::new("idle", true, &json!({"usable":false})), false);
        }
        let lines: Vec<Value> = lines(&path).into_iter().filter(|l| l["ev"] == "iter").collect();
        assert_eq!((lines[0]["close_seq"].clone(), lines[0]["close_stage"].clone(), lines[0]["close_error"].clone(), lines[0]["close_had_session"].clone()),
            (json!(3), json!("audit"), json!("Lettura incoerente"), json!(true)));
        assert!(lines[1].get("close_seq").is_none(), "the same close is written once");
        assert_eq!((lines[2]["close_seq"].clone(), lines[2]["close_stage"].clone()), (json!(4), json!("probe")));
        assert_eq!(lines[2]["close_error"].as_str().unwrap().chars().count(), 120);
        assert!(lines[3].get("close_seq").is_none());
    }
    #[test]
    fn a_registry_rebuild_off_the_request_path_is_marked() {
        let path = temp("background-discovery");
        {
            let mut trace = Trace::open(path.clone());
            let reply = json!({"usable":true,"geometry_ms":3.0,
                "diagnostics":{"snapshot_ms":2.0,"discovered":1,"discovery_ms":734.56,"discovery_background":true}});
            trace.iteration(Iteration { card: true, marks: 2, ..Iteration::new("native", true, &reply) }, false);
        }
        let lines = lines(&path);
        assert_eq!((lines[0]["disc"].clone(), lines[0]["disc_ms"].clone(), lines[0]["disc_bg"].clone()), (json!(1), json!(734.6), json!(true)));
    }
    #[test]
    fn unusable_replies_and_geometry_costs_are_recorded() {
        let path = temp("reasons");
        {
            let mut trace = Trace::open(path.clone());
            let shown = json!({"usable":true,"error":"ignored while usable","geometry_ms":640.0,
                "diagnostics":{"snapshot_ms":612.34,"discovered":1,"discovery_ms":590.04,"refresh_ms":0.2,"retry_count":0}});
            trace.iteration(Iteration { card: true, marks: 5, ..Iteration::new("native", true, &shown) }, false);
            let blank = json!({"usable":false,"live_delivery_fresh":true,"error":"x".repeat(300),"native_error":"Vista focus di un’altra nazione"});
            trace.iteration(Iteration::new("native", true, &blank), false);
        }
        let lines = lines(&path);
        assert_eq!((lines[0]["snap_ms"].clone(), lines[0]["disc"].clone(), lines[0]["disc_ms"].clone()), (json!(612.3), json!(1), json!(590.0)));
        assert!(lines[0].get("error").is_none() && lines[0].get("refresh_ms").is_none() && lines[0].get("retry").is_none());
        assert!(lines[0].get("disc_bg").is_none(), "a rebuild inside the request");
        assert_eq!(lines[2]["error"].as_str().unwrap().chars().count(), 120, "bounded");
        assert_eq!(lines[2]["native_error"], "Vista focus di un’altra nazione");
        assert!(lines[2].get("disc").is_none());
        assert_eq!(lines[3]["ev"], "hide");
        assert_eq!(lines[3]["cause"], "no_card_or_marks");
        assert_eq!(lines[3]["error"].as_str().unwrap().len(), 120, "the blank says why");
    }

    #[test]
    fn construction_map_markers_are_traced_only_while_proposed() {
        let path = temp("map");
        {
            let mut trace = Trace::open(path.clone());
            let proposed = json!([{"panel":"construction_map"},{"panel":"construction_map"},{"panel":"focus"}]);
            let projected = json!({"usable":true,"proposed":proposed,
                "diagnostics":{"map_points":1,"map_camera":[512.0,0.0012,-0.0021,0.5,0.25]}});
            trace.iteration(Iteration { marks: 1, map: 1, ..Iteration::new("native", true, &projected) }, false);
            let failed = json!({"usable":true,"proposed":proposed,"diagnostics":{"map_unavailable":"Stato cambiato durante la lettura"}});
            trace.iteration(Iteration { marks: 1, map: 1, ..Iteration::new("native", true, &failed) }, false);
            let skipped = json!({"usable":true,"proposed":proposed,"diagnostics":{"map_skipped":"central_panel"}});
            trace.iteration(Iteration::new("native", true, &skipped), false);
            trace.iteration(Iteration::new("native", true, &json!({"usable":true,"proposed":[{"panel":"focus"}]})), false);
        }
        let lines: Vec<Value> = lines(&path).into_iter().filter(|l| l["ev"] == "iter").collect();
        assert_eq!((lines[0]["map_n"].clone(), lines[0]["map"].clone(), lines[0]["map_pts"].clone()), (json!(2), json!(1), json!(1)));
        assert_eq!(lines[0]["map_cam"], json!([512.0, 0.0012, -0.0021, 0.5, 0.25]));
        assert!(lines[0].get("map_err").is_none() && lines[0].get("map_skip").is_none());
        assert_eq!(lines[1]["map_err"], "Stato cambiato durante la lettura");
        assert!(lines[1].get("map_cam").is_none() && lines[1].get("map_pts").is_none());
        assert_eq!((lines[2]["map"].clone(), lines[2]["map_skip"].clone()), (json!(0), json!("central_panel")));
        assert!(lines[3].get("map_n").is_none() && lines[3].get("map").is_none(), "no map proposal, no map fields");
    }

    #[test]
    fn blank_cause_names_the_first_reason_that_applies() {
        assert_eq!(blank_cause(true, false, true, false, true), "paused");
        assert_eq!(blank_cause(false, false, true, false, false), "game_background");
        assert_eq!(blank_cause(false, true, true, false, false), "connection_lost");
        assert_eq!(blank_cause(false, true, false, true, true), "capture_error");
        assert_eq!(blank_cause(false, true, false, false, false), "delivery_unknown");
        assert_eq!(blank_cause(false, true, false, true, false), "no_card_or_marks");
    }

    #[test]
    fn the_trace_rotates_and_keeps_a_bounded_number_of_files() {
        let path = temp("rotate");
        {
            let mut trace = Trace::with_limit(path.clone(), 200);
            for i in 0..40 {
                trace.event(json!({"ev":"start","i":i}));
            }
        }
        let total: u64 = [path.clone(), rotated(&path, 1), rotated(&path, 2)].iter()
            .map(|p| std::fs::metadata(p).map_or(0, |m| m.len())).sum();
        assert!(total <= 3 * 200, "three files at most, each within the limit");
        assert!(!rotated(&path, 3).exists());
        let newest = lines(&path);
        assert_eq!(newest.last().unwrap()["i"], 39, "the newest lines are in the main file");
    }
}
