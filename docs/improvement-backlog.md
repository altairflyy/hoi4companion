# Improvement backlog

Shared state of the unattended improvement loop (`/improve-loop`, see
`.claude/skills/improve-loop/`). The coordinator session seeds and ranks the items,
starts one worker session per item and checks the result. Each worker session changes
only its own item. Users read this file to review a run and answer `needs-user` items.

## Run
- Branch: (none yet)
- Started: (none yet)
- Resumed: (one line per resume, if any)
- Limits: 15 items done / 8 h from Started / 2 attempts per item

<!--
Item format (ids IL-01, IL-02, ... are never reused):

### IL-01 — <title>
- Area: functionality | guides | runtime | architecture | performance | ux | harness
- Priority: <1-5, 1 highest> (impact, risk, cost in one line)
- Source: <handoff line, survey result, benchmark, screenshot, file:line>
- Acceptance: <objective check: command + expected result, or measured number before -> target>
- Status: todo | doing | done <short-hash> | needs-user: <question> | dropped: <reason>
- Attempts: 0
- Notes:
- Checked: (coordinator, after a green re-run)
-->

## Items

## Run report
