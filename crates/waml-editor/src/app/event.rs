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
        // Escape always returns an active diagram tab from its properties page
        // to the canvas, including while one of the property fields has focus.
        if matches!(event, Event::KeyDown(ke) if ke.key_code == KeyCode::Escape) {
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
            prevent_quit_after_failed_save(event, &result);
        } else if self.save_timer.is_event(event).is_some() {
            let _ = self.save_or_retry(cx, true);
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
            self.apply_pending_anchor_restore(cx);
        }
    }

    pub(super) fn override_caption_drag_query(&mut self, cx: &mut Cx, event: &Event) {
        // The Window widget marks the entire caption bar (minus the window
        // min/max/close buttons) as an OS window-drag region, which swallows
        // pointer events over the doc-tab strip living there -- tab clicks and
        // hover never reach the widget. Re-answer the drag query as `Client`
        // over the tab strip so it behaves as a normal interactive area. This
        // runs after `ui.handle_event`, so this `set` overrides the Window's
        // `Caption` answer (last write wins before the platform reads it).
        if let Event::WindowDragQuery(dq) = event {
            let over_tab = self
                .ui
                .widget(cx, ids!(doc_tabs))
                .borrow::<crate::doc_tabs::DocTabs>()
                .map(|tabs| tabs.hits_any_tab(dq.abs))
                .unwrap_or(false);
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
            // Same for the tab row's tree-column toggle: it sits in the caption
            // drag region, so without this its clicks become window drags and
            // the toggle is dead.
            let over_tree_btn = self
                .ui
                .widget(cx, ids!(tree_btn))
                .as_icon_button()
                .rect(cx)
                .contains(dq.abs);
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
                || over_tree_btn
                || over_document_header
                || menu_open
            {
                dq.response.set(WindowDragQueryResponse::Client);
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
