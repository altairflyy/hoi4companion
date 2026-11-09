# Windows x64 distribution

The native ZIP contains `HOI4 Companion.exe`, a Rust backend, a separate
Rust/Direct2D HUD, JavaScript/HTML/CSS resources and tools used only on demand.
It installs no service, changes no PATH and needs no administrator rights.
There is no resident Python application runtime, Python server, OpenCV worker
or browser engine bundled for the app. The user's normal browser displays the UI.

Extract the entire ZIP and open **HOI4 Companion.exe**. `--no-hud` starts the web
app alone; `--no-browser` leaves the server/HUD running without opening a tab.
`--stop` stops only the service controlled by the selected application-data
directory and closes its verified HUD window. It does not close HOI4 or Windows.

User data stays in `%LOCALAPPDATA%/HOI4Companion`; source checkouts use `.local`.
`HOI4COMPANION_DATA_DIR` selects a separate profile. Saved settings, campaign
progress, selected guide and reference designs are reused. Personal guides,
mods, saves, credentials and game assets are excluded from packages.
English is the default UI language; Italian remains selectable.

The package contains FFmpeg, standalone yt-dlp and Node for video/source jobs,
and the pinned self-contained CWTools host for requested script analysis.
Their processes are supervised only while the corresponding job needs them.
An interpreter embedded inside an optional external utility is distinct from a
resident application server. Network video acquisition and current game/HUD
coverage need their own verification; offline package checks do not prove them.

## Build and verification

Build prerequisites: Windows x64, Rust/MSVC and the existing pinned CWTools host.
Python is used by the **build/test tooling**, not by the native application.
The offline builder uses already available FFmpeg, yt-dlp and pinned Node files;
explicit tool paths can be supplied. It downloads nothing implicitly.

```powershell
py -3 scripts/build-native-windows.py
```

`--skip-native` is for verifying already compiled binaries during development.
The builder uses one temporary staging directory, an explicit resource allowlist,
checks ZIP contents and hashes, and emits a manifest and SHA-256 checksum.
The native artifact name includes `native-windows-x64`; older mixed-runtime
packages produced by `scripts/build-windows.py` are historical compatibility
artifacts and are not silently replaced.

Rust incremental snapshots are disabled in this checkout to avoid retaining a
large cache for every edit. Compiler targets and development-only evidence are
not included in the application ZIP.

Publishing a commit does not publish a release. Verify the current artifact,
manifest, startup/stop smoke tests, browser behavior and any separately claimed
live/HUD evidence before the release agent uploads a new download. An unsigned
build may be subject to Windows SmartScreen.

See [Rust migration](rust-backend-migration.md) and the dated
[native cutover report](audits/2026-11-09-native-cutover.md) for executed checks
and remaining gates. The [Italian historical reference](it/windows-distribution.md)
retains its original dates and describes the previous mixed-runtime package.
