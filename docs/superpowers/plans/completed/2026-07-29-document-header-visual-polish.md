# Document Header Visual Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give WAML's shared document header the approved compact developer-tool breadcrumb treatment without changing navigation behavior or public APIs.

**Architecture:** Keep layout, rendering, hit testing, and focused tests in the existing `document_header.rs` module. Extend its pure geometry helpers for padded segments, separator placement, and text centering; then have the widget draw a canvas-colored surface, bottom rule, and private Lucide-style chevron from that geometry.

**Tech Stack:** Rust, Makepad live design and SDF drawing, WAML Atlas theme tokens, Cargo tests.

## Global Constraints

- Work only in `C:\tmp\waml-document-header-visual-polish` on `codex/document-header-visual-polish`.
- Do not modify Makepad or its pin.
- Preserve navigation ownership, targets, public Rust APIs, start-screen collapse, drag-query behavior, right-dock placement, current-segment elision, and wide/narrow geometry.
- Reuse `atlas.canvas_ground`, `atlas.surface_border`, `atlas.text`, `atlas.text_dim`, `fonts.text_menu`, and `fonts.text_label`.
- Use a private single Lucide `chevron-right` SDF primitive; do not add a public `Icon` variant or use a Unicode glyph.
- Keep the header height and right-dock reservation at 30 pixels.

---

### Task 1: Refine the shared document header

**Files:**
- Modify: `crates/waml-editor/src/document_header.rs`
- Test: inline `document_header::tests` in `crates/waml-editor/src/document_header.rs`

**Interfaces:**
- Consumes: `layout_header(available_width: f64, label_widths: &[f64], right_button_width: f64) -> DocumentHeaderLayout`, existing `BreadcrumbSegment` values, and `DocumentHeaderState::action_at`.
- Produces: unchanged public signatures; private `separator_rect(left_segment: Rect) -> Rect` and `centered_text_y(row: Rect, font_size: f64) -> f64` helpers used by rendering and tests.

- [ ] **Step 1: Write failing geometry and hit-target tests**

Add constants and tests that lock the approved geometry:

```rust
const HEADER_PAD_X: f64 = 8.0;
const SEGMENT_PAD_X: f64 = 6.0;
const SEPARATOR_SLOT_W: f64 = 16.0;
const SEPARATOR_SIZE: f64 = 8.0;
const TEXT_DY: f64 = 1.0;

#[test]
fn layout_matches_reference_inset_padding_and_separator_spacing() {
    let layout = layout_header(200.0, &[20.0, 30.0], 0.0);
    assert_eq!(layout.segment_rects[0].1.pos.x, 8.0);
    assert_eq!(layout.segment_rects[0].1.size.x, 32.0);
    assert_eq!(layout.segment_rects[1].1.pos.x, 56.0);
    assert_eq!(layout.segment_rects[1].1.size.x, 42.0);

    let separator = separator_rect(layout.segment_rects[0].1);
    assert_eq!(separator.pos.x, 44.0);
    assert_eq!(separator.size, dvec2(8.0, 8.0));
    assert_eq!(layout.segment_rects[0].1.pos.x + SEGMENT_PAD_X, 14.0);
}

#[test]
fn text_is_pixel_snapped_and_optically_centered() {
    let row = Rect {
        pos: dvec2(10.25, 5.25),
        size: dvec2(80.0, DOCUMENT_HEADER_H),
    };
    assert_eq!(centered_text_y(row, 10.0), 16.0);
    assert_eq!(centered_text_y(row, 11.0), 16.0);
}

#[test]
fn padded_hit_rects_keep_original_navigation_targets() {
    let segments = vec![segment("Root", "root"), segment("Current", "current")];
    let layout = layout_header(180.0, &[24.0, 42.0], 0.0);
    let state =
        DocumentHeaderState::for_test(segments.clone(), None, layout.segment_rects.clone());

    for (index, rect) in &layout.segment_rects {
        assert!(rect.size.x >= 2.0 * SEGMENT_PAD_X);
        assert_eq!(
            state.action_at(rect.pos + rect.size * 0.5),
            Some(DocumentHeaderAction::Navigate(
                segments[*index].target.clone()
            ))
        );
    }
}

#[test]
fn constrained_positive_content_keeps_a_positive_current_hit_rect() {
    let layout = layout_header(55.0, &[44.0, 58.0], 30.0);
    assert_eq!(layout.visible_indices, vec![1]);
    assert!(layout.segment_rects[0].1.size.x > 0.0);
}
```

Update existing position expectations to include the leading inset, segment
padding, and 16-pixel separator slot. Keep the existing zero-content-width
test for the case where the right button consumes all available content.

- [ ] **Step 2: Run the focused tests and verify the new tests fail**

Run:

```powershell
rtk cargo test -p waml-editor document_header::tests --bin waml-editor
```

Expected: compilation failures for the two missing private helpers and/or
assertion failures because `layout_header` still uses unpadded label widths.

- [ ] **Step 3: Implement the minimal pure geometry**

Change `layout_header` so content starts at `HEADER_PAD_X`, every natural
segment width is `label_width.max(0.0) + 2.0 * SEGMENT_PAD_X`, and every
inter-segment slot is `SEPARATOR_SLOT_W`. Subtract the leading inset and the
unchanged right-button reservation from usable content width.

Add:

