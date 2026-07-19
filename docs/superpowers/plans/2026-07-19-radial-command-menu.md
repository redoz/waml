# Radial Command Menu Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a dynamic 2–6 wedge radial (marking) command menu to `waml-editor`, opened on right-click of a canvas node, plus a reusable shader-drawn `icon` abstraction it consumes.

**Architecture:** Three landing units in order — (1) an `icon` module (SDF shapes over a `DrawColor`), (2) a `radial` module split into a pure, unit-tested geometry + state-machine core (`RadialCore`) wrapped by an event-passive `Radial` widget that owns the wedge shader + `NextFrame` animation, (3) canvas wiring where `GraphCanvas` detects a right-press on a node, emits an action, and `App` drives the overlay `Radial` and maps its committed `LiveId` to a node command. A fourth, independent, non-blocking task renames `HudFrame`→`AccentFrame`.

**Tech Stack:** Rust, Makepad (redoz/makepad fork, rev `4f9ce7a`), `script_mod!` MPSL shader DSL, immediate-mode hand-rolled widgets. `waml-editor` is a **binary** crate.

## Global Constraints

Every task's requirements implicitly include these (copied verbatim from the spec):

- **Makepad-only.** No web / Svelte frontend in scope.
- **2–6 items max** per radial. No nested / sub-radials.
- **No per-open accent recolour** (mock-only tooling — YAGNI).
- **Theme tokens only:** normal wedges use `atlas.accent`; `danger`/`Remove` wedges use `atlas.danger`; disabled wedges use `atlas.text_dim`. Never hardcode `#x…` literals — reference `atlas.<name>` (`crates/waml-editor/src/theme_atlas.rs:48` `accent: #x1496dc`, `:59` `danger: #xeb4678`, `:63` `text_dim: #x8a97a6`, `:51` `frame_hi`, `:52` `frame_lo`).
- **Geometry is exact:** N sectors of `360/N°`, first wedge **centred at 12 o'clock** proceeding clockwise; disc radius ≈120px screen-space, hub dead-zone radius ≈30px; hit-test is angle-from-centre so screen-edge clipping never changes which wedge is pickable.
- **Shader gotchas (repo memory, `makepad-fork-shader-gotchas`):** use `sdf.rect` for sharp corners (never `sdf.box(...,0.0)` — it floods); `let` bindings are not reassignable inside `pixel: fn()` (use a fresh name per step); a custom `pixel: fn()` attaches only via full `:` assignment, never `+:`; vary a knob per-draw with `set_uniform(cx, live_id!(name), &[f32])` (proven by `draw_node`'s `zoom`), tint via the real `color` field. Shader errors surface at GPU-compile (runtime log `[E] …:LINE`), not `cargo build`.
- **Test invocation:** `waml-editor` is a bin crate — run `cargo test -p waml-editor <filter>` (NOT `--lib`). The implement-plan gate is `cargo test --workspace`.

---

## File Structure

- **Create** `crates/waml-editor/src/icon.rs` — `Icon`/`IconShape` enums, `DrawIcon` shader, `draw_icon()`. (Task 1)
- **Create** `crates/waml-editor/src/radial.rs` — `RadialItem`, `RadialOutcome`, `RadialCore` (pure geometry + state machine), `Radial` widget (shader + animation). (Tasks 2–3)
- **Modify** `crates/waml-editor/src/main.rs` — add `mod icon;` and `mod radial;`. (Tasks 1–2)
- **Modify** `crates/waml-editor/src/app.rs` — register `icon`/`radial` `script_mod`s (`:611-623`), place `Radial` overlay in the `startup()` DSL tree, drive it in `handle_actions`/`handle_event`, map its outcome. (Tasks 3–4)
- **Modify** `crates/waml-editor/src/canvas.rs` — right-press detection, `node_at()` hit-test, `GraphCanvasAction`. (Task 4)
- **Modify** `crates/waml-editor/src/draw_hud.rs`→`frame.rs`, DSL `HudFrame`→`AccentFrame` across all consumers. (Task 5, independent)

---

## Reviewer flags — spec requirements NOT fully grounded in existing code

Read these before implementing; they change some spec assumptions.

1. **No existing "node command path" (spec Unit 3).** `crates/waml-editor/src/canvas.rs:1-6` states the canvas is read-only with **no hit-testing of individual nodes**, and `handle_event` (`:226-260`) only pans/zooms. There are **no** `Open`/`Style`/`Markdown`/`Remove` node handlers anywhere. Task 4 therefore *adds* node hit-testing and defines a **new** `NodeCommand` enum whose handlers are **logging stubs** — matching the established mock convention (`tool_dock.rs:5-7` "No tool behavior is wired into the canvas yet"). Flag for the reviewer: the "route into the existing node command path" language in the spec is aspirational; there is nothing to route into yet.

2. **`WamlButton`/`waml_button.rs`/`WamlButton::tick` does not exist on this branch.** The spec cites it as the animation pattern, but it lives on a **divergent** local `main` (commit `e7fceca`), NOT on `origin/main` (`518efe7`) which is this worktree's baseline. The underlying primitive it uses — `NextFrame` (`cx.new_next_frame()` / `NextFrame::is_event`, fork `platform/src/cx_api.rs:1403` + `event/event.rs:1063-1078`) — **is** in the makepad prelude and available here. Task 3 reproduces the `press`/`tick`/`release` `NextFrame` loop directly from that primitive; the WamlButton reference is illustrative only.

3. **`icon.rs` overlaps an existing `icons.rs` on divergent `main`.** Local `main` already ships `crates/waml-editor/src/icons.rs` (`TreeIcons` SDF glyphs). This is a *different* file/name from the spec's `icon.rs` and is not on this baseline. No collision on `origin/main` today, but if the branches merge, `icon.rs` (generic) and `icons.rs` (tree glyphs) will coexist — reviewer should decide whether to later fold one into the other. This plan builds `icon.rs` as specified.

4. **"One shared stroke recipe" factoring (spec Rendering) is reproduced, not literally factored.** The repo's MPSL DSL has **no** shared-shader-function mechanism in use anywhere; each `DrawColor` inlines its own `pixel: fn()`. Task 3's wedge shader therefore *copies* `HudFrame`'s proven 150° fade stroke recipe (`draw_hud.rs:45-48`) inline. Literal single-source factoring is left as an optional follow-up noted in Task 5.

5. **Right-mouse `Hit::FingerDown` delivery — verify at runtime.** The fork *does* build a `FingerDown` for the secondary button (`platform/src/event/finger.rs:1279-1296`, `device: DigitDevice::Mouse{button: e.button}`), and `FingerDownEvent` derefs to `DigitDevice` exposing `mouse_button() -> Option<MouseButton>` (`:574`) with `MouseButton::SECONDARY` / `is_secondary()` (`platform/studio/src/mouse.rs:50,95`). Task 4 uses `fe.mouse_button() == Some(MouseButton::SECONDARY)`. Confirm during the Task 4 screenshot that a right-press actually reaches the canvas `hits_with_capture_overload` branch (some platforms route the secondary button differently); if not, fall back to reading `Event::MouseDown` directly in `App::handle_event`.

6. **Sequencing of the `AccentFrame` rename.** Placed **last** (Task 5), independent and trailing. Justification: the spec explicitly says "the radial reuses the frame material either way and does not block on it." Renaming first would touch every existing `HudFrame` consumer (`canvas.rs:31`, `tool_dock.rs:22`, `tree_panel`, `draw_hud.rs`) before any feature lands — a large blast radius with no benefit to the radial, which copies the stroke recipe inline regardless. The `waml_button.rs`→`button.rs` / `WamlButton`→`Button` half of the spec's rename table **cannot be grounded on this baseline** (no such file here) and is omitted from Task 5 — flag for the reviewer.

---

### Task 1: `icon` module

**Files:**
- Create: `crates/waml-editor/src/icon.rs`
- Modify: `crates/waml-editor/src/main.rs:11` (add `mod icon;` in the alphabetical `mod` list)
- Test: inline `#[cfg(test)] mod tests` in `crates/waml-editor/src/icon.rs`

**Interfaces:**
- Produces:
  - `pub enum Icon { Glyph(char), Shape(IconShape) }`
  - `pub enum IconShape { Open, Style, Markdown, Remove }` with `pub fn shader_index(self) -> u32`
  - `pub fn glyph(&self) -> Option<char>` on `Icon`
  - `pub fn draw_icon(cx: &mut Cx2d, draw: &mut DrawColor, rect: Rect, icon: &Icon, tint: Vec4)`
  - `pub fn script_mod(vm: &mut ScriptVm) -> ScriptValue` (registers `mod.draw.DrawIcon`)
- Consumes: nothing (first task).

- [ ] **Step 1: Add the module declaration**

In `crates/waml-editor/src/main.rs`, add `mod icon;` immediately before `mod inspector;` (keep the list alphabetical):

```rust
mod draw_hud;
mod icon;
mod inspector;
```

- [ ] **Step 2: Write the failing test**

Create `crates/waml-editor/src/icon.rs` with only the test module first:

```rust
//! Generic icon abstraction: a wedge/tool references an `Icon` without knowing
//! how it paints. Not a font atlas -- `Shape` icons are shader-drawn SDFs on a
//! `DrawColor` (`mod.draw.DrawIcon`), matching the mock's hand-drawn glyphs.
//! `Glyph(char)` keeps the existing single-char `DrawText` path valid for
//! callers that have no SDF shape yet. Grows one branch at a time.

use makepad_widgets::*;

/// A drawable icon. Additive: `Texture(TextureId)` can be added later with no
/// API break.
#[derive(Clone, Debug, PartialEq)]
pub enum Icon {
    /// Single char drawn by the caller's own `DrawText` pen (placeholder path).
    Glyph(char),
    /// Shader-drawn SDF selected by `IconShape`.
    Shape(IconShape),
}

/// The seed SDF set: exactly the four node-radial commands. Adding an icon =
/// one variant here + one `pixel()` branch in `mod.draw.DrawIcon`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconShape {
    Open,
    Style,
    Markdown,
    Remove,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_index_is_stable_and_dense() {
        assert_eq!(IconShape::Open.shader_index(), 0);
        assert_eq!(IconShape::Style.shader_index(), 1);
        assert_eq!(IconShape::Markdown.shader_index(), 2);
        assert_eq!(IconShape::Remove.shader_index(), 3);
    }

    #[test]
    fn glyph_accessor_only_returns_for_glyph_variant() {
        assert_eq!(Icon::Glyph('H').glyph(), Some('H'));
        assert_eq!(Icon::Shape(IconShape::Open).glyph(), None);
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p waml-editor shader_index_is_stable`
Expected: FAIL — `no method named shader_index found for enum IconShape`.

- [ ] **Step 4: Write the minimal `IconShape`/`Icon` methods**

Add, above the `#[cfg(test)]` block:

```rust
impl IconShape {
    /// The `shape` uniform value the `DrawIcon` shader switches on. Dense and
    /// stable -- do not renumber existing variants.
    pub fn shader_index(self) -> u32 {
        match self {
            IconShape::Open => 0,
            IconShape::Style => 1,
            IconShape::Markdown => 2,
            IconShape::Remove => 3,
        }
    }
}

impl Icon {
    /// The char for a `Glyph` icon (caller draws it with its own `DrawText`);
    /// `None` for `Shape` icons.
    pub fn glyph(&self) -> Option<char> {
        match self {
            Icon::Glyph(c) => Some(*c),
            Icon::Shape(_) => None,
        }
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p waml-editor shader_index_is_stable glyph_accessor_only`
Expected: PASS (2 passed).

- [ ] **Step 6: Add the `DrawIcon` shader + `draw_icon()`**

Append to `crates/waml-editor/src/icon.rs`:

```rust
script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas

    // One `DrawColor` whose `pixel()` switches on the `shape` uniform (set per
    // draw via `set_uniform(cx, live_id!(shape), &[idx as f32])`, the proven
    // `draw_node`/`zoom` pattern). `tint` arrives through the real `color`
    // field (accent for normal wedges, danger for Remove, dim for disabled).
    // Sharp shapes use `sdf.rect`/`sdf.circle` with a safe inner margin --
    // path strokes near the viewport edge degenerate silently (repo memory).
    // Branches use MPSL `if` (proven in the fork's draw_glyph pixel shader).
    mod.draw.DrawIcon = mod.draw.DrawColor{
        shape: uniform(0.0)
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let s = self.rect_size.x
            let m = s * 0.22
            if self.shape < 0.5 {
                // Open: a card outline with a corner fold (screenshot-tuned).
                sdf.rect(m, m, s - m * 2.0, s - m * 2.0)
                sdf.stroke(self.color, 1.5)
            } else if self.shape < 1.5 {
                // Style: a filled disc (swatch) -- tuned in the screenshot pass.
                sdf.circle(s * 0.5, s * 0.5, s * 0.24)
                sdf.fill(self.color)
            } else if self.shape < 2.5 {
                // Markdown: three stacked bars.
                sdf.rect(m, s * 0.34, s - m * 2.0, s * 0.06)
                sdf.fill(self.color)
                sdf.rect(m, s * 0.48, s - m * 2.0, s * 0.06)
                sdf.fill(self.color)
                sdf.rect(m, s * 0.62, s - m * 2.0, s * 0.06)
                sdf.fill(self.color)
            } else {
                // Remove: an X built from two short segments (kept off the edge).
                sdf.move_to(m, m)
                sdf.line_to(s - m, s - m)
                sdf.stroke(self.color, 1.8)
                sdf.move_to(s - m, m)
                sdf.line_to(m, s - m)
                sdf.stroke(self.color, 1.8)
            }
            return sdf.result
        }
    }
}

/// Draw a `Shape` icon into `rect`, tinted. `Glyph` icons are a no-op here --
/// the caller draws them with its own `DrawText` pen (spec: the placeholder
/// path stays valid). Returns `true` if this drew (i.e. it was a `Shape`).
pub fn draw_icon(cx: &mut Cx2d, draw: &mut DrawColor, rect: Rect, icon: &Icon, tint: Vec4) -> bool {
    match icon {
        Icon::Shape(shape) => {
            draw.set_uniform(cx, live_id!(shape), &[shape.shader_index() as f32]);
            draw.color = tint;
            draw.draw_abs(cx, rect);
            true
        }
        Icon::Glyph(_) => false,
    }
}
```

Note: `draw_icon` returns `bool` (drew-or-not); the spec's `-> ()` signature is widened so a caller can fall back to its `DrawText` glyph pen when `false`. Record this deviation in the commit body.

- [ ] **Step 7: Register the `script_mod` in `App`**

In `crates/waml-editor/src/app.rs`, inside `impl AppMain for App { fn script_mod(...) }` (`:610`), add after `crate::draw_hud::script_mod(vm);` (`:613`):

```rust
        crate::draw_hud::script_mod(vm);
        crate::icon::script_mod(vm);
```

- [ ] **Step 8: Verify it compiles + tests stay green**

Run: `cargo test -p waml-editor icon`
Expected: PASS (the two `icon` tests pass; crate compiles including the new shader + `draw_icon`).

- [ ] **Step 9: Commit**

```bash
git add crates/waml-editor/src/icon.rs crates/waml-editor/src/main.rs crates/waml-editor/src/app.rs
git commit -m "feat(editor): generic icon module (SDF DrawIcon + Icon/IconShape)"
```

---

### Task 2: `radial` geometry core (pure)

**Files:**
- Create: `crates/waml-editor/src/radial.rs`
- Modify: `crates/waml-editor/src/main.rs` (add `mod radial;` after `mod node_style;`)
- Test: inline `#[cfg(test)] mod tests` in `crates/waml-editor/src/radial.rs`

**Interfaces:**
- Consumes: `Icon` from Task 1.
- Produces:
  - `pub struct RadialItem { pub id: LiveId, pub label: String, pub icon: Icon, pub danger: bool, pub enabled: bool }`
  - `pub enum RadialOutcome { Committed(LiveId), Cancelled, None }`
  - `pub const HUB_RADIUS: f64 = 30.0;` `pub const DISC_RADIUS: f64 = 120.0;`
  - `pub fn wedge_index(center: DVec2, cursor: DVec2, n: usize) -> Option<usize>`
  - `pub fn resolve_target(items: &[RadialItem], center: DVec2, cursor: DVec2) -> Option<usize>`

- [ ] **Step 1: Add the module declaration**

In `crates/waml-editor/src/main.rs`, add `mod radial;` after `mod node_style;`:

```rust
mod node_style;
mod radial;
mod scene;
```

- [ ] **Step 2: Write the failing geometry tests**

Create `crates/waml-editor/src/radial.rs`:

```rust
//! Dynamic 2--6 wedge radial (marking) menu. Immediate-mode component: the
//! parent owns placement and drives it via inherent methods; it does not
//! self-route tree events (same convention as `waml_button`/`tool_dock`).
//!
//! `RadialCore` is the pure, GPU-free geometry + state machine (fully unit
//! tested). The `Radial` widget (Task 3) wraps it with the wedge shader and a
//! `NextFrame` animation loop.
//!
//! Geometry (Layout A): N sectors of 360/N deg, first wedge CENTRED at 12
//! o'clock proceeding clockwise. Fixed disc radius; central hub dead-zone is
//! the cancel target. Hit-test is by angle from centre, so screen-edge
//! clipping of the drawn disc never affects which wedge is pickable.

use crate::icon::Icon;
use makepad_widgets::*;

/// Central cancel zone / neutral origin radius (screen px).
pub const HUB_RADIUS: f64 = 30.0;
/// Disc (rim) radius (screen px).
pub const DISC_RADIUS: f64 = 120.0;

/// One wedge. The radial owns no command semantics -- it reports `id` back on
/// commit and the parent maps it.
#[derive(Clone, Debug)]
pub struct RadialItem {
    pub id: LiveId,
    pub label: String,
    pub icon: Icon,
    /// Danger-token hue across all wedge states.
    pub danger: bool,
    /// `false` = greyed, holds its slot, cannot arm or commit.
    pub enabled: bool,
}

/// What the radial reports to its parent.
#[derive(Clone, Debug, PartialEq)]
pub enum RadialOutcome {
    Committed(LiveId),
    Cancelled,
    None,
}

/// Wedge index under `cursor`, or `None` inside the hub dead-zone. Angle is
/// measured clockwise from 12 o'clock; the first wedge (index 0) is centred on
/// 12 o'clock. Pure geometry -- ignores enabled/disabled (see `resolve_target`).
pub fn wedge_index(center: DVec2, cursor: DVec2, n: usize) -> Option<usize> {
    if n == 0 {
        return None;
    }
    let dx = cursor.x - center.x;
    let dy = cursor.y - center.y;
    let r = (dx * dx + dy * dy).sqrt();
    if r < HUB_RADIUS {
        return None;
    }
    // atan2(dx, -dy): up=0, right=+90, down=+180, left=-90 -> clockwise from 12.
    let deg = dx.atan2(-dy).to_degrees().rem_euclid(360.0);
    let sector = 360.0 / n as f64;
    // First wedge centred on 0 deg => its span is [-sector/2, +sector/2).
    let shifted = (deg + sector * 0.5).rem_euclid(360.0);
    let idx = (shifted / sector).floor() as usize;
    Some(idx.min(n - 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::icon::{Icon, IconShape};

    fn item(id: LiveId, enabled: bool) -> RadialItem {
        RadialItem {
            id,
            label: "x".into(),
            icon: Icon::Shape(IconShape::Open),
            danger: false,
            enabled,
        }
    }

    const C: DVec2 = DVec2 { x: 500.0, y: 400.0 };

    // Points at radius 100 (outside hub 30, inside disc 120) in the four
    // cardinal screen directions.
    fn up() -> DVec2 { dvec2(C.x, C.y - 100.0) }
    fn right() -> DVec2 { dvec2(C.x + 100.0, C.y) }
    fn down() -> DVec2 { dvec2(C.x, C.y + 100.0) }
    fn left() -> DVec2 { dvec2(C.x - 100.0, C.y) }

    #[test]
    fn n4_cardinal_directions_map_clockwise_from_twelve() {
        assert_eq!(wedge_index(C, up(), 4), Some(0));
        assert_eq!(wedge_index(C, right(), 4), Some(1));
        assert_eq!(wedge_index(C, down(), 4), Some(2));
        assert_eq!(wedge_index(C, left(), 4), Some(3));
    }

    #[test]
    fn n2_splits_top_and_bottom() {
        assert_eq!(wedge_index(C, up(), 2), Some(0));
        assert_eq!(wedge_index(C, down(), 2), Some(1));
    }

    #[test]
    fn n3_first_wedge_centred_on_twelve() {
        assert_eq!(wedge_index(C, up(), 3), Some(0));
        // 120 deg clockwise (down-right) -> wedge 1; 240 (down-left) -> wedge 2.
        let dr = dvec2(C.x + 86.6, C.y + 50.0);
        let dl = dvec2(C.x - 86.6, C.y + 50.0);
        assert_eq!(wedge_index(C, dr, 3), Some(1));
        assert_eq!(wedge_index(C, dl, 3), Some(2));
    }

    #[test]
    fn n5_and_n6_stay_in_range() {
        for p in [up(), right(), down(), left()] {
            assert!(wedge_index(C, p, 5).unwrap() < 5);
            assert!(wedge_index(C, p, 6).unwrap() < 6);
        }
        assert_eq!(wedge_index(C, up(), 6), Some(0));
    }

    #[test]
    fn hub_dead_zone_returns_none() {
        assert_eq!(wedge_index(C, C, 4), None);
        assert_eq!(wedge_index(C, dvec2(C.x + 10.0, C.y), 4), None); // r=10 < 30
    }

    #[test]
    fn wrap_around_at_twelve_oclock_stays_in_wedge_zero() {
        // Just clockwise of 12 (deg~5) and just anti-clockwise (deg~355) both
        // fall in wedge 0 for N=4 (span -45..45).
        let just_cw = dvec2(C.x + 8.7, C.y - 99.6); // ~5 deg
        let just_ccw = dvec2(C.x - 8.7, C.y - 99.6); // ~355 deg
        assert_eq!(wedge_index(C, just_cw, 4), Some(0));
        assert_eq!(wedge_index(C, just_ccw, 4), Some(0));
    }

    #[test]
    fn disabled_wedge_resolves_to_none() {
        let items = vec![item(live_id!(a), true), item(live_id!(b), false)];
        // `right()` is wedge 1 for N=2? No -- N=2 top/bottom. Use down() = wedge 1.
        assert_eq!(resolve_target(&items, C, down()), None); // wedge 1 disabled
        assert_eq!(resolve_target(&items, C, up()), Some(0)); // wedge 0 enabled
    }

    #[test]
    fn resolve_target_none_in_hub() {
        let items = vec![item(live_id!(a), true), item(live_id!(b), true)];
        assert_eq!(resolve_target(&items, C, C), None);
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p waml-editor radial`
Expected: FAIL — `cannot find function resolve_target in this scope` (the geometry tests reference it before it exists).

- [ ] **Step 4: Add `resolve_target`**

Add above the `#[cfg(test)]` block:

```rust
/// Wedge index under `cursor` that is actually actionable: `None` in the hub
/// dead-zone OR over a disabled wedge (spec: a disabled wedge is treated like
/// the dead-zone -- arms nothing).
pub fn resolve_target(items: &[RadialItem], center: DVec2, cursor: DVec2) -> Option<usize> {
    let idx = wedge_index(center, cursor, items.len())?;
    if items[idx].enabled {
        Some(idx)
    } else {
        None
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p waml-editor radial`
Expected: PASS (all 8 geometry tests green).

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/radial.rs crates/waml-editor/src/main.rs
git commit -m "feat(editor): radial geometry core (angle->wedge, hub dead-zone, disabled)"
```

---

### Task 3: `radial` state machine + `Radial` widget

**Files:**
- Modify: `crates/waml-editor/src/radial.rs` (add `RadialCore`, its state methods + tests, and the `Radial` widget with shader + animation)
- Modify: `crates/waml-editor/src/app.rs:613` (register `crate::radial::script_mod(vm);`)
- Test: inline `#[cfg(test)] mod tests` in `crates/waml-editor/src/radial.rs`

**Interfaces:**
- Consumes: `wedge_index`, `resolve_target`, `RadialItem`, `RadialOutcome`, `HUB_RADIUS`, `DISC_RADIUS` (Task 2); `draw_icon`, `Icon` (Task 1); `NextFrame` (makepad prelude).
- Produces:
  - `pub struct RadialCore` with pure methods:
    - `fn begin(&mut self, center: DVec2, items: Vec<RadialItem>)`
    - `fn pointer_move(&mut self, cursor: DVec2)`
    - `fn release(&mut self, cursor: DVec2) -> RadialOutcome`
    - `fn click(&mut self, cursor: DVec2) -> RadialOutcome`
    - `fn esc(&mut self) -> RadialOutcome`
    - `fn is_open(&self) -> bool`
  - `pub struct Radial` widget: `open(cx, center, items, time)`, `handle(cx, event) -> RadialOutcome`, `tick(cx, event)`, `draw(cx2d)`, `is_open()`.
  - `pub const DRAG_THRESHOLD: f64 = 12.0;`

- [ ] **Step 1: Write the failing state-machine tests**

Add these tests inside the existing `#[cfg(test)] mod tests` block in `radial.rs` (after the geometry tests):

```rust
    fn menu() -> Vec<RadialItem> {
        // N=4: wedge 0 up, 1 right, 2 down, 3 left. Wedge 2 disabled.
        vec![
            item(live_id!(open), true),
            item(live_id!(style), true),
            item(live_id!(markdown), false), // disabled
            item(live_id!(remove), true),
        ]
    }

    #[test]
    fn tap_opens_persistent_popup_then_click_commits() {
        let mut c = RadialCore::default();
        c.begin(C, menu());
        // Release without moving = tap -> popup, stays open, no outcome yet.
        assert_eq!(c.release(C), RadialOutcome::None);
        assert!(c.is_open());
        // Subsequent click on wedge 1 (right, enabled) commits its id.
        assert_eq!(c.click(right()), RadialOutcome::Committed(live_id!(style)));
        assert!(!c.is_open());
    }

    #[test]
    fn hold_drag_arms_then_release_commits() {
        let mut c = RadialCore::default();
        c.begin(C, menu());
        c.pointer_move(right()); // drag past threshold -> marking, arms wedge 1
        assert_eq!(c.armed, Some(1));
        assert_eq!(c.release(right()), RadialOutcome::Committed(live_id!(style)));
        assert!(!c.is_open());
    }

    #[test]
    fn flick_past_rim_commits_and_flags_flick() {
        let mut c = RadialCore::default();
        c.begin(C, menu());
        let far_right = dvec2(C.x + 160.0, C.y); // r=160 > DISC_RADIUS
        c.pointer_move(far_right);
        assert!(c.flick);
        assert_eq!(c.release(far_right), RadialOutcome::Committed(live_id!(style)));
    }

    #[test]
    fn popup_click_on_hub_cancels() {
        let mut c = RadialCore::default();
        c.begin(C, menu());
        c.release(C); // -> popup
        assert_eq!(c.click(C), RadialOutcome::Cancelled);
        assert!(!c.is_open());
    }

    #[test]
    fn popup_click_outside_disc_cancels() {
        let mut c = RadialCore::default();
        c.begin(C, menu());
        c.release(C); // -> popup
        let outside = dvec2(C.x + 300.0, C.y);
        assert_eq!(c.click(outside), RadialOutcome::Cancelled);
    }

    #[test]
    fn esc_cancels() {
        let mut c = RadialCore::default();
        c.begin(C, menu());
        assert_eq!(c.esc(), RadialOutcome::Cancelled);
        assert!(!c.is_open());
    }

    #[test]
    fn marking_release_in_hub_cancels() {
        let mut c = RadialCore::default();
        c.begin(C, menu());
        c.pointer_move(right()); // establishes marking mode (dragged)
        assert_eq!(c.release(C), RadialOutcome::Cancelled); // released in hub
    }

    #[test]
    fn popup_click_on_disabled_wedge_is_noop_and_stays_open() {
        let mut c = RadialCore::default();
        c.begin(C, menu());
        c.release(C); // -> popup
        assert_eq!(c.click(down()), RadialOutcome::None); // wedge 2 disabled
        assert!(c.is_open());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p waml-editor radial`
Expected: FAIL — `cannot find type RadialCore in this scope`.

- [ ] **Step 3: Implement `RadialCore`**

Add to `radial.rs`, above the `#[cfg(test)]` block:

```rust
/// Minimum drag (screen px) before a right-press is treated as a marking
/// gesture rather than a tap.
pub const DRAG_THRESHOLD: f64 = 12.0;

/// Pure, GPU-free radial state. `Default` = closed. The `Radial` widget owns
/// one of these and forwards translated pointer input into these methods; the
/// unit tests drive them directly.
#[derive(Default)]
pub struct RadialCore {
    open: bool,
    center: DVec2,
    items: Vec<RadialItem>,
    /// Right button currently held (marking candidate).
    pressed: bool,
    /// Passed the drag threshold -> committed to marking mode.
    dragged: bool,
    /// Released as a tap -> persistent popup mode.
    popup: bool,
    /// Wedge currently armed/hovered (resolved, so never a disabled index).
    pub armed: Option<usize>,
    /// Cursor rode past the rim over an armed wedge.
    pub flick: bool,
    press_pos: DVec2,
}

impl RadialCore {
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Items snapshot (widget reads this to draw).
    pub fn items(&self) -> &[RadialItem] {
        &self.items
    }

    pub fn center(&self) -> DVec2 {
        self.center
    }

    /// Open at `center` with `items` (the press point == center == marking
    /// origin). Right button is now held.
    pub fn begin(&mut self, center: DVec2, items: Vec<RadialItem>) {
        self.open = true;
        self.center = center;
        self.items = items;
        self.pressed = true;
        self.dragged = false;
        self.popup = false;
        self.armed = None;
        self.flick = false;
        self.press_pos = center;
    }

    /// Pointer moved to `cursor`. Updates armed wedge (both popup hover and
    /// marking arm), promotes to marking once past `DRAG_THRESHOLD`, and flags
    /// a flick when riding past the rim over an armed wedge.
    pub fn pointer_move(&mut self, cursor: DVec2) {
        if self.pressed && !self.dragged {
            let moved = (cursor - self.press_pos).length();
            if moved > DRAG_THRESHOLD {
                self.dragged = true;
            }
        }
        self.armed = resolve_target(&self.items, self.center, cursor);
        let r = (cursor - self.center).length();
        self.flick = self.pressed && self.dragged && self.armed.is_some() && r > DISC_RADIUS;
    }

    /// Right button released at `cursor`. A tap (no drag) enters persistent
    /// popup mode (stays open, no outcome). A marking release commits over an
    /// armed wedge, or cancels in the hub / over a disabled slot.
    pub fn release(&mut self, cursor: DVec2) -> RadialOutcome {
        if !self.dragged {
            self.pressed = false;
            self.popup = true;
            return RadialOutcome::None;
        }
        self.pressed = false;
        let r = (cursor - self.center).length();
        if r < HUB_RADIUS {
            self.close();
            return RadialOutcome::Cancelled;
        }
        match resolve_target(&self.items, self.center, cursor) {
            Some(i) => {
                let id = self.items[i].id;
                self.close();
                RadialOutcome::Committed(id)
            }
            None => {
                self.close();
                RadialOutcome::Cancelled
            }
        }
    }

    /// A click while in persistent popup mode. Hub or outside-disc cancels; an
    /// enabled wedge commits; a disabled wedge is a no-op that leaves the
    /// radial open.
    pub fn click(&mut self, cursor: DVec2) -> RadialOutcome {
        let r = (cursor - self.center).length();
        if r < HUB_RADIUS || r > DISC_RADIUS {
            self.close();
            return RadialOutcome::Cancelled;
        }
        match resolve_target(&self.items, self.center, cursor) {
            Some(i) => {
                let id = self.items[i].id;
                self.close();
                RadialOutcome::Committed(id)
            }
            None => RadialOutcome::None, // disabled wedge: no-op, stay open
        }
    }

    /// `Esc` cancels an open radial.
    pub fn esc(&mut self) -> RadialOutcome {
        if self.open {
            self.close();
            RadialOutcome::Cancelled
        } else {
            RadialOutcome::None
        }
    }

    fn close(&mut self) {
        self.open = false;
        self.pressed = false;
        self.dragged = false;
        self.popup = false;
        self.armed = None;
        self.flick = false;
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p waml-editor radial`
Expected: PASS (8 geometry + 8 state-machine tests green).

- [ ] **Step 5: Commit the core**

```bash
git add crates/waml-editor/src/radial.rs
git commit -m "feat(editor): radial state machine (tap/hold-drag/flick + cancel paths)"
```

- [ ] **Step 6: Add the wedge shader**

Append to `radial.rs` a `script_mod!` block. The stroke reuses `HudFrame`'s proven 150° fade recipe (`draw_hud.rs:45-48`) inline (see reviewer flag 4). `state`/`danger`/`enabled` are per-wedge uniforms set via `set_uniform`:

```rust
script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.text.*

    // One `DrawColor` per wedge, drawn with `draw_abs` (N per frame). `pixel()`
    // renders the pie-sector fill + per-slice rim arc + two spokes. Fill alpha
    // ramps by `state` (0 rest / 1 hover / 2 arm / 3 flick); `danger` swaps the
    // accent hue to the danger token; `enabled`=0 forces the flat grey disabled
    // look. `a0`/`a1` are the wedge's start/end angles (radians, set per draw);
    // `cx`/`cy`/`hub`/`rim` are the disc geometry in this quad's local px.
    mod.draw.RadialWedge = mod.draw.DrawColor{
        accent: uniform(atlas.accent)
        danger_col: uniform(atlas.danger)
        dim_col: uniform(atlas.text_dim)
        border_hi: uniform(atlas.frame_hi)
        border_lo: uniform(atlas.frame_lo)
        state: uniform(0.0)
        danger: uniform(0.0)
        enabled: uniform(1.0)
        cx: uniform(0.0)
        cy: uniform(0.0)
        hub: uniform(30.0)
        rim: uniform(120.0)
        a0: uniform(0.0)
        a1: uniform(1.5707963)
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let p = self.pos * self.rect_size
            let d = vec2(p.x - self.cx, p.y - self.cy)
            let r = length(d)
            // Angle clockwise from 12 o'clock (matches Rust `wedge_index`).
            let ang = mod(atan(d.x, -d.y) + 6.2831853, 6.2831853)
            let in_ring = step(self.hub, r) * (1.0 - step(self.rim, r))
            let in_wedge = step(self.a0, ang) * (1.0 - step(self.a1, ang))
            let mask = in_ring * in_wedge
            // Fill alpha ramp: rest .05 / hover .15 / arm .18 / flick .28.
            let rest = 0.05
            let hov = mix(rest, 0.15, clamp(self.state, 0.0, 1.0))
            let arm = mix(hov, 0.18, clamp(self.state - 1.0, 0.0, 1.0))
            let flick_a = mix(arm, 0.28, clamp(self.state - 2.0, 0.0, 1.0))
            let hue = mix(self.accent, self.danger_col, self.danger)
            let live_fill = vec4(hue.x, hue.y, hue.z, flick_a * mask)
            // Disabled: flat grey, no ramp.
            let dis_fill = vec4(self.dim_col.x, self.dim_col.y, self.dim_col.z, 0.06 * mask)
            let fill = mix(dis_fill, live_fill, self.enabled)
            sdf.clear(fill)
            // Spokes + rim arc: the source-bright 150deg fade (HudFrame recipe).
            let dir = vec2(0.5, 0.8660254)
            let span = 1.3660254
            let t = clamp((self.pos.x * dir.x + self.pos.y * dir.y) / span, 0.0, 1.0)
            let stroke = mix(self.border_hi, self.border_lo, t)
            // Rim arc for this slice.
            sdf.arc(self.cx, self.cy, self.rim, self.a0, self.a1)
            sdf.stroke(stroke, 1.2)
            return sdf.result
        }
    }
}
```

Note: `sdf.arc` / `mod` / `atan` and the exact spoke geometry are screenshot-tuned in Step 9 (repo memory: SDF ops degenerate silently and are verified visually, not by `cargo build`). If `sdf.arc` is absent in the fork's `sdf.rs`, draw the rim as a thin `sdf.circle` outline clipped by the wedge mask instead; confirm against `draw/src/shader/sdf.rs` during the screenshot pass.

**Known wrap bug to fix in the screenshot pass:** wedge 0's angular span wraps zero (`a0 ≈ 315°`, `a1 ≈ 45°` after `rem_euclid`), so the naive `in_wedge = step(a0,ang) * (1 - step(a1,ang))` renders wedge 0 EMPTY. Replace it with a wrap-aware test: when `a0 > a1`, the slice is `ang >= a0 || ang < a1` (OR across the seam); otherwise `ang >= a0 && ang < a1`. This is visual-only (won't fail `cargo test`) but WILL blank the top wedge until fixed.

