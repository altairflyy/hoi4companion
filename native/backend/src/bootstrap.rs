//! Native startup configuration. Persisted user paths win over discovery;
//! resource lookup never imports a developer's guide, mod or campaign data.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};
type Result<T> = std::result::Result<T, String>;
pub fn hud_status(data: &Path) -> Result<Value> {
    crate::guide_repository::json_file(&data.join("overlay-status.json"), 262144)
}
fn display(path: &Path) -> String {
    crate::installation::display(path)
}
pub fn resources(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return valid_resources(path);
    }
    if let Some(path) = std::env::var_os("HOI4COMPANION_RESOURCES") {
        return valid_resources(Path::new(&path));
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let directory = exe.parent().ok_or("Missing executable directory")?;
    for path in [
        directory.join("resources"),
        directory.to_path_buf(),
        directory.parent().unwrap_or(directory).to_path_buf(),
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join("src/hoi4companion"),
    ] {
        if let Ok(path) = valid_resources(&path) {
            return Ok(path);
        }
    }
    Err(
        "Application resources missing: extract the complete Windows ZIP or pass --resources"
            .into(),
    )
}
fn valid_resources(path: &Path) -> Result<PathBuf> {
    let path = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
    if !path.join("web/index.html").is_file()
        || !path.join("examples/demo_focus.txt").is_file()
        || !path.join("guide_language.json").is_file()
    {
        return Err("Incomplete application resource directory".into());
    }
    Ok(path)
}
pub fn data_directory(package: &Path, explicit: Option<&Path>) -> Result<PathBuf> {
    let path = if let Some(path) = explicit {
        path.to_path_buf()
    } else if let Some(path) = std::env::var_os("HOI4COMPANION_DATA_DIR") {
        PathBuf::from(path)
    } else if package.file_name().is_some_and(|n| n == "hoi4companion")
        && package
            .parent()
            .is_some_and(|p| p.file_name().is_some_and(|n| n == "src"))
    {
        package
            .parent()
            .and_then(Path::parent)
            .ok_or("Invalid checkout resource directory")?
            .join(".local")
    } else {
        PathBuf::from(
            std::env::var_os("LOCALAPPDATA")
                .ok_or("Windows local application data directory unavailable")?,
        )
        .join("HOI4Companion")
    };
    Ok(if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(path)
    })
}
#[cfg(windows)]
fn registry(root: windows::Win32::System::Registry::HKEY, key: &str, name: &str) -> Option<String> {
    use windows::{
        Win32::System::Registry::{RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ, RegGetValueW},
        core::PCWSTR,
    };
    let key: Vec<_> = key.encode_utf16().chain(Some(0)).collect();
    let name: Vec<_> = name.encode_utf16().chain(Some(0)).collect();
    let mut buffer = vec![0u16; 32768];
    let mut length = (buffer.len() * 2) as u32;
    let result = unsafe {
        RegGetValueW(
            root,
            PCWSTR(key.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut length),
        )
    };
    if result.0 != 0
        || length < 2
        || length as usize > buffer.len() * 2
        || !length.is_multiple_of(2)
    {
        return None;
    }
    let end = buffer
        .iter()
        .position(|n| *n == 0)
        .unwrap_or(length as usize / 2);
    String::from_utf16(&buffer[..end])
        .ok()
        .filter(|v| !v.is_empty())
}
fn documents() -> PathBuf {
    #[cfg(windows)]
    if let Some(path) = registry(
        windows::Win32::System::Registry::HKEY_CURRENT_USER,
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders",
        "Personal",
    ) {
        return PathBuf::from(path).join("Paradox Interactive/Hearts of Iron IV");
    }
    PathBuf::from(
        std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .unwrap_or_default(),
    )
    .join("Documents/Paradox Interactive/Hearts of Iron IV")
}
fn detect_game() -> Option<PathBuf> {
    let mut steam = vec![];
    #[cfg(windows)]
    {
        use windows::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        for (root, key, name) in [
            (HKEY_CURRENT_USER, r"Software\Valve\Steam", "SteamPath"),
            (HKEY_LOCAL_MACHINE, r"SOFTWARE\Valve\Steam", "InstallPath"),
            (
                HKEY_LOCAL_MACHINE,
                r"SOFTWARE\WOW6432Node\Valve\Steam",
                "InstallPath",
            ),
        ] {
            if let Some(path) = registry(root, key, name) {
                steam.push(PathBuf::from(path));
            }
        }
    }
    for key in ["PROGRAMFILES(X86)", "PROGRAMFILES"] {
        if let Some(path) = std::env::var_os(key) {
            steam.push(PathBuf::from(path).join("Steam"));
        }
    }
    let mut libraries = steam.clone();
    for root in steam {
        if let Ok(document) =
            crate::script_document::read(&root.join("steamapps/libraryfolders.vdf"))
        {
            for (_, folders) in crate::script_document::entries(&document) {
                for (_, library) in crate::script_document::entries(folders) {
                    let path = if let Some(path) = library.as_str() {
                        path
                    } else {
                        crate::script_document::scalar(library, "path")
                    };
                    if !path.is_empty() {
                        libraries.push(PathBuf::from(path));
                    }
                }
            }
        }
    }
    let mut seen = HashSet::new();
    libraries
        .into_iter()
        .filter(|p| seen.insert(p.clone()))
        .map(|root| root.join("steamapps/common/Hearts of Iron IV"))
        .find(|game| game.join("hoi4.exe").is_file() && game.join("common/national_focus").is_dir())
}
fn tool(package: &Path, data: &Path, name: &str) -> Option<String> {
    let native = package.join("_native/tools").join(format!("{name}.exe"));
    if native.is_file() {
        return Some(display(&native));
    }
    if name == "ffmpeg" {
        // Source-checkout dependency, used as an executable only. No Python
        // import/interpreter is launched to locate or run the utility.
        let binaries = data.join("media-tools/imageio_ffmpeg/binaries");
        if let Ok(entries) = std::fs::read_dir(binaries) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("ffmpeg"))
                    && path.extension().is_some_and(|e| e == "exe")
                {
                    return Some(display(&path));
                }
            }
        }
    }
    None
}
pub fn configuration(
    package: &Path,
    data: &Path,
    overrides: &Value,
    discovery: bool,
) -> Result<Value> {
    let settings_path = data.join("settings.json");
    let mut settings = json!({"game_dir":"","docs_dir":"","tree_id":"","snapshot_path":"","country":"AUS","language":"english","ui_language":"english","mods":"auto"});
    if settings_path.is_file() {
        let stored = crate::guide_repository::json_file(&settings_path, 2_000_000)?;
        for (key, value) in stored.as_object().ok_or("Invalid saved settings")? {
            if settings.get(key).is_some() {
                settings[key] = value.clone();
            }
        }
    }
    for (key, value) in overrides.as_object().ok_or("Invalid startup settings")? {
        if settings.get(key).is_none() {
            return Err(format!("Unknown startup setting: {key}"));
        }
        settings[key] = value.clone();
    }
    if discovery
        && settings["game_dir"] == ""
        && let Some(game) = detect_game()
    {
        settings["game_dir"] = json!(display(&game));
    }
    if settings["docs_dir"] == "" {
        settings["docs_dir"] = json!(display(&documents()));
    }
    let mut config = json!({"path":display(&data.join("guide-progress.sqlite3")),"selection_path":display(&data.join("active-guide.json")),"web_root":display(&package.join("web")),"reference_library":display(&data.join("reference-library")),"guide_directory":display(&data.join("guides")),"guide_directories":[display(&package.join("guides")),display(&data.join("guides"))],"bundled_guides":display(&package.join("guides")),"association_directory":display(&data.join("guide-associations")),"guide_frames":display(&data.join("guide-frames")),"guide_videos":display(&data.join("guide-videos")),"source_error_log":display(&data.join("logs/source-errors.json")),"rules_root":display(&package.join("rules/cwtools")),"cwtools_host":display(&package.join("_native/cwtools")),"parser_library":display(&package.join("_native/hoi4companion_parser.dll")),"catalog":{"settings":settings,"settings_path":display(&settings_path),"demo_file":display(&package.join("examples/demo_focus.txt")),"demo_labels":display(&package.join("examples/demo_labels.json")),"cache":display(&data.join("sprite-cache")),"vocabulary_path":display(&package.join("guide_language.json"))}});
    if let Some(ffmpeg) = tool(package, data, "ffmpeg") {
        config["ffmpeg"] = json!(ffmpeg);
    }
    let exe = PathBuf::from(
        settings["game_dir"]
            .as_str()
            .ok_or("Invalid game installation setting")?,
    )
    .join("hoi4.exe");
    if exe.is_file() {
        let manifest = include_bytes!("../../../src/hoi4companion/live-profiles-v1.json");
        config["provider"] = json!({"path":display(&exe),"manifest_sha256":format!("{:x}",Sha256::digest(manifest)),"audit_seconds":5});
    }
    Ok(config)
}
