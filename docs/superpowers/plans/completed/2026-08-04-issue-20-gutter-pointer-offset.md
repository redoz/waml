# Issue 20 — Markdown-editor pointer events ignore the gutter offset

## Context

`MarkdownEditorWidget` reserves a left gutter for line numbers. The draw path
shifts all content right by that gutter:

- `crates/waml-markdown-editor/src/widget.rs:852`
  `let content_origin = viewport.pos + dvec2(gutter, 0.0) - self.scroll_bars.get_scroll_pos();`

But the pointer path converts hit positions into layout space **without** the
gutter (and copy-pastes the same expression three times):

- `widget.rs:770` (FingerDown): `event.abs - area.rect(cx).pos + dvec2(0.0, self.scroll_y)`
- `widget.rs:785` (FingerMove): same expression
- `widget.rs:790` (FingerUp / tap): same expression, feeding `navigation_position` (:796) and `self.embedded_at(point)` (:804)

With `LineNumberMode != Off`, `gutter_width()` (`widget.rs:973`) returns a
positive width (`gutter_width(last_line+1, GUTTER_DIGIT_WIDTH, GUTTER_GAP)`,
`src/gutter.rs:213`), so every pointer point is too far **right** in layout
space by the gutter width (~36px for 3-digit files). Symptoms: clicks place the
caret left of the clicked glyph, primary-click link navigation and embedded
block activation hit the wrong target or miss.

Verified current at worktree HEAD (recent commits `28cbb990`, `78c604a9`
touched draw caching / reading view, not the event conversion). No existing
test exercises pointer conversion with line numbers on (`tests/widget_parity.rs`
drives `EditorInput` directly, bypassing the Hit conversion).

## Verdict evidence

APPROVE.

- Draw origin includes gutter: `widget.rs:852`.
- Event conversion omits it in all three sites: `widget.rs:770,785,790`.
- Gutter is nonzero whenever `line_numbers != LineNumberMode::Off`:
  `widget.rs:973-984`.
- Draw and hit-test therefore disagree on the same coordinate space —
  exactly the class of bug `.claude/rules/correctness.md` flags ("draw-time
  rects and event-time positions in the same coordinate space").

## Ordering / conflict flags

`crates/waml-markdown-editor/src/widget.rs` is edited by four approved plans:
this one, issue 31 (Task 3, log throttling), issue 33 (LayoutPipeline / font
array / gutter metrics), and issue 34 (Tasks 1-2, draw-path perf).

**Land this plan FIRST.** It is the smallest diff, it is the only correctness
fix of the four, and issue 33 subsequently moves the pipeline fields and
reworks `gutter_width` — doing 33 first would force this fix to be rewritten
against renamed fields. Recommended file order: **20 → 33 → 34 (T1-2) → 31 (T3)**.

## Design decisions

1. **One shared helper, pure and unit-testable.** Add a free function (or
   inherent fn taking plain values, not `Cx`) in `widget.rs`:

   ```rust
   /// Translate a window-absolute pointer position into layout space:
   /// remove the widget origin and the line-number gutter, add back scroll.
   fn abs_to_layout_point(abs: DVec2, area_origin: DVec2, gutter: f64, scroll_y: f64) -> DVec2 {
       abs - area_origin - dvec2(gutter, 0.0) + dvec2(0.0, scroll_y)
   }
   ```

   Keeping it free of `Cx`/`Area` makes it testable headless, per
   testability.md ("could the logic be lifted out and unit-tested").

2. **Gutter value at event time.** The event path has `session`, so call
   `self.gutter_width(session)` once at the top of the Hit handling (before the
   match) and pass it to the helper at all three sites. This matches the value
   the draw path used (`draw_walk_with_session` computes the same
   `self.gutter_width(session)` at :841). Do NOT cache the gutter from draw
   into a field — recomputing keeps event and draw in agreement even if the
   mode or line count changed between frames.

3. **Vertical scroll only, unchanged.** The existing conversion adds only
   `scroll_y`; the draw path subtracts the full 2-D scroll pos. Horizontal
   scrolling is not used by this widget today (wrapped layout), so keep the
   helper y-only-scroll to match current behaviour, but note it in the doc
   comment. Widening to 2-D scroll is out of scope.

4. **Clamp is not needed.** Negative x (a click inside the gutter itself)
   already flows through `point_to_source`/hit-tests the same way a click left
   of the text did before; behaviour for gutter clicks stays "nearest position
   on that line", which is the desired feel. No special-casing.

## Tasks

### Task 1: Add `abs_to_layout_point` helper and use it at all three sites

- File: `crates/waml-markdown-editor/src/widget.rs`
- Add the pure helper near the other free helpers (or as a private fn in the
  widget's impl block taking plain values), with the doc comment explaining the
  draw-side counterpart at the `content_origin` computation (:852) so the two
  stay linked for future readers.
- In the Hit-handling function containing lines 768-814, compute
  `let gutter = self.gutter_width(session);` once, then replace all three
  copies of `event.abs - area.rect(cx).pos + dvec2(0.0, self.scroll_y)` with
  `abs_to_layout_point(event.abs, area.rect(cx).pos, gutter, self.scroll_y)`.
- Gate: `cargo test -p waml-markdown-editor` stays green;
  `cargo clippy -p waml-markdown-editor -- -D warnings` clean.

### Task 2: Unit tests for the helper, including gutter-on

- File: `crates/waml-markdown-editor/src/widget.rs` (a `#[cfg(test)] mod` next
  to the helper) or `tests/widget_parity.rs` if the helper is `pub(crate)`
  re-exported — prefer the inline test mod since the helper is private.
- Tests:
  - `gutter_off_matches_old_translation`: gutter = 0.0 reproduces
    `abs - origin + (0, scroll_y)` exactly.
  - `gutter_on_shifts_x_left`: with gutter = 36.0, x is 36 less than the old
    expression; y unchanged.
  - `realistic_gutter_value`: use `gutter_width(100, GUTTER_DIGIT_WIDTH,
    GUTTER_GAP)` from `src/gutter.rs` so the test breaks if the gutter formula
    and the pointer path ever drift apart.
- Gate: `cargo test -p waml-markdown-editor`.

### Task 3: Visual verification with line numbers enabled

- The gate cannot assert makepad hit-testing end to end (per
  `.claude/rules/testability.md` GUI limits), so verify manually:
  - Run the native editor on a markdown fixture, enable line numbers
    (Absolute), click mid-word at several lines — caret must land on the
    clicked glyph, not ~36px left of it.
  - Primary-click a link and an embedded block near their left edge — both
    must activate; then click just left of them (inside the former error
    band) — must NOT activate.
  - Repeat once with `LineNumberMode::Off` to confirm no regression.
- Record the observed results in the implementation commit message or PR note.
