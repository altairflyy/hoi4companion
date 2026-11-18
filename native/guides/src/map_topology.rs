//! Static land borders with mod overlays and a small generation-keyed cache.
//! Pixel adjacency is geometry, never evidence of live ownership or control.
use crate::installation;
use image::ImageReader;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};
type Result<T> = std::result::Result<T, String>;
type Cache = VecDeque<(String, Value)>;
static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
fn records(path: &Path) -> Result<csv::StringRecordsIntoIter<std::fs::File>> {
    Ok(csv::ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(false)
        .flexible(true)
        .from_path(path)
        .map_err(|e| e.to_string())?
        .into_records())
}
fn stamp(path: &Path) -> Result<Value> {
    let meta = path.metadata().map_err(|e| e.to_string())?;
    let time = meta
        .modified()
        .map_err(|e| e.to_string())?
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?;
    Ok(json!([
        installation::display(path),
        meta.len(),
        time.as_secs(),
        time.subsec_nanos()
    ]))
}
fn load(definition: &Path, bitmap: &Path, adjacencies: Option<&Path>) -> Result<Value> {
    let mut definitions = HashMap::new();
    for row in records(definition)? {
        let row = row.map_err(|e| e.to_string())?;
        let Some(id) = row
            .get(0)
            .map(|s| s.trim_start_matches('\u{feff}'))
            .and_then(|s| s.parse::<u64>().ok())
            .filter(|n| *n > 0)
        else {
            continue;
        };
        if row.len() < 5 {
            continue;
        }
        let rgb: Vec<_> = row
            .iter()
            .skip(1)
            .take(3)
            .map(|s| s.parse::<i64>())
            .collect::<std::result::Result<_, _>>()
            .map_err(|e| e.to_string())?;
        if rgb.iter().any(|n| !(0..=255).contains(n)) {
            continue;
        }
        if row[4].eq_ignore_ascii_case("land") {
            let color = ((rgb[0] as u32) << 16) | ((rgb[1] as u32) << 8) | (rgb[2] as u32);
            if definitions.insert(color, id).is_some() {
                return Err("Duplicate land province color".into());
            }
        }
    }
    let dimensions = ImageReader::open(bitmap)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .into_dimensions()
        .map_err(|e| e.to_string())?;
    let (width, height) = dimensions;
    if u64::from(width) * u64::from(height) > 100_000_000 {
        return Err("Province map exceeds topology limit".into());
    }
    let mut reader = ImageReader::open(bitmap)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(512_000_000);
    reader.limits(limits);
    let image = reader.decode().map_err(|e| e.to_string())?.to_rgb8();
    let mut graph: BTreeMap<u64, BTreeSet<u64>> = BTreeMap::new();
    let mut present = HashSet::new();
    let color = |x, y| {
        let p = image.get_pixel(x, y).0;
        (u32::from(p[0]) << 16) | (u32::from(p[1]) << 8) | u32::from(p[2])
    };
    for y in 0..height {
        for x in 0..width {
            let current = color(x, y);
            if let Some(&id) = definitions.get(&current) {
                present.insert(id);
                graph.entry(id).or_default();
            }
            for other in [
                Some(color((x + 1) % width, y)),
                (y > 0).then(|| color(x, y - 1)),
            ]
            .into_iter()
            .flatten()
            {
                if current != other
                    && let (Some(&a), Some(&b)) =
                        (definitions.get(&current), definitions.get(&other))
                {
                    graph.entry(a).or_default().insert(b);
                    graph.entry(b).or_default().insert(a);
                }
            }
        }
    }
    graph.retain(|id, _| present.contains(id));
    if let Some(path) = adjacencies {
        for row in records(path)? {
            let row = row.map_err(|e| e.to_string())?;
            if row.len() < 3 || !row[2].eq_ignore_ascii_case("impassable") {
                continue;
            }
            if let (Ok(a), Ok(b)) = (row[0].parse::<u64>(), row[1].parse::<u64>()) {
                if let Some(v) = graph.get_mut(&a) {
                    v.remove(&b);
                }
                if let Some(v) = graph.get_mut(&b) {
                    v.remove(&a);
                }
            }
        }
    }
    Ok(Value::Object(
        graph
            .into_iter()
            .map(|(id, neighbors)| (id.to_string(), json!(neighbors)))
            .collect(),
    ))
}
pub fn neighbors(game: &Path, mods: &[PathBuf]) -> Result<Value> {
    let mut selected = HashMap::new();
    for suffix in [".bmp", ".csv"] {
        let files = installation::collect(game, mods, "map", suffix)?;
        for file in files["files"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            let path = PathBuf::from(file);
            selected.insert(
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                path,
            );
        }
    }
    let (Some(definition), Some(bitmap)) = (
        selected.get("definition.csv"),
        selected.get("provinces.bmp"),
    ) else {
        return Ok(json!({}));
    };
    let adjacency = selected.get("adjacencies.csv");
    let key = json!([
        stamp(definition)?,
        stamp(bitmap)?,
        adjacency.map(|p| stamp(p)).transpose()?
    ])
    .to_string();
    let cache = CACHE.get_or_init(|| Mutex::new(VecDeque::new()));
    if let Some(value) = cache
        .lock()
        .map_err(|_| "Topology cache unavailable")?
        .iter()
        .find_map(|(k, v)| (k == &key).then(|| v.clone()))
    {
        return Ok(value);
    }
    let value = load(definition, bitmap, adjacency.map(PathBuf::as_path))?;
    let mut cache = cache.lock().map_err(|_| "Topology cache unavailable")?;
    cache.push_back((key, value.clone()));
    while cache.len() > 2 {
        cache.pop_front();
    }
    Ok(value)
}
