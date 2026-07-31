### Task 8: Implement shaping, wrapping, fallback, and viewport virtualization

**Files:**
- Create: `crates/waml-markdown-editor/src/layout/engine.rs`
- Create: `crates/waml-markdown-editor/src/layout/makepad.rs`
- Modify: `crates/waml-markdown-editor/src/layout/mod.rs`
- Modify: `crates/waml-markdown-editor/tests/layout_geometry.rs`

**Interfaces:**
- Consumes: `LayoutDocument`, immutable document snapshot, `MarkdownSyntaxUpdate::affected_ranges`, and Makepad `DrawText::layout`.
- Produces: `TextShaper`; `ShapedRun`; `LayoutViewport`; `LayoutInvalidation`; `LayoutError`; `LayoutEngine::layout`; `BlockSummary`; `MakepadTextShaper`.

- [ ] **Step 1: Add failing mixed-metric, resize, fallback, and virtualization tests**

Use a deterministic fake `TextShaper` whose glyph advances equal values supplied by `FontKey`. Add:

```rust
#[test]
fn mixed_metrics_wrap_without_a_cell_width() {
    let (document, presentation, mut shaper) =
        fixtures::mixed_heading_and_body(80.0);
    let mut engine = LayoutEngine::default();
    let layout = engine
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(80.0, 60.0, 0.0, 24.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    assert_eq!(layout.visual_lines()[0].height(), 30.0);
    assert_eq!(layout.visual_lines()[1].height(), 16.0);
    assert!(layout.visual_lines().len() > 2);
}

#[test]
fn viewport_shapes_only_visible_blocks_plus_overscan() {
    let (document, presentation, mut shaper) = fixtures::one_hundred_blocks();
    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 100.0, 800.0, 40.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    assert!(shaper.shaped_block_count() < 20);
    assert_eq!(layout.block_summaries().len(), 100);
    assert!(layout.content_size().y >= 2_000.0);
}

#[test]
fn width_change_rewraps_without_changing_document_revision() {
    let (document, presentation, mut shaper) = fixtures::paragraph();
    let mut engine = LayoutEngine::default();
    let wide = engine
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 200.0, 0.0, 40.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    let narrow = engine
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(120.0, 200.0, 0.0, 40.0),
            LayoutInvalidation::ViewportWidth,
            &mut shaper,
        )
        .unwrap();
    assert_eq!(wide.revision(), narrow.revision());
    assert!(narrow.visual_lines().len() > wide.visual_lines().len());
}

#[test]
fn failed_block_uses_editable_plain_text_fallback() {
    let (document, presentation, mut shaper) = fixtures::failing_second_block();
    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 200.0, 0.0, 40.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    assert!(layout.blocks()[1].is_plain_text_fallback());
    let source = layout.blocks()[1].source_range();
    assert_eq!(
        layout.point_to_source(layout.source_to_point(TextPosition::new(
            source.start(),
            Affinity::Before
        )).unwrap().rect.pos),
        TextPosition::new(source.start(), Affinity::Before)
    );
}
```

- [ ] **Step 2: Run and verify the engine APIs are absent**

Run: `rtk cargo test -p waml-markdown-editor --test layout_geometry`

Expected: FAIL with unresolved `LayoutEngine`, `LayoutViewport`, and `LayoutInvalidation`.

- [ ] **Step 3: Implement a testable shaping boundary**

Define:

```rust
pub trait TextShaper {
    fn shape(
        &mut self,
        source: &SourceText,
        run: &LayoutTextRun,
        max_width: f64,
    ) -> Result<ShapedRun, LayoutError>;
}

#[derive(Clone, Debug)]
pub struct ShapedRun {
    pub clusters: Arc<[ShapedCluster]>,
    pub ascender: f64,
    pub descender: f64,
    pub line_gap: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutViewport {
    pub width: f64,
    pub height: f64,
    pub scroll_y: f64,
    pub overscan: f64,
}

#[derive(Clone, Debug)]
pub enum LayoutInvalidation {
    Document,
    SyntaxUpdate(MarkdownSyntaxUpdate),
    ViewportWidth,
    BlockMeasurement(LayoutElementId),
}
```

`ShapedCluster` carries source range, visual advance, bidi level, and a source-ordered array of caret offsets. Missing glyphs remain shaped by Makepad fallback and retain the same source range.

- [ ] **Step 4: Implement block summaries and incremental relayout**

`LayoutEngine` caches summaries by `LayoutElementId`:

```rust
#[derive(Clone, Debug)]
pub struct BlockSummary {
    pub id: LayoutElementId,
    pub source_range: TextRange,
    pub parent: Option<LayoutElementId>,
    pub flow_fingerprint: u64,
    pub y: f64,
    pub height: f64,
    pub width_key: u64,
    pub content_fingerprint: u64,
}
```

For a syntax update, find the first block intersecting the first `affected_range`, invalidate through the last affected block, then continue laying out downstream blocks until id, parent, flow spec, fingerprint, width key, y delta, and height equal the cached summary. For viewport-width invalidation, invalidate every text block's wrap and table column solution while retaining syntax identity. Shape only summaries intersecting `[scroll_y - overscan, scroll_y + height + overscan]`; keep all summaries for total scroll extent and stable navigation.

Lay out the parent-linked block tree inside `LayoutDocument::content_insets`. Apply before/after spacing at sibling boundaries, add nested `EdgeInsets`, preserve hanging marker flow while indenting content, and solve table column widths from all visible row/cell `ColumnConstraint` values plus cached off-screen minimum widths. Wrap only at shaped cluster boundaries. The visual line height is the maximum ascender/descender/line-gap of its runs and embedded inline items. An individual block shaping failure creates a plain body-style run over the block's literal source and marks `BlockGeometry::is_plain_text_fallback`.

- [ ] **Step 5: Implement the Makepad shaper**

In `layout/makepad.rs`, define `FontResolver` and `MakepadTextShaper`:

```rust
pub trait FontResolver {
    fn configure_draw_text(&mut self, key: FontKey, metrics: TextMetrics, draw: &mut DrawText);
}

pub struct MakepadTextShaper<'a, R> {
    pub cx: &'a mut Cx,
    pub draw_text: &'a mut DrawText,
    pub fonts: &'a mut R,
}
```

Call `DrawText::layout(cx, 0.0, 0.0, Some(max_width as f32), true, Align::default(), text)`. Convert `LaidoutText.rows`, each `LaidoutGlyph.cluster`, glyph origin, advance, ascender, and descender into exact UTF-8 `ShapedCluster` ranges relative to `LayoutTextRun::range`. Use Makepad's row visual order for bidi geometry and retain source affinity at duplicate bidi boundaries. `FontResolver` maps the spec-3 `FontKey` to Makepad font family/style; a missing key configures the body fallback rather than dropping text.

- [ ] **Step 6: Run layout tests**

Run: `rtk cargo test -p waml-markdown-editor --test layout_geometry`

Expected: PASS, 7 tests.

- [ ] **Step 7: Commit layout and virtualization**

```bash
rtk git add crates/waml-markdown-editor/src/layout crates/waml-markdown-editor/tests/layout_geometry.rs
rtk git commit -m "feat: lay out visible markdown blocks"
```
