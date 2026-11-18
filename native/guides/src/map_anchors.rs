//! Map anchors from installed game files: region (state) anchor points and
//! victory-point city positions. Static catalog data; the live projection is
//! `map_markers` in the backend.
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
type Result<T> = std::result::Result<T, String>;
pub fn anchors(game: &Path, mods: &[PathBuf]) -> Result<BTreeMap<u64, Value>> {
    let files = crate::installation::collect(game, mods, "map", ".txt")?;
    let mut out = BTreeMap::new();
    for path in files["files"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(PathBuf::from)
        .filter(|p| p.file_name().is_some_and(|n| n == "buildings.txt"))
    {
        if path.metadata().map_err(|e| e.to_string())?.len() > 16 * 1024 * 1024 {
            return Err("Map anchors exceed limit".into());
        }
        let mut csv = csv::ReaderBuilder::new()
            .has_headers(false)
            .delimiter(b';')
            .flexible(true)
            .from_path(path)
            .map_err(|e| e.to_string())?;
        for row in csv.records() {
            let row = row.map_err(|e| e.to_string())?;
            let Some(id) = row
                .get(0)
                .and_then(|s| s.trim_start_matches('\u{feff}').parse::<u64>().ok())
                .filter(|n| *n > 0)
            else {
                continue;
            };
            if !matches!(
                row.get(1),
                Some("arms_factory" | "industrial_complex" | "air_base")
            ) {
                continue;
            }
            let point = (2..5)
                .map(|i| row.get(i).and_then(|s| s.parse::<f64>().ok()))
                .collect::<Option<Vec<_>>>();
            if let Some(p) = point.filter(|p| p.iter().all(|v| v.is_finite() && v.abs() < 100000.))
            {
                out.entry(id).or_insert_with(|| json!(p));
            }
        }
    }
    Ok(out)
}
/// Victory-point icon position of each province (`map/unitstacks.txt`, type
/// 38): where the game draws a city's icon and name on the map.
pub fn city_points(game: &Path, mods: &[PathBuf]) -> Result<BTreeMap<u64, Value>> {
    let files = crate::installation::collect(game, mods, "map", ".txt")?;
    let mut out = BTreeMap::new();
    for path in files["files"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(PathBuf::from)
        .filter(|p| p.file_name().is_some_and(|n| n == "unitstacks.txt"))
    {
        if path.metadata().map_err(|e| e.to_string())?.len() > 64 * 1024 * 1024 {
            return Err("Map unit stacks exceed limit".into());
        }
        out.extend(victory_point_positions(&std::fs::read_to_string(&path).map_err(|e| e.to_string())?));
    }
    Ok(out)
}
fn victory_point_positions(text: &str) -> BTreeMap<u64, Value> {
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let mut cells = line.trim_start_matches('\u{feff}').split(';');
        let (Some(province), Some("38")) = (cells.next().and_then(|s| s.trim().parse::<u64>().ok()), cells.next().map(str::trim)) else {
            continue;
        };
        let point = cells.take(3).map(|s| s.trim().parse::<f64>().ok()).collect::<Option<Vec<_>>>();
        if let Some(p) = point.filter(|p| p.len() == 3 && p.iter().all(|v| v.is_finite() && v.abs() < 100000.))
            && province > 0
        {
            out.insert(province, json!(p));
        }
    }
    out
}
/// The state's marker point: the icon of its largest victory-point city, where
/// the game writes the city's name (ties: the lowest province). Without a
/// positioned city, the first building anchor of the state.
pub fn state_anchor(cities: &Value, points: &BTreeMap<u64, Value>, building: Option<&Value>) -> Option<(Value, Option<u64>)> {
    let city = cities
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| Some((c["province_id"].as_u64()?, c["victory_points"].as_f64().unwrap_or(0.))))
        .filter(|(province, _)| points.contains_key(province))
        .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)));
    match city {
        Some((province, _)) => Some((points[&province].clone(), Some(province))),
        None => building.map(|p| (p.clone(), None)),
    }
}
#[cfg(test)]
mod city_anchor_tests {
    use super::*;
    #[test]
    fn markers_sit_on_the_largest_city_icon_and_fall_back_to_buildings() {
        let points = victory_point_positions("6380;0;3390.00;10.82;1640.00;0.00;0.18\n6380;38;3387.48;10.79;1640.00;1.57;0.18\n9000;38;3400.0;10.0;1600.0;0;0\nbad;38;1;2;3\n");
        assert_eq!(points.len(), 2);
        let cities = json!([{"province_id":9000,"victory_points":1},{"province_id":6380,"victory_points":30}]);
        let building = json!([3428.0, 10.4, 1630.0]);
        assert_eq!(state_anchor(&cities, &points, Some(&building)), Some((json!([3387.48, 10.79, 1640.0]), Some(6380))));
        // A city without a position is skipped; none at all keeps the building anchor.
        let unplaced = json!([{"province_id":1234,"victory_points":50},{"province_id":9000,"victory_points":1}]);
        assert_eq!(state_anchor(&unplaced, &points, Some(&building)).unwrap().1, Some(9000));
        assert_eq!(state_anchor(&json!([]), &points, Some(&building)), Some((building.clone(), None)));
        assert_eq!(state_anchor(&json!([]), &points, None), None);
    }
}
