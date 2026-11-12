//! Optional cache of static prepared guides. Never stores live state or progress.
use serde_json::{Value,json};
use sha2::{Digest,Sha256};
use std::{io::{Read,Write},path::{Path,PathBuf},sync::OnceLock};
type Result<T>=std::result::Result<T,String>;
const LIMIT:u64=32*1024*1024;
struct HashWriter(Sha256);
impl Write for HashWriter {
    fn write(&mut self,bytes:&[u8])->std::io::Result<usize>{self.0.update(bytes);Ok(bytes.len())}
    fn flush(&mut self)->std::io::Result<()>{Ok(())}
}
fn digest(value:&impl serde::Serialize)->Result<String>{
    let mut writer=HashWriter(Sha256::new());
    serde_json::to_writer(&mut writer,value).map_err(|e|e.to_string())?;
    Ok(format!("{:x}",writer.0.finalize()))
}
fn module_path()->Option<PathBuf>{
    #[cfg(windows)] unsafe {
        use windows::{core::PCWSTR,Win32::{Foundation::HMODULE,System::LibraryLoader::{GetModuleHandleExW,GetModuleFileNameW,GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT}}};
        let mut module=HMODULE::default();
        GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS|GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            PCWSTR(module_path as *const () as *const u16),&mut module).ok()?;
        let mut path=vec![0u16;32768];let length=GetModuleFileNameW(Some(module),&mut path) as usize;
        if length==0||length>=path.len(){return None;}
        Some(PathBuf::from(String::from_utf16(&path[..length]).ok()?))
    }
    #[cfg(not(windows))] {std::env::current_exe().ok()}
}
fn engine()->Option<&'static str>{
    static HASH:OnceLock<Option<String>>=OnceLock::new();
    HASH.get_or_init(||{
        let mut file=std::fs::File::open(module_path()?).ok()?;
        let mut sha=Sha256::new();let mut bytes=[0u8;65536];
        loop{let n=file.read(&mut bytes).ok()?;if n==0{break;}sha.update(&bytes[..n]);}
        Some(format!("{:x}",sha.finalize()))
    }).as_deref()
}
pub(crate) struct Cache{root:PathBuf,generation:String}
impl Cache{
    pub fn open(root:&Path,source:&Value,library:&Value,vocabulary:&Value)->Option<Self>{
        Some(Self{root:root.join("compiled-guides-v1"),generation:digest(&(1,engine()?,source,library,vocabulary)).ok()?})
    }
    fn path(&self,guide:&Value)->Option<PathBuf>{
        Some(self.root.join(format!("{}.json",digest(&guide["id"].as_str()?).ok()?)))
    }
    fn read(&self,path:&Path,key:&str)->Option<Value>{
        let file=std::fs::File::open(path).ok()?;
        if file.metadata().ok()?.len()>LIMIT{return None;}
        let mut bytes=Vec::new();file.take(LIMIT+1).read_to_end(&mut bytes).ok()?;
        if bytes.len() as u64>LIMIT{return None;}
        let item:Value=serde_json::from_slice(&bytes).ok()?;
        if item["schema_version"]!=1||item["input"]!=key||item["checksum"]!=digest(&item["guide"]).ok()?{return None;}
        Some(item["guide"].clone())
    }
    fn write(&self,path:&Path,key:&str,guide:&Value)->Result<()>{
        std::fs::create_dir_all(&self.root).map_err(|e|e.to_string())?;
        let bytes=serde_json::to_vec(&json!({"schema_version":1,"input":key,"checksum":digest(guide)?,"guide":guide})).map_err(|e|e.to_string())?;
        if bytes.len() as u64>LIMIT{return Ok(());}
        static NEXT:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(0);
        let temporary=path.with_extension(format!("{}-{}.tmp",std::process::id(),NEXT.fetch_add(1,std::sync::atomic::Ordering::Relaxed)));
        let mut owned=false;
        let result=(||{
            let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(&temporary).map_err(|e|e.to_string())?;
            owned=true;
            let written=file.write_all(&bytes).map_err(|e|e.to_string());drop(file);
            written?;std::fs::rename(&temporary,path).map_err(|e|e.to_string())
        })();
        // Only this call's uniquely created cache temporary is disposable.
        if owned&&result.is_err(){let _=std::fs::remove_file(&temporary);}
        result
    }
    pub fn resolve(&self,guide:&Value,build:impl FnOnce()->Result<Value>)->Result<Value>{
        let key=digest(&(&self.generation,guide))?;
        let path=self.path(guide);
        if let Some(value)=path.as_ref().and_then(|p|self.read(p,&key)){return Ok(value);}
        let value=build()?;
        if let Some(path)=path{let _=self.write(&path,&key,&value);}
        Ok(value)
    }
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn reuse_and_invalidation_preserve_results_and_bound_storage(){
        let root=std::env::temp_dir().join(format!("hoi4-compiled-cache-{}",crate::progress_store::new_id().unwrap()));
        std::fs::create_dir(&root).unwrap();
        let mut cache=Cache{root:root.clone(),generation:"sources-language-library-engine-one".into()};
        let guide=json!({"id":"a","tracks":{}});let prepared=json!({"id":"a","tracks":{},"name":"Prepared"});
        assert_eq!(cache.resolve(&guide,||Ok(prepared.clone())).unwrap(),prepared);
        assert_eq!(cache.resolve(&guide,||panic!("warm cache recompiled")).unwrap(),prepared);
        let changed=json!({"id":"a","tracks":{"operations":[]}});
        assert_eq!(cache.resolve(&changed,||Ok(json!({"revision":2}))).unwrap()["revision"],2);
        cache.generation="changed-game-mod-language-reference-or-program".into();
        assert_eq!(cache.resolve(&changed,||Ok(json!({"revision":3}))).unwrap()["revision"],3);
        let path=cache.path(&changed).unwrap();let mut tampered:Value=serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();tampered["guide"]=json!({"revision":999});
        std::fs::write(&path,serde_json::to_vec(&tampered).unwrap()).unwrap();
        assert_eq!(cache.resolve(&changed,||Ok(json!({"revision":4}))).unwrap()["revision"],4);
        assert_eq!(std::fs::read_dir(&root).unwrap().count(),1);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]fn unavailable_cache_never_blocks_normal_compilation(){
        let root=std::env::temp_dir().join(format!("hoi4-cache-unavailable-{}",crate::progress_store::new_id().unwrap()));
        std::fs::create_dir(&root).unwrap();let blocked=root.join("not-a-directory");std::fs::write(&blocked,b"owned fixture").unwrap();
        let cache=Cache{root:blocked,generation:"one".into()};
        assert_eq!(cache.resolve(&json!({"id":"a"}),||Ok(json!({"ok":true}))).unwrap()["ok"],true);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]fn actual_generation_includes_catalog_language_references_and_vocabulary(){
        let root=std::env::temp_dir().join(format!("hoi4-cache-generation-{}",crate::progress_store::new_id().unwrap()));
        std::fs::create_dir(&root).unwrap();let guide=json!({"id":"a"});
        let a=Cache::open(&root,&json!({"language":"english","mods":[]}),&json!({"designs":[]}),&json!({"schema_version":1})).unwrap();
        a.resolve(&guide,||Ok(json!({"source":"one"}))).unwrap();
        let same=Cache::open(&root,&json!({"language":"english","mods":[]}),&json!({"designs":[]}),&json!({"schema_version":1})).unwrap();
        assert_eq!(same.resolve(&guide,||panic!("same inputs missed cache")).unwrap()["source"],"one");
        for (source,library,vocabulary) in [
            (json!({"language":"italian","mods":[]}),json!({"designs":[]}),json!({"schema_version":1})),
            (json!({"language":"english","mods":["new"]}),json!({"designs":[]}),json!({"schema_version":1})),
            (json!({"language":"english","mods":[]}),json!({"designs":["new"]}),json!({"schema_version":1})),
            (json!({"language":"english","mods":[]}),json!({"designs":[]}),json!({"schema_version":2}))]{
            let cache=Cache::open(&root,&source,&library,&vocabulary).unwrap();
            assert_ne!(a.generation,cache.generation);assert_eq!(cache.resolve(&guide,||Ok(json!({"source":"new"}))).unwrap()["source"],"new");
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
