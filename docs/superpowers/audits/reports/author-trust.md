# Changed files

- `docs/waml/goals/author-in-the-editor/create-and-delete-documents.md`
- `docs/waml/goals/author-in-the-editor/start-from-a-template.md`
- `docs/waml/goals/author-in-the-editor/edit-prose.md`
- `docs/waml/goals/author-in-the-editor/edit-the-model.md`
- `docs/waml/goals/author-in-the-editor/draw-on-the-canvas.md`
- `docs/waml/goals/author-in-the-editor/author-with-the-keyboard.md`
- `docs/waml/goals/author-in-the-editor/arrange-a-diagram.md`
- `docs/waml/goals/author-in-the-editor/reduce-the-effort.md`
- `docs/waml/goals/author-in-the-editor/save-and-undo.md`
- `docs/waml/goals/trust-the-content/round-trip-losslessly.md`
- `docs/waml/goals/trust-the-content/resolve-references.md`
- `docs/waml/goals/trust-the-content/report-every-problem.md`
- `docs/waml/goals/trust-the-content/keep-indexes-correct.md`
- `docs/waml/goals/trust-the-content/format-canonically.md`
- `docs/superpowers/audits/reports/author-trust.md`

# Scenario identifiers

- `NATIVE-023` through `NATIVE-026`, `NATIVE-046`, and `NATIVE-047` — `edit-prose.md`
- `NATIVE-027` through `NATIVE-031`, `NATIVE-048`, `NATIVE-049`, `NATIVE-056`, `NATIVE-057`, and `NATIVE-062` — `save-and-undo.md`
- `NATIVE-032` and `NATIVE-050` — `report-every-problem.md`

The ownership plan moves these frozen rows without changing the inventory:

- `NATIVE-025` and `NATIVE-026` move from
  `author-with-the-keyboard.md` to `edit-prose.md`.
- `NATIVE-049` moves from `keep-indexes-correct.md` to
  `save-and-undo.md`.
- `NATIVE-060`, `NATIVE-065`, `NATIVE-066`, `NATIVE-067`, and
  `NATIVE-069` move from `draw-on-the-canvas.md` to
  `goals/uml/class/interact-with-a-class-diagram.md`.

Task 12 must reconcile these eight `goal_document` values. This stream does
not copy the five class-interaction scenarios. `NATIVE-035` also remains a
Task 8 class-interaction gap; its partial property-state tests do not verify
visible property controls.

# Evidence

