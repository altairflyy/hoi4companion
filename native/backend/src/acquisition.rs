//! Complete acquisition through a retained qualified Game; no host callback.
use crate::{
    country,
    game_process::Game,
    live,
    live_process::{Memory, profile_value},
    watch::{self, Witnesses},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
type Result<T> = std::result::Result<T, String>;
fn publish_focus_tree(state: &mut Value) -> Result<()> {
    // The qualified core decoder returns the actual tree identity. Publish the
    // established consumer field too; never substitute settings or guide scope.
    let tree = state["native_focus_tree_id"].as_str().filter(|s| {
        !s.is_empty() && s.len() <= 160
            && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
    }).ok_or("Qualified focus tree identity missing")?;
    state["focus_tree_id"] = json!(tree);
    Ok(())
}
fn separate_clock(witnesses: &mut Witnesses, world: u64) {
    // The provider observes the clock separately at publication and on every
    // cached read. Requiring the hour to stay unchanged during cold discovery
    // made running games permanently stale. Actual gameplay/identity witnesses
    // remain mandatory; only this separately published clock is excluded.
    witnesses.remove(&(world + 0x468, 4));
}
pub struct Sample {
    pub value: Value,
    pub watch: Witnesses,
    pub world: u64,
    pub event_cache: Value,
}
pub fn compare(game: &Game, packet: &watch::Packet) -> Result<bool> {
    let code = unsafe {
        hoi4companion_observer::hoi4_observer_compare(
            game.raw_handle().0,
            packet.bytes.as_ptr(),
            packet.bytes.len(),
            2,
        )
    };
    match code {
        0 => Ok(false),
        1 => Ok(true),
        -1 => Err("Native observer: invalid packet or bounds".into()),
        -2 => Err("Native observer: read failed or partial".into()),
        _ => Err("Native observer: internal failure".into()),
    }
}
fn verify(game: &Game, witnesses: &Witnesses) -> Result<()> {
    if compare(game, &watch::verification(witnesses)?)? {
        Err("Campione incoerente: riprovare".into())
    } else {
        Ok(())
    }
}
fn from_hex(text: &str) -> Result<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid native evidence".into());
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}
fn watch_rows(output: &Value, plan: &mut Witnesses) -> Result<()> {
    for row in output["watch"]
        .as_array()
        .ok_or("Missing native watch rows")?
    {
        let address = row[0].as_u64().ok_or("Invalid native evidence address")?;
        let size = row[1].as_u64().ok_or("Invalid native evidence size")? as usize;
        let bytes = from_hex(row[2].as_str().ok_or("Invalid native evidence bytes")?)?;
        if bytes.len() != size {
            return Err("Partial native evidence".into());
        }
        plan.insert((address, size), bytes);
    }
    Ok(())
}
pub fn collect(game: &Game, neighbors: &Value, event_cache: &Value, shared_focuses: &Value, building_scopes: &Value) -> Result<Sample> {
    let metadata = game.profile.metadata();
    let attributes = profile_value(&metadata["selected"]);
    let base = game.base;
    let world_rva = attributes["world_rva"]
        .as_u64()
        .ok_or("World profile unavailable")?;
    let mut memory = Memory::new(game, 2 * 1024 * 1024)?;
    let packet = live::collect(
        &mut memory,
        live::Profile {
            schema_version: 1,
            base,
            world_rva,
            definition_vtable: attributes["technology_definition_vtable"]
                .as_u64()
                .ok_or("Technology profile unavailable")?,
            joint_focus_vtable: attributes["joint_focus_vtable"].as_u64(),
            shared_focuses: shared_focuses.clone(),
            watch: true,
        },
    )?;
    let world = packet["world"]
        .as_u64()
        .ok_or("World identity unavailable")?;
    separate_clock(&mut memory.witnesses, world);
    let country = packet["country_pointer"]
        .as_u64()
        .ok_or("Country identity unavailable")?;
    let tag = packet["native_tag"]
        .as_i64()
        .ok_or("Country tag unavailable")? as i32;
    // Cold GUI discovery is provisional and may take longer than a simulation
    // tick. Resolve its candidates before starting the published cross-domain
    // sample, then read the core again. The cache is checked against the live
    // root/registry and every selected window is revalidated by country::collect.
    // Recursion is bounded to one warm-up: the next cache is a non-null array.
    if event_cache.is_null()
        && attributes["country_sections"]
            .as_array()
            .is_some_and(|rows| rows.iter().any(|v| v == "events"))
    {
        let mut discovery = Memory::new(game, 2 * 1024 * 1024)?;
        let mut attrs = attributes.clone();
        attrs["sha256"] = metadata["sha256"].clone();
        attrs["image_base"] = metadata["image_base"].clone();
        if let Ok(output) = country::collect(
            &mut discovery,
            country::Request {
                schema_version: 1,
                base,
                world,
                country,
                tag,
                section: "events".into(),
                qualify: true,
                attributes: attrs,
            },
        ) && let Some(cache) = output.get("event_cache").filter(|v| v.is_array())
        {
            return collect(game, neighbors, cache, shared_focuses, building_scopes);
        }
    }
    let mut state = packet["state"].clone();
    if let Some(compatibility) = attributes.get("compatibility") {
        state["reader_compatibility"] = compatibility.clone();
    }
    let mut plan = Witnesses::new();
    for row in packet["ranges"]
        .as_array()
        .ok_or("Core watch ranges unavailable")?
    {
        let address = row[0].as_u64().ok_or("Invalid core watch address")?;
        let size = row[1].as_u64().ok_or("Invalid core watch size")? as usize;
        plan.insert(
            (address, size),
            memory
                .witnesses
                .get(&(address, size))
                .ok_or("Missing core witness")?
                .clone(),
        );
    }
    let mut section_attributes = attributes.clone();
    section_attributes["building_scopes"] = building_scopes.clone();
    section_attributes["sha256"] = metadata["sha256"].clone();
    section_attributes["image_base"] = metadata["image_base"].clone();
    section_attributes["executable_bytes"] = json!(true);
    let old = memory
        .witnesses
        .keys()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let campaign = country::collect(
        &mut memory,
        country::Request {
            schema_version: 1,
            base,
            world,
            country,
            tag,
            section: "campaign".into(),
            attributes: section_attributes.clone(),
            qualify: false,
        },
    )?;
    for (key, bytes) in &memory.witnesses {
        if !old.contains(key) {
            plan.insert(*key, bytes.clone());
        }
    }
    state["campaign_identity"] = campaign["value"].clone();
    let mut overview = json!({"country":state["country"]});
    let core_plan = plan.clone();
    struct Domain {
        value: Value,
        watch: Witnesses,
        witnesses: Witnesses,
        cache: Option<Value>,
        attrs: Value,
    }
    let read_domain = |section: &str, attrs: Value| -> Result<Domain> {
        let mut current = Memory::new(game, 2 * 1024 * 1024)?;
        let output = country::collect(
            &mut current,
            country::Request {
                schema_version: 1,
                base,
                world,
                country,
                tag,
                section: section.into(),
                qualify: true,
                attributes: attrs.clone(),
            },
        )?;
        verify(game, &current.witnesses)?;
        let mut watch = Witnesses::new();
        watch_rows(&output, &mut watch)?;
        let mut value = output["value"].clone();
        if value.is_object() {
            value["observed_native_tick"] = json!(i32::from_le_bytes(
                game.read(world + 0x468, 4)?.try_into().unwrap()
            ));
            value["observation_scope"] = json!("verified_domain");
        }
        Ok(Domain {
            value,
            watch,
            witnesses: current.witnesses,
            cache: output.get("event_cache").cloned(),
            attrs,
        })
    };
    let mut domains = BTreeMap::<String, Domain>::new();
    let mut next_cache = event_cache.clone();
    for section in [
        "divisions",
        "manpower",
        "states",
        "production",
        "equipment",
        "resources",
        "construction",
        "intelligence",
        "politics",
        "events",
        "decisions",
        "projects",
        "experience",
        "diplomacy",
        "metrics",
        "doctrines",
        "fortifications",
    ] {
        if attributes["country_sections"]
            .as_array()
            .is_some_and(|rows| !rows.iter().any(|v| v == section))
        {
            overview[section] = json!({"status":"unavailable","reason":"Sezione non ancora verificata per questa versione del gioco."});
            continue;
        }
        let mut attrs = section_attributes.clone();
        if section == "fortifications" {
            attrs["province_neighbors"] = neighbors.clone()
        }
        if section == "events" {
            attrs["event_cache"] = event_cache.clone()
        }
        let read = (|| {
            let domain = read_domain(section, attrs)?;
            if section == "fortifications" {
                let mut combined = plan.clone();
                combined.extend(domain.watch.clone());
                watch::compile(&combined)?;
            }
            plan.extend(domain.watch.clone());
            if let Some(cache) = &domain.cache {
                next_cache = cache.clone()
            }
            let value = domain.value.clone();
            domains.insert(section.into(), domain);
            Ok::<_, String>(value)
        })();
        overview[section] = match read {
            Ok(value) => value,
            Err(reason) => json!({"status":"unavailable","reason":reason}),
        };
    }
    // Refresh a domain that changed while another domain was being decoded.
    // A second change withholds that domain rather than poisoning focus/research
    // or publishing stale values. Identity/core witnesses are always mandatory.
    let mut discard = vec![];
    for (section, domain) in &mut domains {
        if verify(game, &domain.witnesses).is_err() {
            match read_domain(section, domain.attrs.clone()) {
                Ok(current) => {
                    overview[section] = current.value.clone();
                    *domain = current;
                }
                Err(reason) => {
                    overview[section] = json!({"status":"unavailable","reason":reason});
                    discard.push(section.clone());
                }
            }
        }
    }
    for section in discard {
        domains.remove(&section);
    }
    // These are independently witnessed domain observations, not a claim that
    // all game memory was frozen in one frame. Each value carries its observed
    // game tick. Requiring all counters to stay unchanged across every domain
    // permanently rejected valid running games. Process/world/country/campaign
    // identity and the core observations remain verified across the full read.
    plan = core_plan;
    for domain in domains.values() {
        plan.extend(domain.watch.clone());
        if let Some(cache) = &domain.cache {
            next_cache = cache.clone();
        }
    }
    verify(game, &memory.witnesses)?;
    if game.identity()? != game.created {
        return Err("Sessione di processo cambiata".into());
    }
    publish_focus_tree(&mut state)?;
    state["country_overview"] = overview;
    state["country_observation_scope"] = json!("independently_verified_domains");
    if state["reader_compatibility"].is_object() {
        state["reader_compatibility"]["live_validated"] = json!(true);
    }
    state["session_id"] = json!(format!(
        "{}:{}:{world:x}:{country:x}",
        game.pid, game.created
    ));
    state["pid"] = json!(game.pid);
    state["executable_sha256"] = metadata["sha256"].clone();
    state["limitations"] = json!([
        "Reader sperimentale: valori da confrontare con l’interfaccia del gioco.",
        "Progresso percentuale e disponibilità dinamica non ancora qualificati.",
        "Campione coerente per rilettura, non una transazione atomica del motore."
    ]);
    Ok(Sample {
        value: state,
        watch: plan,
        world,
        event_cache: next_cache,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn qualified_tree_reaches_runtime_without_borrowing_guide_or_settings_identity() {
        let guide = json!({"id":"tree-publication","applies_to":{"country":"HUN","tree_id":"wuw_hungarian_focus"},"tracks":{"focus":[]}});
        for (country, tree, expected) in [
            ("HUN", "wuw_hungarian_focus", "live"),
            ("LUX", "wuw_hungarian_focus", "wrong_country"),
            ("HUN", "other_hungarian_tree", "wrong_tree"),
        ] {
            let mut sample = json!({"country":country,"native_focus_tree_id":tree});
            publish_focus_tree(&mut sample).unwrap();
            let state = json!({"usable":true,"sample":sample});
            assert_eq!(crate::guide_runtime::evaluate(&guide, &state, &Value::Null).unwrap()["mode"], expected);
        }
        assert!(publish_focus_tree(&mut json!({"focus_tree_id":"configured_tree"})).is_err());
        assert!(publish_focus_tree(&mut json!({"native_focus_tree_id":"bad tree"})).is_err());
    }
    #[test]
    fn only_the_independently_published_clock_is_not_a_stable_gameplay_witness() {
        let world = 0x200000000;
        let mut data = Witnesses::from([
            ((world + 0x468, 4), vec![1; 4]),
            ((world + 0x520, 4), vec![2; 4]),
            ((world, 8), vec![3; 8]),
        ]);
        separate_clock(&mut data, world);
        assert_eq!(data.len(), 2);
        assert_eq!(data[&(world + 0x520, 4)], vec![2; 4]);
        assert_eq!(data[&(world, 8)], vec![3; 8]);
    }
}
