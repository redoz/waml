//! The app-shell / document-view seam (spec 2026-07-23-diagram-view-seam-design).
//!
//! `BodyWidgets` names the one shared body draw surface the per-tab views push
//! into. Pure Rust: nothing here is a widget, so there is no `script_mod`.

use makepad_widgets::*;
use waml::edit::PendingEdit;
use waml::source::SourceBundle;

use crate::document::EditIntent;
use crate::editor_session::SessionChange;
use crate::folder_list::{FolderListViewRef, FolderListViewWidgetRefExt};
use crate::icons::Icon;
use crate::navigation::NavigationIntent;
use crate::popup::base::PopupItem;
use crate::popup::base::PopupResult;
use crate::popup::select::SelectItem;
use crate::search_results_view::{SearchResultsListViewRef, SearchResultsListViewWidgetRefExt};
use crate::view_history::ViewAnchor;
use waml_markdown_editor::reading::{MarkdownViewerRef, MarkdownViewerWidgetRefExt};
use waml_markdown_editor::widget::{MarkdownEditorRef, MarkdownEditorWidgetRefExt};

/// Typed handles to the single shared body surface (canvas + inspector + tool
/// dock + selection toolbar) the active `DocView` renders through. Cheap: holds
/// a clone of the shell's root `ui`; each accessor is the same `ui.widget(..)`
/// lookup the shell used inline, gathered in one place so the seam surface is
/// explicit.
pub struct BodyWidgets {
    ui: WidgetRef,
    canvas: WidgetRef,
    behavior_canvas: WidgetRef,
    markdown_editor: MarkdownEditorRef,
    markdown_viewer: MarkdownViewerRef,
    folder_list: FolderListViewRef,
    search_results: SearchResultsListViewRef,
    book: WidgetRef,
}

impl BodyWidgets {
    pub fn new(_cx: &mut Cx, ui: &WidgetRef) -> BodyWidgets {
        BodyWidgets {
            ui: ui.clone(),
            canvas: ui.widget(_cx, ids!(canvas)),
            behavior_canvas: ui.widget(_cx, ids!(behavior_canvas)),
            markdown_editor: ui
                .widget(_cx, ids!(markdown_surface.editor))
                .as_markdown_editor(),
            markdown_viewer: ui
                .widget(_cx, ids!(markdown_viewer_surface.viewer_body.viewer))
                .as_markdown_viewer(),
            folder_list: ui
                .widget(_cx, ids!(folder_view_surface.folder_list))
                .as_folder_list_view(),
            search_results: ui
                .widget(_cx, ids!(search_results_surface.search_results_list))
                .as_search_results_list_view(),
            book: ui.widget(_cx, ids!(book_surface.book)),
        }
    }

    pub fn canvas(&self, cx: &mut Cx) -> WidgetRef {
        let _ = cx;
        self.canvas.clone()
    }

    pub fn canvas_ref(&self) -> &WidgetRef {
        &self.canvas
    }

    pub fn behavior_canvas(&self, cx: &mut Cx) -> WidgetRef {
        let _ = cx;
        self.behavior_canvas.clone()
    }

