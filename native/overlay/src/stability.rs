//! Visual stability rules for the HUD. They decide what stays on screen across
//! a single missed or unverifiable sample, without ever extending a highlight
//! past its original witness deadline or showing a target that was never
//! verified.
//!
//! Highlights: a row that a verified sample witnessed is kept for at most
//! `HOLD_GRACE` after that witness when the following samples could not verify
//! anything (backend busy, slow request, foreground recheck), and for at most
//! `ABSENT_CONFIRMATIONS - 1` verified samples that no longer contain it. The
//! row keeps its original witness time, so the renderer's 750 ms deadline still
//! counts from the real observation. Any change of guide, guide revision, run
//! server or game window drops every held row at once.
//!
//! Step card: guide text, not GUI evidence. It stays while the backend's live
//! delivery is momentarily unknown (after a progress write or a slow read) for
//! at most `CARD_HOLD`, and goes away as soon as a current delivery says the
//! guide no longer applies or has no next step.
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::render::StepCard;

/// Longest time a witnessed highlight is kept through unverifiable samples.
pub const HOLD_GRACE: Duration = Duration::from_millis(150);
/// Verified samples without a row needed before that row is removed.
pub const ABSENT_CONFIRMATIONS: u32 = 2;
/// Longest time the step card is kept while the guide delivery is unknown.
pub const CARD_HOLD: Duration = Duration::from_millis(2000);

pub enum Observation {
    /// A fully verified sample (identity, revision, window and freshness) with
    /// the rows it witnessed (possibly none) and the witness time.
    Verified(Vec<Value>, Instant),
    /// No trustworthy answer: neither presence nor absence was verified.
    Unverified,
}

struct Held {
    row: Value,
    witnessed: Instant,
    absent: u32,
}

/// Identity of a highlighted row, independent of its rectangle.
fn identity(row: &Value) -> Value {
    serde_json::json!([row["panel"], row["key"], row["navigation_stage"], row["identity"]])
}

#[derive(Default)]
pub struct MarkHold {
    context: Option<Value>,
    held: Vec<Held>,
}

impl MarkHold {
    pub fn clear(&mut self) {
        self.held.clear();
        self.context = None;
    }
    /// Apply one sample. `context` identifies guide, revision, run and window;
    /// it is only trusted for verified samples. Returns the rows to draw and
    /// the oldest witness time among them (the frame's deadline origin).
    pub fn observe(&mut self, context: Value, observation: Observation, now: Instant) -> (Vec<Value>, Option<Instant>) {
        match observation {
            Observation::Verified(rows, witnessed) => {
                if self.context.as_ref() != Some(&context) {
                    self.held.clear();
                    self.context = Some(context);
                }
                let mut next: Vec<Held> = Vec::with_capacity(rows.len());
                for row in rows {
                    let id = identity(&row);
                    if next.iter().any(|h| identity(&h.row) == id) {
                        continue;
                    }
                    next.push(Held { row, witnessed, absent: 0 });
                }
                for mut old in std::mem::take(&mut self.held) {
                    let id = identity(&old.row);
                    if next.iter().any(|h| identity(&h.row) == id) {
                        continue;
                    }
                    old.absent += 1;
                    if old.absent < ABSENT_CONFIRMATIONS && now.saturating_duration_since(old.witnessed) <= HOLD_GRACE {
                        next.push(old);
                    }
                }
                self.held = next;
            }
            Observation::Unverified => {
                self.held.retain(|h| now.saturating_duration_since(h.witnessed) <= HOLD_GRACE);
            }
        }
        let oldest = self.held.iter().map(|h| h.witnessed).min();
        (self.held.iter().map(|h| h.row.clone()).collect(), oldest)
    }
}

#[derive(Default)]
pub struct CardHold {
    card: Option<(Value, StepCard, Instant)>,
}

impl CardHold {
    pub fn clear(&mut self) {
        self.card = None;
    }
    /// `known` is `None` when the backend could not deliver a current guide
    /// projection (unknown), `Some(card)` when it did (the card may be absent).
    pub fn observe(&mut self, guide: &Value, known: Option<Option<StepCard>>, now: Instant) -> Option<StepCard> {
        match known {
            Some(Some(card)) => self.card = Some((guide.clone(), card, now)),
            Some(None) => self.card = None,
            None => {
                let keep = self.card.as_ref().is_some_and(|(id, _, at)| {
                    (guide.is_null() || guide == id) && now.saturating_duration_since(*at) <= CARD_HOLD
                });
                if !keep {
                    self.card = None;
                }
            }
        }
        self.card.as_ref().map(|(_, card, _)| card.clone())
    }
}

