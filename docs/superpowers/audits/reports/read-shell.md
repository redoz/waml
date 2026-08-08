# Changed files

- `docs/waml/goals/read-a-bundle/open-a-bundle.md`
- `docs/waml/goals/read-a-bundle/browse-the-tree.md`
- `docs/waml/goals/read-a-bundle/read-a-document.md`
- `docs/waml/goals/read-a-bundle/read-a-diagram.md`
- `docs/waml/goals/read-a-bundle/navigate-and-return.md`
- `docs/waml/goals/read-a-bundle/fit-the-window.md`
- `docs/waml/goals/read-a-bundle/use-the-shell.md`
- `docs/waml/goals/read-a-bundle/work-with-tabs.md`
- `docs/waml/goals/read-a-bundle/index.md`
- `docs/superpowers/audits/reports/read-shell.md`

# Scenario identifiers

- `NATIVE-001` through `NATIVE-005`, and `NATIVE-014` — `open-a-bundle.md`
- `NATIVE-010` through `NATIVE-012`, and `NATIVE-015` — `browse-the-tree.md`
- `NATIVE-022`, `NATIVE-045`, and `NATIVE-061` — `read-a-document.md`
- `NATIVE-013` and `NATIVE-016` — `navigate-and-return.md`
- `NATIVE-006` and `NATIVE-007` — `fit-the-window.md`
- `NATIVE-008`, `NATIVE-009`, and `NATIVE-055` — `use-the-shell.md`
- `NATIVE-017` through `NATIVE-021` — `work-with-tabs.md`

The ownership plan moves these frozen rows without changing the inventory:

- `NATIVE-014` moves from `navigate-and-return.md` to `open-a-bundle.md`.
- `NATIVE-015` moves from `navigate-and-return.md` to `browse-the-tree.md`.
- `NATIVE-008`, `NATIVE-009`, and `NATIVE-055` move from
  `fit-the-window.md` to `use-the-shell.md`.
- `NATIVE-017` through `NATIVE-021` move from `read-a-document.md` to
  `work-with-tabs.md`.

# Evidence

