//! `SelectFlyout` — the combo/select-box open list, a third `PopupRoot` surface
//! beside `MenuPopup` and `RadialPopup`. Same Atlas HUD material (`AccentFrame
//! {field_bg}` card + `IconSet` glyph rows), driven by the shared `MarkingCore`
//! in popup mode. Unlike `MenuPopup` it is at least as wide as the control that
//! opened it (`min_width`), marks the current selection, and renders each row's
//! own `SelectLead` visual. Item model + pure width clamp live here too; the
//! clamp is unit-tested directly. See
//! `docs/superpowers/specs/2026-07-22-select-box-flyout-design.md`.
#![allow(dead_code)]

use crate::icons::Icon;
use makepad_widgets::*;

/// Safety cap on flyout width (lpx). The card hugs its widest label but never
/// grows past this — unless the control itself is wider (`min_width` wins).
pub const SELECT_MAX_W: f64 = 320.0;
/// Left offset where a row label starts, past the leading `SelectLead` gutter
/// (lpx). Matches `menu::LABEL_X` so the badge/icon share the menu's 14px inset.
pub const LEAD_GUTTER: f64 = 42.0;
/// Trailing margin right of the widest label before the frame edge (lpx).
pub const PAD_R: f64 = 18.0;
/// Gap between the control's bottom edge and the card top (lpx). Tight, flush
/// left — the card sits just under the control, no horizontal indent.
pub const SELECT_GAP: f64 = 2.0;

/// A leading visual for one row. Closed set; extend with a new arm when a new
/// row shape appears (YAGNI over an open-ended draw callback).
#[derive(Clone, Debug)]
pub enum SelectLead {
    None,
    /// Edge rows lead with `Icon(Icon::Spline)`.
    Icon(Icon),
    /// Node rows lead with a per-type coloured square + kind initial.
    Badge {
        color: Vec4,
        letter: String,
    },
}

/// One selectable row. `id` is opaque to the surface — the opener resolves it on
/// commit (same contract as `PopupItem.id`).
#[derive(Clone, Debug)]
pub struct SelectItem {
    pub id: LiveId,
    pub lead: SelectLead,
    pub label: String,
    /// Current value → trailing check mark + subtle persistent fill.
    pub selected: bool,
    /// Disabled rows draw dimmed and never arm or commit.
    pub enabled: bool,
}

/// The flyout width: hug the widest label, but never narrower than the control
/// (`min_width`) and never wider than the cap — except a control wider than the
/// cap is never clipped (`cap` floors to `min_width`).
pub fn select_width(label_hug: f64, min_width: f64, cap: f64) -> f64 {
    label_hug.max(min_width).min(cap.max(min_width))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hug_wins_when_widest() {
        // Label hug (200) beats a narrow control (120), under the cap (320).
        assert_eq!(select_width(200.0, 120.0, 320.0), 200.0);
    }

    #[test]
    fn min_width_floors_a_short_hug() {
        // A wide control (260) floors a short label hug (140).
        assert_eq!(select_width(140.0, 260.0, 320.0), 260.0);
    }

    #[test]
    fn cap_clamps_a_pathological_hug() {
        // A runaway label (900) is capped at 320.
        assert_eq!(select_width(900.0, 120.0, 320.0), 320.0);
    }

    #[test]
    fn control_wider_than_cap_is_never_clipped() {
        // A control wider than the cap (400 > 320) raises the effective cap so
        // the card is never narrower than the control it drops from.
        assert_eq!(select_width(140.0, 400.0, 320.0), 400.0);
    }
}
