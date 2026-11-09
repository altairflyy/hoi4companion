use super::*;

fn includes(chain: &Value, id: &Value) -> bool {
    list(chain).contains(id)
}
fn branches(choice: &Value) -> Vec<(&String, &Value)> {
    let mut branches:Vec<_>=choice["branches"]
        .as_object()
        .map(|o| o.iter().collect())
        .unwrap_or_default();
    branches.sort_by(|(a,_),(b,_)|a.cmp(b));
    branches
}
impl CompiledGuide {
    pub(super) fn project_research(&mut self, input: &Input) {
        for (_, step) in &mut self.rows {
            let selected = input.research_bindings.get(&step.key);
            if truth(&step.research_choice) {
                let valid = selected.filter(|id| {
                    branches(&step.research_choice)
                        .iter()
                        .any(|(_, chain)| includes(chain, id))
                });
                let id = valid
                    .cloned()
                    .unwrap_or_else(|| json!(format!("unbound-choice:{}", step.key)));
                step.catalog_match = if valid.is_none() { Some(false) } else { None };
                step.target = text(&id).into();
                step.action.entity["id"] = id;
            }
            if !step.relative_research.is_null() || yes(&step.relative_research_unresolved) {
                let valid = selected.filter(|id| includes(&step.relative_research["chain"], id));
                if valid.is_none() {
                    step.catalog_match = Some(false)
                } else if text(&step.action.entity["id"]).starts_with("unbound-up:") {
                    step.catalog_match = None
                }
                let id = valid
                    .cloned()
                    .unwrap_or_else(|| json!(format!("unbound-up:{}", step.key)));
                step.target = text(&id).into();
                step.action.entity["id"] = id;
            }
        }
    }
    pub(super) fn research_progression(&self, state: &State) -> BTreeMap<String, String> {
        let mut result = BTreeMap::new();
        let fact = state.facts.get("research.completed");
        if !state.usable
            || fact.is_none_or(|f| {
                f["status"] != "observed" || text(&f["source"]).is_empty() || !f["value"].is_array()
            })
        {
            return result;
        }
        let done = list(&fact.unwrap()["value"]);
        for track in self.guide.tracks.keys() {
            let mut previous = Vec::<&Step>::new();
            for (_, step) in self
                .rows
                .iter()
                .filter(|(t, s)| t == track && s.action.kind == Kind::Research)
            {
                if truth(&step.relative_research) {
                    let chain = list(&step.relative_research["chain"]);
                    let target = &step.action.entity["id"];
                    let reason = if let Some(n) = chain.iter().position(|id| id == target) {
                        chain[..n]
                            .iter()
                            .any(|id| !done.contains(id))
                            .then_some("research_previous_level_incomplete")
                    } else if chain.iter().all(|id| done.contains(id)) {
                        Some("research_line_complete")
                    } else if previous.iter().any(|p| {
                        chain.contains(&p.action.entity["id"])
                            && !done.contains(&p.action.entity["id"])
                    }) {
                        Some("research_previous_level_incomplete")
                    } else {
                        None
                    };
                    if let Some(reason) = reason {
                        result.insert(step.key.clone(), reason.into());
                    }
                }
                previous.push(step);
            }
        }
        result
    }
    pub fn research_choices(&self, input: &Input) -> Object {
        let choices: Vec<_> = self
            .rows
            .iter()
            .enumerate()
            .filter_map(|(i, (_, s))| truth(&s.research_choice).then_some((i, s)))
            .collect();
        let mut result: Object = choices
            .iter()
            .map(|(_, s)| {
                (
                    s.key.clone(),
                    json!({"options":[],"branch":null,"status":"unknown"}),
                )
            })
            .collect();
        if choices.is_empty() || !input.state.usable || input.progress_status != "accepted" {
            return result;
        }
        let mut values = Vec::new();
        for key in ["research.completed", "research.active"] {
            let Some(f) = input.state.facts.get(key) else {
                return result;
            };
            if f["status"] != "observed"
                || text(&f["source"]).is_empty()
                || !f["value"].is_array()
                || !list(&f["value"]).iter().all(Value::is_string)
            {
                return result;
            }
            values.push(list(&f["value"]));
        }
        let completed = values[0];
        let active = values[1];
        let mut groups = BTreeMap::<String, Value>::new();
        for (_, step) in &choices {
            let choice = &step.research_choice;
            let group = text(&choice["group"]);
            if let Some(existing) = groups.get(group) {
                if existing != &choice["branches"] {
                    return result;
                }
            } else {
                groups.insert(group.into(), choice["branches"].clone());
            }
        }
        let mut selected = BTreeMap::<String, Vec<String>>::new();
        for (group, options) in &groups {
            let used: Vec<_> = choices
                .iter()
                .filter(|(_, s)| s.research_choice["group"] == *group)
                .filter_map(|(_, s)| input.research_bindings.get(&s.key))
                .collect();
            let seen = options
                .as_object()
                .unwrap()
                .iter()
                .filter_map(|(branch, chain)| {
                    list(chain)
                        .iter()
                        .any(|id| {
                            completed.contains(id) || active.contains(id) || used.contains(&id)
                        })
                        .then_some(branch.clone())
                })
                .collect();
            selected.insert(group.clone(), seen);
        }
        let mut previous = BTreeMap::<String, Vec<&Step>>::new();
        for (i, step) in choices {
            let choice = &step.research_choice;
            let group = text(&choice["group"]);
            let selection = &selected[group];
            let branch = if selection.len() == 1 {
                Some(selection[0].as_str())
            } else {
                None
            };
            let info = result.get_mut(&step.key).unwrap();
            info["branch"] = json!(branch);
            let bound = input.research_bindings.get(&step.key);
            if let Some(bound) = bound.filter(|id| {
                branches(choice)
                    .iter()
                    .any(|(_, chain)| includes(chain, id))
            }) {
                info["status"] = json!("bound");
                info["options"] = json!([bound]);
                previous.entry(group.into()).or_default().push(step);
                continue;
            }
            if choice["mode"] == "once" {
                let saved = input.research_options.get(&step.key);
                let valid = saved.and_then(Value::as_array).is_some_and(|ids| {
                    !ids.is_empty()
                        && ids.len() <= branches(choice).len()
                        && ids.iter().all(|id| {
                            id.is_string() && branches(choice).iter().any(|(_, c)| includes(c, id))
                        })
                        && ids.iter().collect::<HashSet<_>>().len() == ids.len()
                        && branches(choice)
                            .iter()
                            .all(|(_, c)| ids.iter().filter(|id| includes(c, id)).count() <= 1)
                });
                let mut options = Vec::new();
                for (_, chain) in branches(choice) {
                    let frozen = if valid {
                        saved
                            .and_then(Value::as_array)
                            .and_then(|s| s.iter().find(|id| includes(chain, id)))
                    } else {
                        None
                    };
                    let blocked = list(chain).len() > 1
                        && self.rows[..i]
                            .iter()
                            .filter(|(_, s)| s.action.kind == Kind::Research)
                            .any(|(_, p)| {
                                let id = input
                                    .research_bindings
                                    .get(&p.key)
                                    .unwrap_or(&p.action.entity["id"]);
                                includes(chain, id) && !completed.contains(id)
                                    || (p.relative_research["chain"] == *chain
                                        || branches(&p.research_choice)
                                            .iter()
                                            .any(|(_, c)| *c == chain))
                                        && !input.research_bindings.contains_key(&p.key)
                            });
                    let target = frozen.or_else(|| {
                        if blocked {
                            None
                        } else if list(chain).len() == 1 {
                            list(chain).first()
                        } else {
                            list(chain).iter().find(|id| !completed.contains(id))
                        }
                    });
                    if let Some(id) = target {
                        options.push(id.clone())
                    }
                }
                let chosen: Vec<_> = options
                    .iter()
                    .filter(|id| {
                        active.contains(id)
                            || completed.contains(id)
                                && (valid
                                    || branches(choice).iter().any(|(_, c)| {
                                        list(c).len() == 1 && list(c).first() == Some(*id)
                                    }))
                    })
                    .cloned()
                    .collect();
                if chosen.len() == 1 {
                    info["branch"] = json!(
                        branches(choice)
                            .into_iter()
                            .find(|(_, c)| includes(c, &chosen[0]))
                            .map(|(key, _)| key)
                    );
                    info["status"] = json!("selected");
                    info["options"] = json!(chosen)
                } else if !chosen.is_empty() {
                    info["status"] = json!("conflict");
                    info["options"] = json!([])
                } else {
                    info["branch"] = Value::Null;
                    info["status"] = json!("open");
                    info["options"] = json!(options)
                }
                continue;
            }
            if selection.len() > 1 {
                info["status"] = json!("conflict")
            } else if previous.get(group).is_some_and(|p| {
                p.iter().any(|s| {
                    input
                        .research_bindings
                        .get(&s.key)
                        .is_none_or(|id| !completed.contains(id))
                })
            }) {
                info["status"] = json!("waiting")
            } else {
                let mut options = Vec::new();
                for (key, chain) in branches(choice) {
                    if branch.is_some_and(|b| b != key) {
                        continue;
                    }
                    let target = if let Some(n) = choice["level"].as_u64() {
                        list(chain).get(n.saturating_sub(1) as usize)
                    } else {
                        list(chain).iter().find(|id| !completed.contains(id))
                    };
                    if let Some(id) = target {
                        options.push(id)
                    }
                }
                info["options"] = json!(options);
                info["status"] = json!(if branch.is_some() { "selected" } else { "open" });
            }
            previous.entry(group.into()).or_default().push(step);
        }
        result
    }
}
