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

# Verification gaps

None.

# Open discrepancies

None.

# Feature gaps

- `FG-009` — WAML has no first-class component ports, asynchronous work, or compare-and-swap transaction notation. The ownership views use dependency relationships and prose notes.
