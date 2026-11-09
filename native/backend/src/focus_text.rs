//! Bounded static localization and declared tooltips; never executes scripts.
use crate::script_document::{blocks, entries, scalar};
use serde_json::{Value, json};
use std::collections::HashSet;
const TRUNCATED: &str = "\n[Localizzazione troncata: limite di lunghezza]";
fn bound(value: String) -> String {
    if value.chars().count() <= 16384 {
        return value;
    }
    value
        .chars()
        .take(16384 - TRUNCATED.chars().count())
        .collect::<String>()
        + TRUNCATED
}
fn expand(
    value: &str,
    labels: &Value,
    stack: &mut HashSet<String>,
    depth: usize,
    left: &mut usize,
) -> String {
    let value = bound(value.into());
    if depth >= 12 {
        return value;
    }
    let bytes = value.as_bytes();
    let mut result = String::new();
    let mut i = 0;
    let mut copied = 0;
    while i < bytes.len() {
        if bytes[i] != b'$' || i > 0 && bytes[i - 1] == b'\\' {
            i += 1;
            continue;
        }
        let begin = i;
        i += 1;
        let start = i;
        while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || b"_.:@-".contains(&bytes[i]))
        {
            i += 1;
        }
        if start == i {
            continue;
        }
        let key = &value[start..i];
        let mut format = None;
        if bytes.get(i) == Some(&b'|') {
            i += 1;
            let start = i;
            while i < bytes.len() && bytes[i] != b'$' && bytes[i] != b'\n' {
                i += 1;
            }
            format = Some(&value[start..i]);
        }
        if bytes.get(i) != Some(&b'$') {
            i = begin + 1;
            continue;
        }
        i += 1;
        let valid_format = format.is_none_or(|s| s.len() == 1 && "YRGBWgLbOHCMr".contains(s));
        if *left == 0
            || stack.contains(key)
            || [
                "RIGHT",
                "LEFT",
                "VALUE",
                "COUNT",
                "DAYS",
                "NAME",
                "CHARACTER",
            ]
            .contains(&key)
            || !valid_format
        {
            continue;
        }
        let Some(label) = labels[key].as_str() else {
            continue;
        };
        *left -= 1;
        stack.insert(key.into());
        let expanded = expand(label, labels, stack, depth + 1, left);
        stack.remove(key);
        result.push_str(&value[copied..begin]);
        if let Some(color) = format {
            result.push('§');
            result.push_str(color);
            result.push_str(&expanded);
            result.push_str("§!");
        } else {
            result.push_str(&expanded);
        }
        copied = i;
    }
    result.push_str(&value[copied..]);
    bound(result)
}
pub fn localise(value: &str, labels: &Value) -> String {
    expand(value, labels, &mut HashSet::new(), 0, &mut 512)
}

