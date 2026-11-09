//! Loopback-only native host. One bounded request per connection; no proxy or
//! resident interpreter. The existing cores own observations and decisions.
use crate::{application::Application, catalog_generation::Catalog, events::Hub, monitor::Monitor};
use serde_json::{Value, json};
use sha2::Digest;
use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
type Result<T> = std::result::Result<T, String>;
const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data: blob:; connect-src 'self'; object-src 'none'";
const MAX_BODY: usize = 32 * 1024 * 1024;
static UPLOADS: AtomicUsize = AtomicUsize::new(0);
struct Upload {
    path: PathBuf,
}
impl Upload {
    fn receive(reader: &mut impl Read, length: usize) -> std::result::Result<Self, (u16, String)> {
        if length == 0 || length > 200_000_000 {
            return Err((413, "File vuoto o superiore a 200 MB".into()));
        }
        UPLOADS
            .try_update(Ordering::AcqRel, Ordering::Relaxed, |n| {
                (n < 2).then_some(n + 1)
            })
            .map_err(|_| (503, "Too many document uploads".into()))?;
        let id = match crate::progress_store::new_id() {
            Ok(id) => id,
            Err(e) => {
                UPLOADS.fetch_sub(1, Ordering::AcqRel);
                return Err((503, e));
            }
        };
        let owned = Self {
            path: std::env::temp_dir().join(format!("hoi4-upload-{id}.zip")),
        };
        let receive = (|| {
            let mut file = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&owned.path)
                .map_err(|e| (503, e.to_string()))?;
            let copied = std::io::copy(&mut reader.take(length as u64), &mut file)
                .map_err(|_| (400, "Caricamento incompleto".into()))?;
            if copied != length as u64 {
                return Err((400, "Caricamento incompleto".into()));
            }
            file.flush().map_err(|e| (503, e.to_string()))?;
            Ok(())
        })();
        receive?;
        Ok(owned)
    }
}
impl Drop for Upload {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        UPLOADS.fetch_sub(1, Ordering::AcqRel);
    }
}
struct Reading {
    monitor: Monitor,
    cached: Option<(Instant, Value)>,
}
struct ProviderOwner {
    handle: u64,
    args: Value,
}
impl ProviderOwner {
    fn open(args: Value) -> Result<Self> {
        let handle = crate::request(0, "provider_open", args.clone())?
            .as_u64()
            .ok_or("Invalid provider handle")?;
        Ok(Self { handle, args })
    }
}
impl Drop for ProviderOwner {
    fn drop(&mut self) {
        let _ = crate::request(0, "provider_release", json!({"handle":self.handle}));
    }
}
pub struct Service {
    application: Mutex<Application>,
    catalog: Mutex<Option<Catalog>>,
    catalog_args: Mutex<Option<Value>>,
    reading: Mutex<Reading>,
    provider: Mutex<Option<ProviderOwner>>,
    hud: Mutex<crate::hud_service::Hud>,
    hud_cache: Mutex<Option<(Value, Value)>>,
    hud_status_path: PathBuf,
    web: PathBuf,
    references: PathBuf,
    template_mutation: Mutex<()>,
    frames: Arc<crate::media_frames::Frames>,
    videos: Arc<crate::video_jobs::Jobs>,
    guide_directory: PathBuf,
    bundled_guides: PathBuf,
    association_directory: PathBuf,
    rules_root: PathBuf,
    cwtools_host: PathBuf,
    parser_library: PathBuf,
    source_error_log: PathBuf,
    demo_file: PathBuf,
    analysis_cache: Mutex<Option<(u64, Value)>>,
    analysis_generation: AtomicU64,
    rule_book: Mutex<Option<Value>>,
    association_revision: AtomicU64,
    ui_language: Mutex<String>,
    hub: Hub,
    clients: Mutex<usize>,
    wake: Condvar,
    stopped: AtomicBool,
    shutdown_token: Option<String>,
    began: Instant,
}
fn timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut day = seconds / 86400;
    let mut year = 1970u64;
    let leap = |y: u64| y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400));
    loop {
        let n = if leap(year) { 366 } else { 365 };
        if day < n {
            break;
        }
        day -= n;
        year += 1;
    }
    let mut month = 1;
    for n in [
        31,
        if leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ] {
        if day < n {
            break;
        }
        day -= n;
        month += 1;
    }
    format!(
        "{year:04}-{month:02}-{:02}T{:02}:{:02}:{:02}+00:00",
        day + 1,
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60
    )
}
impl Service {
    pub fn open(config: &Value) -> Result<Arc<Self>> {
        let web = PathBuf::from(
            config["web_root"]
                .as_str()
                .ok_or("Missing web asset root")?,
        )
        .canonicalize()
        .map_err(|e| e.to_string())?;
        let association_directory = config["association_directory"]
            .as_str()
            .map(PathBuf::from)
            .or_else(|| {
                config["path"]
                    .as_str()
                    .and_then(|p| std::path::Path::new(p).parent())
                    .map(|p| p.join("guide-associations"))
            })
            .ok_or("Missing association directory")?;
        let mut app_config = config.clone();
        app_config["association_directory"] = json!(association_directory);
        let mut app = Application::open(&app_config)?;
        let references = config["reference_library"]
            .as_str()
            .map(PathBuf::from)
            .or_else(|| {
                config["path"]
                    .as_str()
                    .and_then(|p| std::path::Path::new(p).parent())
                    .map(|p| p.join("reference-library"))
            })
            .ok_or("Missing reference library root")?;
        let guide_directory = config["guide_directory"]
            .as_str()
            .map(PathBuf::from)
            .or_else(|| {
                config["path"]
                    .as_str()
                    .and_then(|p| std::path::Path::new(p).parent())
                    .map(|p| p.join("guides"))
            })
            .ok_or("Missing guide directory")?;
        let bundled_guides = config["bundled_guides"]
            .as_str()
            .map(PathBuf::from)
            .unwrap_or_else(|| web.parent().unwrap_or(&web).join("guides"));
        let rules_root = config["rules_root"]
            .as_str()
            .map(PathBuf::from)
            .unwrap_or_else(|| web.parent().unwrap_or(&web).join("rules/cwtools"));
        let package = web.parent().unwrap_or(&web);
        let cwtools_host = config["cwtools_host"]
            .as_str()
            .map(PathBuf::from)
            .unwrap_or_else(|| package.join("_native/cwtools"));
        let parser_library = config["parser_library"]
            .as_str()
            .map(PathBuf::from)
            .unwrap_or_else(|| package.join("_native/hoi4companion_parser.dll"));
        let source_error_log = config["source_error_log"]
            .as_str()
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                association_directory
                    .parent()
                    .unwrap_or(&association_directory)
                    .join("logs/source-errors.json")
            });
        let demo_file = config["catalog"]["demo_file"]
            .as_str()
            .map(PathBuf::from)
            .unwrap_or_else(|| package.join("examples/demo_focus.txt"));
        let frames_root = config["guide_frames"]
            .as_str()
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                association_directory
                    .parent()
                    .unwrap_or(&association_directory)
                    .join("guide-frames")
            });
        let configured_ffmpeg = config["ffmpeg"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| std::env::var("HOI4_FFMPEG").ok());
        let ffmpeg =
            crate::process_tools::executable(configured_ffmpeg.as_deref(), "ffmpeg", Some(package));
        let frames = crate::media_frames::Frames::new(frames_root, ffmpeg);
        let videos_root = config["guide_videos"]
            .as_str()
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                association_directory
                    .parent()
                    .unwrap_or(&association_directory)
                    .join("guide-videos")
            });
        let videos =
            crate::video_jobs::Jobs::new(videos_root, frames.clone(), package.to_path_buf());
        let catalog_args = config.get("catalog").cloned();
        let mut catalog = catalog_args.as_ref().map(Catalog::open).transpose()?;
        if let Some(catalog) = catalog.as_mut() {
            app.set_context(crate::runtime_context::Context::new(
                catalog,
                crate::reference_library::manifest(&references),
            )?)?;
        }
        let ui_language = catalog
            .as_ref()
            .and_then(|c| c.sources["settings"]["ui_language"].as_str())
            .unwrap_or("english")
            .to_owned();
        let provider = if let Some(mut spec) = config.get("provider").cloned() {
            if let Some(c) = &catalog
                && c.sources["demo"] != true
                && spec.get("province_neighbors").is_none()
            {
                let mods = c.sources["mod_roots"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(PathBuf::from)
                    .collect::<Vec<_>>();
                spec["province_neighbors"] = crate::map_topology::neighbors(
                    std::path::Path::new(c.sources["settings"]["game_dir"].as_str().unwrap_or("")),
                    &mods,
                )?;
            }
            Some(ProviderOwner::open(spec)?)
        } else {
            None
        };
        let result = Arc::new(Self {
            application: Mutex::new(app),
            catalog: Mutex::new(catalog),
            catalog_args: Mutex::new(catalog_args),
            reading: Mutex::new(Reading {
                monitor: Monitor::default(),
                cached: None,
            }),
            provider: Mutex::new(provider),
            hud: Mutex::new(crate::hud_service::Hud::default()),
            hud_cache: Mutex::new(None),
            hud_status_path: association_directory.parent().unwrap_or(&association_directory).join("overlay-status.json"),
            web,
            references,
            template_mutation: Mutex::new(()),
            frames,
            videos,
            guide_directory,
            bundled_guides,
            association_directory,
            rules_root,
            cwtools_host,
            parser_library,
            source_error_log,
            demo_file,
            analysis_cache: Mutex::new(None),
            analysis_generation: AtomicU64::new(0),
            rule_book: Mutex::new(None),
            association_revision: AtomicU64::new(0),
            ui_language: Mutex::new(ui_language),
            hub: Hub::new(0.25, 8)?,
            clients: Mutex::new(0),
            wake: Condvar::new(),
            stopped: AtomicBool::new(false),
            shutdown_token: config["shutdown_token"]
                .as_str()
                .filter(|s| s.len() == 64 && s.bytes().all(|c| c.is_ascii_hexdigit()))
                .map(str::to_owned),
            began: Instant::now(),
        });
        let weak = Arc::downgrade(&result);
        thread::Builder::new()
            .name("hoi4-native-events".into())
            .spawn(move || {
                while let Some(owner) = weak.upgrade() {
                    let mut clients = match owner.clients.lock() {
                        Ok(v) => v,
                        Err(_) => break,
                    };
                    if owner.stopped.load(Ordering::Acquire) {
                        break;
                    }
                    if *clients == 0 {
                        if let Ok(provider) = owner.provider.lock()
                            && let Some(provider) = provider.as_ref()
                        {
                            let handle = provider.handle;
                            let _ = crate::request(0, "provider_close", json!({"handle":handle}));
                        }
                        clients = match owner.wake.wait_timeout(clients, Duration::from_secs(1)) {
                            Ok(v) => v.0,
                            Err(_) => break,
                        };
                        drop(clients);
                        continue;
                    }
                    drop(clients);
                    let began = Instant::now();
                    let packet = owner.poll().unwrap_or_else(
                        |e| json!({"usable":false,"status":"disconnected","sample":null,"error":e}),
                    );
                    let _ = owner.hub.publish(packet, owner.now());
                    let clients = match owner.clients.lock() {
                        Ok(v) => v,
                        Err(_) => break,
                    };
                    let wait = Duration::from_millis(250).saturating_sub(began.elapsed());
                    let _ = owner.wake.wait_timeout(clients, wait);
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(result)
    }
    fn now(&self) -> f64 {
        self.began.elapsed().as_secs_f64()
    }
    fn poll(&self) -> Result<Value> {
        let mut reading = self
            .reading
            .lock()
            .map_err(|_| "Reader service unavailable")?;
        if let Some((when, packet)) = &reading.cached
            && when.elapsed() < Duration::from_millis(250)
        {
            return Ok(packet.clone());
        }
        let now = self.now();
        let provider = self
            .provider
            .lock()
            .map_err(|_| "Provider service unavailable")?;
        let sample = provider
            .as_ref()
            .ok_or_else(|| "Game installation not configured".into())
            .and_then(|provider| {
                let handle = provider.handle;
                crate::request(0, "provider_collect", json!({"handle":handle,"now":now}))
            });
        let mut value = match sample {
            Ok(sample) => reading.monitor.accept(sample, now, &timestamp())?,
            Err(error) => reading.monitor.failed(&error, now)?,
        };
        if let Some(provider) = provider.as_ref() {
            let handle = provider.handle;
            value["metrics"] =
                crate::request(0, "provider_inspect", json!({"handle":handle}))?["metrics"].clone();
        }
        value["transport"] =
            json!({"kind":"change_events","memory_probe_seconds":0.25,"integrity_audit_seconds":5});
        let mut app = self
            .application
            .lock()
            .map_err(|_| "Application unavailable")?;
        app.accept(value)?;
        let mut packet = app.live_packet()?;
        packet["overlay"] = crate::hud_diagnostics::read_status(
            &self.hud_status_path, &packet["active_guide_id"], &packet["guide_server_id"],
        );
        reading.cached = Some((Instant::now(), packet.clone()));
        Ok(packet)
    }
    fn app(&self, op: &str, args: &Value) -> Result<Value> {
        self.application
            .lock()
            .map_err(|_| "Application unavailable")?
            .request(op, args)
    }
    fn dispatch(&self, request: &Request) -> Result<Value> {
        let id = request.query.get("id");
        let run = request.query.get("run");
        match (request.method.as_str(), request.path.as_str()) {
            ("GET", "/api/guide/media") => {
                let (guide, step) = self.media_step(request)?;
                Ok(self
                    .frames
                    .availability(crate::media_references::references(&guide, &step)))
            }
            ("GET", "/api/script-analysis") => {
                self.script_analysis(request.query.get("refresh").map(String::as_str) == Some("1"))
            }
            ("GET", "/api/script-rules") => {
                let kind = request
                    .query
                    .get("kind")
                    .map(String::as_str)
                    .unwrap_or("trigger");
                let name = request.query.get("name").map(String::as_str).unwrap_or("");
                if !["trigger", "effect"].contains(&kind)
                    || name.is_empty()
                    || name.chars().count() > 160
                {
                    return Err("Indica trigger/effect e un nome valido".into());
                }
                let mut book = self.rule_book.lock().map_err(|_| "Rules unavailable")?;
                if book.is_none() {
                    *book = Some(crate::script_analysis_service::rulebook(&self.rules_root)?);
                }
                Ok(crate::script_analysis_service::describe(
                    book.as_ref().unwrap(),
                    kind,
                    name,
                ))
            }
            ("POST", "/api/import/extract") => {
                let value = request.json_fields(&["sources", "tree_id"])?;
                let key = std::env::var("OPENAI_API_KEY")
                    .ok()
                    .filter(|v| !v.is_empty());
                let model = std::env::var("HOI4_IMPORT_MODEL")
                    .ok()
                    .filter(|v| !v.is_empty());
                let (Some(key), Some(model)) = (key, model) else {
                    return Err("API IA non configurata: usa il pacchetto esterno oppure OPENAI_API_KEY e HOI4_IMPORT_MODEL locali".into());
                };
                let (index, reference) = self
                    .catalog(|c| Ok((c.import_index()?, c.sources["reference_labels"].clone())))?;
                let packet = crate::import_service::packet(
                    &value["sources"],
                    index.rows(),
                    &reference,
                    value["tree_id"].as_str(),
                )?;
                let body = crate::provider_extract::body(&packet, &model)?;
                let response =
                    crate::source_acquisition::Network::system()?.provider(&body, &key)?;
                let draft = crate::provider_extract::document(&response, &value["sources"])?;
                let snapshot = self.catalog(|c| c.import_snapshot(&draft["guide"]))?;
                Ok(snapshot.preview(
                    &draft["guide"],
                    &draft["sources"],
                    &crate::reference_library::manifest(&self.references),
                ))
            }
            ("POST", "/api/import/video") => {
                let input = request.json_fields(&["url", "refresh_source"])?;
                if input.get("refresh_source").is_some_and(|v| !v.is_boolean()) {
                    return Err("Indica il link a un video YouTube".into());
                }
                self.videos.start(
                    input["url"]
                        .as_str()
                        .ok_or("Indica il link a un video YouTube")?,
                    input["refresh_source"].as_bool().unwrap_or(false),
                )
            }
            ("GET", "/api/import/video/status") => {
                self.videos.status(id.map(String::as_str).unwrap_or(""))
            }
            ("POST", "/api/import/source") => crate::source_acquisition::acquire(
                &request.json_fields(&["url", "text"])?,
                self.web.parent(),
            ),
            ("GET", "/api/import/status") => {
                let mut status = crate::import_service::status();
                status["youtube_available"] =
                    json!(crate::source_acquisition::ytdlp(self.web.parent()).is_some());
                Ok(status)
            }
            ("POST", "/api/import/preview") => {
                let value = request.json_fields(&["guide", "sources"])?;
                let library = crate::reference_library::manifest(&self.references);
                let snapshot = self.catalog(|c| c.import_snapshot(&value["guide"]))?;
                Ok(snapshot.preview(&value["guide"], &value["sources"], &library))
            }
            ("POST", "/api/import/install") => {
                let value = request.json_fields(&["guide", "sources", "reviewed"])?;
                if value["reviewed"] != true {
                    return Err("Revisiona e conferma prima di installare la guida".into());
                }
                let library = crate::reference_library::manifest(&self.references);
                let snapshot = self.catalog(|c| c.import_snapshot(&value["guide"]))?;
                let report = snapshot.preview(&value["guide"], &value["sources"], &library);
                if report["compatibility"]["blocked"] == true {
                    return Err("Guide unavailable with the current DLC configuration.".into());
                }
                if report["valid"] != true {
                    return Err(format!(
                        "Guida non installabile: {}",
                        report["errors"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .take(8)
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join("; ")
                    ));
                }
                let mut app = self
                    .application
                    .lock()
                    .map_err(|_| "Application unavailable")?;
                app.can_add_guide(
                    report["guide"]["id"]
                        .as_str()
                        .ok_or("Invalid guide identity")?,
                )?;
                let published = crate::import_service::publish(
                    &report["guide"],
                    &self.guide_directory,
                    &self.bundled_guides,
                )?;
                app.put_guide(&report["guide"])?;
                Ok(published.commit())
            }
            ("POST", "/api/import/packet") => {
                let value = request.json_fields(&["sources", "tree_id"])?;
                let (index, reference) = self
                    .catalog(|c| Ok((c.import_index()?, c.sources["reference_labels"].clone())))?;
                crate::import_service::packet(
                    &value["sources"],
                    index.rows(),
                    &reference,
                    value["tree_id"].as_str(),
                )
            }
            ("POST", "/api/import/templates") => {
                let _mutation = self
                    .template_mutation
                    .lock()
                    .map_err(|_| "Template import unavailable")?;
                let result = crate::reference_library::import(
                    &self.references,
                    &request
                        .upload
                        .as_ref()
                        .ok_or("Carica un documento Word o archivio ZIP")?
                        .path,
                )?;
                self.refresh_guide_context(result.clone())?;
                Ok(result)
            }
            ("GET", "/api/reference-library") => {
                let mut library = crate::reference_library::manifest(&self.references);
                let docs = self
                    .catalog(|c| Ok(c.sources["settings"]["docs_dir"].clone()))
                    .unwrap_or(Value::Null);
                library["game_presets"] =
                    crate::reference_presets::inspect(docs.as_str(), &library["designs"]);
                Ok(library)
            }
            ("POST", "/api/import/word") => {
                let path = &request
                    .upload
                    .as_ref()
                    .ok_or("Carica un documento Word o archivio ZIP")?
                    .path;
                if request.query.contains_key("entry")
                    || request.query.get("prepare").is_some_and(|s| s == "1")
                {
                    let vocabulary = self
                        .catalog(|c| Ok(c.vocabulary.clone()))
                        .unwrap_or_else(|_| crate::import_catalog::language().clone());
                    crate::word_sources::prepare_with_vocabulary(
                        path,
                        request.query.get("entry").map(String::as_str),
                        request.query.get("context").map(String::as_str),
                        &vocabulary,
                    )
                } else {
                    let mut catalog = self.catalog.lock().map_err(|_| "Catalog unavailable")?;
                    if let Some(catalog) = catalog.as_mut() {
                        catalog.word_inventory(path)
                    } else {
                        let mut result = crate::word_sources::documents(path)?;
                        result["compatibility"] = json!({});
                        Ok(result)
                    }
                }
            }
            ("GET", "/api/import/prompt") => Ok(json!({"prompt":crate::authoring::prompt()})),
            ("POST", "/api/import/document") => {
                crate::import_document::document(&request.json_fields(&["document"])?["document"])
            }
            ("GET", "/api/config") => self.catalog(|c| Ok(c.overview())),
            ("GET", "/api/import/catalog") => {
                let export = request
                    .query
                    .get("export")
                    .map(String::as_str)
                    .unwrap_or("");
                if !["", "ai"].contains(&export) {
                    return Err("Formato export catalogo sconosciuto".into());
                }
                self.catalog(|c|{let data=c.identity_data()?;let mut result=json!({"entities":if export=="ai"{crate::entity_catalog::export(&data["entries"],&c.sources["reference_labels"])?}else{data["entries"].clone()},"demo":c.sources["demo"]});if export=="ai"{result["localization_policy"]=json!("local_lookup");result["alias_policy"]=json!("english_reference_only");}Ok(result)})
            }
            ("POST", "/api/config") => self.set_settings(&request.json()?),
            ("GET", "/api/tree") => self.catalog(|c| {
                c.tree(
                    id.map(String::as_str),
                    request.query.get("source").map(String::as_str) != Some("live"),
                )
            }),
            ("GET", "/api/technologies") => self.catalog(Catalog::technology_catalog),
            ("GET", "/api/technology-tree") => {
                self.catalog(|c| c.technology_tree(id.map(String::as_str).unwrap_or("")))
            }
            ("POST", "/api/refresh") => {
                if request.json()? != json!({}) {
                    return Err("Il ricaricamento richiede un oggetto vuoto".into());
                }
                self.configure(&json!({}), false, true)
            }
            ("GET", "/api/state") => self.poll(),
            ("GET", "/api/framework/state") => Ok(crate::state::normalize(&self.poll()?)),
            ("GET", "/api/guides") => {
                let mut guides = self.app("app_guides", &json!({}))?;
                for row in guides.as_array_mut().ok_or("Invalid guide registry")? {
                    let definition = self.app("app_definition", &json!({"guide_id":row["id"]}))?;
                    let mut catalog = self.catalog.lock().map_err(|_| "Catalog unavailable")?;
                    row["compatibility"] = if let Some(catalog) = catalog.as_mut() {
                        catalog.guide_compatibility(&definition)?
                    } else {
                        json!({"blocked":false,"status":"unknown","requirements":[],"unknown":true})
                    };
                }
                Ok(json!({"schema_version":1,"guides":guides}))
            }
            ("GET", "/api/guide/active") => self.app("app_active", &json!({})),
            ("POST", "/api/guide/active") => {
                let result = self.app("app_active", &json!({"update":request.json()?}))?;
                self.invalidate()?;
                Ok(result)
            }
            ("GET", "/api/guide") => self.app("app_guide", &json!({"guide_id":id})),
            ("GET", "/api/guide/progress") => {
                self.app("app_progress", &json!({"guide_id":id,"run_id":run}))
            }
            ("GET", "/api/guide/associations") => self.associations(id.map(String::as_str), None),
            ("POST", "/api/guide/associations") => self.associations(None, Some(&request.json()?)),
            ("GET", "/api/guide/checks") => self.app("app_checks", &json!({"guide_id":id})),
            ("POST", "/api/guide/checks") => {
                let value = self.app(
                    "app_checks",
                    &json!({"guide_id":id,"update":request.json()?}),
                )?;
                self.invalidate()?;
                Ok(value)
            }
            ("POST", "/api/guide/progress") => {
                let result = self.app(
                    "app_progress",
                    &json!({"guide_id":id,"update":request.json()?}),
                )?;
                self.invalidate()?;
                Ok(result)
            }
            ("POST", "/api/overlay") => self.overlay(request),
            ("POST", "/api/shutdown") => {
                let input = request.json_fields(&["token"])?;
                if self
                    .shutdown_token
                    .as_deref()
                    .is_none_or(|token| input["token"].as_str() != Some(token))
                {
                    return Err("Shutdown capability not valid for this service".into());
                }
                self.stopped.store(true, Ordering::Release);
                self.wake.notify_all();
                Ok(json!({"stopping":true,"pid":std::process::id()}))
            }
            ("GET", "/api/guide/runtime") => {
                self.poll()?;
                Ok(
                    self.app("app_projection", &json!({"guide_id":id,"live":true}))?["evaluation"]
                        .clone(),
                )
            }
            ("GET", "/api/guide/projection") => {
                let live = match request.query.get("live").map(String::as_str).unwrap_or("1") {
                    "0" => false,
                    "1" => true,
                    _ => return Err("Invalid projection mode".into()),
                };
                if live {
                    self.poll()?;
                }
                self.app("app_projection", &json!({"guide_id":id,"live":live}))
            }
            ("GET", "/api/backend") => Ok(
                json!({"engine":"rust-backend","schema_version":1,"resident_python":false,"native_hud_protocol":1}),
            ),
            _ => Err("Route not implemented in the native host".into()),
        }
    }
    fn overlay_snapshot(&self) -> Result<Value> {
        let due = self
            .reading
            .lock()
            .map_err(|_| "Reader service unavailable")?
            .cached
            .as_ref()
            .is_none_or(|(when, _)| when.elapsed() >= Duration::from_millis(250));
        if due {
            self.poll()?;
        }
        let projected = self.catalog(|c| {
            let stamp = json!([
                self.analysis_generation.load(Ordering::Acquire),
                self.app("app_overlay_stamp", &json!({}))?
            ]);
            let mut cache = self.hud_cache.lock().map_err(|_| "HUD cache unavailable")?;
            if let Some((key, value)) = cache.as_ref()
                && *key == stamp
            {
                return Ok(value.clone());
            }
            let packet = self.app("app_overlay_packet", &json!({}))?;
            let index = c.import_index()?;
            let value = self
                .hud
                .lock()
                .map_err(|_| "HUD projection unavailable")?
                .project(
                    &packet,
                    &index,
                    self.analysis_generation.load(Ordering::Acquire),
                )?;
            *cache = Some((stamp, value.clone()));
            Ok(value)
        });
        match projected {
            Ok(value) => Ok(value),
            Err(error) => Ok(self
                .hud
                .lock()
                .map_err(|_| "HUD projection unavailable")?
                .unavailable(&error)),
        }
    }
    fn overlay(&self, request: &Request) -> Result<Value> {
        let input = request.json_fields(&[
            "schema_version",
            "kind",
            "token",
            "guide_id",
            "view",
            "version",
            "refresh_guides",
        ])?;
        if input["schema_version"] != 1 || input["token"].as_u64().is_none() {
            return Err("Versione o token overlay non supportati".into());
        }
        let kind = input["kind"]
            .as_str()
            .ok_or("Richiesta overlay non valida")?;
        let mut result = json!({"schema_version":1,"kind":kind,"token":input["token"],"ok":true});
        if kind == "close" {
            result["usable"] = json!(false);
            result["matches"] = json!([]);
            return Ok(result);
        }
        if kind == "select" {
            let guide = input["guide_id"].as_str().ok_or("Guida non installata")?;
            self.app("app_active", &json!({"update":{"guide_id":guide}}))?;
            self.invalidate()?;
        } else if !["snapshot", "observe", "geometry"].contains(&kind) {
            return Err("Richiesta overlay non supportata".into());
        }
        let snapshot = self.overlay_snapshot()?;
        result
            .as_object_mut()
            .unwrap()
            .extend(snapshot.as_object().unwrap().clone());
        if ["snapshot", "observe", "select"].contains(&kind) {
            let guides = self.app("app_guides", &json!({}))?;
            result["guides"] = json!(
                crate::hud_geometry::rows(&guides)
                    .iter()
                    .map(|g| json!({"id":g["id"],"title":g["title"]}))
                    .collect::<Vec<_>>()
            );
        }
        if kind != "geometry" && kind != "observe" {
            return Ok(result);
        }
        let view = &input["view"];
        if view
            .as_array()
            .is_none_or(|v| v.len() != 6 || v.iter().any(|n| n.as_i64().is_none()))
            || view[0].as_u64().is_none_or(|v| v == 0)
            || view[1].as_u64().is_none_or(|v| v == 0)
            || [4, 5]
                .iter()
                .any(|i| view[*i].as_u64().is_none_or(|v| v == 0 || v > 8192))
            || view[4]
                .as_u64()
                .unwrap()
                .saturating_mul(view[5].as_u64().unwrap())
                .saturating_mul(4)
                > 80_000_000
        {
            return Err("Identità finestra overlay non valida".into());
        }
        result["view"] = view.clone();
        result["native_available"] = json!(false);
        result["matches"] = json!([]);
        if snapshot["usable"] != true
            || kind == "geometry" && input["version"] != snapshot["version"]
        {
            result["usable"] = json!(false);
            return Ok(result);
        }
        let began = Instant::now();
        let geometry = {
            let provider = self
                .provider
                .lock()
                .map_err(|_| "Provider service unavailable")?;
            provider
                .as_ref()
                .ok_or("Game installation not configured".into())
                .and_then(|p| {
                    crate::request(
                        0,
                        "provider_geometry",
                        json!({"handle":p.handle,"view":view,"proposals":snapshot["proposed"]}),
                    )
                })
        };
        let current = self.overlay_snapshot()?;
        let usable = current["usable"] == true && current["version"] == snapshot["version"];
        result
            .as_object_mut()
            .unwrap()
            .extend(current.as_object().unwrap().clone());
        result["usable"] = json!(usable);
        result["geometry_ms"] = json!(began.elapsed().as_secs_f64() * 1000.);
        result["memory_access"] = json!(true);
        match geometry {
            Ok(value) => {
                result["native_available"] = value["available"].clone();
                result["matches"] = if usable {
                    value["matches"].clone()
                } else {
                    json!([])
                };
                for key in [
                    "handled_panels",
                    "diagnostics",
                    "active_panel",
                    "suspend_visual",
                    "explanations",
                ] {
                    if let Some(v) = value.get(key) {
                        result[key] = v.clone();
                    }
                }
                result["native_error"] = value.get("error").cloned().unwrap_or(json!(""));
            }
            Err(error) => {
                result["native_error"] = json!(error);
                result["suspend_visual"] = json!(true);
            }
        }
        Ok(result)
    }
    fn refresh_guide_context(&self, library: Value) -> Result<()> {
        let (context, generation) = self.catalog(|c| {
            Ok((
                crate::runtime_context::Context::new(c, library)?,
                self.analysis_generation.load(Ordering::Acquire),
            ))
        })?;
        let input = self
            .application
            .lock()
            .map_err(|_| "Application unavailable")?
            .context_input()?;
        let prepared = Application::prepare_context(&input, context)?;
        self.catalog(|_| {
            if self.analysis_generation.load(Ordering::Acquire) != generation {
                return Err(
                    "Cataloghi ricaricati durante la compilazione: ripeti il controllo".into(),
                );
            }
            let mut app = self
                .application
                .lock()
                .map_err(|_| "Application unavailable")?;
            app.check_context(&prepared)?;
            app.apply_context(prepared);
            Ok(())
        })?;
        self.invalidate()
    }
    fn media_step(&self, request: &Request) -> Result<(Value, Value)> {
        let guide = self.app(
            "app_guide",
            &json!({"guide_id":request.query.get("id").map(String::as_str).unwrap_or("")}),
        )?;
        let key = request.query.get("key").map(String::as_str).unwrap_or("");
        let step = guide["tracks"]
            .as_object()
            .into_iter()
            .flat_map(|m| m.values())
            .flat_map(|v| v.as_array().into_iter().flatten())
            .find(|r| r["key"] == key)
            .cloned()
            .ok_or("Istruzione non trovata")?;
        Ok((guide, step))
    }
    fn media_file(&self, request: &Request) -> Result<PathBuf> {
        if request.path == "/api/import/frame" {
            return self.frames.preview(
                request.query.get("url").map(String::as_str).unwrap_or(""),
                request
                    .query
                    .get("seconds")
                    .and_then(|v| v.parse::<f64>().ok())
                    .ok_or("Minutaggio non valido")?,
                false,
            );
        }
        let (guide, step) = self.media_step(request)?;
        let reference = if let Some(id) = request.query.get("ref") {
            crate::media_references::references(&guide, &step)["references"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|r| r["id"] == id.as_str())
                .cloned()
                .unwrap_or(Value::Null)
        } else {
            crate::media_references::frame(&guide, &step)
        };
        if reference.is_null() {
            return Err(
                "Questa istruzione non contiene un fotogramma temporizzato della fonte".into(),
            );
        }
        self.frames.preview(
            reference["url"].as_str().ok_or("Missing source URL")?,
            reference["seconds"]
                .as_f64()
                .ok_or("Minutaggio non valido")?,
            request.path.ends_with("/clip"),
        )
    }
    fn script_analysis(&self, refresh: bool) -> Result<Value> {
        let mut cached = self
            .analysis_cache
            .lock()
            .map_err(|_| "Analysis unavailable")?;
        let (generation, demo, game, mods, labels, reference) = self.catalog(|c| {
            Ok((
                self.analysis_generation.load(Ordering::Acquire),
                c.sources["demo"] == true,
                c.sources["game_root"].clone(),
                c.sources["mod_roots"].clone(),
                c.sources["labels"].clone(),
                c.sources["reference_labels"].clone(),
            ))
        })?;
        if !refresh
            && let Some((previous, report)) = &*cached
            && *previous == generation
        {
            return Ok(report.clone());
        }
        let report = if demo {
            let mut book = self.rule_book.lock().map_err(|_| "Rules unavailable")?;
            if book.is_none() {
                *book = Some(crate::script_analysis_service::rulebook(&self.rules_root)?);
            }
            let documents = json!([[
                "common/national_focus/demo.txt",
                crate::script_document::read(&self.demo_file)?
            ]]);
            let mut report = crate::script_subset::analyze(
                &documents,
                &labels,
                &reference,
                book.as_ref().unwrap(),
                &json!([]),
                2000,
            )?;
            report["demo"] = json!(true);
            report
        } else {
            let mods = mods
                .as_array()
                .ok_or("Missing mod roots")?
                .iter()
                .map(|v| v.as_str().map(PathBuf::from).ok_or("Invalid mod root"))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let manifest = crate::script_analysis_service::files(
                std::path::Path::new(game.as_str().ok_or("Missing game installation")?),
                &mods,
            )?;
            crate::script_analysis_service::analyze(
                &manifest,
                &self.rules_root,
                &self.cwtools_host,
                &self.parser_library,
            )?
        };
        // Analysis owns no live-reader/application/catalog lock while CWTools runs.
        // Publish only against the unchanged source generation.
        self.catalog(|_| {
            if self.analysis_generation.load(Ordering::Acquire) != generation {
                return Err("Cataloghi ricaricati durante l’analisi: ripeti il controllo".into());
            }
            let report = crate::script_analysis_service::record_errors(
                &report,
                &self.source_error_log,
                &timestamp(),
            );
            *cached = Some((generation, report.clone()));
            Ok(report)
        })
    }
    fn associations(&self, guide_id: Option<&str>, update: Option<&Value>) -> Result<Value> {
        let mut id = guide_id;
        let index = if let Some(update) = update {
            let fields = update
                .as_object()
                .ok_or("Correzione associazione non valida")?;
            if fields.len() != 4
                || fields
                    .keys()
                    .any(|k| !["guide_id", "key", "expected", "entity"].contains(&k.as_str()))
            {
                return Err("Correzione associazione non valida".into());
            }
            id = Some(update["guide_id"].as_str().ok_or("ID guida non valido")?);
            Some(self.catalog(Catalog::import_index)?)
        } else {
            None
        };
        // Keep correction validation, publication and registry replacement serialized.
        // No catalog, reader or SQLite lock is held during route solving.
        let mut app = self
            .application
            .lock()
            .map_err(|_| "Application unavailable")?;
        let source = app.request("app_source", &json!({"guide_id":id}))?;
        let id = source["id"].as_str().ok_or("ID guida non valido")?;
        if let Some(update) = update {
            crate::guide_repository::change(
                &source,
                &update["key"],
                &update["expected"],
                &update["entity"],
                index.as_ref().ok_or("Catalog unavailable")?,
                &self.association_directory,
            )?;
            app.put_guide(&source)?;
            self.association_revision.fetch_add(1, Ordering::AcqRel);
        }
        let result = json!({"guide":app.request("app_guide",&json!({"guide_id":id}))?,
            "changes":crate::guide_repository::associations(id,&self.association_directory)?["changes"],
            "revision":self.association_revision.load(Ordering::Acquire)});
        drop(app);
        if update.is_some() {
            self.invalidate()?;
        }
        Ok(result)
    }
    fn catalog<T>(&self, work: impl FnOnce(&mut Catalog) -> Result<T>) -> Result<T> {
        let mut owner = self.catalog.lock().map_err(|_| "Catalog unavailable")?;
        work(owner.as_mut().ok_or("Catalog not configured")?)
    }
    fn sprite(&self, token: &str) -> Result<Option<Vec<u8>>> {
        self.catalog(|c| match &mut c.assets {
            Some(a) => a
                .png_file(token)?
                .map(fs::read)
                .transpose()
                .map_err(|e| e.to_string()),
            None => Ok(None),
        })
    }
    fn invalidate(&self) -> Result<()> {
        self.reading
            .lock()
            .map_err(|_| "Reader unavailable")?
            .cached = None;
        self.wake.notify_all();
        Ok(())
    }
    fn language(&self) -> String {
        self.ui_language
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| "english".into())
    }
    fn set_settings(&self, update: &Value) -> Result<Value> {
        self.configure(update, true, false)
    }
    fn configure(&self, update: &Value, persist: bool, force_reload: bool) -> Result<Value> {
        let valid = [
            "game_dir",
            "docs_dir",
            "tree_id",
            "snapshot_path",
            "country",
            "language",
            "ui_language",
            "mods",
        ];
        let map = update
            .as_object()
            .filter(|m| m.keys().all(|k| valid.contains(&k.as_str())))
            .ok_or("Impostazioni non riconosciute")?;
        if map
            .values()
            .any(|v| !v.as_str().is_some_and(|s| s.chars().count() <= 2048))
        {
            return Err("Le impostazioni devono contenere solo stringhe".into());
        }
        if map.get("country").is_some_and(|v| {
            !v.as_str().is_some_and(|s| {
                s.chars().count() == 3 && s.chars().all(hoi4companion_catalog_core::unicode_alpha)
            })
        }) {
            return Err("Codice paese non valido (esempio AUS)".into());
        }
        for (key, allowed) in [
            ("language", ["italian", "english"]),
            ("ui_language", ["english", "italian"]),
            ("mods", ["auto", "off"]),
        ] {
            if map
                .get(key)
                .is_some_and(|v| !allowed.contains(&v.as_str().unwrap_or("")))
            {
                return Err(format!("Impostazione non valida: {key}"));
            }
        }
        let mut owner = self.catalog.lock().map_err(|_| "Catalog unavailable")?;
        let catalog = owner.as_mut().ok_or("Catalog not configured")?;
        let mut args = self
            .catalog_args
            .lock()
            .map_err(|_| "Catalog configuration unavailable")?;
        let args = args.as_mut().ok_or("Catalog not configured")?;
        let mut settings = catalog.sources["settings"].clone();
        settings.as_object_mut().unwrap().extend(map.clone());
        let reload = force_reload || map.keys().any(|k| k != "ui_language");
        let mut next_args = args.clone();
        next_args["settings"] = settings.clone();
        let mut replacement = if reload {
            let value = Catalog::open(&next_args)?;
            settings = value.sources["settings"].clone();
            Some(value)
        } else {
            None
        };
        let prepared_context = if let Some(next) = replacement.as_mut() {
            let context = crate::runtime_context::Context::new(
                next,
                crate::reference_library::manifest(&self.references),
            )?;
            let input = self
                .application
                .lock()
                .map_err(|_| "Application unavailable")?
                .context_input()?;
            Some(Application::prepare_context(&input, context)?)
        } else {
            None
        };
        let mut reading = self
            .reading
            .lock()
            .map_err(|_| "Reader service unavailable")?;
        let mut provider = self
            .provider
            .lock()
            .map_err(|_| "Provider service unavailable")?;
        let next_provider = if reload {
            if replacement
                .as_ref()
                .is_some_and(|c| c.sources["demo"] != true)
            {
                let mut spec=provider.as_ref().map(|old|old.args.clone()).unwrap_or_else(||json!({"manifest_sha256":format!("{:x}",sha2::Sha256::digest(include_bytes!("../../../src/hoi4companion/live-profiles-v1.json"))),"audit_seconds":5}));
                spec["path"] = json!(
                    PathBuf::from(settings["game_dir"].as_str().unwrap_or(""))
                        .join("hoi4.exe")
                        .to_string_lossy()
                );
                let source = &replacement.as_ref().unwrap().sources;
                let mods: Vec<_> = source["mod_roots"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(PathBuf::from)
                    .collect();
                spec["province_neighbors"] = if source["demo"] == true {
                    Value::Null
                } else {
                    crate::map_topology::neighbors(
                        std::path::Path::new(settings["game_dir"].as_str().unwrap_or("")),
                        &mods,
                    )?
                };
                Some(ProviderOwner::open(spec)?)
            } else {
                None
            }
        } else {
            None
        };
        let mut application = if reload {
            Some(
                self.application
                    .lock()
                    .map_err(|_| "Application unavailable")?,
            )
        } else {
            None
        };
        if let (Some(app), Some(prepared)) = (&application, &prepared_context) {
            app.check_context(prepared)?;
        }
        if let Some(path) = args["settings_path"].as_str().filter(|_| persist) {
            let path = PathBuf::from(path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let temporary =
                path.with_extension(format!("{}.tmp", crate::progress_store::new_id()?));
            let write = (|| {
                let mut file = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&temporary)
                    .map_err(|e| e.to_string())?;
                file.write_all(
                    serde_json::to_string_pretty(&settings)
                        .map_err(|e| e.to_string())?
                        .as_bytes(),
                )
                .map_err(|e| e.to_string())?;
                file.sync_all().map_err(|e| e.to_string())?;
                drop(file);
                fs::rename(&temporary, &path).map_err(|e| e.to_string())
            })();
            if let Err(e) = write {
                let _ = fs::remove_file(temporary);
                return Err(e);
            }
        }
        args["settings"] = settings.clone();
        *self
            .ui_language
            .lock()
            .map_err(|_| "UI language unavailable")? = settings["ui_language"]
            .as_str()
            .unwrap_or("english")
            .to_owned();
        if let Some(replacement) = replacement {
            self.analysis_generation.fetch_add(1, Ordering::AcqRel);
            *catalog = replacement;
            *provider = next_provider;
            reading.monitor = Monitor::default();
            reading.cached = None;
            if let (Some(app), Some(prepared)) = (&mut application, prepared_context) {
                app.apply_context(prepared);
                app.invalidate_observation();
            }
            self.wake.notify_all();
        } else {
            Arc::make_mut(&mut catalog.sources)["settings"] = settings;
        }
        Ok(catalog.overview())
    }
    fn stream(&self, socket: &mut TcpStream) -> Result<()> {
        let id = self
            .hub
            .subscribe(self.now())?
            .as_u64()
            .ok_or("Invalid stream subscription")?;
        struct Subscription<'a> {
            service: &'a Service,
            id: u64,
        }
        impl Drop for Subscription<'_> {
            fn drop(&mut self) {
                let _ = self.service.hub.unsubscribe(self.id);
                if let Ok(mut clients) = self.service.clients.lock() {
                    *clients = clients.saturating_sub(1);
                    self.service.wake.notify_all();
                }
            }
        }
        {
            let mut clients = self.clients.lock().map_err(|_| "Stream unavailable")?;
            *clients += 1;
            self.wake.notify_all();
        }
        let _subscription = Subscription { service: self, id };
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream; charset=utf-8\r\nCache-Control: no-cache, no-transform\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\nretry: 1000\n\n").map_err(|e|e.to_string())?;
        while !self.stopped.load(Ordering::Acquire) {
            let event = self.hub.wait(id, 5., self.now())?;
            let frame = if event.is_null() {
                format!(
                    "event: heartbeat\ndata: {}\n\n",
                    self.hub.heartbeat(self.now())?
                )
            } else {
                format!(
                    "id: {}\nevent: state\ndata: {}\n\n",
                    event["revision"],
                    crate::i18n::payload(&event["value"], &self.language())
                )
            };
            socket
                .write_all(frame.as_bytes())
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    fn static_file(&self, path: &str) -> Option<(Vec<u8>, &'static str)> {
        let name = match path {
            "/" => "index.html",
            "/walkthrough" | "/hud" => "guide.html",
            "/technologies" => "technologies.html",
            "/importer" => "importer.html",
            "/country" => "country.html",
            "/analysis" => "analysis.html",
            "/associations" => "associations.html",
            other => other.strip_prefix('/')?,
        };
        if name.contains(['/', '\\', ':']) || name.starts_with('.') {
            return None;
        }
        let mime = if name.ends_with(".html") {
            "text/html; charset=utf-8"
        } else if name.ends_with(".js") {
            "text/javascript; charset=utf-8"
        } else if name.ends_with(".css") {
            "text/css; charset=utf-8"
        } else {
            return None;
        };
        let file = self.web.join(name).canonicalize().ok()?;
        if !file.starts_with(&self.web) || fs::metadata(&file).ok()?.len() > 16 * 1024 * 1024 {
            return None;
        }
        let mut data = fs::read(file).ok()?;
        if name.ends_with(".html") {
            data = crate::i18n::html(std::str::from_utf8(&data).ok()?, &self.language())
                .ok()?
                .into_bytes();
        } else if name == "i18n.js" {
            data = [
                format!("globalThis.HOI_TRANSLATIONS={};\n", crate::i18n::catalog()).into_bytes(),
                data,
            ]
            .concat();
        }
        Some((data, mime))
    }
    pub fn close(&self) {
        self.videos.close();
        self.stopped.store(true, Ordering::Release);
        let _ = self.hub.close();
        self.wake.notify_all();
    }
}
struct Request {
    method: String,
    path: String,
    query: HashMap<String, String>,
    body: Vec<u8>,
    upload: Option<Upload>,
    range: Option<String>,
}
impl Request {
    fn json_fields(&self, allowed: &[&str]) -> Result<Value> {
        let value = self.json()?;
        let fields = value
            .as_object()
            .ok_or("Richiesta importazione non valida")?;
        if fields.keys().any(|k| !allowed.contains(&k.as_str())) {
            return Err("Campi importazione non riconosciuti".into());
        }
        Ok(value)
    }
    fn json(&self) -> Result<Value> {
        serde_json::from_slice(&self.body).map_err(|e| e.to_string())
    }
}
fn decode(value: &str) -> Result<String> {
    let bytes = value.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                if i + 2 >= bytes.len() {
                    return Err("Invalid URL escape".into());
                }
                let pair = std::str::from_utf8(&bytes[i + 1..i + 3]).map_err(|e| e.to_string())?;
                out.push(u8::from_str_radix(pair, 16).map_err(|e| e.to_string())?);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|e| e.to_string())
}
fn parse(socket: &mut TcpStream, port: u16) -> std::result::Result<Request, (u16, String)> {
    let fail = |text: &str| (400, text.to_owned());
    let mut data = Vec::new();
    let mut reader = BufReader::new(socket);
    while !data.ends_with(b"\r\n\r\n") {
        if data.len() >= 32768 {
            return Err((431, "Request headers exceed limit".into()));
        }
        let count = reader
            .by_ref()
            .take((32768 - data.len()) as u64)
            .read_until(b'\n', &mut data)
            .map_err(|_| fail("Incomplete request headers"))?;
        if count == 0 {
            return Err(fail("Incomplete request headers"));
        }
    }
    let head = std::str::from_utf8(&data).map_err(|_| fail("Invalid request headers"))?;
    let mut lines = head.split("\r\n");
    let line = lines.next().ok_or_else(|| fail("Missing request line"))?;
    let fields: Vec<_> = line.split(' ').collect();
    if fields.len() != 3
        || !["HTTP/1.0", "HTTP/1.1"].contains(&fields[2])
        || !fields[1].starts_with('/')
        || fields[1].starts_with("//")
    {
        return Err(fail("Invalid request line"));
    }
    let mut headers = HashMap::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let (key, value) = line.split_once(':').ok_or_else(|| fail("Invalid header"))?;
        if key.is_empty()
            || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || value.bytes().any(|b| b < 32 && b != 9)
        {
            return Err(fail("Invalid header"));
        }
        if headers
            .insert(key.to_ascii_lowercase(), value.trim().to_owned())
            .is_some()
        {
            return Err(fail("Duplicate request header"));
        }
    }
    let host = headers.get("host").ok_or_else(|| fail("Missing host"))?;
    if ![format!("127.0.0.1:{port}"), format!("localhost:{port}")].contains(host) {
        return Err((403, "Unauthorized host".into()));
    }
    let (path, query) = fields[1].split_once('?').unwrap_or((fields[1], ""));
    let path = decode(path).map_err(|e| (400, e))?;
    let mut params = HashMap::new();
    for part in query.split('&').filter(|s| !s.is_empty()) {
        let (k, v) = part.split_once('=').unwrap_or((part, ""));
        let key = decode(k).map_err(|e| (400, e))?;
        let value = decode(v).map_err(|e| (400, e))?;
        if params.insert(key, value).is_some() {
            return Err(fail("Duplicate query parameter"));
        }
    }
    if (fields[0] == "POST" || path == "/api/state/events")
        && (headers
            .get("sec-fetch-site")
            .is_some_and(|v| v == "cross-site")
            || headers
                .get("origin")
                .is_some_and(|origin| origin != &format!("http://{host}")))
    {
        return Err((403, "Unauthorized origin".into()));
    }
    if headers.contains_key("transfer-encoding") {
        return Err(fail("Transfer encoding not supported"));
    }
    let length = match headers.get("content-length") {
        Some(v) if v.bytes().all(|b| b.is_ascii_digit()) && !v.is_empty() => v
            .parse::<usize>()
            .map_err(|_| fail("Invalid content length"))?,
        Some(_) => return Err(fail("Invalid content length")),
        None => 0,
    };
    let binary = fields[0] == "POST"
        && ["/api/import/word", "/api/import/templates"].contains(&path.as_str());
    if length > MAX_BODY && !binary {
        return Err((413, "Request body exceeds limit".into()));
    }
    if fields[0] == "POST"
        && headers
            .get("content-type")
            .map(|v| v.split(';').next().unwrap_or("").trim())
            != Some(if binary {
                "application/octet-stream"
            } else {
                "application/json"
            })
    {
        return Err((415, "JSON request required".into()));
    }
    let upload = if binary {
        Some(Upload::receive(&mut reader, length)?)
    } else {
        None
    };
    let mut body = if binary { vec![] } else { vec![0; length] };
    if !binary {
        reader
            .read_exact(&mut body)
            .map_err(|_| fail("Incomplete request body"))?;
    }
    Ok(Request {
        method: fields[0].into(),
        path,
        query: params,
        body,
        upload,
        range: headers.get("range").cloned(),
    })
}
fn response(socket: &mut TcpStream, status: u16, mime: &str, data: &[u8]) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        413 => "Content Too Large",
        415 => "Unsupported Media Type",
        431 => "Request Header Fields Too Large",
        503 => "Service Unavailable",
        _ => "Error",
    };
    write!(
        socket,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nX-Content-Type-Options: nosniff\r\nCache-Control: no-store\r\nContent-Security-Policy: {CSP}\r\nConnection: close\r\n\r\n",
        data.len()
    )?;
    socket.write_all(data)
}
fn send_download(
    socket: &mut TcpStream,
    path: &std::path::Path,
    name: &str,
) -> std::io::Result<()> {
    let mime = match path.extension().and_then(|v| v.to_str()).unwrap_or("") {
        "mp4" => "video/mp4",
        "m4a" => "audio/mp4",
        "mp3" => "audio/mpeg",
        "opus" => "audio/ogg",
        "json" => "application/json",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    };
    let mut file = fs::File::open(path)?;
    let size = file.metadata()?.len();
    write!(
        socket,
        "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {size}\r\nContent-Disposition: attachment; filename=\"{name}\"\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n"
    )?;
    std::io::copy(&mut file, socket)?;
    Ok(())
}
fn send_media_range(
    socket: &mut TcpStream,
    path: &std::path::Path,
    range: Option<&str>,
) -> std::io::Result<()> {
    use std::io::{Seek, SeekFrom};
    let mut file = fs::File::open(path)?;
    let size = file.metadata()?.len();
    let mut start = 0;
    let mut end = size.saturating_sub(1);
    let mut valid = size > 0;
    if let Some(header) = range {
        let pattern = crate::import_catalog::regex(r"^bytes=(\d*)-(\d*)$");
        if let Some(parts) = pattern.captures(header.trim()) {
            if parts[1].is_empty() {
                if let Ok(suffix) = parts[2].parse::<u64>() {
                    start = size.saturating_sub(suffix)
                } else {
                    valid = false
                }
            } else if let Ok(value) = parts[1].parse::<u64>() {
                start = value;
                if !parts[2].is_empty() {
                    if let Ok(value) = parts[2].parse::<u64>() {
                        end = end.min(value)
                    } else {
                        valid = false
                    }
                }
            } else {
                valid = false
            }
        } else {
            valid = false
        }
        valid &= start < size && start <= end;
    }
    if !valid {
        return write!(
            socket,
            "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{size}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
    }
    let (status, reason) = if range.is_some() {
        (206, "Partial Content")
    } else {
        (200, "OK")
    };
    write!(
        socket,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: video/mp4\r\nAccept-Ranges: bytes\r\nContent-Length: {}\r\nX-Content-Type-Options: nosniff\r\nCache-Control: no-store\r\nConnection: close\r\n",
        end - start + 1
    )?;
    if range.is_some() {
        write!(socket, "Content-Range: bytes {start}-{end}/{size}\r\n")?;
    }
    write!(socket, "\r\n")?;
    file.seek(SeekFrom::Start(start))?;
    std::io::copy(&mut file.take(end - start + 1), socket)?;
    Ok(())
}
fn json_response(socket: &mut TcpStream, status: u16, value: &Value) {
    let _ = response(
        socket,
        status,
        "application/json; charset=utf-8",
        value.to_string().as_bytes(),
    );
}
fn connection(service: &Service, mut socket: TcpStream, port: u16) {
    let _ = socket.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = socket.set_write_timeout(Some(Duration::from_secs(10)));
    let request = match parse(&mut socket, port) {
        Ok(r) => r,
        Err((status, error)) => {
            // Small rejected browser POSTs are drained only within a strict bound;
            // otherwise Windows can reset the socket before delivering the error.
            let _ = socket.set_read_timeout(Some(Duration::from_millis(50)));
            let mut discard = [0u8; 16384];
            let _ = socket.read(&mut discard);
            json_response(&mut socket, status, &json!({"error":error}));
            return;
        }
    };
    if request.method == "GET"
        && ["/api/guide/frame", "/api/guide/clip", "/api/import/frame"]
            .contains(&request.path.as_str())
    {
        match service.media_file(&request) {
            Ok(path) => {
                if request.path.ends_with("/clip") {
                    let _ = send_media_range(&mut socket, &path, request.range.as_deref());
                } else {
                    match fs::read(&path) {
                        Ok(bytes) => {
                            let _ = response(&mut socket, 200, "image/jpeg", &bytes);
                        }
                        Err(error) => {
                            json_response(&mut socket, 404, &json!({"error":error.to_string()}))
                        }
                    }
                }
            }
            Err(error) => json_response(&mut socket, 404, &json!({"error":error})),
        }
        return;
    }
    if request.method == "GET" && request.path == "/api/import/video/file" {
        let id = request.query.get("id").map(String::as_str).unwrap_or("");
        let name = request.query.get("name").map(String::as_str).unwrap_or("");
        match service.videos.file(id, name) {
            Ok(path) => {
                let _ = send_download(&mut socket, &path, name);
            }
            Err(error) => json_response(&mut socket, 400, &json!({"error":error})),
        }
        return;
    }
    if request.method == "GET" && request.path == "/api/state/events" {
        let _ = service.stream(&mut socket);
        return;
    }
    if request.method == "GET" && request.path == "/api/reference-library/image" {
        match crate::reference_library::image(
            &service.references,
            request.query.get("id").map(String::as_str).unwrap_or(""),
        ) {
            Ok((bytes, mime)) => {
                let _ = response(&mut socket, 200, &mime, &bytes);
            }
            Err(error) => json_response(&mut socket, 404, &json!({"error":error})),
        }
        return;
    }
    if request.method == "GET"
        && let Some(token) = request
            .path
            .strip_prefix("/api/sprite/")
            .and_then(|s| s.strip_suffix(".png"))
    {
        match service.sprite(token) {
            Ok(Some(data)) => {
                let _ = response(&mut socket, 200, "image/png", &data);
            }
            _ => json_response(&mut socket, 404, &json!({"error":"Sprite not found"})),
        }
        return;
    }
    if request.method == "GET" && !request.path.starts_with("/api/") {
        if let Some((data, mime)) = service.static_file(&request.path) {
            let _ = response(&mut socket, 200, mime, &data);
        } else {
            json_response(&mut socket, 404, &json!({"error":"Not found"}));
        }
        return;
    }
    match service.dispatch(&request) {
        Ok(value) => json_response(
            &mut socket,
            200,
            &crate::i18n::payload(&value, &service.language()),
        ),
        Err(error) => json_response(
            &mut socket,
            if error == "Route not implemented in the native host" {
                404
            } else if request.path == "/api/script-analysis"
                && (error.starts_with("CWTools")
                    || error.starts_with("Versione host CWTools")
                    || error.starts_with("Contratto motore CWTools"))
            {
                503
            } else {
                400
            },
            &crate::i18n::payload(&json!({"error":error}), &service.language()),
        ),
    }
}
pub fn serve(config: Value, port: u16, ready: Option<PathBuf>) -> Result<()> {
    let service = Service::open(&config)?;
    let listener =
        TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let info =
        json!({"engine":"rust-backend","host":"127.0.0.1","port":port,"pid":std::process::id()});
    if let Some(path) = ready {
        fs::write(path, info.to_string()).map_err(|e| e.to_string())?;
    }
    println!("{info}");
    let active = Arc::new(AtomicUsize::new(0));
    while !service.stopped.load(Ordering::Acquire) {
        let mut socket = match listener.accept() {
            Ok((socket, _)) => socket,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(20));
                continue;
            }
            Err(e) => return Err(e.to_string()),
        };
        // Windows can inherit FIONBIO from the listening socket. Only accept
        // is nonblocking; bounded per-request reads must wait for body bytes.
        socket.set_nonblocking(false).map_err(|e| e.to_string())?;
        if active
            .try_update(Ordering::AcqRel, Ordering::Relaxed, |n| {
                (n < 32).then_some(n + 1)
            })
            .is_err()
        {
            json_response(
                &mut socket,
                503,
                &json!({"error":"Too many local connections"}),
            );
            continue;
        }
        let owner = service.clone();
        let count = active.clone();
        if thread::Builder::new()
            .name("hoi4-native-http".into())
            .spawn(move || {
                struct Release(Arc<AtomicUsize>);
                impl Drop for Release {
                    fn drop(&mut self) {
                        self.0.fetch_sub(1, Ordering::AcqRel);
                    }
                }
                let _release = Release(count);
                connection(&owner, socket, port);
            })
            .is_err()
        {
            active.fetch_sub(1, Ordering::AcqRel);
        }
    }
    service.close();
    Ok(())
}
