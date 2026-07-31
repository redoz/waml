# Foundation Layout B5 Design

Status: written specification checkpoint

First design approval: approach 3 and Makepad path A

Implementation status: not started. A second approval of this written specification is required before implementation planning or code changes.

## Purpose

B4 improved inline composition, retained-payload bounds, and table width reuse. Its final review found structural residuals that local fixes cannot solve safely. B5 replaces the ambiguous seams with explicit contracts for paragraph shaping, visual geometry, hierarchy construction, cache fingerprints, table intrinsics, and layout work bounds.

This design keeps `crates/waml-editor/src/app.rs` unchanged.

## Approved direction

B5 uses these four rules:

1. A visual row owns one or more independent lanes.
2. A text shaper receives one complete paragraph and all style spans.
3. Layout indexes use validated hierarchy order and one linear content-hash pass.
4. Each layout has a structural work budget and a monotonic convergence rule.

The Makepad fork gets a small general-purpose uncached layout seam and read-only cache statistics. The Makepad API contains no WAML table or editor policy.

## Alternatives

### Alternative 1: patch the flat line model

This approach adds row identifiers to `VisualLine`, merges per-run shaping output, caps the current convergence loop, and adds table exceptions.

This approach is not selected. A flat line still cannot represent independent same-Y flows. Style-local shaping still loses paragraph bidi and break context. Each new container type would need more geometry exceptions.

### Alternative 2: let `LayoutEngine` be the complete text layouter

This approach makes the backend shape directional style fragments only. The engine computes bidi, legal breaks, rows, caret stops, and style stitching.

This approach is not selected. It duplicates a large part of Makepad text layout. It also increases parity risk for fallback fonts, ligatures, glyph offsets, empty rows, and caret positions.

### Alternative 3: explicit paragraph and visual geometry contracts

This is the selected approach. The backend shapes and breaks one complete styled paragraph. The engine places the returned rows into explicit visual lanes. Geometry queries use row and lane ownership and do not infer ownership from overlapping rectangles.

## Visual row and lane model

The snapshot stores rows and lanes as separate indexed arrays. A representative API is:

```rust
pub struct VisualRow {
    pub id: VisualRowId,
    pub rect: Rect,
    pub baseline: f64,
    pub lanes: Range<usize>,
}

pub struct VisualLane {
    pub id: VisualLaneId,
    pub row_index: usize,
    pub kind: VisualLaneKind,
    pub source_range: TextRange,
    pub rect: Rect,
    pub cluster_range: Range<usize>,
    pub caret_stop_range: Range<usize>,
    pub stable_order: u32,
}

pub enum VisualLaneKind {
    Paragraph,
    TableCell { table: BlockId, row: u32, column: u32 },
    HangingMarker,
    HangingContent,
}
```

The final names can change during implementation. The ownership rules cannot change.

### Row construction

- Container layout creates a row key. Snapshot assembly does not group lanes with a floating-point Y comparison.
- A paragraph line creates one paragraph lane.
- A table creates one visual row for each table-row and cell-line ordinal. All cell lanes for that ordinal share the visual row. The row height is the maximum lane height.
- A hanging marker and the first content line share one explicit visual row. Continuation content lines create later rows without a marker lane.
- A row can contain a lane with no glyphs. An empty continuation lane has an empty source range and one legal caret boundary.

### Geometry queries

- Point-to-source selects a row by Y. It then selects a lane by X and lane bounds. It uses the nearest lane only when the point is outside all lane bounds.
- Source-to-point selects the lane that owns the source range. It does not use the first same-Y line.
- Vertical movement first selects the previous or next visual row. In that row, it selects the caret stop nearest to preferred X. It cannot move to a sibling lane in the current row.
- Selection intersects each lane independently. It does not combine caret stops from all lanes that overlap in Y.
- Each cluster stores its lane index. Each lane stores its final X offset. Cluster placement reads the offset by direct array index. No cluster scans line offsets. Lookup is O(1).

`visual_lines()` can remain as a temporary flattened lane view for compatibility. Navigation, hit testing, selection, and visible-range logic must use rows and lanes.

### Visible source range

The visible range is the minimum source start and maximum source end over all visible lanes. Array order is not part of this calculation. Empty lanes contribute their source boundary. A snapshot with no visible lanes returns the existing empty-range result.

