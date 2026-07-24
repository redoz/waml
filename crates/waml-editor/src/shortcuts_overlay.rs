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

use crate::overlay_shell::{OverlayShell, OverlayShellAction};

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
        shell +: {
            panel_width: 360.0
            draw_scrim +: { color: atlas.scrim }
            draw_panel +: { color: atlas.surface }
            draw_edge +: { color: atlas.frame_hi }
            draw_thumb +: { color: atlas.frame_lo }
        }
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

    #[live]
    shell: OverlayShell,

    #[redraw]
    #[live]
    draw_title: DrawText,
    #[redraw]
    #[live]
    draw_key: DrawText,
    #[redraw]
    #[live]
    draw_desc: DrawText,
}

impl Widget for ShortcutsOverlay {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if let OverlayShellAction::Dismissed = self.shell.handle_event(cx, event) {
            cx.widget_action(self.widget_uid(), ShortcutsOverlayAction::Dismissed);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, _walk: Walk) -> DrawStep {
        let h = self.content_height();
        if let Some(pass) = self.shell.begin(cx, h) {
            self.draw_rows(cx, pass.origin, pass.width);
            self.shell.end(cx);
        }
        DrawStep::done()
    }
}

impl ShortcutsOverlay {
    /// Content height the shell needs to size + scroll the panel.
    fn content_height(&self) -> f64 {
        TITLE_H + BINDINGS.len() as f64 * ROW_H
    }

    /// Draw the title + key/desc rows relative to the shell-provided origin.
    fn draw_rows(&mut self, cx: &mut Cx2d, origin: DVec2, _width: f64) {
        self.draw_title.draw_abs(cx, origin, "Shortcuts");
        let mut y = origin.y + TITLE_H;
        for (key, desc) in BINDINGS {
            self.draw_key.draw_abs(cx, dvec2(origin.x, y), key);
            self.draw_desc
                .draw_abs(cx, dvec2(origin.x + KEY_COL_W, y), desc);
            y += ROW_H;
        }
    }

    pub fn visible(&self) -> bool {
        self.shell.is_open()
    }

    pub fn set_visible(&mut self, cx: &mut Cx, visible: bool) {
        self.shell.set_open(cx, visible);
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
