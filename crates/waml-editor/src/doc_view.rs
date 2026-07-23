//! The app-shell / document-view seam (spec 2026-07-23-diagram-view-seam-design).
//!
//! `BodyWidgets` names the one shared body draw surface the per-tab views push
//! into; the `DocView` trait + `ViewOutcome` + `make_view` factory land in later
//! tasks. Pure Rust — nothing here is a widget, so there is no `script_mod`.

// `DocView`, `ViewOutcome`, `PopupRequest`, and `make_view` land here ahead of
// the wiring that drives them (Tasks 3-5 of the same plan); until then a bin
// crate's dead-code lint would otherwise flag every item. Same convention as
// `nav.rs` / `popup/base.rs`.
#![allow(dead_code)]

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

use waml::model::Model;
use waml::ops::Op;

use crate::doc_tabs::{DocTab, TabKind};
use crate::popup::base::PopupResult;
use crate::popup::select::SelectItem;

/// What a view hands back to the shell per interaction. The shell is the only
/// place that applies ops, opens tabs, and places popups (spec §3).
#[derive(Default)]
pub struct ViewOutcome {
    /// Edit intents the shell applies to `Model`. Empty in the seam migration --
    /// no `Op` is applied in the shell yet; this channel is forward-looking.
    pub ops: Vec<Op>,
    /// Ask the shell to open an element preview by key (spec §5). Unused this
    /// migration: the project tree (shell chrome) still drives previews.
    pub open_preview: Option<String>,
    /// A cross-tree popup the shell must place via `popup_root`.
    pub popup: Option<PopupRequest>,
    /// Ask the shell to promote (pin) the tab whose key matches this subject.
    pub promote_subject: Option<String>,
    /// Ask the shell to close the active tab.
    pub close_active: bool,
    /// Ask the shell to re-push the statusbar snapshot.
    pub statusbar_dirty: bool,
}

/// A popup a view wants placed. The view describes it; the shell computes window
/// bounds + anchor offset and calls `popup_root.show_at` (spec §3 rule 2).
pub enum PopupRequest {
    /// Node command wheel -- items are always `node_radial_items()`.
    NodeRadial { center: DVec2 },
    /// Inspector element-picker flyout.
    ElementPicker {
        anchor_rect: Rect,
        min_width: f64,
        items: Vec<SelectItem>,
    },
}

/// One open document tab's behavior + live state. Shell-owned, one per tab.
pub trait DocView {
    /// Push this view's state into the shared body surface from a read-only
    /// `Model`. Imperative (plain `Cx`), like the shell's old `sync_active_tab`.
    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, model: &Model);

    /// Consume tab-routed actions; return intent upward.
    fn handle(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        actions: &Actions,
        model: &Model,
    ) -> ViewOutcome;

    /// A document-scoped popup this view requested has closed; route its result
    /// back down. `popup_root` is read by the shell; only the result crosses.
    fn on_popup_result(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        model: &Model,
        tag: LiveId,
        result: PopupResult,
    ) -> ViewOutcome {
        let _ = (cx, body, model, tag, result);
        ViewOutcome::default()
    }

    /// Does this view drive the left tool dock? (diagram: yes, preview: no)
    fn wants_tooldock(&self) -> bool;

    fn on_activate(&mut self, cx: &mut Cx, body: &BodyWidgets) {
        let _ = (cx, body);
    }
    fn on_deactivate(&mut self, cx: &mut Cx, body: &BodyWidgets) {
        let _ = (cx, body);
    }
}

/// Create the view object for a tab, discriminating on `TabKind` (spec §5).
pub fn make_view(tab: &DocTab) -> Box<dyn DocView> {
    match tab.kind {
        TabKind::Diagram => Box::new(crate::class_diagram_view::ClassDiagramView::new()),
        TabKind::Classifier => Box::new(
            crate::classifier_preview_view::ClassifierPreviewView::new(tab.key.clone()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc_tabs::{DocTab, TabKind};
    use crate::tree::TreeKind;

    fn tab(kind: TabKind, node_kind: TreeKind) -> DocTab {
        DocTab {
            id: LiveId::from_str("t"),
            key: "k".into(),
            title: "T".into(),
            kind,
            node_kind,
            preview: false,
        }
    }

    #[test]
    fn view_outcome_default_is_all_empty() {
        let o = ViewOutcome::default();
        assert!(o.ops.is_empty());
        assert!(o.open_preview.is_none());
        assert!(o.popup.is_none());
        assert!(o.promote_subject.is_none());
        assert!(!o.close_active);
        assert!(!o.statusbar_dirty);
    }

    #[test]
    fn make_view_dispatches_on_tab_kind() {
        let dv = make_view(&tab(TabKind::Diagram, TreeKind::Diagram));
        assert!(dv.wants_tooldock(), "diagram view drives the tool dock");
        let cv = make_view(&tab(TabKind::Classifier, TreeKind::Class));
        assert!(!cv.wants_tooldock(), "preview view has no tool dock");
    }
}