- [ ] **Step 7: Add the `Radial` widget (event-passive, `NextFrame`-driven)**

Add to `radial.rs`:

```rust
// Bloom-in duration on open (seconds).
const BLOOM_SECS: f64 = 0.12;

#[derive(Script, ScriptHook, Widget)]
pub struct Radial {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    #[redraw]
    #[live]
    draw_wedge: DrawColor,
    #[redraw]
    #[live]
    draw_hub: DrawColor,
    #[redraw]
    #[live]
    draw_icon: DrawColor,
    #[redraw]
    #[live]
    draw_label: DrawText,

    #[rust]
    core: RadialCore,
    #[rust]
    start: f64,
    #[rust]
    next_frame: NextFrame,
}

impl Widget for Radial {
    // Event-passive: the parent (`App`) drives this through the inherent methods
    // below, so a stray tree route can never double-handle a gesture.
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {}

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, _walk: Walk) -> DrawStep {
        self.draw(cx);
        DrawStep::done()
    }
}

impl Radial {
    pub fn is_open(&self) -> bool {
        self.core.is_open()
    }

    /// Open at `center` (the right-press point) with `items`; starts the
    /// bloom-in animation loop.
    pub fn open(&mut self, cx: &mut Cx, center: DVec2, items: Vec<RadialItem>, time: f64) {
        self.core.begin(center, items);
        self.start = time;
        self.next_frame = cx.new_next_frame();
        self.draw_wedge.redraw(cx);
    }

    /// Advance the bloom animation on our scheduled next frame.
    pub fn tick(&mut self, cx: &mut Cx, event: &Event) {
        if self.next_frame.is_event(event).is_some() && self.core.is_open() {
            self.next_frame = cx.new_next_frame();
            self.draw_wedge.redraw(cx);
        }
    }

    /// Translate an `Event` into the pure state machine and return the outcome.
    /// The parent calls this each event while the radial is open, then acts on
    /// a `Committed`/`Cancelled`. `None` means "still open, nothing to do".
    pub fn handle(&mut self, cx: &mut Cx, event: &Event) -> RadialOutcome {
        if !self.core.is_open() {
            return RadialOutcome::None;
        }
        self.tick(cx, event);
        let outcome = match event {
            Event::MouseMove(e) => {
                self.core.pointer_move(e.abs);
                self.draw_wedge.redraw(cx);
                RadialOutcome::None
            }
            Event::MouseUp(e) if e.button.is_secondary() => self.core.release(e.abs),
            // In popup mode a subsequent PRIMARY click selects a wedge.
            Event::MouseDown(e) if e.button.is_primary() => self.core.click(e.abs),
            Event::KeyDown(ke) if ke.key_code == KeyCode::Escape => self.core.esc(),
            _ => RadialOutcome::None,
        };
        if outcome != RadialOutcome::None {
            self.draw_wedge.redraw(cx);
        }
        outcome
    }

    /// Draw the disc at the stored center. N wedges via `draw_abs`, then hub,
    /// then each wedge's icon + label. Called from `draw_walk` / the parent's
    /// draw pass.
    pub fn draw(&mut self, cx: &mut Cx2d) {
        if !self.core.is_open() {
            return;
        }
        let center = self.core.center();
        let n = self.core.items().len();
        if n == 0 {
            return;
        }
        let sector = std::f64::consts::TAU / n as f64;
        // Quad bounding the whole disc; every wedge shader shares it and masks
        // its own slice, so hit geometry is independent of this quad.
        let quad = Rect {
            pos: dvec2(center.x - DISC_RADIUS, center.y - DISC_RADIUS),
            size: dvec2(DISC_RADIUS * 2.0, DISC_RADIUS * 2.0),
        };
        let local_c = dvec2(DISC_RADIUS, DISC_RADIUS); // center within the quad
        let items = self.core.items().to_vec();
        let armed = self.core.armed;
        for (i, it) in items.iter().enumerate() {
            // Slice angles clockwise from 12, first wedge centred on 12.
            let a0 = (i as f64) * sector - sector * 0.5;
            let a1 = a0 + sector;
            let state = if !it.enabled {
                0.0
            } else if self.core.flick && armed == Some(i) {
                3.0
            } else if armed == Some(i) {
                2.0
            } else {
                0.0
            };
            self.draw_wedge.set_uniform(cx, live_id!(cx), &[local_c.x as f32]);
            self.draw_wedge.set_uniform(cx, live_id!(cy), &[local_c.y as f32]);
            self.draw_wedge.set_uniform(cx, live_id!(hub), &[HUB_RADIUS as f32]);
            self.draw_wedge.set_uniform(cx, live_id!(rim), &[DISC_RADIUS as f32]);
            self.draw_wedge.set_uniform(cx, live_id!(a0), &[a0.rem_euclid(std::f64::consts::TAU) as f32]);
            self.draw_wedge.set_uniform(cx, live_id!(a1), &[a1.rem_euclid(std::f64::consts::TAU) as f32]);
            self.draw_wedge.set_uniform(cx, live_id!(state), &[state as f32]);
            self.draw_wedge.set_uniform(cx, live_id!(danger), &[if it.danger { 1.0 } else { 0.0 }]);
            self.draw_wedge.set_uniform(cx, live_id!(enabled), &[if it.enabled { 1.0 } else { 0.0 }]);
            self.draw_wedge.draw_abs(cx, quad);

            // Icon + label centred on the sector mid-angle at a fixed radius.
            let mid = (i as f64) * sector; // mid-angle clockwise from 12
            let icon_r = (HUB_RADIUS + DISC_RADIUS) * 0.5;
            let ix = center.x + icon_r * mid.sin();
            let iy = center.y - icon_r * mid.cos();
            let icon_rect = Rect {
                pos: dvec2(ix - 12.0, iy - 12.0),
                size: dvec2(24.0, 24.0),
            };
            let tint = if !it.enabled {
                vec4(0.54, 0.59, 0.65, 1.0) // atlas.text_dim
            } else if it.danger {
                vec4(0.922, 0.275, 0.471, 1.0) // atlas.danger
            } else {
                vec4(0.078, 0.588, 0.863, 1.0) // atlas.accent
            };
            if !crate::icon::draw_icon(cx, &mut self.draw_icon, icon_rect, &it.icon, tint) {
                if let Some(g) = it.icon.glyph() {
                    self.draw_label
                        .draw_abs(cx, dvec2(ix - 4.0, iy - 8.0), &g.to_string());
                }
            }
            self.draw_label
                .draw_abs(cx, dvec2(ix - 16.0, iy + 14.0), &it.label);
        }
        // Hub: white fill + accent ring drawn as a small quad.
        let hub_rect = Rect {
            pos: dvec2(center.x - HUB_RADIUS, center.y - HUB_RADIUS),
            size: dvec2(HUB_RADIUS * 2.0, HUB_RADIUS * 2.0),
        };
        self.draw_hub.draw_abs(cx, hub_rect);
    }
}
```