- `NATIVE-023` — implementation: `crates/waml-markdown-editor/src/input.rs:120` (`MarkdownEditorController::handle`); test: `crates/waml-editor/tests/markdown_integration.rs::editable_mount_emits_an_exact_revisioned_source_proposal`.
- `NATIVE-024` — implementation: `crates/waml-markdown-editor/src/input.rs:378` (`copy_selections`); tests: `crates/waml-markdown-editor/tests/widget_parity.rs::retained_copy_cut_paste_use_source_text` and `crates/waml-markdown-editor/tests/widget_parity.rs::read_only_mode_allows_selection_and_copy_but_not_mutation`.
- `NATIVE-025` — implementation: `crates/waml-markdown-editor/src/session.rs:476` (`MarkdownDocumentSession::add_selection`); tests: `crates/waml-markdown-editor/tests/widget_parity.rs::retained_platform_modifier_adds_selection` and `crates/waml-markdown-editor/tests/widget_parity.rs::mounted_widget_primary_modifier_adds_a_selection`.
- `NATIVE-026` — implementation: `crates/waml-markdown-editor/src/session.rs:237` (`MarkdownDocumentSession::begin_ime`), `crates/waml-markdown-editor/src/session.rs:251` (`MarkdownDocumentSession::update_ime`), `crates/waml-markdown-editor/src/session.rs:267` (`MarkdownDocumentSession::commit_ime`), and `crates/waml-markdown-editor/src/session.rs:314` (`MarkdownDocumentSession::cancel_ime`); tests: `crates/waml-markdown-editor/tests/unicode_ime.rs::ime_preedit_is_visible_state_but_not_a_published_revision`, `crates/waml-markdown-editor/tests/unicode_ime.rs::ime_cancel_restores_the_last_committed_snapshot_and_selection`, and `crates/waml-markdown-editor/tests/unicode_ime.rs::ime_replaces_nonempty_selection_and_cancel_models_focus_loss`.
- `NATIVE-046` — implementation: `crates/waml-markdown-editor/src/session.rs:361` (`MarkdownDocumentSession::select_word_at`) and `crates/waml-markdown-editor/src/session.rs:379` (`MarkdownDocumentSession::select_line_at`); tests: `crates/waml-markdown-editor/tests/widget_parity.rs::retained_double_click_selects_word` and `crates/waml-markdown-editor/tests/widget_parity.rs::retained_triple_click_selects_source_line`.
- `NATIVE-047` — implementation: `crates/waml-markdown-editor/src/input.rs:416` (`verify_revision`); test: none.
- `NATIVE-027` — implementation: `crates/waml-editor/src/editor_session.rs:755` (`promote_source_edit`) and `crates/waml-editor/src/editor_session.rs:887` (`install_semantic_completion`); test: `crates/waml-editor/src/editor_session/tests.rs::accepted_source_edit_advances_once_and_promotes_the_same_syntax_arc`.
- `NATIVE-028` — implementation: `crates/waml-editor/src/editor_history.rs:159` (`commit_undo`) and `crates/waml-editor/src/editor_history.rs:171` (`commit_redo`); test: `crates/waml-editor/tests/editor_history.rs::one_edit_produces_undo_and_undo_redo_swap_reciprocals`.
- `NATIVE-029` — implementation: `crates/waml-editor/src/editor_history.rs:159` (`commit_undo`), `crates/waml-editor/src/editor_history.rs:211` (`is_saved`), and `crates/waml-editor/src/editor_history.rs:215` (`mark_saved`); test: `crates/waml-editor/tests/editor_history.rs::savepoint_identity_tracks_undo_back_to_saved_state`.
- `NATIVE-030` — implementation: `crates/waml-editor/src/native_save.rs:111` (`save_ticket_atomic`); partial unmarked test: `crates/waml-editor/src/app/tests/workspace.rs::failed_final_save_retains_dirty_and_prevents_quit`.
- `NATIVE-031` — implementation: `crates/waml-editor/src/app/workspace.rs:151` (`prepare_open_documents`); test: `crates/waml-editor/src/app/tests/workspace.rs::successful_bundle_open_clears_the_visible_save_error`.
- `NATIVE-032` — implementation: `crates/waml-editor/src/editor_session.rs:146` (`map_source_range_to_current`); test: `crates/waml-editor/src/editor_session/tests.rs::pending_source_changes_map_only_unchanged_diagnostic_ranges`.
- `NATIVE-048` — implementation: `crates/waml-editor/src/editor_history.rs:124` (`record_edit`); test: `crates/waml-editor/tests/editor_history.rs::new_edit_after_undo_clears_redo`.
- `NATIVE-049` — implementation: `crates/waml-editor/src/editor_session.rs:755` (`promote_source_edit`); test: `crates/waml-editor/src/editor_session/tests.rs::stale_or_invalid_source_proposals_do_not_mutate_snapshot_or_history`.
- `NATIVE-050` — implementation: `crates/waml-editor/src/app/navigation.rs:705` (`refresh_folder_tabs`); test: `crates/waml-editor/src/app/tests/navigation.rs::changed_source_range_navigation_preserves_selection_and_publishes_status`.
- `NATIVE-056` — implementation: none; test: `crates/waml-editor/src/app/tests/workspace.rs::failed_save_blocks_close_and_keeps_document_state`.
- `NATIVE-057` — implementation: none; test: `crates/waml-editor/src/app/tests/workspace.rs::successful_save_allows_close_and_clears_document_state`.
- `NATIVE-062` — implementation: `crates/waml-editor/src/editor_session.rs:755` (`promote_source_edit`); test: `crates/waml-editor/src/editor_session/tests.rs::accepted_source_edit_advances_once_and_promotes_the_same_syntax_arc`.

# Verification gaps

- `NATIVE-030` — target: native; The test asserts quit prevention after a final save failure, but it does not assert that the document remains dirty.
- `NATIVE-047` — target: native; No native integration test asserts the stale-layout rejection result.

# Open discrepancies

None.

# Feature gaps

- `FG-002` — `edit-prose.md` and `draw-on-the-canvas.md` use prose for typed gestures and consumed input.
- `FG-005` — `edit-prose.md` uses prose for semantic text positions, multiple carets, and IME composition.
- `FG-006` — `save-and-undo.md` uses prose for transaction groups and saved-state markers.
- `FG-007` — `draw-on-the-canvas.md` uses prose for semantic canvas targets and drag paths.
