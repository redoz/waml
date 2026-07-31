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
    NavigationQuery,
    NavigationFilter,
    TreeContextMenu,
    TreeNavigation,
    HistoryControls,
    DocumentHeader,
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

const EXCLUSIVE_ORDER: [ExclusiveHandler; 16] = [
    ExclusiveHandler::NavigationQuery,
    ExclusiveHandler::NavigationFilter,
    ExclusiveHandler::TreeContextMenu,
    ExclusiveHandler::TreeNavigation,
    ExclusiveHandler::HistoryControls,
    ExclusiveHandler::DocumentHeader,
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
                ExclusiveHandler::NavigationQuery => self.handle_navigation_query(cx, actions),
                ExclusiveHandler::NavigationFilter => self.handle_navigation_filter(cx, actions),
                ExclusiveHandler::TreeContextMenu => self.handle_tree_context_menu(cx, actions),
                ExclusiveHandler::TreeNavigation => self.handle_tree_navigation(cx, actions),
                ExclusiveHandler::HistoryControls => self.handle_history_controls(cx, actions),
                ExclusiveHandler::DocumentHeader => self.handle_document_header_action(cx, actions),
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

        let document_header_action = self
            .ui
            .widget(cx, ids!(document_header))
            .borrow::<crate::document_header::DocumentHeader>()
            .and_then(|header| header.action(actions));
        if matches!(
            document_header_action,
            Some(crate::document_header::DocumentHeaderAction::ToggleRightDock)
        ) {
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
            if let Some(document) = self
                .documents
                .tabs()
                .iter()
                .find(|tab| tab.id == id)
                .map(|tab| tab.locator())
            {
                self.transition_to_location(
                    cx,
                    crate::view_history::ViewLocation {
                        document,
                        anchor: crate::view_history::ViewAnchor::None,
                    },
                    super::TransitionCause::UserNavigation,
                );
            }
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
                        self.handle_navigation_intent(
                            cx,
                            crate::navigation::NavigationIntent::Resolved {
                                target: crate::navigation::NavigationTarget::ExternalUrl(
                                    "https://github.com/redoz/waml".into(),
                                ),
                                disposition: crate::navigation::OpenDisposition::Preview,
                            },
                        );
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
                        self.open_view_source(cx, &key);
                    }
                    crate::popup::node_menu::NodeMenuCommand::FindInDiagrams => {
                        log!("find in diagrams: {key}");
                    }
                }
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
                            crate::document::EditIntent {
                                edit: waml::edit::PendingEdit::new(waml::uml::Batch(vec![op])),
                                label: "Remove conflicting placement".into(),
                                merge_key: None,
                                after_location: None,
                            },
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
        self.handle_navigation_intent(cx, intent);
        ActionFlow::Consumed
    }

    /// `tab_row`'s view-history pair. Matched on the action TAG rather than the
    /// button uid so the pair can move within the chrome without this seam
    /// caring where it is mounted.
    fn handle_history_controls(&mut self, cx: &mut Cx, actions: &Actions) -> ActionFlow {
        let mut direction = None;
        for action in actions {
            let Some(item) = action.as_widget_action() else {
                continue;
            };
            if let Some(crate::icon_button::IconButtonAction::TaggedClicked(tag)) =
                item.action.downcast_ref::<crate::icon_button::IconButtonAction>()
            {
                if *tag == live_id!(history_back) {
                    direction = Some((crate::view_history::HistoryDirection::Back, "No previous view"));
                    break;
                }
                if *tag == live_id!(history_forward) {
                    direction = Some((crate::view_history::HistoryDirection::Forward, "No next view"));
                    break;
                }
            }
        }
        let Some((direction, problem)) = direction else {
            return ActionFlow::Continue;
        };
        if self.traverse_view_history(cx, direction) {
            self.clear_history_feedback(cx);
        } else {
            self.set_history_problem(cx, Some(problem));
        }
        ActionFlow::Consumed
    }

    fn handle_document_header_action(&mut self, cx: &mut Cx, actions: &Actions) -> ActionFlow {
        let action = self
            .ui
            .widget(cx, ids!(document_header))
            .borrow::<crate::document_header::DocumentHeader>()
            .and_then(|header| header.action(actions));
        match action {
            Some(crate::document_header::DocumentHeaderAction::RevealInTree(target)) => {
                let accepted = self
                    .ui
                    .widget(cx, ids!(project_tree))
                    .borrow_mut::<crate::tree_panel::ProjectTree>()
                    .is_some_and(|mut tree| tree.reveal_target(cx, &target));
                if accepted {
                    let (_, inspector) = self.dock_states(cx);
                    let inspector = if self.narrow {
                        crate::dock::DockState::Flag
                    } else {
                        inspector
                    };
                    self.apply_dock_states(cx, crate::dock::DockState::Pinned, inspector);
                }
                ActionFlow::Consumed
            }
            _ => ActionFlow::Continue,
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
            .uml_projection()
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
                if let Some(document) = self
                    .documents
                    .tabs()
                    .iter()
                    .find(|tab| tab.id == id)
                    .map(|tab| tab.locator())
                {
                    self.transition_to_location(
                        cx,
                        crate::view_history::ViewLocation {
                            document,
                            anchor: crate::view_history::ViewAnchor::None,
                        },
                        super::TransitionCause::UserNavigation,
                    );
                }
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
                self.close_document(cx, id);
            }
            crate::doc_tabs::DocTabsAction::None => {}
        }
        ActionFlow::Consumed
    }

    pub(super) fn apply_session_edit(
        &mut self,
        cx: &mut Cx,
        intent: crate::document::EditIntent,
        error_label: &str,
    ) -> Option<crate::editor_session::SessionChange> {
        let Some(before_location) = self.documents.capture_active_location(cx, &self.ui) else {
            log!("{error_label}: no active document");
            return None;
        };
        match self.session.apply_edit(crate::editor_session::EditRequest {
            intent,
            before_location,
        }) {
            Ok(change) => {
                self.complete_session_change(cx, change);
                Some(change)
            }
            Err(error) => {
                log!("{error_label}: {error:?}");
                None
            }
        }
    }

    fn complete_session_change(
        &mut self,
        cx: &mut Cx,
        change: crate::editor_session::SessionChange,
    ) {
        let prepared = if change.okf_changed || change.uml_changed {
            self.documents
                .tabs()
                .iter()
                .map(|tab| {
                    crate::documents::reopen(
                        self.session.okf_analysis(),
                        self.session.uml_analysis(),
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
                self.session.okf_analysis(),
                self.session.uml_analysis(),
            );
            self.refresh_nav(cx, false);
        }
        if change.conflicts_changed {
            self.sync_conflict_badge(cx);
        }
        if let Some(current) = self.documents.capture_active_location(cx, &self.ui) {
            self.transition_to_location(cx, current, super::TransitionCause::PassiveReconciliation);
        }
        self.mark_dirty(cx);
        self.sync_history_controls(cx);
    }

    fn complete_history_effect(
        &mut self,
        cx: &mut Cx,
        effect: crate::editor_session::HistoryEffect,
        verb: &str,
    ) -> bool {
        let label = effect.label;
        let location = effect.location;
        self.complete_session_change(cx, effect.change);
        if self.transition_to_location(cx, location.clone(), super::TransitionCause::UndoRedoReveal)
        {
            self.set_navigation_message(cx, None);
            self.set_history_success(cx, Some(&format!("{verb}: {label}")));
            true
        } else {
            self.set_history_problem(
                cx,
                Some(&format!(
                    "{verb}: {label}, but could not reveal {}",
                    location.document.concept_id
                )),
            );
            false
        }
    }

    pub(super) fn perform_undo(&mut self, cx: &mut Cx) -> bool {
        match self.session.undo() {
            Ok(Some(effect)) => self.complete_history_effect(cx, effect, "Undid"),
            Ok(None) => {
                self.set_history_problem(cx, Some("Nothing to undo"));
                false
            }
            Err(error) => {
                log!("undo failed: {error:?}");
                self.set_history_problem(cx, Some("Undo failed"));
                false
            }
        }
    }

    pub(super) fn perform_redo(&mut self, cx: &mut Cx) -> bool {
        match self.session.redo() {
            Ok(Some(effect)) => self.complete_history_effect(cx, effect, "Redid"),
            Ok(None) => {
                self.set_history_problem(cx, Some("Nothing to redo"));
                false
            }
            Err(error) => {
                log!("redo failed: {error:?}");
                self.set_history_problem(cx, Some("Redo failed"));
                false
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

        if let Some(intent) = outcome.navigation {
            self.handle_navigation_intent(cx, intent);
            flow = ActionFlow::Consumed;
        }

        if let Some(key) = outcome.view_source {
            self.open_view_source(cx, &key);
            flow = ActionFlow::Consumed;
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
            self.close_document(cx, id);
            flow = ActionFlow::Consumed;
        }
        if outcome.statusbar_dirty {
            self.sync_statusbar(cx);
        }
        if outcome.break_merge_group {
            self.session.break_edit_merge_group();
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
                ExclusiveHandler::NavigationQuery,
                ExclusiveHandler::NavigationFilter,
                ExclusiveHandler::TreeContextMenu,
                ExclusiveHandler::TreeNavigation,
                ExclusiveHandler::HistoryControls,
                ExclusiveHandler::DocumentHeader,
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
