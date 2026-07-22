//! The app-shell / document-view seam (spec 2026-07-23-diagram-view-seam-design).
//!
//! `BodyWidgets` names the one shared body draw surface the per-tab views push
//! into; the `DocView` trait + `ViewOutcome` + `make_view` factory land in later
//! tasks. Pure Rust — nothing here is a widget, so there is no `script_mod`.

use makepad_widgets::*;

/// Typed handles to the single shared body surface (canvas + inspector + tool
/// dock + selection toolbar) the active `DocView` renders through. Cheap: holds
/// a clone of the shell's root `ui`; each accessor is the same `ui.widget(..)`
/// lookup the shell used inline, gathered in one place so the seam surface is
/// explicit.
pub struct BodyWidgets {
    ui: WidgetRef,
}

impl BodyWidgets {
    pub fn new(_cx: &mut Cx, ui: &WidgetRef) -> BodyWidgets {
        BodyWidgets { ui: ui.clone() }
    }

    pub fn canvas(&self, cx: &mut Cx) -> WidgetRef {
        self.ui.widget(cx, ids!(canvas))
    }
    pub fn inspector(&self, cx: &mut Cx) -> WidgetRef {
        self.ui.widget(cx, ids!(inspector))
    }
    pub fn tool_dock(&self, cx: &mut Cx) -> WidgetRef {
        self.ui.widget(cx, ids!(tool_dock))
    }
    pub fn selection_toolbar(&self, cx: &mut Cx) -> WidgetRef {
        self.ui.widget(cx, ids!(selection_toolbar))
    }

    /// Show/hide the left tool dock wrapper (`tool_dock_wrap`). Body of the
    /// shell's old `set_diagram_toolbars`.
    pub fn set_tool_dock_visible(&self, cx: &mut Cx, show: bool) {
        self.ui
            .widget(cx, ids!(tool_dock_wrap))
            .set_visible(cx, show);
    }
}
