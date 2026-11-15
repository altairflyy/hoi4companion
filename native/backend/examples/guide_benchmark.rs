//! Offline diagnostic snapshot; does not attach to the game or open progress.
use hoi4companion_backend::request;
use serde_json::{Value, json};
use std::{env, fs, time::Instant};

struct Catalog(u64);
impl Drop for Catalog {
    fn drop(&mut self) { let _ = request(0, "catalog_release", json!({"handle":self.0})); }
}
fn required<'a>(config: &'a Value, key: &str) -> Result<&'a str, String> {
    config[key].as_str().filter(|v| !v.is_empty()).ok_or_else(|| format!("Missing {key}"))
}
fn read_json(path: &str) -> Result<Value, String> {
    serde_json::from_slice(&fs::read(path).map_err(|e| format!("{path}: {e}"))?).map_err(|e| format!("{path}: {e}"))
}
fn run(config: &Value) -> Result<Value, String> {
    let start = Instant::now();
    let settings = read_json(required(config, "settings_path")?)?;
    if !settings.is_object() { return Err("Settings must be an object".into()); }
    let guides = request(0, "guide_repository", json!({"directories":[required(config,"guides_dir")?],"association_directory":required(config,"association_directory")?}))?;
    let rows = guides["guides"].as_array().ok_or("Invalid guide repository response")?;
    let library = request(0, "reference_library_manifest", json!({"root":required(config,"library_dir")?}))?;
    let handle = request(0, "catalog_open", json!({"settings":settings,"vocabulary_path":required(config,"vocabulary_path")?}))?.as_u64().ok_or("Invalid catalog handle")?;
    let catalog = Catalog(handle);
    let sources = request(0, "catalog_sources", json!({"handle":catalog.0}))?;
    let entities = request(0, "catalog_entities", json!({"handle":catalog.0}))?;
    let mut result = Vec::with_capacity(rows.len());
    let mut errors = 0usize;
    for guide in rows {
        match request(0, "catalog_runtime_guide", json!({"handle":catalog.0,"guide":guide,"library":library})) {
            Ok(value) if value["definition"].is_object() => result.push(json!({"id":guide["id"],"definition":value["definition"]})),
            Ok(_) => { errors += 1; result.push(json!({"id":guide["id"],"error":"Invalid runtime definition response"})); },
            Err(error) => { errors += 1; result.push(json!({"id":guide["id"],"error":error})); },
        }
    }
    Ok(json!({"schema_version":1,"selected_guide_count":rows.len(),"error_count":errors,"elapsed_ms":start.elapsed().as_millis(),"catalog":entities,"catalog_demo":sources["demo"],"guides":result}))
}
fn main() {
    let result = (|| -> Result<(), String> {
        let args: Vec<_> = env::args().collect();
        if args.len() != 2 { return Err("Usage: guide_benchmark <config.json>".into()); }
        let config = read_json(&args[1])?;
        let output = run(&config)?;
        let data = serde_json::to_vec(&output).map_err(|e| e.to_string())?;
        if let Some(path) = config.get("output") {
            let path = path.as_str().filter(|v| !v.is_empty()).ok_or("Invalid output path")?;
            fs::write(path, data).map_err(|e| format!("{path}: {e}"))?;
        } else { println!("{}", String::from_utf8(data).map_err(|e| e.to_string())?); }
        Ok(())
    })();
    if let Err(error) = result { eprintln!("{error}"); std::process::exit(1); }
}