// Decimal rendering uses textual digits, avoiding a float round-trip in text
// such as 0.025 -> +2.5%. Bounds match the existing presentation contract.
fn decimal(value: &str, percent: bool) -> Option<(String, bool)> {
    let cleaned = value.trim().replace('_', "");
    let mut input = cleaned.as_str();
    let negative = input.starts_with('-');
    if input.starts_with(['-', '+']) {
        input = &input[1..];
    }
    let (mantissa, exponent) = if let Some(i) = input.find(['e', 'E']) {
        (&input[..i], input[i + 1..].parse::<i64>().ok()?)
    } else {
        (input, 0)
    };
    if exponent.unsigned_abs() > 100000 {
        return None;
    }
    let mut digits = String::new();
    let mut fractional = 0i64;
    let mut dot = false;
    for c in mantissa.chars() {
        if c == '.' && !dot {
            dot = true;
            continue;
        }
        if !c.is_ascii_digit() {
            return None;
        }
        digits.push(c);
        if dot {
            fractional += 1;
        }
    }
    if digits.is_empty() {
        return None;
    }
    let mut digits = digits.trim_start_matches('0').to_owned();
    let nonzero = !digits.is_empty();
    let mut exponent = exponent - fractional;
    if nonzero && !(-10..=15).contains(&(digits.len() as i64 + exponent - 1)) {
        return None;
    }
    if percent {
        exponent += 2;
    }
    // Decimal multiplication/normalization use the standard 28-digit context.
    if digits.len() > 28 {
        let rest = &digits.as_bytes()[28..];
        let last = digits.as_bytes()[27];
        let up = rest[0] > b'5'
            || rest[0] == b'5' && (rest[1..].iter().any(|b| *b != b'0') || (last - b'0') % 2 == 1);
        exponent += (digits.len() - 28) as i64;
        digits.truncate(28);
        if up {
            let mut value = digits.into_bytes();
            let mut carry = true;
            for digit in value.iter_mut().rev() {
                if *digit == b'9' {
                    *digit = b'0';
                } else {
                    *digit += 1;
                    carry = false;
                    break;
                }
            }
            digits = String::from_utf8(value).ok()?;
            if carry {
                digits.insert(0, '1');
            }
        }
    }
    let adjusted = digits.len() as i64 + exponent - 1;
    if nonzero
        && (!(-8..=15).contains(&adjusted)
            || adjusted == 15
                && (digits.as_bytes()[0] != b'1'
                    || digits.as_bytes()[1..].iter().any(|b| *b != b'0')))
    {
        return None;
    }
    while digits.ends_with('0') {
        digits.pop();
        exponent += 1;
    }
    let rendered = if digits.is_empty() {
        "0".into()
    } else if exponent >= 0 {
        digits + &"0".repeat(exponent as usize)
    } else {
        let point = digits.len() as i64 + exponent;
        if point <= 0 {
            format!("0.{}{digits}", "0".repeat((-point) as usize))
        } else {
            digits.insert(point as usize, '.');
            digits
        }
    };
    Some((
        format!("{}{rendered}", if negative { "-" } else { "" }),
        nonzero && !negative,
    ))
}
fn numeric(key: &str, value: &str) -> Option<String> {
    let (title, percent) = match key {
        "add_political_power" => ("Potere politico", false),
        "add_stability" => ("Stabilità", true),
        "add_war_support" => ("Sostegno bellico", true),
        "add_manpower" => ("Risorse umane", false),
        "army_experience" => ("Esperienza dell'esercito", false),
        "navy_experience" => ("Esperienza della marina", false),
        "air_experience" => ("Esperienza dell'aviazione", false),
        "add_research_slot" => ("Slot di ricerca", false),
        "add_command_power" => ("Potere di comando", false),
        _ => return None,
    };
    let (rendered, positive) = decimal(value, percent)?;
    Some(format!(
        "{title}: {}{rendered}{}",
        if positive { "+" } else { "" },
        if percent { "%" } else { "" }
    ))
}
pub fn tooltips(node: &Value, labels: &Value) -> Value {
    let mut sections = vec![];
    for (section, heading) in [
        ("available", "Requisiti"),
        ("bypass", "Bypass"),
        ("completion_reward", "Effetti"),
    ] {
        let (mut lines, mut unrepresented, mut conditional) = (vec![], false, false);
        let tooltip = |key: &str, lines: &mut Vec<String>| -> bool {
            if ["generic_skip_one_line_tt", "generic_skip_two_lines_tt"].contains(&key) {
                return true;
            }
            let Some(label) = labels[key].as_str() else {
                return false;
            };
            let label = localise(label, labels).trim().to_owned();
            if !label.is_empty() {
                lines.push(label);
            }
            true
        };
        for block in blocks(node, section) {
            for (key, value) in entries(block) {
                if let Some(value) = value.as_str() {
                    if ["custom_effect_tooltip", "tooltip"].contains(&key) {
                        unrepresented |= !tooltip(value, &mut lines);
                    } else if section == "completion_reward"
                        && let Some(effect) = numeric(key, value)
                    {
                        lines.push(effect);
                    } else if section == "completion_reward"
                        && ["show_ideas_tooltip", "add_ideas", "remove_ideas"].contains(&key)
                        && labels.get(value).is_some()
                    {
                        let title = match key {
                            "add_ideas" => "Aggiunge idea",
                            "remove_ideas" => "Rimuove idea",
                            _ => "Idea o consigliere",
                        };
                        lines.push(format!(
                            "{title}: {}",
                            localise(labels[value].as_str().unwrap_or(""), labels)
                        ));
                    } else {
                        unrepresented = true;
                    }
                } else if key == "custom_trigger_tooltip" && section != "completion_reward" {
                    unrepresented |= !tooltip(scalar(value, "tooltip"), &mut lines);
                } else {
                    unrepresented = true;
                    conditional |= [
                        "if",
                        "else",
                        "else_if",
                        "random",
                        "random_list",
                        "while",
                        "every_country",
                        "every_state",
                        "random_country",
                        "random_state",
                        "hidden_effect",
                        "hidden_trigger",
                    ]
                    .contains(&key.to_lowercase().as_str());
                }
            }
        }
        if section == "available"
            && ["allow_branch", "visible"]
                .iter()
                .any(|key| blocks(node, key).next().is_some())
        {
            unrepresented = true;
        }
        if conditional {
            lines.push(
                if section == "completion_reward" {
                    "Altri effetti condizionali da verificare nel gioco"
                } else {
                    "Condizioni aggiuntive da verificare nel gioco"
                }
                .into(),
            );
        } else if unrepresented {
            lines.push(
                if section == "completion_reward" {
                    "Altri effetti da verificare nel gioco"
                } else {
                    "Condizioni aggiuntive da verificare nel gioco"
                }
                .into(),
            );
        }
        if !lines.is_empty() {
            sections.push(json!({"heading":heading,"text":lines.join("\n")}));
        }
    }
    json!(sections)
}
