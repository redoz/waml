# Whole-plan fix waves B2 through B4 layout report

## Status

The B2 through B4 layout gates are green. B4 is complete in this branch and is
awaiting review. The whole plan is **NOT SAFE** until widget integration D is
complete and the whole plan has a new high review.

## B4 implemented design

B4 uses one indexed layout view of blocks, text runs, and embedded content. It
replaces separate run stacking with one source-ordered inline composer
for paragraph and hanging content. Contiguous runs with identical full metrics
use one shaping call. Different styles shape separately and compose
on the same lines. Hanging intervals will be clamped to each original run, and
each composed line will use shared vertical metrics.

Logical cluster identity is assigned from source range, original run
order, and cluster order before bidi reordering or wrapping. Width and visual
direction changes will not renumber the same logical clusters.

The block cache keeps document-wide cheap summaries and exact measured heights,
but it keeps full glyph payloads only in the current 320-pixel measurement
window. Far scrolling evicts old full payloads. Compact block geometry uses an
optional document index instead of a public sentinel, and layout snapshot
payloads have compile-time `Send + Sync` assertions.

Table intrinsics are exact for the full table. One indexed O(total table
content) numeric pass uses a cheap intrinsic-measurement seam that does not
retain glyphs or count as full viewport shaping. Its result will be cached by
the full table-subtree content fingerprint. It will preserve unbreakable spans
across adjacent styled runs, include nested and embedded cell content, and be
reused on later layouts. Full inline shaping and glyph payloads remain bounded
to the viewport measurement window. Center and End alignment is applied
independently to every wrapped line.

This report supersedes the former root `whole-plan-fix-b-report.md`. The old
report described byte-based wrapping estimates, metrics-only renderer data,
and contiguous document ranges for compact geometry. Those claims are not
valid for the final layout implementation.

## Scope

Waves B2 through B4 change only these layout-owned paths:

- `crates/waml-markdown-editor/src/layout/engine.rs`
- `crates/waml-markdown-editor/src/layout/geometry.rs`
- `crates/waml-markdown-editor/src/layout/makepad.rs`
- `crates/waml-markdown-editor/src/layout/mod.rs`
- `crates/waml-markdown-editor/tests/layout_geometry.rs`
- this report

The pre-existing unstaged change in `crates/waml-editor/src/app.rs` is not part
of these waves and was not changed.

## Final layout contracts

1. The shaping payload retains glyph ID, resolved Makepad `FontId`, requested
   font key, font size, paint scale, scaled origin and advance, vertical
   metrics, baseline, raw offset, and color.
2. Makepad parity covers ligatures, combining text, and Arabic. A separate
   real-Makepad multi-row test uses `font_scale = 1.5` and compares final
   emitted origins with Makepad's raw row-origin, glyph-origin, and offset
   paint transform. It does not compare against an adapter-derived formula.
3. Raw Makepad row boundaries and scaled row tops survive shaping. Wrapped
   rows therefore keep their exact paint Y positions.
4. Cluster ordinals are assigned from stable logical source order before bidi
   reordering or wrapping. They stay stable across width changes and visual
   order changes.
5. Each compact visible `BlockGeometry` stores its exact document index.
   Sparse nested views do not use `range.start + local_index`. The legacy
   `blocks()` and `visible_block_range()` compatibility aliases are documented
   as deprecated, and the legacy range is local so existing compact slicing is
   safe.
6. A cold layout creates cheap summaries for the full document, but it shapes
   and retains full layout payloads only in the viewport plus a minimum 320
   logical-pixel measurement window. Repeated far scrolling through 10,000
   blocks retains at most 40 full payloads for the tested 24-pixel blocks.
7. `LayoutInvalidation::Viewport` is explicit. It keeps stable cached
   measurements and measures newly visible estimates without declaring the
   full document invalid.
8. Measurement growth and shrink reposition the tree before visible selection.
   Cache fingerprints cover flow, content, constraints, parent, range, and
   width, while unchanged measured layout data keeps its `Arc` identity.
9. Parent-child flow is recursive. Quotes aggregate child height without a
   phantom line. Structural table and row blocks do not add text height.
