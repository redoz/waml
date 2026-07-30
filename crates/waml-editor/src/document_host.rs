use crate::doc_tabs::{DocTab, DocTabs, OpenTabs};
use crate::doc_view::{BodyChrome, BodyWidgets, DocView, ViewData, ViewOutcome};
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
    PromoteSubject(String),
    Close(LiveId),
}

#[derive(Default)]
pub struct DocumentHost {
    tabs: OpenTabs,
    views: HashMap<LiveId, Box<dyn DocView>>,
}

type RemovedViews = Vec<(LiveId, Box<dyn DocView>)>;

enum ActiveReconciliation {
    Retained,
    Replaced { old_view: Option<Box<dyn DocView>> },
}

fn data(session: &EditorSession) -> ViewData<'_> {
    session.snapshot().into()
}

impl DocumentHost {
    fn replace_tabs_for_session(&mut self, tabs: OpenTabs) -> RemovedViews {
        let removed = self.views.drain().collect();
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
            DocumentCommand::PromoteSubject(key) => {
                if let Some(id) = self
                    .tabs
                    .tabs
                    .iter()
                    .find(|tab| tab.concept_id == key)
                    .map(|tab| tab.id)
                {
                    self.tabs.promote(id);
                }
            }
            DocumentCommand::Close(id) => self.tabs.close(id),
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
        if !body.scroll_markdown_to_fragment(cx, fragment) {
            return false;
        }
        let anchor = ViewAnchor::Markdown {
            fragment: Some(fragment.to_owned()),
            scroll_y: body.markdown_scroll_y(),
        };
        if let Some(view) = self.views.get_mut(&self.tabs.active) {
            let _ = view.restore_anchor(cx, &body, data(session), &anchor);
        }
        true
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
        self.views
            .get_mut(&self.tabs.active)
            .is_some_and(|view| view.restore_anchor(cx, &body, data(session), anchor))
    }

    pub fn restore_location(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        session: &EditorSession,
        location: &ViewLocation,
    ) -> bool {
        let Some(document) = crate::documents::open_locator(
            session.okf_analysis(),
            session.uml_analysis(),
            &location.document,
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
        if let Some(view) = self.views.get_mut(&active) {
            view.sync(cx, &body, data(session));
        } else {
            // No active view at all: neither canvas may take input.
            body.set_canvas_interaction_enabled(cx, false);
            body.set_behavior_canvas_interaction_enabled(cx, false);
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
                view.on_deactivate(cx, &body);
            } else if let Some(view) = self.views.get_mut(&old_active) {
                view.on_deactivate(cx, &body);
            }
            if let Some(view) = self.views.get_mut(&new_active) {
                view.on_activate(cx, &body);
            }
        }
        self.refresh_tabs(cx, ui);
        self.sync_active(cx, ui, session);
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
        prepared: Vec<Option<crate::document::OpenDocument>>,
    ) {
        let reconciliation = if change.okf_changed || change.uml_changed {
            self.reconcile_documents(prepared)
        } else {
            ActiveReconciliation::Retained
        };
        let body = BodyWidgets::new(cx, ui);
        match reconciliation {
            ActiveReconciliation::Retained => {
                if let Some(view) = self.views.get_mut(&self.tabs.active) {
                    view.after_session_change(cx, &body, data(session), change);
                }
            }
            ActiveReconciliation::Replaced { mut old_view } => {
                if let Some(old_view) = old_view.as_mut() {
                    old_view.on_deactivate(cx, &body);
                }
                if let Some(view) = self.views.get_mut(&self.tabs.active) {
                    view.on_activate(cx, &body);
                    view.sync(cx, &body, data(session));
                }
            }
        }
        self.refresh_tabs(cx, ui);
    }

