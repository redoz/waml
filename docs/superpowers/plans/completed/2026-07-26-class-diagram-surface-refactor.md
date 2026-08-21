# Class Diagram Surface Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve the native editor's class-diagram behavior while renaming `GraphCanvas` to `ClassDiagramSurface` and giving viewport, selection, placement, geometry, and rendering one explicit owner each.

**Architecture:** Keep `ClassDiagramSurface` as the only Makepad widget and caller-facing façade. Move diagram-kind-independent camera and geometry mechanics into sibling canvas modules, keep all `Scene` knowledge under `canvas/class/`, and make the class controllers return typed effects which the widget translates into Makepad timers, redraws, and `ClassDiagramSurfaceAction`.

**Tech Stack:** Rust 2021 (MSRV 1.80), Makepad widgets/script DSL, WAML `Scene` projection and solver geometry, inline Rust unit tests in the binary-only `waml-editor` crate, native Windows visual verification.

## Global Constraints

- Preserve current visible behavior; this is a structural refactor, not a visual redesign.
- Preserve gesture, timer, pointer-capture, keyboard, event-priority, and draw-pass ordering.
- Rename `GraphCanvas` to `ClassDiagramSurface` and `GraphCanvasAction` to `ClassDiagramSurfaceAction`; do not add compatibility aliases for either old name.
- Keep the live widget ID `canvas`; do not rename it to `diagram_surface`.
- Keep one Makepad-facing surface façade; callers must not borrow controllers or render-pass types.
- Keep `crate::scene::Scene` in `scene.rs`, but contain every surface use of it under `canvas/class/`.
- `ViewportController` is the only writer of camera state and must contain no `SceneNode`, card, placement, constraint, selection, activity, or sequence semantics.
- `PlacementInteraction` is the only writer of drag-to-place, dwell, compass, candidate-layout, conflict-zone, preview, and return-animation state.
- `SelectionState` is the only writer of selected-node, constraint-visibility, conflict-focus, and class-specific hidden-border state.
- Rendering receives read-only controller snapshots and must not mutate controllers, arm timers, emit actions, or mutate WAML.
- Controllers may return typed effects, but may not call `Cx::widget_action`, open popups, mutate the WAML model, or call `App`.
- A scene replacement must stop or invalidate every timer and animation that refers to the old scene; stale keys clear transient state instead of indexing unchecked.
- Do not create `DiagramCanvas`/`DiagramSurface` traits, a generic scene enum, a shared interaction controller, or activity/sequence renderers.
- Shared canvas code may contain proven viewport and framework-light geometry only; it must not contain class-card, placement-constraint, activity, or sequence vocabulary.
- `waml-editor` is binary-only: keep tests inline under `#[cfg(test)]`; do not add a `--lib` test target.
- This repository is developed on Windows PowerShell; prefix every shell command in this plan with `rtk`.
- Do not change WAML syntax, parsing, model operations, solving, persistence, `DocView`, or the application shell's document-mutation authority.

---

## File Structure

The completed refactor has these ownership boundaries:

```text
crates/waml-editor/src/
├── main.rs                              # declares `mod canvas`; no root camera module
├── app.rs                               # registers/mounts/borrows ClassDiagramSurface
├── class_diagram_view.rs                # translates surface actions to view/shell intent
├── classifier_preview_view.rs           # classifier-focus façade consumer
├── doc_tabs.rs                          # terminology-only comment update
├── inspector_panel.rs                   # terminology-only comment update
├── logo.rs                              # terminology-only comment update
├── view_bar.rs                          # terminology-only comment update
├── scene.rs                             # unchanged class projection authority
└── canvas/
    ├── mod.rs                           # private module declarations, live registration, narrow re-exports
    ├── viewport.rs                      # Camera + ViewportController + camera/pan/pinch/fit/tween tests
    ├── geometry.rs                      # shared pure rect/segment/snap/fillet/marker geometry
    └── class/
        ├── mod.rs                       # class-only declarations and façade re-exports
        ├── widget.rs                    # Makepad widget, draw fields, timers, façade, controller coordination
        ├── interaction.rs               # explicit class pointer/key priority and hit interpretation
        ├── placement.rs                 # drag/dwell/dial/candidate/preview state machine
        ├── selection.rs                 # keyed selection/focus visibility + reconciliation
        └── render/
            ├── mod.rs                   # immutable render inputs and the canonical pass order
            ├── groups.rs                # group chrome and labels
            ├── edges.rs                 # routed edges, fillets, and terminal markers
            ├── nodes.rs                 # classifier frames/cards/footer rendering
            ├── relations.rs             # persistent relations and constraint veils
            ├── overlays.rs              # conflict focus and drag-place overlays
            └── primitives.rs            # borrowed Makepad pens and class-only draw helpers
```

Delete `crates/waml-editor/src/camera.rs` by moving its contents into `canvas/viewport.rs`, and delete the old `crates/waml-editor/src/canvas.rs` by moving it into the directory module in Task 1. Do not relocate `scene.rs`: card, inspector, popup, and preview code still consume that projection.

The public façade remains source-compatible except for the approved type/action rename. Preserve these methods and values on `ClassDiagramSurface`:

```rust
pub fn set_scene(&mut self, cx: &mut Cx, scene: Scene);
pub fn set_focus(&mut self, cx: &mut Cx, scene: Scene);
pub fn update_scene(&mut self, cx: &mut Cx, scene: Scene);
pub fn select_by_key(&mut self, cx: &mut Cx, key: &str);
pub fn context_items(&self, subject: &Subject) -> Vec<PopupItem>;
pub fn set_zone_layouts(
    &mut self,
    cx: &mut Cx,
    layouts: Vec<(Zone, BTreeMap<String, waml::solve::Rect>)>,
);
pub fn preview_zone(&mut self, cx: &mut Cx, zone: Option<Zone>);
pub fn placement_for(&self, zone: Zone) -> Option<DialPlacement>;
pub fn set_conflict_zones(&mut self, cx: &mut Cx, zones: Vec<Zone>);
pub fn conflict_count(&self) -> usize;
pub fn conflicts(&self) -> Vec<SceneConflict>;
pub fn set_conflict_focus_keys(&mut self, cx: &mut Cx, keys: Option<Vec<String>>);
pub fn set_constraint_vis(&mut self, cx: &mut Cx, mode: ConstraintVisibility);
pub fn zoom_step(&mut self, cx: &mut Cx, factor: f64);
pub fn fit_to_scene(&mut self, cx: &mut Cx);
pub fn fit_to_selection(&mut self, cx: &mut Cx);
pub fn has_selection(&self) -> bool;
pub fn set_show_hidden_borders(&mut self, cx: &mut Cx, on: bool);
pub fn constraint_vis(&self) -> ConstraintVisibility;
pub fn show_hidden_borders(&self) -> bool;
pub fn node_count(&self) -> usize;
pub fn surface_action(&self, actions: &Actions) -> Option<ClassDiagramSurfaceAction>;
pub fn zoom_pct(&self) -> i32;
```

`set_focus` keeps its current two-argument public signature. The internal `SceneUpdate::Focus { key }` key is derived from the focus scene's first node, which is the same classifier `build_focus_scene` projected. A missing/empty focus scene produces no focus key and no unchecked index.

### Task 1: Characterize, Rename, and Establish the Directory Module

**Files:**
- Move: `crates/waml-editor/src/canvas.rs` -> `crates/waml-editor/src/canvas/class/widget.rs`
- Create: `crates/waml-editor/src/canvas/mod.rs`
- Create: `crates/waml-editor/src/canvas/class/mod.rs`
- Modify: `crates/waml-editor/src/app.rs`
- Modify: `crates/waml-editor/src/class_diagram_view.rs`
- Modify: `crates/waml-editor/src/classifier_preview_view.rs`
- Modify: `crates/waml-editor/src/doc_tabs.rs`
- Modify: `crates/waml-editor/src/inspector_panel.rs`
- Modify: `crates/waml-editor/src/logo.rs`
- Modify: `crates/waml-editor/src/view_bar.rs`
- Modify: `crates/waml-editor/tests/README.md`
- Test: inline tests moved with `crates/waml-editor/src/canvas/class/widget.rs`

**Interfaces:**
- Consumes: existing `crate::camera::Camera`, `crate::scene::Scene`, Makepad `ScriptVm`, `Widget`, timers, actions, and all current `GraphCanvas` façade behavior.
- Produces: `crate::canvas::{ClassDiagramSurface, ClassDiagramSurfaceAction, ConstraintVisibility, DialPlacement, Placed, Zone, COMPASS_ZONES, DIAL_ZONES, ZOOM_STEP, zone_arrow, zone_id, zone_of_id, zone_placed}` and `crate::canvas::script_mod(&mut ScriptVm) -> ScriptValue`.

- [ ] **Step 1: Run the pre-refactor unit baseline**

Run:

```powershell
rtk cargo test -p waml-editor
```

Expected: PASS for the binary unit-test harness. Record the test count in the task notes; do not proceed with an unexplained pre-existing failure.

- [ ] **Step 2: Capture native visual baselines before moving code**

From the worktree root, run this sequence in a PowerShell terminal. It builds
into this worktree's explicit `target` directory, starts one visible editor at
a time with `Start-Process -PassThru`, waits for that exact process to own a
window, pauses for the requested interaction, captures by returned PID, and
stops only that PID in `finally` before starting the next fixture:

```powershell
rtk proxy pwsh -NoProfile -Command @'
$ErrorActionPreference = "Stop"
$classSurfaceWorktree = (Resolve-Path ".").Path
$classSurfaceTarget = Join-Path $classSurfaceWorktree "target"
$classSurfaceExe = Join-Path $classSurfaceTarget "debug\waml-editor.exe"
$classSurfaceCaptureScript = Join-Path $classSurfaceWorktree "scripts\capture-window.ps1"
$classSurfacePhase = "before"

& rtk cargo build -p waml-editor --target-dir $classSurfaceTarget
if ($LASTEXITCODE -ne 0) {
    throw "waml-editor build failed with exit code $LASTEXITCODE"
}
if (-not (Test-Path -LiteralPath $classSurfaceExe)) {
    throw "worktree executable not found at $classSurfaceExe"
}

function Invoke-ClassSurfaceFixtureCapture {
    param(
        [Parameter(Mandatory = $true)][string]$ClassSurfaceFixture,
        [Parameter(Mandatory = $true)][object[]]$ClassSurfaceCaptures
    )

    $classSurfaceFixturePath =
        (Resolve-Path (Join-Path $classSurfaceWorktree $ClassSurfaceFixture)).Path
    $classSurfaceFixtureProcess = Start-Process `
        -FilePath $classSurfaceExe `
        -ArgumentList @($classSurfaceFixturePath) `
        -WorkingDirectory $classSurfaceWorktree `
        -WindowStyle Normal `
        -PassThru
    try {
        $classSurfaceWindowDeadline = (Get-Date).AddSeconds(30)
        do {
            Start-Sleep -Milliseconds 200
            $classSurfaceFixtureProcess.Refresh()
            if ($classSurfaceFixtureProcess.HasExited) {
                throw "editor pid=$($classSurfaceFixtureProcess.Id) exited before opening a window"
            }
        } while (
            $classSurfaceFixtureProcess.MainWindowHandle -eq 0 -and
            (Get-Date) -lt $classSurfaceWindowDeadline
        )
        if ($classSurfaceFixtureProcess.MainWindowHandle -eq 0) {
            throw "editor pid=$($classSurfaceFixtureProcess.Id) opened no window within 30 seconds"
        }

        foreach ($classSurfaceCapture in $ClassSurfaceCaptures) {
            $null = Read-Host $classSurfaceCapture.Prompt
            $classSurfaceCaptureOut = "C:\tmp\class-surface-$classSurfacePhase-$($classSurfaceCapture.Name).png"
            & rtk pwsh -File $classSurfaceCaptureScript `
                -Out $classSurfaceCaptureOut `
                -ProcessId $classSurfaceFixtureProcess.Id
            if ($LASTEXITCODE -ne 0) {
                throw "capture failed for pid=$($classSurfaceFixtureProcess.Id)"
            }
        }
    }
    finally {
        $classSurfaceFixtureProcess.Refresh()
        if (-not $classSurfaceFixtureProcess.HasExited) {
            Stop-Process -Id $classSurfaceFixtureProcess.Id -ErrorAction SilentlyContinue
            Wait-Process -Id $classSurfaceFixtureProcess.Id -Timeout 10 -ErrorAction SilentlyContinue
        }
    }
}

Invoke-ClassSurfaceFixtureCapture `
    -ClassSurfaceFixture "crates/waml-editor/tests/fixtures/mini" `
    -ClassSurfaceCaptures @(
        @{
            Name = "mini"
            Prompt = "Open Orders, fit the overview, then press Enter to capture"
        }
    )
Invoke-ClassSurfaceFixtureCapture `
    -ClassSurfaceFixture "crates/waml-editor/tests/fixtures/groups" `
    -ClassSurfaceCaptures @(
        @{
            Name = "groups"
            Prompt = "Open the groups diagram with hidden borders OFF, then press Enter"
        },
        @{
            Name = "groups-hidden"
            Prompt = "Enable hidden borders without moving the camera, then press Enter"
        }
    )
Invoke-ClassSurfaceFixtureCapture `
    -ClassSurfaceFixture "crates/waml-editor/tests/fixtures/sixkind" `
    -ClassSurfaceCaptures @(
        @{
            Name = "sixkind-overview"
            Prompt = "Open the sixkind overview and fit the scene, then press Enter"
        },
        @{
            Name = "sixkind-zoomed-out"
            Prompt = "Zoom out to the small-font raster level, then press Enter"
        },
        @{
            Name = "sixkind-zoomed-in"
            Prompt = "Zoom in to the large-font raster level, then press Enter"
        },
        @{
            Name = "sixkind-focus"
            Prompt = "Select a large classifier, expand its compartment, fit selection, then press Enter"
        }
    )
'@
```

Expected: seven native-resolution `class-surface-before-*.png` files in
`C:\tmp`. Each comes from the PID returned for this worktree executable, and no
other editor process is stopped. These files are verification artifacts and
are not committed.

- [ ] **Step 3: Move the canvas into the approved class subtree**

Use `apply_patch` to create the directory module files; adding the nested files
creates the destination directories without a shell filesystem command:

```text
*** Begin Patch
*** Add File: crates/waml-editor/src/canvas/class/mod.rs
+mod widget;
+
+pub(crate) use widget::{
+    script_mod, zone_arrow, zone_id, zone_of_id, zone_placed, ClassDiagramSurface,
+    ClassDiagramSurfaceAction, ConstraintVisibility, DialPlacement, Placed, Zone, COMPASS_ZONES,
+    DIAL_ZONES, ZOOM_STEP,
+};
*** Add File: crates/waml-editor/src/canvas/mod.rs
+mod class;
+
+pub(crate) use class::{
+    zone_arrow, zone_id, zone_of_id, zone_placed, ClassDiagramSurface,
+    ClassDiagramSurfaceAction, ConstraintVisibility, DialPlacement, Placed, Zone, COMPASS_ZONES,
+    DIAL_ZONES, ZOOM_STEP,
+};
+pub(crate) use class::script_mod;
*** End Patch
```

Then use an `apply_patch` move hunk. The documentation-line edit gives the move
an explicit changed hunk; the rest of the 4,345-line file moves unchanged:

```text
*** Begin Patch
*** Update File: crates/waml-editor/src/canvas.rs
*** Move to: crates/waml-editor/src/canvas/class/widget.rs
@@
-//! The `GraphCanvas` widget: draws the flattened `Scene` under a pan/zoom
+//! The `ClassDiagramSurface` widget: draws the flattened `Scene` under a pan/zoom
*** End Patch
```

Do not declare `geometry` or `viewport` until their files exist in later tasks.

- [ ] **Step 4: Rename the Rust widget, action, DSL registration, and action reader**

In `canvas/class/widget.rs`, make these exact renames:

```rust
mod.widgets.ClassDiagramSurfaceBase =
    #(ClassDiagramSurface::register_widget(vm))

mod.widgets.ClassDiagramSurface =
    set_type_default() do mod.widgets.ClassDiagramSurfaceBase{
        width: Fill
        height: Fill
    }