10. Tables measure exact numeric min-content for every cell subtree before the
    final width plan. Unbreakable words cross adjacent styled runs, nested and
    embedded content contributes, and the result is cached by the full subtree
    fingerprint. A 10,000-row regression performs 10,000 intrinsic
    measurements once and reuses them while full shaping stays within 50 cell
    IDs per tested viewport.
11. Hanging flow safely splits a marker from a text run that spans the marker
    boundary. Marker and content pieces with different metrics share the
    maximum first-row ascender as one paint baseline.
12. Paragraph and hanging content use one multi-run composer. Every wrapped
    Center or End line receives its own offset, and the line, clusters, carets,
    and glyph origins move together.

## TDD evidence

- B2 baseline: 17 focused tests and 82 full crate tests passed.
- B2 glyph/range RED: 23 intended missing APIs. GREEN: 19 focused tests.
- B2 paint payload RED: missing exact paint color. GREEN: complex-script raw
  shaping parity and 19 focused tests.
- B2 overscan/cache RED: eight intended missing APIs. GREEN: 22 focused and 88
  full crate tests.
- B2 tree-flow RED: wrong quote origin and phantom table height. GREEN: 24
  focused and 90 full crate tests.
- B3 paint RED: missing paint scale, then wrong multi-row X and Y positions.
  GREEN: exact scaled multi-row paint parity and unique block ordinals; 25
  focused and 92 full crate tests.
- B3 mapping/virtualization RED: three missing document-index APIs and one
  missing viewport invalidation variant. The first implementation also exposed
  two predecessor-convergence regressions. GREEN: sparse exact mapping, safe
  compact consumer slicing, bounded 10,000-block cold layout, and stable
  growth/shrink; 27 focused and 94 full crate tests.
- B3 table/hanging RED: equal table widths ignored measured proportions and
  alignment, and a spanning run produced no marker cluster. GREEN: measured
  proportional columns, all three alignments, safe marker splitting, and a
  mixed-metric shared baseline; 29 focused and 96 full crate tests.
- B4 inline RED: adjacent styled runs stacked, a wholly pre-marker run extended
  outside its source range, and width-dependent bidi output renumbered logical
  clusters. GREEN: shared composition, clamped splits, stable IDs, optional
  document indexes, and explicit `Send + Sync`; 34 focused and 102 full tests.
- B4 cache RED: the 10,000-block engine had no bounded-payload contract. GREEN:
  all summaries and exact heights remain while full payload retention stays
  within the 320-pixel measurement window; 35 focused and 103 full tests.
- B4 table/alignment RED: nested embedded width was 60 instead of 90, a
  10,000-row table full-shaped all cells, and a centered wrapped line started at
  zero instead of five. GREEN: cached numeric subtree intrinsics and per-line
  payload alignment; 38 focused and 106 full tests.

## Commits

- `6d3de522` — `feat(layout): retain exact glyph payload`
- `ee5acc5b` — `fix(layout): converge measured block cache`
- `67a29fe2` — `fix(layout): dispatch block flow trees`
- `0351fd75` — `fix(layout): keep snapshots thread-safe`
- `ced13d31` — `fix(layout): match Makepad glyph paint`
- `1711a287` — `fix(layout): bound cold viewport shaping`
- `180d5078` — `fix(layout): measure table content widths`
- `b3ce1ad4` — `docs(layout): record B3 verification`
- `e001b796` — `docs(layout): record B4 design`
- `67a9f43` — `fix(layout): compose inline runs stably`
- `7394464` — `perf(layout): bound retained block payloads`
- `0719e6c` — `fix(layout): cache table widths and align lines`

## Final gates

- `rtk cargo test -p waml-markdown-editor --test layout_geometry` — 38 passed.
- `rtk cargo test -p waml-markdown-editor` — 106 passed.
- `rtk cargo fmt --all -- --check` — passed.
- `rtk cargo clippy -p waml-markdown-editor --all-targets -- -D warnings` —
  passed with zero Rust lint errors. Cargo prints two upstream Makepad
  duplicate-package selection warnings.
- `rtk git diff --check` — passed.

## Remaining work

These waves do not change widget, session, input, or application production
code. They do not claim that the widget paints the retained glyph payload.
Widget integration D and a new whole-plan high review remain necessary before
the whole plan can be marked safe. B4 remains awaiting that review.
