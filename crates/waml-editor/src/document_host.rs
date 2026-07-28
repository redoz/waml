use crate::doc_tabs::{DocTab, DocTabs, OpenTabs};
use crate::doc_view::{BodyChrome, BodyWidgets, DocView, ViewData, ViewOutcome};
use crate::document::OpenDocument;
use crate::editor_session::{EditorSession, SessionChange};
use crate::popup::base::PopupResult;
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

fn data(session: &EditorSession) -> ViewData<'_> {
    ViewData {
        source: session.source(),
        okf: session.okf(),
        uml: session.uml_projection(),
        revision: session.revision(),
    }
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
        self.tabs
            .active_tab()
            .and_then(|tab| tab.presentation.accent)
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
            body.set_canvas_interaction_enabled(cx, false);
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
        if change.okf_changed || change.uml_changed {
            self.reconcile_documents(prepared);
        }
        let body = BodyWidgets::new(cx, ui);
        if let Some(view) = self.views.get_mut(&self.tabs.active) {
            view.after_session_change(cx, &body, data(session), change);
        }
        self.refresh_tabs(cx, ui);
    }

    fn reconcile_documents(
        &mut self,
        prepared_documents: Vec<Option<crate::document::OpenDocument>>,
    ) {
        for (index, prepared) in prepared_documents.into_iter().enumerate() {
            if index >= self.tabs.tabs.len() {
                break;
            }
            let current = &self.tabs.tabs[index];
            let Some(prepared) = prepared else {
                continue;
            };
            if prepared.tab_id == current.id {
                self.tabs.tabs[index].title = prepared.title;
                self.tabs.tabs[index].presentation = prepared.presentation;
                continue;
            }
            let preview = current.preview;
            let old_id = current.id;
            let (mut tab, view) = prepared.into_tab(preview);
            tab.preview = preview;
            if self.tabs.active == old_id {
                self.tabs.active = tab.id;
            }
            self.views.remove(&old_id);
            self.views.insert(tab.id, view);
            self.tabs.tabs[index] = tab;
        }
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
    use crate::document::{DocumentPresentation, NavCategory, OpenDocument};
    use crate::icons::Icon;
    use std::cell::Cell;
    use std::rc::Rc;

    struct ProbeView(Rc<Cell<usize>>);

    impl DocView for ProbeView {
        fn sync(&mut self, _: &mut Cx, _: &BodyWidgets, _: ViewData<'_>) {}
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
            self.0.set(self.0.get() + 1);
            BodyChrome {
                tool_dock: true,
                view_bar: false,
                canvas_overlays: false,
                right_dock: None,
            }
        }
    }

    fn prepared(key: &str, category: NavCategory, calls: Rc<Cell<usize>>) -> OpenDocument {
        OpenDocument {
            tab_id: LiveId::from_str(&format!("test-{key}")),
            concept_id: key.into(),
            title: key.into(),
            presentation: DocumentPresentation {
                icon: Icon::StickyNote,
                accent: None,
                category,
            },
            view: Box::new(ProbeView(calls)),
        }
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
