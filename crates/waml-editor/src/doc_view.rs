//! The app-shell / document-view seam (spec 2026-07-23-diagram-view-seam-design).
//!
//! `BodyWidgets` names the one shared body draw surface the per-tab views push
//! into. Pure Rust: nothing here is a widget, so there is no `script_mod`.

use makepad_widgets::*;
use waml::edit::PendingEdit;
use waml::source::SourceBundle;

use crate::editor_session::SessionChange;
use crate::icons::Icon;
use crate::navigation::NavigationIntent;
use crate::popup::base::PopupItem;
use crate::popup::base::PopupResult;
use crate::popup::select::SelectItem;
use crate::view_history::ViewAnchor;

/// Typed handles to the single shared body surface (canvas + inspector + tool
/// dock + selection toolbar) the active `DocView` renders through. Cheap: holds
/// a clone of the shell's root `ui`; each accessor is the same `ui.widget(..)`
/// lookup the shell used inline, gathered in one place so the seam surface is
/// explicit.
pub struct BodyWidgets {
    ui: WidgetRef,
    canvas: WidgetRef,
    markdown: MarkdownRef,
}

impl BodyWidgets {
    pub fn new(_cx: &mut Cx, ui: &WidgetRef) -> BodyWidgets {
        BodyWidgets {
            ui: ui.clone(),
            canvas: ui.widget(_cx, ids!(canvas)),
            markdown: ui.widget(_cx, ids!(markdown_surface.md)).as_markdown(),
        }
    }

    pub fn canvas(&self, cx: &mut Cx) -> WidgetRef {
        let _ = cx;
        self.canvas.clone()
    }

    pub fn canvas_ref(&self) -> &WidgetRef {
        &self.canvas
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
        crate::markdown_surface::hide(&self.ui, cx);
        self.ui.widget(cx, ids!(canvas_wrap)).set_visible(cx, true);
        self.set_canvas_interaction_enabled(cx, true);
    }

    pub fn show_markdown(&self, cx: &mut Cx) {
        crate::markdown_surface::show(&self.ui, cx);
        self.set_canvas_interaction_enabled(cx, false);
    }

    pub fn set_markdown(&self, cx: &mut Cx, markdown: &str) {
        crate::markdown_surface::set_markdown(&self.ui, cx, markdown);
    }

    pub fn markdown_link(&self, actions: &Actions) -> Option<String> {
        crate::markdown_surface::link_navigated(actions)
    }

    pub fn scroll_markdown_to_fragment(&self, cx: &mut Cx, fragment: &str) -> bool {
        self.markdown.scroll_to_fragment(cx, fragment)
    }

    pub fn markdown_scroll_y(&self) -> f64 {
        self.markdown.scroll_y()
    }

    pub fn set_markdown_scroll_y(&self, cx: &mut Cx, scroll_y: f64) {
        self.markdown.set_scroll_y(cx, scroll_y);
    }

