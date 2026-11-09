use super::*;
fn focus_metadata(guide: &Value) -> Result<()> {
    if let Some(policy) = guide.get("focus_sequence").filter(|v| !v.is_null()) {
        if !keys(
            policy,
            &["schema_version", "coverage", "initial_focuses"],
            &[],
        ) || !integer(&policy["schema_version"], 1, 1)
            || !["partial", "complete"].contains(&text(&policy["coverage"]))
        {
            return Err("Invalid focus sequence policy".into());
        }
        let empty = json!([]);
        let initial = policy.get("initial_focuses").unwrap_or(&empty);
        if policy["coverage"] == "complete" && policy.get("initial_focuses").is_none()
            || !strings(initial, 0, 4096, 512, true)
        {
            return Err("Invalid initial focus list".into());
        }
    }
    let mut positions = HashSet::new();
    for track in guide["tracks"].as_object().unwrap().values() {
        let mut previous = 0;
        for row in track.as_array().unwrap() {
            let Some(r) = row.get("focus_resolution") else {
                continue;
            };
            if row["action"]["kind"] != "select_focus"
                || !keys(
                    r,
                    &[
                        "schema_version",
                        "position",
                        "query",
                        "classification",
                        "entity",
                        "locked",
                    ],
                    &[],
                )
                || !integer(&r["schema_version"], 1, 1)
                || !integer(&r["position"], 1, 4096)
                || r["position"].as_i64().unwrap() <= previous
                || !positions.insert(r["position"].as_i64().unwrap())
                || !nonempty(&r["query"], 2048)
                || !["direct", "reconstructed", "probable", "unresolved"]
                    .contains(&text(&r["classification"]))
                || r.get("locked").is_some_and(|v| !v.is_boolean())
            {
                return Err("Invalid focus source position or resolution provenance".into());
            }
            entity(&r["entity"])?;
            if r["entity"]["kind"] != "focus" {
                return Err("Invalid focus provenance entity".into());
            }
            previous = r["position"].as_i64().unwrap();
        }
    }
    Ok(())
}
fn focus_rules(rules: &Value) -> Result<()> {
    if !keys(
        rules,
        &[
            "schema_version",
            "available",
            "branch",
            "prerequisites",
            "mutex",
            "conflicts",
        ],
        &[
            "schema_version",
            "available",
            "branch",
            "prerequisites",
            "mutex",
            "conflicts",
        ],
    ) || !integer(&rules["schema_version"], 1, 1)
    {
        return Err("Invalid focus criteria metadata".into());
    }
    let mut budget = 0;
    criterion(&rules["available"], 0, &mut budget)?;
    criterion(&rules["branch"], 0, &mut budget)?;
    for field in ["mutex", "conflicts"] {
        if !strings(&rules[field], 0, 4096, usize::MAX, false) {
            return Err("Invalid focus exclusions".into());
        }
    }
    if rules["prerequisites"]
        .as_array()
        .is_none_or(|a| a.len() > 128 || a.iter().any(|g| !strings(g, 1, 512, usize::MAX, false)))
    {
        return Err("Invalid focus prerequisite groups".into());
    }
    Ok(())
}
fn criterion(node: &Value, depth: usize, budget: &mut usize) -> Result<()> {
    *budget += 1;
    if depth > 32 || *budget > 4096 || !node.is_object() {
        return Err("Invalid focus criterion".into());
    }
    match text(&node["op"]) {
        "all" | "any" => {
            if !keys(node, &["op", "args"], &["op", "args"]) || !node["args"].is_array() {
                return Err("Invalid focus criteria group".into());
            }
            for child in node["args"].as_array().unwrap() {
                criterion(child, depth + 1, budget)?
            }
        }
        "not" => {
            if !keys(node, &["op", "arg"], &["op", "arg"]) {
                return Err("Invalid focus criteria negation".into());
            }
            criterion(&node["arg"], depth + 1, budget)?
        }
        "if" => {
            if !keys(
                node,
                &["op", "condition", "then"],
                &["op", "condition", "then"],
            ) {
                return Err("Invalid focus criteria implication".into());
            }
            criterion(&node["condition"], depth + 1, budget)?;
            criterion(&node["then"], depth + 1, budget)?
        }
        "const" => {
            if !keys(node, &["op", "value"], &["op", "value"]) || !node["value"].is_boolean() {
                return Err("Invalid focus constant".into());
            }
        }
        "unknown" => {
            if !keys(node, &["op", "trigger", "value"], &[])
                || !node["trigger"].is_string()
                || node.get("value").is_some_and(|v| !v.is_string())
            {
                return Err("Invalid unknown focus predicate".into());
            }
        }
        "compare" => {
            let test = text(&node["test"]);
            let value = &node["value"];
            if !keys(
                node,
                &["op", "fact", "test", "value", "trigger"],
                &["op", "fact", "test", "value", "trigger"],
            ) || !node["fact"].is_string()
                || ![">", "<", ">=", "<=", "=", "!=", "contains"].contains(&test)
                || !(value.is_string() || value.is_boolean() || value.is_number())
                || [">", "<", ">=", "<="].contains(&test) && !value.is_number()
                || test == "contains" && !value.is_string()
                || !node["trigger"].is_string()
            {
                return Err("Invalid focus comparison".into());
            }
        }
        _ => return Err("Unsupported focus criteria operator".into()),
    }
    Ok(())
}
fn spirit_currency(group: &Value) -> Option<&'static str> {
    match text(group) {
        "academy_spirit" | "army_spirit" | "division_command_spirit" => Some("army_experience"),
        "naval_academy_spirit" | "navy_spirit" | "naval_command_spirit" => Some("navy_experience"),
        "air_force_spirit" | "air_force_command_spirit" | "air_force_academy_spirit" => {
            Some("air_experience")
        }
        _ => None,
    }
}
fn valid_date(value: &Value) -> bool {
    let Some(value) = value.as_str() else {
        return false;
    };
    let parts = value.split('.').collect::<Vec<_>>();
    if parts.len() != 3
        || ![4, 2, 2].iter().zip(&parts).all(|(max, p)| {
            !p.is_empty() && p.len() <= *max && p.bytes().all(|b| b.is_ascii_digit())
        })
    {
        return false;
    }
    let parts = parts
        .iter()
        .map(|p| p.parse::<u32>().unwrap())
        .collect::<Vec<_>>();
    let (y, m, d) = (parts[0], parts[1], parts[2]);
    if y == 0 || y > 9999 || m == 0 || m > 12 {
        return false;
    }
    let leap = y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400));
    let length = match m {
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    (1..=length).contains(&d)
}
fn milestone_eligible(step: &Value) -> bool {
    let node = &step["completion"];
    step["tracking"] != "reminder"
        && node["op"] == "fact"
        && [
            "politics.ideas",
            "politics.advisors",
            "research.completed",
            "focus.completed",
            "projects.completed",
            "doctrines.selected",
        ]
        .contains(&text(&node["key"]))
        && node["test"] == "contains"
        && nonempty(&node["value"], usize::MAX)
        && ["when", "skip_when", "depends_on", "depends_on_started"]
            .iter()
            .all(|k| !truth(&step[k]))
}
fn milestones(values: &Value, by_key: &HashMap<&str, &Value>, key: &str) -> Result<()> {
    let values = values
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 64)
        .ok_or("Invalid production milestones")?;
    let mut seen = HashSet::new();
    for value in values {
        if !keys(
            value,
            &["step", "token", "condition"],
            &["step", "token", "condition"],
        ) {
            return Err("Invalid production milestone".into());
        }
        let anchor = by_key.get(text(&value["step"]));
        let Some(anchor) = anchor.filter(|s| s["key"] != key && milestone_eligible(s)) else {
            return Err("Invalid production milestone reference".into());
        };
        let expected_token = json!([
            anchor["key"],
            anchor["completion"]["key"],
            anchor["completion"]["value"]
        ])
        .to_string();
        if !metadata_equal(&anchor["completion"], &value["condition"])
            || !value["token"].is_string()
            || text(&value["token"]).chars().count() > 2048
            || text(&value["token"]) != expected_token
        {
            return Err("Invalid production milestone reference".into());
        }
        if !seen.insert(text(&value["step"])) {
            return Err("Duplicate production milestone".into());
        }
    }
    Ok(())
}
pub(super) fn validate(guide: &Value) -> Result<()> {
    focus_metadata(guide)?;
    let empty = json!([]);
    let sources = identifiers(guide.get("sources").unwrap_or(&empty), "Fonti", 32)?;
    let phases = identifiers(guide.get("phases").unwrap_or(&empty), "Fasi", 128)?;
    for source in guide.get("sources").unwrap_or(&empty).as_array().unwrap() {
        if !["youtube", "web", "file", "manual"].contains(&text(&source["type"])) {
            return Err("Tipo fonte non valido".into());
        }
        if source.get("url").is_some_and(|v| !valid_url(v)) {
            return Err("URL fonte non valido".into());
        }
    }
    for phase in guide.get("phases").unwrap_or(&empty).as_array().unwrap() {
        if !nonempty(&phase["title"], usize::MAX) {
            return Err("Titolo fase mancante".into());
        }
        if let Some(value) = phase.get("when") {
            condition(value, 0)?
        }
    }
    let rows = rows(guide);
    let by_key = rows
        .iter()
        .map(|s| (text(&s["key"]), *s))
        .collect::<HashMap<_, _>>();
    let reminders = rows
        .iter()
        .filter(|s| s["tracking"] == "reminder")
        .map(|s| text(&s["key"]))
        .collect::<HashSet<_>>();
    let mut pending = HashMap::<&str, usize>::new();
    let mut successors = HashMap::<&str, Vec<&str>>::new();
    for step in &rows {
        let key = text(&step["key"]);
        if let Some(rules) = step.get("focus_criteria") {
            if step["action"]["kind"] != "select_focus" {
                return Err("Focus criteria on a non-focus step".into());
            }
            focus_rules(rules)?
        }
        relative(step)?;
        choice(step)?;
        if step
            .get("tracking")
            .is_some_and(|v| !["action", "reminder"].contains(&text(v)))
        {
            return Err(format!("Modalità monitoraggio non valida: {key}"));
        }
        if step["tracking"] == "reminder"
            && (step["action"]["kind"] != "manual"
                || step.get("completion").is_some()
                || step.get("started_when").is_some())
        {
            return Err(format!(
                "Promemoria incompatibile con azione o completamento: {key}"
            ));
        }
        let mut deps = vec![];
        for field in ["depends_on", "depends_on_started"] {
            let values = step
                .get(field)
                .unwrap_or(&empty)
                .as_array()
                .filter(|a| a.len() <= 64 && a.iter().all(Value::is_string))
                .ok_or_else(|| format!("Dipendenze non valide: {key}"))?;
            deps.extend(values.iter().map(text));
        }
        if deps.len() > 64
            || deps.iter().any(|d| *d == key || !by_key.contains_key(d))
            || deps.iter().collect::<HashSet<_>>().len() != deps.len()
        {
            return Err(format!("Dipendenze non valide: {key}"));
        }
        if deps.iter().any(|d| reminders.contains(d)) {
            return Err(format!("Dipendenza da promemoria non completabile: {key}"));
        }
        pending.insert(key, deps.len());
        for dep in deps {
            successors.entry(dep).or_default().push(key);
        }
        if ["political_power_cost", "resource_costs", "requirements"]
            .iter()
            .any(|k| step.get(k).is_some())
            && (!["manual", "special_project"].contains(&text(&step["action"]["kind"]))
                || step["tracking"] == "reminder")
        {
            return Err(format!("Requisiti di azione incompatibili: {key}"));
        }
        if step
            .get("political_power_cost")
            .is_some_and(|v| !number(v) || v.as_f64().is_none_or(|n| n < 0.))
        {
            return Err(format!("Costo politico non valido: {key}"));
        }
        if let Some(value) = step.get("requirements") {
            condition(value, 0)?
        }
        if step.get("resource_costs").is_some_and(|v| {
            v.as_object().is_none_or(|m| {
                m.len() > 64
                    || m.iter().any(|(k, v)| {
                        k.is_empty()
                            || k.chars().count() > 256
                            || !number(v)
                            || v.as_f64().is_none_or(|n| n < 0.)
                    })
            })
        }) {
            return Err(format!("Costi risorse non validi: {key}"));
        }
        if step
            .get("phase")
            .is_some_and(|v| !v.is_string() || !phases.contains(text(v)))
        {
            return Err(format!("Fase sconosciuta: {key}"));
        }
        if let Some(value) = step.get("completion") {
            condition(value, 0)?
        }
        if let Some(value) = step.get("started_when") {
            if !["manual", "special_project"].contains(&text(&step["action"]["kind"])) {
                return Err(format!(
                    "Condizione di avvio incompatibile con azione: {key}"
                ));
            }
            condition(value, 0)?
        }
        if let Some(p) = step["action"].get("params") {
            if !p.is_object() {
                return Err(format!("Parametri azione non validi: {key}"));
            }
            json_data(p, 0)?;
            if let Some(revision) = p.get("production_revision")
                && (p["category"] != "production"
                    || !keys(revision, &["group", "order"], &["group", "order"])
                    || !nonempty(&revision["group"], 160)
                    || !integer(&revision["order"], 0, 10_000_000))
            {
                return Err(format!("Revisione produzione non valida: {key}"));
            }
            if let Some(value) = p.get("production_section_when") {
                if p.get("production_revision").is_none() {
                    return Err(format!("Fase produzione senza revisione: {key}"));
                }
                condition(value, 0)?
            }
            if let Some(value) = p.get("production_milestones") {
                if p["category"] != "production" {
                    return Err("Milestone outside production".into());
                }
                milestones(value, &by_key, key)?
            }
            let mut productions = vec![p];
            if let Some(targets) = p["production_targets"].as_array() {
                productions.extend(targets)
            }
            for production in productions {
                if production.is_object()
                    && let Some(priority) = production.get("production_priority")
                    && (!["first", "last"].contains(&text(priority))
                        || production["production_category"] != "military"
                        || production["quantity_mode"] == "additional")
                {
                    return Err(format!("Priorità produzione non valida: {key}"));
                }
            }
            if let Some(currency) = p.get("cost_currency") {
                let fact = match text(currency) {
                    "army_experience" => "country.experience.army",
                    "navy_experience" => "country.experience.navy",
                    "air_experience" => "country.experience.air",
                    _ => "",
                };
                if fact.is_empty()
                    || step["action"]["kind"] != "manual"
                    || !nonempty(&p["idea_id"], usize::MAX)
                    || spirit_currency(&p["idea_group"]) != currency.as_str()
                    || step["resource_costs"].get(fact).is_none()
                {
                    return Err(format!("Valuta di esperienza incompatibile: {key}"));
                }
            }
            if let Some(mode) = p.get("focus_mode")
                && (step["action"]["kind"] != "select_focus" || *mode != "continuous")
            {
                return Err(format!("Modalità focus non valida: {key}"));
            }
            if let Some(optional) = p.get("focus_optional")
                && (step["action"]["kind"] != "select_focus"
                    || p["focus_mode"] != "continuous"
                    || !optional.is_boolean())
            {
                return Err(format!("Opzione focus non valida: {key}"));
            }
            if p.get("game_date").is_some_and(|v| !valid_date(v)) {
                return Err(format!("Data della partita non valida: {key}"));
            }
        }
        let evidence = step
            .get("evidence")
            .unwrap_or(&empty)
            .as_array()
            .filter(|a| a.len() <= 32)
            .ok_or_else(|| format!("Evidenze non valide: {key}"))?;
        for item in evidence {
            if !item.is_object()
                || !item["source_id"].is_string()
                || !sources.contains(text(&item["source_id"]))
                || ![
                    "explicit",
                    "visual",
                    "inferred",
                    "game_derived",
                    "unverified",
                ]
                .contains(&text(&item["basis"]))
            {
                return Err(format!("Provenienza non valida: {key}"));
            }
            for field in ["timestamp_seconds", "end_seconds"] {
                if item
                    .get(field)
                    .is_some_and(|v| !number(v) || v.as_f64().is_none_or(|n| n < 0.))
                {
                    return Err(format!("Timestamp non valido: {key}"));
                }
            }
            if item.get("timestamp_seconds").is_some()
                && item.get("end_seconds").is_some()
                && item["end_seconds"].as_f64() < item["timestamp_seconds"].as_f64()
            {
                return Err(format!("Intervallo fonte non valido: {key}"));
            }
        }
    }
    let mut queue = pending
        .iter()
        .filter(|(_, n)| **n == 0)
        .map(|(k, _)| *k)
        .collect::<VecDeque<_>>();
    let mut visited = 0;
    while let Some(key) = queue.pop_front() {
        visited += 1;
        for other in successors.get(key).into_iter().flatten() {
            let n = pending.get_mut(other).unwrap();
            *n -= 1;
            if *n == 0 {
                queue.push_back(other)
            }
        }
    }
    if visited != by_key.len() {
        return Err("Dipendenze della guida cicliche".into());
    }
    Ok(())
}
