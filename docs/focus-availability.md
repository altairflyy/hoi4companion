# Focus criteria and optional guide alternatives

The existing importer/catalog reads `available`, `allow_branch`, AND/OR focus
prerequisites and mutual exclusions from installed game/mod definitions. Supported
scripted triggers are expanded once at catalog refresh, with depth/work bounds.
Guide files and the AI prompt need no new fields. `focus_criteria` v1 is enriched
locally; it is not a guessed live fact supplied by the guide author.

The shared Python/JavaScript runtime evaluates explicit numeric comparisons,
boolean combinations, simple conditional blocks, DLC constants and qualified
membership (focuses, technologies and ideas). In particular `has_war_support >
0.84` uses a strict boundary: exactly 84% does not satisfy it. The native metric is
the effective value, not base support. Its read-only implementation and real HUN
comparison are documented in [country-metrics-live.md](country-metrics-live.md).

Unsupported predicates, original-tag/foreign-country scope, unknown government,
territorial control, unqualified metric sections and ambiguous numeric defaults
remain unknown. A known false part of an AND can establish a block even while
other parts are unknown; an OR must remain logically correct. Native completion
or active selection still takes precedence: a criterion falling below its
threshold does not cancel work or manufacture completion.

When the main prescribed focus is demonstrably blocked and the focus slot is
free, the runtime scans **later actions in the same guide**, respecting declared
priorities, dependencies, conditions and game prerequisites. It offers at most
one independently available alternative. An alternative must have fully true
availability and must not exclude any remaining guide focus or a required branch.
No new strategy, automatic selection, guide reordering or progress change occurs.
The original focus keeps its place and returns when available. Unknown criteria
never authorize an alternative. A main target with unknown criteria can remain a
navigation reference; the application explicitly says its availability is not
verified.

The web focus tree uses a purple ALTERNATIVE marker alongside the waiting main
focus and observed threshold. The native HUD reuses its existing purple choice
tone and the same runtime admission/coordinate engine. Active work remains blue.
No new screen capture/OCR loop or memory-poll interval was added.

Tests cover shared runtime fixtures, strict boundaries, OR/NOT, missing and stale
facts, explicit dependencies, branch exclusions, active focus capacity, preservation
of source order and HUD projection. Replay tests use modified **copies** of state;
they do not change game memory. The real native value was checked against the
paused game's displayed 87%; visual alignment of a purple alternative in the live
focus panel is a separate verification, not implied by those tests.
