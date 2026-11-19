//! The HUD's one palette. Every highlight, caption and card emphasis takes its
//! color from the role it plays, so the same meaning always has the same color
//! on the map, in the trees, on event popups and on the step card.
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

/// What a highlighted control or card emphasis means to the player.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Role {
    /// The guide's next action (amber).
    #[default]
    Next,
    /// Already in progress or reached in the game (cyan).
    Active,
    /// One of several options the guide leaves to the player (purple).
    Choice,
    /// An event answer the guide's text does not identify with certainty (pink).
    Ambiguous,
    /// Reached on the card's checklist (green).
    Done,
}

impl Role {
    pub const ALL: [Role; 5] = [Role::Next, Role::Active, Role::Choice, Role::Ambiguous, Role::Done];
    /// The backend's proposal `role`; None when absent or unknown.
    pub fn parse(value: &str) -> Option<Role> {
        match value {
            "next" => Some(Role::Next),
            "active" => Some(Role::Active),
            "choice" => Some(Role::Choice),
            "ambiguous" => Some(Role::Ambiguous),
            _ => None,
        }
    }
    pub fn index(self) -> usize {
        self as usize
    }
}

const fn rgba(r: f32, g: f32, b: f32, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r, g, b, a }
}

/// The opaque color of a role.
pub fn color(role: Role) -> D2D1_COLOR_F {
    match role {
        Role::Next => rgba(1.0, 0.83, 0.30, 1.0),
        Role::Active => rgba(0.27, 0.84, 1.0, 1.0),
        Role::Choice => rgba(0.78, 0.59, 1.0, 1.0),
        Role::Ambiguous => rgba(1.0, 0.45, 0.75, 1.0),
        Role::Done => rgba(0.42, 0.88, 0.52, 1.0),
    }
}

/// The same color with another opacity (highlight fill, card border).
pub fn with_alpha(color: D2D1_COLOR_F, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F { a, ..color }
}

/// Opacity of a highlight's fill over the game.
pub const FILL_ALPHA: f32 = 0.09;
/// Opacity of the card's border.
pub const CARD_BORDER_ALPHA: f32 = 0.55;

// Surfaces and text are not roles: they carry no meaning of their own.
/// Cleared surface.
pub const TRANSPARENT: D2D1_COLOR_F = rgba(0.0, 0.0, 0.0, 0.0);
/// Dark halo around a highlight's border, so it reads on any game background.
pub const SHADOW: D2D1_COLOR_F = rgba(0.01, 0.02, 0.03, 0.85);
/// Background of captions and card buttons.
pub const LABEL_BACKGROUND: D2D1_COLOR_F = rgba(0.035, 0.055, 0.075, 0.82);
/// Background of the step card.
pub const CARD_BACKGROUND: D2D1_COLOR_F = rgba(0.035, 0.055, 0.075, 0.88);
pub const TEXT: D2D1_COLOR_F = rgba(0.86, 0.89, 0.91, 1.0);
pub const MUTED: D2D1_COLOR_F = rgba(0.62, 0.69, 0.73, 1.0);
/// Card checklist: still to do.
pub const TODO: D2D1_COLOR_F = rgba(1.0, 0.64, 0.32, 1.0);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn palette_roles_have_the_spec_colors() {
        let rgb = |role: Role| {
            let c = color(role);
            (c.r, c.g, c.b, c.a)
        };
        assert_eq!(rgb(Role::Next), (1.0, 0.83, 0.30, 1.0));
        assert_eq!(rgb(Role::Active), (0.27, 0.84, 1.0, 1.0));
        assert_eq!(rgb(Role::Choice), (0.78, 0.59, 1.0, 1.0));
        assert_eq!(rgb(Role::Ambiguous), (1.0, 0.45, 0.75, 1.0));
        assert_eq!(rgb(Role::Done), (0.42, 0.88, 0.52, 1.0));
        // Each role indexes its own brush.
        for (i, role) in Role::ALL.iter().enumerate() {
            assert_eq!(role.index(), i);
        }
        assert_eq!(Role::parse("ambiguous"), Some(Role::Ambiguous));
        assert_eq!(Role::parse("done"), None, "the backend never sends a card-only role");
        assert_eq!(Role::parse("bogus"), None);
        assert_eq!(with_alpha(color(Role::Next), FILL_ALPHA).a, 0.09);
    }
}