    pub fn apply_chrome(&self, cx: &mut Cx, chrome: BodyChrome) {
        self.set_tool_dock_visible(cx, chrome.tool_dock);
        self.set_view_bar_visible(cx, chrome.view_bar);
        self.set_conflict_badge_visible(cx, chrome.canvas_overlays);

        if let Some(mut header) = self
            .ui
            .widget(cx, ids!(document_header))
            .borrow_mut::<crate::document_header::DocumentHeader>()
        {
            header.set_right_dock(cx, chrome.document_header.right_dock);
        }
        if chrome.document_header.right_dock.is_none() {
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
    pub navigation: Option<NavigationIntent>,
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
    /// A document-owned select flyout. The tag is opaque to the shell and is
    /// relayed back to the active document when the popup closes.
    Select {
        tag: LiveId,
        anchor_rect: Rect,
        min_width: f64,
        items: Vec<SelectItem>,
        compact_frame: bool,
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
    pub source: &'a SourceBundle,
    pub okf_analysis: &'a waml::analysis::OkfAnalysis,
    pub uml_analysis: &'a waml::uml::Analysis,
    #[allow(dead_code)]
    pub revision: u64,
}

#[allow(dead_code)]
pub struct PreparedAction {
    pub title: String,
    pub edit: PendingEdit,
}

impl<'a> From<crate::editor_session::EditorSnapshot<'a>> for ViewData<'a> {
    fn from(snapshot: crate::editor_session::EditorSnapshot<'a>) -> Self {
        Self {
            source: snapshot.source,
            okf_analysis: snapshot.okf_analysis,
            uml_analysis: snapshot.uml_analysis,
            revision: snapshot.revision,
        }
    }
}

#[allow(dead_code)]
impl ViewData<'_> {
    fn document_id(&self, concept_id: &str) -> Option<waml::analysis::DocumentId> {
        let path = self.source.document_by_concept_id(concept_id)?.path();
        self.okf_analysis.catalog.id_for_path(path)
    }

    pub fn uml_repair_actions(
        self,
        concept_id: &str,
    ) -> Result<Vec<PreparedAction>, waml::action::ActionError> {
        let Some(document) = self.document_id(concept_id) else {
            return Ok(Vec::new());
        };
        let context =
            waml::uml::ActionContext::new(self.okf_analysis, self.uml_analysis, self.revision)?;
        waml::uml::repair_actions(context, document)?
            .into_iter()
            .map(|action| {
                let title = action.title.clone();
                let batch = waml::action::SyntaxChangeBatch::new(action)?;
                Ok(PreparedAction {
                    title,
                    edit: PendingEdit::new(batch),
                })
            })
            .collect()
    }

    pub fn uml_format_action(
        self,
        concept_id: &str,
    ) -> Result<Option<PreparedAction>, waml::edit::EditError> {
        let Some(document) = self.document_id(concept_id) else {
            return Ok(None);
        };
        let context =
            waml::uml::ActionContext::new(self.okf_analysis, self.uml_analysis, self.revision)
                .map_err(waml::edit::EditError::from)?;
        let action = waml::uml::Formatter
            .format(context, document)
            .map_err(waml::edit::EditError::from)?;
        let title = action.title.clone();
        let batch =
            waml::action::SyntaxChangeBatch::new(action).map_err(waml::edit::EditError::from)?;
        Ok(Some(PreparedAction {
            title,
            edit: PendingEdit::new(batch),
        }))
    }
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

    fn capture_anchor(&self, _body: &BodyWidgets) -> ViewAnchor {
        ViewAnchor::None
    }

    fn restore_anchor(&mut self, _cx: &mut Cx, _body: &BodyWidgets, anchor: &ViewAnchor) -> bool {
        matches!(anchor, ViewAnchor::None)
    }
}

/// Which pieces of the shared body chrome the active tab drives.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DocumentHeaderChrome {
    pub breadcrumb: bool,
    pub right_dock: Option<Icon>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BodyChrome {
    /// The left tool dock (`tool_dock_wrap`).
    pub tool_dock: bool,
    /// The bottom-centre view bar (`view_bar_wrap`).
    pub view_bar: bool,
    /// Canvas-only overlays such as the conflict badge.
    pub canvas_overlays: bool,
    pub document_header: DocumentHeaderChrome,
}

impl BodyChrome {
    pub const HIDDEN: BodyChrome = BodyChrome {
        tool_dock: false,
        view_bar: false,
        canvas_overlays: false,
        document_header: DocumentHeaderChrome {
            breadcrumb: false,
            right_dock: None,
        },
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
        assert!(o.navigation.is_none());
    }

    #[test]
    fn concrete_views_declare_the_existing_chrome() {
        let diagram = crate::class_diagram_view::ClassDiagramView::new("d".into());
        let classifier = crate::classifier_preview_view::ClassifierPreviewView::new(
            "order".into(),
            TreeKind::Class,
        );
        let source = crate::source_view::SourceView::new("order".into());
        let generic = crate::generic_okf_view::GenericOkfView::new("order".into());

        assert_eq!(
            diagram.chrome(),
            BodyChrome {
                tool_dock: true,
                view_bar: true,
                canvas_overlays: true,
                document_header: DocumentHeaderChrome {
                    breadcrumb: true,
                    right_dock: Some(Icon::SlidersHorizontal),
                },
            }
        );
        for chrome in [classifier.chrome(), source.chrome()] {
            assert_eq!(
                chrome,
                BodyChrome {
                    tool_dock: false,
                    view_bar: false,
                    canvas_overlays: false,
                    document_header: DocumentHeaderChrome {
                        breadcrumb: true,
                        right_dock: Some(Icon::SlidersHorizontal),
                    },
                }
            );
        }
        assert_eq!(
            generic.chrome(),
            BodyChrome {
                tool_dock: false,
                view_bar: false,
                canvas_overlays: false,
                document_header: DocumentHeaderChrome {
                    breadcrumb: true,
                    right_dock: None,
                },
            }
        );
        assert_eq!(
            BodyChrome::HIDDEN.document_header,
            DocumentHeaderChrome::default()
        );
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
