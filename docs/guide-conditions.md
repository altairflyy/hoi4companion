# Guide timing conditions

The importer and the app/HUD runtime projection compile supported timing labels
to existing Guide Schema v1 predicates. There is no strategy generator, extra
rule engine, native reader or manual confirmation flow in this compiler.

| Source instruction | Predicate |
| --- | --- |
| `At 200PP` | Qualified political power is at least 200. |
| `At 60AE`, `Once 20 NE`, `When 50 AFE` | Army, navy or air experience meets the threshold. A resolved spirit's explicit currency disambiguates generic AE shorthand. |
| `On "Institute Price Controls"` | The uniquely resolved focus is active or completed. |
| `After "Institute Price Controls"`, `On ... done` | That focus is completed. |
| `Before picking ...` | The focus is neither active nor completed; both observations must be qualified to prove absence. |
| `After Basic Machine Tools research` | The uniquely resolved technology is completed. |
| `After the war` | The existing campaign history has observed war and now observes peace. Initial peace is insufficient. |

Whole supported clauses may be joined with `and` or `or`; a semicolon joins
conditions with AND. Every clause must be understood. Mixed AND/OR without
explicit grouping is not guessed. Quotes protect entity names containing these
words. Unknown qualifiers never disappear from the condition.

Resolution reuses the installed identity index, English/local aliases, nation
and focus-tree scope. Ambiguous focus/research names remain unresolved. A named
technology does not imply a particular next UP level. Calendar annotations,
military readiness and diplomatic outcomes are not inferred from name matches.

Only guides using the explicit dependency policy receive this adaptation.
Existing authored `when` predicates, dependencies (including started versus
completed), action completion and original evidence remain unchanged. Old exact
generated timing placeholders, including split source rows, can be upgraded.
Imported definitions and campaign progress are not rewritten. Legacy implicit
barriers retain their original semantics. No new AI prompt fields are required.

Insufficient resources make a condition false. Missing, stale or unsupported
observations make it unknown. These conditions gate the existing runtime and
common HUD projection; they do not prove that an action was executed.

## Verified archive boundary, 31 October 2026

The read-only audit of the downloaded country-guide archive checked 61 guides:
47 usable guides validate and 14 are disabled for unavailable DLC. Against the
previous census of 343 unknown step conditions, 51 now compile, including 20 of
111 structured actions. The remaining 292 conditions comprise:

| Evidence family (exclusive classification for this census) | Steps | Needed next |
| --- | ---: | --- |
| Identity or other qualifiers | 126 | Unique event/focus/decision reference and every extra qualifier. |
| War identity or history | 56 | Named participants, war type, declaration/history or peace-conference evidence. Own-country `at_war` alone is insufficient. |
| Capitulation or defeat | 44 | Named country's qualified capitulation/defeat state and the required transition. Neither peace nor disappearance alone suffices. |
| Annexation or national formation | 27 | Owner/control/subject or formation identity, with the source's exact outcome. A tag or country name alone is insufficient. |
| Player readiness or training | 23 | `When ready` / `if possible` remain player guidance; explicit training criteria need unit/template evidence. |
| Landings or invasion readiness | 10 | Named destination, own troops/control and/or the declared invasion preparation. These are distinct states. |
| Resources with additional qualifiers | 6 | Both the balance and the remaining diplomatic/readiness clause. |

These are condition counts, not a count of all bugs or a claim of automatic
execution coverage. The 169 resolved decisions still lack qualified native
execution history. Three previously suspended compact-number interpretations
remain suspended. The complete local audit is
`.local/timing-conditions-audit-20261031.json`; it retains every source title,
step key, timing and compiled predicate.

Fixture tests cover selection/completion, thresholds, unknown/stale data,
conjunctions, disjunctions, country/tree ambiguity, explicit-condition preservation,
split-import adaptation, idempotence and Python/JavaScript runtime equivalence.
Archive validation is separate from real-session and visual HUD validation.
