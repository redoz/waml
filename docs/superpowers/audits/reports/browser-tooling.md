# Changed files

- `docs/waml/goals/share-and-publish/share-a-link.md`
- `docs/waml/goals/share-and-publish/run-in-a-browser.md`
- `docs/waml/goals/share-and-publish/serve-locally.md`
- `docs/waml/goals/share-and-publish/publish-a-site.md`
- `docs/waml/goals/share-and-publish/export-a-bundle.md`
- `docs/waml/goals/tooling-around-the-repo/command-line-tool.md`
- `docs/waml/goals/tooling-around-the-repo/language-server.md`
- `docs/waml/goals/tooling-around-the-repo/text-editor-integration.md`
- `docs/superpowers/audits/reports/browser-tooling.md`

The Task 10 indexes remain unchanged. Task 12 owns final index generation and
cross-cutting integration.

# Scenario identifiers

- `CLI-003` and `BROWSER-015` — `share-a-link.md`
- `BROWSER-005` — `run-in-a-browser.md`
- `BROWSER-002`, `BROWSER-007` through `BROWSER-010`, and `BROWSER-017`
  through `BROWSER-020` — `serve-locally.md`
- `BROWSER-012` — `publish-a-site.md`
- `BROWSER-001`, `BROWSER-003`, `BROWSER-004`, `BROWSER-006`, `BROWSER-011`,
  and `BROWSER-016` — `export-a-bundle.md`
- `CLI-001`, `CLI-002`, and `CLI-004` through `CLI-011` —
  `command-line-tool.md`
- `LSP-001` through `LSP-005` — `language-server.md`
- `VSCODE-001` through `VSCODE-006` and `VSCODE-008` —
  `text-editor-integration.md`

All 41 shipped identifiers and applicability values remain unchanged. The
non-GWT records are `BHV-BRW-013` (unsupported), `BHV-BRW-021`,
`BHV-BRW-022`, and `BHV-LSP-007` (discrepant), and `BHV-LSP-006` and
`BHV-VSC-007` (planned). `BHV-BRW-014` remains in `goals/mvp.md`; Task 12 owns
that cross-cutting discrepancy and its inventory reconciliation.

# Evidence

