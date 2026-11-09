//! Bounded passive web/YouTube text intake. DNS answers are checked together,
//! curl connects to the selected address, redirects are revalidated individually.
use crate::{media_references, process_tools};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    net::{IpAddr, ToSocketAddrs},
    path::{Path, PathBuf},
};
pub const MAX_TEXT: usize = 180000;
fn a(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
pub fn global(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let o = ip.octets();
            let n = u32::from(ip);
            let in_net = |base: u32, bits: u32| n >> (32 - bits) == base >> (32 - bits);
            ![
                (0x00000000, 8),
                (0x0a000000, 8),
                (0x64400000, 10),
                (0x7f000000, 8),
                (0xa9fe0000, 16),
                (0xac100000, 12),
                (0xc0000000, 24),
                (0xc0000200, 24),
                (0xc0a80000, 16),
                (0xc6120000, 15),
                (0xc6336400, 24),
                (0xcb007100, 24),
                (0xe0000000, 4),
                (0xf0000000, 4),
            ]
            .iter()
            .any(|(base, bits)| in_net(*base, *bits))
                || o == [192, 0, 0, 9]
                || o == [192, 0, 0, 10]
        }
        IpAddr::V6(ip) => {
            if let Some(ip) = ip.to_ipv4_mapped() {
                return global(IpAddr::V4(ip));
            }
            let b = ip.segments();
            (b[0] & 0xe000) == 0x2000
                && !(b[0] == 0x2001 && (b[1] < 0x200 || b[1] == 0xdb8))
                && b[0] != 0x2002
                && !(b[0] == 0x3fff && b[1] < 0x1000)
        }
    }
}
pub fn public_url(value: &str) -> Result<url::Url, String> {
    if value.len() > 4096 || value.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("URL non valido".into());
    }
    let parsed = url::Url::parse(value).map_err(|_| "URL non valido")?;
    if !["http", "https"].contains(&parsed.scheme())
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some_and(|p| ![80, 443].contains(&p))
    {
        return Err(
            "Usa un URL HTTP/HTTPS pubblico senza credenziali e porte personalizzate".into(),
        );
    }
    Ok(parsed)
}
pub struct Network {
    curl: PathBuf,
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn temporary(suffix: &str) -> Result<Temporary, String> {
    let path = std::env::temp_dir().join(format!(
        "hoi4-source-{}-{suffix}",
        crate::progress_store::new_id()?
    ));
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    Ok(Temporary(path))
}
impl Network {
    pub fn system() -> Result<Self, String> {
        let curl = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .map(|p| p.join("System32/curl.exe"))
            .filter(|p| p.is_file())
            .or_else(|| process_tools::executable(None, "curl", None))
            .ok_or("curl non trovato per l’acquisizione HTTP")?;
        Ok(Self { curl })
    }
    pub fn fetch(&self, input: &str) -> Result<(Vec<u8>, String, String), String> {
        let mut current = input.to_owned();
        for redirects in 0..=4 {
            let parsed = public_url(&current)?;
            let host = parsed
                .host_str()
                .ok_or("URL non valido")?
                .trim_matches(['[', ']']);
            let port = parsed.port_or_known_default().ok_or("URL non valido")?;
            let addresses = (host, port)
                .to_socket_addrs()
                .map_err(|_| "Fonte non raggiungibile; puoi incollare il testo")?
                .collect::<Vec<_>>();
            if addresses.is_empty() || addresses.iter().any(|a| !global(a.ip())) {
                return Err("Indirizzi locali, privati o riservati non consentiti".into());
            }
            let body = temporary("body")?;
            let headers = temporary("headers")?;
            let address = addresses[0].ip();
            let pinned = match address {
                IpAddr::V6(_) => format!("[{address}]"),
                _ => address.to_string(),
            };
            let argv = vec![
                self.curl.to_string_lossy().into_owned(),
                "--disable".into(),
                "--silent".into(),
                "--show-error".into(),
                "--noproxy".into(),
                "*".into(),
                "--proto".into(),
                "=http,https".into(),
                "--max-time".into(),
                "18".into(),
                "--connect-timeout".into(),
                "18".into(),
                "--max-filesize".into(),
                "2000000".into(),
                "--header".into(),
                "Accept-Encoding: identity".into(),
                "--user-agent".into(),
                "HOI4Companion-GuideImporter/1".into(),
                "--resolve".into(),
                format!("{host}:{port}:{pinned}"),
                "--dump-header".into(),
                headers.0.to_string_lossy().into_owned(),
                "--output".into(),
                body.0.to_string_lossy().into_owned(),
                "--write-out".into(),
                "%{response_code}\\n%{content_type}\\n%{redirect_url}".into(),
                "--".into(),
                current.clone(),
            ];
            let output = process_tools::run(argv, 20000, 2_100_000)?;
            if output.status["returncode"] != 0 {
                return Err(
                    "Fonte non raggiungibile o troppo grande; puoi incollare il testo".into(),
                );
            }
            let mut lines = output.stdout.lines();
            let status = lines.next().unwrap_or("").parse::<u16>().unwrap_or(0);
            let mime = lines.next().unwrap_or("").to_owned();
            let location = lines.next().unwrap_or("");
            if [301, 302, 303, 307, 308].contains(&status) {
                if redirects == 4 {
                    return Err("Troppi reindirizzamenti".into());
                }
                if location.is_empty() {
                    return Err("Reindirizzamento privo di destinazione".into());
                }
                current = parsed
                    .join(location)
                    .map_err(|_| "Reindirizzamento non valido")?
                    .to_string();
                continue;
            }
            if status != 200 {
                return Err(format!(
                    "Fonte non accessibile (HTTP {status}); puoi incollare il testo"
                ));
            }
            let mut header_bytes = vec![];
            std::fs::File::open(&headers.0)
                .map_err(|e| e.to_string())?
                .take(65537)
                .read_to_end(&mut header_bytes)
                .map_err(|e| e.to_string())?;
            if header_bytes.len() > 65536 {
                return Err("Intestazioni fonte troppo grandi".into());
            }
            let header_text = String::from_utf8_lossy(&header_bytes);
            if header_text
                .lines()
                .filter_map(|line| line.split_once(':'))
                .any(|(key, value)| {
                    key.eq_ignore_ascii_case("content-encoding") && value.trim() != "identity"
                })
            {
                return Err("Fonte troppo grande o codifica non supportata".into());
            }
            let mut data = vec![];
            std::fs::File::open(&body.0)
                .map_err(|e| e.to_string())?
                .take(2_000_001)
                .read_to_end(&mut data)
                .map_err(|e| e.to_string())?;
            if data.len() > 2_000_000 {
                return Err("Fonte troppo grande o codifica non supportata".into());
            }
            return Ok((data, mime, current));
        }
        Err("Troppi reindirizzamenti".into())
    }
}
pub fn html(text: &str) -> Result<String, String> {
    use lol_html::{RewriteStrSettings, doc_text, element, end_tag, rewrite_str};
    use std::{cell::RefCell, rc::Rc};
    let cleaned = rewrite_str(
        text,
        RewriteStrSettings::new().append_element_content_handler(element!(
            "script,style,noscript,svg,nav,footer",
            |e| {
                e.remove();
                Ok(())
            }
        )),
    )
    .map_err(|e| e.to_string())?;
    let output = Rc::new(RefCell::new(String::new()));
    let on_element = output.clone();
    let on_text = output.clone();
    rewrite_str(
        &cleaned,
        RewriteStrSettings::new()
            .append_element_content_handler(element!("*", move |element| {
                if ["p", "br", "div", "h1", "h2", "h3", "li", "tr", "article"]
                    .contains(&element.tag_name().as_str())
                {
                    on_element.borrow_mut().push('\n')
                }
                if element.can_have_content() {
                    let out = on_element.clone();
                    element.on_end_tag(end_tag!(move |_| {
                        out.borrow_mut().push(' ');
                        Ok(())
                    }))?;
                }
                Ok(())
            }))
            .append_document_content_handler(doc_text!(move |text| {
                on_text.borrow_mut().push_str(text.as_str());
                Ok(())
            })),
    )
    .map_err(|e| e.to_string())?;
    let collected = output.borrow();
    let decoded = html_escape::decode_html_entities(collected.as_str());
    let result = decoded
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    Ok(result)
}
pub fn captions(value: &Value) -> Vec<Value> {
    a(&value["events"])
        .iter()
        .filter_map(|event| {
            let text = a(&event["segs"])
                .iter()
                .map(|v| s(&v["utf8"]))
                .collect::<String>();
            let start = event["tStartMs"]
                .as_f64()
                .filter(|n| n.is_finite() && *n >= 0.)?;
            (!text.trim().is_empty()).then(|| json!({"start":start/1000.,"text":text.trim()}))
        })
        .collect()
}
pub fn ytdlp(package: Option<&Path>) -> Option<PathBuf> {
    let configured = std::env::var("HOI4_YTDLP").ok();
    let downloaded = std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .map(|p| p.join("Downloads/yt-dlp.exe"))
        .filter(|p| p.is_file());
    // Prefer the downloaded standalone executable to Python console-script shims.
    if configured.is_some() {
        process_tools::executable(configured.as_deref(), "yt-dlp", package)
    } else {
        downloaded.or_else(|| process_tools::executable(None, "yt-dlp", package))
    }
}
pub fn acquire(input: &Value, package: Option<&Path>) -> Result<Value, String> {
    let mut url = input.get("url").cloned().unwrap_or_else(|| json!(""));
    let text = input.get("text").cloned().unwrap_or_else(|| json!(""));
    let supplied = text.as_str().ok_or("Fonte non valida")?;
    let original = url.as_str().ok_or("Fonte non valida")?.to_owned();
    let mut warnings = vec![];
    let mut segments = vec![];
    let mut title = "Testo fornito".to_owned();
    let mut kind = "manual";
    let text;
    if !supplied.trim().is_empty() {
        text = supplied.to_owned();
        if !original.is_empty() {
            let parsed = url::Url::parse(&original).map_err(|_| "URL fonte non valido")?;
            if !["http", "https"].contains(&parsed.scheme())
                || parsed.host_str().is_none()
                || !parsed.username().is_empty()
                || parsed.password().is_some()
            {
                return Err("URL fonte non valido".into());
            }
            kind = if ["youtube.com", "www.youtube.com", "youtu.be"]
                .contains(&parsed.host_str().unwrap_or(""))
            {
                "youtube"
            } else {
                "web"
            };
        }
        warnings.push(json!(
            "Testo fornito manualmente, non verificato contro la pagina remota."
        ));
    } else if url::Url::parse(&original)
        .ok()
        .and_then(|p| p.host_str().map(str::to_owned))
        .is_some_and(|h| {
            [
                "youtube.com",
                "www.youtube.com",
                "m.youtube.com",
                "youtu.be",
            ]
            .contains(&h.as_str())
        })
    {
        let canonical = media_references::youtube(&original)?;
        let tool = ytdlp(package).ok_or(
            "yt-dlp non trovato: configura HOI4_YTDLP oppure incolla descrizione e trascrizione",
        )?;
        let mut argv = vec![
            tool.to_string_lossy().into_owned(),
            "--ignore-config".into(),
            "--no-plugin-dirs".into(),
            "--no-playlist".into(),
        ];
        if let Some(node) = process_tools::executable(None, "node", package) {
            argv.extend(["--js-runtimes".into(), format!("node:{}", node.display())]);
        }
        argv.extend(
            [
                "--skip-download",
                "--dump-single-json",
                "--socket-timeout",
                "12",
                "--retries",
                "1",
                "--",
            ]
            .into_iter()
            .map(str::to_owned),
        );
        argv.push(canonical.clone());
        let result = process_tools::run(argv, 55000, 12_000_000)?;
        if result.status["returncode"] != 0 {
            return Err("YouTube non accessibile con yt-dlp; nessun tentativo di login. Puoi incollare il testo".into());
        }
        let metadata: Value = serde_json::from_str(&result.stdout).map_err(|e| e.to_string())?;
        warnings.push(json!("Il video non è analizzato visivamente; i contenuti mostrati solo nei fotogrammi possono mancare."));
        for collection in [&metadata["subtitles"], &metadata["automatic_captions"]] {
            let mut languages = collection
                .as_object()
                .into_iter()
                .flatten()
                .map(|(k, _)| k)
                .collect::<Vec<_>>();
            languages.sort_by_key(|k| (!k.starts_with("it"), !k.starts_with("en"), *k));
            for language in languages {
                if let Some(caption) = a(&collection[language])
                    .iter()
                    .find(|c| c["ext"] == "json3")
                {
                    let attempt = Network::system()
                        .and_then(|n| n.fetch(s(&caption["url"])))
                        .and_then(|(data, _, _)| {
                            serde_json::from_slice::<Value>(&data).map_err(|e| e.to_string())
                        });
                    match attempt {
                        Ok(value) => {
                            segments = captions(&value);
                            if !segments.is_empty() {
                                warnings.push(json!(format!("Sottotitoli: {language}; possono contenere errori o essere automatici.")));
                                break;
                            }
                        }
                        Err(_) => {
                            warnings.push(json!(
                                "Sottotitoli non scaricabili; acquisita la descrizione disponibile."
                            ));
                            break;
                        }
                    }
                }
            }
            if !segments.is_empty() {
                break;
            }
        }
        if segments.is_empty() {
            warnings.push(json!(
                "Trascrizione assente: estrazione limitata alla descrizione."
            ));
        }
        let mut combined = s(&metadata["description"]).to_owned();
        if !segments.is_empty() {
            combined.push_str("\n\nTRASCRIZIONE\n");
            combined.push_str(
                &segments
                    .iter()
                    .map(|r| format!("[{}s] {}", r["start"].as_f64().unwrap_or(0.), s(&r["text"])))
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        text = combined;
        title = metadata["title"].as_str().unwrap_or("Video YouTube").into();
        url = json!(canonical);
        kind = "youtube";
    } else if !original.is_empty() {
        let (data, mime, final_url) = Network::system()?.fetch(&original)?;
        let pattern = crate::import_catalog::regex(r"charset=([\w-]+)");
        let name = pattern
            .captures(&mime)
            .map(|c| c[1].to_owned())
            .unwrap_or_else(|| "utf-8".into());
        let encoding = encoding_rs::Encoding::for_label(name.as_bytes())
            .ok_or("Codifica della fonte non supportata")?;
        let (decoded, _, _) = encoding.decode(&data);
        text = if mime.contains("html") {
            html(&decoded)?
        } else if mime.contains("text/plain") {
            decoded.into_owned()
        } else {
            return Err(
                "Usa una pagina HTML o testo; PDF e contenuti con login vanno convertiti in testo"
                    .into(),
            );
        };
        title = url::Url::parse(&final_url)
            .ok()
            .and_then(|p| p.host_str().map(str::to_owned))
            .unwrap_or_default();
        url = json!(final_url);
        kind = "web";
        warnings.push(json!(
            "Estratto il testo HTML statico; script, immagini e contenuti con login non acquisiti."
        ));
    } else {
        text = String::new()
    }
    if text.trim().is_empty() || text.chars().count() > MAX_TEXT {
        return Err(format!(
            "Testo fonte vuoto o maggiore di {MAX_TEXT} caratteri: suddividi la fonte"
        ));
    }
    let digest = format!("{:x}", Sha256::digest(text.as_bytes()));
    let mut source = json!({"id":format!("src-{}",&digest[..16]),"type":kind,"title":title,"text":text,"content_sha256":digest,"segments":segments,"warnings":warnings});
    if url != "" {
        source["url"] = url;
    }
    Ok(source)
}
impl Network {
    pub fn provider(&self, body: &Value, key: &str) -> Result<Value, String> {
        if key.is_empty() || key.len() > 16384 || key.chars().any(char::is_control) {
            return Err("Chiave API locale non valida".into());
        }
        let request = temporary("request.json")?;
        let authorization = temporary("authorization")?;
        let response = temporary("response.json")?;
        let payload = serde_json::to_vec(body).map_err(|e| e.to_string())?;
        if payload.len() > 4_000_000 {
            return Err("Richiesta API troppo grande".into());
        }
        std::fs::write(&request.0, payload).map_err(|e| e.to_string())?;
        std::fs::write(
            &authorization.0,
            format!("Authorization: Bearer {key}\nContent-Type: application/json\n"),
        )
        .map_err(|e| e.to_string())?;
        // Neither the key nor request content appears in argv, stdout, stderr or logs.
        let argv = vec![
            self.curl.to_string_lossy().into_owned(),
            "--disable".into(),
            "--silent".into(),
            "--show-error".into(),
            "--noproxy".into(),
            "*".into(),
            "--proto".into(),
            "=https".into(),
            "--max-time".into(),
            "90".into(),
            "--max-filesize".into(),
            "2000000".into(),
            "--header".into(),
            format!("@{}", authorization.0.display()),
            "--data-binary".into(),
            format!("@{}", request.0.display()),
            "--output".into(),
            response.0.to_string_lossy().into_owned(),
            "--write-out".into(),
            "%{response_code}".into(),
            "--".into(),
            "https://api.openai.com/v1/responses".into(),
        ];
        let output = process_tools::run(argv, 92000, 2_100_000)
            .map_err(|_| "API IA non raggiungibile o timeout; puoi usare il pacchetto esterno")?;
        if output.status["returncode"] != 0 {
            return Err(
                "API IA non raggiungibile o timeout; puoi usare il pacchetto esterno".into(),
            );
        }
        let status = output
            .stdout
            .trim()
            .parse::<u16>()
            .map_err(|_| "API IA: risposta HTTP non valida")?;
        if status != 200 {
            return Err(format!(
                "API IA: HTTP {status}. Controlla localmente modello, chiave e quota"
            ));
        }
        let mut data = vec![];
        std::fs::File::open(&response.0)
            .map_err(|e| e.to_string())?
            .take(2_000_001)
            .read_to_end(&mut data)
            .map_err(|e| e.to_string())?;
        if data.len() > 2_000_000 {
            return Err("Risposta IA troppo grande".into());
        }
        serde_json::from_slice(&data).map_err(|_| "Risposta IA non valida".into())
    }
}