Note: the RGBA fallbacks in `tint` mirror the Atlas tokens (`accent #x1496dc`, `danger #xeb4678`, `text_dim #x8a97a6`) numerically because a free `draw_abs` icon needs a concrete `Vec4`; if the reviewer prefers, thread the token values in from the DSL as uniforms during the Task 5 rename. Record in the commit body.

**Rendering-state scope (be honest at archive time):** this `draw()` realizes the wedge `rest`/`arm`/`flick` states, the icon+label, and a plain hub. Several spec §Rendering/Animation states are deliberately deferred to the Step 9 / Task 4 **screenshot-tuning** pass and are NOT done when the code compiles green: popup **hover** fill (`.15`, state=1 — currently folded into `arm`), the marking **recede** of passed-over wedges (`dim`/`gone` by distance-to-rim), the **bloom-in** scale/opacity (`start`/`BLOOM_SECS` are stored but not yet applied in `draw`), the **hub** accent ring + grey ✕, and the frosted disc fill / drop-shadow / accent bloom backing. Add these during screenshot tuning; do not mark the spec's rendering section fully delivered until they render.

- [ ] **Step 8: Add the widget's DSL default + register `script_mod`**

Extend the `script_mod!` block in `radial.rs` (add a `mod.widgets.Radial` default like `tool_dock.rs:17-19`):

