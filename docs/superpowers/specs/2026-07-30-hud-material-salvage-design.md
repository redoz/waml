# HUD Material Phase-C Salvage

**Status:** Approved in conversation
**Date:** 2026-07-30
**Source worktree:** `hud-material`

## Goal

Restore the completed Atlas surface material from the disconnected
`hud-material` history onto rewritten `main` without regressing the newer
screen-space linework work.

The salvage includes:

- accent bloom;
- theme-aware frost-gradient fill;
- `PanelSurface`, `NodeSurface`, and `ButtonSurface` presets;
- selected-surface shadow and bloom lift;
- device-pixel snapping for crisp borders;
- Rust/shader drift guards and bleed/snapping tests.

## Integration approach

Adapt the five Phase-C frame commits manually into the current
`crates/waml-editor/src/frame.rs`. Do not cherry-pick the disconnected commits
or replace the file wholesale.

The current `stroke_scale` uniform remains authoritative for screen-space
linework. Border thickness must therefore use `zoom * stroke_scale`, while
shadow and bloom continue to use raw zoom with the existing low-zoom floor.
The padded draw quad must account for the selected lift so the wider selected
halo cannot clip.

The uncommitted `mod.hud.HudFrame.zero` experiment is excluded. It is only
eligible for a follow-up if native rendering proves the committed zero-valued
`rect_pos` dependency is optimized incorrectly.

## Data flow

`SurfaceExt::draw_surface_abs` reads the surface preset and selection uniforms,
computes the effective shadow/bloom extent, snaps the true surface and padding
to the device grid, pushes `bleed`, and draws the inflated quad.

Canvas nodes derive `NodeSurface`. Menus, select flyouts, conflict lists, and
the selection toolbar derive `PanelSurface`. `ButtonSurface` stays available
without a consumer until a button adopts it explicitly.

The shader composites depth shadow and accent bloom beneath the surface,
applies the theme-aware frost ramp to the fill, and draws a snapped gradient
border. Selection lifts the shadow and bloom knobs toward the reference canvas
values without changing unselected presets.

## Verification

Use test-driven development:

1. Port the Phase-C helper and shader drift-guard tests and confirm they fail
   against current `main`.
2. Add the minimal helpers, uniforms, shader layers, and presets needed to pass.
3. Run the focused frame tests and the broader `waml-editor` test suite.
4. Run `cargo clippy -p waml-editor --all-targets -- -D warnings`.
5. Launch the native editor with the existing run script and capture a
   HiDPI-correct window image for visual verification of crisp borders, frost,
   bloom, and selected lift.

No other worktree changes or unrelated UI styling are in scope.
