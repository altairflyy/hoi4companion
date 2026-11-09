//! Local sprite fallback. A requested glyph and two different GUI landmarks
//! must agree on scale and translation. Pixel matches never imply completion.
use crate::{hud_geometry::rows, hud_targets::text_key};
use image::{
    RgbImage, RgbaImage,
    imageops::{self, FilterType},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicU64, Ordering},
};
type Result<T> = std::result::Result<T, String>;
fn n(v: &Value) -> Option<f64> {
    v.as_f64().filter(|n| n.is_finite())
}
fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn decode_hex(text: &str, limit: usize) -> Result<Vec<u8>> {
    if !text.len().is_multiple_of(2) || text.len() > limit * 2 || !text.is_ascii() {
        return Err("Invalid visual byte payload".into());
    }
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| {
            let a = (b[0] as char)
                .to_digit(16)
                .ok_or("Invalid visual byte payload")?;
            let c = (b[1] as char)
                .to_digit(16)
                .ok_or("Invalid visual byte payload")?;
            Ok((a * 16 + c) as u8)
        })
        .collect()
}
#[derive(Clone)]
struct Icon {
    image: Arc<RgbaImage>,
    digest: String,
}
fn asset(bytes: &[u8]) -> Result<Icon> {
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("Visual asset exceeds limit".into());
    }
    let mut reader =
        image::ImageReader::with_format(std::io::Cursor::new(bytes), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(256);
    limits.max_image_height = Some(256);
    limits.max_alloc = Some(2 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().map_err(|e| e.to_string())?.to_rgba8();
    if image.width().min(image.height()) < 12 {
        return Err("Visual asset too small".into());
    }
    let digest = format!("{:x}", Sha256::digest(image.as_raw()));
    Ok(Icon {
        image: Arc::new(image),
        digest,
    })
}
#[derive(Clone)]
struct Node {
    id: String,
    position: [f64; 4],
    icon: [f64; 2],
    centered: bool,
    asset: Icon,
}
struct Folder {
    nodes: Vec<Node>,
    captions: HashSet<String>,
}
#[derive(Clone)]
struct Tab {
    id: String,
    name: String,
    order: f64,
    offset: [f64; 2],
    asset: Icon,
}
pub(crate) struct Vision {
    assets: HashMap<String, Icon>,
    folders: HashMap<String, Folder>,
    tabs: Vec<Tab>,
    cached: Option<(Value, Value)>,
}
#[derive(Clone)]
struct Template {
    w: usize,
    h: usize,
    pixels: Vec<(usize, usize, [u8; 3], u8)>,
    gray_norm: f64,
}
#[derive(Clone, Debug)]
struct Hit {
    x: usize,
    y: usize,
    score: f64,
}
fn gray(rgb: [u8; 3]) -> u8 {
    ((rgb[0] as f64 * 0.299 + rgb[1] as f64 * 0.587 + rgb[2] as f64 * 0.114).round() as u32)
        .min(255) as u8
}
fn template(icon: &Icon, scale: f64) -> Template {
    let width = (icon.image.width() as f64 * scale)
        .round_ties_even()
        .max(1.) as u32;
    let height = (icon.image.height() as f64 * scale)
        .round_ties_even()
        .max(1.) as u32;
    let data = if (width, height) == icon.image.dimensions() {
        (*icon.image).clone()
    } else {
        imageops::resize(icon.image.as_ref(), width, height, FilterType::CatmullRom)
    };
    let pixels: Vec<_> = data
        .enumerate_pixels()
        .filter(|(_, _, p)| p[3] > 245)
        .map(|(x, y, p)| {
            (
                x as usize,
                y as usize,
                [p[0], p[1], p[2]],
                gray([p[0], p[1], p[2]]),
            )
        })
        .collect();
    let gray_norm = pixels.iter().map(|p| (p.3 as f64).powi(2)).sum();
    Template {
        w: width as usize,
        h: height as usize,
        pixels,
        gray_norm,
    }
}
fn error(
    frame: &RgbImage,
    template: &Template,
    x: usize,
    y: usize,
    normalized: bool,
) -> Option<f64> {
    let mut sum = 0u64;
    let mut gray_sum = 0f64;
    let mut gray_norm = 0f64;
    let limit = 160 * template.pixels.len() as u64 * 3;
    let stride = frame.width() as usize * 3;
    let data = frame.as_raw();
    for &(px, py, rgb, g) in &template.pixels {
        let off = (y + py) * stride + (x + px) * 3;
        let actual = [data[off], data[off + 1], data[off + 2]];
        for i in 0..3 {
            let delta = actual[i] as i32 - rgb[i] as i32;
            sum += (delta * delta) as u64;
        }
        if sum >= limit {
            return None;
        }
        if normalized {
            let a = gray(actual) as f64;
            gray_sum += (a - g as f64).powi(2);
            gray_norm += a * a;
        }
    }
    if normalized {
        let denominator = (gray_norm * template.gray_norm).sqrt();
        if denominator <= 0. || gray_sum / denominator > 0.006 {
            return None;
        }
        Some(gray_sum / denominator)
    } else {
        Some(sum as f64 / (template.pixels.len() * 3) as f64)
    }
}
fn search(
    frame: &RgbImage,
    template: &Template,
    region_height: usize,
    normalized: bool,
) -> Vec<Hit> {
    let width = frame.width() as usize;
    let height = region_height.min(frame.height() as usize);
    if template.pixels.len() < 80 || template.h >= height || template.w >= width {
        return vec![];
    }
    let mut locations = vec![];
    for y in 0..=height - template.h {
        for x in 0..=width - template.w {
            if let Some(score) = error(frame, template, x, y, normalized) {
                locations.push(Hit { x, y, score });
                if locations.len() > 4096 {
                    return vec![];
                }
            }
        }
    }
    locations.sort_by(|a, b| {
        a.score
            .total_cmp(&b.score)
            .then(a.y.cmp(&b.y))
            .then(a.x.cmp(&b.x))
    });
    let mut hits: Vec<Hit> = vec![];
    for candidate in locations {
        if hits.iter().any(|hit| {
            candidate.x.abs_diff(hit.x) <= template.w / 2
                && candidate.y.abs_diff(hit.y) <= template.h / 2
        }) {
            continue;
        }
        hits.push(candidate);
        if hits.len() == 3 {
            break;
        }
    }
    hits
}
impl Vision {
    pub(crate) fn new(args: &Value) -> Result<Self> {
        let mut out = Self {
            assets: HashMap::new(),
            folders: HashMap::new(),
            tabs: vec![],
            cached: None,
        };
        if let Some(assets) = args["assets"].as_object() {
            if assets.len() > 1024 {
                return Err("Too many visual assets".into());
            }
            let mut used = 0;
            for (url, data) in assets {
                if !url.starts_with("/api/sprite/") {
                    continue;
                }
                let bytes = decode_hex(s(data), 2 * 1024 * 1024)?;
                let icon = asset(&bytes)?;
                used += icon.image.as_raw().len();
                if used > 128 * 1024 * 1024 {
                    return Err("Visual asset cache exceeds limit".into());
                }
                out.assets.insert(url.clone(), icon);
            }
        }
        out.definitions(args)?;
        Ok(out)
    }
    fn definitions(&mut self, args: &Value) -> Result<()> {
        if let Some(trees) = args["trees"].as_object() {
            if trees.len() > 64 {
                return Err("Too many visual folders".into());
            }
            for (id, tree) in trees {
                let captions = rows(&tree["layout"]["captions"])
                    .iter()
                    .filter(|c| c["kind"] == "title")
                    .map(|c| text_key(s(&c["text"])))
                    .collect();
                let mut nodes = vec![];
                for node in rows(&tree["technologies"]) {
                    let Some(position) = rows(&node["folders"])
                        .iter()
                        .find(|p| p["id"] == *id)
                        .map(|p| &p["layout"])
                    else {
                        continue;
                    };
                    let Some(asset) = self.assets.get(s(&node["sprite"]["url"])) else {
                        continue;
                    };
                    let icon = &node["visual"]["icon"];
                    let fields = [
                        n(&position["x"]),
                        n(&position["y"]),
                        n(&position["width"]),
                        n(&position["height"]),
                        n(&icon["x"]),
                        n(&icon["y"]),
                    ];
                    if fields.iter().any(Option::is_none) {
                        continue;
                    }
                    let fields = fields.map(Option::unwrap);
                    nodes.push(Node {
                        id: s(&node["id"]).into(),
                        position: [fields[0], fields[1], fields[2], fields[3]],
                        icon: [fields[4], fields[5]],
                        centered: crate::state::truth(&icon["centered"]),
                        asset: asset.clone(),
                    });
                    if nodes.len() > 4096 {
                        return Err("Too many visual nodes".into());
                    }
                }
                self.folders.insert(id.clone(), Folder { nodes, captions });
            }
        }
        for folder in rows(&args["catalog"]["folders"]) {
            let tab = &folder["tab"];
            let sprite = tab
                .get("inactive_sprite")
                .filter(|v| !v.is_null() && v.as_object().is_some_and(|o| !o.is_empty()))
                .unwrap_or_else(|| tab.get("sprite").unwrap_or(&Value::Null));
            let Some(icon) = self.assets.get(s(&sprite["url"])) else {
                continue;
            };
            let Some(order) = n(&tab["order"]) else {
                continue;
            };
            let w = icon.image.width() as f64;
            let h = icon.image.height() as f64;
            let crop = [
                (w * 0.16).round_ties_even() as u32,
                (h * 0.2).round_ties_even() as u32,
                (w * 0.84).round_ties_even() as u32,
                (h * 0.77).round_ties_even() as u32,
            ];
            let image = imageops::crop_imm(
                icon.image.as_ref(),
                crop[0],
                crop[1],
                crop[2] - crop[0],
                crop[3] - crop[1],
            )
            .to_image();
            self.tabs.push(Tab {
                id: s(&folder["id"]).into(),
                name: s(&folder["name"]).into(),
                order,
                offset: [crop[0] as f64, crop[1] as f64],
                asset: Icon {
                    image: Arc::new(image),
                    digest: icon.digest.clone(),
                },
            });
            if self.tabs.len() > 128 {
                return Err("Too many visual tabs".into());
            }
        }
        Ok(())
    }
    pub(crate) fn matches(&self, frame: &RgbImage, proposals: &Value, ocr: &Value) -> Vec<Value> {
        let keys: HashSet<_> = rows(&ocr["lines"])
            .iter()
            .map(|l| text_key(s(&l["text"])))
            .collect();
        let mut result = vec![];
        for proposal in rows(proposals) {
            if proposal["panel"] != "research" {
                continue;
            }
            let Some(folder) = self.folders.get(s(&proposal["folder"])) else {
                continue;
            };
            if !keys.iter().any(|k| folder.captions.contains(k)) {
                continue;
            }
            let Some(target) = folder
                .nodes
                .iter()
                .find(|n| n.id == s(&proposal["identity"]))
            else {
                continue;
            };
            if folder
                .nodes
                .iter()
                .any(|n| n.id != target.id && n.asset.digest == target.asset.digest)
            {
                continue;
            }
            let mut candidates = vec![];
            for scale in [1., 0.8, 1.25, 1.5] {
                let pixels = template(&target.asset, scale);
                for hit in search(frame, &pixels, frame.height() as usize, false) {
                    let origin = |node: &Node| {
                        let center = if node.centered {
                            [
                                node.asset.image.width() as f64 / 2.,
                                node.asset.image.height() as f64 / 2.,
                            ]
                        } else {
                            [0., 0.]
                        };
                        [
                            (node.position[0] + node.icon[0] - center[0]) * scale,
                            (node.position[1] + node.icon[1] - center[1]) * scale,
                        ]
                    };
                    let p = origin(target);
                    let dx = hit.x as f64 - p[0];
                    let dy = hit.y as f64 - p[1];
                    let mut digests = HashSet::new();
                    let mut evidence = vec![];
                    for other in &folder.nodes {
                        if other.asset.digest == target.asset.digest
                            || digests.contains(&other.asset.digest)
                        {
                            continue;
                        }
                        let p = origin(other);
                        let x = (dx + p[0]).round_ties_even();
                        let y = (dy + p[1]).round_ties_even();
                        let other_pixels = template(&other.asset, scale);
                        if x < 0.
                            || y < 0.
                            || x as usize + other_pixels.w > frame.width() as usize
                            || y as usize + other_pixels.h > frame.height() as usize
                            || other_pixels.pixels.len() < 80
                        {
                            continue;
                        }
                        if error(frame, &other_pixels, x as usize, y as usize, false).is_some() {
                            digests.insert(other.asset.digest.clone());
                            evidence.push(json!([
                                x,
                                y,
                                x + other_pixels.w as f64,
                                y + other_pixels.h as f64
                            ]));
                        }
                        if digests.len() >= 2 {
                            break;
                        }
                    }
                    if digests.len() < 2 {
                        continue;
                    }
                    let left = dx + target.position[0] * scale;
                    let top = dy + target.position[1] * scale;
                    let rect = [
                        left,
                        top,
                        left + target.position[2] * scale,
                        top + target.position[3] * scale,
                    ];
                    if rect[0].min(rect[1]) < 0.
                        || rect[2] > frame.width() as f64
                        || rect[3] > frame.height() as f64
                    {
                        continue;
                    }
                    let mut m = proposal.clone();
                    m["rect"] = json!(rect);
                    m["evidence_rects"] = json!(evidence);
                    m["basis"] = json!("sprite_and_two_gui_landmarks");
                    m["score"] = json!(hit.score);
                    m["landmarks"] = json!(digests.len());
                    candidates.push(m);
                }
                if !candidates.is_empty() {
                    break;
                }
            }
            if candidates.len() == 1 {
                result.push(candidates.remove(0));
            }
        }
        result
    }
    pub(crate) fn navigation(
        &self,
        frame: &RgbImage,
        proposals: &Value,
        node_matches: &Value,
    ) -> Vec<Value> {
        let visible: HashSet<_> = rows(node_matches)
            .iter()
            .filter(|m| m["panel"] == "research")
            .map(|m| s(&m["folder"]))
            .collect();
        let mut requested: Vec<(&str, Vec<&Value>)> = vec![];
        for p in rows(proposals) {
            let folder = s(&p["folder"]);
            if p["panel"] == "research"
                && p["status"] != "active"
                && !folder.is_empty()
                && !visible.contains(folder)
            {
                if let Some((_, steps)) = requested.iter_mut().find(|(f, _)| *f == folder) {
                    steps.push(p);
                } else {
                    requested.push((folder, vec![p]));
                }
            }
        }
        if requested.is_empty() {
            return vec![];
        }
        let region = (frame.height() as f64 * 0.3).round_ties_even().max(1.) as usize;
        let mut candidates: Vec<(String, Vec<Value>)> = vec![];
        for scale in [1., 0.8, 1.25, 1.5] {
            let mut cache: HashMap<usize, Vec<Hit>> = HashMap::new();
            for (index, tab) in self.tabs.iter().enumerate() {
                if self
                    .tabs
                    .iter()
                    .any(|other| other.id != tab.id && other.asset.digest == tab.asset.digest)
                {
                    continue;
                }
                let Some((_, steps)) = requested.iter().find(|(id, _)| *id == tab.id) else {
                    continue;
                };
                let hits = cache
                    .entry(index)
                    .or_insert_with(|| search(frame, &template(&tab.asset, scale), region, true))
                    .clone();
                for hit in hits {
                    let dx = hit.x as f64 - (tab.order + tab.offset[0]) * scale;
                    let dy = hit.y as f64 - tab.offset[1] * scale;
                    let mut peers: Vec<(String, Value)> = vec![];
                    for (other_index, other) in self.tabs.iter().enumerate() {
                        if self.tabs.iter().any(|peer| {
                            peer.id != other.id && peer.asset.digest == other.asset.digest
                        }) {
                            continue;
                        }
                        if other.asset.digest == tab.asset.digest {
                            continue;
                        }
                        let other_hits = cache.entry(other_index).or_insert_with(|| {
                            search(frame, &template(&other.asset, scale), region, true)
                        });
                        for p in other_hits {
                            let px = p.x as f64 - (other.order + other.offset[0]) * scale;
                            let py = p.y as f64 - other.offset[1] * scale;
                            if (px - dx).abs() <= 2.
                                && (py - dy).abs() <= 2.
                                && !peers
                                    .iter()
                                    .any(|(digest, _)| *digest == other.asset.digest)
                            {
                                let t = template(&other.asset, scale);
                                peers.push((
                                    other.asset.digest.clone(),
                                    json!([p.x, p.y, p.x + t.w, p.y + t.h]),
                                ));
                            }
                        }
                        if peers.len() >= 2 {
                            break;
                        }
                    }
                    if peers.len() < 2 {
                        continue;
                    }
                    let t = template(&tab.asset, scale);
                    let m = json!({"key":format!("research-tab:{}",tab.id),"panel":"research_tab","identity":tab.id,"folder":tab.id,"title":tab.name,"status":"ready","rect":[hit.x,hit.y,hit.x+t.w,hit.y+t.h],"step_keys":steps.iter().map(|p|&p["key"]).collect::<Vec<_>>(),"count":steps.len(),"basis":"tab_sprite_and_two_gui_peers","evidence_rects":peers.into_iter().take(2).map(|(_,r)|r).collect::<Vec<_>>()});
                    if let Some((_, matches)) = candidates.iter_mut().find(|(id, _)| *id == tab.id)
                    {
                        matches.push(m);
                    } else {
                        candidates.push((tab.id.clone(), vec![m]));
                    }
                }
            }
            if !candidates.is_empty() {
                break;
            }
        }
        candidates
            .into_iter()
            .filter_map(|(_, mut matches)| {
                if matches.len() == 1 {
                    Some(matches.remove(0))
                } else {
                    None
                }
            })
            .collect()
    }
}

impl Vision {
    pub(crate) fn render(&mut self, args: &Value, bgra: &[u8]) -> Result<Value> {
        let width = args["width"]
            .as_u64()
            .filter(|v| *v > 0 && *v <= 8192)
            .ok_or("Dimensioni frame overlay non valide")?;
        let height = args["height"]
            .as_u64()
            .filter(|v| *v > 0 && *v <= 8192)
            .ok_or("Dimensioni frame overlay non valide")?;
        let expected = width * height * 4;
        if expected > 80_000_000
            || bgra.len() as u64 != expected
            || args["raw_len"].as_u64() != Some(expected)
        {
            return Err("Dimensione payload BGRA non valida".into());
        }
        let view = &args["view"];
        if view
            .as_array()
            .is_none_or(|v| v.len() != 6 || v.iter().any(|n| n.as_i64().is_none()))
            || view[0].as_u64().is_none_or(|v| v == 0)
            || view[1].as_u64().is_none_or(|v| v == 0)
            || view[4].as_u64() != Some(width)
            || view[5].as_u64() != Some(height)
        {
            return Err("Identità finestra overlay non valida".into());
        }
        crate::hud_ocr::validate(&args["ocr"], width, height)?;
        let began = std::time::Instant::now();
        let digest = format!(
            "{:x}",
            Sha256::digest(&bgra[(height.min(80) * width * 4) as usize..])
        );
        let key = json!([
            view,
            args["version"],
            args["guide_id"],
            args["guide_server_id"],
            args["proposals"],
            digest,
            args["ocr"]
        ]);
        if let Some((old, matches)) = &self.cached
            && *old == key
        {
            return Ok(
                json!({"matches":matches,"cache_hit":true,"recognition_ms":began.elapsed().as_secs_f64()*1000.}),
            );
        }
        let mut rgb = Vec::with_capacity((width * height * 3) as usize);
        for pixel in bgra.as_chunks::<4>().0 {
            rgb.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
        }
        let frame =
            RgbImage::from_raw(width as u32, height as u32, rgb).ok_or("Truncated visual frame")?;
        let proposals = &args["proposals"];
        let mut matches =
            crate::hud_geometry::rows(&crate::hud_ocr::rectangles(&args["ocr"], proposals))
                .to_vec();
        let mut visual = self.matches(&frame, proposals, &args["ocr"]);
        let mut node_matches = matches.clone();
        node_matches.extend(visual.clone());
        visual.extend(self.navigation(&frame, proposals, &json!(node_matches)));
        let keys: HashSet<_> = visual
            .iter()
            .map(|m| m["key"].clone().to_string())
            .collect();
        matches.retain(|m| !keys.contains(&m["key"].to_string()));
        matches.extend(visual);
        let matches = json!(matches);
        self.cached = Some((key, matches.clone()));
        Ok(
            json!({"matches":matches,"cache_hit":false,"recognition_ms":began.elapsed().as_secs_f64()*1000.}),
        )
    }
}
static VISIONS: OnceLock<Mutex<HashMap<u64, Arc<Mutex<Vision>>>>> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
fn visions() -> &'static Mutex<HashMap<u64, Arc<Mutex<Vision>>>> {
    VISIONS.get_or_init(Default::default)
}
pub(crate) fn execute(op: &str, args: &Value) -> Result<Value> {
    if op == "vision_open" {
        let vision = Vision::new(args)?;
        let mut owners = visions()
            .lock()
            .map_err(|_| "Vision registry unavailable")?;
        if owners.len() >= 8 {
            return Err("Too many visual observers".into());
        }
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        owners.insert(id, Arc::new(Mutex::new(vision)));
        return Ok(json!(id));
    }
    let id = args["handle"]
        .as_u64()
        .ok_or("Missing visual observer handle")?;
    if op == "vision_release" {
        return Ok(json!(
            visions()
                .lock()
                .map_err(|_| "Vision registry unavailable")?
                .remove(&id)
                .is_some()
        ));
    }
    Err("Unknown visual observer operation".into())
}
/// # Safety
/// The JSON and BGRA buffers remain readable for their declared lengths until
/// return. No caller pointer or captured frame is retained by the observer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoi4_backend_hud_frame(
    handle: u64,
    data: *const u8,
    size: usize,
    pixels: *const u8,
    pixel_size: usize,
) -> *mut std::ffi::c_char {
    crate::reply(
        std::panic::catch_unwind(|| {
            if data.is_null()
                || size == 0
                || size > 1024 * 1024
                || pixels.is_null()
                || pixel_size == 0
                || pixel_size > 80_000_000
            {
                return Err("Invalid visual observer buffers".into());
            }
            let args: Value =
                serde_json::from_slice(unsafe { std::slice::from_raw_parts(data, size) })
                    .map_err(|e| e.to_string())?;
            let vision = visions()
                .lock()
                .map_err(|_| "Vision registry unavailable")?
                .get(&handle)
                .cloned()
                .ok_or("Unknown visual observer handle")?;
            let result = vision
                .lock()
                .map_err(|_| "Visual observer unavailable")?
                .render(&args, unsafe {
                    std::slice::from_raw_parts(pixels, pixel_size)
                })?;
            Ok(result)
        })
        .unwrap_or_else(|_| Err("Visual observation refused".into())),
    )
}
pub(crate) fn fixture(args: &Value) -> Result<Value> {
    let width = args["frame"]["width"]
        .as_u64()
        .filter(|v| *v > 0 && *v <= 8192)
        .ok_or("Invalid visual width")?;
    let height = args["frame"]["height"]
        .as_u64()
        .filter(|v| *v > 0 && *v <= 8192)
        .ok_or("Invalid visual height")?;
    let size = width * height * 3;
    if size > 80_000_000 {
        return Err("Visual frame exceeds limit".into());
    }
    let raw = decode_hex(s(&args["frame"]["rgb"]), size as usize)?;
    let frame =
        RgbImage::from_raw(width as u32, height as u32, raw).ok_or("Truncated visual frame")?;
    let vision = Vision::new(args)?;
    let matches = vision.matches(&frame, &args["proposals"], &args["ocr"]);
    let navigation = vision.navigation(&frame, &args["proposals"], &args["node_matches"]);
    Ok(json!({"matches":matches,"navigation":navigation}))
}
