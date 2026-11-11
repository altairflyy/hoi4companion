//! A complete installation generation. Reload constructs a replacement before
//! publication; requests cannot combine old DLC/mod metadata with new assets.
use crate::{
    assets::Assets,
    focus_catalog, focus_source_rules, gui_visuals, installation, operational_catalog,
    script_document::{self, blocks, scalar},
    source_eligibility, technology_catalog, technology_visuals,
};
use serde_json::{Map, Value, json};
use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};
type Result<T> = std::result::Result<T, String>;
pub struct Catalog {
    pub sources: Arc<Value>,
    pub vocabulary: Value,
    pub(crate) import_index_cache: Option<crate::import_catalog::Index>,
    pub(crate) research_context_cache: HashMap<String, Value>,
    pub assets: Option<Assets>,
    pub identities: Option<Value>,
    pub compatibility: Option<crate::guide_compatibility::Check>,
}
fn append(warnings: &mut Vec<Value>, node: &Value) {
    warnings.extend(node["warnings"].as_array().into_iter().flatten().cloned());
}
fn files(
    game: &Path,
    mods: &[PathBuf],
    directory: &str,
    suffix: &str,
    warnings: &mut Vec<Value>,
) -> Result<Value> {
    let value = installation::collect(game, mods, directory, suffix)?;
    append(warnings, &value);
    Ok(value["files"].clone())
}
fn static_branch(node: &Value) -> Value {
    json!({"entries":script_document::entries(node).map(|(key,value)|{
        let upper=key.to_uppercase();let key=if ["AND","OR","NOT","NAND","NOR"].contains(&upper.as_str()){upper.as_str()}else if ["has_dlc","always"].contains(&key){key}else{"unobserved_runtime_predicate"};
        json!([key,if value.get("entries").is_some(){static_branch(value)}else{value.clone()}])
    }).collect::<Vec<_>>()})
}
fn excluded(trees: &Value, dlcs: &Value) -> Vec<Value> {
    if dlcs.is_null() {
        return vec![];
    }
    let mut result = vec![];
    for (scope, tree) in trees.as_object().into_iter().flatten() {
        let mut disabled: BTreeMap<String, &str> = tree["branch_conditions"]
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(_, node)| {
                source_eligibility::condition(&static_branch(node), "", dlcs, &json!({}))
                    == Some(false)
            })
            .map(|(id, _)| (id.clone(), "DLC condition on allow_branch is false."))
            .collect();
        loop {
            let mut changed = false;
            for (id, focus) in tree["focuses"].as_object().into_iter().flatten() {
                if !disabled.contains_key(id)
                    && focus["prerequisites"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|group| {
                            group.as_array().is_some_and(|a| {
                                !a.is_empty()
                                    && a.iter().all(|parent| {
                                        parent.as_str().is_some_and(|id| disabled.contains_key(id))
                                    })
                            })
                        })
                {
                    disabled.insert(
                        id.clone(),
                        "Every alternative in a required branch is excluded by DLC conditions.",
                    );
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        result.extend(
            disabled
                .into_iter()
                .map(|(id, reason)| json!({"scope":scope,"id":id,"reason":reason})),
        );
    }
    result
}
fn names(game: &Path, mods: &[PathBuf], warnings: &mut Vec<Value>) -> Result<Value> {
    let files = files(game, mods, "common/scripted_localisation", ".txt", warnings)?;
    let mut definitions = Map::new();
    for path in files
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        match script_document::read(Path::new(path)) {
            Ok(node) => {
                for node in blocks(&node, "defined_text") {
                    let name = scalar(node, "name");
                    if !name.is_empty() {
                        definitions.insert(
                            name.into(),
                            json!(operational_catalog::unique(
                                blocks(node, "text")
                                    .map(|n| scalar(n, "localization_key"))
                                    .filter(|s| !s.is_empty())
                                    .map(|s| json!(s))
                            )),
                        );
                    }
                }
            }
            Err(e) => warnings.push(json!(format!(
                "Scripted names ({}): {e}",
                Path::new(path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            ))),
        }
    }
    Ok(Value::Object(definitions))
}
impl Catalog {
    pub fn open(args: &Value) -> Result<Self> {
        let vocabulary = if let Some(path) = args["vocabulary_path"].as_str() {
            crate::guide_repository::json_file(Path::new(path), 2_000_000)?
        } else {
            args.get("vocabulary")
                .cloned()
                .unwrap_or_else(|| crate::import_catalog::language().clone())
        };
        if vocabulary["schema_version"] != 1 || !vocabulary["abbreviations"].is_object() {
            return Err("Invalid guide vocabulary version or abbreviations".into());
        }
        let mut settings = json!({"game_dir":"","docs_dir":"","tree_id":"","snapshot_path":"","country":"AUS","language":"english","ui_language":"english","mods":"auto"});
        if let Some(update) = args["settings"].as_object() {
            for (key, value) in update {
                if settings.get(key).is_some() {
                    settings[key] = value.clone();
                }
            }
        }
        let game = PathBuf::from(
            settings["game_dir"]
                .as_str()
                .ok_or("Invalid game directory")?,
        );
        let docs = PathBuf::from(
            settings["docs_dir"]
                .as_str()
                .ok_or("Invalid documents directory")?,
        );
        let demo = !game.join("common/national_focus").is_dir();
        let mut warnings = vec![];
        let mut mods = vec![];
        let mut mod_inventory = json!({"status":if settings["mods"]=="off"{"off"}else{"unavailable"},"source":"","basis":"launcher_profile","items":[],"roots":[],"warnings":[]});
        let (labels, reference, focus_files, dlcs, scripted_names, operation_entities) = if demo {
            let file = args["demo_file"]
                .as_str()
                .ok_or("Demo source unavailable")?;
            let label_path = args["demo_labels"]
                .as_str()
                .ok_or("Demo labels unavailable")?;
            let labels: Value =
                serde_json::from_slice(&std::fs::read(label_path).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            warnings.push(json!(
                "Modalità dimostrativa: nessun file HOI4 reale caricato."
            ));
            (
                labels.clone(),
                labels,
                json!([file]),
                Value::Null,
                json!({}),
                json!([]),
            )
        } else {
            if settings["mods"] == "auto" && !settings["docs_dir"].as_str().unwrap_or("").is_empty()
            {
                mod_inventory = installation::mods(&docs)?;
                mods = mod_inventory["roots"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(PathBuf::from)
                    .collect();
                append(&mut warnings, &mod_inventory);
            }
            let focus_files = files(&game, &mods, "common/national_focus", ".txt", &mut warnings)?;
            let labels = installation::localisations(
                &game,
                &mods,
                settings["language"].as_str().unwrap_or("english"),
            )?;
            let input = json!({"game":installation::display(&game),"mods":mods.iter().map(|p|installation::display(p)).collect::<Vec<_>>(),"labels":labels[0],"reference":labels[1]});
            let mut source = operational_catalog::Source::new(&input)?;
            let operations = operational_catalog::operations(&mut source)?;
            warnings.extend(source.warnings);
            if focus_files.as_array().is_none_or(Vec::is_empty) {
                warnings.push(json!(
                    "Nessun file di focus trovato; controlla il percorso di installazione."
                ));
            }
            // Preserve warning order: focus parsing precedes continuous/names/DLC.
            (
                labels[0].clone(),
                labels[1].clone(),
                focus_files,
                Value::Null,
                Value::Null,
                json!(operations),
            )
        };
        let focus = focus_catalog::load(&focus_files, &labels, false)?;
        let mut trees = focus["trees"].clone();
        let mut all_trees = trees.clone();
        let mut continuous = json!({});
        let mut scripts = json!({});
        let mut excluded_trees = json!([]);
        let mut technology_definitions = json!({});
        let mut technologies =
            json!({"folders":{},"technologies":{},"warnings":[],"excluded_folders":[]});
        let mut technology_visuals = json!({"templates":{},"folders":{}});
        let mut focus_visuals = json!({});
        let mut assets = None;
        let mut dlcs = dlcs;
        let mut operation_entities = operation_entities;
        let mut scripted_names = scripted_names;
        if !demo {
            let continuous_files = files(
                &game,
                &mods,
                "common/continuous_focus",
                ".txt",
                &mut warnings,
            )?;
            let loaded = focus_catalog::load(&continuous_files, &labels, true)?;
            continuous = loaded["focuses"].clone();
            append(&mut warnings, &loaded);
            scripted_names = names(&game, &mods, &mut warnings)?;
        }
        append(&mut warnings, &focus);
        if !demo {
            dlcs = installation::dlcs(
                &game,
                (!settings["docs_dir"].as_str().unwrap_or("").is_empty()).then_some(docs.as_path()),
            )?;
            append(&mut warnings, &dlcs);
            let trigger_files = files(
                &game,
                &mods,
                "common/scripted_triggers",
                ".txt",
                &mut warnings,
            )?;
            let loaded = source_eligibility::scripts(&trigger_files)?;
            scripts = loaded["scripts"].clone();
            append(&mut warnings, &loaded);
            let eligibility = source_eligibility::filter(&trees, &dlcs, &scripts)?;
            excluded_trees = eligibility["excluded"].clone();
            trees.as_object_mut().unwrap().retain(|key, _| {
                eligibility["visible"]
                    .as_array()
                    .is_some_and(|a| a.contains(&json!(key)))
            });
            for (key, tree) in trees.as_object_mut().unwrap() {
                tree["eligible_countries"] = eligibility["countries"][key].clone();
                for focus in tree["focuses"]
                    .as_object_mut()
                    .into_iter()
                    .flat_map(|m| m.values_mut())
                {
                    if focus["criteria"].as_object().is_some_and(|m| !m.is_empty()) {
                        focus["criteria"] =
                            focus_source_rules::expand(&focus["criteria"], &scripts, &dlcs);
                    }
                }
            }
            for row in operation_entities.as_array_mut().into_iter().flatten() {
                if row["operational_criteria"].is_object() { row["operational_criteria"] = focus_source_rules::expand(&row["operational_criteria"], &scripts, &dlcs); }
            }
            let tech_files = files(&game, &mods, "common/technologies", ".txt", &mut warnings)?;
            let tag_files = files(
                &game,
                &mods,
                "common/technology_tags",
                ".txt",
                &mut warnings,
            )?;
            let all_gui = files(&game, &mods, "interface", ".gui", &mut warnings)?;
            let gui: Vec<_> = all_gui
                .as_array()
                .into_iter()
                .flatten()
                .filter(|p| {
                    Path::new(p.as_str().unwrap_or(""))
                        .file_name()
                        .is_some_and(|n| {
                            n.to_string_lossy()
                                .to_lowercase()
                                .starts_with("countrytechtree")
                        })
                })
                .cloned()
                .collect();
            let loaded = technology_catalog::load(
                &json!({"files":tech_files,"tag_files":tag_files,"labels":labels,"country":settings["country"].as_str().unwrap_or("AUS").to_uppercase(),"dlcs":dlcs,"scripts":scripts,"gui_files":gui,"definitions":true}),
            )?;
            technologies = loaded["catalog"].clone();
            technology_definitions = loaded["definitions"].clone();
            append(&mut warnings, &technologies);
            let roots: Vec<_> = dlcs["items"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|d| ["enabled", "integrated"].contains(&d["status"].as_str().unwrap_or("")))
                .filter_map(|d| d["descriptor"].as_str())
                .filter_map(|d| Path::new(d).parent())
                .map(|p| installation::display(&game.join(p)))
                .chain(mods.iter().map(|p| installation::display(p)))
                .collect();
            let mut library = Assets::open(
                &json!({"game":installation::display(&game),"roots":roots,"cache":args["cache"]}),
            )?;
            append(&mut warnings, &library.info());
            let gfx = files(&game, &mods, "interface", ".gfx", &mut warnings)?;
            technology_visuals =
                technology_visuals::load(&json!(gui), &gfx, Some(&mut library))?["value"].clone();
            append(&mut warnings, &technology_visuals);
            focus_visuals = gui_visuals::focus(&all_gui, Some(&mut library))?;
            append(&mut warnings, &focus_visuals);
            assets = Some(library);
        }
        if !trees
            .as_object()
            .unwrap()
            .contains_key(settings["tree_id"].as_str().unwrap_or(""))
        {
            settings["tree_id"] = trees
                .as_object()
                .unwrap()
                .keys()
                .next()
                .map(|s| json!(s))
                .unwrap_or(json!(""));
        }
        // The unfiltered index must carry the same qualified definitions for
        // visible trees; unavailable alternate trees retain their source data.
        for (key, tree) in trees.as_object().into_iter().flatten() {
            all_trees[key] = tree.clone();
        }
        let excluded_focuses = excluded(&trees, &dlcs);
        Ok(Self {
            vocabulary,
            import_index_cache: None,
            research_context_cache: HashMap::new(),
            sources: Arc::new(
                json!({"settings":settings,"demo":demo,"warnings":warnings,"labels":labels,"reference_labels":reference,"all_trees":all_trees,"trees":trees,"continuous_focuses":continuous,"scripted_names":scripted_names,"scripts":scripts,"dlcs":dlcs,"mod_inventory":mod_inventory,"mod_roots":mods.iter().map(|p|installation::display(p)).collect::<Vec<_>>(),"game_root":if demo&&settings["game_dir"]==""{Value::Null}else{json!(installation::display(&game))},"excluded_trees":excluded_trees,"excluded_focuses":excluded_focuses,"operation_entities":operation_entities,"technologies":technologies,"technology_definitions":technology_definitions,"technology_visuals":technology_visuals,"focus_visuals":focus_visuals}),
            ),
            assets,
            identities: None,
            compatibility: None,
        })
    }
}
static ITEMS: OnceLock<Mutex<HashMap<u64, Arc<Mutex<Catalog>>>>> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
pub fn execute(op: &str, args: &Value) -> Result<Value> {
    let items = ITEMS.get_or_init(|| Mutex::new(HashMap::new()));
    if op == "catalog_open" {
        let catalog = Catalog::open(args)?;
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        items
            .lock()
            .map_err(|_| "Catalog registry unavailable")?
            .insert(id, Arc::new(Mutex::new(catalog)));
        return Ok(json!(id));
    }
    let handle = args["handle"].as_u64().ok_or("Missing catalog handle")?;
    if op == "catalog_release" {
        return Ok(json!(
            items
                .lock()
                .map_err(|_| "Catalog registry unavailable")?
                .remove(&handle)
                .is_some()
        ));
    }
    let owner = items
        .lock()
        .map_err(|_| "Catalog registry unavailable")?
        .get(&handle)
        .cloned()
        .ok_or("Unknown catalog generation")?;
    let mut owner = owner.lock().map_err(|_| "Catalog unavailable")?;
    if let Some(op) = op.strip_prefix("catalog_assets_") {
        return crate::assets::request(
            owner.assets.as_mut().ok_or("Assets unavailable in demo")?,
            &format!("assets_{op}"),
            args,
        );
    }
    match op {
        "catalog_runtime_guide" => crate::runtime_context::Context::new(
            &mut owner,
            args.get("library").cloned().unwrap_or(Value::Null),
        )?
        .compare(&args["guide"]),
        "catalog_sources" => Ok(owner.sources.as_ref().clone()),
        "catalog_overview" => Ok(owner.overview()),
        "catalog_tree" => owner.tree(
            args["tree_id"].as_str(),
            args["include_snapshot"].as_bool().unwrap_or(true),
        ),
        "catalog_technologies" => owner.technology_catalog(),
        "catalog_technology_tree" => {
            owner.technology_tree(args["folder_id"].as_str().unwrap_or(""))
        }
        "catalog_entities" => owner.identity_data(),
        "catalog_compatibility" => owner.guide_compatibility(&args["guide"]),
        "catalog_word_inventory" => owner.word_inventory(Path::new(
            args["path"].as_str().ok_or("Missing Word source")?,
        )),
        "catalog_export" => {
            let value = owner.identity_data()?;
            crate::entity_catalog::export(&value["entries"], &owner.sources["reference_labels"])
        }
        _ => Err("Unknown catalog operation".into()),
    }
}
