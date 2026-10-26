# CWTools script analysis

The analysis page uses the bundled CWTools host and rules for references, scopes, trigger/effect parameters, and localization. Rust/Jomini provides the parsing integration used elsewhere; the tools have different roles.

Static findings are not gameplay tests. Missing/incompatible rules and excluded parse inputs can produce incomplete checks and follow-up warnings. Report unavailable coverage as unknown, not zero. Preserve parser classifications, recovered/excluded counts, provenance, and persistent error logs.

Detailed historical implementation notes and recorded evidence are preserved in the
[Italian reference](it/script-analysis.md). Those records retain their original dates and
verification limits; they are not a claim that every check was rerun for this translation.
