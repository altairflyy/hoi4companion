#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Small native launcher; it exits after starting/reusing the two owned Rust
//! applications. A stop capability targets only this data-directory service.
use hoi4companion_backend::{bootstrap, loopback::Api};
use serde_json::{Value, json};
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE,HWND},
        System::{Com::CoCreateGuid, Threading::CreateMutexW},
        UI::{
            Shell::ShellExecuteW,
            WindowsAndMessaging::{MB_ICONERROR, MessageBoxW, SW_SHOWNORMAL,CreateWindowExW,DestroyWindow,SetWindowTextW,PeekMessageW,TranslateMessage,DispatchMessageW,IsWindow,MSG,PM_REMOVE,WS_EX_APPWINDOW,WS_OVERLAPPED,WS_CAPTION,WS_SYSMENU,WS_VISIBLE,WS_CHILD,CW_USEDEFAULT},
        },
    },
    core::PCWSTR,
};
type Result<T> = std::result::Result<T, String>;
struct Owned(HANDLE);
impl Drop for Owned {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
struct LoadingNotice{window:HWND,label:HWND,italian:bool,last_second:u64}
impl LoadingNotice{
    fn new(data:&std::path::Path,enabled:bool)->Option<Self>{
        if !enabled{return None;}
        let italian=std::fs::read(data.join("settings.json")).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok()).is_some_and(|s|s["ui_language"]=="italian");
        unsafe{
            let class=wide("STATIC");let title=wide("HOI4 Companion");
            let window=CreateWindowExW(WS_EX_APPWINDOW,PCWSTR(class.as_ptr()),PCWSTR(title.as_ptr()),WS_OVERLAPPED|WS_CAPTION|WS_SYSMENU|WS_VISIBLE,CW_USEDEFAULT,CW_USEDEFAULT,540,155,None,None,None,None).ok()?;
            let text=wide(&loading_text(italian,0));
            let label=match CreateWindowExW(Default::default(),PCWSTR(class.as_ptr()),PCWSTR(text.as_ptr()),WS_CHILD|WS_VISIBLE,20,18,490,92,Some(window),None,None,None){Ok(v)=>v,Err(_)=>{let _=DestroyWindow(window);return None;}};
            Some(Self{window,label,italian,last_second:0})
        }
    }
    fn tick(&mut self,elapsed:Duration){unsafe{
        if elapsed.as_secs()!=self.last_second&&IsWindow(Some(self.label)).as_bool(){self.last_second=elapsed.as_secs();let text=wide(&loading_text(self.italian,self.last_second));let _=SetWindowTextW(self.label,PCWSTR(text.as_ptr()));}
        let mut message=MSG::default();while PeekMessageW(&mut message,None,0,0,PM_REMOVE).as_bool(){let _=TranslateMessage(&message);DispatchMessageW(&message);}
    }}
}
impl Drop for LoadingNotice{fn drop(&mut self){unsafe{if IsWindow(Some(self.window)).as_bool(){let _=DestroyWindow(self.window);}}}}
fn loading_text(italian:bool,seconds:u64)->String{
    if italian{format!("Preparazione dei dati HOI4 e delle guide… {seconds} s\nIl primo avvio dopo un aggiornamento può richiedere più tempo.\nLa finestra si chiude automaticamente quando Companion è pronto.")}
    else{format!("Preparing HOI4 data and your guides… {seconds} s\nThe first start after an update can take longer.\nThis window closes automatically when Companion is ready.")}
}
struct Options {
    port: u16,
    resources: Option<PathBuf>,
    data: Option<PathBuf>,
    browser: bool,
    app_window: bool,
    classic: bool,
    legacy_hud: bool,
    hud: bool,
    stop: bool,
    check: bool,
}
fn options() -> Result<Options> {
    let mut value = Options {
        port: 8766,
        resources: None,
        data: None,
        browser: true,
        app_window: true,
        classic: false,
        legacy_hud: false,
        hud: true,
        stop: false,
        check: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--port" => {
                value.port = args
                    .next()
                    .ok_or("Missing port")?
                    .parse()
                    .map_err(|_| "Invalid port")?
            }
            "--resources" => value.resources = Some(args.next().ok_or("Missing resources")?.into()),
            "--data-dir" => value.data = Some(args.next().ok_or("Missing data directory")?.into()),
            "--no-browser" => value.browser = false,
            "--browser" => value.app_window = false,
            "--classic" => value.classic = true,
            "--legacy-hud" => value.legacy_hud = true,
            "--no-hud" => value.hud = false,
            "--stop" => value.stop = true,
            "--self-check" => value.check = true,
            _ => return Err(format!("Unknown launcher option: {arg}")),
        }
    }
    if value.port < 1024 {
        return Err("Invalid local port".into());
    }
    Ok(value)
}
fn spawn(
    path: &std::path::Path,
    args: &[String],
    log: &std::path::Path,
) -> Result<std::process::Child> {
    use std::os::windows::io::AsRawHandle;
    use std::os::windows::process::CommandExt;
    use windows::Win32::Foundation::{
        GetHandleInformation, HANDLE_FLAG_INHERIT, HANDLE_FLAGS, SetHandleInformation,
    };
    struct Restore(Vec<(HANDLE, HANDLE_FLAGS)>);
    impl Drop for Restore {
        fn drop(&mut self) {
            for (handle, flags) in &self.0 {
                let _ = unsafe { SetHandleInformation(*handle, HANDLE_FLAG_INHERIT.0, *flags) };
            }
        }
    }
    // The launcher can itself be invoked through pipes (tests, terminals,
    // installers). A detached service must not retain those parent pipes.
    // Only the explicitly redirected log/NUL handles belong to the child.
    let mut restore = Restore(vec![]);
    for raw in [
        std::io::stdin().as_raw_handle(),
        std::io::stdout().as_raw_handle(),
        std::io::stderr().as_raw_handle(),
    ] {
        let handle = HANDLE(raw);
        let mut flags = HANDLE_FLAGS(0);
        if unsafe { GetHandleInformation(handle, &mut flags.0) }.is_ok()
            && flags.0 & HANDLE_FLAG_INHERIT.0 != 0
        {
            unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0)) }
                .map_err(|e| e.to_string())?;
            restore.0.push((handle, flags));
        }
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .map_err(|e| e.to_string())?;
    Command::new(path)
        .args(args)
        .creation_flags(0x08000000)
        .stdin(Stdio::null())
        .stdout(file.try_clone().map_err(|e| e.to_string())?)
        .stderr(file)
        .spawn()
        .map_err(|e| e.to_string())
}
fn run(options: Options) -> Result<Value> {
    let resources = bootstrap::resources(options.resources.as_deref())?;
    let data = bootstrap::data_directory(&resources, options.data.as_deref())?;
    if options.check {
        return Ok(
            json!({"schema_version":1,"engine":"rust-desktop","resident_python":false,"configuration":bootstrap::configuration(&resources,&data,&json!({}),false)?}),
        );
    }
    std::fs::create_dir_all(&data).map_err(|e| e.to_string())?;
    let key = wide(&format!("Local\\HOI4CompanionLaunch{}", options.port));
    let mutex = Owned(
        unsafe { CreateMutexW(None, false, PCWSTR(key.as_ptr())) }.map_err(|e| e.to_string())?,
    );
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        return Ok(json!({"status":"startup_already_in_progress"}));
    }
    let _mutex = mutex;
    let api = Api { port: options.port };
    let identity = api.json("/api/backend", None).ok();
    let ready_path = data.join(format!("native-service-{}.json", options.port));
    let control_path = data.join(format!("native-control-{}.json", options.port));
    if options.stop {
        if identity
            .as_ref()
            .is_none_or(|v| v["engine"] != "rust-backend")
        {
            return Err(
                "Native Companion is not running on this port; no other process was stopped".into(),
            );
        }
        let control: Value = serde_json::from_slice(
            &std::fs::read(&control_path)
                .map_err(|_| "Control capability not available for this data directory")?,
        )
        .map_err(|_| "Invalid control capability")?;
        let stopped = api.json("/api/shutdown", Some(&json!({"token":control["token"]})))?;
        close_owned_hud(&resources, &data)?;
        return Ok(stopped);
    }
    if identity
        .as_ref()
        .is_some_and(|v| v["engine"] != "rust-backend" || v["native_hud_protocol"] != 1)
    {
        return Err("An incompatible service is using this port; close the previous Companion before starting the native version".into());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let backend = std::env::var_os("HOI4_BACKEND_EXE")
        .map(PathBuf::from)
        .unwrap_or_else(|| resources.join("_native/hoi4-backend.exe"));
    let backend = if backend.is_file() {
        backend
    } else {
        exe.parent()
            .ok_or("Missing launcher directory")?
            .join("hoi4-backend.exe")
    };
    if identity.is_none() {
        if std::net::TcpStream::connect_timeout(
            &([127, 0, 0, 1], options.port).into(),
            Duration::from_secs(1),
        )
        .is_ok()
        {
            return Err("Local port is occupied by another program; no process was stopped".into());
        }
        if !backend.is_file() {
            return Err(
                "Native backend executable missing; extract the complete application ZIP".into(),
            );
        }
        let token = format!(
            "{:032x}{:032x}",
            unsafe { CoCreateGuid() }
                .map_err(|e| e.to_string())?
                .to_u128(),
            unsafe { CoCreateGuid() }
                .map_err(|e| e.to_string())?
                .to_u128()
        );
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&control_path)
            .map_err(|e| e.to_string())?;
        file.write_all(
            json!({"schema_version":1,"token":token})
                .to_string()
                .as_bytes(),
        )
        .map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        let args = vec![
            "--resources".into(),
            resources.to_string_lossy().into(),
            "--data-dir".into(),
            data.to_string_lossy().into(),
            "--port".into(),
            options.port.to_string(),
            "--ready-file".into(),
            ready_path.to_string_lossy().into(),
            "--control-file".into(),
            control_path.to_string_lossy().into(),
        ];
        let mut server = spawn(&backend, &args, &data.join("server.log"))?;
        let began = Instant::now();
        let mut loading=LoadingNotice::new(&data,options.browser);
        loop {
            if let Some(loading)=loading.as_mut(){loading.tick(began.elapsed());}
            if let Ok(value) = api.json("/api/backend", None) {
                if value["engine"] != "rust-backend" || value["native_hud_protocol"] != 1 {
                    return Err("Native service identity could not be verified".into());
                }
                break;
            }
            if server.try_wait().map_err(|e| e.to_string())?.is_some() {
                return Err(format!(
                    "Native backend startup failed: {}",
                    data.join("server.log").display()
                ));
            }
            if began.elapsed() > Duration::from_secs(240) {
                return Err(format!(
                    "Startup is still loading; see {}. The owned server was left running.",
                    data.join("server.log").display()
                ));
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }
    if options.hud {
        let overlay = std::env::var_os("HOI4_OVERLAY_NATIVE")
            .map(PathBuf::from)
            .unwrap_or_else(|| resources.join("_native/overlay/hoi4-overlay.exe"));
        if !overlay.is_file() {
            return Err(
                "Native HUD missing; rebuild or extract the complete application ZIP".into(),
            );
        }
        let config = api.json("/api/config", None)?;
        let language = config["settings"]["ui_language"]
            .as_str()
            .unwrap_or("english");
        let mut args = vec![
            "--port".into(),
            options.port.to_string(),
            "--start".into(),
            "--ui-language".into(),
            language.into(),
            "--status-file".into(),
            data.join("overlay-status.json").to_string_lossy().into(),
        ];
        if !options.legacy_hud { args.push("--discreet".into()); }
        let mut hud = spawn(&overlay, &args, &data.join("hud.log"))?;
        std::thread::sleep(Duration::from_millis(300));
        if hud
            .try_wait()
            .map_err(|e| e.to_string())?
            .is_some_and(|v| !v.success())
        {
            return Err(format!(
                "Native HUD startup failed: {}",
                data.join("hud.log").display()
            ));
        }
    }
    if options.browser {
        open_workspace(options.port, &data, options.app_window, options.classic)?;
    }
    Ok(
        json!({"schema_version":1,"engine":"rust-desktop","resident_python":false,"status":"ready","port":options.port,"data_directory":data}),
    )
}
fn workspace_url(port:u16,classic:bool)->String {
    format!("http://127.0.0.1:{}/{}",port,if classic{"walkthrough"}else{"suite.html"})
}
fn app_arguments(url:&str,data:&std::path::Path)->Vec<std::ffi::OsString> {
    let mut profile=std::ffi::OsString::from("--user-data-dir=");profile.push(data.join("desktop-profile"));
    vec![format!("--app={url}").into(),profile,"--guest".into(),"--disable-sync".into(),"--no-first-run".into(),"--no-default-browser-check".into(),"--disable-background-mode".into()]
}
fn open_workspace(port:u16,data:&std::path::Path,app_window:bool,classic:bool)->Result<()> {
    let address=workspace_url(port,classic);
    // Use the installed browser engine in app mode rather than shipping another
    // Electron runtime. If unavailable, classic browser access still works.
    if app_window {
        for base in ["ProgramFiles(x86)","ProgramFiles","LOCALAPPDATA"] {
            if let Some(base)=std::env::var_os(base) {
                let edge=PathBuf::from(base).join("Microsoft/Edge/Application/msedge.exe");
                if edge.is_file() && Command::new(edge).args(app_arguments(&address,data))
                    .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().is_ok() {return Ok(())}
            }
        }
    }
        let url = wide(&address);
        let action = wide("open");
        let result = unsafe {
            ShellExecuteW(
                None,
                PCWSTR(action.as_ptr()),
                PCWSTR(url.as_ptr()),
                None,
                None,
                SW_SHOWNORMAL,
            )
        };
        if result.0 as isize <= 32 {
            return Err("The application is running but Windows could not open the browser".into());
        }
    Ok(())
}
#[cfg(test)] mod suite_tests {
    use super::*;
    #[test] fn suite_and_classic_share_the_current_service_port(){
        assert_eq!(workspace_url(8766,false),"http://127.0.0.1:8766/suite.html");
        assert_eq!(workspace_url(9123,true),"http://127.0.0.1:9123/walkthrough");
    }
    #[test]fn loading_notice_reports_real_elapsed_time_in_both_languages(){assert!(loading_text(false,12).contains("12 s"));assert!(loading_text(true,3).contains("3 s"));assert!(loading_text(true,3).contains("automaticamente"));}
    #[test] fn browser_profile_is_dedicated_and_arguments_preserve_spaces(){
        let args=app_arguments("http://127.0.0.1:8766/suite.html",std::path::Path::new(r"C:\User data\Companion"));
        assert_eq!(args.len(),7);
        assert_eq!(args[1],std::ffi::OsString::from(r"--user-data-dir=C:\User data\Companion\desktop-profile"));
        assert!(!args.iter().any(|s|s.to_string_lossy().contains("remote-debugging")));
        assert!(args.contains(&std::ffi::OsString::from("--guest")));
        assert!(args.contains(&std::ffi::OsString::from("--disable-sync")));
    }
}
fn close_owned_hud(resources: &std::path::Path, data: &std::path::Path) -> Result<()> {
    use windows::{
        Win32::{
            Foundation::HWND,
            System::Threading::{
                OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
                QueryFullProcessImageNameW,
            },
            UI::WindowsAndMessaging::{GetWindowThreadProcessId, PostMessageW, WM_CLOSE},
        },
        core::PWSTR,
    };
    let path = data.join("overlay-status.json");
    if !path.is_file() {
        return Ok(());
    }
    let value = bootstrap::hud_status(data)?;
    let Some(pid) = value["host_pid"]
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
    else {
        return Ok(());
    };
    let Some(window) = value["control_window"].as_i64() else {
        return Ok(());
    };
    let hwnd = HWND(window as usize as *mut _);
    let mut actual = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut actual)) };
    if actual != pid {
        return Ok(());
    }
    let handle = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
        Ok(handle) => Owned(handle),
        Err(_) => return Ok(()),
    };
    let mut name = vec![0u16; 32768];
    let mut length = name.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            handle.0,
            PROCESS_NAME_WIN32,
            PWSTR(name.as_mut_ptr()),
            &mut length,
        )
    }
    .map_err(|e| e.to_string())?;
    let actual =
        PathBuf::from(String::from_utf16(&name[..length as usize]).map_err(|e| e.to_string())?)
            .canonicalize()
            .map_err(|e| e.to_string())?;
    let expected = std::env::var_os("HOI4_OVERLAY_NATIVE")
        .map(PathBuf::from)
        .unwrap_or_else(|| resources.join("_native/overlay/hoi4-overlay.exe"))
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if actual == expected {
        unsafe { PostMessageW(Some(hwnd), WM_CLOSE, Default::default(), Default::default()) }
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn main() {
    match options().and_then(run) {
        Ok(value) => println!("{value}"),
        Err(error) => {
            eprintln!("{error}");
            if !cfg!(debug_assertions) {
                let title = wide("HOI4 Companion");
                let message = wide(&error);
                unsafe {
                    MessageBoxW(
                        None,
                        PCWSTR(message.as_ptr()),
                        PCWSTR(title.as_ptr()),
                        MB_ICONERROR,
                    )
                };
            }
            std::process::exit(1);
        }
    }
}