```rust
    mod.widgets.RadialBase = #(Radial::register_widget(vm))

    mod.widgets.Radial = set_type_default() do mod.widgets.RadialBase{
        width: Fill
        height: Fill
        draw_wedge: mod.draw.RadialWedge{ color: #x00000000 }
        draw_hub +: { color: atlas.field_bg }
        draw_icon: mod.draw.DrawIcon{ color: atlas.accent }
        draw_label +: {
            color: atlas.text
            text_style: theme.font_regular{ font_size: 10 line_spacing: 1.2 }
        }
    }
```

Then in `crates/waml-editor/src/app.rs` `fn script_mod` (`:610`), add after the `icon` line from Task 1:

```rust
        crate::icon::script_mod(vm);
        crate::radial::script_mod(vm);
```

- [ ] **Step 9: Verify it compiles + core tests stay green, then screenshot-tune the shader**

Run: `cargo test -p waml-editor radial`
Expected: PASS (16 tests). The crate compiles including the `Radial` widget + shaders.

Then verify visually (shaders compile at GPU-runtime, not `cargo build`): follow the repo self-screenshot recipe (repo memory `makepad-fork-shader-gotchas`): `./scripts/run-native.ps1` (launches on `tests/fixtures/mini`), capture with `PrintWindow(hwnd, hdc, 2)` at native resolution to a `C:/…` path, and inspect the app stdout for `[E] …radial.rs:LINE` shader errors. (The radial only appears once Task 4 wires the open trigger — this step confirms the crate launches and logs no shader-compile error from the newly-registered `RadialWedge`/`DrawIcon`. Full visual sign-off of each N and each state is in Task 4.)

