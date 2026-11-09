# Production guide monitoring

Production goals distinguish equipment identity, variant, military/naval category, and requested versus assigned factories. Requested counts include factories not currently available; assigned, active, and damaged counts remain separate observations.

Compare only matching qualified lines. Added quantities require a valid baseline; absolute targets compare against the declared total. Changing a line, closing the production panel, or observing a resource fluctuation does not independently prove the guide goal. Visual source frames support details that cannot be expressed or observed confidently.

`Inf:ALL` means all remaining military capacity for that explicitly named final
product. Informal `MIL:`/`Production:` instructions and authoring
`factories:{mode:"remaining"}` compile to the same runtime policy. The target is
`max(current military capacity - requested factories on every other military line, 0)`.
The selected product must match one unique line and be the final military line;
naval lines do not consume military capacity. The requested count must be positive
and cover the target, so a missing line never completes a zero remainder.
Requested counts can exceed currently assigned factories. Capacity growth
recalculates the target; a line requesting extra capacity can remain satisfied.
This observes the allocation, not an unobserved automatic-assignment setting.
Missing capacity, incomplete/foreign/stale production data, or ambiguous lines
cannot prove completion. Numeric instructions retain their exact requested totals.

Explicit military line priorities use `production_priority:"first"/"last"`.
Qualifying the requested counts and the observed line order is necessary for
completion. A group of first-priority products can be internally reordered, but
unrelated lines cannot precede it. Unavailable line lists do not prove priority.
The external authoring format exposes this as optional line `production_priority`.
See [reference library](reference-library.md) for model aliases and visual designs.

## Production HUD

### Source phases and revised allocations

Word priority tables retain source-ordered production revisions in generated
`params.production_revision: {group, order}`. A later table section is entered
only when an objective from that section is selected/completed, or its previous
section's final focus is observed complete. This is a section-entry witness,
not proof that its operational instructions were performed. Explicit timing,
phase conditions and dependencies remain binding. Complete unprefixed allocation
lists are accepted; a trailing numeric quantity covers its contiguous product
group (`Train, Truck:1` gives one factory to each). Missing quantities, detached
numbers and shared `ALL` remain reminders. A range such as
`Inf:0->10` declares a total target of 10, without inventing an observed baseline.

Within one revision group, an admitted later instruction replaces the same
equipment selector; a later military `ALL` also replaces the previous remaining-
capacity product. Other fixed targets remain monitored. Unknown future gates
never override a current plan. Steps used as explicit dependencies are protected
from automatic replacement. Guides without revision metadata keep their existing
semantics. Legacy Word imports receive the same cached in-memory compilation as
new imports; neither source documents nor installed guide JSON are rewritten.

Phase placement and action readiness are separate. A later revision with a
qualified true section/phase/timing boundary retires the matching old goals even
when its adoption milestones or other action dependencies cannot yet be verified.
The replacement action keeps those gates: it is not highlighted or considered
started/completed merely because its phase was reached. Its missing prerequisites
remain visible with links. This avoids reverting to the initial allocation when
the game has demonstrably progressed but an earlier appointment is unobservable.
Unknown or false boundaries, calendar age alone, and matching production counts
do not retire old goals. Explicit dependency targets remain protected. If there
is no declared boundary, a blocked later revision does not replace anything.
Partially replaced blocked actions retain unknown execution and cannot release
downstream completion/start dependencies.

The browser presentation forwards campaign milestone history to the same runtime
used by the API and HUD. Observed historical adoptions therefore remain available
after a law/advisor changes, while a different campaign/run still clears them.

Superseded targets carry `superseded_by` references. A fully replaced instruction
is `not_applicable / production_superseded`, displayed as **Superseded**, with
unknown historical completion and a separate `original_completion` observation.
Partly replaced layouts monitor only their remaining targets. App summaries and
HUD omit replaced goals; historical requested/assigned counts remain inspectable.
The shared Python/JavaScript runtime uses the same rules. Qualified allocation
counts do not bypass a revision's conditions or dependencies.

