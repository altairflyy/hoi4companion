#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod capture;
mod ocr;
mod render;
mod protocol;
mod evidence;

use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::path::PathBuf;
use std::time::{Duration,Instant};
use serde_json::{Value,json};
use windows::core::{PCWSTR,PWSTR};
use windows::Win32::Foundation::{CloseHandle,GetLastError,ERROR_ALREADY_EXISTS,HANDLE,POINT,RECT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::System::Threading::{CreateMutexW,OpenProcess,QueryFullProcessImageNameW,PROCESS_NAME_WIN32,PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext,DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::WindowsAndMessaging::{GetClientRect,GetForegroundWindow,GetWindowThreadProcessId,IsIconic,IsWindowVisible};
use capture::{Capture,Frame};
use render::{Renderer,TargetWindow,Highlight,OverlayFrame,ControlAction};

struct Options {
    port:u16, guide:String, python:String, start:bool, proof:Option<PathBuf>,
    ocr_image:Option<PathBuf>, ocr_scale:u32, ocr_repeat:usize, ocr_prepared:bool,
    prepare_dir:Option<PathBuf>,
    capture_access_check:bool,
    status:PathBuf,
}
fn options()->Result<Options,String> {
    let mut result=Options {port:8766,guide:String::new(),python:"python.exe".into(),start:false,
        proof:None,ocr_image:None,ocr_scale:2,ocr_repeat:3,ocr_prepared:false,prepare_dir:None,capture_access_check:false,
        status:PathBuf::from(".local/overlay-status.json")};
    let mut args=std::env::args().skip(1);
    while let Some(arg)=args.next() {
        match arg.as_str() {
            "--port"=>result.port=args.next().ok_or("Missing port")?.parse().map_err(|_|"Invalid port")?,
            "--guide"=>result.guide=args.next().ok_or("Missing guide")?,
            "--bridge-python"=>result.python=args.next().ok_or("Missing Python path")?,
            "--start"=>result.start=true,
            "--proof"=>result.proof=Some(args.next().ok_or("Missing proof path")?.into()),
            "--status-file"=>result.status=args.next().ok_or("Missing status path")?.into(),
            "--capture-access-check"=>result.capture_access_check=true,
            "--ocr-image"=>result.ocr_image=Some(args.next().ok_or("Missing image path")?.into()),
            "--ocr-scale"=>result.ocr_scale=args.next().ok_or("Missing scale")?.parse().map_err(|_|"Invalid OCR scale")?,
            "--ocr-repeat"=>result.ocr_repeat=args.next().ok_or("Missing repetitions")?.parse().map_err(|_|"Invalid repetitions")?,
            "--ocr-prepared"=>result.ocr_prepared=true,
            "--prepare-dir"=>result.prepare_dir=Some(args.next().ok_or("Missing prepared output directory")?.into()),
            _=>return Err(format!("Unknown option: {arg}")),
        }
    }
    if result.port<1024 || !matches!(result.ocr_scale,1|2) || !(1..=20).contains(&result.ocr_repeat) {
        return Err("Invalid port, OCR scale or repetition count".into());
    }
    Ok(result)
}

fn game_view()->Option<TargetWindow> {
    unsafe {
        let hwnd=GetForegroundWindow();
        if hwnd.0.is_null() || !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool(){return None;}
        let mut pid=0;GetWindowThreadProcessId(hwnd,Some(&mut pid));
        let handle=OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION,false,pid).ok()?;
        let mut name=vec![0u16;32768];let mut length=name.len() as u32;
        let queried=QueryFullProcessImageNameW(handle,PROCESS_NAME_WIN32,PWSTR(name.as_mut_ptr()),&mut length);
        let _=CloseHandle(handle);
        queried.ok()?;
        let name=String::from_utf16_lossy(&name[..length as usize]);
        if !name.rsplit(['\\','/']).next()?.eq_ignore_ascii_case("hoi4.exe") {return None;}
        let mut rect=RECT::default();GetClientRect(hwnd,&mut rect).ok()?;
        let mut point=POINT::default();if !ClientToScreen(hwnd,&mut point).as_bool(){return None;}
        let width=u32::try_from(rect.right).ok()?;let height=u32::try_from(rect.bottom).ok()?;
        if width==0||height==0||width>8192||height>8192||width as u64*height as u64*4>80_000_000{return None;}
        Some(TargetWindow {hwnd:hwnd.0 as isize,pid,x:point.x,y:point.y,width,height})
    }
}
fn scene_hash(frame:&Frame)->u64 {
    let mut hasher=DefaultHasher::new();
    hasher.write(&frame.bgra[(frame.height.min(82)*frame.width*4) as usize..]);hasher.finish()
}
fn aligned(frame:&Frame,view:&TargetWindow)->bool {
    frame.width==view.width&&frame.height==view.height&&frame.client_x==view.x&&frame.client_y==view.y
}
fn view_json(view:&TargetWindow)->Value {json!([view.hwnd,view.pid,view.x,view.y,view.width,view.height])}
fn native_route(snapshot:&Value,geometry:&Value)->bool {
    if geometry["suspend_visual"]==true{return true;}
    if geometry["native_available"]!=true{return false;}
    // Native trees and event popups route through the same renderer. A popup
    // takes priority over recommendations in the tree underneath it.
    if matches!(geometry["active_panel"].as_str(),Some("focus"|"research"|"event"|"politics"|"projects")){return true;}
    if geometry["matches"].as_array().is_some_and(|rows| rows.iter().any(|row|
        row["basis"]=="native_gui_action_identity")) {return true;}
    let handled=geometry["handled_panels"].as_array();
    snapshot["proposed"].as_array().is_some_and(|rows| !rows.is_empty()&&rows.iter().all(|row|
        handled.is_some_and(|panels|panels.iter().any(|panel|panel==&row["panel"]))))
}
fn marks(rows:&[Value],width:u32,height:u32)->Vec<Highlight> {
    rows.iter().filter_map(|row|{
        let rect=evidence::valid_rectangle(&row["rect"],width,height)?;
        let panel=row["panel"].as_str().unwrap_or("").to_owned();
        let title=if panel=="research"||panel=="focus"||panel=="event" {row["labels"][0].as_str().or_else(||row["title"].as_str())}
            else{row["title"].as_str()}.unwrap_or("Indicazione della guida");
        let count=row["count"].as_u64().unwrap_or(1);
        let caption=if panel=="research_tab" {format!("{} · {} {}",title,count,if count==1{"ricerca"}else{"ricerche"})}
            else if let Some(quantity)=row["quantity"].as_u64() {format!("{} · totale {} fabbriche",title,quantity)}
            else {title.to_owned()};
        Some(Highlight {rect:rect.map(|n|n as f32),caption:caption.chars().take(140).collect(),
             active:row["status"]=="active"&&panel!="event",panel})
    }).take(64).collect()
}
fn guides(snapshot:&Value)->Vec<(String,String)> {
    snapshot["guides"].as_array().into_iter().flatten().filter_map(|g|
        Some((g["id"].as_str()?.to_owned(),g["title"].as_str()?.to_owned()))).collect()
}
fn geometry_notice(geometry:&Value,count:usize)->(String,String) {
    if let Some(error)=geometry["native_error"].as_str().filter(|s|!s.is_empty()) {
        return ("technical_error".into(),format!("Lettura GUI sospesa: {error}"));
    }
    let items=geometry["explanations"].as_array();
    if let Some(item)=items.and_then(|rows|rows.iter().find(|r|r["code"]!="highlighted")) {
        if let (Some(code),Some(message))=(item["code"].as_str(),item["message"].as_str()) {
            return (code.to_owned(),if count>0 {format!("{count} indicazioni visibili. {message}")}else{message.to_owned()});
        }
    }
    if count>0 {("highlighted".into(),format!("{count} indicazioni individuate tramite ID e geometria GUI"))}
    else {("no_visible_targets".into(),"Nessun pulsante della guida individuato nella schermata corrente.".into())}
}
fn save_status(path:&PathBuf,value:&Value) {
    if let Some(parent)=path.parent(){let _=std::fs::create_dir_all(parent);}
    let temporary=path.with_extension("json.tmp");
    if let Ok(bytes)=serde_json::to_vec_pretty(value)
        && std::fs::write(&temporary,bytes).is_ok() {
        let _=std::fs::rename(temporary,path);
    }
}
struct Instance(HANDLE);
impl Drop for Instance {fn drop(&mut self){unsafe{let _=CloseHandle(self.0);}}}
fn singleton()->Result<Option<Instance>,String> {
    let mut hash=DefaultHasher::new();hash.write(std::env::current_exe().map_err(|e|e.to_string())?.to_string_lossy().as_bytes());
    let name:Vec<u16>=format!("Local\\HOI4CompanionNativeOverlay-{:x}",hash.finish()).encode_utf16().chain(Some(0)).collect();
    let handle=unsafe{CreateMutexW(None,false,PCWSTR(name.as_ptr()))}.map_err(|e|e.to_string())?;
    if unsafe{GetLastError()}==ERROR_ALREADY_EXISTS {unsafe{let _=CloseHandle(handle);}return Ok(None);}
    Ok(Some(Instance(handle)))
}
fn benchmark(options:&Options)->Result<(),String> {
    let path=options.ocr_image.as_ref().ok_or("No OCR image")?;
    let image=image::open(path).map_err(|e|e.to_string())?.into_rgba8();
    let (width,height)=image.dimensions();let mut data=image.into_raw();
    for pixel in data.as_chunks_mut::<4>().0{pixel.swap(0,2);}
    if let Some(directory)=&options.prepare_dir {
        std::fs::create_dir_all(directory).map_err(|e|e.to_string())?;
        for tile in ocr::prepare_tiles(width,height,&data,options.ocr_scale).map_err(|e|e.to_string())? {
            let mut rgba=tile.bgra;for pixel in rgba.as_chunks_mut::<4>().0{pixel.swap(0,2);}
            image::save_buffer(directory.join(format!("{}-{}.png",tile.left,tile.top)),&rgba,tile.width,tile.height,image::ColorType::Rgba8).map_err(|e|e.to_string())?;
        }
    }
    let engine=ocr::Ocr::new().map_err(|e|e.to_string())?;
    let run=|| if options.ocr_prepared {engine.recognize_bitmap_bgra(width,height,&data)}
        else{engine.recognize_bgra(width,height,&data,options.ocr_scale)};
    if options.ocr_prepared {run().map_err(|e|e.to_string())?;}
    else {engine.recognize_file(path,options.ocr_scale).map_err(|e|e.to_string())?;}
    let mut samples=Vec::new();let mut pages=Vec::new();
    for _ in 0..options.ocr_repeat {
        let started=Instant::now();let page=run().map_err(|e|e.to_string())?;
        samples.push(started.elapsed().as_secs_f64()*1000.);pages.push(page);
    }
    println!("{}",json!({"schema_version":1,"samples_ms":samples,"page":pages.last(),"all_pages":pages}));
    Ok(())
}

fn run(options:Options)->Result<(),String> {
    if options.capture_access_check {
        let _apartment=capture::Apartment::new().map_err(|e|e.to_string())?;
        save_status(&options.status,&json!({"capture_borderless_allowed":capture::borderless_access(),"capture_active":false}));
        return Ok(());
    }
    if options.ocr_image.is_some(){return benchmark(&options);}
    let Some(_instance)=singleton()? else {return Ok(());};
    unsafe {let _=SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);}
    let mut renderer=Renderer::new()?;renderer.set_paused(!options.start);
    let mut bridge=protocol::Bridge::new(&options.python,options.port,&options.guide)?;
    let mut engine:Option<ocr::Ocr>=None;
    let mut capture:Option<Capture>=None;let mut target:Option<TargetWindow>=None;
    let mut last_scene=None;let mut last_version=None;let mut cached_ocr=None;
    let mut last_matches:Vec<Value>=Vec::new();
    let mut reference_pixels=Vec::new();let mut evidence_cache_hits=0u64;
    let mut proof_saved=false;let mut last_status=Instant::now()-Duration::from_secs(2);
    let mut total_ocr=0u64;let mut total_frames=0u64;let mut last_draw=Instant::now()-Duration::from_secs(2);
    let mut native_frames=0u64;
    let mut native_unchanged_since=Instant::now();
    let mut ocr_ms=0.;let mut matching_ms=0.;
    while !renderer.should_exit() {
        let mut refresh=false;
        while let Some(action)=renderer.take_action(){
            renderer.hide();last_matches.clear();last_version=None;
            match action {
                ControlAction::SelectGuide(id)=>{let _=bridge.request(json!({"kind":"select","guide_id":id}),&[])?;},
                ControlAction::RefreshGuides=>refresh=true,
            }
        }
        let snapshot=bridge.snapshot(refresh)?;
        renderer.set_guides(guides(&snapshot),snapshot["guide_id"].as_str().unwrap_or(""));
        let current=game_view();
        if renderer.is_paused()||current.is_none()||snapshot["usable"]!=true {
            renderer.hide();capture=None;target=None;last_scene=None;last_matches.clear();cached_ocr=None;
            let message=if renderer.is_paused(){"Overlay in pausa"}else if current.is_none(){"Passa a HOI4 per mostrare le indicazioni"}
                else{snapshot["idle_explanation"]["message"].as_str().or_else(||snapshot["error"].as_str().filter(|s|!s.is_empty())).unwrap_or("Nessuna indicazione attuale della guida")};
            let code=if renderer.is_paused(){"paused"}else if current.is_none(){"game_background"}else{snapshot["idle_explanation"]["code"].as_str().unwrap_or("live_unavailable")};
            if last_status.elapsed()>Duration::from_millis(500) {
                renderer.set_status(message);save_status(&options.status,&json!({"engine":"rust-wgc-winrtocr-direct2d","visible":false,"error":message,"memory_access":false,"host_pid":std::process::id(),"guide_id":snapshot["guide_id"],"guide_title":snapshot["title"],"capture_active":false,"guide_server_id":snapshot["guide_server_id"],"status_message":message,"status_code":code,"explanations":[]}));last_status=Instant::now();
            }
            std::thread::sleep(Duration::from_millis(100));continue;
        }
        let view=current.unwrap();
        if target.as_ref()!=Some(&view) {
            renderer.hide();capture=None;
            target=Some(view.clone());last_scene=None;cached_ocr=None;last_matches.clear();
        }
        let started=Instant::now();
        let version=snapshot["version"].as_u64().ok_or("Missing guide revision")?;
        // Runtime identities and freshly witnessed game GUI rectangles take
        // precedence over OCR. Neither the label nor a previous image supplies
        // the tree transform; panning/zooming changes these native rectangles.
        let geometry_observed=Instant::now();
        let geometry=bridge.request(json!({"kind":"geometry","version":version,"view":view_json(&view)}),&[])?;
        if native_route(&snapshot,&geometry) {
            // Native identities/geometry do not depend on a captured image or
            // OCR availability. No WGC readback or image bytes on this path.
            capture=None;
            last_scene=None;cached_ocr=None;last_version=None;
            let previous_matches=last_matches.clone();
            last_matches=if geometry["usable"]==true&&geometry["suspend_visual"]!=true&&geometry["version"]==version&&geometry["view"]==view_json(&view)
                &&geometry_observed.elapsed()<Duration::from_millis(750)&&game_view().as_ref()==Some(&view) {
                geometry["matches"].as_array().cloned().unwrap_or_default()
            }else{Vec::new()};
            if previous_matches!=last_matches {native_unchanged_since=Instant::now();}
            let highlights=marks(&last_matches,view.width,view.height);
            if highlights.is_empty(){renderer.hide();}
            else{renderer.show(OverlayFrame {target:view.clone(),marks:highlights,observed_at:geometry_observed,valid_for:Duration::from_millis(750)})?;}
            native_frames+=1;
            if last_status.elapsed()>Duration::from_millis(500){
                let ms=geometry["geometry_ms"].as_f64().unwrap_or(0.);
                let (code,message)=geometry_notice(&geometry,last_matches.len());
                renderer.set_status(&message);
                save_status(&options.status,&json!({"engine":"rust-native-gui-direct2d","host_pid":std::process::id(),
                    "guide_id":snapshot["guide_id"],"guide_title":snapshot["title"],
                    "guide_server_id":snapshot["guide_server_id"],"status_message":message,"status_code":code,"explanations":geometry["explanations"],
                    "visible":!last_matches.is_empty(),"overlay_window":renderer.overlay_window(),"control_window":renderer.control_window(),
                    "game_window":view.hwnd,"target_count":snapshot["proposed"].as_array().map_or(0,Vec::len),
                    "matches":last_matches,"geometry_ms":ms,"native_frames":native_frames,"frames_observed":total_frames,
                    "ocr_calls":total_ocr,"diagnostics":geometry["diagnostics"],"active_panel":geometry["active_panel"],
                    "error":geometry["native_error"],"capture_active":false,
                    "iteration_ms":started.elapsed().as_secs_f64()*1000.,
                    "evidence_age_ms":geometry_observed.elapsed().as_secs_f64()*1000.,"memory_access":true,"memory_access_mode":"query_vm_read",
                    "proof_saved":proof_saved}));
                last_status=Instant::now();
            }
            if let Some(path)=options.proof.as_ref().filter(|_|!proof_saved&&!last_matches.is_empty()
                &&native_unchanged_since.elapsed()>Duration::from_millis(250)) {
                renderer.set_capture_excluded(false);std::thread::sleep(Duration::from_millis(80));
                let result=render::save_proof(&view,path);renderer.set_capture_excluded(true);result?;
                let after=bridge.request(json!({"kind":"geometry","version":version,"view":view_json(&view)}),&[])?;
                proof_saved=after["usable"]==true&&after["version"]==version&&after["view"]==view_json(&view)
                    &&after["matches"]==geometry["matches"]&&game_view().as_ref()==Some(&view);
                save_status(&path.with_extension("json"),&json!({"verified":proof_saved,"before":geometry,
                    "after":after,"reason":"Native identities and geometry must remain unchanged across compositor capture"}));
                if !proof_saved {native_unchanged_since=Instant::now();}
            }
            // Bound external observations to display cadence, avoiding a busy
            // loop. The deadline is independent of this worker's progress.
            std::thread::sleep(Duration::from_millis(16).saturating_sub(started.elapsed()));
            continue;
        }
        if capture.is_none(){capture=Some(Capture::new(view.hwnd).map_err(|e|e.to_string())?);}
        let source=capture.as_mut().ok_or("Capture not initialized")?;
        let Some(frame)=source.wait_latest(Duration::from_millis(100)).map_err(|e|e.to_string())? else{continue;};
        if !aligned(&frame,&view)||game_view().as_ref()!=Some(&view){renderer.hide();continue;}
        total_frames+=1;
        let stable=last_version==Some(version)&&cached_ocr.as_ref().is_some_and(|page| {
            serde_json::to_value(page).ok().is_some_and(|page|
                evidence::stable(&reference_pixels,&frame.bgra,view.width,view.height,&last_matches,&page))
        });
        let digest=if stable{last_scene.unwrap_or(0)}else{scene_hash(&frame)};
        if stable {evidence_cache_hits+=1;}
        let changed=!stable&&(last_scene!=Some(digest)||last_version!=Some(version));
        let mut verified_at=frame.captured_at;
        if changed {
            renderer.hide();
            if last_scene!=Some(digest)||cached_ocr.is_none() {
                if engine.is_none(){engine=Some(ocr::Ocr::new().map_err(|e|e.to_string())?);}
                let began=Instant::now();cached_ocr=Some(engine.as_ref().unwrap().recognize(&frame,2).map_err(|e|e.to_string())?);
                ocr_ms=began.elapsed().as_secs_f64()*1000.;total_ocr+=1;
            }
            let response=bridge.request(json!({"kind":"frame","version":version,"view":view_json(&view),
                "width":frame.width,"height":frame.height,"raw_len":frame.bgra.len(),"ocr":cached_ocr,
                "frame_hash":format!("{digest:x}")}),&frame.bgra)?;
            matching_ms=response["recognition_ms"].as_f64().unwrap_or(0.);
            last_matches=if response["usable"]==true && response["version"]==version && response["view"]==view_json(&view) {
                response["matches"].as_array().cloned().unwrap_or_default()
            }else{Vec::new()};
            let Some(fresh)=source.wait_latest(Duration::from_millis(250)).map_err(|e|e.to_string())? else{
                last_matches.clear();last_scene=None;continue;
            };
            if !aligned(&fresh,&view)||game_view().as_ref()!=Some(&view){last_matches.clear();last_scene=None;continue;}
            last_matches.retain(|row|evidence::unchanged(&frame.bgra,&fresh.bgra,view.width,view.height,row));
            reference_pixels=frame.bgra.clone();
            verified_at=fresh.captured_at;
            last_scene=Some(digest);last_version=Some(version);
        }
        if game_view().as_ref()!=Some(&view){renderer.hide();continue;}
        let highlights=marks(&last_matches,view.width,view.height);
        if highlights.is_empty(){renderer.hide();}
        else if changed||last_draw.elapsed()>Duration::from_millis(250){
            renderer.show(OverlayFrame {target:view.clone(),marks:highlights,observed_at:verified_at,valid_for:Duration::from_millis(750)})?;
            last_draw=Instant::now();
        }
        if last_status.elapsed()>Duration::from_millis(500){
            let (code,message)=if last_matches.is_empty(){("visual_unresolved","Nessun pulsante identificato in modo univoco nel pannello visivo.".to_owned())}else{("highlighted",format!("{} indicazioni identificate nel pannello visivo",last_matches.len()))};
            renderer.set_status(&message);
            save_status(&options.status,&json!({"engine":"rust-wgc-winrtocr-direct2d","host_pid":std::process::id(),"visible":!last_matches.is_empty(),
                "guide_id":snapshot["guide_id"],"guide_title":snapshot["title"],"capture_active":true,"capture_borderless_allowed":source.borderless_allowed,
                "guide_server_id":snapshot["guide_server_id"],"status_message":message,"status_code":code,"explanations":[],
                "overlay_window":renderer.overlay_window(),"control_window":renderer.control_window(),"game_window":view.hwnd,"target_count":snapshot["proposed"].as_array().map_or(0,Vec::len),
                "matches":last_matches,"ocr_ms":ocr_ms,"matching_ms":matching_ms,"iteration_ms":started.elapsed().as_secs_f64()*1000.,
                "frames_observed":total_frames,"ocr_calls":total_ocr,"frame_generation":frame.generation,
                "evidence_cache_hits":evidence_cache_hits,
                "frame_age_ms":verified_at.elapsed().as_secs_f64()*1000.,"memory_access":false,"proof_saved":proof_saved}));
            last_status=Instant::now();
        }
        if let Some(path)=options.proof.as_ref().filter(|_|!proof_saved&&!last_matches.is_empty()) {
            renderer.set_capture_excluded(false);std::thread::sleep(Duration::from_millis(80));
            let result=render::save_proof(&view,path);renderer.set_capture_excluded(true);
            result?;proof_saved=true;
        }
    }
    renderer.shutdown();Ok(())
}
fn main() {
    if let Err(error)=options().and_then(run) {
        eprintln!("Overlay Rust: {error}");
        let path=PathBuf::from(".local/overlay-native-error.json");
        save_status(&path,&json!({"error":error,"visible":false,"engine":"rust","memory_access":false}));
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explanations_separate_unknown_evidence_from_visible_marks_and_errors() {
        let mut geometry=json!({"explanations":[{"code":"highlighted","message":"Visibile"},
            {"code":"different_folder","message":"Apri Industria"}]});
        assert_eq!(geometry_notice(&geometry,1),("different_folder".into(),"1 indicazioni visibili. Apri Industria".into()));
        geometry["native_error"]=json!("Proprietario incoerente");
        assert_eq!(geometry_notice(&geometry,0).0,"technical_error");
        assert!(geometry_notice(&geometry,0).1.contains("Proprietario incoerente"));
        assert_eq!(geometry_notice(&json!({}),0).0,"no_visible_targets");
    }
    #[test]
    fn native_tree_routing_preserves_mixed_guides_and_fails_closed() {
        let mixed=json!({"proposed":[{"panel":"focus"},{"panel":"research"},{"panel":"event"}]});
        let mut geometry=json!({"native_available":true,"active_panel":"focus","handled_panels":["focus","research"]});
        assert!(native_route(&mixed,&geometry));
        geometry["active_panel"]=json!("research");
        assert!(native_route(&mixed,&geometry));
        geometry["active_panel"]=json!("event");
        assert!(native_route(&mixed,&geometry));
        geometry["active_panel"]=json!("politics");
        assert!(native_route(&mixed,&geometry));
        geometry["active_panel"]=json!("projects");
        assert!(native_route(&mixed,&geometry));
        geometry["active_panel"]=Value::Null;
        assert!(!native_route(&mixed,&geometry));
        assert!(native_route(&json!({"proposed":[{"panel":"research"}]}),&geometry));
        geometry["matches"]=json!([{"basis":"native_gui_action_identity","navigation_stage":"access"}]);
        assert!(native_route(&mixed,&geometry));
        geometry["matches"]=json!([{"basis":"exact_unique_ocr"}]);
        assert!(!native_route(&mixed,&geometry));
        geometry["native_available"]=json!(false);
        assert!(!native_route(&mixed,&geometry));
        geometry["suspend_visual"]=json!(true);
        assert!(native_route(&mixed,&geometry));
    }
}