```

Preserve every existing draw-field override inside the second block; only the two DSL type names change. Rename the Rust declarations and implementations:

```rust
#[derive(Script, ScriptHook, Widget)]
pub struct ClassDiagramSurface {
```

The line above is the new declaration header; retain every concrete field and
attribute currently between the old declaration braces. Rename the action and
implementation headers exactly:

```rust
#[derive(Clone, Debug, Default)]
pub enum ClassDiagramSurfaceAction {
    #[default]
    None,
    NodeMenu { abs: DVec2, key: String },
    NodeSelect { key: String },
    NodeDeselect,
    ToggleExpand { key: String },
    CompassArmed {
        subject_key: String,
        reference_key: String,
        center: DVec2,
    },
    DialDismiss,
}
```

Replace `impl Widget for GraphCanvas {` with this header and keep its current
methods inside the block:

```rust
impl Widget for ClassDiagramSurface {
```

Rename the inherent implementation and action reader:

```rust
impl ClassDiagramSurface {
    pub fn surface_action(
        &self,
        actions: &Actions,
    ) -> Option<ClassDiagramSurfaceAction> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.cast() {
            ClassDiagramSurfaceAction::None => None,
            action => Some(action),
        }
    }
}
```

Keep the existing `handle_event` and `draw_walk` bodies byte-for-byte in this
task except for renamed action constructors.

- [ ] **Step 5: Migrate all Makepad and Rust callers in one compile-breaking rename**

In `app.rs`, change the live import and mount while preserving the widget ID:

```rust
use mod.widgets.ClassDiagramSurface

canvas := ClassDiagramSurface{
    width: Fill
    height: Fill
}
```

Replace every `borrow::<crate::canvas::GraphCanvas>()` and `borrow_mut::<crate::canvas::GraphCanvas>()` in `app.rs`, `class_diagram_view.rs`, and `classifier_preview_view.rs` with the corresponding `ClassDiagramSurface` type. Replace every action pattern with `ClassDiagramSurfaceAction`, and replace `.canvas_action(actions)` with `.surface_action(actions)`.

Update comments in the listed Rust files and `tests/README.md` to say “class diagram surface” or `ClassDiagramSurface` where they currently present the widget as a generic graph canvas. Do not rename the UI ID `canvas`, `BodyWidgets::canvas`, local variables named `canvas`, or user-facing “canvas” prose.

- [ ] **Step 6: Verify the rename and module registration**

Run:

```powershell
rtk cargo fmt --check
rtk cargo test -p waml-editor
rtk rg -n "GraphCanvas|GraphCanvasAction|mod\.widgets\.GraphCanvas" crates/waml-editor/src crates/waml-editor/tests
```

Expected: formatting and tests PASS; the final search exits with no matches. Confirm `AppMain::script_mod` still calls `crate::canvas::script_mod(vm)` after popup registration and before `icon_button`, preserving the eager Makepad registration order.

- [ ] **Step 7: Commit the independently buildable rename**

```powershell
rtk git add crates/waml-editor/src crates/waml-editor/tests/README.md
rtk git commit -m "refactor(editor): rename class diagram surface"
```

### Task 2: Extract Proven Shared Geometry

**Files:**
- Create: `crates/waml-editor/src/canvas/geometry.rs`
- Modify: `crates/waml-editor/src/canvas/mod.rs`
- Modify: `crates/waml-editor/src/canvas/class/widget.rs`
- Test: inline `canvas::geometry::tests`
- Test: inline class-only geometry tests retained in `canvas::class::widget::tests`

**Interfaces:**
- Consumes: Makepad `DVec2`/`Rect` value types and `waml::adornment::Marker`; it consumes no `Scene`, `SceneNode`, controller, or Makepad draw object.
- Produces: `intersect_rect(Rect, Rect) -> Rect`, `segment_quad(DVec2, DVec2, f64) -> Rect`, `snap_bar_to_device(Rect, f64) -> Rect`, `elbow_radius(DVec2, DVec2, DVec2, f64) -> f64`, `corner_fillet(DVec2, DVec2, DVec2, Rect, Rect, f64) -> Option<CornerFillet>`, and `marker_geometry(Marker, DVec2, DVec2, f64) -> Option<MarkerGeometry>`.

- [ ] **Step 1: Add failing geometry-module tests before moving implementation**

Declare `mod geometry;` in `canvas/mod.rs`. Create `geometry.rs` with imports and these tests first:

```rust
use makepad_widgets::{dvec2, DVec2, Rect};
use waml::adornment::Marker;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_quad_centres_a_horizontal_bar() {
        let quad = segment_quad(dvec2(10.0, 20.0), dvec2(50.0, 20.0), 4.0);
        assert_eq!(quad.pos, dvec2(10.0, 18.0));
        assert_eq!(quad.size, dvec2(40.0, 4.0));
    }

    #[test]
    fn marker_none_has_no_geometry() {
        assert!(marker_geometry(
            Marker::None,
            dvec2(20.0, 30.0),
            dvec2(1.0, 0.0),
            10.0,
        )
        .is_none());
    }

    #[test]
    fn snapping_respects_hidpi_device_pixels() {
        let snapped = snap_bar_to_device(
            Rect {
                pos: dvec2(10.3, 20.3),
                size: dvec2(0.4, 12.2),
            },
            2.0,
        );
        assert_eq!(snapped.pos, dvec2(10.5, 20.5));
        assert_eq!(snapped.size, dvec2(0.5, 12.0));
    }
}
```

- [ ] **Step 2: Run the new tests to verify the module is red**

Run:

```powershell
rtk cargo test -p waml-editor canvas::geometry::tests
```

Expected: FAIL because `segment_quad`, `marker_geometry`, and `snap_bar_to_device` are not defined in `geometry.rs`.

- [ ] **Step 3: Move the shared geometry and expose only draw-ready values**

Move the existing bodies of `intersect_rect`, `segment_quad`, `snap_bar_to_device`, `elbow_radius`, `corner_fillet`, and `marker_geometry` unchanged into `geometry.rs`. Rename only `MarkerDraw` to `MarkerGeometry`, and use these exact visibility boundaries:

```rust
pub(crate) const ELBOW_MIN_DEVICE_PX: f64 = 6.0;

pub(crate) struct CornerFillet {
    pub(crate) quad: Rect,
    pub(crate) bar_in: [f32; 4],
    pub(crate) bar_out: [f32; 4],
    pub(crate) gate: [f32; 4],
    pub(crate) center: DVec2,
    pub(crate) radius: f64,
    pub(crate) hw: f64,
}

pub(crate) struct MarkerGeometry {
    pub(crate) quad: Rect,
    pub(crate) v01: [f32; 4],
    pub(crate) v23: [f32; 4],
    pub(crate) hollow: f32,
    pub(crate) filled: f32,
}
```

Keep `CORNER_STUB_OVERLAP` and `CORNER_STUB_SEAL` private. Import the helpers in `class/widget.rs`:

```rust
use crate::canvas::geometry::{
    corner_fillet, elbow_radius, intersect_rect, marker_geometry, segment_quad,
    snap_bar_to_device, ELBOW_MIN_DEVICE_PX,
};
```

Keep `node_at`, `footer_screen_rect`, veil geometry, compass zones, click slop, `lerp_rect`, `preview_zoom`, and `edge_point_to_screen` under `canvas/class/`; each contains class interaction/render or viewport policy.

- [ ] **Step 4: Move the existing geometry characterization tests**

Move the existing tests for segment quads, elbow radii, fillet joins, marker tips/flags, and device-grid snapping from `widget.rs` into `geometry.rs`. Preserve their assertions and fixture values. Leave the dash-mask shader-idiom test in the class render code because it characterizes a class Makepad primitive rather than framework-light geometry.

- [ ] **Step 5: Run the focused and crate tests**

Run:

```powershell
rtk cargo fmt --check
rtk cargo test -p waml-editor canvas::geometry::tests
rtk cargo test -p waml-editor
```

Expected: all PASS, with no duplicated definitions remaining in `widget.rs`.

- [ ] **Step 6: Commit shared geometry**

```powershell
rtk git add crates/waml-editor/src/canvas
rtk git commit -m "refactor(editor): extract canvas geometry"
```

### Task 3: Move Camera and Viewport State Under `ViewportController`

**Files:**
- Move: `crates/waml-editor/src/camera.rs` -> `crates/waml-editor/src/canvas/viewport.rs`
- Modify: `crates/waml-editor/src/main.rs`
- Modify: `crates/waml-editor/src/canvas/mod.rs`
- Modify: `crates/waml-editor/src/canvas/class/widget.rs`
- Test: inline `canvas::viewport::tests`

**Interfaces:**
- Consumes: world bounds as `waml::solve::Rect`, Makepad value-only `DVec2`/`Rect`, touch samples as stable IDs/positions, and timer timestamps supplied by the widget.
- Produces: `Camera`, `ViewportController`, `ViewportSnapshot`, `InitialFit`, `ViewportEffects`, `TimerCommand`, `ZOOM_STEP`, and read-only world/local transformations. No method accepts `Scene`, `SceneNode`, `SelectionState`, `PlacementInteraction`, or class actions.

- [ ] **Step 1: Move `camera.rs` and make the old import fail**

Move the file with `apply_patch`, updating its module description in the same
hunk:

```text
*** Begin Patch
*** Update File: crates/waml-editor/src/camera.rs
*** Move to: crates/waml-editor/src/canvas/viewport.rs
@@
-//! Pan/zoom camera. Pure math — no makepad types. `local` coordinates are
+//! Shared viewport and pan/zoom mechanics. `local` coordinates are
*** End Patch
```

Use `apply_patch` for the module declarations and re-export:

```text
*** Begin Patch
*** Update File: crates/waml-editor/src/main.rs
@@
-mod camera;
 mod canvas;
*** Update File: crates/waml-editor/src/canvas/mod.rs
@@
 mod class;
+mod viewport;
@@
 pub(crate) use class::script_mod;
+pub(crate) use viewport::ZOOM_STEP;
*** End Patch
```

Remove `ZOOM_STEP` from the temporary `class`/`widget` re-export lists created
in Task 1; callers continue to use the unchanged `crate::canvas::ZOOM_STEP`
path.

Run:

```powershell
rtk cargo test -p waml-editor
```

Expected: FAIL at `use crate::camera::Camera` and remaining `crate::camera::{Camera, MIN_ZOOM}` references. This proves every old root dependency is visible before the controller migration.

- [ ] **Step 2: Add failing viewport transition tests**

Append tests in `viewport.rs` for controller ownership:

```rust
#[test]
fn pan_is_owned_by_the_viewport() {
    let mut viewport = ViewportController::default();
    viewport.set_view_rect(Rect {
        pos: dvec2(100.0, 50.0),
        size: dvec2(800.0, 600.0),
    });
    viewport.begin_pan(dvec2(300.0, 200.0));
    viewport.pan_to(dvec2(360.0, 230.0));
    assert_eq!(viewport.camera().pan_x, -60.0);
    assert_eq!(viewport.camera().pan_y, -30.0);
}

#[test]
fn camera_tick_lands_exactly_on_target_and_stops() {
    let mut viewport = ViewportController::default();
    viewport.set_view_rect(Rect {
        pos: dvec2(0.0, 0.0),
        size: dvec2(800.0, 600.0),
    });
    let target = Camera { pan_x: 20.0, pan_y: 30.0, zoom: 2.0 };
    assert_eq!(
        viewport.glide_to(target).camera_timer,
        TimerCommand::StartInterval(CAMERA_TICK),
    );
    viewport.tick_camera(10.0);
    let effects = viewport.tick_camera(10.0 + CAMERA_SECS);
    assert_eq!(viewport.camera(), target);
    assert_eq!(effects.camera_timer, TimerCommand::Stop);
}

#[test]
fn pinch_rejects_degenerate_spread_and_keeps_the_fixed_point() {
    assert_eq!(pinch_factor(4.0, 8.0), None);
    let mut viewport = ViewportController::default();
    viewport.set_view_rect(Rect {
        pos: dvec2(50.0, 25.0),
        size: dvec2(800.0, 600.0),
    });
    let before = viewport.camera().local_to_world(400.0, 300.0);
    viewport.apply_pinch_sample(
        TouchPair { a: 1, b: 2, spread: 100.0, midpoint_abs: dvec2(450.0, 325.0) },
    );
    viewport.apply_pinch_sample(
        TouchPair { a: 1, b: 2, spread: 150.0, midpoint_abs: dvec2(450.0, 325.0) },
    );
    let after = viewport.camera().local_to_world(400.0, 300.0);
    approx(before, after);
}
```

- [ ] **Step 3: Implement the viewport types and move the existing pure math**

Retain the existing `Camera`, `MIN_ZOOM`, `MAX_ZOOM`, `Camera::fit`, round-trip, zoom-at-point, and clamp code. Add these exact controller types:

```rust
pub(crate) const FIT_PAD: f64 = 48.0;
pub(crate) const ZOOM_STEP: f64 = 1.2;
pub(crate) const CAMERA_SECS: f64 = 0.22;
pub(crate) const CAMERA_TICK: f64 = 1.0 / 144.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum InitialFit {
    None,
    Scene(waml::solve::Rect),
    Focus(waml::solve::Rect),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TimerCommand {
    Keep,
    StartInterval(f64),
    Stop,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ViewportEffects {
    pub(crate) redraw: bool,
    pub(crate) camera_timer: TimerCommand,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ViewportSnapshot {
    pub(crate) camera: Camera,
    pub(crate) view_rect: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TouchPair {
    pub(crate) a: u64,
    pub(crate) b: u64,
    pub(crate) spread: f64,
    pub(crate) midpoint_abs: DVec2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PanOrigin {
    down_abs: DVec2,
    pan_x: f64,
    pan_y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CameraTween {
    from: Camera,
    to: Camera,
    t: f64,
}

pub(crate) struct ViewportController {
    camera: Camera,
    view_rect: Rect,
    initial_fit: InitialFit,
    pan: Option<PanOrigin>,
    pinch: Option<TouchPair>,
    tween: Option<CameraTween>,
    tween_last_time: f64,
}
```

Move the existing `ease_out`, `lerp_camera`, `fit_scene_camera`, `pinch_factor`, camera target/retarget, and camera tests into this module.

Implement `Default` with `Camera::default()`, `Rect::default()`,
`InitialFit::None`, empty pan/pinch/tween state, and zero
`tween_last_time`; Makepad's `#[rust]` field initialization depends on it.

Implement these exact methods:

```rust
impl ViewportController {
    pub(crate) fn camera(&self) -> Camera;
    pub(crate) fn snapshot(&self) -> ViewportSnapshot;
    pub(crate) fn set_view_rect(&mut self, rect: Rect);
    pub(crate) fn request_initial_fit(&mut self, fit: InitialFit);
    pub(crate) fn apply_initial_fit(&mut self) -> bool;
    pub(crate) fn retain_for_scene_update(&mut self);
    pub(crate) fn begin_pan(&mut self, abs: DVec2);
    pub(crate) fn pan_to(&mut self, abs: DVec2) -> bool;
    pub(crate) fn end_pan(&mut self);
    pub(crate) fn apply_scroll_zoom(&mut self, abs: DVec2, factor: f64) -> ViewportEffects;
    pub(crate) fn apply_pinch_sample(&mut self, sample: TouchPair) -> ViewportEffects;
    pub(crate) fn end_pinch(&mut self) -> bool;
    pub(crate) fn zoom_step(&mut self, factor: f64) -> ViewportEffects;
    pub(crate) fn fit_to_bounds(&mut self, bounds: Option<waml::solve::Rect>) -> ViewportEffects;
    pub(crate) fn glide_to(&mut self, target: Camera) -> ViewportEffects;
    pub(crate) fn cancel_glide(&mut self) -> ViewportEffects;
    pub(crate) fn tick_camera(&mut self, now: f64) -> ViewportEffects;
    pub(crate) fn set_transient_camera(&mut self, camera: Camera);
}
```

`set_transient_camera` is the sole entry used later by placement preview; it keeps `ViewportController` the camera writer while allowing a class controller to request a calculated camera.

- [ ] **Step 4: Replace widget camera fields with one controller**

In `ClassDiagramSurface`, replace `camera`, `fitted`, `focus_mode`, `view_rect`, `drag_start_pan`, `pinch`, `cam_tween`, and `cam_last_time` with:

```rust
#[rust]
viewport: ViewportController,
#[rust]
cam_timer: Timer,
```

Keep the Makepad `Timer` handle in the widget because only the framework adapter owns `Cx`. Translate `ViewportEffects::camera_timer` in one helper:

```rust
fn apply_viewport_effects(&mut self, cx: &mut Cx, effects: ViewportEffects) {
    match effects.camera_timer {
        TimerCommand::Keep => {}
        TimerCommand::StartInterval(seconds) => {
            self.cam_timer = cx.start_interval(seconds);
        }
        TimerCommand::Stop => cx.stop_timer(self.cam_timer),
    }
    if effects.redraw {
        self.draw_bg.redraw(cx);
    }
}
```

Route draw-time initial fit, pan, wheel zoom, pinch, `zoom_step`, `fit_to_scene`, `fit_to_selection`, preview-camera requests, `zoom_pct`, and every coordinate transform through the controller. The widget computes `bounding_box(&self.scene)` or the selected node rect and passes only bounds to the viewport.

- [ ] **Step 5: Verify viewport ownership and dependency direction**

Run:

```powershell
rtk cargo fmt --check
rtk cargo test -p waml-editor canvas::viewport::tests
rtk cargo test -p waml-editor
rtk rg -n "crate::camera|mod camera|camera:\s*Camera|cam_tween|drag_start_pan|focus_mode|fitted:" crates/waml-editor/src
```

Expected: tests PASS; the search returns no root camera module and no camera/tween/pan ownership fields in `ClassDiagramSurface`. References to `viewport.camera()` and local variables named `camera` are allowed.

- [ ] **Step 6: Commit viewport ownership**

```powershell
rtk git add crates/waml-editor/src/main.rs crates/waml-editor/src/canvas
rtk git commit -m "refactor(editor): extract viewport controller"
```

### Task 4: Extract Selection State and Unify Scene Reconciliation

**Files:**
- Create: `crates/waml-editor/src/canvas/class/selection.rs`
- Modify: `crates/waml-editor/src/canvas/class/mod.rs`
- Modify: `crates/waml-editor/src/canvas/class/widget.rs`
- Test: inline `canvas::class::selection::tests`
- Test: inline reconciliation tests in `canvas::class::widget::tests`

**Interfaces:**
- Consumes: `&[SceneNode]`, stable node keys, and internal `SceneUpdate`.
- Produces: `SelectionState`, immutable `SelectionSnapshot`, and keyed reconciliation. Rendering and callers never mutate its fields.

- [ ] **Step 1: Write failing keyed-selection tests**

Create `selection.rs` with these tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserve_viewport_re_resolves_the_selected_key() {
        let mut state = SelectionState::default();
        state.select("b", &nodes(&["a", "b"]));
        state.reconcile(&nodes(&["b", "c"]), SelectionPolicy::Preserve);
        assert_eq!(state.selected_key(), Some("b"));
        assert_eq!(state.selected_index(), Some(0));
    }

    #[test]
    fn a_missing_selected_key_clears_both_key_and_index() {
        let mut state = SelectionState::default();
        state.select("b", &nodes(&["a", "b"]));
        state.reconcile(&nodes(&["a"]), SelectionPolicy::Preserve);
        assert_eq!(state.selected_key(), None);
        assert_eq!(state.selected_index(), None);
    }

    #[test]
    fn stale_conflict_focus_keys_are_removed() {
        let mut state = SelectionState::default();
        state.set_conflict_focus_keys(Some(vec!["a".into(), "missing".into()]));
        state.reconcile(&nodes(&["a", "b"]), SelectionPolicy::Preserve);
        assert_eq!(
            state.snapshot().conflict_focus_keys,
            Some(HashSet::from(["a".to_string()])),
        );
    }
}
```

The local `nodes(&[&str]) -> Vec<SceneNode>` test builder must construct complete `SceneNode` values using the same zero-value pattern as the existing `many_attr_node` fixture; it must not introduce a production constructor solely for tests.

Use this complete local builder:

```rust
fn nodes(keys: &[&str]) -> Vec<SceneNode> {
    use waml::model::{ElementType, UmlMetaclass};
    keys.iter()
        .enumerate()
        .map(|(index, key)| SceneNode {
            key: (*key).to_string(),
            title: (*key).to_string(),
            element_type: ElementType::Uml(UmlMetaclass::Class),
            stereotypes: Vec::new(),
            attributes: Vec::new(),
            operations: Vec::new(),
            header: crate::scene::HeaderStyle::Plain,
            ports: false,
            rect: waml::solve::Rect {
                x: index as f64 * 100.0,
                y: 0.0,
                w: 80.0,
                h: 60.0,
            },
            emphasized: false,
            collapsed: false,
            expanded: false,
        })
        .collect()
}
```

- [ ] **Step 2: Run the selection tests to verify they fail**

Run:

```powershell
rtk cargo test -p waml-editor canvas::class::selection::tests
```

Expected: FAIL because `SelectionState`, `SelectionPolicy`, and `SelectionSnapshot` are not implemented.

- [ ] **Step 3: Implement the single selection owner**

Add `mod selection;` to `canvas/class/mod.rs`. Implement:

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ConstraintVisibility {
    None,
    #[default]
    Selected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SelectionPolicy {
    Clear,
    Preserve,
}

pub(crate) struct SelectionState {
    selected_key: Option<String>,
    selected_index: Option<usize>,
    constraint_visibility: ConstraintVisibility,
    conflict_focus_keys: Option<HashSet<String>>,
    show_hidden_borders: bool,
}

pub(crate) struct SelectionSnapshot {
    pub(crate) selected_key: Option<String>,
    pub(crate) selected_index: Option<usize>,
    pub(crate) constraint_visibility: ConstraintVisibility,
    pub(crate) conflict_focus_keys: Option<HashSet<String>>,
    pub(crate) show_hidden_borders: bool,
}

impl SelectionState {
    pub(crate) fn select(&mut self, key: &str, nodes: &[SceneNode]) -> bool;
    pub(crate) fn clear(&mut self) -> bool;
    pub(crate) fn reconcile(&mut self, nodes: &[SceneNode], policy: SelectionPolicy);
    pub(crate) fn selected_key(&self) -> Option<&str>;
    pub(crate) fn selected_index(&self) -> Option<usize>;
    pub(crate) fn has_selection(&self) -> bool;
    pub(crate) fn set_constraint_visibility(&mut self, mode: ConstraintVisibility);
    pub(crate) fn set_conflict_focus_keys(&mut self, keys: Option<Vec<String>>);
    pub(crate) fn set_show_hidden_borders(&mut self, on: bool);
    pub(crate) fn snapshot(&self) -> SelectionSnapshot;
}
```

Move the existing `ConstraintVisibility` declaration out of `widget.rs` and
re-export it from `canvas/class/mod.rs`; `canvas/mod.rs` keeps the unchanged
`crate::canvas::ConstraintVisibility` caller path.
`reconcile` resolves by key with `.position`, intersects conflict-focus keys with the new scene's key set, and converts an empty conflict-focus set to `None`.
Implement `Default` explicitly so selection/key/index/conflict focus are empty,
`constraint_visibility` is `ConstraintVisibility::Selected`, and
`show_hidden_borders` is `false`, matching today's widget defaults.

- [ ] **Step 4: Add the one scene reconciliation entry point**

Define the internal mode in `canvas/class/mod.rs`:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum SceneUpdate {
    Replace,
    Focus { key: String },
    PreserveViewport,
}
```

Replace widget fields `selected`, `selected_key`, `constraint_vis`, `conflict_focus_keys`, and `show_hidden_borders` with:

```rust
#[rust]
selection: SelectionState,
```

Add one internal method and make `set_scene`, `set_focus`, and `update_scene` delegate to it:

```rust
fn reconcile_scene(&mut self, cx: &mut Cx, scene: Scene, update: SceneUpdate) {
    self.reset_placement_for_scene_change(cx);
    let cancel_effects = self.viewport.cancel_glide();
    self.apply_viewport_effects(cx, cancel_effects);
    let bounds = bounding_box(&scene);
    let selection_policy = match &update {
        SceneUpdate::Replace | SceneUpdate::Focus { .. } => SelectionPolicy::Clear,
        SceneUpdate::PreserveViewport => SelectionPolicy::Preserve,
    };
    self.selection.reconcile(&scene.nodes, selection_policy);
    match &update {
        SceneUpdate::Replace => {
            self.viewport.request_initial_fit(
                bounds.map(InitialFit::Scene).unwrap_or(InitialFit::None),
            );
        }
        SceneUpdate::Focus { key } => {
            let focus = scene
                .nodes
                .iter()
                .find(|node| &node.key == key)
                .map(|node| InitialFit::Focus(node.rect))
                .unwrap_or(InitialFit::None);
            self.viewport.request_initial_fit(focus);
        }
        SceneUpdate::PreserveViewport => self.viewport.retain_for_scene_update(),
    }
    self.scene = scene;
    self.draw_bg.redraw(cx);
}
```

Implement `reset_placement_for_scene_change` in this task by calling the existing `cancel_drag(cx)`, clearing `zone_layouts`/`conflict_zones`, and stopping `dwell_timer`. Task 5 moves that complete behavior into `PlacementInteraction`.

For `set_focus`, derive `key` before moving `scene`:

```rust
pub fn set_focus(&mut self, cx: &mut Cx, scene: Scene) {
    let key = scene.nodes.first().map(|node| node.key.clone()).unwrap_or_default();
    self.reconcile_scene(cx, scene, SceneUpdate::Focus { key });
}
```

Current `set_focus` clears selection and the preview view explicitly disables fit-to-selection. Preserve that characterized behavior: the `Focus` key selects the camera target, not a click-selection highlight.

- [ ] **Step 5: Add reconciliation-table characterization tests**

Add focused tests around a pure `reconciliation_policy(&SceneUpdate) -> ReconciliationPolicy` helper:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CameraPolicy {
    Refit,
    Focus,
    Retain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ReconciliationPolicy {
    clear_placement: bool,
    selection: SelectionPolicy,
    camera: CameraPolicy,
}

fn reconciliation_policy(update: &SceneUpdate) -> ReconciliationPolicy {
    match update {
        SceneUpdate::Replace => ReconciliationPolicy {
            clear_placement: true,
            selection: SelectionPolicy::Clear,
            camera: CameraPolicy::Refit,
        },
        SceneUpdate::Focus { .. } => ReconciliationPolicy {
            clear_placement: true,
            selection: SelectionPolicy::Clear,
            camera: CameraPolicy::Focus,
        },
        SceneUpdate::PreserveViewport => ReconciliationPolicy {
            clear_placement: true,
            selection: SelectionPolicy::Preserve,
            camera: CameraPolicy::Retain,
        },
    }
}

#[test]
fn replace_clears_selection_and_refits() {
    assert_eq!(
        reconciliation_policy(&SceneUpdate::Replace),
        ReconciliationPolicy {
            clear_placement: true,
            selection: SelectionPolicy::Clear,
            camera: CameraPolicy::Refit,
        },
    );
}

#[test]
fn focus_preserves_the_unselected_preview_behavior() {
    let policy = reconciliation_policy(&SceneUpdate::Focus { key: "order".into() });
    assert_eq!(policy.selection, SelectionPolicy::Clear);
    assert_eq!(policy.camera, CameraPolicy::Focus);
}

#[test]
fn update_scene_preserves_camera_and_re_resolves_selection() {
    assert_eq!(
        reconciliation_policy(&SceneUpdate::PreserveViewport),
        ReconciliationPolicy {
            clear_placement: true,
            selection: SelectionPolicy::Preserve,
            camera: CameraPolicy::Retain,
        },
    );
}
```

- [ ] **Step 6: Run focused and crate tests**

```powershell
rtk cargo fmt --check
rtk cargo test -p waml-editor canvas::class::selection::tests
rtk cargo test -p waml-editor reconciliation
rtk cargo test -p waml-editor
```

Expected: all PASS. Inspect `ClassDiagramSurface` and confirm no duplicate selection/focus-visibility fields remain.

- [ ] **Step 7: Commit selection and reconciliation**

```powershell
rtk git add crates/waml-editor/src/canvas/class
rtk git commit -m "refactor(editor): extract selection state"
```

### Task 5: Extract Class Input Policy and the Complete Placement State Machine

**Files:**
- Create: `crates/waml-editor/src/canvas/class/interaction.rs`
- Create: `crates/waml-editor/src/canvas/class/placement.rs`
- Modify: `crates/waml-editor/src/canvas/class/mod.rs`
- Modify: `crates/waml-editor/src/canvas/class/widget.rs`
- Modify: `crates/waml-editor/src/canvas/viewport.rs`
- Test: inline `canvas::class::interaction::tests`
- Test: inline `canvas::class::placement::tests`

**Interfaces:**
- Consumes: stable `SceneNode::key` values, class pointer/key input, read-only scene geometry, `ViewportController`, and `SelectionState`.
- Produces: `ClassInteraction`, `PlacementInteraction`, `InteractionEffects`, `PlacementSnapshot`, `SurfaceIntent`, `TimerCommand`, and `FrameCommand`. The widget alone translates those values to `Cx`.

- [ ] **Step 1: Write failing click-slop and hit-policy tests**

In `interaction.rs`, define tests which preserve topmost-node, footer, and click/drag behavior:

```rust
fn test_node(key: &str, rect: waml::solve::Rect) -> SceneNode {
    use waml::model::{ElementType, UmlMetaclass};
    SceneNode {
        key: key.to_string(),
        title: key.to_string(),
        element_type: ElementType::Uml(UmlMetaclass::Class),
        stereotypes: Vec::new(),
        attributes: Vec::new(),
        operations: Vec::new(),
        header: crate::scene::HeaderStyle::Plain,
        ports: false,
        rect,
        emphasized: false,
        collapsed: false,
        expanded: false,
    }
}

fn test_viewport() -> ViewportController {
    let mut viewport = ViewportController::default();
    viewport.set_view_rect(Rect {
        pos: dvec2(0.0, 0.0),
        size: dvec2(800.0, 600.0),
    });
    viewport
}

#[test]
fn release_inside_click_slop_selects_the_topmost_node() {
    let rect = waml::solve::Rect { x: 80.0, y: 80.0, w: 80.0, h: 60.0 };
    let nodes = vec![test_node("back", rect), test_node("front", rect)];
    let hit = classify_release(
        dvec2(100.0, 100.0),
        dvec2(103.0, 100.0),
        &nodes,
        test_viewport().snapshot(),
    );
    assert_eq!(hit, ReleaseIntent::Select { key: "front".into() });
}

#[test]
fn footer_release_toggles_without_selecting() {
    let mut node = test_node(
        "order",
        waml::solve::Rect { x: 80.0, y: 80.0, w: 200.0, h: 200.0 },
    );
    node.attributes = (0..7)
        .map(|index| crate::inspector::AttrRow {
            name: format!("field{index}"),
            ty: "Int".into(),
            multiplicity: String::new(),
            visibility: "+".into(),
        })
        .collect();
    let screen = Rect {
        pos: dvec2(80.0, 80.0),
        size: dvec2(200.0, 200.0),
    };
    let footer = footer_screen_rect(&node, screen, 1.0).unwrap();
    let hit = classify_release(
        footer.pos + footer.size * 0.5,
        footer.pos + footer.size * 0.5,
        &[node],
        test_viewport().snapshot(),
    );
    assert_eq!(hit, ReleaseIntent::ToggleExpand { key: "order".into() });
}

#[test]
fn movement_at_the_slop_boundary_is_not_a_click() {
    assert!(!is_click(dvec2(0.0, 0.0), dvec2(SELECT_SLOP, 0.0)));
}
```

Move the existing `node_at`, `footer_screen_rect`, `selection click`, and `is_click` tests into this module without weakening assertions.

- [ ] **Step 2: Write failing placement transition tests**

In `placement.rs`, add state-machine tests with stable keys:

```rust
fn test_node(key: &str, x: f64) -> SceneNode {
    use waml::model::{ElementType, UmlMetaclass};
    SceneNode {
        key: key.to_string(),
        title: key.to_string(),
        element_type: ElementType::Uml(UmlMetaclass::Class),
        stereotypes: Vec::new(),
        attributes: Vec::new(),
        operations: Vec::new(),
        header: crate::scene::HeaderStyle::Plain,
        ports: false,
        rect: waml::solve::Rect { x, y: 0.0, w: 80.0, h: 60.0 },
        emphasized: false,
        collapsed: false,
        expanded: false,
    }
}

fn scene() -> Scene {
    Scene {
        nodes: vec![test_node("a", 0.0), test_node("b", 120.0), test_node("c", 240.0)],
        groups: Vec::new(),
        edges: Vec::new(),
        relations: Vec::new(),
        conflicts: Vec::new(),
    }
}

fn scene_without(key: &str) -> Scene {
    let mut scene = scene();
    scene.nodes.retain(|node| node.key != key);
    scene
}

fn viewport() -> ViewportController {
    let mut viewport = ViewportController::default();
    viewport.set_view_rect(Rect {
        pos: dvec2(0.0, 0.0),
        size: dvec2(800.0, 600.0),
    });
    viewport
}

fn dragging(key: &str) -> PlacementInteraction {
    let mut placement = PlacementInteraction::default();
    placement.begin_drag(key, dvec2(10.0, 10.0), (2.0, 3.0));
    placement
}

#[test]
fn sub_slop_motion_never_starts_placement() {
    let mut placement = PlacementInteraction::default();
    placement.begin_drag("a", dvec2(10.0, 10.0), (2.0, 3.0));
    let mut scene = scene();
    let mut viewport = viewport();
    let effects = placement.drag_to(
        dvec2(13.0, 10.0),
        &mut scene,
        &mut viewport,
    );
    assert!(!placement.snapshot().drag_moved);
    assert_eq!(effects.intent, None);
}

#[test]
fn dwell_retarget_stops_the_old_timer_and_starts_a_new_one() {
    let mut placement = dragging("a");
    let first = placement.hover_target(Some("b"), &scene());
    assert_eq!(first.dwell_timer, TimerCommand::StartTimeout(DWELL_SECS));
    let second = placement.hover_target(Some("c"), &scene());
    assert_eq!(second.dwell_timer, TimerCommand::RestartTimeout(DWELL_SECS));
}

#[test]
fn dwell_arm_emits_keys_and_frozen_center() {
    let mut placement = dragging("a");
    placement.hover_target(Some("b"), &scene());
    let effects = placement.dwell_elapsed(&scene(), dvec2(400.0, 300.0));
    assert_eq!(
        effects.intent,
        Some(SurfaceIntent::CompassArmed {
            subject_key: "a".into(),
            reference_key: "b".into(),
            center: dvec2(400.0, 300.0),
        }),
    );
}

#[test]
fn scene_change_clears_stale_drag_dwell_dial_and_preview() {
    let mut placement = dragging("a");
    let mut scene = scene();
    let mut viewport = viewport();
    placement.hover_target(Some("b"), &scene);
    placement.dwell_elapsed(&scene, dvec2(400.0, 300.0));
    let layout = BTreeMap::from([
        ("a".to_string(), waml::solve::Rect { x: 160.0, y: 0.0, w: 80.0, h: 60.0 }),
        ("b".to_string(), waml::solve::Rect { x: 40.0, y: 0.0, w: 80.0, h: 60.0 }),
    ]);
    placement.set_candidate_layouts(
        vec![(Zone::Right, layout)],
        &mut scene,
        &mut viewport,
    );
    placement.preview_zone(
        Some(Zone::Right),
        &mut scene,
        &mut viewport,
    );
    let effects = placement.cancel_for_scene_change(&mut scene, &mut viewport);
    assert_eq!(placement.snapshot(), PlacementSnapshot::default());
    assert_eq!(effects.dwell_timer, TimerCommand::Stop);
    assert_eq!(effects.preview_frame, FrameCommand::Stop);
}

#[test]
fn missing_keys_cancel_instead_of_using_cached_indices() {
    let mut placement = dragging("deleted");
    let mut scene = scene_without("deleted");
    let mut viewport = viewport();
    let effects = placement.drag_to(
        dvec2(40.0, 40.0),
        &mut scene,
        &mut viewport,
    );
    assert_eq!(effects.intent, None);
    assert_eq!(placement.snapshot().dragged_key, None);
}
```

Add these companion tests using the concrete helpers above:

```rust
#[test]
fn leaving_dial_reach_requests_dismiss_and_clears_verdicts() {
    let mut placement = dragging("a");
    let mut scene = scene();
    let mut viewport = viewport();
    placement.hover_target(Some("b"), &scene);
    placement.dwell_elapsed(&scene, dvec2(100.0, 100.0));
    placement.set_conflict_zones(vec![Zone::Left]);
    let effects = placement.drag_to(
        dvec2(100.0 + DIAL_REACH + 1.0, 100.0),
        &mut scene,
        &mut viewport,
    );
    assert_eq!(effects.intent, Some(SurfaceIntent::DialDismiss));
    assert!(placement.snapshot().conflict_zones.is_empty());
}

#[test]
fn popup_commit_can_read_the_pair_after_pointer_up_teardown() {
    let mut placement = dragging("a");
    let mut scene = scene();
    let mut viewport = viewport();
    placement.hover_target(Some("b"), &scene);
    placement.dwell_elapsed(&scene, dvec2(100.0, 100.0));
    placement.finish_pointer_up(&mut scene, &mut viewport);
    let authored = placement.placement_for(Zone::Right).unwrap();
    assert_eq!(authored.subject_key, "a");
    assert_eq!(authored.reference_key, "b");
    assert_eq!(authored.directions, vec![waml::syntax::Direction::RightOf]);
}

#[test]
fn preview_retargets_returns_and_clears() {
    let mut placement = dragging("a");
    let mut scene = scene();
    let mut viewport = viewport();
    placement.hover_target(Some("b"), &scene);
    placement.dwell_elapsed(&scene, dvec2(100.0, 100.0));
    let right = BTreeMap::from([
        ("a".to_string(), waml::solve::Rect { x: 200.0, y: 0.0, w: 80.0, h: 60.0 }),
        ("b".to_string(), waml::solve::Rect { x: 80.0, y: 0.0, w: 80.0, h: 60.0 }),
    ]);
    let left = BTreeMap::from([
        ("a".to_string(), waml::solve::Rect { x: 0.0, y: 0.0, w: 80.0, h: 60.0 }),
        ("b".to_string(), waml::solve::Rect { x: 120.0, y: 0.0, w: 80.0, h: 60.0 }),
    ]);
    placement.set_candidate_layouts(
        vec![(Zone::Right, right), (Zone::Left, left)],
        &mut scene,
        &mut viewport,
    );
    placement.preview_zone(Some(Zone::Right), &mut scene, &mut viewport);
    placement.preview_zone(Some(Zone::Left), &mut scene, &mut viewport);
    assert_eq!(placement.preview.as_ref().map(|preview| preview.zone), Some(Zone::Left));
    placement.preview_zone(None, &mut scene, &mut viewport);
    assert!(placement.preview.as_ref().is_some_and(|preview| preview.closing));
    placement.tick_preview(10.0, &mut scene, &mut viewport);
    placement.tick_preview(10.0 + PREVIEW_SECS, &mut scene, &mut viewport);
    assert!(placement.preview.is_none());
}

#[test]
fn escape_cancel_clears_drag_candidate_and_preview_state() {
    let mut placement = dragging("a");
    let mut scene = scene();
    let mut viewport = viewport();
    let effects = placement.cancel(&mut scene, &mut viewport);
    assert_eq!(placement.snapshot(), PlacementSnapshot::default());
    assert_eq!(effects.dwell_timer, TimerCommand::Stop);
}
```

These tests lock `DIAL_REACH`, `PREVIEW_SECS = 0.22`, ordinary cancel, and the
important existing popup ordering: pointer-up tears down the visual drag but
retains `DialPair` long enough for `placement_for` to run when the popup result
is drained later in the same action cycle.

- [ ] **Step 3: Run both new modules red**

```powershell
rtk cargo test -p waml-editor canvas::class::interaction::tests
rtk cargo test -p waml-editor canvas::class::placement::tests
```

Expected: FAIL because the controllers and typed effects do not exist.

- [ ] **Step 4: Implement typed effects with no Makepad authority**

Add both modules to `canvas/class/mod.rs`. Use these exact shared class-internal types:

```rust
pub(super) const SELECT_SLOP: f64 = 4.0;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum ReleaseIntent {
    NotClick,
    Select { key: String },
    Deselect,
    ToggleExpand { key: String },
}

pub(super) fn node_at(
    nodes: &[SceneNode],
    viewport: ViewportSnapshot,
    abs: DVec2,
) -> Option<usize>;

pub(super) fn footer_screen_rect(
    node: &SceneNode,
    screen: Rect,
    zoom: f64,
) -> Option<Rect>;

pub(super) fn classify_release(
    down_abs: DVec2,
    up_abs: DVec2,
    nodes: &[SceneNode],
    viewport: ViewportSnapshot,
) -> ReleaseIntent;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum SurfaceIntent {
    NodeMenu { abs: DVec2, key: String },
    NodeSelect { key: String },
    NodeDeselect,
    ToggleExpand { key: String },
    CompassArmed {
        subject_key: String,
        reference_key: String,
        center: DVec2,
    },
    DialDismiss,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum TimerCommand {
    Keep,
    StartTimeout(f64),
    RestartTimeout(f64),
    Stop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FrameCommand {
    Keep,
    Request,
    Stop,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct InteractionEffects {
    pub(super) consumed: bool,
    pub(super) redraw: bool,
    pub(super) dwell_timer: TimerCommand,
    pub(super) preview_frame: FrameCommand,
    pub(super) intent: Option<SurfaceIntent>,
}
```

Implement `Default for InteractionEffects` as `consumed: false`,
`redraw: false`, both commands `Keep`, and `intent: None`.
Keep this `TimerCommand` class-private and distinct from `viewport::TimerCommand`, because dwell timeout commands and camera interval commands have different legal operations.

- [ ] **Step 5: Move every placement field and transition together**

Move `Placed`, `Zone`, `DialPlacement`, `COMPASS_ZONES`, `DIAL_ZONES`,
`zone_id`, `zone_of_id`, `zone_arrow`, and `zone_placed` from `widget.rs` into
`placement.rs` with their current derives, values, and function bodies.
Re-export those caller-facing values from `canvas/class/mod.rs` so
`canvas/mod.rs` retains the exact current paths. Keep `DialPair` and `Preview`
private to `placement.rs`.

Implement `PlacementInteraction` with stable keys at boundaries:

```rust
#[derive(Default)]
pub(super) struct PlacementInteraction {
    dragged_key: Option<String>,
    cached_drag_index: Option<usize>,
    grab_offset: (f64, f64),
    drag_moved: bool,
    ghost: Option<waml::solve::Rect>,
    dwell_candidate_key: Option<String>,
    armed_target_key: Option<String>,
    compass_zone: Option<Zone>,
    dial_center: Option<DVec2>,
    dial_pair: Option<DialPair>,
    candidate_layouts: Vec<(Zone, BTreeMap<String, waml::solve::Rect>)>,
    conflict_zones: Vec<Zone>,
    preview: Option<Preview>,
    preview_last_time: f64,
    cursor_abs: DVec2,
}
```

Expose rendering state as owned, read-only values rather than controller
references:

```rust
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct PreviewGhost {
    pub(super) center: DVec2,
    pub(super) size: DVec2,
    pub(super) key: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct PlacementSnapshot {
    pub(super) dragged_key: Option<String>,
    pub(super) drag_moved: bool,
    pub(super) ghost: Option<waml::solve::Rect>,
    pub(super) armed_target_key: Option<String>,
    pub(super) compass_zone: Option<Zone>,
    pub(super) dial_center: Option<DVec2>,
    pub(super) conflict_zones: Vec<Zone>,
    pub(super) placed: Placed,
    pub(super) preview_ghost: Option<PreviewGhost>,
}
```

Add `Debug` to the existing `Placed` derive so `PlacementSnapshot` can derive
`Debug` while retaining `Clone`, `Copy`, `Default`, and `PartialEq`.

Move the complete existing bodies for candidate layout latching, edge baseline restoration, preview retarget/return, conflict verdict clearing, dial close, cancel, and placement construction. Replace index persistence with:

```rust
fn resolve_index(scene: &Scene, key: &str) -> Option<usize> {
    scene.nodes.iter().position(|node| node.key == key)
}
```

Cached indices may be used only after confirming `scene.nodes.get(index).is_some_and(|node| node.key == key)`. Otherwise re-resolve by key; cancel the affected transition if resolution fails.

Expose only:

```rust
impl PlacementInteraction {
    pub(super) fn begin_drag(&mut self, key: &str, abs: DVec2, grab_offset: (f64, f64));
    pub(super) fn drag_to(&mut self, abs: DVec2, scene: &mut Scene, viewport: &mut ViewportController) -> InteractionEffects;
    pub(super) fn hover_target(&mut self, key: Option<&str>, scene: &Scene) -> InteractionEffects;
    pub(super) fn dwell_elapsed(&mut self, scene: &Scene, center: DVec2) -> InteractionEffects;
    pub(super) fn set_candidate_layouts(&mut self, layouts: Vec<(Zone, BTreeMap<String, waml::solve::Rect>)>, scene: &mut Scene, viewport: &mut ViewportController) -> InteractionEffects;
    pub(super) fn set_conflict_zones(&mut self, zones: Vec<Zone>) -> bool;
    pub(super) fn preview_zone(&mut self, zone: Option<Zone>, scene: &mut Scene, viewport: &mut ViewportController) -> InteractionEffects;
    pub(super) fn tick_preview(&mut self, time: f64, scene: &mut Scene, viewport: &mut ViewportController) -> InteractionEffects;
    pub(super) fn finish_pointer_up(&mut self, scene: &mut Scene, viewport: &mut ViewportController) -> InteractionEffects;
    pub(super) fn cancel(&mut self, scene: &mut Scene, viewport: &mut ViewportController) -> InteractionEffects;
    pub(super) fn cancel_for_scene_change(&mut self, scene: &mut Scene, viewport: &mut ViewportController) -> InteractionEffects;
    pub(super) fn placement_for(&self, zone: Zone) -> Option<DialPlacement>;
    pub(super) fn snapshot(&self) -> PlacementSnapshot;
}
```

After migration, remove all corresponding fields from `ClassDiagramSurface`. Keep only Makepad `dwell_timer: Timer` and `preview_frame: NextFrame` handles in the widget.

- [ ] **Step 6: Make `ClassInteraction` own explicit class-input priority**

`ClassInteraction` must not be a forwarding-only module. It owns class hit classification, click-slop/footer/context-menu priority, selection coordination, and the decision “placement consumes move; otherwise viewport pans”:

```rust
#[derive(Default)]
pub(super) struct ClassInteraction;

impl ClassInteraction {
    pub(super) fn secondary_down(
        &mut self,
        abs: DVec2,
        scene: &Scene,
        viewport: ViewportSnapshot,
    ) -> InteractionEffects;

    pub(super) fn primary_down(
        &mut self,
        abs: DVec2,
        scene: &Scene,
        viewport: &mut ViewportController,
        placement: &mut PlacementInteraction,
    ) -> InteractionEffects;

    pub(super) fn pointer_move(
        &mut self,
        abs: DVec2,
        scene: &mut Scene,
        viewport: &mut ViewportController,
        selection: &mut SelectionState,
        placement: &mut PlacementInteraction,
    ) -> InteractionEffects;

    pub(super) fn pointer_up(
        &mut self,
        abs: DVec2,
        primary: bool,
        scene: &mut Scene,
        viewport: &mut ViewportController,
        selection: &mut SelectionState,
        placement: &mut PlacementInteraction,
    ) -> InteractionEffects;
}
```

Keep widget event order exactly:

1. camera interval tick;
2. Escape placement cancellation;
3. dwell timeout;
4. preview frame tick;
5. raw two-touch pinch capture and early return;
6. captured pointer hits: secondary down, primary down, move, primary up, other up, hover, scroll.

Add an `EVENT_PRIORITY` test-only array or a pure dispatch-order helper and assert this ordering without constructing a GPU context.

- [ ] **Step 7: Translate controller effects in one widget helper**

Add:

```rust
fn apply_interaction_effects(&mut self, cx: &mut Cx, effects: InteractionEffects) {
    match effects.dwell_timer {
        TimerCommand::Keep => {}
        TimerCommand::StartTimeout(seconds) => {
            self.dwell_timer = cx.start_timeout(seconds);
        }
        TimerCommand::RestartTimeout(seconds) => {
            cx.stop_timer(self.dwell_timer);
            self.dwell_timer = cx.start_timeout(seconds);
        }
        TimerCommand::Stop => cx.stop_timer(self.dwell_timer),
    }
    if effects.preview_frame == FrameCommand::Request {
        self.preview_frame = cx.new_next_frame();
    }
    if effects.redraw {
        self.draw_bg.redraw(cx);
    }
    if let Some(intent) = effects.intent {
        let action = ClassDiagramSurfaceAction::from(intent);
        cx.widget_action(self.widget_uid(), action);
    }
}
```

Implement `From<SurfaceIntent> for ClassDiagramSurfaceAction` as an exhaustive one-to-one mapping and unit-test every variant and payload. `FrameCommand::Stop` invalidates controller preview state; a stale `NextFrame` event then produces no effect because `tick_preview` sees no preview.

- [ ] **Step 8: Run the transition and crate gates**

```powershell
rtk cargo fmt --check
rtk cargo test -p waml-editor canvas::class::interaction::tests
rtk cargo test -p waml-editor canvas::class::placement::tests
rtk cargo test -p waml-editor
rtk rg -n "drag_node|drag_target|dwell_cand|dial_pair|zone_layouts|conflict_zones|preview_last_time|drag_ghost" crates/waml-editor/src/canvas/class/widget.rs
```

Expected: all tests PASS; the ownership-field search has no matches in `widget.rs` except accesses through `self.placement`.

- [ ] **Step 9: Commit class interaction ownership**

```powershell
rtk git add crates/waml-editor/src/canvas
rtk git commit -m "refactor(editor): extract class interaction state"
```

### Task 6: Split Rendering into Ordered, Read-Only Passes

**Files:**
- Create: `crates/waml-editor/src/canvas/class/render/mod.rs`
- Create: `crates/waml-editor/src/canvas/class/render/groups.rs`
- Create: `crates/waml-editor/src/canvas/class/render/edges.rs`
- Create: `crates/waml-editor/src/canvas/class/render/nodes.rs`
- Create: `crates/waml-editor/src/canvas/class/render/relations.rs`
- Create: `crates/waml-editor/src/canvas/class/render/overlays.rs`
- Create: `crates/waml-editor/src/canvas/class/render/primitives.rs`
- Modify: `crates/waml-editor/src/canvas/class/mod.rs`
- Modify: `crates/waml-editor/src/canvas/class/widget.rs`
- Test: inline render module and pass-local tests

**Interfaces:**
- Consumes: `RenderSnapshot<'_>` containing `&Scene`, `ViewportSnapshot`, `SelectionSnapshot`, and `PlacementSnapshot`; `ClassDrawResources<'_>` borrowing the widget's Makepad draw fields.
- Produces: the same GPU drawing in an asserted `RenderPass` order. No pass receives `&mut SelectionState`, `&mut PlacementInteraction`, or `&mut ViewportController`.

- [ ] **Step 1: Write the failing pass-order test**

Create `render/mod.rs` with:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RenderPass {
    Background,
    Groups,
    Edges,
    Nodes,
    Relations,
    ConflictFocus,
    Placement,
}

pub(super) const PASS_ORDER: [RenderPass; 7] = [
    RenderPass::Background,
    RenderPass::Groups,
    RenderPass::Edges,
    RenderPass::Nodes,
    RenderPass::Relations,
    RenderPass::ConflictFocus,
    RenderPass::Placement,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_render_order_is_behaviorally_stable() {
        assert_eq!(
            PASS_ORDER,
            [
                RenderPass::Background,
                RenderPass::Groups,
                RenderPass::Edges,
                RenderPass::Nodes,
                RenderPass::Relations,
                RenderPass::ConflictFocus,
                RenderPass::Placement,
            ],
        );
    }
}
```

Declare `mod render;` in `canvas/class/mod.rs`, then run:

```powershell
rtk cargo test -p waml-editor canvas::class::render::tests
```

Expected: the order test PASS; the render coordinator is not yet wired, so the task remains incomplete until later steps.

- [ ] **Step 2: Define immutable snapshots and borrowed draw resources**

In `render/mod.rs`:

```rust
pub(super) struct RenderSnapshot<'a> {
    pub(super) scene: &'a Scene,
    pub(super) viewport: ViewportSnapshot,
    pub(super) selection: SelectionSnapshot,
    pub(super) placement: PlacementSnapshot,
}
```

In `primitives.rs`, define `ClassDrawResources<'a>` with one mutable borrow for each existing Makepad draw field:

```rust
pub(super) struct ClassDrawResources<'a> {
    pub(super) bg: &'a mut DrawColor,
    pub(super) node: &'a mut DrawColor,
    pub(super) group: &'a mut DrawColor,
    pub(super) group_dashed: &'a mut DrawColor,
    pub(super) group_title_dim: &'a mut DrawColor,
    pub(super) edge: &'a mut DrawColor,
    pub(super) elbow: &'a mut DrawColor,
    pub(super) marker: &'a mut DrawColor,
    pub(super) rule: &'a mut DrawColor,
    pub(super) veil: &'a mut DrawColor,
    pub(super) text: &'a mut DrawText,
    pub(super) mono_dim: &'a mut DrawText,
    pub(super) mono_bold: &'a mut DrawText,
    pub(super) mono_accent: &'a mut DrawText,
    pub(super) mono_amber: &'a mut DrawText,
}
```

Move `fill_rect`, font-raster selection, screen-rect projection helpers, and the class Makepad shader idiom test into `primitives.rs`. Pure shared geometry remains in `canvas/geometry.rs`.

- [ ] **Step 3: Extract groups and edges with their policies**

Move group policy (`GroupDraw`, `group_draw_mode`, `untitled_label`, `group_label`, `group_plan`) and the complete group draw loop into:

```rust
pub(super) fn draw_groups(
    cx: &mut Cx2d,
    snapshot: &RenderSnapshot<'_>,
    draws: &mut ClassDrawResources<'_>,
);
```

Move edge bar, fillet, terminal-marker, and device-pixel draw code into:

```rust
pub(super) fn draw_edges(
    cx: &mut Cx2d,
    snapshot: &RenderSnapshot<'_>,
    draws: &mut ClassDrawResources<'_>,
);
```

Keep the existing constants, route order, “markers after segments”, and “nodes later cover marker overhang” behavior. Import shared geometry from `canvas::geometry`; do not copy it into `edges.rs`.

- [ ] **Step 4: Extract nodes/cards and relations/veils**

Move `draw_card`, footer rendering, focus-state/desaturation, and the node loop into:

```rust
pub(super) fn draw_nodes(
    cx: &mut Cx2d,
    snapshot: &RenderSnapshot<'_>,
    draws: &mut ClassDrawResources<'_>,
);
```

Move `relations_for_visibility`, `veil_band`, `cross_fade_params`, `veil_ramp`, `draw_veil_for`, and persistent relation drawing into:

```rust
pub(super) fn draw_relations(
    cx: &mut Cx2d,
    snapshot: &RenderSnapshot<'_>,
    draws: &mut ClassDrawResources<'_>,
);
```

Preserve existing relation order and the selected-node veil semantics. Move their current unit tests beside the owning functions.

- [ ] **Step 5: Extract conflict and placement overlays**

In `overlays.rs`, implement two separate pass functions so the order is explicit:

```rust
pub(super) fn draw_conflict_focus(
    cx: &mut Cx2d,
    snapshot: &RenderSnapshot<'_>,
    draws: &mut ClassDrawResources<'_>,
);

pub(super) fn draw_placement(
    cx: &mut Cx2d,
    snapshot: &RenderSnapshot<'_>,
    draws: &mut ClassDrawResources<'_>,
);
```

Move the current conflict fade, drag ghost, dial/compass, conflict verdict, selected-relation emphasis, and preview ghost rendering intact. These functions may query `PlacementSnapshot`; they may not transition placement state.

- [ ] **Step 6: Wire the canonical coordinator and shorten `draw_walk`**

Implement:

```rust
pub(super) fn draw(
    cx: &mut Cx2d,
    snapshot: &RenderSnapshot<'_>,
    draws: &mut ClassDrawResources<'_>,
) {
    draws.bg.draw_abs(cx, snapshot.viewport.view_rect);
    groups::draw_groups(cx, snapshot, draws);
    edges::draw_edges(cx, snapshot, draws);
    nodes::draw_nodes(cx, snapshot, draws);
    relations::draw_relations(cx, snapshot, draws);
    overlays::draw_conflict_focus(cx, snapshot, draws);
    overlays::draw_placement(cx, snapshot, draws);
}
```

Reduce the widget method to layout, initial-fit coordination, immutable snapshot construction, draw-resource borrowing, and the call:

```rust
fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
    let rect = cx.walk_turtle(walk);
    self.viewport.set_view_rect(rect);
    self.viewport.apply_initial_fit();
    let snapshot = RenderSnapshot {
        scene: &self.scene,
        viewport: self.viewport.snapshot(),
        selection: self.selection.snapshot(),
        placement: self.placement.snapshot(),
    };
    let mut draws = self.draw_resources();
    render::draw(cx, &snapshot, &mut draws);
    DrawStep::done()
}
```

`draw_resources(&mut self) -> ClassDrawResources<'_>` borrows only the concrete draw fields. If Rust reports overlapping borrows, destructure `ClassDiagramSurface` into disjoint references before constructing `snapshot` and `draws`; do not clone `Scene` every frame and do not give render code controller mutability.

- [ ] **Step 7: Verify pass tests and read-only boundaries**

```powershell
rtk cargo fmt --check
rtk cargo test -p waml-editor canvas::class::render
rtk cargo test -p waml-editor
rtk rg -n "&mut (SelectionState|PlacementInteraction|ViewportController)|widget_action|start_timeout|start_interval|new_next_frame" crates/waml-editor/src/canvas/class/render
```

Expected: tests PASS; the boundary search returns no matches in the render tree.

- [ ] **Step 8: Commit ordered rendering**

```powershell
rtk git add crates/waml-editor/src/canvas
rtk git commit -m "refactor(editor): split class render passes"
```

### Task 7: Remove Forwarding State, Verify the Façade, and Run the Full Gate

**Files:**
- Modify: `crates/waml-editor/src/canvas/class/widget.rs`
- Modify: `crates/waml-editor/src/canvas/class/mod.rs`
- Modify: `crates/waml-editor/src/canvas/mod.rs`
- Modify: `crates/waml-editor/src/app.rs`
- Modify: `crates/waml-editor/src/class_diagram_view.rs`
- Modify: `crates/waml-editor/src/classifier_preview_view.rs`
- Modify: `crates/waml-editor/tests/README.md`
- Test: all inline `waml-editor` tests
- Verify: full workspace, clippy, native visual comparison, and manual interaction checklist

**Interfaces:**
- Consumes: completed `ViewportController`, `SelectionState`, `PlacementInteraction`, `ClassInteraction`, and immutable render coordinator.
- Produces: the final narrow `ClassDiagramSurface` façade and the existing `ClassDiagramSurfaceAction` vocabulary consumed by `ClassDiagramView`/`ClassifierPreviewView`; `App` continues to own document writes.

- [ ] **Step 1: Remove all temporary forwarding state and centralize effect translation**

Inspect `ClassDiagramSurface` and retain only:

- Makepad identity/source/walk/layout fields;
- the existing concrete draw resources;
- `scene: Scene`;
- `viewport: ViewportController`;
- `interaction: ClassInteraction`;
- `selection: SelectionState`;
- `placement: PlacementInteraction`;
- `cam_timer: Timer`, `dwell_timer: Timer`, and `preview_frame: NextFrame`.

Delete forwarding fields and duplicate writers. Keep two translation helpers only:

```rust
fn apply_viewport_effects(&mut self, cx: &mut Cx, effects: ViewportEffects);
fn apply_interaction_effects(&mut self, cx: &mut Cx, effects: InteractionEffects);
```

Make `handle_event` a coordinator in the tested priority order from Task 5. Make every public façade method delegate to exactly one controller or to `reconcile_scene`, then request redraw/timer effects at the widget boundary.

- [ ] **Step 2: Add final action-translation and API characterization tests**

Add pure action mapping tests:

```rust
#[test]
fn every_surface_intent_maps_without_losing_payloads() {
    let intents = [
        SurfaceIntent::NodeMenu { abs: dvec2(10.0, 20.0), key: "a".into() },
        SurfaceIntent::NodeSelect { key: "b".into() },
        SurfaceIntent::NodeDeselect,
        SurfaceIntent::ToggleExpand { key: "c".into() },
        SurfaceIntent::CompassArmed {
            subject_key: "a".into(),
            reference_key: "b".into(),
            center: dvec2(30.0, 40.0),
        },
        SurfaceIntent::DialDismiss,
    ];
    let actions: Vec<ClassDiagramSurfaceAction> =
        intents.into_iter().map(ClassDiagramSurfaceAction::from).collect();
    assert!(matches!(
        &actions[0],
        ClassDiagramSurfaceAction::NodeMenu { abs, key }
            if *abs == dvec2(10.0, 20.0) && key == "a"
    ));
    assert!(matches!(
        &actions[4],
        ClassDiagramSurfaceAction::CompassArmed {
            subject_key,
            reference_key,
            center,
        } if subject_key == "a"
            && reference_key == "b"
            && *center == dvec2(30.0, 40.0)
    ));
    assert_eq!(actions.len(), 6);
}
```

Add a reconciliation reset test proving scene replacement leaves placement snapshot default, no selected key, no conflict focus, a pending scene fit, and stop commands for camera/dwell/preview clocks.

- [ ] **Step 3: Run completion scans**

```powershell
rtk rg -n "GraphCanvas|GraphCanvasAction|mod\.widgets\.GraphCanvas" crates/waml-editor/src crates/waml-editor/tests docs
rtk rg -n "SceneNode|ClassDiagramSurfaceAction|ConstraintVisibility|Zone|placement|constraint|activity|sequence" crates/waml-editor/src/canvas/viewport.rs crates/waml-editor/src/canvas/geometry.rs
rtk rg -n "pub\(crate\).*Controller|pub use .*Controller|pub\(crate\) use .*render" crates/waml-editor/src/canvas
```

Expected:

- first search: no old identifiers;
- second search: no class-specific semantics in shared modules (comments discussing the prohibition should not be added there);
- third search: no controller or render implementation re-exported from `canvas/mod.rs`.

Confirm `scene.rs` remains at the crate root and only files under `canvas/class/` import it from the canvas subsystem.

- [ ] **Step 4: Run formatting, focused tests, workspace tests, and clippy**

```powershell
rtk cargo fmt --check
rtk cargo test -p waml-editor
rtk cargo test --workspace
rtk cargo clippy --workspace --all-targets
```

Expected: all commands PASS. If clippy exposes a pre-existing workspace warning, record the exact warning and verify no warning originates in this refactor; do not suppress it broadly.

- [ ] **Step 5: Capture post-refactor native screenshots**

Run the same PID-specific sequence from the worktree root. The prompts and
capture names deliberately mirror the baseline so every before image has one
after image:

```powershell
rtk proxy pwsh -NoProfile -Command @'
$ErrorActionPreference = "Stop"
$classSurfaceWorktree = (Resolve-Path ".").Path
$classSurfaceTarget = Join-Path $classSurfaceWorktree "target"
$classSurfaceExe = Join-Path $classSurfaceTarget "debug\waml-editor.exe"
$classSurfaceCaptureScript = Join-Path $classSurfaceWorktree "scripts\capture-window.ps1"
$classSurfacePhase = "after"

& rtk cargo build -p waml-editor --target-dir $classSurfaceTarget
if ($LASTEXITCODE -ne 0) {
    throw "waml-editor build failed with exit code $LASTEXITCODE"
}
if (-not (Test-Path -LiteralPath $classSurfaceExe)) {
    throw "worktree executable not found at $classSurfaceExe"
}

function Invoke-ClassSurfaceFixtureCapture {
    param(
        [Parameter(Mandatory = $true)][string]$ClassSurfaceFixture,
        [Parameter(Mandatory = $true)][object[]]$ClassSurfaceCaptures
    )

    $classSurfaceFixturePath =
        (Resolve-Path (Join-Path $classSurfaceWorktree $ClassSurfaceFixture)).Path
    $classSurfaceFixtureProcess = Start-Process `
        -FilePath $classSurfaceExe `
        -ArgumentList @($classSurfaceFixturePath) `
        -WorkingDirectory $classSurfaceWorktree `
        -WindowStyle Normal `
        -PassThru
    try {
        $classSurfaceWindowDeadline = (Get-Date).AddSeconds(30)
        do {
            Start-Sleep -Milliseconds 200
            $classSurfaceFixtureProcess.Refresh()
            if ($classSurfaceFixtureProcess.HasExited) {
                throw "editor pid=$($classSurfaceFixtureProcess.Id) exited before opening a window"
            }
        } while (
            $classSurfaceFixtureProcess.MainWindowHandle -eq 0 -and
            (Get-Date) -lt $classSurfaceWindowDeadline
        )
        if ($classSurfaceFixtureProcess.MainWindowHandle -eq 0) {
            throw "editor pid=$($classSurfaceFixtureProcess.Id) opened no window within 30 seconds"
        }

        foreach ($classSurfaceCapture in $ClassSurfaceCaptures) {
            $null = Read-Host $classSurfaceCapture.Prompt
            $classSurfaceCaptureOut = "C:\tmp\class-surface-$classSurfacePhase-$($classSurfaceCapture.Name).png"
            & rtk pwsh -File $classSurfaceCaptureScript `
                -Out $classSurfaceCaptureOut `
                -ProcessId $classSurfaceFixtureProcess.Id
            if ($LASTEXITCODE -ne 0) {
                throw "capture failed for pid=$($classSurfaceFixtureProcess.Id)"
            }
        }
    }
    finally {
        $classSurfaceFixtureProcess.Refresh()
        if (-not $classSurfaceFixtureProcess.HasExited) {
            Stop-Process -Id $classSurfaceFixtureProcess.Id -ErrorAction SilentlyContinue
            Wait-Process -Id $classSurfaceFixtureProcess.Id -Timeout 10 -ErrorAction SilentlyContinue
        }
    }
}

Invoke-ClassSurfaceFixtureCapture `
    -ClassSurfaceFixture "crates/waml-editor/tests/fixtures/mini" `
    -ClassSurfaceCaptures @(
        @{
            Name = "mini"
            Prompt = "Open Orders, fit the overview, then press Enter to capture"
        }
    )
Invoke-ClassSurfaceFixtureCapture `
    -ClassSurfaceFixture "crates/waml-editor/tests/fixtures/groups" `
    -ClassSurfaceCaptures @(
        @{
            Name = "groups"
            Prompt = "Open the groups diagram with hidden borders OFF, then press Enter"
        },
        @{
            Name = "groups-hidden"
            Prompt = "Enable hidden borders without moving the camera, then press Enter"
        }
    )
Invoke-ClassSurfaceFixtureCapture `
    -ClassSurfaceFixture "crates/waml-editor/tests/fixtures/sixkind" `
    -ClassSurfaceCaptures @(
        @{
            Name = "sixkind-overview"
            Prompt = "Open the sixkind overview and fit the scene, then press Enter"
        },
        @{
            Name = "sixkind-zoomed-out"
            Prompt = "Zoom out to the small-font raster level, then press Enter"
        },
        @{
            Name = "sixkind-zoomed-in"
            Prompt = "Zoom in to the large-font raster level, then press Enter"
        },
        @{
            Name = "sixkind-focus"
            Prompt = "Select a large classifier, expand its compartment, fit selection, then press Enter"
        }
    )
'@
```

Expected: seven native-resolution `class-surface-after-*.png` files in
`C:\tmp`, captured from only the returned worktree PID. Compare each matching
before/after pair. Verify group and nested-group bounds, routed edges and
terminal adornments, large/expanded classifier cards, selected and constraint
focus, conflict focus, hidden borders, and zoomed-in/zoomed-out font raster
levels. Accept only expected capture-time cursor/timing differences;
investigate geometry, color, z-order, or text-raster changes.

- [ ] **Step 6: Run the temporal manual interaction checklist**

In the native editor verify, in order:

1. left-drag pan, wheel zoom at cursor, two-finger pinch, fit-to-scene, zoom in/out glide, and fit-to-selection;
2. click selection/deselection, footer expansion, inspector-driven selection, and right-click context menu;
3. drag threshold below/above 4 px, dwell arm, target retarget, compass movement, candidate preview, hub return, Escape cancel, out-of-reach dismiss, and committed placement;
4. scene refresh after placement and expand/collapse while retaining the camera;
5. conflict-list focus and clearing/revalidation after conflict deletion;
6. tab/scene change during or immediately after dwell/preview, confirming no stale dial, timer, selection index, preview layout, or camera animation remains.

Screenshots do not replace this step because dwell, tween, capture, cancellation, and tab-change behavior are temporal.

- [ ] **Step 7: Commit the final façade cleanup**

```powershell
rtk git add crates/waml-editor/src crates/waml-editor/tests/README.md
rtk git commit -m "refactor(editor): finalize class surface facade"
```
