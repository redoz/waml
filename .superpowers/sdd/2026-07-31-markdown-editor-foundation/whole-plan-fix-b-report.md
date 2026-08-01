# Whole-plan fix waves B2 through B5 layout report

## Status

The B2 through B5 layout gates are green. B5 is complete in this branch and is
awaiting review. The whole plan is **NOT SAFE** until widget integration D is
complete and the whole plan has a new high review.

## B5 corrections to the B4 claims

B4 claimed contracts this report now states exactly, because B5 changed how they
are met:

- B4 said table intrinsics are "exact numeric min-content". They are now a
  memoized `IntrinsicSize { min_content, max_content }` per block, computed in
  validated hierarchy postorder. A parent reads child entries by index, so a
  nested table no longer remeasures its descendants once per enclosing table.
- B4 said tables use "measured proportions". Unconstrained tables previously
  split the available width equally and ignored measurement entirely. Columns
  now take their minimum from the maximum cell min-content and their preferred
  width from the maximum cell max-content, allocate preferred widths when they
  fit, shrink toward minimum when they do not, and overflow rather than break an
  unbreakable word. Explicit constraints join the same calculation.
- B4 did not state a convergence bound. The measurement loop was open. It is now
  bounded by an explicit `LayoutBudget` with typed failure.
- B4 treated same-Y lines as one row implicitly. Row ownership is now explicit.

## B5 implemented design

### Rows and lanes

The snapshot stores `VisualRow` and `VisualLane` arrays. A row key is created by
container placement — the table ID plus table-row ordinal plus cell-line
ordinal, or the block itself outside a table — so snapshot assembly never infers
row ownership from a floating-point Y comparison. A hanging marker and the first
content line share one row key; continuation lines create later rows.

- Point-to-source selects a row by Y, then a lane by X, and only falls back to
  the nearest lane when the point is outside every lane in the row.
- Source-to-point selects the lane that owns the source range.
- Vertical motion changes visual row before it chooses preferred X, so it cannot
  step into a sibling lane of the current row.
- Selection intersects one lane at a time and never combines caret stops from
  unrelated same-Y lanes.
- The visible source range folds the minimum start and maximum end over all
  lanes, so a visible prefix lane stored after later lanes still sets the start.
- Each cluster stores its lane index and each lane its final X offset. Cluster
  placement reads the offset by direct index; `LaneOffsetStats` records
  `direct_lane_offset_lookups` equal to the cluster count and
  `linear_lane_offset_scans` of zero.

`visual_lines()` remains as a flattened compatibility view in block order.

### Work budget

`LayoutBudget::for_index` derives hard limits before shaping. A block can be
shaped once per distinct final width, and a width can only change when a table
first measures its intrinsics, so the structural call limit is
`blocks * (tables + 1)`, with `max_hydration_passes` one greater and matching
source-byte limits. `max_intrinsic_calls` is the number of paragraph blocks in
live table subtrees.

A `ShapeLedger` key is block ID, paragraph content fingerprint, and final width
key. A key can be fully shaped at most once per layout; a repeat, or a pass that
adds no key while work is still pending, returns `LayoutError::NonConvergent`.
Call, byte, and pass limits are checked before the backend call, so an exhausted
budget returns `LayoutError::BudgetExceeded { phase, limit, observed }` and never
reaches the shaper. Table intrinsic cache entries are pruned each layout to live
tables with the current subtree fingerprint.

### Known deviations from the B5 design

1. The design specifies solving every table width once in a phase that precedes
   all paragraph hydration. The implementation keeps the existing lazy path,
   which measures a table's intrinsics when it first enters the measurement
   window, and re-solves widths at that point. The observable contracts the
   design asks for — no key shaped twice, bounded passes, typed exhaustion — are
   enforced by the ledger and budget rather than by phase ordering. This keeps
   the 10,000-row virtualization behaviour unchanged.
2. `LayoutError::NonConvergent` is a defensive guard. With widths re-solved only
   on first table measurement, every pending block claims a fresh ledger key, so
   no fixture reaches it; it has no behavioural test.
3. `VisualRow` carries no `baseline` field. Block output does not produce a
   per-row baseline, and no consumer needs one yet.

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

## B5 TDD evidence

- Task 4 rows/lanes RED: `visual_rows`, `visual_lanes`, and
  `last_lane_offset_stats_for_test` did not exist, so the four new tests failed
  to compile. GREEN: 50 focused tests.
- Task 5 table RED: 4 failed — equal-split columns ignored intrinsic bases, a
  narrow table did not overflow on an unbreakable word, a nested table measured
  its paragraph twice, and there was no cache-pruning API. GREEN: 54 focused.
- Task 6 budget RED: `LayoutBudget`, `LayoutWorkPhase`, and
  `last_shape_call_stats_for_test` did not exist. GREEN: 59 focused.

