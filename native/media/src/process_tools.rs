//! Bounded owned utility invocation, shared by media and source jobs. No shell,
//! process attachment or resident interpreter; only task-created outputs removed.
use serde_json::{Value, json};
use std::{
    io::Read,
    path::{Path, PathBuf},
};
pub struct Output {
    pub stdout: String,
    pub stderr: String,
    pub status: Value,
}
struct Logs {
    stdout: PathBuf,
    stderr: PathBuf,
}
impl Drop for Logs {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.stdout);
        let _ = std::fs::remove_file(&self.stderr);
    }
}
pub fn run(argv: Vec<String>, millis: u64, limit: u64) -> Result<Output, String> {
    let key = crate::progress_store::new_id()?;
    let root = std::env::temp_dir();
    let logs = Logs {
        stdout: root.join(format!("hoi4-{key}-stdout.log")),
        stderr: root.join(format!("hoi4-{key}-stderr.log")),
    };
    // Reserve names exclusively before passing them to the append-only supervisor.
    for path in [&logs.stdout, &logs.stderr] {
        std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .map_err(|e| e.to_string())?;
    }
    let tool = hoi4companion_tools::OwnedTool::spawn(
        json!({"schema_version":1,"argv":argv,"stdout":logs.stdout,"stderr":logs.stderr,"max_millis":millis,"max_output_bytes":limit}),
    )?;
    let status = loop {
        let value = tool.wait(1000)?;
        if value["running"] != true {
            break value;
        }
    };
    drop(tool);
    let read = |path: &Path| -> Result<String, String> {
        let mut data = vec![];
        std::fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take(limit + 1)
            .read_to_end(&mut data)
            .map_err(|e| e.to_string())?;
        if data.len() as u64 > limit {
            return Err("Strumento oltre il limite di output".into());
        }
        Ok(String::from_utf8_lossy(&data).into_owned())
    };
    Ok(Output {
        stdout: read(&logs.stdout)?,
        stderr: read(&logs.stderr)?,
        status,
    })
}
pub fn checked(argv: Vec<String>, millis: u64, limit: u64) -> Result<String, String> {
    let result = run(argv, millis, limit)?;
    if result.status["reason"] == "timeout" {
        return Err("Strumento oltre il limite di tempo".into());
    }
    if result.status["reason"] == "output_limit" {
        return Err("Strumento oltre il limite di output".into());
    }
    if result.status["returncode"] != 0 {
        return Err(format!(
            "Strumento: {}",
            result
                .stderr
                .chars()
                .rev()
                .take(1200)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<String>()
        ));
    }
    Ok(result.stdout + &result.stderr)
}
pub fn executable(configured: Option<&str>, name: &str, package: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = configured {
        let path = PathBuf::from(path);
        return path.is_file().then_some(path);
    }
    if let Some(package) = package {
        let candidate = package.join("_native/tools").join(format!("{name}.exe"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .map(|p| p.join(format!("{name}.exe")))
        .find(|p| p.is_file())
}
