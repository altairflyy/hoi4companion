# Campaign identity and persisted progress

Guide definitions, campaign identity, and per-run progress are separate. The application persists run/checklist state locally and reconciles it with qualified campaign observations. Starting another checklist does not alter the guide or assert live completion. A timeline rewind can require a separate branch of progression.

Unknown campaign identity must not silently associate observations with a different campaign. Existing stored guides and progress remain compatible. A fixture proving isolation is not proof of a real save-switch experiment.

Detailed historical implementation notes and recorded evidence are preserved in the
[Italian reference](it/campaign-progress.md). Those records retain their original dates and
verification limits; they are not a claim that every check was rerun for this translation.
