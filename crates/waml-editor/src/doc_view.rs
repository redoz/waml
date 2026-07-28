//! The app-shell / document-view seam (spec 2026-07-23-diagram-view-seam-design).
//!
//! `BodyWidgets` names the one shared body draw surface the per-tab views push
//! into. Pure Rust: nothing here is a widget, so there is no `script_mod`.

// A bin crate's dead-code lint would otherwise flag seam members exercised by
// unit tests and provider-owned views. Same convention as `nav.rs`.
#![allow(dead_code)]

use makepad_widgets::*;
use waml::edit::PendingEdit;
use waml::model::Model;
use waml::source::SourceBundle;

use crate::editor_session::SessionChange;
use crate::icon_button::IconButtonWidgetRefExt;
use crate::icons::Icon;
use crate::popup::base::PopupItem;
use crate::popup::base::PopupResult;
use crate::popup::select::SelectItem;

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
    pub fn source_view(&self, cx: &mut Cx) -> WidgetRef {
        self.ui.widget(cx, ids!(source_view))
    }
    pub fn view_bar(&self, cx: &mut Cx) -> WidgetRef {
        self.ui.widget(cx, ids!(view_bar))
    }
    pub fn diagram_properties(&self, cx: &mut Cx) -> WidgetRef {
        self.ui.widget(cx, ids!(diagram_properties))
    }

    /// Swap the shared center surface between the diagram canvas and the
    /// diagram-properties page. This only changes wrapper visibility: the
    /// canvas scene, selection, and camera remain untouched.
    pub fn set_diagram_properties_visible(&self, cx: &mut Cx, visible: bool) {
        self.ui
            .widget(cx, ids!(canvas_wrap))
            .set_visible(cx, !visible);
        self.set_canvas_interaction_enabled(cx, !visible);
        self.ui
            .widget(cx, ids!(diagram_properties_wrap))
            .set_visible(cx, visible);
    }

    pub fn set_canvas_interaction_enabled(&self, cx: &mut Cx, enabled: bool) {
        if let Some(mut canvas) = self
            .canvas(cx)
            .borrow_mut::<crate::canvas::ClassDiagramSurface>()
        {
            canvas.set_interaction_enabled(cx, enabled);
        }
    }

    /// Show/hide the left tool dock wrapper (`tool_dock_wrap`). Body of the
    /// shell's old `set_diagram_toolbars`.
    pub fn set_tool_dock_visible(&self, cx: &mut Cx, show: bool) {
        self.ui
            .widget(cx, ids!(tool_dock_wrap))
            .set_visible(cx, show);
    }

    /// Show/hide the bottom-centre view bar (`view_bar_wrap`). Diagram-only,
    /// like the tool dock: the bar's actions are routed by
    /// `ClassDiagramView::handle`, so showing it over a preview/source tab
    /// would flip its toggles with nothing to act on them.
    pub fn set_view_bar_visible(&self, cx: &mut Cx, show: bool) {
        self.ui
            .widget(cx, ids!(view_bar_wrap))
            .set_visible(cx, show);
    }

    pub fn show_canvas(&self, cx: &mut Cx) {
        self.source_view(cx).set_visible(cx, false);
        self.ui.widget(cx, ids!(canvas_wrap)).set_visible(cx, true);
        self.set_canvas_interaction_enabled(cx, true);
    }

    pub fn show_source(&self, cx: &mut Cx) {
        self.source_view(cx).set_visible(cx, true);
        self.ui.widget(cx, ids!(canvas_wrap)).set_visible(cx, false);
        self.set_canvas_interaction_enabled(cx, false);
    }

    pub fn set_source_markdown(&self, cx: &mut Cx, markdown: &str) {
        self.ui
            .widget(cx, ids!(source_view.md))
            .as_markdown()
            .set_text(cx, markdown);
    }

    pub fn apply_chrome(&self, cx: &mut Cx, chrome: BodyChrome) {
        self.set_tool_dock_visible(cx, chrome.tool_dock);
        self.set_view_bar_visible(cx, chrome.view_bar);
        self.set_conflict_badge_visible(cx, chrome.canvas_overlays);

        let button = self.ui.widget(cx, ids!(inspector_btn));
        if button.visible() != chrome.right_dock.is_some() {
            button.set_visible(cx, chrome.right_dock.is_some());
            cx.redraw_all();
        }
        if let Some(icon) = chrome.right_dock {
            button.as_icon_button().set_icon(cx, icon);
        }
        if let Some(mut tabs) = self
            .ui
            .widget(cx, ids!(doc_tabs))
            .borrow_mut::<crate::doc_tabs::DocTabs>()
        {
            tabs.set_right_dock_btn(cx, chrome.right_dock.is_some());
        }
        if chrome.right_dock.is_none() {
            if let Some(mut panel) = self
                .ui
                .widget(cx, ids!(inspector))
                .borrow_mut::<crate::inspector_panel::Inspector>()
            {
                panel.close_dock(cx);
            }
        }
    }

    /// Show/hide the canvas conflict badge wrapper. The badge retains its own
    /// count-driven visibility while hidden, so returning to the canvas
    /// restores the correct state without recomputing it.
    pub fn set_conflict_badge_visible(&self, cx: &mut Cx, show: bool) {
        self.ui
            .widget(cx, ids!(conflict_badge_wrap))
            .set_visible(cx, show);
    }
}