An abbreviated focus ending with an ellipsis resolves only to a unique focus
declared in this guide. A reference design with an omitted tier resolves only if
its complete qualifier identifies a unique library recipe. Neither rule guesses
an ambiguous ID, a campaign outcome, or a native variant's unobserved modules.

The HUD projects only production steps admitted by Guide Runtime, including each
pending product in a multi-product layout. It highlights the Production entry
button first, then the unique matching existing military line when the panel is
open. Captions distinguish the requested target, requested factories and actually
assigned factories, and retain explicit first/last priority and remaining-capacity
instructions. Unsatisfied lines use the recommendation color; qualified goals
remain tracked in light blue on their own production rows. Requested and assigned
factory counts stay separate: a requested-factory goal can turn blue even when
current capacity is lower. Exact goals reject over-allocation; minimum goals
accept higher counts. Unknown evidence never turns a row blue. Gate/dependency
checks also apply to completed production goals, so a coincidentally matching
future allocation cannot become an actionable or blue target. Superseded goals
stay excluded.

Missing products route through the native add-product category, then a unique
equipment/variant button in the open selector. Category routing inherits the
installed equipment's `interface_category`; it never guesses from an ID or model
name. Missing or conflicting metadata suppresses the category highlight. Several
products sharing an add button produce one annotation listing their names.
An open selector suppresses highlights on the underlying rows. Multiple matching
models, unqualified presence and foreign views never select an arbitrary product.

The current qualified GUI profile is the exact executable SHA-256
`cf781ed178cd801643b233b7e3fcd1c0efd19bd01aecd24bf07a35f69f10476f`.
Row admission requires own-country pool membership, controller/view ownership,
equipment and optional variant identity, witnessed geometry and viewport clipping.
Unavailable, foreign or ambiguous controls produce no guessed highlight. The HUD
does not click or infer execution from a click. It uses the application's active
guide and existing campaign-scoped runtime evidence.

Existing military row identities were observed on a real Hungarian campaign in
background. Foreground visual alignment, scrolling and ordering after the final
build remain unverified. Add-product/picker layouts have exact-build constructor
evidence and synthetic ownership/geometry tests; their real-screen recognition
and the new blue transition remain unverified because HOI4 was closed during
this extension. Naval production row geometry remains unsupported. No guide
definitions or game files need rewriting for this extension.

Detailed historical implementation notes and recorded evidence are preserved in the
[Italian reference](it/production-guide-monitoring.md). Those records retain their original dates and
verification limits; they are not a claim that every check was rerun for this translation.

### Deadlines and named tank designs

The adapter decodes the already acquired, qualified Paradox clock using Jomini's
365-day calendar and year offset of 5000; it does not add memory reads. The
calculated `campaign.date` fact is a numeric YYYYMMDD shared by Python and JavaScript.
SSE and the browser propagate day changes without making every hourly tick an update.
`On TAG down/no later than MONTH YEAR` is an explicit alternative: an observed
defeat or the deadline can admit the action. Month-only deadlines are certainly
reached on the last day of the named month; earlier defeat remains unknown until
qualified evidence is available. Missing or stale clocks never admit a deadline.
Existing generated timing placeholders are adapted in memory; authored conditions
and original guide files are preserved.

A named design without a resolved recipe still requires its named identity. If no exact match is
observed but exactly one owned production line uses a compatible chassis, the HUD
can mark that line as requiring design verification, showing both its current name and
the requested recipe. This is a yellow design-verification cue, not evidence that
its modules match. Its factories do not satisfy the template goal. Ambiguous
candidate lines are not guessed, and future or unknown-gated steps remain hidden.

On 2026-10-31 the activated service was checked against the running Hungarian
campaign in background. Its qualified date decoded to 1938-12-03; the rifle goal
was admitted with 10 requested/assigned factories. Native GUI ownership and row
coordinates were read successfully for both rifles and the unique tank candidate.
The native HUD was rebuilt and restarted on the same active guide; the guide file,
run and checklist were preserved. Foreground rendering was not visually inspected,
and tank module equivalence remains unverified.

