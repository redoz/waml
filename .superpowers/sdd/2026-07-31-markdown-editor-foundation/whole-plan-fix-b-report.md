# Whole-plan fix wave B2 layout report

## Status

The layout implementation gates are green. The whole plan is **NOT SAFE**
until widget integration is complete and the whole plan has a new review.

This report supersedes the old root `whole-plan-fix-b-report.md`. That report
described a byte-based wrapping heuristic and metrics-only renderer data. B2
removes the heuristic and retains exact glyph data from the shaping authority.

## Scope

B2 changes only these layout-owned paths:

- `crates/waml-markdown-editor/src/layout/engine.rs`
- `crates/waml-markdown-editor/src/layout/geometry.rs`
- `crates/waml-markdown-editor/src/layout/makepad.rs`
- `crates/waml-markdown-editor/src/layout/mod.rs`
- `crates/waml-markdown-editor/tests/layout_geometry.rs`
- this report

The pre-existing unstaged change in `crates/waml-editor/src/app.rs` is not part
of B2 and was not changed by this work.

## Layout contracts proved

1. Shaped clusters retain each renderer glyph ID, exact resolved Makepad
   `FontId`, requested font key, origin, advance, size, vertical metrics,
   baseline, offset, and color.
2. The Makepad parity test compares the retained payload with raw shaping for
   a ligature, a combining sequence, and right-to-left text.
3. Snapshot APIs distinguish document block ranges from visible-local ranges
   and map local blocks back to document indices.
4. The first layout measures block data before it selects the visible range.
   Growth and shrink both converge the measured positions and visible start.
5. The viewport default overscan is 320 logical pixels. A boundary regression
   proves the document and local ranges independently.
6. Cache fingerprints include flow, content, constraints, parent, range,
   width, position, and height inputs. A dirty document range is explicit,
   and unchanged block layout data keeps its `Arc` identity.
7. Parent-child flow is recursive. Quote insets and sibling spacing are
   applied once, and structural parents aggregate child height without a
   phantom line.
8. Hanging flow shares one baseline for marker and content. Table flow solves
   shared column widths and positions rows and cells from those columns.

## TDD evidence

- Baseline: focused layout tests 17 passed; full crate tests 82 passed.
- Glyph and range RED: 23 intended missing-API errors. GREEN: 19 focused
  tests passed.
- Paint payload RED: the exact Makepad color field was absent. GREEN: the
  complex-script raw-shaping parity test and all 19 focused tests passed.
- Overscan, cache, and dirty-range RED: eight intended missing APIs. GREEN:
  22 focused tests and 88 full crate tests passed.
- Tree-flow RED: quote origins and table height were wrong. GREEN: 24 focused
  tests and 90 full crate tests passed.

## Commits

- `6d3de522` — `feat(layout): retain exact glyph payload`
- `ee5acc5b` — `fix(layout): converge measured block cache`
- `67a29fe2` — `fix(layout): dispatch block flow trees`

## Final gates

- `rtk cargo test -p waml-markdown-editor --test layout_geometry` — 24 passed.
- `rtk cargo test -p waml-markdown-editor` — 90 passed.
- `rtk cargo fmt -p waml-markdown-editor -- --check` — passed.
- `rtk cargo clippy -p waml-markdown-editor --all-targets -- -D warnings` —
  passed.
- `rtk git diff --check` — passed.

## Remaining work

B2 does not change widget, session, input, or application production code. It
does not claim that the widget consumes the exact glyph payload. Widget
integration and a whole-plan review remain necessary before the whole plan can
be marked safe.
