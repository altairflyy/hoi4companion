//! All backend integration tests in one executable: one link instead of one
//! per file. Each former `tests/<name>.rs` is a module; filter with
//! `cargo test --test integration <name>::`.
mod commander_assignments;
mod confirmed_answers;
mod contracts;
mod decision_history;
mod decomposition_activation;
mod definition_identity_characterization;
mod definition_pinning;
mod entity_resolution_corpus;
mod guide_benchmark_contracts;
mod guide_evaluation;
mod guide_identity_diagnostics;
mod interpretations;
mod occurrence_identity;
mod presentation_fixtures;
mod real_catalog_contract;
mod startup_preparation;
