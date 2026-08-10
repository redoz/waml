use super::*;

impl App {
    pub(super) fn rehydrate_for_event(&mut self, cx: &mut Cx, event: &Event) {
        // Theme live-edit: the framework has already re-run `script_mod` and
        // `Apply::Reload`ed the widget tree (wiping imperatively-pushed
        // content) *before* this `Event::LiveEdit` lands, so re-hydrate now.
        if let Event::LiveEdit = event {
            self.rehydrate(cx);
        }
    }

    pub(super) fn update_fps_meter(&mut self, cx: &mut Cx, event: &Event) {
        // Logo FPS-heat meter: `App` forwards every raw event to the meter,
        // which owns all interaction-span detection (primary press/release plus
        // the mouse-wheel scroll tail) and framerate sampling. When it reports a
        // change, push the fresh colour/strength to the top-bar logo. This is
        // app-wide (not hit-tested), so it fires no matter which child widget
        // captures the drag, and is a no-op on the splash instance.
        if self.fps_meter.on_event(cx, event) {
            if let Some(mut logo) = self
                .ui
                .widget(cx, ids!(logo))
                .borrow_mut::<crate::logo::LogoMark>()
            {
                logo.set_heat(cx, self.fps_meter.color(), self.fps_meter.strength());
            }
        }
    }

    pub(super) fn handle_global_shortcuts(&mut self, cx: &mut Cx, event: &Event) -> bool {
        // The mouse's fourth/fifth buttons drive view history, the same pair
        // the chrome's arrows drive. Handled app-wide rather than hit-tested
        // (no widget owns a thumb button), and consumed so no hovered widget
        // reads the press as a click. `MouseDown` only -- acting on the
        // matching `MouseUp` too would traverse twice per press.
        if let Event::MouseDown(me) = event {
            let direction = if me.button.is_back() {
                Some((HistoryDirection::Back, "No previous view"))
            } else if me.button.is_forward() {
                Some((HistoryDirection::Forward, "No next view"))
            } else {
                None
            };
            if let Some((direction, problem)) = direction {
                // Gate on the same condition that shows the chrome pair: with
                // no document open there is no history to speak about, so stay
                // silent rather than posting a dead-end message.
                if self.documents.active_tab().is_some() {
                    self.traverse_history_with_feedback(cx, direction, problem);
                }
                return true;
            }
        }

        // Model history owns the standard platform chords, even while an
        // editor has focus. Returning here keeps focused widgets from also
        // applying a competing local undo/redo when a stack is empty.
        if let Event::KeyDown(ke) = event {
            let macos = matches!(cx.os_type(), OsType::Macos);
            if let Some(command) =
                crate::shortcuts::history_command_for(ke.key_code, ke.modifiers, macos)
            {
                match command {
                    crate::shortcuts::HistoryCommand::Undo => {
                        self.perform_undo(cx);
                    }
                    crate::shortcuts::HistoryCommand::Redo => {
                        self.perform_redo(cx);
                    }
                }
                return true;
            }
        }

        // Search commands (Task 11) own their chords app-wide too, the same
        // as history above -- a command palette has to be reachable even
        // while an editor has focus.
        if let Event::KeyDown(ke) = event {
            let macos = matches!(cx.os_type(), OsType::Macos);
            if let Some(command) =
                crate::shortcuts::search_command_for(ke.key_code, ke.modifiers, macos)
            {
                match command {
                    crate::shortcuts::SearchCommand::OpenPalette => {
                        self.open_palette(cx);
                    }
                    crate::shortcuts::SearchCommand::OpenFindStrip => {
                        self.open_find_strip(cx);
                    }
                    // F3/Shift+F3 (Task 14): a live document-scoped find
                    // session (Task 13, the strip is open) takes precedence
                    // over the bundle-wide session (spec §Search session's
                    // precedence rule) -- `find` IS that "strip is open"
                    // signal (see its own doc comment).
                    crate::shortcuts::SearchCommand::NextHit => {
                        if self.find.is_some() {
                            self.step_find(cx, true);
                        } else {
                            self.step_session(cx, true);
                        }
                    }
                    crate::shortcuts::SearchCommand::PreviousHit => {
                        if self.find.is_some() {
                            self.step_find(cx, false);
                        } else {
                            self.step_session(cx, false);
                        }
                    }
                }
                return true;
            }
        }

        // Tool-dock hotkeys (V/N/C): global, visual-only mode switch. Only
        // live while nothing holds key focus, so they don't fight with the
        // inspector's inline-edit text entry.
        if let Event::KeyDown(ke) = event {
            if cx.key_focus() == Area::Empty {
                let letter = match ke.key_code {
                    KeyCode::KeyV => Some('V'),
                    KeyCode::KeyN => Some('N'),
                    KeyCode::KeyC => Some('C'),
                    _ => None,
                };
                if let Some(tool) = letter.and_then(crate::tool_dock::tool_for_hotkey) {
                    if let Some(mut dock) = self
                        .ui
                        .widget(cx, ids!(tool_dock))
                        .borrow_mut::<crate::tool_dock::ToolDock>()
                    {
                        dock.set_active(cx, tool);
                    }
                    self.sync_statusbar(cx);
                }
                // Shortcuts overlay (U8): `?` opens it, `Escape` closes it --
                // same global-hotkey guard (nothing holding key focus) as
                // the tool-dock modes above.
                match ke.key_code {
                    KeyCode::Slash => self.toggle_shortcuts_overlay(cx),
                    KeyCode::Escape => self.close_page_overlays(cx),
                    // Theme toggle: persist the flip, then request a live-edit.
                    // The reload re-runs `script_mod` (repointing `mod.atlas`)
                    // and `Apply::Reload`s the tree; `Event::LiveEdit` then
                    // lands in `rehydrate` to re-push the wiped content.
                    KeyCode::KeyT => {
                        let mode = crate::config::toggle_theme();
                        log!("theme toggled -> {mode:?}");
                        cx.request_live_edit();
                    }
                    _ => {}
                }
            }
        }

        false
    }

