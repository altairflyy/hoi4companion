//! Versioned Unicode compatibility data, independent of a Python installation.
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct Profile {
    schema_version: u32,
    unicode_version: String,
    casefold: HashMap<char, String>,
    normal: HashMap<char, String>,
    nfkd: HashMap<char, String>,
    lower: HashMap<char, String>,
    alnum: Vec<[u32; 2]>,
    alpha: Vec<[u32; 2]>,
    decimal: Vec<[u32; 2]>,
    combining: Vec<[u32; 2]>,
}
fn profile() -> &'static Profile {
    static DATA: OnceLock<Profile> = OnceLock::new();
    DATA.get_or_init(|| {
        let value: Profile = serde_json::from_str(include_str!("../data/text-profile-v1.json"))
            .expect("checked-in Unicode profile");
        assert_eq!(value.schema_version, 1);
        value
    })
}
fn inside(c: char, ranges: &[[u32; 2]]) -> bool {
    let n = c as u32;
    let i = ranges.partition_point(|range| range[1] < n);
    ranges.get(i).is_some_and(|r| r[0] <= n)
}
pub fn unicode_version() -> &'static str {
    &profile().unicode_version
}
pub fn alnum(c: char) -> bool {
    inside(c, &profile().alnum)
}
pub fn alpha(c: char) -> bool {
    inside(c, &profile().alpha)
}
pub fn decimal(c: char) -> bool {
    inside(c, &profile().decimal)
}
pub fn space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}
pub fn fold(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            profile()
                .casefold
                .get(&c)
                .cloned()
                .unwrap_or_else(|| c.to_string())
        })
        .collect()
}
pub fn nfkd(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            profile()
                .nfkd
                .get(&c)
                .cloned()
                .unwrap_or_else(|| c.to_string())
        })
        .collect()
}
/// DLC identity has a separate exact contract: compatibility decomposition,
/// combining-mark removal and case folding, preserving punctuation.
pub fn dlc_name(value: &str) -> String {
    let decomposed = value
        .chars()
        .map(|c| {
            profile()
                .nfkd
                .get(&c)
                .cloned()
                .unwrap_or_else(|| c.to_string())
        })
        .collect::<String>();
    let stripped = decomposed
        .chars()
        .filter(|c| !inside(*c, &profile().combining))
        .collect::<String>();
    fold(&stripped)
        .split(space)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
/// Exact legacy recipe-label normalization is deliberately separate from the
/// alias resolver: no Roman-level substitutions, transliteration or synonyms.
pub fn production_name(value: &str) -> String {
    let decomposed = value
        .chars()
        .map(|c| {
            profile()
                .nfkd
                .get(&c)
                .cloned()
                .unwrap_or_else(|| c.to_string())
        })
        .collect::<String>();
    let context = decomposed.to_lowercase().chars().collect::<Vec<_>>();
    let mut offset = 0;
    let mut output = String::new();
    for c in decomposed.chars() {
        let lowered = if c == 'Σ' {
            context.get(offset).copied().unwrap_or('σ').to_string()
        } else {
            profile()
                .lower
                .get(&c)
                .cloned()
                .unwrap_or_else(|| c.to_string())
        };
        output.extend(lowered.chars().filter(|c| alnum(*c) || *c == '+'));
        offset += c.to_lowercase().count();
    }
    output
}
pub fn words(value: &str, underscore: bool) -> Vec<String> {
    value
        .split(|c| !(alnum(c) || underscore && c == '_'))
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}
fn translated(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            'ı' => "i".into(),
            'İ' => "I".into(),
            'ł' => "l".into(),
            'Ł' => "L".into(),
            'ø' => "o".into(),
            'Ø' => "O".into(),
            'æ' => "ae".into(),
            'Æ' => "AE".into(),
            'œ' => "oe".into(),
            'Œ' => "OE".into(),
            'ß' => "ss".into(),
            '&' => " and ".into(),
            _ => c.to_string(),
        })
        .collect()
}
fn phrases(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    let mut output = String::new();
    let mut i = 0;
    while i < chars.len() {
        if i == 0 || !(alnum(chars[i - 1]) || chars[i - 1] == '_') {
            for (left, right, replacement, hyphen) in [
                ("machine", "gun", "mg", false),
                ("anti", "aircraft", "anti-air", true),
            ] {
                let lhs: Vec<_> = left.chars().collect();
                if !chars[i..].starts_with(&lhs) {
                    continue;
                }
                let mut j = i + lhs.len();
                while j < chars.len() && (space(chars[j]) || hyphen && chars[j] == '-') {
                    j += 1;
                }
                let rhs: Vec<_> = right.chars().collect();
                if !chars[j..].starts_with(&rhs) {
                    continue;
                }
                j += rhs.len();
                if !hyphen && chars.get(j) == Some(&'s') {
                    j += 1;
                }
                if j == chars.len() || !(alnum(chars[j]) || chars[j] == '_') {
                    output.push_str(replacement);
                    i = j;
                    break;
                }
            }
            if i >= chars.len() {
                break;
            }
            // A replacement can end just before punctuation. Processing that
            // character here preserves the original boundary semantics.
        }
        output.push(chars[i]);
        i += 1;
    }
    output
}
pub fn normal(value: &str) -> String {
    let mut source = phrases(&fold(&translated(value)));
    let end = source.trim_end_matches(space).len();
    if let Some((i, _)) = source[..end].char_indices().rev().find(|(_, c)| space(*c)) {
        let suffix = &source[i..end].trim_start_matches(space);
        let roman = ["i", "ii", "iii", "iv", "v", "vi", "vii", "viii", "ix", "x"];
        if let Some(number) = roman.iter().position(|r| r == suffix) {
            source.replace_range(i.., &format!(" {}", number + 1));
        }
    }
    source
        .chars()
        .filter_map(|c| {
            if let Some(value) = profile().normal.get(&c) {
                Some(value.clone())
            } else if alnum(c) {
                Some(c.to_string())
            } else {
                None
            }
        })
        .collect()
}
pub fn spelling(value: &str) -> Vec<String> {
    words(&value.replace('&', " and "), false)
        .iter()
        .map(|w| normal(w))
        .collect()
}
pub fn strip_to(value: &str) -> Option<String> {
    let mut chars = value.chars();
    if fold(&chars.by_ref().take(2).collect::<String>()) != "to" {
        return None;
    }
    let rest: String = chars.collect();
    if !rest.chars().next().is_some_and(space) {
        return None;
    }
    Some(rest.trim_start_matches(space).into())
}
pub fn optional_focus(value: &str) -> Option<String> {
    let parts: Vec<_> = value.split('/').map(|s| s.trim_matches(space)).collect();
    if parts.len() != 2 {
        return None;
    }
    let empty = |s: &str| matches!(fold(s).as_str(), "nothing" | "none" | "no focus");
    if empty(parts[1]) && !parts[0].is_empty() {
        Some(parts[0].into())
    } else if empty(parts[0]) && !parts[1].is_empty() {
        Some(parts[1].into())
    } else {
        None
    }
}
pub fn expand(value: &str, families: &HashMap<String, String>) -> String {
    let mut value = value.to_string();
    let mut end = value.len();
    for (i, c) in value.char_indices().rev() {
        if decimal(c) {
            end = i;
        } else {
            break;
        }
    }
    if end < value.len()
        && value[..end]
            .chars()
            .last()
            .is_some_and(|c| c.is_ascii_alphabetic())
    {
        value.insert(end, ' ');
    }
    let trimmed = value.trim_matches(space);
    let last_space = trimmed
        .char_indices()
        .rev()
        .find(|(_, c)| space(*c))
        .map(|(i, _)| i);
    let (base, suffix) = if let Some(i) = last_space {
        let token = trimmed[i..].trim_matches(space);
        if !token.is_empty()
            && (token.chars().all(decimal)
                || matches!(fold(token).as_str(), "i" | "ii" | "iii" | "iv" | "v" | "vi"))
        {
            let base = trimmed[..i].trim_end_matches(space);
            (base, &trimmed[base.len()..])
        } else {
            (trimmed, "")
        }
    } else {
        (trimmed, "")
    };
    families
        .get(&fold(base))
        .map(|family| format!("{family}{suffix}"))
        .unwrap_or_else(|| value.clone())
}

