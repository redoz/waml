# Native Diagram Properties — Final Review Fix Wave

Date: 2026-07-27

Baseline: `d5c7070`

Implementation commit: this report is committed with `fix(editor): close diagram properties review gaps`

## Outcome

All 11 final-review findings were reproduced with focused tests, fixed, and verified. The
native editor now preserves document intent across editing, persistence, reload, and shell
refreshes. No Svelte UI or component code was changed.

The review scope expanded into the non-Svelte `packages/core`, `packages/okf`, and generated
`packages/wasm` surfaces only where the Rust DTO/operation contract is shared with TypeScript.
Leaving those consumers unchanged would have made the new tri-state multiplicity contract and
cardinality authority inconsistent across the supported boundaries.

## Findings and fixes

1. **Multiline diagram descriptions**
   - The Properties inspector now labels and presents description as a one-line field.
   - CRLF, LF, and CR input are normalized to spaces.
   - `DiagramSet` rejects multiline descriptions at the operation boundary, so non-UI callers
     cannot persist an invalid value.
   - Focused editor and operation tests cover normalization and rejection.

2. **`AttrSet` multiplicity could not distinguish omit from clear**
   - Added `FieldEdit<T>` with `Unchanged`, `Clear`, and `Set(T)` states.
   - Serde now maps those states to omitted, explicit `null`, and a string value.
   - Rust DTO, CLI flags, core adapter, generated Wasm declarations/runtime, and package tests
     use the same contract: `mult?: string | null`.
   - Tests prove type-only edits preserve multiplicity while explicit null clears it.

3. **Saving rewrote clean files and could overwrite external edits**
   - Native save now receives both the persisted baseline and current bundle.
   - Only changed paths are written.
   - External edits to unchanged files are preserved; conflicting dirty edits/deletions fail.
   - A disk file already matching the desired content is treated idempotently.
   - Baseline-only deletions are rejected before filesystem mutation because deletion is not yet
     a supported bundle operation.

4. **Document transitions could drop dirty changes**
   - Replacing a backing flushes the old backing before loading/installing the new one.
   - Failed flushes abort replacement and preserve the original document.
   - Close follows the same rule and keeps the editor open on failure.
   - Lifecycle tests cover successful close and both failure paths, including same-directory reopen.

5. **Legacy cardinality flag could override the enum**
   - `CardinalityVisibility` is authoritative whenever present.
   - The legacy boolean is only a fallback for documents without the enum and is normalized when
     persisting or adapting.
   - Parser, native display resolution, Rust DTO, OKF resolver, outbound core adapter, and inbound
     overlay normalization now agree.
   - Contradictory-input tests prove the enum wins.

6. **Rename left stale shell titles**
   - Open tabs reconcile titles without replacing tab identity, order, preview, or active state.
   - Relay outcomes refresh navigation, tabs, switcher, status, inspector elements, and the active
     scene after a diagram rename.
   - Runtime verification renamed `Orders` to `Orders Runtime`; tab, navigation row, title field,
     and status bar all refreshed, and the source persisted after debounce.

7. **Stereotype `Off` still rendered a metaclass fallback**
   - Scene nodes carry resolved stereotype visibility.
   - Focus rendering emits no eyebrow when visibility is off, including metaclass fallback text.
   - A behavioral interface test protects this path.

8. **Relationship labels used the wrong text/position**
   - Association references are no longer painted as relationship names.
   - Straight and bent labels use the polyline arc-length midpoint instead of a segment-count
     midpoint.
   - Focused tests cover both semantic text selection and geometry.

9. **Save preflight could create directories before discovering an unsafe target**
   - A complete lexical, duplicate/alias, conflict, and containment preflight runs before any
     directory creation or write.
   - The nearest existing ancestor is canonicalized to detect linked-parent escapes.
   - Tests prove a later unsafe target cannot leave earlier safe parents behind.
   - Per-file writes retain atomic replacement.

10. **Generated Wasm declaration omitted `clearDesc`**
    - Regenerated declarations expose `clearDesc?: boolean`.
    - Package build/tests protect the generated contract.

11. **Segmented control tests exercised a test-only replica**
    - Removed the `cfg(test)` offset replica.
    - Production `segmented_offset_index` is used by both the widget and its tests.

## TDD evidence

Each behavioral fix began with a focused regression test that failed against the baseline
behavior, followed by the smallest production change needed to make it pass. The final focused
suites include native-save conflict/containment cases, application transition lifecycle cases,
tab reconciliation, parser/display authority, operation serde/application, edge-label geometry,
stereotype visibility, and package adapter/display contract tests.

Notable final focused results:

- Native save suite: 12 passed.
- Native editor application suite: 18 passed.
- Wasm package tests: 7 passed across 4 files.
- OKF package tests: 53 passed across 3 files.
- Core package tests: 108 passed across 14 files.

## Final verification

- `cargo test --workspace`: **1165 passed** across 21 suites.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy -p waml-editor --all-targets -- -D warnings`: passed with no lint errors.
  Cargo emitted two pre-existing duplicate-Makepad-checkout package warnings.
- `cargo build -p waml-editor --bin waml-editor`: passed with the same two Cargo warnings.
- `pnpm --filter @waml/wasm build`: passed.
- `pnpm --filter @waml/wasm test`: passed.
- `pnpm --filter @waml/okf build` and package tests: passed.
- `pnpm --filter @waml/core build` and package tests: passed.
- `git diff --check`: passed.
- The committed fixture was restored byte-for-byte after runtime rename verification.

## Runtime and visual verification

A freshly rebuilt native editor was run against
`crates/waml-editor/tests/fixtures/mini` at 1280×840. The Properties view showed the new
`Description (one line)` label and placeholder with correct alignment and no clipping.

The retained untracked visual artifact is:

- `diagram-properties.png`

The temporary automation script and the transient rename screenshot were removed, and the test
process was stopped.

## Residual considerations

- Persistence is atomic per file, not transactional across the entire bundle. Idempotent retry
  makes a partially completed multi-file save recoverable, but a process failure can still leave
  a subset at the desired revision.
- A filesystem time-of-check/time-of-use window remains between canonical containment preflight
  and replacement. Eliminating it completely requires platform-specific handle-relative,
  no-reparse traversal.
- The linked-parent regression test runs where the host permits symlink creation and skips only
  when platform privileges prevent constructing the fixture.
- A bundle loaded through a linked directory may be readable and later rejected for saving by the
  stricter containment policy; aligning load-time policy is a separate hardening opportunity.
