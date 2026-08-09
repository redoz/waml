# Changed files

- `docs/waml/architecture/concepts/implementation/index.md`
- `docs/waml/architecture/concepts/implementation/affected-analysis.md`
- `docs/waml/architecture/concepts/implementation/app-shell.md`
- `docs/waml/architecture/concepts/implementation/diagram-renderer.md`
- `docs/waml/architecture/concepts/implementation/document-host.md`
- `docs/waml/architecture/concepts/implementation/editor-session.md`
- `docs/waml/architecture/concepts/implementation/markdown-editor.md`
- `docs/waml/architecture/concepts/implementation/markdown-syntax.md`
- `docs/waml/architecture/concepts/implementation/okf-analysis.md`
- `docs/waml/architecture/concepts/implementation/platform-adapter.md`
- `docs/waml/architecture/concepts/implementation/prepared-candidate.md`
- `docs/waml/architecture/concepts/implementation/source-bundle.md`
- `docs/waml/architecture/concepts/implementation/uml-analysis.md`
- `docs/waml/architecture/concepts/implementation/waml-cli-crate.md`
- `docs/waml/architecture/concepts/implementation/waml-core-crate.md`
- `docs/waml/architecture/concepts/implementation/waml-editor-crate.md`
- `docs/waml/architecture/concepts/implementation/waml-markdown-editor-crate.md`
- `docs/waml/architecture/concepts/implementation/waml-ops-dto-crate.md`
- `docs/waml/architecture/concepts/implementation/waml-syntax-crate.md`
- `docs/waml/architecture/views/crate-ownership.md`
- `docs/waml/architecture/views/editor-ownership.md`
- `docs/waml/architecture/overview.md`
- `docs/waml/architecture/index.md`
- `docs/waml/architecture/views/index.md`
- `docs/waml/architecture/views/preparation-pipeline.md`
- `docs/waml/architecture/views/incremental-analysis.md`
- `docs/waml/architecture/views/revisioned-edit-transaction.md`
- `docs/waml/architecture/views/deployment-surfaces.md`
- `docs/waml/architecture/views/system-context.md`
- `docs/waml/architecture/views/authoring-and-validation.md`
- `docs/waml/architecture/views/share-round-trip.md`
- `docs/waml/architecture/views/domain-model.md`
- `docs/waml/architecture/views/layout-solving.md`
- `docs/waml/architecture/views/model-vocabulary.md`
- `docs/waml/architecture/views/editing-round-trip.md`
- `docs/waml/architecture/views/web-delivery.md`
- `docs/waml/architecture/concepts/workflows/model-projection.md`
- `docs/waml/architecture/concepts/workflows/validation-and-diagnostics.md`
- `docs/waml/architecture/concepts/workflows/editor.md`
- `docs/waml/architecture/concepts/runtime/native-editor.md`
- `docs/waml/architecture/concepts/runtime/command-line-tool.md`
- `docs/waml/architecture/concepts/runtime/language-server.md`
- `docs/waml/architecture/concepts/runtime/native-web-delivery.md`
- `docs/superpowers/audits/reports/architecture.md`

# Scenario identifiers

None.

# Evidence

