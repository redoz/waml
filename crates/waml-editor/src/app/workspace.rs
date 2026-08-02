use super::*;

/// How long the document has to sit unchanged before `mark_dirty` turns into a
/// `save`. Sized for a pause in editing, not for the tail of a single gesture:
/// a save is a full deflate of the bundle, so coalescing a run of related edits
/// into one is worth more than persisting each of them promptly.
const SAVE_DEBOUNCE_SECS: f64 = 3.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum BackingTransitionError {
    Save(String),
    Load(String),
}

/// Flush the current document before loading its replacement. Keeping this
/// ordering explicit prevents a reopen of the same directory from observing
/// stale pre-save source.
pub(super) fn replace_after_save<T, S, L>(save: S, load: L) -> Result<T, BackingTransitionError>
where
    S: FnOnce() -> Result<(), String>,
    L: FnOnce() -> Result<T, String>,
{
    save().map_err(BackingTransitionError::Save)?;
    load().map_err(BackingTransitionError::Load)
}

pub(super) fn close_after_save<T, S>(state: &mut T, save: S) -> Result<(), String>
where
    T: Default,
    S: FnOnce(&T) -> Result<(), String>,
{
    save(state)?;
    *state = T::default();
    Ok(())
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct SaveFeedback {
    save_error: Option<String>,
}

impl SaveFeedback {
    pub(super) fn finish_save(&mut self, result: &Result<(), String>) {
        match result {
            Ok(()) => self.save_error = None,
            Err(error) => self.save_error = Some(error.clone()),
        }
    }

    pub(super) fn save_error(&self) -> Option<&str> {
        self.save_error.as_deref()
    }

    pub(super) fn opened_replacement_bundle(&mut self) {
        *self = Self::default();
    }
}

pub(super) fn should_flush_save(event: &Event) -> bool {
    matches!(event, Event::Shutdown | Event::QuitRequested(_))
}

pub(super) fn prevent_quit_after_failed_save(event: &Event, result: &Result<(), String>) -> bool {
    if result.is_err() {
        if let Event::QuitRequested(quit) = event {
            quit.handle();
            return true;
        }
    }
    false
}

pub(super) fn restore_markdown_asset_host_after_open(
    installed: &mut Option<crate::markdown_hosts::SharedMarkdownAssetHost>,
    previous: Option<crate::markdown_hosts::SharedMarkdownAssetHost>,
    opened: bool,
) -> bool {
    if !opened {
        *installed = previous;
    }
    opened
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(super) fn browser_save_fragment(ticket: &SaveTicket) -> (String, SaveCompletion) {
    (
        format!("#{}", waml::share::encode_source(&ticket.snapshot.source)),
        SaveCompletion {
            revision: ticket.revision,
            history_state: ticket.history_state,
            result: Ok(()),
        },
    )
}

impl App {
    pub(super) fn ensure_markdown_asset_host(
        &mut self,
        policy: crate::markdown_hosts::MarkdownAssetPolicy,
    ) -> crate::markdown_hosts::SharedMarkdownAssetHost {
        self.markdown_assets
            .get_or_insert_with(|| crate::markdown_hosts::EditorMarkdownAssetHost::shared(policy))
            .clone()
    }

    pub(super) fn prepare_open_documents(
        &self,
    ) -> Result<Vec<Option<crate::document::OpenDocument>>, String> {
        let assets = self
            .markdown_assets
            .as_ref()
            .ok_or_else(|| "Markdown asset host is not initialized".to_string())?;
        Ok(self
            .documents
            .tabs()
            .iter()
            .map(|tab| {
                crate::documents::reopen_with_asset_host(
                    self.session.okf_analysis(),
                    self.session.uml_analysis(),
                    tab,
                    assets,
                )
            })
            .collect())
    }

    /// The open document changed. Schedules a save; does not perform one.
    ///
    /// Restarts the timer rather than extending it, so a burst of ops -- a drag
    /// that re-authors placement as it moves -- coalesces into a single save
    /// when it settles instead of one per frame.
    pub(super) fn mark_dirty(&mut self, cx: &mut Cx) {
        self.schedule_save(cx);
    }

    fn schedule_save(&mut self, cx: &mut Cx) {
        cx.stop_timer(self.save_timer);
        self.save_timer = cx.start_timeout(SAVE_DEBOUNCE_SECS);
    }

    /// Persist the open bundle, by whatever means this build has.
    ///
    /// The editor has one document model and two very different backings, so
    /// this is the seam where that difference lives; callers only ever say the
    /// document changed (`mark_dirty`), never how to store it.
    fn save(&mut self, cx: &mut Cx) -> Result<(), String> {
        let Some(ticket) = self.session.save_ticket() else {
            return Ok(());
        };
        if ticket.snapshot.source.is_empty() {
            return Err("cannot save an empty bundle".to_string());
        }
        let completion = self.save_backend(cx, &ticket)?;
        let result = completion.result.clone();
        self.session.finish_save(completion);
        result
    }

    fn sync_save_error(&mut self, cx: &mut Cx) {
        let error = self.save_feedback.save_error().map(str::to_owned);
        if let Some(mut statusbar) = self
            .ui
            .widget(cx, ids!(statusbar))
            .borrow_mut::<crate::statusbar::Statusbar>()
        {
            statusbar.set_save_error(cx, error.as_deref());
        }
    }

    pub(super) fn save_or_retry(
        &mut self,
        cx: &mut Cx,
        retry_on_error: bool,
    ) -> Result<(), String> {
        let result = self.save(cx);
        if let Err(error) = &result {
            log!("failed to save open document: {error}");
            if retry_on_error && self.session.is_dirty() {
                self.schedule_save(cx);
            }
        }
        self.save_feedback.finish_save(&result);
        self.sync_save_error(cx);
        result
    }

    #[allow(dead_code)] // Native file watching calls this ingress when enabled.
    fn replace_external_document(
        &mut self,
        cx: &mut Cx,
        document: waml::analysis::DocumentId,
        base_revision: waml_markdown_editor::syntax::DocumentRevision,
        text: String,
    ) -> Result<ExternalReplacement, String> {
        let mut replacement = self
            .session
            .replace_external(document, base_revision, text.clone())
            .map_err(|error| error.to_string())?;
        if let ExternalReplacement::Conflict { dirty_revision } = &replacement {
            debug_assert_eq!(
                self.session.snapshot().dirty_revision,
                Some(*dirty_revision)
            );
            self.save_or_retry(cx, false)?;
            replacement = self
                .session
                .replace_external(document, base_revision, text)
                .map_err(|error| error.to_string())?;
        }
        let ExternalReplacement::Installed(change) = &replacement else {
            return Ok(replacement);
        };
        let prepared = self.prepare_open_documents()?;
        self.documents.after_external_replacement(
            cx,
            &self.ui,
            &self.session,
            change.clone(),
            prepared,
        );
        self.synchronize_session_change_projections(cx, change);
        self.sync_history_controls(cx);
        Ok(replacement)
    }

    /// Browser backing: the URL fragment is the whole filesystem.
    ///
    /// Two things ride on this. A refresh restores the document, because
    /// `handle_startup` decodes exactly this fragment. And the update-check
    /// toast in `index.html` (see `scripts/inject-runtime-shell.mjs`) can build
    /// its reload URL from `location.hash` alone -- it never has to call into
    /// wasm, which makepad gives us no channel for anyway.
    ///
    /// `replace`, not push: an edit is not a navigation, and one history entry
    /// per save would make Back mean "undo some edits, sometimes".
    #[cfg(target_arch = "wasm32")]
    fn save_backend(&self, cx: &mut Cx, ticket: &SaveTicket) -> Result<SaveCompletion, String> {
        let (fragment, completion) = browser_save_fragment(ticket);
        cx.browser_update_url(&fragment, true);
        Ok(completion)
    }

    /// Native backing: atomically replace each authored file in the opened OKF
    /// directory. The helper validates bundle paths before performing writes.
    #[cfg(not(target_arch = "wasm32"))]
    fn save_backend(&self, _cx: &mut Cx, ticket: &SaveTicket) -> Result<SaveCompletion, String> {
        let Some(root) = self.open_dir.as_deref() else {
            return Err("native bundle has no opened directory".to_string());
        };
        let mut completion = crate::native_save::save_ticket_atomic(root, ticket)
            .map_err(|error| format!("failed to save OKF dir {root:?}: {error}"))?;
        completion.result = completion
            .result
            .map_err(|error| format!("failed to save OKF dir {root:?}: {error}"));
        Ok(completion)
    }

    /// Read `dir` from disk and populate the editor. Returns `false` if a
    /// required save, bundle load, asset-root policy or canonicalization step,
    /// or replacement-session analysis fails. The caller then keeps the
    /// current screen visible.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn open_dir(
        &mut self,
        cx: &mut Cx,
        dir: &Path,
        wanted_diagram: Option<&str>,
    ) -> bool {
        let next_root = dir.to_path_buf();
        let transition = replace_after_save(
            || self.save(cx),
            || {
                load::read_bundle(&next_root)
                    .map_err(|error| format!("failed to load OKF dir {next_root:?}: {error}"))
            },
        );
        let bundle = match transition {
            Ok(loaded) => loaded,
            Err(BackingTransitionError::Save(error)) => {
                self.save_feedback.finish_save(&Err(error.clone()));
                self.schedule_save(cx);
                self.sync_save_error(cx);
                log!("{error}");
                return false;
            }
            Err(BackingTransitionError::Load(error)) => {
                // Any old edits were successfully flushed before the new load
                // was attempted, so the retained backing is now clean.
                self.save_feedback.finish_save(&Ok(()));
                self.sync_save_error(cx);
                log!("{error}");
                return false;
            }
        };
        // Folder basename backs the display name when the bundle has no root
        // name of its own. `..` / drive-root degenerate to an empty basename;
        // "bundle" is the last-ditch label.
        let display_name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .filter(|n| !n.is_empty())
            .unwrap_or("bundle")
            .to_string();
        // Record this open in the recents store (best-effort; see config.rs).
        // Recents are a filesystem affordance: only an open with a path behind
        // it can be reopened later, so this stays on the `open_dir` side.
        let asset_policy = match crate::markdown_hosts::MarkdownAssetPolicy::native(&next_root) {
            Ok(policy) => policy,
            Err(error) => {
                log!("failed to canonicalize Markdown asset root {next_root:?}: {error}");
                return false;
            }
        };
        let previous_assets =
            self.markdown_assets
                .replace(crate::markdown_hosts::EditorMarkdownAssetHost::shared(
                    asset_policy,
                ));
        let opened = self.open_bundle(cx, bundle, display_name, wanted_diagram);
        if !restore_markdown_asset_host_after_open(
            &mut self.markdown_assets,
            previous_assets,
            opened,
        ) {
            return false;
        }
        self.open_dir = Some(next_root);
        let root_name = if self.session.uml_projection().path.is_empty() {
            self.open_name.as_str()
        } else {
            self.session.uml_projection().path.as_str()
        };
        crate::config::push_recent(dir, root_name);
        true
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn open_dir(
        &mut self,
        _cx: &mut Cx,
        _dir: &Path,
        _wanted_diagram: Option<&str>,
    ) -> bool {
        false
    }

    /// Populate the editor from an already-read bundle (tree, canvas, tabs,
    /// inspector, statusbar, diagram switcher). A model with zero diagrams
    /// still opens -- empty canvas, no diagram tab.
    ///
    /// Split out of `open_dir` so the web build, which has no filesystem, can
    /// open a model decoded from the URL fragment through exactly this path.
    /// Returns `false` if the replacement bundle cannot complete session
    /// analysis. Reading and source-bundle construction have already completed.
    pub(super) fn open_bundle(
        &mut self,
        cx: &mut Cx,
        files: waml::source::SourceBundle,
        display_name: String,
        wanted_diagram: Option<&str>,
    ) -> bool {
        self.ensure_markdown_asset_host(crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle);
        cx.stop_timer(self.save_timer);
        let change = match self.session.replace(files) {
            Ok(change) => change,
            Err(error) => {
                log!("failed to analyze replacement bundle: {error}");
                return false;
            }
        };
        debug_assert_eq!(change.revision, self.session.revision());
        self.save_feedback.opened_replacement_bundle();
        self.sync_save_error(cx);
        // Retain the raw bundle so drag-to-place ops can re-author `## Layout`
        // in-memory: the diagram view emits `Op::PlaceSet`, the shell applies it
        // against this bundle and rebuilds the model (see `handle_actions`).
        // Fresh model: reset the scope to the whole-model browse state.
        self.nav_state = NavState::default();

        self.open_name = display_name;

        let root_name = self
            .session
            .okf()
            .index("/")
            .and_then(|index| index.title.as_deref())
            .unwrap_or(self.open_name.as_str());
        self.ui.label(cx, ids!(model_name)).set_text(cx, root_name);

        self.refresh_nav(cx, true);

        // Start with the requested/first supported diagram, otherwise the first
        // indexed Concept. An empty bundle keeps an empty canvas and no tab.
        self.documents
            .replace_for_session(cx, &self.ui, &self.session, OpenTabs::default());
        self.view_history.reset(None);
        match crate::cli::select_initial_document(
            self.session.okf(),
            self.session.uml_projection(),
            wanted_diagram,
        ) {
            crate::cli::InitialDocument::Diagram(concept_id)
            | crate::cli::InitialDocument::Concept(concept_id) => {
                let concept_id = concept_id.to_owned();
                self.transition_document(cx, &concept_id, false);
            }
            crate::cli::InitialDocument::None => {
                log!(
                    "no documents in {:?}; opening bundle with an empty canvas",
                    self.open_name
                );
                // Empty scene draws nothing and `bounding_box` returns `None`, so
                // the fit path leaves the camera untouched (no divide-by-zero). No
                // diagram tab; the project tree was already populated by
                // `refresh_nav` above.
                if let Some(mut canvas) = self
                    .ui
                    .widget(cx, ids!(canvas))
                    .borrow_mut::<crate::canvas::ClassDiagramSurface>()
                {
                    canvas.clear(cx);
                }
            }
        }
        self.sync_document_shell(cx);
        true
    }

    /// The caption bar ships hidden and makepad only unhides it for the
    /// platforms that hand an app its own window chrome -- `sync_caption_bar_state`
    /// has arms for Windows, macOS and Wayland CSD, and an empty one for
    /// `OsType::Web`. That is the right default for a title bar, but ours also
    /// carries the logo, doc tabs, tree toggle and burger, so the browser
    /// build would come up with no navigation at all. Reveal it ourselves.
    #[cfg(target_arch = "wasm32")]
    pub(super) fn reveal_caption_bar_on_web(&mut self, cx: &mut Cx) {
        self.ui.widget(cx, ids!(caption_bar)).set_visible(cx, true);
    }

    /// Reveal the editor, hide the start screen. `main_column` is a `View`
    /// (honors `WidgetRef::set_visible`); `StartScreen` is a custom widget
    /// whose no-op default `Widget::set_visible` means we must toggle its own
    /// `visible` flag via the borrowed inherent method instead.
    pub(super) fn show_editor(&mut self, cx: &mut Cx) {
        self.editor_shown = true;
        self.ui.widget(cx, ids!(main_column)).set_visible(cx, true);
        // Caption burger + tree toggle + doc-tab strip belong to an open model.
        self.ui.widget(cx, ids!(menu_btn)).set_visible(cx, true);
        self.ui
            .widget(cx, ids!(menu_btn))
            .as_icon_button()
            .set_icon(cx, crate::icons::Icon::Menu);
        self.ui.widget(cx, ids!(tree_btn)).set_visible(cx, true);
        self.ui
            .widget(cx, ids!(tree_btn))
            .as_icon_button()
            .set_icon(cx, crate::icons::Icon::PanelLeft);
        if let Some(mut doc_tabs) = self
            .ui
            .widget(cx, ids!(doc_tabs))
            .borrow_mut::<crate::doc_tabs::DocTabs>()
        {
            doc_tabs.set_visible(cx, true);
        }
        if let Some(mut screen) = self
            .ui
            .widget(cx, ids!(start_screen))
            .borrow_mut::<crate::start_screen::StartScreen>()
        {
            screen.set_visible(cx, false);
        }
    }

    /// Re-push every imperatively-set widget content after a theme live-edit
    /// (`Event::LiveEdit` -> `Apply::Reload`) wiped it. Reads from the in-memory
    /// `model`/`tabs`, so the open project and active tab survive the toggle;
    /// the tool-dock mode (back to `Select`) and the inspector element-picker
    /// are the only bits not restored, both cheap to re-touch by hand.
    pub(super) fn rehydrate(&mut self, cx: &mut Cx) {
        // First, before the start-screen early return: the marks apply to both
        // screens.
        self.apply_agent_marks(cx);
        self.agent_row_w = 0.0; // force `sync_agent_row` to re-push after reload
        if !self.editor_shown {
            // Start screen: `show_start_screen` re-reads recents and re-shows.
            self.show_start_screen(cx);
            if let Some(mut tabs) = self
                .ui
                .widget(cx, ids!(doc_tabs))
                .borrow_mut::<crate::doc_tabs::DocTabs>()
            {
                tabs.set_narrow(cx, self.narrow);
            }
            self.dock_layout = ResponsiveDockLayout::default();
            self.tree_gap_w = -1.0;
            self.rule_overshoot = -1.0;
            self.sync_dock_slots(cx);
            return;
        }
        let root_name = if self.session.uml_projection().path.is_empty() {
            self.open_name.as_str()
        } else {
            self.session.uml_projection().path.as_str()
        };
        self.ui.label(cx, ids!(model_name)).set_text(cx, root_name);

        self.refresh_nav(cx, true);
        self.documents.sync_active(cx, &self.ui, &self.session);
        self.sync_document_shell(cx);
        self.show_editor(cx);
        if let Some(mut tabs) = self
            .ui
            .widget(cx, ids!(doc_tabs))
            .borrow_mut::<crate::doc_tabs::DocTabs>()
        {
            tabs.set_narrow(cx, self.narrow);
        }
        self.dock_layout = ResponsiveDockLayout::default();
        self.tree_gap_w = -1.0;
        self.rule_overshoot = -1.0;
        self.sync_dock_slots(cx);
    }

    /// Flush the current backing before closing it. A failed flush leaves the
    /// editor, source snapshots, and retry timer intact.
    pub(super) fn close_model(&mut self, cx: &mut Cx) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        let result = {
            let root = self.open_dir.as_deref();
            close_after_save(&mut self.session, |session| {
                let Some(ticket) = session.save_ticket() else {
                    return Ok(());
                };
                let root =
                    root.ok_or_else(|| "native bundle has no opened directory".to_string())?;
                crate::native_save::save_ticket_atomic(root, &ticket)
                    .map_err(|error| format!("failed to save OKF dir {root:?}: {error}"))?
                    .result
                    .map_err(|error| format!("failed to save OKF dir {root:?}: {error}"))
            })
        };

        #[cfg(target_arch = "wasm32")]
        let result = close_after_save(&mut self.session, |session| {
            if let Some(ticket) = session.save_ticket() {
                cx.browser_update_url(
                    &format!("#{}", waml::share::encode_source(&ticket.snapshot.source)),
                    true,
                );
            }
            Ok(())
        });

        self.save_feedback.finish_save(&result);
        if let Err(error) = &result {
            log!("failed to close open document: {error}");
            self.schedule_save(cx);
            self.sync_save_error(cx);
            return false;
        }

        cx.stop_timer(self.save_timer);
        self.open_dir = None;
        self.markdown_assets = None;
        self.sync_save_error(cx);
        self.show_start_screen(cx);
        true
    }

    /// Load recents into the start screen and reveal it, hiding the editor.
    pub(super) fn show_start_screen(&mut self, cx: &mut Cx) {
        self.start_recents = crate::config::recents();
        let rows: Vec<crate::start_screen::RecentRow> = self
            .start_recents
            .iter()
            // Keep the complete config list in `start_recents`; `StartScreen`
            // caps only its rendered copy to the first five. Screen indices
            // therefore still map 1:1 to this backing list.
            .map(|r| crate::start_screen::RecentRow {
                title: r.title().to_string(),
                path: r.path().display().to_string(),
                when: format_opened(r.opened_at()),
                pinned: r.pinned(),
            })
            .collect();
        if let Some(mut screen) = self
            .ui
            .widget(cx, ids!(start_screen))
            .borrow_mut::<crate::start_screen::StartScreen>()
        {
            screen.set_recents(cx, rows);
            screen.set_visible(cx, true);
        }
        self.ui.widget(cx, ids!(main_column)).set_visible(cx, false);
        // No open model on the start screen: hide burger + tree toggle +
        // doc-tab strip, and drop the editor's tab state so a re-open starts
        // clean rather than inheriting the closed model's tabs (open_dir
        // rebuilds from scratch).
        self.ui.widget(cx, ids!(menu_btn)).set_visible(cx, false);
        self.ui.widget(cx, ids!(tree_btn)).set_visible(cx, false);
        // Replacing the host with no tabs applies hidden chrome through the
        // same transition path used when the final document closes.
        // Clear the stale model title: the caption bar keeps drawing (logo +
        // name) even with no model open, so a leftover name reads as if the
        // closed model were still loaded.
        self.ui.label(cx, ids!(model_name)).set_text(cx, "");
        self.documents
            .replace_for_session(cx, &self.ui, &self.session, OpenTabs::default());
        self.sync_document_shell(cx);
        if let Some(mut doc_tabs) = self
            .ui
            .widget(cx, ids!(doc_tabs))
            .borrow_mut::<crate::doc_tabs::DocTabs>()
        {
            doc_tabs.set_visible(cx, false);
        }
        self.editor_shown = false;
    }

    /// Prompt for a model directory via the native folder picker and open it.
    /// Shared by the start screen's "Open a model" and the burger's "Open
    /// model". Blocks the window while modal, as OS file dialogs do; Cancel
    /// yields `None` (no-op); a non-model dir makes `open_dir` log + return
    /// false, so we stay put.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn open_model_via_picker(&mut self, cx: &mut Cx) {
        if let Some(dir) = rfd::FileDialog::new()
            .set_title("Open a model")
            .pick_folder()
        {
            if self.open_dir(cx, &dir, None) {
                self.show_editor(cx);
            }
        }
    }

    /// The browser has no directory picker and no filesystem to point one at,
    /// so the web build gets its model from the URL fragment instead. Keeping
    /// the method (rather than cfg-ing out every call site) leaves the start
    /// screen and burger wiring identical across targets.
    #[cfg(target_arch = "wasm32")]
    pub(super) fn open_model_via_picker(&mut self, _cx: &mut Cx) {}

    /// The main window's client rect in main-window coords (popup clip bounds).
    pub(super) fn window_bounds(&mut self, cx: &mut Cx) -> Rect {
        let sz = self.ui.window(cx, ids!(main_window)).get_inner_size(cx);
        Rect {
            pos: dvec2(0.0, 0.0),
            size: dvec2(sz.x, sz.y),
        }
    }
}

