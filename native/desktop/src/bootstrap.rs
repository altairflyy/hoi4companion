use serde_json::Value;
use std::path::{Path, PathBuf};
type Result<T> = std::result::Result<T, String>;
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
pub fn hud_status(data: &Path) -> Result<Value> {
    serde_json::from_slice(
        &std::fs::read(data.join("overlay-status.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
pub fn configuration(package: &Path, data: &Path, _: &Value, _: bool) -> Result<Value> {
    let exe = std::env::var_os("HOI4_BACKEND_EXE")
        .map(PathBuf::from)
        .unwrap_or_else(|| package.join("_native/hoi4-backend.exe"));
    let out = std::process::Command::new(exe)
        .args(["--resources"])
        .arg(package)
        .arg("--data-dir")
        .arg(data)
        .args(["--print-config", "--no-discovery"])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())
}
