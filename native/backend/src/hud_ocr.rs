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
/// An event title is on screen: exactly, or, when the game fills script
/// placeholders ("[TUR.GetNameDefCap] prende il controllo del Bosforo!"), every
/// literal part in order on one line or two consecutive lines. The literal
/// parts must carry at least 8 letters, so a title made of placeholders never
/// matches.
fn title_seen(title: &str, keys: &[String]) -> bool {
    if !title.contains('[') {
        let key = text_key(title);
        return !key.is_empty() && keys.contains(&key);
    }
    let mut fragments = vec![];
    let mut literal = String::new();
    let mut depth = 0;
    for c in title.chars() {
        match c {
            '[' => {
                depth += 1;
                fragments.push(text_key(&std::mem::take(&mut literal)));
            }
            ']' if depth > 0 => depth -= 1,
            _ if depth == 0 => literal.push(c),
            _ => {}
        }
    }
    fragments.push(text_key(&literal));
    fragments.retain(|f| f.chars().count() >= 3);
    if fragments.iter().map(|f| f.chars().count()).sum::<usize>() < 8 {
        return false;
    }
    let in_order = |key: &str| {
        let mut rest = key;
        fragments.iter().all(|f| match rest.find(f.as_str()) {
            Some(at) => {
                rest = &rest[at + f.len()..];
                true
            }
            None => false,
        })
    };
    keys.iter().any(|k| in_order(k)) || keys.windows(2).any(|w| in_order(&w.concat()))
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
            let seen = std::iter::once(&proposal["event_title"])
                .chain(rows(&proposal["event_titles"]).iter())
                .filter_map(Value::as_str)
                .any(|title| title_seen(title, &keys));
            if !seen {
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

#[cfg(test)]
mod tests {
    use super::*;
    fn line(text: &str, x: f64, y: f64) -> Value {
        json!({"text":text,"words":[{"x":x,"y":y,"width":20. * text.len() as f64 / 2.,"height":20.}]})
    }
    fn answer(title: &str) -> Value {
        json!({"key":"word-0008","panel":"event","identity":"bftb_turkey.4:bftb_turkey.4.a","event_id":"bftb_turkey.4",
            "option_id":"bftb_turkey.4.a","labels":["Mobilitare immediatamente le forze armate!"],
            "event_title":title,"event_titles":[title],"watch":false,"live_visible":false,"ocr_only":true})
    }
    #[test]
    fn an_event_answer_is_placed_by_its_title_and_exact_label_even_with_placeholders() {
        let ocr = json!({"width":1920,"height":1080,"lines":[
            line("La Turchia prende il controllo del Bosforo!", 700., 200.),
            line("Mobilitare immediatamente le forze armate!", 720., 700.),
            line("Questo circo farsesco conta poco per noi.", 720., 760.)]});
        let found = rectangles(&ocr, &json!([answer("[TUR.GetNameDefCap] prende il controllo del Bosforo!")]));
        assert_eq!(found.as_array().unwrap().len(), 1);
        assert_eq!(found[0]["basis"], "exact_unique_ocr");
        assert!(found[0]["rect"][1].as_f64().unwrap() > 690.);
        // Another popup with the same answer text: its title does not match.
        let other = json!({"width":1920,"height":1080,"lines":[line("Un altro evento", 700., 200.), line("Mobilitare immediatamente le forze armate!", 720., 700.)]});
        assert_eq!(rectangles(&other, &json!([answer("[TUR.GetNameDefCap] prende il controllo del Bosforo!")])), json!([]));
        // A title made only of placeholders never matches.
        assert_eq!(rectangles(&ocr, &json!([answer("[FROM.GetName] [ROOT.GetName]")])), json!([]));
    }
    #[test]
    fn titles_match_in_order_across_two_lines() {
        let keys = vec![text_key("La Turchia prende il"), text_key("controllo del Bosforo!")];
        assert!(title_seen("[TUR.GetNameDefCap] prende il controllo del Bosforo!", &keys));
        assert!(!title_seen("[TUR.GetNameDefCap] controllo del Bosforo! prende il", &keys));
        assert!(title_seen("Il Patto Molotov-Ribbentrop", &[text_key("Il Patto Molotov-Ribbentrop")]));
    }
}