- [ ] **Step 10: Commit**

```bash
git add crates/waml-editor/src/radial.rs crates/waml-editor/src/app.rs
git commit -m "feat(editor): Radial widget (wedge shader + NextFrame bloom, event-passive)"
```

---

### Task 4: Node radial wiring on the canvas

**Files:**
- Modify: `crates/waml-editor/src/canvas.rs` (right-press detection, `node_at()`, `GraphCanvasAction`)
- Modify: `crates/waml-editor/src/app.rs` (place `Radial` overlay in DSL tree; drive it in `handle_actions`/`handle_event`; `NodeCommand` mapping + stub handlers)
- Test: inline `#[cfg(test)] mod tests` in both files

**Interfaces:**
- Consumes: `Radial::open`/`handle`/`is_open`, `RadialItem`, `RadialOutcome`, `Icon`/`IconShape` (Tasks 1–3); `Camera::world_to_local`.
- Produces:
  - `pub fn node_at(node_rects: &[waml::solve::Rect], camera: &Camera, view: Rect, abs: DVec2) -> Option<usize>` in `canvas.rs`
  - `pub enum GraphCanvasAction { None, NodeMenu { abs: DVec2, node: usize } }` in `canvas.rs`; reader `pub fn canvas_action(&self, actions: &Actions) -> Option<GraphCanvasAction>`
  - `pub enum NodeCommand { Open, Style, Markdown, Remove }` + `pub fn node_command_for(id: LiveId) -> Option<NodeCommand>` in `canvas.rs`

