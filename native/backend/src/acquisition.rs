//! Complete acquisition through a retained qualified Game; no host callback.
use crate::{
    country,
    game_process::Game,
    live,
    live_process::{Memory, profile_value},
    watch::{self, Witnesses},
};
use serde_json::{Value, json};
type Result<T> = std::result::Result<T, String>;
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
    if compare(game, &watch::sample(witnesses)?)? {
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
pub fn collect(game: &Game, neighbors: &Value, event_cache: &Value) -> Result<Sample> {
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
            watch: true,
        },
    )?;
    let world = packet["world"]
        .as_u64()
        .ok_or("World identity unavailable")?;
    let country = packet["country_pointer"]
        .as_u64()
        .ok_or("Country identity unavailable")?;
    let tag = packet["native_tag"]
        .as_i64()
        .ok_or("Country tag unavailable")? as i32;
    let mut state = packet["state"].clone();
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
    section_attributes["sha256"] = metadata["sha256"].clone();
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
    let mut witnesses = Witnesses::new();
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
        let mut current = Memory::new(game, 2 * 1024 * 1024)?;
        let mut attrs = section_attributes.clone();
        if section == "fortifications" {
            attrs["province_neighbors"] = neighbors.clone()
        }
        if section == "events" {
            attrs["event_cache"] = event_cache.clone()
        }
        let read = (|| {
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
                    attributes: attrs,
                },
            )?;
            verify(game, &current.witnesses)?;
            let mut section_plan = Witnesses::new();
            watch_rows(&output, &mut section_plan)?;
            if section == "fortifications" {
                let mut combined = plan.clone();
                combined.extend(section_plan.clone());
                watch::compile(&combined)?;
            }
            plan.extend(section_plan);
            witnesses.extend(current.witnesses);
            if let Some(cache) = output.get("event_cache") {
                next_cache = cache.clone()
            }
            Ok::<_, String>(output["value"].clone())
        })();
        overview[section] = match read {
            Ok(value) => value,
            Err(reason) => json!({"status":"unavailable","reason":reason}),
        };
    }
    // Cross-domain coherence is verified after per-section validation too.
    for (address, size) in witnesses.keys() {
        if game.read(*address, *size)? != witnesses[&(*address, *size)] {
            return Err("Riepilogo cambiato durante il campione".into());
        }
    }
    verify(game, &memory.witnesses)?;
    if game.identity()? != game.created {
        return Err("Sessione di processo cambiata".into());
    }
    state["country_overview"] = overview;
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