    fn reconcile_documents(
        &mut self,
        prepared_documents: Vec<Option<crate::document::OpenDocument>>,
    ) -> ActiveReconciliation {
        let mut reconciliation = ActiveReconciliation::Retained;
        for (index, prepared) in prepared_documents.into_iter().enumerate() {
            if index >= self.tabs.tabs.len() {
                break;
            }
            let current = &self.tabs.tabs[index];
            let current_id = current.id;
            let Some(prepared) = prepared else {
                continue;
            };
            let compatible = prepared.tab_id == current_id
                && self.views.get(&current_id).is_some_and(|current_view| {
                    current_view.identity() == prepared.view.identity()
                });
            if compatible {
                self.tabs.tabs[index].title = prepared.title;
                self.tabs.tabs[index].presentation = prepared.presentation;
                continue;
            }
            let preview = current.preview;
            let old_id = current.id;
            let (mut tab, view) = prepared.into_tab(preview);
            tab.preview = preview;
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
        self.views
            .get_mut(&self.tabs.active)
            .map(|view| view.handle(cx, &body, actions, data(session)))
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
        self.views
            .get_mut(&self.tabs.active)
            .map(|view| view.on_popup_result(cx, &body, data(session), tag, result))
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
        self.views
            .get_mut(&self.tabs.active)
            .map(|view| view.on_popup_armed(cx, &body, data(session), tag, id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc_view::{DocViewIdentity, DocumentHeaderChrome};
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
    }

    struct ProbeView {
        identity: DocViewIdentity,
        chrome_calls: Rc<Cell<usize>>,
        lifecycle: Rc<RefCell<ProbeLifecycle>>,
    }

    impl DocView for ProbeView {
        fn identity(&self) -> DocViewIdentity {
            self.identity
        }

        fn sync(&mut self, _: &mut Cx, _: &BodyWidgets, _: ViewData<'_>) {
            self.lifecycle.borrow_mut().sync += 1;
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
            self.lifecycle.borrow_mut().after_session_change += 1;
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
                },
            }
        }

        fn on_activate(&mut self, _: &mut Cx, _: &BodyWidgets) {
            self.lifecycle.borrow_mut().on_activate += 1;
        }

        fn on_deactivate(&mut self, _: &mut Cx, _: &BodyWidgets) {
            self.lifecycle.borrow_mut().on_deactivate += 1;
        }
    }

    fn identity_for(category: NavCategory) -> DocViewIdentity {
        match category {
            NavCategory::Diagram => DocViewIdentity::ClassDiagram,
            NavCategory::Behavior => DocViewIdentity::BehaviorFlow,
            NavCategory::Sequence => DocViewIdentity::BehaviorInteraction,
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
            concept_id: key.into(),
            kind: crate::view_history::DocumentKind::Primary,
            title: key.into(),
            presentation: DocumentPresentation {
                icon: Icon::StickyNote,
                accent: None,
                category,
            },
            view: Box::new(ProbeView {
                identity,
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
        assert_eq!(host.tabs.tabs[0].concept_id, "second");
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
        source.tab_id = crate::okf_documents::source_document_tab_id("order");
        source.kind = crate::view_history::DocumentKind::Source;
        let source_id = source.tab_id;
        host.apply_command(DocumentCommand::Open {
            document: source,
            persistent: true,
        });

        assert_eq!(
            host.tab_id_for_locator(&DocumentLocator::primary("order")),
            Some(primary_id)
        );
        assert_eq!(
            host.tab_id_for_locator(&DocumentLocator::source("order")),
            Some(source_id)
        );
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
                document: DocumentLocator::primary("stale"),
                anchor: ViewAnchor::None,
            },
        ));
        assert_eq!(host.active_id(), current_id);
        assert_eq!(host.active_tab().unwrap().concept_id, "current");
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
        assert_eq!(host.active_tab().unwrap().concept_id, "future-widget");
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
    fn compatible_prepared_document_keeps_the_live_view() {
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
            },
            vec![Some(replacement)],
        );

        assert_eq!(host.active_tab().unwrap().title, "Purchase Order");
        assert_eq!(host.active_tab().unwrap().presentation.icon, Icon::Package);
        assert_eq!(old_lifecycle.borrow().after_session_change, 1);
        assert_eq!(old_lifecycle.borrow().sync, 0);
        assert_eq!(new_lifecycle.borrow().after_session_change, 0);
        assert_eq!(new_lifecycle.borrow().sync, 0);
        assert_eq!(new_calls.get(), 0);
        assert_eq!(old_calls.get(), 0);
        assert!(!host.active_tab().unwrap().preview);
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
            },
            vec![Some(replacement)],
        );

        assert_eq!(old_lifecycle.borrow().on_deactivate, 1);
        assert_eq!(new_lifecycle.borrow().on_activate, 1);
        assert_eq!(new_lifecycle.borrow().sync, 1);
        assert_eq!(new_lifecycle.borrow().after_session_change, 0);
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
}
