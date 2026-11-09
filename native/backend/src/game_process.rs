//! Owned read-only game handles. No write, VM-operation, debugger, thread
//! injection, privilege escalation or arbitrary PID-selection interface.
use crate::{pe::number, profile};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::ffi::OsString;
use std::ffi::c_char;
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicU64, Ordering},
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, FILETIME, HANDLE},
        System::{
            Diagnostics::{Debug::ReadProcessMemory, ToolHelp::*},
            Threading::*,
        },
    },
    core::PWSTR,
};
type Result<T> = std::result::Result<T, String>;
struct Owned(HANDLE);
// Windows handles can be borrowed for read-only operations concurrently. Their
// lifetime belongs to this RAII object retained by the registry/request Arc.
unsafe impl Send for Owned {}
unsafe impl Sync for Owned {}
impl Drop for Owned {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
pub(crate) struct Game {
    process: Owned,
    pub(crate) profile: Arc<profile::Profile>,
    pub(crate) pid: u32,
    pub(crate) base: u64,
    pub(crate) created: u64,
}
static GAMES: OnceLock<Mutex<HashMap<u64, Arc<Game>>>> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
fn registry() -> &'static Mutex<HashMap<u64, Arc<Game>>> {
    GAMES.get_or_init(|| Mutex::new(HashMap::new()))
}
fn text(raw: &[u16]) -> OsString {
    OsString::from_wide(&raw[..raw.iter().position(|c| *c == 0).unwrap_or(raw.len())])
}
fn canonical(path: &Path) -> Result<PathBuf> {
    std::fs::canonicalize(path).map_err(|_| "Identità processo non verificata".into())
}
impl Game {
    pub(crate) fn open(profile: Arc<profile::Profile>) -> Result<Self> {
        let snapshot = Owned(
            unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }
                .map_err(|_| "Elenco processi non disponibile")?,
        );
        let mut row = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut more = unsafe { Process32FirstW(snapshot.0, &mut row) }.is_ok();
        let mut found = vec![];
        while more {
            if text(&row.szExeFile)
                .to_string_lossy()
                .eq_ignore_ascii_case("hoi4.exe")
            {
                found.push(row.th32ProcessID)
            }
            more = unsafe { Process32NextW(snapshot.0, &mut row) }.is_ok();
        }
        if found.len() != 1 {
            return Err(if found.is_empty() {
                "Gioco non avviato"
            } else {
                "Più processi HOI4: selezione ambigua"
            }
            .into());
        }
        Self::open_pid(profile, found[0])
    }
    fn open_pid(profile: Arc<profile::Profile>, pid: u32) -> Result<Self> {
        let process = Owned(
            unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid) }
                .map_err(|_| "Accesso in sola lettura al processo negato")?,
        );
        let mut game = Self {
            process,
            profile,
            pid,
            base: 0,
            created: 0,
        };
        game.created = game.identity()?;
        let snapshot = Owned(
            unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid) }
                .map_err(|_| "Modulo HOI4 non disponibile")?,
        );
        let mut row = MODULEENTRY32W {
            dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32,
            ..Default::default()
        };
        let mut more = unsafe { Module32FirstW(snapshot.0, &mut row) }.is_ok();
        while more {
            if canonical(Path::new(&text(&row.szExePath))).is_ok_and(|p| p == game.profile.path) {
                game.base = row.modBaseAddr as u64;
                if row.modBaseSize as u64
                    != game.profile.metadata()["image_size"]
                        .as_u64()
                        .ok_or("Dimensione profilo non valida")?
                {
                    return Err("Dimensione modulo differente dal profilo".into());
                }
                break;
            }
            more = unsafe { Module32NextW(snapshot.0, &mut row) }.is_ok();
        }
        if game.base == 0 {
            return Err("Percorso modulo diverso da quello configurato".into());
        }
        let header = game.read(game.base, 4096)?;
        let pe = number(&header, 0x3c, 4)? as usize;
        if header.get(pe..pe + 4) != Some(b"PE\0\0".as_slice())
            || number(&header, pe + 8, 4)?
                != game.profile.metadata()["timestamp"]
                    .as_u64()
                    .ok_or("Timestamp profilo non valido")?
        {
            return Err("Identità del modulo caricato non verificata".into());
        }
        if game.identity()? != game.created {
            return Err("Sessione di processo cambiata".into());
        }
        Ok(game)
    }
    pub(crate) fn identity(&self) -> Result<u64> {
        let mut path = vec![0u16; 32768];
        let mut length = path.len() as u32;
        unsafe {
            QueryFullProcessImageNameW(
                self.process.0,
                PROCESS_NAME_WIN32,
                PWSTR(path.as_mut_ptr()),
                &mut length,
            )
        }
        .map_err(|_| "Identità processo non verificata")?;
        if canonical(Path::new(&OsString::from_wide(&path[..length as usize])))?
            != self.profile.path
        {
            return Err("Identità processo non verificata".into());
        }
        let (mut created, mut exited, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        unsafe {
            GetProcessTimes(
                self.process.0,
                &mut created,
                &mut exited,
                &mut kernel,
                &mut user,
            )
        }
        .map_err(|_| "Processo terminato o non verificabile")?;
        if exited.dwLowDateTime != 0 || exited.dwHighDateTime != 0 {
            return Err("Processo terminato o non verificabile".into());
        }
        Ok(created.dwLowDateTime as u64 | ((created.dwHighDateTime as u64) << 32))
    }
    pub(crate) fn read(&self, address: u64, size: usize) -> Result<Vec<u8>> {
        if !(0x10000..0x7fff_ffff_ffff).contains(&address)
            || !(1..=65536).contains(&size)
            || address
                .checked_add(size as u64)
                .is_none_or(|a| a > 0x7fff_ffff_ffff)
        {
            return Err("Richiesta memoria fuori limite".into());
        }
        let mut buffer = vec![0; size];
        let mut count = 0;
        if unsafe {
            ReadProcessMemory(
                self.process.0,
                address as *const _,
                buffer.as_mut_ptr().cast(),
                size,
                Some(&mut count),
            )
        }
        .is_err()
            || count != size
        {
            return Err("Memoria del gioco non leggibile".into());
        }
        Ok(buffer)
    }
    fn info(&self) -> Value {
        json!({"pid":self.pid,"base":self.base,"created":self.created,"handle":self.process.0.0 as usize as u64})
    }
    pub(crate) fn raw_handle(&self) -> HANDLE {
        self.process.0
    }
}
pub(crate) fn execute(op: &str, args: &Value) -> Result<Value> {
    if op == "game_open" {
        let profile = profile::get(args["profile"].as_u64().ok_or("Profilo non valido")?)?;
        let game = Game::open(profile)?;
        let info = game.info();
        let mut games = registry()
            .lock()
            .map_err(|_| "Registro processi non disponibile")?;
        if games.len() >= 64 {
            return Err("Limite lettori nativi raggiunto".into());
        }
        let handle = NEXT.fetch_add(1, Ordering::Relaxed);
        games.insert(handle, Arc::new(game));
        return Ok(json!({"handle":handle,"info":info}));
    }
    let handle = args["handle"].as_u64().ok_or("Lettore nativo non valido")?;
    if op == "game_release" {
        return Ok(json!(
            registry()
                .lock()
                .map_err(|_| "Registro processi non disponibile")?
                .remove(&handle)
                .is_some()
        ));
    }
    let game = registry()
        .lock()
        .map_err(|_| "Registro processi non disponibile")?
        .get(&handle)
        .cloned()
        .ok_or("Lettore nativo non disponibile")?;
    match op {
        "game_identity" => game.identity().map(|id| json!(id)),
        "game_read" => {
            let bytes = game.read(
                args["address"]
                    .as_u64()
                    .ok_or("Richiesta memoria fuori limite")?,
                args["size"]
                    .as_u64()
                    .filter(|n| *n <= 65536)
                    .ok_or("Richiesta memoria fuori limite")? as usize,
            )?;
            Ok(json!(
                bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
            ))
        }
        _ => Err("Operazione processo non valida".into()),
    }
}

