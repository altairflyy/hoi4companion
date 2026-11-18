//! GUI-referenced sprite evidence and decoration layers; headers are measured
//! lazily per generation, without opening thousands of unrelated textures.
use crate::{
    assets::Assets,
    gui_visuals::{dimensions, field, numeric, point, text, walk},
    installation,
    script_document::{self, entries},
    source_eligibility::number,
};
use serde_json::{Map, Value, json};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};
type Result<T> = std::result::Result<T, String>;
const STATES: [&str; 4] = [
    "unavailable",
    "available",
    "researched",
    "currently_researching",
];
fn texture_path(gfx: &Path, texture: &str) -> Option<PathBuf> {
    let relative = texture.replace('\\', "/");
    if relative.starts_with('/') || relative.contains(':') || relative.split('/').any(|s| s == "..")
    {
        return None;
    }
    let interface = gfx.ancestors().skip(1).find(|p| {
        p.file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("interface"))
    })?;
    let root = interface.parent()?.canonicalize().ok()?;
    let relative = Path::new(&relative);
    for candidate in [relative.to_path_buf(), relative.with_extension("dds")] {
        if let Ok(path) = root.join(candidate).canonicalize()
            && path.starts_with(&root)
            && path.is_file()
        {
            return Some(path);
        }
    }
    None
}
struct Textures<'a> {
    sprites: Map<String, Value>,
    measured: HashSet<String>,
    reads: usize,
    library: Option<&'a mut Assets>,
}
impl Textures<'_> {
    fn asset(&mut self, name: &str) -> Result<Value> {
        if self.sprites.contains_key(name) && self.measured.insert(name.into()) {
            let item = self.sprites.get_mut(name).unwrap();
            let path = texture_path(
                Path::new(item["source"].as_str().ok_or("Invalid sprite source")?),
                item["texture"].as_str().ok_or("Invalid sprite texture")?,
            );
            if let Some(path) = path {
                self.reads += 1;
                if let Some(size) = dimensions(&path, item["frames"].as_u64().unwrap_or(0)) {
                    item.as_object_mut()
                        .unwrap()
                        .extend(size.as_object().unwrap().clone());
                }
            }
        }
        let mut value = self.sprites.get(name).cloned().unwrap_or(Value::Null);
        if !value.is_null()
            && let Some(library) = self.library.as_deref_mut()
        {
            let resolved = library.resolve(name, 1)?;
            if let Some(sprite) = resolved["sprite"].as_object() {
                value.as_object_mut().unwrap().extend(sprite.clone());
            }
        }
        Ok(value)
    }
}
fn metric(raw: &str, reference: f64) -> f64 {
    number(raw.trim_end_matches('%'))
        .map(|value| {
            if raw.ends_with('%') {
                value * reference / 100.
            } else {
                value
            }
        })
        .unwrap_or(0.)
}
fn layers(
    node: &Value,
    x: f64,
    y: f64,
    parent: &Value,
    textures: &mut Textures<'_>,
    out: &mut Vec<Value>,
) -> Result<()> {
    for (kind, child) in entries(node) {
        if !child.is_object() {
            continue;
        }
        let position = point(child, "position", ("x", "y"));
        let raw_position = field(child, "position").unwrap_or(&Value::Null);
        if ["x", "y"]
            .iter()
            .any(|k| position[*k].is_null() && !text(raw_position, k).is_empty())
        {
            continue;
        }
        let (cx, cy) = (
            x + position["x"].as_f64().unwrap_or(0.),
            y + position["y"].as_f64().unwrap_or(0.),
        );
        let sprite = if text(child, "spritetype").is_empty() {
            text(child, "quadtexturesprite")
        } else {
            text(child, "spritetype")
        };
        let decoration = sprite.contains("techtree_bg")
            || [
                "GFX_techtree_stripes",
                "GFX_wonderweapons_bg",
                "GFX_tiled_research_bg",
            ]
            .contains(&sprite);
        if decoration && text(child, "hide") != "yes" {
            let value = textures.asset(sprite)?;
            if !value.is_null() {
                let mut layer = json!({"name":text(child,"name"),"x":cx,"y":cy,"sprite_name":sprite,"sprite":value,"repeat":value["tile"]==true});
                if parent.is_object() && value["tile"] == true {
                    for (k, v) in parent.as_object().unwrap() {
                        if !v.is_null() {
                            layer[k] = v.clone();
                        }
                    }
                } else {
                    for key in ["width", "height"] {
                        if let Some(v) = value.get(key) {
                            layer[key] = v.clone();
                        }
                    }
                }
                out.push(layer);
            }
        }
        if kind.eq_ignore_ascii_case("containerwindowtype") {
            let size = field(child, "size").unwrap_or(&Value::Null);
            let minimum = field(size, "min").unwrap_or(&Value::Null);
            let mut resolved = json!({});
            for (key, default) in [("width", 2522.), ("height", 1276.)] {
                let reference = parent[key].as_f64().filter(|v| *v != 0.).unwrap_or(default);
                let value =
                    metric(text(size, key), reference).max(metric(text(minimum, key), reference));
                resolved[key] = json!(if value == 0. { reference } else { value });
            }
            layers(child, cx, cy, &resolved, textures, out)?;
        }
    }
    Ok(())
}
pub fn load(gui_files: &Value, gfx_files: &Value, library: Option<&mut Assets>) -> Result<Value> {
    let mut result = json!({"templates":{},"folders":{},"warnings":[],"basis":"installed_gui_gfx_texture_headers"});
    let mut sprites = Map::new();
    for path in gfx_files.as_array().ok_or("Missing graphics files")? {
        let path = Path::new(path.as_str().ok_or("Invalid graphics path")?);
        let loaded = (|| -> Result<()> {
            let root = script_document::read(path)?;
            let mut nodes = vec![];
            walk(&root, &mut nodes);
            for (kind, node) in nodes {
                if ![
                    "spritetype",
                    "frameanimatedspritetype",
                    "corneredtilespritetype",
                ]
                .contains(&kind.as_str())
                {
                    continue;
                }
                let name = text(node, "name");
                let texture = text(node, "texturefile");
                if name.is_empty() || texture.is_empty() {
                    continue;
                }
                let frames = field(node, "noofframes")
                    .and_then(Value::as_str)
                    .unwrap_or("1")
                    .trim()
                    .replace('_', "")
                    .parse::<u64>()
                    .map_err(|_| "Invalid sprite frame count")?;
                if !(1..=256).contains(&frames) {
                    continue;
                }
                let mut asset = json!({"name":name,"sprite_name":name,"texture":texture,"frames":frames,"frame":1,"source":installation::display(path),"kind":kind});
                if kind == "corneredtilespritetype" {
                    asset["tile"] = json!(text(node, "tilingcenter") == "yes");
                    asset["border"] = point(node, "bordersize", ("x", "y"));
                }
                sprites.insert(name.into(), asset);
            }
            Ok(())
        })();
        if let Err(error) = loaded {
            result["warnings"]
                .as_array_mut()
                .unwrap()
                .push(json!(format!(
                    "Technology graphics not readable ({}): {error}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                )));
        }
    }
    let mut textures = Textures {
        sprites,
        measured: HashSet::new(),
        reads: 0,
        library,
    };
    for path in gui_files.as_array().ok_or("Missing GUI files")? {
        let path = Path::new(path.as_str().ok_or("Invalid GUI path")?);
        let loaded = (|| -> Result<()> {
            let root = script_document::read(path)?;
            let mut nodes = vec![];
            walk(&root, &mut nodes);
            for (kind, node) in nodes {
                let name = text(node, "name");
                if kind != "containerwindowtype" {
                    continue;
                }
                if let Some(suffix) = name
                    .strip_prefix("techtree_")
                    .and_then(|n| n.strip_suffix("_item"))
                {
                    let small = suffix.ends_with("_small");
                    let folder = if small {
                        suffix.strip_suffix("_small").unwrap()
                    } else {
                        suffix
                    };
                    let icon = entries(node).find_map(|(k, v)| {
                        (k.eq_ignore_ascii_case("icontype")
                            && v.is_object()
                            && text(v, "name") == "Icon")
                            .then_some(v)
                    });
                    let Some(icon) = icon else {
                        continue;
                    };
                    let label = entries(node).find_map(|(k, v)| {
                        (k.eq_ignore_ascii_case("instanttextboxtype")
                            && v.is_object()
                            && text(v, "name") == "Name")
                            .then_some(v)
                    });
                    let prefix = format!(
                        "GFX_technology_{folder}_{}",
                        if small { "small_" } else { "" }
                    );
                    let (mut backgrounds, mut specials) = (json!({}), json!({}));
                    for state in STATES {
                        let mut value = textures.asset(&format!("{prefix}{state}_item_bg"))?;
                        if value.is_null() {
                            value = textures.asset(&format!("GFX_technology_{state}_item_bg"))?;
                        }
                        if !value.is_null() {
                            backgrounds[state] = value;
                        }
                        let value = textures.asset(&format!(
                            "GFX_technology_{folder}_special_project_{}{state}_item_bg",
                            if small { "small_" } else { "" }
                        ))?;
                        if !value.is_null() {
                            specials[state] = value;
                        }
                    }
                    let declared = point(node, "size", ("width", "height"));
                    let normal = &backgrounds["unavailable"];
                    let mut size = json!({});
                    for key in ["width", "height"] {
                        size[key] = normal.get(key).unwrap_or(&declared[key]).clone();
                    }
                    let mut icon_position = point(icon, "position", ("x", "y"));
                    icon_position["centered"] = json!(text(icon, "centerposition") == "yes");
                    let mut template = json!({"name":name,"folder_id":folder,"variant":if label.is_some(){"equipment"}else{"small"},"position":point(node,"position",("x","y")),"declared_size":declared,"size":size,"icon":icon_position,"states":backgrounds,"special_project_states":specials,"source":installation::display(path)});
                    if let Some(label) = label {
                        let mut label_position = point(label, "position", ("x", "y"));
                        label_position["max_width"] = json!(numeric(label, "maxwidth"));
                        label_position["max_height"] = json!(numeric(label, "maxheight"));
                        label_position["font"] = json!(text(label, "font"));
                        template["label"] = label_position;
                    }
                    result["templates"][name] = template;
                } else if name.ends_with("folder") {
                    let mut output = vec![];
                    layers(
                        node,
                        0.,
                        0.,
                        &json!({"width":2522,"height":1276}),
                        &mut textures,
                        &mut output,
                    )?;
                    if !output.is_empty() {
                        result["folders"][name] =
                            json!({"layers":output,"source":installation::display(path)});
                    }
                }
            }
            Ok(())
        })();
        if let Err(error) = loaded {
            result["warnings"]
                .as_array_mut()
                .unwrap()
                .push(json!(format!(
                    "Technology visuals not readable ({}): {error}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                )));
        }
    }
    Ok(json!({"value":result,"header_reads":textures.reads}))
}
