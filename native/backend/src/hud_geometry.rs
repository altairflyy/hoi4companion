//! GUI-space geometry and navigation. Identity comes only from witnessed
//! native observations; labels, clicks and pixel changes are never evidence.
use serde_json::{Value, json};
type Result<T> = std::result::Result<T, String>;
pub(crate) fn rows(value: &Value) -> &[Value] {
    value.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn text(value: &Value) -> &str {
    value.as_str().unwrap_or("")
}
fn yes(value: &Value) -> bool {
    value.as_bool() == Some(true)
}
fn present(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(a) => !a.is_empty(),
        Value::Number(n) => n.as_f64() != Some(0.),
    }
}
fn numbers<const N: usize>(v: &Value) -> Result<[f64; N]> {
    let items = v
        .as_array()
        .filter(|a| a.len() == N)
        .ok_or("Invalid GUI vector")?;
    let mut out = [0.; N];
    for (i, item) in items.iter().enumerate() {
        out[i] = item
            .as_f64()
            .filter(|f| f.is_finite())
            .ok_or("Invalid GUI number")?;
    }
    Ok(out)
}
pub(crate) fn intersect(a: [f64; 4], b: [f64; 4]) -> [f64; 4] {
    let x = a[0].max(b[0]);
    let y = a[1].max(b[1]);
    [
        x,
        y,
        (a[0] + a[2]).min(b[0] + b[2]).sub(x).max(0.),
        (a[1] + a[3]).min(b[1] + b[3]).sub(y).max(0.),
    ]
}
use std::ops::Sub;
pub(crate) fn anchored(
    parent: [f64; 16],
    position: [f64; 2],
    size: [f64; 2],
    child: f64,
    clip: [f64; 4],
) -> Result<Value> {
    let sx = parent[0];
    let sy = parent[5];
    if parent
        .iter()
        .chain(position.iter())
        .chain([child].iter())
        .any(|v| !v.is_finite())
        || !(0.01..=10.).contains(&sx)
        || (sx - sy).abs() > 0.0001
        || !(0.01..=10.).contains(&child)
        || size
            .iter()
            .any(|v| !v.is_finite() || *v <= 0. || *v > 65536.)
        || [1, 2, 3, 4, 6, 7, 8, 9, 11]
            .iter()
            .any(|i| parent[*i].abs() > 0.0001)
    {
        return Err("Ancoraggio sprite non qualificato".into());
    }
    let rect = [
        parent[12] + position[0] * sx,
        -parent[13] + position[1] * sy,
        size[0] * sx * child,
        size[1] * sy * child,
    ];
    let clipped = intersect(rect, clip);
    Ok(
        json!({"rect":rect,"clip":clipped,"visible":clipped[2]>0. && clipped[3]>0.,"scale":sx*child}),
    )
}
pub(crate) fn sprite(args: &Value) -> Result<Value> {
    let mut matrix = numbers::<16>(&args["matrix"])?;
    let size = numbers::<2>(&args["size"])?;
    let clip = numbers::<4>(&args["clip"])?;
    if args.get("position").is_some() {
        return anchored(
            matrix,
            numbers::<2>(&args["position"])?,
            size,
            args["child_scale"].as_f64().ok_or("Invalid child scale")?,
            clip,
        );
    }
    let resolution = numbers::<2>(&args["resolution"])?;
    matrix[12] += resolution[0] / 2.;
    matrix[13] = matrix[13] - resolution[1] / 2. + size[1] * matrix[5];
    anchored(matrix, [0., 0.], size, 1., clip)
}
fn bbox(v: &Value) -> Option<[f64; 4]> {
    let b = numbers::<4>(v).ok()?;
    if b[2] <= 0. || b[3] <= 0. {
        None
    } else {
        Some([b[0], b[1], b[0] + b[2], b[1] + b[3]])
    }
}
pub(crate) fn rectangle(node: &Value, panel: &Value, view: &Value) -> Option<Value> {
    let a = bbox(&node["rect"])?;
    let b = bbox(&node["clip"])?;
    let c = bbox(&panel["clip"])?;
    let width = view[4].as_f64()?;
    let height = view[5].as_f64()?;
    let left = a[0].max(b[0]).max(c[0]).max(0.);
    let top = a[1].max(b[1]).max(c[1]).max(0.);
    let right = a[2].min(b[2]).min(c[2]).min(width);
    let bottom = a[3].min(b[3]).min(c[3]).min(height);
    (right - left >= 8. && bottom - top >= 8.).then(|| json!([left, top, right, bottom]))
}
fn rect(control: &Value, view: &Value, pass: bool) -> Option<Value> {
    if pass {
        control.get("rect").cloned()
    } else {
        rectangle(control, &json!({"clip":control["clip"]}), view)
    }
}
pub(crate) fn resource_ready(v: &Value) -> bool {
    v.is_object()
        && yes(&v["resources_available"])
        && !present(&v["blocked"])
        && !(present(&v["requirements_declared"])
            && rows(&v["unknown"])
                .iter()
                .any(|r| r == "action_requirements_unknown"))
}
pub(crate) fn actions(raw: &Value, proposals: &Value, view: &Value, pass: bool) -> Vec<Value> {
    let obs = &raw["action_ui"];
    let active = text(&raw["active_panel"]);
    if obs["status"] != "observed" || ["focus", "research", "event"].contains(&active) {
        return vec![];
    }
    let mut result = vec![];
    let panels = &obs["panels"];
    let controls = rows(&obs["controls"]);
    for target in rows(proposals) {
        let panel = text(&target["panel"]);
        let politics = panel == "politics";
        if !["politics", "projects"].contains(&panel)
            || target["country"] != raw["country"]
            || !resource_ready(&target["action_readiness"])
        {
            continue;
        }
        let required: &[&str] = if politics {
            &["topbar", "countrypoliticsview", "political_ideas_window"]
        } else {
            &[
                "topbar",
                "countrytechnologyview",
                "facilities_view",
                "program_window",
            ]
        };
        if required.iter().any(|key| panels.get(key).is_none()) {
            continue;
        }
        let category = &target["category_id"];
        let (stage, choices): (&str, Vec<&Value>) = if politics {
            if present(&panels["political_ideas_window"]) {
                (
                    "entity",
                    controls
                        .iter()
                        .filter(|c| {
                            c["root"] == "political_ideas_window"
                                && c["entity_id"] == target["identity"]
                                && c["entity_kind"] == target["entity_kind"]
                                && c["category_id"] == *category
                                && present(category)
                        })
                        .collect(),
                )
            } else if present(&panels["countrypoliticsview"]) {
                (
                    "slot",
                    controls
                        .iter()
                        .filter(|c| {
                            c["root"] == "countrypoliticsview"
                                && present(category)
                                && c["category_id"] == *category
                                && [
                                    "add_idea_button",
                                    "idea_entry",
                                    "open_roster_button",
                                    "button",
                                ]
                                .contains(&text(&c["name"]))
                        })
                        .collect(),
                )
            } else {
                (
                    "access",
                    controls
                        .iter()
                        .filter(|c| c["root"] == "topbar" && c["name"] == "player_flag")
                        .collect(),
                )
            }
        } else if present(&panels["program_window"]) {
            (
                "entity",
                controls
                    .iter()
                    .filter(|c| {
                        c["root"] == "program_window"
                            && present(&target["specialization"])
                            && c["specialization"] == target["specialization"]
                            && c["entity_kind"] == "project"
                            && c["entity_id"] == target["identity"]
                    })
                    .collect(),
            )
        } else if present(&panels["facilities_view"]) {
            (
                "facility",
                controls
                    .iter()
                    .filter(|c| {
                        c["root"] == "facilities_view"
                            && present(&target["specialization"])
                            && c["specialization"] == target["specialization"]
                            && ["open_facility_button", "facility_button"]
                                .contains(&text(&c["name"]))
                    })
                    .collect(),
            )
        } else if present(&panels["countrytechnologyview"]) {
            (
                "access",
                controls
                    .iter()
                    .filter(|c| {
                        c["root"] == "countrytechnologyview" && c["name"] == "facilities_tab"
                    })
                    .collect(),
            )
        } else {
            (
                "access",
                controls
                    .iter()
                    .filter(|c| c["root"] == "topbar" && c["name"] == "technology_button")
                    .collect(),
            )
        };
        let choices: Vec<_> = choices.into_iter().filter(|c| yes(&c["visible"])).collect();
        if choices.len() != 1 {
            continue;
        }
        let Some(rectangle) = rect(choices[0], view, pass) else {
            continue;
        };
        let title = match stage {
            "access" => Value::String(
                if politics {
                    "Apri Politica"
                } else {
                    "Apri Ricerca e progetti speciali"
                }
                .into(),
            ),
            "slot" => json!(format!("Apri lo slot {}", text(category))),
            "facility" => json!("Apri l’impianto compatibile"),
            _ => target.get("title").unwrap_or(&target["identity"]).clone(),
        };
        let mut item = target.clone();
        item["title"] = title;
        item["rect"] = rectangle;
        item["navigation_stage"] = json!(stage);
        item["basis"] = json!("native_gui_action_identity");
        result.push(item);
    }
    result
}
fn name(v: &str) -> String {
    hoi4companion_catalog_core::nfkd(v)
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '+')
        .collect()
}
fn variants<'a>(controls: Vec<&'a Value>, target: &Value, picker: bool) -> Vec<&'a Value> {
    let selector = &target["production_selector"];
    if let Some(expected) = selector.get("design").filter(|v| !v.is_null()) {
        if !picker {
            return controls;
        }
        return controls
            .into_iter()
            .filter(|c| {
                crate::production_context::compare(expected, &c["design"])["status"] == "match"
            })
            .collect();
    }
    let names = rows(&selector["variant_names"]);
    if names.is_empty() {
        return controls;
    }
    let normalized: Vec<_> = names.iter().filter_map(Value::as_str).map(name).collect();
    controls
        .into_iter()
        .filter(|c| {
            c["variant_name"]
                .as_str()
                .is_some_and(|s| normalized.contains(&name(s)))
        })
        .collect()
}
pub(crate) fn production(raw: &Value, proposals: &Value, view: &Value, pass: bool) -> Vec<Value> {
    let ui = &raw["production_ui"];
    if ui["status"] != "observed"
        || ["focus", "research", "event"].contains(&text(&raw["active_panel"]))
    {
        return vec![];
    }
    let mut result = vec![];
    for target in rows(proposals) {
        if target["panel"] != "production" || target["country"] != raw["country"] {
            continue;
        }
        let missing = yes(&target["production_missing"]) && !yes(&target["goal_satisfied"]);
        if present(&ui["visible"]) && missing {
            let key = &target["production_add_control"];
            let category: Vec<_> = rows(&ui["add"])
                .iter()
                .filter(|c| present(key) && c["id"] == *key && yes(&c["visible"]))
                .collect();
            if category.len() == 1
                && let Some(r) = rect(category[0], view, pass)
            {
                let mut m = target.clone();
                m["rect"] = r;
                m["navigation_stage"] = json!("add");
                m["basis"] = json!("native_gui_action_identity");
                result.push(m);
            }
        }
        let equipment = rows(&target["production_selector"]["equipment_ids"]);
        let (stage, controls) = if !present(&ui["visible"]) {
            if yes(&target["goal_satisfied"]) {
                continue;
            }
            ("access", rows(&ui["access"]).iter().collect())
        } else if present(&ui["picker_open"]) {
            if ui["picker_count"] != 1 || !missing {
                continue;
            }
            (
                "equipment",
                variants(
                    rows(&ui["equipment"])
                        .iter()
                        .filter(|c| equipment.contains(&c["equipment"]))
                        .collect(),
                    target,
                    true,
                ),
            )
        } else if yes(&target["production_design_unverified"]) {
            let ids = rows(&target["production_candidate_line_ids"]);
            if ids.len() != 1 {
                continue;
            }
            (
                "design_candidate",
                rows(&ui["lines"])
                    .iter()
                    .filter(|c| c["line_id"] == ids[0] && equipment.contains(&c["equipment"]))
                    .collect(),
            )
        } else if missing {
            continue;
        } else {
            let ids = rows(&target["production_line_ids"]);
            if ids.len() != 1 {
                continue;
            }
            (
                "line",
                variants(
                    rows(&ui["lines"])
                        .iter()
                        .filter(|c| c["line_id"] == ids[0] && equipment.contains(&c["equipment"]))
                        .collect(),
                    target,
                    false,
                ),
            )
        };
        let controls: Vec<_> = controls
            .into_iter()
            .filter(|c| yes(&c["visible"]))
            .collect();
        if controls.len() != 1 {
            continue;
        }
        let Some(r) = rect(controls[0], view, pass) else {
            continue;
        };
        let mut item = target.clone();
        item["rect"] = r;
        item["navigation_stage"] = json!(stage);
        item["basis"] = json!("native_gui_action_identity");
        item["title"] = if stage == "access" {
            json!("Apri Produzione")
        } else {
            target["title"].clone()
        };
        result.push(item);
        if stage == "access" {
            break;
        }
    }
    let mut grouped: Vec<Value> = vec![];
    for mut item in result {
        if item["navigation_stage"] == "add" {
            if let Some(previous) = grouped
                .iter_mut()
                .find(|p| p["navigation_stage"] == "add" && p["rect"] == item["rect"])
            {
                for (field, value) in [
                    ("production_add_names", &item["title"]),
                    ("step_keys", &item["key"]),
                ] {
                    let a = previous[field].as_array_mut().unwrap();
                    if !a.contains(value) {
                        a.push(value.clone());
                    }
                }
                continue;
            }
            item["production_add_names"] = json!([item["title"]]);
            item["step_keys"] = json!([item["key"]]);
        }
        grouped.push(item);
    }
    grouped
}
pub(crate) fn navigation(args: &Value) -> Value {
    let pass = yes(&args["passthrough"]);
    json!({"actions":actions(&args["raw"],&args["proposals"],&args["view"],pass),"production":production(&args["raw"],&args["proposals"],&args["view"],pass)})
}

