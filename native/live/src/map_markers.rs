//! Small map points from installed region anchors and a fresh native camera.
use crate::{gui_observer::Sample, live::Memory};
use serde_json::{Value, json};
type Result<T> = std::result::Result<T, String>;
pub fn project(
    m: [f64; 16],
    mut p: [f64; 3],
    center: f64,
    world_width: f64,
    size: [f64; 2],
) -> Option<[f64; 3]> {
    if !m
        .iter()
        .chain(p.iter())
        .chain([&center, &world_width])
        .all(|v| v.is_finite())
        || world_width <= 0.
        || size.iter().any(|n| !n.is_finite() || *n <= 0.)
    {
        return None;
    }
    p[0] += ((center - p[0]) / world_width).round() * world_width;
    let c: [f64; 4] =
        std::array::from_fn(|j| p[0] * m[j] + p[1] * m[4 + j] + p[2] * m[8 + j] + m[12 + j]);
    if c[3] <= 0.00001 {
        return None;
    }
    let x = (0.5 + 0.5 * c[0] / c[3]) * size[0];
    let y = (0.5 - 0.5 * c[1] / c[3]) * size[1];
    let z = c[2] / c[3];
    (x.is_finite()
        && y.is_finite()
        && z.is_finite()
        && z >= 0.
        && z <= 1.
        && x >= 8.
        && y >= 8.
        && x < size[0] - 8.
        && y < size[1] - 8.)
        .then_some([x, y, z])
}
pub fn geometry(
    memory: &mut impl Memory,
    base: u64,
    profile: &Value,
    proposals: &Value,
    view: &Value,
) -> Result<Value> {
    // (build, CInGameIdler slot, CGameGraphics slot, CMap slot, RTTI checked). HOI4 1.20
    // slots were found live by viewport/width agreement and confirmed by their RTTI.
    const BUILDS: [(&str, u64, u64, u64, bool); 2] = [
        ("709f916afbebcfe64a26678702691aeb40945d206074c2a80de8e9d601fce148", 0x33ddb08, 0x3500ed0, 0x33e7700, false),
        ("1beb2a38fe2d747e5b7adb9b9648e350b0b851845dd228ffb87d2d2ac1b63a9f", 0x33e3f88, 0x3507360, 0x33edb80, true),
    ];
    let Some(&(_, idler_rva, gfx_rva, map_rva, rtti)) = BUILDS.iter().find(|b| profile["sha256"] == b.0) else {
        return Err("Map camera is not qualified for this build".into());
    };
    let mut s = Sample::new(memory, base, profile);
    let idler = s.u(base + idler_rva)?;
    let gfx = s.u(base + gfx_rva)?;
    let map = s.u(base + map_rva)?;
    if rtti {
        s.typed(idler, "CInGameIdler")?;
        s.typed(gfx, "CGameGraphics")?;
        s.typed(map, "CMap")?;
    }
    let camera = s.u(idler + 0x760)?;
    let width = s.i(camera + 0x6c8)?;
    let height = s.i(camera + 0x6cc)?;
    let resolution = [s.i(gfx + 0x180)?, s.i(gfx + 0x184)?];
    if width < 1
        || height < 1
        || width > 8192
        || height > 8192
        || resolution != [width, height]
        || view[4] != width
        || view[5] != height
    {
        return Err("Map viewport does not match client".into());
    }
    let raw = s.read(camera + 0x90, 64)?;
    let matrix: [f64; 16] = std::array::from_fn(|i| {
        f32::from_le_bytes(raw[i * 4..i * 4 + 4].try_into().unwrap()) as f64
    });
    let center = s.f(camera + 0x190)?;
    let world_width = s.i(map + 0x40)? as f64;
    if !(128.0..=65536.0).contains(&world_width) {
        return Err("Invalid native map width".into());
    }
    let mut matches = vec![];
    for proposal in crate::hud_geometry::rows(proposals)
        .iter()
        .filter(|p| p["panel"] == "construction_map")
        .take(64)
    {
        let Some(a) = proposal["world_point"].as_array().filter(|p| p.len() == 3) else {
            continue;
        };
        let Some(point) = a.iter().map(Value::as_f64).collect::<Option<Vec<_>>>() else {
            continue;
        };
        if let Some(p) = project(
            matrix,
            [point[0], point[1], point[2]],
            center,
            world_width,
            [width as f64, height as f64],
        ) {
            let mut row = proposal.clone();
            row["rect"] = json!([p[0] - 6., p[1] - 6., p[0] + 6., p[1] + 6.]);
            row["basis"] = json!("native_map_projection");
            matches.push(row);
        }
    }
    s.verify()?;
    // Diagnostics only (HUD quality plan F1 step 1): a compact camera
    // signature, so the trace can tell a still camera from pan and zoom.
    let camera = [center, matrix[0], matrix[5], matrix[12], matrix[13]].map(signature);
    Ok(json!({"matches":matches,"bytes":s.used,"camera":camera}))
}
/// `value` rounded to 5 significant digits (keeps trace lines short).
fn signature(value: f64) -> f64 {
    format!("{value:.4e}").parse().unwrap_or(0.)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    struct Fixture(BTreeMap<u64, u8>);
    impl Fixture {
        fn put(&mut self, a: u64, b: &[u8]) {
            for (i, v) in b.iter().enumerate() {
                self.0.insert(a + i as u64, *v);
            }
        }
        fn u(&mut self, a: u64, v: u64) {
            self.put(a, &v.to_le_bytes());
        }
        fn i(&mut self, a: u64, v: i32) {
            self.put(a, &v.to_le_bytes());
        }
    }
    impl Memory for Fixture {
        fn read(&mut self, a: u64, n: usize) -> Result<Vec<u8>> {
            Ok((0..n)
                .map(|i| *self.0.get(&(a + i as u64)).unwrap_or(&0))
                .collect())
        }
        fn fixture_transport(&self) -> bool {
            true
        }
        fn class_name(&mut self, _: u64) -> Result<String> {
            Err("No fixture RTTI".into())
        }
    }
    #[test]
    fn native_camera_requires_build_and_viewport_then_verifies_every_read() {
        let base = 0x140000000;
        let mut m = Fixture(BTreeMap::new());
        m.u(base + 0x33ddb08, 0x400000);
        m.u(0x400760, 0x500000);
        m.u(base + 0x3500ed0, 0x600000);
        m.u(base + 0x33e7700, 0x700000);
        m.i(0x5006c8, 1000);
        m.i(0x5006cc, 500);
        m.i(0x600180, 1000);
        m.i(0x600184, 500);
        m.i(0x700040, 1000);
        for (i, v) in [
            1f32, 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
        ]
        .iter()
        .enumerate()
        {
            m.put(0x500090 + i as u64 * 4, &v.to_le_bytes());
        }
        let profile =
            json!({"sha256":"709f916afbebcfe64a26678702691aeb40945d206074c2a80de8e9d601fce148"});
        let proposals =
            json!([{"panel":"construction_map","state_id":219,"world_point":[0.,0.,0.5]}]);
        let view = json!([1, 2, 0, 0, 1000, 500]);
        let result = geometry(&mut m, base, &profile, &proposals, &view).unwrap();
        assert_eq!(
            result["matches"][0]["rect"],
            json!([494., 244., 506., 256.])
        );
        assert_eq!(result["camera"], json!([0., 1., 1., 0., 0.]));
        assert_eq!(signature(1234.5678), 1234.6);
        assert_eq!(signature(-0.000123456), -0.00012346);
        assert!(geometry(&mut m, base, &json!({"sha256":"other"}), &proposals, &view).is_err());
        // HOI4 1.20 slots are admitted only with their RTTI, which this fixture lacks.
        let v120 = json!({"sha256":"1beb2a38fe2d747e5b7adb9b9648e350b0b851845dd228ffb87d2d2ac1b63a9f"});
        m.u(base + 0x33e3f88, 0x400000);
        m.u(base + 0x3507360, 0x600000);
        m.u(base + 0x33edb80, 0x700000);
        assert!(geometry(&mut m, base, &v120, &proposals, &view).is_err());
        m.i(0x600180, 1200);
        assert!(geometry(&mut m, base, &profile, &proposals, &view).is_err());
    }
    #[test]
    fn projection_clips_wraps_and_tracks_camera() {
        let identity = [
            1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
        ];
        assert_eq!(
            project(identity, [0., 0., 0.5], 0., 100., [1000., 500.]),
            Some([500., 250., 0.5])
        );
        assert_eq!(
            project(identity, [100., 0., 0.5], 0., 100., [1000., 500.]),
            Some([500., 250., 0.5])
        );
        assert!(project(identity, [2., 0., 0.5], 0., 100., [1000., 500.]).is_none());
        let mut moved = identity;
        moved[12] = 0.5;
        assert_eq!(
            project(moved, [0., 0., 0.5], 0., 100., [1000., 500.]),
            Some([750., 250., 0.5])
        );
    }
}
