//! Owned video preparation. Immutable source hashes, bounded jobs and identical
//! preset/overlap metadata; utilities exist only during their own jobs.
use crate::{
    guide_repository, media_frames::Frames, media_references, process_tools, reference_library,
    source_acquisition,
};
use hoi4companion_tools::OwnedTool;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
type Result<T> = std::result::Result<T, String>;
pub const MAX_PART: u64 = 150_000_000;
pub const OVERLAP: f64 = 10.;
pub const SEGMENT: f64 = 300.;
const KEYFRAME: f64 = 10.;
const PRESET: u64 = 4;
fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn a(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| (*v).into()).collect()
}
fn display(path: &Path) -> String {
    crate::installation::display(path)
}
pub fn profile() -> Value {
    json!({"version":1,"height_limit":1080,"fps":15,"codec":"hevc","quality":22,"audio":"aac-mono-64k","keyframe_seconds":10,"encoder_selection":"sample-size-hardware-within-25-percent"})
}
pub fn digest(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut bytes = [0u8; 65536];
    loop {
        let n = file.read(&mut bytes).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&bytes[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn cpu() -> Vec<String> {
    args(&[
        "-c:v",
        "libx265",
        "-preset",
        "medium",
        "-crf",
        "22",
        "-x265-params",
        "pools=4:frame-threads=2:log-level=error:open-gop=0:keyint=150:min-keyint=150:scenecut=0",
    ])
}
fn encoders() -> Vec<(&'static str, Vec<String>)> {
    vec![
        (
            "NVIDIA NVENC",
            args(&[
                "-c:v",
                "hevc_nvenc",
                "-preset",
                "p6",
                "-rc",
                "vbr",
                "-cq",
                "22",
                "-b:v",
                "0",
                "-g",
                "150",
                "-bf",
                "3",
                "-forced-idr",
                "1",
            ]),
        ),
        (
            "Intel QSV",
            args(&[
                "-c:v",
                "hevc_qsv",
                "-preset",
                "medium",
                "-global_quality",
                "22",
                "-g",
                "150",
                "-flags",
                "+cgop",
            ]),
        ),
        (
            "AMD AMF",
            args(&[
                "-c:v", "hevc_amf", "-quality", "quality", "-rc", "cqp", "-qp_i", "22", "-qp_p",
                "22", "-g", "150",
            ]),
        ),
        ("CPU x265", cpu()),
    ]
}
fn media_info(text: &str) -> Result<Value> {
    let re = crate::import_catalog::regex(r"Duration: (\d+):(\d+):(\d+(?:\.\d+)?)");
    let capture = re.captures(text).ok_or("Durata del video non leggibile")?;
    let duration = capture[1].parse::<f64>().unwrap_or(0.) * 3600.
        + capture[2].parse::<f64>().unwrap_or(0.) * 60.
        + capture[3].parse::<f64>().unwrap_or(0.);
    let line = text.lines().find(|l| l.contains("Video:")).unwrap_or("");
    let dims = crate::import_catalog::regex(r"\b(\d{2,5})x(\d{2,5})\b").captures(line);
    let audio = crate::import_catalog::regex(r"Audio:\s*([^\s,(]+)")
        .captures(text)
        .map(|c| json!(&c[1]))
        .unwrap_or(Value::Null);
    Ok(
        json!({"duration_seconds":duration,"width":dims.as_ref().and_then(|c|c[1].parse::<u64>().ok()),"height":dims.as_ref().and_then(|c|c[2].parse::<u64>().ok()),"audio_codec":audio}),
    )
}
fn identity(source: &Path) -> Result<(String, Value)> {
    let identity = json!({"source_sha256":digest(source)?,"profile":profile()});
    let key = format!(
        "{:x}",
        Sha256::digest(reference_library::legacy_json(&identity).as_bytes())
    );
    Ok((key, identity))
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
pub struct Jobs {
    pub root: PathBuf,
    frames: Arc<Frames>,
    package: PathBuf,
    jobs: Mutex<HashMap<String, Value>>,
    tools: Mutex<HashMap<String, Arc<OwnedTool>>>,
    admission: Mutex<()>,
    closed: AtomicBool,
}
impl Jobs {
    pub fn new(root: PathBuf, frames: Arc<Frames>, package: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            root,
            frames,
            package,
            jobs: Mutex::new(HashMap::new()),
            tools: Mutex::new(HashMap::new()),
            admission: Mutex::new(()),
            closed: AtomicBool::new(false),
        })
    }
    fn ffmpeg(&self) -> Result<String> {
        self.frames
            .ffmpeg
            .as_ref()
            .map(|p| display(p))
            .ok_or("FFmpeg non trovato: configura HOI4_FFMPEG".into())
    }
    fn update(&self, key: &str, fields: Value) -> Result<()> {
        let mut jobs = self.jobs.lock().map_err(|_| "Video jobs unavailable")?;
        let row = jobs.get_mut(key).ok_or("Preparazione video non trovata")?;
        for (k, v) in fields.as_object().ok_or("Invalid job update")? {
            row[k] = v.clone();
        }
        Ok(())
    }
    fn valid_key(key: &str) -> bool {
        key.len() == 32
            && key
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }
    pub fn status(&self, key: &str) -> Result<Value> {
        if !Self::valid_key(key) {
            return Err("Identificatore video non valido".into());
        }
        let mut jobs = self.jobs.lock().map_err(|_| "Video jobs unavailable")?;
        if let Some(value) = jobs.get(key) {
            return Ok(value.clone());
        }
        let path = self.root.join(key).join("manifest.json");
        let mut value = guide_repository::json_file(&path, 4_000_000)
            .map_err(|_| "Preparazione video non trovata")?;
        if value["preset_version"] != PRESET {
            return Err(
                "Video creato con un preset precedente: preparalo di nuovo dal link".into(),
            );
        }
        for(k,v)in json!({"id":key,"status":"completed","stage":"completed","message":"Video pronto","manifest_url":format!("/api/import/video/file?id={key}&name=manifest.json"),"description_url":format!("/api/import/video/file?id={key}&name=descrizione.txt")}).as_object().unwrap(){value[k]=v.clone();}
        jobs.insert(key.into(), value.clone());
        Ok(value)
    }
    fn execute(
        &self,
        key: &str,
        argv: Vec<String>,
        timeout: u64,
        check: bool,
        encode: bool,
    ) -> Result<String> {
        if self.closed.load(Ordering::Acquire) {
            return Err("Preparazione interrotta: programma chiuso".into());
        }
        let folder = self.root.join(key);
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let token = crate::progress_store::new_id()?;
        let out = Temporary(folder.join(format!("tool-{token}-out.log")));
        let err = Temporary(folder.join(format!("tool-{token}-err.log")));
        let progress = folder.join("encoding-progress.txt");
        if encode {
            std::fs::write(&progress, "").map_err(|e| e.to_string())?;
        }
        let tool = Arc::new(OwnedTool::spawn(
            json!({"schema_version":1,"argv":argv,"stdout":out.0,"stderr":err.0,"max_millis":timeout,"max_output_bytes":64*1024*1024}),
        )?);
        {
            let mut active = self.tools.lock().map_err(|_| "Video tools unavailable")?;
            if self.closed.load(Ordering::Acquire) {
                tool.cancel()?;
                return Err("Preparazione interrotta: programma chiuso".into());
            }
            active.insert(key.into(), tool.clone());
        }
        let mut offset = 0;
        let result = (|| -> Result<String> {
            loop {
                let status = tool.wait(250)?;
                if encode && let Ok(mut file) = std::fs::File::open(&progress) {
                    file.seek(SeekFrom::Start(offset))
                        .map_err(|e| e.to_string())?;
                    let mut chunk = String::new();
                    file.take(1_000_000)
                        .read_to_string(&mut chunk)
                        .map_err(|e| e.to_string())?;
                    offset += chunk.len() as u64;
                    if let Some(time) = crate::import_catalog::regex(r"out_time_us=(\d+)")
                        .captures_iter(&chunk)
                        .last()
                    {
                        let seconds = time[1].parse::<f64>().unwrap_or(0.) / 1e6;
                        self.update(key, json!({"encoded_seconds":(seconds*10.).round()/10.}))?;
                    }
                }
                if self.closed.load(Ordering::Acquire) {
                    return Err("Preparazione interrotta: programma chiuso".into());
                }
                if status["running"] != true {
                    let stdout = std::fs::read_to_string(&out.0).unwrap_or_default();
                    let stderr = std::fs::read_to_string(&err.0).unwrap_or_default();
                    if encode {
                        reference_library::atomic_write(
                            &folder.join("encoding.log"),
                            stderr.as_bytes(),
                        )?;
                    }
                    if status["reason"] == "timeout" {
                        return Err("Strumento video oltre il limite di tempo".into());
                    }
                    if status["reason"] == "output_limit" {
                        return Err("Strumento video oltre il limite di output".into());
                    }
                    if check && status["returncode"] != 0 {
                        return Err(format!(
                            "Strumento video: {}",
                            stderr
                                .chars()
                                .rev()
                                .take(1200)
                                .collect::<Vec<_>>()
                                .into_iter()
                                .rev()
                                .collect::<String>()
                        ));
                    }
                    return Ok(if check { stdout } else { stderr + &stdout });
                }
            }
        })();
        self.tools
            .lock()
            .map_err(|_| "Video tools unavailable")?
            .remove(key);
        drop(tool);
        result
    }
    fn info(&self, key: &str, source: &Path) -> Result<Value> {
        media_info(&self.execute(
            key,
            vec![
                self.ffmpeg()?,
                "-hide_banner".into(),
                "-i".into(),
                display(source),
            ],
            25000,
            false,
            false,
        )?)
    }
    fn duration(&self, key: &str, source: &Path) -> Result<f64> {
        self.info(key, source)?["duration_seconds"]
            .as_f64()
            .ok_or("Durata video non valida".into())
    }
    fn audio(&self, key: &str, source: &Path, info: &Value) -> Result<Value> {
        let codec = s(&info["audio_codec"]);
        if codec.is_empty() {
            self.update(key,json!({"audio_status":"absent","audio_warning":"Il sorgente non contiene una traccia audio."}))?;
            return Ok(Value::Null);
        }
        let suffix = match codec {
            "aac" => "m4a",
            "opus" => "opus",
            "mp3" => "mp3",
            _ => "m4a",
        };
        let copy = ["aac", "opus", "mp3"].contains(&codec);
        let name = format!("audio.{suffix}");
        let target = self.root.join(key).join(&name);
        let temporary = Temporary(self.root.join(key).join(format!("audio.part.{suffix}")));
        let result = (|| -> Result<Value> {
            let mut argv = vec![self.ffmpeg()?];
            argv.extend(args(&["-v", "error", "-nostdin", "-y", "-i"]));
            argv.push(display(source));
            argv.extend(args(&["-vn", "-map", "0:a:0"]));
            argv.extend(if copy {
                args(&["-c:a", "copy"])
            } else {
                args(&["-c:a", "aac", "-b:a", "128k", "-ac", "2"])
            });
            argv.extend(args(&["-map_metadata", "-1"]));
            if suffix == "m4a" {
                argv.extend(args(&["-movflags", "+faststart"]));
            }
            argv.push(display(&temporary.0));
            self.execute(key, argv, 300000, true, false)?;
            std::fs::rename(&temporary.0, &target).map_err(|e| e.to_string())?;
            let audio = json!({"name":name,"bytes":target.metadata().map_err(|e|e.to_string())?.len(),"sha256":digest(&target)?,"codec":if copy{codec}else{"aac"},"transcoded":!copy,"url":format!("/api/import/video/file?id={key}&name={name}")});
            self.update(
                key,
                json!({"audio_status":"completed","audio_warning":null}),
            )?;
            Ok(audio)
        })();
        match result {
            Ok(value) => Ok(value),
            Err(error) => {
                self.update(key,json!({"audio_status":"failed","audio_warning":format!("Audio separato non disponibile: {}",error.chars().take(300).collect::<String>())}))?;
                Ok(Value::Null)
            }
        }
    }
    fn select(
        &self,
        key: &str,
        source: &Path,
        info: &Value,
    ) -> Result<(String, Vec<String>, Value, Value)> {
        let mut unavailable = vec![];
        let mut selected = None;
        for (name, encoder) in encoders() {
            let mut argv = vec![self.ffmpeg()?];
            argv.extend(args(&[
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=size=1280x720:rate=15",
                "-frames:v",
                "3",
            ]));
            argv.extend(encoder.clone());
            argv.extend(args(&["-f", "null", "-"]));
            if self.execute(key, argv, 30000, true, false).is_ok() {
                selected = Some((name.to_owned(), encoder));
                break;
            }
            unavailable.push(json!(name));
        }
        let (name, encoder) = selected.ok_or("Nessun encoder H.265 utilizzabile")?;
        if name == "CPU x265" {
            return Ok((name, encoder, json!(unavailable), json!([])));
        }
        let mut measurements = vec![];
        for (label, params) in [(name.clone(), encoder.clone()), ("CPU x265".into(), cpu())] {
            let began = Instant::now();
            let mut size = 0;
            let measured = (|| -> Result<()> {
                for (index, fraction) in [0.2, 0.6].into_iter().enumerate() {
                    let target =
                        Temporary(self.root.join(key).join(format!("calibration-{index}.mp4")));
                    let duration = info["duration_seconds"]
                        .as_f64()
                        .ok_or("Durata non valida")?;
                    let mut argv = vec![self.ffmpeg()?];
                    argv.extend(args(&[
                        "-v", "error", "-nostdin", "-y", "-threads", "4", "-ss",
                    ]));
                    argv.push((duration * fraction).to_string());
                    argv.extend([
                        "-i".into(),
                        display(source),
                        "-t".into(),
                        (10f64.min(duration * (1. - fraction))).to_string(),
                    ]);
                    argv.extend(args(&[
                        "-map",
                        "0:v:0",
                        "-an",
                        "-vf",
                        "scale=-2:'min(1080,ih)',fps=15",
                    ]));
                    argv.extend(params.clone());
                    argv.extend(args(&[
                        "-pix_fmt",
                        "yuv420p",
                        "-force_key_frames",
                        "expr:gte(t,n_forced*10)",
                    ]));
                    argv.push(display(&target.0));
                    self.execute(key, argv, 120000, true, false)?;
                    size += target.0.metadata().map_err(|e| e.to_string())?.len();
                }
                Ok(())
            })();
            if measured.is_ok() {
                measurements.push(json!({"encoder":label,"bytes":size,"seconds":(began.elapsed().as_secs_f64()*100.).round()/100.}));
            } else if self.closed.load(Ordering::Acquire) {
                return Err("Preparazione interrotta: programma chiuso".into());
            } else {
                unavailable.push(json!(label));
            }
        }
        let smallest = measurements
            .iter()
            .filter_map(|v| v["bytes"].as_u64())
            .min()
            .ok_or("Nessun encoder ha completato i campioni reali")?;
        let chosen = measurements
            .iter()
            .find(|v| {
                v["encoder"] == name
                    && v["bytes"].as_f64().unwrap_or(f64::MAX) <= smallest as f64 * 1.25
            })
            .or_else(|| {
                measurements
                    .iter()
                    .min_by_key(|v| v["bytes"].as_u64().unwrap_or(u64::MAX))
            })
            .unwrap();
        let chosen = s(&chosen["encoder"]).to_owned();
        Ok((
            chosen.clone(),
            if chosen == name { encoder } else { cpu() },
            json!(unavailable),
            json!(measurements),
        ))
    }
    fn encode(&self, key: &str, source: &Path, target: &Path, encoder: &[String]) -> Result<()> {
        let mut argv = vec![self.ffmpeg()?];
        argv.extend(args(&[
            "-hide_banner",
            "-nostdin",
            "-nostats",
            "-y",
            "-threads",
            "4",
            "-i",
        ]));
        argv.push(display(source));
        argv.extend(args(&[
            "-map",
            "0:v:0",
            "-map",
            "0:a:0?",
            "-vf",
            "scale=-2:'min(1080,ih)',fps=15",
        ]));
        argv.extend(encoder.iter().cloned());
        argv.extend(args(&[
            "-pix_fmt",
            "yuv420p",
            "-tag:v",
            "hvc1",
            "-force_key_frames",
            "expr:gte(t,n_forced*10)",
            "-c:a",
            "aac",
            "-ac",
            "1",
            "-b:a",
            "64k",
            "-movflags",
            "+faststart",
            "-map_metadata",
            "-1",
            "-progress",
        ]));
        argv.push(display(&self.root.join(key).join("encoding-progress.txt")));
        argv.push(display(target));
        self.execute(key, argv, 6 * 3600000, true, true).map(|_| ())
    }
    pub fn split(&self, key: &str, master: &Path, seconds: f64, limit: u64) -> Result<Value> {
        if !seconds.is_finite()
            || seconds <= 0.
            || seconds > 14400.
            || limit == 0
            || limit > MAX_PART
        {
            return Err("Video vuoto o limite parti non valido".into());
        }
        let folder = self.root.join(key);
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let size = master.metadata().map_err(|e| e.to_string())?.len();
        let mut result = vec![];
        let mut start = 0.;
        while start < seconds {
            if result.len() >= 1440 {
                return Err("Numero parti video eccessivo".into());
            }
            let mut length = (seconds - start).min(SEGMENT);
            if size > limit {
                length = length.min(
                    ((seconds * limit as f64 / size as f64 * 0.92 / KEYFRAME).floor() * KEYFRAME)
                        .max(OVERLAP + KEYFRAME),
                );
            }
            let name = format!("parte-{:02}.mp4", result.len() + 1);
            let target = folder.join(&name);
            let mut accepted = None;
            for _attempt in 0..24 {
                let mut argv = vec![self.ffmpeg()?];
                argv.extend(args(&["-v", "error", "-nostdin", "-y", "-ss"]));
                argv.push(start.to_string());
                argv.extend([
                    "-i".into(),
                    display(master),
                    "-t".into(),
                    length.to_string(),
                ]);
                argv.extend(args(&[
                    "-map",
                    "0:v:0",
                    "-map",
                    "0:a:0?",
                    "-c",
                    "copy",
                    "-avoid_negative_ts",
                    "make_zero",
                    "-movflags",
                    "+faststart",
                ]));
                argv.push(display(&target));
                self.execute(key, argv, 180000, true, false)?;
                let actual = target.metadata().map_err(|e| e.to_string())?.len();
                if actual <= limit {
                    if (self.duration(key, &target)? - length).abs() > 0.5 {
                        return Err("Taglio non allineato al minuto originale: encoder senza punti IDR adeguati".into());
                    }
                    accepted = Some(actual);
                    break;
                }
                let reduced =
                    (length * limit as f64 / actual as f64 * 0.9 / KEYFRAME).floor() * KEYFRAME;
                if reduced <= OVERLAP || reduced >= length {
                    return Err(
                        "Impossibile rispettare 150 MB conservando la sovrapposizione e la qualità"
                            .into(),
                    );
                }
                length = reduced;
            }
            let actual = accepted.ok_or("Limite parti video non raggiunto")?;
            let end = seconds.min(start + length);
            result
                .push(json!({"name":name,"start_seconds":start,"end_seconds":end,"bytes":actual}));
            self.update(key,json!({"stage":"splitting","message":format!("Parte {} verificata: {:.1} MB",result.len(),actual as f64/1e6)}))?;
            if end >= seconds {
                break;
            }
            let next = end - OVERLAP;
            if next <= start {
                return Err("Intervallo video troppo breve".into());
            }
            start = next;
        }
        Ok(json!(result))
    }
    fn load_master(&self, key: &str) -> Option<(PathBuf, Value)> {
        let path = self.root.join("_masters").join(format!("{key}.mp4"));
        let metadata = guide_repository::json_file(&path.with_extension("json"), 4_000_000).ok()?;
        if metadata["profile"] != profile()
            || path.is_symlink()
            || path.metadata().ok()?.len() != metadata["bytes"].as_u64()?
            || digest(&path).ok()? != s(&metadata["sha256"])
        {
            return None;
        }
        Some((path, metadata))
    }
    fn save_master(
        &self,
        key: &str,
        source_identity: &Value,
        encoded: &Path,
        details: &Value,
    ) -> Result<(PathBuf, Value)> {
        let root = self.root.join("_masters");
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let path = root.join(format!("{key}.mp4"));
        let mut metadata = details.clone();
        for (k, v) in source_identity
            .as_object()
            .ok_or("Invalid encoding identity")?
        {
            metadata[k] = v.clone();
        }
        metadata["bytes"] = json!(encoded.metadata().map_err(|e| e.to_string())?.len());
        metadata["sha256"] = json!(digest(encoded)?);
        std::fs::rename(encoded, &path).map_err(|e| e.to_string())?;
        reference_library::atomic_write(
            &path.with_extension("json"),
            &serde_json::to_vec_pretty(&metadata).map_err(|e| e.to_string())?,
        )?;
        Ok((path, metadata))
    }
    fn valid_parts(&self, folder: &Path, saved: &Value) -> bool {
        !a(&saved["parts"]).is_empty()
            && a(&saved["parts"]).iter().all(|part| {
                let name = s(&part["name"]);
                let path = folder.join(name);
                crate::import_catalog::regex(r"^parte-\d+\.mp4$").is_match(name)
                    && !path.is_symlink()
                    && path.metadata().is_ok_and(|m| {
                        m.len() <= MAX_PART && part["bytes"].as_u64() == Some(m.len())
                    })
                    && (part["sha256"].is_null()
                        || digest(&path).is_ok_and(|h| h == s(&part["sha256"])))
            })
    }
    pub fn start(self: &Arc<Self>, url: &str, refresh: bool) -> Result<Value> {
        let canonical = media_references::youtube(url)?;
        let _admission = self
            .admission
            .lock()
            .map_err(|_| "Video admission unavailable")?;
        if self.closed.load(Ordering::Acquire) {
            return Err("Preparazione video chiusa".into());
        }
        let source_hash = if refresh {
            None
        } else {
            self.frames
                .source(&canonical)
                .ok()
                .and_then(|(p, _)| digest(&p).ok())
        };
        let mut folders = std::fs::read_dir(&self.root)
            .into_iter()
            .flatten()
            .filter_map(std::result::Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                !p.is_symlink()
                    && p.is_dir()
                    && Self::valid_key(&p.file_name().unwrap_or_default().to_string_lossy())
            })
            .collect::<Vec<_>>();
        folders.sort_by_key(|p| {
            std::cmp::Reverse(
                p.join("manifest.json")
                    .metadata()
                    .and_then(|m| m.modified())
                    .ok(),
            )
        });
        for folder in folders {
            let Ok(saved) = guide_repository::json_file(&folder.join("manifest.json"), 4_000_000)
            else {
                continue;
            };
            if !refresh
                && saved["preset_version"] == PRESET
                && saved["source_url"] == canonical
                && saved["fps"] == 15
                && saved["encoding_profile"] == profile()
                && source_hash
                    .as_ref()
                    .is_none_or(|h| *h == s(&saved["source_sha256"]))
                && saved["max_part_bytes"] == MAX_PART
                && saved["overlap_seconds"] == 10
                && saved["segment_seconds"] == 300
                && self.valid_parts(&folder, &saved)
            {
                let key = folder.file_name().unwrap().to_string_lossy().into_owned();
                let job = self.status(&key)?;
                let audio = &job["audio"];
                let valid = ["audio.m4a", "audio.opus", "audio.mp3"].contains(&s(&audio["name"]))
                    && digest(&folder.join(s(&audio["name"])))
                        .is_ok_and(|h| h == s(&audio["sha256"]));
                if !valid && job["audio_status"] != "running" && job["audio_status"] != "absent" {
                    self.update(&key, json!({"audio_status":"running","audio":null}))?;
                    let owner = self.clone();
                    let selected = key.clone();
                    std::thread::Builder::new()
                        .name("hoi4-audio".into())
                        .spawn(move || {
                            let _ = owner.audio_job(&selected);
                        })
                        .map_err(|e| e.to_string())?;
                }
                return self.status(&key);
            }
        }
        if self
            .jobs
            .lock()
            .map_err(|_| "Video jobs unavailable")?
            .values()
            .any(|j| j["status"] == "running" || j["audio_status"] == "running")
        {
            return Err("Una preparazione video è già in corso".into());
        }
        let key = crate::progress_store::new_id()?;
        self.jobs.lock().map_err(|_|"Video jobs unavailable")?.insert(key.clone(),json!({"id":key,"url":canonical,"status":"running","stage":"queued","message":"Preparazione avviata","parts":[],"refresh_source":refresh}));
        let owner = self.clone();
        let selected = key.clone();
        std::thread::Builder::new().name("hoi4-video".into()).spawn(move||{
    if let Err(error)=owner.prepare(&selected,&canonical,refresh){let _=owner.update(&selected,json!({"status":"failed","stage":"failed","message":error.chars().take(1600).collect::<String>()}));}
    let path=owner.root.join(&selected).join("master.mp4");if path.is_file()&&!path.is_symlink(){let _=std::fs::remove_file(path);}
  }).map_err(|e|e.to_string())?;
        self.status(&key)
    }
    fn audio_job(&self, key: &str) -> Result<()> {
        let result = (|| -> Result<()> {
            let path = self.root.join(key).join("manifest.json");
            let mut manifest = guide_repository::json_file(&path, 4_000_000)?;
            let (source, _) = self.frames.source(s(&manifest["source_url"]))?;
            if !manifest["source_sha256"].is_null()
                && digest(&source)? != s(&manifest["source_sha256"])
            {
                return Err("Il sorgente è cambiato: prepara nuovamente il video".into());
            }
            let info = self.info(key, &source)?;
            manifest["audio"] = self.audio(key, &source, &info)?;
            let status = self.status(key)?;
            for field in ["audio_status", "audio_warning"] {
                manifest[field] = status[field].clone();
            }
            reference_library::atomic_write(
                &path,
                &serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
            )?;
            self.update(key, json!({"audio":manifest["audio"]}))
        })();
        if let Err(error) = &result {
            let _=self.update(key,json!({"audio_status":"failed","audio_warning":format!("Audio separato non disponibile: {}",error.chars().take(300).collect::<String>())}));
        }
        result
    }
    fn prepare(&self, key: &str, url: &str, refresh: bool) -> Result<()> {
        let folder = self.root.join(key);
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        reference_library::atomic_write(
            &folder.join("job.json"),
            &serde_json::to_vec(&json!({"source_url":url})).map_err(|e| e.to_string())?,
        )?;
        let cached = if refresh {
            None
        } else {
            self.frames.source(url).ok()
        };
        let mut base = vec![];
        if let Some(tool) = source_acquisition::ytdlp(Some(&self.package)) {
            base.push(display(&tool));
            base.extend(args(&[
                "--ignore-config",
                "--no-plugin-dirs",
                "--no-playlist",
                "--socket-timeout",
                "20",
                "--retries",
                "2",
            ]));
            if let Some(node) = process_tools::executable(None, "node", Some(&self.package)) {
                base.extend(["--js-runtimes".into(), format!("node:{}", node.display())]);
            }
        }
        if base.is_empty() && cached.is_none() {
            return Err(
                "yt-dlp non trovato: installa le dipendenze video o configura HOI4_YTDLP".into(),
            );
        }
        self.update(
            key,
            json!({"stage":"metadata","message":"Lettura delle informazioni YouTube"}),
        )?;
        let metadata = if let Some((path, stored)) = &cached {
            let mut value = json!({"title":url,"description":""});
            for field in ["title", "description", "duration"] {
                if let Some(v) = stored.get(field) {
                    value[field] = v.clone();
                }
            }
            value["duration"] = json!(self.duration(key, path)?);
            value
        } else {
            let mut argv = base.clone();
            argv.extend(args(&["--skip-download", "--dump-single-json", "--"]));
            argv.push(url.into());
            serde_json::from_str::<Value>(&self.execute(key, argv, 120000, true, false)?)
                .map_err(|e| e.to_string())?
        };
        let seconds = metadata["duration"]
            .as_f64()
            .filter(|n| n.is_finite() && *n > 0. && *n <= 14400.)
            .ok_or("Usa un video registrato di durata inferiore a quattro ore")?;
        if crate::state::truth(&metadata["is_live"]) {
            return Err("Usa un video registrato di durata inferiore a quattro ore".into());
        }
        self.update(key,json!({"title":metadata.get("title").unwrap_or(&json!("Video guida")),"duration_seconds":seconds,"stage":"downloading","message":"Download del sorgente fino a 1080p, con audio originale inglese quando disponibile"}))?;
        reference_library::atomic_write(
            &folder.join("descrizione.txt"),
            s(&metadata["description"]).as_bytes(),
        )?;
        let source = if let Some((path, _)) = cached {
            self.update(key,json!({"message":"Riutilizzo del sorgente locale: nessun nuovo download","cache_reused":true}))?;
            path
        } else {
            let mut argv = base;
            argv.extend(args(&["--no-progress", "--ffmpeg-location"]));
            argv.push(self.ffmpeg()?);
            argv.extend(args(&[
                "-f",
                "bv[height<=1080]+ba[language^=en]/bv[height<=1080]+ba/b[height<=1080]",
                "--merge-output-format",
                "mkv",
                "-o",
            ]));
            argv.push(display(&folder.join("source.%(ext)s")));
            argv.extend(["--".into(), url.into()]);
            self.execute(key, argv, 3600000, true, false)?;
            let sources = std::fs::read_dir(&folder)
                .map_err(|e| e.to_string())?
                .filter_map(std::result::Result::ok)
                .map(|e| e.path())
                .filter(|p| {
                    p.file_stem().is_some_and(|s| s == "source")
                        && p.extension().is_some_and(|s| {
                            ["mkv", "mp4", "webm"].contains(&s.to_string_lossy().as_ref())
                        })
                })
                .collect::<Vec<_>>();
            if sources.len() != 1 {
                return Err("Sorgente video scaricato non identificabile".into());
            }
            sources[0].clone()
        };
        self.frames.register(url, &source, &metadata)?;
        let info = self.info(key, &source)?;
        self.update(
            key,
            json!({"stage":"audio","message":"Preparazione della traccia audio separata"}),
        )?;
        let audio = self.audio(key, &source, &info)?;
        self.update(
            key,
            json!({"source_width":info["width"],"source_height":info["height"]}),
        )?;
        let (cache_key, source_identity) = identity(&source)?;
        let (mut master, mut details) = if let Some((master, details)) =
            self.load_master(&cache_key)
        {
            self.update(key,json!({"encoder":details["encoder"],"quality_mode":details["quality_mode"],"master_reused":true,"message":"Master compatibile verificato: salto la ricodifica","encoder_measurements":details.get("encoder_measurements").unwrap_or(&json!([]))}))?;
            (master, details)
        } else {
            self.update(key,json!({"stage":"calibrating","message":"Confronto GPU/CPU su campioni reali del video"}))?;
            let (name, encoder, unavailable, measurements) = self.select(key, &source, &info)?;
            let quality = if name == "CPU x265" {
                "CRF 22"
            } else {
                "Qualità hardware 22 (non equivalente numericamente a CRF)"
            };
            self.update(key,json!({"encoder":name,"unavailable_encoders":unavailable,"stage":"encoding","message":format!("Codifica H.265 a 15 FPS · {name}"),"quality_mode":quality,"encoder_measurements":measurements}))?;
            let temporary = folder.join("master.mp4");
            if let Err(error) = self.encode(key, &source, &temporary, &encoder) {
                if name == "CPU x265" || self.closed.load(Ordering::Acquire) {
                    return Err(error);
                }
                self.update(key,json!({"encoder":"CPU x265","quality_mode":"CRF 22","message":"Encoder hardware non riuscito sul sorgente: riprovo con CPU x265"}))?;
                self.encode(key, &source, &temporary, &cpu())?;
            }
            let real = self.duration(key, &temporary)?;
            if (real - info["duration_seconds"].as_f64().unwrap()).abs() > 0.5 {
                return Err("Durata della codifica incoerente con il sorgente".into());
            }
            if identity(&source)?.0 != cache_key {
                return Err("Sorgente modificato durante la codifica".into());
            }
            let job = self.status(key)?;
            let details = json!({"source_url":url,"duration_seconds":real,"encoder":job["encoder"],"quality_mode":job["quality_mode"],"encoder_measurements":measurements});
            self.save_master(&cache_key, &source_identity, &temporary, &details)?
        };
        let mut real = details["duration_seconds"]
            .as_f64()
            .ok_or("Durata master non valida")?;
        let mut parts = match self.split(key, &master, real, MAX_PART) {
            Ok(parts) => parts,
            Err(error) => {
                if self.status(key)?["encoder"] == "CPU x265" || self.closed.load(Ordering::Acquire)
                {
                    return Err(error);
                }
                self.update(key,json!({"encoder":"CPU x265","quality_mode":"CRF 22","stage":"encoding","message":"Riallineamento dei tagli con CPU x265"}))?;
                let temporary = folder.join("master.mp4");
                self.encode(key, &source, &temporary, &cpu())?;
                real = self.duration(key, &temporary)?;
                details["encoder"] = json!("CPU x265");
                details["quality_mode"] = json!("CRF 22");
                details["duration_seconds"] = json!(real);
                (master, details) =
                    self.save_master(&cache_key, &source_identity, &temporary, &details)?;
                self.split(key, &master, real, MAX_PART)?
            }
        };
        for part in parts.as_array_mut().unwrap() {
            let name = s(&part["name"]).to_owned();
            part["url"] = json!(format!("/api/import/video/file?id={key}&name={name}"));
            part["sha256"] = json!(digest(&folder.join(name))?);
        }
        let job = self.status(key)?;
        let manifest = json!({"preset_version":PRESET,"source_url":url,"title":metadata.get("title").unwrap_or(&json!(url)),"fps":15,"max_part_bytes":MAX_PART,"overlap_seconds":10,"segment_seconds":300,"duration_seconds":real,"parts":parts,"encoder":job["encoder"],"quality_mode":job["quality_mode"],"source_width":info["width"],"source_height":info["height"],"source_sha256":source_identity["source_sha256"],"master_key":cache_key,"encoding_profile":profile(),"encoder_measurements":details.get("encoder_measurements").unwrap_or(&json!([])),"audio":audio,"audio_status":job["audio_status"],"audio_warning":job["audio_warning"]});
        reference_library::atomic_write(
            &folder.join("manifest.json"),
            &serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
        )?;
        self.update(key,json!({"status":"completed","stage":"completed","message":"Video pronto: ogni parte è stata verificata sotto 150 MB","parts":parts,"manifest_url":format!("/api/import/video/file?id={key}&name=manifest.json"),"description_url":format!("/api/import/video/file?id={key}&name=descrizione.txt"),"source_width":info["width"],"source_height":info["height"],"audio":audio}))?;
        match self.cleanup(url, key, &cache_key) {
            Ok(bytes) => self.update(key, json!({"cache_freed_bytes":bytes}))?,
            Err(_) => self.update(
                key,
                json!({"cache_warning":"Video pronto; pulizia della cache non completata"}),
            )?,
        }
        Ok(())
    }
    fn cleanup(&self, url: &str, keep: &str, master_key: &str) -> Result<u64> {
        let registry = self.frames.registry()?;
        let protected = registry
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(_, v)| v.get("path").and_then(Value::as_str).or_else(|| v.as_str()))
            .filter_map(|p| Path::new(p).canonicalize().ok())
            .collect::<Vec<_>>();
        let root = self.root.canonicalize().map_err(|e| e.to_string())?;
        let mut removed = 0;
        let generated = crate::import_catalog::regex(
            r"^(?:parte-\d+\.mp4|master\.mp4|source\.(?:mp4|mkv|webm)|audio(?:\.part)?\.(?:m4a|opus|mp3)|encoding-progress\.txt|encoding\.log|download\.log|manifest\.json|descrizione\.txt|job\.json|calibration-\d+\.mp4)$",
        );
        for entry in std::fs::read_dir(&root)
            .map_err(|e| e.to_string())?
            .filter_map(std::result::Result::ok)
        {
            let folder = entry.path();
            let id = entry.file_name().to_string_lossy().into_owned();
            if id == keep
                || !Self::valid_key(&id)
                || folder.is_symlink()
                || folder
                    .canonicalize()
                    .ok()
                    .and_then(|p| p.parent().map(Path::to_path_buf))
                    .as_ref()
                    != Some(&root)
            {
                continue;
            }
            if self
                .jobs
                .lock()
                .map_err(|_| "Video jobs unavailable")?
                .get(&id)
                .is_some_and(|v| v["status"] == "running" || v["audio_status"] == "running")
            {
                continue;
            }
            let manifest = folder.join("manifest.json");
            let record_path = if manifest.is_file() {
                manifest
            } else {
                folder.join("job.json")
            };
            let record = guide_repository::json_file(&record_path, 4_000_000);
            if record.as_ref().map_or(true, |v| v["source_url"] != url) {
                continue;
            }
            for file in std::fs::read_dir(&folder)
                .map_err(|e| e.to_string())?
                .filter_map(std::result::Result::ok)
            {
                let path = file.path();
                if !path.is_file()
                    || path.is_symlink()
                    || !generated.is_match(&file.file_name().to_string_lossy())
                    || path.canonicalize().is_ok_and(|p| protected.contains(&p))
                {
                    continue;
                }
                let bytes = path.metadata().map_err(|e| e.to_string())?.len();
                if std::fs::remove_file(path).is_ok() {
                    removed += bytes;
                }
            }
            if protected
                .iter()
                .any(|p| p.parent() == Some(folder.as_path()))
            {
                reference_library::atomic_write(
                    &folder.join("job.json"),
                    &serde_json::to_vec(&json!({"source_url":url,"source_only":true})).unwrap(),
                )?;
            }
            let _ = std::fs::remove_dir(&folder);
            self.jobs
                .lock()
                .map_err(|_| "Video jobs unavailable")?
                .remove(&id);
        }
        let masters = root.join("_masters");
        for record in std::fs::read_dir(&masters)
            .into_iter()
            .flatten()
            .filter_map(std::result::Result::ok)
        {
            let path = record.path();
            let name = path.file_stem().unwrap_or_default().to_string_lossy();
            if path.extension().is_none_or(|v| v != "json")
                || name == master_key
                || name.len() != 64
                || !name.bytes().all(|b| b.is_ascii_hexdigit())
                || path.is_symlink()
            {
                continue;
            }
            let Ok(metadata) = guide_repository::json_file(&path, 4_000_000) else {
                continue;
            };
            if metadata["source_url"] != url {
                continue;
            }
            let master = path.with_extension("mp4");
            for file in [master, path] {
                if file.is_file() && !file.is_symlink() {
                    let bytes = file.metadata().map_err(|e| e.to_string())?.len();
                    if std::fs::remove_file(file).is_ok() {
                        removed += bytes;
                    }
                }
            }
        }
        Ok(removed)
    }
    pub fn file(&self, key: &str, name: &str) -> Result<PathBuf> {
        let job = self.status(key)?;
        let allowed = ["manifest.json", "descrizione.txt"].contains(&name)
            || a(&job["parts"]).iter().any(|p| p["name"] == name)
            || ["audio.m4a", "audio.opus", "audio.mp3"].contains(&name)
                && job["audio"]["name"] == name;
        if job["status"] != "completed"
            || !allowed
            || Path::new(name).file_name().is_none_or(|v| v != name)
        {
            return Err("File video non disponibile".into());
        }
        let path = self.root.join(key).join(name);
        if !path.is_file() || path.is_symlink() {
            return Err("File video non disponibile".into());
        }
        Ok(path)
    }
    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
        if let Ok(tools) = self.tools.lock() {
            for tool in tools.values() {
                let _ = tool.cancel();
            }
        }
    }
}

pub fn split_local(args: &Value) -> Result<Value> {
    let root = PathBuf::from(args["root"].as_str().ok_or("Missing video output root")?);
    let frames = Frames::new(
        root.join("frames"),
        args["ffmpeg"].as_str().map(PathBuf::from),
    );
    let jobs = Jobs::new(root, frames, PathBuf::new());
    let key = "00000000000000000000000000000001";
    jobs.jobs
        .lock()
        .map_err(|_| "Video jobs unavailable")?
        .insert(key.into(), json!({"status":"running"}));
    jobs.split(
        key,
        Path::new(args["master"].as_str().ok_or("Missing source master")?),
        args["seconds"].as_f64().ok_or("Missing duration")?,
        args["limit"].as_u64().unwrap_or(MAX_PART),
    )
}
