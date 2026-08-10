use crate::doc_tabs::{DocTab, DocTabs, OpenTabs};
use crate::doc_view::{
    BodyChrome, BodyWidgets, DocView, RevealTarget, ViewOutcome, ViewReconcilePolicy,
};
use crate::document::{NavCategory, OpenDocument};
use crate::editor_session::{EditorSession, SessionChange};
use crate::popup::base::PopupResult;
use crate::view_history::{DocumentLocator, ViewAnchor, ViewLocation};
use makepad_widgets::*;
use std::collections::{HashMap, HashSet};

pub enum DocumentCommand {
    Open {
        document: OpenDocument,
        persistent: bool,
    },
    Activate(LiveId),
    Promote(LiveId),
    Close(LiveId),
    /// Swap the view behind an ALREADY-OPEN tab, leaving tab order,
    /// selection, and the active tab untouched. A no-op when `document`'s tab
    /// is not open.
    ///
    /// Distinct from `Open`, which deliberately keeps the existing view when
    /// the tab id is already open: an ordinary re-navigation must not discard
    /// a markdown editor's scroll position or an in-flight edit. Only a caller
    /// that KNOWS the view must change -- a session-wide projected/raw flip --
    /// asks for this.
    ReopenInPlace {
        document: OpenDocument,
    },
}

#[derive(Default)]
pub struct DocumentHost {
    tabs: OpenTabs,
    views: HashMap<LiveId, Box<dyn DocView>>,
    /// Last anchor (selection, camera, scroll) each tab had when it stopped
    /// being active, so switching back to a tab restores where the user left
    /// it instead of resetting to a blank view. Entries are evicted when
    /// their tab closes.
    anchors: HashMap<LiveId, ViewAnchor>,
}

type RemovedViews = Vec<(LiveId, Box<dyn DocView>)>;

enum ActiveReconciliation {
    Retained,
    Replaced { old_view: Option<Box<dyn DocView>> },
}

impl DocumentHost {
    fn replace_tabs_for_session(&mut self, tabs: OpenTabs) -> RemovedViews {
        let removed = self.views.drain().collect();
        self.anchors.clear();
        self.tabs = tabs;
        self.reconcile_registry();
        removed
    }

    fn reconcile_registry(&mut self) -> RemovedViews {
        let open: HashSet<LiveId> = self.tabs.tabs.iter().map(|tab| tab.id).collect();
        let stale: Vec<LiveId> = self
            .views
            .keys()
            .copied()
            .filter(|id| !open.contains(id))
            .collect();
        for id in &stale {
            self.anchors.remove(id);
        }
        stale
            .into_iter()
            .filter_map(|id| self.views.remove(&id).map(|view| (id, view)))
            .collect()
    }

    fn apply_command(&mut self, command: DocumentCommand) -> (bool, RemovedViews) {
        let before = self.tabs.clone();
        match command {
            DocumentCommand::Open {
                document,
                persistent,
            } => {
                let already_open = self.tabs.tabs.iter().any(|tab| tab.id == document.tab_id);
                let (tab, view) = document.into_tab(true);
                let id = self.tabs.open_preview(tab);
                if !already_open {
                    self.views.insert(id, view);
                }
                if persistent {
                    self.tabs.promote(id);
                }
            }
            DocumentCommand::Activate(id) => self.tabs.activate(id),
            DocumentCommand::Promote(id) => {
                self.tabs.activate(id);
                self.tabs.promote(id);
            }
            DocumentCommand::Close(id) => self.tabs.close(id),
            DocumentCommand::ReopenInPlace { document } => {
                let tab_id = document.tab_id;
                if self.tabs.tabs.iter().any(|tab| tab.id == tab_id) {
                    let (_tab, view) = document.into_tab(true);
                    self.views.insert(tab_id, view);
                }
            }
        }
        let removed = self.reconcile_registry();
        (self.tabs != before, removed)
    }

    pub fn active_tab(&self) -> Option<&DocTab> {
        self.tabs.active_tab()
    }

    pub fn tabs(&self) -> &[DocTab] {
        &self.tabs.tabs
    }

    pub fn tab_id_for_locator(&self, locator: &DocumentLocator) -> Option<LiveId> {
        self.tabs
            .tabs
            .iter()
            .find(|tab| tab.locator() == *locator)
            .map(|tab| tab.id)
    }

    pub fn active_id(&self) -> LiveId {
        self.tabs.active
    }

    pub fn active_chrome(&self) -> BodyChrome {
        self.views
            .get(&self.tabs.active)
            .map(|view| view.chrome())
            .unwrap_or(BodyChrome::HIDDEN)
    }

    pub fn active_accent(&self) -> Option<Vec4> {
        self.tabs.active_tab().and_then(|tab| {
            tab.presentation.accent.or_else(|| {
                self.views
                    .get(&self.tabs.active)
                    .and_then(|view| view.tab_accent())
            })
        })
    }

    pub fn scroll_active_to_fragment(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        session: &EditorSession,
        fragment: &str,
    ) -> bool {
        let active_uses_markdown = self
            .tabs
            .active_tab()
            .is_some_and(|tab| tab.presentation.category == NavCategory::OkfDocument);
        if !active_uses_markdown {
            return false;
        }
        let body = BodyWidgets::new(cx, ui);
        if let Some(view) = self.views.get_mut(&self.tabs.active) {
            let mut anchor = view.capture_anchor(&body);
            let ViewAnchor::Markdown {
                fragment: anchor_fragment,
                ..
            } = &mut anchor
            else {
                return false;
            };
            *anchor_fragment = Some(fragment.to_owned());
            let snapshot = session.snapshot();
            return view.restore_anchor(cx, &body, snapshot.borrowed().into(), &anchor);
        }
        false
    }

    pub fn capture_active_location(&self, cx: &mut Cx, ui: &WidgetRef) -> Option<ViewLocation> {
        let tab = self.tabs.active_tab()?;
        let view = self.views.get(&tab.id)?;
        let body = BodyWidgets::new(cx, ui);
        Some(ViewLocation {
            document: tab.locator(),
            anchor: view.capture_anchor(&body),
        })
    }