## Paragraph shaping contract

The required `TextShaper` seam becomes one paragraph operation. The old required `shape` method and the default `shape_inline` method are removed. No default implementation can ignore the first-row width.

A representative request is:

```rust
pub struct ParagraphShapeRequest<'a> {
    pub source: &'a SourceText,
    pub paragraph_id: GeometryElementId,
    pub paragraph_range: TextRange,
    pub spans: &'a [ShapeSpan],
    pub full_width: f64,
    pub first_row_width: f64,
    pub base_direction: BaseDirection,
}

pub struct ShapeSpan {
    pub id: GeometryElementId,
    pub run_id: LayoutRunId,
    pub stable_ordinal: u32,
    pub source_range: TextRange,
    pub metrics: TextMetrics,
    pub style: TextStylePayload,
}

pub trait TextShaper {
    fn shape_paragraph(
        &mut self,
        request: ParagraphShapeRequest<'_>,
    ) -> Result<ShapedParagraph, LayoutError>;

    fn measure_paragraph_intrinsic(
        &mut self,
        request: ParagraphIntrinsicRequest<'_>,
    ) -> Result<ParagraphIntrinsic, LayoutError>;
}
```

The intrinsic operation is separate because it must not retain full glyph layouts.

### Whole-paragraph context

- The request contains the complete paragraph source and every ordered style span.
- The backend runs bidi analysis once for the complete paragraph.
- The backend creates legal break opportunities once for the complete paragraph. A style boundary is not an automatic break opportunity.
- The backend can split glyph shaping at font, style, script, or bidi-run boundaries. Row breaking still uses the shared paragraph break map and paragraph bidi levels.
- A word that crosses a style boundary remains unbreakable when the paragraph break map has no legal break there.
- `first_row_width` is a required input and applies to row zero. Later rows use `full_width`.

### Shaped result

`ShapedParagraph` contains the paragraph bidi context, legal break map, and an explicit ordered row array. Every row has source boundaries, row metrics, visual fragment order, and caret boundaries.

The result includes empty continuation rows. A trailing newline or backend-produced empty continuation is not removed because it has no glyph cluster.

Each styled fragment retains:

- its input span ID and stable span ordinal;
- its logical source range;
- exact `TextMetrics`, font key, font identity, and style payload;
- logical caret stops;
- exact glyph identifiers, advances, offsets, origins, paint scale, color, and font data needed by painting.

Adjacent spans with equal shaping properties can shape together. The result still maps each fragment to its input span.

### Stable identifier contract

Stable identifiers are required output, not an optional engine repair.

- Each shaped row, fragment, and cluster returns a `GeometryElementId`.
- A cluster ID derives from the paragraph ID, input span ID, logical source range, and intra-range ordinal.
- An empty row ID derives from the paragraph ID, its source boundary, and its logical empty-row ordinal.
- IDs are independent of width, row vector position, bidi visual order, and table lane order.
- The engine rejects duplicate IDs, missing input-span mappings, or IDs that change for the same logical cluster during one layout.

## Makepad shaping and intrinsic measurement

### General-purpose fork API

The Makepad fork adds public APIs at the `Layouter` and `Fonts` boundary:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LayoutCacheStats {
    pub entries: usize,
    pub bytes: usize,
}

pub fn layout_uncached(
    &mut self,
    params: impl LayoutParams,
) -> LaidoutText;