/// Qualify proposals against one witnessed snapshot and one viewport. A native
/// failure suspends visual fallback; stale coordinates never survive failure.
pub(crate) fn admit(raw: &Value, proposals: &Value, view: &Value) -> Result<Value> {
    let mut result = json!({"available":true,"matches":[],"handled_panels":["focus","research","event"],"active_panel":null,"diagnostics":raw.get("diagnostics").cloned().unwrap_or(json!({}))});
    if !yes(&raw["available"]) {
        result["suspend_visual"] = json!(true);
        result["error"] = raw
            .get("error")
            .cloned()
            .unwrap_or(json!("GUI non verificabile"));
        return Ok(result);
    }
    if raw["pid"] != view[1] || raw["resolution"] != json!([view[4], view[5]]) {
        result["suspend_visual"] = json!(true);
        result["error"] = json!("Processo o risoluzione GUI non corrispondenti");
        return Ok(result);
    }
    let windows: Vec<_> = rows(&raw["event_windows"])
        .iter()
        .filter(|w| yes(&w["visible"]))
        .collect();
    let active = if windows.is_empty() {
        text(&raw["active_panel"])
    } else {
        "event"
    };
    let panels = &raw["panels"];
    let country = &raw["country"];
    if active == "production" && raw["production_ui"]["status"] == "observed"
        || ["focus", "research"].contains(&active) && yes(&panels[active]["visible"])
        || active == "event"
    {
        result["active_panel"] = json!(active);
    }
    let mut matches = vec![];
    for proposal in rows(proposals) {
        let panel = text(&proposal["panel"]);
        let info = &panels[panel];
        if active == "event" && panel != "event" {
            continue;
        }
        if panel == "event" {
            if *country != proposal["country"]
                || raw["events"]["status"] != "observed"
                || windows.len() != 1
            {
                continue;
            }
            let event_id = proposal["event_id"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| text(&proposal["identity"]).split(':').next().unwrap_or(""));
            let window = windows[0];
            if window["event_id"] != event_id {
                continue;
            }
            let nodes: Vec<_> = rows(&raw["nodes"])
                .iter()
                .filter(|n| {
                    n["panel"] == "event"
                        && n["identity"] == proposal["identity"]
                        && yes(&n["visible"])
                        && n["event_id"] == event_id
                        && n["option_id"] == proposal["option_id"]
                        && n["instance_id"] == window["instance_id"]
                })
                .collect();
            if nodes.len() != 1 {
                continue;
            }
            if let Some(rect) = rectangle(nodes[0], window, view) {
                let mut m = proposal.clone();
                m["rect"] = rect;
                m["instance_id"] = window["instance_id"].clone();
                m["basis"] = json!("native_gui_identity");
                matches.push(m);
            }
            continue;
        }
        if !["focus", "research"].contains(&panel)
            || !yes(&info["visible"])
            || !present(country)
            || *country != proposal["country"]
        {
            continue;
        }
        if panel == "focus" && present(&proposal["scope"]) && proposal["scope"] != raw["scope"]
            || panel == "research"
                && (!present(&info["folder"]) || proposal["folder"] != info["folder"])
        {
            continue;
        }
        let nodes: Vec<_> = rows(&raw["nodes"])
            .iter()
            .filter(|n| {
                n["panel"] == panel
                    && n["identity"] == proposal["identity"]
                    && yes(&n["visible"])
                    && (panel != "focus" || n["scope"] == raw["scope"])
                    && (panel != "research" || n["folder"] == info["folder"])
            })
            .collect();
        if nodes.len() != 1 {
            continue;
        }
        if let Some(rect) = rectangle(nodes[0], info, view) {
            let mut m = proposal.clone();
            m["rect"] = rect;
            m["basis"] = json!("native_gui_identity");
            matches.push(m);
        }
    }
    let research = &panels["research"];
    if active == "research" && yes(&research["visible"]) && present(&research["folder"]) {
        let mut folders: Vec<(&Value, Vec<&Value>)> = vec![];
        for p in rows(proposals) {
            if p["panel"] == "research"
                && p["status"] != "active"
                && p["country"] == *country
                && present(&p["folder"])
                && p["folder"] != research["folder"]
            {
                if let Some((_, steps)) = folders
                    .iter_mut()
                    .find(|(folder, _)| **folder == p["folder"])
                {
                    steps.push(p);
                } else {
                    folders.push((&p["folder"], vec![p]));
                }
            }
        }
        for (folder, steps) in folders {
            let tabs: Vec<_> = rows(&raw["tabs"])
                .iter()
                .filter(|t| {
                    t["panel"] == "research_tab" && t["identity"] == *folder && yes(&t["visible"])
                })
                .collect();
            if tabs.len() != 1 {
                continue;
            }
            if let Some(rect) = rectangle(tabs[0], &json!({"clip":tabs[0]["clip"]}), view) {
                matches.push(json!({"key":format!("research-tab:{}",text(folder)),"panel":"research_tab","identity":folder,"folder":folder,"title":if present(&steps[0]["folder_name"]){&steps[0]["folder_name"]}else{folder},"status":if steps.iter().all(|p|p["status"]=="active"){"active"}else{"ready"},"tone":if steps.iter().all(|p|p["tone"]=="choice"){"choice"}else{"next"},"rect":rect,"step_keys":steps.iter().map(|p|&p["key"]).collect::<Vec<_>>(),"count":steps.len(),"basis":"native_gui_identity"}));
            }
        }
    }
    let mut qualified = raw.clone();
    qualified["active_panel"] = json!(active);
    if raw["production_ui"]["status"] == "observed" {
        result["handled_panels"]
            .as_array_mut()
            .unwrap()
            .push(json!("production"));
        if active != "event" {
            matches.extend(production(&qualified, proposals, view, false));
        }
    }
    let actions_obs = &raw["action_ui"];
    if actions_obs["status"] == "observed" {
        if let Some(coverage) = actions_obs["coverage"].as_object() {
            for (panel, known) in coverage {
                if present(known) {
                    result["handled_panels"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!(panel));
                }
            }
        }
        if let Some(controls) = qualified["action_ui"]["controls"].as_array_mut() {
            for control in controls {
                if control["entity_kind"] == "advisor" {
                    let mut roles: Vec<_> = rows(proposals)
                        .iter()
                        .filter(|p| {
                            p["panel"] == "politics"
                                && p["entity_kind"] == "advisor"
                                && p["identity"] == control["entity_id"]
                        })
                        .map(|p| p["category_id"].clone())
                        .collect();
                    roles.sort_by_key(Value::to_string);
                    roles.dedup();
                    if roles.len() == 1 {
                        control["category_id"] = roles.remove(0);
                    }
                }
            }
        }
        matches.extend(actions(&qualified, proposals, view, false));
        let p = &actions_obs["panels"];
        if result["active_panel"].is_null()
            && present(&actions_obs["coverage"]["politics"])
            && (present(&p["countrypoliticsview"]) || present(&p["political_ideas_window"]))
        {
            result["active_panel"] = json!("politics");
        }
        if result["active_panel"].is_null()
            && present(&actions_obs["coverage"]["projects"])
            && (present(&p["program_window"]) || present(&p["facilities_view"]))
        {
            result["active_panel"] = json!("projects");
        }
    }
    result["explanations"] = crate::hud_diagnostics::explain(raw, proposals, &matches);
    result["matches"] = json!(matches);
    Ok(result)
}
