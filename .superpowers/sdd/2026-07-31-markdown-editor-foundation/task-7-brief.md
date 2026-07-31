### Task 7: Define variable-metric layout geometry and exact queries

**Files:**
- Create: `crates/waml-markdown-editor/src/layout/mod.rs`
- Create: `crates/waml-markdown-editor/src/layout/geometry.rs`
- Create: `crates/waml-markdown-editor/tests/layout_geometry.rs`
- Modify: `crates/waml-markdown-editor/src/session.rs`

**Interfaces:**
- Consumes: spec 3 supplies a foundation-owned `LayoutDocument`; spec 1 supplies `SyntaxIdentity`; Makepad supplies `DVec2` and `Rect`.
- Produces: `LayoutElementId`; `GeometryElementId`; `FontKey`; `FontWeight`; `TextMetrics`; `LayoutTextRun`; `LayoutBlock`; `BlockLayoutSpec`; `MeasuredBlock`; `LayoutDocument`; `GlyphCluster`; `CaretStop`; `VisualLine`; `BlockGeometry`; `LayoutSnapshot`; source/point/selection/vertical-motion queries.

- [ ] **Step 1: Write failing hand-built geometry query tests**

Create `tests/layout_geometry.rs`:

```rust
use std::sync::Arc;
use makepad_widgets::{dvec2, DVec2, Rect};
use waml_markdown_editor::{
    layout::{
        Affinity, BlockGeometry, CaretStop, GlyphCluster, LayoutSnapshot, VisualLine,
    },
    selection::{Selection, TextPosition},
};
use waml_syntax::{DocumentRevision, TextRange, TextSize};

fn t(n: usize) -> TextSize {
    TextSize::try_from_usize(n).unwrap()
}

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(t(start), t(end)).unwrap()
}

#[test]
fn source_point_round_trip_handles_proportional_clusters_and_affinity() {
    let snapshot = LayoutSnapshot::from_parts_for_test(
        DocumentRevision::new(3),
        dvec2(120.0, 24.0),
        vec![VisualLine::for_test(range(0, 3), 0.0, 24.0)],
        vec![GlyphCluster::for_test(
            range(0, 3),
            Rect { pos: dvec2(0.0, 0.0), size: dvec2(30.0, 24.0) },
            vec![
                CaretStop::new(TextPosition::new(t(0), Affinity::Before), dvec2(0.0, 0.0)),
                CaretStop::new(TextPosition::new(t(1), Affinity::After), dvec2(9.0, 0.0)),
                CaretStop::new(TextPosition::new(t(3), Affinity::After), dvec2(30.0, 0.0)),
            ],
        )],
        Vec::<BlockGeometry>::new(),
    );
    for position in [
        TextPosition::new(t(0), Affinity::Before),
        TextPosition::new(t(1), Affinity::After),
        TextPosition::new(t(3), Affinity::After),
    ] {
        let point = snapshot.source_to_point(position).unwrap().rect.pos;
        assert_eq!(snapshot.point_to_source(point), position);
    }
}

#[test]
fn selection_rects_split_across_wrapped_mixed_height_lines() {
    let snapshot = LayoutSnapshot::wrapped_fixture_for_test();
    let selection = Selection::new(
        TextPosition::new(t(1), Affinity::Before),
        TextPosition::new(t(8), Affinity::After),
    );
    let rects = snapshot.selection_rects(selection).unwrap();
    assert_eq!(rects.len(), 2);
    assert_eq!(rects[0].size.y, 18.0);
    assert_eq!(rects[1].size.y, 30.0);
}

#[test]
fn vertical_motion_uses_preferred_pixels_not_character_columns() {
    let snapshot = LayoutSnapshot::proportional_fixture_for_test();
    let start = TextPosition::new(t(2), Affinity::After);
    let (down, preferred_x) = snapshot.move_vertical(start, None, 1).unwrap();
    assert_eq!(preferred_x, 26.0);
    let (up, _) = snapshot.move_vertical(down, Some(preferred_x), -1).unwrap();
    assert_eq!(up, start);
}
```

- [ ] **Step 2: Run and verify geometry types are absent**

Run: `rtk cargo test -p waml-markdown-editor --test layout_geometry`

Expected: FAIL with unresolved layout geometry types.

- [ ] **Step 3: Define the low-level presentation-to-layout seam**