- `BROWSER-001` — implementation: `crates/waml-editor/src/browser_boot.rs:48`; partial test: `scripts/export-site-browser.test.mjs::an exported site boots and exports its model back`.
- `BROWSER-002` — implementation: `crates/waml-editor/src/browser_boot.rs:48`, `crates/waml-editor/src/browser_boot.rs:165`, and `crates/waml-cli/src/serve/mod.rs:30`; test: none.
- `BROWSER-003` — implementation: `crates/waml-editor/src/browser_boot.rs:48` and `crates/waml-editor/src/browser_boot.rs:122`; partial test: `crates/waml-editor/src/browser_boot.rs::relative_bundle_url_is_kept_as_written`.
- `BROWSER-004` — implementation: `crates/waml-editor/src/browser_boot.rs:100` and `crates/waml-cli/src/site.rs:108`; test: `scripts/export-site-browser.test.mjs::an exported site boots and exports its model back`.
- `BROWSER-005` — implementation: `crates/waml-editor/src/browser_boot.rs:136`, `crates/waml-editor/src/app.rs:902`, and `crates/waml-editor/src/app.rs:1080`; partial test: `crates/waml-editor/src/browser_boot.rs::network_failure_says_cors_too`.
- `BROWSER-006` — implementation: none; test: `scripts/export-site-browser.test.mjs::an exported site boots and exports its model back`.
- `BROWSER-007` — implementation: `crates/waml-cli/src/serve/routes.rs:151` and `crates/waml-cli/src/serve/guard.rs:100`; test: none.
- `BROWSER-008` — implementation: `crates/waml-cli/src/serve/guard.rs:100`; partial test: `crates/waml-cli/src/serve/routes.rs::a_foreign_origin_is_403`.
- `BROWSER-009` — implementation: `crates/waml-cli/src/serve/routes.rs:230`; test: none.
- `BROWSER-010` — implementation: `crates/waml-editor/src/api_save.rs:81` and `crates/waml-cli/src/serve/routes.rs:253`; partial tests: `crates/waml-editor/src/api_save.rs::conflict_and_error_responses_parse` and `crates/waml-cli/src/serve/routes.rs::a_stale_revision_is_409`.
- `BROWSER-011` — implementation: `crates/waml-cli/src/site.rs:108` and `crates/waml-cli/src/main.rs:1035`; test: `scripts/export-site-browser.test.mjs::an exported site boots and exports its model back`.
- `BROWSER-012` — implementation: `.github/workflows/pages.yml:184` and `scripts/verify-web-artifact.mjs:22`; partial tests: `scripts/verify-web-artifact.test.mjs::passes on a complete artifact`, `scripts/verify-web-artifact.test.mjs::catches the missing makepad glue`, and `scripts/verify-web-artifact.test.mjs::catches a missing transitive import inside the glue`.
- `BROWSER-015` — implementation: `crates/waml-editor/src/app.rs:836`; test: none.
- `BROWSER-016` — implementation: `crates/waml-editor/src/app/workspace.rs:101` and `crates/waml-editor/src/app/workspace.rs:301`; partial test: `crates/waml-editor/src/app/tests/workspace.rs::a_first_edit_replaces_the_fragment_and_keeps_the_bundle_query`.
- `BROWSER-017` — implementation: `crates/waml-cli/src/serve/routes.rs:167`; partial test: `crates/waml-cli/src/serve/routes.rs::every_read_carries_the_revision`.
- `BROWSER-018` — implementation: `crates/waml-cli/src/serve/routes.rs:195` and `crates/waml-cli/src/serve/state.rs:78`; partial tests: `crates/waml-cli/src/serve/routes.rs::an_op_post_mutates_the_disk_and_answers_changed_files` and `crates/waml-cli/src/serve/routes.rs::a_rejected_batch_is_422_with_the_edit_error_shape`.
- `BROWSER-019` — implementation: `crates/waml-cli/src/serve/guard.rs:100` and `crates/waml-cli/src/serve/routes.rs:86`; partial tests: `crates/waml-cli/src/serve/routes.rs::reads_require_a_token` and `crates/waml-cli/src/serve/routes.rs::an_unauthenticated_post_is_401_even_with_a_malformed_body`.
- `BROWSER-020` — implementation: `crates/waml-cli/src/serve/guard.rs:100` and `crates/waml-cli/src/serve/routes.rs:86`; partial test: `crates/waml-cli/src/serve/routes.rs::a_mutating_post_without_the_client_header_is_403`.
- `CLI-001` — implementation: `crates/waml-cli/src/main.rs:358` and `crates/waml-cli/src/commands.rs:162`; test: `crates/waml-cli/tests/cli_e2e.rs::check_reports_malformed_claimed_uml_from_parser_analysis`.
- `CLI-002` — implementation: `crates/waml-cli/src/main.rs:358` and `crates/waml-cli/src/commands.rs:288`; test: `crates/waml-cli/tests/cli_e2e.rs::fmt_canonical_output_is_idempotent`.
- `CLI-003` — implementation: `crates/waml-cli/src/main.rs:358`; partial test: `scripts/export-site-browser.test.mjs::an exported site boots and exports its model back`.
- `CLI-004` — implementation: `crates/waml-cli/src/main.rs:726` and `crates/waml-cli/src/main.rs:619`; tests: `crates/waml-cli/tests/cli_e2e.rs::attr_add_writes_the_file` and `crates/waml-cli/tests/cli_e2e.rs::duplicate_attr_exits_1`.
- `CLI-005` — implementation: `crates/waml-cli/src/main.rs:844`; test: `crates/waml-cli/tests/cli_e2e.rs::apply_late_multi_file_failure_writes_nothing`.
- `CLI-006` — implementation: `crates/waml-cli/src/main.rs:885`; test: `crates/waml-cli/tests/cli_e2e.rs::show_json_and_refs_share_prepared_referrer_results`.
- `CLI-007` — implementation: `crates/waml-cli/src/main.rs:954`; test: `crates/waml-cli/tests/cli_e2e.rs::show_json_and_refs_share_prepared_referrer_results`.
- `CLI-008` — implementation: `crates/waml-cli/src/main.rs:1075`; test: none.
- `CLI-009` — implementation: `crates/waml-cli/src/main.rs:988` and `crates/waml-cli/src/commands.rs:148`; test: none.
- `CLI-010` — implementation: `crates/waml-cli/src/main.rs:358` and `crates/waml-cli/src/commands.rs:288`; test: none.
- `CLI-011` — implementation: `crates/waml-cli/src/main.rs:726`, `crates/waml-cli/src/main.rs:575`, `crates/waml-cli/src/main.rs:660`, and `crates/waml-cli/src/main.rs:675`; test: none.
- `LSP-001` — implementation: `crates/waml-cli/src/lsp/server.rs:199`; test: `crates/waml-cli/tests/lsp_e2e.rs::publishes_diagnostics_for_unresolved_target_over_stdio`.
- `LSP-002` — implementation: `crates/waml-cli/src/lsp/server.rs:61` and `crates/waml-cli/src/lsp/server.rs:283`; test: `crates/waml-cli/tests/lsp_e2e.rs::snapshot_queries_are_advertised_unicode_exact_and_revision_current_over_stdio`.
- `LSP-003` — implementation: `crates/waml-cli/src/lsp/server.rs:61` and `crates/waml-cli/src/lsp/server.rs:321`; test: `crates/waml-cli/tests/lsp_e2e.rs::snapshot_queries_are_advertised_unicode_exact_and_revision_current_over_stdio`.
- `LSP-004` — implementation: `crates/waml-cli/src/lsp/server.rs:61` and `crates/waml-cli/src/lsp/server.rs:301`; test: `crates/waml-cli/tests/lsp_e2e.rs::snapshot_queries_are_advertised_unicode_exact_and_revision_current_over_stdio`.
- `LSP-005` — implementation: `crates/waml-cli/src/lsp/server.rs:61` and `crates/waml-cli/src/lsp/server.rs:335`; test: `crates/waml-cli/tests/lsp_e2e.rs::snapshot_queries_are_advertised_unicode_exact_and_revision_current_over_stdio`.
- `VSCODE-001` — implementation: `editors/vscode/src/serverPath.ts:34`; partial tests: the environment, explicit setting, bundled binary, and environment-over-setting cases in `editors/vscode/src/serverPath.test.ts`.
- `VSCODE-002` — implementation: `editors/vscode/src/extension.ts:19`; tests: `editors/vscode/src/serverPath.test.ts::starts the configured executable once with stdio, restart policy, and markdown initialization` and `editors/vscode/src/serverPath.test.ts::pins the installed client's bounded default error and crash-restart behavior`.
- `VSCODE-003` — implementation: `editors/vscode/src/extension.ts:19`; test: `editors/vscode/src/serverPath.test.ts::reports an unresolved executable without constructing a client`.
- `VSCODE-004` — implementation: `editors/vscode/src/extension.ts:19`; test: `editors/vscode/src/serverPath.test.ts::stops the previous client before a repeated activation starts another`.
- `VSCODE-005` — implementation: `editors/vscode/src/extension.ts:19`; test: `editors/vscode/src/serverPath.test.ts::cleans up and reports a launch failure`.
- `VSCODE-006` — implementation: `editors/vscode/src/extension.ts:76`; test: `editors/vscode/src/serverPath.test.ts::deactivate stops the active client once and clears it`.
- `VSCODE-008` — implementation: `editors/vscode/src/extension.ts:19`; test: none.