- `NATIVE-001` — implementation: `crates/waml-editor/src/start_screen.rs::StartScreen`; test: none.
- `NATIVE-002` — implementation: `crates/waml-editor/src/config.rs::sort_recents`; test: none.
- `NATIVE-003` — implementation: `crates/waml-editor/src/config.rs::set_pinned`; test: none.
- `NATIVE-004` — implementation: `crates/waml-editor/src/load.rs::read_bundle`; test: none.
- `NATIVE-005` — implementation: `crates/waml-editor/src/app/workspace.rs::prepare_open_documents`; test: none.
- `NATIVE-006` — implementation: `crates/waml-editor/src/app/shell.rs::next_narrow`; tests: `crates/waml-editor/src/app/tests/menus.rs::breakpoint_enters_below_640_and_leaves_above_680` and `crates/waml-editor/src/app/tests/menus.rs::breakpoint_preserves_mode_through_the_hysteresis_band`.
- `NATIVE-007` — implementation: `crates/waml-editor/src/app/shell.rs::tree_toggle_layout`; test: `crates/waml-editor/src/app/tests/shell.rs::the_toggle_rides_the_column_edge_and_falls_back_to_the_row_head`.
- `NATIVE-008` — implementation: `crates/waml-editor/src/app/shell.rs::apply_dock_states` and `crates/waml-editor/src/app/shell.rs::sync_dock_slots`; test: `crates/waml-editor/src/app/tests/shell.rs::mounted_dock_close_keeps_presented_geometry_until_motion_completes`.
- `NATIVE-009` — implementation: `crates/waml-editor/src/app/shell.rs::sync_dock_slots`; test: `crates/waml-editor/src/app/tests/shell.rs::mounted_dock_areas_follow_wide_and_narrow_production_layout`.
- `NATIVE-010` — implementation: `crates/waml-editor/src/tree.rs::build_tree`; test: none.
- `NATIVE-011` — implementation: `crates/waml-editor/src/tree_panel.rs::reveal_path`; test: none.
- `NATIVE-012` — implementation: `crates/waml-editor/src/tree_panel.rs::reconcile_open_directories`; test: none.
- `NATIVE-013` — implementation: `crates/waml-editor/src/navigation.rs::breadcrumb_for` and `crates/waml-editor/src/navigation.rs::breadcrumb_for_directory`; test: none.
- `NATIVE-014` — implementation: `crates/waml-editor/src/navigation.rs::resolve_link`; test: `crates/waml-editor/src/app/tests/navigation.rs::navigation_markdown_resolves_only_at_the_app_boundary`.
- `NATIVE-015` — implementation: `crates/waml-editor/src/navigation.rs::resolve_link`; test: `crates/waml-editor/src/app/tests/navigation.rs::navigation_external_target_invokes_only_the_browser_adapter_once`.
- `NATIVE-016` — implementation: `crates/waml-editor/src/app/actions.rs::handle_document_tabs`; tests: `crates/waml-editor/src/app/tests/navigation.rs::manual_and_preview_transitions_follow_back_and_forward_history` and `crates/waml-editor/tests/history_integration.rs::back_and_forward_restore_view_anchors_without_tab_metadata`.
- `NATIVE-017` — implementation: `crates/waml-editor/src/doc_tabs.rs::OpenTabs`; test: `crates/waml-editor/src/doc_tabs.rs::open_preview_twice_replaces_the_single_preview_slot`.
- `NATIVE-018` — implementation: `crates/waml-editor/src/doc_tabs.rs::OpenTabs`; test: `crates/waml-editor/src/doc_tabs.rs::promote_then_open_preview_keeps_the_promoted_tab_and_adds_a_fresh_preview`.
- `NATIVE-019` — implementation: `crates/waml-editor/src/document_host.rs::tabs`; test: `crates/waml-editor/src/doc_tabs.rs::reopening_a_promoted_tab_focuses_it_instead_of_duplicating`.
- `NATIVE-020` — implementation: `crates/waml-editor/src/doc_tabs.rs::OpenTabs`; test: `crates/waml-editor/src/doc_tabs.rs::close_activates_right_adjacent_then_left_then_first_tab`.
- `NATIVE-021` — implementation: `crates/waml-editor/src/document_host.rs::DocumentHost`; test: `crates/waml-editor/src/document_host.rs::prepared_preview_replacement_drops_the_old_live_view`.
- `NATIVE-022` — implementation: `crates/waml-editor/src/generic_okf_view.rs:34` (`GenericOkfView::new_with_asset_host`) and `crates/waml-editor/src/source_view.rs:236` (`SourceView::install_snapshot`); partial test: `crates/waml-editor/tests/markdown_integration.rs::read_only_mount_never_emits_a_source_proposal`.
- `NATIVE-045` — implementation: `crates/waml-markdown-editor/src/widget.rs::navigation_position`; test: `crates/waml-editor/tests/markdown_integration.rs::external_replacement_maps_selection_and_scroll_and_cuts_motion`.
- `NATIVE-055` — implementation: `crates/waml-editor/src/app/shell.rs::sync_document_shell`; test: `crates/waml-editor/src/app/tests/shell.rs::mounted_history_buttons_lead_the_tab_strip_past_the_tree_column`.
- `NATIVE-061` — implementation: `crates/waml-markdown-editor/src/widget.rs::draw_walk_with_session`; test: none.

# Verification gaps

- `NATIVE-001` — target: native; No native test asserts the visible empty/start screen.
- `NATIVE-002` — target: native; No native test asserts rendered recent-item order.
- `NATIVE-003` — target: native; No native test asserts pinning from the start screen.
- `NATIVE-004` — target: native; The test covers replacement saves, not the full active-workspace open result.
- `NATIVE-005` — target: native; The test checks the asset root only, not the complete prior-workspace result.
- `NATIVE-010` — target: native; No native test asserts the complete visible tree construction.
- `NATIVE-011` — target: native; No direct native assertion covers reveal expansion.
- `NATIVE-012` — target: native; No native test checks tree refresh and retained open folders.
- `NATIVE-013` — target: native; No native test asserts breadcrumb labels and targets.
- `NATIVE-022` — target: native; The cited native integration test proves that read-only input is rejected and source text is unchanged, but it does not assert rendered Markdown presentation.
- `NATIVE-061` — target: native; No native test checks the full visible reading surface.

# Open discrepancies

None.

# Feature gaps

- `FG-003` — `fit-the-window.md` and `read-a-diagram.md` use prose for semantic view anchors and post-draw results.
- `FG-004` — `work-with-tabs.md` uses prose for ordered tabs and selected state.
