# Fortification live qualification follow-up - 3 November 2026

The latest local save (`autosave.hoi4`, last modified 2 November 2026 22:28:18) was loaded using the executable's `-continuelastsave` option. Native identity was HUN, and the game screenshot showed the United Kingdom of Europe on 1 May 1954 at 02:00, in pause, with the campaign statistics modal open.

Every attempted desktop input failed with `failed to activate captured window`, including after fresh window selection and through the second supported computer-use interface. The user is away and requested shutdown if the visual comparison cannot be completed. No game commands, orders, file edits, memory writes or manual save were issued.

The unqualified diagnostic reproduced a real scalability failure: the country controlled 9,995 qualifying land provinces. The old reader consumed 6,033,658 bytes in repeated calls (3,375,110 unique bytes, 145,625 witnesses), exceeding its 2 MiB acquisition budget and the native observer's range budget. Its watch also exceeded the existing page-byte budget. These diagnostic values were obtained with a temporary, probe-only larger acquisition budget; they were not accepted as qualified observations or enabled in a shipped profile.

The reader now samples only the fort definition slots in each provincial map, and the selected building pointers, instead of reading/tracking every building kind. The state registry header is witnessed once per sample. Reciprocal owner/template and location flags remain watched. All consumed bytes are still revalidated. Existing sample, native observer and watch budgets were not increased. A synthetic 4,097-province regression with 256 building-map slots reproduces the old budget failure and passes after the change. Fort-map creation is also covered by a watch-change regression.

Validation after the final change: 23 fortification Python tests passed; targeted diff whitespace checks passed. Full Python/JavaScript suites were not rerun for this follow-up. The prior 1,151 Python / 431 JavaScript result predates these two new tests.

The installed executable hash remains cf781ed178cd801643b233b7e3fcd1c0efd19bd01aecd24bf07a35f69f10476f. Its fortification capability remains DISABLED. Its real sample under the optimized reader, budget fit on this save, visible built-level comparison, and end-to-end campaign recognition are NOT validated. The current profile's building database RVA has also not been enabled by this investigation.

Counter for this follow-up: 1 scalability defect corrected with fixture evidence; 1 live qualification blocker remains. This is not a count of all project issues.
