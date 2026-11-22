//! Persistent, passive ZIP collections. Reuses the single-guide compiler and
//! publication contract; collection preparation never changes the active run.
use super::*;
use std::path::Path;

fn read(path: &Path) -> Result<Value> {
    serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
fn save(path: &Path, value: &Value) -> Result<()> {
    crate::reference_library::atomic_write(
        path,
        &serde_json::to_vec(value).map_err(|e| e.to_string())?,
    )
}
fn identity(value: &str) -> Result<&str> {
    if value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        Ok(value)
    } else {
        Err("Invalid collection identity".into())
    }
}
fn title(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}
fn selection(documents: &Value) -> (Vec<String>, Option<String>) {
    let all = documents["documents"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    let contexts = all
        .iter()
        .filter(|p| {
            let p = title(p).to_lowercase();
            p.contains("abbreviation") || p.contains("glossary")
        })
        .map(|p| (*p).to_owned())
        .collect::<Vec<_>>();
    let context = (contexts.len() == 1).then(|| contexts[0].clone());
    let country = all
        .iter()
        .any(|p| format!("/{p}").to_lowercase().contains("/country guides/"));
    let guides = all
        .into_iter()
        .filter(|p| {
            if crate::advanced_references::is_advanced(p){return false}
            let lower = format!("/{p}").to_lowercase();
            !lower.contains("/tools/")
                && !contexts.iter().any(|c| c == *p)
                && (!country || lower.contains("/country guides/"))
        })
        .map(str::to_owned)
        .collect();
    (guides, context)
}
fn reference_count(value: &Value, kind: &str) -> usize {
    value[kind].as_array().map(Vec::len).unwrap_or(0)
}
fn unfinished(value: &Value) -> bool {
    value["status"] == "preparing" || value["status"] == "prepared"
        && value["auto_import"] == true && value["auto_finished"] != true
}
fn summary(mut value: Value) -> Value {
    let rows = value["items"]
        .as_array()
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    value["counts"] = json!({
        "total": rows.len(),
        "processed": rows.iter().filter(|r| r["status"] != "pending").count(),
        "ready": rows.iter().filter(|r| r["status"] == "ready").count(),
        "installed": rows.iter().filter(|r| r["status"] == "installed").count(),
        "dlc_unavailable": rows.iter().filter(|r| r["status"] == "dlc_unavailable").count(),
        "needs_revision": rows.iter().filter(|r| r["status"] == "needs_revision").count()
    });
    value
}

impl Service {
    pub(super) fn resume_collections(&self) {
        if self.stopped.load(Ordering::Acquire) {
            return;
        }
        if let Ok(entries) = fs::read_dir(self.collections_root()) {
            for entry in entries.flatten() {
                let id = entry.file_name().to_string_lossy().into_owned();
                if identity(&id).is_ok()
                    && read(&entry.path().join("manifest.json"))
                        .is_ok_and(|v| unfinished(&v))
                {
                    let _ = self.start_collection(&id);
                    return;
                }
            }
        }
    }
    fn collections_root(&self) -> PathBuf {
        self.association_directory
            .parent()
            .unwrap_or(&self.association_directory)
            .join("collections")
    }
    fn collection_root(&self, id: &str) -> Result<PathBuf> {
        Ok(self.collections_root().join(identity(id)?))
    }
    pub(super) fn collections(&self) -> Result<Value> {
        let root = self.collections_root();
        let registry = self.app("app_guides", &json!({}))?;
        let installed = registry
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|r| r["id"].as_str())
            .collect::<std::collections::HashSet<_>>();
        let mut values = vec![];
        if root.exists() {
            for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let id = entry.file_name().to_string_lossy().into_owned();
                if identity(&id).is_err() {
                    continue;
                }
                let mut value = read(&entry.path().join("manifest.json"))?;
                if unfinished(&value) {
                    self.start_collection(&id)?;
                }
                if matches!(value["mio_install"]["status"].as_str(), Some("pending" | "waiting_for_game_exit")) {
                    let current = crate::mio_install::status(&self.references);
                    if current["status"] != "not_requested" {value["mio_install"] = current;}
                }
                // An item corrected through the existing editor needs no re-upload.
                for row in value["items"].as_array_mut().into_iter().flatten() {
                    if let Some(id) = row["guide_id"].as_str() {
                        if installed.contains(id) {
                            row["status"] = json!("installed");
                        }
                    }
                }
                values.push(summary(value));
            }
        }
        values.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
        Ok(json!({"schema_version":1,"collections":values}))
    }
    pub(super) fn collection_retry(&self, request: &Request) -> Result<Value> {
        let input = request.json_fields(&["id"])?;
        let id = input["id"].as_str().ok_or("Missing collection")?;
        let root = self.collection_root(id)?;
        let _guard = self
            .template_mutation
            .lock()
            .map_err(|_| "Library unavailable")?;
        let mut value = read(&root.join("manifest.json"))?;
        if value["status"] == "failed" || value["references_status"] == "needs_revision" {
            value["status"] = json!("preparing");
            value["import_stage"] = json!("validating");
            value["auto_finished"] = json!(false);
            value.as_object_mut().unwrap().remove("error");
            if value["references_status"] == "needs_revision" {
                value["references_status"] = json!("pending");
                value.as_object_mut().unwrap().remove("references_error");
            }
            save(&root.join("manifest.json"), &value)?;
        }
        self.start_collection(id)?;
        Ok(summary(value))
    }
    pub(super) fn collection_get(&self, request: &Request) -> Result<Value> {
        let id = request.query.get("id").ok_or("Missing collection")?;
        let root = self.collection_root(id)?;
        let value = read(&root.join("manifest.json"))?;
        if unfinished(&value) {
            self.start_collection(id)?;
        }
        Ok(summary(value))
    }
    pub(super) fn collection_item(&self, request: &Request) -> Result<Value> {
        let root = self.collection_root(request.query.get("id").ok_or("Missing collection")?)?;
        let key = request.query.get("key").ok_or("Missing guide")?;
        identity(key)?;
        read(&root.join("reports").join(format!("{key}.json")))
    }
    pub(super) fn collection_upload(&self, request: &Request) -> Result<Value> {
        let automatic = match request.query.get("automatic").map(String::as_str) {
            Some("1") => true, Some("0") | None => false,
            _ => return Err("Invalid automatic import option".into()),
        };
        let path = &request.upload.as_ref().ok_or("Choose a ZIP archive")?.path;
        let docs = crate::word_sources::documents(path)?;
        if docs["single_document"] == true {
            return Err(
                "Choose a collection ZIP; single Word files remain available in advanced import."
                    .into(),
            );
        }
        let (guides, context) = selection(&docs);
        let sizes = crate::word_sources::document_sizes(path)?;
        if guides.len() > 128
            || guides
                .iter()
                .map(|p| sizes.get(p).copied().unwrap_or(0))
                .sum::<u64>()
                > 64_000_000
        {
            return Err(
                "Collection exceeds 128 guides or 64 MB of Word documents; split the archive."
                    .into(),
            );
        }
        let assets = sizes.keys().any(|p| {
            Path::new(p)
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|s| ["txt", "png", "jpg", "jpeg"].contains(&s.to_lowercase().as_str()) || crate::advanced_references::is_advanced(p)&&["docx","xlsx"].contains(&s.to_lowercase().as_str()))
        });
        if guides.is_empty() && !assets {
            return Err(
                "No country guides, equipment designs or reference images in this ZIP".into(),
            );
        }
        let mut source = fs::File::open(path).map_err(|e| e.to_string())?;
        let mut hash = sha2::Sha256::new();
        std::io::copy(&mut source, &mut hash).map_err(|e| e.to_string())?;
        let id = format!("{:x}", hash.finalize());
        let root = self.collection_root(&id)?;
        let _guard = self
            .template_mutation
            .lock()
            .map_err(|_| "Library unavailable")?;
        if root.join("manifest.json").exists() {
            let mut manifest = read(&root.join("manifest.json"))?;
            if automatic && manifest["reviewed"] != true {
                manifest["auto_import"] = json!(true);
                save(&root.join("manifest.json"), &manifest)?;
            }
            let result = summary(manifest);
            self.start_collection(&id)?;
            return Ok(result);
        }
        fs::create_dir_all(root.join("reports")).map_err(|e| e.to_string())?;
        fs::copy(path, root.join("source.zip")).map_err(|e| e.to_string())?;
        let items = guides.iter().map(|p| json!({"key":format!("{:x}",sha2::Sha256::digest(p.as_bytes())),"entry":p,"title":title(p),"status":"pending"})).collect::<Vec<_>>();
        let name = request
            .query
            .get("name")
            .map(|p| {
                Path::new(p)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .chars()
                    .take(180)
                    .collect::<String>()
            })
            .unwrap_or_else(|| "ZIP collection".into());
        let manifest = json!({"schema_version":1,"id":id,"name":name,"status":"preparing","import_stage":"reading","auto_import":automatic,"context":context,"has_assets":assets,"references_status":if assets{"pending"}else{"none"},"items":items});
        save(&root.join("manifest.json"), &manifest)?;
        self.start_collection(&id)?;
        Ok(summary(manifest))
    }
    fn start_collection(&self, id: &str) -> Result<()> {
        let mut jobs = self
            .collection_jobs
            .lock()
            .map_err(|_| "Collection worker unavailable")?;
        if !jobs.is_empty() {
            return Ok(());
        } // One bounded compiler; the next list request resumes queued jobs.
        let owner = self.owner.upgrade().ok_or("Service unavailable")?;
        let id = id.to_owned();
        jobs.insert(id.clone());
        let failed_id = id.clone();
        if let Err(e) = thread::Builder::new()
            .name("hoi4-collection".into())
            .spawn(move || {
                let result = owner.prepare_collection(&id).and_then(|_| {
                    if owner.stopped.load(Ordering::Acquire) {return Ok(());}
                    let manifest = read(&owner.collection_root(&id)?.join("manifest.json"))?;
                    if manifest["status"] == "prepared" && manifest["auto_import"] == true && manifest["auto_finished"] != true {
                        owner.commit_collection(&id, true)?;
                    }
                    Ok(())
                });
                if let Err(error) = result {
                    if let Ok(root) = owner.collection_root(&id) {
                        if let Ok(mut value) = read(&root.join("manifest.json")) {
                            value["status"] = json!("failed");
                            value["import_stage"] = json!("failed");
                            value["error"] = json!(error);
                            let _ = save(&root.join("manifest.json"), &value);
                        }
                    }
                }
                if let Ok(mut jobs) = owner.collection_jobs.lock() {
                    jobs.remove(&id);
                }
                owner.resume_collections();
            })
        {
            jobs.remove(&failed_id);
            return Err(e.to_string());
        }
        Ok(())
    }
    fn prepare_collection(&self, id: &str) -> Result<()> {
        let root = self.collection_root(id)?;
        let file = root.join("manifest.json");
        let mut manifest = read(&file)?;
        if manifest["status"] != "preparing" {
            return Ok(());
        }
        if manifest["has_assets"] == true && manifest["references_status"] == "pending" {
            match crate::reference_library::import(
                &root.join("references"),
                &root.join("source.zip"),
            ) {
                Ok(refs) => {
                    manifest["references_status"] = json!("ready");
                    manifest["references"] = json!({"designs":reference_count(&refs,"designs"),"images":reference_count(&refs,"images"),"documents":reference_count(&refs,"documents"),"mio_queues":reference_count(&refs,"mio_queues")});
                }
                Err(e) => {
                    manifest["references_status"] = json!("needs_revision");
                    manifest["references_error"] = json!(e);
                }
            }
            save(&file, &manifest)?;
        }
        let mut library = crate::reference_library::manifest(&self.references);
        manifest["import_stage"] = json!("validating");
        save(&file, &manifest)?;
        let staged = crate::reference_library::manifest(&root.join("references"));
        for kind in ["designs", "images"] {
            library[kind]
                .as_array_mut()
                .ok_or("Invalid library")?
                .extend(staged[kind].as_array().into_iter().flatten().cloned());
        }
        let vocabulary = self.catalog(|c| Ok(c.vocabulary.clone()))?;
        let context = manifest["context"].as_str().map(str::to_owned);
        let total = manifest["items"]
            .as_array()
            .ok_or("Invalid collection")?
            .len();
        for n in 0..total {
            if self.stopped.load(Ordering::Acquire) {
                return Ok(());
            }
            if manifest["items"][n]["status"] != "pending" {
                continue;
            }
            let entry = manifest["items"][n]["entry"]
                .as_str()
                .ok_or("Missing document")?
                .to_owned();
            let key = manifest["items"][n]["key"].as_str().unwrap().to_owned();
            let compiled = (|| {
                let packet = crate::word_sources::prepare_with_vocabulary(
                    &root.join("source.zip"),
                    Some(&entry),
                    context.as_deref(),
                    &vocabulary,
                )?;
                let mut document = crate::import_document::document(&packet["document"])?;
                // Use the resolver's existing country inference before loading
                // scenario research, so preparation and publication validate the
                // same country rather than discovering it only at commit time.
                if document["guide"]["applies_to"]["country"]
                    .as_str()
                    .is_none()
                {
                    let country = self.catalog(|c| c.import_index()?.query(json!({"op":"country_hint","query":document["guide"]["title"],"guide":document["guide"]})))?;
                    if country.is_string() {
                        document["guide"]["applies_to"]["country"] = country;
                    }
                }
                let snapshot = self.catalog(|c| c.import_snapshot(&document["guide"]))?;
                let mut report =
                    snapshot.preview(&document["guide"], &document["sources"], &library);
                // Re-evaluate compatibility after country/tree inference, using the actual compiled definition.
                if !report["guide"].is_null() {
                    let compatibility =
                        self.catalog(|c| c.guide_compatibility(&report["guide"]))?;
                    report["availability"] =
                        crate::import_service::availability(&report, Some(&compatibility));
                    report["compatibility"] = compatibility;
                }
                report["sources"] = document["sources"].clone();
                report["source_warnings"] = packet["warnings"].clone();
                Ok::<_, String>(report)
            })();
            let row = &mut manifest["items"][n];
            match compiled {
                Ok(report) => {
                    save(&root.join("reports").join(format!("{key}.json")), &report)?;
                    let definition = if report["guide"].is_object() {
                        &report["guide"]
                    } else {
                        &report["draft"]
                    };
                    row["guide_id"] = definition["id"].clone();
                    row["applies_to"] = definition["applies_to"].clone();
                    row["status"] = report["availability"]["status"].clone();
                    row["compatibility"] = report["compatibility"].clone();
                    row["issues"] = report["errors"].clone();
                }
                Err(e) => {
                    row["status"] = json!("needs_revision");
                    row["issues"] = json!([e]);
                }
            }
            save(&file, &manifest)?;
        }
        manifest["status"] = json!("prepared");
        save(&file, &manifest)
    }
    pub(super) fn collection_commit(&self, request: &Request) -> Result<Value> {
        let input = request.json_fields(&["id", "reviewed", "automatic"])?;
        if input["reviewed"] != true && input["automatic"] != true {
            return Err("Review the collection before adding it to the library".into());
        }
        self.commit_collection(input["id"].as_str().ok_or("Missing collection")?, input["automatic"] == true)
    }
    fn commit_collection(&self, id: &str, automatic: bool) -> Result<Value> {
        let root = self.collection_root(id)?;
        let _guard = self
            .template_mutation
            .lock()
            .map_err(|_| "Library unavailable")?;
        let file = root.join("manifest.json");
        let mut manifest = read(&file)?;
        if manifest["status"] != "prepared" {
            return Err("Wait for collection preparation to finish".into());
        }
        manifest["import_stage"] = json!("saving");
        save(&file, &manifest)?;
        if manifest["references_status"] == "ready" {
            let refs = crate::reference_library::merge(&self.references, &root.join("references"))?;
            self.refresh_guide_context(refs)?;
            manifest["references_status"] = json!("installed");
            save(&file, &manifest)?;
        }
        for n in 0..manifest["items"].as_array().unwrap().len() {
            let row = &manifest["items"][n];
            if row["status"] == "installed" || row["status"] == "pending" {
                continue;
            }
            let key = row["key"].as_str().ok_or("Missing document")?.to_owned();
            let Ok(old) = read(&root.join("reports").join(format!("{key}.json"))) else {
                continue;
            };
            if old["guide"].is_null() {
                continue;
            }
            let id = old["guide"]["id"]
                .as_str()
                .ok_or("Invalid guide identity")?;
            if self.app("app_definition", &json!({"guide_id":id})).is_ok() {
                manifest["items"][n]["status"] = json!("installed");
                continue;
            }
            let installed = (|| -> Result<()> {
                let snapshot = self.catalog(|c| c.import_snapshot(&old["guide"]))?;
                let report = snapshot.preview(
                    &old["guide"],
                    &old["sources"],
                    &crate::reference_library::manifest(&self.references),
                );
                let row = &mut manifest["items"][n];
                row["status"] = report["availability"]["status"].clone();
                row["issues"] = report["errors"].clone();
                if report["valid"] == true && report["compatibility"]["blocked"] != true {
                    let mut app = self
                        .application
                        .lock()
                        .map_err(|_| "Application unavailable")?;
                    app.can_add_guide(id)?;
                    let result = crate::import_service::publish(
                        &report["guide"],
                        &self.guide_directory,
                        &self.bundled_guides,
                    );
                    match result {
                        Ok(published) => {
                            app.put_guide(&report["guide"])?;
                            published.commit();
                            row["status"] = json!("installed");
                            if let Some(interpretation) = report.get("interpretation") {
                                row["interpretation"] = json!({
                                    "verification": interpretation["verification"],
                                    "first_uninterpreted": interpretation["uninterpreted"][0]["key"]
                                });
                            }
                        }
                        Err(e) => {
                            row["status"] = json!("needs_revision");
                            row["issues"] = json!([e]);
                        }
                    }
                }
                Ok(())
            })();
            if let Err(error) = installed {
                // One country/script failure must not abort unrelated guides.
                // The publication guard rolls back its own uncommitted file.
                let row = &mut manifest["items"][n];
                row["status"] = json!("needs_revision");
                row["issues"] = json!([error]);
                let mut failed = old.clone();
                failed["valid"] = json!(false);
                failed["errors"] = row["issues"].clone();
                save(&root.join("reports").join(format!("{key}.json")), &failed)?;
            }
            save(&file, &manifest)?;
        }
        manifest["reviewed"] = json!(true);
        manifest["review_mode"] = json!(if automatic {"automatic"} else {"manual"});
        manifest["auto_finished"] = json!(true);
        manifest["import_stage"] = json!("complete");
        let library=crate::reference_library::manifest(&root.join("references"));
        if !library["mio_queues"].as_array().unwrap().is_empty(){
            manifest["mio_install"]=self.install_mio_presets().unwrap_or_else(|error|json!({"status":"error","error":error}));
        }
        save(&file, &manifest)?;
        Ok(summary(manifest))
    }
    pub(super) fn install_mio_presets(&self)->Result<Value>{
        let docs=self.catalog(|c|Ok(c.sources["settings"]["docs_dir"].clone()))?;
        let library=crate::reference_library::manifest(&self.references);
        crate::mio_install::request(&self.references,docs.as_str().ok_or("Configure the HOI4 documents folder first")?,library["mio_queues"].as_array().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn automatic_jobs_resume_publication_but_not_finished_or_legacy_reviews() {
        assert!(unfinished(&json!({"status":"preparing"})));
        assert!(unfinished(&json!({"status":"prepared","auto_import":true})));
        assert!(!unfinished(&json!({"status":"prepared","auto_import":true,"auto_finished":true})));
        assert!(!unfinished(&json!({"status":"prepared","reviewed":false})));
        assert!(!unfinished(&json!({"status":"failed","auto_import":true})));
    }
    #[test]
    fn selection_keeps_country_guides_and_uses_unique_glossary() {
        let docs = json!({"documents":["Root/Country Guides/Hungary.docx","Root/Country Guides/Germany.docx","Root/Tools/Abbreviations.docx","Root/Other.docx"]});
        let (guides, context) = selection(&docs);
        assert_eq!(guides.len(), 2);
        assert_eq!(context.as_deref(), Some("Root/Tools/Abbreviations.docx"));
        assert_eq!(title(&guides[0]), "Hungary");
    }
    #[test]
    fn dlc_is_not_revision_and_identity_rejects_paths() {
        let result = summary(
            json!({"items":[{"status":"dlc_unavailable"},{"status":"needs_revision"},{"status":"ready"}]}),
        );
        assert_eq!(result["counts"]["needs_revision"], 1);
        assert!(identity("../manifest.json").is_err());
        assert!(identity(&"a".repeat(64)).is_ok());
    }
    #[test]
    fn advanced_documents_are_passive_references_not_country_campaigns(){
        let (guides,_)=selection(&json!({"documents":["ADVANCED GUIDES (NEW & UPDATING)/BEST LAND DOCTRINES.docx","ADVANCED GUIDES (NEW & UPDATING)/MIO PRIORITY SYSTEM.docx"]}));
        assert!(guides.is_empty());
        let (guides,_)=selection(&json!({"documents":["COUNTRY GUIDES/Finland.docx","ADVANCED GUIDES (NEW)/TIER LIST.docx"]}));
        assert_eq!(guides,vec!["COUNTRY GUIDES/Finland.docx"]);
    }
}
