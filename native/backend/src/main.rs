//! Native application host. Development cutover is explicit until catalog and
//! HUD adapter parity is complete; it never starts a Python compatibility server.
use std::path::PathBuf;
fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (mut config, mut ready, mut port) = (None, None, 8766u16);
    let (mut resources, mut data, mut print, mut discovery) = (None, None, false, true);
    let mut control_file = None;
    let (mut export, mut output, mut encoding, mut duplicates) = (None, None, 0u32, 1u32);
    let mut overrides = serde_json::json!({});
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--export-script" => export=Some(PathBuf::from(args.next().ok_or("Missing script path")?)),
            "--output" => output=Some(PathBuf::from(args.next().ok_or("Missing output path")?)),
            "--script-encoding" => encoding=match args.next().as_deref(){Some("auto")=>0,Some("utf-8")=>1,Some("windows-1252")=>2,_=>return Err("Invalid script encoding".into())},
            "--json-duplicates" => duplicates=match args.next().as_deref(){Some("key_value_pairs")=>1,Some("group")=>2,Some("preserve")=>3,_=>return Err("Invalid duplicate key mode".into())},
            "--config" => config = Some(PathBuf::from(args.next().ok_or("Missing config path")?)),
            "--control-file" => {
                control_file = Some(PathBuf::from(args.next().ok_or("Missing control file")?))
            }
            "--resources" => {
                resources = Some(PathBuf::from(
                    args.next().ok_or("Missing resources directory")?,
                ))
            }
            "--data-dir" => {
                data = Some(PathBuf::from(args.next().ok_or("Missing data directory")?))
            }
            "--print-config" => print = true,
            "--no-discovery" => discovery = false,
            "--no-browser" => {}
            "--game-dir" | "--docs-dir" | "--country" | "--ui-language" | "--language"
            | "--snapshot" => {
                let key = arg.trim_start_matches("--").replace('-', "_");
                let key = if key == "snapshot" {
                    "snapshot_path".into()
                } else {
                    key
                };
                overrides[key] = serde_json::json!(args.next().ok_or("Missing startup setting")?);
            }
            "--ready-file" => {
                ready = Some(PathBuf::from(args.next().ok_or("Missing ready file path")?))
            }
            "--port" => {
                port = args
                    .next()
                    .ok_or("Missing port")?
                    .parse()
                    .map_err(|_| "Invalid port")?
            }
            _ => return Err(format!("Unknown argument: {arg}")),
        }
    }
    if let Some(path)=export {
        use std::io::{Read, Write};
        let mut bytes=Vec::new();
        std::fs::File::open(path).map_err(|e|e.to_string())?.take(80_000_001).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
        let value:serde_json::Value=serde_json::from_slice(&hoi4companion_parser::process(&bytes,encoding,duplicates)).map_err(|e|e.to_string())?;
        if let Some(error)=value["error"].as_str() { return Err(error.into()); }
        let text=format!("{}\n",value["json"].as_str().ok_or("Missing Jomini JSON result")?);
        if let Some(path)=output {
            if !path.extension().is_some_and(|e|e.eq_ignore_ascii_case("json")) { return Err("Export requires a new .json file".into()); }
            std::fs::OpenOptions::new().create_new(true).write(true).open(path).and_then(|mut f|f.write_all(text.as_bytes())).map_err(|e|e.to_string())?;
        } else { std::io::stdout().write_all(text.as_bytes()).map_err(|e|e.to_string())?; }
        return Ok(());
    }
    if output.is_some() { return Err("--output requires --export-script".into()); }
    let mut config = if let Some(path) = config {
        if resources.is_some() || data.is_some() || !overrides.as_object().unwrap().is_empty() {
            return Err("--config cannot be combined with startup path/settings overrides".into());
        }
        if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 32 * 1024 * 1024 {
            return Err("Configuration exceeds limit".into());
        }
        serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?
    } else {
        let package = hoi4companion_backend::bootstrap::resources(resources.as_deref())?;
        let directory =
            hoi4companion_backend::bootstrap::data_directory(&package, data.as_deref())?;
        if !print {
            std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        }
        hoi4companion_backend::bootstrap::configuration(
            &package, &directory, &overrides, discovery,
        )?
    };
    if print {
        println!(
            "{}",
            serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if let Some(path) = control_file {
        let value = std::fs::read(&path).map_err(|e| e.to_string())?;
        let value: serde_json::Value = serde_json::from_slice(&value).map_err(|e| e.to_string())?;
        let token = value["token"]
            .as_str()
            .filter(|s| s.len() == 64 && s.bytes().all(|c| c.is_ascii_hexdigit()))
            .ok_or("Invalid service control capability")?;
        config["shutdown_token"] = serde_json::json!(token);
    }
    hoi4companion_backend::http::serve(config, port, ready)
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
