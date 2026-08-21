# Diagram Properties Post-Rebase Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:test-driven-development and superpowers:systematic-debugging. Each production change must follow a witnessed RED/GREEN cycle against the real production path.

**Goal:** Resolve the five post-rebase staff findings without changing Svelte components or weakening compatibility at legacy DTO/parse boundaries.

**Architecture:** Keep the native view and quit fixes at their existing event boundaries. Give TypeScript diagram updates an explicit description-clear state that the Rust DTO already understands. Make cardinality the sole internal display authority while deriving the compatibility boolean only at parse/persistence/wire boundaries.

**Tech Stack:** Rust/Makepad, TypeScript/Vitest, serde/JSON DTOs, pnpm workspace.

## Global Constraints

- Preserve the untracked `diagram-properties.png` and all unrelated user edits.
- Do not edit Svelte components.
- Do not add tests that inspect source text; exercise real production behavior.
- Use strict TDD: write one focused regression, run it to an expected failure, make the minimum production change, and rerun it green before the next behavior.
- Keep `showAttributeMultiplicity` only at legacy parse, persisted frontmatter, and wire DTO boundaries; internal resolved/set types carry `cardinality` only.
- Run `cargo fmt --all -- --check`, `cargo test --workspace`, strict editor Clippy, and the core/OKF/Wasm build-test gates before completion.

---

### Task 1: Cancelable logo-menu Exit

**Files:**
- Modify: `crates/waml-editor/src/app/actions.rs`
- Modify only if the real integration harness belongs there: `crates/waml-editor/src/app.rs`

**Interfaces:**
- Consumes: `PopupResult::Invoked(live_id!(exit))`, `logo_command_for`, and `Cx::request_quit(QuitReason::Menu)`.
- Produces: the logo popup invocation enters `Event::QuitRequested`, so `prevent_quit_after_failed_save` can handle the request and prevent the final OS quit.

- [ ] Add a regression that invokes the real logo-menu command-dispatch path and observes a menu-reason quit request, including the handled/save-failure outcome.
- [ ] Run the focused editor test and verify RED because the path currently queues an unconditional `CxOsOp::Quit`.
- [ ] Replace only the Exit branch with `cx.request_quit(QuitReason::Menu)`.
- [ ] Rerun the focused test and verify GREEN.

### Task 2: Properties lifecycle and current diagram identity

**Files:**
- Modify: `crates/waml-editor/src/class_diagram_view.rs`
- Modify only if needed for real widget assertions: existing inspector/diagram-properties test utilities in `crates/waml-editor/src`.

**Interfaces:**
- Consumes: `ClassDiagramMode`, `BodyWidgets::diagram_properties`, `Cx::hide_text_ime`, `Cx::set_key_focus(Area::Empty)`, and current `Model.diagrams`.
- Produces: every `Properties -> Canvas` transition releases a focused descendant and IME; `Canvas -> Canvas` does not disturb unrelated focus. `ClassDiagramView` stores only the immutable diagram key and resolves title from the current model whenever inspector rows are synchronized.

- [ ] Add a focused-field regression that enters Properties and leaves through the real transition, then observes empty keyboard focus and hidden IME.
- [ ] Run the focused test and verify RED because visibility alone preserves descendant focus.
- [ ] Add one transition seam that reports whether Properties was actually left; clear focus/IME only on that branch and use it from close, Escape, tool-toggle, and deactivation paths.
- [ ] Rerun the focus regression and verify GREEN.
- [ ] Add a rename -> deactivate/reactivate regression that inspects the real diagram row text after sync.
- [ ] Run it and verify RED because `ClassDiagramView::title` still supplies the constructor title.
- [ ] Remove the stored title, keep constructor compatibility only if call sites require it, and pass the matching model diagram's current title to `diagram_elements` on every sync.
- [ ] Rerun the focused view tests and verify GREEN.

### Task 3: Explicit web description clearing through Rust

**Files:**
- Modify: `packages/core/src/state/ops-adapter.ts`
- Modify: `packages/core/src/state/ops-adapter.test.ts`
- Modify only if the store boundary must expose the sentinel: `packages/core/src/state/model.ts` and its tests.
- Modify: `crates/waml-ops-dto/src/lib.rs` tests only when coverage needs the TypeScript-shaped payload.
- Modify: `crates/waml/src/ops/mod.rs` tests only when application coverage needs the full persisted result.

**Interfaces:**
- Consumes: a diagram patch whose description clear is represented explicitly (nullable/sentinel at the non-Svelte adapter boundary) and the existing `OpDto::DiagramSet.clearDesc`.
- Produces: one `{ op: "diagram.set", key, clearDesc: true }` DTO and, after Rust DTO conversion/application, removal of the authored `description` frontmatter key.

- [ ] Add an adapter regression for the explicit clear representation; verify RED because the adapter cannot emit `clearDesc`.
- [ ] Add a Rust test that deserializes the exact TypeScript-shaped JSON and applies the resulting op to a diagram containing a description; verify the boundary/application behavior.
- [ ] Implement the smallest adapter/store normalization that lets the unchanged Svelte empty-field path reach the explicit clear representation, without treating omission as clear.
- [ ] Run the adapter and Rust focused tests and verify GREEN.

### Task 4: Cardinality-only internal display contracts

**Files:**
- Modify: `crates/waml/src/ops/mod.rs`
- Modify: `crates/waml-ops-dto/src/lib.rs`
- Modify: `crates/waml-editor/src/diagram_display.rs`
- Modify Rust callers/tests constructing `DiagramDisplaySet` or `ResolvedDiagramDisplay`, including `diagram_properties.rs`, `class_diagram_view.rs`, `edge_labels.rs`, and `main.rs`.
- Modify non-Svelte adapters/generated bindings only if a public boundary changes.

**Interfaces:**
- Consumes: legacy `DiagramDisplay.show_attribute_multiplicity: Option<bool>` and wire `DisplayDto.show_attribute_multiplicity`.
- Produces: internal `DiagramDisplaySet` and `ResolvedDiagramDisplay` with `cardinality` as the only multiplicity visibility state. Legacy booleans are derived via `CardinalityVisibility::legacy_attribute_gate()` when serializing wire/frontmatter data.

- [ ] First remove the duplicate fields from test constructions and add/adjust behavior regressions that prove legacy fallback and boundary derivation.
- [ ] Run the focused Rust tests and verify RED compile errors identify every internal caller still constructing the contradictory state.
- [ ] Remove the fields from production internal structs and conversions; keep legacy values only in model parse and DTO/persistence shapes.
- [ ] Rerun focused `waml`, `waml-ops-dto`, and `waml-editor` tests and verify GREEN.

### Task 5: Full verification and handoff

**Files:**
- Create: `.superpowers/sdd/2026-07-26-native-diagram-properties/post-rebase-fix-report.md`

- [ ] Run `cargo fmt --all -- --check` and inspect `git diff --check`.
- [ ] Run `cargo test --workspace`.
- [ ] Run strict editor Clippy with warnings denied for every target.
- [ ] Run focused and full package tests/builds for core, OKF, and Wasm using repository scripts.
- [ ] Automate the native focus/exit/title runtime checks where the existing harness supports them; document any genuinely non-automatable visual check.
- [ ] Request a final code review, address all Critical/Important findings, then rerun affected gates.
- [ ] Write the report with RED/GREEN evidence, gate outputs, runtime evidence, changed contracts, and remaining concerns.
- [ ] Commit the coherent fix wave without staging `diagram-properties.png`.
