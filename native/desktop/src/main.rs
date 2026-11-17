#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Resident native desktop host and explicit headless service launcher.
mod bootstrap;
#[path = "../../common/loopback.rs"]
#[allow(dead_code)]
mod loopback;
use loopback::Api;
use serde_json::{Value, json};
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND},
        System::{Com::CoCreateGuid, Threading::CreateMutexW},
        UI::WindowsAndMessaging::{
            CW_USEDEFAULT, CreateWindowExW, DestroyWindow, DispatchMessageW, IsWindow,
            MB_ICONERROR, MSG, MessageBoxW, PM_REMOVE, PeekMessageW, SetWindowTextW,
            TranslateMessage, WS_CAPTION, WS_CHILD, WS_EX_APPWINDOW, WS_OVERLAPPED, WS_SYSMENU,
            WS_VISIBLE,
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
// During startup the launcher owns this exact child. If startup is cancelled or
// fails, do not leave a server behind without a corresponding desktop window.
struct StartingChild(Option<std::process::Child>);
impl Drop for StartingChild {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
struct LoadingNotice {
    window: HWND,
    label: HWND,
    italian: bool,
    last_second: u64,
}
impl LoadingNotice {
    fn new(data: &std::path::Path, enabled: bool, port: u16) -> Option<Self> {
        if !enabled {
            return None;
        }
        let italian = std::fs::read(data.join("settings.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .is_some_and(|s| s["ui_language"] == "italian");
        unsafe {
            let class = wide("STATIC");
            let title = wide(&window_title(port));
            let window = CreateWindowExW(
                WS_EX_APPWINDOW,
                PCWSTR(class.as_ptr()),
                PCWSTR(title.as_ptr()),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                540,
                155,
                None,
                None,
                None,
                None,
            )
            .ok()?;
            let text = wide(&loading_text(italian, 0));
            let label = match CreateWindowExW(
                Default::default(),
                PCWSTR(class.as_ptr()),
                PCWSTR(text.as_ptr()),
                WS_CHILD | WS_VISIBLE,
                20,
                18,
                490,
                92,
                Some(window),
                None,
                None,
                None,
            ) {
                Ok(v) => v,
                Err(_) => {
                    let _ = DestroyWindow(window);
                    return None;
                }
            };
            Some(Self {
                window,
                label,
                italian,
                last_second: 0,
            })
        }
    }
    fn tick(&mut self, elapsed: Duration) {
        unsafe {
            if elapsed.as_secs() != self.last_second && IsWindow(Some(self.label)).as_bool() {
                self.last_second = elapsed.as_secs();
                let text = wide(&loading_text(self.italian, self.last_second));
                let _ = SetWindowTextW(self.label, PCWSTR(text.as_ptr()));
            }
            let mut message = MSG::default();
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
}
impl Drop for LoadingNotice {
    fn drop(&mut self) {
        unsafe {
            if IsWindow(Some(self.window)).as_bool() {
                let _ = DestroyWindow(self.window);
            }
        }
    }
}
fn loading_text(italian: bool, seconds: u64) -> String {
    if italian {
        format!(
            "Preparazione dei dati HOI4 e delle guide… {seconds} s\nIl primo avvio dopo un aggiornamento può richiedere più tempo.\nLa finestra si chiude automaticamente quando Companion è pronto."
        )
    } else {
        format!(
            "Preparing HOI4 data and your guides… {seconds} s\nThe first start after an update can take longer.\nThis window closes automatically when Companion is ready."
        )
    }
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
        foreground_existing(options.port, options.stop)?;
        return Ok(if options.stop {
            json!({"stopping":true})
        } else {
            json!({"status":"activated"})
        });
    }

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
        // The control token authenticates the backend, not an unrelated HUD process.
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
    if identity.is_some() {
        return Err(
            "Local port is occupied by an existing service; it was not adopted or stopped".into(),
        );
    }
    let job = if options.browser {
        Some(ChildJob::new()?)
    } else {
        None
    };
    let mut owned_token = String::new();
    let mut starting_server = StartingChild(None);
    let mut starting_hud = StartingChild(None);
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
        owned_token = token.clone();
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
        starting_server.0 = Some(spawn(&backend, &args, &data.join("server.log"))?);
        if let Some(job) = &job {
            job.add(starting_server.0.as_ref().unwrap())?;
        }
        let began = Instant::now();
        let mut loading = LoadingNotice::new(&data, options.browser, options.port);
        loop {
            if let Some(loading) = loading.as_mut() {
                loading.tick(began.elapsed());
            }
            if loading
                .as_ref()
                .is_some_and(|notice| unsafe { !IsWindow(Some(notice.window)).as_bool() })
            {
                return Ok(json!({"status":"startup_cancelled"}));
            }
            if let Ok(value) = api.json("/api/backend", None) {
                let proof =
                    api.json("/api/desktop/identity", Some(&json!({"token":owned_token})))?;
                if proof["verified"] != true
                    || proof["pid"].as_u64()
                        != Some(starting_server.0.as_ref().unwrap().id() as u64)
                    || value["engine"] != "rust-backend"
                    || value["native_hud_protocol"] != 1
                {
                    return Err("Native service identity could not be verified".into());
                }
                break;
            }
            if starting_server
                .0
                .as_mut()
                .unwrap()
                .try_wait()
                .map_err(|e| e.to_string())?
                .is_some()
            {
                return Err(format!(
                    "Native backend startup failed: {}",
                    data.join("server.log").display()
                ));
            }
            if began.elapsed() > Duration::from_secs(240) {
                return Err(format!(
                    "Startup timed out; see {}. The server started by this launch was stopped.",
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
        if !options.legacy_hud {
            args.push(
                if options.browser {
                    "--managed"
                } else {
                    "--discreet"
                }
                .into(),
            );
        }
        starting_hud.0 = Some(spawn(&overlay, &args, &data.join("hud.log"))?);
        if let Some(job) = &job {
            job.add(starting_hud.0.as_ref().unwrap())?;
        }
        std::thread::sleep(Duration::from_millis(300));
        if starting_hud
            .0
            .as_mut()
            .unwrap()
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
        return desktop_window(
            options.port,
            data,
            options.classic,
            owned_token,
            starting_server,
            starting_hud,
            job.unwrap(),
            mutex,
        )
        .map(|_| json!({"status":"closed"}));
    }
    // The current launcher transfers ownership only after all startup steps
    // succeed. A resident desktop host must take over this lifecycle instead.
    starting_server.0.take();
    starting_hud.0.take();
    Ok(
        json!({"schema_version":1,"engine":"rust-desktop","resident_python":false,"status":"ready","port":options.port,"data_directory":data}),
    )
}
fn workspace_url(port: u16, classic: bool) -> String {
    format!(
        "http://127.0.0.1:{}/{}",
        port,
        if classic { "walkthrough" } else { "suite.html" }
    )
}
#[cfg(test)]
mod suite_tests {
    use super::*;
    #[test]
    fn suite_and_classic_share_the_current_service_port() {
        assert_eq!(
            workspace_url(8766, false),
            "http://127.0.0.1:8766/suite.html"
        );
        assert_eq!(
            workspace_url(9123, true),
            "http://127.0.0.1:9123/walkthrough"
        );
    }
    #[test]
    fn loading_notice_reports_real_elapsed_time_in_both_languages() {
        assert!(loading_text(false, 12).contains("12 s"));
        assert!(loading_text(true, 3).contains("3 s"));
        assert!(loading_text(true, 3).contains("automaticamente"));
    }
}
fn main() {
    // Headless invocations must always return errors to their caller, even
    // when argument parsing fails before Options can be constructed.
    let headless = std::env::args_os().skip(1).any(|arg| {
        matches!(
            arg.to_str(),
            Some("--no-browser" | "--self-check" | "--stop")
        )
    });
    match options().and_then(run) {
        Ok(value) => println!("{value}"),
        Err(error) => {
            eprintln!("{error}");
            if !cfg!(debug_assertions) && !headless {
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

use std::os::windows::io::AsRawHandle;
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};
struct ChildJob(Owned);
impl ChildJob {
    fn new() -> Result<Self> {
        let job = Owned(unsafe { CreateJobObjectW(None, None) }.map_err(|e| e.to_string())?);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const _,
                std::mem::size_of_val(&limits) as u32,
            )
        }
        .map_err(|e| e.to_string())?;
        Ok(Self(job))
    }
    fn add(&self, child: &std::process::Child) -> Result<()> {
        unsafe { AssignProcessToJobObject(self.0.0, HANDLE(child.as_raw_handle())) }
            .map_err(|e| e.to_string())
    }
}
fn window_title(port: u16) -> String {
    if port == 8766 {
        "HOI4 Companion".into()
    } else {
        format!("HOI4 Companion ({port})")
    }
}
fn foreground_existing(port: u16, stop: bool) -> Result<()> {
    use windows::{
        Win32::{
            System::Threading::{
                OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
                QueryFullProcessImageNameW,
            },
            UI::WindowsAndMessaging::{
                FindWindowW, GetWindowThreadProcessId, PostMessageW, SW_RESTORE,
                SetForegroundWindow, ShowWindow, WM_CLOSE,
            },
        },
        core::PWSTR,
    };
    let title = wide(&window_title(port));
    let deadline = Instant::now() + Duration::from_secs(5);
    let window = loop {
        if let Ok(window) = unsafe { FindWindowW(None, PCWSTR(title.as_ptr())) } {
            break window;
        }
        if Instant::now() >= deadline {
            return if stop {
                Err("Companion is starting; try stopping it again shortly".into())
            } else {
                Ok(())
            };
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut pid));
    }
    let handle = Owned(
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
            .map_err(|e| e.to_string())?,
    );
    let mut buffer = vec![0u16; 32768];
    let mut length = buffer.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            handle.0,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
    }
    .map_err(|e| e.to_string())?;
    let actual =
        PathBuf::from(String::from_utf16(&buffer[..length as usize]).map_err(|e| e.to_string())?)
            .canonicalize()
            .map_err(|e| e.to_string())?;
    let expected = std::env::current_exe()
        .and_then(|p| p.canonicalize())
        .map_err(|e| e.to_string())?;
    if actual != expected {
        return Err("Companion window ownership could not be verified".into());
    }
    unsafe {
        if stop {
            PostMessageW(
                Some(window),
                WM_CLOSE,
                Default::default(),
                Default::default(),
            )
            .map_err(|e| e.to_string())?;
        } else {
            let _ = ShowWindow(window, SW_RESTORE);
            let _ = SetForegroundWindow(window);
        }
    }
    Ok(())
}
fn desktop_window(
    port: u16,
    data: PathBuf,
    classic: bool,
    token: String,
    mut server: StartingChild,
    mut hud: StartingChild,
    job: ChildJob,
    mutex: Owned,
) -> Result<()> {
    use tao::{
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
        window::WindowBuilder,
    };
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title(window_title(port))
        .with_inner_size(tao::dpi::LogicalSize::new(1280.0, 800.0))
        .build(&event_loop)
        .map_err(|e| e.to_string())?;
    let mut context = wry::WebContext::new(Some(data.join("webview2-profile")));
    let webview = wry::WebViewBuilder::new_with_web_context(&mut context)
        .with_url(workspace_url(port, classic))
        .build(&window)
        .map_err(|e| format!("WebView2: {e}"))?;
    let mut job = Some(job);
    event_loop.run(move |event, _, control_flow| {
        let _keep = (&window,&webview,&job,&mutex);
        *control_flow = ControlFlow::WaitUntil(Instant::now()+Duration::from_secs(1));
        let failed = matches!(event, Event::NewEvents(_)) && (server.0.as_mut().is_some_and(|c|c.try_wait().ok().flatten().is_some()) || hud.0.as_mut().is_some_and(|c|c.try_wait().ok().flatten().is_some()));
        if failed || matches!(event, Event::WindowEvent {event: WindowEvent::CloseRequested,..}) {
            if let Some(child)=hud.0.as_ref() { let _=close_exact_hud(&data,child.id()); }
            let api=Api{port};
            if server.0.as_ref().is_some_and(|child| api.json("/api/desktop/identity",Some(&json!({"token":token}))).ok().is_some_and(|v|v["verified"]==true && v["pid"].as_u64()==Some(child.id() as u64))) {
                let _=api.json("/api/shutdown",Some(&json!({"token":token})));
            }
            let until=Instant::now()+Duration::from_secs(5);
            while Instant::now()<until {
                let done_server=server.0.as_mut().is_none_or(|c|c.try_wait().ok().flatten().is_some());
                let done_hud=hud.0.as_mut().is_none_or(|c|c.try_wait().ok().flatten().is_some());
                if done_server && done_hud {break;}
                std::thread::sleep(Duration::from_millis(50));
            }
            // Stop any child that did not respond before showing a modal error.
            // Closing the job also clears descendants owned by this instance.
            drop(server.0.take().map(|child| StartingChild(Some(child))));
            drop(hud.0.take().map(|child| StartingChild(Some(child))));
            drop(job.take());
            if failed { let text=wide("A Companion component has stopped unexpectedly. The application will close. See server.log or hud.log for details."); let title=wide("HOI4 Companion"); unsafe {MessageBoxW(None,PCWSTR(text.as_ptr()),PCWSTR(title.as_ptr()),MB_ICONERROR); } }
            *control_flow = ControlFlow::Exit;
        }
    });
}
fn close_exact_hud(data: &std::path::Path, pid: u32) -> Result<()> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowThreadProcessId, PostMessageW, WM_CLOSE,
    };
    let status = bootstrap::hud_status(data)?;
    if status["host_pid"].as_u64() != Some(pid as u64) {
        return Ok(());
    }
    if let Some(raw) = status["control_window"].as_i64() {
        let hwnd = HWND(raw as usize as *mut _);
        let mut actual = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut actual));
        }
        if actual == pid {
            unsafe { PostMessageW(Some(hwnd), WM_CLOSE, Default::default(), Default::default()) }
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