pub fn layout_cache_stats(&self) -> LayoutCacheStats;
```

`layout_uncached` loads required fonts and performs the same layout algorithm as `get_or_layout`. It does not read, insert, refresh, or evict the `LaidoutText` result cache. It can use bounded lower-level font and shape caches that are already part of Makepad. It returns an owned result, so the caller controls its lifetime.

`Fonts` forwards both operations. `DrawText` gets an uncached entry point that builds the same general `BorrowedLayoutParams`. The API is useful to any Makepad caller that needs one-shot layout or measurement. It contains no WAML types and no table policy.

Read-only statistics report only the retained `LaidoutText` cache entry count and accounted bytes. They do not expose cache mutation.

### WAML production path

- Visible paragraph shaping uses the exact Makepad glyph payload required for painting.
- Intrinsic measurement calls only the uncached Makepad seam.
- WAML reduces each uncached result to numeric source ranges, min-content widths, max-content widths, and required metrics. It then drops the full result before it measures the next paragraph.
- Intrinsic measurement never calls the default cached `DrawText::layout` path.
- The production 10,000-cell regression records Makepad cache statistics before and after intrinsic measurement. Entry count and bytes must be unchanged.

The paragraph adapter applies one complete bidi and legal-break context across all WAML style spans. It must not call the old span-local `shape_inline` loop.

### Focused Makepad tests

The Makepad fork tests these guarantees:

1. Cached and uncached layout return equivalent rows, glyphs, metrics, and truncation state for the same parameters.
2. Repeated uncached calls do not change cache entry count, cache bytes, LRU order, or cache generation.
3. An uncached call does not refresh a cached entry's recency.
4. `layout_cache_stats` reports insert, replacement, eviction, and clear operations correctly.
5. The zero-cache configuration and the uncached API have equivalent no-retention behavior.

## Validated hierarchy

`BlockHierarchy::new` becomes a fallible constructor such as `BlockHierarchy::try_new`.

It returns typed errors for:

- duplicate block IDs;
- a missing parent;
- a self-parent link;
- a parent cycle.

Construction creates the ID map and parent/child adjacency once. It uses iterative color-state depth-first traversal to validate cycles and produce postorder. It does not recurse, so deep valid nesting cannot overflow the call stack.

Sibling order is stable document order: source start, source end, then stable block ID. Hierarchy fingerprints combine child fingerprints in this order. The input block vector order and the position of a parent relative to its child do not affect the result.

## Linear index hashing

Index construction has one source-byte sweep and one metadata visit per run, embedded item, and block.

- A source fingerprint builder reads the document byte stream once.
- Sorted validated run intervals let the same sweep update the owning run content hash. Direct run intervals cannot overlap. Adjacent style spans are valid.
- Embedded payload metadata and run metadata are hashed once.
- A block own fingerprint contains fixed-size block metadata plus the already-computed hashes of its direct runs and embedded items. It does not read `block.source_range` bytes.
- A subtree fingerprint combines the block own fingerprint and fixed-size child fingerprints in validated postorder.

No ancestor re-reads descendant source. Complexity is O(source bytes + blocks + runs + embedded items). The instrumented test counts source-byte and record visits instead of using elapsed time.

If the parser can produce overlapping direct run intervals, index construction must reject them or normalize them into non-overlapping logical spans before hashing. It must not silently hash the overlapping bytes twice.

## Intrinsic memo and table widths

Each live block has one memoized `IntrinsicSize` for its current subtree fingerprint:

```rust
pub struct IntrinsicSize {
    pub min_content: f64,
    pub max_content: f64,
}
```

The engine computes missing entries in validated hierarchy postorder. A parent reads child memo entries in O(1). Nested table measurement does not recursively recompute descendants.

### Unconstrained tables

- A table with no column constraints derives its column count from live rows and cells.
- Each column minimum is the maximum cell `min_content` in that column.
- Each column preferred width is the maximum cell `max_content` in that column.
- If preferred widths fit, columns receive preferred widths before surplus distribution.
- If preferred widths do not fit, columns shrink toward minimum widths.
- If the sum of minimum widths exceeds available width, the table overflows. It does not introduce an illegal break in an unbreakable word.
- Explicit constraints take part in the same minimum, preferred, and surplus calculation. They do not bypass intrinsic safety.

### Cache lifetime

Table intrinsic cache keys contain the table ID and subtree fingerprint. At the start or end of each layout, the engine retains only entries for live tables with the current fingerprint. Removed or replaced tables cannot leave stale cache entries.

## Work budget and convergence

The open convergence loop is removed. Layout uses these phases:

1. validate hierarchy and build indexes;
2. compute missing intrinsic memo entries;
3. solve all table widths once;
4. position block summaries;
5. shape unseen paragraphs that enter the measurement window at their final width;
6. reposition summaries and repeat phase 5 only when the monotonic candidate set grows;
7. build rows, lanes, geometry, and the final snapshot.

Table widths do not change after phase 3. A `ShapeLedger` key contains block ID, paragraph fingerprint, and final width key. The engine can fully shape one key at most once in one layout.

### Explicit structural limits

`LayoutBudget::for_index` computes hard limits before shaping:

- `max_full_shape_calls` is the number of live paragraph keys in the validated index;
- `max_intrinsic_calls` is the number of unique invalid intrinsic paragraph keys in live table subtrees;
- `max_hydration_passes` is `max_full_shape_calls + 1`;
- `max_full_shape_source_bytes` is the sum of source lengths for all live paragraph keys;
- `max_intrinsic_source_bytes` is the sum of source lengths for the unique invalid intrinsic keys.

The measurement window normally consumes much less than these structural limits. The limits are proof bounds, not work targets.

Each shaping or intrinsic call checks and consumes its call and byte budget before backend entry. Each repeat after the first must add at least one new ledger key. If it does not, the engine returns `LayoutError::NonConvergent`. A budget exhaustion returns `LayoutError::BudgetExceeded` with phase, limit, and observed count. The engine never returns a silent partial snapshot.

This gives these bounds:

- full paragraph shapes are O(live paragraphs) in the adversarial case and O(measurement-window paragraphs) in the normal case;
- intrinsic calls are O(unique invalid intrinsic paragraphs);
- hydration passes are at most live paragraphs plus one;
- a paragraph is never fully shaped twice at the same final width in one layout.

## Test plan

### Rows, lanes, and ranges

- Same-Y table cells: hit testing selects by X; vertical movement skips sibling cells in the current row; preferred X selects the nearest lane in the next row; selection does not bridge unrelated cells.
- Hanging marker and content: both share one row, but hit testing and selection use independent lanes. Vertical movement goes to the next row.
- Empty continuation: the shaped result, visual row, empty lane, and caret boundary remain present.
- Prefix visible range: a visible prefix lane stored after later lanes still sets the minimum source start. The maximum comes from all lanes.
- Line offset: an instrumented large paragraph proves one direct offset lookup per cluster and no per-cluster line scan.

### Styled paragraphs

- A word split across two style spans has no legal break at the style boundary.
- Mixed LTR and RTL text split across styles uses one paragraph bidi context. The test checks visual order, caret stops, stable IDs, exact fonts, metrics, and glyph payload.
- Empty and trailing-newline continuations stay in the row result.
- The required shape seam uses the smaller first-row width. A backend that ignores it fails the contract test.
- Stable IDs do not change when width changes, rows wrap differently, or bidi visual order changes.

### Hierarchy and hashes

- Child-before-parent input gives the same postorder and subtree fingerprints as parent-before-child input.
- Duplicate IDs, missing parents, self-parent links, and cycles return the correct typed errors.
- Deep nesting completes without recursion and with linear node visits.
- An instrumented source proves one byte sweep, one run visit, one embedded visit, and one block visit. Ancestor ranges cause no extra byte reads.

### Budgets and tables

- An adversarial shaper causes the exact documented call/pass bound or a typed budget error. It cannot cause an unbounded loop.
- Total call tests count full paragraph shapes and intrinsic calls separately.
- A 10,000-row document shapes only measurement-window paragraphs but can compute the required numeric table intrinsics.
- The production Makepad 10,000-cell test proves unchanged retained layout-cache entries and bytes.
- An unconstrained table starts from intrinsic column widths.
- An unbreakable minimum wider than available width causes overflow.
- Nested tables reuse postorder intrinsic memo entries.
- Removing a table prunes its cached intrinsic entry.

## Verification and report correction

After implementation, verification must include:

- focused B5 layout and Makepad cache tests;
- all `waml-markdown-editor` tests;
- focused Makepad text tests;
- Clippy for affected WAML and Makepad crates;
- format checks;
- the repository diff check;
- a final status check that confirms `crates/waml-editor/src/app.rs` was not changed by B5.

The existing whole-plan B report must then be corrected. It must not claim span-local shaping provides whole-paragraph bidi, that vector-reverse hierarchy order is robust, or that cached Makepad layout is a cheap intrinsic path. The report must record exact fresh commands, test counts, warnings, cache-stat evidence, call-count evidence, complexity evidence, and TokenSave savings.

The whole-plan verdict remains `NOT SAFE` until the remaining widget/session/input wave and high-level review gates are complete.

## Approval gate

This document is the written-spec checkpoint. Implementation planning, Makepad fork edits, WAML source edits, and test edits remain blocked until the second approval confirms this specification.
