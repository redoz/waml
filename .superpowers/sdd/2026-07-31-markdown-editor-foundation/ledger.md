# Markdown Editor Foundation SDD Ledger

## Task 6 clean re-review

- Re-reviewed at base `5aec24cf`.
- Task 6 implementation and typed IME commit failure fix are committed.
- The only unstaged production changes are the two pre-existing hunks in `crates/waml-editor/src/app.rs`.
- No Task 6 follow-up remains open.

## Task 7 start

- Started from base `5aec24cf`.
- Scope: variable-metric layout geometry and exact queries only.
- RED: `layout_geometry` failed with unresolved Task 7 geometry imports.
- GREEN: geometry queries, preferred-pixel session motion, reset behavior, and revision rejection pass.
- Verification: `layout_geometry` 5 passed, `unicode_ime` 8 passed, full crate 31 passed.

## Task 7 approval

- Approved at commit `e0549f4b`.
- No Task 7 follow-up remains open.

## Task 8 start

- Started from base `e0549f4b`.
- Scope: shaping, wrapping, fallback, incremental summaries, and viewport virtualization only.
- RED: `layout_geometry` failed with unresolved Task 8 layout-engine imports.
- GREEN: mixed-metric wrapping, viewport virtualization, width-only rewrap, and editable fallback pass.
- Verification: `layout_geometry` 9 passed, `unicode_ime` 8 passed, full crate 35 passed.

## Task 8 final approval

- Approved at commit `4f7e419e` after the geometry-contract, authoritative-bidi, and double-reorder fixes.
- No Task 8 follow-up remains open.

## Task 9 start

- Started from base `4f7e419e`.
- Scope: platform-neutral input, retained selection behavior, read-only behavior, and caret scrolling only.
- RED: `widget_parity` failed with unresolved Task 9 controller/input/scroll types and missing `set_read_only`.
- GREEN: retained pointer gestures, additive and extended selection, read-only copy/mutation suppression, and geometry-based caret scrolling pass.
- Verification: `widget_parity` 4 passed, `layout_geometry` 12 passed, `unicode_ime` 8 passed, full crate 44 passed.

## Task 9 final approval

- Approved at commit `9dde907b` after stale-entry and post-mutation IME geometry fixes.
- No Task 9 follow-up remains open.

## Task 10 start

- Started from base `9dde907b`.
- Scope: one borrowed-session Makepad widget, typed action forwarding, and one ordered layout draw pipeline only.
- TDD state: mounted widget tests not yet added.
## Task 10

- Base: `9dde907b4a8bc26d1eea8581ca9224494f18c44b`
- Scope: Add the Makepad `MarkdownEditor` widget, its action and reference APIs, ordered draw layers, and provenance guards.
- RED: `cargo test -p waml-markdown-editor --test widget_parity` failed on the missing widget surface and reference extension.
- GREEN: The focused suite passes 10 tests. The complete crate passes 50 tests in 7 suites.
## Task 10 amended host boundary

- Makepad `Scope::with_data<T: Any>` cannot carry the planned non-`'static` borrowed session wrapper.
- Approved replacement: explicit host-driven event, input, and draw methods take `&mut MarkdownDocumentSession`.
- The widget owns no source or session. It obtains the matching snapshot from the passed session.
- Typed layout and controller errors expose revision mismatches.
- Focused verification: widget 14, provenance 2, layout 12, Unicode and IME 8; full crate 54.
- Task 10 draw follow-up uses real Makepad primitives for available neutral geometry. Link and decoration metadata is absent, so the widget does not fabricate it from source parsing.
- Task 10 draw follow-up verification: widget 15, provenance 2, layout 12, Unicode and IME 8; full crate 55.
## Task 10 final approval

- Approved at `70cd0b73b576f9ac81dd5d68f17833d2286c2a85`.
- Minor follow-up: a native `FingerUp` window/pass harness does not yet cover embedded activation.

## Task 11 start

- Base: `70cd0b73b576f9ac81dd5d68f17833d2286c2a85`.
- Scope: tests and provenance only, as listed in `task-11-brief.md`.
- TDD: exact named widget evidence added first; focused suite is GREEN with 23 tests.
## Task 11 complete

- Tests and provenance only; no production API changes.
- GREEN: document 19, Unicode and IME 13, layout 15, widget 23, full crate 74 in 7 suites.
- No ignored tests. Fuzzing remains explicitly deferred to its later plan.
## Task 11 final approval

- Approved at `2d10a9db89b727e0f177babadb25a0478072c8d5` with no follow-up.

## Task 12 start

- Base: `2d10a9db89b727e0f177babadb25a0478072c8d5`.
- Gates will run in the exact plan order.
- Environment: Windows NT 10.0.26200.0, rustc 1.98.0-nightly ce9954c0c, cargo 1.98.0-nightly a595d0da2.
- The existing `app.rs` diff is one three-line comment hunk and must remain byte-for-byte unchanged.
## Task 12 complete

- Foundation formatting check, 74 focused tests, 1,729 workspace tests, strict Clippy, provenance,
  forbidden-import scan, ignored-test scan, and dependency inspection pass.
- Global formatting check differs only at the preserved user-owned `app.rs:1639` expression.
- Cargo reports two upstream Makepad duplicate-package selection warnings; no Rust lint warning remains.
- Syntax fuzz iterations: 0 due to the platform limitation. No editor fuzz coverage is claimed.

## Task 12 final approval

- Independently approved at `c715eff5`.
- The markdown editor foundation is complete and awaiting whole-plan review.

## Whole-plan review

Status: **NOT SAFE pending fix waves and re-review.** Task-level approvals and
the Task 12 gates do not prove whole-plan safety.

Critical findings:

1. The widget raw `DrawText` path reshapes source instead of consuming the
   authoritative `LayoutSnapshot` glyph placements and font attributes.
2. Task 8 has no stable-boundary dirty-interval convergence or `BlockFlow`
   convergence proof.
3. The visible-only block vector is indexed with document indexes and can panic
   after scrolling. Scroll authority is split, and the authoritative scroll is
   not applied consistently.

Important findings:

- Accepted edits and all-document invalidation can leave stale widget layout.
- `apply_edit` cancels IME before validation; undo and redo pop history before
  fallible application succeeds.
- The widget erases typed edit errors.
- Read-only authority is split and still permits mutation leaks.
- Parity tests depend too heavily on façades instead of the integrated paths.

Minor evidence findings:

- Task 5 had no report. It is reconstructed in `task-5-report.md` with explicit
  evidence limits.
- Nine review diff artifacts contained literal `System.Object[]`. They are
  replaced by the exact commit range index in `review-ranges.md`.
- The design written-spec status was stale.
- Syntax fuzzing remains 0 iterations and deferred. No fuzz coverage is
  claimed.
- Global formatting remains clean for the foundation crate. The global check
  differs only at the preserved user-owned `app.rs:1639` formatting hunk.
  Cargo's two Makepad duplicate-package messages remain external warnings.

Fix-wave assignments:

- Wave A: session atomicity, read-only authority, and typed error behavior —
  `foundation_controller` and `task5_implementer`.
- Wave B: authoritative glyph placement, stable-boundary and flow convergence,
  and virtualization/scroll indexing — `task5_rereview`.
- Wave C: evidence repair — this evidence owner.
- Widget integration follows Wave B. Whole-plan re-review follows all assigned
  waves; no safe status is claimed before that review.