    pub(super) fn handle_escape_event(&mut self, cx: &mut Cx, event: &Event) {
        if matches!(event, Event::KeyDown(ke) if ke.key_code == KeyCode::Escape) {
            // A live bundle-wide search session (Task 14, spec §Search
            // session) consumes the FIRST Esc and ends there -- the existing
            // per-view escape below waits for a second press, the same
            // "peel one layer at a time" shape the shortcuts overlay and
            // properties page already follow.
            if self.session_search.is_some() {
                self.end_session_search(cx);
                return;
            }
            // Escape always returns an active diagram tab from its properties page
            // to the canvas, including while one of the property fields has focus.
            self.documents.on_active_escape(cx, &self.ui);
            self.session.break_edit_merge_group();
        }
    }

    pub(super) fn handle_persistence_event(&mut self, cx: &mut Cx, event: &Event) {
        // Debounced save: the document has sat unchanged for a beat, so persist
        // it through whichever backing this build has.
        if should_flush_save(event) {
            // A graceful quit can be cancelled, so keep the app alive and retry
            // after surfacing an error. Shutdown cannot be cancelled and remains
            // a final best-effort write for forced/platform teardown paths.
            let retry_on_error = matches!(event, Event::QuitRequested(_));
            let result = self.save_or_retry(cx, retry_on_error);
            if result.is_ok() {
                self.refresh_search_after_save();
            }
            prevent_quit_after_failed_save(event, &result);
        } else if self.save_timer.is_event(event).is_some() && self.save_or_retry(cx, true).is_ok()
        {
            self.refresh_search_after_save();
        }
    }

    /// After a successful save flush, refresh the text index for exactly the
    /// documents the current snapshot names as affected -- spec §Index
    /// lifecycle's per-document update, not a full rebuild.
    fn refresh_search_after_save(&mut self) {
        let snapshot = self.session.snapshot();
        for &document in snapshot.affected_documents.iter() {
            let Some(path) = self.session.document_path(document) else {
                continue;
            };
            self.search.refresh_document(
                path.as_str(),
                &snapshot.source,
                &snapshot.okf_analysis,
                &snapshot.uml_analysis,
            );
        }
    }

    pub(super) fn route_popup_event(&mut self, cx: &mut Cx, event: &Event) {
        // Single popup seam: light-dismiss + active-surface routing + emission.
        let popup_was_open = self
            .ui
            .widget(cx, ids!(popup_root))
            .borrow::<PopupRoot>()
            .map(|root| root.is_open())
            .unwrap_or(false);
        if let Some(mut pr) = self
            .ui
            .widget(cx, ids!(popup_root))
            .borrow_mut::<PopupRoot>()
        {
            pr.route(cx, event);
        }
        self.route_narrow_dock_pointer(cx, event, popup_was_open);
    }

    pub(super) fn handle_draw_restores(&mut self, cx: &mut Cx, event: &Event) {
        if matches!(event, Event::Draw(_)) {
            self.apply_pending_fragment(cx);
            self.apply_pending_reveal(cx);
            self.apply_pending_anchor_restore(cx);
        }
    }

