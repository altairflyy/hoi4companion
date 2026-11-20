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
    /// Hidden for this run only: never saved, so a card hidden once (Hide or
    /// Ctrl+Alt+H) is back on the next launch. The desktop app's HUD has no
    /// tray icon, so a remembered hide left the card gone with no visible way
    /// to bring it back.
    #[serde(skip)]
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
pub enum Button {Done, Collapse, Hide, Undo,
    /// "Set aside" for the card's step: go on with the next steps.
    SetAside,
    /// "Resume" for the n-th step the card lists as set aside.
    Resume(u8),
    /// "I chose this" on an ambiguous event answer: confirms the guide's
    /// answer the event popup shows now.
    ConfirmAnswer,
    /// A quick-reference tag (army setup, war plans, events): opens its section.
    Tag(u8),
    /// A row's checkbox in the open section.
    Item(u16)}

/// The card's header buttons, right to left. An ambiguous answer for the
/// card's own step replaces its Done (one confirmation of the same line). An
/// answer for another step keeps the card step's Done and adds "I chose this"
/// beside it; the card's badge names the line that button confirms.
pub fn header_buttons(answer: bool, answer_is_card_step: bool, checkable: bool) -> Vec<Button> {
    let mut buttons = vec![Button::Hide, Button::Collapse];
    if checkable && !(answer && answer_is_card_step) {
        buttons.push(Button::Done);
    }
    if answer {
        buttons.push(Button::ConfirmAnswer);
    }
    buttons
}

/// English text of a labeled header button (translated when drawn). Done on
/// an answer the player already confirmed reads like the confirmation.
pub fn header_label(button: Button, confirmed_by_user: bool) -> Option<&'static str> {
    match button {
        Button::ConfirmAnswer => Some("I chose this"),
        Button::Done => Some(if confirmed_by_user {"I chose this"} else {"Mark done"}),
        _ => None,
    }
}

/// Places header buttons of the given widths right to left from `right`,
/// `gap` apart. Returns their rectangles and where the header text must end.
pub fn header_layout(buttons: &[(Button, f32)], mut right: f32, top: f32, row: f32, gap: f32) -> (Vec<(Button, [f32; 4])>, f32) {
    let mut placed = Vec::new();
    for (button, width) in buttons {
        placed.push((*button, [right - width, top, right, top + row]));
        right -= width + gap;
    }
    (placed, right)
}

/// Quick-reference rows the player ticked as done, per checklist run
/// ("guide:run"), in the HUD's data folder. Only the HUD reads them: they
/// never change guide progress or completion.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Marks {
    pub done: std::collections::BTreeMap<String, std::collections::BTreeSet<String>>,
}
impl Marks {
    pub fn path(card_prefs: &Path) -> PathBuf {
        card_prefs.with_file_name("overlay-references.json")
    }
    pub fn load(path: &Path) -> Self {
        std::fs::read(path).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
    }
    pub fn save(&self, path: &Path) {
        if let Some(parent) = path.parent() {let _ = std::fs::create_dir_all(parent);}
        let temporary = path.with_extension("json.tmp");
        if let Ok(bytes) = serde_json::to_vec_pretty(self)
            && std::fs::write(&temporary, bytes).is_ok() {
            let _ = std::fs::rename(temporary, path);
        }
    }
    pub fn is_done(&self, scope: &str, id: &str) -> bool {
        self.done.get(scope).is_some_and(|set| set.contains(id))
    }
    pub fn toggle(&mut self, scope: &str, id: &str) {
        if scope.is_empty() || id.is_empty() {return;}
        let set = self.done.entry(scope.to_owned()).or_default();
        if !set.remove(id) {set.insert(id.to_owned());}
        if set.is_empty() {self.done.remove(scope);}
    }
}

