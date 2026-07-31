# Foundation Layout B5 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the approved B5 paragraph, visual row/lane, hierarchy, hashing, intrinsic table, Makepad cache, and bounded-layout contracts with test-first evidence.

**Architecture:** Makepad first exposes a general uncached text-layout seam and read-only retained-layout cache statistics. WAML then validates and indexes the document before work, shapes complete styled paragraphs, places explicit lanes into visual rows, computes intrinsic sizes in hierarchy postorder, and hydrates a monotonic measurement set under structural budgets.

**Tech Stack:** Rust, Makepad text layouter, `unicode-bidi`, `unicode-segmentation`, `cargo test`, `cargo clippy`, `cargo fmt`, RTK command wrapper.

## Global Constraints

- Preserve unrelated changes in both repositories. Never revert or stage `crates/waml-editor/src/app.rs`.
- Preserve the unrelated untracked `C:\dev\makepad\docs\superpowers\plans\` directory.
- Makepad prerequisite changes must be general-purpose and committed separately in `C:\dev\makepad`.
- WAML production ownership is `crates/waml-markdown-editor/src/layout/**` only.
- WAML test ownership is `crates/waml-markdown-editor/tests/layout_geometry.rs` and unit tests inside owned layout files.
- Report ownership is `.superpowers/sdd/2026-07-31-markdown-editor-foundation/whole-plan-fix-b-report.md`.
- Do not edit widget, session, input, or application production code.
- Use ASD-STE100 Simplified Technical English in documentation and diagnostics.
- Use TokenSave before source scans. Use `rtk` for project shell commands.
- Every behavior change follows RED, GREEN, REFACTOR. Record the expected RED reason before production edits.
- Each slice must pass its focused tests and a `waml-markdown-editor` regression test before commit.
- Stop after full gates and the corrected B report. Do not start widget D or Plan3 work.

---

### Task 1: General Makepad uncached layout API

**Files:**
- Modify: `C:/dev/makepad/draw/src/text/layouter.rs`
- Modify: `C:/dev/makepad/draw/src/text/fonts.rs`
- Modify: `C:/dev/makepad/draw/src/shader/draw_text.rs`
- Test: unit tests in the same three Makepad files, with cache-accounting tests in `layouter.rs`

**Interfaces:**
- Consumes: existing `LayoutParams`, `OwnedLayoutParams`, `LaidoutText`, `Fonts`, and `DrawText::layout` parameter construction.
- Produces: `LayoutCacheStats`, `Layouter::layout_uncached`, `Layouter::layout_cache_stats`, `Fonts` forwarders, and `DrawText::layout_uncached`.

- [ ] **Step 1: Write failing Makepad cache tests**

Add these tests beside the existing `Layouter` cache tests:

```rust
#[test]
fn uncached_layout_matches_cached_without_touching_cache() {
    let mut layouter = test_layouter_with_loaded_font();
    let params = cache_test_params("mixed text");
    let before = layouter.layout_cache_stats();
    let uncached = layouter.layout_uncached(params.clone());
    let after = layouter.layout_cache_stats();
    let cached = layouter.get_or_layout(params);
    assert_eq!(uncached.rows, cached.rows);
    assert_eq!(uncached.size_in_lpxs, cached.size_in_lpxs);
    assert_eq!(before, after);
}

#[test]
fn uncached_layout_does_not_refresh_cached_recency() {
    let mut layouter = cache_test_layouter(2);
    seed_two_cache_entries(&mut layouter, "old", "new");
    let before = layouter.cache_debug_order_for_test();
    let _ = layouter.layout_uncached(cache_test_params("old"));
    assert_eq!(layouter.cache_debug_order_for_test(), before);
}

#[test]
fn cache_stats_follow_insert_replace_evict_and_clear() {
    let mut layouter = cache_test_layouter(2);
    assert_eq!(layouter.layout_cache_stats(), LayoutCacheStats { entries: 0, bytes: 0 });
    seed_cache_entry(&mut layouter, "a");
    assert_eq!(layouter.layout_cache_stats().entries, 1);
    assert!(layouter.layout_cache_stats().bytes > 0);
    layouter.set_font_family_definition(test_family_id(), test_family_definition());
    assert_eq!(layouter.layout_cache_stats(), LayoutCacheStats { entries: 0, bytes: 0 });
}
```

Use the existing cache fixtures. Derive `PartialEq` for row/glyph types only if exact result comparison does not already compile; otherwise compare their existing public fields.

- [ ] **Step 2: Run RED**

Run from `C:\dev\makepad`:

```powershell
rtk cargo test -p makepad-draw uncached_layout -- --nocapture
rtk cargo test -p makepad-draw cache_stats_follow -- --nocapture
```

Expected: compilation fails because `LayoutCacheStats`, `layout_uncached`, and `layout_cache_stats` do not exist.

- [ ] **Step 3: Implement the minimal general APIs**

Add this public value type and methods:

```rust
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LayoutCacheStats {
    pub entries: usize,
    pub bytes: usize,
}

impl Layouter {
    pub fn layout_uncached(&mut self, params: impl LayoutParams) -> LaidoutText {
        self.layout(params.to_owned())
    }

    pub fn layout_cache_stats(&self) -> LayoutCacheStats {
        LayoutCacheStats {
            entries: self.cached_results.len(),
            bytes: self.cache_bytes,
        }
    }
}
```

Forward both methods through `Fonts`. Add `DrawText::layout_uncached` with the same public arguments as `DrawText::layout`; reuse one private `borrowed_layout_params` helper so cached and uncached paths cannot drift. The uncached method returns `LaidoutText` and must not call `get_or_layout`.

- [ ] **Step 4: Run GREEN and Makepad regression**

```powershell
rtk cargo test -p makepad-draw uncached_layout -- --nocapture
rtk cargo test -p makepad-draw cache_stats_follow -- --nocapture
rtk cargo test -p makepad-draw text::layouter::tests -- --nocapture
rtk cargo clippy -p makepad-draw --all-targets -- -D warnings
```

Expected: all focused tests pass. Clippy has zero Makepad Rust errors.

- [ ] **Step 5: Commit only Makepad source and tests**

```powershell
rtk git add -- draw/src/text/layouter.rs draw/src/text/fonts.rs draw/src/shader/draw_text.rs
rtk git commit -m "feat(text): add uncached layout API"
```

Confirm `docs/superpowers/plans/` remains untracked and unstaged.

---

### Task 2: Validated hierarchy and linear index fingerprints

**Files:**
- Modify: `crates/waml-markdown-editor/src/layout/mod.rs`
- Modify: `crates/waml-markdown-editor/src/layout/engine.rs`
- Modify: `crates/waml-markdown-editor/tests/layout_geometry.rs`

**Interfaces:**
- Consumes: `LayoutDocument`, `LayoutBlock`, `LayoutElementId`, current `DocumentLayoutIndex` cache fingerprints.
- Produces: fallible `BlockHierarchy::try_new`, deterministic `postorder`, typed hierarchy errors, and `IndexBuildStats` test evidence.

- [ ] **Step 1: Write hierarchy RED tests**

Add integration tests that call `LayoutEngine::layout` with these inputs:

```rust
#[test]
fn hierarchy_rejects_duplicate_missing_and_cyclic_parents() {
    assert!(matches!(layout_with_blocks(duplicate_id_blocks()), Err(LayoutError::DuplicateBlockId { .. })));
    assert!(matches!(layout_with_blocks(missing_parent_blocks()), Err(LayoutError::MissingParent { .. })));
    assert!(matches!(layout_with_blocks(self_parent_blocks()), Err(LayoutError::HierarchyCycle { .. })));
    assert!(matches!(layout_with_blocks(two_node_cycle_blocks()), Err(LayoutError::HierarchyCycle { .. })));
}

#[test]
fn child_before_parent_has_order_independent_subtree_fingerprints() {
    let first = layout_and_fingerprint(parent_before_child_document());
    let second = layout_and_fingerprint(child_before_parent_document());
    assert_eq!(first, second);
}

#[test]
fn deep_hierarchy_build_is_iterative_and_linear() {
    let (document, snapshot, mut shaper) = deep_document(20_000);
    let mut engine = LayoutEngine::default();
    engine.layout(
        &document,
        &snapshot,
        LayoutViewport::default_overscan(400.0, 600.0, 0.0),
        LayoutInvalidation::Document,
        &mut shaper,
    ).unwrap();
    let stats = engine.last_index_build_stats_for_test();
    assert_eq!(stats.block_visits, 20_000);
}
```

- [ ] **Step 2: Write the hashing RED test**

```rust
#[test]
fn index_hashing_visits_source_and_records_once() {
    let (document, snapshot) = overlapping_parent_range_fixture();
    let mut engine = LayoutEngine::default();
    engine.layout(&document, &snapshot, viewport(), LayoutInvalidation::Full, &mut shaper()).unwrap();
    let stats = engine.last_index_build_stats_for_test();
    assert_eq!(stats.source_bytes, snapshot.text().len());
    assert_eq!(stats.run_visits, document.text_runs.len());
    assert_eq!(stats.embedded_visits, document.embedded_blocks.len());
    assert_eq!(stats.block_visits, document.blocks.len());
}
```

- [ ] **Step 3: Run RED**

```powershell
rtk cargo test -p waml-markdown-editor --test layout_geometry hierarchy_rejects -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry child_before_parent -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry index_hashing_visits -- --nocapture
```

Expected: invalid hierarchies are accepted or misclassified, child-before-parent fingerprints differ, and the stats API is missing.

- [ ] **Step 4: Implement validation and deterministic postorder**

Add typed variants:

```rust
DuplicateBlockId { id: LayoutElementId },
MissingParent { block: LayoutElementId, parent: LayoutElementId },
HierarchyCycle { block: LayoutElementId },
OverlappingTextRuns { first: LayoutElementId, second: LayoutElementId },
```

`BlockHierarchy::try_new` must:

1. build an ID map with duplicate detection;
2. reject missing and self parents;
3. sort roots and siblings by `(source_start, source_end, block_id)`;
4. run iterative white/gray/black DFS;
5. store validated `postorder`.

Compute subtree fingerprints by iterating `postorder`, never by reversing the document vector.

- [ ] **Step 5: Implement the one-pass index hash builder**

Sort direct run intervals by source start. Reject overlaps. During one source-byte sweep, update the document hash and the active direct-run hash. Visit run metadata, embedded metadata, and block metadata once. A block hash combines fixed-size direct payload hashes. A subtree hash combines fixed-size child hashes in validated sibling order. Do not hash `block.source_range` bytes.

Store `IndexBuildStats` only under `#[cfg(any(test, debug_assertions))]` or behind hidden test accessors. Counters count semantic visits, not elapsed time.

- [ ] **Step 6: Run GREEN and regression**

```powershell
rtk cargo test -p waml-markdown-editor --test layout_geometry hierarchy_ -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry child_before_parent -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry index_hashing_visits -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry logical_cluster_ids_survive -- --nocapture
```

- [ ] **Step 7: Commit the hierarchy/index slice**

```powershell
rtk git add -- crates/waml-markdown-editor/src/layout/mod.rs crates/waml-markdown-editor/src/layout/engine.rs crates/waml-markdown-editor/tests/layout_geometry.rs
rtk git commit -m "fix(layout): validate hierarchy indexes"
```

---

### Task 3: Whole-paragraph styled shaping contract

**Files:**
- Modify: `crates/waml-markdown-editor/src/layout/mod.rs`
- Modify: `crates/waml-markdown-editor/src/layout/engine.rs`
- Modify: `crates/waml-markdown-editor/src/layout/makepad.rs`
- Modify: `crates/waml-markdown-editor/tests/layout_geometry.rs`

**Interfaces:**
- Consumes: Makepad `DrawText::layout_uncached`, `LayoutTextRun`, `TextMetrics`, exact `ShapedGlyph` payload.
- Produces: `ParagraphShapeRequest`, `ShapeSpan`, `ShapedParagraph`, explicit `ShapedRow`, paragraph intrinsic results, required first-row width, and stable shaped IDs.

- [ ] **Step 1: Write paragraph RED tests**

Add these exact behavior tests with a paragraph-aware fake shaper:

```rust
#[test]
fn styled_unbreakable_word_does_not_break_at_span_boundary() {
    let snapshot = layout_fixture(fixtures::styled_word("inter", "national"), 72.0);
    assert_eq!(snapshot.visual_lines().len(), 1);
    assert!(snapshot.visual_lines()[0].rect().size.x > 72.0);
}

#[test]
fn styled_bidi_uses_one_paragraph_context_and_exact_span_payloads() {
    let snapshot = layout_fixture(fixtures::styled_bidi("abc ", "אבג"), 300.0);
    let clusters = snapshot.glyph_clusters();
    assert_eq!(clusters.iter().map(|item| item.metrics.font).collect::<Vec<_>>(), expected_bidi_fonts());
    assert_eq!(clusters.iter().flat_map(|item| item.glyphs.iter().map(|glyph| glyph.glyph_id)).collect::<Vec<_>>(), expected_bidi_glyph_ids());
    assert_eq!(snapshot.point_to_source(dvec2(42.0, 8.0)), expected_rtl_position());
}

#[test]
fn empty_continuation_row_and_caret_boundary_survive_shaping() {
    let snapshot = layout_fixture(fixtures::trailing_newline("line\n"), 200.0);
    assert_eq!(snapshot.visual_rows().len(), 2);
    assert!(snapshot.visual_lanes()[1].cluster_range.is_empty());
    assert_eq!(snapshot.visual_lanes()[1].caret_stop_range.len(), 1);
}

#[test]
fn paragraph_shape_contract_honors_first_row_width() {
    let requests = recorded_requests(fixtures::first_row_width_paragraph(), 100.0, 20.0);
    assert_eq!(requests, vec![(100.0, 20.0)]);
    assert!(layout_fixture(fixtures::first_row_width_paragraph(), 100.0).visual_rows().len() > 1);
}

#[test]
fn stable_shape_ids_survive_wrap_and_bidi_order_changes() {
    let narrow = logical_cluster_ids(layout_fixture(fixtures::styled_bidi("abc ", "אבג"), 48.0));
    let wide = logical_cluster_ids(layout_fixture(fixtures::styled_bidi("abc ", "אבג"), 300.0));
    assert_eq!(narrow, wide);
}
```

The fake shaper records one request per paragraph and asserts that every input span is present in the one request.

- [ ] **Step 2: Run RED**

```powershell
rtk cargo test -p waml-markdown-editor --test layout_geometry styled_unbreakable_word -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry styled_bidi_uses -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry empty_continuation_row -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry paragraph_shape_contract -- --nocapture
```

Expected: current span-local calls break paragraph context; empty rows disappear; the default seam can ignore first-row width.

- [ ] **Step 3: Replace the trait seam**

Define the approved paragraph request/result types. The trait has only these required operations:

```rust
pub trait TextShaper {
    fn shape_paragraph(&mut self, request: ParagraphShapeRequest<'_>) -> Result<ShapedParagraph, LayoutError>;
    fn measure_paragraph_intrinsic(&mut self, request: ParagraphIntrinsicRequest<'_>) -> Result<ParagraphIntrinsic, LayoutError>;
}
```

Each returned row, fragment, and cluster carries a stable `GeometryElementId`. Validate duplicate IDs and missing input-span mappings before geometry composition. Derive IDs from logical paragraph/span/range/ordinal input, never row or visual order.

- [ ] **Step 4: Implement paragraph composition**

Replace `InlineComposer::push_run` with one `push_paragraph` operation. Use the backend row table directly. Preserve zero-cluster rows and their caret boundary. Copy exact span metrics/font/style/glyph payload into `GlyphCluster`.

The paragraph break map is built across the complete paragraph. It treats whitespace and explicit newline boundaries as legal breaks and does not treat style boundaries as breaks. Use whole-paragraph `BidiInfo` levels when visual rows reorder clusters.

- [ ] **Step 5: Implement Makepad paragraph and intrinsic adapters**

For visible paragraph shaping, configure each span and extract its exact unwrapped Makepad glyph clusters, then compose all logical clusters with the one paragraph bidi/break context and the two row widths. The public call remains one WAML paragraph request.

For intrinsic measurement, call only `DrawText::layout_uncached`. Reduce each result to numeric cluster advances and paragraph min/max widths, then drop it before the next span. Do not call cached `DrawText::layout` from the intrinsic method.

Add a Makepad unit test named `makepad_ten_thousand_intrinsics_do_not_retain_laidout_text` that records `LayoutCacheStats`, measures 10,000 short paragraphs, and asserts unchanged entry and byte counts.

- [ ] **Step 6: Run GREEN and paragraph regressions**

```powershell
rtk cargo test -p waml-markdown-editor --test layout_geometry styled_unbreakable_word -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry styled_bidi_uses -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry empty_continuation_row -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry paragraph_shape_contract -- --nocapture
rtk cargo test -p waml-markdown-editor makepad_ten_thousand_intrinsics -- --nocapture
rtk cargo test -p waml-markdown-editor makepad_shaper_retains_exact -- --nocapture
```

- [ ] **Step 7: Commit the paragraph slice**

```powershell
rtk git add -- crates/waml-markdown-editor/src/layout/mod.rs crates/waml-markdown-editor/src/layout/engine.rs crates/waml-markdown-editor/src/layout/makepad.rs crates/waml-markdown-editor/tests/layout_geometry.rs
rtk git commit -m "fix(layout): shape complete styled paragraphs"
```

---

### Task 4: Explicit visual rows and lanes

**Files:**
- Modify: `crates/waml-markdown-editor/src/layout/geometry.rs`
- Modify: `crates/waml-markdown-editor/src/layout/engine.rs`
- Modify: `crates/waml-markdown-editor/src/layout/mod.rs`
- Modify: `crates/waml-markdown-editor/tests/layout_geometry.rs`

**Interfaces:**
- Consumes: explicit shaped paragraph rows and stable cluster IDs from Task 3.
- Produces: `VisualRow`, `VisualLane`, lane-indexed clusters, row/lane navigation, min/max visible range, and O(1) alignment offsets.

- [ ] **Step 1: Write same-Y lane RED tests**

```rust
#[test]
fn same_y_table_lanes_keep_hit_testing_vertical_motion_and_selection_independent() {
    let snapshot = layout_fixture(fixtures::two_by_two_table(), 240.0);
    assert_eq!(snapshot.point_to_source(dvec2(20.0, 8.0)), fixtures::left_cell_position());
    assert_eq!(snapshot.point_to_source(dvec2(180.0, 8.0)), fixtures::right_cell_position());
    assert_eq!(snapshot.move_vertical(fixtures::left_cell_position(), 20.0, VerticalDirection::Down), fixtures::next_left_cell_position());
    assert_eq!(snapshot.selection_rects(fixtures::cross_cell_selection()).unwrap().len(), 2);
}

#[test]
fn hanging_marker_and_content_share_a_row_without_sharing_stops() {
    let snapshot = layout_fixture(fixtures::wrapped_hanging_item(), 96.0);
    assert_eq!(snapshot.visual_rows()[0].lanes.len(), 2);
    assert_eq!(snapshot.point_to_source(dvec2(4.0, 8.0)), fixtures::marker_position());
    assert_eq!(snapshot.point_to_source(dvec2(40.0, 8.0)), fixtures::content_position());
}

#[test]
fn visible_source_range_uses_all_lane_min_and_max_boundaries() {
    let snapshot = LayoutSnapshot::new_for_test(fixtures::out_of_order_visible_lanes());
    assert_eq!(snapshot.metadata().visible_source_range, range(0, 24));
}

#[test]
fn cluster_alignment_offset_lookup_is_constant_time() {
    let (snapshot, stats) = layout_with_offset_stats(fixtures::ten_thousand_cluster_paragraph());
    assert_eq!(stats.direct_lane_offset_lookups, snapshot.glyph_clusters().len());
    assert_eq!(stats.linear_lane_offset_scans, 0);
}
```

- [ ] **Step 2: Run RED**

```powershell
rtk cargo test -p waml-markdown-editor --test layout_geometry same_y_table_lanes -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry hanging_marker_and_content_share -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry visible_source_range_uses_all -- --nocapture
```

Expected: the flat same-Y stop lookup selects sibling content and the first/last visible range is wrong.

- [ ] **Step 3: Add row/lane data and explicit ownership**

Add `VisualRow`, `VisualLane`, `VisualLaneKind`, and stable row/lane IDs to geometry. Add `lane_index` to each `GlyphCluster`. Store `rows` and `lanes` in `LayoutSnapshot`. Keep `visual_lines()` as a flattened compatibility view until all non-owned callers migrate in a later approved wave.

Table placement creates a row key from table ID, table-row ordinal, and cell-line ordinal. Hanging placement creates one shared first-row key for marker/content. Snapshot assembly uses these keys and never compares Y floats to infer row ownership.

- [ ] **Step 4: Rewrite geometry queries around ownership**

Implement row-by-Y and lane-by-X/source selection. Vertical motion changes row before it chooses preferred X. Selection intersects one lane at a time. Empty lanes expose their one caret stop.

Replace the `line_offsets.iter().find(...)` cluster path with `lane_offsets[cluster.lane_index]`. Count direct accesses in a hidden test counter only.

Compute visible source range with a fold over all visible lanes:

```rust
let start = lanes.iter().map(|lane| lane.source_range.start()).min();
let end = lanes.iter().map(|lane| lane.source_range.end()).max();
```

- [ ] **Step 5: Run GREEN and existing geometry regressions**

```powershell
rtk cargo test -p waml-markdown-editor --test layout_geometry same_y_table_lanes -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry hanging_marker_and_content_share -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry visible_source_range_uses_all -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry source_point_round_trip -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry selection_rects_split -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry vertical_motion_uses -- --nocapture
```

- [ ] **Step 6: Commit the geometry slice**

```powershell
rtk git add -- crates/waml-markdown-editor/src/layout/geometry.rs crates/waml-markdown-editor/src/layout/engine.rs crates/waml-markdown-editor/src/layout/mod.rs crates/waml-markdown-editor/tests/layout_geometry.rs
rtk git commit -m "fix(layout): model visual rows and lanes"
```

---

### Task 5: Memoized intrinsic tables and cache pruning

**Files:**
- Modify: `crates/waml-markdown-editor/src/layout/engine.rs`
- Modify: `crates/waml-markdown-editor/tests/layout_geometry.rs`

**Interfaces:**
- Consumes: validated hierarchy postorder and paragraph intrinsic results.
- Produces: `IntrinsicSize { min_content, max_content }`, postorder memo, unconstrained intrinsic columns, and live table-cache pruning.

- [ ] **Step 1: Write table RED tests**

```rust
#[test]
fn unconstrained_table_uses_intrinsic_column_bases() {
    let snapshot = layout_fixture(fixtures::unconstrained_unequal_table(), 300.0);
    let widths = fixtures::table_column_widths(&snapshot);
    assert!(widths[1] > widths[0] * 2.0);
}

#[test]
fn styled_unbreakable_min_content_overflows_narrow_table() {
    let snapshot = layout_fixture(fixtures::styled_unbreakable_table(), 80.0);
    assert!(fixtures::table_rect(&snapshot).size.x > 80.0);
}

#[test]
fn nested_tables_measure_each_intrinsic_paragraph_once() {
    let (_, shaper) = layout_with_counting_shaper(fixtures::nested_table());
    assert_eq!(shaper.intrinsic_calls(), fixtures::nested_table_unique_paragraphs());
}

#[test]
fn removed_tables_prune_intrinsic_cache_entries() {
    let mut engine = LayoutEngine::default();
    layout_in_engine(&mut engine, fixtures::one_table());
    assert_eq!(engine.cached_table_intrinsic_count_for_test(), 1);
    layout_in_engine(&mut engine, fixtures::paragraph());
    assert_eq!(engine.cached_table_intrinsic_count_for_test(), 0);
}
```

- [ ] **Step 2: Run RED**

```powershell
rtk cargo test -p waml-markdown-editor --test layout_geometry unconstrained_table_uses -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry nested_tables_measure -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry removed_tables_prune -- --nocapture
```

- [ ] **Step 3: Build one postorder intrinsic memo**

Replace recursive `measure_block_intrinsic` with one `Vec<Option<IntrinsicSize>>`. Iterate hierarchy postorder. Measure each direct paragraph once, combine embedded widths, then combine child memo values by direct index. Cache table results by `(table_id, subtree_fingerprint)`.

Before or after layout, retain only cache keys whose table ID is live and whose fingerprint equals the current subtree fingerprint.

- [ ] **Step 4: Solve constrained and unconstrained columns from min/preferred widths**

Derive column count from table cells when constraints are empty. For each column, compute maximum cell minimum and maximum cell preferred width. Allocate preferred widths first when they fit. When they do not fit, shrink toward minimum. If minimum sum exceeds available width, keep minimum widths and overflow. Apply explicit min/max constraints to the same calculation.

- [ ] **Step 5: Run GREEN and 10k regression**

```powershell
rtk cargo test -p waml-markdown-editor --test layout_geometry unconstrained_table_uses -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry styled_unbreakable_min_content -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry nested_tables_measure -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry removed_tables_prune -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry ten_thousand_row_table -- --nocapture
```

- [ ] **Step 6: Commit the table slice**

```powershell
rtk git add -- crates/waml-markdown-editor/src/layout/engine.rs crates/waml-markdown-editor/tests/layout_geometry.rs
rtk git commit -m "perf(layout): memoize table intrinsics"
```

---

### Task 6: Structural shape budget and bounded convergence

**Files:**
- Modify: `crates/waml-markdown-editor/src/layout/mod.rs`
- Modify: `crates/waml-markdown-editor/src/layout/engine.rs`
- Modify: `crates/waml-markdown-editor/tests/layout_geometry.rs`

**Interfaces:**
- Consumes: final table widths, paragraph fingerprints, measurement-window summaries.
- Produces: `LayoutBudget`, `ShapeLedger`, typed budget errors, monotonic hydration, and total call evidence.

- [ ] **Step 1: Write budget RED tests**

```rust
#[test]
fn adversarial_height_changes_stop_at_structural_hydration_bound() {
    let (result, stats) = layout_with_adversarial_heights(fixtures::one_hundred_blocks());
    result.unwrap();
    assert!(stats.full_shape_calls <= 100);
    assert!(stats.hydration_passes <= 101);
}

#[test]
fn repeated_shape_key_returns_non_convergent_error() {
    let error = layout_with_repeated_pending_key(fixtures::paragraph()).unwrap_err();
    assert!(matches!(error, LayoutError::NonConvergent { .. }));
}

#[test]
fn exhausted_shape_budget_returns_typed_error_before_backend_call() {
    let (error, calls) = layout_with_budget(fixtures::paragraph(), LayoutBudget::for_test(0, 0, 1)).unwrap_err();
    assert!(matches!(error, LayoutError::BudgetExceeded { phase: LayoutWorkPhase::FullShape, .. }));
    assert_eq!(calls.full_shape, 0);
}

#[test]
fn total_shape_calls_equal_unique_intrinsic_and_visible_paragraph_keys() {
    let (_, calls) = layout_with_counting_shaper(fixtures::table_with_offscreen_tail());
    assert_eq!(calls.full_shape, fixtures::visible_unique_paragraphs());
    assert_eq!(calls.intrinsic, fixtures::table_unique_intrinsic_paragraphs());
}
```

- [ ] **Step 2: Run RED**

```powershell
rtk cargo test -p waml-markdown-editor --test layout_geometry adversarial_height_changes -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry repeated_shape_key -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry total_shape_calls_equal -- --nocapture
```

Expected: the current open loop has no structural limit or typed failure.

- [ ] **Step 3: Add explicit budget and ledger types**

```rust
pub enum LayoutWorkPhase { Intrinsic, FullShape, Hydration }

pub struct LayoutBudget {
    max_full_shape_calls: usize,
    max_intrinsic_calls: usize,
    max_hydration_passes: usize,
    max_full_shape_source_bytes: usize,
    max_intrinsic_source_bytes: usize,
}
```

Add `BudgetExceeded { phase, limit, observed }` and `NonConvergent { passes, pending }` to `LayoutError`. Build structural limits from the validated index exactly as specified in the design. Add a hidden test-only budget override; production always uses structural limits.

- [ ] **Step 4: Replace the open loop with monotonic phases**

Compute intrinsic memo and final table widths before paragraph hydration. Keep a `HashSet<(block_id, paragraph_fingerprint, width_bits)>` ledger. Each repeat must add a key. Check call and byte budgets before backend calls. Return a typed error on exhaustion or a pass that adds no key.

- [ ] **Step 5: Run GREEN and virtualization regressions**

```powershell
rtk cargo test -p waml-markdown-editor --test layout_geometry adversarial_height_changes -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry repeated_shape_key -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry total_shape_calls_equal -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry cold_large_document -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry measurement_growth_and_shrink -- --nocapture
rtk cargo test -p waml-markdown-editor --test layout_geometry repeated_far_scrolls -- --nocapture
```

- [ ] **Step 6: Commit the budget slice**

```powershell
rtk git add -- crates/waml-markdown-editor/src/layout/mod.rs crates/waml-markdown-editor/src/layout/engine.rs crates/waml-markdown-editor/tests/layout_geometry.rs
rtk git commit -m "perf(layout): bound paragraph hydration"
```

---

### Task 7: Full verification and corrected B report

**Files:**
- Modify: `.superpowers/sdd/2026-07-31-markdown-editor-foundation/whole-plan-fix-b-report.md`
- Verify only: all B5 WAML and Makepad files

**Interfaces:**
- Consumes: all slice commits and fresh command output.
- Produces: exact B5 evidence and a whole-plan `NOT SAFE` high-review handoff.

- [ ] **Step 1: Run focused B5 suites**

```powershell
rtk cargo test -p waml-markdown-editor --test layout_geometry -- --nocapture
rtk cargo test -p waml-markdown-editor layout::makepad::tests -- --nocapture
```

From `C:\dev\makepad`:

```powershell
rtk cargo test -p makepad-draw text::layouter::tests -- --nocapture
```

Record exact passed/failed/ignored counts and cache-stat assertions.

- [ ] **Step 2: Run full WAML and Makepad gates**

```powershell
rtk cargo test -p waml-markdown-editor --all-targets
rtk cargo clippy -p waml-markdown-editor --all-targets -- -D warnings
rtk cargo fmt --all -- --check
rtk git diff --check
```

From `C:\dev\makepad`:

```powershell
rtk cargo test -p makepad-draw
rtk cargo clippy -p makepad-draw --all-targets -- -D warnings
rtk cargo fmt --all -- --check
rtk git diff --check
```

Do not report success from an earlier run. Use the final fresh outputs.

- [ ] **Step 3: Correct the B report from evidence**

Replace B4 overclaims with exact B5 contracts and observed results. Record:

- WAML and Makepad commit hashes;
- exact focused and full test counts;
- exact Clippy and format results;
- unchanged Makepad retained cache entries/bytes for 10,000 intrinsics;
- exact paragraph and intrinsic call counts;
- deep hierarchy and source-visit counts;
- explicit convergence limit and adversarial observed calls;
- TokenSave savings;
- warnings that come from upstream dependencies;
- confirmation that `crates/waml-editor/src/app.rs` stayed unstaged and untouched.

Keep the whole-plan verdict `NOT SAFE` because widget D, Plan3, and high review are not complete.

- [ ] **Step 4: Commit the evidence report**

```powershell
rtk git add -f -- .superpowers/sdd/2026-07-31-markdown-editor-foundation/whole-plan-fix-b-report.md
rtk git commit -m "docs(layout): record B5 evidence"
```

- [ ] **Step 5: Final status and high-review handoff**

```powershell
rtk git status --short
rtk git log -8 --oneline
```

Expected WAML status: only the pre-existing unstaged `crates/waml-editor/src/app.rs` change. Expected Makepad status: only unrelated pre-existing files, including `docs/superpowers/plans/` if it is still present. Stop and request high review. Do not start widget D or Plan3.

## Self-review checklist

- Spec coverage: Tasks 1-7 cover every approved design section and every named regression.
- Placeholder scan: clean; every code step names its required behavior and result.
- Type consistency: paragraph, row/lane, hierarchy, intrinsic, budget, and Makepad API names are consistent between producing and consuming tasks.
- Repository separation: Task 1 is the only Makepad commit. Tasks 2-7 are WAML worktree commits.
- TDD: every production slice starts with a focused failing test and records the expected failure reason.
- Stop condition: Task 7 ends at the high-review checkpoint.
