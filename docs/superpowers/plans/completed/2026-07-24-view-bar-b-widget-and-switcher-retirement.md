# ViewBar Plan B — `ViewBar` Widget + Switcher Retirement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the top-left three-cell constraint switcher with a new bottom-centre `ViewBar` carrying all six view controls, wire `ShowConstraints` end-to-end, and collapse `ConstraintVisibility` from three variants to two.

**Architecture:** `ViewBar` is a new widget cloned from the `ToolDock` pattern — a `#[derive(Script, ScriptHook, Widget)]` struct with a `#[deref] View`, six `IconButton` children plus a divider declared in `script_mod!`, `draw_walk` syncing each child's glyph + lit state from owned state, and `handle_event` reading each child's `clicked`. It is deliberately *not* merged into `ToolDock`: the dock's "lit" means one exclusive active mode, whereas view controls are N independent toggles plus one-shot camera actions. Actions route through `ClassDiagramView::handle` (the diagram tab owns body actions), which drives `GraphCanvas::set_constraint_vis`. Dropping the `All` visibility variant also removes the parallax-layer-scrub machinery that only ran in `All` mode.

**Tech Stack:** Rust, makepad widgets + script DSL (`script_mod!`), `cargo test`.

## Global Constraints

- **Spec:** `docs/superpowers/specs/2026-07-24-canvas-view-bar-design.md` §1, §2, §5, §7.
- **Depends on Plan A** (`docs/superpowers/plans/2026-07-24-view-bar-a-catalog-glyphs.md`) — it must be landed on `origin/main` first. This plan references `Icon::ZoomIn`, `Icon::ZoomOut`, `Icon::Maximize`, `Icon::ScanSearch`, `Icon::SquareDashed`, `Icon::Ruler` and will not compile without it.
- **Plan order:** A → B → {C, D}. Plans C and D both extend this plan's `ViewBar` and its action match arms.
- **Out of scope:** how constraints are *drawn* (the Selected-mode veil, relation glyphs, `reframe_to_selected`, state colouring, the off-canvas error list) is untouched. No model or wasm-ABI change. No new WAML syntax. No camera behaviour (Plan D). No group render change (Plan C).
- **The camera one-shots (`ZoomIn`/`ZoomOut`/`FitToSize`/`FitToSelection`) are deliberately `log!` no-ops in this plan.** Plan D replaces those arms. Emit the actions correctly regardless.
- **`script_mod` registration order:** `crate::view_bar::script_mod(vm);` must run **before** `self::script_mod(vm)` in `App::script_mod` (`app.rs:1956`), because the DSL resolves `mod.widgets.*` eagerly at `use`-time. Register late and the widget mounts as a dead, invisible node — `cargo test` and code review both pass. Put it exactly where `crate::constraint_toggle::script_mod(vm);` is today (`app.rs:1944`).
- **`mod.X` namespaces are ONE object literal** with colon fields — never field-by-field `mod.X.f = ...`, which aborts the VM type-check silently and blanks all chrome text. The whole `cargo`/`pnpm` gate never boots the script VM, so it is blind to this and to the registration-order rule above. Only a launched build proves them.
- **`-D warnings` promotes rustc `dead_code` to a hard error** in the plan gate. Any new item with no consumer needs an explicit `#[allow(dead_code)]` and a comment saying why, or it must not be added. This is why `ConstraintVisibility::ALL` is *deleted* rather than trimmed to two entries (Task 6) — a deviation from the spec, which said "drops to two entries"; nothing iterates it once the switcher is gone.
- **Never edit the main checkout.** Work in a git worktree. `Edit`/`Write` take absolute paths and have no cwd — a main-root path edits main while the worktree build "passes" against a stale copy. Tell: a new test missing from the worktree's `cargo test -- --list`.
- **`scripts/run-native.ps1` builds the checkout the script lives in** (`$PSScriptRoot`), not your cwd. Launch the worktree's own copy or main's stale binary starts instead.
- **Screenshot by specific pid, in one PowerShell call.** Screenshot-by-name grabs the user's own open editor, and `Stop-Process` by name kills their session.
- **Full gate before each commit:** `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`.

---

### Task 1: `ViewBar` widget

**Files:**
- Create: `crates/waml-editor/src/view_bar.rs`
- Modify: `crates/waml-editor/src/main.rs` (the alphabetical `mod` list; `mod view_bar;` goes between `mod tree_panel;` and `mod veil;`)
- Test: `crates/waml-editor/src/view_bar.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `crate::icons::Icon` (Plan A glyphs), `crate::icon_button::{IconButton, IconButtonWidgetRefExt}`.
- Produces:
  - `pub enum ViewOption { ZoomIn, ZoomOut, FitToSize, FitToSelection, ShowHiddenBorders, ShowConstraints }` with `pub const ALL: [ViewOption; 6]`, `pub fn is_toggle(self) -> bool`, `pub fn label(self) -> &'static str`.
  - `pub enum ViewBarAction { None, Triggered(ViewOption), Toggled(ViewOption, bool) }`.
  - `pub struct ViewBar` with `pub fn view_bar_action(&self, actions: &Actions) -> Option<ViewBarAction>`. (No `show_constraints()`/`show_hidden_borders()` getters: nothing reads them, and under `-D warnings` an unused `pub` method on a binary crate's type is a hard error. The toggles' state travels in `ViewBarAction::Toggled(opt, on)`.)
  - DSL widget `mod.widgets.ViewBar`, child ids `zoom_in_btn`, `zoom_out_btn`, `fit_size_btn`, `fit_selection_btn`, `divider`, `hidden_borders_btn`, `constraints_btn`.
  - `pub fn script_mod(vm: &mut ScriptVm) -> ScriptValue` (generated by `script_mod!`).

