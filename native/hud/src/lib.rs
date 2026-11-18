//! HUD logic: target geometry, proposals, diagnostics, OCR matching and event answers.
//! Split out of hoi4companion-backend so an edit elsewhere does not recompile it;
//! the backend re-exports each module at its crate root, so `crate::<module>`
//! paths there are unchanged.
pub mod hud_diagnostics;
pub mod hud_events;
pub mod hud_geometry;
pub mod hud_guides;
pub mod hud_ocr;
pub mod hud_production;
pub mod hud_targets;
// Modules of the crates below, at this crate root, so moved code keeps its crate::<module> paths.
#[allow(unused_imports)]
use hoi4companion_model::{state,domain_context,production_context,script_document,import_catalog,recipient,reference_identity,word_sources,guide_schema,geography,installation,compiled_cache,source_eligibility,action_blocks,focus_text,focus_import,events};
#[allow(unused_imports)]
use hoi4companion_guides::{advanced_references,authoring,command_families,construction_binding,construction_instructions,decision_admission,decomposition,definition_archive,entity_resolution,event_instructions,event_reach,focus_catalog,focus_routes,focus_source_rules,guide_enrich,guide_observe,guide_prepare,guide_repository,guide_runtime,import_document,import_preview,informal_targets,occurrence_manifest,operational_catalog,operation_binding,operation_language,political_binding,production_binding,production_milestones,progress_store,reference_candidates,reference_library,reference_presets,research_import,research_projection,research_start,runtime_definition,run_upgrade,technology_catalog,technology_layout,timing_compiler,map_anchors,map_topology,import_reports,production_revisions};
