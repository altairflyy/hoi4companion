<div align="center">

# 🎖️ HOI4 Companion

### A local companion for Hearts of Iron IV: interactive national focus and technology trees, AI-assisted walkthroughs, live memory monitoring, and external in-game HUD overlay.

[![Release](https://img.shields.io/github/v/release/altairflyy/hoi4companion?logo=github&label=Latest%20Release&color=0969da&style=flat-square)](https://github.com/altairflyy/hoi4companion/releases)
[![Downloads](https://img.shields.io/github/downloads/altairflyy/hoi4companion/total?color=blue&label=Downloads&logo=github&style=flat-square)](https://github.com/altairflyy/hoi4companion/releases)
[![Build Status](https://img.shields.io/github/actions/workflow/status/altairflyy/hoi4companion/windows-release.yml?branch=main&label=CI%20Build&logo=githubactions&logoColor=white&style=flat-square)](https://github.com/altairflyy/hoi4companion/actions)
[![Platform](https://img.shields.io/badge/Platform-Windows%20x64-0078D6?logo=windows&logoColor=white&style=flat-square)](https://github.com/altairflyy/hoi4companion/releases/latest)
[![HOI4 Compatibility](https://img.shields.io/badge/HOI4%20Live-verified%20build%20profiles-critical?logo=paradoxinteractive&logoColor=white&style=flat-square)](#what-it-does)
[![Runtime](https://img.shields.io/badge/Runtime-Rust%20backend%20%26%20HUD-orange?logo=rust&style=flat-square)](docs/rust-backend-migration.md)
[![Tech Stack](https://img.shields.io/badge/Stack-Rust%20%7C%20Direct2D%20%7C%20.NET%209-orange?style=flat-square)](#what-it-does)
[![License](https://img.shields.io/badge/License-MIT-2ea44f?style=flat-square)](LICENSE)

<br/>

<p align="center">
  <a href="https://github.com/altairflyy/hoi4companion/releases/latest">
    <img src="https://img.shields.io/badge/DOWNLOAD_NOW-Windows_x64_(Portable_ZIP)-2ea44f?style=for-the-badge&logo=windows&logoColor=white" alt="Download Windows" height="42" />
  </a>
  &nbsp;&nbsp;
  <a href="https://github.com/altairflyy/hoi4companion/releases">
    <img src="https://img.shields.io/badge/GitHub-ALL_RELEASES-0969da?style=for-the-badge&logo=github&logoColor=white" alt="Releases" height="42" />
  </a>
  &nbsp;&nbsp;
  <a href="docs/README.md">
    <img src="https://img.shields.io/badge/DOCS-DOCUMENTATION-6f42c1?style=for-the-badge&logo=gitbook&logoColor=white" alt="Docs" height="42" />
  </a>
  &nbsp;&nbsp;
  <a href="docs/it/README.md">
    <img src="https://img.shields.io/badge/LINGUA-ITALIANO-238636?style=for-the-badge&logo=readme&logoColor=white" alt="Guida in Italiano" height="42" />
  </a>
</p>

*100% Free & Open Source • Zero invasive installers • Full runtime bundled*

---

</div>

[Download Windows builds](https://github.com/altairflyy/hoi4companion/releases)
· [Windows distribution](docs/windows-distribution.md)
· [Documentation](docs/README.md)
· [Italian reference](docs/it/README.md)

## What it does

- Reconstructs focus and research trees from your installed game and active mods,
  using the Rust/Jomini parser, game layouts, sprites, and localization.
- Imports guides produced by an external AI from videos or web sources. The
  application exports the instructions and thematic ID catalogs, resolves names,
  validates dependencies and evidence, and offers an editable preview.
- Prepares YouTube material for analysis: video clips of up to five minutes with
  ten-second overlap and a 150 MB maximum per clip, a timing manifest, and audio.
- Evaluates the guide deterministically. Construction can be started, in progress,
  or completed; missing observations never imply success. Campaign progression is
  separate from the guide definition.
- Reads supported live data without requiring saves. Qualified facts include
  research, focuses, construction, production, and supported political/project data.
  Availability varies by game build and by field; unknown values remain unknown.
- Uses an external Rust/Direct2D HUD for supported screens, including focus and
  research targets and supported event/action navigation. App and HUD share the
  active guide. Resource checks can withhold a recommendation when requirements
  are insufficient or not verifiable.
- Provides optional CWTools script analysis. Static diagnostics do not certify a
  campaign's state, and partial coverage is reported explicitly.

**This is a read-only assistant, not an automated player.** It does not click,
modify game files, inject code, install mods, or write process memory. It is not a
Paradox Interactive product and does not redistribute HOI4 assets. Live reading
and visual recognition have validation limits; neither zero latency nor zero
performance overhead is guaranteed.

## What's new in v0.2.0 (Native Backend & Engine Cutover)

Version 0.2.0 marks the transition to a **100% standalone native Rust architecture** for the Companion backend and runtime services.

### Why these changes were made
- **Eliminate resident Python runtime overhead:** The desktop Windows distribution no longer requires a background Python interpreter, Flask server, or resident worker processes. The native binary `HOI4 Companion.exe` serves HTTP/SSE, runs the deterministic guide decision engine (`native/guide-core`), resolves catalog queries (`native/catalog-core`), and coordinates with the Direct2D HUD directly.
- **Strict process containment & leak prevention:** External tools (FFmpeg, yt-dlp, Node.js, and CWTools) now run inside Windows Job Objects (`native/tools`) with automatic kill-on-close, output growth bounds, and zero orphaned background processes.
- **Provincial fortifications & frontier tracking:** Walkthroughs now support land (`bunker`) and coastal (`coastal_bunker`) forts per province, province sets, or entire national borders (e.g., `1 fort in each tile at YUG border`), combining static map topology with live controllers.
- **Differential parity assurance:** Over 1,380 Python tests serve as independent behavioral oracles alongside 451 JavaScript tests, ensuring complete determinism between the new Rust engine and reference specifications.

## Start on Windows

1. Download the Windows x64 ZIP from **Releases**, rather than the source archive.
2. Extract the entire archive into a writable folder.
3. Launch **`HOI4 Companion.exe`**. The native build runs a Rust backend and HUD;
   it does not require an installed Python interpreter.
4. Check the detected game installation and HOI4 documents directory in the app.
5. Open **Import guide**, prepare the AI attachments, and import the returned JSON.
6. Review unresolved associations and source issues, then install/select the guide.
7. Load the matching campaign in HOI4. Bring the game to the foreground for HUD
   highlights. The guide still works as a reference when live data is unavailable.

English is the default **interface language**. Use the **Interface language**
selector in the navigation bar to choose Italian. This preference is separate
from **Game content language**, which controls installed game/mod names and
descriptions. Existing imported guides and quoted evidence retain their language.
Restart the HUD after changing the interface language to update its controls.

For the source checkout, build the native components and use
**`START COMPANION AND HUD.bat`**. This is the single launcher in the project root.
See [START HERE.txt](START%20HERE.txt). Closing the browser does not stop the server;
close the HUD through its control window. Web-only portable startup:

```powershell
& '.\HOI4 Companion.exe' --no-hud
```

Stop the native server and its HUD with `HOI4 Companion.exe --stop`; HOI4 remains
open. Source checkouts use `powershell -NoProfile -File scripts/avvia-tutto.ps1 -Stop`.
Python compatibility modules remain available for development and independent
regression tests; the native launcher does not start them.

The native package is built separately from older mixed-runtime releases. A
source commit does not replace an existing GitHub download; see the distribution
manifest and the [migration verification record](docs/rust-backend-migration.md).

The default local address is `http://127.0.0.1:8766/walkthrough`. Importing another
guide remains available through `/importer`. No personal guide, mod list, campaign,
or machine-specific path is included in the distribution.

## Create a guide with an external AI

1. In **Import guide**, select the relevant country and export the creation prompt
   and necessary thematic catalogs.
2. Supply the source video/article, prepared clips and timing manifest, prompt,
   and relevant catalogs to your AI. No API key is needed for this workflow.
3. Ask for one complete `.hoi4-guide.json` using `hoi4-guide-authoring` v1.
4. Upload the JSON, inspect the preview, and resolve genuine ambiguities.
5. Install the guide and select it in the app/HUD.

The application compiles the simpler authoring format into Guide Schema v1;
existing import formats remain supported. It does not invent strategy or certify
that an AI reviewed the entire source. Exact evidence, timestamps, unresolved
choices, and source coverage remain reviewable. An optional integrated AI adapter
is retained, but is not required for external-AI guide creation.

## Requirements and compatibility

- Windows x64 for native capture, process observation, and HUD rendering.
- A local HOI4 installation for real game data and assets.
- Live process layouts are qualified for specific builds. File parsing is broader
  than live memory support. An unqualified layout must not be treated as valid
  simply because the version string looks similar.
- Game version, DLCs, active mods, localization, and the guide's selected route can
  affect available entities. National catalog filtering is static and conservative.
- The HUD needs a supported visible screen. Unknown identity, missing resources,
  stale observations, or incompatible context can intentionally suppress highlights.

Portable user data lives in `%LOCALAPPDATA%\HOI4Companion`; source checkouts retain
`.local/`. Set `HOI4COMPANION_DATA_DIR` for an isolated profile. Never distribute
that data directory. Back it up to retain installed guides and campaign progress.

## Development

The standalone Rust backend owns guide decisions, catalog resolution, import
orchestration, state normalization, qualified game-memory decoding, HTTP/SSE and
SQLite progress. It supervises external tools only while jobs run. Rust/Jomini
parses scripts; the self-contained CWTools host handles requested analysis.
The separate Rust/Direct2D HUD uses the same active guide and native projection.
Browser JavaScript presents the interface. Python remains for development/build
tools and independent regression comparisons, outside the default application.
See the [migration and verification report](docs/rust-backend-migration.md).

Prerequisites: Python 3.11+, Node.js for JavaScript tests, Rust/MSVC for native
builds, and the project's pinned .NET SDK for CWTools. Windows release builds use
the locked packaging environment documented in [distribution](docs/windows-distribution.md).

```powershell
py -3 -m pip install -e ".[overlay]"
powershell -NoProfile -File scripts/build-parser.ps1
powershell -NoProfile -File scripts/build-observer.ps1
powershell -NoProfile -File scripts/build-guide-core.ps1
powershell -NoProfile -File scripts/build-catalog-core.ps1
powershell -NoProfile -File scripts/build-tools.ps1
powershell -NoProfile -File scripts/build-storage.ps1
powershell -NoProfile -File scripts/build-backend.ps1
powershell -NoProfile -File scripts/build-overlay.ps1
py -3 scripts/build-cwtools.py
powershell -NoProfile -File scripts/avvia-tutto.ps1
```

Source YouTube jobs need standalone yt-dlp and Node executables. `HOI4_YTDLP`
selects the downloader explicitly. Native portable builds include those tools;
they are started on demand, independently of the application runtime.

Select game-content and interface languages separately in the application's
settings. The developer `hoi4-backend.exe` also accepts `--language` and
`--ui-language` overrides. Do not run a second instance on an occupied port;
use a separate port/profile for isolated development.

```powershell
$env:PYTHONPATH = 'src'
py -3 -m unittest discover -s tests -p 'test_*.py'
node --test tests/*.cjs
```

Native tests and Windows package smoke tests are described in the corresponding
documentation. Passing unit tests is not proof of real-game HUD alignment.

## Contributing and distribution

Keep changes nation-independent. Preserve observed/calculated/unknown evidence,
guide compatibility, campaign isolation, and the distinction between start and
completion. Do not add an autonomous strategy engine or infer success from clicks.

Never commit `.local/`, saves, personal settings, credentials, video downloads,
game/mod assets, or generated runtime caches. Native build artifacts are produced
by the release workflow. A source commit does **not** update an existing GitHub
release automatically; see [Windows distribution](docs/windows-distribution.md).

The project is MIT licensed; bundled components retain their own licenses and
notices, including the distributed FFmpeg build. See [LICENSE](LICENSE).
