use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PendingFragment {
    pub(super) concept_id: String,
    pub(super) fragment: String,
}

/// A search-hit reveal awaiting its target document's tab (`ViewOutcome.reveal`,
/// spec §DocView::reveal), applied by `App::apply_pending_reveal` once the
/// active tab's concept matches -- the same deferred-apply shape as
/// `PendingFragment`.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct PendingReveal {
    pub(super) concept_id: String,
    pub(super) target: crate::doc_view::RevealTarget,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct PendingAnchorRestore {
    pub(super) document: crate::navigation::DocumentLocator,
    pub(super) anchor: ViewAnchor,
    /// Stamped from `App::anchor_restore_generation` when this restore was
    /// scheduled, so a later traversal that supersedes it can be detected.
    pub(super) generation: u64,
}

/// The anchor restore a history traversal still owes once its target tab has
/// drawn, plus the generation that says whether it is still the current one.
///
/// The two moved together because they are one rule: scheduling BUMPS the
/// generation and stamps the new restore with it, and applying compares. Held
/// apart, "bump then stamp" was two statements a caller had to get right in
/// order, and nothing said the stamp had to come from the bump.
#[derive(Default)]
pub(super) struct DeferredAnchorRestore {
    pending: Option<PendingAnchorRestore>,
    generation: u64,
}

impl DeferredAnchorRestore {
    /// Schedule a restore, superseding any still-deferred one.
    pub(super) fn schedule(
        &mut self,
        document: crate::navigation::DocumentLocator,
        anchor: ViewAnchor,
    ) {
        self.generation = self.generation.wrapping_add(1);
        self.pending = Some(PendingAnchorRestore {
            document,
            anchor,
            generation: self.generation,
        });
    }

    /// Take the deferred restore, if one is owed.
    pub(super) fn take(&mut self) -> Option<PendingAnchorRestore> {
        self.pending.take()
    }

    /// The document a still-deferred restore is for, if any.
    ///
    /// A departing view whose restore has not applied yet has a stale captured
    /// anchor, and refreshing history with it would corrupt the entry that
    /// restore is about to write.
    pub(super) fn pending_document(&self) -> Option<&crate::navigation::DocumentLocator> {
        self.pending.as_ref().map(|pending| &pending.document)
    }

    /// Whether `pending` is still the newest scheduled restore.
    ///
    /// A superseded restore's anchor is still correct for the view, but
    /// refreshing history with it would clobber the newer entry.
    pub(super) fn is_current(&self, pending: &PendingAnchorRestore) -> bool {
        pending.generation == self.generation
    }

    /// The current generation. Test seam: a scenario asserts that the restore
    /// it observed carries the generation the schedule minted.
    #[cfg(test)]
    pub(super) fn generation(&self) -> u64 {
        self.generation
    }