### Ordered adoption milestones

#### Recipe-based model matching

Missing equipment types or a known differing recipe illuminate the relevant
add-production category icon (infantry, armor, aircraft or naval). This cue
coexists with a uniquely identified differing line; after opening the picker it
coexists with the exact qualified product. Shared category icons are grouped
once, with all admitted requested product names. Factory-count-only shortfalls
and satisfied quotas keep their line highlights without an add-category cue.
Unknown presence/configuration is not treated as a proven missing product.
Hidden, duplicate, foreign or ambiguous widgets never authorize a rectangle;
when one cue is unqualified, an independently qualified other cue can remain.

Resolved equipment references now compare the full native chassis/type,
slot-to-module map and upgrade levels; their names only retrieve the source
recipe. Correct configurations with different display names count. Equal names
with changed, missing or extra modules do not count. Explicit empty slots and
zero upgrade levels are normalized only for complete observed containers.

Each layout detail exposes `design_check` (`match`, `different`, `unknown`,
`missing`) and per-line module/upgrade differences. An unreadable compatible
candidate makes the count unknown, even if another line matches: a partial
inventory cannot prove totals or remaining allocation. Relative baselines include
the canonical recipe, so changing it cannot reuse the old recipe's count.

The walkthrough lists the exact differences. The HUD keeps a non-completed cue
on a uniquely identified differing/unknown line and turns the quota blue only
for a qualified matching recipe. The equipment picker also compares native
configuration and does not filter matching models by their localized names.
Generic trains, trucks, rifles and unnamed families keep type/family matching.
No source guide or campaign data is rewritten and no new AI authoring fields
are required. Native provenance and runtime limits: [design-live.md](design-live.md).

For Word priority tables, production revisions also use prior resolved adoption
actions from the ordered **Actions** column. The latest mandatory milestone per
role or law category is retained: appointments, laws and completed doctrines or
research can gate the next production target. The parallel Research and Focus
columns do not create such sequential prerequisites. Optional/conditional
adoptions and explicit dependency policies are not silently rewritten.

The importer adds `action.params.production_milestones` in memory, with references
to the source steps and their exact completion predicates. Original files and
authored timing conditions remain intact. A reached date or matching factory
allocation does not bypass these milestones. Missing evidence produces
`milestone_unknown`; known unmet goals produce `milestone_not_completed`. Both
the application and HUD use the same runtime admission and show prerequisite
links in the application instead of requesting a manual production confirmation.

Qualified observed achievements are persisted in campaign progress, separately
from checklists, under `campaign_conditions.milestones`. Identity tokens include
the step and exact predicate, preventing an edited guide from reusing a different
requirement's history. Replacing an observed advisor or law does not erase that
achievement. Guide/run/campaign isolation and timeline rewind protection use
the existing RunStore mechanisms. Unobserved historical appointments are not
inferred from the date, political power, a later focus or matching production.

Unresolved prose (for example an unbound doctrine abbreviation) cannot supply a
verified milestone. This extension consumes existing resolved predicates; it does
not add native political/doctrine acquisition capabilities. These remain subject
to the reader's per-version qualification and real-game verification.

Production evaluation reuses each distinct product/configuration comparison only
within the current evaluation. Priority peers and military queue rows are indexed
once per snapshot. No matching cache survives a new sample, guide or campaign;
an unavailable configuration still makes counts unknown. Per-step quantities,
admission gates, explanations and replacement annotations remain independent.

Revision identity uses canonical recipe maps in both Python and JavaScript.
Property order and display-name changes do not keep an obsolete recipe target
active; different recipes and generic explicitly named variants stay distinct.
This identity does not change persisted relative-baseline keys or infer that a
superseded allocation was ever completed. Tests cover reordered recipes, changed
samples, unavailable evidence and cross-runtime parity without opening HOI4.
