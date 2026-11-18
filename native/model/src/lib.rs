//! Shared backend model: game state, catalogs, schemas and text sources.
//! Split out of hoi4companion-backend so an edit elsewhere does not recompile it;
//! the backend re-exports each module at its crate root, so `crate::<module>`
//! paths there are unchanged.
pub mod state;
pub mod domain_context;
pub mod production_context;
pub mod script_document;
pub mod import_catalog;
pub mod recipient;
pub mod reference_identity;
pub mod word_sources;
pub mod guide_schema;
pub mod geography;
pub mod installation;
pub mod compiled_cache;
pub mod source_eligibility;
pub mod action_blocks;
pub mod focus_text;
pub mod focus_import;
pub mod events;