In `layout/mod.rs`, re-export `crate::selection::Affinity` and define:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct LayoutElementId {
    pub owner: waml_syntax::SyntaxIdentity,
    pub fragment_ordinal: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct GeometryElementId {
    pub layout: LayoutElementId,
    pub cluster_ordinal: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FontKey(pub u64);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FontWeight(pub u16);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextMetrics {
    pub font: FontKey,
    pub font_size: f32,
    pub line_spacing: f32,
    pub weight: FontWeight,
    pub italic: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutTextRun {
    pub id: LayoutElementId,
    pub range: TextRange,
    pub metrics: TextMetrics,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MeasuredBlock {
    pub id: LayoutElementId,
    pub source_range: TextRange,
    pub size: DVec2,
    pub baseline: Option<f64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EdgeInsets {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColumnAlignment {
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColumnConstraint {
    pub min_width: f64,
    pub max_width: Option<f64>,
    pub alignment: ColumnAlignment,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BlockFlow {
    Paragraph,
    Hanging {
        marker_range: TextRange,
        content_indent: f64,
    },
    Quote,
    Code,
    Table,
    TableRow,
    TableCell {
        column: u32,
    },
    Embedded,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockLayoutSpec {
    pub flow: BlockFlow,
    pub insets: EdgeInsets,
    pub space_before: f64,
    pub space_after: f64,
    pub columns: Arc<[ColumnConstraint]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutBlock {
    pub id: LayoutElementId,
    pub source_range: TextRange,
    pub parent: Option<LayoutElementId>,
    pub spec: BlockLayoutSpec,
}

#[derive(Clone, Debug)]
pub struct LayoutDocument {
    pub revision: DocumentRevision,
    pub content_insets: EdgeInsets,
    pub blocks: Arc<[LayoutBlock]>,
    pub text_runs: Arc<[LayoutTextRun]>,
    pub embedded_blocks: Arc<[MeasuredBlock]>,
}
```

`LayoutDocument` is owned by the foundation crate but built by spec 3. Its `revision` must equal both the live `MarkdownDocumentSession` revision and the produced `LayoutSnapshot` revision; the layout engine returns `LayoutError::RevisionMismatch` before using input from another snapshot. `content_insets` carries the spec-3 24-logical-pixel document inset without hard-coding that visual choice in the engine. The parent-linked `LayoutBlock` tree gives the engine enough neutral flow information for paragraph/heading spacing, hanging lists, nested quotes, code blocks, table rows/cells and columns, and embedded blocks. It intentionally contains no semantic role, color, decoration, or link type, which prevents a foundation/presentation crate dependency cycle.

- [ ] **Step 4: Implement one immutable geometry authority**

In `layout/geometry.rs`, define immutable `VisualLine`, `BlockGeometry`, `GlyphCluster`, `CaretStop`, `CaretGeometry`, and:

```rust
#[derive(Clone, Debug)]
pub struct LayoutSnapshot {
    revision: DocumentRevision,
    viewport_width: f64,
    content_size: DVec2,
    visual_lines: Arc<[VisualLine]>,
    blocks: Arc<[BlockGeometry]>,
    clusters: Arc<[GlyphCluster]>,
    visible_source_range: TextRange,
    visible_block_range: Range<usize>,
}
```

Expose `revision`, `content_size`, `visible_source_range`, `visible_block_range`, `source_to_point`, `point_to_source`, `selection_rects`, and `move_vertical`. Binary-search source-sorted caret stops for source queries and visual-line y ranges for point queries. Resolve equal offsets with exact `Affinity`; never reconstruct geometry separately for caret or selection. `move_vertical` keeps the original x in logical pixels as `preferred_x`.

Each `GlyphCluster` must carry a unique `GeometryElementId`. Assign `cluster_ordinal` in stable source-cluster order within one `LayoutTextRun`, independent of wrapping and visual bidi order. Spec 3 motion matches clusters by `(LayoutElementId, cluster_ordinal)`; two glyph clusters from one run must never share an identity.

Expose the small hand-built geometry constructors used by `tests/layout_geometry.rs` as `#[doc(hidden)] pub` functions. Do not put them behind `#[cfg(test)]`, because integration tests compile the library without that configuration; do not add a feature that can change production behavior.

- [ ] **Step 5: Run geometry query tests**

Run: `rtk cargo test -p waml-markdown-editor --test layout_geometry`

Expected: PASS, 3 tests.

- [ ] **Step 6: Connect session vertical movement**

Add `MarkdownDocumentSession::move_vertical(&mut self, layout: &LayoutSnapshot, lines: i32, extend: bool)`. Reject a layout whose revision differs from the session revision, call `LayoutSnapshot::move_vertical`, store `preferred_x`, and reset `preferred_x` on horizontal movement, edits, and pointer placement.

Add `preferred_x: Option<f64>` to `MarkdownDocumentSession` and initialize it to `None`.

- [ ] **Step 7: Commit shared geometry contracts**

```bash
rtk git add crates/waml-markdown-editor/src/layout crates/waml-markdown-editor/src/session.rs crates/waml-markdown-editor/tests/layout_geometry.rs
rtk git commit -m "feat: define variable metric markdown geometry"
```