- The root `Cargo.toml` lists exactly six workspace members. Their production `[dependencies]` sections define the seven path-dependency arrows in `crate-ownership.md`.
- `waml-editor` also has a dev-only path dependency on `waml-syntax`. The production dependency view excludes it.
- `crates/waml/src/source.rs::SourceBundle` owns the source documents.
- `crates/waml/src/analysis.rs::OkfAnalysis`, `PreparedCandidate`, and `AffectedAnalysis` define candidate analysis state.
- `crates/waml/src/uml/analysis.rs::Analysis` owns UML analysis state.
- `crates/waml-editor/src/app.rs::App` owns shell composition and navigation coordination.
- `crates/waml-editor/src/editor_session.rs::EditorSession` owns the installed revisioned snapshot.
- `crates/waml-editor/src/document_host.rs::DocumentHost`, `navigation.rs`, and `doc_tabs.rs` define document, navigation, and tab ownership.
- `crates/waml-markdown-editor/src/widget.rs::MarkdownEditor` and `session.rs::MarkdownDocumentSession` define Markdown editor ownership.
- `crates/waml-editor/src/class_diagram_view.rs:192` (`ClassDiagramView`) and `behavior_doc_view.rs:692` (`BehaviorDocView`) coordinate projection, action routing, and document-view lifecycle.
- `crates/waml-editor/src/canvas/class/widget.rs:359` (`ClassDiagramSurface`) owns class scene, viewport, interaction, placement, and selection state.
- `crates/waml-editor/src/canvas/behavior/mod.rs:230` (`BehaviorSurface`) owns behavior scene, viewport, and selection state.
- `crates/waml-editor/src/document_host.rs:33` (`DocumentHost`) owns inactive per-tab selection, camera, and scroll anchors.
- `crates/waml-editor/src/doc_view.rs:27` (`BodyWidgets`) provides access to shared canvases, the inspector, tool dock, selection toolbar, and view toolbar.
- `crates/waml-editor/src/native_save.rs`, `platform_browser.rs`, `browser_boot.rs`, and `api_save.rs` define platform-adapter ownership.
- `crates/waml-editor/src/document_host.rs::reconcile_registry` and `prepared_preview_replacement_drops_the_old_live_view` establish that a removed tab cannot retain a live view.
- `crates/waml/src/analysis.rs:654` (`prepare_candidate`) starts immutable candidate preparation. `prepare_candidate_inner_with_markdown_updates` at line 756 builds OKF analysis before UML analysis.
- `crates/waml/src/analysis.rs:964` (`analyze_okf_inner`) reuses, reparses, or promotes Markdown syntax. It quarantines a shell-failed document instead of rejecting the complete bundle.
- `crates/waml/src/uml/analysis.rs:275` (`analyze`) visits every claimed concept and builds the projection at line 489. It then calls `analysis_metadata` at line 490 to compute affected closure, retained projections, and per-island freshness.
- `crates/waml-editor/src/editor_session.rs:539` (`apply_pending_with_preparer`) prepares a semantic edit before snapshot replacement. A failed preparation does not change session state.
- `crates/waml-editor/src/editor_session.rs:755` (`promote_source_edit`) validates an exact Markdown edit and installs its source-only revision. `install_semantic_completion` at line 887 rejects stale identity or revision results.
- `crates/waml-markdown-editor/src/session.rs:864` (`apply_edit_without_history`) validates the document revision and exact changes before `reparse_markdown`.
- `DocumentRevision` belongs to the Markdown snapshot. `EditorSession::promote_source_edit` advances a separate `session_revision`, and semantic installation also compares source `Arc` identity.
- `crates/waml-editor/src/app/actions.rs:983` runs semantic preparation and then installs its completion at line 985 in the same production call stack.
- `crates/waml-editor/src/app/workspace.rs:151` prepares every open tab. `crates/waml-editor/src/document_host.rs:377` reconciles that set and synchronizes the active view.
- `crates/waml-syntax/src/markdown/snapshot.rs:347` (`reparse_markdown`) reports incremental or full reparse and preserves reusable syntax identities.
- `crates/waml-cli/src/main.rs:28` (`Command`) defines separate check, format, index, query, mutation, share, site, serve, and LSP command surfaces.
- `crates/waml-cli/src/lsp/server.rs:61` (`server_capabilities`) advertises full sync, document symbols, definitions, document links, and full semantic tokens. Diagnostics are published from analyzed bundle snapshots.
- `editors/vscode/src/extension.ts:19` (`activate`) starts `waml lsp` for Markdown documents. It does not host the WAML editor.
- `crates/waml-editor/src/main.rs:4` starts the same `App` type used by conditional native and WebAssembly paths in `app.rs`.
- `crates/waml-editor/src/browser_boot.rs:48` (`select_browser_boot`) selects share, API, bundle, or start input for the browser editor.
- `crates/waml/src/share.rs:74` (`encode_source`) and line 97 (`decode_source`) implement the complete-bundle share fragment.
- `crates/waml-cli/src/site.rs:108` (`assemble_site`) writes the static editor artifact and its selected bundle or API boot configuration.
- `crates/waml-cli/src/serve/routes.rs:121`, line 195, and line 230 define bundle reads, semantic-operation writes, and revisioned document writes for local serve.

# Verification gaps

None.

# Open discrepancies

None.

# Feature gaps

- `FG-009` also applies to `revisioned-edit-transaction.md`, which uses messages and notes for compare-and-swap and the separate completion boundary.
- `FG-009` — WAML has no first-class component ports, asynchronous work, or compare-and-swap transaction notation. The ownership views use dependency relationships and prose notes.
