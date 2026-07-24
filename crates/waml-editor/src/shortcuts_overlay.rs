//! Command/shortcut hint overlay (UX mock, U8): a full-window scrim listing
//! the static keybindings this mock recognizes. Toggled by the tool dock's
//! `Shortcuts` (`?`) button or the `?` hotkey; `Escape` or clicking the
//! scrim closes it.
//!
//! Declared as the *last* child of a `flow: Overlay` wrapper around the
//! whole window body (see `app.rs`), alongside the normal `main_column`.
//! `Flow::Overlay` (unlike `Flow::Right`/`Flow::Down`) gives every child the
//! *same* full turtle rect instead of splitting space between them, so this
//! widget's own `width: Fill height: Fill` genuinely covers the whole body
//! -- painting after `main_column` means it draws on top of everything
//! (doc-tabs, canvas, inspector, statusbar) when visible, and draws nothing
//! at all when hidden. See `diagram_switcher.rs`'s doc comment for the
//! z-order investigation (U7) that ruled out reaching for hardcoded
//! absolute coordinates or zero-footprint siblings for this instead.
//!
//! Hand-rolled immediate-mode widget, same `draw_abs`/rect-hit-test
//! convention as `doc_tabs.rs`/`tool_dock.rs`.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*
    use mod.fonts

    mod.widgets.ShortcutsOverlayBase = #(ShortcutsOverlay::register_widget(vm))

    mod.widgets.ShortcutsOverlay = set_type_default() do mod.widgets.ShortcutsOverlayBase{
        width: Fill
        height: Fill
        draw_scrim +: { color: atlas.scrim }
        draw_panel +: { color: atlas.surface }
        draw_edge +: { color: atlas.frame_hi }
        draw_title +: {
            color: atlas.text
            text_style: fonts.text_title
        }
        draw_key +: {
            color: atlas.text
            text_style: fonts.text_heading
        }
        draw_desc +: {
            color: atlas.text_dim
            text_style: fonts.text_body
        }
    }
}

/// One row in the cheat sheet: the key label and what it does.
pub const BINDINGS: &[(&str, &str)] = &[
    ("V", "Select tool"),
    ("N", "Add tool"),
    ("C", "Connect tool"),
    ("T", "Toggle light/dark theme"),
    ("?", "Toggle this overlay"),
    ("Esc", "Close this overlay"),
];

#[derive(Clone, Debug, Default)]
pub enum ShortcutsOverlayAction {
    #[default]
    None,
    /// Emitted when the scrim (not the panel itself) is clicked.
    Dismissed,
}

const PANEL_W: f64 = 360.0;
const PANEL_PAD: f64 = 24.0;
const TITLE_H: f64 = 28.0;
const ROW_H: f64 = 26.0;
const KEY_COL_W: f64 = 56.0;

#[derive(Script, ScriptHook, Widget)]
pub struct ShortcutsOverlay {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    #[redraw]
    #[live]
    draw_scrim: DrawColor,
    #[redraw]
    #[live]
    draw_panel: DrawColor,
    /// Subtle source-bright top edge (shared HUD panel material).
    #[redraw]
    #[live]
    draw_edge: DrawColor,
    #[redraw]
    #[live]
    draw_title: DrawText,
    #[redraw]
    #[live]
    draw_key: DrawText,
    #[redraw]
    #[live]
    draw_desc: DrawText,

    #[rust]
    visible: bool,
    #[rust]
    panel_rect: Rect,
}

impl Widget for ShortcutsOverlay {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if !self.visible {
            return;
        }
        let uid = self.widget_uid();
        match event.hits_with_capture_overload(cx, self.draw_scrim.area(), false) {
            Hit::FingerUp(fe) if fe.is_primary_hit() && !self.panel_rect.contains(fe.abs) => {
                cx.widget_action(uid, ShortcutsOverlayAction::Dismissed);
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let rect = cx.walk_turtle(walk);
        if !self.visible {
            // Nothing drawn -- `main_column` (painted first, same overlay
            // rect) shows through untouched.
            return DrawStep::done();
        }

        self.draw_scrim.draw_abs(cx, rect);

        let panel_h = TITLE_H + BINDINGS.len() as f64 * ROW_H + PANEL_PAD * 2.0;
        let panel_x = rect.pos.x + rect.size.x * 0.5 - PANEL_W * 0.5;
        let panel_y = rect.pos.y + rect.size.y * 0.5 - panel_h * 0.5;
        self.panel_rect = Rect {
            pos: dvec2(panel_x, panel_y),
            size: dvec2(PANEL_W, panel_h),
        };
        self.draw_panel.draw_abs(cx, self.panel_rect);
        self.draw_edge.draw_abs(
            cx,
            Rect {
                pos: self.panel_rect.pos,
                size: dvec2(self.panel_rect.size.x, 1.5),
            },
        );

        self.draw_title.draw_abs(
            cx,
            dvec2(panel_x + PANEL_PAD, panel_y + PANEL_PAD),
            "Shortcuts",
        );

        let mut y = panel_y + PANEL_PAD + TITLE_H;
        for (key, desc) in BINDINGS {
            self.draw_key
                .draw_abs(cx, dvec2(panel_x + PANEL_PAD, y), key);
            self.draw_desc
                .draw_abs(cx, dvec2(panel_x + PANEL_PAD + KEY_COL_W, y), desc);
            y += ROW_H;
        }

        DrawStep::done()
    }
}

impl ShortcutsOverlay {
    pub fn visible(&self) -> bool {
        self.visible
    }

    pub fn set_visible(&mut self, cx: &mut Cx, visible: bool) {
        if self.visible != visible {
            self.visible = visible;
            // `draw_scrim.redraw(cx)` alone isn't enough the first time:
            // `draw_walk` returns early (no draw_abs calls at all) while
            // `!visible`, so `draw_scrim`'s own `Area` stays `Area::Empty`
            // (never assigned a draw-list id) and `Area::redraw` is a no-op
            // for an invalid area (see `Cx::redraw_area`, which only acts
            // when `area.draw_list_id()` is `Some`). `redraw_all` forces
            // the whole window to repaint regardless, which is cheap enough
            // for a rarely-toggled full-screen overlay.
            cx.redraw_all();
        }
    }

    /// Convenience reader for `App`, mirroring `ToolDock::dock_action`.
    pub fn overlay_action(&self, actions: &Actions) -> Option<ShortcutsOverlayAction> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.cast() {
            ShortcutsOverlayAction::None => None,
            action => Some(action),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_list_is_non_empty_and_has_the_toggle_and_close_keys() {
        assert!(!BINDINGS.is_empty());
        assert!(BINDINGS.iter().any(|(k, _)| *k == "?"));
        assert!(BINDINGS.iter().any(|(k, _)| *k == "Esc"));
    }
}
