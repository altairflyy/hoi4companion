//! Application copy only. Authored content and installed entity names remain intact.
use regex::{Captures, Regex};
use serde_json::{Value, json};
use std::sync::OnceLock;
pub fn catalog() -> &'static Value {
    static CATALOG: OnceLock<Value> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../../../src/hoi4companion/translations.json"))
            .expect("packaged UI catalog")
    })
}
struct Pattern {
    regex: Regex,
    groups: Vec<String>,
    target: String,
    prefix: String,
}
fn patterns(italian: bool) -> &'static Vec<Pattern> {
    static IT: OnceLock<Vec<Pattern>> = OnceLock::new();
    static EN: OnceLock<Vec<Pattern>> = OnceLock::new();
    (if italian { &IT } else { &EN }).get_or_init(|| {
        let tokens = Regex::new(r"\{\d+\}").unwrap();
        let mut result = vec![];
        for (original, target) in catalog()[if italian { "messages" } else { "legacy" }]
            .as_object()
            .unwrap()
        {
            let mut source = String::from("(?s)^");
            let mut groups = vec![];
            let mut previous = 0;
            for token in tokens.find_iter(original) {
                source.push_str(&regex::escape(&original[previous..token.start()]));
                source.push_str("(.*?)");
                groups.push(token.as_str().to_owned());
                previous = token.end();
            }
            if groups.is_empty() {
                continue;
            }
            source.push_str(&regex::escape(&original[previous..]));
            source.push('$');
            result.push(Pattern {
                regex: Regex::new(&source).expect("escaped UI template"),
                groups,
                target: target.as_str().unwrap_or("").into(),
                prefix: original.split('{').next().unwrap_or("").into(),
            });
        }
        result
    })
}
pub fn italian(locale: &str) -> bool {
    ["italian", "it"].contains(&locale)
}
pub fn translate(message: &str, locale: &str) -> String {
    let italian = italian(locale);
    if let Some(text) = catalog()[if italian { "messages" } else { "legacy" }][message].as_str() {
        return text.into();
    }
    static TOKENS: OnceLock<Regex> = OnceLock::new();
    let tokens = TOKENS.get_or_init(|| Regex::new(r"\{\d+\}").unwrap());
    for pattern in patterns(italian) {
        if !pattern.prefix.is_empty() && !message.starts_with(&pattern.prefix) {
            continue;
        }
        if let Some(captures) = pattern.regex.captures(message) {
            return tokens
                .replace_all(&pattern.target, |token: &Captures<'_>| {
                    pattern
                        .groups
                        .iter()
                        .rposition(|g| g == &token[0])
                        .and_then(|i| captures.get(i + 1))
                        .map(|v| v.as_str())
                        .unwrap_or(&token[0])
                        .to_owned()
                })
                .into_owned();
        }
    }
    message.into()
}
pub fn payload(value: &Value, locale: &str) -> Value {
    fn visit(value: &Value, field: &str, locale: &str) -> Value {
        if [
            "guide",
            "tracks",
            "sources",
            "source",
            "evidence",
            "source_inventory",
            "instructions",
            "assumptions",
            "params",
            "note",
        ]
        .contains(&field)
        {
            return value.clone();
        }
        match value {
            Value::Object(map)
                if map.contains_key("tracks") && map.contains_key("schema_version") =>
            {
                value.clone()
            }
            Value::Object(map) => Value::Object(
                map.iter()
                    .map(|(k, v)| (k.clone(), visit(v, k, locale)))
                    .collect(),
            ),
            Value::Array(values) => json!(
                values
                    .iter()
                    .map(|v| visit(v, field, locale))
                    .collect::<Vec<_>>()
            ),
            Value::String(s)
                if [
                    "error",
                    "message",
                    "reason",
                    "hint",
                    "warning",
                    "warnings",
                    "status_message",
                    "help",
                ]
                .contains(&field) =>
            {
                json!(translate(s, locale))
            }
            _ => value.clone(),
        }
    }
    visit(value, "", locale)
}
pub fn html(source: &str, locale: &str) -> Result<String, String> {
    use lol_html::{HtmlRewriter, Settings, doc_text, element, end_tag, html_content::ContentType};
    use std::{cell::Cell, rc::Rc};
    let mut output = vec![];
    let raw = Rc::new(Cell::new(0usize));
    let mut buffered = String::new();
    {
        let on_start = raw.clone();
        let on_text = raw;
        let mut rewriter = HtmlRewriter::new(
            Settings::new()
                .append_element_content_handler(element!("*", |element| {
                    let tag = element.tag_name();
                    for key in ["lang", "title", "alt", "placeholder", "aria-label"] {
                        if let Some(value) = element.get_attribute(key) {
                            let replacement = if tag == "html" && key == "lang" {
                                if italian(locale) { "it" } else { "en" }.into()
                            } else if key != "lang" {
                                translate(&value, locale)
                            } else {
                                value.clone()
                            };
                            if replacement != value {
                                element.set_attribute(key, &replacement)?;
                            }
                        }
                    }
                    if ["script", "style"].contains(&tag.as_str()) {
                        on_start.set(on_start.get() + 1);
                        let on_end = on_start.clone();
                        element.on_end_tag(end_tag!(move |_| {
                            on_end.set(on_end.get().saturating_sub(1));
                            Ok(())
                        }))?;
                    }
                    Ok(())
                }))
                .append_document_content_handler(doc_text!(|chunk| {
                    if on_text.get() > 0 {
                        return Ok(());
                    }
                    buffered.push_str(chunk.as_str());
                    if buffered.len() > 1_000_000 {
                        return Err("UI text node exceeds limit".into());
                    }
                    if chunk.last_in_text_node() {
                        let decoded = html_escape::decode_html_entities(&buffered);
                        let clean = decoded.trim();
                        let text = translate(clean, locale);
                        let replacement = if text == clean {
                            decoded.into_owned()
                        } else {
                            let before = decoded.len() - decoded.trim_start().len();
                            let after = decoded.trim_end().len();
                            format!("{}{}{}", &decoded[..before], text, &decoded[after..])
                        };
                        chunk.replace(&replacement, ContentType::Text);
                        buffered.clear();
                    } else {
                        chunk.remove();
                    }
                    Ok(())
                })),
            |bytes: &[u8]| output.extend_from_slice(bytes),
        );
        rewriter
            .write(source.as_bytes())
            .map_err(|e| e.to_string())?;
        rewriter.end().map_err(|e| e.to_string())?;
    }
    String::from_utf8(output).map_err(|e| e.to_string())
}
