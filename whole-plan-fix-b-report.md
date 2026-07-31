# Whole-plan fix wave B report

## Scope

Changed only the Markdown editor layout authority and its geometry tests:

- `crates/waml-markdown-editor/src/layout/engine.rs`
- `crates/waml-markdown-editor/src/layout/geometry.rs`
- `crates/waml-markdown-editor/tests/layout_geometry.rs`

## Fixed contracts

- The document-local block-summary index now premeasures potentially wrapping
  predecessors before it selects a scrolled visible window. It then reflows
  summary positions and derives visible blocks from the corrected index. A
  tall wrapped block can no longer leave the viewport represented by stale
  downstream estimates.
- Renderer-ready glyph placements now carry the exact `TextMetrics` used by
  layout and hit testing. `LayoutSnapshot::glyph_clusters` exposes those same
  authoritative placements to a renderer instead of requiring a parallel
  font/weight/italic/line-spacing reconstruction.
- Added regressions for a scrolled height shift and for glyph metric carriage.

## Verification

- `rtk cargo test -p waml-markdown-editor --test layout_geometry` — 17 passed.
- `rtk cargo test -p waml-markdown-editor` — 82 passed.
- `rtk git diff --check` — clean.

## Widget migration

None. The layout API addition is read-only (`glyph_clusters`) and does not
require a widget/session/input migration. Renderers should consume the
cluster metrics carried by the snapshot rather than build separate geometry.
