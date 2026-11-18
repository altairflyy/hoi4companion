//! Explicitly requested saved-preset installation, never campaign mutation.
use serde_json::{Value, json};
use std::{fs, io::Write, path::Path, sync::Mutex};
type Result<T> = std::result::Result<T, String>;
static INSTALL: Mutex<()> = Mutex::new(());
fn save(root: &Path, value: &Value) -> Result<()> {
    crate::reference_library::atomic_write(&root.join("mio-install.json"), &serde_json::to_vec(value).map_err(|e|e.to_string())?)
}
pub fn status(root: &Path) -> Value {
    let mut v=crate::guide_repository::json_file(&root.join("mio-install.json"),2_000_000)
        .unwrap_or_else(|_|json!({"status":"not_requested","restart_required":false}));
    if let Some(o)=v.as_object_mut(){o.remove("rows");o.remove("docs");}
    v
}
pub fn request(root: &Path, docs: &str, rows: &[Value]) -> Result<Value> {
    let _guard=INSTALL.lock().map_err(|e|e.to_string())?;
    if rows.is_empty(){return Err("No MIO presets to install".into())}
    let docs=Path::new(docs).canonicalize().map_err(|e|e.to_string())?;
    // Validate the requested payload before persisting any work.
    crate::advanced_references::export_mios(rows,None)?;
    save(root,&json!({"status":"pending","restart_required":false,"docs":docs,"rows":rows}))?;
    sync_locked(root,game_running)
}
pub fn sync(root: &Path) -> Result<Value> {
    let _guard=INSTALL.lock().map_err(|e|e.to_string())?;
    sync_locked(root,game_running)
}
#[cfg(not(windows))]
fn game_running() -> Result<bool> {Err("Automatic MIO installation requires Windows".into())}
#[cfg(windows)]
fn game_running() -> Result<bool> {
    use windows::Win32::{Foundation::{CloseHandle,ERROR_NO_MORE_FILES},System::Diagnostics::ToolHelp::*};
    let snapshot=unsafe{CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS,0)}.map_err(|e|e.to_string())?;
    struct Snapshot(windows::Win32::Foundation::HANDLE);
    impl Drop for Snapshot{fn drop(&mut self){unsafe{let _=CloseHandle(self.0);}}}
    let snapshot=Snapshot(snapshot);
    let mut row=PROCESSENTRY32W{dwSize:std::mem::size_of::<PROCESSENTRY32W>() as u32,..Default::default()};
    let mut next=unsafe{Process32FirstW(snapshot.0,&mut row)};
    loop {
        if let Err(error)=next {return if error.code()==ERROR_NO_MORE_FILES.to_hresult(){Ok(false)}else{Err(error.to_string())}}
        let len=row.szExeFile.iter().position(|c|*c==0).unwrap_or(row.szExeFile.len());
        if String::from_utf16_lossy(&row.szExeFile[..len]).eq_ignore_ascii_case("hoi4.exe"){return Ok(true)}
        next=unsafe{Process32NextW(snapshot.0,&mut row)};
    }
}
fn sync_locked(root: &Path, running: impl Fn()->Result<bool>) -> Result<Value> {
    let file=root.join("mio-install.json");
    if !file.exists(){return Ok(status(root))}
    let mut job=crate::guide_repository::json_file(&file,2_000_000)?;
    if !["pending","waiting_for_game_exit"].contains(&job["status"].as_str().unwrap_or("")){return Ok(status(root))}
    let result=(||->Result<Value>{
        if running()?{job["status"]=json!("waiting_for_game_exit");job["restart_required"]=json!(true);return Ok(job.clone())}
        let docs=Path::new(job["docs"].as_str().ok_or("Missing HOI4 documents directory")?).canonicalize().map_err(|e|e.to_string())?;
        let unresolved=docs.join("session_persistence");
        if !unresolved.exists(){fs::create_dir(&unresolved).map_err(|e|e.to_string())?;}
        let directory=unresolved.canonicalize().map_err(|e|e.to_string())?;
        if !directory.starts_with(&docs){return Err("MIO destination outside HOI4 documents".into())}
        let path=directory.join("mio_queues.txt");
        if path.exists()&&!path.canonicalize().map_err(|e|e.to_string())?.starts_with(&directory){return Err("MIO file outside session directory".into())}
        let old=if path.exists(){
            if fs::metadata(&path).map_err(|e|e.to_string())?.len()>2_000_000{return Err("MIO file exceeds limit".into())}
            Some(fs::read(&path).map_err(|e|e.to_string())?)
        }else{None};
        let export=crate::advanced_references::export_mios(job["rows"].as_array().ok_or("Missing MIO presets")?,old.as_deref())?;
        if running()?{job["status"]=json!("waiting_for_game_exit");job["restart_required"]=json!(true);return Ok(job.clone())}
        let mut backup=Value::Null;
        if export["added"].as_u64().unwrap_or(0)>0 {
            if let Some(raw)=old.as_ref(){
                let b=directory.join(format!("mio_queues.companion-{}.bak",crate::advanced_references::id(raw)));
                if b.exists(){if fs::read(&b).map_err(|e|e.to_string())?!=*raw{return Err("MIO backup conflict".into())}}
                else{let mut f=fs::OpenOptions::new().write(true).create_new(true).open(&b).map_err(|e|e.to_string())?;f.write_all(raw).and_then(|_|f.sync_all()).map_err(|e|e.to_string())?;}
                backup=json!(b);
            }
            let current=if path.exists(){Some(fs::read(&path).map_err(|e|e.to_string())?)}else{None};
            if current!=old{return Err("MIO file changed before installation; retry".into())}
            if running()?{job["status"]=json!("waiting_for_game_exit");job["restart_required"]=json!(true);return Ok(job.clone())}
            crate::reference_library::atomic_write(&path,export["text"].as_str().ok_or("Invalid MIO export")?.as_bytes())?;
        }
        Ok(json!({"status":"installed","restart_required":false,"load_on_next_launch":true,"count":export["count"],"added":export["added"],"existing":export["existing"],"renamed":export["renamed"],"path":path,"backup":backup,"campaign_verified":false}))
    })();
    let value=result.unwrap_or_else(|error|json!({"status":"error","restart_required":false,"error":error}));
    save(root,&value)?;
    Ok(status(root))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deferred_install_preserves_backups_and_is_idempotent(){
        let root=std::env::temp_dir().join(format!("hoi4-mio-install-{}",crate::progress_store::new_id().unwrap()));
        fs::create_dir(&root).unwrap();
        struct Cleanup(std::path::PathBuf);impl Drop for Cleanup{fn drop(&mut self){let _=fs::remove_dir_all(&self.0);}}
        let _cleanup=Cleanup(root.clone());let docs=root.join("docs");let library=root.join("library");fs::create_dir(&docs).unwrap();fs::create_dir(&library).unwrap();fs::create_dir(docs.join("session_persistence")).unwrap();
        let path=docs.join("session_persistence/mio_queues.txt");let original=br#"version=0 X={ {name="User" queued_trait={ {trait="first"} } } }"#;
        fs::write(&path,original).unwrap();let rows=crate::advanced_references::mio_queues(br#"version=0 X={ {name="Guide" queued_trait={ {trait="second"} } } }"#).unwrap();
        save(&library,&json!({"status":"pending","docs":docs,"rows":rows})).unwrap();
        assert_eq!(sync_locked(&library,||Ok(true)).unwrap()["status"],"waiting_for_game_exit");assert_eq!(fs::read(&path).unwrap(),original);
        let result=sync_locked(&library,||Ok(false)).unwrap();assert_eq!(result["status"],"installed");assert_eq!(result["count"],2);assert_eq!(fs::read(result["backup"].as_str().unwrap()).unwrap(),original);
        let bytes=fs::read(&path).unwrap();assert_eq!(sync_locked(&library,||Ok(false)).unwrap(),result);assert_eq!(fs::read(&path).unwrap(),bytes);
        save(&library,&json!({"status":"pending","docs":docs,"rows":rows})).unwrap();assert_eq!(sync_locked(&library,||Ok(false)).unwrap()["added"],0);
        fs::write(&path,b"version=99").unwrap();save(&library,&json!({"status":"pending","docs":docs,"rows":rows})).unwrap();assert_eq!(sync_locked(&library,||Ok(false)).unwrap()["status"],"error");assert_eq!(fs::read(&path).unwrap(),b"version=99");
        save(&library,&json!({"status":"pending","docs":docs,"rows":rows})).unwrap();assert_eq!(sync_locked(&library,||Err("process scan unavailable".into())).unwrap()["status"],"error");
    }
}
