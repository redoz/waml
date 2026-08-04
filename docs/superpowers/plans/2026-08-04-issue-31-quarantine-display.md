# Issue 31 — Quarantine messages are Debug dumps shown to users (P2)

## Context

Three related observability defects where user-facing failure text is a `{:?}`
Debug dump, or where failure information is dropped entirely.

### Verdict evidence (verified 2026-08-04 at worktree HEAD)

1. **`crates/waml/src/analysis.rs:660-664`** — `impl Display for AnalysisError`
   is `write!(f, "analysis error: {self:?}")`. This string is stored as the
   user-facing quarantine message via `quarantined.insert(path, format!("{error}").into())`
   at `analysis.rs:1194`, `:1211`, and `:1296`. A user whose document is
   quarantined sees a raw Rust Debug struct dump. **SURVIVES.**

2. **`crates/waml-editor/src/class_diagram_view.rs:742-744`** — the
   `ToggleExpand` re-solve calls `build_scene` and dumps `diags` into
   `log!("diagnostic: {d:?}")`, while the sync path routes the same
   diagnostics to `self.set_scene_diagnostics(cx, body, &diags)` at `:457`
   (and `:365`). Commit `88f83472` added the statusbar routing (O-2) but
   missed the ToggleExpand arm. **SURVIVES.**

3. **`crates/waml-markdown-editor/src/widget.rs`** —
   - `:664` (`draw_walk`) and `:652` (`handle_event`) log
     `log!("... failed: {error:?}")` with no dedup; a persistent failure
     (e.g. `StalePresentation` steady state) logs every frame and buries
     the console. **SURVIVES.**
   - `:1652-1654` (`install_presentation`): `if presentation.validate().is_err() { return; }`
     — validation failure is silently swallowed; no log, no diagnostic, the
     editor keeps showing the old revision with no evidence why. **SURVIVES.**

## Ordering / conflict flags

- **Task 1 (`waml/src/analysis.rs`)** — issue 30 moves the UML highlighting
  block out of this file and issue 34 Task 4 rewrites `OkfAnalysis::code_spans`.
  Task 1 here edits `impl Display for AnalysisError` (:660), a disjoint region;
  safe in any order, but do not run concurrently with either.
- **Task 3 (`waml-markdown-editor/src/widget.rs`)** — shared with issues 20, 33,
  and 34. Issue 33 moves the pipeline fields into a `LayoutPipeline` struct and
  this task adds two new tracking fields to the same widget. **Land Task 3
  LAST** of that cluster: order **20 → 33 → 34 (T1-2) → 31 (T3)**.
- **Task 2 (`waml-editor/src/class_diagram_view.rs`)** — no other approved plan
  edits this file (issue 36 sub-item 1, which would, is deferred). Standalone.

## Design decisions

- **Display arms**: write real `Display` arms for every `AnalysisError`
  variant, not just `SourceTooLarge` and `Shell` — the enum is small (8
  variants) and a match without catch-all keeps future variants honest.
  Messages are written for the document author ("document exceeds the N-byte
  source limit"), carry the path where available, and delegate to inner
  errors' own `Display` (`ParseError`, `okf::BundleError`) rather than `{:?}`.
- **ToggleExpand diagnostics**: reuse the existing `set_scene_diagnostics`
  seam — one-line routing fix, keep the `log!` only if the sync path also
  logs (it does not; drop it for parity).
- **Markdown editor log throttling**: log once per *distinct* error. Store
  the last-logged message (`Option<String>` field on `MarkdownEditor`,
  separate fields for the event and draw paths) and skip the `log!` when the
  formatted error equals the stored one; clear the field on success so a
  recurrence after recovery logs again.
- **install_presentation swallow**: on `validate()` failure, log the
  validation error with the revision, once (same distinct-error guard style).
  Surfacing it as a user diagnostic is out of scope here — the caller seam
  for editor-level diagnostics does not exist in this widget; the log names
  the cause and the revision so a debugger can act.

## Tasks

### Task 1: Real Display for AnalysisError

- File: `crates/waml/src/analysis.rs` (impl at :660).
- Replace the `{self:?}` Display with an exhaustive `match` over all
  variants: `SourceTooLarge` ("document '{path}' is too large to analyze
  ({bytes} bytes)"), `Shell` ("failed to parse '{path}': {source}" using
  `ParseError`'s Display), `Okf` (delegate), `CatalogInvariant`,
  `InvalidPromotedMarkdownUpdate` (match the reason enum to a sentence),
  `Specialization`, `AmbiguousClaim`, `StructuralInvariant` — no `_ =>` arm.
- `InvalidPromotedMarkdownUpdateReason` needs its own Display (or inline
  match) covering all 7 variants including the revision pairs.
- Tests: unit tests in `analysis.rs` asserting the `SourceTooLarge` and
  `Shell` messages contain the path and no `{` / `}` Debug braces; run
  `cargo test -p waml`.

### Task 2: Route ToggleExpand diagnostics to the statusbar

- File: `crates/waml-editor/src/class_diagram_view.rs`, `ToggleExpand` arm
  (~:729-753).
- Replace the `for d in &diags { log!(...) }` loop with
  `self.set_scene_diagnostics(cx, body, &diags);`, matching the sync path
  at :457. Note: call before borrowing the canvas, mirroring :457's
  ordering, to avoid a double borrow of `body`.
- Tests: gate only (widget path, no headless seam); state in the commit
  message that the statusbar routing was verified by expanding a node on a
  diagram with known scene diagnostics, or note it as visually unverified.

### Task 3: Throttle markdown-editor failure logs and surface install_presentation validation failure

- File: `crates/waml-markdown-editor/src/widget.rs`.
- Add `last_draw_error: Option<String>` and `last_event_error: Option<String>`
  fields (non-live, `#[rust]`) to `MarkdownEditor`; in `handle_event` (:652)
  and `draw_walk` (:664), format the error, log only when it differs from
  the stored value, store it; set the field to `None` on the Ok branch.
- In `install_presentation` (:1652), bind the `Err(e)` from `validate()`,
  `log!` it with the presentation revision before returning; apply the same
  once-per-distinct-error guard if a field is reachable (this is an
  `impl MarkdownEditorRef`-style method on the ref — the guard field lives
  on the inner widget, set inside the existing `borrow_mut`; if borrow
  fails, log unconditionally).
- Tests: gate (`cargo test --workspace`); log behavior is not unit-testable
  here — state in the commit that steady-state StalePresentation now logs
  once, verified by reading the code path.