/// # Safety
/// packet addresses length readable bytes for this call. The owned game Arc
/// retains the borrowed read-only handle through both verification passes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoi4_backend_game_compare(
    handle: u64,
    packet: *const u8,
    length: usize,
    passes: u32,
) -> *mut c_char {
    crate::reply(
        std::panic::catch_unwind(|| {
            let game = registry()
                .lock()
                .map_err(|_| "Registro processi non disponibile")?
                .get(&handle)
                .cloned()
                .ok_or("Lettore nativo non disponibile")?;
            match unsafe {
                hoi4companion_observer::hoi4_observer_compare(
                    game.process.0.0,
                    packet,
                    length,
                    passes,
                )
            } {
                0 => Ok(json!(false)),
                1 => Ok(json!(true)),
                -1 => Err("Native observer: invalid packet or bounds".into()),
                -2 => Err("Native observer: read failed or partial".into()),
                _ => Err("Native observer: internal failure".into()),
            }
        })
        .unwrap_or_else(|_| Err("Verifica memoria rifiutata".into())),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owned_process_path_module_identity_read_and_witnesses() {
        let profile = profile::Profile::for_test(&std::env::current_exe().unwrap());
        let game = Game::open_pid(Arc::new(profile), unsafe { GetCurrentProcessId() }).unwrap();
        assert_eq!(game.identity().unwrap(), game.created);
        let mut buffer = *b"owned-reader";
        let address = buffer.as_ptr() as u64;
        assert_eq!(game.read(address, buffer.len()).unwrap(), buffer);
        let mut packet = Vec::new();
        packet.extend(address.to_le_bytes());
        packet.extend((buffer.len() as u32).to_le_bytes());
        packet.extend(1u32.to_le_bytes());
        packet.extend(0u32.to_le_bytes());
        packet.extend((buffer.len() as u32).to_le_bytes());
        packet.extend(255u32.to_le_bytes());
        packet.extend(buffer);
        assert_eq!(
            unsafe {
                hoi4companion_observer::hoi4_observer_compare(
                    game.process.0.0,
                    packet.as_ptr(),
                    packet.len(),
                    2,
                )
            },
            0
        );
        buffer[0] = b'X';
        std::hint::black_box(&buffer);
        assert_eq!(
            unsafe {
                hoi4companion_observer::hoi4_observer_compare(
                    game.process.0.0,
                    packet.as_ptr(),
                    packet.len(),
                    2,
                )
            },
            1
        );
    }
}
