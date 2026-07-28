use super::*;
use crate::popup::root::RadialOpen;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ActionFlow {
    Continue,
    Consumed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ObserverHandler {
    CaptionAndDocks,
    PopupResults,
    ConflictList,
}

const OBSERVER_ORDER: [ObserverHandler; 3] = [
    ObserverHandler::CaptionAndDocks,
    ObserverHandler::PopupResults,
    ObserverHandler::ConflictList,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PopupRelay {
    Armed,
    Closed,
}

const DOCUMENT_POPUP_RELAY_ORDER: [PopupRelay; 2] = [PopupRelay::Armed, PopupRelay::Closed];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExclusiveHandler {
    NavigationScope,
    NavigationQuery,
    NavigationFilter,
    TreeContextMenu,
    TreeNavigation,
    DiagramSwitcher,
    ConflictBadge,
    ActiveDocumentView,
    LogoMenu,
    StartScreen,
    ShortcutsOverlay,
    FontsOverlay,
    IconsOverlay,
    ColorsOverlay,
    DocumentTabs,
}

const EXCLUSIVE_ORDER: [ExclusiveHandler; 15] = [
    ExclusiveHandler::NavigationScope,
    ExclusiveHandler::NavigationQuery,
    ExclusiveHandler::NavigationFilter,
    ExclusiveHandler::TreeContextMenu,
    ExclusiveHandler::TreeNavigation,
    ExclusiveHandler::DiagramSwitcher,
    ExclusiveHandler::ConflictBadge,
    ExclusiveHandler::ActiveDocumentView,
    ExclusiveHandler::LogoMenu,
    ExclusiveHandler::StartScreen,
    ExclusiveHandler::ShortcutsOverlay,
    ExclusiveHandler::FontsOverlay,
    ExclusiveHandler::IconsOverlay,
    ExclusiveHandler::ColorsOverlay,
    ExclusiveHandler::DocumentTabs,
];

impl App {
    pub(super) fn handle_action_batch(&mut self, cx: &mut Cx, actions: &Actions) {
        for observer in OBSERVER_ORDER {
            match observer {
                ObserverHandler::CaptionAndDocks => self.observe_caption_and_docks(cx, actions),
                ObserverHandler::PopupResults => self.observe_popup_results(cx, actions),
                ObserverHandler::ConflictList => self.observe_conflict_list(cx, actions),
            }
        }

        for handler in EXCLUSIVE_ORDER {
            let flow = match handler {
                ExclusiveHandler::NavigationScope => self.handle_navigation_scope(cx, actions),
                ExclusiveHandler::NavigationQuery => self.handle_navigation_query(cx, actions),
                ExclusiveHandler::NavigationFilter => self.handle_navigation_filter(cx, actions),
                ExclusiveHandler::TreeContextMenu => self.handle_tree_context_menu(cx, actions),
                ExclusiveHandler::TreeNavigation => self.handle_tree_navigation(cx, actions),
                ExclusiveHandler::DiagramSwitcher => self.handle_diagram_switcher(cx, actions),
                ExclusiveHandler::ConflictBadge => self.handle_conflict_badge(cx, actions),
                ExclusiveHandler::ActiveDocumentView => {
                    self.handle_active_document_view(cx, actions)
                }
                ExclusiveHandler::LogoMenu => self.handle_logo_menu(cx, actions),
                ExclusiveHandler::StartScreen => self.handle_start_screen_action(cx, actions),
                ExclusiveHandler::ShortcutsOverlay => self.handle_shortcuts_overlay(cx, actions),
                ExclusiveHandler::FontsOverlay => self.handle_fonts_overlay(cx, actions),
                ExclusiveHandler::IconsOverlay => self.handle_icons_overlay(cx, actions),
                ExclusiveHandler::ColorsOverlay => self.handle_colors_overlay(cx, actions),
                ExclusiveHandler::DocumentTabs => self.handle_document_tabs(cx, actions),
            };
            if flow == ActionFlow::Consumed {
                return;
            }
        }
    }

    fn observe_caption_and_docks(&mut self, cx: &mut Cx, actions: &Actions) {
        if let Some(press) = self
            .ui
            .widget(cx, ids!(menu_btn))
            .as_icon_button()
            .pressed(actions)
        {
            let button = self.ui.widget(cx, ids!(menu_btn)).as_icon_button().rect(cx);
            let anchor = dvec2(
                button.pos.x + crate::popup::menu::MENU_INDENT_X,
                button.pos.y + button.size.y + crate::popup::menu::MENU_GAP,
            );
            let bounds = self.window_bounds(cx);
            if let Some(mut popup) = self
                .ui
                .widget(cx, ids!(popup_root))
                .borrow_mut::<PopupRoot>()
            {
                popup.show_at(
                    cx,
                    PopupSpec::Menu {
                        tag: live_id!(burger),
                        anchor,
                        bounds,
                        items: burger_menu_items(),
                        open: MenuOpen::Press(press),
                    },
                );
            }
            self.ui
                .widget(cx, ids!(menu_btn))
                .as_icon_button()
                .set_active(cx, true);
        }

        if self
            .ui
            .widget(cx, ids!(tree_btn))
            .as_icon_button()
            .clicked(actions)
        {
            if self.narrow {
                let (tree, inspector) = self.dock_states(cx);
                let (tree, inspector) =
                    crate::dock::narrow_toggle_states(tree, inspector, crate::dock::DockEdge::Left);
                self.apply_dock_states(cx, tree, inspector);
            } else if let Some(mut panel) = self
                .ui
                .widget(cx, ids!(project_tree))
                .borrow_mut::<crate::tree_panel::ProjectTree>()
            {
                panel.toggle_dock(cx);
            }
        }

        if self
            .ui
            .widget(cx, ids!(inspector_btn))
            .as_icon_button()
            .clicked(actions)
        {
            if self.narrow {
                let (tree, inspector) = self.dock_states(cx);
                let (tree, inspector) = crate::dock::narrow_toggle_states(
                    tree,
                    inspector,
                    crate::dock::DockEdge::Right,
                );
                self.apply_dock_states(cx, tree, inspector);
            } else if let Some(mut panel) = self
                .ui
                .widget(cx, ids!(inspector))
                .borrow_mut::<crate::inspector_panel::Inspector>()
            {
                panel.toggle_dock(cx);
            }
        }
    }

    fn observe_popup_results(&mut self, cx: &mut Cx, actions: &Actions) {
        let popup_root = self.ui.widget(cx, ids!(popup_root));
        let Some(popup) = popup_root.borrow::<PopupRoot>() else {
            return;
        };
        let mut document_closed = popup.closed_event(actions);
        let result_for = |wanted: LiveId| {
            document_closed
                .as_ref()
                .and_then(|(tag, result)| (*tag == wanted).then(|| result.clone()))
        };
        let logo_closed = result_for(live_id!(logo));
        let burger_closed = result_for(live_id!(burger));
        let doc_switcher_closed = result_for(live_id!(doc_switcher));
        let node_closed = result_for(live_id!(node_menu));
        let nav_scope_closed = result_for(live_id!(nav_scope));
        let nav_filter_closed = result_for(live_id!(nav_filter));
        let mut document_armed = popup.armed_event(actions);
        drop(popup);

        if burger_closed.is_some() {
            self.ui
                .widget(cx, ids!(menu_btn))
                .as_icon_button()
                .set_active(cx, false);
        }
        if let Some(PopupResult::Invoked(id)) = doc_switcher_closed {
            self.documents
                .transition(cx, &self.ui, &self.session, DocumentCommand::Activate(id));
            self.sync_document_shell(cx);
        }
        if let Some(PopupResult::Invoked(id)) = burger_closed {
            if id == live_id!(new_model) {
                log!("New model: not yet implemented (template picker is a later slice)");
            } else if id == live_id!(open_model) {
                self.open_model_via_picker(cx);
            } else if id == live_id!(close_model) {
                self.close_model(cx);
            }
        }
        if let Some(PopupResult::Invoked(id)) = logo_closed {
            if let Some(command) = logo_command_for(id) {
                match command {
                    LogoCommand::Properties => log!("logo command: Properties (stub)"),
                    LogoCommand::About => {
                        cx.open_url("https://github.com/redoz/waml", OpenUrlInPlace::No)
                    }
                    LogoCommand::Fonts => self.open_page_overlay(cx, LogoCommand::Fonts),
                    LogoCommand::Icons => self.open_page_overlay(cx, LogoCommand::Icons),
                    LogoCommand::Colors => self.open_page_overlay(cx, LogoCommand::Colors),
                    LogoCommand::Exit => {
                        cx.request_quit(QuitReason::Menu);
                    }
                }
            }
        }
        if let Some(PopupResult::Invoked(id)) = node_closed {
            if let Some(command) = crate::popup::node_menu::command_for(id) {
                let key = self.node_menu_key.clone().unwrap_or_default();
                match command {
                    crate::popup::node_menu::NodeMenuCommand::ViewSource => {
                        if let Some(document) =
                            crate::okf_documents::open_source(self.session.okf(), &key)
                        {
                            self.documents.transition(
                                cx,
                                &self.ui,
                                &self.session,
                                DocumentCommand::Open {
                                    document,
                                    persistent: false,
                                },
                            );
                            self.sync_document_shell(cx);
                        }
                    }
                    crate::popup::node_menu::NodeMenuCommand::FindInDiagrams => {
                        log!("find in diagrams: {key}");
                    }
                }
            }
        }
        if let Some(PopupResult::Invoked(id)) = nav_scope_closed {
            if let Some((_, key)) = self.nav_scope_ids.iter().find(|(item, _)| *item == id) {
                self.nav_state.scope = key.clone();
                self.refresh_nav(cx, true);
            }
        }
        if let Some(PopupResult::Invoked(id)) = nav_filter_closed {
            if let Some((_, filter)) = self.nav_filter_ids.iter().find(|(item, _)| *item == id) {
                self.nav_state.filter = *filter;
                self.refresh_nav(cx, false);
            }
        }

        for relay in DOCUMENT_POPUP_RELAY_ORDER {
            let outcome = match relay {
                PopupRelay::Armed => document_armed.take().and_then(|(tag, id)| {
                    self.documents
                        .on_active_popup_armed(cx, &self.ui, &self.session, tag, id)
                }),
                PopupRelay::Closed => document_closed.take().and_then(|(tag, result)| {
                    self.documents
                        .on_active_popup_result(cx, &self.ui, &self.session, tag, result)
                }),
            };
            if let Some(outcome) = outcome {
                let _ = self.apply_view_outcome(cx, outcome);
            }
        }
    }

    fn observe_conflict_list(&mut self, cx: &mut Cx, actions: &Actions) {
        let conflict_action = self
            .ui
            .widget(cx, ids!(popup_root))
            .borrow::<PopupRoot>()
            .and_then(|popup| popup.conflict_action(cx, actions));

        match conflict_action {
            Some(crate::popup::conflict_list::ConflictListAction::Focus { subject, reference }) => {
                if let Some(mut canvas) = self
                    .ui
                    .widget(cx, ids!(canvas))
                    .borrow_mut::<crate::canvas::ClassDiagramSurface>()
                {
                    canvas.set_conflict_focus_keys(cx, Some(vec![subject, reference]));
                }
            }
            Some(action @ crate::popup::conflict_list::ConflictListAction::Delete { .. }) => {
                let diagram = self
                    .documents
                    .active_tab()
                    .map(|tab| tab.concept_id.clone())
                    .unwrap_or_default();
                if let Some(op) = place_rm_for(&diagram, &action) {
                    if self
                        .apply_session_edit(
                            cx,
                            waml::edit::PendingEdit::new(waml::uml::Batch(vec![op])),
                            "place.rm failed",
                        )
                        .is_some()
                    {
                        let conflicts = self
                            .ui
                            .widget(cx, ids!(canvas))
                            .borrow::<crate::canvas::ClassDiagramSurface>()
                            .map(|canvas| canvas.conflicts())
                            .unwrap_or_default();
                        if conflicts.is_empty() {
                            if let Some(mut popup) = self
                                .ui
                                .widget(cx, ids!(popup_root))
                                .borrow_mut::<PopupRoot>()
                            {
                                popup.close(cx);
                            }
                        } else {
                            self.open_conflict_list(cx, conflicts);
                        }
                    }
                }
            }
            Some(crate::popup::conflict_list::ConflictListAction::None) | None => {}
        }
    }

    fn handle_navigation_scope(&mut self, cx: &mut Cx, actions: &Actions) -> ActionFlow {
        let request = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
            .and_then(|panel| panel.scope_request(actions));
        let Some(anchor_rect) = request else {
            return ActionFlow::Continue;
        };

        self.nav_scope_ids.clear();
        let items = crate::nav::packages(self.session.okf(), self.session.uml_projection())
            .into_iter()
            .map(|row| {
                let id = LiveId::from_str(&format!("scope:{}", row.key));
                self.nav_scope_ids.push((id, row.key.clone()));
                crate::popup::base::PopupItem {
                    id,
                    label: format!("{}{}", "  ".repeat(row.depth), row.title),
                    icon: Some(crate::icons::Icon::Folder),
                    danger: false,
                    enabled: true,
                }
            })
            .collect();
        let anchor = dvec2(
            anchor_rect.pos.x,
            anchor_rect.pos.y + anchor_rect.size.y + crate::popup::menu::MENU_GAP,
        );
        let bounds = self.window_bounds(cx);
        if let Some(mut popup) = self
            .ui
            .widget(cx, ids!(popup_root))
            .borrow_mut::<PopupRoot>()
        {
            popup.show_at(
                cx,
                PopupSpec::Menu {
                    tag: live_id!(nav_scope),
                    anchor,
                    bounds,
                    items,
                    open: MenuOpen::Popup {
                        open_marking: None,
                        max_height: None,
                    },
                },
            );
        }
        ActionFlow::Consumed
    }

    fn handle_navigation_query(&mut self, cx: &mut Cx, actions: &Actions) -> ActionFlow {
        let query = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
            .and_then(|panel| panel.query_changed(actions));
        let Some(query) = query else {
            return ActionFlow::Continue;
        };
        self.nav_state.query = query;
        self.refresh_nav(cx, false);
        ActionFlow::Consumed
    }

    fn handle_navigation_filter(&mut self, cx: &mut Cx, actions: &Actions) -> ActionFlow {
        let request = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
            .and_then(|panel| panel.filter_request(actions));
        let Some(anchor_rect) = request else {
            return ActionFlow::Continue;
        };

        self.nav_filter_ids.clear();
        let mut items = Vec::new();
        for filter in std::iter::once(None).chain(self.nav_kinds.iter().copied().map(Some)) {
            let id = match filter {
                None => live_id!(filter_all),
                Some(kind) => LiveId::from_str(&format!("filter:{kind:?}")),
            };
            self.nav_filter_ids.push((id, filter));
            let lead = match filter {
                None => SelectLead::Icon(crate::icons::Icon::Funnel),
                Some(kind) => crate::icons::IconSet::icon_for(kind)
                    .map(SelectLead::Icon)
                    .unwrap_or(SelectLead::None),
            };
            items.push(SelectItem {
                id,
                lead,
                label: crate::nav::chip_label(filter).to_string(),
                selected: filter == self.nav_state.filter,
                enabled: true,
            });
        }
        let anchor = dvec2(
            anchor_rect.pos.x,
            anchor_rect.pos.y + anchor_rect.size.y + crate::popup::select::SELECT_GAP,
        );
        let bounds = self.window_bounds(cx);
        if let Some(mut popup) = self
            .ui
            .widget(cx, ids!(popup_root))
            .borrow_mut::<PopupRoot>()
        {
            popup.show_at(
                cx,
                PopupSpec::Select {
                    tag: live_id!(nav_filter),
                    anchor,
                    min_width: anchor_rect.size.x,
                    bounds,
                    items,
                    compact_frame: false,
                },
            );
        }
        ActionFlow::Consumed
    }

    fn handle_tree_context_menu(&mut self, cx: &mut Cx, actions: &Actions) -> ActionFlow {
        let request = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
            .and_then(|panel| panel.context_menu_request(actions));
        let Some((key, anchor)) = request else {
            return ActionFlow::Continue;
        };

        self.transition_document(cx, &key, false);
        self.node_menu_key = Some(key);
        let bounds = self.window_bounds(cx);
        if let Some(mut popup) = self
            .ui
            .widget(cx, ids!(popup_root))
            .borrow_mut::<PopupRoot>()
        {
            popup.show_at(
                cx,
                PopupSpec::Menu {
                    tag: live_id!(node_menu),
                    anchor,
                    bounds,
                    items: crate::popup::node_menu::compose(
                        vec![],
                        crate::popup::node_menu::base_items(),
                    ),
                    open: MenuOpen::Popup {
                        open_marking: None,
                        max_height: None,
                    },
                },
            );
        }
        ActionFlow::Consumed
    }

    fn handle_tree_navigation(&mut self, cx: &mut Cx, actions: &Actions) -> ActionFlow {
        let intent = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
            .and_then(|panel| panel.navigation(actions));
        let Some(intent) = intent else {
            return ActionFlow::Continue;
        };
        match intent {
            crate::navigation::NavigationIntent::Resolved {
                target:
                    crate::navigation::NavigationTarget::Document {
                        concept_id,
                        fragment: _,
                    },
                disposition,
            } => {
                self.transition_document(
                    cx,
                    &concept_id,
                    disposition == crate::navigation::OpenDisposition::Persistent,
                );
                ActionFlow::Consumed
            }
            crate::navigation::NavigationIntent::Resolved {
                target: crate::navigation::NavigationTarget::Directory { address },
                ..
            } => {
                let toggled = self
                    .ui
                    .widget(cx, ids!(project_tree))
                    .borrow_mut::<crate::tree_panel::ProjectTree>()
                    .is_some_and(|mut panel| panel.toggle_directory(cx, &address));
                if toggled {
                    ActionFlow::Consumed
                } else {
                    ActionFlow::Continue
                }
            }
            crate::navigation::NavigationIntent::Resolved {
                target: crate::navigation::NavigationTarget::ExternalUrl(_),
                ..
            }
            | crate::navigation::NavigationIntent::MarkdownLink { .. } => ActionFlow::Continue,
        }
    }

    fn handle_diagram_switcher(&mut self, cx: &mut Cx, actions: &Actions) -> ActionFlow {
        let clicked = self
            .ui
            .widget(cx, ids!(diagram_switcher))
            .borrow_mut::<crate::diagram_switcher::DiagramSwitcher>()
            .and_then(|switcher| switcher.switcher_action(actions));
        if !matches!(
            clicked,
            Some(crate::diagram_switcher::DiagramSwitcherAction::Clicked)
        ) {
            return ActionFlow::Continue;
        }

        let keys = self
            .session
            .model()
            .diagrams
            .iter()
            .map(|diagram| diagram.key.clone())
            .collect::<Vec<_>>();
        let current = self
            .documents
            .active_tab()
            .filter(|tab| tab.presentation.category == NavCategory::Diagram)
            .or_else(|| {
                self.documents
                    .tabs()
                    .iter()
                    .find(|tab| tab.presentation.category == NavCategory::Diagram)
            })
            .map(|tab| tab.concept_id.clone())
            .unwrap_or_default();
        if let Some(next) = crate::diagram_switcher::next_diagram_key(&keys, &current) {
            self.transition_document(cx, &next, false);
        }
        ActionFlow::Consumed
    }

    fn handle_conflict_badge(&mut self, cx: &mut Cx, actions: &Actions) -> ActionFlow {
        let clicked = self
            .ui
            .widget(cx, ids!(conflict_badge))
            .borrow::<crate::conflict_badge::ConflictBadge>()
            .is_some_and(|badge| badge.clicked(actions));
        if !clicked {
            return ActionFlow::Continue;
        }
        let conflicts = self
            .ui
            .widget(cx, ids!(canvas))
            .borrow::<crate::canvas::ClassDiagramSurface>()
            .map(|canvas| canvas.conflicts())
            .unwrap_or_default();
        if !conflicts.is_empty() {
            self.open_conflict_list(cx, conflicts);
        }
        ActionFlow::Consumed
    }

    fn handle_active_document_view(&mut self, cx: &mut Cx, actions: &Actions) -> ActionFlow {
        let Some(outcome) = self
            .documents
            .handle_active(cx, &self.ui, actions, &self.session)
        else {
            return ActionFlow::Continue;
        };
        self.apply_view_outcome(cx, outcome)
    }

    fn handle_logo_menu(&mut self, cx: &mut Cx, actions: &Actions) -> ActionFlow {
        let logo = self
            .ui
            .widget(cx, ids!(logo))
            .borrow::<crate::logo::LogoMark>()
            .and_then(|logo| logo.logo_action(actions).map(|_| logo.drawn_rect()));
        let Some(logo_rect) = logo else {
            return ActionFlow::Continue;
        };

        let anchor = dvec2(
            logo_rect.pos.x,
            logo_rect.pos.y + logo_rect.size.y + crate::popup::menu::MENU_GAP,
        );
        let bounds = self.window_bounds(cx);
        if let Some(mut popup) = self
            .ui
            .widget(cx, ids!(popup_root))
            .borrow_mut::<PopupRoot>()
        {
            popup.show_at(
                cx,
                PopupSpec::Menu {
                    tag: live_id!(logo),
                    anchor,
                    bounds,
                    items: logo_menu_items(),
                    open: MenuOpen::Popup {
                        open_marking: None,
                        max_height: None,
                    },
                },
            );
        }
        ActionFlow::Consumed
    }

    fn handle_start_screen_action(&mut self, cx: &mut Cx, actions: &Actions) -> ActionFlow {
        let action = self
            .ui
            .widget(cx, ids!(start_screen))
            .borrow_mut::<crate::start_screen::StartScreen>()
            .and_then(|screen| screen.screen_action(actions));
        let Some(action) = action else {
            return ActionFlow::Continue;
        };

        match action {
            crate::start_screen::StartScreenAction::OpenRecent(index) => {
                if let Some(recent) = self.start_recents.get(index).cloned() {
                    if self.open_dir(cx, recent.path(), None) {
                        self.show_editor(cx);
                    }
                }
            }
            crate::start_screen::StartScreenAction::TogglePin(index) => {
                if let Some(recent) = self.start_recents.get(index).cloned() {
                    crate::config::set_pinned(recent.path(), !recent.pinned());
                    self.show_start_screen(cx);
                }
            }
            crate::start_screen::StartScreenAction::NewProject => {
                log!("New project: not yet implemented (template picker is a later slice)");
            }
            crate::start_screen::StartScreenAction::OpenProject => {
                self.open_model_via_picker(cx);
            }
            crate::start_screen::StartScreenAction::None => {}
        }
        ActionFlow::Consumed
    }

    fn handle_shortcuts_overlay(&mut self, cx: &mut Cx, actions: &Actions) -> ActionFlow {
        let dismissed = self
            .ui
            .widget(cx, ids!(shortcuts_overlay))
            .borrow_mut::<crate::shortcuts_overlay::ShortcutsOverlay>()
            .and_then(|overlay| overlay.overlay_action(actions));
        if matches!(
            dismissed,
            Some(crate::shortcuts_overlay::ShortcutsOverlayAction::Dismissed)
        ) {
            self.toggle_shortcuts_overlay(cx);
            ActionFlow::Consumed
        } else {
            ActionFlow::Continue
        }
    }

    fn handle_fonts_overlay(&mut self, cx: &mut Cx, actions: &Actions) -> ActionFlow {
        let dismissed = self
            .ui
            .widget(cx, ids!(fonts_overlay))
            .borrow_mut::<crate::fonts_overlay::FontsOverlay>()
            .and_then(|overlay| overlay.overlay_action(actions));
        if matches!(
            dismissed,
            Some(crate::fonts_overlay::FontsOverlayAction::Dismissed)
        ) {
            self.close_page_overlays(cx);
            ActionFlow::Consumed
        } else {
            ActionFlow::Continue
        }
    }

    fn handle_icons_overlay(&mut self, cx: &mut Cx, actions: &Actions) -> ActionFlow {
        let dismissed = self
            .ui
            .widget(cx, ids!(icons_overlay))
            .borrow_mut::<crate::icons_overlay::IconsOverlay>()
            .and_then(|overlay| overlay.overlay_action(actions));
        if matches!(
            dismissed,
            Some(crate::icons_overlay::IconsOverlayAction::Dismissed)
        ) {
            self.close_page_overlays(cx);
            ActionFlow::Consumed
        } else {
            ActionFlow::Continue
        }
    }

    fn handle_colors_overlay(&mut self, cx: &mut Cx, actions: &Actions) -> ActionFlow {
        let dismissed = self
            .ui
            .widget(cx, ids!(colors_overlay))
            .borrow_mut::<crate::colors_overlay::ColorsOverlay>()
            .and_then(|overlay| overlay.overlay_action(actions));
        if matches!(
            dismissed,
            Some(crate::colors_overlay::ColorsOverlayAction::Dismissed)
        ) {
            self.close_page_overlays(cx);
            ActionFlow::Consumed
        } else {
            ActionFlow::Continue
        }
    }

    fn handle_document_tabs(&mut self, cx: &mut Cx, actions: &Actions) -> ActionFlow {
        let action = self
            .ui
            .widget(cx, ids!(doc_tabs))
            .borrow_mut::<crate::doc_tabs::DocTabs>()
            .and_then(|tabs| tabs.tab_action(actions));
        let Some(action) = action else {
            return ActionFlow::Continue;
        };

        match action {
            crate::doc_tabs::DocTabsAction::OpenSwitcher { anchor } => {
                if self.documents.active_tab().is_some() {
                    let items = doc_switcher_items(self.documents.tabs());
                    let bounds = self.window_bounds(cx);
                    if let Some(mut popup) = self
                        .ui
                        .widget(cx, ids!(popup_root))
                        .borrow_mut::<PopupRoot>()
                    {
                        popup.show_at(
                            cx,
                            PopupSpec::Menu {
                                tag: live_id!(doc_switcher),
                                anchor,
                                bounds,
                                items,
                                open: MenuOpen::Popup {
                                    open_marking: Some(self.documents.active_id()),
                                    max_height: Some(DOC_SWITCHER_MAX_H),
                                },
                            },
                        );
                    }
                }
            }
            crate::doc_tabs::DocTabsAction::Activate(id) => {
                self.documents.transition(
                    cx,
                    &self.ui,
                    &self.session,
                    DocumentCommand::Activate(id),
                );
                self.sync_document_shell(cx);
            }
            crate::doc_tabs::DocTabsAction::Promote(id) => {
                self.documents.transition(
                    cx,
                    &self.ui,
                    &self.session,
                    DocumentCommand::Promote(id),
                );
                self.sync_document_shell(cx);
            }
            crate::doc_tabs::DocTabsAction::Close(id) => {
                self.documents
                    .transition(cx, &self.ui, &self.session, DocumentCommand::Close(id));
                self.sync_document_shell(cx);
            }
            crate::doc_tabs::DocTabsAction::None => {}
        }
        ActionFlow::Consumed
    }

    pub(super) fn apply_session_edit(
        &mut self,
        cx: &mut Cx,
        edit: waml::edit::PendingEdit,
        error_label: &str,
    ) -> Option<crate::editor_session::SessionChange> {
        match self.session.apply(edit) {
            Ok(change) => {
                let prepared = if change.okf_changed || change.uml_changed {
                    self.documents
                        .tabs()
                        .iter()
                        .map(|tab| {
                            crate::documents::reopen(
                                self.session.okf(),
                                self.session.uml_projection(),
                                tab,
                            )
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                self.documents
                    .after_session_change(cx, &self.ui, &self.session, change, prepared);
                if change.uml_changed {
                    self.sync_document_shell(cx);
                }
                if change.navigation_changed {
                    self.nav_kinds = crate::nav::kinds_in_model(
                        self.session.okf(),
                        self.session.uml_projection(),
                    );
                    self.refresh_nav(cx, false);
                }
                if change.conflicts_changed {
                    self.sync_conflict_badge(cx);
                }
                self.mark_dirty(cx);
                Some(change)
            }
            Err(error) => {
                log!("{error_label}: {error:?}");
                None
            }
        }
    }

    fn present_view_popup(&mut self, cx: &mut Cx, request: crate::doc_view::PopupRequest) {
        let bounds = self.window_bounds(cx);
        if let Some(mut popup) = self
            .ui
            .widget(cx, ids!(popup_root))
            .borrow_mut::<PopupRoot>()
        {
            match request {
                crate::doc_view::PopupRequest::NodeContextMenu {
                    anchor,
                    key,
                    context,
                } => {
                    self.node_menu_key = Some(key);
                    popup.show_at(
                        cx,
                        PopupSpec::Menu {
                            tag: live_id!(node_menu),
                            anchor,
                            bounds,
                            items: crate::popup::node_menu::compose(
                                context,
                                crate::popup::node_menu::base_items(),
                            ),
                            open: MenuOpen::Popup {
                                open_marking: None,
                                max_height: None,
                            },
                        },
                    );
                }
                crate::doc_view::PopupRequest::Select {
                    tag,
                    anchor_rect,
                    min_width,
                    items,
                    compact_frame,
                } => {
                    let anchor = dvec2(
                        anchor_rect.pos.x,
                        anchor_rect.pos.y + anchor_rect.size.y + crate::popup::select::SELECT_GAP,
                    );
                    popup.show_at(
                        cx,
                        PopupSpec::Select {
                            tag,
                            anchor,
                            min_width,
                            bounds,
                            items,
                            compact_frame,
                        },
                    );
                }
                crate::doc_view::PopupRequest::PlaceDial { center, items } => {
                    popup.show_at(
                        cx,
                        PopupSpec::Radial {
                            tag: live_id!(place_dial),
                            center,
                            bounds,
                            items,
                            open: RadialOpen::Dial,
                        },
                    );
                }
                crate::doc_view::PopupRequest::Dismiss => popup.close(cx),
            }
        }
    }

    pub(super) fn apply_view_outcome(
        &mut self,
        cx: &mut Cx,
        outcome: crate::doc_view::ViewOutcome,
    ) -> ActionFlow {
        let mut flow = ActionFlow::Continue;
        if let Some(edit) = outcome.edit {
            self.apply_session_edit(cx, edit, "view edit failed");
        }

        if let Some(request) = outcome.popup {
            self.present_view_popup(cx, request);
            flow = ActionFlow::Consumed;
        }
        if let Some(key) = outcome.promote_subject {
            self.documents.transition(
                cx,
                &self.ui,
                &self.session,
                DocumentCommand::PromoteSubject(key),
            );
            self.sync_document_shell(cx);
            flow = ActionFlow::Consumed;
        }
        if outcome.close_active {
            let id = self.documents.active_id();
            self.documents
                .transition(cx, &self.ui, &self.session, DocumentCommand::Close(id));
            self.sync_document_shell(cx);
            flow = ActionFlow::Consumed;
        }
        if outcome.statusbar_dirty {
            self.sync_statusbar(cx);
        }
        flow
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::popup::root::PopupRootAction;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Debug, Default, PartialEq, Eq)]
    struct QuitProbe {
        reasons: Vec<QuitReason>,
        cancelled_after_failed_save: usize,
    }

    #[test]
    fn logo_exit_popup_action_requests_a_cancelable_menu_quit() {
        let probe = Rc::new(RefCell::new(QuitProbe::default()));
        let event_probe = probe.clone();
        let mut cx = Cx::new(Box::new(move |_cx, event| {
            if let Event::QuitRequested(request) = event {
                event_probe.borrow_mut().reasons.push(request.reason);
                let failed_save = Err("disk full".to_string());
                event_probe.borrow_mut().cancelled_after_failed_save +=
                    usize::from(prevent_quit_after_failed_save(event, &failed_save));
                assert!(request.handled.get());
            }
        }));

        let popup_root = cx.with_vm(PopupRoot::script_new_with_default);
        let popup_root = WidgetRef::new_with_inner(Box::new(popup_root));
        let popup_uid = popup_root.widget_uid();
        let mut ui = cx.with_vm(View::script_new_with_default);
        ui.children.push((live_id!(popup_root), popup_root));

        let mut app = cx.with_vm(App::script_new_with_default);
        app.ui = WidgetRef::new_with_inner(Box::new(ui));
        let actions: ActionsBuf = vec![Box::new(WidgetAction {
            data: None,
            action: Box::new(PopupRootAction::Closed {
                tag: live_id!(logo),
                result: PopupResult::Invoked(live_id!(exit)),
            }),
            widget_uid: popup_uid,
            group: None,
        })];

        app.observe_popup_results(&mut cx, &actions);

        assert_eq!(
            *probe.borrow(),
            QuitProbe {
                reasons: vec![QuitReason::Menu],
                cancelled_after_failed_save: 1,
            }
        );
    }

    #[test]
    fn non_exclusive_observers_keep_the_existing_order() {
        assert_eq!(
            OBSERVER_ORDER,
            [
                ObserverHandler::CaptionAndDocks,
                ObserverHandler::PopupResults,
                ObserverHandler::ConflictList,
            ]
        );
    }

    #[test]
    fn exclusive_handlers_keep_the_existing_priority() {
        assert_eq!(
            EXCLUSIVE_ORDER,
            [
                ExclusiveHandler::NavigationScope,
                ExclusiveHandler::NavigationQuery,
                ExclusiveHandler::NavigationFilter,
                ExclusiveHandler::TreeContextMenu,
                ExclusiveHandler::TreeNavigation,
                ExclusiveHandler::DiagramSwitcher,
                ExclusiveHandler::ConflictBadge,
                ExclusiveHandler::ActiveDocumentView,
                ExclusiveHandler::LogoMenu,
                ExclusiveHandler::StartScreen,
                ExclusiveHandler::ShortcutsOverlay,
                ExclusiveHandler::FontsOverlay,
                ExclusiveHandler::IconsOverlay,
                ExclusiveHandler::ColorsOverlay,
                ExclusiveHandler::DocumentTabs,
            ]
        );
    }

    #[test]
    fn placement_dial_armed_is_relayed_before_closed() {
        let armed = DOCUMENT_POPUP_RELAY_ORDER
            .iter()
            .position(|handler| *handler == PopupRelay::Armed)
            .unwrap();
        let closed = DOCUMENT_POPUP_RELAY_ORDER
            .iter()
            .position(|handler| *handler == PopupRelay::Closed)
            .unwrap();
        assert!(armed < closed);
    }

    #[test]
    fn document_select_popups_share_one_shell_request_path() {
        let source = include_str!("actions.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();

        assert_eq!(production.matches("PopupRequest::Select {").count(), 1);
        assert!(!production.contains("PopupRequest::MaxAttributesPicker"));
        assert!(!production.contains("PopupRequest::ElementPicker"));
    }
}
