# Whole-plan fix waves B2 and B3 layout report

## Status

The B2 and B3 layout gates are green. The whole plan is **NOT SAFE** until
widget integration is complete and the whole plan has a new high review.

This report supersedes the former root `whole-plan-fix-b-report.md`. The old
report described byte-based wrapping estimates, metrics-only renderer data,
and contiguous document ranges for compact geometry. Those claims are not
valid for the final layout implementation.

## Scope

Waves B2 and B3 change only these layout-owned paths:

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
4. Cluster ordinals are normalized once over the complete block output. They
   stay unique across runs, hanging branches, bidirectional reordering, and
   wrapped rows.
5. Each compact visible `BlockGeometry` stores its exact document index.
   Sparse nested views do not use `range.start + local_index`. The legacy
   `blocks()` and `visible_block_range()` compatibility aliases are documented
   as deprecated, and the legacy range is local so existing compact slicing is
   safe.
6. A cold layout creates cheap summaries for the full document, but it shapes
   only blocks in the viewport plus a minimum 320 logical-pixel measurement
   window. The 10,000-block regression retains at most 50 visible layouts and
   shapes at most 50 distinct blocks. A scroll-only update adds at most 12.
7. `LayoutInvalidation::Viewport` is explicit. It keeps stable cached
   measurements and measures newly visible estimates without declaring the
   full document invalid.
8. Measurement growth and shrink reposition the tree before visible selection.
   Cache fingerprints cover flow, content, constraints, parent, range, and
   width, while unchanged measured layout data keeps its `Arc` identity.
9. Parent-child flow is recursive. Quotes aggregate child height without a
   phantom line. Structural table and row blocks do not add text height.
10. Visible tables measure cell min-content through the shaping authority
    before the final width plan. Fixed constraints keep their caps, flexible
    columns grow in measured-content proportion, and Start, Center, and End
    alignments offset cell content after measurement.
11. Hanging flow safely splits a marker from a text run that spans the marker
    boundary. Marker and content pieces with different metrics share the
    maximum first-row ascender as one paint baseline.

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

## Commits

- `6d3de522` — `feat(layout): retain exact glyph payload`
- `ee5acc5b` — `fix(layout): converge measured block cache`
- `67a29fe2` — `fix(layout): dispatch block flow trees`
- `0351fd75` — `fix(layout): keep snapshots thread-safe`
- `ced13d3` — `fix(layout): match Makepad glyph paint`
- `1711a28` — `fix(layout): bound cold viewport shaping`
- `180d507` — `fix(layout): measure table content widths`

## Final gates

- `rtk cargo test -p waml-markdown-editor --test layout_geometry` — 29 passed.
- `rtk cargo test -p waml-markdown-editor` — 96 passed.
- `rtk cargo fmt -p waml-markdown-editor -- --check` — passed.
- `rtk cargo clippy -p waml-markdown-editor --all-targets -- -D warnings` —
  passed with zero Rust lint errors. Cargo prints two upstream Makepad
  duplicate-package selection warnings.
- `rtk git diff --check` — passed.

## Remaining work

These waves do not change widget, session, input, or application production
code. They do not claim that the widget paints the retained glyph payload.
Widget integration and a new whole-plan high review remain necessary before
the whole plan can be marked safe.
