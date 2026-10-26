# Guide Schema v1

Guide Schema v1 is the internal, nation-independent definition consumed by the runtime. It includes tracks of keyed steps, typed actions/entity references, conditions, dependencies, phases, evidence, and source metadata. IDs and semantic codes are stable across interface languages.

Validators reject unsupported structures, duplicate/missing references, cyclic dependencies, invalid numbers, and mismatched evidence. Both start and completion dependency edges participate in cycle detection. Reminders have no finite completion goal. Conditions use fact/check/all/any/not with qualified values.

Prefer the simpler external [authoring contract](guide-authoring.md) for newly generated files. Existing guides are normalized without replacing authored instructions, source quotations, or campaign progression.

Detailed historical implementation notes and recorded evidence are preserved in the
[Italian reference](it/guide-schema.md). Those records retain their original dates and
verification limits; they are not a claim that every check was rerun for this translation.