pub fn typo_cost(query: &[String], other: &[String], max_edits: usize) -> Option<usize> {
    if query.len() != other.len() {
        return None;
    }
    let different: Vec<_> = query.iter().zip(other).filter(|(a, b)| a != b).collect();
    if different.is_empty() || different.len() > 2 {
        return None;
    }
    let protected = [
        "east",
        "eastern",
        "eastward",
        "west",
        "western",
        "westward",
        "north",
        "northern",
        "northward",
        "south",
        "southern",
        "southward",
        "basic",
        "improved",
        "advanced",
        "modern",
        "medium",
        "heavy",
        "light",
        "communism",
        "communist",
        "fascism",
        "fascist",
        "democratic",
        "neutrality",
    ];
    let mut budget = 0;
    for (a, b) in different {
        let x: Vec<_> = a.chars().collect();
        let y: Vec<_> = b.chars().collect();
        let minimum = x.len().min(y.len());
        let plural = minimum >= 3 && (a == &(b.clone() + "s") || b == &(a.clone() + "s"));
        if protected.contains(&a.as_str())
            || protected.contains(&b.as_str())
            || minimum < 4 && !plural
            || x.len().abs_diff(y.len()) > 2
            || !x.iter().copied().all(alpha)
            || !y.iter().copied().all(alpha)
        {
            return None;
        }
        let cost = distance(&x, &y);
        if cost > 2 || cost > 1 && minimum < if max_edits > 2 { 4 } else { 5 } {
            return None;
        }
        budget += cost;
    }
    if budget > max_edits
        || budget > 2
            && !query
                .iter()
                .zip(other)
                .any(|(a, b)| a == b && a.chars().count() >= 5)
    {
        None
    } else {
        Some(budget)
    }
}
fn distance(a: &[char], b: &[char]) -> usize {
    let mut rows = vec![(0..=b.len()).collect::<Vec<_>>()];
    for (i, x) in a.iter().enumerate() {
        let mut row = vec![i + 1];
        for (j, y) in b.iter().enumerate() {
            let mut value = (row[j] + 1)
                .min(rows[i][j + 1] + 1)
                .min(rows[i][j] + usize::from(x != y));
            if i > 0 && j > 0 && *x == b[j - 1] && a[i - 1] == *y {
                value = value.min(rows[i - 1][j - 1] + 1);
            }
            row.push(value);
        }
        rows.push(row);
    }
    rows[a.len()][b.len()]
}

