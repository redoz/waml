# Shader Tree Icons — Design

**Date:** 2026-07-19
**Status:** Approved (design), pending implementation plan

## Problem

The 9 project-tree / doc-tab kind glyphs (`class`, `interface`, `enum`,
`datatype`, `package`, `diagram`, `flow`, `sequence`, `note`) ship as SVGs in
`crates/waml-editor/resources/icons/` and render blurry. They are drawn with
`DrawSvg::draw_abs` at 14px from a 24-unit viewBox with a 2.2 stroke; the
vector rasterizer downscales with no GPU-side antialiasing re-center, so thin
strokes soften. `tree_panel.rs` already fights this by rounding draw positions
to whole device pixels — a symptom, not a fix.

## Goal

Replace the SVG glyphs with hand-authored SDF shaders that read crisp at the
real 14px display size, using makepad's `Sdf2d`. This is an **open redesign**:
the 9 semantic kinds are fixed, but each silhouette is redrawn from scratch in
SDF rather than tracing the current SVG geometry. Icons keep the Atlas "HUD"
material language: single accent tint (`atlas.accent`), hollow interiors
(thin stroke, low-alpha or no fill).

## Architecture

### `icons.rs` (new module)

Owns the icon set, extracted out of `tree_panel.rs` so a standalone harness bin
can reference the exact same shaders (see Harness).

- **9 named shaders**, one per kind, declared in a `script_mod!` block as
  `mod.draw.IconClass = mod.draw.DrawColor{ pixel: fn(){ … } }`,
  `mod.draw.IconInterface{ … }`, etc. One shader per icon (not one
  parameterized `DrawIcon` with an `icon` uniform branch): mirrors the existing
  9-field `TreeIcons`, matches the one-shader-per-primitive idiom in
  `draw_hud.rs`, keeps each `pixel: fn` small and independently hot-reloadable,
  and avoids a fat 9-way GPU branch.
- **`TreeIcons`** struct + its `mod.widgets.TreeIcons` DSL move here from
  `tree_panel.rs`. Its 9 fields change type `DrawSvg` → `DrawColor`, each DSL
  field pointing at its `mod.draw.IconX{}` and keeping `color: atlas.accent`.
- **`icon_for(kind)`** return type changes `Option<&mut DrawSvg>` →
  `Option<&mut DrawColor>`. `Unknown` still returns `None`.

### Shader material rules

Per `[[makepad-fork-shader-gotchas]]`:
- Sharp corners use `sdf.rect`, never `sdf.box(…, 0.0)` (degenerates + floods).
  `sdf.box` only where a real corner radius is wanted.
- Hollow HUD look: `sdf.stroke(self.color, w)` for outlines; interiors either
  bare or a low-alpha `fill_keep` before stroke, matching the existing 0.16
  fill feel — tuned live, not copied from the SVG.
- Geometry authored in the shader's local `rect_size` (14px target) so stroke
  widths are chosen for that size, not scaled down from 24 units.

### Callers (mechanical)

- `tree_panel.rs`: drop the `TreeIcons`/icon `script_mod!`, import from
  `icons.rs`. `draw_row_icon` unchanged (`DrawColor` also has `draw_abs`).
- `doc_tabs.rs`: `use crate::icons::TreeIcons` instead of `tree_panel`;
  `icon_for(...).draw_abs(...)` call unchanged.
- `main.rs`: add `mod icons;`.

### Rip-out

- Delete the 9 files in `resources/icons/*.svg`.
- Remove `DrawSvg` icon fields and `crate_resource(...)` calls. (`DrawSvg` has
  no other use in the crate; the import goes.)

## Harness

`crates/waml-editor/src/bin/icon_harness.rs` — a standalone `app_main!` bin that
pulls the real shaders via `#[path = "../icons.rs"] mod icons;` (no lib
conversion). It renders a grid: every icon at real 14px **and** a ~5× zoom cell
for detail, on the Atlas `field_bg`, with a light/dark toggle. Run it, and
edit `icons.rs` shader source while it's live — makepad hot-reloads the DSL, so
each silhouette is tuned against real `Sdf2d` output, not an HTML mock.

**First harness task verifies hot-reload actually fires** in this fork build
before the redesign leans on it; if it doesn't, fall back to
edit → rebuild loop (slower, still works).

The harness is dev-only: not wired into the shipping `waml-editor` bin, not a
workspace default-run target.

## Testing / acceptance

- `build_id_maps` and other pure logic untouched — no new unit tests needed
  there.
- Visual acceptance is by eye in the harness: each icon crisp at 14px, no soft
  strokes, hollow HUD look, single accent tint, recognizable silhouette for its
  kind. Cross-check in the real editor tree + doc-tab strip.
- `cargo build -p waml-editor` (both bins) clean; no dangling `DrawSvg` /
  `crate_resource` icon references.

## Out of scope

- Node-body / canvas glyphs, toolbar icons, favicon — only the tree/doc-tab
  kind set.
- Any change to `TreeKind` or the kind→icon mapping.
- Parameterized/atlas icon system for future icons — revisit if the set grows.