- [ ] **Step 1: Write the failing `node_at` + `node_command_for` tests**

In `crates/waml-editor/src/canvas.rs`, add to the existing `#[cfg(test)] mod tests` (`:474`):

```rust
    #[test]
    fn node_at_hits_the_topmost_node_under_the_point() {
        let rects = vec![
            WorldRect { x: 0.0, y: 0.0, w: 100.0, h: 60.0 },
            WorldRect { x: 200.0, y: 0.0, w: 100.0, h: 60.0 },
        ];
        let camera = Camera { pan_x: 0.0, pan_y: 0.0, zoom: 1.0 };
        let view = Rect { pos: dvec2(0.0, 0.0), size: dvec2(800.0, 600.0) };
        assert_eq!(node_at(&rects, &camera, view, dvec2(50.0, 30.0)), Some(0));
        assert_eq!(node_at(&rects, &camera, view, dvec2(250.0, 30.0)), Some(1));
        assert_eq!(node_at(&rects, &camera, view, dvec2(150.0, 30.0)), None);
    }

    #[test]
    fn node_command_maps_the_four_committed_ids() {
        assert_eq!(node_command_for(live_id!(open)), Some(NodeCommand::Open));
        assert_eq!(node_command_for(live_id!(style)), Some(NodeCommand::Style));
        assert_eq!(node_command_for(live_id!(markdown)), Some(NodeCommand::Markdown));
        assert_eq!(node_command_for(live_id!(remove)), Some(NodeCommand::Remove));
        assert_eq!(node_command_for(live_id!(bogus)), None);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p waml-editor node_at node_command`
