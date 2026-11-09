//! Exact visible OCR labels are only visual location evidence. No fuzzy
//! identity, guessed panels or progress mutation is permitted here.
use crate::{hud_geometry::rows, hud_targets::text_key};
use serde_json::{Value, json};
#[derive(Clone)]
struct Line {
    text: String,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}
fn number(v: &Value) -> Option<f64> {
    v.as_f64().filter(|v| v.is_finite())
}
pub(crate) fn validate(value: &Value, width: u64, height: u64) -> Result<(), String> {
    if value["width"].as_u64() != Some(width)
        || value["height"].as_u64() != Some(height)
        || value["lines"].as_array().is_none_or(|v| v.len() > 20000)
    {
        return Err("Documento OCR non valido".into());
    }
    let mut count = 0;
    for line in rows(&value["lines"]) {
        if !line["text"].is_string() || !line["words"].is_array() {
            return Err("Riga OCR non valida".into());
        }
        count += rows(&line["words"]).len();
        if count > 100000 {
            return Err("Troppe parole OCR".into());
        }
        for word in rows(&line["words"]) {
            if !word.is_object()
                || ["x", "y", "width", "height"]
                    .iter()
                    .any(|k| number(&word[k]).is_none())
            {
                return Err("Parola OCR non valida".into());
            }
        }
    }
    Ok(())
}
pub(crate) fn rectangles(ocr: &Value, proposals: &Value) -> Value {
    let Some(width) = number(&ocr["width"]).filter(|v| *v > 0. && *v <= 8192.) else {
        return json!([]);
    };
    let Some(height) = number(&ocr["height"]).filter(|v| *v > 0. && *v <= 8192.) else {
        return json!([]);
    };
    let mut lines = vec![];
    for line in rows(&ocr["lines"]) {
        let words = rows(&line["words"]);
        if words.is_empty() {
            continue;
        }
        let mut boxes = vec![];
        for w in words {
            let fields = ["x", "y", "width", "height"].map(|k| number(&w[k]));
            if fields.iter().any(Option::is_none) {
                boxes.clear();
                break;
            }
            let [x, y, ww, h] = fields.map(Option::unwrap);
            if x < 0. || y < 0. || ww <= 0. || h <= 0. || x + ww > width || y + h > height {
                boxes.clear();
                break;
            }
            boxes.push([x, y, ww, h]);
        }
        if boxes.len() != words.len() {
            continue;
        }
        let x = boxes.iter().map(|b| b[0]).fold(f64::INFINITY, f64::min);
        let y = boxes.iter().map(|b| b[1]).fold(f64::INFINITY, f64::min);
        lines.push(Line {
            text: line["text"].as_str().unwrap_or("").into(),
            x,
            y,
            w: boxes
                .iter()
                .map(|b| b[0] + b[2])
                .fold(f64::NEG_INFINITY, f64::max)
                - x,
            h: boxes
                .iter()
                .map(|b| b[1] + b[3])
                .fold(f64::NEG_INFINITY, f64::max)
                - y,
        });
    }
    let keys: Vec<_> = lines.iter().map(|l| text_key(&l.text)).collect();
    let joined = keys.concat();
    let mut found = vec![];
    for proposal in rows(proposals) {
        let panel = proposal["panel"].as_str().unwrap_or("");
        if ["politics", "projects"].contains(&panel) {
            continue;
        }
        if panel == "event" {
            if crate::state::truth(&proposal["watch"])
                && !crate::state::truth(&proposal["live_visible"])
            {
                continue;
            }
            let titles: Vec<_> = std::iter::once(&proposal["event_title"])
                .chain(rows(&proposal["event_titles"]).iter())
                .filter_map(Value::as_str)
                .map(text_key)
                .filter(|s| !s.is_empty())
                .collect();
            if !keys.iter().any(|key| titles.contains(key)) {
                continue;
            }
        } else if panel == "focus" {
            let header: Vec<_> = lines
                .iter()
                .filter(|l| l.y < 150_f64.max(height * 0.12))
                .collect();
            let italian = header.iter().any(|a| {
                text_key(&a.text) == "focus"
                    && header
                        .iter()
                        .any(|b| text_key(&b.text).contains("nazionali") && (a.y - b.y).abs() < 30.)
            });
            if !italian
                && !header.iter().any(|l| {
                    let key = text_key(&l.text);
                    key.contains("nationalfocus") || key.contains("focusnazionali")
                })
            {
                continue;
            }
        } else {
            let anchors: &[&str] = match panel {
                "research" => &[
                    "ricerca",
                    "research",
                    "armi ed equipaggiamento",
                    "weapons and equipment",
                    "ingegneria elettronica",
                ],
                "production" => &["produzione", "production"],
                _ => &[],
            };
            if !anchors.iter().any(|a| joined.contains(&text_key(a))) {
                continue;
            }
        }
        let wanted: Vec<_> = rows(&proposal["labels"])
            .iter()
            .filter_map(Value::as_str)
            .map(text_key)
            .collect();
        let mut matches = vec![];
        for line in &lines {
            let mut combined = vec![line];
            let mut current = line;
            for _ in 0..3 {
                let text = combined
                    .iter()
                    .map(|l| l.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                if wanted.contains(&text_key(&text)) {
                    let b = [
                        combined.iter().map(|l| l.x).fold(f64::INFINITY, f64::min),
                        combined.iter().map(|l| l.y).fold(f64::INFINITY, f64::min),
                        combined
                            .iter()
                            .map(|l| l.x + l.w)
                            .fold(f64::NEG_INFINITY, f64::max),
                        combined
                            .iter()
                            .map(|l| l.y + l.h)
                            .fold(f64::NEG_INFINITY, f64::max),
                    ];
                    if !matches.contains(&b) {
                        matches.push(b);
                    }
                    break;
                }
                let following: Vec<_> = lines
                    .iter()
                    .filter(|other| {
                        let gap = other.y - (current.y + current.h);
                        gap >= 0.
                            && gap <= 10_f64.max(current.h * 0.8)
                            && ((other.x + other.w / 2.) - (current.x + current.w / 2.)).abs()
                                < current.w.max(other.w) / 2.
                    })
                    .collect();
                if following.len() != 1 {
                    break;
                }
                current = following[0];
                combined.push(current);
            }
        }
        if matches.len() == 1 {
            let [x, y, right, bottom] = matches[0];
            let mut p = proposal.clone();
            p["rect"] = json!([
                (x - 4.).max(0.),
                (y - 3.).max(0.),
                (right + 4.).min(width),
                (bottom + 3.).min(height)
            ]);
            p["basis"] = json!("exact_unique_ocr");
            found.push(p);
        }
    }
    json!(
        found
            .iter()
            .filter(|item| found
                .iter()
                .filter(|other| other["rect"] == item["rect"])
                .all(|other| other["identity"] == item["identity"]))
            .collect::<Vec<_>>()
    )
}
