//! Application backend services. The current C ABI is a migration adapter;
//! the same owned services will be hosted by the Rust HTTP/SSE executable.
#[cfg(windows)]
mod acquisition;
mod abi;
mod action_blocks;
pub mod application;
mod assets;
mod authoring;
pub mod bootstrap;
mod catalog_generation;
mod catalog_views;
mod command_families;
mod compiled_cache;
mod construction_binding;
pub mod country;
mod country_ffi;
pub mod domain_context;
mod entity_catalog;
mod event_instructions;
pub mod events;
mod focus_catalog;
mod focus_import;
mod focus_routes;
mod focus_source_rules;
mod focus_text;
#[cfg(windows)]
mod game_process;
mod geography;
mod gui_actions;
mod gui_observer;
mod gui_production;
mod gui_registry;
mod gui_visuals;
mod guide_compatibility;
mod guide_enrich;
pub mod guide_observe;
pub mod guide_prepare;
mod guide_repository;
pub mod guide_runtime;
pub mod guide_schema;
pub mod http;
mod hud_diagnostics;
mod hud_events;
mod hud_geometry;
mod hud_guides;
mod hud_ocr;
mod hud_production;
mod hud_service;
mod hud_targets;
mod hud_vision;
mod i18n;
mod import_catalog;
mod import_document;
mod import_preview;
mod import_reports;
mod import_service;
mod informal_targets;
pub mod installation;
pub mod live;
mod live_ffi;
#[cfg(windows)]
mod live_process;
#[path = "../../common/loopback.rs"]
pub mod loopback;
mod map_topology;
mod media_frames;
mod media_references;
pub mod monitor;
mod operation_binding;
mod operation_language;
mod operational_catalog;
mod pe;
mod political_binding;
mod process_tools;
mod production_binding;
pub mod production_context;
mod production_milestones;
mod production_revisions;
mod profile;
pub mod progress_store;
#[cfg(windows)]
mod provider;
mod provider_extract;
mod recipient;
mod reference_identity;
mod reference_library;
mod reference_presets;
mod research_import;
mod research_projection;
mod research_start;
mod runtime_context;
mod runtime_definition;
mod script_analysis_service;
mod script_document;
mod script_subset;
mod scripted_names;
mod snapshot;
mod source_acquisition;
mod source_eligibility;
pub mod state;
mod technology_catalog;
mod technology_layout;
mod technology_visuals;
mod texture;
mod timing_compiler;
mod video_jobs;
pub mod watch;
mod word_sources;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::ffi::{CString, c_char};
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicU64, Ordering},
};
static HUBS: OnceLock<Mutex<HashMap<u64, Arc<events::Hub>>>> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
fn hubs() -> &'static Mutex<HashMap<u64, Arc<events::Hub>>> {
    HUBS.get_or_init(|| Mutex::new(HashMap::new()))
}
fn reply(result: Result<Value, String>) -> *mut c_char {
    let result = match result {
        Ok(v) => json!({"schema_version":1,"result":v}),
        Err(e) => json!({"schema_version":1,"error":e}),
    };
    CString::new(result.to_string())
        .expect("escaped JSON")
        .into_raw()
}
#[derive(Deserialize)]
struct Request {
    schema_version: u32,
    op: String,
    #[serde(default)]
    args: Value,
}
fn execute(handle: u64, request: Request) -> Result<Value, String> {
    if request.schema_version != 1 {
        return Err("Unsupported backend request version".into());
    }
    let args = request.args;
    if request.op == "source_research_start" {
        return research_start::read(&args);
    }
    if request.op == "import_preview" {
        return Ok(import_preview::preview(
            &args["guide"],
            &args["sources"],
            &import_catalog::Index::new(&args)?,
            &args["library"],
            &args["research_context"],
        ));
    }
    if request.op == "import_promote_operations" {
        return operation_language::promote(
            &args["guide"],
            &import_catalog::Index::new(&args)?,
            &args["library"],
        );
    }
    if request.op == "import_production_revisions" {
        return production_revisions::annotate(&args["guide"]);
    }
    if [
        "import_foreign_commands",
        "import_foreign_prefix",
        "import_fortification_command",
        "import_doctrine_command",
    ]
    .contains(&request.op.as_str())
    {
        let index = import_catalog::Index::new(&args)?;
        let text = args["text"].as_str().ok_or("Missing command text")?;
        return match request.op.as_str() {
            "import_foreign_commands" => {
                Ok(command_families::foreign(text, &index)?.unwrap_or(Value::Null))
            }
            "import_foreign_prefix" => command_families::foreign_prefix(text, &index),
            "import_fortification_command" => {
                Ok(command_families::fortification(text, &index).unwrap_or(Value::Null))
            }
            _ => {
                Ok(command_families::doctrine(text, &index, &args["country"])?
                    .unwrap_or(Value::Null))
            }
        };
    }
    if request.op == "import_action_sequence" {
        return Ok(informal_targets::sequence(
            args["text"].as_str().ok_or("Missing command text")?,
            &import_catalog::Index::new(&args)?,
            &args["country"],
        )?
        .unwrap_or(Value::Null));
    }
    if request.op == "import_explicit_action" {
        return Ok(args["text"]
            .as_str()
            .and_then(word_sources::explicit_action)
            .unwrap_or(Value::Null));
    }
    if request.op == "import_reading_hint" {
        return Ok(args["text"]
            .as_str()
            .and_then(|text| {
                word_sources::abbreviation_hint(
                    text,
                    args.get("vocabulary").unwrap_or(import_catalog::language()),
                )
            })
            .map(Value::String)
            .unwrap_or(Value::Null));
    }
    if request.op == "import_reading_terms" {
        return Ok(args["text"]
            .as_str()
            .map(|text| {
                guide_enrich::glossary(
                    text,
                    args["category"].as_str().unwrap_or(""),
                    args.get("vocabulary"),
                )
            })
            .unwrap_or(json!([])));
    }
    if ["import_named_command", "import_production_command"].contains(&request.op.as_str()) {
        let index = import_catalog::Index::new(&args)?;
        let text = args["text"].as_str().ok_or("Missing command text")?;
        return Ok(if request.op == "import_named_command" {
            informal_targets::named(text, &index, &args["country"])?
        } else {
            informal_targets::production(
                text,
                &index,
                &args["country"],
                &args["library"],
                &args["declared"],
            )?
        }
        .unwrap_or(Value::Null));
    }
    if request.op == "import_event_command" {
        return Ok(event_instructions::command(
            &args["text"],
            &import_catalog::Index::new(&args)?,
            &args["country"],
        )?
        .unwrap_or(Value::Null));
    }
    if request.op == "import_timing" {
        return timing_compiler::execute(&args);
    }
    if request.op == "import_action_blocks" {
        return action_blocks::annotate(&args["guide"]);
    }
    if request.op == "import_annotate_milestones" {
        let mut guide = args["guide"].clone();
        let changes = production_milestones::annotate(&mut guide)?;
        return Ok(json!({"guide":guide,"changes":changes}));
    }
    if request.op == "import_bind_operations" {
        return operation_binding::bind(
            &args["guide"],
            &import_catalog::Index::new(&args)?,
            &args["library"],
        );
    }
    if request.op == "import_bind_production" {
        return production_binding::bind(
            &args["step"],
            &import_catalog::Index::new(&args)?,
            &args["country"],
            &args["library"],
            &args["declared"],
        );
    }
    if request.op.starts_with("reference_library_") {
        if request.op == "reference_library_presets" {
            return Ok(reference_presets::inspect(
                args["docs_dir"].as_str(),
                &args["designs"],
            ));
        }
        return reference_library::execute(&request.op, &args);
    }
    if request.op.starts_with("reference_") {
        return reference_identity::execute(&request.op, &args);
    }
    if request.op == "import_bind_foundation" {
        let index = import_catalog::Index::new(&args)?;
        return construction_binding::bind(
            &args["step"],
            &index,
            &geography::Geography::new(index.rows()),
        );
    }
    if request.op.starts_with("geography_") {
        return geography::execute(&request.op, &args);
    }
    if request.op == "import_bind_foreign" {
        return political_binding::foreign(&args["step"], &import_catalog::Index::new(&args)?);
    }
    if request.op == "import_bind_political" {
        return political_binding::bind(
            &args["step"],
            &import_catalog::Index::new(&args)?,
            &args["country"],
            args["audit"].as_bool().unwrap_or(false),
        );
    }
    if request.op == "import_bind_doctrine" {
        return political_binding::doctrine(
            &args["step"],
            &import_catalog::Index::new(&args)?,
            &args["country"],
        );
    }
    if request.op == "import_inventory" {
        return Ok(json!(import_document::inventory(&args["guide"])?));
    }
    if request.op == "import_coverage" {
        return import_reports::coverage(&args["guide"], &args["sources"]);
    }
    if request.op == "import_sequence" {
        return import_reports::sequence(&args["guide"]);
    }
    if request.op.starts_with("compatibility_") {
        return guide_compatibility::execute(&request.op, &args);
    }
    if request.op == "guide_compatibility" {
        return guide_compatibility::assess(&args);
    }
    if request.op == "research_compile" {
        return research_import::compile(
            &args["guide"],
            &import_catalog::Index::new(&args)?,
            &args["research_context"],
        );
    }
    if request.op == "research_chain" {
        return research_import::chain_request(&args);
    }
    if request.op == "focus_infer_scope" {
        return focus_routes::infer_scope(&args["guide"], &import_catalog::Index::new(&args)?);
    }
    if request.op == "focus_resolve_routes" {
        return focus_routes::resolve(&args["guide"], &import_catalog::Index::new(&args)?);
    }
    if request.op == "focus_source_guide" {
        return focus_import::source_guide(&args["guide"]);
    }
    if request.op == "focus_compile_conditionals" {
        return focus_import::conditionals(&args["guide"], &import_catalog::Index::new(&args)?);
    }
    if request.op == "focus_reconstruct" {
        return focus_import::reconstruct(
            &args["guide"],
            &import_catalog::Index::new(&args)?,
            &args["named"],
            args["work_limit"].as_u64().unwrap_or(2_000_000),
        );
    }
    if request.op.starts_with("identity_") {
        return hoi4companion_catalog_core::request(&request.op, &args);
    }
    if request.op.starts_with("word_") {
        return word_sources::execute(&request.op, &args);
    }
    if request.op == "guide_research_projection" {
        return research_projection::project(&args["guide"], &args["bindings"]);
    }
    if request.op == "guide_associations_read" {
        return guide_repository::associations(
            args["guide_id"].as_str().ok_or("ID guida non valido")?,
            std::path::Path::new(
                args["directory"]
                    .as_str()
                    .ok_or("Missing association directory")?,
            ),
        );
    }
    if request.op == "guide_associations_apply" {
        let source = guide_schema::normalize(&args["guide"])?;
        let changes = guide_repository::associations(
            source["id"].as_str().ok_or("ID guida non valido")?,
            std::path::Path::new(
                args["directory"]
                    .as_str()
                    .ok_or("Missing association directory")?,
            ),
        )?;
        return guide_repository::apply(&source, &changes);
    }
    if request.op == "media_register"
        || request.op == "media_registry"
        || request.op == "media_preview"
    {
        let frames = media_frames::Frames::new(
            std::path::PathBuf::from(args["root"].as_str().ok_or("Missing media root")?),
            args["ffmpeg"].as_str().map(std::path::PathBuf::from),
        );
        if request.op == "media_register" {
            frames.register(
                args["url"].as_str().ok_or("Missing URL")?,
                std::path::Path::new(args["path"].as_str().ok_or("Missing video source")?),
                &args["metadata"],
            )?;
            return Ok(Value::Null);
        }
        if request.op == "media_registry" {
            return frames.registry();
        }
        return Ok(json!(frames.preview(
            args["url"].as_str().ok_or("Missing video URL")?,
            args["seconds"].as_f64().ok_or("Minutaggio non valido")?,
            args["clip"].as_bool().unwrap_or(false)
        )?));
    }
    if request.op == "import_provider_body" {
        return provider_extract::body(
            &args["packet"],
            args["model"].as_str().ok_or("Missing model")?,
        );
    }
    if request.op == "import_provider_document" {
        return provider_extract::document(&args["response"], &args["sources"]);
    }
    if request.op == "import_source" {
        return source_acquisition::acquire(&args, None);
    }
    if request.op == "source_html" {
        return Ok(json!(source_acquisition::html(
            args["text"].as_str().ok_or("Missing HTML")?
        )?));
    }
    if request.op == "source_captions" {
        return Ok(json!(source_acquisition::captions(&args["value"])));
    }
    if request.op == "source_global_ip" {
        return Ok(json!(source_acquisition::global(
            args["ip"]
                .as_str()
                .ok_or("Missing IP")?
                .parse()
                .map_err(|_| "Invalid IP")?
        )));
    }
    if request.op == "video_profile" {
        return Ok(video_jobs::profile());
    }
    if request.op == "video_split_local" {
        return video_jobs::split_local(&args);
    }
    if request.op == "hud_sprite_geometry" {
        return hud_geometry::sprite(&args);
    }
    if request.op == "gui_fixture" {
        return gui_observer::fixture(&args);
    }
    if request.op == "hud_navigation" {
        return Ok(hud_geometry::navigation(&args));
    }
    if request.op == "hud_geometry_admit" {
        return hud_geometry::admit(&args["raw"], &args["proposals"], &args["view"]);
    }
    if request.op == "hud_ocr_rectangles" {
        let width = args["ocr"]["width"].as_u64().ok_or("Invalid OCR width")?;
        let height = args["ocr"]["height"].as_u64().ok_or("Invalid OCR height")?;
        hud_ocr::validate(&args["ocr"], width, height)?;
        return Ok(hud_ocr::rectangles(&args["ocr"], &args["proposals"]));
    }
    if request.op == "hud_vision_fixture" {
        return hud_vision::fixture(&args);
    }
    if request.op.starts_with("vision_") {
        return hud_vision::execute(&request.op, &args);
    }
    if request.op == "hud_production" {
        return Ok(json!(hud_production::proposals(
            &args["row"],
            &args["item"],
            import_catalog::Index::new(&args)?.rows(),
            &args["country"],
            &args["state"]
        )));
    }
    if request.op == "guide_runtime_definition" {
        return runtime_definition::prepare(
            &args["guide"],
            &import_catalog::Index::new(&args)?,
            &args["continuous"],
            &args["library"],
        );
    }
    if request.op == "hud_event_plan" {
        return hud_events::plan(&args["guide"], &import_catalog::Index::new(&args)?);
    }
    if request.op == "hud_targets" {
        return hud_targets::targets(
            &args["guide"],
            &args["runtime"],
            &import_catalog::Index::new(&args)?,
            &args["state"],
            args["prepare_events"].as_bool().unwrap_or(false),
            &args["event_plan"],
        );
    }
    if request.op == "source_youtube_url" {
        return Ok(json!(media_references::youtube(
            args["url"].as_str().ok_or("URL YouTube non valido")?
        )?));
    }
    if request.op == "guide_frame_reference" {
        return Ok(media_references::frame(&args["guide"], &args["step"]));
    }
    if request.op == "guide_media_references" {
        return Ok(media_references::references(&args["guide"], &args["step"]));
    }
    if request.op == "guide_association_change" {
        return guide_repository::change(
            &args["guide"],
            &args["key"],
            &args["expected"],
            &args["entity"],
            &import_catalog::Index::new(&args)?,
            std::path::Path::new(
                args["directory"]
                    .as_str()
                    .ok_or("Missing association directory")?,
            ),
        );
    }
    if request.op == "script_rule_describe" {
        let book = if args["book"].is_object() {
            args["book"].clone()
        } else {
            script_analysis_service::rulebook(std::path::Path::new(
                args["root"].as_str().ok_or("Missing rules root")?,
            ))?
        };
        return Ok(script_analysis_service::describe(
            &book,
            args["kind"].as_str().ok_or("Missing rule kind")?,
            args["name"].as_str().ok_or("Missing rule name")?,
        ));
    }
    if request.op == "script_analysis_run" {
        return script_analysis_service::analyze(
            &args["manifest"],
            std::path::Path::new(args["rules"].as_str().ok_or("Missing rules root")?),
            std::path::Path::new(args["host"].as_str().ok_or("Missing CWTools host root")?),
            std::path::Path::new(args["parser"].as_str().ok_or("Missing parser library")?),
        );
    }
    if request.op == "script_error_log" {
        return Ok(script_analysis_service::record_errors(
            &args["report"],
            std::path::Path::new(args["path"].as_str().ok_or("Missing source log")?),
            args["now"]
                .as_str()
                .ok_or("Missing observation timestamp")?,
        ));
    }
    if request.op == "script_analyze_documents" {
        let book = if args["book"].is_object() {
            args["book"].clone()
        } else {
            script_analysis_service::rulebook(std::path::Path::new(
                args["root"].as_str().ok_or("Missing rules root")?,
            ))?
        };
        return script_subset::analyze(
            &args["documents"],
            &args["labels"],
            &args["reference"],
            &book,
            &args["incomplete"],
            args["limit"].as_u64().unwrap_or(2000) as usize,
        );
    }
    if request.op == "script_rulebook" {
        return script_analysis_service::rulebook(std::path::Path::new(
            args["root"].as_str().ok_or("Missing rules root")?,
        ));
    }
    if request.op == "script_normalize_report" {
        return script_analysis_service::normalize(
            &args["raw"],
            &args["files"],
            &args["warnings"],
            std::path::Path::new(args["root"].as_str().ok_or("Missing rules root")?),
            args["limit"].as_u64().unwrap_or(2000).min(100000) as usize,
        );
    }
    if request.op == "script_analysis_files" {
        let mods = args["mods"]
            .as_array()
            .ok_or("Missing mod roots")?
            .iter()
            .map(|v| {
                v.as_str()
                    .map(std::path::PathBuf::from)
                    .ok_or("Invalid mod root")
            })
            .collect::<Result<Vec<_>, _>>()?;
        return script_analysis_service::files(
            std::path::Path::new(args["game"].as_str().ok_or("Missing game root")?),
            &mods,
        );
    }
    if request.op == "guide_repository" {
        return guide_repository::load(
            &args["directories"],
            args["association_directory"]
                .as_str()
                .map(std::path::Path::new),
        );
    }
    if request.op == "source_enrich" {
        return guide_enrich::enrich(&args);
    }
    if request.op == "source_project_costs" {
        return guide_enrich::project_costs(args["guide"].clone(), &args["catalog"]);
    }
    if request.op == "import_document" {
        return import_document::document(&args["document"]);
    }
    if request.op == "import_validate_sources" {
        return import_document::validate_sources(&args["sources"]);
    }
    match request.op.as_str() {
        "authoring_compile" => return authoring::compile(&args["document"]),
        "authoring_reference" => return Ok(authoring::action_reference()),
        "authoring_prompt" => return Ok(json!(authoring::prompt())),
        _ => {}
    }
    if request.op == "source_name_variants" {
        let expander = scripted_names::Expander::new(&args["definitions"], &args["labels"]);
        return Ok(json!(
            args["queries"]
                .as_array()
                .ok_or("Missing name queries")?
                .iter()
                .map(|q| expander.expand(
                    q["text"].as_str().unwrap_or(""),
                    &q["countries"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                ))
                .collect::<Vec<_>>()
        ));
    }
    if request.op == "source_entities" {
        return entity_catalog::entities(&args);
    }
    if request.op == "source_export" {
        return entity_catalog::export(&args["entries"], &args["reference"]);
    }
    match request.op.as_str() {
        "ui_translate" => {
            return Ok(json!(i18n::translate(
                args["text"].as_str().ok_or("Missing UI text")?,
                args["locale"].as_str().unwrap_or("english")
            )));
        }
        "ui_payload" => {
            return Ok(i18n::payload(
                &args["value"],
                args["locale"].as_str().unwrap_or("english"),
            ));
        }
        "ui_html" => {
            return i18n::html(
                args["text"].as_str().ok_or("Missing HTML")?,
                args["locale"].as_str().unwrap_or("english"),
            )
            .map(|s| json!(s));
        }
        _ => {}
    }
    if request.op == "source_snapshot" {
        return snapshot::read(
            std::path::Path::new(args["path"].as_str().ok_or("Missing snapshot path")?),
            args["country"].as_str().unwrap_or("AUS"),
        );
    }
    if request.op.starts_with("catalog_") {
        return catalog_generation::execute(&request.op, &args);
    }
    if [
        "source_political",
        "source_doctrines",
        "source_operations",
        "source_neighbors",
    ]
    .contains(&request.op.as_str())
    {
        return operational_catalog::execute(&request.op, &args);
    }
    if request.op == "source_expand_rules" {
        return Ok(Value::Object(
            args["rules"]
                .as_object()
                .ok_or("Missing focus criteria")?
                .iter()
                .map(|(key, rules)| {
                    (
                        key.clone(),
                        focus_source_rules::expand(rules, &args["scripts"], &args["dlcs"]),
                    )
                })
                .collect(),
        ));
    }
    if request.op == "source_technology_visuals" {
        return assets::with(args["assets_handle"].as_u64(), |library| {
            technology_visuals::load(&args["gui_files"], &args["gfx_files"], library)
        });
    }
    if request.op == "source_focus_visuals" {
        return assets::with(args["assets_handle"].as_u64(), |library| {
            gui_visuals::focus(&args["files"], library)
        });
    }
    if request.op.starts_with("assets_") {
        return assets::execute(&request.op, &args);
    }
    if request.op == "technology_layout" {
        return technology_layout::apply(args["catalog"].clone(), &args["files"], &args["labels"]);
    }
    if request.op == "technology_sources" {
        return technology_catalog::load(&args);
    }
    if request.op == "source_condition" {
        return Ok(json!(source_eligibility::condition(
            &args["node"],
            args["country"].as_str().unwrap_or(""),
            &args["dlcs"],
            &args["scripts"]
        )));
    }
    if request.op == "source_filter_trees" {
        return source_eligibility::filter(&args["trees"], &args["dlcs"], &args["scripts"]);
    }
    if request.op == "source_scripted_triggers" {
        return source_eligibility::scripts(&args["files"]);
    }
    if request.op == "focus_build" {
        return Ok(focus_catalog::build(
            &args["node"],
            &args["labels"],
            args["source"].as_str().ok_or("Missing focus source")?,
            args["joint"].as_bool().unwrap_or(false),
        )
        .unwrap_or(Value::Null));
    }
    if request.op == "focus_trees" || request.op == "focus_continuous" {
        return focus_catalog::load(
            &args["files"],
            &args["labels"],
            request.op == "focus_continuous",
        );
    }
    if request.op == "focus_localise" {
        return Ok(json!(focus_text::localise(
            args["text"].as_str().ok_or("Missing localization text")?,
            &args["labels"]
        )));
    }
    if request.op == "focus_tooltips" {
        return Ok(focus_text::tooltips(&args["node"], &args["labels"]));
    }
    if request.op == "focus_rules" {
        return Ok(focus_source_rules::rules(&args["node"]));
    }
    if request.op.starts_with("installation_") {
        return installation::execute(&request.op, &args);
    }
    if request.op.starts_with("app_") {
        return application::execute(&request.op, &args);
    }
    if request.op.starts_with("runs_") {
        return progress_store::execute(&request.op, &args);
    }
    if request.op == "guide_observe" {
        let guide = guide_schema::normalize(&args["guide"])?;
        return guide_observe::observe(
            &guide,
            &args["state"],
            &args["record"],
            &args["runs"],
            args["allow_research"].as_bool().unwrap_or(true),
        );
    }
    if request.op == "guide_normalize" {
        return guide_schema::normalize(&args["guide"]);
    }
    if matches!(
        request.op.as_str(),
        "guide_evaluate" | "guide_info" | "guide_clear" | "guide_cache_info"
    ) {
        return guide_runtime::execute(&request.op, &args);
    }
    if request.op == "guide_prepare" {
        return guide_prepare::prepare(&args["guide"], &args["state"], &args["progress"]);
    }
    if request.op == "production_context" {
        return production_context::execute(
            args["kind"]
                .as_str()
                .ok_or("Missing production projection")?,
            &args,
        );
    }
    if request.op == "domain_context" {
        return domain_context::execute(
            args["kind"].as_str().ok_or("Missing evidence projection")?,
            &args,
        );
    }
    if request.op == "state_normalize" {
        return Ok(state::normalize(&args["value"]));
    }
    if request.op == "state_valid_fact" {
        return Ok(json!(state::valid_fact(
            args["key"].as_str().ok_or("Missing fact key")?,
            &args["value"]
        )));
    }
    #[cfg(windows)]
    if request.op.starts_with("provider_") {
        return provider::execute(&request.op, &args);
    }
    if request.op.starts_with("monitor_") {
        return monitor::execute(&request.op, &args);
    }
    if request.op.starts_with("profile_") {
        return profile::execute(&request.op, &args);
    }
    #[cfg(windows)]
    if request.op.starts_with("game_") {
        return game_process::execute(&request.op, &args);
    }
    if request.op == "signature" {
        return events::signature(&args["value"]).map(Value::String);
    }
    if request.op == "open" {
        let hub = events::Hub::new(
            args["interval"].as_f64().ok_or("Invalid interval")?,
            args["maximum"].as_u64().ok_or("Invalid subscriber limit")? as usize,
        )?;
        let mut hubs = hubs().lock().map_err(|_| "Backend registry unavailable")?;
        if hubs.len() >= 128 {
            return Err("Backend hub limit exceeded".into());
        }
        let handle = NEXT.fetch_add(1, Ordering::Relaxed);
        hubs.insert(handle, Arc::new(hub));
        return Ok(json!(handle));
    }
    let hub = hubs()
        .lock()
        .map_err(|_| "Backend registry unavailable")?
        .get(&handle)
        .cloned()
        .ok_or("Unknown backend handle")?;
    let now = args["now"].as_f64().unwrap_or(0.);
    if !now.is_finite() {
        return Err("Invalid backend clock".into());
    }
    match request.op.as_str() {
        "subscribe" => hub.subscribe(now),
        "unsubscribe" => hub.unsubscribe(args["id"].as_u64().ok_or("Invalid subscriber")?),
        "publish" => hub.publish(args["value"].clone(), now),
        "heartbeat" => hub.heartbeat(now),
        "peek" => hub.peek(now),
        "wait" => hub.wait(
            args["id"].as_u64().ok_or("Invalid subscriber")?,
            args["seconds"].as_f64().ok_or("Invalid timeout")?,
            now,
        ),
        "close" => hub.close(),
        _ => Err("Invalid backend operation".into()),
    }
}

/// Safe in-process entry point used by the native HTTP and HUD hosts.
pub fn request(handle: u64, op: &str, args: Value) -> Result<Value, String> {
    execute(
        handle,
        Request {
            schema_version: 1,
            op: op.into(),
            args,
        },
    )
}
#[unsafe(no_mangle)]
pub extern "C" fn hoi4_backend_abi_version() -> u32 {
    1
}
#[unsafe(no_mangle)]
pub extern "C" fn hoi4_backend_info() -> *mut c_char {
    reply(Ok(
        json!({"engine":"rust-backend","schema_version":1,"abi_version":1,"state_events":1,"live_core":1,"source_catalogs":1,"catalog_generation":1,"game_access":true,"read_only":true,"qualified_handle_required":true}),
    ))
}
/// # Safety
/// data points to size readable bytes until this function returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoi4_backend_request(
    handle: u64,
    data: *const u8,
    size: usize,
) -> *mut c_char {
    reply(
        std::panic::catch_unwind(|| {
            if data.is_null() || size == 0 || size > 64 * 1024 * 1024 {
                return Err("Invalid backend input size".into());
            }
            let request: Request =
                serde_json::from_slice(unsafe { std::slice::from_raw_parts(data, size) })
                    .map_err(|e| e.to_string())?;
            if size > 32 * 1024 * 1024 && !request.op.starts_with("identity_") {
                return Err("Invalid backend input size".into());
            }
            execute(handle, request)
        })
        .unwrap_or_else(|_| Err("Backend request failed".into())),
    )
}
#[unsafe(no_mangle)]
pub extern "C" fn hoi4_backend_release(handle: u64) -> bool {
    let hub = hubs().lock().ok().and_then(|mut r| r.remove(&handle));
    if let Some(hub) = hub {
        let _ = hub.close();
        true
    } else {
        false
    }
}
/// # Safety
/// value is null or one live result allocated by this library, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoi4_backend_free(value: *mut c_char) {
    if !value.is_null() {
        drop(unsafe { CString::from_raw(value) })
    }
}
