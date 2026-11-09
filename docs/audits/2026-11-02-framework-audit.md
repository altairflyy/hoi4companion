# Framework and tooling audit — 2026-11-02

## Result

14 reproduced defects or measured inefficiencies corrected. No outstanding
reproduced regression in this audit's checks. Real-game campaign/HUD validation
remains pending: HOI4 was closed and was not started for this audit.

The review covered the reading/qualification pipeline, GUI reader lifecycle,
Python/JavaScript contracts, guide/runtime projection, production matching,
progress storage, browser selection, media workers, Rust/Jomini, the CWTools
process bridge, and source/portable launch and dependency paths. This is an
architecture and regression audit, not formal proof that every program path or
upstream library is error-free.

## Corrections

| # | Area | Reproduced problem and correction |
|---|---|---|
| 1 | Reader lifecycle | Global method caches retained closed GUI readers and PE images. Caches now belong to one reader and are cleared/released on close; borrowed process ownership is preserved. |
| 2 | Video source registry | Separate `GuideFrames` instances could lose concurrent registrations or collide on one temporary filename. In-process registry transactions are serialized and replacements use unique temporary files. |
| 3 | Frame cache | Failed extraction could leave a partial JPEG that subsequent requests accepted as cached success. Frame generation now validates JPEGs and publishes them atomically; invalid old cached frames are rebuilt. |
| 4 | Media errors | Frame timeouts escaped as raw subprocess exceptions and left partial files. They now produce a recoverable error and clean up only their own temporary output. |
| 5 | Fact qualification | An unavailable/stale nested section could inherit its observed/partial parent's qualification. Python and JavaScript now propagate nested unavailability without reviving an unavailable ancestor. Explicit observed zero is preserved. |
| 6 | Research capacity | Duplicate/invalid slot identities or more than 64 slots could inflate capacity. Both validators reject these observations as unknown while retaining legacy unindexed and zero-based inputs. |
| 7 | Project data | A missing/malformed breakthrough section could crash Python adaptation. It now remains unknown, consistent with JavaScript. |
| 8 | Production priority | An unreadable peer design was treated as a proven wrong queue order. Matching, unknown and excluded lines now remain distinct; a known wrong line still blocks and irrelevant unknown lines do not invalidate a known order. |
| 9 | CWTools process | The analysis timeout started after a potentially blocking stdin write. Input/output are drained concurrently under the deadline. A real child fixture that does not read its pipe verifies bounded termination. |
| 10 | Browser guide state | A delayed recovery GET after a rejected write could replace progress after another guide was selected. Applying the reply now rechecks the captured selection generation. |
| 11 | Source dependencies | The source install omitted yt-dlp's matching JavaScript solver. It now declares `yt-dlp[default]`; the installed 2026.10.21 version was retained and its missing default dependencies installed. |
| 12 | JavaScript runtime | Source YouTube tools did not enable Node from PATH. Source checkouts now discover it; portable builds remain restricted to their bundled Node. |
| 13 | Downloader selection | Metadata acquisition could fail without a console executable even when the Python module was installed. Metadata/video preparation now share module/override/standalone selection. Explicit `HOI4_YTDLP` remains authoritative. |
| 14 | HUD projection | Targets repeatedly scanned the full catalog, including an event-title loop for non-event panels. One projection-local lookup indexes needed references and shares the equipment subset. Duplicates/scopes remain ambiguous as before; no cross-catalog cache is retained. |

No nation-specific rules, guide rewrites, new AI prompt fields, game-file writes,
memory writes or automatic clicks were introduced. Campaign persistence and the
guide schema remain separate from these changes.

## Validation and evidence

- Python: **1,103 passed**, full `unittest discover -s tests`, using the installed
  rebuilt parser, not an override for the final run.
- JavaScript: **411 passed**, all `.cjs` suites.
- Rust parser/CWTools bridge: **23 passed** (19 unit, 2 CLI, 2 recovery).
- Rust HUD: **22 passed**.
- Release parser rebuilt and both existing `.pyd`/`.dll` entry points replaced
  after stopping only verified Companion/HUD processes; SHA-256 verified:
  `6d105be22d40a82112d852b24f32ad1ac9d55aa7ccb8946971893152361a0ab8`.
- Source deployment self-check executed the actual parser and packaged CWTools,
  verified host/rules manifests, encoded a synthetic HEVC sample, and loaded
  OpenCV 5.0.0, NumPy 2.5.2, yt-dlp 2026.10.21, EJS 0.8.0 and Node v24.15.0.
- Synthetic HUD projection benchmark: 30,003 catalog entries / 32 targets;
  median **31.66 → 15.57 ms**, with identical serialized-result SHA-256
  `54baed0b662eae4d3c20083d8c253e0a2920ccca523bcabf11ffa4fbadfe656a`.
  This is CPU projection timing, not visible HUD latency or game FPS.
- Imported Hungary guide SHA-256 remained
  `287d7d5f3188ae96dd0f70ff63eb5067994b64ff7cc7c12d964e98b6c1726f8e`.
- Progress database and previous parser binaries were backed up under `.local`.
- `git diff --check` passed for edited files. Existing Pillow `getdata()`
  deprecation warnings occur in a test fixture; they are not suite failures.

Local evidence: `.local/full-audit-final-python.log`,
`.local/full-audit-final-js.log`, `.local/full-audit-parser.log`,
`.local/full-audit-overlay.log`, `.local/full-audit-deployment-check.json`,
`.local/full-audit-activation.json`, and the `full-audit-*-before.log` regression
reproductions. Evidence paths are intentionally ignored local artifacts.

## Language and data-structure assessment

No broad language migration was justified by the reproduced problems. Python
continues to coordinate I/O, declarative rules and persistence; Rust/Jomini owns
script parsing, the bounded CWTools bridge, and the external HUD; the upstream
CWTools engine remains .NET/F#. Replacing upstream engines would add compatibility
risk without addressing these measured causes.

The changes use reader-local bounded caches, projection-local dictionaries and
sets, tri-state evidence, transactional registry updates and bounded process I/O.
Existing topological ordering and SQLite optimistic concurrency remain intact.

## Budget monitoring and remaining limits

`.local/audit-budget-2026-11-02.json` records the initial and approximately
five-minute usage checks with actual timestamps/durations. Limits are shared
across the account; consumption cannot be attributed exclusively to this task.
The maximum observed interval consumption is compared with the remaining value
of each corresponding window. Reset/missing values are not treated as zero.

The final shutdown record is `.local/full-audit-shutdown.json`. Shutdown is
requested on completion or if the specified budget threshold is met.

Remaining verification: real-game first-panel latency, visible border alignment,
and campaign progression were not revalidated here. A new portable ZIP/GitHub
release was not built or published. Source changes and local native components
are ready for the next normal startup. No Git commit, reset or branch change was
performed; unrelated pre-existing workspace edits were preserved.
