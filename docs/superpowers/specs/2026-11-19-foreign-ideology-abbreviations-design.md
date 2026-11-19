# Live diplomacy: foreign ideology and guide country abbreviations

Status: approved 2026-11-19. Two steps, one session each (user runs /clear between them).

## Intent
Guide steps that act on other countries should complete automatically from live state.
Trigger case: ROMANIA (Fascist) "The Glory of Romania is Eternal!", step 8
`ASAP: Attache to NSPA (100PP)` stays "Non interpretata" (manual).

## Findings (verified 2026-11-19)
- Live already reads diplomatic relations with direction (`native/live/src/country_foreign.rs`), including `attache`.
- Rust already compiles `send_attache` to `diplomacy.outgoing contains attache:<TAG>` (`native/guides/src/guide_prepare.rs:228`).
- The step fails in text recognition (`native/guides/src/command_families.rs:32` `foreign`):
  the preposition regex lacks `to`; `send_attache` aliases (`src/hoi4companion/foreign-tasks-v1.json:81`) lack plain `attache`;
  `NSPA` is an author shorthand, not a tag or localized name (`native/catalog-core/src/lib.rs:685` `resolve_country`).
- "Nationalist Spain" is not a fixed tag. Game `localisation/english/countries_l_english.yml`:
  `SPA_fascism`/`SPA_neutrality`, `SPR_fascism`, `SPB_fascism` are all "Nationalist Spain"; `SPA_communism` is "Republican Spain".
- Live reads no ruling ideology today, not even for the played country.

## Approaches
- A static abbreviation table `NSPA -> SPA`: cheap, but wrong on non-historical paths (false completion if SPA goes communist).
- Ideology-aware resolution (chosen): read the ruling ideology of every country live, resolve abbreviations by the
  name the game would show for that ideology. Unknown ideology keeps the step unverifiable.

## Step 1: live ruling ideology of all countries
- Qualify a read-only memory read of each country's ruling party ideology on the qualified build only
  (`live-profiles-v1.json`, `live-abi-v1.json`). Needs HOI4 running; never write memory or click.
- Expose it as a Rust fact (e.g. `diplomacy.ideologies` as `TAG:ideology` entries) with `unknown` when not observed.
- Tests: `native/live/src/country_tests.rs` synthetic memory, plus a live check by the user.

## Step 2: abbreviations and "Attache to"
- Add `to` to the `foreign` preposition regex and `attache`/`attaché` to `send_attache` aliases.
- Add a guide-abbreviation table in a JSON build input, entries like
  `NSPA: {name: "Nationalist Spain", candidates: [SPA, SPR, SPB]}` (and RSPA for "Republican Spain").
- Completion: attache to the candidate whose observed ideology yields that localized name; if ideology unobserved
  for the relevant candidates, the step stays unverifiable. Never complete on a guess.
- Test: Rust case with the exact text `ASAP: Attache to NSPA (100PP)`; presentation fixture regenerated if affected.

## Non-goals
- No new HUD buttons for diplomacy. No change to other relation actions beyond the alias/preposition fix.