# Verification gaps

- `BROWSER-001` — target: browser; The browser E2E proves that a share fragment overrides site boot configuration, but it does not provide competing API and bundle query sources or assert the full boot priority.
- `BROWSER-002` — target: browser; The headed serve check runs as top-level script code, so no addressable browser test call asserts startup from the printed API URL and fragment token without a console panic.
- `BROWSER-003` — target: browser; The host test selects the URL but no browser test asserts that this query boot fetches and opens the bundle.
- `BROWSER-005` — target: browser; Host tests cover message construction, but no headed browser test observes the action error for a failed bundle URL boot.
- `BROWSER-007` — target: browser; The headed serve check performs the authenticated same-origin read as top-level script code, but no addressable browser test call asserts the HTTP 200 result.
- `BROWSER-008` — target: browser; The exact host route test proves the server 403 result, but no browser test distinguishes server rejection from browser CORS enforcement.
- `BROWSER-009` — target: browser; The headed serve check performs the baseline-guarded document write as top-level script code, but no addressable browser test call asserts the API result and changed disk bytes.
- `BROWSER-010` — target: browser; Host tests cover the 409 wire contract, but no browser test causes a conflicting save and observes the browser result.
- `BROWSER-012` — target: browser; The verifier checks artifact structure before publication, but no browser test asserts a deployed artifact has every referenced resource.
- `BROWSER-015` — target: browser; The wasm startup path calls report_action_error after share decoding fails, but no headed browser test observes the start screen and visible action error.
- `BROWSER-016` — target: browser; The host test proves the fragment shape and precedence, but no headed browser test drives an editor save and observes browser_update_url.
- `BROWSER-017` — target: browser; The route test proves the HTTP response, but no headed browser test reads or presents served diagnostics.
- `BROWSER-018` — target: browser; Host route tests prove success and rejection, but no headed browser test performs an operations API batch or observes its result.
- `BROWSER-019` — target: browser; Host route tests prove 401 responses, but no headed browser test observes the missing-token refusal.
- `BROWSER-020` — target: browser; The host route test proves the 403 guard, but no headed browser test observes the anti-CSRF client-header refusal.
- `CLI-003` — target: native; The browser E2E invokes waml share --fragment-only as one step, but no native CLI test asserts share fragment or share URL output.
- `CLI-008` — target: native; The List command is source-evidenced, but no targeted CLI E2E test asserts its output and type filtering.
- `CLI-009` — target: native; The Bundle command is source-evidenced, but no targeted CLI E2E test asserts JSON and TypeScript artifact output.
- `CLI-010` — target: native; The Fmt command source sets a non-zero check result for changed files, but no CLI E2E asserts the noncanonical fmt --check exit.
- `CLI-011` — target: native; The mutation dispatch is source-evidenced, but the cited CLI E2E covers only the separate attribute-add row.
- `VSCODE-001` — target: native; No test proves a runnable bare waml command on PATH; the existing PATH test proves only the missing-command error path.
- `VSCODE-008` — target: native; The extension wires the action to workbench.action.openSettings, but the mocked VS Code test never selects the action and does not assert the command.

# Open discrepancies

- `BHV-BRW-022` remains an implementation discrepancy. A non-success API boot
  HTTP response is logged but does not produce the visible error that the goal
  requires.
- `BHV-BRW-021` is a stale-document discrepancy corrected in
  `publish-a-site.md`. Task 12 must reconcile its frozen state after this task.
- `BHV-LSP-007` is a stale-document discrepancy corrected in
  `language-server.md`. Task 12 must reconcile its frozen state after this task.
- `BHV-BRW-014` is owned by `goals/mvp.md`, which is outside Task 10. Task 12
  must reconcile it; `serve-locally.md` does not duplicate the record.

# Feature gaps

- [FG-001](../../../waml/waml-feature-gaps.md#fg-001--platform-and-capability-predicates)
  records that browser applicability and required capabilities are prose, not
  typed scenario predicates.
- [FG-010](../../../waml/waml-feature-gaps.md#fg-010--scenario-to-evidence-traceability)
  records the manual scenario-to-use-case-to-test traceability seam for all 41
  shipped scenarios.
- The frozen unsupported row `BHV-BRW-013` names `FG-014`, which is not in the
  current feature-gap ledger. Task 12 must reconcile that frozen reference. This
  task does not invent or edit a gap outside its file ownership.
