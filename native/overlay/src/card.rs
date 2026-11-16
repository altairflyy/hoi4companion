//! Player controls for the step card: position, collapse and hide, plus the
//! header buttons. The card lives in its own non-activating window, so clicks
//! and drags on it reach the HUD only: HOI4 keeps the foreground and never
//! receives input from the HUD.
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// How long the "marked done" notice (with its Undo button) stays on the card.
pub const NOTICE_FOR: Duration = Duration::from_secs(8);

/// Remembered across restarts, in the HUD's data folder.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Prefs {
    /// Top-left corner as fractions of the game's client area; None = default
    /// place on the right under the theater selector.
    pub position: Option<[f32; 2]>,
    pub collapsed: bool,
    pub hidden: bool,
}

impl Prefs {
    pub fn path(status: &Path) -> PathBuf {
        status.with_file_name("overlay-card.json")
    }
    pub fn load(path: &Path) -> Self {
        std::fs::read(path).ok().and_then(|bytes| serde_json::from_slice::<Prefs>(&bytes).ok())
            .map(Prefs::sanitized).unwrap_or_default()
    }
    pub fn save(&self, path: &Path) {
        if let Some(parent) = path.parent() {let _ = std::fs::create_dir_all(parent);}
        let temporary = path.with_extension("json.tmp");
        if let Ok(bytes) = serde_json::to_vec_pretty(self)
            && std::fs::write(&temporary, bytes).is_ok() {
            let _ = std::fs::rename(temporary, path);
        }
    }
    fn sanitized(mut self) -> Self {
        if self.position.is_some_and(|p| !p.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v))) {
            self.position = None;
        }
        self
    }
}

/// Card rectangle in client coordinates: the remembered place (or the default
/// one) kept fully inside the game's client area.
pub fn place(prefs: &Prefs, default: (f32, f32), size: (f32, f32), client: (f32, f32)) -> [f32; 4] {
    let (width, height) = (size.0.min(client.0), size.1.min(client.1));
    let (x, y) = prefs.position.map_or(default, |[fx, fy]| (fx * client.0, fy * client.1));
    let x = x.clamp(0.0, (client.0 - width).max(0.0));
    let y = y.clamp(0.0, (client.1 - height).max(0.0));
    [x, y, x + width, y + height]
}

/// Remembered position for a card whose top-left is now at `left, top`.
pub fn position_of(left: f32, top: f32, client: (f32, f32)) -> Option<[f32; 2]> {
    (client.0 > 0.0 && client.1 > 0.0).then(|| [(left / client.0).clamp(0.0, 1.0), (top / client.1).clamp(0.0, 1.0)])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {Done, Collapse, Hide, Undo}

/// Result of the last "Done" click, shown on the card for `NOTICE_FOR`.
#[derive(Clone, Debug, PartialEq)]
pub struct Notice {
    pub text: String,
    /// Step that can still be unchecked from the card.
    pub undo: Option<(String, String)>,
    pub at: Instant,
}
impl Notice {
    pub fn live(&self, now: Instant) -> bool {now.saturating_duration_since(self.at) <= NOTICE_FOR}
}

/// The button under a point in card coordinates, if any.
pub fn hit(buttons: &[(Button, [f32; 4])], x: f32, y: f32) -> Option<Button> {
    buttons.iter().find(|(_, r)| x >= r[0] && x < r[2] && y >= r[1] && y < r[3]).map(|(b, _)| *b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_stays_inside_the_game_at_its_remembered_place() {
        let mut prefs = Prefs::default();
        assert_eq!(place(&prefs, (1500.0, 140.0), (360.0, 200.0), (1920.0, 1080.0)), [1500.0, 140.0, 1860.0, 340.0]);
        prefs.position = position_of(100.0, 50.0, (1920.0, 1080.0));
        let r = place(&prefs, (1500.0, 140.0), (360.0, 200.0), (1920.0, 1080.0));
        assert!((r[0] - 100.0).abs() < 0.01 && (r[1] - 50.0).abs() < 0.01);
        // Same relative place at another resolution, never outside the window.
        let r = place(&prefs, (0.0, 0.0), (360.0, 200.0), (1280.0, 720.0));
        assert!((r[0] - 1280.0 * 100.0 / 1920.0).abs() < 0.01);
        prefs.position = Some([0.99, 0.99]);
        let r = place(&prefs, (0.0, 0.0), (360.0, 200.0), (1280.0, 720.0));
        assert_eq!((r[2], r[3]), (1280.0, 720.0));
        let r = place(&prefs, (0.0, 0.0), (2000.0, 900.0), (1280.0, 720.0));
        assert_eq!(r, [0.0, 0.0, 1280.0, 720.0]);
    }

    #[test]
    fn preferences_round_trip_and_reject_damaged_positions() {
        let dir = std::env::temp_dir().join(format!("hoi4-card-{}", std::process::id()));
        let path = Prefs::path(&dir.join("overlay-status.json"));
        assert_eq!(path.file_name().unwrap(), "overlay-card.json");
        let prefs = Prefs {position: Some([0.25, 0.5]), collapsed: true, hidden: false};
        prefs.save(&path);
        assert_eq!(Prefs::load(&path), prefs);
        std::fs::write(&path, br#"{"position":[2.0,0.5],"collapsed":false,"hidden":true}"#).unwrap();
        assert_eq!(Prefs::load(&path), Prefs {position: None, collapsed: false, hidden: true});
        std::fs::write(&path, b"not json").unwrap();
        assert_eq!(Prefs::load(&path), Prefs::default());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn buttons_are_hit_only_inside_their_rectangles() {
        let buttons = [(Button::Done, [200.0, 8.0, 260.0, 30.0]), (Button::Hide, [300.0, 8.0, 322.0, 30.0])];
        assert_eq!(hit(&buttons, 210.0, 10.0), Some(Button::Done));
        assert_eq!(hit(&buttons, 321.9, 29.9), Some(Button::Hide));
        assert_eq!(hit(&buttons, 280.0, 10.0), None, "the rest of the card drags it");
        assert_eq!(hit(&buttons, 260.0, 10.0), None);
    }

    #[test]
    fn done_notice_expires() {
        let at = Instant::now();
        let notice = Notice {text: "Done".into(), undo: None, at};
        assert!(notice.live(at + NOTICE_FOR));
        assert!(!notice.live(at + NOTICE_FOR + Duration::from_millis(1)));
    }
}
