use super::*;

pub(super) fn deny(result: &mut Value, decision: Value) {
    if result.get("original_completion").is_none() {
        result["original_completion"] = result["completion"].clone()
    }
    if result.get("original_selection").is_none() {
        result["original_selection"] = result["selection"].clone()
    }
    result
        .as_object_mut()
        .unwrap()
        .extend(decision.as_object().unwrap().clone());
    result["production_admitted"] = json!(false);
    result["completion"] = unknown("revision_not_admitted");
    result["selection"] = unknown("revision_not_admitted");
}
fn push_unique(values: &mut Vec<Value>, v: Value) {
    if !values.contains(&v) {
        values.push(v)
    }
}
struct Revision {
    group: String,
    order: f64,
    row: usize,
    target: usize,
    remaining: bool,
    identity: Value,
    category: Value,
}
impl CompiledGuide {
    fn phase_reached(&self, step: &Step, input: &Input) -> bool {
        let mut guards = Vec::new();
        if let Some(v) = step.action.params.get("production_section_when") {
            guards.push(
                serde_json::from_value::<Condition>(v.clone())
                    .ok()
                    .and_then(|c| c.evaluate(&input.state, &input.checks)),
            )
        }
        for c in [
            self.phases.get(&step.phase).and_then(|p| p.when.as_ref()),
            step.when.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            guards.push(c.evaluate(&input.state, &input.checks))
        }
        !guards.is_empty()
            && guards.iter().all(|v| *v == Some(true))
            && (step.skip_when.is_none() || condition_value(&step.skip_when, input) == Some(false))
    }
    pub(super) fn revise(&self, input: &Input, results: &mut [Value]) -> Result<(), Error> {
        if !input.state.usable {
            return Ok(());
        }
        let protected: HashSet<_> = self
            .rows
            .iter()
            .flat_map(|(_, s)| s.depends_on.iter().chain(&s.depends_on_started))
            .collect();
        let mut admitted = Vec::new();
        for (i, (_, step)) in self.rows.iter().enumerate() {
            let p = &step.action.params;
            let revision = &p["production_revision"];
            if !truth(revision) || results[i]["reason"] == "guide_scope_mismatch" {
                continue;
            }
            if let Some(denied) = self.admission(step, input, results) {
                deny(&mut results[i], denied);
                if !self.phase_reached(step, input) {
                    continue;
                }
            }
            let targets = p
                .get("production_targets")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_else(|| vec![p.clone()]);
            let details = if p.get("production_targets").is_some() {
                list(&results[i]["production"]["layout"]).to_vec()
            } else {
                vec![results[i].get("production").cloned().unwrap_or(json!({}))]
            };
            let selectors = input.selectors.get(&step.key);
            for (n, (target, _)) in targets.iter().zip(details.iter()).enumerate() {
                let selector = selectors.and_then(|s| s.get(n));
                if let Some(selector) = selector
                    .filter(|s| !s.is_null())
                    .filter(|_| target["quantity_mode"] != "additional")
                {
                    admitted.push(Revision {
                        group: text(&revision["group"]).to_owned(),
                        order: revision["order"]
                            .as_f64()
                            .ok_or("Invalid production revision")?,
                        row: i,
                        target: n,
                        remaining: target["quantity_mode"] == "remaining",
                        identity: selector["identity"].clone(),
                        category: selector["category"].clone(),
                    });
                }
            }
        }
        admitted.sort_by(|a, b| a.group.cmp(&b.group).then(a.order.total_cmp(&b.order)));
        for (n, old) in admitted.iter().enumerate() {
            if protected.contains(&self.rows[old.row].1.key) {
                continue;
            }
            let mut replacements = Vec::new();
            for newer in admitted.iter().skip(n + 1) {
                if newer.group != old.group || newer.order <= old.order || newer.row == old.row {
                    continue;
                }
                if newer.identity == old.identity
                    || old.remaining
                        && newer.remaining
                        && old.category == "military"
                        && newer.category == "military"
                {
                    push_unique(&mut replacements, json!(self.rows[newer.row].1.key));
                }
            }
            if !replacements.is_empty() {
                if self.rows[old.row]
                    .1
                    .action
                    .params
                    .get("production_targets")
                    .is_some()
                {
                    results[old.row]["production"]["layout"][old.target]["superseded_by"] =
                        json!(replacements)
                } else {
                    results[old.row]["production"]["superseded_by"] = json!(replacements)
                }
            }
        }
        for (i, (_, step)) in self.rows.iter().enumerate() {
            let layout = step.action.params.get("production_targets").is_some();
            let details = if layout {
                list(&results[i]["production"]["layout"]).to_vec()
            } else {
                vec![results[i].get("production").cloned().unwrap_or(json!({}))]
            };
            if !details.iter().any(|d| truth(&d["superseded_by"])) {
                continue;
            }
            let mut replacements = Vec::new();
            for d in &details {
                for key in list(&d["superseded_by"]) {
                    push_unique(&mut replacements, key.clone())
                }
            }
            results[i]["superseded_by"] = json!(replacements);
            let remaining: Vec<_> = details
                .iter()
                .enumerate()
                .filter_map(|(n, d)| (!truth(&d["superseded_by"])).then_some(n))
                .collect();
            let clauses: Vec<_> = match &step.completion {
                Some(Condition::All { args }) if layout => args.iter().map(Some).collect(),
                other => vec![other.as_ref()],
            };
            let values = if clauses.len() == details.len() {
                remaining
                    .iter()
                    .map(|n| clauses[*n].and_then(|c| c.evaluate(&input.state, &input.checks)))
                    .collect::<Vec<_>>()
            } else {
                vec![None]
            };
            results[i]["original_completion"] = results[i]["completion"].clone();
            if !remaining.is_empty() && no(&results[i]["production_admitted"]) {
                continue;
            }
            if remaining.is_empty() || values.iter().all(|v| *v == Some(true)) {
                set(&mut results[i], "not_applicable", "production_superseded");
                results[i]["completion"] = unknown("requirements_replaced")
            } else if values.contains(&Some(false)) {
                let status = if yes(&results[i]["selection"]["value"]) {
                    "active"
                } else {
                    "ready"
                };
                set(&mut results[i], status, "production_partially_superseded");
                results[i]["completion"] = calculated(false, "effective_production_goal")
            } else {
                set(
                    &mut results[i],
                    "unverifiable",
                    "production_evidence_unknown",
                )
            }
        }
        Ok(())
    }
}
