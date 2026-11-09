//! Explicitly partial demo/document analysis. Installed games use full CWTools;
//! unsupported grammar is counted, never guessed or evaluated as campaign state.
use crate::script_document::{all, blocks, entries, scalar};
use serde_json::{Map, Value, json};
use std::collections::{BTreeSet, HashMap, HashSet};
fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn a(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn block(v: &Value) -> bool {
    v.get("entries").is_some()
}
fn re(p: &str) -> regex::Regex {
    crate::import_catalog::regex(p)
}
fn dynamic(s: &str) -> bool {
    re(r"[@$\[\]?:]").is_match(s)
}
fn inc(m: &mut Map<String, Value>, key: &str) {
    let n = m.get(key).and_then(Value::as_u64).unwrap_or(0);
    m.insert(key.into(), json!(n + 1));
}
fn number(m: &Map<String, Value>, key: &str) -> u64 {
    m.get(key).and_then(Value::as_u64).unwrap_or(0)
}
const NAMESPACES: [&str; 18] = [
    "focus",
    "shared_focus",
    "joint_focus",
    "technology",
    "idea",
    "character",
    "decision",
    "event",
    "country_event",
    "news_event",
    "state_event",
    "unit_leader_event",
    "state",
    "special_project",
    "building",
    "country_tags",
    "scripted_trigger",
    "scripted_effect",
];
type Failure = Option<(String, String, String)>;
fn failure(code: &str, message: impl Into<String>, category: &str) -> Failure {
    Some((code.into(), message.into(), category.into()))
}
#[derive(Clone)]
struct Analyzer<'a> {
    book: &'a Value,
    index: HashMap<String, HashSet<String>>,
    labels: &'a Value,
    reference: &'a Value,
    incomplete: HashSet<String>,
    limit: usize,
    diagnostics: Vec<Value>,
    counts: Map<String, Value>,
    coverage: Map<String, Value>,
    unhandled: Map<String, Value>,
    loc: HashSet<String>,
    file: String,
}
impl Analyzer<'_> {
    fn emit(
        &mut self,
        code: &str,
        path: &str,
        value: &str,
        message: &str,
        category: &str,
        rule: Option<&str>,
    ) {
        for key in ["total", "warning", category] {
            inc(&mut self.counts, key)
        }
        if self.diagnostics.len() < self.limit {
            self.diagnostics.push(json!({"code":code,"file":self.file,"path":path,"value":value.chars().take(300).collect::<String>(),"message":message,"category":category,"severity":"warning","source":"static_analysis","rule":rule,"runtime_evaluated":false}));
        }
    }
    fn unknown(&mut self, key: &str) {
        inc(&mut self.coverage, "unsupported");
        inc(&mut self.unhandled, key)
    }
    fn has(&self, kind: &str, value: &str) -> bool {
        self.index.get(kind).is_some_and(|v| v.contains(value))
    }
    fn reference_check(&mut self, kind: &str, value: &str, _path: &str) -> Failure {
        if dynamic(value) {
            self.unknown("dynamic_reference");
            return None;
        }
        let kind = kind
            .strip_prefix("event.")
            .filter(|k| {
                [
                    "country_event",
                    "news_event",
                    "state_event",
                    "unit_leader_event",
                ]
                .contains(k)
            })
            .unwrap_or(kind);
        let kinds = match kind {
            "focus" => vec!["focus", "shared_focus", "joint_focus"],
            "event" => vec![
                "event",
                "country_event",
                "news_event",
                "state_event",
                "unit_leader_event",
            ],
            _ => vec![kind],
        };
        if NAMESPACES.contains(&kind) && kinds.iter().any(|k| self.has(k, value)) {
            return None;
        }
        if !NAMESPACES.contains(&kind) || kinds.iter().any(|k| self.incomplete.contains(*k)) {
            self.unknown(&format!("namespace:{kind}"));
            return None;
        }
        if kinds.iter().any(|k| self.has(k, value)) {
            None
        } else {
            failure(
                "reference_missing",
                format!("ID {kind} «{value}» non trovato nel catalogo installato"),
                "references",
            )
        }
    }
    fn localise(&mut self, key: &str, path: &str, inline: bool) {
        if key.is_empty()
            || dynamic(key)
            || key.chars().any(hoi4companion_catalog_core::unicode_space)
        {
            return;
        }
        self.loc.insert(key.into());
        if self.labels.get(key).is_none() && self.reference.get(key).is_none() {
            if inline {
                self.unknown("localisation_or_inline")
            } else {
                self.emit(
                    "localisation_missing",
                    path,
                    key,
                    "Chiave non trovata nei testi installati",
                    "localisation",
                    None,
                )
            }
        } else if self.labels.get(key).is_none() {
            self.emit(
                "localisation_fallback",
                path,
                key,
                "Testo disponibile solo nella lingua di riferimento",
                "localisation",
                None,
            )
        }
    }
    fn scalar_check(&mut self, value: &Value, schema: &str, path: &str) -> Failure {
        if !(["bool", "scalar", "string"].contains(&schema)
            || [
                "<",
                "scope[",
                "event_target[",
                "enum[",
                "int",
                "float",
                "variable_field",
                "value_field",
                "localisation",
            ]
            .iter()
            .any(|p| schema.starts_with(p)))
        {
            self.unknown(&format!("datatype:{schema}"));
            return None;
        }
        let Some(value) = value.as_str() else {
            return failure(
                "parameter_type",
                "Era atteso un valore scalare",
                "parameters",
            );
        };
        if dynamic(value) {
            self.unknown("dynamic_value");
            return None;
        }
        if schema.contains('|') {
            return self.alternatives(
                &json!(value),
                &schema.split('|').map(|v| json!(v)).collect::<Vec<_>>(),
                path,
            );
        }
        if schema.starts_with('<') && schema.ends_with('>') {
            return self.reference_check(&schema[1..schema.len() - 1], value, path);
        }
        if schema.starts_with("scope[") || schema.starts_with("event_target[") {
            let target = &schema[schema.find('[').unwrap() + 1..schema.len() - 1];
            if value.contains('.') {
                self.unknown("scope_expression");
                return None;
            }
            if [
                "ROOT",
                "FROM",
                "THIS",
                "PREV",
                "OWNER",
                "CONTROLLER",
                "CAPITAL",
            ]
            .contains(&value.to_uppercase().as_str())
                || self.book["links"].get(value.to_lowercase()).is_some()
            {
                self.unknown("scope_value");
                return None;
            }
            return self.reference_check(
                if target == "country" {
                    "country_tags"
                } else {
                    target
                },
                value,
                path,
            );
        }
        if let Some(name) = schema
            .strip_prefix("enum[")
            .and_then(|v| v.strip_suffix(']'))
        {
            if name == "country_tags" {
                return self.reference_check(name, value, path);
            }
            if let Some(values) = self.book["enums"].get(name) {
                return if a(values).contains(&json!(value)) {
                    None
                } else {
                    failure(
                        "parameter_type",
                        format!("Valore non incluso in {name}"),
                        "parameters",
                    )
                };
            }
            self.unknown(&format!("enum:{name}"));
            return None;
        }
        if schema == "bool" {
            return if ["yes", "no"].contains(&value) {
                None
            } else {
                failure("parameter_type", "Era atteso yes oppure no", "parameters")
            };
        }
        if ["int", "float", "variable_field", "value_field"]
            .iter()
            .any(|p| schema.starts_with(p))
        {
            let parsed = value.parse::<f64>();
            if (schema.contains("variable") || schema.contains("value_field")) && parsed.is_err() {
                self.unknown("variable");
                return None;
            }
            let Ok(n) = parsed else {
                return failure(
                    "parameter_type",
                    format!("Numero non valido per {schema}"),
                    "parameters",
                );
            };
            if !n.is_finite() || schema.starts_with("int") && n.fract() != 0.0 {
                return failure(
                    "parameter_type",
                    format!("Numero non valido per {schema}"),
                    "parameters",
                );
            }
            if let Some(bounds) = re(r"\[([^\]]+)\.\.([^\]]+)\]").captures(schema) {
                let low = bounds[1].parse::<f64>();
                let high = bounds[2].parse::<f64>();
                if let (Ok(low), Ok(high)) = (low, high) {
                    if !(low..=high).contains(&n) {
                        return failure(
                            "parameter_range",
                            format!(
                                "Valore fuori dall’intervallo CWT {schema}; compatibilità con il gioco da verificare"
                            ),
                            "parameters",
                        );
                    }
                } else {
                    return failure(
                        "parameter_type",
                        format!("Numero non valido per {schema}"),
                        "parameters",
                    );
                }
            }
            return None;
        }
        if schema.starts_with("localisation") {
            self.localise(value, path, true);
            return None;
        }
        if ["scalar", "string"].contains(&schema) {
            return None;
        }
        self.unknown(&format!("datatype:{schema}"));
        None
    }
    fn alternatives(&mut self, value: &Value, schemas: &[Value], path: &str) -> Failure {
        if schemas.len() == 1 {
            return self.shape(value, &schemas[0], path);
        }
        let mut chosen = None;
        for schema in schemas {
            let mut candidate = self.clone();
            let fail = candidate.shape(value, schema, path);
            let rank = (
                fail.is_some(),
                block(value) != schema.is_array(),
                number(&candidate.coverage, "unsupported"),
            );
            if chosen.as_ref().is_none_or(|(old, _, _)| rank < *old) {
                chosen = Some((rank, fail, candidate));
            }
        }
        if let Some((_, fail, candidate)) = chosen {
            *self = candidate;
            fail
        } else {
            None
        }
    }
    fn shape(&mut self, value: &Value, schema: &Value, path: &str) -> Failure {
        if schema.is_null() {
            self.unknown("schema_missing");
            return None;
        }
        if let Some(schema) = schema.as_str() {
            return self.scalar_check(value, schema, path);
        }
        if !block(value) {
            return failure(
                "parameter_type",
                "Era atteso un blocco di parametri",
                "parameters",
            );
        }
        let mut fields: HashMap<&str, Vec<Value>> = HashMap::new();
        for row in a(schema) {
            fields.entry(s(&row[0])).or_default().push(row[1].clone());
        }
        for (raw, item) in entries(value) {
            let key = raw.split(' ').next().unwrap_or("");
            if let Some(schemas) = fields.get(key) {
                if let Some((code, message, category)) =
                    self.alternatives(item, schemas, &format!("{path}/{key}"))
                {
                    return Some((code, format!("{key}: {message}"), category));
                }
            } else if fields.keys().any(|k| {
                ["alias_name[", "<", "subtype["]
                    .iter()
                    .any(|p| k.starts_with(p))
                    || ["scalar", "scope_field"].contains(k)
            }) {
                self.unknown("open_schema")
            } else {
                self.unknown(&format!("parameter:{key}"))
            }
        }
        None
    }
    fn block(
        &mut self,
        node: &Value,
        kind: &str,
        context: (Option<&str>, Option<&str>, Option<&str>),
        path: &str,
        depth: usize,
    ) {
        let (scope, root, previous) = context;
        if depth > 60 {
            self.unknown("depth_limit");
            return;
        }
        for (position, (raw, value)) in entries(node).enumerate() {
            let key = raw.split(' ').next().unwrap_or("");
            let lower = key.to_lowercase();
            let upper = key.to_uppercase();
            let at = format!("{path}/{key}[{position}]");
            if [
                "and",
                "or",
                "not",
                "hidden_trigger",
                "hidden_effect",
                "else",
                "else_if",
                "if",
                "limit",
            ]
            .contains(&lower.as_str())
                && block(value)
            {
                self.block(
                    value,
                    if ["limit", "hidden_trigger"].contains(&lower.as_str()) {
                        "trigger"
                    } else {
                        kind
                    },
                    (scope, root, previous),
                    &at,
                    depth + 1,
                );
                continue;
            }
            if block(value)
                && (["ROOT", "THIS", "PREV", "FROM"].contains(&upper.as_str())
                    || self.book["links"].get(&lower).is_some()
                    || self.has("country_tags", key)
                    || self.has("state", key))
            {
                let link = self.book["links"][&lower].clone();
                let next = match upper.as_str() {
                    "ROOT" => root,
                    "THIS" => scope,
                    "PREV" => previous,
                    "FROM" => None,
                    _ => {
                        if self.has("country_tags", key) {
                            Some("country")
                        } else if self.has("state", key) {
                            Some("state")
                        } else {
                            let inputs = a(&link["inputs"]);
                            if let Some(scope) = scope
                                && !inputs.is_empty()
                                && !inputs.contains(&json!(scope))
                                && !inputs.contains(&json!("any"))
                            {
                                self.emit(
                                    "scope_mismatch",
                                    &at,
                                    key,
                                    &format!("Transizione non ammessa dal contesto {scope}"),
                                    "scopes",
                                    Some("links.cwt"),
                                )
                            }
                            link["output"].as_str()
                        }
                    }
                };
                self.block(value, kind, (next, root, scope), &at, depth + 1);
                continue;
            }
            let variants = a(&self.book["rules"][kind][&lower]).to_vec();
            if self.has(&format!("scripted_{kind}"), &lower) {
                self.unknown(&format!("scripted_call:{lower}"));
                continue;
            }
            if variants.is_empty() {
                self.unknown(&format!("{kind}:{key}"));
                continue;
            }
            inc(&mut self.coverage, "recognized");
            let scopes: BTreeSet<_> = variants
                .iter()
                .flat_map(|r| a(&r["scopes"]))
                .filter_map(Value::as_str)
                .collect();
            if scopes.is_empty() {
                self.unknown("scope_metadata_missing")
            }
            if let Some(scope) = scope {
                if !scopes.is_empty()
                    && !scopes.contains("any")
                    && !scopes.contains(scope)
                    && variants.iter().all(|v| !a(&v["scopes"]).is_empty())
                {
                    self.emit(
                        "scope_mismatch",
                        &at,
                        key,
                        &format!(
                            "Contesto {scope}; atteso {}",
                            scopes.into_iter().collect::<Vec<_>>().join(", ")
                        ),
                        "scopes",
                        variants[0]["file"].as_str(),
                    );
                }
            } else {
                inc(&mut self.coverage, "unknown_scope");
            }
            if let Some((code, message, category)) = self.alternatives(
                value,
                &variants
                    .iter()
                    .map(|v| v["schema"].clone())
                    .collect::<Vec<_>>(),
                &at,
            ) {
                self.emit(
                    &code,
                    &at,
                    if value.is_string() { s(value) } else { key },
                    &message,
                    &category,
                    variants[0]["file"].as_str(),
                );
            }
            if block(value) {
                let aliases = variants
                    .iter()
                    .flat_map(|v| a(&v["schema"]))
                    .filter(|r| s(&r[0]).starts_with("alias_name["))
                    .collect::<Vec<_>>();
                if !aliases.is_empty() {
                    self.block(
                        value,
                        if aliases.iter().any(|r| s(&r[0]).contains("trigger")) {
                            "trigger"
                        } else {
                            kind
                        },
                        (None, root, scope),
                        &at,
                        depth + 1,
                    )
                } else if variants.iter().any(|r| r["schema"] == "scope_field") {
                    self.block(value, kind, (None, root, scope), &at, depth + 1)
                }
            }
        }
    }
    fn entity(&mut self, kind: &str, id: &str, node: &Value, scope: Option<&str>) {
        let path = format!("{kind}:{id}");
        if [
            "focus",
            "shared_focus",
            "joint_focus",
            "technology",
            "idea",
            "decision",
            "special_project",
        ]
        .contains(&kind)
        {
            let text = scalar(node, "text");
            let name =
                if ["focus", "shared_focus", "joint_focus"].contains(&kind) && !text.is_empty() {
                    text
                } else {
                    id
                };
            self.localise(name, &format!("{path}/name"), false);
            let desc = format!("{name}_desc");
            if self.labels.get(&desc).is_some() || self.reference.get(&desc).is_some() {
                self.localise(&desc, &format!("{path}/description"), false);
            }
        }
        for (key, value) in entries(node) {
            let at = format!("{path}/{key}");
            if ["prerequisite", "mutually_exclusive"].contains(&key) && block(value) {
                for target in all(value, "focus").filter_map(Value::as_str) {
                    if let Some((code, message, category)) =
                        self.reference_check("focus", target, &at)
                    {
                        self.emit(&code, &at, target, &message, &category, None)
                    }
                }
            } else if ["title", "desc", "name"].contains(&key) && value.is_string() {
                self.localise(s(value), &at, true)
            } else if block(value)
                && [
                    "available",
                    "allowed",
                    "visible",
                    "bypass",
                    "allow_branch",
                    "trigger",
                    "cancel",
                    "allowed_to_remove",
                    "allowed_civil_war",
                ]
                .contains(&key)
            {
                self.block(value, "trigger", (scope, scope, None), &at, 0)
            } else if block(value)
                && [
                    "completion_reward",
                    "select_effect",
                    "immediate",
                    "complete_effect",
                    "remove_effect",
                    "cancel_effect",
                    "on_research_complete",
                ]
                .contains(&key)
            {
                self.block(value, "effect", (scope, scope, None), &at, 0)
            } else if key == "option" && block(value) {
                for (field, item) in entries(value) {
                    let at = format!("{path}/option/{field}");
                    if field == "name" && item.is_string() {
                        self.localise(s(item), &format!("{path}/option/name"), true)
                    } else if ["trigger", "available", "visible"].contains(&field) && block(item) {
                        self.block(item, "trigger", (scope, scope, None), &at, 0)
                    } else if !["name", "ai_chance", "highlight", "default", "original_tag"]
                        .contains(&field)
                    {
                        self.block(
                            &json!({"entries":[[field,item]]}),
                            "effect",
                            (scope, scope, None),
                            &format!("{path}/option"),
                            0,
                        )
                    }
                }
            }
        }
        if let Some(kind) = kind.strip_prefix("scripted_") {
            self.block(node, kind, (None, None, None), &path, 0)
        }
    }
    fn local_graph(&mut self) {
        for (language, labels) in [("display", self.labels), ("reference", self.reference)] {
            let mut done = HashSet::new();
            let mut active = HashSet::new();
            let starts: BTreeSet<_> = self.loc.iter().cloned().collect();
            for start in starts {
                let mut stack = vec![(start, false)];
                while let Some((key, leaving)) = stack.pop() {
                    if leaving {
                        active.remove(&key);
                        done.insert(key);
                        continue;
                    }
                    if done.contains(&key) || labels.get(&key).is_none() {
                        continue;
                    }
                    if active.contains(&key) {
                        self.emit(
                            "localisation_cycle",
                            language,
                            &key,
                            "Riferimento ciclico nei testi",
                            "localisation",
                            None,
                        );
                        continue;
                    }
                    active.insert(key.clone());
                    stack.push((key.clone(), true));
                    let text = s(&labels[&key]);
                    let regex = re(r"\$([A-Za-z0-9_.:@-]+)(?:\|([^$\n]*))?\$");
                    for m in regex
                        .captures_iter(text)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                    {
                        if m.get(0).is_some_and(|r| {
                            r.start() > 0 && text.as_bytes()[r.start() - 1] == b'\\'
                        }) {
                            continue;
                        }
                        let target = &m[1];
                        if [
                            "RIGHT",
                            "LEFT",
                            "VALUE",
                            "COUNT",
                            "DAYS",
                            "NAME",
                            "CHARACTER",
                        ]
                        .contains(&target)
                        {
                            continue;
                        }
                        if labels.get(target).is_none() {
                            self.emit(
                                "localisation_reference_missing",
                                &format!("{language}/{key}"),
                                target,
                                "Riferimento testuale non risolto",
                                "localisation",
                                None,
                            )
                        } else {
                            stack.push((target.into(), false));
                        }
                    }
                }
            }
        }
    }
}
fn entities(path: &str, root: &Value) -> Vec<(String, String, Value, Option<String>)> {
    let mut out = vec![];
    let mut push = |kind: &str, id: &str, body: &Value, scope: Option<&str>| {
        out.push((
            kind.into(),
            id.into(),
            body.clone(),
            scope.map(str::to_owned),
        ))
    };
    if path.starts_with("common/national_focus/") {
        for (kind, value) in entries(root) {
            if kind == "focus_tree" && block(value) {
                for (key, body) in entries(value) {
                    if ["focus", "shared_focus", "joint_focus"].contains(&key) && block(body) {
                        push(key, scalar(body, "id"), body, Some("country"))
                    }
                }
            } else if ["shared_focus", "joint_focus"].contains(&kind) && block(value) {
                push(kind, scalar(value, "id"), value, Some("country"))
            }
        }
    } else if path.starts_with("events/") {
        for (kind, body) in entries(root) {
            if kind.ends_with("_event") && block(body) {
                push(
                    kind,
                    scalar(body, "id"),
                    body,
                    Some(match kind {
                        "state_event" => "state",
                        "unit_leader_event" => "unit_leader",
                        _ => "country",
                    }),
                )
            }
        }
    } else if path.starts_with("history/states/") {
        for body in blocks(root, "state") {
            push("state", scalar(body, "id"), body, Some("state"))
        }
    } else {
        for (prefix, group, kind, scope) in [
            (
                "common/technologies/",
                "technologies",
                "technology",
                Some("country"),
            ),
            (
                "common/characters/",
                "characters",
                "character",
                Some("character"),
            ),
            ("common/buildings/", "buildings", "building", None),
            (
                "common/special_projects/",
                "special_projects",
                "special_project",
                Some("special_project"),
            ),
        ] {
            if path.starts_with(prefix) {
                for container in blocks(root, group) {
                    for (key, body) in entries(container) {
                        if block(body) {
                            push(kind, key, body, scope)
                        }
                    }
                }
                return out;
            }
        }
        if path.starts_with("common/ideas/") || path.starts_with("common/decisions/") {
            let kind = if path.starts_with("common/ideas/") {
                "idea"
            } else {
                "decision"
            };
            let mut groups = entries(root)
                .filter(|(_, v)| block(v))
                .map(|(_, v)| v)
                .collect::<Vec<_>>();
            if kind == "idea" {
                groups = groups
                    .into_iter()
                    .flat_map(entries)
                    .filter(|(_, v)| block(v))
                    .map(|(_, v)| v)
                    .collect();
            }
            for group in groups {
                for (key, body) in entries(group) {
                    if block(body)
                        && !["allowed", "visible", "available", "default", "ai_will_do"]
                            .contains(&key)
                    {
                        push(kind, key, body, Some("country"))
                    }
                }
            }
        } else if path.starts_with("common/country_tags/")
            || path.starts_with("common/country_tag_aliases/")
        {
            for (key, _) in entries(root) {
                if re("^[A-Z0-9]{3}$").is_match(key) {
                    push("country_tags", key, &json!({"entries":[]}), None)
                }
            }
        } else if path.starts_with("common/scripted_triggers/")
            || path.starts_with("common/scripted_effects/")
        {
            let kind = if path.starts_with("common/scripted_triggers/") {
                "scripted_trigger"
            } else {
                "scripted_effect"
            };
            for (key, body) in entries(root) {
                if block(body) {
                    push(kind, key, body, None)
                }
            }
        }
    }
    out
}
pub fn analyze(
    documents: &Value,
    labels: &Value,
    reference: &Value,
    book: &Value,
    incomplete: &Value,
    limit: usize,
) -> Result<Value, String> {
    if !(1..=10000).contains(&limit) {
        return Err("Limite diagnosi non valido".into());
    }
    let documents = documents.as_array().ok_or("Missing analysis documents")?;
    let mut indexed = vec![];
    let mut index = HashMap::<String, HashSet<String>>::new();
    for document in documents {
        let file = document[0].as_str().ok_or("Invalid analysis document")?;
        for (kind, id, node, scope) in entities(&file.replace('\\', "/"), &document[1]) {
            if !id.is_empty() {
                index.entry(kind.clone()).or_default().insert(id.clone());
                indexed.push((file.to_owned(), kind.clone(), id, node.clone(), scope));
            }
            if kind == "character" {
                for advisor in blocks(&node, "advisor") {
                    let token = scalar(advisor, "idea_token");
                    if !token.is_empty() {
                        index.entry("idea".into()).or_default().insert(token.into());
                    }
                }
            }
        }
    }
    let incomplete: HashSet<_> = a(incomplete)
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    let mut engine = Analyzer {
        book,
        index,
        labels,
        reference,
        incomplete: incomplete.clone(),
        limit,
        diagnostics: vec![],
        counts: Map::new(),
        coverage: Map::new(),
        unhandled: Map::new(),
        loc: HashSet::new(),
        file: String::new(),
    };
    for (file, kind, id, node, scope) in &indexed {
        engine.file = file.clone();
        engine.entity(kind, id, node, scope.as_deref())
    }
    engine.file = "localisation".into();
    engine.local_graph();
    let mut unhandled = engine.unhandled.iter().collect::<Vec<_>>();
    unhandled.sort_by_key(|(_, v)| std::cmp::Reverse(v.as_u64().unwrap_or(0)));
    let unhandled: Map<_, _> = unhandled
        .into_iter()
        .take(100)
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    Ok(
        json!({"schema_version":1,"source":"static_analysis","status":"partial","runtime_evaluated":false,
 "rules":{"repository":book["provenance"]["repository"],"revision":book["provenance"]["revision"],"adapter":"cwtools-subset-v2","game_version_compatibility":"not_certified","trigger_names":book["rules"]["trigger"].as_object().map(Map::len).unwrap_or(0),"effect_names":book["rules"]["effect"].as_object().map(Map::len).unwrap_or(0),"load_errors":book["load_errors"]},
 "coverage":{"recognized":number(&engine.coverage,"recognized"),"unsupported":number(&engine.coverage,"unsupported"),"unknown_scope":number(&engine.coverage,"unknown_scope"),"unhandled":unhandled,"documents":documents.len(),"entities":indexed.len(),"incomplete_namespaces":incomplete.into_iter().collect::<BTreeSet<_>>()},
 "summary":engine.counts,"diagnostics":engine.diagnostics,"truncated":number(&engine.counts,"total")>engine.diagnostics.len()as u64}),
    )
}