    pub(super) fn override_caption_drag_query(&mut self, cx: &mut Cx, event: &Event) {
        // Two-way override, both directions off the same query.
        //
        // Client-ward: the Window widget marks the entire caption bar (minus the
        // window min/max/close buttons) as an OS window-drag region, which
        // swallows pointer events over the interactive controls living there --
        // clicks and hover never reach the widget.
        //
        // Caption-ward: `tab_row` moved OUT of the caption and into the body
        // (see `app.rs`), so its empty gutter -- the strip past the last tab
        // card, the slivers between cards -- defaults to client area and no
        // longer drags the window. Windows dispatches this query for the whole
        // window, not just the non-client band (`win32_window.rs`, WM_NCHITTEST
        // falls through to the app for every non-resize-edge hit), so answering
        // `Caption` there gives the drag surface back. With the caption down to
        // one 34px row crowded by the logo, burger, model name and window
        // buttons, that gutter is most of what is left to grab.
        //
        // This runs after `ui.handle_event`, so both `set`s override the
        // Window's own answer (last write wins before the platform reads it).
        if let Event::WindowDragQuery(dq) = event {
            let tabs = self.ui.widget(cx, ids!(doc_tabs));
            let over_tab = tabs
                .borrow::<crate::doc_tabs::DocTabs>()
                .map(|tabs| tabs.hits_any_tab(dq.abs))
                .unwrap_or(false);
            // The tab row's own rect, minus everything interactive in it: `[T]`,
            // the history pair and the tab cards are all handled below, so what
            // is left of this rect is the draggable gutter.
            let over_tab_row = self
                .ui
                .widget(cx, ids!(tab_row))
                .area()
                .rect(cx)
                .contains(dq.abs);
            // The logo also lives in the caption drag region; without this
            // the logo never gets hover/click (the whole feature is dead).
            let over_logo = self
                .ui
                .widget(cx, ids!(logo))
                .borrow::<crate::logo::LogoMark>()
                .map(|l| l.drawn_rect().contains(dq.abs))
                .unwrap_or(false);
            // The caption burger lives in the drag region too; treat its
            // rect as client area so clicks reach the widget.
            let over_btn = self
                .ui
                .widget(cx, ids!(menu_btn))
                .as_icon_button()
                .rect(cx)
                .contains(dq.abs);
            // The tab row's own controls: the tree-column toggle and the
            // view-history pair. They are client area by default now, but the
            // gutter rule below hands the rest of the row back to the OS, so
            // each one has to be named here or it goes with it and the control
            // is dead.
            let over_row_btn = [
                ids!(tree_btn),
                ids!(history_back_btn),
                ids!(history_forward_btn),
            ]
            .into_iter()
            .any(|id| {
                let btn = self.ui.widget(cx, id);
                btn.visible() && btn.as_icon_button().rect(cx).contains(dq.abs)
            });
            // Breadcrumb segments and the right-dock button share the header's
            // live rect. Keep that interactive row in client space.
            let document_header = self.ui.widget(cx, ids!(document_header));
            let over_document_header = document_header
                .borrow::<crate::document_header::DocumentHeader>()
                .map(|header| {
                    header.visible_height() > 0.0
                        && document_header.area().rect(cx).contains(dq.abs)
                })
                .unwrap_or(false);
            // While the drop-down is open, treat the WHOLE caption as client
            // area. The header is otherwise an OS window-drag region, so a press
            // there starts a drag and never reaches the app as a click -- the
            // one spot the menu wouldn't dismiss from. Client-izing it turns a
            // header press into a normal MouseDown, which the menu's
            // outside-click path dismisses on.
            let menu_open = self
                .ui
                .widget(cx, ids!(popup_root))
                .borrow::<PopupRoot>()
                .map(|pr| pr.is_open())
                .unwrap_or(false);
            if over_tab
                || over_logo
                || over_btn
                || over_row_btn
                || over_document_header
                || menu_open
            {
                dq.response.set(WindowDragQueryResponse::Client);
            } else if over_tab_row {
                // Nothing interactive under the cursor, but we are inside the
                // tab row: this is the strip's empty gutter, the window's main
                // drag handle now that the caption is one row (see the note at
                // the top of this function). Ordered AFTER the client checks so
                // a card, a control or an open menu always wins.
                dq.response.set(WindowDragQueryResponse::Caption);
            }
        }
    }

    pub(super) fn synchronize_after_event(&mut self, cx: &mut Cx) {
        // Push each panel's sampled motion slot width to its reservation
        // spacer and body width to its host every frame. NextFrame samples
        // active motion.
        self.sync_dock_slots(cx);
        // Same shape for the marker's row width: it is mounted zero-width, so
        // `App` is the only thing that knows how wide the title row is.
        self.sync_agent_row(cx);
    }
}