**Context:** Copy the structure of `crates/waml-editor/src/tool_dock.rs` — that is the proven in-tree pattern for "a `#[deref] View` strip of `IconButton` children whose glyph + lit state are pushed per draw". The HUD frame material (`field_bg` fill ringed by a `frame_hi`→`frame_lo` 150° diagonal accent stroke) is inlined on `draw_bg` in `tool_dock.rs:44-59` and `constraint_toggle.rs:29-44`; reproduce it verbatim. `IconButton` is 32x32 with a 16px glyph and exposes `set_icon`, `set_active`, `clicked` through `IconButtonWidgetRefExt::as_icon_button`. A `View{}` child inside a `mod.prelude.widgets_internal.*` module is proven (`inspector_panel.rs:98`).

- [ ] **Step 1: Write the failing tests**

Create `crates/waml-editor/src/view_bar.rs` containing only this test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::icons::Icon;

    #[test]
    fn all_lists_six_options_in_layout_order() {
        assert_eq!(ViewOption::ALL.len(), 6);
        assert_eq!(ViewOption::ALL[0], ViewOption::ZoomIn);
        assert_eq!(ViewOption::ALL[1], ViewOption::ZoomOut);
        assert_eq!(ViewOption::ALL[2], ViewOption::FitToSize);
        assert_eq!(ViewOption::ALL[3], ViewOption::FitToSelection);
        assert_eq!(ViewOption::ALL[4], ViewOption::ShowHiddenBorders);
        assert_eq!(ViewOption::ALL[5], ViewOption::ShowConstraints);
    }

    #[test]
    fn only_the_last_two_options_are_toggles() {
        for (i, opt) in ViewOption::ALL.iter().enumerate() {
            assert_eq!(opt.is_toggle(), i >= 4, "{opt:?} toggle-ness mismatch");
        }
    }

    #[test]
    fn every_option_maps_to_a_catalog_icon() {
        assert_eq!(ViewBar::icon_for(ViewOption::ZoomIn), Icon::ZoomIn);
        assert_eq!(ViewBar::icon_for(ViewOption::ZoomOut), Icon::ZoomOut);
        assert_eq!(ViewBar::icon_for(ViewOption::FitToSize), Icon::Maximize);
        assert_eq!(ViewBar::icon_for(ViewOption::FitToSelection), Icon::ScanSearch);
        assert_eq!(
            ViewBar::icon_for(ViewOption::ShowHiddenBorders),
            Icon::SquareDashed
        );
        assert_eq!(ViewBar::icon_for(ViewOption::ShowConstraints), Icon::Ruler);
    }

    #[test]
    fn every_option_has_a_nonempty_label() {
        for opt in ViewOption::ALL {
            assert!(!opt.label().is_empty(), "empty label for {opt:?}");
        }
        assert_eq!(ViewOption::ShowConstraints.label(), "Show Constraints");
    }
}
```

Add `mod view_bar;` to `crates/waml-editor/src/main.rs` between `mod tree_panel;` and `mod veil;`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-editor view_bar::`
Expected: FAIL to compile — `cannot find type 'ViewOption' in this scope`.

- [ ] **Step 3: Write the widget**

Replace the contents of `crates/waml-editor/src/view_bar.rs` with the following, keeping the test module from Step 1 at the bottom:

```rust
//! Canvas view bar (spec 2026-07-24-canvas-view-bar-design §1): a bottom-centre
//! icon strip owning every *view-level* control — the camera one-shots (zoom
//! in/out, fit to size, fit to selection) and the independent view toggles
//! (hidden group borders, constraint veils).
//!
//! Deliberately a separate widget from `ToolDock`, not a section of it: the
//! dock's "lit" means *one exclusive active mode* (`Select`/`Add`/`Connect`),
//! while these are N independent toggles plus one-shot actions. One widget
//! would make "lit" mean two different things.
//!
//! Built on the `ToolDock` pattern: a `#[deref] View` laying out `IconButton`
//! children in a `flow: Right` strip; `draw_walk` syncs each child's glyph +
//! lit state from the owned toggle bools, and `handle_event` reads each child's
//! `clicked` to emit a `ViewBarAction`. The strip's own `draw_bg` paints the
//! Atlas HUD frame, matching `ToolDock`.

use makepad_widgets::*;

