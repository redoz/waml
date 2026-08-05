# Issue 33 — MarkdownEditor's 54 fields hide a hand-reset state machine

**Verdict: APPROVE (all three sub-items survive as of 2026-08-04, HEAD 2fdb5ff9).**

## Context

`crates/waml-markdown-editor/src/widget.rs` defines `MarkdownEditor` (struct at
:463-582) with ~50 fields. Three clusters cause maintenance drag:

1. A ten-field layout/motion pipeline reset by enumeration in
   `clear_presentation` (:1664-1676).
2. Eight `DrawText` faces plumbed by hand through `WidgetFonts` (:151-185),
   `configure_face` (:164-184), and a per-install eight-clone block in
   `install_layout` (:1345-1366).
3. Gutter geometry from hardcoded font metrics (:455-461).

## Verdict evidence

### Sub-item 1 — reset-by-enumeration has already drifted (VALID, with nuance)

`clear_presentation` (:1664-1676) resets `installed`, `target_layout`,
`previous_layout`, `frame_layout`, `pending_cause`, `pending_invalidation`,
`last_layout_width`, `next_frame` — and misses `draw_commands_cache`,
`scroll_y`, and `motion`. `draw_commands_cache` was added in commit 28cbb990
(P-6) without updating `clear_presentation`: the enumerated reset drifted
exactly as predicted.

Nuance on severity (this is a smell + resource bug, not a wrong-pixels bug
today):

- The stale `draw_commands_cache` cannot paint the wrong document: its reuse
  check (:932-937) keys on `Arc::ptr_eq` against the current `installed` and
  `layout`, so a new document always misses. But the stale entry **pins the
  old document's `InstalledPresentation`, `LayoutSnapshot`, selections, and
  full command list in memory** until the next draw of that widget — memory
  retention across document swap.
- Stale `scroll_y` self-corrects at the next `draw_walk_with_session`
  (:834-839 re-syncs from `session.scroll()`), but between `clear_presentation`
  and that draw, event handlers (:770, :785, :790) translate pointer positions
  using the old document's `scroll_y` — a real, if narrow, hit-test window.
- `motion` (the `MotionController`) is never reset either; only `next_frame`
  being defaulted keeps it from being sampled.

The structural point stands: three fields have already fallen out of an
enumerated reset, and the next field added will too.

### Sub-item 2 — eightfold font plumbing (VALID)

Adding a text face requires coordinated edits at: `TextFace` enum (:122-132),
`text_face()` match (:134-149), `WidgetFonts` struct (:151-161),
`configure_face` match (:164-184), the eight `#[live] DrawText` fields
(:512-527), and the eight-clone block in `install_layout` (:1345-1366) that
runs on **every** layout install (each `FontFamily::clone()` per install, 8x,
including pure invalidation-driven reinstalls).

### Sub-item 3 — hardcoded gutter metrics (VALID)

`GUTTER_DIGIT_WIDTH = 6.6` and `GUTTER_ASCENT = 0.8` (:455-461) encode the
current mono face's advance/ascent at `GUTTER_FONT_SIZE = 11.0`. Used in
`gutter_width` (:983) and gutter label x-placement (:1049). A theme/font swap
silently misaligns line numbers; nothing measures or asserts these constants
against the shaper.

## Ordering / conflict flags

`crates/waml-markdown-editor/src/widget.rs` is edited by four approved plans:
issue 20 (pointer/gutter correctness fix), this one, issue 34 Tasks 1-2
(draw-path perf), and issue 31 Task 3 (log throttling).

This plan has the widest mechanical footprint — Task 1 renames ten fields to
live under `self.pipeline`, touching every reader in the file. **Land issue 20
first** (it is a small correctness fix that would otherwise have to be rewritten
against the renamed fields), then this plan, then the other two.
Recommended order: **20 → 33 → 34 (T1-2) → 31 (T3)**.

Note for Task 3: issue 20 changes how the event path derives the gutter width
(computing `self.gutter_width(session)` at event time). Task 3 here replaces the
hardcoded gutter constants with shaper-measured metrics — keep issue 20's
event-time recomputation intact rather than caching a measured value from the
draw path, or the draw/event coordinate agreement that issue 20 establishes is
lost again.

## Design decisions