## B5 commits

- `76b9a1f5` — `fix(layout): model visual rows and lanes`
- `c76f1794` — `perf(layout): memoize table intrinsics`
- `355758eb` — `perf(layout): bound paragraph hydration`

Earlier in this wave: `1cd29571` `fix(layout): validate hierarchy indexes`,
`29e0ef80` `build: pin makepad to the pushed uncached-layout rev`, `2f50ece1`
`fix(layout): honor makepad line spacing scale in row advance`.

Makepad fork commit: `12d60b45` `feat(text): add uncached layout API`.

## Final gates

Fresh output from the final B5 tree:

- `cargo test -p waml-markdown-editor --test layout_geometry` — 59 passed,
  0 failed, 0 ignored.
- `cargo test -p waml-markdown-editor --lib layout::makepad` — 7 passed,
  0 failed. Includes
  `makepad_ten_thousand_intrinsics_do_not_retain_laidout_text`, which asserts
  Makepad's retained `LaidoutText` cache entry count and byte count are
  unchanged across 10,000 intrinsic measurements.
- `cargo test -p waml-markdown-editor --all-targets` — 7 + 22 + 59 + 2 + 16 + 23
  passed, 0 failed.
- `cargo test --workspace` — 0 failed.
- `cargo clippy -p waml-markdown-editor --all-targets -- -D warnings` — zero
  errors. Cargo still prints two upstream Makepad duplicate-package selection
  warnings (`bitflags`, `cfg-if`), which come from the dependency, not this
  crate.
- `cargo fmt --all -- --check` — passed.
- `git diff --check` — passed.

From `C:\dev\makepad`:

- `cargo test -p makepad-draw text::layouter` — 15 passed, 0 failed,
  35 filtered out.
- `cargo clippy -p makepad-draw --all-targets -- -D warnings` — 115 errors, all
  in vendored upstream libraries (`libs/regex`, `libs/gif`). Zero errors in
  `text/layouter.rs`, `text/fonts.rs`, or `draw_text.rs`.
- `cargo fmt -p makepad-draw -- --check` — passed. The workspace-wide
  `cargo fmt --all -- --check` is dirty in unrelated vendored crates.
- `git status --short` — only the untracked, unrelated `docs/superpowers/plans/`.

Observed call evidence:

- 100 blocks with an adversarial per-call height shaper: layout returns `Ok`,
  full shape calls stay at or below 100, hydration passes at or below 101.
- 100 blocks in a 100-pixel viewport: full shape calls equal the shaper's
  paragraph request count and stay below 100; intrinsic calls are zero with no
  table present.
- Nested table: exactly 1 intrinsic call for 1 unique paragraph.
- Budget `for_test(0, 0, 1)`: `BudgetExceeded { phase: FullShape, limit: 0,
  observed: 0 }` with zero shaper calls recorded.
- Deep hierarchy validation and single-visit index hashing remain covered by
  `deep_hierarchy_validation_and_indexing_are_iterative_and_linear` and
  `index_hashing_visits_source_and_records_once`, both passing.

`crates/waml-editor/src/app.rs` was not touched by B5; the worktree is clean.

## Known pre-existing defect found during B5 verification

`cargo test -p waml-syntax --lib` intermittently fails
`incremental::properties::valid_edit_sequences_match_full_parse`. Incremental
reparse keeps a trailing space before EOF as a `WhitespaceToken` inside the last
ATX heading, while a full parse attaches it as EOF leading trivia, so the heading
node range differs by one byte. This is in `waml-syntax`, which B5 does not
touch, and it is found only by specific proptest seeds. The failing seed is:

```
cc f3c3c9271d61afad350e28dcb129dcad278d8cfb424bb43204961fdc47599548 # shrinks to source = "# H1\n## H2\n### H3\n#### H4\n##### H5\n###### H6\naప®A®AA® 𐬀ড়¡Aaa0 🫰", edits = [(12292425823953761300, 5796941469478648778, "\u{dd6}0a𑚀ໆ"), (17592253029422531037, 8462834623090167354, "מּa𞹴®ೠA𘠀a a 0 ቘAﷰ𜰀0ΣΣ A A𝔍ⵯ𞸅a🢐எ®"), (3717481895314225075, 10060811693013582572, "豈A\u{bbe}0 "), (3853727756417273253, 611882602685291348, "A ")]
```

The generated `crates/waml-syntax/proptest-regressions/` file was not committed,
because pinning the seed would make the shared gate permanently red and block
unrelated work. The defect is real and needs its own fix.

## Remaining work

These waves do not change widget, session, input, or application production
code. They do not claim that the widget paints the retained glyph payload.
Widget integration D and a new whole-plan high review remain necessary before
the whole plan can be marked safe. B4 remains awaiting that review.
