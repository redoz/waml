use crate::doc_tabs::{DocTab, DocTabs, OpenTabs, TabKind};
use crate::doc_view::{BodyChrome, BodyWidgets, DocView, ViewData, ViewOutcome};
use crate::editor_session::{EditorSession, SessionChange};
use crate::popup::base::PopupResult;
use crate::tree::TreeKind;
use makepad_widgets::*;
use std::collections::{HashMap, HashSet};

pub enum DocumentCommand {
    Open {
        key: String,
        title: String,
        node_kind: TreeKind,
        persistent: bool,
    },
    OpenSource {
        key: String,
        title: String,
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

fn make_view(tab: &DocTab) -> Box<dyn DocView> {
    match tab.kind {
        TabKind::Diagram => Box::new(crate::class_diagram_view::ClassDiagramView::new(
            tab.key.clone(),
            tab.title.clone(),
        )),
        TabKind::Classifier => {
            Box::new(crate::classifier_preview_view::ClassifierPreviewView::new(
                tab.key.clone(),
                tab.node_kind,
            ))
        }
        TabKind::Source => Box::new(crate::source_view::SourceView::new(
            tab.key.clone(),
            tab.node_kind,
        )),
    }
}

fn data(session: &EditorSession) -> ViewData<'_> {
    ViewData {
        model: session.model(),
        bundle: session.bundle(),
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
        let removed = stale
            .into_iter()
            .filter_map(|id| self.views.remove(&id).map(|view| (id, view)))
            .collect();
        for tab in &self.tabs.tabs {
            self.views.entry(tab.id).or_insert_with(|| make_view(tab));
        }
        removed
    }

    fn apply_command(&mut self, command: DocumentCommand) -> (bool, RemovedViews) {
        let before = self.tabs.clone();
        match command {
            DocumentCommand::Open {
                key,
                title,
                node_kind,
                persistent,
            } => {
                let id = self.tabs.open_preview(key, title, node_kind);
                if persistent {
                    self.tabs.promote(id);
                }
            }
            DocumentCommand::OpenSource { key, title } => {
                self.tabs.open_source(key, title);
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
                    .find(|tab| tab.key == key)
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
        self.views
            .get(&self.tabs.active)
            .and_then(|view| view.tab_accent())
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
    ) {
        if change.model_changed {
            self.tabs.reconcile_titles(session.model());
        }
        let body = BodyWidgets::new(cx, ui);
        if let Some(view) = self.views.get_mut(&self.tabs.active) {
            view.after_session_change(cx, &body, data(session), change);
        }
        self.refresh_tabs(cx, ui);
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
    use std::cell::Cell;
    use std::rc::Rc;

    struct ProbeView {
        chrome_calls: Rc<Cell<usize>>,
        accent_calls: Rc<Cell<usize>>,
    }

    impl DocView for ProbeView {
        fn sync(&mut self, _: &mut Cx, _: &BodyWidgets, _: ViewData<'_>) {
            unreachable!()
        }

        fn handle(
            &mut self,
            _: &mut Cx,
            _: &BodyWidgets,
            _: &Actions,
            _: ViewData<'_>,
        ) -> ViewOutcome {
            unreachable!()
        }

        fn chrome(&self) -> BodyChrome {
            self.chrome_calls.set(self.chrome_calls.get() + 1);
            BodyChrome {
                tool_dock: true,
                view_bar: false,
                canvas_overlays: false,
                right_dock: None,
            }
        }

        fn tab_accent(&self) -> Option<Vec4> {
            self.accent_calls.set(self.accent_calls.get() + 1);
            Some(vec4(0.1, 0.2, 0.3, 1.0))
        }
    }

    #[test]
    fn preview_replacement_drops_the_replaced_live_view() {
        let mut host = DocumentHost {
            tabs: OpenTabs::diagram_preview("orders", "Orders"),
            ..DocumentHost::default()
        };
        let replaced = host.tabs.active;
        host.views.insert(
            replaced,
            Box::new(ProbeView {
                chrome_calls: Rc::new(Cell::new(0)),
                accent_calls: Rc::new(Cell::new(0)),
            }),
        );

        assert!(
            host.apply_command(DocumentCommand::Open {
                key: "customer".into(),
                title: "Customer".into(),
                node_kind: TreeKind::Class,
                persistent: false,
            })
            .0
        );

        assert!(!host.views.contains_key(&replaced));
        assert!(host.views.contains_key(&host.tabs.active));
    }

    #[test]
    fn close_reconciles_and_keeps_the_existing_right_then_left_fallback() {
        let mut host = DocumentHost {
            tabs: OpenTabs::diagram_preview("orders", "Orders"),
            ..DocumentHost::default()
        };
        let orders = host.tabs.active;
        host.tabs.promote(orders);
        let customer = host
            .tabs
            .open_preview("customer", "Customer", TreeKind::Class);
        host.tabs.promote(customer);
        let source = host.tabs.open_source("order", "Order");
        host.reconcile_registry();

        assert!(host.apply_command(DocumentCommand::Close(customer)).0);
        assert_eq!(host.active_id(), source);
        assert!(!host.views.contains_key(&customer));

        assert!(host.apply_command(DocumentCommand::Close(source)).0);
        assert_eq!(host.active_id(), orders);
    }

    #[test]
    fn chrome_is_queried_from_the_registered_live_view() {
        let calls = Rc::new(Cell::new(0));
        let mut host = DocumentHost {
            tabs: OpenTabs::diagram_preview("orders", "Orders"),
            ..DocumentHost::default()
        };
        host.views.insert(
            host.tabs.active,
            Box::new(ProbeView {
                chrome_calls: calls.clone(),
                accent_calls: calls.clone(),
            }),
        );

        assert_eq!(
            host.active_chrome(),
            BodyChrome {
                tool_dock: true,
                view_bar: false,
                canvas_overlays: false,
                right_dock: None,
            }
        );
        assert_eq!(calls.get(), 1);
        assert_eq!(host.active_accent(), Some(vec4(0.1, 0.2, 0.3, 1.0)));
        assert_eq!(calls.get(), 2);
        assert_eq!(host.views.len(), 1);
    }

    #[test]
    fn replacing_a_session_drops_views_even_when_tab_ids_repeat() {
        let mut host = DocumentHost {
            tabs: OpenTabs::diagram_preview("orders", "Old Orders"),
            ..DocumentHost::default()
        };
        let repeated = host.tabs.active;
        host.views.insert(
            repeated,
            Box::new(ProbeView {
                chrome_calls: Rc::new(Cell::new(0)),
                accent_calls: Rc::new(Cell::new(0)),
            }),
        );

        let removed =
            host.replace_tabs_for_session(OpenTabs::diagram_preview("orders", "New Orders"));

        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].0, repeated);
        assert_eq!(
            host.active_tab().map(|tab| tab.title.as_str()),
            Some("New Orders")
        );
        assert!(host.views.contains_key(&repeated));
    }
}