/// SequenceMatcher ratio with default autojunk; similarities are review-only.
pub fn similarity(a: &str, b: &str) -> f64 {
    let a: Vec<_> = a.chars().collect();
    let b: Vec<_> = b.chars().collect();
    if a.is_empty() && b.is_empty() {
        return 1.;
    }
    let mut positions: HashMap<char, Vec<usize>> = HashMap::new();
    for (j, c) in b.iter().enumerate() {
        positions.entry(*c).or_default().push(j);
    }
    if b.len() >= 200 {
        positions.retain(|_, v| v.len() <= b.len() / 100 + 1);
    }
    let mut pending = vec![(0, a.len(), 0, b.len())];
    let mut matching = 0;
    while let Some((alo, ahi, blo, bhi)) = pending.pop() {
        let (mut best_i, mut best_j, mut best_size) = (alo, blo, 0);
        let mut prior = HashMap::<usize, usize>::new();
        for (i, c) in a.iter().enumerate().take(ahi).skip(alo) {
            let mut current = HashMap::new();
            for j in positions
                .get(c)
                .into_iter()
                .flatten()
                .copied()
                .filter(|j| *j >= blo && *j < bhi)
            {
                let length = if j > 0 {
                    prior.get(&(j - 1)).copied().unwrap_or(0)
                } else {
                    0
                } + 1;
                current.insert(j, length);
                if length > best_size {
                    best_i = i + 1 - length;
                    best_j = j + 1 - length;
                    best_size = length;
                }
            }
            prior = current;
        }
        while best_i > alo && best_j > blo && a[best_i - 1] == b[best_j - 1] {
            best_i -= 1;
            best_j -= 1;
            best_size += 1;
        }
        while best_i + best_size < ahi
            && best_j + best_size < bhi
            && a[best_i + best_size] == b[best_j + best_size]
        {
            best_size += 1;
        }
        if best_size > 0 {
            matching += best_size;
            if alo < best_i && blo < best_j {
                pending.push((alo, best_i, blo, best_j));
            }
            if best_i + best_size < ahi && best_j + best_size < bhi {
                pending.push((best_i + best_size, ahi, best_j + best_size, bhi));
            }
        }
    }
    2. * matching as f64 / (a.len() + b.len()) as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_levels_and_protected_semantics() {
        assert_eq!(normal("The Kırıkkale Tank"), "thekirikkaletank");
        assert_eq!(normal("Heavy Machine Guns II"), "heavymg2");
        assert_eq!(normal("Anti Aircraft I"), "antiair1");
        assert_eq!(normal("Łódź & Æther"), "lodzandaether");
        assert!(typo_cost(&spelling("Eastern Claims"), &spelling("Western Claims"), 3).is_none());
        assert_eq!(
            typo_cost(
                &spelling("Panzer Troop Schools"),
                &spelling("Panzer Troops School"),
                3
            ),
            Some(2)
        );
    }
    #[test]
    fn matching_ratio_and_empty_input() {
        assert_eq!(similarity("abcd", "bcde"), 0.75);
        assert_eq!(similarity("", ""), 1.);
        assert_eq!(similarity("tide", "diet"), 0.25);
    }
}
