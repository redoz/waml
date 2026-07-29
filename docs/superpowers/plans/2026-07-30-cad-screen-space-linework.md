# CAD-style screen-space linework implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make CAD-style fixed-pixel canvas linework the experimental default while retaining the current zoom-scaled formulas behind one private mode branch.

**Architecture:** Add a pure `LineworkMetrics` calculation under the class renderer and compute it once when building `RenderSnapshot`. Existing group, edge, node, and card drawing paths consume that bundle; the shared `AccentFrame` receives a separate stroke multiplier so node-border sizing can change without changing its zoom-scaled shadow or contrast behavior.

**Tech Stack:** Rust, Makepad `script_mod!` shaders, existing `waml-editor` unit tests, native Windows editor capture via `scripts/capture-window.ps1`.

## Global constraints

- CAD rendering is the internal default; add no user-facing name, control, persistence, or public configuration.
- Camera zoom continues to scale world positions, card/group bounds, padding, hit regions, text positions, and font sizes.
- Node borders, hidden-group borders, divider thickness, routed edge strokes, elbow radii, dash periods, arrowheads, diamonds, and port nubs remain fixed in logical screen pixels in CAD mode.
- Edge cardinalities and role names are text labels and therefore continue scaling with zoom.
- Preserve current scaled formulas exactly behind `LineworkMode::Scaled`, including floors and clamps.
- Preserve the existing 100% appearance: 1.5 px node frame inset, 1 px dividers, 3 px edges, 6 px group dash period, 10 px end markers, and 6 px nubs.
- Keep edge/node zoom uniforms for their existing color-deepening, shadow, and contrast behavior; isolate only linework sizing.
- Do not change the solver, scene model, camera, selection, placement, routing, interaction, or hit-testing code.
- Use `rtk` to prefix every shell command.

## File structure

| File | Responsibility | Change |
|---|---|---|
| `crates/waml-editor/src/canvas/class/render/metrics.rs` | Private linework mode and pure metric derivation | Create |
| `crates/waml-editor/src/canvas/class/render/mod.rs` | Render-module wiring and once-per-frame metric snapshot | Modify |
| `crates/waml-editor/src/canvas/class/render/groups.rs` | Hidden-group stroke and dash sizing | Modify |
| `crates/waml-editor/src/canvas/class/render/edges.rs` | Edge, elbow, and end-marker sizing | Modify |
| `crates/waml-editor/src/canvas/class/render/nodes.rs` | Node-frame scale, divider thickness, and nub sizing | Modify |
| `crates/waml-editor/src/canvas/class/widget.rs` | Construct `RenderSnapshot`; declare canvas draw shaders/resources | Modify only if snapshot construction needs the new field |
| `crates/waml-editor/src/frame.rs` | Add an independent multiplier to `AccentFrame` stroke geometry | Modify |

No scene, camera, interaction, label, or model files change.

---

### Task 1: Define and test the linework metric boundary

**Files:**

- Create: `crates/waml-editor/src/canvas/class/render/metrics.rs`
- Modify: `crates/waml-editor/src/canvas/class/render/mod.rs:1-20`
- Test: inline `#[cfg(test)] mod tests` in `metrics.rs`

**Interfaces:**

- Produces: `LineworkMode::{Cad, Scaled}`
- Produces: `DEFAULT_LINEWORK_MODE: LineworkMode`
- Produces: `LineworkMetrics::for_zoom(mode: LineworkMode, zoom: f64) -> LineworkMetrics`
- Produces fields consumed by later tasks: `frame_stroke_scale`, `group_stroke_width`, `group_dash_period`, `divider_thickness`, `edge_thickness`, `marker_size`, and `nub_size`

- [ ] **Step 1: Register the module and write failing metric tests**

Add `mod metrics;` to `render/mod.rs`. Create `metrics.rs` with the public-within-render type declarations and tests first:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LineworkMode {
    Cad,
    Scaled,
}

