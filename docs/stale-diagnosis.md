# Diagnosing stale and inconsistent observations

Stale transport state, unsupported process layout, country identity mismatch, and inconsistent focus state are different failures. Reproduce the symptom on a valid campaign and compare timestamps, identity, and qualified field reads.

Do not weaken nation identity checks, introduce arbitrary fallback nations, or revive old samples from a heartbeat. A changing game may produce a transient inconsistent read; keep that distinct from a permanently invalid layout. Historical diagnoses below apply to their recorded process/build.

Detailed historical implementation notes and recorded evidence are preserved in the
[Italian reference](it/stale-diagnosis.md). Those records retain their original dates and
verification limits; they are not a claim that every check was rerun for this translation.
