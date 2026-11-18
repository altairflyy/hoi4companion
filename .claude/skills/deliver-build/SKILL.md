---
name: deliver-build
description: Rebuild and restart HOI4 Companion after a committed change so the user gets the updated app without swapping binaries. Stops the app, waits for hoi4-* processes to exit, builds backend (and overlay when needed), checks the installed exe, starts it and waits for HTTP.
disable-model-invocation: true
---

# Deliver the updated build

Use after the change is **committed locally** (never push without asking).

```bash
node .claude/skills/deliver-build/deliver.cjs            # overlay rebuilt only if native/overlay or translations.json is newer than its exe
node .claude/skills/deliver-build/deliver.cjs --overlay  # force the overlay build
node .claude/skills/deliver-build/deliver.cjs --no-start # build only
```

Run it with a long timeout (up to 10 min) or in the background. It:

1. runs `scripts/avvia-tutto.ps1 -Stop` and waits until no `hoi4-backend`, `hoi4-desktop` or `hoi4-overlay` process remains (otherwise the copy into `_native` is skipped);
2. runs `scripts/build-backend.ps1` and fails if the installed `hoi4-backend.exe` differs from the fresh build;
3. runs `scripts/build-overlay.ps1` when needed: `translations.json` is compiled into both backend and overlay with `include_str!`;
4. runs `avvia-tutto.ps1` with stdio detached (a piped call hangs on the resident host) and waits for `http://127.0.0.1:8766/events`.

Web JS/HTML/CSS is served live: a web-only change needs no rebuild, just a page reload.

Afterwards verify the change live (API or built-in browser at `http://127.0.0.1:8766/suite.html`). Never control, foreground or close HOI4.