- **D1:** Extract a `LayoutPipeline` struct owning the ten pipeline fields
  (`installed`, `target_layout`, `previous_layout`, `frame_layout`, `motion`,
  `pending_cause`, `pending_invalidation`, `last_layout_width`, `next_frame`)
  plus `draw_commands_cache` (it is invalidated by exactly the same events),
  with `reset()` doing a whole-struct `*self = Default::default()` so a new
  field can never be missed. `scroll_y` stays on the widget (it is also
  scrollbar state) but `clear_presentation` explicitly zeroes it.
- **D2:** Do NOT move the `#[live] DrawText` and palette fields — the live
  system requires them flat on the widget (per issue text and `#[live]`
  attribute mechanics). Only `#[rust]` fields move.
- **D3:** Collapse `WidgetFonts` to `[Option<FontFamily>; 8]` indexed by
  `TextFace as usize`, and skip the re-clone in `install_layout` when the
  families are already populated and unchanged (compare by
  `FontFamily` identity if cheap, else populate once and refresh only on
  live-apply).
- **D4:** For the gutter, measure one digit ("0") through the shaper at layout
  time (the shaper seam already exists in `install_layout` via
  `MakepadTextShaper`) and derive width/ascent from that, caching per
  font-family+size. Fall back to the current constants if measurement fails.

### Task 1 — Extract LayoutPipeline and fix the reset drift

- In `widget.rs`, define `#[derive(Default)] struct LayoutPipeline` holding the
  nine `#[rust]` pipeline fields plus `draw_commands_cache`
  (`NextFrame: Default` holds; verify `MotionController: Default`).
- Replace the flat fields on `MarkdownEditor` with `pipeline: LayoutPipeline`;
  mechanical `self.installed` -> `self.pipeline.installed` etc. across
  `widget.rs` only (fields are private, no cross-crate fallout).
- `clear_presentation` becomes `inner.pipeline = LayoutPipeline::default();
  inner.scroll_y = 0.0; inner.redraw(cx);` — the drift bug (stale
  `draw_commands_cache`, `scroll_y`, `motion`) is fixed by construction.
- Test: unit test in `widget.rs` tests module — install a presentation, drive
  one cached-draw-commands build, call `clear_presentation`, assert
  `draw_commands_cache.is_none()`, `scroll_y == 0.0`, and all pipeline
  Options are `None`.
- Gate: `cargo test -p waml-markdown-editor` plus workspace gate.

### Task 2 — Index fonts by TextFace and stop the per-install clones

- Replace `WidgetFonts`'s eight named fields with `faces: [Option<FontFamily>; 8]`;
  add `impl TextFace { const COUNT: usize = 8; fn index(self) -> usize }` (or a
  `match`-free `as usize` on a `#[repr(usize)]` enum).
- `configure_face` becomes `self.faces[face.index()].as_ref()`.
- In `install_layout` (:1345-1366), replace the eight clone statements with a
  loop over `TextFace`-to-`DrawText` pairs; add a helper
  `fn draw_text_for(&self, face: TextFace) -> &DrawText` so the mapping lives
  in one place. Refresh the array only when empty or after live-apply
  (`after_apply` hook), not every install.
- Test: existing rendering/e2e tests in the crate must stay green; add a unit
  test asserting `text_face(metrics).index()` round-trips for all 8 metric
  combinations.

### Task 3 — Measure gutter digit metrics through the shaper

- Add `struct GutterMetrics { digit_width: f64, ascent: f64 }` with a cache
  keyed on (mono font family id, `GUTTER_FONT_SIZE`).
- At the top of `install_layout` (after fonts are populated), shape the single
  glyph "0" in the mono face at `GUTTER_FONT_SIZE` via `MakepadTextShaper` and
  fill the cache; keep `GUTTER_DIGIT_WIDTH`/`GUTTER_ASCENT` as documented
  fallbacks when shaping is unavailable (headless tests).
- Thread `GutterMetrics` into `gutter_width` (:972-983) and `paint_gutter`
  (:1049 label x placement).
- Test: assert measured digit width for the shipped mono face is within 5% of
  6.6 (regression tripwire — fires if the theme font swaps silently);
  visual verification of gutter alignment required (GUI limit, state it in the
  commit).

### Task 4 — Gate and visual verification

- `cargo test --workspace`; run the native editor on a markdown fixture with
  line numbers on, verify gutter alignment and that document swap shows no
  scroll/cache carryover (open doc A, scroll, swap to doc B, confirm top-of-doc
  and correct content).