/// Humanize a recent's `opened_at` (unix seconds) as a coarse relative stamp
/// ("just now", "yesterday", "3 weeks ago") for the start-screen row -- easier
/// to scan than an absolute date and self-explanatory without a header.
fn format_opened(secs: u64) -> String {
    const MIN: u64 = 60;
    const HOUR: u64 = 60 * MIN;
    const DAY: u64 = 24 * HOUR;
    const WEEK: u64 = 7 * DAY;
    const MONTH: u64 = 30 * DAY;
    const YEAR: u64 = 365 * DAY;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(secs);
    let d = now.saturating_sub(secs);

    // "1 unit ago" reads better as "a unit ago"; "an hour" is special-cased.
    fn ago(n: u64, unit: &str) -> String {
        if n == 1 {
            format!("a {unit} ago")
        } else {
            format!("{n} {unit}s ago")
        }
    }
    match d {
        0..=44 => "just now".to_string(),
        45..=89 => "a minute ago".to_string(),
        _ if d < HOUR => ago(d / MIN, "minute"),
        _ if d < 2 * HOUR => "an hour ago".to_string(),
        _ if d < DAY => ago(d / HOUR, "hour"),
        _ if d < 2 * DAY => "yesterday".to_string(),
        _ if d < WEEK => ago(d / DAY, "day"),
        _ if d < 2 * WEEK => "a week ago".to_string(),
        _ if d < MONTH => ago(d / WEEK, "week"),
        _ if d < 2 * MONTH => "a month ago".to_string(),
        _ if d < YEAR => ago(d / MONTH, "month"),
        _ if d < 2 * YEAR => "a year ago".to_string(),
        _ => ago(d / YEAR, "year"),
    }
}

/// The current URL fragment, `#` included, or `None` when the page has none.
///
/// makepad hands the browser's `location` to Rust as `OsType::Web`, refreshed
/// on every `hashchange`, so this is a plain read -- no JS interop needed.
#[cfg(target_arch = "wasm32")]
pub(super) fn web_location_hash(cx: &Cx) -> Option<String> {
    match cx.os_type() {
        makepad_widgets::makepad_platform::OsType::Web(params) if !params.hash.is_empty() => {
            Some(params.hash.clone())
        }
        _ => None,
    }
}
