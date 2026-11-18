//! Shared ordered Jomini document boundary. Reviewed recoveries are the same
//! content-pinned transformations as the parser CLI, never guessed repairs.
use serde_json::Value;
use std::path::Path;
type Result<T> = std::result::Result<T, String>;
pub fn read(path: &Path) -> Result<Value> {
    use std::io::Read;
    let mut data = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(80_000_001)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    serde_json::to_value(hoi4companion_parser::read_reviewed(&data)?).map_err(|e| e.to_string())
}
pub fn entries(node: &Value) -> impl Iterator<Item = (&str, &Value)> {
    node["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| Some((v[0].as_str()?, &v[1])))
}
pub fn scalar<'a>(node: &'a Value, key: &str) -> &'a str {
    entries(node)
        .find(|(k, _)| *k == key)
        .and_then(|(_, v)| v.as_str())
        .unwrap_or("")
}
pub fn all<'a>(node: &'a Value, key: &'a str) -> impl Iterator<Item = &'a Value> {
    entries(node).filter_map(move |(k, v)| (k == key).then_some(v))
}
pub fn first<'a>(node: &'a Value, key: &str) -> Option<&'a Value> {
    entries(node).find_map(|(k, v)| (k == key).then_some(v))
}
pub fn blocks<'a>(node: &'a Value, key: &'a str) -> impl Iterator<Item = &'a Value> {
    all(node, key).filter(|v| v.get("entries").is_some())
}