/// What the card's Undo button takes back.
#[derive(Clone, Debug, PartialEq)]
pub enum Undo {
    /// A step marked done with "Done": uncheck it.
    Check {guide_id: String, key: String},
    /// An event answer confirmed with "I chose this": put back the row it
    /// replaced (or none) and the step's check as they were.
    Answer {guide_id: String, key: String, quote: String, previous: serde_json::Value, previous_checked: bool},
    /// A step set aside (or resumed) from the card: do the opposite.
    Aside {guide_id: String, key: String, aside: bool},
}

/// Result of the last "Done" or "I chose this" click, shown on the card for
/// `NOTICE_FOR`.
#[derive(Clone, Debug, PartialEq)]
pub struct Notice {
    pub text: String,
    /// What can still be taken back from the card.
    pub undo: Option<Undo>,
    pub at: Instant,
}
impl Notice {
    pub fn live(&self, now: Instant) -> bool {now.saturating_duration_since(self.at) <= NOTICE_FOR}
}

/// The button under a point in card coordinates, if any.
pub fn hit(buttons: &[(Button, [f32; 4])], x: f32, y: f32) -> Option<Button> {
    buttons.iter().find(|(_, r)| x >= r[0] && x < r[2] && y >= r[1] && y < r[3]).map(|(b, _)| *b)
}

/// Emphasis on the card's checklist lines, so what is done and what is left
/// reads at a glance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hue {
    /// Reached (a ✓ line mark, a count at or above its target).
    Done,
    /// Still to do (a • line mark, a count below its target, "up to the maximum").
    Todo,
    /// Already queued in the game.
    Queued,
    /// Not observed ("?").
    Unknown,
    /// The region or product a line is about (semibold, default color).
    Strong,
}

