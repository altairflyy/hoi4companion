# Windows x64 distribution

The ZIP contains a self-contained folder. HOI4 Companion.exe is the GUI launcher; Companion Runtime.exe is the worker, sharing _internal. It installs no system service and needs no PATH changes or administrator rights.

The bundle includes Python dependencies, Rust/Jomini, the self-contained CWTools host, Rust HUD, OpenCV, yt-dlp/EJS, FFmpeg, and Node. User data lives in %LOCALAPPDATA%/HOI4Companion; source checkouts use .local/. HOI4COMPANION_DATA_DIR selects an isolated profile. Personal data, mods, saves, credentials, and game assets are excluded.

Build prerequisites are Windows x64, Python 3.13, Rust/MSVC, and the pinned .NET SDK. Run packaging/requirements-windows.lock in a build environment, scripts/build-windows.py, then scripts/smoke-windows.py. Native components build in an isolated staging path. Release builds must not use --skip-native.

A matching v<version> tag starts build/test creation of a draft release. A source commit alone does not update published downloads. Check ZIP hashes, manifests, licenses, and smoke results before publishing. Package smoke tests are not a clean-VM, YouTube-download, or real-game HUD validation. The unsigned build may trigger SmartScreen.

Detailed historical implementation notes and recorded evidence are preserved in the
[Italian reference](it/windows-distribution.md). Those records retain their original dates and
verification limits; they are not a claim that every check was rerun for this translation.