/// Whether a HUD snapshot carries a current guide delivery. The backend marks
/// a missing or expired delivery with `live_delivery_fresh: false`.
pub fn delivery_known(snapshot: &Value) -> bool {
    snapshot["live_delivery_fresh"] != false
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(key: &str, x: f64) -> Value {
        json!({"panel":"focus","key":key,"rect":[x,10.0,x+50.0,40.0]})
    }
    fn ctx(version: u64) -> Value {
        json!(["guide", "server", version, [1, 2, 0, 0, 800, 600]])
    }

    #[test]
    fn one_missed_verified_sample_keeps_the_row_and_its_original_witness() {
        let mut hold = MarkHold::default();
        let t0 = Instant::now();
        let (rows, at) = hold.observe(ctx(1), Observation::Verified(vec![row("a", 10.0)], t0), t0);
        assert_eq!((rows.len(), at), (1, Some(t0)));
        let t1 = t0 + Duration::from_millis(16);
        let (rows, at) = hold.observe(ctx(1), Observation::Verified(vec![], t1), t1);
        assert_eq!(rows.len(), 1, "a single empty sample must not blink the border");
        assert_eq!(at, Some(t0), "holding never renews the witness time");
        let t2 = t0 + Duration::from_millis(32);
        let (rows, at) = hold.observe(ctx(1), Observation::Verified(vec![], t2), t2);
        assert!(rows.is_empty() && at.is_none(), "a confirmed absence removes the row");
    }

    #[test]
    fn a_row_that_comes_back_takes_its_new_rectangle_and_witness() {
        let mut hold = MarkHold::default();
        let t0 = Instant::now();
        hold.observe(ctx(1), Observation::Verified(vec![row("a", 10.0), row("b", 100.0)], t0), t0);
        let t1 = t0 + Duration::from_millis(16);
        let (rows, _) = hold.observe(ctx(1), Observation::Verified(vec![row("a", 12.0)], t1), t1);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["rect"][0], 12.0, "a moved row uses the newest geometry");
        let t2 = t0 + Duration::from_millis(32);
        let (rows, at) = hold.observe(ctx(1), Observation::Verified(vec![row("a", 14.0), row("b", 102.0)], t2), t2);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1]["rect"][0], 102.0);
        assert_eq!(at, Some(t2));
    }

    #[test]
    fn unverifiable_samples_hold_only_within_the_grace_after_the_witness() {
        let mut hold = MarkHold::default();
        let t0 = Instant::now();
        hold.observe(ctx(1), Observation::Verified(vec![row("a", 10.0)], t0), t0);
        let (rows, at) = hold.observe(ctx(9), Observation::Unverified, t0 + HOLD_GRACE);
        assert_eq!((rows.len(), at), (1, Some(t0)), "unverified samples carry no identity to compare");
        let (rows, _) = hold.observe(ctx(1), Observation::Unverified, t0 + HOLD_GRACE + Duration::from_millis(1));
        assert!(rows.is_empty(), "never beyond the grace, whatever happens next");
        let (rows, _) = hold.observe(ctx(1), Observation::Verified(vec![], t0 + HOLD_GRACE * 2), t0 + HOLD_GRACE * 2);
        assert!(rows.is_empty());
    }

    #[test]
    fn identity_changes_drop_every_held_row_immediately() {
        let t0 = Instant::now();
        let t1 = t0 + Duration::from_millis(16);
        for changed in [ctx(2), json!(["other", "server", 1, [1, 2, 0, 0, 800, 600]]), json!(["guide", "server", 1, [1, 2, 5, 0, 800, 600]])] {
            let mut hold = MarkHold::default();
            hold.observe(ctx(1), Observation::Verified(vec![row("a", 10.0)], t0), t0);
            let (rows, _) = hold.observe(changed, Observation::Verified(vec![], t1), t1);
            assert!(rows.is_empty());
        }
        let mut hold = MarkHold::default();
        hold.observe(ctx(1), Observation::Verified(vec![row("a", 10.0)], t0), t0);
        hold.clear();
        assert!(hold.observe(ctx(1), Observation::Unverified, t1).0.is_empty());
    }

    #[test]
    fn nothing_is_ever_shown_without_a_verified_witness() {
        let mut hold = MarkHold::default();
        let t0 = Instant::now();
        assert!(hold.observe(ctx(1), Observation::Unverified, t0).0.is_empty());
        assert!(hold.observe(ctx(1), Observation::Verified(vec![], t0), t0).0.is_empty());
    }

    #[test]
    fn step_card_survives_unknown_delivery_briefly_but_not_a_known_change() {
        let card = StepCard { heading: "Build".into(), ..Default::default() };
        let guide = json!("g");
        let mut hold = CardHold::default();
        let t0 = Instant::now();
        assert_eq!(hold.observe(&guide, Some(Some(card.clone())), t0), Some(card.clone()));
        assert_eq!(hold.observe(&guide, None, t0 + Duration::from_millis(900)), Some(card.clone()));
        assert_eq!(hold.observe(&Value::Null, None, t0 + CARD_HOLD), Some(card.clone()));
        assert_eq!(hold.observe(&guide, None, t0 + CARD_HOLD + Duration::from_millis(1)), None);
        hold.observe(&guide, Some(Some(card.clone())), t0);
        assert_eq!(hold.observe(&json!("other"), None, t0), None, "another guide never inherits the card");
        hold.observe(&guide, Some(Some(card.clone())), t0);
        assert_eq!(hold.observe(&guide, Some(None), t0), None, "a current delivery without a step clears it");
        assert_eq!(hold.observe(&guide, None, t0), None);
    }

    #[test]
    fn delivery_is_unknown_only_when_the_backend_says_so() {
        assert!(delivery_known(&json!({"usable":false})));
        assert!(!delivery_known(&json!({"live_delivery_fresh":false})));
    }
}
