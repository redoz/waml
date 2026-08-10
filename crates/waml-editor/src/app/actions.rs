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
    Inspector,
    FindStrip,
}

const OBSERVER_ORDER: [ObserverHandler; 5] = [
    ObserverHandler::CaptionAndDocks,
    ObserverHandler::PopupResults,
    ObserverHandler::ConflictList,
    ObserverHandler::Inspector,
    ObserverHandler::FindStrip,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PopupRelay {
    Armed,
    Closed,
}

const DOCUMENT_POPUP_RELAY_ORDER: [PopupRelay; 2] = [PopupRelay::Armed, PopupRelay::Closed];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExclusiveHandler {
    TreeContextMenu,
    TreeNavigation,
    TreeProjectionMenu,
    ProjectionMenuToggle,
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
    ExclusiveHandler::TreeContextMenu,
    ExclusiveHandler::TreeNavigation,
    ExclusiveHandler::TreeProjectionMenu,
    ExclusiveHandler::ProjectionMenuToggle,
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

/// Map a `ConflictListAction::Delete` to the `Op::PlaceRm` that removes it
/// from `diagram`'s `## Layout` section. Pure so it is unit-testable without
/// a live `Cx`/`App`; `None` for any other action (nothing to remove).
fn place_rm_for(
    diagram: &str,
    action: &crate::popup::conflict_list::ConflictListAction,
) -> Option<waml::uml::Op> {
    match action {
        crate::popup::conflict_list::ConflictListAction::Delete { subject, reference } => {
            Some(waml::uml::Op::PlacementRemove {
                diagram: diagram.to_string(),
                subject_slug: subject.clone(),
                reference_slug: reference.clone(),
            })
        }
        _ => None,
    }
}

impl App {
    pub(super) fn handle_action_batch(&mut self, cx: &mut Cx, actions: &Actions) {
        for observer in OBSERVER_ORDER {
            match observer {
                ObserverHandler::CaptionAndDocks => self.observe_caption_and_docks(cx, actions),
                ObserverHandler::PopupResults => self.observe_popup_results(cx, actions),
                ObserverHandler::ConflictList => self.observe_conflict_list(cx, actions),
                ObserverHandler::Inspector => self.observe_inspector(cx, actions),
                ObserverHandler::FindStrip => self.observe_find_strip(cx, actions),
            }
        }

        for handler in EXCLUSIVE_ORDER {
            let flow = match handler {
                ExclusiveHandler::TreeContextMenu => self.handle_tree_context_menu(cx, actions),
                ExclusiveHandler::TreeNavigation => self.handle_tree_navigation(cx, actions),
                ExclusiveHandler::TreeProjectionMenu => {
                    self.handle_tree_projection_menu(cx, actions)
                }
                ExclusiveHandler::ProjectionMenuToggle => {
                    self.handle_projection_menu_toggle(cx, actions)
                }
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
                        items: burger_menu_items(!self.session.snapshot().source.is_empty()),
                        open: MenuOpen::Press(press),
                    },
                );
            }
            self.ui
                .widget(cx, ids!(menu_btn))
                .as_icon_button()
                .set_active(cx, true);
        }

        // The caption's search button: the one visible route to the palette,
        // which is otherwise reachable only by knowing Ctrl+K. `clicked`, not
        // `pressed` -- the palette is a latched popup, not a marking menu, so
        // it opens on release like every other latched surface.
        if self
            .ui
            .widget(cx, ids!(search_btn))
            .as_icon_button()
            .clicked(actions)
        {
            self.open_palette(cx);
        }

        // The tree-column toggle, which lives in the caption's tab row at the
        // column's right edge (see `tree_toggle_layout`).
        let tree_toggled = self
            .ui
            .widget(cx, ids!(tree_btn))
            .as_icon_button()
            .clicked(actions);
        if tree_toggled {
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

        self.observe_panel_splitters(cx, actions);
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
        let palette_closed = result_for(live_id!(palette));
        let palette_query = popup.palette_query_changed(cx, live_id!(palette), actions);
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
                    ViewLocation {
                        document,
                        anchor: ViewAnchor::None,
                    },
                    TransitionCause::UserNavigation,
                );
            }
        }
        if let Some(PopupResult::Invoked(id)) = burger_closed {
            if id == live_id!(new_model) {
                log!("New model: not yet implemented (template picker is a later slice)");
            } else if id == live_id!(open_model) {
                self.open_model_via_picker(cx);
            } else if id == live_id!(export_waml) {
                self.export_current_bundle(cx);
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
        // Live re-query (Task 11): a keystroke inside the open palette
        // re-runs `SearchState` and pushes fresh sections back in place --
        // the palette never closes for this, so it rides its own branch
        // rather than `DOCUMENT_POPUP_RELAY_ORDER` below (that loop only
        // fires on Armed/Closed).
        if let Some(query) = palette_query {
            self.push_palette_query(cx, query);
        }
        if let Some(result) = palette_closed {
            self.handle_palette_closed(cx, result);
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

    /// Ctrl+K (`shortcuts::SearchCommand::OpenPalette`, Task 11): show the
    /// palette centred-top over the window, seeded with the empty-query
    /// RECENT section (spec §Palette).
    pub(super) fn open_palette(&mut self, cx: &mut Cx) {
        let bounds = self.window_bounds(cx);
        // Once per open, not once per keystroke: see `palette_hidden`.
        self.palette_hidden = self
            .search
            .hidden_documents(self.session.okf_analysis(), self.session.uml_analysis());
        let sections = self.build_palette_sections("");
        self.palette_query.clear();
        // Fallback only: if the popup is not there to trim, the model we
        // built is what a commit would have to resolve against.
        let mut kept = sections.clone();
        if let Some(mut popup) = self
            .ui
            .widget(cx, ids!(popup_root))
            .borrow_mut::<PopupRoot>()
        {
            popup.show_at(
                cx,
                PopupSpec::Palette {
                    tag: live_id!(palette),
                    bounds,
                    sections,
                },
            );
            if let Some(trimmed) = popup.palette_sections(cx, live_id!(palette)) {
                kept = trimmed;
            }
        }
        self.palette_sections = kept;
    }

    /// A keystroke inside the still-open palette (`PaletteAction::QueryChanged`):
    /// re-run `SearchState` for `query` and push fresh sections back in place.
    fn push_palette_query(&mut self, cx: &mut Cx, query: String) {
        let sections = self.build_palette_sections(&query);
        self.palette_query = query;
        let mut kept = sections.clone();
        if let Some(mut popup) = self
            .ui
            .widget(cx, ids!(popup_root))
            .borrow_mut::<PopupRoot>()
        {
            popup.set_palette_sections(cx, live_id!(palette), sections);
            if let Some(trimmed) = popup.palette_sections(cx, live_id!(palette)) {
                kept = trimmed;
            }
        }
        self.palette_sections = kept;
    }

    /// The blended, sectioned palette model for `query` (spec §Palette):
    /// live hits from `SearchState`, the projection's hidden-document set,
    /// and the empty-query RECENT fallback.
    fn build_palette_sections(&self, query: &str) -> Vec<PaletteSectionModel> {
        let scope = waml::search::QueryScope::default();
        let hits = self.search.query(query, &scope);
        let recents = self.palette_recents();
        crate::popup::palette::build_palette_model(
            query,
            &hits,
            &|hit| self.search.snippet(hit, 80),
            &self.palette_hidden,
            &recents,
            self.search.status(),
        )
    }

    /// Most recent distinct concept documents from `view_history`, newest
    /// first, capped at 8 (decision 2) -- the palette's empty-query RECENT
    /// section.
    fn palette_recents(&self) -> Vec<(String, String)> {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for index in (0..self.view_history.len()).rev() {
            let Some(location) = self.view_history.entry_at(index) else {
                continue;
            };
            let Some(concept_id) = location.document.concept_id() else {
                continue;
            };
            if !seen.insert(concept_id.to_string()) {
                continue;
            }
            let title = self
                .session
                .okf()
                .concept(concept_id)
                .and_then(|concept| concept.title.clone())
                .unwrap_or_else(|| {
                    concept_id
                        .rsplit('/')
                        .next()
                        .unwrap_or(concept_id)
                        .to_string()
                });
            out.push((concept_id.to_string(), title));
            if out.len() >= 8 {
                break;
            }
        }
        out
    }

    /// Resolve a palette commit's opaque `p:{section}:{row}` id back to the
    /// `PaletteRow` it was built from. `palette_sections` mirrors what the
    /// card KEPT after trimming, not what it was handed, because the ids are
    /// positional over the trimmed model. The widget is reset before the
    /// close action is readable, so this is the only place left to recover
    /// which row committed.
    fn resolve_palette_commit(&self, id: LiveId) -> Option<crate::popup::palette::PaletteRow> {
        for (section_index, section) in self.palette_sections.iter().enumerate() {
            for (row_index, row) in section.rows.iter().enumerate() {
                if LiveId::from_str(&format!("p:{section_index}:{row_index}")) == id {
                    return Some(row.clone());
                }
            }
        }
        None
    }

    fn handle_palette_closed(&mut self, cx: &mut Cx, result: PopupResult) {
        let PopupResult::Invoked(id) = result else {
            return;
        };
        if let Some(row) = self.resolve_palette_commit(id) {
            self.apply_palette_row(cx, &row);
        }
    }

    /// Palette commit routing (spec §Palette, Task 11): a `Concept`/`Text`/
    /// `Structure` row opens on the surface its hit maps to and reveals it;
    /// a `Document`/`Recent` row opens normally, with no reveal; a
    /// `MoreText`/`Escalate` row escalates to the full results tab.
    /// `NoResults`/`Indexing` rows are never activatable (see
    /// `popup::palette::is_activatable`), so they never reach here.
    fn apply_palette_row(&mut self, cx: &mut Cx, row: &crate::popup::palette::PaletteRow) {
        use crate::popup::palette::PaletteRowKind;
        match &row.kind {
            PaletteRowKind::Concept { .. }
            | PaletteRowKind::Text { .. }
            | PaletteRowKind::Structure { .. } => {
                if let Some(hit) = &row.hit {
                    let (navigation, reveal) = crate::search_results_view::navigation_for_hit(hit);
                    let outcome = crate::doc_view::ViewOutcome {
                        navigation: Some(navigation),
                        reveal,
                        ..Default::default()
                    };
                    let _ = self.apply_view_outcome(cx, outcome);
                }
            }
            PaletteRowKind::Document => {
                if let Some(hit) = &row.hit {
                    let concept_id = crate::search_results_view::concept_id_for_hit(hit);
                    self.transition_document(cx, &concept_id, false);
                }
            }
            PaletteRowKind::Recent { concept_id } => {
                self.transition_document(cx, concept_id, false);
            }
            PaletteRowKind::Escalate { query, .. } => {
                self.open_search_results(cx, query);
            }
            PaletteRowKind::MoreText { .. } => {
                let query = self.palette_query.clone();
                self.open_search_results(cx, &query);
            }
            PaletteRowKind::NoResults { .. } | PaletteRowKind::Indexing => {}
        }
    }

    /// Ctrl+F (`shortcuts::SearchCommand::OpenFindStrip`, Task 13): open the
    /// find strip pre-scoped to the active document (spec §Find strip).
    /// Opening over a tab with no searchable surface (folder view, no
    /// concept) leaves the session hitless -- the strip still opens, reading
    /// "0 results" once a query is typed, never a crash.
    pub(super) fn open_find_strip(&mut self, cx: &mut Cx) {
        let scope = waml::search::QueryScope {
            document: self.active_document_path(),
        };
        self.find = Some(SearchSession::new(String::new(), Vec::new(), scope));
        self.ui
            .widget(cx, ids!(find_strip))
            .as_find_strip()
            .open(cx);
        self.push_find_model(cx);
    }

    /// The active tab's bundle-relative document path -- the same shape as
    /// `Hit::document`/`QueryScope::document` -- or `None` when nothing is
    /// open or the active tab carries no concept (folder view, start
    /// screen).
    fn active_document_path(&self) -> Option<String> {
        let concept_id = self.documents.active_tab()?.concept_id()?;
        self.session
            .snapshot()
            .source
            .document_by_concept_id(concept_id)
            .map(|document| document.path().as_str().to_string())
    }

    /// Route the find strip's own actions (Task 13): a keystroke re-queries
    /// `SearchState` scoped to the document the strip opened against;
    /// Next/Previous step the session cursor; Close ends it and clears
    /// whatever highlight/spotlight state it left on the document.
    fn observe_find_strip(&mut self, cx: &mut Cx, actions: &Actions) {
        let Some(action) = self
            .ui
            .widget(cx, ids!(find_strip))
            .as_find_strip()
            .action(actions)
        else {
            return;
        };
        match action {
            FindStripAction::QueryChanged(query) => self.requery_find(cx, query),
            FindStripAction::Next => self.step_find(cx, true),
            FindStripAction::Previous => self.step_find(cx, false),
            FindStripAction::Close => self.close_find_strip(cx),
        }
    }

    /// Re-run the strip's query against `SearchState`, scoped to whatever
    /// document `open_find_strip` pinned. A fresh `SearchSession` per
    /// keystroke means the cursor always resets to `None` on a query change
    /// (spec: no stale "3 of 12" surviving into a different result set).
    fn requery_find(&mut self, cx: &mut Cx, query: String) {
        let scope = self
            .find
            .as_ref()
            .map(|session| session.scope.clone())
            .unwrap_or_default();
        // `QueryScope::document == None` means UNSCOPED to the index, but for
        // the find strip it means "the active tab has no searchable document"
        // (folder view, results tab, start screen). Searching the whole bundle
        // there would report a bundle-wide count from a strip that says "find
        // in document"; the spec's answer is `0 results`.
        let hits = if scope.document.is_some() {
            self.search.query(&query, &scope)
        } else {
            Vec::new()
        };
        self.find = Some(SearchSession::new(query, hits, scope));
        self.apply_find_highlights(cx);
        self.push_find_model(cx);
    }

    /// Push the strip's `FindModel` counter from the live session's hit
    /// count and cursor. Called after every session change (open, requery,
    /// step) so the widget's "3 of 12" readout never lags the session.
    fn push_find_model(&mut self, cx: &mut Cx) {
        let Some(session) = &self.find else {
            return;
        };
        let model = FindModel {
            query: session.query.clone(),
            total: session.hits.len(),
            current: session.cursor,
        };
        self.ui
            .widget(cx, ids!(find_strip))
            .as_find_strip()
            .set_model(cx, &model);
    }

    /// Light every current match at once (spec §Find strip): text hits go to
    /// the markdown editor surface as search-match decoration, model-element
    /// hits go to the canvas as a spotlight set. Whichever surface isn't
    /// showing simply holds state nothing draws -- cheap and always correct
    /// once that surface becomes active. Called on every query change; a
    /// step (`Next`/`Previous`) narrows the markdown editor's highlight back
    /// down to the current span via the shared `DocView::reveal` path
    /// instead (`step_find`), the same single-target reveal the results tab
    /// and F3 use.
    fn apply_find_highlights(&mut self, cx: &mut Cx) {
        let Some(session) = &self.find else {
            return;
        };
        let mut text_ranges = Vec::new();
        let mut model_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
        for hit in &session.hits {
            match &hit.target {
                waml::search::HitTarget::TextSpan { start, end, .. } => {
                    if let Ok(range) = TextRange::new(TextSize::new(*start), TextSize::new(*end)) {
                        text_ranges.push(range);
                    }
                }
                waml::search::HitTarget::ModelElement { key } => {
                    model_keys.insert(key.clone());
                }
            }
        }

        self.install_search_highlights(cx, text_ranges, model_keys);
    }

    /// Install a search decoration on every surface that can show one: BOTH
    /// markdown surfaces -- the raw-source editor AND the reading viewer,
    /// which is what `GenericOkfView` shows by default, so highlighting only
    /// the editor lights nothing on a freshly-opened concept -- plus the
    /// canvas spotlight. Whichever surface is not showing simply holds state
    /// nothing draws.
    fn install_search_highlights(
        &mut self,
        cx: &mut Cx,
        text_ranges: Vec<TextRange>,
        model_keys: std::collections::HashSet<String>,
    ) {
        let body = crate::doc_view::BodyWidgets::new(cx, &self.ui);
        body.markdown_editor()
            .set_search_highlights(cx, text_ranges.clone());
        body.markdown_viewer()
            .set_search_highlights(cx, text_ranges);

        let spotlight = (!model_keys.is_empty()).then_some(model_keys);
        if let Some(mut canvas) = body
            .canvas(cx)
            .borrow_mut::<crate::canvas::ClassDiagramSurface>()
        {
            canvas.set_search_spotlight(cx, spotlight);
        }
    }

    /// Drop every installed search decoration. The counterpart of
    /// `install_search_highlights`: the markdown surfaces are SHARED between
    /// documents, so a document switch must clear them or the previous
    /// document's byte ranges stay lit over whatever loads next.
    pub(super) fn clear_search_highlights(&mut self, cx: &mut Cx) {
        let body = crate::doc_view::BodyWidgets::new(cx, &self.ui);
        body.markdown_editor().clear_search_highlights(cx);
        body.markdown_viewer().clear_search_highlights(cx);
        if let Some(mut canvas) = body
            .canvas(cx)
            .borrow_mut::<crate::canvas::ClassDiagramSurface>()
        {
            canvas.set_search_spotlight(cx, None);
        }
    }

    /// Advance the session cursor and reveal the landed hit through the
    /// shared `DocView::reveal` path (spec: "Next/Previous call the active
    /// view's reveal") -- a text hit scrolls+highlights its span, a model
    /// hit selects and pans the canvas to it (composing with the persistent
    /// spotlight `apply_find_highlights` already set). A no-op when the
    /// session is empty; `reveal_active` itself no-ops when nothing is
    /// active, so this never crashes over a tab with no searchable surface.
    pub(super) fn step_find(&mut self, cx: &mut Cx, forward: bool) {
        let hit = match &mut self.find {
            Some(session) => session.advance(forward).cloned(),
            None => None,
        };
        self.push_find_model(cx);
        let Some(hit) = hit else {
            return;
        };
        let target = match hit.target {
            waml::search::HitTarget::TextSpan { start, end, .. } => {
                crate::doc_view::RevealTarget::TextSpan { start, end }
            }
            waml::search::HitTarget::ModelElement { key } => {
                crate::doc_view::RevealTarget::ModelElement { key }
            }
        };
        self.documents.reveal_active(cx, &self.ui, &target);
    }

    /// End the find session: hide the strip, drop it, and clear whatever
    /// highlight/spotlight it left on the document (spec §Find strip:
    /// "Close clears highlights AND the spotlight").
    fn close_find_strip(&mut self, cx: &mut Cx) {
        self.find = None;
        self.ui
            .widget(cx, ids!(find_strip))
            .as_find_strip()
            .close(cx);
        self.clear_search_highlights(cx);
    }

    // ----------------------------------------------------------------
    // Task 14: bundle-wide live search session -- F3/Shift+F3 traversal,
    // landing-site marking, and Esc termination (spec §Search session).
    // ----------------------------------------------------------------

    /// The open results tab's id for the CURRENT `session_search`'s query
    /// (decision 7's locator shape, `documents::tab_id_for`) -- recomputed on
    /// demand rather than cached, since it is cheap and the query never
    /// changes without `session_search` itself being replaced.
    fn session_search_tab_id(&self) -> Option<LiveId> {
        let session = self.session_search.as_ref()?;
        let locator = crate::navigation::DocumentLocator::new(
            waml::view::row::RowTarget::Virtual,
            waml::view::surface::SurfaceId(format!("search:{}", session.query)),
        );
        Some(crate::documents::tab_id_for(&locator))
    }

    /// Every producer of `ViewOutcome.reveal` is a search-hit landing -- a
    /// results-tab row click, a palette commit, or F3/Shift+F3 re-running
    /// `navigation_for_hit` on the session's own next/previous hit (spec
    /// §Search session) -- so `apply_view_outcome` calls this whenever
    /// `outcome.reveal` is `Some`, the one place a live session's cursor gets
    /// marked: locates `(concept_id, target)` in `session_search.hits`
    /// (matching how `navigation_for_hit` maps a hit to a reveal, since that
    /// is the only place doing the reverse mapping), mirrors the index into
    /// the results tab if it is still open, and re-lights every OTHER hit
    /// the session found in the document that just landed.
    fn mark_session_landing(
        &mut self,
        cx: &mut Cx,
        concept_id: &str,
        target: &crate::doc_view::RevealTarget,
    ) {
        let stepped = self.stepped_session_index.take();
        let Some(session) = self.session_search.as_mut() else {
            return;
        };
        // An F3/Shift+F3 step already chose the index; only a landing from
        // somewhere else (a results-tab row click, a palette commit) has to
        // locate itself, and it can only do so by target -- which several
        // hits of one concept share, so it lands on the first of them.
        let index = stepped
            .filter(|&index| index < session.hits.len())
            .or_else(|| {
                session.hits.iter().position(|hit| {
                    crate::search_results_view::navigation_for_hit(hit)
                        .1
                        .as_ref()
                        .is_some_and(|(cid, t)| cid == concept_id && t == target)
                })
            });
        session.cursor = index;
        if let Some(tab_id) = self.session_search_tab_id() {
            self.documents
                .mark_search_cursor(cx, &self.ui, tab_id, index);
        }
        self.apply_session_highlights(cx, concept_id);
    }

    /// Light every OTHER hit `session_search` found in `concept_id`'s
    /// document (spec §Search session: "every other match in the open
    /// document is highlighted") -- text hits as markdown search-match
    /// decoration, model-element hits as a canvas spotlight, the same split
    /// `apply_find_highlights` (Task 13) uses, filtered here to just this
    /// document since the session spans the whole bundle.
    fn apply_session_highlights(&mut self, cx: &mut Cx, concept_id: &str) {
        let Some((text_ranges, model_keys)) = self.session_highlights_for(concept_id) else {
            return;
        };
        self.install_search_highlights(cx, text_ranges, model_keys);
    }

    /// Re-light the session's matches in `concept_id`'s document AFTER its
    /// reveal has landed. `DocView::reveal` installs the landed range as the
    /// surfaces' ONLY search highlight (`set_search_highlights(cx,
    /// vec![range])`), replacing the set `apply_session_highlights` installed
    /// while the tab was still opening -- so without this the documented
    /// "every other match in the open document is highlighted" never survives
    /// to a frame. A no-op when the session found nothing in this document:
    /// installing an empty set there would clear the reveal's own highlight.
    pub(super) fn relight_session_highlights(&mut self, cx: &mut Cx, concept_id: &str) {
        let Some((text_ranges, model_keys)) = self.session_highlights_for(concept_id) else {
            return;
        };
        if text_ranges.is_empty() && model_keys.is_empty() {
            return;
        }
        self.install_search_highlights(cx, text_ranges, model_keys);
    }

    /// The live session's `(text ranges, model keys)` inside `concept_id`'s
    /// document. `None` with no live session at all.
    #[allow(clippy::type_complexity)]
    fn session_highlights_for(
        &self,
        concept_id: &str,
    ) -> Option<(Vec<TextRange>, std::collections::HashSet<String>)> {
        let session = self.session_search.as_ref()?;
        let mut text_ranges = Vec::new();
        let mut model_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
        for hit in &session.hits {
            if crate::search_results_view::concept_id_for_hit(hit) != concept_id {
                continue;
            }
            match &hit.target {
                waml::search::HitTarget::TextSpan { start, end, .. } => {
                    if let Ok(range) = TextRange::new(TextSize::new(*start), TextSize::new(*end)) {
                        text_ranges.push(range);
                    }
                }
                waml::search::HitTarget::ModelElement { key } => {
                    model_keys.insert(key.clone());
                }
            }
        }
        Some((text_ranges, model_keys))
    }

    /// F3/Shift+F3 (`shortcuts::SearchCommand::NextHit`/`PreviousHit`, when
    /// the document-scoped find strip is NOT open -- spec §Search session's
    /// precedence rule): advance the session cursor, wrapping, and run the
    /// SAME navigation+reveal path a results-tab row click uses
    /// (`navigation_for_hit` -> `apply_view_outcome`) so crossing into a
    /// different document opens it exactly like any other search landing --
    /// including re-marking the session and mirroring the results tab, both
    /// of which `apply_view_outcome`'s `outcome.reveal` handling already
    /// does. A no-op with no live session or no hits.
    pub(super) fn step_session(&mut self, cx: &mut Cx, forward: bool) {
        let stepped = match self.session_search.as_mut() {
            Some(session) => session
                .advance(forward)
                .cloned()
                .map(|hit| (session.cursor, hit)),
            None => None,
        };
        let Some((index, hit)) = stepped else {
            return;
        };
        // The landing this outcome triggers must keep THIS index; it cannot
        // recover it from the reveal (see `stepped_session_index`).
        self.stepped_session_index = index;
        let (navigation, reveal) = crate::search_results_view::navigation_for_hit(&hit);
        let outcome = crate::doc_view::ViewOutcome {
            navigation: Some(navigation),
            reveal,
            ..Default::default()
        };
        let _ = self.apply_view_outcome(cx, outcome);
    }

    /// Esc (spec §Search session): end the live bundle-wide session -- clear
    /// it, whatever highlight/spotlight it left on the document, the results
    /// tab's cursor mirror (if that tab is still open), and the find strip
    /// too if it happened to be open alongside it (a clean end state, even
    /// though the two sessions are otherwise independent -- see `find`'s own
    /// doc comment). Called by `handle_escape_event` BEFORE the existing
    /// per-view escape, so a live session consumes the first Esc.
    pub(super) fn end_session_search(&mut self, cx: &mut Cx) {
        if let Some(tab_id) = self.session_search_tab_id() {
            self.documents
                .mark_search_cursor(cx, &self.ui, tab_id, None);
        }
        self.session_search = None;
        self.clear_search_highlights(cx);
        if self.find.is_some() {
            self.close_find_strip(cx);
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
                    .and_then(|tab| tab.concept_id().map(str::to_string))
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

    fn observe_inspector(&mut self, cx: &mut Cx, actions: &Actions) {
        let action = self
            .ui
            .widget(cx, ids!(inspector))
            .borrow::<crate::inspector_panel::Inspector>()
            .and_then(|panel| panel.trace_action(actions));
        let Some(action) = action else {
            return;
        };
        use crate::inspector_panel::InspectorAction;
        let (selector, edit, label) = match action {
            InspectorAction::AddTrace {
                selector,
                index,
                label,
                href,
            } => (
                selector,
                waml::uml::TraceEdit::Insert { index, label, href },
                "Add transition trace",
            ),
            InspectorAction::UpdateTrace {
                selector,
                index,
                label,
                href,
            } => (
                selector,
                waml::uml::TraceEdit::Update { index, label, href },
                "Update transition trace",
            ),
            InspectorAction::RemoveTrace { selector, index } => (
                selector,
                waml::uml::TraceEdit::Remove { index },
                "Remove transition trace",
            ),
            InspectorAction::MoveTrace { selector, from, to } => (
                selector,
                waml::uml::TraceEdit::Move { from, to },
                "Move transition trace",
            ),
            InspectorAction::OpenTrace(target) => {
                self.handle_navigation_intent(
                    cx,
                    crate::navigation::NavigationIntent::Resolved {
                        target,
                        disposition: crate::navigation::OpenDisposition::Preview,
                    },
                );
                return;
            }
            InspectorAction::None | InspectorAction::Edited(_) => return,
        };
        self.apply_session_edit(
            cx,
            crate::document::EditIntent {
                edit: waml::edit::PendingEdit::new(waml::uml::Batch(vec![
                    waml::uml::Op::EditTransitionTraces { selector, edit },
                ])),
                label: label.into(),
                merge_key: None,
                after_location: None,
            },
            "transition trace edit failed",
        );
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

    fn handle_tree_projection_menu(&mut self, cx: &mut Cx, actions: &Actions) -> ActionFlow {
        let opened = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow::<crate::tree_panel::ProjectTree>()
            .and_then(|panel| panel.projection_menu_opened(actions));
        let Some(anchor) = opened else {
            return ActionFlow::Continue;
        };
        let registry = crate::folder_projection::core_registry();
        let maskable = crate::folder_projection::maskable_names(&registry);
        let items = crate::app::menus::projection_menu_items(&maskable, &self.projection_mask);
        let bounds = self.window_bounds(cx);
        if let Some(mut popup) = self
            .ui
            .widget(cx, ids!(popup_root))
            .borrow_mut::<PopupRoot>()
        {
            popup.show_at(
                cx,
                PopupSpec::Menu {
                    tag: live_id!(projection_menu),
                    anchor,
                    bounds,
                    items,
                    open: MenuOpen::Sticky { max_height: None },
                },
            );
        }
        ActionFlow::Consumed
    }

    /// A row in the open projection popup was toggled: update the session
    /// mask and re-seed the still-open card's checkmarks in place --
    /// reopening would reset the anchor and drop the hover.
    fn handle_projection_menu_toggle(&mut self, cx: &mut Cx, actions: &Actions) -> ActionFlow {
        let toggled = self
            .ui
            .widget(cx, ids!(popup_root))
            .borrow::<PopupRoot>()
            .and_then(|popup| popup.toggled_event(actions));
        let Some((tag, id)) = toggled else {
            return ActionFlow::Continue;
        };
        if tag != live_id!(projection_menu) {
            return ActionFlow::Continue;
        }
        let registry = crate::folder_projection::core_registry();
        let maskable = crate::folder_projection::maskable_names(&registry);
        let Some(target) = crate::app::menus::projection_toggle_target(id, &maskable) else {
            return ActionFlow::Consumed;
        };
        let mask = crate::app::menus::apply_projection_toggle(&self.projection_mask, &target);
        self.set_projection_mask(cx, mask);
        let items = crate::app::menus::projection_menu_items(&maskable, &self.projection_mask);
        if let Some(mut popup) = self
            .ui
            .widget(cx, ids!(popup_root))
            .borrow_mut::<PopupRoot>()
        {
            popup.set_menu_items(cx, tag, items);
        }
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
                item.action
                    .downcast_ref::<crate::icon_button::IconButtonAction>()
            {
                if *tag == live_id!(history_back) {
                    direction = Some((HistoryDirection::Back, "No previous view"));
                    break;
                }
                if *tag == live_id!(history_forward) {
                    direction = Some((HistoryDirection::Forward, "No next view"));
                    break;
                }
            }
        }
        let Some((direction, problem)) = direction else {
            return ActionFlow::Continue;
        };
        self.traverse_history_with_feedback(cx, direction, problem);
        ActionFlow::Consumed
    }

    /// The one place a history traversal reports itself: the chrome pair and
    /// the mouse's back/forward buttons both land here so a dead end reads the
    /// same either way.
    pub(super) fn traverse_history_with_feedback(
        &mut self,
        cx: &mut Cx,
        direction: HistoryDirection,
        problem: &'static str,
    ) {
        if self.traverse_view_history(cx, direction) {
            self.clear_history_feedback(cx);
        } else {
            self.set_history_problem(cx, Some(problem));
        }
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
                        DockState::Flag
                    } else {
                        inspector
                    };
                    self.apply_dock_states(cx, DockState::Pinned, inspector);
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
            .and_then(|tab| tab.concept_id().map(str::to_string))
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
                        ViewLocation {
                            document,
                            anchor: ViewAnchor::None,
                        },
                        TransitionCause::UserNavigation,
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
                self.complete_session_change(cx, change.clone());
                Some(change)
            }
            Err(error) => {
                log!("{error_label}: {error:?}");
                None
            }
        }
    }

    pub(super) fn complete_session_change(
        &mut self,
        cx: &mut Cx,
        change: crate::editor_session::SessionChange,
    ) {
        self.ensure_markdown_asset_host(crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle);
        let prepared = if change.okf_changed || change.uml_changed {
            self.prepare_open_documents()
                .expect("an open editor session owns one Markdown asset host")
        } else {
            Vec::new()
        };
        self.documents
            .after_session_change(cx, &self.ui, &self.session, change.clone(), prepared);
        self.synchronize_session_change_projections(cx, &change);
        if let Some(current) = self.documents.capture_active_location(cx, &self.ui) {
            self.transition_to_location(cx, current, TransitionCause::PassiveReconciliation);
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
        if self.transition_to_location(cx, location.clone(), TransitionCause::UndoRedoReveal) {
            self.set_navigation_message(cx, None);
            self.set_history_success(cx, Some(&format!("{verb}: {label}")));
            true
        } else {
            self.set_history_problem(
                cx,
                Some(&format!(
                    "{verb}: {label}, but could not reveal {}",
                    location.document.concept_id().unwrap_or("that location")
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
                crate::doc_view::PopupRequest::Confirm { anchor, title, tag } => {
                    popup.show_at(
                        cx,
                        PopupSpec::Menu {
                            tag,
                            anchor,
                            bounds,
                            items: vec![crate::popup::base::PopupItem {
                                id: live_id!(confirm),
                                label: title,
                                icon: None,
                                danger: false,
                                enabled: true,
                                checked: None,
                            }],
                            open: MenuOpen::Popup {
                                open_marking: None,
                                max_height: None,
                            },
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
        let outcome_active_id = self.documents.active_id();
        let mut flow = ActionFlow::Continue;
        let mut edit_succeeded = if let Some(edit) = outcome.edit {
            self.apply_session_edit(cx, edit, "view edit failed")
                .is_some()
        } else {
            true
        };

        if let Some(source_edit) = outcome.source_edit {
            let Some(before_location) = self.documents.capture_active_location(cx, &self.ui) else {
                log!("source edit failed: no active document");
                return ActionFlow::Consumed;
            };
            match self
                .session
                .promote_source_edit(source_edit, before_location)
            {
                Ok((change, request)) => {
                    self.complete_session_change(cx, change);
                    match crate::markdown_analysis::run_semantic_request(request) {
                        Ok(completion) => {
                            match self.session.install_semantic_completion(completion) {
                                crate::markdown_analysis::CompletionInstall::Installed(change) => {
                                    self.complete_session_change(cx, change);
                                }
                                crate::markdown_analysis::CompletionInstall::IgnoredStale => {}
                                crate::markdown_analysis::CompletionInstall::RejectedInvariant(
                                    error,
                                ) => {
                                    log!("source semantic completion rejected: {error:?}");
                                }
                            }
                        }
                        Err(error) => {
                            log!("source semantic analysis failed: {error:?}");
                        }
                    }
                    flow = ActionFlow::Consumed;
                }
                Err(error) => {
                    log!("source edit promotion failed: {error:?}");
                    self.documents.sync_active(cx, &self.ui, &self.session);
                    edit_succeeded = false;
                    flow = ActionFlow::Consumed;
                }
            }
        }

        if let Some(intent) = outcome.navigation {
            self.handle_navigation_intent(cx, intent);
            flow = ActionFlow::Consumed;
        }

        // AFTER the navigation above: a transition to a different document
        // clears the shared markdown surfaces' search decoration, so the
        // landing's own highlights must be installed on the far side of it.
        if let Some((concept_id, target)) = outcome.reveal {
            self.mark_session_landing(cx, &concept_id, &target);
            self.pending_reveal = Some(PendingReveal { concept_id, target });
        }

        if let Some(key) = outcome.view_source {
            self.open_view_source(cx, &key);
            flow = ActionFlow::Consumed;
        }

        if let Some(request) = outcome.popup {
            self.present_view_popup(cx, request);
            flow = ActionFlow::Consumed;
        }
        if outcome.promote_active && edit_succeeded {
            self.documents.transition(
                cx,
                &self.ui,
                &self.session,
                DocumentCommand::Promote(outcome_active_id),
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
    use crate::navigation::DocumentLocator;
    use crate::popup::conflict_list::ConflictListAction;
    use crate::popup::root::PopupRootAction;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Debug, Default, PartialEq, Eq)]
    struct QuitProbe {
        reasons: Vec<QuitReason>,
        cancelled_after_failed_save: usize,
    }

    fn promotion_app() -> (Cx, App, LiveId) {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.widget_tree_mark_dirty(WidgetUid(0));
        let mut app = cx.with_vm(App::script_new_with_default);
        let source = waml::source::SourceBundle::try_from_pairs([(
            "order.md",
            "---\ntype: Runbook\ntitle: Order\n---\n# Order\n",
        )])
        .unwrap();
        app.session.replace(source).unwrap();

        app.open_view_source(&mut cx, "order");
        let source_id = app.documents.active_id();
        app.documents.transition(
            &mut cx,
            &app.ui,
            &app.session,
            DocumentCommand::Promote(source_id),
        );
        app.transition_to_location(
            &mut cx,
            ViewLocation {
                document: DocumentLocator::concept(
                    "order",
                    waml::view::surface::SurfaceId::markdown(),
                ),
                anchor: ViewAnchor::None,
            },
            TransitionCause::UserNavigation,
        );

        let primary_id = app.documents.active_id();
        assert!(app.documents.active_tab().unwrap().preview);
        (cx, app, primary_id)
    }

    #[test]
    fn view_outcome_promotes_entry_active_preview_after_source_navigation() {
        let (mut cx, mut app, primary_id) = promotion_app();

        app.apply_view_outcome(
            &mut cx,
            crate::doc_view::ViewOutcome {
                promote_active: true,
                view_source: Some("order".into()),
                ..Default::default()
            },
        );

        assert_eq!(app.documents.active_id(), primary_id);
        let primary = app
            .documents
            .tabs()
            .iter()
            .find(|tab| tab.id == primary_id)
            .unwrap();
        assert!(!primary.preview);
    }

    #[test]
    fn failed_view_edit_keeps_active_preview_unpinned() {
        let (mut cx, mut app, primary_id) = promotion_app();
        let edit = crate::document::EditIntent {
            edit: waml::edit::PendingEdit::new(waml::uml::Batch(vec![
                waml::uml::Op::AttributeRemove {
                    node: "missing".into(),
                    name: "missing".into(),
                },
            ])),
            label: "Broken".into(),
            merge_key: None,
            after_location: None,
        };

        app.apply_view_outcome(
            &mut cx,
            crate::doc_view::ViewOutcome {
                edit: Some(edit),
                promote_active: true,
                ..Default::default()
            },
        );

        let primary = app
            .documents
            .tabs()
            .iter()
            .find(|tab| tab.id == primary_id)
            .unwrap();
        assert!(primary.preview);
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
                ObserverHandler::Inspector,
                ObserverHandler::FindStrip,
            ]
        );
    }

    #[test]
    fn exclusive_handlers_keep_the_existing_priority() {
        assert_eq!(
            EXCLUSIVE_ORDER,
            [
                ExclusiveHandler::TreeContextMenu,
                ExclusiveHandler::TreeNavigation,
                ExclusiveHandler::TreeProjectionMenu,
                ExclusiveHandler::ProjectionMenuToggle,
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

    #[test]
    fn conflict_delete_maps_to_place_rm() {
        let action = ConflictListAction::Delete {
            subject: "order".to_string(),
            reference: "payment-gateway".to_string(),
        };
        let op = place_rm_for("dia", &action);
        assert_eq!(
            op,
            Some(waml::uml::Op::PlacementRemove {
                diagram: "dia".to_string(),
                subject_slug: "order".to_string(),
                reference_slug: "payment-gateway".to_string(),
            })
        );
    }

    #[test]
    fn conflict_focus_never_maps_to_an_op() {
        let action = ConflictListAction::Focus {
            subject: "order".to_string(),
            reference: "payment-gateway".to_string(),
        };
        assert_eq!(place_rm_for("dia", &action), None);
        assert_eq!(place_rm_for("dia", &ConflictListAction::None), None);
    }

    // End-to-end at the ops layer (no live `Cx`/`App` needed): the mapped
    // `Op::PlaceRm` removes ONLY the targeted placement from the re-serialized
    // bundle, leaving an unrelated one intact. The solver's dropped/
    // conflicts_with reporting is already covered by `waml::edit`
    // tests and `scene.rs`'s `project_conflicts` tests.
    #[test]
    fn conflict_delete_removes_only_the_targeted_placement() {
        let source = waml::source::SourceBundle::try_from_pairs([(
            "shop/dia.md".to_string(),
            "---\ntype: uml.ClassDiagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Layout\n\
             - [Order](./order.md) left of [PaymentGateway](./payment-gateway.md)\n\
             - [Customer](./customer.md) below [Order](./order.md)\n"
                .to_string(),
        )])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source.clone(), None, 1).unwrap();
        let action = ConflictListAction::Delete {
            subject: "order".to_string(),
            reference: "payment-gateway".to_string(),
        };
        let op = place_rm_for("dia", &action).expect("Delete maps to an Op");
        let out = waml::edit::EditBatch::lower(
            &waml::uml::Batch(vec![op]),
            waml::edit::EditContext {
                source: &source,
                okf_analysis: prepared.okf(),
                session_revision: prepared.revision(),
                uml: prepared.uml(),
            },
        )
        .unwrap();
        let markdown = out.document_by_concept_id("shop/dia").unwrap().text();
        assert!(
            !markdown.contains("left of"),
            "the deleted placement is gone: {markdown}"
        );
        assert!(
            markdown.contains("below"),
            "the OTHER placement survives: {markdown}"
        );
    }
}
