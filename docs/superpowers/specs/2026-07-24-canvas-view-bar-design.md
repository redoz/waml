# Canvas View Bar — design

**Date:** 2026-07-24
**Status:** approved, awaiting split into implementation plans

## Problem

Constraint visibility lives in a three-cell segmented switcher (`constraint_toggle.rs`)
pinned to the top-left of the canvas. Its `All` cell draws every relation in the diagram
at once, which reads as an undifferentiated grey mess — it is being dropped.

More view-level controls are coming: zoom in/out, fit to size, fit to selection, and a
toggle for group visibility. They have nowhere to live. `ToolDock` is the wrong home: its
lit state means *"this one exclusive mode is active"* (`Select`/`Add`/`Connect`), whereas
view controls are N **independent** toggles plus one-shot camera **actions**. Putting both
in one strip would make "lit" mean two different things in the same widget.

Separately, native group rendering diverges from both the spec and the web renderer.
`canvas.rs:1390-1418` draws chrome — tinted fill, outline, title — for *every* group
regardless of its `Shape`. Per `docs/uaml-spec.md:674-681` a group renders nothing unless
it opts into `frame` (or `box`); the default is `shrink`. Since the default is what almost
every group actually is, the canvas shows boxes that were never meant to be drawn.

## Non-goals

- **No change to how constraints are drawn.** The Selected-mode veil, relation glyphs,
  `reframe_to_selected`, state colouring, and the off-canvas error list are untouched.
  This change moves the *button* and removes one *option*.
- **No model or wasm-ABI change.** `DiagramGroup`, `SolvedGroup`, and `Shape` stay as they
  are; the wasm ABI stays frozen.
- **No new WAML syntax.** The `frame`/`box`/`shrink` opt-in already exists and already
  reaches the renderer.
- **No new solver work.** Group visibility is purely a render decision.

## Existing substrate (verified against the tree, 2026-07-24)

| Thing | Where | State |
|---|---|---|
| `SolvedGroup { rect, shape, title, depth }` | `crates/waml/src/solve/mod.rs:67` | `shape` already reaches the native renderer, unread |
| Group shape default `Shape::Shrink` | `crates/waml/src/solve/resolve.rs:102` | every group starts invisible |
| `frame`/`box` opt-in via layout hint | `crates/waml/src/solve/resolve.rs:122` (`Hint::Shape(s) => bx.shape = *s`) | parsed at `layout.rs:433-447` |
| Shape semantics | `docs/uaml-spec.md:674-681` | `frame` = visible titled box; `box`/`shrink` = layout only, no chrome |
| Web renderer honours it | `docs/superpowers/plans/2026-07-17-prose-solved-diagram-rendering.md:18` | "Only `shape === "Frame"` draws chrome" |
| Native ignores it | `crates/waml-editor/src/canvas.rs:1390-1418` | draws all groups — the divergence |
| `Camera::fit(bbox, w, h, pad)` | `crates/waml-editor/src/camera.rs:37` | clamps zoom to `MIN_ZOOM`/`MAX_ZOOM` |
| `Camera::zoom_at(lx, ly, factor)` | `crates/waml-editor/src/camera.rs:29` | same clamp; keeps the point under `(lx,ly)` fixed |
| `scene::bounding_box(&scene)` | `crates/waml-editor/src/scene.rs:662` | `Option<Rect>` over nodes + groups |
| Load-time fit | `crates/waml-editor/src/canvas.rs:1360-1378` | `Camera::fit(bbox, rect.w, rect.h, 48.0)`, gated by `self.fitted` |
| `self.view_rect` | set each `draw_walk`, `canvas.rs:1357` | the viewport the fit actions need |
| `IconButton` shared widget | `crates/waml-editor/src/icon_button.rs` | glyph + accent wash + `set_active` lit |
| Icon catalog | `crates/waml-editor/src/icons.rs` | 94 entries; none of the glyphs this needs |
| Lucide source | `C:\dev\vendor\lucide-icons\icons\` | has all six SVGs needed |

## Design

### 1. `ViewBar` — new widget, bottom-center, always visible

New file `crates/waml-editor/src/view_bar.rs`, built on the `ToolDock` pattern: a
`#[derive(Script, ScriptHook, Widget)]` struct with a `#[deref] View`, `IconButton`
children declared in `script_mod!`, `draw_walk` syncing each child's glyph + lit state
from owned state, and `handle_event` reading each child's `clicked` action.

