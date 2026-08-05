use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PendingFragment {
    pub(super) concept_id: String,
    pub(super) fragment: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct PendingAnchorRestore {
    pub(super) document: crate::navigation::DocumentLocator,
    pub(super) anchor: ViewAnchor,
    /// Stamped from `App::anchor_restore_generation` when this restore was
    /// scheduled, so a later traversal that supersedes it can be detected.
    pub(super) generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TransitionCause {
    UserNavigation,
    UndoRedoReveal,
    HistoryTraversal,
    PassiveReconciliation,
}

impl App {
    pub(super) fn set_navigation_message(&mut self, cx: &mut Cx, message: Option<&str>) {
        if let Some(mut statusbar) = self
            .ui
            .widget(cx, ids!(statusbar))
            .borrow_mut::<crate::statusbar::Statusbar>()
        {
            statusbar.set_navigation_message(cx, message);
        }
    }

    pub(super) fn set_history_problem(&mut self, cx: &mut Cx, message: Option<&str>) {
        if let Some(mut statusbar) = self
            .ui
            .widget(cx, ids!(statusbar))
            .borrow_mut::<crate::statusbar::Statusbar>()
        {
            statusbar.set_history_problem(cx, message);
        }
    }

    pub(super) fn set_history_success(&mut self, cx: &mut Cx, message: Option<&str>) {
        if let Some(mut statusbar) = self
            .ui
            .widget(cx, ids!(statusbar))
            .borrow_mut::<crate::statusbar::Statusbar>()
        {
            statusbar.set_history_success(cx, message);
        }
    }

    pub(super) fn clear_history_feedback(&mut self, cx: &mut Cx) {
        if let Some(mut statusbar) = self
            .ui
            .widget(cx, ids!(statusbar))
            .borrow_mut::<crate::statusbar::Statusbar>()
        {
            statusbar.clear_history_feedback(cx);
        }
    }

    pub(super) fn sync_history_controls(&mut self, cx: &mut Cx) {
        let has_active_document = self.documents.active_tab().is_some();
        let can_back = self.markdown_assets.as_ref().is_some_and(|assets| {
            self.view_history
                .can_traverse(HistoryDirection::Back, |location| {
                    crate::documents::open_locator_with_asset_host(
                        self.session.okf_analysis(),
                        self.session.uml_analysis(),
                        &location.document,
                        assets,
                    )
                    .is_some()
                })
        });
        let can_forward = self.markdown_assets.as_ref().is_some_and(|assets| {
            self.view_history
                .can_traverse(HistoryDirection::Forward, |location| {
                    crate::documents::open_locator_with_asset_host(
                        self.session.okf_analysis(),
                        self.session.uml_analysis(),
                        &location.document,
                        assets,
                    )
                    .is_some()
                })
        });
        if let Some(mut header) = self
            .ui
            .widget(cx, ids!(document_header))
            .borrow_mut::<crate::document_header::DocumentHeader>()
        {
            header.set_document_active(cx, has_active_document);
        }

        let back = self.ui.widget(cx, ids!(history_back_btn));
        let forward = self.ui.widget(cx, ids!(history_forward_btn));
        if self.history_controls_visible != has_active_document {
            self.history_controls_visible = has_active_document;
            back.set_visible(cx, has_active_document);
            forward.set_visible(cx, has_active_document);
        }
        let back = back.as_icon_button();
        back.set_icon(cx, crate::icons::Icon::ArrowLeft);
        back.set_action_tag(live_id!(history_back));
        back.set_dim(cx, !can_back);
        let forward = forward.as_icon_button();
        forward.set_icon(cx, crate::icons::Icon::ArrowRight);
        forward.set_action_tag(live_id!(history_forward));
        forward.set_dim(cx, !can_forward);
    }

    #[cfg(test)]
    pub(super) fn test_history_enabled(&mut self, cx: &mut Cx) -> (bool, bool) {
        let dim_back = self
            .ui
            .widget(cx, ids!(history_back_btn))
            .as_icon_button()
            .test_dim();
        let dim_forward = self
            .ui
            .widget(cx, ids!(history_forward_btn))
            .as_icon_button()
            .test_dim();
        (!dim_back, !dim_forward)
    }

    pub(super) fn handle_navigation_intent(
        &mut self,
        cx: &mut Cx,
        intent: crate::navigation::NavigationIntent,
    ) -> bool {
        let (target, disposition) = match intent {
            crate::navigation::NavigationIntent::SourceRange {
                document,
                revision,
                range,
            } => return self.navigate_to_source_range(cx, document, revision, range),
            crate::navigation::NavigationIntent::Resolved {
                target,
                disposition,
            } => (target, disposition),
            crate::navigation::NavigationIntent::MarkdownLink {
                current_concept_id,
                href,
            } => {
                let target = match crate::navigation::resolve_link(
                    self.session.okf(),
                    &current_concept_id,
                    &href,
                ) {
                    Ok(target) => target,
                    Err(error) => {
                        self.set_navigation_message(cx, Some(&error.status_message()));
                        return false;
                    }
                };
                (target, crate::navigation::OpenDisposition::Preview)
            }
        };
        self.navigate_with(cx, target, disposition, &mut PlatformBrowser)
    }

    pub(super) fn navigate_to_source_range(
        &mut self,
        cx: &mut Cx,
        document: waml::analysis::DocumentId,
        revision: waml_markdown_editor::syntax::DocumentRevision,
        range: waml_markdown_editor::syntax::TextRange,
    ) -> bool {
        let snapshot = self.session.snapshot();
        let mapped = match snapshot.map_source_range_to_current(document, revision, range) {
            Ok(mapped) => mapped,
            Err(_) => {
                self.set_navigation_message(cx, Some("Source location is no longer available"));
                return false;
            }
        };
        let Some(syntax) = snapshot.markdown_snapshot(document) else {
            self.set_navigation_message(cx, Some("Source location is no longer available"));
            return false;
        };
        let Some(version) = snapshot.okf_analysis.catalog.document(document) else {
            self.set_navigation_message(cx, Some("Source location is no longer available"));
            return false;
        };
        let markdown =
            waml_markdown_editor::document::MarkdownDocumentSnapshot::new(syntax.clone());
        let selection = waml_markdown_editor::selection::SelectionSet::single(
            &markdown,
            waml_markdown_editor::selection::Selection::new(
                waml_markdown_editor::selection::TextPosition::new(
                    mapped.start(),
                    waml_markdown_editor::selection::Affinity::Before,
                ),
                waml_markdown_editor::selection::TextPosition::new(
                    mapped.end(),
                    waml_markdown_editor::selection::Affinity::After,
                ),
            ),
        );
        let Ok(selection) = selection else {
            self.set_navigation_message(cx, Some("Source location is no longer available"));
            return false;
        };
        let changed = self.transition_to_location(
            cx,
            ViewLocation {
                document: crate::navigation::DocumentLocator::source(waml::okf::id_of(
                    version.path().as_str(),
                )),
                anchor: ViewAnchor::Markdown {
                    fragment: None,
                    revision: syntax.revision(),
                    selection,
                    scroll: waml_markdown_editor::input::ScrollState::default(),
                },
            },
            TransitionCause::UserNavigation,
        );
        if changed {
            self.set_navigation_message(cx, None);
        } else {
            self.set_navigation_message(cx, Some("Source location is no longer available"));
        }
        changed
    }

    pub(super) fn navigate_with<B: ExternalUrlAdapter>(
        &mut self,
        cx: &mut Cx,
        target: crate::navigation::NavigationTarget,
        disposition: crate::navigation::OpenDisposition,
        browser: &mut B,
    ) -> bool {
        match target {
            crate::navigation::NavigationTarget::Document {
                concept_id,
                fragment,
            } => {
                if self.session.okf().concept(&concept_id).is_none() {
                    self.set_navigation_message(
                        cx,
                        Some(&format!("Document not found: {concept_id}")),
                    );
                    return false;
                }
                self.pending_fragment = fragment.map(|fragment| PendingFragment {
                    concept_id: concept_id.clone(),
                    fragment,
                });
                let anchor = self
                    .pending_fragment
                    .as_ref()
                    .and_then(|pending| {
                        let snapshot = self.session.snapshot();
                        crate::source_view::SourceView::resolve_document(&snapshot, &concept_id)
                            .map(|(_, syntax)| {
                                ViewAnchor::markdown_start(
                                    syntax.revision(),
                                    Some(pending.fragment.clone()),
                                    waml_markdown_editor::input::ScrollState::default(),
                                )
                            })
                    })
                    .unwrap_or(ViewAnchor::None);
                let changed = self.transition_to_location(
                    cx,
                    ViewLocation {
                        document: crate::navigation::DocumentLocator::primary(&concept_id),
                        anchor,
                    },
                    TransitionCause::UserNavigation,
                );
                if disposition == crate::navigation::OpenDisposition::Persistent {
                    let id = self.documents.active_id();
                    self.documents.transition(
                        cx,
                        &self.ui,
                        &self.session,
                        DocumentCommand::Promote(id),
                    );
                }
                cx.redraw_all();
                self.set_navigation_message(cx, None);
                changed
            }
            crate::navigation::NavigationTarget::Directory { address } => {
                // Opens the folder's own view -- the tree's fold/unfold
                // affordance moved to the chevron (`tree_panel.rs`'s
                // chevron-vs-row-body split); a `Directory` navigation
                // target now always means "open", never "toggle".
                let Some(document) = crate::documents::open_folder(
                    self.session.okf_analysis(),
                    &address,
                    self.chain_limits,
                    self.view_mode,
                ) else {
                    self.set_navigation_message(cx, Some(&format!("Folder not found: {address}")));
                    return false;
                };
                let tab_id = document.tab_id;
                self.documents.transition(
                    cx,
                    &self.ui,
                    &self.session,
                    DocumentCommand::Open {
                        document,
                        persistent: disposition == crate::navigation::OpenDisposition::Persistent,
                    },
                );
                // `transition`'s own return reports whether the tab STATE
                // changed, which is false for re-navigating to an already
                // active preview tab (matches the `Document` arm's own
                // same-document short circuit in `transition_to_location`) --
                // success here means "landed on the folder's tab", not "tabs
                // changed".
                let opened = self.documents.active_id() == tab_id;
                if opened {
                    // The `Document` arm reaches this through
                    // `transition_to_location`; a folder open bypasses that
                    // path entirely, so without this the shell projections --
                    // including the tree panel's selected row -- keep pointing
                    // at whatever document was active before the folder.
                    self.sync_document_shell(cx);
                    cx.redraw_all();
                    self.set_navigation_message(cx, None);
                }
                opened
            }
            crate::navigation::NavigationTarget::ExternalUrl(url) => match browser.open(cx, &url) {
                Ok(()) => {
                    self.set_navigation_message(cx, None);
                    true
                }
                Err(error) => {
                    self.set_navigation_message(cx, Some(&format!("Could not open link: {error}")));
                    false
                }
            },
        }
    }

    pub(super) fn apply_pending_fragment(&mut self, cx: &mut Cx) {
        let Some(pending) = self.pending_fragment.as_ref() else {
            return;
        };
        if self
            .documents
            .active_tab()
            .map_or(true, |tab| tab.concept_id != pending.concept_id)
        {
            return;
        }
        let fragment = pending.fragment.clone();
        let found =
            self.documents
                .scroll_active_to_fragment(cx, &self.ui, &self.session, &fragment);
        self.pending_fragment = None;
        if found {
            if let Some(current) = self.documents.capture_active_location(cx, &self.ui) {
                self.view_history.refresh_current(current);
            }
            self.set_navigation_message(cx, None);
        } else {
            self.set_navigation_message(cx, Some(&format!("Section not found: {fragment}")));
        }
    }

    pub(super) fn apply_pending_anchor_restore(&mut self, cx: &mut Cx) {
        let Some(pending) = self.pending_anchor_restore.take() else {
            return;
        };
        if self
            .documents
            .active_tab()
            .map_or(true, |tab| tab.locator() != pending.document)
        {
            return;
        }
        let _ = self
            .documents
            .restore_active_anchor(cx, &self.ui, &self.session, &pending.anchor);
        // A newer traversal may have scheduled its own pending restore (and
        // already refreshed history for the entry it targets) while this one
        // was still deferred. Applying this stale restore's anchor is still
        // correct for the view itself, but refreshing history with it would
        // clobber the newer entry with this superseded generation's anchor.
        if pending.generation != self.anchor_restore_generation {
            return;
        }
        if let Some(current) = self.documents.capture_active_location(cx, &self.ui) {
            self.view_history.refresh_current(current);
        }
    }

    /// Open or focus a document through the shared preview slot. All callers
    /// use this path so replacement cleanup and view/chrome synchronization
    /// stay identical for classifiers and diagrams.
    pub(super) fn transition_document(
        &mut self,
        cx: &mut Cx,
        concept_id: &str,
        persistent: bool,
    ) -> bool {
        let changed = self.transition_to_location(
            cx,
            ViewLocation {
                document: crate::navigation::DocumentLocator::primary(concept_id),
                anchor: ViewAnchor::None,
            },
            TransitionCause::UserNavigation,
        );
        if persistent && changed {
            let id = self.documents.active_id();
            self.documents
                .transition(cx, &self.ui, &self.session, DocumentCommand::Promote(id));
        }
        changed
    }

    /// Open `key`'s raw markdown source through the shared history-aware
    /// transition path (spec §5.2). Factored out of the node context menu's
    /// `ViewSource` handler so a read-only surface with no context menu (the
    /// behavior canvas, Task 9) can reach the same code path from its own
    /// selection affordance.
    pub(super) fn open_view_source(&mut self, cx: &mut Cx, key: &str) {
        self.transition_to_location(
            cx,
            ViewLocation {
                document: crate::navigation::DocumentLocator::source(key),
                anchor: ViewAnchor::None,
            },
            TransitionCause::UserNavigation,
        );
    }

    pub(super) fn transition_to_location(
        &mut self,
        cx: &mut Cx,
        location: ViewLocation,
        cause: TransitionCause,
    ) -> bool {
        let departing = self.documents.capture_active_location(cx, &self.ui);
        if matches!(cause, TransitionCause::UserNavigation)
            && matches!(location.anchor, ViewAnchor::None)
            && departing
                .as_ref()
                .is_some_and(|current| current.document == location.document)
        {
            self.session.break_edit_merge_group();
            self.view_history
                .refresh_current(departing.expect("same-document location was checked"));
            self.sync_history_controls(cx);
            return true;
        }
        if matches!(cause, TransitionCause::HistoryTraversal) {
            // Skip the refresh when a restore for the departing document is
            // still pending: the departing view's captured anchor is
            // pre-restore stale (the deferred restore from the first
            // traversal has not applied yet), and refreshing history with it
            // would corrupt the entry that restore is about to write.
            let departing_is_pending_restore = self
                .pending_anchor_restore
                .as_ref()
                .zip(departing.as_ref())
                .is_some_and(|(pending, departing)| pending.document == departing.document);
            if !departing_is_pending_restore {
                if let Some(departing) = departing.clone() {
                    self.view_history.refresh_current(departing);
                }
            }
        }
        let assets = self
            .ensure_markdown_asset_host(crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle);
        if !self.documents.restore_location_with_asset_host(
            cx,
            &self.ui,
            &self.session,
            &location,
            &assets,
        ) {
            return false;
        }
        if matches!(
            cause,
            TransitionCause::HistoryTraversal | TransitionCause::UndoRedoReveal
        ) && !matches!(location.anchor, ViewAnchor::None)
        {
            self.anchor_restore_generation = self.anchor_restore_generation.wrapping_add(1);
            self.pending_anchor_restore = Some(PendingAnchorRestore {
                document: location.document.clone(),
                anchor: location.anchor.clone(),
                generation: self.anchor_restore_generation,
            });
            cx.redraw_all();
        }
        self.sync_document_shell(cx);
        // Re-submit the complete composed tree after the selection change.
        // Makepad's immediate-mode `FileTree` otherwise retains only the rows
        // visited before its clicked leaf on that redraw, making a trailing
        // Generic OKF row disappear until the next query/filter event.
        self.refresh_nav(cx, false);
        let Some(mut arriving) = self.documents.capture_active_location(cx, &self.ui) else {
            return false;
        };
        if matches!(
            location.anchor,
            ViewAnchor::Markdown {
                fragment: Some(_),
                ..
            }
        ) {
            arriving.anchor = location.anchor.clone();
        }

        match cause {
            TransitionCause::UserNavigation => {
                self.session.break_edit_merge_group();
                if let Some(departing) = departing {
                    let explicit_fragment = matches!(
                        location.anchor,
                        ViewAnchor::Markdown {
                            fragment: Some(_),
                            ..
                        }
                    );
                    if departing.document == arriving.document && !explicit_fragment {
                        self.view_history.refresh_current(arriving);
                    } else {
                        self.view_history.record_transition(departing, arriving);
                    }
                } else {
                    self.view_history.reset(Some(arriving));
                }
            }
            TransitionCause::UndoRedoReveal => {
                self.session.break_edit_merge_group();
                if let Some(departing) = departing {
                    self.view_history.record_transition(departing, arriving);
                } else {
                    self.view_history.reset(Some(arriving));
                }
            }
            TransitionCause::HistoryTraversal => {}
            TransitionCause::PassiveReconciliation => {
                if self
                    .view_history
                    .current()
                    .is_some_and(|current| current.document == arriving.document)
                {
                    self.view_history.refresh_current(arriving);
                }
            }
        }
        self.sync_history_controls(cx);
        true
    }

    pub(super) fn traverse_view_history(
        &mut self,
        cx: &mut Cx,
        direction: HistoryDirection,
    ) -> bool {
        let Some(assets) = self.markdown_assets.as_ref() else {
            return false;
        };
        let Some(target) = self.view_history.target(direction, |location| {
            crate::documents::open_locator_with_asset_host(
                self.session.okf_analysis(),
                self.session.uml_analysis(),
                &location.document,
                assets,
            )
            .is_some()
        }) else {
            return false;
        };
        let location = target.location.clone();
        if !self.transition_to_location(cx, location, TransitionCause::HistoryTraversal) {
            return false;
        }
        self.view_history.commit_traversal(target);
        self.session.break_edit_merge_group();
        self.sync_history_controls(cx);
        true
    }

    pub(super) fn close_document(&mut self, cx: &mut Cx, id: LiveId) -> bool {
        let was_active = self.documents.active_id() == id;
        let departing = was_active
            .then(|| self.documents.capture_active_location(cx, &self.ui))
            .flatten();
        let changed =
            self.documents
                .transition(cx, &self.ui, &self.session, DocumentCommand::Close(id));
        if !changed {
            return false;
        }
        self.sync_document_shell(cx);
        if was_active {
            self.session.break_edit_merge_group();
            match (
                departing,
                self.documents.capture_active_location(cx, &self.ui),
            ) {
                (Some(departing), Some(arriving)) => {
                    self.view_history.record_transition(departing, arriving);
                }
                (_, None) => self.view_history.reset(None),
                _ => {}
            }
        }
        self.sync_history_controls(cx);
        true
    }

    /// Rebuild the nav projection from the current `nav_state` and push it to
    /// the tree panel. The single choke point for every scope change.
    ///
    /// A tree build runs the folder-view chain for every directory in the
    /// bundle, recursively, so the view and the scope-title lookup share ONE
    /// build. `refresh_nav` fires on every row click and every navigation
    /// change too, where nothing about the projection moved -- so the build is
    /// memoized on everything it reads: the session revision (bumped by every
    /// edit and by `replace`), the mode, and the descent cap. Scope and
    /// selection are applied to the CACHED tree by `view_of`, which is why
    /// they are not part of the key.
    pub(super) fn refresh_nav(&mut self, cx: &mut Cx, scope_changed: bool) {
        let key = (
            self.session.revision(),
            self.view_mode,
            self.chain_limits.max_depth,
        );
        if self.nav_tree.as_ref().map(|(cached, _)| *cached) != Some(key) {
            let tree = crate::tree::build_tree(
                self.session.okf_analysis(),
                self.session.uml_analysis(),
                "Untitled",
                self.view_mode,
                self.chain_limits,
            );
            self.nav_tree = Some((key, tree));
        }
        let full = &self
            .nav_tree
            .as_ref()
            .expect("the build above populates the cache")
            .1;
        let view = crate::nav::view_of(full, &self.nav_state);
        let title = scope_changed.then(|| {
            crate::nav::packages_of(full, self.session.okf_analysis())
                .into_iter()
                .find(|r| r.key == self.nav_state.scope)
                .map(|r| r.title)
                .unwrap_or_else(|| "Untitled".to_string())
        });
        if let Some(mut panel) = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
        {
            panel.set_view_with_fold_reset(cx, view, scope_changed);
            if let Some(title) = title {
                panel.set_scope_title(cx, title);
            }
            panel.set_view_mode(cx, self.view_mode);
        }
    }

    /// Flip the session-wide projected/raw switch.
    ///
    /// Both surfaces read the same flag, so there is no state in which the
    /// tree and a folder view disagree about what a directory contains. The
    /// flag lives in memory only: raw is a deliberate act, not a preference,
    /// so it is never written to `.waml/settings.json` and every launch
    /// starts Projected.
    ///
    /// This is presentational. A row the declared chain does not emit is not
    /// protected by anything; raw simply asks for the identity listing.
    pub(super) fn toggle_view_mode(&mut self, cx: &mut Cx) {
        self.view_mode = match self.view_mode {
            crate::folder_projection::ViewMode::Projected => {
                crate::folder_projection::ViewMode::Raw
            }
            crate::folder_projection::ViewMode::Raw => {
                crate::folder_projection::ViewMode::Projected
            }
        };
        self.refresh_nav(cx, false);
        self.refresh_folder_tabs(cx);
        cx.redraw_all();
    }

    /// Re-run every OPEN folder tab under the current mode, in place -- same
    /// tab, view swapped. Concept tabs are untouched: a mode is about how a
    /// container lists its contents, and a concept has none.
    ///
    /// `ReopenInPlace` rather than `Open` because `Open` keeps the existing
    /// view whenever the tab id is already open; that is exactly what made
    /// the old per-folder "View raw" build a view and throw it away.
    pub(super) fn refresh_folder_tabs(&mut self, cx: &mut Cx) {
        let folders: Vec<String> = self
            .documents
            .tabs()
            .iter()
            .filter(|tab| tab.presentation.category == crate::document::NavCategory::Directory)
            .map(|tab| tab.concept_id.clone())
            .collect();
        for directory in folders {
            let Some(document) = crate::documents::open_folder(
                self.session.okf_analysis(),
                &directory,
                self.chain_limits,
                self.view_mode,
            ) else {
                // The directory left the bundle underneath us. Leaving the
                // stale view up is wrong, but so is silently closing a tab
                // the user opened; the next model refresh reconciles it.
                continue;
            };
            self.documents.transition(
                cx,
                &self.ui,
                &self.session,
                DocumentCommand::ReopenInPlace { document },
            );
        }
    }
}
