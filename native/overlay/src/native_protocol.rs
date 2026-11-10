//! Direct Rust HUD transport. The backend owns the active guide, runtime and
//! observed GUI; this process owns capture, OCR, local fallback and rendering.
use crate::local_api::{Api, Library};
use serde_json::{Map, Value, json};
use std::collections::HashSet;
type Result<T> = std::result::Result<T, String>;
pub struct Client {
    api: Api,
    library: Option<Library>,
    vision: Option<u64>,
    source: Value,
    catalog: Value,
    trees: Map<String, Value>,
    assets: Map<String, Value>,
    _subscription: crate::subscription::Subscription,
    last: Value,
}
fn rows(value: &Value) -> &[Value] {
    value.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn escaped(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    const H: &[u8] = b"0123456789abcdef";
    for b in bytes {
        out.push(H[(b >> 4) as usize] as char);
        out.push(H[(b & 15) as usize] as char);
    }
    out
}
impl Client {
    pub fn diagnose(&mut self) -> Result<Value> {
        let snapshot = self.request(
            &json!({"schema_version":1,"kind":"snapshot","token":1}),
            &[],
        )?;
        let library = Library::open()?;
        let handle = library
            .request("vision_open", json!({"assets":{},"trees":{},"catalog":{}}))?
            .as_u64()
            .ok_or("Invalid native visual observer")?;
        let proof = (|| -> Result<Value> {
            let pixels = vec![0u8; 100 * 100 * 4];
            let args = json!({"width":100,"height":100,"raw_len":pixels.len(),"version":1,"guide_id":"diagnostic","guide_server_id":"diagnostic","view":[1,1,0,0,100,100],"proposals":[],"ocr":{"width":100,"height":100,"lines":[]}});
            let first = library.frame(handle, &args, &pixels)?;
            let second = library.frame(handle, &args, &pixels)?;
            if first["cache_hit"] != false
                || second["cache_hit"] != true
                || first["matches"] != json!([])
                || second["matches"] != json!([])
            {
                return Err("Native visual ABI smoke test failed".into());
            }
            Ok(
                json!({"schema_version":1,"engine":"rust-native-hud","resident_python":false,"snapshot":snapshot,"binary_frame_verified":true,"frame_cache_verified":true}),
            )
        })();
        let released = library.request("vision_release", json!({"handle":handle}));
        let result = proof?;
        if released? != true {
            return Err("Native visual observer release failed".into());
        }
        Ok(result)
    }
    pub fn probe(port: u16) -> Result<Option<Self>> {
        let api = Api { port };
        let (status, bytes) = match api.request("/api/backend", None, 65536) {
            Ok(value) => value,
            Err(_) => return Ok(None),
        };
        if status != 200 {
            return Ok(None);
        }
        let descriptor: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if descriptor["engine"] != "rust-backend" || descriptor["resident_python"] != false {
            return Ok(None);
        }
        if descriptor["native_hud_protocol"] != 1 {
            return Err(
                "Native backend HUD protocol is outdated; rebuild both applications".into(),
            );
        }
        Ok(Some(Self {
            api,
            library: None,
            vision: None,
            source: Value::Null,
            catalog: Value::Null,
            trees: Map::new(),
            assets: Map::new(),
            _subscription: crate::subscription::Subscription::start(port)?,
            last: json!({}),
        }))
    }
    fn source(&mut self, snapshot: &Value) -> Result<()> {
        let source = json!([snapshot["guide_server_id"], snapshot["source_generation"]]);
        if self.source != source {
            self.release();
            self.source = source;
            self.catalog = Value::Null;
            self.trees.clear();
            self.assets.clear();
        }
        let mut changed = false;
        if self.catalog.is_null() {
            self.catalog = self.api.json("/api/technologies", None)?;
            changed = true;
        }
        for p in rows(&snapshot["proposed"]) {
            if p["panel"] != "research" {
                continue;
            }
            let Some(folder) = p["folder"].as_str().filter(|f| !f.is_empty()) else {
                continue;
            };
            if !self.trees.contains_key(folder) {
                self.trees.insert(
                    folder.into(),
                    self.api.json(
                        &format!("/api/technology-tree?id={}", escaped(folder)),
                        None,
                    )?,
                );
                changed = true;
            }
        }
        let mut urls = HashSet::new();
        for folder in rows(&self.catalog["folders"]) {
            let tab = &folder["tab"];
            let sprite = tab
                .get("inactive_sprite")
                .filter(|v| v.as_object().is_some_and(|m| !m.is_empty()))
                .unwrap_or(&tab["sprite"]);
            if let Some(url) = sprite["url"].as_str() {
                urls.insert(url.to_owned());
            }
        }
        for tree in self.trees.values() {
            for node in rows(&tree["technologies"]) {
                if let Some(url) = node["sprite"]["url"].as_str() {
                    urls.insert(url.to_owned());
                }
            }
        }
        if urls.len() > 1024 {
            return Err("Too many HUD visual assets".into());
        }
        for url in urls {
            if !url.starts_with("/api/sprite/") || self.assets.contains_key(&url) {
                continue;
            }
            let (status, bytes) = self.api.request(&url, None, 2 * 1024 * 1024)?;
            if status != 200 {
                return Err("HUD asset not available in current source generation".into());
            }
            self.assets.insert(url, json!(hex(&bytes)));
            changed = true;
        }
        if self.library.is_none() {
            self.library = Some(Library::open()?);
        }
        if self.vision.is_none() || changed {
            let library = self.library.as_ref().unwrap();
            let next = library
                .request(
                    "vision_open",
                    json!({"assets":self.assets,"catalog":self.catalog,"trees":self.trees}),
                )?
                .as_u64()
                .ok_or("Invalid native visual observer")?;
            if let Some(previous) = self.vision.replace(next) {
                let _ = library.request("vision_release", json!({"handle":previous}));
            }
        }
        Ok(())
    }
    fn release(&mut self) {
        if let Some(handle) = self.vision.take()
            && let Some(library) = &self.library
        {
            let _ = library.request("vision_release", json!({"handle":handle}));
        }
    }
    pub fn request(&mut self, input: &Value, raw: &[u8]) -> Result<Value> {
        match self.inner(input, raw) {
            Ok(value) => {
                self.last = value.clone();
                Ok(value)
            }
            Err(error) => {
                self.release();
                Ok(unavailable(&self.last, input, &error))
            }
        }
    }
    fn inner(&mut self, input: &Value, raw: &[u8]) -> Result<Value> {
        if input["kind"] != "frame" {
            return self.api.overlay(input);
        }
        let snapshot = self
            .api
            .overlay(&json!({"schema_version":1,"kind":"snapshot","token":input["token"]}))?;
        let mut response = snapshot.clone();
        response["kind"] = json!("frame");
        response["view"] = input["view"].clone();
        response["matches"] = json!([]);
        response["cache_hit"] = json!(false);
        response["recognition_ms"] = json!(0);
        if snapshot["usable"] != true || snapshot["version"] != input["version"] {
            response["usable"] = json!(false);
            return Ok(response);
        }
        self.source(&snapshot)?;
        let mut args = input.clone();
        args["proposals"] = snapshot["proposed"].clone();
        args["guide_id"] = snapshot["guide_id"].clone();
        args["guide_server_id"] = snapshot["guide_server_id"].clone();
        let visual = self
            .library
            .as_ref()
            .unwrap()
            .frame(self.vision.unwrap(), &args, raw)?;
        let current = self
            .api
            .overlay(&json!({"schema_version":1,"kind":"snapshot","token":input["token"]}))?;
        let usable = current["usable"] == true
            && current["version"] == snapshot["version"]
            && current["guide_id"] == snapshot["guide_id"]
            && current["guide_server_id"] == snapshot["guide_server_id"];
        response
            .as_object_mut()
            .unwrap()
            .extend(current.as_object().unwrap().clone());
        response["kind"] = json!("frame");
        response["usable"] = json!(usable);
        response["matches"] = if usable {
            visual["matches"].clone()
        } else {
            json!([])
        };
        response["cache_hit"] = visual["cache_hit"].clone();
        response["recognition_ms"] = visual["recognition_ms"].clone();
        Ok(response)
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.release();
    }
}
fn unavailable(last: &Value, input: &Value, error: &str) -> Value {
    json!({"schema_version":1,"kind":input["kind"],"token":input["token"],"ok":true,"version":last["version"],"guide_id":last["guide_id"],"guide_server_id":last["guide_server_id"],"title":last["title"],"guides":[],"guide_filter":{"status":"unavailable","country":null},"usable":false,"proposed":[],"matches":[],"native_available":false,"suspend_visual":true,"error":error,"age_ms":null,"view":input["view"]})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lost_connection_clears_country_choices_without_switching_active_guide() {
        let last=json!({"guide_id":"hungary","guides":[{"id":"hungary"}],"guide_filter":{"status":"observed","country":"HUN"}});
        let response=unavailable(&last,&json!({"kind":"snapshot","token":7}),"Disconnected");
        assert_eq!(response["guides"],json!([]));
        assert_eq!(response["guide_filter"]["status"],"unavailable");
        assert_eq!(response["guide_filter"]["country"],Value::Null);
        assert_eq!(response["guide_id"],"hungary");
        assert_eq!(response["usable"],false);
        assert_eq!(response["matches"],json!([]));
    }
    #[test]
    fn folder_ids_cannot_escape_local_query_parameters() {
        assert_eq!(escaped("industry_folder"), "industry_folder");
        assert_eq!(escaped("x&source=foreign"), "x%26source%3Dforeign");
        assert_eq!(hex(&[0, 255, 16]), "00ff10");
    }
}