    /// The deferred restore without taking it. Test seam: a scenario checks
    /// what a traversal scheduled before the deferred draw applies it.
    #[cfg(test)]
    pub(super) fn peek(&self) -> Option<&PendingAnchorRestore> {
        self.pending.as_ref()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TransitionCause {
    UserNavigation,
    UndoRedoReveal,
    HistoryTraversal,
    PassiveReconciliation,
}

impl App {
    /// The "primary" resolution at click sites: the surface a concept opens
    /// on when nothing requests one explicitly.
    pub(super) fn primary_locator(&self, concept_id: &str) -> crate::navigation::DocumentLocator {
        let surface = crate::documents::default_surface_for(
            self.session.okf_analysis(),
            self.session.uml_analysis(),
            &waml::view::row::RowTarget::Concept(concept_id.to_string()),
        );
        crate::navigation::DocumentLocator::concept(concept_id.to_string(), surface)
    }

    /// The folder sibling of `primary_locator`: the surface a DIRECTORY
    /// opens on when nothing requests one. A folder declaring `view: book`
    /// resolves to the book surface; everything else keeps today's listing.
    /// Asks the declared chain statically (`Chain::resolution_surface`)
    /// instead of running it -- a click site must not project rows twice.
    /// Deliberately narrowed to `book`: the `markdown`/`member:` folder
    /// resolutions are not yet consumed by any editor open path, and
    /// widening a click route is its own spec, not a side effect of this
    /// one.
    pub(super) fn primary_folder_locator(
        &self,
        address: &str,
    ) -> crate::navigation::DocumentLocator {
        let registry = crate::folder_projection::core_registry();
        let (chain, _diagnostics) = crate::folder_projection::chain_for(
            self.session.okf_analysis(),
            address,
            self.projection.mask(),
            &registry,
        );
        if chain.resolution_surface() == Some(waml::view::surface::SurfaceId::book()) {
            crate::navigation::DocumentLocator::new(
                waml::view::row::RowTarget::Folder(address.to_string()),
                waml::view::surface::SurfaceId::book(),
            )
        } else {
            crate::navigation::DocumentLocator::folder(address)
        }
    }

    /// "Read as scroll" (spec 2026-08-11-read-as-scroll-design): open
    /// `address`'s BOOK tab through the shared history-aware transition path
    /// -- the same path a stored book locator in history takes. A navigation,
    /// not a mode: nothing is written, and `book_documents::open` already
    /// opens any directory in the bundle, declared or not. The tab identity
    /// bakes the surface in, so this tab is distinct from the folder's
    /// listing tab and re-invoking activates rather than duplicates.
    pub(super) fn open_folder_as_scroll(&mut self, cx: &mut Cx, address: &str) -> bool {
        let changed = self.transition_to_location(
            cx,
            ViewLocation {
                document: crate::navigation::DocumentLocator::new(
                    waml::view::row::RowTarget::Folder(address.to_string()),
                    waml::view::surface::SurfaceId::book(),
                ),
                anchor: ViewAnchor::None,
            },
            TransitionCause::UserNavigation,
        );
        if !changed {
            self.set_navigation_message(cx, Some(&format!("Folder not found: {address}")));
            return false;
        }
        cx.redraw_all();
        self.set_navigation_message(cx, None);
        true
    }

    /// `query`'s `ResultRow`s (snippet width 80, spec §Results tab), `hidden`
    /// from `SearchState::hidden_documents`. Shared by `build_search_document`
    /// (the results tab's own contents) and `open_search_results` (Task 14's
    /// bundle-wide `session_search`, over the identical rows so the tab and
    /// the session never disagree about what "next" means).
    fn build_search_rows(&self, query: &str) -> Vec<crate::search_results_view::ResultRow> {
        let hits = self
            .search
            .query(query, &waml::search::QueryScope::default());
        let hidden = self
            .search
            .hidden_documents(self.session.okf_analysis(), self.session.uml_analysis());
        hits.into_iter()
            .map(|hit| {
                let snippet = self.search.snippet(&hit, 80);
                let hidden_flag = hidden.contains(&hit.document);
                crate::search_results_view::ResultRow {
                    label: crate::search_results_view::label_for(&hit),
                    hit,
                    snippet,
                    hidden: hidden_flag,
                }
            })
            .collect()
    }

    /// Runs `query` against the bundle-wide text index (`SearchState`) and
    /// builds the results-tab `OpenDocument` for it (decision 7's
    /// `documents::open_search` factory). Called both by `open_search_results`
    /// (a fresh query) and by `transition_to_location`'s search-locator arm
    /// (re-running the query on reopen).
    pub(super) fn build_search_document(&self, query: &str) -> crate::document::OpenDocument {
        crate::documents::open_search(query, self.build_search_rows(query))
    }

    /// Opens (or re-activates) `query`'s results tab through the shared
    /// history-aware transition path (spec §Results tab; decision 7). Same
    /// query -> same locator -> same tab id, so a re-run activates rather
    /// than duplicates, and the tab participates in view history like any
    /// other (Back/Forward re-runs the query via the same locator). Called
    /// by the Ctrl+K palette's `MoreText`/`Escalate` row commit (Task 11).
    ///
    /// Also (re)starts the bundle-wide `session_search` (Task 14, spec
    /// §Search session) fresh over this query's hits, in the SAME
    /// results-tab order (`search_results_view::ordered_hits`) the tab
    /// itself just opened with -- F3/Shift+F3 and the results tab's own
    /// cursor mirror never drift apart. The cursor starts `None`; a row
    /// activation or a palette commit marks it (`App::mark_session_landing`,
    /// via `ViewOutcome.reveal`).
    pub(crate) fn open_search_results(&mut self, cx: &mut Cx, query: &str) {
        let rows = self.build_search_rows(query);
        let hits = crate::search_results_view::ordered_hits(rows);
        self.session_search.begin(SearchSession::new(
            query.to_string(),
            hits,
            waml::search::QueryScope::default(),
        ));
        let locator = crate::navigation::DocumentLocator::new(
            waml::view::row::RowTarget::Virtual,
            waml::view::surface::SurfaceId(format!("search:{query}")),
        );
        self.transition_to_location(
            cx,
            ViewLocation {
                document: locator,
                anchor: ViewAnchor::None,
            },
            TransitionCause::UserNavigation,
        );
    }

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
        // An existence probe, not an open: `locator_opens` answers exactly
        // what `open_locator_with_asset_host(..).is_some()` would, without
        // building the surface table, resolving a chain, or allocating a view
        // per stored location -- this sync runs on every shell/workspace
        // refresh.
        let openable = self.markdown_assets.is_some();
        let can_back = openable
            && self
                .view_history
                .can_traverse(HistoryDirection::Back, |location| {
                    crate::documents::locator_opens(
                        self.session.okf_analysis(),
                        self.session.uml_analysis(),
                        &location.document,
                    )
                });
        let can_forward = openable
            && self
                .view_history
                .can_traverse(HistoryDirection::Forward, |location| {
                    crate::documents::locator_opens(
                        self.session.okf_analysis(),
                        self.session.uml_analysis(),
                        &location.document,
                    )
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
        document: waml::source::DocumentId,
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
                surface,
                fragment,
            } => {
                if self.session.okf().concept(&concept_id).is_none() {
                    self.set_navigation_message(
                        cx,
                        Some(&format!("Document not found: {concept_id}")),
                    );
                    return false;
                }
                let locator = match surface {
                    Some(surface) => {
                        let target = waml::view::row::RowTarget::Concept(concept_id.clone());
                        let (resolved, _diagnostic) = crate::documents::resolve_surface_for(
                            self.session.okf_analysis(),
                            self.session.uml_analysis(),
                            Some(surface.as_str()),
                            &target,
                            "index.md",
                            0,
                        );
                        crate::navigation::DocumentLocator::new(target, resolved)
                    }
                    None => self.primary_locator(&concept_id),
                };
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
                        document: locator,
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
                // target now always means "open", never "toggle". Routed
                // through `transition_to_location` (like the `Document`
                // arm above) so a folder tab participates in view history
                // like any other tab -- Back/Forward now stops on it
                // instead of skipping past it (spike Q5; the locator now
                // resolves per Task 2's `open_locator_with_asset_host`
                // folder arm).
                //
                // The transition is its own existence probe: a directory that
                // is not in the bundle makes `open_locator_with_asset_host`
                // return `None`, which surfaces here as `false`. Probing
                // separately with `open_folder` would build the whole
                // `FolderView` twice per navigation. The locator is
                // chain-routed: a `view: book` declaration opens the book
                // surface, everything else the listing.
                let changed = self.transition_to_location(
                    cx,
                    ViewLocation {
                        document: self.primary_folder_locator(&address),
                        anchor: ViewAnchor::None,
                    },
                    TransitionCause::UserNavigation,
                );
                if !changed {
                    self.set_navigation_message(cx, Some(&format!("Folder not found: {address}")));
                    return false;
                }
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
            .is_none_or(|tab| tab.concept_id() != Some(pending.concept_id.as_str()))
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

    /// Apply a search-hit reveal once its target document's tab is active
    /// AND drawn (`handle_draw_restores`, the same `Event::Draw` gate
    /// `apply_pending_fragment` uses). Cleared unconditionally once checked --
    /// a reveal that lands on the wrong tab (the user navigated away before
    /// the draw) is dropped, not retried against whatever opened next.
    ///
    /// **This is NOT what `apply_pending_fragment` does**, though this comment
    /// claimed it was until 2026-08-21. That one takes the pending value by
    /// reference and returns early on a tab mismatch WITHOUT clearing it, so
    /// the fragment stays armed and fires the next time its document becomes
    /// active -- which may be a later, unrelated visit. Whether that is a
    /// feature (the navigation eventually completes) or a stale-navigation bug
    /// (the document scrolls somewhere the user did not just ask for) is an
    /// open product question, recorded here rather than settled by whichever
    /// of the two a future refactor happens to unify onto.
    pub(super) fn apply_pending_reveal(&mut self, cx: &mut Cx) {
        let Some(pending) = self.pending_reveal.take() else {
            return;
        };
        if self
            .documents
            .active_tab()
            .is_none_or(|tab| tab.concept_id() != Some(pending.concept_id.as_str()))
        {
            return;
        }
        if self.documents.reveal_active(cx, &self.ui, &pending.target) {
            // The reveal just replaced the whole highlight set with the one
            // landed range; put the session's other matches in this document
            // back (spec §Search session).
            self.relight_session_highlights(cx, &pending.concept_id);
        }
    }

    pub(super) fn apply_pending_anchor_restore(&mut self, cx: &mut Cx) {
        let Some(pending) = self.anchor_restore.take() else {
            return;
        };
        if self
            .documents
            .active_tab()
            .is_none_or(|tab| tab.locator() != pending.document)
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
        if !self.anchor_restore.is_current(&pending) {
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
                document: self.primary_locator(concept_id),
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
        self.open_source_for(cx, waml::view::row::RowTarget::Concept(key.to_string()));
    }

    /// Opens `target`'s "source" surface through the shared history-aware
    /// transition path (spec §2/§5). `open_view_source` is a thin
    /// concept-keyed wrapper around this; the follow-on spec's folder-tab
    /// source affordance calls this directly with a `RowTarget::Folder`.
    pub(super) fn open_source_for(&mut self, cx: &mut Cx, target: waml::view::row::RowTarget) {
        self.transition_to_location(
            cx,
            ViewLocation {
                document: crate::navigation::DocumentLocator::new(
                    target,
                    waml::view::surface::SurfaceId::source(),
                ),
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
                .anchor_restore
                .pending_document()
                .zip(departing.as_ref())
                .is_some_and(|(pending, departing)| *pending == departing.document);
            if !departing_is_pending_restore {
                if let Some(departing) = departing.clone() {
                    self.view_history.refresh_current(departing);
                }
            }
        }
        // The markdown editor and reading viewer are ONE shared surface per
        // shell, so a search highlight installed for the departing document
        // is a set of byte ranges into text that is about to be replaced.
        // Drop it here; the arriving landing (if any) installs its own after
        // this transition (`apply_view_outcome`).
        if departing
            .as_ref()
            .is_none_or(|current| current.document != location.document)
        {
            self.clear_search_highlights(cx);
        }
        // A search-results locator (decision 7) has no factory in the
        // surface table -- `Virtual` never resolves through
        // `open_locator_with_asset_host` -- so it is rebuilt by re-running
        // the query, not by the generic asset-host restore path. This is
        // also how tab history traversal reopens a search tab: Back/Forward
        // reaches this same `transition_to_location`.
        let restored = if let Some(query) =
            crate::documents::search_query_from_locator(&location.document).map(str::to_string)
        {
            let document = self.build_search_document(&query);
            self.documents.restore_location_with_document(
                cx,
                &self.ui,
                &self.session,
                &location,
                document,
            )
        } else {
            let assets = self.ensure_markdown_asset_host(
                crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle,
            );
            self.documents.restore_location_with_asset_host(
                cx,
                &self.ui,
                &self.session,
                &location,
                &assets,
                self.markdown_emphasis,
                self.projection.limits(),
                self.projection.mask(),
            )
        };
        if !restored {
            return false;
        }
        if matches!(
            cause,
            TransitionCause::HistoryTraversal | TransitionCause::UndoRedoReveal
        ) && !matches!(location.anchor, ViewAnchor::None)
        {
            self.anchor_restore
                .schedule(location.document.clone(), location.anchor.clone());
            cx.redraw_all();
        }
        self.sync_document_shell(cx);
        // Re-submit the composed tree after the selection change so the panel's
        // projection matches the newly active document.
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
                self.markdown_emphasis,
                self.projection.limits(),
                self.projection.mask(),
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

    /// Rebuild the nav projection from the current scope and push it to the
    /// tree panel. The single choke point for every scope change.
    ///
    /// A tree build runs the folder-view chain for every directory in the
    /// bundle, recursively, so the view and the scope-title lookup share ONE
    /// build. `refresh_nav` fires on every row click and every navigation
    /// change too, where nothing about the projection moved -- which is why the
    /// build is memoized; see [`Projection`] for what the memo is keyed on.
    pub(super) fn refresh_nav(&mut self, cx: &mut Cx, scope_changed: bool) {
        // Taken before the tree, which borrows `projection` for as long as the
        // tree is in hand. The panel is handed an owned mask either way.
        let mask = self.projection.mask().clone();
        let (full, scope) = self.projection.tree_with_scope(
            self.session.revision(),
            self.session.okf_analysis(),
            self.session.uml_analysis(),
        );
        let view = crate::nav::view_of(full, scope);
        let title = scope_changed.then(|| {
            crate::nav::packages_of(full, self.session.okf_analysis())
                .into_iter()
                .find(|r| r.key == scope.scope)
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
            let registry = crate::folder_projection::core_registry();
            let maskable = crate::folder_projection::maskable_names(&registry)
                .into_iter()
                .flat_map(|(_owner, names)| names)
                .map(|name| name.to_string())
                .collect::<Vec<_>>();
            panel.set_projection(cx, mask, maskable);
        }
    }

    /// Install a new session-wide projection mask.
    ///
    /// Both surfaces read the same mask, so there is no state in which the
    /// tree and a folder view disagree about what a directory contains. It
    /// lives in memory only: raw is a deliberate act, not a preference, so it
    /// is never written to `.waml/editor.json` and every launch starts with
    /// an empty mask.
    ///
    /// This is presentational. A row a masked stage would have removed is not
    /// protected by anything; masking simply asks for the listing without it.
    pub(super) fn set_projection_mask(
        &mut self,
        cx: &mut Cx,
        mask: waml::view::mask::ProjectionMask,
    ) {
        if !self.projection.set_mask(mask) {
            return;
        }
        self.refresh_nav(cx, false);
        self.refresh_folder_tabs(cx);
        cx.redraw_all();
    }

    /// Re-run every OPEN folder tab under the current mask, in place -- same
    /// tab, view swapped. Concept tabs are untouched: a mask is about how a
    /// container lists its contents, and a concept has none.
    ///
    /// `ReopenInPlace` rather than `Open` because `Open` keeps the existing
    /// view whenever the tab id is already open; that is exactly what made
    /// the old per-folder "View raw" build a view and throw it away.
    pub(super) fn refresh_folder_tabs(&mut self, cx: &mut Cx) {
        for (directory, surface) in self.open_directory_tab_addresses() {
            let document = if surface == waml::view::surface::SurfaceId::book() {
                crate::book_documents::open(
                    self.session.okf_analysis(),
                    &directory,
                    self.projection.limits(),
                    self.projection.mask(),
                )
            } else {
                crate::documents::open_folder(
                    self.session.okf_analysis(),
                    &directory,
                    self.projection.limits(),
                    self.projection.mask(),
                )
            };
            let Some(document) = document else {
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

    /// The addresses of open tabs showing a directory SURFACE (listing or
    /// book) -- both halves of the locator matter, same reasoning as before:
    /// a folder's `source` tab shares the target but must not be rebuilt as
    /// a listing.
    pub(super) fn open_directory_tab_addresses(
        &self,
    ) -> Vec<(String, waml::view::surface::SurfaceId)> {
        self.documents
            .tabs()
            .iter()
            .filter_map(|tab| match &tab.locator.target {
                waml::view::row::RowTarget::Folder(address)
                    if tab.locator.surface == waml::view::surface::SurfaceId::folder()
                        || tab.locator.surface == waml::view::surface::SurfaceId::book() =>
                {
                    Some((address.clone(), tab.locator.surface.clone()))
                }
                _ => None,
            })
            .collect()
    }
}
