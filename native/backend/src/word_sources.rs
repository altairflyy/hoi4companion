//! Passive bounded Word/ZIP intake. Files are read, never extracted or executed.
use regex::Regex;
use roxmltree::{Document, Node};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    io::{Cursor, Read, Seek, SeekFrom},
    path::Path,
    sync::{Arc, Mutex, OnceLock},
};
use zip::ZipArchive;
type Result<T> = std::result::Result<T, String>;
const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
fn re(pattern: &str) -> Arc<Regex> {
    static CACHE: OnceLock<Mutex<std::collections::HashMap<String, Arc<Regex>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    if let Ok(values) = cache.lock()
        && let Some(value) = values.get(pattern)
    {
        return value.clone();
    }
    let value = Arc::new(Regex::new(pattern).expect("static source expression"));
    if let Ok(mut values) = cache.lock() {
        if values.len() >= 512 {
            values.clear();
        }
        values.insert(pattern.into(), value.clone());
    }
    value
}
pub(crate) fn safe(name: &str) -> bool {
    name.chars().count() <= 1200
        && !name.contains('\\')
        && !name.starts_with('/')
        && name
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != "..")
}
pub(crate) fn archive<R: Read + Seek>(reader: R) -> Result<ZipArchive<R>> {
    let archive =
        ZipArchive::new(reader).map_err(|_| "Documento Word o archivio ZIP non valido")?;
    if archive.len() > 5000 {
        return Err("Archivio con troppi elementi".into());
    }
    // zip 2.x maps names to their final entry. Check raw central records first
    // so duplicate names cannot hide behind that map. Decompression stays in zip.
    let offset = archive.central_directory_start();
    let mut reader = archive.into_inner();
    reader
        .seek(SeekFrom::Start(offset))
        .map_err(|e| e.to_string())?;
    let mut names = HashSet::new();
    let mut count = 0;
    loop {
        let mut signature = [0u8; 4];
        if reader.read_exact(&mut signature).is_err() || &signature != b"PK\x01\x02" {
            break;
        }
        let mut header = [0u8; 42];
        reader
            .read_exact(&mut header)
            .map_err(|_| "Archivio danneggiato")?;
        let length = u16::from_le_bytes([header[24], header[25]]) as usize;
        let extra = u16::from_le_bytes([header[26], header[27]]) as i64;
        let comment = u16::from_le_bytes([header[28], header[29]]) as i64;
        let mut name = vec![0; length];
        reader
            .read_exact(&mut name)
            .map_err(|_| "Archivio danneggiato")?;
        if !names.insert(name) {
            return Err("Archivio con elementi dal nome duplicato".into());
        }
        count += 1;
        if count > 5000 {
            return Err("Archivio con troppi elementi".into());
        }
        reader
            .seek(SeekFrom::Current(extra + comment))
            .map_err(|e| e.to_string())?;
    }
    reader.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    ZipArchive::new(reader).map_err(|_| "Documento Word o archivio ZIP non valido".into())
}
pub(crate) fn read_entry<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
    limit: u64,
) -> Result<Vec<u8>> {
    let mut entry = archive
        .by_name(name)
        .map_err(|_| "Archivio danneggiato o compressione non supportata")?;
    if entry.encrypted() || entry.size() > limit {
        return Err("Documento troppo grande o protetto da password".into());
    }
    let mut data = vec![];
    entry
        .by_ref()
        .take(limit + 1)
        .read_to_end(&mut data)
        .map_err(|_| "Archivio danneggiato o compressione non supportata")?;
    if data.len() as u64 > limit {
        return Err("Documento troppo grande".into());
    }
    Ok(data)
}
pub fn documents(path: &Path) -> Result<Value> {
    let mut archive = archive(std::fs::File::open(path).map_err(|e| e.to_string())?)?;
    if archive.file_names().any(|s| s.as_deref().unwrap_or("") == "word/document.xml") {
        return Ok(json!({"documents":[],"single_document":true}));
    }
    let mut result = vec![];
    for i in 0..archive.len() {
        let file = archive.by_index_raw(i).map_err(|e| e.to_string())?;
        let Ok(name) = file.name() else {
            continue;
        };
        if !file.is_dir()
            && safe(&name)
            && name.to_lowercase().ends_with(".docx")
            && file.size() <= 12_000_000
        {
            result.push(name.into_owned());
        }
    }
    result.sort_by_key(|s| hoi4companion_catalog_core::casefold(s));
    Ok(json!({"documents":result,"single_document":false}))
}
pub fn document_sizes(path: &Path) -> Result<std::collections::HashMap<String, u64>> {
    let mut archive = archive(std::fs::File::open(path).map_err(|e| e.to_string())?)?;
    let mut result = std::collections::HashMap::new();
    for i in 0..archive.len() {
        let file = archive.by_index_raw(i).map_err(|e| e.to_string())?;
        if let Ok(name) = file.name() {
            result.insert(name.into_owned(), file.size());
        }
    }
    Ok(result)
}
fn xml_text(bytes: &[u8]) -> Result<String> {
    if bytes.starts_with(b"\xff\xfe") || bytes.starts_with(b"\xfe\xff") {
        let little = bytes.starts_with(b"\xff\xfe");
        let bytes = &bytes[2..];
        if !bytes.len().is_multiple_of(2) {
            return Err("Documento Word danneggiato".into());
        }
        let values: Vec<_> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| {
                if little {
                    u16::from_le_bytes([p[0], p[1]])
                } else {
                    u16::from_be_bytes([p[0], p[1]])
                }
            })
            .collect();
        String::from_utf16(&values).map_err(|_| "Documento Word danneggiato".into())
    } else {
        String::from_utf8(bytes.to_vec()).map_err(|_| "Documento Word danneggiato".into())
    }
}
fn xml(source: &str) -> Result<Document<'_>> {
    if source.to_uppercase().contains("<!DOCTYPE") || source.to_uppercase().contains("<!ENTITY") {
        return Err("XML Word non supportato".into());
    }
    Document::parse_with_options(
        source,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 1_000_000,
        },
    )
    .map_err(|_| "Documento Word danneggiato".into())
}
fn word_tag(node: Node, name: &str) -> bool {
    node.is_element() && node.tag_name().namespace() == Some(W) && node.tag_name().name() == name
}
fn paragraph(node: Node) -> String {
    let mut result = String::new();
    for child in node.descendants() {
        if word_tag(child, "t") {
            result.push_str(child.text().unwrap_or(""));
        } else if word_tag(child, "tab") {
            result.push('\t');
        } else if word_tag(child, "br") || word_tag(child, "cr") {
            result.push('\n');
        }
    }
    result.trim().into()
}
type Groups = Vec<Vec<Vec<String>>>;
fn content(data: &[u8]) -> Result<(String, Groups, usize)> {
    let mut archive = archive(Cursor::new(data))?;
    if !archive.file_names().any(|s| s.as_deref().unwrap_or("") == "word/document.xml") {
        return Err("Contenuto Word mancante o duplicato".into());
    }
    let raw = xml_text(&read_entry(&mut archive, "word/document.xml", 8_000_000)?)?;
    let doc = xml(&raw)?;
    let body = doc
        .root_element()
        .children()
        .find(|n| word_tag(*n, "body"))
        .ok_or("Contenuto Word mancante")?;
    let media = archive
        .file_names()
        .filter(|s| {
            s.as_deref()
                .map(|name| name.starts_with("word/media/") && !name.ends_with('/'))
                .unwrap_or(false)
        })
        .count();
    let mut links = vec![];
    if archive
        .file_names()
        .any(|s| s.as_deref().unwrap_or("") == "word/_rels/document.xml.rels")
    {
        let raw = xml_text(&read_entry(
            &mut archive,
            "word/_rels/document.xml.rels",
            8_000_000,
        )?)?;
        let rels = xml(&raw)?;
        for rel in rels.root_element().children().filter(Node::is_element) {
            let target = rel.attribute("Target").unwrap_or("");
            if rel.attribute("TargetMode") == Some("External")
                && rel
                    .attribute("Type")
                    .is_some_and(|s| s.ends_with("/hyperlink"))
                && ["https://", "http://"]
                    .iter()
                    .any(|prefix| target.starts_with(prefix))
                && target.chars().count() <= 4096
            {
                links.push(target.to_owned());
            }
        }
    }
    let mut lines = vec![];
    let mut groups = vec![];
    for block in body.children().filter(Node::is_element) {
        if word_tag(block, "p") {
            let text = paragraph(block);
            if !text.is_empty() {
                lines.push(text);
            }
        } else if word_tag(block, "tbl") {
            let rows: Vec<Vec<Vec<String>>> = block
                .children()
                .filter(|n| word_tag(*n, "tr"))
                .map(|row| {
                    row.children()
                        .filter(|n| word_tag(*n, "tc"))
                        .map(|cell| {
                            cell.descendants()
                                .filter(|n| word_tag(*n, "p"))
                                .map(paragraph)
                                .filter(|s| !s.is_empty())
                                .collect()
                        })
                        .collect()
                })
                .collect();
            let Some(first) = rows.first() else {
                continue;
            };
            let headings: Vec<_> = first
                .iter()
                .map(|cell| {
                    cell.join(" ")
                        .trim()
                        .to_uppercase()
                        .trim_end_matches('*')
                        .to_owned()
                })
                .collect();
            let priority = headings == vec!["ACTIONS", "RESEARCH PRIORITY", "FOCUS PRIORITY"];
            for row in &rows {
                lines.push("[Table row]".into());
                for (i, cell) in row.iter().enumerate() {
                    lines.push(format!("[Column {}]", i + 1));
                    lines.extend(cell.iter().cloned());
                }
            }
            if priority {
                groups.extend(rows.into_iter().skip(1).filter(|r| r.len() == 3));
            }
        }
    }
    if !links.is_empty() {
        lines.push("[Document links]".into());
        let mut seen = HashSet::new();
        lines.extend(links.into_iter().filter(|s| seen.insert(s.clone())));
    }
    let text = lines.join("\n");
    if text.is_empty() || text.chars().count() > 180000 {
        return Err("Testo Word vuoto o superiore al limite di importazione".into());
    }
    Ok((text, groups, media))
}
fn priority_target(line: &str) -> String {
    let mut target = line.trim().trim_end_matches('*').trim().to_owned();
    while target.ends_with(')') {
        let mut depth = 0i32;
        let mut start = None;
        for (i, c) in target.char_indices().rev() {
            if c == ')' {
                depth += 1;
            } else if c == '(' {
                depth -= 1;
            }
            if depth == 0 {
                start = Some(i);
                break;
            }
        }
        if let Some(i) = start {
            target = target[..i].trim().trim_end_matches('*').trim().into();
        } else {
            break;
        }
    }
    if target.starts_with('(')
        && let Some(end) = target.find(')').filter(|i| *i > 0)
    {
        target = target[end + 1..].trim().into();
    }
    re(r"(?i)^(?:ASAP\s*:\s*|On\s+[^:]+:\s*)")
        .replace(&target, "")
        .trim()
        .into()
}
pub(crate) fn explicit_action(text: &str) -> Option<Value> {
    let value = re(r"^\s*-\s*").replace(text, "").trim().to_owned();
    let law = re(r"(?i)^(.+?)(?:\s*\((\d+)\s*PP\))?\s*$");
    let laws = [
        "Civilian Economy",
        "Early Mobilization",
        "Partial Mobilization",
        "War Economy",
        "Total Mobilization",
        "Disarmed Nation",
        "Volunteer Only",
        "Limited Conscription",
        "Extensive Conscription",
        "Service by Requirement",
        "All Adults Serve",
        "Scraping the Barrel",
        "Free Trade",
        "Export Focus",
        "Limited Exports",
        "Closed Economy",
    ];
    if let Some(hit) = law.captures(&value)
        && let Some(law) = laws.iter().find(|s| s.eq_ignore_ascii_case(hit[1].trim()))
    {
        let mut action = json!({"type":"law","target":law});
        if let Some(cost) = hit.get(2).and_then(|s| s.as_str().parse::<u64>().ok()) {
            action["political_power_cost"] = json!(cost);
        }
        return Some(action);
    }
    if re(r"(?i)^create\s+(?:an?\s+)?(?:intelligence\s+)?agency$").is_match(&value) {
        return Some(json!({"type":"create_agency"}));
    }
    if let Some(hit) =
        re(r"(?i)^build\s+(\d+)\s+(military|civilian)\s+factor(?:y|ies)\s+in\s+([^,:;()]+)$")
            .captures(&value)
        && let Ok(quantity) = hit[1].parse::<u64>()
        && (1..=1000).contains(&quantity)
        && !re(r"(?i)\b(?:and|then|or|when|after)\b").is_match(&hit[3])
    {
        return Some(
            json!({"type":"build","building":format!("{} factory",hit[2].to_lowercase()),"state":hit[3].trim(),"quantity":{"mode":"add","value":quantity}}),
        );
    }
    re(r"(?i)^event\s*:\s*([^;\n]+?)\s*;\s*(?:choose|select)\s*:\s*([^;\n]+?)\s*$")
        .captures(&value)
        .map(|hit| json!({"type":"event","event":hit[1].trim(),"option":hit[2].trim()}))
}
pub(crate) fn abbreviation_hint(text: &str, vocabulary: &Value) -> Option<String> {
    let build = re(r"(?i)\b(?:build|construct|construction)\b").is_match(text);
    let production =
        re(r"(?i)\b(?:produce|production|producing|assign(?:ment|ed)?)\b").is_match(text);
    if build == production {
        return None;
    }
    let domain = if build { "construction" } else { "production" };
    let mut found = vec![];
    for (code, meaning) in vocabulary["abbreviations"][domain]
        .as_object()
        .into_iter()
        .flatten()
    {
        let pattern = re(&format!("(?i){}s?", regex::escape(code)));
        if pattern.find_iter(text).any(|m| {
            (m.start() == 0 || !text.as_bytes()[m.start() - 1].is_ascii_alphanumeric())
                && text
                    .as_bytes()
                    .get(m.end())
                    .is_none_or(|b| !b.is_ascii_alphanumeric())
        }) {
            found.push(format!("{code} = {}", meaning.as_str().unwrap_or("")));
        }
    }
    (!found.is_empty()).then(|| format!("Abbreviations ({domain}): {}", found.join("; ")))
}
fn add(steps: &mut Vec<Value>, source: &str, typ: &str, line: &str, extra: Value) {
    let mut step = json!({"key":format!("word-{:04}",steps.len()+1),"type":typ,"evidence":[{"source_id":source,"basis":"explicit","quote":line}]});
    step.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    steps.push(step);
}
pub fn prepare(path: &Path, entry: Option<&str>, context: Option<&str>) -> Result<Value> {
    prepare_with_vocabulary(path, entry, context, crate::import_catalog::language())
}
pub fn prepare_with_vocabulary(
    path: &Path,
    entry: Option<&str>,
    context: Option<&str>,
    vocabulary: &Value,
) -> Result<Value> {
    let context = context.filter(|s| !s.is_empty());
    if context.is_some() && context == entry {
        return Err("La fonte di contesto deve essere diversa dalla guida".into());
    }
    let mut archive = archive(std::fs::File::open(path).map_err(|e| e.to_string())?)?;
    let (data, title) = if archive.file_names().any(|s| s.as_deref().unwrap_or("") == "word/document.xml") && entry.is_none()
    {
        let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let mut data = vec![];
        file.by_ref()
            .take(12_000_001)
            .read_to_end(&mut data)
            .map_err(|e| e.to_string())?;
        if data.len() > 12_000_000 {
            return Err("Documento Word troppo grande".into());
        }
        (data, "Word guide.docx".to_owned())
    } else {
        let name = entry
            .filter(|s| safe(s) && s.to_lowercase().ends_with(".docx"))
            .ok_or("Seleziona un documento Word presente nell’archivio")?;
        if !archive.file_names().any(|s| s.as_deref().unwrap_or("") == name) {
            return Err("Documento assente o duplicato nell’archivio".into());
        }
        (
            read_entry(&mut archive, name, 12_000_000)?,
            name.rsplit('/').next().unwrap().into(),
        )
    };
    let (text, groups, media) = content(&data)?;
    let digest = format!("{:x}", Sha256::digest(text.as_bytes()));
    let source_id = format!("src-{}", &digest[..16]);
    let source = json!({"id":source_id,"type":"file","title":title,"text":text,"content_sha256":digest,"segments":[],"warnings":["Il testo è estratto dal documento Word; immagini incorporate e allegati separati non sono interpretati automaticamente."]});
    let mut steps = vec![];
    let mut ordinal = 0;
    for row in &groups {
        for line in &row[0] {
            ordinal += 1;
            let mut action = explicit_action(line).unwrap_or(json!({"type":"reminder"}));
            let typ = action
                .as_object_mut()
                .unwrap()
                .remove("type")
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned();
            action["title"] = json!(line.chars().take(180).collect::<String>());
            action["instructions"] = json!([line]);
            if let Some(hint) = abbreviation_hint(line, vocabulary) {
                action["note"] = json!(hint);
            }
            add(&mut steps, &source_id, &typ, line, action);
            let step = steps.last_mut().unwrap();
            step["details"] = json!({"source_action":{"source_id":source_id,"key":step["key"],"ordinal":ordinal}});
        }
        for (lane, values) in [("research", &row[1]), ("focus", &row[2])] {
            for line in values {
                let mut target = priority_target(line);
                let choice = if target.is_empty() {
                    line.trim().trim_matches(['(', ')']).trim()
                } else {
                    &target
                };
                let title = line.chars().take(180).collect::<String>();
                if re(r"(?i)^free(?:\s*$|\s*[/–—:-])").is_match(choice)
                    || re(r"(?i)^[\w -]+\s*/\s*free$").is_match(choice)
                    || re(r"(?i)^future focuses are free/optional$").is_match(choice)
                {
                    add(
                        &mut steps,
                        &source_id,
                        "free",
                        line,
                        json!({"lane":lane,"title":title,"note":line}),
                    );
                    continue;
                }
                if re(r"(?i)^(?:no focus.*|no research.*|nothing|(?:my )?suggestions?\s*:?|suggested\s*:|air techs|industry upgrades)$").is_match(choice)||re(r"(?i)/\s*feel free to pick another one or none\s*$").is_match(choice){add(&mut steps,&source_id,"reminder",line,json!({"title":title,"instructions":[line]}));continue;}
                if target.is_empty() {
                    if lane == "focus" {
                        add(
                            &mut steps,
                            &source_id,
                            "focus",
                            line,
                            json!({"target":line,"title":title,"note":line}),
                        );
                    } else {
                        add(
                            &mut steps,
                            &source_id,
                            "reminder",
                            line,
                            json!({"title":title,"instructions":[line]}),
                        );
                    }
                    continue;
                }
                let typ = if lane == "research" && re(r"(?i)\bspecial project\b").is_match(line) {
                    "special_project"
                } else {
                    lane
                };
                if typ == "special_project" {
                    for pattern in [
                        r"(?i)^\s*special\s+project\s*:\s*",
                        r"(?i)^\s*ASAP\s*:\s*",
                        r"(?i)\s*\(\s*special\s+project\s*,\s*(?:get|assign|hire|recruit)\s+(?:an?\s+)?scientist\s*\)",
                        r"(?i)\s*\(\s*special\s+project\s*\)",
                        r"(?i)\s*\(\s*(?:get|assign|hire|recruit)\s+(?:an?\s+)?scientist(?:\s*\(\s*\d+\s*PP\s*\))?\s*\)",
                        r"(?i)\s*\(\s*(?:optional|if\s+(?:possible|available)|(?:might|may|can)\s+be\s+later)\s*\)\s*$",
                        r"(?i)\s*\(\s*(?:get|assign|hire|recruit)\s+(?:an?\s+)?scientist\s*,\s*optional\s*\)\s*$",
                    ] {
                        target = re(pattern).replace_all(&target, "").into_owned();
                    }
                    target = target.trim().into();
                    if re(r"(?i)^flame\s+tanks?$").is_match(&target) {
                        target = "Flamethrower Tank".into();
                    }
                    if target.is_empty() {
                        add(
                            &mut steps,
                            &source_id,
                            "reminder",
                            line,
                            json!({"title":title,"instructions":[line]}),
                        );
                        continue;
                    }
                }
                let mut extra = json!({"target":target,"title":title});
                if target != *line {
                    extra["note"] = json!(line);
                }
                add(&mut steps, &source_id, typ, line, extra);
            }
        }
    }
    if steps.is_empty() {
        let quote = text.chars().take(2000).collect::<String>();
        add(
            &mut steps,
            &source_id,
            "reminder",
            &quote,
            json!({"title":"Review source document","instructions":["Read the source document; no recognized priority table was found."]}),
        );
    }
    let mut sources = vec![source];
    if let Some(context) = context {
        let other = prepare(path, Some(context), None)?["sources"][0].clone();
        if other["id"] == sources[0]["id"] {
            return Err("La fonte di contesto duplica la guida".into());
        }
        sources.push(other);
    }
    let document = json!({"format":"hoi4-guide-authoring","authoring_schema_version":1,"sources":sources,"guide":{"id":format!("word-{}",&format!("{:x}",Sha256::digest(&data))[..16]),"title":title,"applies_to":{},"dependency_policy":"explicit","assumptions":["Draft generated from Word priority columns; review context, conditions, references and ID bindings. Other sections are retained in the source and are not converted to steps."],"steps":steps}});
    Ok(
        json!({"sources":sources,"document":document,"priority_tables":groups.len(),"embedded_images":media,"warnings":["La bozza usa solo le colonne prioritarie riconosciute; le altre sezioni restano nella fonte e richiedono revisione.","Solo alcune istruzioni esplicite sono strutturate; condizioni e indicazioni nei promemoria non sono monitorate automaticamente."]}),
    )
}
pub fn execute(op: &str, args: &Value) -> Result<Value> {
    let path = Path::new(args["path"].as_str().ok_or("Missing document path")?);
    if path.metadata().map_err(|e| e.to_string())?.len() > 200_000_000 {
        return Err("File vuoto o superiore a 200 MB".into());
    }
    match op {
        "word_documents" => documents(path),
        "word_prepare" => prepare_with_vocabulary(
            path,
            args["entry"].as_str(),
            args["context"].as_str(),
            args.get("vocabulary")
                .unwrap_or_else(|| crate::import_catalog::language()),
        ),
        _ => Err("Unknown Word operation".into()),
    }
}