    pub fn restore_active_anchor(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        session: &EditorSession,
        anchor: &ViewAnchor,
    ) -> bool {
        let body = BodyWidgets::new(cx, ui);
        let snapshot = session.snapshot();
        self.views
            .get_mut(&self.tabs.active)
            .is_some_and(|view| view.restore_anchor(cx, &body, snapshot.borrowed().into(), anchor))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore_location_with_asset_host(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        session: &EditorSession,
        location: &ViewLocation,
        assets: &crate::markdown_hosts::SharedMarkdownAssetHost,
        emphasis: waml_markdown_editor::EditorEmphasis,
        limits: waml::view::chain::ChainLimits,
        mask: &waml::view::mask::ProjectionMask,
    ) -> bool {
        let Some(document) = crate::documents::open_locator_with_asset_host(
            session.okf_analysis(),
            session.uml_analysis(),
            &location.document,
            assets,
            emphasis,
            limits,
            mask,
        ) else {
            return false;
        };
        if let Some(id) = self.tab_id_for_locator(&location.document) {
            self.transition(cx, ui, session, DocumentCommand::Activate(id));
        } else {
            self.transition(
                cx,
                ui,
                session,
                DocumentCommand::Open {
                    document,
                    persistent: false,
                },
            );
        }
        let _ = self.restore_active_anchor(cx, ui, session, &location.anchor);
        true
    }

    /// The search-results counterpart to `restore_location_with_asset_host`:
    /// `document` is already built (a live-query result, not something the
    /// surface table can produce for a `Virtual` target) so this only
    /// activates-or-opens it, the same tab-identity rule as the generic path
    /// (same locator -> same tab id -> `Activate`, never a duplicate).
    /// `App::transition_to_location` calls this instead of
    /// `restore_location_with_asset_host` for a search locator (decision 7),
    /// so a search tab shares every other bookkeeping step (view history,
    /// chrome sync, pending-anchor restore) with an ordinary document.
    ///
    /// Opens PERSISTENT, unlike the shared classifier/diagram/source preview
    /// slot: decision 7 says "two queries are two tabs", and a preview open
    /// would replace one query's tab with the next the moment a second query
    /// runs (`open_preview_twice_replaces_the_single_preview_slot`) -- search
    /// results are never a fly-by preview the way a tree-panel click is.
    pub fn restore_location_with_document(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        session: &EditorSession,
        location: &ViewLocation,
        document: OpenDocument,
    ) -> bool {
        if let Some(id) = self.tab_id_for_locator(&location.document) {
            // `document` is a LIVE re-run of the query, not a rebuild of what
            // the tab already shows: merely activating the open tab would
            // leave the previous run's rows on screen while the caller
            // (`App::open_search_results`) has already installed a session
            // over THESE hits, and that session's flat index would then mark
            // a row from a different list. `ReopenInPlace` is the one command
            // that swaps a view behind an already-open tab.
            self.transition(cx, ui, session, DocumentCommand::ReopenInPlace { document });
            self.transition(cx, ui, session, DocumentCommand::Activate(id));
        } else {
            self.transition(
                cx,
                ui,
                session,
                DocumentCommand::Open {
                    document,
                    persistent: true,
                },
            );
        }
        let _ = self.restore_active_anchor(cx, ui, session, &location.anchor);
        true
    }

    /// Apply a search-hit reveal to the ACTIVE view (Task 9's `pending_reveal`
    /// consumer, `App::apply_pending_reveal`). `false` when nothing is active
    /// to reveal on -- the caller checks the active tab's identity first, so
    /// this is a defensive fallback, not the primary guard.
    pub fn reveal_active(&mut self, cx: &mut Cx, ui: &WidgetRef, target: &RevealTarget) -> bool {
        let body = BodyWidgets::new(cx, ui);
        match self.views.get_mut(&self.tabs.active) {
            Some(view) => {
                view.reveal(cx, &body, target);
                true
            }
            None => false,
        }
    }

    /// Mirror the live search session's cursor into `tab_id`'s view (Task 14)
    /// -- NOT necessarily the active tab: F3 traversal keeps walking after
    /// the results tab itself is no longer showing, and Esc must still be
    /// able to clear its mark. A no-op (`false`) when that tab isn't open
    /// (closed, or never opened at all -- the session still ends cleanly).
    pub fn mark_search_cursor(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        tab_id: LiveId,
        index: Option<usize>,
    ) -> bool {
        let body = BodyWidgets::new(cx, ui);
        match self.views.get_mut(&tab_id) {
            Some(view) => {
                view.mark_search_cursor(cx, &body, index);
                true
            }
            None => false,
        }
    }

    #[cfg(test)]
    pub fn restore_location(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        session: &EditorSession,
        location: &ViewLocation,
    ) -> bool {
        self.restore_location_with_asset_host(
            cx,
            ui,
            session,
            location,
            &crate::markdown_hosts::EditorMarkdownAssetHost::shared(
                crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle,
            ),
            waml_markdown_editor::EditorEmphasis::Code,
            waml::view::chain::ChainLimits::default(),
            &waml::view::mask::ProjectionMask::default(),
        )
    }

    fn refresh_tabs(&self, cx: &mut Cx, ui: &WidgetRef) {
        if let Some(mut tabs) = ui.widget(cx, ids!(doc_tabs)).borrow_mut::<DocTabs>() {
            tabs.set_tabs(cx, &self.tabs);
            tabs.set_active_accent(cx, self.active_accent());
        }
    }

    pub fn sync_active(&mut self, cx: &mut Cx, ui: &WidgetRef, session: &EditorSession) {
        self.reconcile_registry();
        let body = BodyWidgets::new(cx, ui);
        body.apply_chrome(cx, self.active_chrome());
        let active = self.tabs.active;
        let active_resolved = self.tabs.active_tab().is_some_and(|tab| tab.resolved);
        if active_resolved {
            if let Some(view) = self.views.get_mut(&active) {
                let snapshot = session.snapshot();
                view.sync_from_session(cx, &body, &snapshot);
                return;
            }
        }
        // No active view at all, or the active tab is tombstoned (its
        // locator no longer resolves): neither canvas may take input.
        body.set_canvas_interaction_enabled(cx, false);
        body.set_behavior_canvas_interaction_enabled(cx, false);
        if self.tabs.active_tab().is_none() {
            // Nothing is open, so nothing owns the centre. A tombstoned tab is
            // the other way round -- it stays on screen, dimmed and inert, so
            // only the genuinely empty case takes the surfaces down.
            body.hide_document_surfaces(cx);
        }
    }

    fn finish_transition(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        session: &EditorSession,
        old_active: LiveId,
        mut removed: RemovedViews,
    ) {
        let body = BodyWidgets::new(cx, ui);
        let new_active = self.tabs.active;
        if old_active != new_active {
            if let Some((_, view)) = removed.iter_mut().find(|(id, _)| *id == old_active) {
                // The old active tab was removed (closed or replaced): its id
                // is gone, so caching an anchor for it would leak forever with
                // no way to restore it. Just deactivate.
                view.on_deactivate(cx, &body);
            } else if let Some(view) = self.views.get_mut(&old_active) {
                let anchor = view.capture_anchor(&body);
                self.anchors.insert(old_active, anchor);
                view.on_deactivate(cx, &body);
            }
            if let Some(view) = self.views.get_mut(&new_active) {
                view.on_activate(cx, &body);
            }
        }
        self.refresh_tabs(cx, ui);
        self.sync_active(cx, ui, session);
        if old_active != new_active {
            if let Some(anchor) = self.anchors.get(&new_active).cloned() {
                let snapshot = session.snapshot();
                if let Some(view) = self.views.get_mut(&new_active) {
                    view.restore_anchor(cx, &body, snapshot.borrowed().into(), &anchor);
                }
            }
        }
    }

    pub fn transition(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        session: &EditorSession,
        command: DocumentCommand,
    ) -> bool {
        let old_active = self.tabs.active;
        let (changed, removed) = self.apply_command(command);
        self.finish_transition(cx, ui, session, old_active, removed);
        changed
    }

    pub fn replace_for_session(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        session: &EditorSession,
        tabs: OpenTabs,
    ) -> bool {
        let old_active = self.tabs.active;
        let changed = self.tabs != tabs;
        let removed = self.replace_tabs_for_session(tabs);
        self.finish_transition(cx, ui, session, old_active, removed);
        changed
    }

    pub fn after_session_change(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        session: &EditorSession,
        change: SessionChange,
        prepared: Vec<Option<OpenDocument>>,
    ) {
        self.after_session_change_with_cause(cx, ui, session, change, prepared, false);
    }

    pub fn after_external_replacement(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        session: &EditorSession,
        change: SessionChange,
        prepared: Vec<Option<OpenDocument>>,
    ) {
        self.after_session_change_with_cause(cx, ui, session, change, prepared, true);
    }

    fn after_session_change_with_cause(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        session: &EditorSession,
        change: SessionChange,
        prepared: Vec<Option<OpenDocument>>,
        external_replacement: bool,
    ) {
        let reconciliation = if change.okf_changed || change.uml_changed {
            self.reconcile_documents(prepared)
        } else {
            ActiveReconciliation::Retained
        };
        let body = BodyWidgets::new(cx, ui);
        match reconciliation {
            ActiveReconciliation::Retained => {
                if !self.tabs.active_tab().is_some_and(|tab| tab.resolved) {
                    // This very event tombstoned the active tab (or there is
                    // no active tab). Syncing the stale view would leave the
                    // canvas of a deleted/renamed document interactive while
                    // the tab draws dimmed -- run the shared resolved-gate in
                    // `sync_active`, which disables interaction instead.
                    self.sync_active(cx, ui, session);
                } else if let Some(view) = self.views.get_mut(&self.tabs.active) {
                    let snapshot = session.snapshot();
                    if external_replacement {
                        view.sync_external_replacement(cx, &body, &snapshot);
                    } else {
                        view.after_session_snapshot(cx, &body, &snapshot, change);
                    }
                }
            }
            ActiveReconciliation::Replaced { mut old_view } => {
                if let Some(old_view) = old_view.as_mut() {
                    old_view.on_deactivate(cx, &body);
                }
                if let Some(view) = self.views.get_mut(&self.tabs.active) {
                    view.on_activate(cx, &body);
                    let snapshot = session.snapshot();
                    if external_replacement {
                        view.sync_external_replacement(cx, &body, &snapshot);
                    } else {
                        view.sync_from_session(cx, &body, &snapshot);
                    }
                }
            }
        }
        self.refresh_tabs(cx, ui);
    }

    fn reconcile_documents(
        &mut self,
        prepared_documents: Vec<Option<OpenDocument>>,
    ) -> ActiveReconciliation {
        let mut reconciliation = ActiveReconciliation::Retained;
        for (index, prepared) in prepared_documents.into_iter().enumerate() {
            if index >= self.tabs.tabs.len() {
                break;
            }
            let current = &self.tabs.tabs[index];
            let current_id = current.id;
            let Some(prepared) = prepared else {
                // The locator no longer resolves (renamed or deleted out
                // from under the tab). Tombstone it instead of silently
                // retaining stale state -- the tab and its view stay around
                // so closing it still works, but `sync_active` disables
                // interaction while it is the active tab.
                self.tabs.tabs[index].resolved = false;
                continue;
            };
            let compatible = prepared.tab_id == current_id
                && self.views.get(&current_id).is_some_and(|current_view| {
                    current_view.reconcile_policy() == ViewReconcilePolicy::RetainLiveState
                        && current_view.identity() == prepared.view.identity()
                });
            if compatible {
                self.tabs.tabs[index].title = prepared.title;
                self.tabs.tabs[index].presentation = prepared.presentation;
                self.tabs.tabs[index].resolved = true;
                continue;
            }
            let preview = current.preview;
            let old_id = current.id;
            let (mut tab, view) = prepared.into_tab(preview);
            tab.preview = preview;
            tab.resolved = true;
            let active_replacement = self.tabs.active == old_id;
            if active_replacement {
                self.tabs.active = tab.id;
            }
            let old_view = self.views.remove(&old_id);
            self.views.insert(tab.id, view);
            self.tabs.tabs[index] = tab;
            if active_replacement {
                reconciliation = ActiveReconciliation::Replaced { old_view };
            }
        }
        reconciliation
    }

    pub fn handle_active(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        actions: &Actions,
        session: &EditorSession,
    ) -> Option<ViewOutcome> {
        let body = BodyWidgets::new(cx, ui);
        let snapshot = session.snapshot();
        self.views
            .get_mut(&self.tabs.active)
            .map(|view| view.handle(cx, &body, actions, snapshot.borrowed().into()))
    }

    pub fn route_ui_event(&mut self, cx: &mut Cx, ui: &WidgetRef, event: &Event) {
        if let Some(view) = self.views.get_mut(&self.tabs.active) {
            view.route_ui_event(cx, ui, event);
        } else {
            ui.handle_event(cx, event, &mut Scope::empty());
        }
    }

    pub fn on_active_escape(&mut self, cx: &mut Cx, ui: &WidgetRef) {
        let body = BodyWidgets::new(cx, ui);
        if let Some(view) = self.views.get_mut(&self.tabs.active) {
            view.on_escape(cx, &body);
        }
    }

    pub fn on_active_popup_result(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        session: &EditorSession,
        tag: LiveId,
        result: PopupResult,
    ) -> Option<ViewOutcome> {
        let body = BodyWidgets::new(cx, ui);
        let snapshot = session.snapshot();
        self.views
            .get_mut(&self.tabs.active)
            .map(|view| view.on_popup_result(cx, &body, snapshot.borrowed().into(), tag, result))
    }

    pub fn on_active_popup_armed(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        session: &EditorSession,
        tag: LiveId,
        id: Option<LiveId>,
    ) -> Option<ViewOutcome> {
        let body = BodyWidgets::new(cx, ui);
        let snapshot = session.snapshot();
        self.views
            .get_mut(&self.tabs.active)
            .map(|view| view.on_popup_armed(cx, &body, snapshot.borrowed().into(), tag, id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc_view::{DocViewIdentity, DocumentHeaderChrome, ViewData, ViewReconcilePolicy};
    use crate::document::{DocumentPresentation, NavCategory, OpenDocument};
    use crate::icons::Icon;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    #[derive(Default)]
    struct ProbeLifecycle {
        sync: usize,
        after_session_change: usize,
        on_activate: usize,
        on_deactivate: usize,
        chrome: usize,
        visible_body: Option<(Rc<Cell<&'static str>>, &'static str)>,
    }

    impl ProbeLifecycle {
        fn mark_visible_body(&self) {
            if let Some((visible_body, marker)) = &self.visible_body {
                visible_body.set(marker);
            }
        }
    }

    struct ProbeView {
        identity: DocViewIdentity,
        reconcile_policy: ViewReconcilePolicy,
        chrome_calls: Rc<Cell<usize>>,
        lifecycle: Rc<RefCell<ProbeLifecycle>>,
    }

    impl DocView for ProbeView {
        fn identity(&self) -> DocViewIdentity {
            self.identity
        }

        fn reconcile_policy(&self) -> ViewReconcilePolicy {
            self.reconcile_policy
        }

        fn sync(&mut self, _: &mut Cx, _: &BodyWidgets, _: ViewData<'_>) {
            let mut lifecycle = self.lifecycle.borrow_mut();
            lifecycle.sync += 1;
            lifecycle.mark_visible_body();
        }

        fn handle(
            &mut self,
            _: &mut Cx,
            _: &BodyWidgets,
            _: &Actions,
            _: ViewData<'_>,
        ) -> ViewOutcome {
            ViewOutcome::default()
        }

        fn after_session_change(
            &mut self,
            _: &mut Cx,
            _: &BodyWidgets,
            _: ViewData<'_>,
            _: SessionChange,
        ) {
            let mut lifecycle = self.lifecycle.borrow_mut();
            lifecycle.after_session_change += 1;
            lifecycle.mark_visible_body();
        }

        fn chrome(&self) -> BodyChrome {
            self.chrome_calls.set(self.chrome_calls.get() + 1);
            self.lifecycle.borrow_mut().chrome += 1;
            BodyChrome {
                tool_dock: true,
                view_bar: false,
                canvas_overlays: false,
                document_header: DocumentHeaderChrome {
                    breadcrumb: false,
                    right_dock: None,
                    view_toggle: None,
                },
            }
        }

        fn on_activate(&mut self, _: &mut Cx, _: &BodyWidgets) {
            let mut lifecycle = self.lifecycle.borrow_mut();
            lifecycle.on_activate += 1;
            lifecycle.mark_visible_body();
        }

        fn on_deactivate(&mut self, _: &mut Cx, _: &BodyWidgets) {
            let mut lifecycle = self.lifecycle.borrow_mut();
            lifecycle.on_deactivate += 1;
            lifecycle.mark_visible_body();
        }
    }

    fn identity_for(category: NavCategory) -> DocViewIdentity {
        match category {
            NavCategory::Diagram => {
                DocViewIdentity::StructuralDiagram(crate::StructuralVisualKind::Class)
            }
            NavCategory::Behavior => DocViewIdentity::Diagram(waml::model::DiagramKind::Activity),
            NavCategory::Sequence => DocViewIdentity::Diagram(waml::model::DiagramKind::Sequence),
            NavCategory::OkfDocument => DocViewIdentity::GenericOkf,
            category => DocViewIdentity::ClassifierPreview(category),
        }
    }

    fn prepared_with_identity(
        key: &str,
        category: NavCategory,
        identity: DocViewIdentity,
        calls: Rc<Cell<usize>>,
        lifecycle: Rc<RefCell<ProbeLifecycle>>,
    ) -> OpenDocument {
        OpenDocument {
            tab_id: LiveId::from_str(&format!("test-{key}")),
            locator: crate::view_history::DocumentLocator::concept(
                key,
                waml::view::surface::SurfaceId::markdown(),
            ),
            title: key.into(),
            presentation: DocumentPresentation {
                icon: Icon::StickyNote,
                accent: None,
                category,
            },
            view: Box::new(ProbeView {
                identity,
                reconcile_policy: ViewReconcilePolicy::Replace,
                chrome_calls: calls,
                lifecycle,
            }),
        }
    }

    fn prepared(key: &str, category: NavCategory, calls: Rc<Cell<usize>>) -> OpenDocument {
        prepared_with_identity(
            key,
            category,
            identity_for(category),
            calls,
            Rc::new(RefCell::new(ProbeLifecycle::default())),
        )
    }

    #[test]
    fn prepared_preview_replacement_drops_the_old_live_view() {
        let mut host = DocumentHost::default();
        let first = prepared("first", NavCategory::Class, Rc::new(Cell::new(0)));
        let first_id = first.tab_id;
        host.apply_command(DocumentCommand::Open {
            document: first,
            persistent: false,
        });
        host.apply_command(DocumentCommand::Open {
            document: prepared("second", NavCategory::OkfDocument, Rc::new(Cell::new(0))),
            persistent: false,
        });
        assert!(!host.views.contains_key(&first_id));
        assert_eq!(host.tabs.tabs.len(), 1);
        assert_eq!(host.tabs.tabs[0].concept_id(), Some("second"));
    }

    /// The capability whose absence made the old per-folder "View raw" inert:
    /// `Open` keeps the existing view whenever the tab id is already open, so
    /// re-navigating a folder tab built the new view and discarded it. A mode
    /// flip must swap the view of a tab that stays put, in place, without
    /// closing and reopening it (which would lose tab order and selection).
    #[test]
    fn reopen_in_place_replaces_the_view_of_an_already_open_tab() {
        let mut host = DocumentHost::default();
        let first = prepared("first", NavCategory::Class, Rc::new(Cell::new(0)));
        let tab_id = first.tab_id;
        host.apply_command(DocumentCommand::Open {
            document: first,
            persistent: false,
        });
        assert_eq!(host.tabs().len(), 1);

        let mut replacement = prepared("first", NavCategory::OkfDocument, Rc::new(Cell::new(0)));
        replacement.tab_id = tab_id;
        host.apply_command(DocumentCommand::ReopenInPlace {
            document: replacement,
        });

        assert_eq!(host.tabs().len(), 1, "no second tab for the same document");
        assert_eq!(host.tabs()[0].id, tab_id, "the tab keeps its identity");
        assert_eq!(
            host.views.get(&tab_id).map(|view| view.identity()),
            Some(DocViewIdentity::GenericOkf),
            "the view was replaced, not discarded",
        );
    }

    /// The search-results tab's `OpenDocument` is a LIVE re-run of the query
    /// (`App::open_search_results` builds it, then installs a session over
    /// exactly those hits). Re-running the same query must therefore land the
    /// freshly built view: activating the open tab and dropping the document
    /// leaves the previous run's rows on screen while the session's flat
    /// index resolves against a different row list.
    #[test]
    fn restore_location_with_document_installs_the_freshly_built_view() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let ui = WidgetRef::default();
        let session = EditorSession::default();
        let mut host = DocumentHost::default();

        let stale = prepared_with_identity(
            "search-order",
            NavCategory::OkfDocument,
            DocViewIdentity::GenericOkf,
            Rc::new(Cell::new(0)),
            Rc::new(RefCell::new(ProbeLifecycle::default())),
        );
        let tab_id = stale.tab_id;
        let location = ViewLocation {
            document: stale.locator.clone(),
            anchor: ViewAnchor::None,
        };
        host.transition(
            &mut cx,
            &ui,
            &session,
            DocumentCommand::Open {
                document: stale,
                persistent: true,
            },
        );

        let fresh = prepared_with_identity(
            "search-order",
            NavCategory::OkfDocument,
            DocViewIdentity::SearchResults,
            Rc::new(Cell::new(0)),
            Rc::new(RefCell::new(ProbeLifecycle::default())),
        );
        assert!(host.restore_location_with_document(&mut cx, &ui, &session, &location, fresh));

        assert_eq!(host.tabs().len(), 1, "no second tab for the same query");
        assert_eq!(host.active_id(), tab_id, "and it is the active one");
        assert_eq!(
            host.views.get(&tab_id).map(|view| view.identity()),
            Some(DocViewIdentity::SearchResults),
            "the freshly built view must replace the previous run's",
        );
    }

    #[test]
    fn reopen_in_place_on_a_closed_tab_opens_nothing() {
        let mut host = DocumentHost::default();
        host.apply_command(DocumentCommand::ReopenInPlace {
            document: prepared("first", NavCategory::Class, Rc::new(Cell::new(0))),
        });
        assert!(host.tabs().is_empty(), "ReopenInPlace never opens a tab");
    }

    #[test]
    fn locator_lookup_distinguishes_primary_and_source_tabs_for_one_concept() {
        let mut host = DocumentHost::default();
        let primary = prepared("order", NavCategory::Class, Rc::new(Cell::new(0)));
        let primary_id = primary.tab_id;
        host.apply_command(DocumentCommand::Open {
            document: primary,
            persistent: true,
        });
        let mut source = prepared("order", NavCategory::OkfDocument, Rc::new(Cell::new(0)));
        source.tab_id =
            crate::documents::tab_id_for(&crate::view_history::DocumentLocator::source("order"));
        source.locator = crate::view_history::DocumentLocator::source("order");
        let source_id = source.tab_id;
        host.apply_command(DocumentCommand::Open {
            document: source,
            persistent: true,
        });

        assert_eq!(
            host.tab_id_for_locator(&DocumentLocator::concept(
                "order",
                waml::view::surface::SurfaceId::markdown()
            )),
            Some(primary_id)
        );
        assert_eq!(
            host.tab_id_for_locator(&DocumentLocator::source("order")),
            Some(source_id)
        );
    }

    #[test]
    fn promotion_request_pins_active_primary_preview_not_earlier_source_tab() {
        let mut host = DocumentHost::default();

        let mut source = prepared("order", NavCategory::OkfDocument, Rc::new(Cell::new(0)));
        source.tab_id =
            crate::documents::tab_id_for(&crate::view_history::DocumentLocator::source("order"));
        source.locator = crate::view_history::DocumentLocator::source("order");
        host.apply_command(DocumentCommand::Open {
            document: source,
            persistent: true,
        });

        let primary = prepared("order", NavCategory::Class, Rc::new(Cell::new(0)));
        let primary_id = primary.tab_id;
        host.apply_command(DocumentCommand::Open {
            document: primary,
            persistent: false,
        });

        assert_eq!(host.active_id(), primary_id);
        assert!(host.active_tab().unwrap().preview);

        let active_id = host.active_id();
        host.apply_command(DocumentCommand::Promote(active_id));

        let primary = host.tabs().iter().find(|tab| tab.id == primary_id).unwrap();
        assert!(!primary.preview);
    }

    #[test]
    fn unresolved_locator_does_not_activate_a_stale_matching_tab() {
        let mut host = DocumentHost::default();
        for key in ["stale", "current"] {
            host.apply_command(DocumentCommand::Open {
                document: prepared(key, NavCategory::Class, Rc::new(Cell::new(0))),
                persistent: true,
            });
        }
        let current_id = host.active_id();
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let ui = WidgetRef::empty();
        let session = EditorSession::default();

        assert!(!host.restore_location(
            &mut cx,
            &ui,
            &session,
            &ViewLocation {
                document: DocumentLocator::concept(
                    "stale",
                    waml::view::surface::SurfaceId::markdown()
                ),
                anchor: ViewAnchor::None,
            },
        ));
        assert_eq!(host.active_id(), current_id);
        assert_eq!(host.active_tab().unwrap().concept_id(), Some("current"));
    }

    #[test]
    fn supplied_view_drives_active_chrome_without_a_host_factory() {
        let calls = Rc::new(Cell::new(0));
        let mut host = DocumentHost::default();
        host.apply_command(DocumentCommand::Open {
            document: prepared("order", NavCategory::Class, calls.clone()),
            persistent: true,
        });
        assert!(host.active_chrome().tool_dock);
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn statically_prepared_sibling_needs_no_host_family_dispatch() {
        let calls = Rc::new(Cell::new(0));
        let mut sibling = prepared("future-widget", NavCategory::OkfDocument, calls.clone());
        sibling.tab_id = LiveId::from_str("future-sibling-provider");
        let mut host = DocumentHost::default();

        host.apply_command(DocumentCommand::Open {
            document: sibling,
            persistent: true,
        });

        assert_eq!(
            host.active_id(),
            LiveId::from_str("future-sibling-provider")
        );
        assert_eq!(
            host.active_tab().unwrap().concept_id(),
            Some("future-widget")
        );
        assert!(host.active_chrome().tool_dock);
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn active_accent_comes_from_provider_presentation() {
        let mut host = DocumentHost::default();
        let accent = vec4(0.2, 0.4, 0.6, 1.0);
        let mut document = prepared("order", NavCategory::Class, Rc::new(Cell::new(0)));
        document.presentation.accent = Some(accent);
        host.apply_command(DocumentCommand::Open {
            document,
            persistent: false,
        });

        assert_eq!(host.active_accent(), Some(accent));
    }

    #[test]
    fn reconciliation_consumes_prepared_documents_without_resolving_providers() {
        let mut host = DocumentHost::default();
        let original = prepared("order", NavCategory::Class, Rc::new(Cell::new(0)));
        host.apply_command(DocumentCommand::Open {
            document: original,
            persistent: false,
        });

        let mut renamed = prepared("order", NavCategory::Class, Rc::new(Cell::new(0)));
        renamed.title = "Purchase Order".into();
        host.reconcile_documents(vec![Some(renamed)]);
        assert_eq!(host.active_tab().unwrap().title, "Purchase Order");

        let mut changed_provider =
            prepared("order", NavCategory::OkfDocument, Rc::new(Cell::new(0)));
        changed_provider.tab_id = LiveId::from_str("replacement-provider");
        host.reconcile_documents(vec![Some(changed_provider)]);
        assert_eq!(host.active_id(), LiveId::from_str("replacement-provider"));
        assert_eq!(host.views.len(), 1);
    }

    #[test]
    fn revision_bound_view_replaces_even_when_identity_is_compatible() {
        let old_calls = Rc::new(Cell::new(0));
        let new_calls = Rc::new(Cell::new(0));
        let old_lifecycle = Rc::new(RefCell::new(ProbeLifecycle::default()));
        let new_lifecycle = Rc::new(RefCell::new(ProbeLifecycle::default()));
        let mut host = DocumentHost::default();
        host.apply_command(DocumentCommand::Open {
            document: prepared_with_identity(
                "order",
                NavCategory::Class,
                DocViewIdentity::ClassifierPreview(NavCategory::Class),
                old_calls.clone(),
                old_lifecycle.clone(),
            ),
            persistent: true,
        });
        let mut replacement = prepared_with_identity(
            "order",
            NavCategory::Class,
            DocViewIdentity::ClassifierPreview(NavCategory::Class),
            new_calls.clone(),
            new_lifecycle.clone(),
        );
        replacement.title = "Purchase Order".into();
        replacement.presentation.icon = Icon::Package;
        let mut cx = Cx::new(Box::new(|_, _| {}));
        host.after_session_change(
            &mut cx,
            &WidgetRef::empty(),
            &EditorSession::default(),
            SessionChange {
                revision: 1,
                source_changed: false,
                okf_changed: false,
                uml_changed: true,
                navigation_changed: false,
                conflicts_changed: false,
                affected_documents: std::sync::Arc::from([]),
                affected_diagrams: std::sync::Arc::from([]),
            },
            vec![Some(replacement)],
        );

        assert_eq!(host.active_tab().unwrap().title, "Purchase Order");
        assert_eq!(host.active_tab().unwrap().presentation.icon, Icon::Package);
        assert_eq!(old_lifecycle.borrow().after_session_change, 0);
        assert_eq!(old_lifecycle.borrow().sync, 0);
        assert_eq!(new_lifecycle.borrow().after_session_change, 0);
        assert_eq!(old_lifecycle.borrow().on_deactivate, 1);
        assert_eq!(new_lifecycle.borrow().on_activate, 1);
        assert_eq!(new_lifecycle.borrow().sync, 1);
        assert_eq!(new_calls.get(), 0);
        assert_eq!(old_calls.get(), 0);
        assert!(!host.active_tab().unwrap().preview);
    }

    #[test]
    fn retaining_view_keeps_its_allocation_when_a_compatible_document_is_reprepared() {
        let old_lifecycle = Rc::new(RefCell::new(ProbeLifecycle::default()));
        let replacement_lifecycle = Rc::new(RefCell::new(ProbeLifecycle::default()));
        let mut current = prepared_with_identity(
            "order",
            NavCategory::OkfDocument,
            DocViewIdentity::Source,
            Rc::new(Cell::new(0)),
            old_lifecycle.clone(),
        );
        current.view = Box::new(ProbeView {
            identity: DocViewIdentity::Source,
            reconcile_policy: ViewReconcilePolicy::RetainLiveState,
            chrome_calls: Rc::new(Cell::new(0)),
            lifecycle: old_lifecycle.clone(),
        });
        let current_id = current.tab_id;
        let mut host = DocumentHost::default();
        host.apply_command(DocumentCommand::Open {
            document: current,
            persistent: true,
        });

        let replacement = prepared_with_identity(
            "order",
            NavCategory::OkfDocument,
            DocViewIdentity::Source,
            Rc::new(Cell::new(0)),
            replacement_lifecycle.clone(),
        );
        host.reconcile_documents(vec![Some(replacement)]);

        assert_eq!(host.active_id(), current_id);
        assert!(host.views.contains_key(&current_id));
        assert_eq!(old_lifecycle.borrow().on_deactivate, 0);
        assert_eq!(replacement_lifecycle.borrow().on_activate, 0);
    }

    #[test]
    fn stale_use_case_reconciliation_keeps_the_structural_view_allocation() {
        let identity = DocViewIdentity::StructuralDiagram(crate::StructuralVisualKind::UseCase);
        let old_lifecycle = Rc::new(RefCell::new(ProbeLifecycle::default()));
        let replacement_lifecycle = Rc::new(RefCell::new(ProbeLifecycle::default()));
        let mut current = prepared_with_identity(
            "use-cases",
            NavCategory::Diagram,
            identity,
            Rc::new(Cell::new(0)),
            old_lifecycle.clone(),
        );
        current.view = Box::new(ProbeView {
            identity,
            reconcile_policy: ViewReconcilePolicy::RetainLiveState,
            chrome_calls: Rc::new(Cell::new(0)),
            lifecycle: old_lifecycle.clone(),
        });
        let current_id = current.tab_id;
        let mut host = DocumentHost::default();
        host.apply_command(DocumentCommand::Open {
            document: current,
            persistent: true,
        });

        let mut replacement = prepared_with_identity(
            "use-cases",
            NavCategory::Diagram,
            identity,
            Rc::new(Cell::new(0)),
            replacement_lifecycle.clone(),
        );
        replacement.tab_id = current_id;
        host.reconcile_documents(vec![Some(replacement)]);

        assert_eq!(host.active_id(), current_id);
        assert_eq!(
            host.views.get(&current_id).map(|view| view.identity()),
            Some(identity)
        );
        assert_eq!(old_lifecycle.borrow().on_deactivate, 0);
        assert_eq!(replacement_lifecycle.borrow().on_activate, 0);
    }

    #[test]
    fn retained_diagram_view_is_replaced_when_declared_identity_changes() {
        let old_lifecycle = Rc::new(RefCell::new(ProbeLifecycle::default()));
        let replacement_lifecycle = Rc::new(RefCell::new(ProbeLifecycle::default()));
        let mut current = prepared_with_identity(
            "diagram",
            NavCategory::Diagram,
            DocViewIdentity::StructuralDiagram(crate::StructuralVisualKind::Class),
            Rc::new(Cell::new(0)),
            old_lifecycle.clone(),
        );
        current.view = Box::new(ProbeView {
            identity: DocViewIdentity::StructuralDiagram(crate::StructuralVisualKind::Class),
            reconcile_policy: ViewReconcilePolicy::RetainLiveState,
            chrome_calls: Rc::new(Cell::new(0)),
            lifecycle: old_lifecycle,
        });
        let current_id = current.tab_id;
        let mut host = DocumentHost::default();
        host.apply_command(DocumentCommand::Open {
            document: current,
            persistent: true,
        });

        let mut replacement = prepared_with_identity(
            "diagram",
            NavCategory::Diagram,
            DocViewIdentity::StructuralDiagram(crate::StructuralVisualKind::UseCase),
            Rc::new(Cell::new(0)),
            replacement_lifecycle.clone(),
        );
        replacement.tab_id = current_id;
        replacement.view = Box::new(ProbeView {
            identity: DocViewIdentity::StructuralDiagram(crate::StructuralVisualKind::UseCase),
            reconcile_policy: ViewReconcilePolicy::RetainLiveState,
            chrome_calls: Rc::new(Cell::new(0)),
            lifecycle: replacement_lifecycle,
        });
        host.reconcile_documents(vec![Some(replacement)]);

        assert_eq!(
            host.views.get(&current_id).map(|view| view.identity()),
            Some(DocViewIdentity::StructuralDiagram(
                crate::StructuralVisualKind::UseCase,
            ))
        );
    }

    #[test]
    fn incompatible_active_replacement_runs_full_lifecycle() {
        let old_calls = Rc::new(Cell::new(0));
        let new_calls = Rc::new(Cell::new(0));
        let old_lifecycle = Rc::new(RefCell::new(ProbeLifecycle::default()));
        let new_lifecycle = Rc::new(RefCell::new(ProbeLifecycle::default()));
        let mut host = DocumentHost::default();
        host.apply_command(DocumentCommand::Open {
            document: prepared_with_identity(
                "order",
                NavCategory::Class,
                DocViewIdentity::ClassifierPreview(NavCategory::Class),
                old_calls,
                old_lifecycle.clone(),
            ),
            persistent: true,
        });
        let mut replacement = prepared_with_identity(
            "order",
            NavCategory::OkfDocument,
            DocViewIdentity::GenericOkf,
            new_calls,
            new_lifecycle.clone(),
        );
        replacement.tab_id = host.active_id();
        let mut cx = Cx::new(Box::new(|_, _| {}));
        host.after_session_change(
            &mut cx,
            &WidgetRef::empty(),
            &EditorSession::default(),
            SessionChange {
                revision: 1,
                source_changed: false,
                okf_changed: false,
                uml_changed: true,
                navigation_changed: false,
                conflicts_changed: false,
                affected_documents: std::sync::Arc::from([]),
                affected_diagrams: std::sync::Arc::from([]),
            },
            vec![Some(replacement)],
        );

        assert_eq!(old_lifecycle.borrow().on_deactivate, 1);
        assert_eq!(new_lifecycle.borrow().on_activate, 1);
        assert_eq!(new_lifecycle.borrow().sync, 1);
        assert_eq!(new_lifecycle.borrow().after_session_change, 0);
    }

    #[test]
    fn incompatible_inactive_replacement_keeps_active_surface_untouched() {
        let visible_body = Rc::new(Cell::new("active"));
        let old_lifecycle = Rc::new(RefCell::new(ProbeLifecycle {
            visible_body: Some((visible_body.clone(), "inactive-old")),
            ..ProbeLifecycle::default()
        }));
        let new_lifecycle = Rc::new(RefCell::new(ProbeLifecycle {
            visible_body: Some((visible_body.clone(), "inactive-new")),
            ..ProbeLifecycle::default()
        }));
        let mut host = DocumentHost::default();
        let inactive = prepared_with_identity(
            "inactive",
            NavCategory::Class,
            DocViewIdentity::ClassifierPreview(NavCategory::Class),
            Rc::new(Cell::new(0)),
            old_lifecycle.clone(),
        );
        let inactive_id = inactive.tab_id;
        host.apply_command(DocumentCommand::Open {
            document: inactive,
            persistent: true,
        });
        host.apply_command(DocumentCommand::Open {
            document: prepared("active", NavCategory::Diagram, Rc::new(Cell::new(0))),
            persistent: true,
        });
        let active_id = host.active_id();
        let mut replacement = prepared_with_identity(
            "inactive",
            NavCategory::OkfDocument,
            DocViewIdentity::GenericOkf,
            Rc::new(Cell::new(0)),
            new_lifecycle.clone(),
        );
        replacement.tab_id = inactive_id;

        let mut cx = Cx::new(Box::new(|_, _| {}));
        host.after_session_change(
            &mut cx,
            &WidgetRef::empty(),
            &EditorSession::default(),
            SessionChange {
                revision: 1,
                source_changed: false,
                okf_changed: false,
                uml_changed: true,
                navigation_changed: false,
                conflicts_changed: false,
                affected_documents: std::sync::Arc::from([]),
                affected_diagrams: std::sync::Arc::from([]),
            },
            vec![Some(replacement), None],
        );

        assert_eq!(host.active_id(), active_id);
        assert_eq!(visible_body.get(), "active");
        assert_eq!(
            host.views.get(&inactive_id).unwrap().identity(),
            DocViewIdentity::GenericOkf,
        );
        let old_lifecycle = old_lifecycle.borrow();
        assert_eq!(old_lifecycle.on_deactivate, 0);
        assert_eq!(old_lifecycle.on_activate, 0);
        assert_eq!(old_lifecycle.sync, 0);
        assert_eq!(old_lifecycle.after_session_change, 0);
        let new_lifecycle = new_lifecycle.borrow();
        assert_eq!(new_lifecycle.on_deactivate, 0);
        assert_eq!(new_lifecycle.on_activate, 0);
        assert_eq!(new_lifecycle.sync, 0);
        assert_eq!(new_lifecycle.after_session_change, 0);
    }

    #[test]
    fn promoted_tabs_keep_right_then_left_close_fallback() {
        let mut host = DocumentHost::default();
        for key in ["a", "b", "c"] {
            host.apply_command(DocumentCommand::Open {
                document: prepared(key, NavCategory::Class, Rc::new(Cell::new(0))),
                persistent: true,
            });
        }
        let b = host.tabs.tabs[1].id;
        let c = host.tabs.tabs[2].id;
        host.apply_command(DocumentCommand::Activate(b));
        host.apply_command(DocumentCommand::Close(b));
        assert_eq!(host.tabs.active, c);
    }

    #[test]
    fn unresolved_locator_tombstones_the_tab_and_disables_interaction_when_active() {
        let mut host = DocumentHost::default();
        let first = prepared("first", NavCategory::Class, Rc::new(Cell::new(0)));
        let first_id = first.tab_id;
        host.apply_command(DocumentCommand::Open {
            document: first,
            persistent: true,
        });
        let second = prepared("second", NavCategory::Class, Rc::new(Cell::new(0)));
        let second_id = second.tab_id;
        host.apply_command(DocumentCommand::Open {
            document: second,
            persistent: true,
        });
        assert_eq!(host.active_id(), second_id);

        // Second document's locator no longer resolves (renamed/deleted).
        host.reconcile_documents(vec![
            Some(prepared("first", NavCategory::Class, Rc::new(Cell::new(0)))),
            None,
        ]);

        let second_tab = host.tabs().iter().find(|tab| tab.id == second_id).unwrap();
        assert!(!second_tab.resolved, "tombstoned tab must be retained");
        // The tab is still there and still the active one.
        assert_eq!(host.active_id(), second_id);
        assert!(host.views.contains_key(&second_id), "view is retained too");

        let mut cx = Cx::new(Box::new(|_, _| {}));
        let ui = WidgetRef::empty();
        let session = EditorSession::default();
        host.sync_active(&mut cx, &ui, &session);
        // No assertion surface for canvas interaction without a live widget
        // tree; the branch is exercised (no panic) and covered structurally
        // by `sync_active`'s resolved-gate above.

        // Undo: the locator resolves again -- revival happens through the
        // ordinary prepared-document flow.
        host.reconcile_documents(vec![
            Some(prepared("first", NavCategory::Class, Rc::new(Cell::new(0)))),
            Some(prepared(
                "second",
                NavCategory::Class,
                Rc::new(Cell::new(0)),
            )),
        ]);
        let second_tab = host.tabs().iter().find(|tab| tab.id == second_id).unwrap();
        assert!(second_tab.resolved, "tab must revive once resolved again");
        let _ = first_id;
    }

    #[test]
    fn tombstoning_the_active_tab_skips_the_stale_view_sync_on_the_same_event() {
        let lifecycle = Rc::new(RefCell::new(ProbeLifecycle::default()));
        let mut host = DocumentHost::default();
        host.apply_command(DocumentCommand::Open {
            document: prepared_with_identity(
                "order",
                NavCategory::Class,
                DocViewIdentity::ClassifierPreview(NavCategory::Class),
                Rc::new(Cell::new(0)),
                lifecycle.clone(),
            ),
            persistent: true,
        });

        let mut cx = Cx::new(Box::new(|_, _| {}));
        // The session change that tombstones the ACTIVE tab: its locator no
        // longer resolves, so `prepared` carries `None` for it.
        host.after_session_change(
            &mut cx,
            &WidgetRef::empty(),
            &EditorSession::default(),
            SessionChange {
                revision: 1,
                source_changed: false,
                okf_changed: false,
                uml_changed: true,
                navigation_changed: false,
                conflicts_changed: false,
                affected_documents: std::sync::Arc::from([]),
                affected_diagrams: std::sync::Arc::from([]),
            },
            vec![None],
        );

        assert!(
            !host.active_tab().unwrap().resolved,
            "active tab must be tombstoned"
        );
        // The stale view must NOT be synced on the tombstoning event: syncing
        // re-enables canvas interaction, so the deleted/renamed document's
        // canvas would stay interactive while the tab draws dimmed. The
        // shared resolved-gate in `sync_active` must run instead.
        assert_eq!(lifecycle.borrow().after_session_change, 0);
        assert_eq!(lifecycle.borrow().sync, 0);

        // Same invariant on the external-replacement path.
        host.after_external_replacement(
            &mut cx,
            &WidgetRef::empty(),
            &EditorSession::default(),
            SessionChange {
                revision: 2,
                source_changed: false,
                okf_changed: false,
                uml_changed: true,
                navigation_changed: false,
                conflicts_changed: false,
                affected_documents: std::sync::Arc::from([]),
                affected_diagrams: std::sync::Arc::from([]),
            },
            vec![None],
        );
        assert_eq!(lifecycle.borrow().after_session_change, 0);
        assert_eq!(lifecycle.borrow().sync, 0);
    }

    #[test]
    fn closing_a_tombstoned_tab_removes_it_and_its_view() {
        let mut host = DocumentHost::default();
        let first = prepared("first", NavCategory::Class, Rc::new(Cell::new(0)));
        host.apply_command(DocumentCommand::Open {
            document: first,
            persistent: true,
        });
        let second = prepared("second", NavCategory::Class, Rc::new(Cell::new(0)));
        let second_id = second.tab_id;
        host.apply_command(DocumentCommand::Open {
            document: second,
            persistent: true,
        });

        host.reconcile_documents(vec![
            Some(prepared("first", NavCategory::Class, Rc::new(Cell::new(0)))),
            None,
        ]);
        assert!(
            !host
                .tabs()
                .iter()
                .find(|tab| tab.id == second_id)
                .unwrap()
                .resolved
        );

        host.apply_command(DocumentCommand::Close(second_id));

        assert!(host.tabs().iter().all(|tab| tab.id != second_id));
        assert!(!host.views.contains_key(&second_id));
    }

    struct AnchorProbeView {
        state: Rc<RefCell<ViewAnchor>>,
    }

    impl DocView for AnchorProbeView {
        fn identity(&self) -> DocViewIdentity {
            DocViewIdentity::GenericOkf
        }

        fn sync(&mut self, _: &mut Cx, _: &BodyWidgets, _: ViewData<'_>) {
            // Mirrors ClassDiagramView::sync clearing selection/camera on
            // every sync -- the cached-anchor restore must run after this.
            *self.state.borrow_mut() = ViewAnchor::None;
        }

        fn handle(
            &mut self,
            _: &mut Cx,
            _: &BodyWidgets,
            _: &Actions,
            _: ViewData<'_>,
        ) -> ViewOutcome {
            ViewOutcome::default()
        }

        fn chrome(&self) -> BodyChrome {
            BodyChrome::HIDDEN
        }

        fn capture_anchor(&self, _: &BodyWidgets) -> ViewAnchor {
            self.state.borrow().clone()
        }

        fn restore_anchor(
            &mut self,
            _: &mut Cx,
            _: &BodyWidgets,
            _: ViewData<'_>,
            anchor: &ViewAnchor,
        ) -> bool {
            *self.state.borrow_mut() = anchor.clone();
            true
        }
    }

    fn anchor_document(key: &str, state: Rc<RefCell<ViewAnchor>>) -> OpenDocument {
        OpenDocument {
            tab_id: LiveId::from_str(&format!("anchor-{key}")),
            locator: crate::view_history::DocumentLocator::concept(
                key,
                waml::view::surface::SurfaceId::markdown(),
            ),
            title: key.into(),
            presentation: DocumentPresentation {
                icon: Icon::StickyNote,
                accent: None,
                category: NavCategory::Class,
            },
            view: Box::new(AnchorProbeView { state }),
        }
    }

    fn sample_anchor(tag: &str) -> ViewAnchor {
        ViewAnchor::Diagram {
            selected_key: Some(tag.into()),
            camera: crate::view_history::DiagramCameraAnchor {
                pan_x: 1.0,
                pan_y: 2.0,
                zoom: 3.0,
            },
        }
    }

    fn anchor_test_env() -> (Cx, WidgetRef, EditorSession) {
        (
            Cx::new(Box::new(|_, _| {})),
            WidgetRef::empty(),
            EditorSession::default(),
        )
    }

    #[test]
    fn switching_away_and_back_restores_the_departing_tabs_last_anchor() {
        let mut host = DocumentHost::default();
        let (mut cx, ui, session) = anchor_test_env();

        let state_a = Rc::new(RefCell::new(ViewAnchor::None));
        host.transition(
            &mut cx,
            &ui,
            &session,
            DocumentCommand::Open {
                document: anchor_document("a", state_a.clone()),
                persistent: true,
            },
        );
        let a_id = host.active_id();
        let anchor_a = sample_anchor("a-selection");
        *state_a.borrow_mut() = anchor_a.clone();

        let state_b = Rc::new(RefCell::new(ViewAnchor::None));
        host.transition(
            &mut cx,
            &ui,
            &session,
            DocumentCommand::Open {
                document: anchor_document("b", state_b.clone()),
                persistent: true,
            },
        );
        assert_ne!(host.active_id(), a_id);

        host.transition(&mut cx, &ui, &session, DocumentCommand::Activate(a_id));

        assert_eq!(host.active_id(), a_id);
        assert_eq!(*state_a.borrow(), anchor_a);
    }

    #[test]
    fn an_explicit_incoming_anchor_wins_over_the_cached_one() {
        let mut host = DocumentHost::default();
        let (mut cx, ui, session) = anchor_test_env();

        let state_a = Rc::new(RefCell::new(ViewAnchor::None));
        host.transition(
            &mut cx,
            &ui,
            &session,
            DocumentCommand::Open {
                document: anchor_document("a", state_a.clone()),
                persistent: true,
            },
        );
        let a_id = host.active_id();
        let cached_anchor = sample_anchor("cached");
        *state_a.borrow_mut() = cached_anchor.clone();

        let state_b = Rc::new(RefCell::new(ViewAnchor::None));
        host.transition(
            &mut cx,
            &ui,
            &session,
            DocumentCommand::Open {
                document: anchor_document("b", state_b),
                persistent: true,
            },
        );

        // Activate A the same way a location restore does, then apply an
        // explicit anchor afterward -- it must win over the cached one.
        host.transition(&mut cx, &ui, &session, DocumentCommand::Activate(a_id));
        assert_eq!(*state_a.borrow(), cached_anchor);
        let explicit_anchor = sample_anchor("explicit");
        assert!(host.restore_active_anchor(&mut cx, &ui, &session, &explicit_anchor));

        assert_eq!(*state_a.borrow(), explicit_anchor);
    }

    #[test]
    fn closing_a_tab_evicts_its_cached_anchor() {
        let mut host = DocumentHost::default();
        let (mut cx, ui, session) = anchor_test_env();

        let state_a = Rc::new(RefCell::new(ViewAnchor::None));
        host.transition(
            &mut cx,
            &ui,
            &session,
            DocumentCommand::Open {
                document: anchor_document("a", state_a.clone()),
                persistent: true,
            },
        );
        let a_id = host.active_id();
        *state_a.borrow_mut() = sample_anchor("stale");

        let state_b = Rc::new(RefCell::new(ViewAnchor::None));
        host.transition(
            &mut cx,
            &ui,
            &session,
            DocumentCommand::Open {
                document: anchor_document("b", state_b),
                persistent: true,
            },
        );
        assert!(host.anchors.contains_key(&a_id));

        host.transition(&mut cx, &ui, &session, DocumentCommand::Close(a_id));
        assert!(!host.anchors.contains_key(&a_id));

        // Reopening "a" under a fresh tab id must not see the stale anchor.
        let state_a2 = Rc::new(RefCell::new(ViewAnchor::None));
        host.transition(
            &mut cx,
            &ui,
            &session,
            DocumentCommand::Open {
                document: anchor_document("a", state_a2.clone()),
                persistent: true,
            },
        );
        assert_eq!(*state_a2.borrow(), ViewAnchor::None);
    }

    #[test]
    fn closing_the_active_tab_caches_no_anchor_for_the_dead_id() {
        let mut host = DocumentHost::default();
        let (mut cx, ui, session) = anchor_test_env();

        let state_a = Rc::new(RefCell::new(ViewAnchor::None));
        host.transition(
            &mut cx,
            &ui,
            &session,
            DocumentCommand::Open {
                document: anchor_document("a", state_a.clone()),
                persistent: true,
            },
        );

        let state_b = Rc::new(RefCell::new(ViewAnchor::None));
        host.transition(
            &mut cx,
            &ui,
            &session,
            DocumentCommand::Open {
                document: anchor_document("b", state_b.clone()),
                persistent: true,
            },
        );
        let b_id = host.active_id();
        *state_b.borrow_mut() = sample_anchor("dying");

        // Closing the ACTIVE tab removes its view; the dead tab id must not
        // leave an anchor behind (it is unrestorable and would leak forever).
        host.transition(&mut cx, &ui, &session, DocumentCommand::Close(b_id));
        assert_ne!(host.active_id(), b_id);
        assert!(
            !host.anchors.contains_key(&b_id),
            "closing the active tab must not cache an anchor for its dead id"
        );
    }
}