Expected: FAIL — `cannot find function node_at` / `cannot find function node_command_for`.

- [ ] **Step 3: Implement `node_at`, `NodeCommand`, `node_command_for`, `GraphCanvasAction`**

Add to `canvas.rs` (near `border_point`, outside the `impl Widget`):

```rust
/// Index of the topmost node whose on-screen rect contains `abs`, or `None`.
/// Topmost = last-drawn, so we scan in reverse. Pure (takes world rects +
/// camera), matching the draw-time transform in `draw_walk`.
pub fn node_at(
    node_rects: &[waml::solve::Rect],
    camera: &Camera,
    view: Rect,
    abs: DVec2,
) -> Option<usize> {
    for (i, nr) in node_rects.iter().enumerate().rev() {
        let (lx, ly) = camera.world_to_local(nr.x, nr.y);
        let screen = Rect {
            pos: dvec2(view.pos.x + lx, view.pos.y + ly),
            size: dvec2(nr.w * camera.zoom, nr.h * camera.zoom),
        };
        if screen.contains(abs) {
            return Some(i);
        }
    }
    None
}

/// The four node commands a radial reports. Handlers are logging stubs for now
/// (there is no node-editing command path yet -- mirrors the `tool_dock` mock).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeCommand {
    Open,
    Style,
    Markdown,
    Remove,
}

/// Map a radial-committed `LiveId` to a node command. `None` = not one of ours.
pub fn node_command_for(id: LiveId) -> Option<NodeCommand> {
    if id == live_id!(open) {
        Some(NodeCommand::Open)
    } else if id == live_id!(style) {
        Some(NodeCommand::Style)
    } else if id == live_id!(markdown) {
        Some(NodeCommand::Markdown)
    } else if id == live_id!(remove) {
        Some(NodeCommand::Remove)
    } else {
        None
    }
}

/// Canvas -> App action (same convention as `ToolDockAction`).
#[derive(Clone, Debug, Default)]
pub enum GraphCanvasAction {
    #[default]
    None,
    /// A right-press landed on a node: open the radial at `abs` for `node`.
    NodeMenu { abs: DVec2, node: usize },
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p waml-editor node_at node_command`
Expected: PASS.

- [ ] **Step 5: Emit `NodeMenu` on right-press in `GraphCanvas::handle_event`**

In `canvas.rs` `handle_event` (`:226`), add a right-press branch. The node rects come from `self.scene`. Add to the `match` inside `handle_event`, before the existing `Hit::FingerDown` primary branch. Use the real `widget_action` signature already used by `tool_dock.rs:208` (`cx.widget_action(uid, action)`):

```rust
            Hit::FingerDown(fe) if fe.mouse_button() == Some(MouseButton::SECONDARY) => {
                let rects: Vec<waml::solve::Rect> =
                    self.scene.nodes.iter().map(|n| n.rect).collect();
                if let Some(node) = node_at(&rects, &self.camera, self.view_rect, fe.abs) {
                    let uid = self.widget_uid();
                    cx.widget_action(uid, GraphCanvasAction::NodeMenu { abs: fe.abs, node });
                }
            }
```

Add the reader method to `impl GraphCanvas` (near `node_count`, `:463`):

```rust
    /// Convenience reader for `App` (mirrors `ToolDock::dock_action`).
    pub fn canvas_action(&self, actions: &Actions) -> Option<GraphCanvasAction> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.cast() {
            GraphCanvasAction::None => None,
            action => Some(action),
        }
    }
```

- [ ] **Step 6: Place the `Radial` overlay in the App DSL tree**

In `crates/waml-editor/src/app.rs`, `script_mod!` `startup()` block, add `use mod.widgets.Radial` next to the other `use mod.widgets.*` lines (`:11-19`), and add a `radial` overlay as the last child of the root `Window`/overlay stack — placed like `shortcuts_overlay` so it draws on top of the canvas. Add near the other overlay widgets:

```rust
                radial := Radial{}
```

(Match the existing overlay placement idiom used for `shortcuts_overlay`; the widget fills the window and draws only when open, at its stored center.)

- [ ] **Step 7: Drive the radial from `App`**

In `crates/waml-editor/src/app.rs` `handle_actions` (`:455`), add a branch that opens the radial on `NodeMenu` (after the `dock_action` block, `:536`):

```rust
        let canvas_menu = self
            .ui
            .widget(cx, ids!(canvas))
            .borrow_mut::<crate::canvas::GraphCanvas>()
            .and_then(|c| c.canvas_action(actions));
        if let Some(crate::canvas::GraphCanvasAction::NodeMenu { abs, node: _ }) = canvas_menu {
            let items = crate::app::node_radial_items();
            if let Some(mut radial) = self
                .ui
                .widget(cx, ids!(radial))
                .borrow_mut::<crate::radial::Radial>()
            {
                radial.open(cx, abs, items, cx.seconds_since_app_start());
            }
            return;
        }
```

Add the item-list builder + outcome mapping as free/`impl App` items in `app.rs`:

```rust
/// The four node-radial commands (Remove = danger). Ids are what `Radial`
/// reports on commit; `node_command_for` maps them back.
pub fn node_radial_items() -> Vec<crate::radial::RadialItem> {
    use crate::icon::{Icon, IconShape};
    use crate::radial::RadialItem;
    vec![
        RadialItem { id: live_id!(open), label: "Open".into(), icon: Icon::Shape(IconShape::Open), danger: false, enabled: true },
        RadialItem { id: live_id!(style), label: "Style".into(), icon: Icon::Shape(IconShape::Style), danger: false, enabled: true },
        RadialItem { id: live_id!(markdown), label: "Markdown".into(), icon: Icon::Shape(IconShape::Markdown), danger: false, enabled: true },
        RadialItem { id: live_id!(remove), label: "Remove".into(), icon: Icon::Shape(IconShape::Remove), danger: true, enabled: true },
    ]
}
```

Note: `cx.seconds_since_app_start() -> f64` is the fork's confirmed `Cx` current-time accessor (`platform/src/cx_api.rs:167`); there is **no** `cx.time_now()` on this fork (a compile-gate failure if used). It is only read by the bloom animation start.

- [ ] **Step 8: Forward events to the radial + map its outcome**

In `crates/waml-editor/src/app.rs` `handle_event` (`:626`), before `self.ui.handle_event(...)` (`:659`), drive the radial and act on a commit:

```rust
        // Radial: while open, it consumes pointer/keys; a commit maps to a node
        // command (logging stub -- no node-edit path exists yet).
        let outcome = self
            .ui
            .widget(cx, ids!(radial))
            .borrow_mut::<crate::radial::Radial>()
            .filter(|r| r.is_open())
            .map(|mut r| r.handle(cx, event));
        if let Some(outcome) = outcome {
            match outcome {
                crate::radial::RadialOutcome::Committed(id) => {
                    if let Some(cmd) = crate::canvas::node_command_for(id) {
                        log!("node command: {cmd:?}");
                    }
                }
                crate::radial::RadialOutcome::Cancelled => {}
                crate::radial::RadialOutcome::None => {}
            }
        }
```

