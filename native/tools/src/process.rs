use super::*;
use std::fs::{File, OpenOptions};
use std::mem::size_of;
use std::os::windows::io::AsRawHandle;
use std::time::{Duration, Instant};
use windows::{
    Win32::{
        Foundation::*,
        System::{JobObjects::*, Threading::*},
    },
    core::{PCWSTR, PWSTR},
};

struct Owned(usize);
impl Owned {
    fn new(handle: HANDLE) -> Self {
        Self(handle.0 as usize)
    }
    fn get(&self) -> HANDLE {
        HANDLE(self.0 as *mut _)
    }
}
impl Drop for Owned {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.get());
        }
    }
}
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
fn error(context: &str) -> String {
    format!("{context}: {}", std::io::Error::last_os_error())
}
fn inherit(file: &File) -> Result<Owned, String> {
    unsafe {
        let mut duplicate = HANDLE::default();
        DuplicateHandle(
            GetCurrentProcess(),
            HANDLE(file.as_raw_handle()),
            GetCurrentProcess(),
            &mut duplicate,
            0,
            true,
            DUPLICATE_SAME_ACCESS,
        )
        .map_err(|e| e.to_string())?;
        Ok(Owned::new(duplicate))
    }
}
fn quoted(value: &str) -> String {
    let mut result = String::from("\"");
    let mut slashes = 0;
    for c in value.chars() {
        if c == '\\' {
            slashes += 1;
            continue;
        }
        if c == '"' {
            result.extend(std::iter::repeat_n('\\', slashes * 2 + 1));
            result.push('"');
        } else {
            result.extend(std::iter::repeat_n('\\', slashes));
            result.push(c);
        }
        slashes = 0;
    }
    result.extend(std::iter::repeat_n('\\', slashes * 2));
    result.push('"');
    result
}
struct Attributes {
    buffer: Vec<usize>,
    list: LPPROC_THREAD_ATTRIBUTE_LIST,
}
impl Attributes {
    fn new(handles: &[HANDLE]) -> Result<Self, String> {
        unsafe {
            let mut bytes = 0;
            let _ = InitializeProcThreadAttributeList(None, 1, None, &mut bytes);
            let mut buffer = vec![0usize; bytes.div_ceil(size_of::<usize>())];
            let list = LPPROC_THREAD_ATTRIBUTE_LIST(buffer.as_mut_ptr().cast());
            InitializeProcThreadAttributeList(Some(list), 1, None, &mut bytes)
                .map_err(|e| e.to_string())?;
            let result = Self { buffer, list };
            UpdateProcThreadAttribute(
                list,
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                Some(handles.as_ptr().cast()),
                size_of_val(handles),
                None,
                None,
            )
            .map_err(|e| e.to_string())?;
            Ok(result)
        }
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        unsafe {
            DeleteProcThreadAttributeList(self.list);
        }
        let _ = self.buffer.len();
    }
}
#[derive(Default)]
struct Status {
    code: Option<u32>,
    reason: Option<&'static str>,
}
pub struct Tool {
    job: Owned,
    process: Owned,
    pid: u32,
    began: Instant,
    limit: Duration,
    output: Vec<(std::path::PathBuf, u64)>,
    max_output: u64,
    status: Mutex<Status>,
}
impl Tool {
    pub fn spawn(spec: Spawn) -> Result<Arc<Self>, String> {
        let out = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&spec.stdout)
            .map_err(|e| e.to_string())?;
        let err = if spec.stdout == spec.stderr {
            out.try_clone()
        } else {
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&spec.stderr)
        }
        .map_err(|e| e.to_string())?;
        let mut output = vec![(
            std::path::PathBuf::from(&spec.stdout),
            out.metadata().map_err(|e| e.to_string())?.len(),
        )];
        if spec.stdout != spec.stderr {
            output.push((
                std::path::PathBuf::from(&spec.stderr),
                err.metadata().map_err(|e| e.to_string())?.len(),
            ));
        }
        let input = File::open("NUL").map_err(|e| e.to_string())?;
        let stdout = inherit(&out)?;
        let stderr = inherit(&err)?;
        let stdin = inherit(&input)?;
        let inherited = [stdin.get(), stdout.get(), stderr.get()];
        let attributes = Attributes::new(&inherited)?;
        let startup = STARTUPINFOEXW {
            StartupInfo: STARTUPINFOW {
                cb: size_of::<STARTUPINFOEXW>() as u32,
                dwFlags: STARTF_USESTDHANDLES,
                hStdInput: stdin.get(),
                hStdOutput: stdout.get(),
                hStdError: stderr.get(),
                ..Default::default()
            },
            lpAttributeList: attributes.list,
        };
        let mut info = PROCESS_INFORMATION::default();
        let mut command = wide(
            &spec
                .argv
                .iter()
                .map(|s| quoted(s))
                .collect::<Vec<_>>()
                .join(" "),
        );
        let executable = wide(&spec.argv[0]);
        let cwd = spec.cwd.as_deref().map(wide);
        let job = unsafe {
            Owned::new(CreateJobObjectW(None, PCWSTR::null()).map_err(|e| e.to_string())?)
        };
        let limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
            BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION {
                LimitFlags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                ..Default::default()
            },
            ..Default::default()
        };
        unsafe {
            SetInformationJobObject(
                job.get(),
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of_val(&limits) as u32,
            )
            .map_err(|e| e.to_string())?;
        }
        unsafe {
            CreateProcessW(
                PCWSTR(executable.as_ptr()),
                Some(PWSTR(command.as_mut_ptr())),
                None,
                None,
                true,
                CREATE_SUSPENDED
                    | CREATE_NO_WINDOW
                    | BELOW_NORMAL_PRIORITY_CLASS
                    | EXTENDED_STARTUPINFO_PRESENT,
                None,
                cwd.as_ref().map_or(PCWSTR::null(), |v| PCWSTR(v.as_ptr())),
                &startup.StartupInfo,
                &mut info,
            )
            .map_err(|e| e.to_string())?;
        }
        let process = Owned::new(info.hProcess);
        let thread = Owned::new(info.hThread);
        if let Err(e) = unsafe { AssignProcessToJobObject(job.get(), process.get()) } {
            unsafe {
                let _ = TerminateProcess(process.get(), 1);
                let _ = WaitForSingleObject(process.get(), 5000);
            }
            return Err(format!("Cannot isolate owned tool: {e}"));
        }
        if unsafe { ResumeThread(thread.get()) } == u32::MAX {
            unsafe {
                let _ = TerminateJobObject(job.get(), 1);
                let _ = WaitForSingleObject(process.get(), 5000);
            }
            return Err(error("Cannot resume owned tool"));
        }
        let tool = Arc::new(Self {
            job,
            process,
            pid: info.dwProcessId,
            began: Instant::now(),
            limit: Duration::from_millis(spec.max_millis),
            output,
            max_output: spec.max_output_bytes,
            status: Mutex::new(Status::default()),
        });
        let watch = Arc::downgrade(&tool);
        std::thread::Builder::new()
            .name("companion-owned-tool".into())
            .spawn(move || {
                while let Some(tool) = watch.upgrade() {
                    match tool.refresh() {
                        Ok(true) | Err(_) => break,
                        _ => {}
                    }
                    drop(tool);
                    std::thread::sleep(Duration::from_millis(25));
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(tool)
    }
    fn refresh(&self) -> Result<bool, String> {
        let mut status = self.status.lock().map_err(|_| "Tool state unavailable")?;
        if status.code.is_some() {
            return Ok(true);
        }
        let waited = unsafe { WaitForSingleObject(self.process.get(), 0) };
        if waited == WAIT_FAILED {
            return Err(error("Cannot observe owned tool"));
        }
        let bytes = self
            .output
            .iter()
            .filter_map(|(path, start)| {
                std::fs::metadata(path)
                    .ok()
                    .map(|m| m.len().saturating_sub(*start))
            })
            .sum::<u64>();
        if waited == WAIT_OBJECT_0 {
            let mut code = 0;
            unsafe {
                GetExitCodeProcess(self.process.get(), &mut code).map_err(|e| e.to_string())?;
                let _ = TerminateJobObject(self.job.get(), 1);
            }
            status.code = Some(code);
            if bytes > self.max_output {
                status.reason = Some("output_limit");
            }
            return Ok(true);
        }
        let reason = if self.began.elapsed() >= self.limit {
            Some("timeout")
        } else if bytes > self.max_output {
            Some("output_limit")
        } else {
            None
        };
        if let Some(reason) = reason {
            self.stop(&mut status, reason)?;
            return Ok(true);
        }
        Ok(false)
    }
    fn stop(&self, status: &mut Status, reason: &'static str) -> Result<(), String> {
        unsafe {
            TerminateJobObject(self.job.get(), 1).map_err(|e| e.to_string())?;
            if WaitForSingleObject(self.process.get(), 5000) != WAIT_OBJECT_0 {
                return Err("Owned tool did not terminate".into());
            }
            let mut code = 0;
            GetExitCodeProcess(self.process.get(), &mut code).map_err(|e| e.to_string())?;
            status.code = Some(code);
            status.reason = Some(reason);
        }
        Ok(())
    }
    pub fn status(&self) -> Result<Value, String> {
        self.refresh()?;
        let status = self.status.lock().map_err(|_| "Tool state unavailable")?;
        Ok(
            json!({"pid":self.pid,"returncode":status.code,"reason":status.reason,"running":status.code.is_none()}),
        )
    }
    pub fn wait(&self, millis: u32) -> Result<Value, String> {
        let began = Instant::now();
        while !self.refresh()? && began.elapsed() < Duration::from_millis(millis.into()) {
            unsafe {
                let _ = WaitForSingleObject(self.process.get(), 25);
            }
        }
        self.status()
    }
    pub fn cancel(&self) -> Result<Value, String> {
        if !self.refresh()? {
            let mut status = self.status.lock().map_err(|_| "Tool state unavailable")?;
            if status.code.is_none() {
                self.stop(&mut status, "cancelled")?;
            }
        }
        self.status()
    }
}
impl Drop for Tool {
    fn drop(&mut self) {
        unsafe {
            let _ = TerminateJobObject(self.job.get(), 1);
            let _ = WaitForSingleObject(self.process.get(), 5000);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_quoting_preserves_empty_quotes_slashes_and_shell_literals() {
        assert_eq!(quoted(""), "\"\"");
        assert_eq!(quoted("abc\\"), "\"abc\\\\\"");
        assert_eq!(quoted("a\"b"), "\"a\\\"b\"");
        assert_eq!(quoted("& | >"), "\"& | >\"");
    }
}
