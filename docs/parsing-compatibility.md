# Parsing compatibility and excluded inputs

A game can tolerate constructs that stricter analysis tools reject. Classify disagreement between Jomini, CWTools, and game-tolerated syntax rather than treating every diagnostic as a proven game defect.

Inputs rejected by both parsers are logged and excluded from applicable analysis instead of aborting the entire catalog. Coverage remains partial. Source recovery is performed only in memory under explicit content/hash verification; original game/mod files remain unchanged.

Detailed historical implementation notes and recorded evidence are preserved in the
[Italian reference](it/parsing-compatibility.md). Those records retain their original dates and
verification limits; they are not a claim that every check was rerun for this translation.