```rust
fn separator_rect(left_segment: Rect) -> Rect {
    Rect {
        pos: dvec2(
            left_segment.pos.x
                + left_segment.size.x
                + (SEPARATOR_SLOT_W - SEPARATOR_SIZE) * 0.5,
            left_segment.pos.y + (DOCUMENT_HEADER_H - SEPARATOR_SIZE) * 0.5,
        ),
        size: dvec2(SEPARATOR_SIZE, SEPARATOR_SIZE),
    }
}

fn centered_text_y(row: Rect, font_size: f64) -> f64 {
    (row.pos.y + (row.size.y - font_size) * 0.5 + TEXT_DY).round()
}
```

Keep `visible_indices` ordered oldest-to-current and always seed it with the
current segment when `available_width > 0.0`.

- [ ] **Step 4: Run focused tests and verify geometry passes**

Run:

```powershell
rtk cargo test -p waml-editor document_header::tests --bin waml-editor
```

Expected: all focused document-header tests pass.

- [ ] **Step 5: Add the canvas surface, bottom rule, and private chevron**

In the live-design block:

```rust
mod.draw.DocumentHeaderChevron = mod.draw.DrawColor {
    pixel: fn() {
        let s = self.rect_size.x
        let w = max(1.0, s * 0.125)
        let sdf = Sdf2d.viewport(self.pos * self.rect_size)
        sdf.move_to(s * 0.375, s * 0.25)
        sdf.line_to(s * 0.625, s * 0.5)
        sdf.line_to(s * 0.375, s * 0.75)
        sdf.stroke(self.color, w)
        return sdf.result
    }
}
```

Keep the outer `DocumentHeader` as the area-owning overlay and paint the
surface through its first child, before the content row:

```rust
flow: Overlay
surface := View {
    width: Fill
    height: Fill
    show_bg: true
    draw_bg: {
        color: atlas.canvas_ground
        pixel: fn() {
            return Pal::premul(self.color)
        }
    }
}
content_row := View {
    width: Fill
    height: Fill
    flow: Right
    align: { y: 0.5 }
}
draw_border +: { color: atlas.surface_border }
draw_separator: mod.draw.DocumentHeaderChevron { color: atlas.text_dim }
```

The surface child avoids changing the outer widget's turtle `Area`, which is
used by mounted height and drag-query behavior. Replace the live
`draw_chevron: DrawText` field with private `DrawColor` fields `draw_border`
and `draw_separator`. In `draw_walk`:

1. Draw a one-pixel bottom border at
   `self.draw_rect.pos.y + DOCUMENT_HEADER_H - 1.0`.
2. Draw each title at `rect.pos.x + SEGMENT_PAD_X` and
   `centered_text_y(*rect, draw.text_style.font_size as f64)`.
3. Draw `draw_separator` into `separator_rect(*rect)` after every visible
   non-current segment.
4. Keep the existing content clip ending at the right-button edge.

- [ ] **Step 6: Run focused tests and build the release editor**

Run:

```powershell
rtk cargo test -p waml-editor document_header::tests --bin waml-editor
rtk cargo build -p waml-editor --bin waml-editor --release
```

Expected: tests pass and release build completes with no errors.

- [ ] **Step 7: Capture matching after screenshots and compare**

Launch the isolated release editor with:

```powershell
$headerEditor = Start-Process `
    -FilePath target\release\waml-editor.exe `
    -ArgumentList @(
        "crates\waml-editor\tests\fixtures\mini",
        "--diagram",
        "Orders",
        "--title",
        "header-after"
    ) `
    -PassThru
```

Use the repository capture script after sizing the client area to 1440×900 and
820×900:

```powershell
rtk proxy pwsh -NoProfile -File scripts\capture-window.ps1 -Out C:\tmp\waml-document-header-visual-polish-after-wide.png -ProcessId $headerEditor.Id
rtk proxy pwsh -NoProfile -File scripts\capture-window.ps1 -Out C:\tmp\waml-document-header-visual-polish-after-narrow.png -ProcessId $headerEditor.Id
```

Compare against:

- `C:\tmp\waml-document-header-visual-polish-before-wide.png`
- `C:\tmp\waml-document-header-visual-polish-before-narrow.png`

Confirm canvas-color continuity, visible bottom rule, reference-like spacing,
centered text, crisp single chevrons, unchanged inspector placement, and
unchanged narrow elision.

- [ ] **Step 8: Run the complete requested verification**

Run:

```powershell
rtk cargo test -p waml-editor document_header::tests --bin waml-editor
rtk cargo test -p waml-editor --bin waml-editor
rtk cargo clippy -p waml-editor --all-targets -- -D warnings
rtk cargo fmt --all -- --check
rtk git diff --check
```

Expected: every command exits zero.

- [ ] **Step 9: Self-review and request a fresh visual/layout review**

Inspect the exact diff, compare both screenshot pairs, and confirm the only
production module changed is `document_header.rs`. Dispatch a fresh reviewer
with the approved spec, diff, and four screenshot paths, asking specifically
for navigation-target, hit-rectangle, current-segment, inspector-reservation,
wide/narrow, and visual alignment regressions. Address any findings and rerun
the affected verification.

- [ ] **Step 10: Commit the scoped implementation**

Stage only the plan and header changes:

```powershell
rtk git add docs/superpowers/plans/2026-07-29-document-header-visual-polish.md crates/waml-editor/src/document_header.rs
rtk git commit -m "fix(editor): polish document header"
```

Expected: a clean worktree with the implementation commit on
`codex/document-header-visual-polish`; do not push, merge, or modify `main`.
