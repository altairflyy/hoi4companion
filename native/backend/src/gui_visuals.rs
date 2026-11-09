//! Static GUI/texture evidence. Geometry never establishes gameplay completion.
use crate::{
    assets::Assets,
    installation,
    script_document::{self, entries},
    source_eligibility::number,
};
use serde_json::{Map, Value, json};
use std::{collections::HashMap, io::Read, path::Path};
type Result<T> = std::result::Result<T, String>;
pub fn field<'a>(node: &'a Value, key: &str) -> Option<&'a Value> {
    entries(node).find_map(|(k, v)| k.eq_ignore_ascii_case(key).then_some(v))
}
pub fn text<'a>(node: &'a Value, key: &str) -> &'a str {
    field(node, key).and_then(Value::as_str).unwrap_or("")
}
pub fn numeric(node: &Value, key: &str) -> Option<f64> {
    number(text(node, key))
}
pub fn point(node: &Value, key: &str, names: (&str, &str)) -> Value {
    let child = field(node, key).unwrap_or(&Value::Null);
    let mut result = json!({});
    result[names.0] = json!(numeric(child, names.0));
    result[names.1] = json!(numeric(child, names.1));
    result
}
pub fn walk<'a>(node: &'a Value, values: &mut Vec<(String, &'a Value)>) {
    for (key, value) in entries(node) {
        if value.is_object() {
            values.push((key.to_lowercase(), value));
            walk(value, values);
        }
    }
}
pub fn dimensions(path: &Path, frames: u64) -> Option<Value> {
    let mut data = vec![];
    std::fs::File::open(path)
        .ok()?
        .take(128)
        .read_to_end(&mut data)
        .ok()?;
    let (width, height) = if data.starts_with(b"DDS ") && data.len() >= 128 {
        (
            u32::from_le_bytes(data[16..20].try_into().ok()?) as u64,
            u32::from_le_bytes(data[12..16].try_into().ok()?) as u64,
        )
    } else if data.starts_with(b"\x89PNG\r\n\x1a\n") && data.len() >= 24 {
        (
            u32::from_be_bytes(data[16..20].try_into().ok()?) as u64,
            u32::from_be_bytes(data[20..24].try_into().ok()?) as u64,
        )
    } else if path
        .extension()
        .is_some_and(|s| s.eq_ignore_ascii_case("tga"))
        && data.len() >= 18
    {
        (
            u16::from_le_bytes(data[12..14].try_into().ok()?) as u64,
            u16::from_le_bytes(data[14..16].try_into().ok()?) as u64,
        )
    } else {
        return None;
    };
    if width == 0
        || height == 0
        || width.checked_mul(height)? > 8_000_000
        || !(1..=256).contains(&frames)
        || !width.is_multiple_of(frames)
    {
        return None;
    }
    Some(json!({"width":width/frames,"height":height}))
}
fn asset(
    library: &mut Assets,
    measured: &mut HashMap<String, Option<Value>>,
    name: &str,
    frame: &Value,
) -> Result<Value> {
    let Some(frame) = frame
        .as_f64()
        .filter(|v| v.is_finite() && v.fract() == 0. && *v >= 1. && *v <= 256.)
    else {
        return Ok(Value::Null);
    };
    let packet = library.resolve(name, frame as u64)?;
    let mut value = packet["sprite"].clone();
    if value.is_null() {
        return Ok(value);
    }
    let size = measured.entry(name.into()).or_insert_with(|| {
        packet["asset"]["path"].as_str().and_then(|p| {
            dimensions(
                Path::new(p),
                packet["asset"]["frames"].as_u64().unwrap_or(0),
            )
        })
    });
    if let Some(size) = size {
        value
            .as_object_mut()
            .unwrap()
            .extend(size.as_object().unwrap().clone());
    }
    Ok(value)
}
fn has_word(source: &str, word: &str) -> bool {
    source.match_indices(word).any(|(i, _)| {
        let before = source[..i].chars().next_back();
        let after = source[i + word.len()..].chars().next();
        before.is_none_or(|c| !c.is_alphanumeric() && c != '_')
            && after.is_none_or(|c| !c.is_alphanumeric() && c != '_')
    })
}
pub fn focus(files: &Value, library: Option<&mut Assets>) -> Result<Value> {
    const POSITIONS: [&str; 9] = [
        "focus_spacing",
        "national_focus_center",
        "link_spacing",
        "link_offsets",
        "link_begin",
        "link_end",
        "exclusive_offset",
        "exclusive_offset_left",
        "exclusive_positioning",
    ];
    const TEMPLATES: [&str; 3] = [
        "national_focus_item",
        "national_focus_link",
        "national_focus_exclusive_item",
    ];
    let mut result = json!({"schema_version":1,"basis":"installed_gui","status":"unavailable","positions":{},"templates":{},"states":{},"warnings":[]});
    for path in files.as_array().ok_or("Missing focus GUI files")? {
        let path = Path::new(path.as_str().ok_or("Invalid GUI path")?);
        let parsed = (|| -> Result<Option<Value>> {
            let mut data = vec![];
            std::fs::File::open(path)
                .map_err(|e| e.to_string())?
                .take(80_000_001)
                .read_to_end(&mut data)
                .map_err(|e| e.to_string())?;
            let source = String::from_utf8_lossy(&data);
            if !POSITIONS
                .iter()
                .chain(TEMPLATES.iter())
                .any(|key| has_word(&source, key))
            {
                return Ok(None);
            }
            script_document::read(path).map(Some)
        })();
        let root = match parsed {
            Ok(Some(root)) => root,
            Ok(None) => continue,
            Err(error) => {
                result["warnings"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!(format!(
                        "Focus GUI non leggibile ({}): {error}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    )));
                continue;
            }
        };
        let mut nodes = vec![];
        walk(&root, &mut nodes);
        for (kind, node) in nodes {
            let name = text(node, "name");
            if POSITIONS.contains(&name) && kind == "positiontype" {
                let p = point(node, "position", ("x", "y"));
                if ["x", "y"]
                    .iter()
                    .all(|k| p[*k].as_f64().is_some_and(|v| v.abs() <= 65536.))
                {
                    let mut p = p;
                    p["source"] = json!(installation::display(path));
                    result["positions"][name] = p;
                } else {
                    result["positions"].as_object_mut().unwrap().remove(name);
                }
            } else if TEMPLATES.contains(&name) && kind == "containerwindowtype" {
                let mut children = Map::new();
                for (child_kind, child) in entries(node) {
                    if !["icontype", "buttontype", "instanttextboxtype"]
                        .contains(&child_kind.to_lowercase().as_str())
                    {
                        continue;
                    }
                    let child_name = text(child, "name");
                    if child_name.is_empty() {
                        continue;
                    }
                    let sprite = if text(child, "spritetype").is_empty() {
                        text(child, "quadtexturesprite")
                    } else {
                        text(child, "spritetype")
                    };
                    children.insert(child_name.into(),json!({"position":point(child,"position",("x","y")),"sprite_name":sprite,"frame":numeric(child,"frame").unwrap_or(1.),"center":text(child,"centerposition")=="yes","orientation":field(child,"orientation").and_then(Value::as_str).unwrap_or("UPPER_LEFT").to_uppercase(),"scale":numeric(child,"scale").unwrap_or(1.),"width":numeric(child,"maxwidth"),"height":numeric(child,"maxheight"),"font":text(child,"font"),"align":field(child,"format").and_then(Value::as_str).unwrap_or("center")}));
                }
                result["templates"][name] = json!({"size":point(node,"size",("width","height")),"position":point(node,"position",("x","y")),"children":children,"source":installation::display(path)});
            }
        }
    }
    if [
        result["positions"]["focus_spacing"]["x"].as_f64(),
        result["positions"]["focus_spacing"]["y"].as_f64(),
        result["templates"]["national_focus_item"]["size"]["width"].as_f64(),
        result["templates"]["national_focus_item"]["size"]["height"].as_f64(),
    ]
    .iter()
    .all(|v| v.is_some_and(|v| v > 0. && v <= 4096.))
    {
        result["status"] = json!("observed");
    }
    if let Some(library) = library {
        let mut measured = HashMap::new();
        for (key, suffix) in [
            ("locked", "unavailable"),
            ("unknown", "unavailable"),
            ("verify", "unavailable"),
            ("excluded", "unavailable"),
            ("ready", "can_start"),
            ("active", "current"),
            ("completed", "completed"),
        ] {
            let value = asset(
                library,
                &mut measured,
                &format!("GFX_focus_{suffix}"),
                &json!(1),
            )?;
            if !value.is_null() {
                result["states"][key] = value;
            }
        }
        let mut states = json!({});
        for (state, frame) in [
            ("neutral", 3),
            ("alternative", 4),
            ("completed", 1),
            ("completed_alternative", 2),
        ] {
            let mut links = json!({});
            for suffix in [
                "up_down",
                "left_right",
                "down_right",
                "down_left",
                "up_right",
                "up_left",
                "down_left_right",
                "up_left_right",
                "up_down_left",
                "up_down_right",
                "up_down_left_right",
            ] {
                let value = asset(
                    library,
                    &mut measured,
                    &format!("GFX_focus_link_{suffix}"),
                    &json!(frame),
                )?;
                if !value.is_null() {
                    links[suffix] = value;
                }
            }
            states[state] = links;
        }
        result["links"] = states["neutral"].clone();
        result["link_states"] = states;
        for template in result["templates"].as_object_mut().unwrap().values_mut() {
            for child in template["children"].as_object_mut().unwrap().values_mut() {
                child["sprite"] = asset(
                    library,
                    &mut measured,
                    child["sprite_name"].as_str().unwrap_or(""),
                    &child["frame"],
                )?;
            }
        }
    }
    Ok(result)
}