- [ ] **Step 9: Verify tests + visual sign-off**

Run: `cargo test -p waml-editor`
Expected: PASS (all `icon` + `radial` + `canvas` tests green; crate compiles).

Run: `cargo test --workspace`
Expected: PASS (the implement-plan gate).

Then visual sign-off via the self-screenshot recipe: `./scripts/run-native.ps1`, right-click a node, capture with `PrintWindow(hwnd, hdc, 2)` at native res. Confirm: the disc opens at the cursor; hover highlights a wedge (popup); hold-drag arms the wedge under the cursor and dims the others; release commits (stdout logs `node command: …`); Esc / hub-click / outside-click cancel; the `Remove` wedge is danger-hued. Repeat for N = 2..6 by temporarily varying `node_radial_items()` length, and screenshot the disabled state by setting one item `enabled: false`. Watch the app stdout for `[E] …radial.rs:LINE` shader errors and tune the `RadialWedge`/`DrawIcon` SDF ops (repo memory: they blank silently; verify every state visually).

- [ ] **Step 10: Commit**

```bash
git add crates/waml-editor/src/canvas.rs crates/waml-editor/src/app.rs
git commit -m "feat(editor): wire node right-click radial into the canvas + App"
```

---

### Task 5: `AccentFrame` rename (independent, non-blocking)

Sequenced **last** and independent — the radial does not block on it (see reviewer flag 6). This is the `draw_hud.rs`/`HudFrame`→`frame.rs`/`AccentFrame` half of the spec's rename table only. The `waml_button.rs`→`button.rs` / `WamlButton`→`Button` half is **omitted**: no such file exists on this baseline (flag for the reviewer).

**Files:**
- Rename: `crates/waml-editor/src/draw_hud.rs` → `crates/waml-editor/src/frame.rs`
- Modify: `crates/waml-editor/src/main.rs` (`mod draw_hud;` → `mod frame;`)
- Modify: `crates/waml-editor/src/app.rs:613` (`crate::draw_hud::script_mod` → `crate::frame::script_mod`)
- Modify **every** `mod.draw.HudFrame` consumer — the full grounded list is: `canvas.rs`, `tool_dock.rs`, `tree_panel.rs`, `inspector_panel.rs`, `selection_toolbar.rs` (plus the definition in `draw_hud.rs` and the registration in `main.rs`/`app.rs`) — DSL `mod.draw.HudFrame` → `mod.draw.AccentFrame`. DSL symbol resolution is a script-VM **runtime** concern, so a missed consumer passes `cargo test --workspace` green but crashes the app at launch — Step 1's `git grep` is authoritative over this list.

**Interfaces:**
- Produces: `mod.draw.AccentFrame` (was `HudFrame`); Rust module `frame` (was `draw_hud`). No behavior change.

- [ ] **Step 1: Find every reference**

Run: `git grep -n "HudFrame\|draw_hud"`
Expected: matches in `draw_hud.rs`, `main.rs`, `app.rs`, `canvas.rs`, `tool_dock.rs`, and any `tree_panel`/comments. Record the full list before editing.

- [ ] **Step 2: Rename the file + module**

```bash
git mv crates/waml-editor/src/draw_hud.rs crates/waml-editor/src/frame.rs
```

In `main.rs`, change `mod draw_hud;` to `mod frame;` (re-sort: `mod frame;` goes after `mod doc_tabs;`).

- [ ] **Step 3: Rename the DSL symbol + Rust references**

In `frame.rs`: rename `mod.draw.HudFrame` → `mod.draw.AccentFrame` (the block at former `draw_hud.rs:36`) and update the module doc comment (`HudFrame` → `AccentFrame`). In `app.rs:613`: `crate::draw_hud::script_mod(vm)` → `crate::frame::script_mod(vm)`. Then in **each** consumer surfaced by Step 1's `git grep` — `canvas.rs`, `tool_dock.rs`, `tree_panel.rs`, `inspector_panel.rs`, `selection_toolbar.rs` — change `mod.draw.HudFrame{ … }` → `mod.draw.AccentFrame{ … }`. Do NOT rely on this list being exhaustive; the Step 4 `git grep` must return empty.

- [ ] **Step 4: Verify no stragglers**

Run: `git grep -n "HudFrame\|draw_hud"`
Expected: no matches (empty output).

- [ ] **Step 5: Verify build + tests + shader**

Run: `cargo test --workspace`
Expected: PASS.

Then launch once (`./scripts/run-native.ps1`) and screenshot to confirm the renamed `AccentFrame` shader still compiles at GPU-runtime (no `[E] …frame.rs:LINE` in stdout) and node/panel frames render identically.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor(editor): rename HudFrame -> AccentFrame (frame.rs)"
```

---

## Self-Review

**1. Spec coverage.**
- Unit 1 `icon` module (spec:29-53) → Task 1 (`Icon`/`IconShape`/`draw_icon`/`DrawIcon`, seed set = the four commands). ✓
- Unit 2 `radial` module + `RadialItem`/`RadialOutcome`/`open`/`handle`/`draw`/`is_open` (spec:55-78) → Tasks 2–3. ✓ (`handle`/`draw`/`is_open`/`open` all present; `open` takes `time` per spec.)
- Unit 3 node wiring (spec:80-86) → Task 4. ✓ (flagged: no pre-existing command path; stubs.)
- Geometry: N sectors 360/N, first wedge centred at 12 o'clock clockwise, ~120px disc, ~30px hub, angle-based hit-test (spec:88-108) → Task 2 `wedge_index`/`resolve_target` + tests. ✓
- Interaction: tap→popup→commit, hold-drag→arm→release-commit, flick, all four dismiss paths, disabled no-op (spec:111-137) → Task 3 `RadialCore` methods + 8 state tests. ✓
- Rendering/material: per-wedge `DrawColor`+`draw_abs`, state/danger/enabled uniforms, hub, disabled grey, shared 150° stroke recipe (spec:140-158) → Task 3 `RadialWedge` shader. ◑ PARTIAL — `rest`/`arm`/`flick` + disabled grey + inline stroke recipe land in code; popup-hover fill, marking recede (`dim`/`gone`), hub accent ring + ✕, and frosted disc/shadow/bloom backing are deferred to the screenshot-tuning pass (see Task 3 Step 7 "Rendering-state scope" note). Shared-recipe = inline copy, flagged.
- Animation: `NextFrame` bloom-in (spec:160-166) → Task 3 `open`/`tick`. ◑ PARTIAL — the `NextFrame` loop runs and redraws; the actual bloom scale/opacity ramp is applied during screenshot tuning (`start`/`BLOOM_SECS` stored, not yet read in `draw`).
- Placement: draw clipped at edges, angle hit-test unaffected (spec:169-173) → the disc quad + angle-based `wedge_index` (never uses the quad for hit-testing). ✓
- Accent/danger tokens, no per-open recolour (spec:176-185) → Global Constraints + shader uniforms. ✓
- `AccentFrame` rename (spec:187-201) → Task 5, sequenced last with justification. ✓
- Testing: geometry units (N=2..6, hub, disabled, wrap) + state-machine units + manual screenshots (spec:204-214) → Tasks 2–3 tests + Task 4/3 screenshot steps. ✓

**2. Placeholder scan.** No "TBD"/"handle edge cases"/"write tests for the above". Every code step shows real Rust/MPSL; every command step gives the exact `cargo test -p waml-editor …` invocation and expected pass/fail. Shader SDF ops are real code explicitly marked as screenshot-tuned (a repo-standard verification step, not a placeholder). ✓

**3. Type consistency.** `RadialItem`/`RadialOutcome`/`RadialCore`/`Radial` names stable across Tasks 2–4. `wedge_index`/`resolve_target` signatures match their call sites. `node_command_for`/`NodeCommand`/`GraphCanvasAction` consistent between `canvas.rs` definition and `app.rs` use. `Icon`/`IconShape`/`shader_index`/`draw_icon` consistent between Task 1 and Task 3. `live_id!(open/style/markdown/remove)` used identically in `node_radial_items` (Task 4), the state-machine tests (Task 3), and `node_command_for` (Task 4). ✓