pub(super) const DEFAULT_LINEWORK_MODE: LineworkMode = LineworkMode::Cad;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct LineworkMetrics {
    pub(super) frame_stroke_scale: f32,
    pub(super) group_stroke_width: f32,
    pub(super) group_dash_period: f32,
    pub(super) divider_thickness: f64,
    pub(super) edge_thickness: f64,
    pub(super) marker_size: f64,
    pub(super) nub_size: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::viewport::{MAX_ZOOM, MIN_ZOOM};

    const CAD_BASELINE: LineworkMetrics = LineworkMetrics {
        frame_stroke_scale: 1.0,
        group_stroke_width: 1.0,
        group_dash_period: 6.0,
        divider_thickness: 1.0,
        edge_thickness: 3.0,
        marker_size: 10.0,
        nub_size: 6.0,
    };

    fn assert_positive_and_finite(metrics: LineworkMetrics) {
        for value in [
            metrics.frame_stroke_scale as f64,
            metrics.group_stroke_width as f64,
            metrics.group_dash_period as f64,
            metrics.divider_thickness,
            metrics.edge_thickness,
            metrics.marker_size,
            metrics.nub_size,
        ] {
            assert!(value.is_finite(), "{value} is not finite");
            assert!(value > 0.0, "{value} is not positive");
        }
    }

    #[test]
    fn cad_mode_is_the_experimental_default() {
        assert_eq!(DEFAULT_LINEWORK_MODE, LineworkMode::Cad);
    }

    #[test]
    fn cad_linework_is_screen_fixed_across_the_supported_zoom_range() {
        for zoom in [MIN_ZOOM, 0.25, 1.0, 4.0, MAX_ZOOM] {
            let metrics = LineworkMetrics::for_zoom(LineworkMode::Cad, zoom);
            assert_eq!(metrics.group_stroke_width, CAD_BASELINE.group_stroke_width);
            assert_eq!(metrics.group_dash_period, CAD_BASELINE.group_dash_period);
            assert_eq!(metrics.divider_thickness, CAD_BASELINE.divider_thickness);
            assert_eq!(metrics.edge_thickness, CAD_BASELINE.edge_thickness);
            assert_eq!(metrics.marker_size, CAD_BASELINE.marker_size);
            assert_eq!(metrics.nub_size, CAD_BASELINE.nub_size);
            assert!((metrics.frame_stroke_scale as f64 * zoom - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn scaled_mode_preserves_the_existing_formulas() {
        assert_eq!(
            LineworkMetrics::for_zoom(LineworkMode::Scaled, 0.25),
            LineworkMetrics {
                frame_stroke_scale: 1.0,
                group_stroke_width: 1.0,
                group_dash_period: 3.0,
                divider_thickness: 1.0,
                edge_thickness: 1.8,
                marker_size: 4.0,
                nub_size: 1.5,
            }
        );
        assert_eq!(
            LineworkMetrics::for_zoom(LineworkMode::Scaled, 2.0),
            LineworkMetrics {
                frame_stroke_scale: 1.0,
                group_stroke_width: 1.0,
                group_dash_period: 12.0,
                divider_thickness: 2.0,
                edge_thickness: 6.0,
                marker_size: 20.0,
                nub_size: 12.0,
            }
        );
        assert_eq!(
            LineworkMetrics::for_zoom(LineworkMode::Scaled, 20.0).group_dash_period,
            18.0
        );
    }

    #[test]
    fn both_modes_match_the_intended_linework_at_one_hundred_percent() {
        let cad = LineworkMetrics::for_zoom(LineworkMode::Cad, 1.0);
        let scaled = LineworkMetrics::for_zoom(LineworkMode::Scaled, 1.0);
        assert_eq!(cad, scaled);
        assert_eq!(cad, CAD_BASELINE);
    }

    #[test]
    fn every_metric_is_positive_and_finite_at_supported_extremes() {
        for mode in [LineworkMode::Cad, LineworkMode::Scaled] {
            assert_positive_and_finite(LineworkMetrics::for_zoom(mode, MIN_ZOOM));
            assert_positive_and_finite(LineworkMetrics::for_zoom(mode, MAX_ZOOM));
        }
    }
}
```

- [ ] **Step 2: Run the focused tests and verify the red state**

Run:

```powershell
rtk cargo test -p waml-editor canvas::class::render::metrics::tests
```

Expected: compilation fails because `LineworkMetrics::for_zoom` does not exist.

- [ ] **Step 3: Implement the minimal mode branch**

Replace `for_zoom` with:

```rust
pub(super) fn for_zoom(mode: LineworkMode, zoom: f64) -> Self {
    debug_assert!(zoom.is_finite() && zoom > 0.0);
    match mode {
        LineworkMode::Cad => Self {
            // AccentFrame still receives camera zoom for shadow and contrast.
            // This multiplier cancels zoom only in its frame inset/stroke.
            frame_stroke_scale: (1.0 / zoom) as f32,
            group_stroke_width: 1.0,
            group_dash_period: 6.0,
            divider_thickness: 1.0,
            edge_thickness: 3.0,
            marker_size: 10.0,
            nub_size: 6.0,
        },
        LineworkMode::Scaled => Self {
            frame_stroke_scale: 1.0,
            // This stroke was already screen-fixed; retain that exact behavior.
            group_stroke_width: 1.0,
            group_dash_period: (6.0 * zoom).clamp(3.0, 18.0) as f32,
            divider_thickness: zoom.max(1.0),
            edge_thickness: (3.0 * zoom).max(1.8),
            marker_size: (10.0 * zoom).max(4.0),
            nub_size: 6.0 * zoom,
        },
    }
}
```

- [ ] **Step 4: Run metric and full render-module tests**

Run:

```powershell
rtk cargo test -p waml-editor canvas::class::render
```

Expected: all render-module tests pass, including the five new metric tests.

- [ ] **Step 5: Commit the metric seam**

```powershell
rtk git add crates/waml-editor/src/canvas/class/render/metrics.rs crates/waml-editor/src/canvas/class/render/mod.rs
rtk git commit -m "refactor(canvas): centralize linework metrics"
```

---

### Task 2: Compute metrics once and apply them to node and group chrome

**Files:**

- Modify: `crates/waml-editor/src/canvas/class/render/mod.rs:15-20`
- Modify: `crates/waml-editor/src/canvas/class/widget.rs:587-643`
- Modify: `crates/waml-editor/src/frame.rs:121-205`
- Modify: `crates/waml-editor/src/canvas/class/render/groups.rs:58-117`
- Modify: `crates/waml-editor/src/canvas/class/render/nodes.rs:38-196`
- Test: existing tests plus `metrics.rs` tests from Task 1

**Interfaces:**

- Consumes: `LineworkMetrics::for_zoom(DEFAULT_LINEWORK_MODE, zoom)`
- Produces: `RenderSnapshot::linework: LineworkMetrics`
- Produces shader uniform: `AccentFrame.stroke_scale: f32`, default `1.0`
- Preserves: `draws.node.zoom = camera.zoom` for shadow and contrast

- [ ] **Step 1: Extend the snapshot and verify the compiler identifies every constructor**

In `render/mod.rs`, re-export the private metrics used by sibling renderer
modules and `widget.rs`, then add the snapshot field:

```rust
pub(super) use metrics::{LineworkMetrics, DEFAULT_LINEWORK_MODE};

pub(super) struct RenderSnapshot<'a> {
    pub(super) scene: &'a Scene,
    pub(super) viewport: ViewportSnapshot,
    pub(super) selection: SelectionSnapshot,
    pub(super) placement: PlacementSnapshot,
    pub(super) linework: LineworkMetrics,
}
```

Run:

```powershell
rtk cargo check -p waml-editor
```

Expected: FAIL at the single `RenderSnapshot` constructor in
`ClassDiagramSurface::draw_walk`, reporting missing field `linework`.

- [ ] **Step 2: Build the metric bundle once per frame**

In `ClassDiagramSurface::draw_walk`, take one viewport snapshot, derive metrics,
then construct the render snapshot:

```rust
let viewport = viewport.snapshot();
let snapshot = RenderSnapshot {
    scene,
    linework: LineworkMetrics::for_zoom(DEFAULT_LINEWORK_MODE, viewport.camera.zoom),
    viewport,
    selection: selection.snapshot(),
    placement: placement.snapshot(),
};
```

Make `LineworkMetrics` and `DEFAULT_LINEWORK_MODE` visible to `widget.rs` through
`render::{...}` rather than reaching into `render::metrics`.

- [ ] **Step 3: Decouple `AccentFrame` stroke sizing from shadow and contrast zoom**

In `frame.rs`, add a defaulted uniform:

```text
stroke_scale: uniform(1.0)
```

Change only the linework expression:

```text
let inset = 1.5 * self.zoom * self.stroke_scale * mix(1.0, 1.5, self.selected)
```

Do not change `z = max(0.35, self.zoom)`, `sblur`, or the contrast calculation
`k = clamp((1.0 - self.zoom) * 2.0, 0.0, 0.85)`. The default multiplier of
`1.0` keeps every non-canvas `AccentFrame` caller byte-for-byte equivalent.

- [ ] **Step 4: Route group borders and dash periods through the snapshot**

In `draw_groups`, delete the local `dash_px` formula. Before each dashed group
draw, push both metric fields:

```rust
draws.group_dashed.set_uniform(
    cx,
    live_id!(dash_px),
    &[snapshot.linework.group_dash_period],
);
draws.group_dashed.set_uniform(
    cx,
    live_id!(stroke_w),
    &[snapshot.linework.group_stroke_width],
);
```

Leave group title size and offsets on `zoom`; those are typography and padding.
Leave `GroupDraw::Chrome` unchanged because `draw_group` is a fill, not a
linework border.

- [ ] **Step 5: Route node frames, dividers, and nubs through the snapshot**

In `draw_nodes`, keep setting `zoom` and also set:

```rust
draws.node.set_uniform(
    cx,
    live_id!(stroke_scale),
    &[snapshot.linework.frame_stroke_scale],
);
```

Change `draw_card` to accept `linework: LineworkMetrics`. Preserve the existing
`zoom` parameter for card width, fills, text positions, and font sizes:

```rust
fn draw_card(
    cx: &mut Cx2d,
    screen: Rect,
    node: &crate::scene::SceneNode,
    zoom: f64,
    linework: LineworkMetrics,
    grey: bool,
    draws: &mut ClassDrawResources<'_>,
)
```

Pass `snapshot.linework` from `draw_nodes`. Replace both divider heights:

```rust
size: dvec2(card_w, linework.divider_thickness),
```

Replace the nub formula:

```rust
let nub = linework.nub_size;
```

Keep `dy * zoom`, `placed.size.1 * 0.5 * zoom`, and `card_w` unchanged so
anchors and geometry still track the camera.

- [ ] **Step 6: Run focused tests and compile the shader**

Run:

```powershell
rtk cargo test -p waml-editor canvas::class::render
rtk cargo check -p waml-editor
```

Expected: both commands pass. `cargo check` is required here because the
Makepad `script_mod!` shader declarations must still parse and bind the new
`stroke_scale` uniform.

- [ ] **Step 7: Commit node/group CAD linework**

```powershell
rtk git add crates/waml-editor/src/canvas/class/render/mod.rs crates/waml-editor/src/canvas/class/widget.rs crates/waml-editor/src/canvas/class/render/groups.rs crates/waml-editor/src/canvas/class/render/nodes.rs crates/waml-editor/src/frame.rs
rtk git commit -m "feat(canvas): fix node linework in screen space"
```

---

### Task 3: Apply fixed metrics to routed edges, elbows, and end markers

**Files:**

- Modify: `crates/waml-editor/src/canvas/class/render/edges.rs:12-195`
- Test: `crates/waml-editor/src/canvas/class/render/metrics.rs`
- Verify existing geometry tests: `crates/waml-editor/src/canvas/geometry.rs`

**Interfaces:**

- Consumes: `snapshot.linework.edge_thickness`
- Consumes: `snapshot.linework.marker_size`
- Preserves: camera zoom uniforms on `EdgeLine` and `EdgeElbow` for color only
- Preserves: existing `segment_quad`, `elbow_radius`, `corner_fillet`, and `marker_geometry` APIs

- [ ] **Step 1: Add an edge-derived-metric regression test**

Add a test in `metrics.rs` proving the dependent elbow and marker stroke values
remain fixed in CAD mode while scaled mode changes:

```rust
#[test]
fn edge_dependents_derive_from_the_mode_specific_stroke() {
    let cad_low = LineworkMetrics::for_zoom(LineworkMode::Cad, 0.25);
    let cad_high = LineworkMetrics::for_zoom(LineworkMode::Cad, 4.0);
    assert_eq!(cad_low.edge_thickness * 2.0, cad_high.edge_thickness * 2.0);
    assert_eq!(cad_low.edge_thickness * 0.5, cad_high.edge_thickness * 0.5);
    assert_eq!(cad_low.marker_size, cad_high.marker_size);

    let scaled_low = LineworkMetrics::for_zoom(LineworkMode::Scaled, 0.25);
    let scaled_high = LineworkMetrics::for_zoom(LineworkMode::Scaled, 4.0);
    assert_ne!(scaled_low.edge_thickness * 2.0, scaled_high.edge_thickness * 2.0);
    assert_ne!(scaled_low.marker_size, scaled_high.marker_size);
}
```

- [ ] **Step 2: Run the focused test before changing edge drawing**

Run:

```powershell
rtk cargo test -p waml-editor edge_dependents_derive_from_the_mode_specific_stroke
```

Expected: PASS. This is a characterization gate for the metric interface that
the edge renderer will now consume.

- [ ] **Step 3: Replace local edge size formulas with snapshot metrics**

At the top of `draw_edges`, retain camera zoom for the shader color behavior,
but replace the linework formulas:

```rust
let zoom = camera.zoom;
let thickness = snapshot.linework.edge_thickness;
let marker_size = snapshot.linework.marker_size;
draws.edge.set_uniform(cx, live_id!(zoom), &[zoom as f32]);
draws.elbow.set_uniform(cx, live_id!(zoom), &[zoom as f32]);
let r_base = thickness * 2.0;
```

Leave the rest of the function structurally unchanged:

- `segment_quad(..., thickness)` fixes the routed bar width;
- `r_base = thickness * 2.0` fixes elbow radius while retaining short-segment
  clamping inside `elbow_radius`;
- `marker_geometry(..., marker_size)` fixes arrowhead and diamond geometry;
- `stroke_w = thickness * 0.5` fixes hollow/open marker outline width;
- endpoint positions and segment lengths still come from camera-projected world
  points.

Do not change `EdgeLine.zoom` or `EdgeElbow.zoom`; those uniforms deepen color
at low zoom and do not size geometry.

- [ ] **Step 4: Run edge, geometry, and render tests**

Run:

```powershell
rtk cargo test -p waml-editor canvas::geometry
rtk cargo test -p waml-editor canvas::class::render
```

Expected: all existing segment, fillet, marker, dash-mask, label, pass-order,
and new metric tests pass.

- [ ] **Step 5: Commit fixed edge linework**

```powershell
rtk git add crates/waml-editor/src/canvas/class/render/edges.rs crates/waml-editor/src/canvas/class/render/metrics.rs
rtk git commit -m "feat(canvas): fix edge adornments in screen space"
```

---

### Task 4: Full verification and native multi-zoom visual review

**Files:**

- Verify only; do not commit generated screenshots
- Fixtures:
  - `crates/waml-editor/tests/fixtures/sixkind`
  - `crates/waml-editor/tests/fixtures/groups`

**Interfaces:**

- Consumes the completed CAD-default renderer
- Produces test, lint, and native visual evidence

- [ ] **Step 1: Run formatting and inspect the diff**

Run:

```powershell
rtk cargo fmt --all -- --check
rtk git diff --check
rtk git diff main...HEAD --stat
```

Expected: formatting and whitespace checks pass; the stat contains only the
planned renderer, frame, test, spec, and plan files.

- [ ] **Step 2: Run the complete editor test suite**

Run:

```powershell
rtk cargo test -p waml-editor
```

Expected: all `waml-editor` tests pass.

- [ ] **Step 3: Run strict linting**

Run:

```powershell
rtk cargo clippy -p waml-editor --all-targets -- -D warnings
```

Expected: zero warnings and zero errors.

- [ ] **Step 4: Launch the marker-rich fixture**

Run in the feature worktree:

```powershell
rtk cargo run -p waml-editor -- crates/waml-editor/tests/fixtures/sixkind
```

Use the editor zoom controls to inspect approximately 25%, 100%, and 400%.
Confirm:

- node bounds and all fonts scale;
- node border, dividers, routed bars, elbow weight, arrows, triangles, and
  diamonds retain the same apparent screen-pixel size;
- marker tips stay anchored to route endpoints;
- shadows and low-zoom contrast still respond to camera zoom;
- role/cardinality labels scale because they are text.

- [ ] **Step 5: Capture the running editor at each zoom**

At each zoom, run from another shell:

```powershell
rtk proxy pwsh -File scripts/capture-window.ps1 -Out C:\tmp\cad-linework-25.png -Process waml-editor
rtk proxy pwsh -File scripts/capture-window.ps1 -Out C:\tmp\cad-linework-100.png -Process waml-editor
rtk proxy pwsh -File scripts/capture-window.ps1 -Out C:\tmp\cad-linework-400.png -Process waml-editor
```

Expected: three native-pixel, HiDPI-correct captures. Compare line weights and
marker dimensions directly; do not add these temporary PNGs to Git.

- [ ] **Step 6: Verify hidden-group dashes**

Launch:

```powershell
rtk cargo run -p waml-editor -- crates/waml-editor/tests/fixtures/groups
```

Enable hidden borders/x-ray, compare approximately 25%, 100%, and 400%, and
confirm both the 1 px outline and 6 px dash period remain screen-fixed while
the group rectangle and title scale.

- [ ] **Step 7: Confirm the branch is clean**

Run:

```powershell
rtk git status --short --branch
rtk git log --oneline main..HEAD
```

Expected: clean `diagram-render-tweak`; commits consist of the design/plan and
the three focused implementation commits. If `cargo fmt --check` required a
formatting edit, apply `rtk cargo fmt --all`, rerun Steps 1–3, and amend the
specific implementation commit that introduced the formatting error rather
than creating an unrelated cleanup commit.