    /// Show/hide the behavior canvas wrapper (`behavior_canvas_wrap`), the
    /// sibling of `canvas_wrap` for activity/state-machine/sequence tabs.
    /// Swap the shared center surface between the class-diagram canvas and
    /// the behavior canvas (activity/state-machine/sequence tabs). Unlike
    /// `show_canvas`/`show_markdown`, both wrappers are canvases, so each
    /// side's own interaction is toggled independently rather than through
    /// `set_canvas_interaction_enabled` (which drives both at once for the
    /// diagram-vs-properties/source seam).
    pub fn set_behavior_canvas_visible(&self, cx: &mut Cx, visible: bool) {
        self.ui
            .widget(cx, ids!(markdown_surface))
            .set_visible(cx, false);
        self.ui
            .widget(cx, ids!(canvas_wrap))
            .set_visible(cx, !visible);
        if let Some(mut canvas) = self
            .canvas(cx)
            .borrow_mut::<crate::canvas::ClassDiagramSurface>()
        {
            canvas.set_interaction_enabled(cx, !visible);
        }
        self.ui
            .widget(cx, ids!(behavior_canvas_wrap))
            .set_visible(cx, visible);
        self.set_behavior_canvas_interaction_enabled(cx, visible);
    }
    /// Push the active document's solver diagnostics to the shared status bar
    /// (spec §5.3). `None` clears the line.
    pub fn set_solver_diagnostics(&self, cx: &mut Cx, message: Option<&str>) {
        if let Some(mut statusbar) = self
            .ui
            .widget(cx, ids!(statusbar))
            .borrow_mut::<crate::statusbar::Statusbar>()
        {
            statusbar.set_solver_diagnostics(cx, message);
        }
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

    /// Toggle the CLASS-diagram canvas' interaction only. The behavior canvas
    /// has its own toggle (`set_behavior_canvas_interaction_enabled`): the two
    /// surfaces are never visible at once, so a class-diagram path must not
    /// re-enable the hidden behavior surface.
    pub fn set_canvas_interaction_enabled(&self, cx: &mut Cx, enabled: bool) {
        if let Some(mut canvas) = self
            .canvas(cx)
            .borrow_mut::<crate::canvas::ClassDiagramSurface>()
        {
            canvas.set_interaction_enabled(cx, enabled);
        }
    }

    /// Toggle the BEHAVIOR canvas' interaction only (the sibling of
    /// `set_canvas_interaction_enabled`).
    pub fn set_behavior_canvas_interaction_enabled(&self, cx: &mut Cx, enabled: bool) {
        if let Some(mut canvas) = self
            .behavior_canvas(cx)
            .borrow_mut::<crate::canvas::BehaviorSurface>()
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
        self.ui
            .widget(cx, ids!(markdown_surface))
            .set_visible(cx, false);
        self.ui
            .widget(cx, ids!(markdown_viewer_surface))
            .set_visible(cx, false);
        self.ui
            .widget(cx, ids!(folder_view_surface))
            .set_visible(cx, false);
        self.ui
            .widget(cx, ids!(search_results_surface))
            .set_visible(cx, false);
        self.ui
            .widget(cx, ids!(book_surface))
            .set_visible(cx, false);
        self.ui.widget(cx, ids!(canvas_wrap)).set_visible(cx, true);
        self.set_canvas_interaction_enabled(cx, true);
    }

    pub fn show_markdown_editor(&self, cx: &mut Cx) {
        self.ui
            .widget(cx, ids!(markdown_surface))
            .set_visible(cx, true);
        self.ui
            .widget(cx, ids!(markdown_viewer_surface))
            .set_visible(cx, false);
        self.ui
            .widget(cx, ids!(folder_view_surface))
            .set_visible(cx, false);
        self.ui
            .widget(cx, ids!(search_results_surface))
            .set_visible(cx, false);
        self.ui
            .widget(cx, ids!(book_surface))
            .set_visible(cx, false);
        self.ui.widget(cx, ids!(canvas_wrap)).set_visible(cx, false);
        self.set_canvas_interaction_enabled(cx, false);
    }

    /// Show the markdown reading view (`markdown_viewer_surface`), the
    /// sibling of `markdown_surface` (the raw-markdown editor). Mutually
    /// exclusive with both the editor surface and the diagram canvas.
    pub fn show_markdown_viewer(&self, cx: &mut Cx) {
        self.ui
            .widget(cx, ids!(markdown_surface))
            .set_visible(cx, false);
        self.ui
            .widget(cx, ids!(markdown_viewer_surface))
            .set_visible(cx, true);
        self.ui
            .widget(cx, ids!(folder_view_surface))
            .set_visible(cx, false);
        self.ui
            .widget(cx, ids!(search_results_surface))
            .set_visible(cx, false);
        self.ui
            .widget(cx, ids!(book_surface))
            .set_visible(cx, false);
        self.ui.widget(cx, ids!(canvas_wrap)).set_visible(cx, false);
        self.set_canvas_interaction_enabled(cx, false);
    }

    /// Show the folder-listing surface (`folder_view_surface`), mutually
    /// exclusive with the canvas and both markdown surfaces above.
    pub fn show_folder_view(&self, cx: &mut Cx) {
        self.ui
            .widget(cx, ids!(markdown_surface))
            .set_visible(cx, false);
        self.ui
            .widget(cx, ids!(markdown_viewer_surface))
            .set_visible(cx, false);
        self.ui
            .widget(cx, ids!(folder_view_surface))
            .set_visible(cx, true);
        self.ui
            .widget(cx, ids!(search_results_surface))
            .set_visible(cx, false);
        self.ui
            .widget(cx, ids!(book_surface))
            .set_visible(cx, false);
        self.ui.widget(cx, ids!(canvas_wrap)).set_visible(cx, false);
        self.set_canvas_interaction_enabled(cx, false);
    }

    /// Show the search-results surface (`search_results_surface`), mutually
    /// exclusive with the canvas, both markdown surfaces, and the folder
    /// view above (`SearchResultsView::sync`, Task 8/9).
    pub fn show_search_results(&self, cx: &mut Cx) {
        self.ui
            .widget(cx, ids!(markdown_surface))
            .set_visible(cx, false);
        self.ui
            .widget(cx, ids!(markdown_viewer_surface))
            .set_visible(cx, false);
        self.ui
            .widget(cx, ids!(folder_view_surface))
            .set_visible(cx, false);
        self.ui
            .widget(cx, ids!(search_results_surface))
            .set_visible(cx, true);
        self.ui
            .widget(cx, ids!(book_surface))
            .set_visible(cx, false);
        self.ui.widget(cx, ids!(canvas_wrap)).set_visible(cx, false);
        self.set_canvas_interaction_enabled(cx, false);
    }

    /// Show the book surface (`book_surface`), mutually exclusive with every
    /// sibling above. The widget itself arrives in a later task; until the
    /// DSL mounts it, the lookup is an absent-widget no-op, which is also
    /// what keeps every headless test green.
    pub fn show_book_view(&self, cx: &mut Cx) {
        for surface in [
            ids!(markdown_surface),
            ids!(markdown_viewer_surface),
            ids!(folder_view_surface),
            ids!(search_results_surface),
            ids!(canvas_wrap),
        ] {
            self.ui.widget(cx, surface).set_visible(cx, false);
        }
        self.ui.widget(cx, ids!(book_surface)).set_visible(cx, true);
        self.set_canvas_interaction_enabled(cx, false);
    }

    /// Take every document surface down. The counterpart to the `show_*`
    /// family above, which are all "mine on, my siblings off" -- fine while
    /// some view owns the centre, but there is no view to run when the last
    /// tab closes, and the surface the closed document was drawing on would
    /// otherwise stay up behind empty chrome.
    pub fn hide_document_surfaces(&self, cx: &mut Cx) {
        for surface in [
            ids!(canvas_wrap),
            ids!(behavior_canvas_wrap),
            ids!(diagram_properties_wrap),
            ids!(markdown_surface),
            ids!(markdown_viewer_surface),
            ids!(folder_view_surface),
            ids!(search_results_surface),
            ids!(book_surface),
        ] {
            self.ui.widget(cx, surface).set_visible(cx, false);
        }
        self.set_canvas_interaction_enabled(cx, false);
        self.set_behavior_canvas_interaction_enabled(cx, false);
    }

    pub fn folder_list(&self) -> FolderListViewRef {
        self.folder_list.clone()
    }

    pub fn search_results(&self) -> SearchResultsListViewRef {
        self.search_results.clone()
    }

    pub fn book_view_widget(&self) -> WidgetRef {
        self.book.clone()
    }

    pub fn markdown_editor(&self) -> MarkdownEditorRef {
        self.markdown_editor.clone()
    }

    pub fn markdown_viewer(&self) -> MarkdownViewerRef {
        self.markdown_viewer.clone()
    }

    /// Scroll the reading surface's own scroller (`viewer_body`) so `offset`
    /// logical pixels into the drawn document sit at the top. The viewer
    /// widget cannot move its own parent, so a reveal on the reading surface
    /// comes through here.
    pub fn scroll_markdown_viewer_to(&self, cx: &mut Cx, offset: f64) {
        if let Some(mut view) = self
            .ui
            .widget(cx, ids!(markdown_viewer_surface.viewer_body))
            .borrow_mut::<View>()
        {
            view.set_scroll_pos(cx, dvec2(0.0, offset.max(0.0)));
        }
    }

    /// The active document's view action button in the breadcrumb header.
    /// Views use it for source/rendered or emphasis destination actions.
    pub fn header_view_action_button(&self, cx: &mut Cx) -> WidgetRef {
        self.ui
            .widget(cx, ids!(document_header))
            .borrow::<crate::document_header::DocumentHeader>()
            .map(|header| header.view_action_button(cx))
            .unwrap_or_default()
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
            header.set_view_toggle(cx, chrome.document_header.view_toggle);
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
    pub edit: Option<EditIntent>,
    pub source_edit: Option<crate::editor_session::ProposedSourceEdit>,
    /// Ask the shell to open an element preview by key (spec §5). Unused this
    /// A cross-tree popup the shell must place via `popup_root`.
    pub popup: Option<PopupRequest>,
    /// Ask the shell to promote (pin) the tab that was active when this
    /// outcome entered shell processing.
    pub promote_active: bool,
    /// Ask the shell to close the active tab.
    pub close_active: bool,
    /// Ask the shell to re-push the statusbar snapshot.
    pub statusbar_dirty: bool,
    /// A focus/selection boundary that must stop older text edits coalescing.
    pub break_merge_group: bool,
    pub navigation: Option<NavigationIntent>,
    /// Ask the shell to open this key's raw markdown source, through the same
    /// path the node context menu's "View Source" item uses (spec §5.2). Read-
    /// only surfaces without a context menu (the behavior canvas) reach this
    /// path from their own selection affordance instead.
    pub view_source: Option<String>,
    /// A search-hit reveal to apply once `navigation` (if any) has landed on
    /// its target document -- `(concept_id, target)`. The shell stashes this
    /// as a pending reveal and applies it through `DocView::reveal` once the
    /// arriving tab has drawn (Task 9, spec §DocView::reveal /
    /// §Activation per document kind).
    pub reveal: Option<(String, RevealTarget)>,
    /// Ask the shell to open this directory's LISTING surface -- the book
    /// header's toggle destination. A `NavigationTarget::Directory` cannot
    /// express this (the Directory arm chain-routes back to the book), so
    /// like `view_source` it names the surface explicitly.
    pub open_folder_listing: Option<String>,
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
    /// A one-item confirm menu (spec §States, activating a projection-hidden
    /// hit -- decision 8). The opener's `tag` comes back on `on_popup_result`
    /// so it can tell a commit from a light-dismiss; `Invoked` means "yes".
    Confirm {
        anchor: DVec2,
        title: String,
        tag: LiveId,
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

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocViewIdentity {
    StructuralDiagram(crate::canvas::StructuralVisualKind),
    Diagram(waml::model::DiagramKind),
    ClassifierPreview(crate::document::NavCategory),
    GenericOkf,
    Source,
    Folder,
    SearchResults,
    Book,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewReconcilePolicy {
    Replace,
    RetainLiveState,
}

/// What a search hit asks the active view to show (spec §DocView::reveal).
/// The one reveal path the results tab, find strip, and F3 traversal all
/// call: text-surface views (source, markdown) map `TextSpan` onto their own
/// scroll/highlight and ignore `ModelElement`; canvas views do the opposite.
/// Both variants have a `DocView::reveal` consumer already (`SourceView`,
/// Task 6); `SearchResultsView` (Task 9) is the first producer.
#[derive(Clone, Debug, PartialEq)]
pub enum RevealTarget {
    TextSpan {
        start: u32,
        end: u32,
    },
    ModelElement {
        key: String,
    },
    /// A projected row inside a composite surface (the book): scroll that
    /// row's section to the fold. Minted by the view that owns the rows
    /// (`DocView::reveal_target_for`), because only it knows which RowIds it
    /// is showing; every other view keeps its no-op `reveal` default.
    Row {
        id: waml::view::row::RowId,
    },
}

#[allow(dead_code)]
pub trait DocView {
    fn identity(&self) -> DocViewIdentity;

    fn reconcile_policy(&self) -> ViewReconcilePolicy {
        ViewReconcilePolicy::Replace
    }

    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, data: ViewData<'_>);

    fn sync_from_session(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        snapshot: &crate::editor_session::EditorSessionSnapshot,
    ) {
        self.sync(cx, body, snapshot.borrowed().into());
    }

    fn sync_external_replacement(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        snapshot: &crate::editor_session::EditorSessionSnapshot,
    ) {
        self.sync_from_session(cx, body, snapshot);
    }

    fn route_ui_event(&mut self, cx: &mut Cx, ui: &WidgetRef, event: &Event) {
        ui.handle_event(cx, event, &mut Scope::empty());
    }

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

    fn after_session_snapshot(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        snapshot: &crate::editor_session::EditorSessionSnapshot,
        change: SessionChange,
    ) {
        self.after_session_change(cx, body, snapshot.borrowed().into(), change);
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

    /// Show `target` on the body surface this view owns (spec
    /// §DocView::reveal). No-op default so folder/generic views -- and any
    /// view kind a reveal target does not apply to -- compile and behave
    /// unchanged.
    fn reveal(&mut self, cx: &mut Cx, body: &BodyWidgets, target: &RevealTarget) {
        let _ = (cx, body, target);
    }

    /// Translate a navigation target into a reveal THIS view can service, or
    /// `None` to let the ordinary open path run. Only the book answers today
    /// (a tree click on a section scrolls instead of opening -- spec
    /// decision 4); the default keeps every other view's click behavior
    /// byte-for-byte unchanged.
    fn reveal_target_for(&self, target: &waml::view::row::RowTarget) -> Option<RevealTarget> {
        let _ = target;
        None
    }

    /// Mirror the live bundle-wide search session's cursor (Task 14, spec
    /// §Search session): `index` is a flat position into `SearchSession::hits`
    /// (results-tab order), or `None` to clear the mark. No-op default --
    /// only `SearchResultsView` has a row list to mark; every other view
    /// ignores it, the same shape as `reveal`.
    fn mark_search_cursor(&mut self, cx: &mut Cx, body: &BodyWidgets, index: Option<usize>) {
        let _ = (cx, body, index);
    }

    fn restore_anchor(
        &mut self,
        _cx: &mut Cx,
        _body: &BodyWidgets,
        _data: ViewData<'_>,
        anchor: &ViewAnchor,
    ) -> bool {
        matches!(anchor, ViewAnchor::None)
    }
}

/// A destination action shown in the shared document header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeaderViewAction {
    pub icon: Icon,
    pub tooltip: &'static str,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DocumentHeaderChrome {
    pub breadcrumb: bool,
    pub right_dock: Option<Icon>,
    /// The active document's destination action in the trailing button row.
    pub view_toggle: Option<HeaderViewAction>,
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
            view_toggle: None,
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
        assert!(o.source_edit.is_none());
        assert!(o.popup.is_none());
        assert!(!o.promote_active);
        assert!(!o.close_active);
        assert!(!o.statusbar_dirty);
        assert!(!o.break_merge_group);
        assert!(o.navigation.is_none());
        assert!(o.view_source.is_none());
        assert!(o.reveal.is_none());
    }

    #[test]
    fn concrete_views_declare_the_existing_chrome() {
        let diagram = crate::class_diagram_view::ClassDiagramView::new(
            "d".into(),
            crate::StructuralVisualKind::Class,
        );
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
                    right_dock: Some(Icon::PanelRight),
                    view_toggle: None,
                },
            }
        );
        assert_eq!(
            classifier.chrome(),
            BodyChrome {
                tool_dock: false,
                view_bar: false,
                canvas_overlays: false,
                document_header: DocumentHeaderChrome {
                    breadcrumb: true,
                    right_dock: Some(Icon::PanelRight),
                    view_toggle: None,
                },
            }
        );
        assert_eq!(
            source.chrome(),
            BodyChrome {
                tool_dock: false,
                view_bar: false,
                canvas_overlays: false,
                document_header: DocumentHeaderChrome {
                    breadcrumb: true,
                    right_dock: Some(Icon::PanelRight),
                    view_toggle: Some(HeaderViewAction {
                        icon: Icon::Eye,
                        tooltip: "Use layout emphasis",
                    }),
                },
            }
        );
        assert_eq!(
            generic.chrome(),
            BodyChrome {
                tool_dock: false,
                view_bar: false,
                canvas_overlays: false,
                document_header: DocumentHeaderChrome {
                    breadcrumb: true,
                    right_dock: None,
                    view_toggle: Some(HeaderViewAction {
                        icon: Icon::Code,
                        tooltip: "View source",
                    }),
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