/// What a view hands back to the shell per interaction. The shell is the only
/// place that applies ops, opens tabs, and places popups (spec §3).
#[derive(Default)]
pub struct ViewOutcome {
    pub edit: Option<PendingEdit>,
    /// Ask the shell to open an element preview by key (spec §5). Unused this
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
    /// The uniform node context menu -- `context` items (surface-contributed)
    /// followed by the base items, placed by the shell at `anchor`.
    NodeContextMenu {
        anchor: DVec2,
        key: String,
        context: Vec<PopupItem>,
    },
    /// Inspector element-picker flyout.
    ElementPicker {
        anchor_rect: Rect,
        min_width: f64,
        items: Vec<SelectItem>,
    },
    /// The drag-to-place dial: the shared radial, popped centred on `center`
    /// mid-drag and released with the primary button (the drag that opened it
    /// is still in flight).
    PlaceDial {
        center: DVec2,
        items: Vec<PopupItem>,
    },
    /// Dismiss whatever popup is open, without opening a replacement.
    Dismiss,
}

#[derive(Clone, Copy)]
pub struct ViewData<'a> {
    pub model: &'a Model,
    pub bundle: &'a SourceBundle,
    pub revision: u64,
}

pub trait DocView {
    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, data: ViewData<'_>);

    fn handle(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        actions: &Actions,
        data: ViewData<'_>,
    ) -> ViewOutcome;

    fn on_popup_result(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        data: ViewData<'_>,
        tag: LiveId,
        result: PopupResult,
    ) -> ViewOutcome {
        let _ = (cx, body, data, tag, result);
        ViewOutcome::default()
    }

    fn on_popup_armed(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        data: ViewData<'_>,
        tag: LiveId,
        id: Option<LiveId>,
    ) -> ViewOutcome {
        let _ = (cx, body, data, tag, id);
        ViewOutcome::default()
    }

    fn after_session_change(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        data: ViewData<'_>,
        _change: SessionChange,
    ) {
        self.sync(cx, body, data);
    }

    fn chrome(&self) -> BodyChrome;

    fn tab_accent(&self) -> Option<Vec4> {
        None
    }

    fn on_activate(&mut self, cx: &mut Cx, body: &BodyWidgets) {
        let _ = (cx, body);
    }

    fn on_deactivate(&mut self, cx: &mut Cx, body: &BodyWidgets) {
        let _ = (cx, body);
    }

    fn on_escape(&mut self, cx: &mut Cx, body: &BodyWidgets) {
        let _ = (cx, body);
    }
}

/// Which pieces of the shared body chrome the active tab drives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BodyChrome {
    /// The left tool dock (`tool_dock_wrap`).
    pub tool_dock: bool,
    /// The bottom-centre view bar (`view_bar_wrap`).
    pub view_bar: bool,
    /// Canvas-only overlays such as the conflict badge.
    pub canvas_overlays: bool,
    /// The right-hand docked panel the active view drives, and the glyph its
    /// caption toggle wears (`None` = no dock, so the toggle is hidden).
    pub right_dock: Option<Icon>,
}

impl BodyChrome {
    pub const HIDDEN: BodyChrome = BodyChrome {
        tool_dock: false,
        view_bar: false,
        canvas_overlays: false,
        right_dock: None,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::TreeKind;

    #[test]
    fn view_outcome_default_is_all_empty() {
        let o = ViewOutcome::default();
        assert!(o.edit.is_none());
        assert!(o.popup.is_none());
        assert!(o.promote_subject.is_none());
        assert!(!o.close_active);
        assert!(!o.statusbar_dirty);
    }

    #[test]
    fn concrete_views_declare_the_existing_chrome() {
        let diagram = crate::class_diagram_view::ClassDiagramView::new("d".into());
        let classifier = crate::classifier_preview_view::ClassifierPreviewView::new(
            "order".into(),
            TreeKind::Class,
        );
        let source = crate::source_view::SourceView::new("order".into());

        assert_eq!(
            diagram.chrome(),
            BodyChrome {
                tool_dock: true,
                view_bar: true,
                canvas_overlays: true,
                right_dock: Some(Icon::SlidersHorizontal),
            }
        );
        for chrome in [classifier.chrome(), source.chrome()] {
            assert_eq!(
                chrome,
                BodyChrome {
                    tool_dock: false,
                    view_bar: false,
                    canvas_overlays: false,
                    right_dock: Some(Icon::SlidersHorizontal),
                }
            );
        }
    }

    #[test]
    fn accents_come_from_self_identifying_views() {
        let classifier = crate::classifier_preview_view::ClassifierPreviewView::new(
            "status".into(),
            TreeKind::Enum,
        );
        let source = crate::source_view::SourceView::new("status".into());

        assert_eq!(
            classifier.tab_accent(),
            crate::accent::tree_kind_color(TreeKind::Enum)
        );
        assert_eq!(
            source.tab_accent(),
            Some(crate::accent::bucket_color(
                crate::node_style::AccentBucket::None,
            ))
        );
    }
}
