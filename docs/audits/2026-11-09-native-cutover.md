# Native runtime cutover — 2026-11-09

The default source launcher starts the standalone Rust backend and the separate
Rust/Direct2D HUD. Browser JavaScript handles presentation. The native ZIP has
no resident Python application runtime or HUD worker. Development facades,
scripts and independent Python comparison implementations remain in the repo.

## Implemented and checked

- Direct native HUD protocol, bounded raw BGRA images, geometry cache
  invalidation, guide/source/campaign identity and visibility witnesses.
  App and HUD use the same application projection and active guide.
- Native startup, installation/documents discovery, saved configuration reuse,
  backend lifecycle capability and adjacent native libraries. No resident
  compatibility server fallback.
- Native package allowlist, on-demand FFmpeg/yt-dlp/Node/CWTools, manifest
  checksums and clean-install smoke test. GitHub workflow targets this package;
  the workflow itself was not executed remotely.
- Fragmented HTTP regression: Windows accepted sockets must be blocking.
- HUD diagnostics in the state feed. Missing/stale files and different
  guide/server identities never become current observations.
- Native Jomini CLI export, Windows-1252, duplicate-key policies and exclusive
  output creation while preserving the input script.
- Background/paused HUD status retains control-window identity, allowing the
  native stop command to close the verified owned HUD as well as the backend.
- Cargo cleanup on verified project-local paths, with incremental snapshots
  disabled. Installed executables, user data and required tools are preserved.

## Executed tests

| Check | Result |
| --- | --- |
| Full Python regression with independent guide/catalog and HUD projection comparison | 1,384 passed |
| JavaScript | 451 passed, none skipped |
| Optimized Rust backend unit tests | 12 passed |
| Rust HUD unit tests, including inactive-window lifecycle identity | 25 passed |
| Independent HUD image fixtures | 12 passed |
| All-target backend Clippy, warnings denied | Passed |
| Windows package smoke using actual native executables | Passed in isolated data |
| Installed guide/settings/selection hashes | Preserved |
| SQLite progress table versus pre-cutover backup | Equal |

Package smoke checks manifest files, empty personal-guide inventory, six page
routes, guide preview/install/selection, Jomini export, restart persistence,
native HUD binary-frame protocol/cache, partial demo analysis and owned stop.
It does not prove that an actual game button was rendered or aligned.

Previously documented archive comparisons cover 65 documents and 61 country
guides: 47 ready, 14 unavailable due to DLC. Missing DLC is not an import error.
These are static/import checks, not real campaign proof.

## Unverified boundaries

1. The newly installed game executable has SHA-256
   `bccc62909b6a1ec2c1d5a46ef18608e4d0c7cdc89ad341b7402a23c40f7a8037`.
   Core/GUI candidate offsets have static evidence in a compact local report.
   Country sections and production GUI remain unqualified. No new live profile
   was admitted: the API explicitly reports unsupported acquisition.
2. Current browser/game/HUD visual verification remains incomplete. The in-app
   browser is disabled and Windows Computer Use stopped because it could not
   verify the browser URL. No further UI inputs were attempted.
3. YouTube download on the final pipeline and paid provider extraction were not
   repeated. Real local FFmpeg jobs/source preservation were checked separately;
   no network success or hardware speedup is inferred.
4. Automatic approval review rejected generic cleanup of residual test browser
   profiles/old environments despite user authorization. These leftovers remain
   separate from Cargo's successful cleanup.

Local evidence records final package SHA/size, cleanup bytes and native process
identity. The PC stays on. No game files/memory, imported guide definition,
git commit or published release was changed.
