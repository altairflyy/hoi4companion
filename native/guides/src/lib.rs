//! Guide definitions: preparation, binding, language, decomposition, references and progress.
//! Split out of hoi4companion-backend so an edit elsewhere does not recompile it;
//! the backend re-exports each module at its crate root, so `crate::<module>`
//! paths there are unchanged.
pub mod advanced_references;
pub mod authoring;
pub mod command_families;
pub mod construction_binding;
pub mod construction_instructions;
pub mod decision_admission;
#[cfg(test)]
pub mod decision_admission_tests;
pub mod decomposition;
pub mod definition_archive;
pub mod entity_resolution;
pub mod event_instructions;
pub mod event_reach;
pub mod focus_catalog;
pub mod focus_routes;
pub mod focus_source_rules;
pub mod guide_enrich;
pub mod guide_observe;
pub mod guide_prepare;
pub mod guide_repository;
pub mod guide_runtime;
pub mod import_document;
pub mod import_preview;
pub mod informal_targets;
pub mod occurrence_manifest;
pub mod operational_catalog;
pub mod operation_binding;
pub mod operation_language;
pub mod political_binding;
pub mod production_binding;
pub mod production_milestones;
pub mod progress_store;
pub mod reference_candidates;
pub mod reference_library;
pub mod reference_presets;
pub mod research_import;
pub mod research_projection;
pub mod research_start;
pub mod runtime_definition;
pub mod run_upgrade;
pub mod technology_catalog;
pub mod technology_layout;
pub mod timing_compiler;
pub mod map_anchors;
pub mod map_topology;
pub mod import_reports;
pub mod production_revisions;
// Model modules at this crate root, so moved code keeps its crate::<module> paths.
#[allow(unused_imports)]
use hoi4companion_model::{state,domain_context,production_context,script_document,import_catalog,recipient,reference_identity,word_sources,guide_schema,geography,installation,compiled_cache,source_eligibility,action_blocks,focus_text,focus_import,events};