use crate::icon_button::IconButtonWidgetRefExt;
use crate::icons::Icon;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*

    mod.widgets.ViewBarBase = #(ViewBar::register_widget(vm))

    mod.widgets.ViewBar = set_type_default() do mod.widgets.ViewBarBase{
        // Fit so a seventh button later needs no arithmetic.
        width: Fit
        height: 36.0
        flow: Right
        align: Align{x: 0.5, y: 0.5}
        padding: Inset{left: 4.0, right: 4.0, top: 2.0, bottom: 2.0}
        spacing: 2.0
        show_bg: true
        // The strip carries the Atlas HUD frame -- the AccentFrame material
        // inlined onto the View's `draw_bg` (keep in sync with `frame.rs` /
        // `tool_dock.rs`): a `field_bg` fill ringed by the source-bright accent
        // stroke fading along a 150deg diagonal.
        draw_bg +: {
            color: atlas.field_bg
            border_hi: uniform(atlas.frame_hi)
            border_lo: uniform(atlas.frame_lo)
            pixel: fn() {
                let inset = 1.5
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.rect(inset, inset, self.rect_size.x - inset * 2.0, self.rect_size.y - inset * 2.0)
                sdf.fill_keep(self.color)
                let dir = vec2(0.5, 0.8660254)
                let span = 1.3660254
                let t = clamp((self.pos.x * dir.x + self.pos.y * dir.y) / span, 0.0, 1.0)
                sdf.stroke(mix(self.border_hi, self.border_lo, t), inset)
                return sdf.result
            }
        }

        // Camera one-shots (never lit), then a hairline divider, then the
        // independent view toggles (lit while on).
        zoom_in_btn := IconButton {}
        zoom_out_btn := IconButton {}
        fit_size_btn := IconButton {}
        fit_selection_btn := IconButton {}
        divider := View{
            width: 1.0
            height: Fill
            show_bg: true
            margin: Inset{left: 5.0, right: 5.0, top: 6.0, bottom: 6.0}
            draw_bg +: { color: atlas.frame_lo }
        }
        hidden_borders_btn := IconButton {}
        constraints_btn := IconButton {}
    }
}

/// A view-bar entry. The first four are one-shot camera *actions*; the last two
/// are independent *toggles* (lit while on).
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
        matches!(
            self,
            ViewOption::ShowHiddenBorders | ViewOption::ShowConstraints
        )
    }

    /// Human-readable name. No consumer yet -- the bar has no tooltips and the
    /// statusbar doesn't report view state; kept because it is the natural home
    /// for that copy and the tests pin it.
    #[allow(dead_code)]
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

#[derive(Script, ScriptHook, Widget)]
pub struct ViewBar {
    /// The strip: a `flow: Right` `View` whose `draw_bg` paints the HUD frame
    /// and which lays out the six `IconButton` children plus the divider.
    #[deref]
    view: View,

    /// Constraint veils on/off. Defaults ON so the bar preserves today's
    /// behaviour (`ConstraintVisibility::default()` is `Selected`).
    #[rust(true)]
    show_constraints: bool,
    /// X-ray for groups that opt out of chrome. Defaults OFF.
    #[rust]
    show_hidden_borders: bool,
}

impl Widget for ViewBar {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Drive the children so their `clicked`/hover actions are emitted.
        self.view.handle_event(cx, event, scope);