/// Colored ranges of card text, in UTF-16 units as DirectWrite counts them:
/// line marks, the name before a line's first colon, "a/b" and "a of b"
/// counts (in both interface languages) and a few state phrases.
pub fn spans(text: &str) -> Vec<(u32, u32, Hue)> {
    const QUEUED: [&str; 2] = ["(in coda)", "(queued)"];
    const TODO: [&str; 2] = ["fino al massimo", "up to the maximum"];
    let mut out = Vec::new();
    let mut base = 0u32;
    for line in text.split('\n') {
        let chars: Vec<char> = line.chars().collect();
        // UTF-16 offset of each char index (plus the end).
        let mut at = Vec::with_capacity(chars.len() + 1);
        let mut unit = base;
        for c in &chars {
            at.push(unit);
            unit += c.len_utf16() as u32;
        }
        at.push(unit);
        let range = |from: usize, to: usize, hue: Hue, out: &mut Vec<(u32, u32, Hue)>| {
            if to > from {
                out.push((at[from], at[to] - at[from], hue));
            }
        };
        let start = chars.iter().position(|c| !c.is_whitespace()).unwrap_or(chars.len());
        if let Some(mark) = chars.get(start).and_then(|c| match c { '✓' => Some(Hue::Done), '•' => Some(Hue::Todo), _ => None }) {
            range(start, start + 1, mark, &mut out);
            let name = start + 2;
            if let Some(colon) = chars.iter().skip(name).position(|c| *c == ':').map(|p| p + name).filter(|p| p - name <= 60) {
                range(name, colon, Hue::Strong, &mut out);
            }
        }
        let number = |i: usize| -> Option<(usize, Option<u64>)> {
            if chars.get(i) == Some(&'?') {
                return Some((i + 1, None));
            }
            let end = (i..chars.len()).find(|j| !chars[*j].is_ascii_digit()).unwrap_or(chars.len());
            (end > i).then(|| (end, chars[i..end].iter().collect::<String>().parse().ok()))
        };
        let mut i = 0;
        while i < chars.len() {
            let fresh = i == 0 || !chars[i - 1].is_ascii_alphanumeric();
            if fresh && let Some((end, have)) = number(i) {
                // "a/b" or "a di b" / "a of b".
                let separator = [("/", 1usize), (" di ", 4), (" of ", 4)].into_iter().find(|(sep, n)| {
                    chars.get(end..end + n).is_some_and(|s| s.iter().collect::<String>() == *sep)
                });
                if let Some((_, n)) = separator && let Some((stop, want)) = number(end + n) {
                    let hue = match (have, want) {
                        (Some(have), Some(want)) if have >= want => Hue::Done,
                        (Some(_), Some(_)) => Hue::Todo,
                        _ => Hue::Unknown,
                    };
                    range(i, stop, hue, &mut out);
                    i = stop;
                    continue;
                }
                i = end;
                continue;
            }
            i += 1;
        }
        let lower: String = chars.iter().collect();
        for (phrases, hue) in [(&QUEUED[..], Hue::Queued), (&TODO[..], Hue::Todo)] {
            for phrase in phrases {
                if let Some(byte) = lower.find(phrase) {
                    let from = lower[..byte].chars().count();
                    range(from, from + phrase.chars().count(), hue, &mut out);
                }
            }
        }
        if let (Some(open), Some(close)) = (lower.find('('), lower.find(')'))
            && close > open
            && (lower[open..close].ends_with(" in uso") || lower[open..close].ends_with(" in use"))
        {
            let from = lower[..open].chars().count();
            range(from, from + lower[open..=close].chars().count(), Hue::Todo, &mut out);
        }
        base = unit + 1;
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn checklist_lines_color_marks_counts_and_states() {
        let text = "In 2 regioni, segnate sulla mappa:
✓ Mosca: Infrastruttura 5/5
• Kharkov: Infrastruttura 4/5 (in coda) · Fabbrica militare 1, fino al massimo
• Camion: 3 di 4 fabbriche (2 in uso)
• Kursk: Infrastruttura ?/5";
        let spans = spans(text);
        let pick = |hue: Hue| -> Vec<String> {
            let units: Vec<u16> = text.encode_utf16().collect();
            spans.iter().filter(|s| s.2 == hue).map(|s| String::from_utf16_lossy(&units[s.0 as usize..(s.0 + s.1) as usize])).collect()
        };
        assert_eq!(pick(Hue::Done), ["✓", "5/5"]);
        assert_eq!(pick(Hue::Strong), ["Mosca", "Kharkov", "Camion", "Kursk"]);
        assert_eq!(pick(Hue::Todo), ["•", "4/5", "fino al massimo", "•", "3 di 4", "(2 in uso)", "•"]);
        assert_eq!(pick(Hue::Queued), ["(in coda)"]);
        assert_eq!(pick(Hue::Unknown), ["?/5"]);
        assert!(spans.iter().all(|s| s.2 != Hue::Done || !pick(Hue::Done).is_empty()));
        assert!(super::spans("I-16:5 Truck:4").is_empty(), "author shorthand is not a count");
    }

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
        // A hide saved by an older HUD is not restored.
        std::fs::write(&path, br#"{"position":[2.0,0.5],"collapsed":false,"hidden":true}"#).unwrap();
        assert_eq!(Prefs::load(&path), Prefs {position: None, collapsed: false, hidden: false});
        let hidden = Prefs {hidden: true, ..prefs};
        hidden.save(&path);
        assert_eq!(Prefs::load(&path), prefs);
        std::fs::write(&path, b"not json").unwrap();
        assert_eq!(Prefs::load(&path), Prefs::default());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn reference_marks_toggle_per_run_and_survive_a_restart() {
        let dir = std::env::temp_dir().join(format!("hoi4-marks-{}", std::process::id()));
        let path = Marks::path(&dir.join("overlay-card.json"));
        assert_eq!(path.file_name().unwrap(), "overlay-references.json");
        let mut marks = Marks::default();
        marks.toggle("sov:run-1", "abc");
        assert!(marks.is_done("sov:run-1", "abc"));
        assert!(!marks.is_done("sov:run-2", "abc"), "another checklist starts clean");
        marks.save(&path);
        assert_eq!(Marks::load(&path), marks);
        marks.toggle("sov:run-1", "abc");
        assert!(marks.done.is_empty());
        marks.toggle("", "abc");
        assert!(marks.done.is_empty(), "no scope, no mark");
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

    fn ids(buttons: &[(Button, [f32; 4])]) -> Vec<Button> {buttons.iter().map(|(b, _)| *b).collect()}
    /// Header layout with 24 px icon buttons and 90 px labeled buttons.
    fn header(answer: bool, same_step: bool, checkable: bool) -> (Vec<(Button, [f32; 4])>, f32) {
        let sized: Vec<(Button, f32)> = header_buttons(answer, same_step, checkable).into_iter()
            .map(|b| (b, if matches!(b, Button::ConfirmAnswer | Button::Done) {90.0} else {24.0})).collect();
        header_layout(&sized, 350.0, 10.0, 24.0, 6.0)
    }
    #[test]
    fn confirm_button_only_for_ambiguous_or_confirmed() {
        let layout = |answer: bool, checkable: bool| header(answer, true, checkable);
        assert_eq!(ids(&layout(false, false).0), [Button::Hide, Button::Collapse]);
        assert_eq!(ids(&layout(false, true).0), [Button::Hide, Button::Collapse, Button::Done]);
        // An ambiguous answer replaces Done: one confirmation, never two.
        assert_eq!(ids(&layout(true, true).0), [Button::Hide, Button::Collapse, Button::ConfirmAnswer]);
        let (buttons, right) = layout(true, false);
        assert_eq!(ids(&buttons), [Button::Hide, Button::Collapse, Button::ConfirmAnswer]);
        assert_eq!(buttons[0].1, [326.0, 10.0, 350.0, 34.0]);
        assert_eq!(buttons[1].1, [296.0, 10.0, 320.0, 34.0]);
        assert_eq!(buttons[2].1, [200.0, 10.0, 290.0, 34.0]);
        assert_eq!(right, 194.0, "the header text ends before the leftmost button");
        assert_eq!(hit(&buttons, 210.0, 12.0), Some(Button::ConfirmAnswer));
        assert_eq!(hit(&buttons, 293.0, 12.0), None, "the gap between buttons drags the card");
        assert_eq!(hit(&buttons, 300.0, 12.0), Some(Button::Collapse));
        // The label: a user-confirmed answer keeps "I chose this" on Done.
        assert_eq!(header_label(Button::ConfirmAnswer, false), Some("I chose this"));
        assert_eq!(header_label(Button::Done, true), Some("I chose this"));
        assert_eq!(header_label(Button::Done, false), Some("Mark done"));
        assert_eq!(header_label(Button::Hide, true), None);
    }

    #[test]
    fn other_step_answer_keeps_done_and_adds_confirm() {
        // The answer belongs to another guide line: the card's own step keeps
        // Done, and "I chose this" sits beside it.
        let (buttons, right) = header(true, false, true);
        assert_eq!(ids(&buttons), [Button::Hide, Button::Collapse, Button::Done, Button::ConfirmAnswer]);
        assert_eq!(buttons[2].1, [200.0, 10.0, 290.0, 34.0]);
        assert_eq!(buttons[3].1, [104.0, 10.0, 194.0, 34.0]);
        assert_eq!(right, 98.0);
        assert_eq!(hit(&buttons, 210.0, 12.0), Some(Button::Done));
        assert_eq!(hit(&buttons, 110.0, 12.0), Some(Button::ConfirmAnswer));
        assert_eq!(hit(&buttons, 197.0, 12.0), None);
        // Not checkable: only the confirmation.
        assert_eq!(ids(&header(true, false, false).0), [Button::Hide, Button::Collapse, Button::ConfirmAnswer]);
    }

    #[test]
    fn done_notice_expires() {
        let at = Instant::now();
        let notice = Notice {text: "Done".into(), undo: None, at};
        assert!(notice.live(at + NOTICE_FOR));
        assert!(!notice.live(at + NOTICE_FOR + Duration::from_millis(1)));
    }
}
