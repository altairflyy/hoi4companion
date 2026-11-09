//! Deterministic guide decisions. No I/O, game access, clocks or persistence.
mod ffi;
mod research;
mod revisions;
#[cfg(test)]
mod tests;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

pub type Error = String;
pub fn engine_info()->Value{json!({"engine":"rust-guide-core","schema_version":1,"abi_version":1,"input":"prepared-v1","game_access":false})}
type Object = Map<String, Value>;
fn text(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn yes(v: &Value) -> bool {
    v.as_bool() == Some(true)
}
fn no(v: &Value) -> bool {
    v.as_bool() == Some(false)
}
fn truth(v: &Value) -> bool {
    !v.is_null()
        && v != false
        && v.as_f64() != Some(0.)
        && v != ""
        && v.as_array().is_none_or(|x| !x.is_empty())
        && v.as_object().is_none_or(|x| !x.is_empty())
}
fn list(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn strings(v: &Value) -> Vec<String> {
    list(v)
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}
fn unknown(reason: &str) -> Value {
    json!({"status":"unknown","value":null,"source":"","reason":reason})
}
fn calculated(v: bool, source: &str) -> Value {
    json!({"status":"calculated","value":v,"source":source})
}
fn set(result: &mut Value, status: &str, reason: &str) {
    result["status"] = json!(status);
    result["reason"] = json!(reason);
}
fn group(values: &[Option<bool>], all: bool) -> Option<bool> {
    if values.contains(&Some(!all)) {
        Some(!all)
    } else if values.contains(&None) {
        None
    } else {
        Some(all)
    }
}
fn scalar_equal(a: &Value, b: &Value) -> Option<bool> {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => Some(x.as_f64() == y.as_f64()),
        (Value::String(x), Value::String(y)) => Some(x == y),
        (Value::Bool(x), Value::Bool(y)) => Some(x == y),
        _ => None,
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Condition {
    All {
        args: Vec<Condition>,
    },
    Any {
        args: Vec<Condition>,
    },
    Not {
        arg: Box<Condition>,
    },
    Check {
        key: String,
    },
    Fact {
        key: String,
        test: Test,
        value: Value,
    },
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Test {
    Eq,
    Contains,
    Gte,
}
impl Condition {
    fn validate(&self, depth: usize) -> Result<(), Error> {
        if depth > 16 {
            return Err("Condition depth exceeded".into());
        }
        match self {
            Self::All { args } | Self::Any { args } => {
                if args.is_empty() || args.len() > 64 {
                    return Err("Invalid condition operands".into());
                }
                for a in args {
                    a.validate(depth + 1)?;
                }
            }
            Self::Not { arg } => arg.validate(depth + 1)?,
            Self::Check { key } | Self::Fact { key, .. } => {
                if key.is_empty() || key.len() > 2048 {
                    return Err("Invalid fact key".into());
                }
            }
        }
        if let Self::Fact { value, .. } = self
            && (!(value.is_boolean() || value.is_string() || value.is_number())
                || value
                    .as_f64()
                    .is_some_and(|n| !n.is_finite() || n.abs() > 9_007_199_254_740_991.))
        {
            return Err("Invalid condition scalar".into());
        }
        Ok(())
    }
    pub fn evaluate(&self, state: &State, checks: &BTreeMap<String, bool>) -> Option<bool> {
        match self {
            Self::All { args } => group(
                &args
                    .iter()
                    .map(|c| c.evaluate(state, checks))
                    .collect::<Vec<_>>(),
                true,
            ),
            Self::Any { args } => group(
                &args
                    .iter()
                    .map(|c| c.evaluate(state, checks))
                    .collect::<Vec<_>>(),
                false,
            ),
            Self::Not { arg } => arg.evaluate(state, checks).map(|v| !v),
            Self::Check { key } => checks.get(key).copied(),
            Self::Fact { key, test, value } => {
                let actual = state.value(key)?;
                match test {
                    Test::Eq => scalar_equal(actual, value),
                    Test::Contains => actual
                        .as_array()
                        .map(|items| items.iter().any(|x| scalar_equal(x, value) == Some(true))),
                    Test::Gte => Some(actual.as_f64()? >= value.as_f64()?),
                }
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct State {
    pub usable: bool,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub facts: BTreeMap<String, Value>,
}
impl State {
    fn value(&self, key: &str) -> Option<&Value> {
        let fact = self.facts.get(key)?;
        if !self.usable
            || !matches!(text(&fact["status"]), "observed" | "calculated")
            || text(&fact["source"]).is_empty()
            || fact["value"].is_null()
        {
            return None;
        }
        Some(&fact["value"])
    }
    fn membership(&self, key: &str, target: &str) -> Value {
        match self.value(key).and_then(Value::as_array) {
            Some(values) if values.iter().all(Value::is_string) => calculated(
                values.iter().any(|x| x == target),
                &format!("membership:{key}"),
            ),
            _ => unknown("not_observed"),
        }
    }
    fn contains(&self, key: &str, target: &str) -> bool {
        self.value(key)
            .and_then(Value::as_array)
            .is_some_and(|a| a.iter().any(|x| x == target))
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    SelectFocus,
    Research,
    Manual,
    SpecialProject,
    Free,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Action {
    pub kind: Kind,
    #[serde(default)]
    pub entity: Value,
    #[serde(default)]
    pub params: Value,
}
#[derive(Clone, Debug)]
pub struct Costs(Vec<(String, Value)>);
impl<'de> Deserialize<'de> for Costs {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Costs;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("an ordered resource cost object")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Costs, A::Error> {
                let mut values = Vec::new();
                while let Some(pair) = map.next_entry()? {
                    values.push(pair)
                }
                Ok(Costs(values))
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}
#[derive(Clone, Debug, Deserialize)]
pub struct Step {
    pub key: String,
    pub action: Action,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub tracking: String,
    #[serde(default)]
    pub phase: String,
    #[serde(default)]
    pub priority: Value,
    #[serde(default)]
    pub priority_when: Option<Condition>,
    #[serde(default)]
    pub completion: Option<Condition>,
    #[serde(default)]
    pub started_when: Option<Condition>,
    #[serde(default)]
    pub skip_when: Option<Condition>,
    #[serde(default)]
    pub when: Option<Condition>,
    #[serde(default)]
    pub requirements: Option<Condition>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub depends_on_started: Vec<String>,
    #[serde(default)]
    pub catalog_match: Option<bool>,
    #[serde(default)]
    pub political_power_cost: Option<Value>,
    #[serde(default)]
    pub resource_costs: Option<Costs>,
    #[serde(default)]
    pub relative_research: Value,
    #[serde(default)]
    pub relative_research_unresolved: Value,
    #[serde(default)]
    pub research_choice: Value,
    #[serde(default)]
    pub focus_criteria: Value,
}
impl Step {
    fn reminder(&self) -> bool {
        self.tracking == "reminder"
    }
    fn optional(&self) -> bool {
        truth(&self.action.params["focus_optional"])
    }
    fn native(&self) -> bool {
        matches!(self.action.kind, Kind::SelectFocus | Kind::Research)
    }
    fn domain(&self) -> Option<&'static str> {
        let p = &self.action.params;
        if self.reminder() {
            None
        } else if self.action.kind == Kind::SpecialProject {
            Some("project")
        } else if truth(&p["commander_id"])
            || matches!(text(&p["role"]), "corps_commander" | "navy_leader")
        {
            Some("appointment")
        } else if self.political_power_cost.is_some()
            || [
                "law_id",
                "law_name",
                "idea_id",
                "idea_name",
                "advisor_id",
                "person_id",
                "person_name",
                "trait_name",
                "role",
                "decision_id",
                "decision_name",
            ]
            .iter()
            .any(|k| truth(&p[k]))
        {
            Some("political")
        } else {
            None
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
pub struct Phase {
    pub id: String,
    #[serde(default)]
    pub when: Option<Condition>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Guide {
    pub id: String,
    #[serde(default)]
    pub applies_to: Value,
    #[serde(default)]
    pub dependency_policy: String,
    pub tracks: BTreeMap<String, Vec<Step>>,
    #[serde(default)]
    pub phases: Vec<Phase>,
}
#[derive(Clone)]
pub struct CompiledGuide {
    guide: Guide,
    rows: Vec<(String, Step)>,
    by_key: HashMap<String, usize>,
    topology: Vec<usize>,
    implicit: Vec<Vec<usize>>,
    phases: HashMap<String, Phase>,
}
impl CompiledGuide {
    pub fn compile(guide: Guide) -> Result<Self, Error> {
        let rows: Vec<_> = guide
            .tracks
            .iter()
            .flat_map(|(track, steps)| steps.iter().map(move |s| (track.clone(), s.clone())))
            .collect();
        if rows.len() > 131072 {
            return Err("Guide step limit exceeded".into());
        }
        let mut by_key = HashMap::new();
        for (i, (_, s)) in rows.iter().enumerate() {
            if s.key.is_empty() || by_key.insert(s.key.clone(), i).is_some() {
                return Err("Duplicate or empty step key".into());
            }
            for c in [
                &s.priority_when,
                &s.completion,
                &s.started_when,
                &s.skip_when,
                &s.when,
                &s.requirements,
            ]
            .into_iter()
            .flatten()
            {
                c.validate(0)?;
            }
        }
        let mut degrees = vec![0; rows.len()];
        let mut children = vec![Vec::new(); rows.len()];
        for (i, (_, s)) in rows.iter().enumerate() {
            for key in s.depends_on.iter().chain(&s.depends_on_started) {
                let prev = *by_key.get(key).ok_or("Unresolved dependency")?;
                degrees[i] += 1;
                children[prev].push(i);
            }
        }
        let mut queue: VecDeque<_> = degrees
            .iter()
            .enumerate()
            .filter_map(|(i, n)| (*n == 0).then_some(i))
            .collect();
        let mut topology = Vec::new();
        while let Some(i) = queue.pop_front() {
            topology.push(i);
            for j in &children[i] {
                degrees[*j] -= 1;
                if degrees[*j] == 0 {
                    queue.push_back(*j)
                }
            }
        }
        if topology.len() != rows.len() {
            return Err("Cyclic guide dependencies".into());
        }
        let mut implicit = vec![Vec::new(); rows.len()];
        // Normalized legacy steps carry explicitness separately in preparation.
        if !matches!(guide.dependency_policy.as_str(), "" | "legacy" | "explicit") {
            return Err("Invalid dependency policy".into());
        }
        let phases = guide
            .phases
            .iter()
            .map(|p| (p.id.clone(), p.clone()))
            .collect();
        for p in &guide.phases {
            if let Some(c) = &p.when {
                c.validate(0)?;
            }
        }
        for (track, steps) in &guide.tracks {
            let mut barriers = Vec::new();
            for step in steps {
                let i = by_key[&step.key];
                implicit[i] = barriers.clone();
                if step.action.kind == Kind::Manual && !step.reminder() {
                    barriers.push(i);
                }
            }
            let _ = track;
        }
        Ok(Self {
            guide,
            rows,
            by_key,
            topology,
            implicit,
            phases,
        })
    }
}

/// Transitional adapters supply qualified facts and domain projections. They do
/// not supply statuses, dependency decisions, priorities or proposed actions.
#[derive(Deserialize)]
pub struct Input {
    pub schema_version: u32,
    pub state: State,
    pub original_state: State,
    pub progress_status: String,
    pub run_id: Option<String>,
    #[serde(default)]
    pub checks: BTreeMap<String, bool>,
    #[serde(default)]
    pub started: BTreeMap<String, bool>,
    #[serde(default)]
    pub explicit_dependencies: HashSet<String>,
    #[serde(default)]
    pub construction: Object,
    #[serde(default)]
    pub production: Object,
    #[serde(default)]
    pub research_bindings: BTreeMap<String, Value>,
    #[serde(default)]
    pub research_options: Value,
    #[serde(default)]
    pub foreign_tokens: BTreeMap<String, String>,
    #[serde(default)]
    pub selectors: BTreeMap<String, Vec<Value>>,
}

fn condition_value(node: &Option<Condition>, input: &Input) -> Option<bool> {
    node.as_ref()
        .and_then(|c| c.evaluate(&input.state, &input.checks))
}
fn condition_fact(v: Option<bool>, source: &str) -> Value {
    v.map(|b| calculated(b, source))
        .unwrap_or_else(|| unknown("not_observed"))
}

fn readiness(step: &Step, input: &Input) -> Option<Value> {
    let domain = step.domain()?;
    let mut missing = Vec::<&str>::new();
    let mut blocked = Vec::<&str>::new();
    let mut resources = Vec::new();
    let mut affordability = Vec::new();
    let mut out = json!({"domain":domain,"requirements_declared":step.requirements.is_some()});
    if domain == "political" && !truth(&step.action.params["cost_currency"])
        || step.political_power_cost.is_some()
    {
        let cost = step.political_power_cost.as_ref();
        let amount = input
            .state
            .value("country.politics.political_power")
            .filter(|v| v.is_number());
        let affordable = cost
            .zip(amount)
            .and_then(|(c, a)| Some(a.as_f64()? >= c.as_f64()?));
        out["political_power"] = json!({"cost":cost,"available":amount,"affordable":affordable,"cost_basis":if cost.is_some(){"guide_declared"}else{"unknown"}});
        if cost.is_none() {
            missing.push("political_cost_unknown")
        };
        if amount.is_none() {
            missing.push("political_power_unknown")
        };
        if affordable == Some(false) {
            blocked.push("insufficient_political_power")
        };
        affordability.push(affordable);
    }
    if domain == "project" && step.resource_costs.is_none() {
        missing.push("project_cost_unknown");
        affordability.push(None)
    }
    for (key, cost) in step
        .resource_costs
        .as_ref()
        .map(|c| c.0.as_slice())
        .unwrap_or(&[])
    {
        let amount = input.state.value(key).filter(|v| v.is_number());
        let affordable = amount.and_then(|a| Some(a.as_f64()? >= cost.as_f64()?));
        resources.push(json!({"key":key,"cost":cost,"available":amount,"affordable":affordable}));
        affordability.push(affordable);
        if affordable == Some(false) && !blocked.contains(&"insufficient_resource") {
            blocked.push("insufficient_resource")
        } else if affordable.is_none() && !missing.contains(&"resource_balance_unknown") {
            missing.push("resource_balance_unknown")
        }
    }
    out["resources_available"] = json!(group(&affordability, true));
    match condition_value(&step.requirements, input) {
        Some(false) => blocked.push("action_requirements_false"),
        None => missing.push("action_requirements_unknown"),
        _ => {}
    }
    if domain == "appointment" {
        missing.push("commander_assignment_unobserved")
    };
    if domain == "project" {
        missing.push("facility_compatibility_unknown")
    };
    out["value"] = json!(if !blocked.is_empty() {
        Some(false)
    } else if !missing.is_empty() {
        None
    } else {
        Some(true)
    });
    out["unknown"] = json!(missing);
    out["blocked"] = json!(blocked);
    out["resources"] = json!(resources);
    Some(out)
}

fn focus_readiness(rules: &Value, state: &State) -> Value {
    fn evaluate(
        node: &Value,
        state: &State,
        checks: &mut Vec<Value>,
        depth: usize,
    ) -> Option<bool> {
        if depth > 32 {
            return None;
        }
        match text(&node["op"]) {
            "const" => node["value"].as_bool(),
            "all" | "any" => group(
                &list(&node["args"])
                    .iter()
                    .map(|a| evaluate(a, state, checks, depth + 1))
                    .collect::<Vec<_>>(),
                node["op"] == "all",
            ),
            "not" => evaluate(&node["arg"], state, checks, depth + 1).map(|x| !x),
            "if" => {
                let a = evaluate(&node["condition"], state, checks, depth + 1);
                let b = evaluate(&node["then"], state, checks, depth + 1);
                if a == Some(false) || b == Some(true) {
                    Some(true)
                } else if a == Some(true) {
                    b
                } else {
                    None
                }
            }
            "unknown" => {
                checks.push(
                    json!({"trigger":node["trigger"],"value":null,"reason":"unsupported_trigger"}),
                );
                None
            }
            "compare" => {
                let actual = state.value(text(&node["fact"]));
                let required = &node["value"];
                let test = text(&node["test"]);
                let value = actual.and_then(|a| match test {
                    "contains" => a
                        .as_array()
                        .filter(|v| v.iter().all(Value::is_string))
                        .map(|v| v.contains(required)),
                    "=" => scalar_equal(a, required),
                    "!=" => scalar_equal(a, required).map(|x| !x),
                    ">" | "<" | ">=" | "<=" => Some(match test {
                        ">" => a.as_f64()? > required.as_f64()?,
                        "<" => a.as_f64()? < required.as_f64()?,
                        ">=" => a.as_f64()? >= required.as_f64()?,
                        _ => a.as_f64()? <= required.as_f64()?,
                    }),
                    _ => None,
                });
                checks.push(json!({"trigger":node["trigger"],"fact":node["fact"],"test":test,"required":required,"actual":actual,"value":value}));
                value
            }
            _ => None,
        }
    }
    let mut checks = Vec::new();
    let available = evaluate(&rules["available"], state, &mut checks, 0);
    let branch = evaluate(&rules["branch"], state, &mut checks, 0);
    let done = state
        .value("focus.completed")
        .and_then(Value::as_array)
        .filter(|v| v.iter().all(Value::is_string));
    let prereq = done.map(|d| {
        list(&rules["prerequisites"])
            .iter()
            .all(|g| list(g).iter().any(|p| d.contains(p)))
    });
    let excluded = done.map(|d| list(&rules["mutex"]).iter().any(|p| d.contains(p)));
    let value = group(&[available, branch, prereq, excluded.map(|v| !v)], true);
    json!({"value":value,"available":available,"branch":branch,"prerequisites":prereq,"excluded":excluded,"checks":checks,"reason":if excluded==Some(true){"focus_excluded"}else if prereq==Some(false){"focus_prerequisites_false"}else if value==Some(false){"focus_requirements_false"}else if value.is_none(){"focus_requirements_unknown"}else{"focus_requirements_satisfied"}})
}

impl CompiledGuide {
    pub fn evaluate(&self, input: &Input) -> Result<Value, Error> {
        if self.rows.iter().any(|(_, s)| {
            truth(&s.relative_research)
                || truth(&s.relative_research_unresolved)
                || truth(&s.research_choice)
        }) {
            let mut projected = self.clone();
            projected.project_research(input);
            return projected.evaluate_projected(input);
        }
        self.evaluate_projected(input)
    }
    fn evaluate_projected(&self, input: &Input) -> Result<Value, Error> {
        let choices = self.research_choices(input);
        let research_progression = self.research_progression(&input.state);
        if input.schema_version != 1 {
            return Err("Unsupported prepared input version".into());
        }
        let country = input.original_state.value("country");
        let tree = input.original_state.value("focus.tree");
        let scope = &self.guide.applies_to;
        let mode = if !input.original_state.usable {
            "preview"
        } else if truth(&scope["country"])
            && country.is_some()
            && country != Some(&scope["country"])
        {
            "wrong_country"
        } else if truth(&scope["tree_id"]) && tree.is_some() && tree != Some(&scope["tree_id"]) {
            "wrong_tree"
        } else if truth(&scope["country"]) && country.is_none()
            || truth(&scope["tree_id"]) && tree.is_none()
        {
            "preview"
        } else {
            "live"
        };
        let mismatched = matches!(mode, "wrong_country" | "wrong_tree");
        let mut results = Vec::with_capacity(self.rows.len());
        for (track, step) in &self.rows {
            let key = &step.key;
            let native = step.native();
            let reminder = step.reminder();
            let ref_id = text(&step.action.entity["id"]);
            let basis = if reminder {
                "reminder"
            } else if step.completion.is_some() {
                "condition"
            } else if native {
                "native_membership"
            } else if step.action.kind == Kind::Free {
                "free"
            } else {
                "manual"
            };
            let mut completion = unknown(if reminder {
                "not_tracked"
            } else {
                "not_observed"
            });
            let mut selection = unknown("not_observed");
            let entity_scope = &step.action.entity["scope"];
            let entity_mismatch = step.action.kind == Kind::SelectFocus
                && truth(entity_scope)
                && tree.is_some()
                && tree != Some(entity_scope);
            let entity_unknown =
                step.action.kind == Kind::SelectFocus && truth(entity_scope) && tree.is_none();
            if !reminder && !mismatched && !entity_mismatch && !entity_unknown {
                if native {
                    let prefix = if step.action.kind == Kind::SelectFocus {
                        "focus"
                    } else {
                        "research"
                    };
                    let continuous = step.action.kind == Kind::SelectFocus
                        && step.action.params["focus_mode"] == "continuous";
                    selection = input.state.membership(
                        if continuous {
                            "focus.continuous"
                        } else if prefix == "focus" {
                            "focus.active"
                        } else {
                            "research.active"
                        },
                        ref_id,
                    );
                    completion = if continuous {
                        calculated(false, "continuous_focus_has_no_completion")
                    } else {
                        input
                            .state
                            .membership(&format!("{prefix}.completed"), ref_id)
                    };
                } else if matches!(step.action.kind, Kind::Manual | Kind::SpecialProject)
                    && input.checks.contains_key(key)
                {
                    completion = calculated(input.checks[key], &format!("check:{key}"));
                }
                if step.started_when.is_some() {
                    selection =
                        condition_fact(condition_value(&step.started_when, input), "declared_start")
                } else if matches!(step.action.kind, Kind::Manual | Kind::SpecialProject)
                    && input.started.contains_key(key)
                {
                    selection = calculated(input.started[key], &format!("manual_started:{key}"))
                }
                if step.completion.is_some() {
                    completion = condition_fact(
                        condition_value(&step.completion, input),
                        "declared_completion",
                    )
                }
            }
            let mut priority_known = true;
            let mut priority = if step.priority.is_null() {
                json!(0)
            } else {
                step.priority.clone()
            };
            if step.priority_when.is_some() {
                let v = condition_value(&step.priority_when, input);
                priority_known = v.is_some();
                if v != Some(true) {
                    priority = json!(0)
                }
            }
            let mut r = json!({"key":key,"track":track,"status":"unverifiable","reason":"","blocked_by":[],"completion":completion,"selection":selection,"completion_basis":basis,"priority":priority,"priority_known":priority_known,"availability":unknown("dynamic_availability_not_evaluated")});
            if step.action.kind == Kind::SelectFocus && truth(&step.focus_criteria) {
                let f = focus_readiness(&step.focus_criteria, &input.state);
                r["availability"] = f["value"]
                    .as_bool()
                    .map(|v| calculated(v, "game_focus_criteria"))
                    .unwrap_or_else(|| unknown(text(&f["reason"])));
                r["focus_readiness"] = f;
            }
            let choice = choices.get(key).filter(|v| truth(v));
            if choice.is_some() || truth(&step.relative_research) {
                r["research_target"] = json!(step.target)
            }
            if let Some(choice) = choice.filter(|_| step.target.starts_with("unbound-choice:")) {
                r["research_options"] = choice["options"].clone();
                r["research_choice_status"] = choice["status"].clone();
                if truth(&choice["options"]) {
                    if step.completion.is_none() {
                        r["completion"] = calculated(false, "unselected_research_choice")
                    };
                    r["selection"] = calculated(false, "unselected_research_choice");
                }
            }
            if let Some(a) = readiness(step, input) {
                r["availability"] = a["value"]
                    .as_bool()
                    .map(|v| calculated(v, "declared_action_requirements"))
                    .unwrap_or_else(|| unknown(&strings(&a["unknown"]).join(",")));
                r["action_readiness"] = a;
            }
            if reminder {
                r["tracking"] = json!("reminder")
            };
            if let Some(c) = input.construction.get(key) {
                r[if c.get("province_ids").is_some() {
                    "fortifications"
                } else {
                    "construction"
                }] = c.clone();
            }
            if let Some(p) = input.production.get(key) {
                r["production"] = p.clone();
            }
            if mismatched {
                set(&mut r, "not_applicable", "guide_scope_mismatch")
            } else if entity_mismatch {
                set(&mut r, "not_applicable", "entity_scope_mismatch")
            } else if entity_unknown {
                r["reason"] = json!("entity_scope_unknown")
            } else if choice.is_some()
                && ref_id.starts_with("unbound-choice:")
                && !truth(&choice.unwrap()["options"])
            {
                let c = choice.unwrap();
                set(
                    &mut r,
                    if c["status"] == "waiting" {
                        "waiting"
                    } else {
                        "unverifiable"
                    },
                    &format!("research_choice_{}", text(&c["status"])),
                )
            } else if (truth(&step.relative_research) || truth(&step.relative_research_unresolved))
                && ref_id.starts_with("unbound-up:")
            {
                let reason = research_progression
                    .get(key)
                    .map(String::as_str)
                    .unwrap_or("research_binding_unknown");
                set(
                    &mut r,
                    if reason == "research_line_complete" {
                        "not_applicable"
                    } else if reason == "research_previous_level_incomplete" {
                        "waiting"
                    } else {
                        "unverifiable"
                    },
                    reason,
                );
            } else if yes(&r["completion"]["value"]) {
                set(&mut r, "completed", "completion_true")
            } else if yes(&r["selection"]["value"]) {
                set(&mut r, "active", "selection_true")
            } else if mode != "live" && native {
                r["reason"] = json!("live_state_unavailable")
            } else {
                r["reason"] = json!("pending")
            }
            results.push(r);
        }
        for &i in &self.topology {
            let step = &self.rows[i].1;
            if mode == "live"
                && input
                    .foreign_tokens
                    .get(&step.key)
                    .is_some_and(|token| !input.state.contains("guide.campaign.milestones", token))
                && let Some(denied) = self.admission(step, input, &results)
            {
                results[i]
                    .as_object_mut()
                    .unwrap()
                    .extend(denied.as_object().unwrap().clone());
                results[i]["completion"] = unknown("interaction_not_admitted");
                results[i]["selection"] = unknown("interaction_not_admitted");
                continue;
            }
            let production = step.action.params["category"] == "production" && !step.reminder();
            if production {
                results[i]["production_admitted"] = json!(mode == "live")
            }
            if mode == "live"
                && production
                && let Some(denied) = self.admission(step, input, &results)
            {
                revisions::deny(&mut results[i], denied);
                continue;
            }
            if results[i]["reason"] != "pending" {
                continue;
            }
            if step.skip_when.is_some() {
                let value = condition_value(&step.skip_when, input);
                if value != Some(false) {
                    set(
                        &mut results[i],
                        if value == Some(true) {
                            "not_applicable"
                        } else {
                            "unverifiable"
                        },
                        if value == Some(true) {
                            "skip_condition_true"
                        } else {
                            "skip_condition_unknown"
                        },
                    );
                    continue;
                }
            }
            let phase = self.phases.get(&step.phase);
            let mut failed = false;
            for (node, reason) in [
                (phase.and_then(|p| p.when.as_ref()), "phase_condition"),
                (step.when.as_ref(), "condition"),
            ] {
                if let Some(node) = node {
                    let v = node.evaluate(&input.state, &input.checks);
                    if v != Some(true) {
                        set(
                            &mut results[i],
                            if v == Some(false) {
                                "waiting"
                            } else {
                                "unverifiable"
                            },
                            &format!(
                                "{reason}_{}",
                                if v == Some(false) { "false" } else { "unknown" }
                            ),
                        );
                        failed = true;
                        break;
                    }
                }
            }
            if failed {
                continue;
            }
            if step.catalog_match == Some(false) && !truth(&results[i]["research_options"]) {
                results[i]["reason"] = json!("catalog_unmatched");
                continue;
            }
            let mut blocked = Vec::new();
            let mut unknown_dep = false;
            for dep in &step.depends_on {
                let prev = &results[self.by_key[dep]];
                if prev["status"] != "completed" {
                    blocked.push(dep.clone());
                    unknown_dep |= prev["status"] == "unverifiable"
                }
            }
            for dep in &step.depends_on_started {
                let prev = &results[self.by_key[dep]];
                if !yes(&prev["selection"]["value"]) && !yes(&prev["completion"]["value"]) {
                    blocked.push(dep.clone());
                    unknown_dep |=
                        prev["selection"]["value"].is_null() && !yes(&prev["completion"]["value"])
                }
            }
            if self.guide.dependency_policy != "explicit"
                && !input.explicit_dependencies.contains(&step.key)
            {
                for &dep in &self.implicit[i] {
                    if !yes(&results[dep]["completion"]["value"]) {
                        blocked.push(self.rows[dep].1.key.clone());
                        unknown_dep |= results[dep]["completion"]["value"].is_null();
                    }
                }
            }
            if !blocked.is_empty() {
                set(
                    &mut results[i],
                    if unknown_dep {
                        "unverifiable"
                    } else {
                        "waiting"
                    },
                    if unknown_dep {
                        "dependency_unknown"
                    } else {
                        "dependency_not_completed"
                    },
                );
                results[i]["blocked_by"] = json!(blocked)
            } else if truth(&results[i]["action_readiness"]["blocked"]) {
                let reason = text(&results[i]["action_readiness"]["blocked"][0]).to_owned();
                set(&mut results[i], "waiting", &reason)
            } else if no(&results[i]["focus_readiness"]["value"]) {
                let reason = text(&results[i]["focus_readiness"]["reason"]).to_owned();
                set(&mut results[i], "waiting", &reason)
            } else if research_progression
                .get(&step.key)
                .is_some_and(|v| v == "research_previous_level_incomplete")
            {
                set(
                    &mut results[i],
                    "waiting",
                    "research_previous_level_incomplete",
                )
            } else if step.native() && results[i]["selection"]["value"].is_null() {
                results[i]["reason"] = json!("selection_unknown")
            } else if step.reminder() {
                set(&mut results[i], "ready", "reminder_available")
            } else if results[i]["completion"]["value"].is_null() && step.action.kind != Kind::Free
            {
                results[i]["reason"] = json!(if matches!(
                    step.action.kind,
                    Kind::Manual | Kind::SpecialProject
                ) {
                    "manual_confirmation_required"
                } else {
                    "completion_unknown"
                })
            } else {
                set(
                    &mut results[i],
                    "ready",
                    if step.action.kind == Kind::Free {
                        "free_choice"
                    } else {
                        "prerequisites_satisfied"
                    },
                )
            }
        }
        self.revise(input, &mut results)?;
        self.finish(input, mode, country, results)
    }

    fn admission(&self, step: &Step, input: &Input, results: &[Value]) -> Option<Value> {
        let mut values = Vec::new();
        if let Some(v) = step.action.params.get("production_section_when") {
            values.push(
                serde_json::from_value::<Condition>(v.clone())
                    .ok()
                    .and_then(|n| n.evaluate(&input.state, &input.checks)),
            );
        }
        for node in [
            self.phases.get(&step.phase).and_then(|p| p.when.as_ref()),
            step.when.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            values.push(node.evaluate(&input.state, &input.checks))
        }
        if step.skip_when.is_some() {
            values.push(condition_value(&step.skip_when, input).map(|v| !v))
        }
        if values.iter().any(|v| *v != Some(true)) {
            let unknown = !values.contains(&Some(false));
            return Some(
                json!({"status":if unknown{"unverifiable"}else{"waiting"},"reason":if unknown{"condition_unknown"}else{"condition_false"}}),
            );
        }
        let mut blocked = Vec::new();
        let mut milestone_values = Vec::new();
        for p in list(&step.action.params["production_milestones"]) {
            let current = serde_json::from_value::<Condition>(p["condition"].clone())
                .ok()
                .and_then(|n| n.evaluate(&input.state, &input.checks));
            if current == Some(true)
                || input
                    .state
                    .contains("guide.campaign.milestones", text(&p["token"]))
            {
                continue;
            }
            blocked.push(p["step"].clone());
            milestone_values.push(current);
        }
        if !blocked.is_empty() {
            let known = milestone_values.contains(&Some(false));
            return Some(
                json!({"status":if known{"waiting"}else{"unverifiable"},"reason":if known{"milestone_not_completed"}else{"milestone_unknown"},"blocked_by":blocked}),
            );
        }
        for dep in &step.depends_on {
            if results[self.by_key[dep]]["status"] != "completed" {
                blocked.push(json!(dep))
            }
        }
        for dep in &step.depends_on_started {
            let r = &results[self.by_key[dep]];
            if !yes(&r["selection"]["value"]) && !yes(&r["completion"]["value"]) {
                blocked.push(json!(dep))
            }
        }
        if blocked.is_empty() {
            None
        } else {
            let unknown = blocked
                .iter()
                .any(|d| results[self.by_key[text(d)]]["status"] == "unverifiable");
            Some(
                json!({"status":if unknown{"unverifiable"}else{"waiting"},"reason":if unknown{"dependency_unknown"}else{"dependency_not_completed"},"blocked_by":blocked}),
            )
        }
    }

    fn finish(
        &self,
        input: &Input,
        mode: &str,
        country: Option<&Value>,
        results: Vec<Value>,
    ) -> Result<Value, Error> {
        let mut order: Vec<_> = (0..self.rows.len()).collect();
        order.sort_by(|a, b| {
            results[*b]["priority"]
                .as_f64()
                .unwrap_or(0.)
                .total_cmp(&results[*a]["priority"].as_f64().unwrap_or(0.))
                .then(a.cmp(b))
        });
        let mut ranks = vec![0; order.len()];
        for (rank, i) in order.iter().enumerate() {
            ranks[*i] = rank;
        }
        let mut tracks = Object::new();
        let mut candidates = [Vec::new(), Vec::new()];
        for track in self.guide.tracks.keys() {
            let authored: Vec<_> = self
                .rows
                .iter()
                .enumerate()
                .filter_map(|(i, (t, _))| (t == track).then_some(i))
                .collect();
            let sorted: Vec<_> = order
                .iter()
                .copied()
                .filter(|i| self.rows[*i].0 == *track)
                .collect();
            let next = sorted.iter().find(|i| {
                !self.rows[**i].1.reminder()
                    && !self.rows[**i].1.optional()
                    && !matches!(
                        text(&results[**i]["status"]),
                        "completed" | "not_applicable" | "active"
                    )
            });
            tracks.insert(track.clone(),json!({"keys":authored.iter().map(|i|&self.rows[*i].1.key).collect::<Vec<_>>(),"order":sorted.iter().map(|i|&self.rows[*i].1.key).collect::<Vec<_>>(),"next":next.map(|i|&self.rows[*i].1.key)}));
            for i in sorted {
                let s = &self.rows[i].1;
                let r = &results[i];
                if s.reminder()
                    || s.optional()
                    || s.action.kind == Kind::SpecialProject
                    || matches!(
                        text(&r["status"]),
                        "completed" | "active" | "not_applicable"
                    )
                {
                    continue;
                }
                if matches!(text(&r["status"]), "waiting" | "unverifiable") {
                    if matches!(
                        text(&r["reason"]),
                        "dependency_not_completed"
                            | "dependency_unknown"
                            | "research_previous_level_incomplete"
                    ) {
                        continue;
                    } else {
                        break;
                    }
                }
                match s.action.kind {
                    Kind::SelectFocus => candidates[0].push(i),
                    Kind::Research => candidates[1].push(i),
                    _ => {}
                }
            }
        }
        let active = input.state.value("focus.active").and_then(Value::as_array);
        let focus_capacity = active.map(|a| usize::from(a.is_empty()));
        let research_capacity = input
            .state
            .value("research.slots")
            .and_then(Value::as_array)
            .map(|slots| slots.iter().filter(|s| !truth(&s["technology_id"])).count());
        let mut parallel = json!({"focus":{"capacity":focus_capacity,"steps":[]},"research":{"capacity":research_capacity,"steps":[]}});
        for (n, (lane, capacity)) in [("focus", focus_capacity), ("research", research_capacity)]
            .iter()
            .enumerate()
        {
            let mut seen = HashSet::new();
            let mut keys = Vec::new();
            candidates[n].sort_by_key(|i| ranks[*i]);
            for &i in &candidates[n] {
                if capacity.is_none_or(|cap| keys.len() >= cap) {
                    break;
                }
                let s = &self.rows[i].1;
                let identity = serde_json::to_string(&json!([
                    s.action.entity["scope"],
                    s.action.entity["id"]
                ]))
                .unwrap();
                if seen.insert(identity) {
                    keys.push(&s.key)
                }
            }
            parallel[lane]["steps"] = json!(keys);
        }
        if self.rows.iter().any(|(_, s)| s.optional()) {
            parallel["focus"]["optional_steps"] = json!(
                order
                    .iter()
                    .filter_map(|i| {
                        let s = &self.rows[*i].1;
                        (s.optional()
                            && results[*i]["status"] == "ready"
                            && focus_capacity.is_some_and(|c| c > 0))
                        .then_some(&s.key)
                    })
                    .collect::<Vec<_>>()
            );
        }
        let mut output = json!({"schema_version":1,"guide_id":self.guide.id,"run_id":input.run_id,"progress_status":input.progress_status,"mode":mode,"country":country,
            "tracks":tracks,"ready":order.iter().filter_map(|i|(results[*i]["status"]=="ready"&&!self.rows[*i].1.reminder()).then_some(&self.rows[*i].1.key)).collect::<Vec<_>>(),"parallel":parallel,
            "action_context":{"session_id":input.original_state.session_id,"campaign_id":input.original_state.value("campaign.id")},
            "action_sequence":{"political":self.rows.iter().enumerate().find_map(|(i,(_,s))|(s.domain()==Some("political")&&!matches!(text(&results[i]["status"]),"completed"|"not_applicable")).then_some(&s.key)),
                "projects":self.rows.iter().enumerate().filter_map(|(i,(_,s))|(s.domain()==Some("project")&&!matches!(text(&results[i]["status"]),"completed"|"not_applicable")).then_some(&s.key)).collect::<Vec<_>>()}});
        if results.iter().any(|r| r.get("focus_readiness").is_some()) {
            let keys: Vec<_> = self
                .guide
                .tracks
                .keys()
                .flat_map(|t| {
                    order.iter().copied().filter(|i| {
                        self.rows[*i].0 == *t && self.rows[*i].1.action.kind == Kind::SelectFocus
                    })
                })
                .filter(|i| {
                    !matches!(
                        text(&results[*i]["status"]),
                        "completed" | "not_applicable" | "active"
                    ) && !self.rows[*i].1.optional()
                })
                .collect();
            if let Some(&primary) = keys.first() {
                let mut plan = json!({"primary":self.rows[primary].1.key,"blocked":results[primary].get("focus_readiness").cloned().unwrap_or(json!({})),"steps":[]});
                if focus_capacity == Some(1) && no(&results[primary]["focus_readiness"]["value"]) {
                    let protected: HashSet<_> = keys
                        .iter()
                        .map(|i| text(&self.rows[*i].1.action.entity["id"]))
                        .collect();
                    for &i in keys.iter().skip(1) {
                        let s = &self.rows[i].1;
                        if s.action.entity == self.rows[primary].1.action.entity {
                            continue;
                        }
                        if truth(&s.focus_criteria)
                            && results[i]["status"] == "ready"
                            && yes(&results[i]["focus_readiness"]["value"])
                            && !strings(&s.focus_criteria["conflicts"])
                                .iter()
                                .any(|k| protected.contains(k.as_str()))
                        {
                            plan["steps"] = json!([s.key]);
                            break;
                        }
                    }
                }
                output["parallel"]["focus"]["alternative_steps"] = plan["steps"].clone();
                output["focus_plan"] = plan;
            }
        }
        output["steps"] = json!(results);
        Ok(output)
    }
}