        let uid = self.widget_uid();
        if let Event::Actions(actions) = event {
            for opt in ViewOption::ALL {
                if self.button(cx, opt).as_icon_button().clicked(actions) {
                    if opt.is_toggle() {
                        let on = !self.toggle_state(opt);
                        self.set_toggle_state(opt, on);
                        self.view.redraw(cx);
                        cx.widget_action(uid, ViewBarAction::Toggled(opt, on));
                    } else {
                        cx.widget_action(uid, ViewBarAction::Triggered(opt));
                    }
                    break;
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // Sync each child's glyph + lit state before the View lays them out:
        // only a toggle that is ON is lit; one-shot buttons never are.
        for opt in ViewOption::ALL {
            let lit = opt.is_toggle() && self.toggle_state(opt);
            let btn = self.button(cx, opt).as_icon_button();
            btn.set_icon(cx, Self::icon_for(opt));
            btn.set_active(cx, lit);
        }

        while self.view.draw_walk(cx, scope, walk).step().is_some() {}

        DrawStep::done()
    }
}

impl ViewBar {
    /// The child `IconButton` for an option. Central option->widget map, shared
    /// by the draw-time sync and the event-time click read.
    fn button(&mut self, cx: &mut Cx, opt: ViewOption) -> WidgetRef {
        match opt {
            ViewOption::ZoomIn => self.view.widget(cx, ids!(zoom_in_btn)),
            ViewOption::ZoomOut => self.view.widget(cx, ids!(zoom_out_btn)),
            ViewOption::FitToSize => self.view.widget(cx, ids!(fit_size_btn)),
            ViewOption::FitToSelection => self.view.widget(cx, ids!(fit_selection_btn)),
            ViewOption::ShowHiddenBorders => self.view.widget(cx, ids!(hidden_borders_btn)),
            ViewOption::ShowConstraints => self.view.widget(cx, ids!(constraints_btn)),
        }
    }

    /// The catalog glyph for an option. Pure meaning->glyph map; the child
    /// `IconButton` fetches the shader and tints it per-draw.
    fn icon_for(opt: ViewOption) -> Icon {
        match opt {
            ViewOption::ZoomIn => Icon::ZoomIn,
            ViewOption::ZoomOut => Icon::ZoomOut,
            ViewOption::FitToSize => Icon::Maximize,
            ViewOption::FitToSelection => Icon::ScanSearch,
            ViewOption::ShowHiddenBorders => Icon::SquareDashed,
            ViewOption::ShowConstraints => Icon::Ruler,
        }
    }

    /// Current state of a toggle. `false` for a one-shot option (never lit).
    fn toggle_state(&self, opt: ViewOption) -> bool {
        match opt {
            ViewOption::ShowConstraints => self.show_constraints,
            ViewOption::ShowHiddenBorders => self.show_hidden_borders,
            _ => false,
        }
    }

    /// Store a toggle's new state. A one-shot option is ignored.
    fn set_toggle_state(&mut self, opt: ViewOption, on: bool) {
        match opt {
            ViewOption::ShowConstraints => self.show_constraints = on,
            ViewOption::ShowHiddenBorders => self.show_hidden_borders = on,
            _ => {}
        }
    }

    /// Convenience reader for the active `DocView`, mirroring
    /// `ToolDock::dock_action`.
    pub fn view_bar_action(&self, actions: &Actions) -> Option<ViewBarAction> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.cast() {
            ViewBarAction::None => None,
            action => Some(action),
        }
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p waml-editor view_bar::`
Expected: PASS — 4 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/waml-editor/src/view_bar.rs crates/waml-editor/src/main.rs
git commit -m "feat(view-bar): add the ViewBar widget with six view controls"
```

---

### Task 2: Mount `ViewBar` bottom-centre and expose it on the body seam

**Files:**
- Modify: `crates/waml-editor/src/app.rs` — DSL `use` list (`:26-37`), the `constraint_toggle_wrap` mount (`:274-284`), the selection-toolbar mount (`:295-305`), the `tool_dock` mount (`:265-272`), `sync_active_tab`'s source-tab visibility block (`:474-478`), `App::script_mod` (`:1943-1944`)
- Modify: `crates/waml-editor/src/doc_view.rs` — `BodyWidgets` accessors (`:29-51`)

**Interfaces:**
- Consumes: `mod.widgets.ViewBar` + `crate::view_bar::script_mod` from Task 1.
- Produces:
  - DSL ids `view_bar_wrap` / `view_bar` inside `center_stack`.
  - `BodyWidgets::view_bar(&self, cx: &mut Cx) -> WidgetRef`.
  - `BodyWidgets::set_view_bar_visible(&self, cx: &mut Cx, show: bool)`.

**Context:** `center_stack` (`app.rs:206`) is `flow: Overlay`; each HUD floater is a `Fill`/`Fill` wrapper `View` that parks itself with `align` and carries no background, so the canvas keeps pan/zoom in the gaps. `constraint_toggle_wrap` is the top-left floater being retired; `ViewBar` takes the bottom-centre slot and the contextual selection pill moves up to clear it (`12 + 36 + 8 = 56`). The `tool_dock` mount's `height: 308.0` carries a stale comment claiming `Fit` collapses to 0 — that was true before the `IconButton` extraction; the dock is five real child widgets in a `flow: Down` turtle now, so `Fit` measures correctly. `canvas_wrap` is hidden on a Source tab; `view_bar_wrap` must follow it, or the bar floats over rendered markdown.

- [ ] **Step 1: Register the widget module**

In `crates/waml-editor/src/app.rs`, in `App::script_mod`, replace line `:1944`:

```rust
        crate::constraint_toggle::script_mod(vm);
```

with:

```rust
        crate::view_bar::script_mod(vm);
```

This sits after `crate::tool_dock::script_mod(vm);` and well before `self::script_mod(vm)` at the end of the function — which is required, because `mod.widgets.*` resolves eagerly at `use`-time.

- [ ] **Step 2: Swap the DSL import**

In the `script_mod!` `use` list, replace `    use mod.widgets.ConstraintToggle` (`:27`) with:

```
    use mod.widgets.ViewBar
```

- [ ] **Step 3: Replace the top-left switcher mount with the bottom-centre bar**

Replace the whole `constraint_toggle_wrap` block (`:274-284`):

```
                                // Constraint-veil toggle: top-left of center.
                                constraint_toggle_wrap := View{
                                    width: Fill
                                    height: Fill
                                    align: Align{x: 0.0, y: 0.0}
                                    constraint_toggle := ConstraintToggle{
                                        width: 110.0
                                        height: 36.0
                                        margin: Inset{left: 12.0, top: 12.0}
                                    }
                                }
```

with:

```
                                // Canvas view bar: bottom-center, ALWAYS visible
                                // over a diagram, so its click targets never move.
                                // The contextual selection pill below stacks above
                                // it (bottom margin 12 + 36 + 8 = 56).
                                view_bar_wrap := View{
                                    width: Fill
                                    height: Fill
                                    align: Align{x: 0.5, y: 1.0}
                                    view_bar := ViewBar{
                                        width: Fit
                                        height: 36.0
                                        margin: Inset{bottom: 12.0}
                                    }
                                }
```

- [ ] **Step 4: Lift the selection pill clear of the bar**

In the selection-toolbar mount (`:300-304`), change `margin: Inset{bottom: 12.0}` to:

```
                                        margin: Inset{bottom: 56.0}
```

- [ ] **Step 5: Let the tool dock measure itself**

In the `tool_dock` mount (`:265-272`), replace:

```
                                    tool_dock := ToolDock{
                                        width: 48.0
                                        // Hugs its 7 buttons (7 * ITEM_H 44); the widget
                                        // draws items manually so Fit collapses to 0 --
                                        // an explicit height is required.
                                        height: 308.0
                                        margin: Inset{left: 12.0}
                                    }
```

with:

```
                                    tool_dock := ToolDock{
                                        width: 48.0
                                        // Five real `IconButton` children in a
                                        // `flow: Down` turtle since the IconButton
                                        // extraction, so `Fit` measures correctly.
                                        height: Fit
                                        margin: Inset{left: 12.0}
                                    }
```

- [ ] **Step 6: Add the body-seam accessors**

In `crates/waml-editor/src/doc_view.rs`, after the `selection_toolbar` accessor (`:38-40`), add:

```rust
    pub fn view_bar(&self, cx: &mut Cx) -> WidgetRef {
        self.ui.widget(cx, ids!(view_bar))
    }
```

and after `set_tool_dock_visible` (`:47-51`), add:

```rust
    /// Show/hide the bottom-centre view bar (`view_bar_wrap`). Follows the
    /// canvas: a Source tab renders markdown, not a diagram, so the view
    /// controls have nothing to act on.
    pub fn set_view_bar_visible(&self, cx: &mut Cx, show: bool) {
        self.ui.widget(cx, ids!(view_bar_wrap)).set_visible(cx, show);
    }
```

- [ ] **Step 7: Hide the bar on Source tabs**

In `crates/waml-editor/src/app.rs`, in `sync_active_tab`, immediately after:

```rust
        self.ui
            .widget(cx, ids!(canvas_wrap))
            .set_visible(cx, !is_source);
```

add:

```rust
        body.set_view_bar_visible(cx, !is_source);
```

- [ ] **Step 8: Verify the crate builds**

Run: `cargo build -p waml-editor`
Expected: FAIL — `app.rs` still borrows `crate::constraint_toggle::ConstraintToggle` at `:1584` and `main.rs` still declares the module. That handler is removed in Task 4; if you are running tasks strictly in order, expect this build to be red here and green after Task 4. To keep every task independently committable, do Tasks 2, 3 and 4 as one working set and gate once at the end of Task 4 — or reorder locally so Task 4's deletions land first.

- [ ] **Step 9: Commit (after Task 4's gate is green)**

```bash
git add crates/waml-editor/src/app.rs crates/waml-editor/src/doc_view.rs
git commit -m "feat(view-bar): mount ViewBar bottom-centre and expose it on the body seam"
```

---

### Task 3: Route `ViewBar` actions through the diagram view

**Files:**
- Modify: `crates/waml-editor/src/class_diagram_view.rs` (insert a `ViewBar` block after the tool-dock block at `:167-179`)

**Interfaces:**
- Consumes: `BodyWidgets::view_bar` (Task 2), `ViewBarAction` / `ViewOption` (Task 1), `GraphCanvas::set_constraint_vis` (`canvas.rs:2232`).
- Produces: the action match arms Plans C and D extend. Plan C fills in `ShowHiddenBorders`; Plan D fills in the four camera one-shots.

**Context:** `ClassDiagramView::handle` fully owns the diagram tab's body actions — tool dock, canvas pointer actions, selection toolbar — and each block returns early on a hit. `GraphCanvas::set_constraint_vis(cx, mode)` already exists and repaints; no new canvas API is needed for `ShowConstraints`.

- [ ] **Step 1: Add the routing block**

In `crates/waml-editor/src/class_diagram_view.rs`, immediately after the tool-dock block (which ends with its `return out;` and closing `}` at `:179`), insert:

```rust
        // View bar: `ShowConstraints` drives the canvas veil mode. The camera
        // one-shots and `ShowHiddenBorders` are `log!` no-ops here -- Plan D
        // wires the camera, Plan C wires the hidden borders.
        if let Some(action) = body
            .view_bar(cx)
            .borrow_mut::<crate::view_bar::ViewBar>()
            .and_then(|bar| bar.view_bar_action(actions))
        {
            match action {
                crate::view_bar::ViewBarAction::Toggled(
                    crate::view_bar::ViewOption::ShowConstraints,
                    on,
                ) => {
                    if let Some(mut canvas) =
                        body.canvas(cx).borrow_mut::<crate::canvas::GraphCanvas>()
                    {
                        canvas.set_constraint_vis(
                            cx,
                            if on {
                                crate::canvas::ConstraintVisibility::Selected
                            } else {
                                crate::canvas::ConstraintVisibility::None
                            },
                        );
                    }
                }
                other => log!("view bar: {other:?}"),
            }
            return out;
        }
```

- [ ] **Step 2: Verify the crate builds**

Run: `cargo build -p waml-editor`
Expected: same `constraint_toggle` breakage as Task 2 Step 8 until Task 4 lands; nothing new.

- [ ] **Step 3: Commit (after Task 4's gate is green)**

```bash
git add crates/waml-editor/src/class_diagram_view.rs
git commit -m "feat(view-bar): route ShowConstraints from the view bar to the canvas"
```

---

### Task 4: Delete the segmented switcher

**Files:**
- Delete: `crates/waml-editor/src/constraint_toggle.rs`
- Modify: `crates/waml-editor/src/main.rs` (drop `mod constraint_toggle;` at `:15`)
- Modify: `crates/waml-editor/src/app.rs` (drop the `constraint_toggle` handler at `:1580-1595`)

**Interfaces:**
- Consumes: nothing.
- Produces: nothing. Removes `ConstraintToggle`, `ConstraintToggleAction`, and the shell's `toggle_action` read.

**Context:** The switcher's DSL import and `script_mod` registration were already replaced in Task 2. Its two remaining references are the `mod` line and the shell handler that mapped `Picked(mode)` onto `set_constraint_vis`; the `ViewBar` block from Task 3 replaces that path. The file's own two tests (`default_active_is_selected`, `each_mode_maps_to_a_catalog_icon`) go with it — `default_active_is_selected` is re-asserted in Task 6's canvas tests.

- [ ] **Step 1: Delete the widget file**

```bash
git rm crates/waml-editor/src/constraint_toggle.rs
```

- [ ] **Step 2: Drop the module declaration**

In `crates/waml-editor/src/main.rs`, delete the line:

```rust
mod constraint_toggle;
```

- [ ] **Step 3: Drop the shell handler**

In `crates/waml-editor/src/app.rs`, delete the whole block at `:1580-1595`:

```rust
        // Constraint-veil visibility toggle -> canvas.
        let vis = self
            .ui
            .widget(cx, ids!(constraint_toggle))
            .borrow::<crate::constraint_toggle::ConstraintToggle>()
            .and_then(|t| t.toggle_action(actions));
        if let Some(mode) = vis {
            if let Some(mut canvas) = self
                .ui
                .widget(cx, ids!(canvas))
                .borrow_mut::<crate::canvas::GraphCanvas>()
            {
                canvas.set_constraint_vis(cx, mode);
            }
            return;
        }
```

- [ ] **Step 4: Verify no references survive**

Run: `grep -rn "constraint_toggle\|ConstraintToggle" crates/waml-editor/src/`
Expected: no output.

- [ ] **Step 5: Build and test**

Run: `cargo test -p waml-editor`
Expected: PASS. If `dead_code` fires on anything in `canvas.rs`, that is Task 6's work — note it and continue; the `ConstraintVisibility::All` variant is still constructed by `relations_for_visibility`'s own tests at this point, so it should not fire yet.

- [ ] **Step 6: Run the full gate**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: all green. This gate covers Tasks 2, 3 and 4 together.

- [ ] **Step 7: Commit**

```bash
git add -A crates/waml-editor/src/
git commit -m "refactor(view-bar): retire the segmented constraint switcher"
```

---

### Task 5: Collapse `ConstraintVisibility` to `None` | `Selected`

**Files:**
- Modify: `crates/waml-editor/src/canvas.rs` — `ConstraintVisibility` (`:471-488`), `relations_for_visibility` (`:494-512`)
- Test: `crates/waml-editor/src/canvas.rs` `mod tests` (the `relations_for_visibility` test's `All` assertion at `:2749-2753`)

**Interfaces:**
- Consumes: nothing new.
- Produces: `pub enum ConstraintVisibility { None, Selected }` — `#[default]` stays on `Selected`. `ConstraintVisibility::ALL` is **removed** (no consumer once the switcher is gone; under `-D warnings` an unconsumed associated const is a hard error).

**Context:** Nothing can enter `All` mode any more, so the variant and every branch that only ran in it go. **Selected-mode drawing must stay byte-identical** — every branch removed here was already skipped when the mode was `Selected`. This task removes the enum-level `All`; Task 6 removes the parallax/scrub machinery it gated.

- [ ] **Step 1: Update the failing test**

In `crates/waml-editor/src/canvas.rs`, in the `relations_for_visibility` test, delete the trailing `All` assertion (`:2749-2753`):

```rust
        // All: every relation.
        assert_eq!(
            relations_for_visibility(&rels, ConstraintVisibility::All, None).len(),
            3
        );
```

and add, in its place:

```rust
        // The default is `Selected` -- the bar's constraints toggle starts ON.
        assert_eq!(ConstraintVisibility::default(), ConstraintVisibility::Selected);
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p waml-editor canvas::tests::`
Expected: PASS (the edit only removed an assertion and added one that already holds). This step is a checkpoint, not a red bar — the red bar comes in Step 4 once `All` is gone and the compiler finds its remaining users.

- [ ] **Step 3: Collapse the enum**

Replace `crates/waml-editor/src/canvas.rs:469-488` with:

```rust
/// What constraint veils the canvas draws (spec §1). Persisted in view state and
/// driven by the view bar's constraints toggle.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ConstraintVisibility {
    /// No constraint marks — pure diagram.
    None,
    /// Selecting a node lights every constraint touching it (sticky). Default.
    #[default]
    Selected,
}
```

(The `impl ConstraintVisibility { pub const ALL: ... }` block is deleted outright — the retired switcher was its only consumer.)

- [ ] **Step 4: Drop the `All` arm from `relations_for_visibility`**

In `relations_for_visibility` (`:494-512`), delete the line:

```rust
        ConstraintVisibility::All => relations.iter().collect(),
```

and update the doc comment above it to drop the `All ⇒ every relation` clause:

```rust
/// The relations that should be drawn under a visibility mode + sticky selection
/// (spec §1). `None` ⇒ empty; `Selected` ⇒ relations touching `selected_key` as
/// subject OR reference (empty if nothing selected). Pure, GPU-free (mirrors
/// `node_at` selection logic).
```

- [ ] **Step 5: Run the test to see what still references `All`**

Run: `cargo test -p waml-editor canvas::tests::`
Expected: FAIL to compile — `no variant named 'All' found for enum 'ConstraintVisibility'` at the `handle_event` bracket-key guard (`:1088`) and the two `draw_relations_overlay` comparisons (`:1812`, `:1835`). Those are Task 6's deletions.

- [ ] **Step 6: Commit (after Task 6's gate is green)**

```bash
git add crates/waml-editor/src/canvas.rs
git commit -m "refactor(canvas): collapse ConstraintVisibility to None | Selected"
```

---

### Task 6: Delete the `All`-only parallax and layer-scrub machinery

**Files:**
- Modify: `crates/waml-editor/src/canvas.rs` — `scrub_layer` field (`:377-380`), `parallax_base` field (`:381-384`), bracket-key handler (`:1085-1101`), load-time `parallax_base` init (`:1377`), `PARALLAX_SPREAD` + `parallax_offset` (`:538-549`), `draw_relations_overlay`'s `pov` branch and parallax block (`:1809-1857`), `set_scene` resets (`:2149-2150`), `set_focus` resets (`:2172-2173`), `cycle_scrub_layer` (`:2237-2248`)
- Test: `crates/waml-editor/src/canvas.rs` — delete `parallax_offset_scales_with_depth_and_pan` (`:2790-2811`)

**Interfaces:**
- Consumes: nothing new.
- Produces: nothing new. `draw_relations_overlay` keeps its exact `Selected`-mode output.

**Context:** Every item here only ever fired in `All` mode: the `[`/`]` scrub keys were guarded on `constraint_vis == All`, and `draw_relations_overlay`'s parallax offset was `None` (i.e. `dvec2(0,0)`) in any other mode. With `All` gone, `pov` is unconditionally the selected key, and the per-layer offset is unconditionally zero. Deleting them is required, not optional: under `-D warnings` the now-unreachable `parallax_offset` helper and `scrub_layer` field are hard errors.

- [ ] **Step 1: Delete the bracket-key handler**

In `Widget::handle_event` for `GraphCanvas`, delete the whole block at `:1085-1101`:

```rust
        // Parallax layer scrub (spec §3): `[`/`]` advances the All-mode front
        // layer. Only meaningful (and only acts) in All mode.
        if let Event::KeyDown(ke) = event {
            if self.constraint_vis == ConstraintVisibility::All {
                match ke.key_code {
                    KeyCode::RBracket => {
                        self.cycle_scrub_layer(cx, 1);
                        return;
                    }
                    KeyCode::LBracket => {
                        self.cycle_scrub_layer(cx, -1);
                        return;
                    }
                    _ => {}
                }
            }
        }
```

- [ ] **Step 2: Simplify `draw_relations_overlay`**

Replace the body of `draw_relations_overlay` (`:1805-1858`) with:

```rust
    /// Persistent constraint overlay, gated by the visibility mode + sticky
    /// selection (spec §1): None draws nothing, Selected draws only relations
    /// touching the selected node.
    fn draw_relations_overlay(&mut self, cx: &mut Cx2d) {
        let selected_key = self.selected_key.clone();
        // Selected mode is the only drawing mode, so the veil always reframes
        // onto the selected node's POV.
        let pov = selected_key.as_deref();
        let chosen: Vec<(usize, usize, waml::syntax::Direction)> = relations_for_visibility(
            &self.scene.relations,
            self.constraint_vis,
            selected_key.as_deref(),
        )
        .into_iter()
        .filter_map(|rel| {
            let (subject, reference, dir) =
                reframe_to_selected(&rel.subject, &rel.reference, rel.dir, pov);
            let si = self.scene.nodes.iter().position(|n| n.key == subject)?;
            let ri = self.scene.nodes.iter().position(|n| n.key == reference)?;
            Some((si, ri, dir))
        })
        .collect();

        for (si, ri, dir) in chosen {
            self.draw_veil_for(cx, si, ri, dir, dvec2(0.0, 0.0));
        }
    }
```

- [ ] **Step 3: Delete the parallax helper and its constant**

Delete `PARALLAX_SPREAD` and `parallax_offset` (`:538-549`) — the doc comment, the `const`, and the whole `fn`.

- [ ] **Step 4: Delete the two struct fields and every reset**

- Delete the `scrub_layer` field and its doc comment (`:377-380`).
- Delete the `parallax_base` field and its doc comment (`:381-384`).
- In `draw_walk`, delete `                self.parallax_base = Some((self.camera.pan_x, self.camera.pan_y));` (`:1377`).
- In `set_scene`, delete `        self.parallax_base = None;` and `        self.scrub_layer = 0;` (`:2149-2150`).
- In `set_focus`, delete `        self.parallax_base = None;` and `        self.scrub_layer = 0;` (`:2172-2173`).

- [ ] **Step 5: Delete `cycle_scrub_layer`**

Delete the whole method (`:2237-2248`), including its doc comment.

- [ ] **Step 6: Delete the parallax test**

Delete `parallax_offset_scales_with_depth_and_pan` (`:2790-2811`).

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo test -p waml-editor`
Expected: PASS. `relations_for_visibility` keeps its `None`/`Selected` assertions; `reframe_puts_the_selected_node_in_the_clear` is unchanged (it tests the pure function, including its no-POV case, which is still reachable when nothing is selected).

- [ ] **Step 8: Run the full gate**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: all green, no `dead_code` errors.

- [ ] **Step 9: Commit**

```bash
git add crates/waml-editor/src/canvas.rs
git commit -m "refactor(canvas): drop the All-only parallax and layer-scrub machinery"
```

---

### Task 7: Interactive verification

**Files:**
- Modify: none (verification only)

**Interfaces:**
- Consumes: everything above.
- Produces: a per-pid visual sign-off. This is the **only** check that covers the two script-VM hazards — registration order and namespace shape — because the `cargo`/`pnpm` gate never boots the VM.

**Context:** `scripts/run-native.ps1` builds the checkout the script itself lives in (`$PSScriptRoot`), so launch the **worktree's own copy**, not main's. Capture the screenshot by the specific launched pid in one PowerShell call — screenshot-by-name grabs the user's own open editor, and `Stop-Process` by name kills their session.

- [ ] **Step 1: Launch the worktree build**

Run the worktree's own `scripts/run-native.ps1` on a diagram fixture, capturing the launched process id.

- [ ] **Step 2: Confirm the bar mounted and is not a dead node**

Check, at the bottom centre of the canvas:
- the HUD-framed strip is present, hugging six glyphs with a hairline divider after the fourth;
- all six glyphs render (a missing glyph or an invisible bar means `view_bar::script_mod(vm)` registered too late);
- **all other chrome text still renders** — caption model name, doc-tab labels, tree labels. Blank chrome text is the `mod.X` namespace-shape failure, not a font bug.

- [ ] **Step 3: Confirm the constraints toggle works end-to-end**

- The rightmost (ruler) button starts **lit** — constraints default ON.
- Click a node: the constraint veil draws as it does today.
- Click the ruler button: it unlights and the veil disappears.
- Click it again: it re-lights and the veil returns, identical to before.

- [ ] **Step 4: Confirm layout and stacking**

- The left tool dock is unchanged in appearance and hugs its five buttons (no dead space below them) after the `height: Fit` change.
- Open a classifier preview tab so the selection pill appears: it sits **above** the view bar with a clear gap, not overlapping it.
- Open a Source (View Source) tab: the view bar is hidden along with the canvas.
- The old top-left three-cell switcher is gone.

- [ ] **Step 5: Screenshot for the record and close the app by pid**

Capture by the pid from Step 1, in a single PowerShell call. Do not `Stop-Process` by name.

- [ ] **Step 6: Fix and re-verify, or record the sign-off**

If the bar is invisible, re-check that `crate::view_bar::script_mod(vm);` precedes `self::script_mod(vm)`. If chrome text blanked, re-check that every `mod.X` in `view_bar.rs` is one object literal with colon fields.

---

## Done when

- `crates/waml-editor/src/constraint_toggle.rs` is gone and `grep -rn "constraint_toggle\|ConstraintToggle" crates/waml-editor/src/` is empty.
- `ConstraintVisibility` has exactly `None` and `Selected`, `Selected` is `#[default]`, and `ALL` is gone.
- `scrub_layer`, `parallax_base`, `PARALLAX_SPREAD`, `parallax_offset`, `cycle_scrub_layer`, and the `[`/`]` handler are gone.
- `cargo test -p waml-editor view_bar::` is green (4 tests).
- The full gate (`cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`) is green.
- Interactively: the bar renders bottom-centre with six live glyphs, the ruler toggle flips the veil on and off, the selection pill clears the bar, the dock measures with `Fit`, and no chrome text blanked.
