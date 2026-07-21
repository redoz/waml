//! The popup contract: the item shape, the closed-result, the per-event verdict,
//! the surface trait, and the two pure event predicates the authority routes on.
#![allow(dead_code)]

use crate::icons::Icon;
use makepad_widgets::*;

/// One selectable entry. The surface owns no command semantics — it reports `id`
/// back on commit and the opener maps it. (Renamed + moved from `radial::RadialItem`.)
#[derive(Clone, Debug)]
pub struct PopupItem {
    pub id: LiveId,
    pub label: String,
    pub icon: Icon,
    /// Danger-token hue across all states.
    pub danger: bool,
    /// `false` = greyed, holds its slot, cannot arm or commit.
    pub enabled: bool,
}

/// What a closed popup reports. `Invoked` carries the chosen item's id; any
/// light-dismiss (Esc / outside / blur / superseded) reports `Dismissed`.
#[derive(Clone, Debug, PartialEq)]
pub enum PopupResult {
    Invoked(LiveId),
    Dismissed,
}

/// A surface's answer to one event, returned from `Popup::handle`.
#[derive(Clone, Debug, PartialEq)]
pub enum PopupVerdict {
    /// The surface handled it (hover move, arm, in-surface press).
    Consumed,
    /// Not for the surface. A *primary press* here is an outside-click: the
    /// authority turns it into a dismiss (see `PopupRoot::route`).
    Ignored,
    /// The surface committed or self-dismissed; the authority emits the matching
    /// `PopupRootAction::Closed` and clears the active slot.
    Closed(PopupResult),
}

/// Every surface kind implements this. The surface owns its geometry + marking
/// interaction; the authority owns the active slot, light-dismiss, and emission.
pub trait Popup {
    /// Drive one already-localized event; return the verdict.
    fn handle(&mut self, cx: &mut Cx, event: &Event) -> PopupVerdict;
    /// Return to the closed state WITHOUT emitting (the authority emits the
    /// `Closed` action). Called on any light-dismiss / supersede.
    fn reset(&mut self);
}

/// True for events that collapse transient UI regardless of pointer position:
/// Escape, and window focus-loss / app-deactivate. Outside-click is NOT here —
/// it is derived from an `Ignored` primary press in `PopupRoot::route`.
pub fn is_light_dismiss(event: &Event) -> bool {
    match event {
        Event::KeyDown(ke) if ke.key_code == KeyCode::Escape => true,
        Event::WindowLostFocus(_) => true,
        _ => false,
    }
}

/// True for a primary (left) button press.
pub fn is_primary_press(event: &Event) -> bool {
    matches!(event, Event::MouseDown(e) if e.button.is_primary())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn escape_keydown_is_light_dismiss() {
        let e = Event::KeyDown(KeyEvent {
            key_code: KeyCode::Escape,
            ..Default::default()
        });
        assert!(is_light_dismiss(&e));
        assert!(!is_primary_press(&e));
    }

    #[test]
    fn primary_mousedown_is_a_primary_press_not_a_dismiss() {
        // `MouseDownEvent` has no `Default` impl (its `window_id: WindowId`
        // doesn't derive one), so build it field-by-field.
        let e = Event::MouseDown(MouseDownEvent {
            abs: Vec2d::default(),
            button: MouseButton::PRIMARY,
            window_id: WindowId(0, 0),
            modifiers: KeyModifiers::default(),
            handled: Cell::new(Area::default()),
            time: 0.0,
        });
        assert!(is_primary_press(&e));
        assert!(!is_light_dismiss(&e));
    }
}