```
        ┌──────────────┐
        │ 3 selected ✕ │        selection pill — contextual, sits above
        └──────────────┘
     ┌────────────────────────┐
     │  ⊕  ⊖  ⛶  ⦿ │ ⬚  📐  │   ViewBar — always visible
     └────────────────────────┘
     ──────── canvas bottom ────────
```

`width: Fit`, `height: 36.0`, `flow: Right`, the same Atlas HUD frame material as
`ToolDock`/`ConstraintToggle` (`field_bg` fill, `frame_hi`→`frame_lo` 150° diagonal accent
stroke). Two sections separated by a `divider := View{ width: 1.0, height: Fill }` child
tinted `atlas.frame_lo`:

- **camera one-shots:** `ZoomIn`, `ZoomOut`, `FitToSize`, `FitToSelection` — never lit
- **view toggles:** `ShowHiddenBorders`, `ShowConstraints` — independently lit while on

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewOption {
    ZoomIn,
    ZoomOut,
    FitToSize,
    FitToSelection,
    ShowHiddenBorders,
    ShowConstraints,
}

impl ViewOption {
    /// Declaration order == left-to-right layout order.
    pub const ALL: [ViewOption; 6] = [
        ViewOption::ZoomIn,
        ViewOption::ZoomOut,
        ViewOption::FitToSize,
        ViewOption::FitToSelection,
        ViewOption::ShowHiddenBorders,
        ViewOption::ShowConstraints,
    ];
    /// Independent on/off state (lit while on) vs. a one-shot action.
    pub fn is_toggle(self) -> bool {
        matches!(self, ViewOption::ShowHiddenBorders | ViewOption::ShowConstraints)
    }
    pub fn label(self) -> &'static str {
        match self {
            ViewOption::ZoomIn => "Zoom In",
            ViewOption::ZoomOut => "Zoom Out",
            ViewOption::FitToSize => "Fit to Size",
            ViewOption::FitToSelection => "Fit to Selection",
            ViewOption::ShowHiddenBorders => "Show Hidden Borders",
            ViewOption::ShowConstraints => "Show Constraints",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum ViewBarAction {
    #[default]
    None,
    /// A camera one-shot fired.
    Triggered(ViewOption),
    /// A toggle flipped; carries its new state.
    Toggled(ViewOption, bool),
}
```

State is two bools (`show_hidden_borders`, `show_constraints`), owned by the widget under
`#[rust]`. Defaults: **constraints ON** (preserves today's behaviour — `ConstraintVisibility::default()`
is `Selected`, asserted at `constraint_toggle.rs:138`), **hidden borders OFF**.

`FitToSelection` with nothing selected is a no-op. The button renders dim in that state
rather than disappearing, so the bar's width and the other buttons' positions never shift.
The bar exposes `set_fit_to_selection_enabled(cx, bool)` for `App` to drive from selection
changes; `App` already tracks the focused subject for the selection pill and inspector.

`App` reads it with a `view_bar_action(&Actions) -> Option<ViewBarAction>` convenience
reader, mirroring `ToolDock::dock_action` (`tool_dock.rs:244`).

### 2. Retire the segmented switcher

Delete `crates/waml-editor/src/constraint_toggle.rs`, its `mod` line in `main.rs:15`, its
`script_mod` registration in `app.rs:1944`, its `use mod.widgets.ConstraintToggle` at
`app.rs:27`, and the `constraint_toggle_wrap` mount at `app.rs:274-284`.

`ConstraintVisibility` (`canvas.rs:472`) collapses to two variants:

```rust
pub enum ConstraintVisibility { None, Selected }   // was None | Selected | All
```

Because nothing can enter `All` any more, the branches that only ran in `All` mode go with
it. **Selected-mode drawing is byte-identical** — these branches were already skipped when
the mode was `Selected`:

| Deleted | Where |
|---|---|
| `[` / `]` parallax layer scrub keys | `canvas.rs:1085-1095` |
| `cycle_scrub_layer()` | `canvas.rs:2238-2246` |
| `scrub_layer: usize` field | `canvas.rs:380`, reset at `:2150` and `:2173` |
| parallax offset computation | `canvas.rs:1832-1845`, applied at `:1849-1851` |
| `parallax_base: Option<(f64, f64)>` field | `canvas.rs:384`, init at `:1377`, reset at `:2149` and `:2172` |
| `parallax_offset()` helper | `canvas.rs:546` |
| `parallax_offset_scales_with_depth_and_pan` test | `canvas.rs:2791-2809` |
| the `pov` else-branch — `pov` is now always the selected key | `canvas.rs:1810-1816` |
| the `All` arm of `relations_for_visibility` | `canvas.rs:501` |
| the `All` assertion in `relations_for_visibility` tests | `canvas.rs:2751` |

`ConstraintVisibility::ALL` (`canvas.rs:483`) drops to two entries. `canvas.rs:2738-2744`
(the `None`/`Selected` assertions) stay as-is.

Wiring: `Toggled(ShowConstraints, on)` calls the existing
`Canvas::set_constraint_vis(cx, if on { Selected } else { None })` (`canvas.rs:2232`) —
no new canvas API.

### 3. Group render gating

Rewrite the group loop at `canvas.rs:1390-1418` around two questions per group:

```
visible = group.shape == Shape::Frame
if !visible && !show_hidden_borders { skip }
if visible  { fill + solid border + title }        // as today
else        { dashed border only, no fill, dim title }
```

- **Default (hidden borders off):** only `Shape::Frame` groups draw. This is what the spec
  says and what the web renderer already does — the change brings native into line.
- **Hidden borders on:** `Box` and `Shrink` groups additionally draw as a dashed hairline
  with no fill, plus their title in `atlas.text_dim`. Nesting keeps working unchanged:
  draw order stays shallow-first so inner groups land on top.
- **Untitled fallback:** `SolvedGroup.title` is `None` for unnamed groups (inline groups,
  `resolve.rs:92-96`). When drawing a hidden group, a `None` title renders as
  `Untitled {n}` — `n` being a 1-based counter over the untitled groups in `scene.groups`
  order. Pure function `untitled_label(n) -> String`, renderer-side only, no model change.

New `Canvas::set_show_hidden_borders(cx, bool)` mirroring `set_constraint_vis`, plus a
`#[rust] show_hidden_borders: bool` field.

**Dashed border implementation.** Preferred: a `draw_group_dashed: DrawColor` whose `pixel`
fn takes the Sdf2d distance to the group rect's border and masks the stroke with a dash
period derived from `(pos.x + pos.y)`, so the dash rides the border consistently on all
four sides and scales with `zoom` (which is already pushed as a uniform at
`canvas.rs:1387`). If the diagonal parameterization reads badly at the corners, fall back
to stamping short axis-aligned segment quads along the four sides — the same technique
`segment_quad` already uses for edges. The plan should try the shader first and record
which one shipped.

### 4. Camera actions

All four use existing camera API. `self.view_rect` (`canvas.rs:1357`) is the viewport.

| Action | Implementation |
|---|---|
| `ZoomIn` | `camera.zoom_at(view_rect.w * 0.5, view_rect.h * 0.5, 1.2)` — anchored at the viewport centre so a button press keeps the middle of the canvas stable, unlike the cursor-anchored scroll path |
| `ZoomOut` | same with `1.0 / 1.2` |
| `FitToSize` | `bounding_box(&self.scene)` → `Camera::fit(bbox, view_rect.w, view_rect.h, 48.0)`. Pad `48.0` matches the load-time fit at `canvas.rs:1374`; extract it as a `FIT_PAD` const so the two can't drift. `None` bbox (empty scene) is a no-op |
| `FitToSelection` | selected node's rect via `scene.nodes.iter().find(|n| n.key == selected_key)` → same `Camera::fit` with `FIT_PAD`. No selection, or key not found, is a no-op |

Both zoom and fit already clamp to `MIN_ZOOM`/`MAX_ZOOM` inside `camera.rs`, so no
clamping is needed at the call sites. Each action ends with a redraw, as the existing
scroll-zoom path does (`canvas.rs:1349`).

New `Canvas` methods: `zoom_step(&mut self, cx, factor: f64)`, `fit_to_scene(&mut self, cx)`,
`fit_to_selection(&mut self, cx)`. `focus_mode` (`canvas.rs:1362`) keeps its zoom-1.0
special case for the initial frame; the explicit fit actions ignore `focus_mode` because
the user asked for a fit.

### 5. Fit sizing

`ToolDock`'s mount hardcodes `height: 308.0` (`app.rs:270`) with the comment *"the widget
draws items manually so Fit collapses to 0 — an explicit height is required"*. That comment
is **stale**: the dock has been five real `IconButton` children in a `flow: Down` turtle
since the IconButton extraction, so `Fit` measures correctly. Change to `height: Fit` and
delete the stale comment.

`ViewBar` is `width: Fit` from the start, so adding a seventh button later needs no
arithmetic.

### 6. Catalog glyphs

Six new entries authored from `C:\dev\vendor\lucide-icons\icons\`:

| `ViewOption` | Lucide source |
|---|---|
| `ZoomIn` | `zoom-in.svg` |
| `ZoomOut` | `zoom-out.svg` |
| `FitToSize` | `maximize.svg` |
| `FitToSelection` | `scan-search.svg` |
| `ShowHiddenBorders` | `square-dashed.svg` |
| `ShowConstraints` | `ruler.svg` |

`ruler` is the CAD dimension-constraint metaphor, consistent with the CAD framing of the
constraint-visibility work. `square-dashed` is literally what the toggle reveals.

Add-only, per the standing catalog rule: never drop an existing glyph for being unused.
Each entry must respect the catalog's order invariant — `enum` == struct field == DSL
`mod.draw.*` == `get` match arm == `ALL` array == `label` arm, all in the same order — and
the count assertion bumps 94 → 100 (`icons.rs:3946` region).

### 7. Bottom-center stacking

`ViewBar` owns the bottom-center slot at `margin: Inset{bottom: 12.0}`. It is always
visible, so its click targets never move.

The selection pill (`selection_toolbar`, `app.rs:295-305`) is contextual — shown only when
a classifier is focused — and moves up to clear the bar: bottom margin `12 + 36 + 8 = 56`.
Both live in the same bottom-aligned `Fill`/`Fill` overlay `View` pattern the other canvas
HUD surfaces already use, so no new compositing mechanism is needed.

## Data flow

```
ViewBar child IconButton clicked
  -> ViewBar::handle_event maps the child to a ViewOption
  -> toggle: flip owned bool, redraw, emit Toggled(opt, on)
     one-shot: emit Triggered(opt)
  -> App::handle_actions reads view_bar_action(actions)
  -> Toggled(ShowConstraints, on)    -> canvas.set_constraint_vis(cx, Selected | None)
     Toggled(ShowHiddenBorders, on)  -> canvas.set_show_hidden_borders(cx, on)
     Triggered(ZoomIn | ZoomOut)     -> canvas.zoom_step(cx, 1.2 | 1/1.2)
     Triggered(FitToSize)            -> canvas.fit_to_scene(cx)
     Triggered(FitToSelection)       -> canvas.fit_to_selection(cx)
  -> canvas redraws
```

Selection changes flow the other way: `App` calls
`view_bar.set_fit_to_selection_enabled(cx, has_selection)` wherever it already syncs the
selection pill and inspector subject.

## Edge cases

| Case | Behaviour |
|---|---|
| Empty scene, `FitToSize` | `bounding_box` returns `None` — no-op, no camera mutation |
| No selection, `FitToSelection` | no-op; button drawn dim |
| Zoom at a clamp bound | `zoom_at` clamps; the button stays enabled and simply does nothing further |
| Diagram with zero `frame` groups, hidden borders off | no group chrome at all — correct per spec, and the visible improvement this change is for |
| Nested hidden groups | all draw dashed; shallow-first order keeps inner ones on top |
| Two untitled groups | `Untitled 1`, `Untitled 2` by `scene.groups` order — stable across redraws of the same scene |
| `focus_mode` canvas | initial framing keeps its zoom-1.0 special case; explicit fit actions override it |

## Testing

Pure, `Cx`-free unit tests in the style already used by `tool_dock.rs`:

- `ViewOption::ALL.len() == 6`; `is_toggle()` true only for the last two (mirrors
  `only_the_first_three_tools_are_modes`).
- `ViewBar::icon_for(opt)` maps each option to its catalog glyph (mirrors
  `icon_map_tests`).
- `Icon::ALL` count assertion at 100, plus the existing order-invariant tests in
  `icons.rs`.
- `untitled_label(n)` formatting.
- `group_draws_chrome(shape, show_hidden)` — the pure predicate behind §3, over all six
  `(Shape, bool)` combinations.
- `relations_for_visibility` keeps its `None`/`Selected` tests; the `All` case is removed
  with the variant.
- Camera behaviour is already covered by `camera.rs:107-131` (`fit`) and `:80-104`
  (`zoom_at` fixed-point + clamp). Add a test that `FIT_PAD` is the value both the
  load-time fit and `fit_to_scene` pass, so they cannot drift.

Delete `constraint_toggle.rs`'s two tests with the file.

## Implementation gotchas

These have each cost a debugging session before and the full `cargo test` + `pnpm` gate is
blind to all of them:

1. **`script_mod` registration order.** `view_bar::script_mod(vm)` must be registered
   **before** `app.rs`'s own module registration (`app.rs:1944` region), because
   `mod.widgets.*` resolves eagerly. Register late and the widget mounts as a dead,
   invisible node — tests and review both pass.
2. **`script_mod` namespace shape.** Any `mod.X` namespace must be created by **one
   object-literal assignment** with colon fields, never field-by-field `mod.X.f = ...`,
   which aborts the VM type-check silently and blanks chrome text.
3. **Per-pid visual verification is mandatory.** Screenshot the specific launched pid —
   screenshot-by-name grabs the user's own open editor, and `Stop-Process` by name kills
   their session.
4. **`run-native.ps1` builds the checkout the script lives in** (`$PSScriptRoot`), not the
   cwd. Launch the worktree's own copy or main's stale binary starts instead.
5. **Absolute paths in `Edit`/`Write`.** They have no cwd. A main-root path edits main
   while the worktree build "passes" against a stale copy.

## Plan split

Four plans, in dependency order. A and D are independent of each other; B depends on A; C
depends on A.

**Plan A — catalog glyphs.** Author the six SDF glyphs from the lucide sources. Order
invariant across all six sites, count 94 → 100. No behaviour change, so it lands on its
own and unblocks both B and C. Verification is the icon-preview harness plus the catalog's
existing order/count tests.

**Plan B — `ViewBar` + retire the switcher.** New `view_bar.rs` with all six buttons
declared and laid out; `ShowConstraints` wired end-to-end; camera one-shots emit their
actions but `App` logs them as no-ops pending Plan D. Delete `constraint_toggle.rs` and
collapse `ConstraintVisibility` per §2. `ToolDock` height → `Fit`; bottom stacking per §7.
Verification: constraints toggle visibly flips the veil on/off; `Fit` sizing measured
correctly on the dock; the selection pill clears the bar when it appears.

**Plan C — group gating + hidden borders.** §3 in full: shape-gated chrome, dashed hidden
groups, `Untitled N`. Wires `ShowHiddenBorders`. Verification is visual on a fixture with
both a `frame` group and a default group.

**Plan D — camera actions.** §4: `zoom_step`, `fit_to_scene`, `fit_to_selection`,
`FIT_PAD` extraction, `set_fit_to_selection_enabled` driven from selection. Verification is
interactive: zoom holds the viewport centre, fit frames the whole diagram, fit-to-selection
frames one node, and the button is dim with nothing selected.
