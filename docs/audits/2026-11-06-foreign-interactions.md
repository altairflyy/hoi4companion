# Foreign diplomatic actions and operative assignments - 6 November 2026

## Scope and implementation

Added three diplomatic intents (improve relations, stop improvements, established
non-aggression pact) and eight operative assignment intents. The generic
country resolver, Word adaptation, existing authoring types, declarative
runtime and campaign milestone store are reused. No nation-specific action
logic, source-guide edits, game writes, extra polling loop or automatic input.

Strict Python/JavaScript domain adapters distinguish complete observed absence
from unknown/partial observations. Incoming relation improvements do not count
as outgoing activity; pacts are bilateral; cancelled relations do not count.
Operatives require matching agency ownership, unique handles, enum/class pairs
and reciprocal mission container references. Captured/cooling-down/operation
states are distinct from an actively performed mission. Invalid mission data
does not erase otherwise qualified agency-creation flags or war observations.

The native country registry in the real save reuses D01/D02/D03 names. Repeated
names are aliases only if they resolve to the same country object. Ambiguous
names cannot become a guessed target, and adapter membership is deduplicated
only after these checks.

The original Hungary guide contains 14 newly bound diplomatic/operative
instructions after adaptation. Its SHA-256 remains
`287d7d5f3188ae96dd0f70ff63eb5067994b64ff7cc7c12d964e98b6c1726f8e`.
Foreign-action adaptation is idempotent and preserves original block metadata,
existing keys and source quotes. Comma order is retained within complete
commands; plus-separated siblings stay parallel. Standalone timing headings
now gate every child, and unresolved timing remains unknown. Existing custom
predicates are not replaced. Compound source costs are not copied as an
individual cost onto every child.

Previously observed execution is retained in the existing guide/campaign/session
milestones after a mission is reassigned or improvements are stopped. The
runtime applies declaration/dependency admission even to matching future
foreign actions; those cannot seed history early. Existing guide/run validation
still decides whether stored progress belongs to the current campaign.

## Native evidence

The inspected executable was authenticated by REA `open_binary` as PE x86-64,
SHA-256 `cf781ed178cd801643b233b7e3fcd1c0efd19bd01aecd24bf07a35f69f10476f`.
The attempted Ghidra procedure query timed out after 300 seconds; no successful
decompilation is claimed. REA was closed afterward. The static field evidence
below comes from separate local immutable-PE/RTTI/Capstone inspection, not
fabricated REA decompilation output.

- Relation factory sites D809CA/D80A0A bind tokens 2FBC/2CD7 to the respective
  non-aggression/improvement RTTI types. Serializer D8F4B0 corroborates endpoints
  at +10/+14 and cancellation at +48. Improvement cleanup D7A780 uses the first
  endpoint, corroborating its direction.
- Agency serializer 1064F00 corroborates C0/C1 creation flags and F0 operative
  pointers. Agency +78 reciprocally identifies the selected country.
- Operative serialization C7EB80 and its C7ED04 switch corroborate mission
  container +1088 and state +1080. Mission serializer 104AB80 and its 104ACD4
  switch corroborate the nine mission kinds, with type at container +18 and data
  at +10. Setter 104AAB0 corroborates the container's owning-agent pointer +8.
- Country/state-based mission serializers 1AC5640/1AC45D0 corroborate target
  country at mission-data +10. Real mission data reciprocates its mission
  container at +8, not the agent itself. This distinction was reproduced as a
  failing fixture before correcting the reader.
- Fourteen executable-byte signatures and the existing exact profile gate pin
  this layout. Other executable profiles do not inherit this capability.

All game access used the existing path/hash/process-identity-qualified read-only
WindowsGame handle and NativeSample revalidation. No game functions or commands
were called. The optional domain failures preserve independently valid domains.

## Real-session observations

The latest save was loaded in the background using `-continuelastsave`. HUN was
observed with a created agency, two active `root_out_resistance:HUN` agents,
and known-empty outgoing relation-improvement and pact lists. The full native
provider, strict adapter and five unchanged cheap probes succeeded: one full
sample, zero detected changes, 2,112 selected watch ranges / 47,191 selected
bytes. Example cheap-probe wall time was about 1.1 ms; this is not a HUD latency
or frame-rate measurement.

A separate world diagnostic checked 439 countries. It positively observed YEM
improving relations with MAC and SAR improving relations with YEM. The incoming
MAC relation did not produce a false outgoing improvement. Active network
building, counter-intelligence and resistance suppression were also observed.
These world observations are structural evidence, not player execution history.

No orders were issued. No real change-of-assignment/cancellation was induced.
No positive established pact or diplomatic-pressure assignment was observed in
this save; their positive real-session comparison remains pending. Five mission
kinds in total, including diplomatic pressure, have synthetic/static coverage
without a positive world sample. There is no visual HUD/button alignment
verification in this task.

## Validation

- Full Python suite: **1,163 passed**, none skipped.
- Full JavaScript suite: **435 passed**, none skipped.
- Focused additions: 10 Python cases, 4 JavaScript cases, including strict
  contract parity, directional/bilateral relations, cancellation watch changes,
  all eight missions, captured agents, reused tags, ownership failures, local
  ordering, campaign/session history, custom predicates, unresolved goals,
  heading propagation and old-key preservation. A later newline-boundary
  assertion in the existing parity case also passed.
- Installed-catalog archive audit: **61 country guides**, **47 ready**,
  **14 DLC-unavailable**, **0 requiring association revision**. DLC absence is
  not an import defect.
- Relevant `git diff --check` passed, apart from existing CRLF conversion notices.

Ignored local evidence: `foreign-interactions-provider.json`,
`foreign-interactions-world-probe.json`, `foreign-interactions-guide.json`,
`foreign-interactions-python.log`, `foreign-interactions-js.log` and
`foreign-interactions-archive.json` / `.log`.

Counter for this intervention: **11 action types implemented**, **14 Hungary
instructions bound**, **0 new archive import errors**. Positive real-world
observations cover **4 of the 11 types**; **7 positive cases remain to be
corroborated in a real session**. This is a verification counter, not seven
known defects or a count of all remaining project issues.

The owned hidden test game was closed after verification. The source guide and
its saved campaign were not edited. The computer was not shut down in this task.

Network strength thresholds, covert-operation success, agency upgrades and
pending diplomatic proposals are not added by this task. Assignment execution
must not be presented as those outcomes. No release, commit or full Windows
distribution rebuild was performed.
