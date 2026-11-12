# Desktop suite — scope and acceptance

The suite is an additive presentation shell over the existing Rust API, guide
runtime, campaign store and game renderers. The classic routes remain available.
No guide, save, acquisition offset or game order is changed by this feature.

## Design reference

Read-only REA analysis of the installed Blitz 3.0.22 `resources/app.asar`:
SHA-256 `6eeaec3671f786b2f73369b61a51fde2797fae1e266bde0bf715b40af808ed2b`.
Parent evidence `ev_6bb6d93c53a7d9fb1c3b49d7c721591f89c78a68b2faed0de87ae63654542c29`;
tray evidence `ev_8fc7fdac8562ae8e04d1651b2f3281ed304a03d23eb2933a297839ef20347c74`.
Source boundaries: window creation/lifecycle, tray, overlay listeners and UI copy.
Observed static code defines a frameless primary window, tray restoration,
window-bound persistence, dark `#0c0e13` chrome, and distinct overlay controls.
The rendered main interface is loaded separately; exact live navigation/layout
and runtime reachability were not verified. No proprietary source, icons or fonts
are copied. Blitz was not executed; user databases/authentication were not read.

Adapted behavior: one persistent navigation shell; guide/ZIP library first;
qualified campaign context; original focus/technology/operational screens;
discreet game highlights with secondary tray controls. Keep Rust and JavaScript,
without adding Electron or an AI strategy engine.

## Acceptance

- `/suite.html` and classic routes work side by side; no migration of user data.
- Embedded views reuse one qualified live stream and release subscriptions.
- All completion/readiness statements come from the existing Rust projection.
- Wrong-country, stale and missing evidence never become current dashboard data.
- Selecting a guide uses the existing active-guide API shared with the HUD.
- Guide and template ZIP slots remain in the existing importer.
- The native launcher has browser fallback; the HUD retains legacy controls.
- The installed Edge engine runs in guest app mode, with sync disabled and an
  app-specific data directory. No browser login or personal profile is required.
- Verify navigation, empty/live states, language, campaign/guide changes, keyboard
  access and desktop/tray lifecycle without giving game input or changing saves.

UI craft uses the Unslop workflow by Damir Sokolovsky (CC BY 4.0),
<https://heydamir.design/unslop>.

## Verification — 2026-11-11

- PASS: 577 JavaScript tests; 2 launcher and 29 HUD Rust tests; both release
  executables built and installed. `git diff --check` reports no whitespace errors.
- PASS: real guest app window, dashboard with unavailable values hidden, searchable
  48-guide library, separate guide/template ZIP inputs, existing collection,
  embedded Austrian focus tree (249 nodes), settings dialog and return navigation.
  Settings were closed without saving; no guide was imported during validation.
- PASS: backend retained PID 7868; discreet HUD reports `controls_in_tray: true`.
  At the final API check, active guide and HUD both used `word-d399143789691798`.
- NOT VERIFIED: tray mouse actions, full keyboard-only/responsive accessibility,
  and visual in-game highlight alignment. HOI4 was not controlled for this task.
- Existing focus-view warnings for unclosed blocks in `SOV.txt` and
  `switzerland.txt` remain visible; this UI change does not repair those catalogs.
- Edge app mode uses the installed browser engine and its standard window
  titlebar. It is not a new custom native WebView host or a copy of Blitz.

Installed SHA-256:

- Launcher: `712be9fdd25e85c08fa5e1b5ae60cc31ce4afd5010ec29ecd1228ccbdc41343d`
- HUD: `c4e7b4545619261802d2d3f268c80c5a511c8bf860eee974e7cb71a9adfc3722`

Recovery: `--classic` opens the previous workspace, `--browser` uses the default
browser, and `--legacy-hud` restores the original HUD controls window. Right-click
the notification-area HUD icon to access those controls in discreet mode.

Validation cleanup: automatic review rejected deletion (`blocked by policy`).
The owned temporary directory was moved to ignored
`scarto/desktop-suite-e2fcc462` (3,685,121 bytes including rollback executables);
the original temporary root no longer exists. The dedicated Edge profile is
retained because the desktop launcher uses it.

## HUD responsiveness correction — 2026-11-12

The suite must not change highlight timing or appearance. Background/embedded
dashboard projections are now suppressed, and reader-only timestamps do not
trigger another projection. The native display path consumes an epoch-guarded
publication prepared by the existing reader instead of acquiring application
and catalog locks on each frame. Invalidation and the existing two-second state
budget remain; native GUI session checks and the 750 ms geometry budget remain.

PASS: 579 JavaScript tests. Two isolated Rust tests executed the exact production
`overlay_latest`, `current_hud_delivery` and `recent_delivery` source, checking
application-lock independence, invalidation and expiry. 10,000 lookups took
2.6591 ms in that isolated test; this is not an in-game or end-to-end measurement.
The previous real native requests averaged 738.7 ms in a 12-request observation.
NOT VERIFIED: post-correction in-game latency and visible flicker; HOI4 was closed
at installation. No overlay renderer, highlight lifetime or color was changed
by this correction. The final release build completed after interruption and
was installed on 2026-11-12; the earlier interrupted attempt had left the
intermediate build installed.

Installed backend SHA-256:
`ebac89d0c8ba0565a7a23ea3652b4798c81520436f2040025e951c91a841bfb5`.
Installed DLL SHA-256:
`830b33ea65fb326f8ee89be56b61481a3a85da7f1e0503503d733f524b7a65c0`.

## Startup correction — 2026-11-12

The source BAT now starts the native GUI launcher directly and exits. The manual
PowerShell helper waits only for that launcher, rather than its entire persistent
server/HUD/browser process tree. Legacy switches and quoted paths are retained.
Cold starts show a native preparation notice; errors retain the native dialog.

Static prepared guides use a bounded, checksum-validated, atomically replaced
cache under `.local/sprite-cache/compiled-guides-v1`. Its generation includes the
actual executing module hash, source catalogs/settings, vocabulary and reference
manifest; each entry additionally matches the complete source guide. Missing,
corrupt, stale or unwritable cache entries compile normally. No live facts,
campaign progress or runtime completion evidence are stored in this cache.

Verified on the installed application with 48 guides:

- Cold backend readiness: 111.001 s; cached backend readiness: 9.075 s.
- Complete cached PowerShell launch: 13.171 s, with server and HUD still running
  when the helper returned. All 48 cache-file timestamps/checksums were unchanged.
- Real BAT command process exited in 146 ms. Isolated delayed-launcher fixtures
  also verify immediate exit, legacy/combined options and paths with spaces.
- 579 JavaScript tests, 87 backend Rust tests and 3 desktop Rust tests passed.
- Active guide remained `word-d399143789691798`. Game files and memory were not
  modified. First startup after an invalidating update may still take longer.

The pre-existing stuck BAT/PowerShell wrapper was closed after verifying its
exact script path and parent/child IDs. Backend PID 19952 and HUD PID 15180
remained running. The prepared-guide cache occupies 9,520,890 bytes for 48 guides;
it is application data retained for subsequent startups, not a test artifact.
