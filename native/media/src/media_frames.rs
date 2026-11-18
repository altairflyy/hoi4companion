//! Registered local source previews. Only the registry supplies file paths;
//! request parameters never select arbitrary files or cause remote downloads.
use crate::{guide_repository, media_references, process_tools, reference_library};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
pub struct Frames {
    pub root: PathBuf,
    pub ffmpeg: Option<PathBuf>,
    work: Mutex<()>,
}
impl Frames {
    pub fn new(root: PathBuf, ffmpeg: Option<PathBuf>) -> Arc<Self> {
        Arc::new(Self {
            root,
            ffmpeg,
            work: Mutex::new(()),
        })
    }
    pub fn registry(&self) -> Result<Value, String> {
        let path = self.root.join("sources.json");
        if !path.exists() {
            return Ok(json!({}));
        }
        guide_repository::json_file(&path, 16_000_000)
    }
    pub fn source(&self, url: &str) -> Result<(PathBuf, Value), String> {
        let canonical = media_references::youtube(url)?;
        let registry = self.registry()?;
        let item = &registry[canonical];
        let value = if item.is_object() {
            &item["path"]
        } else {
            item
        };
        let path = PathBuf::from(value.as_str().unwrap_or(""));
        if !path.is_file() {
            return Err(
                "Video sorgente non disponibile: preparalo nella pagina Importa guida".into(),
            );
        }
        Ok((path, item.clone()))
    }
    pub fn register(&self, url: &str, path: &Path, metadata: &Value) -> Result<(), String> {
        let canonical = media_references::youtube(url)?;
        let target = path.canonicalize().map_err(|e| e.to_string())?;
        if !target.is_file() {
            return Err("Video locale non trovato".into());
        }
        let _lock = self.work.lock().map_err(|_| "Media registry unavailable")?;
        let mut registry = self.registry()?;
        let mut row = json!({"path":crate::installation::display(&target)});
        for field in ["title", "duration", "description"] {
            if let Some(value) = metadata.get(field) {
                row[field] = value.clone();
            }
        }
        registry[canonical] = row;
        std::fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        reference_library::atomic_write(
            &self.root.join("sources.json"),
            &serde_json::to_vec_pretty(&registry).map_err(|e| e.to_string())?,
        )
    }
    pub fn availability(&self, mut media: Value) -> Value {
        for row in media["references"].as_array_mut().into_iter().flatten() {
            match self.source(row["url"].as_str().unwrap_or("")) {
                Ok((_, metadata)) => {
                    row["source_available"] = json!(true);
                    if let Some(total) = metadata["duration"]
                        .as_f64()
                        .filter(|t| t.is_finite() && *t > 0.)
                    {
                        let valid = row["seconds"].as_f64().is_some_and(|n| n < total);
                        row["time_available"] = json!(valid);
                        row["clip_end"] = if valid {
                            json!(row["clip_end"].as_f64().unwrap_or(total).min(total))
                        } else {
                            Value::Null
                        };
                    }
                }
                Err(_) => row["source_available"] = json!(false),
            }
        }
        media
    }
    fn valid_jpeg(path: &Path) -> bool {
        std::fs::metadata(path).is_ok_and(|m| m.len() > 0 && m.len() <= 30_000_000)
            && image::ImageReader::open(path)
                .ok()
                .and_then(|r| r.with_guessed_format().ok())
                .is_some_and(|mut r| {
                    if r.format() != Some(image::ImageFormat::Jpeg) {
                        return false;
                    }
                    let mut limits = image::Limits::default();
                    limits.max_image_width = Some(8192);
                    limits.max_image_height = Some(8192);
                    limits.max_alloc = Some(180_000_000);
                    r.limits(limits);
                    r.decode().is_ok()
                })
    }
    pub fn preview(&self, url: &str, seconds: f64, clip: bool) -> Result<PathBuf, String> {
        if !seconds.is_finite() || !(0.0..14400.0).contains(&seconds) {
            return Err("Minutaggio non valido".into());
        }
        let _work = self.work.lock().map_err(|_| "Media previews unavailable")?;
        let (source, _) = self.source(url)?;
        let metadata = source.metadata().map_err(|e| e.to_string())?;
        let stamp = format!(
            "native-preview-v1:{}:{}:{:?}:{seconds}:{clip}",
            source.display(),
            metadata.len(),
            metadata.modified().map_err(|e| e.to_string())?
        );
        let key = format!("{:x}", Sha256::digest(stamp.as_bytes()));
        let target = self.root.join(format!(
            "{}{}.{}",
            if clip { "clip-" } else { "" },
            key,
            if clip { "mp4" } else { "jpg" }
        ));
        if target.is_file() && (clip || Self::valid_jpeg(&target)) {
            return Ok(target);
        }
        let ffmpeg = self
            .ffmpeg
            .as_ref()
            .ok_or("FFmpeg non trovato: configura HOI4_FFMPEG")?;
        let info = process_tools::run(
            vec![
                ffmpeg.to_string_lossy().into_owned(),
                "-hide_banner".into(),
                "-i".into(),
                source.to_string_lossy().into_owned(),
            ],
            25000,
            2_000_000,
        )?;
        let pattern = crate::import_catalog::regex(r"Duration: (\d+):(\d+):(\d+(?:\.\d+)?)");
        let duration = pattern
            .captures(&info.stderr)
            .ok_or("Durata del video non leggibile")?;
        let total = duration[1].parse::<f64>().unwrap_or(0.) * 3600.
            + duration[2].parse::<f64>().unwrap_or(0.) * 60.
            + duration[3].parse::<f64>().unwrap_or(0.);
        if seconds >= total {
            return Err("Fotogramma oltre la durata del video".into());
        }
        let temporary = self.root.join(format!(
            "{}-tmp-{}.{}",
            if clip { "clip" } else { "frame" },
            crate::progress_store::new_id()?,
            if clip { "mp4" } else { "jpg" }
        ));
        struct Temp(PathBuf);
        impl Drop for Temp {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let owned = Temp(temporary);
        let mut argv = vec![
            ffmpeg.to_string_lossy().into_owned(),
            "-v".into(),
            "error".into(),
            "-nostdin".into(),
            "-ss".into(),
            if clip {
                (seconds - 5.).max(0.).to_string()
            } else {
                seconds.to_string()
            },
            "-i".into(),
            source.to_string_lossy().into_owned(),
        ];
        if clip {
            argv.extend([
                "-t".into(),
                ((seconds + 10.).min(total) - (seconds - 5.).max(0.)).to_string(),
            ]);
            argv.extend(
                [
                    "-map",
                    "0:v:0",
                    "-map",
                    "0:a:0?",
                    "-c:v",
                    "libx264",
                    "-preset",
                    "veryfast",
                    "-crf",
                    "23",
                    "-pix_fmt",
                    "yuv420p",
                    "-c:a",
                    "aac",
                    "-b:a",
                    "96k",
                    "-movflags",
                    "+faststart",
                ]
                .into_iter()
                .map(str::to_owned),
            );
        } else {
            argv.extend(
                ["-frames:v", "1", "-q:v", "2"]
                    .into_iter()
                    .map(str::to_owned),
            );
        }
        argv.extend(["-y".into(), owned.0.to_string_lossy().into_owned()]);
        process_tools::checked(argv, if clip { 90000 } else { 40000 }, 2_000_000)?;
        if clip {
            if !owned
                .0
                .metadata()
                .is_ok_and(|m| m.len() > 0 && m.len() <= 30_000_000)
            {
                return Err("Clip vuota o superiore a 30 MB".into());
            }
        } else if !Self::valid_jpeg(&owned.0) {
            return Err("Fotogramma generato non valido".into());
        }
        std::fs::rename(&owned.0, &target).map_err(|e| e.to_string())?;
        if clip {
            let mut cached = std::fs::read_dir(&self.root)
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name().is_some_and(|v| {
                        v.to_string_lossy().starts_with("clip-")
                            && !v.to_string_lossy().starts_with("clip-tmp-")
                    }) && p.extension().is_some_and(|v| v == "mp4")
                })
                .collect::<Vec<_>>();
            cached.sort_by_key(|p| std::cmp::Reverse(p.metadata().and_then(|m| m.modified()).ok()));
            for old in cached.into_iter().skip(32).filter(|p| p != &target) {
                let _ = std::fs::remove_file(old);
            }
        }
        Ok(target)
    }
}
