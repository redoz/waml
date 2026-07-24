# ViewBar Plan D — Camera Actions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the view bar's four camera buttons real — zoom in, zoom out, fit the whole diagram, fit the selected node — and dim the fit-to-selection button when nothing is selected.

**Architecture:** All four actions are thin wrappers over existing, already-clamped `Camera` API (`zoom_at`, `fit`), driven from `ClassDiagramView::handle`'s view-bar match block. The load-time fit's `48.0` pad literal is extracted into a `FIT_PAD` const behind a single `fit_scene_camera` helper that both the load-time fit and the explicit fit action call, so the two cannot drift. The disabled state is a new `dim` mode on the shared `IconButton`, driven by a `ViewBar` flag that the diagram view pushes from the canvas's selection.

**Tech Stack:** Rust, makepad widgets + script DSL (`script_mod!`), `cargo test`.

## Global Constraints

- **Spec:** `docs/superpowers/specs/2026-07-24-canvas-view-bar-design.md` §4.
- **Depends on Plans A and B**, both landed on `origin/main` first. Plan A supplies `Icon::Maximize` / `Icon::ScanSearch`; Plan B supplies `ViewBar`, `ViewOption`, and the `ViewBarAction` match block in `ClassDiagramView::handle` whose `log!` catch-all this plan replaces. (The spec's Plan-split line calls A and D independent; that is loose — D extends B's widget and B's match block.)
- **Plan order:** A → B → {C, D}. C and D touch different arms of the same `match` and different regions of `canvas.rs`; if both are in flight, land one and rebase the other.
- **No clamping at the call sites.** `Camera::zoom_at` (`camera.rs:29`) and `Camera::fit` (`camera.rs:37`) both already clamp to `MIN_ZOOM`/`MAX_ZOOM`. Adding a second clamp is a bug, not belt-and-braces.
- **`focus_mode` keeps its zoom-1.0 special case for the initial framing** (`canvas.rs:1362`). The explicit fit actions deliberately ignore `focus_mode` — the user asked for a fit.
- **Out of scope:** no change to how constraints or groups are drawn; no model or wasm-ABI change; no new WAML syntax; no pan API.
- **`-D warnings` promotes rustc `dead_code` to a hard error.** Anything added must have a consumer or an explicit `#[allow(dead_code)]` with a reason.
- **Never edit the main checkout.** Work in a git worktree. `Edit`/`Write` take absolute paths and have no cwd — a main-root path edits main while the worktree build "passes" against a stale copy. Tell: a new test missing from the worktree's `cargo test -- --list`.
- **`scripts/run-native.ps1` builds the checkout the script lives in** (`$PSScriptRoot`), not your cwd. Launch the worktree's own copy.
- **Screenshot by specific pid, in one PowerShell call.** By-name capture grabs the user's own open editor; `Stop-Process` by name kills their session.
- **Full gate before each commit:** `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`.

---

### Task 1: Extract `FIT_PAD` behind one shared fit helper

**Files:**
- Modify: `crates/waml-editor/src/canvas.rs` — add the const + helper near the other pure canvas helpers (after `is_click`, around `:408`), and repoint the load-time fit (`draw_walk`, the `Camera::fit(bbox, rect.size.x, rect.size.y, 48.0)` line)
- Test: `crates/waml-editor/src/canvas.rs` `mod tests`

**Interfaces:**
- Consumes: `crate::camera::Camera`, `waml::solve::Rect`.
- Produces:
  - `const FIT_PAD: f64 = 48.0;`
  - `fn fit_scene_camera(bbox: waml::solve::Rect, viewport_w: f64, viewport_h: f64) -> Camera`.

**Context:** The pad is currently a bare `48.0` literal inside `draw_walk`'s one-shot load-time fit. Task 2's `fit_to_scene`/`fit_to_selection` need the same value, and a second literal is exactly the drift the spec calls out. A helper (rather than just a shared const) means there is a *single* `Camera::fit` call site carrying a pad, which is what makes drift impossible rather than merely unlikely. Note `canvas.rs` uses makepad's `Rect` from the prelude, so the solver rect must be spelled `waml::solve::Rect`.

- [ ] **Step 1: Write the failing test**

In `crates/waml-editor/src/canvas.rs`, inside `mod tests`, add:

```rust
    #[test]
    fn the_scene_fit_helper_uses_the_shared_pad() {
        let bbox = waml::solve::Rect {
            x: 0.0,
            y: 0.0,
            w: 200.0,
            h: 100.0,
        };
        assert_eq!(FIT_PAD, 48.0);
        // Both the load-time fit and the explicit fit action go through this
        // helper, so they cannot drift apart.
        assert_eq!(
            fit_scene_camera(bbox, 800.0, 600.0),
            crate::camera::Camera::fit(bbox, 800.0, 600.0, FIT_PAD)
        );
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p waml-editor canvas::tests::the_scene_fit_helper_uses_the_shared_pad`
Expected: FAIL to compile — `cannot find value 'FIT_PAD' in this scope`.

- [ ] **Step 3: Add the const and the helper**

In `crates/waml-editor/src/canvas.rs`, after the `is_click` helper (`:406-408`), add:

```rust
/// Viewport inset used whenever the canvas frames a bounding box — the one-shot
/// load-time fit and the view bar's explicit fit actions alike. Extracted so the
/// two cannot drift apart.
const FIT_PAD: f64 = 48.0;

/// Frame `bbox` in a `viewport_w` x `viewport_h` canvas at the shared pad. The
/// single `Camera::fit` call site that carries a pad. `Camera::fit` clamps zoom
/// to `MIN_ZOOM`/`MAX_ZOOM`, so callers need no clamping of their own. Pure,
/// GPU-free.
fn fit_scene_camera(bbox: waml::solve::Rect, viewport_w: f64, viewport_h: f64) -> Camera {
    Camera::fit(bbox, viewport_w, viewport_h, FIT_PAD)
}
```

- [ ] **Step 4: Repoint the load-time fit**

In `draw_walk`, in the `if !self.fitted` block, replace:

```rust
                    Camera::fit(bbox, rect.size.x, rect.size.y, 48.0)
```

with:

```rust
                    fit_scene_camera(bbox, rect.size.x, rect.size.y)
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p waml-editor canvas::tests::the_scene_fit_helper_uses_the_shared_pad`
Expected: PASS.

- [ ] **Step 6: Verify no bare pad literal survives**

Run: `grep -n "Camera::fit" crates/waml-editor/src/canvas.rs`
Expected: exactly one hit, inside `fit_scene_camera`.

- [ ] **Step 7: Commit**

```bash
git add crates/waml-editor/src/canvas.rs
git commit -m "refactor(canvas): extract FIT_PAD behind one shared fit helper"
```

---

### Task 2: Camera methods on `GraphCanvas`

**Files:**
- Modify: `crates/waml-editor/src/canvas.rs` — add methods in `impl GraphCanvas` beside `set_constraint_vis` (`:2231-2235`), and a `ZOOM_STEP` const beside `FIT_PAD`

**Interfaces:**
- Consumes: `fit_scene_camera` / `FIT_PAD` (Task 1), `Camera::zoom_at` (`camera.rs:29`), `scene::bounding_box` (`scene.rs:662`), `self.view_rect` (set every `draw_walk`, `canvas.rs:1357`).
- Produces:
  - `pub const ZOOM_STEP: f64 = 1.2;`
  - `pub fn zoom_step(&mut self, cx: &mut Cx, factor: f64)`
  - `pub fn fit_to_scene(&mut self, cx: &mut Cx)`
  - `pub fn fit_to_selection(&mut self, cx: &mut Cx)`
  - `pub fn has_selection(&self) -> bool`

**Context:** `self.view_rect` is the canvas's own rect, refreshed each `draw_walk`; it is `Rect::default()` (zero-sized) before the first draw, which is why the fits bail on a degenerate viewport. Zoom is anchored at the *viewport centre*, unlike the scroll path (`canvas.rs:1339-1350`), which anchors at the cursor: a button press has no cursor position to honour, and holding the middle of the canvas stable is the predictable behaviour. Setting `self.fitted = true` in `fit_to_scene` stops a pending one-shot load-time fit from stomping the user's explicit fit on the next draw. `SceneNode` has `key: String` and `rect: waml::solve::Rect`.

- [ ] **Step 1: Write the failing test**

In `crates/waml-editor/src/canvas.rs`, inside `mod tests`, add:

```rust
    #[test]
    fn zoom_step_is_a_symmetric_pair() {
        // In and out are exact inverses, so a press-and-undo round-trips.
        let mut cam = crate::camera::Camera {
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
        };
        cam.zoom_at(400.0, 300.0, ZOOM_STEP);
        cam.zoom_at(400.0, 300.0, 1.0 / ZOOM_STEP);
        assert!((cam.zoom - 1.0).abs() < 1e-9, "zoom drifted: {}", cam.zoom);
        assert!(cam.pan_x.abs() < 1e-9 && cam.pan_y.abs() < 1e-9);
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p waml-editor canvas::tests::zoom_step_is_a_symmetric_pair`
Expected: FAIL to compile — `cannot find value 'ZOOM_STEP' in this scope`.

- [ ] **Step 3: Add the constant**

In `crates/waml-editor/src/canvas.rs`, immediately after `FIT_PAD`, add:

```rust
/// Multiplicative step for one press of the view bar's zoom buttons. `ZoomIn`
/// multiplies by it, `ZoomOut` divides, so a press-and-undo round-trips exactly.
pub const ZOOM_STEP: f64 = 1.2;
```

- [ ] **Step 4: Add the four methods**

In `impl GraphCanvas`, immediately after `set_constraint_vis`, add:

```rust
    /// Zoom by `factor` about the VIEWPORT CENTRE (spec §4). Deliberately
    /// unlike the scroll path, which anchors at the cursor: a button press has
    /// no cursor to honour, so holding the middle of the canvas stable is the
    /// predictable behaviour. `Camera::zoom_at` clamps to `MIN_ZOOM`/`MAX_ZOOM`;
    /// at a bound this is simply a no-op.
    pub fn zoom_step(&mut self, cx: &mut Cx, factor: f64) {
        self.camera.zoom_at(
            self.view_rect.size.x * 0.5,
            self.view_rect.size.y * 0.5,
            factor,
        );
        self.draw_bg.redraw(cx);
    }

    /// Frame the whole scene (spec §4). An empty scene (`bounding_box` returns
    /// `None`) or a not-yet-drawn canvas is a no-op with no camera mutation.
    /// Marks the camera as fitted so a pending one-shot load-time fit cannot
    /// stomp this on the next draw, and ignores `focus_mode` -- the user asked
    /// for a fit.
    pub fn fit_to_scene(&mut self, cx: &mut Cx) {
        if self.view_rect.size.x <= 0.0 || self.view_rect.size.y <= 0.0 {
            return;
        }
        let Some(bbox) = bounding_box(&self.scene) else {
            return;
        };
        self.camera = fit_scene_camera(bbox, self.view_rect.size.x, self.view_rect.size.y);
        self.fitted = true;
        self.draw_bg.redraw(cx);
    }

    /// Frame the selected node (spec §4). No selection, a key with no node in
    /// this scene, or a not-yet-drawn canvas is a no-op.
    pub fn fit_to_selection(&mut self, cx: &mut Cx) {
        if self.view_rect.size.x <= 0.0 || self.view_rect.size.y <= 0.0 {
            return;
        }
        let Some(key) = self.selected_key.clone() else {
            return;
        };
        let Some(bbox) = self
            .scene
            .nodes
            .iter()
            .find(|n| n.key == key)
            .map(|n| n.rect)
        else {
            return;
        };
        self.camera = fit_scene_camera(bbox, self.view_rect.size.x, self.view_rect.size.y);
        self.fitted = true;
        self.draw_bg.redraw(cx);
    }

    /// Whether a node is currently selected — drives the view bar's
    /// fit-to-selection button between enabled and dim.
    pub fn has_selection(&self) -> bool {
        self.selected_key.is_some()
    }
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p waml-editor canvas::tests::zoom_step_is_a_symmetric_pair`
Expected: PASS.

- [ ] **Step 6: Verify the crate builds**

Run: `cargo build -p waml-editor`
Expected: FAIL with `dead_code` on `zoom_step`, `fit_to_scene`, `fit_to_selection`, `has_selection` — no consumer until Task 5. Expect red here and green after Task 5; treat Tasks 2-5 as one working set and gate once at the end of Task 5.

- [ ] **Step 7: Commit (after Task 5's gate is green)**

```bash
git add crates/waml-editor/src/canvas.rs
git commit -m "feat(canvas): add zoom_step, fit_to_scene and fit_to_selection"
```

---

### Task 3: A dim (disabled) state on the shared `IconButton`

**Files:**
- Modify: `crates/waml-editor/src/icon_button.rs` — DSL colour holders (`:62-64`), struct fields (`:88-115`), `handle_event` hover arm (`:127-131`), `draw_walk` tint pick (`:140-165`), `impl IconButton` (`:168-210`), `impl IconButtonRef` (`:212-241`)

**Interfaces:**
- Consumes: nothing new.
- Produces:
  - DSL holder `draw_icon_dim` on `mod.widgets.IconButton`.
  - `IconButton::set_dim(&mut self, cx: &mut Cx, dim: bool)` and `IconButtonRef::set_dim(&self, cx: &mut Cx, dim: bool)`.

**Context:** `IconButton` already picks its glyph tint per draw by copying `.color` from one of two colour-only DSL holders (`draw_icon_lit` / `draw_icon_idle`) — no RGBA crosses Rust. A third holder (`draw_icon_dim`, `atlas.text_dim`) extends that pattern. A dim button also suppresses its hover wash and its Hand cursor, so it reads as inert; it still emits `Clicked` (the host decides what a click on a disabled control means — `ViewBar` swallows it in Task 4). The button keeps its full size when dim, so the bar's width and the other buttons' positions never shift.

- [ ] **Step 1: Add the DSL holder**

In `crates/waml-editor/src/icon_button.rs`, in the `mod.widgets.IconButton` block, after `        draw_icon_idle +: { color: atlas.text }` (`:63`), add:

```
        // Disabled ink: the glyph greys out and the wash never lights, so a
        // no-op control reads inert without changing size or moving neighbours.
        draw_icon_dim +: { color: atlas.text_dim }
```

- [ ] **Step 2: Add the struct field and state**

After the `draw_icon_idle` field declaration (`:90-91`), add:

```rust
    #[live]
    draw_icon_dim: DrawColor,
```

and after the `active` field (`:106-107`), add:

```rust
    /// Disabled: the glyph greys and the wash never lights. The button keeps its
    /// size and still emits `Clicked` — the host decides what that means.
    #[rust]
    dim: bool,
```

- [ ] **Step 3: Suppress the hover affordance when dim**

In `handle_event`, replace the `Hit::FingerHoverIn` arm (`:127-131`):

```rust
            Hit::FingerHoverIn(_) => {
                cx.set_cursor(MouseCursor::Hand);
                self.hovered = true;
                self.view.redraw(cx);
            }
```

with:

```rust
            Hit::FingerHoverIn(_) => {
                if !self.dim {
                    cx.set_cursor(MouseCursor::Hand);
                }
                self.hovered = true;
                self.view.redraw(cx);
            }
```

- [ ] **Step 4: Pick the dim tint in `draw_walk`**

In `draw_walk`, replace:

```rust
        let lit = self.hovered || self.active;
```

with:

```rust
        // A dim button never lights, however it is hovered or flagged active.
        let lit = (self.hovered || self.active) && !self.dim;
```

and replace the tint pick (`:149-153`):

```rust
            let tint = if lit {
                self.draw_icon_lit.color
            } else {
                self.draw_icon_idle.color
            };
```

with:

```rust
            let tint = if lit {
                self.draw_icon_lit.color
            } else if self.dim {
                self.draw_icon_dim.color
            } else {
                self.draw_icon_idle.color
            };
```

- [ ] **Step 5: Add the setters**

In `impl IconButton`, after `set_active` (`:179-184`), add:

```rust
    /// Drive the disabled (dim) state, redrawing only on a change.
    pub fn set_dim(&mut self, cx: &mut Cx, dim: bool) {
        if self.dim != dim {
            self.dim = dim;
            self.view.redraw(cx);
        }
    }
```

In `impl IconButtonRef`, after its `set_active` (`:221-225`), add:

```rust
    /// See [`IconButton::set_dim`].
    pub fn set_dim(&self, cx: &mut Cx, dim: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_dim(cx, dim);
        }
    }
```

- [ ] **Step 6: Verify the crate builds**

Run: `cargo build -p waml-editor`
Expected: still red on Task 2's `dead_code` (and now `set_dim` too) until Task 5. No *new* kind of error.

- [ ] **Step 7: Commit (after Task 5's gate is green)**

```bash
git add crates/waml-editor/src/icon_button.rs
git commit -m "feat(icon-button): add a dim (disabled) state"
```

---

### Task 4: `ViewBar` fit-to-selection enablement

**Files:**
- Modify: `crates/waml-editor/src/view_bar.rs` — struct state, `handle_event`, `draw_walk`, `impl ViewBar`
- Test: `crates/waml-editor/src/view_bar.rs` `mod tests`

**Interfaces:**
- Consumes: `IconButtonRef::set_dim` (Task 3).
- Produces: `ViewBar::set_fit_to_selection_enabled(&mut self, cx: &mut Cx, on: bool)` and the pure `fn option_is_dim(opt: ViewOption, fit_enabled: bool) -> bool`. (No `fit_to_selection_enabled()` getter — nothing reads it, and under `-D warnings` an unused `pub` method is a hard error.)

**Context:** Per the spec, `FitToSelection` with nothing selected is a no-op and the button renders *dim* rather than disappearing, so the bar's width and the other buttons' positions never shift. `ViewBar` swallows the click rather than emitting a `Triggered` the view would have to ignore — one place decides, and the emitted-action stream stays meaningful. Both the draw-time dim push and the event-time swallow ask the same question, so it lives in one pure function that the tests can reach without a `Cx`. The flag starts `false` (nothing is selected before the first sync); Task 5 pushes the real value.

- [ ] **Step 1: Write the failing test**

In `crates/waml-editor/src/view_bar.rs`, inside `mod tests`, add:

```rust
    #[test]
    fn only_fit_to_selection_dims_and_only_when_disabled() {
        for opt in ViewOption::ALL {
            // With a selection, nothing is dim.
            assert!(!option_is_dim(opt, true), "{opt:?} dim despite a selection");
            // With no selection, only FitToSelection dims.
            assert_eq!(
                option_is_dim(opt, false),
                opt == ViewOption::FitToSelection,
                "{opt:?} dim-ness mismatch"
            );
        }
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p waml-editor view_bar::`
Expected: FAIL to compile — `cannot find function 'option_is_dim' in this scope`.

- [ ] **Step 3: Add the predicate and the state field**

At module level in `crates/waml-editor/src/view_bar.rs`, after the `ViewOption` impl block, add:

```rust
/// Whether an option's button renders dim (and swallows its own clicks). Only
/// `FitToSelection` ever does, and only when nothing is selected. Pure, so the
/// draw-time push and the event-time swallow can't drift apart.
fn option_is_dim(opt: ViewOption, fit_enabled: bool) -> bool {
    opt == ViewOption::FitToSelection && !fit_enabled
}
```

Then, in `pub struct ViewBar`, after `show_hidden_borders`, add:

```rust
    /// Whether `FitToSelection` has anything to fit. Pushed by the diagram view
    /// from the canvas's selection; when false the button draws dim and swallows
    /// its own clicks. Starts false — nothing is selected before the first sync.
    #[rust]
    fit_to_selection_enabled: bool,
```

- [ ] **Step 4: Swallow the click when disabled**

In `handle_event`, replace the body of the `if ... clicked(actions)` block's opening so the disabled button is inert. The loop becomes:

```rust
            for opt in ViewOption::ALL {
                if self.button(cx, opt).as_icon_button().clicked(actions) {
                    // A dim FitToSelection has nothing to fit: swallow the click
                    // here so the action stream only ever carries real intent.
                    if option_is_dim(opt, self.fit_to_selection_enabled) {
                        break;
                    }
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
```

- [ ] **Step 5: Push the dim state each draw**

In `draw_walk`, inside the per-option sync loop, after `btn.set_active(cx, lit);` add:

```rust
            btn.set_dim(cx, option_is_dim(opt, self.fit_to_selection_enabled));
```

- [ ] **Step 6: Add the setter**

In `impl ViewBar`, after `view_bar_action`, add:

```rust
    /// Drive the fit-to-selection button between enabled and dim, redrawing only
    /// on a change (this is pushed on every action batch).
    pub fn set_fit_to_selection_enabled(&mut self, cx: &mut Cx, on: bool) {
        if self.fit_to_selection_enabled != on {
            self.fit_to_selection_enabled = on;
            self.view.redraw(cx);
        }
    }
```

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo test -p waml-editor view_bar::`
Expected: PASS — Plan B's four tests plus `only_fit_to_selection_dims_and_only_when_disabled`.

- [ ] **Step 8: Commit (after Task 5's gate is green)**

```bash
git add crates/waml-editor/src/view_bar.rs
git commit -m "feat(view-bar): dim fit-to-selection when nothing is selected"
```

---

### Task 5: Wire the camera actions and the selection-driven enablement

**Files:**
- Modify: `crates/waml-editor/src/class_diagram_view.rs` — the top of `handle` (before the first early return, `:118`), and the view-bar match block's `log!` catch-all
- Modify: `crates/waml-editor/src/classifier_preview_view.rs` — `sync`, beside the existing `toolbar.set_selection(cx, Some(1))` call

**Interfaces:**
- Consumes: `GraphCanvas::{zoom_step, fit_to_scene, fit_to_selection, has_selection}` + `ZOOM_STEP` (Task 2), `ViewBar::set_fit_to_selection_enabled` (Task 4), `ViewBarAction::Triggered` (Plan B).
- Produces: nothing new.

**Context:** `ClassDiagramView::handle` runs on every `Event::Actions` batch and returns early once a block claims the batch, so the enablement push must sit at the very top, before the first `return`. It reads current state: `GraphCanvas` mutates `selected_key` during `handle_event`, which runs *before* `Event::Actions` is dispatched, so a click that selects a node is already visible in the same batch. `set_fit_to_selection_enabled` only redraws on a change, so pushing every batch is cheap. The classifier preview tab shares the canvas but never selects a node, so it pins the button dim.

- [ ] **Step 1: Push the enablement at the top of `handle`**

In `crates/waml-editor/src/class_diagram_view.rs`, in `impl DocView for ClassDiagramView`, immediately after `        let mut out = ViewOutcome::default();` (`:118`) and before the inline-edit block, insert:

```rust
        // Keep the view bar's fit-to-selection button in step with the canvas
        // selection. `GraphCanvas` mutates `selected_key` in `handle_event`,
        // which runs before `Event::Actions` is dispatched, so this reads the
        // selection as of THIS batch. Cheap: the setter redraws only on change.
        let has_selection = body
            .canvas(cx)
            .borrow::<crate::canvas::GraphCanvas>()
            .is_some_and(|c| c.has_selection());
        if let Some(mut bar) = body
            .view_bar(cx)
            .borrow_mut::<crate::view_bar::ViewBar>()
        {
            bar.set_fit_to_selection_enabled(cx, has_selection);
        }
```

- [ ] **Step 2: Push the same state on `sync`**

In the same file, in `DocView::sync`, immediately after the `toolbar.set_selection(cx, None);` block (`:103-108`), add:

```rust
        // A fresh scene clears the selection (`set_scene`), so the button starts
        // dim on every diagram activation.
        if let Some(mut bar) = body
            .view_bar(cx)
            .borrow_mut::<crate::view_bar::ViewBar>()
        {
            bar.set_fit_to_selection_enabled(cx, false);
        }
```

- [ ] **Step 3: Pin the button dim on the classifier preview tab**

In `crates/waml-editor/src/classifier_preview_view.rs`, in `sync`, immediately after the `toolbar.set_selection(cx, Some(1));` block, add:

```rust
        // The preview tab focuses one classifier but never selects a canvas
        // node, so fit-to-selection has no target here.
        if let Some(mut bar) = body
            .view_bar(cx)
            .borrow_mut::<crate::view_bar::ViewBar>()
        {
            bar.set_fit_to_selection_enabled(cx, false);
        }
```

- [ ] **Step 4: Replace the `log!` catch-all with the four camera arms**

In `crates/waml-editor/src/class_diagram_view.rs`, in the view-bar `match action { ... }`, replace:

```rust
                other => log!("view bar: {other:?}"),
```

with:

```rust
                crate::view_bar::ViewBarAction::Triggered(opt) => {
                    if let Some(mut canvas) =
                        body.canvas(cx).borrow_mut::<crate::canvas::GraphCanvas>()
                    {
                        match opt {
                            crate::view_bar::ViewOption::ZoomIn => {
                                canvas.zoom_step(cx, crate::canvas::ZOOM_STEP)
                            }
                            crate::view_bar::ViewOption::ZoomOut => {
                                canvas.zoom_step(cx, 1.0 / crate::canvas::ZOOM_STEP)
                            }
                            crate::view_bar::ViewOption::FitToSize => canvas.fit_to_scene(cx),
                            crate::view_bar::ViewOption::FitToSelection => {
                                canvas.fit_to_selection(cx)
                            }
                            // The toggles never arrive as `Triggered`.
                            _ => {}
                        }
                    }
                }
                other => log!("view bar: {other:?}"),
```

and update the block's leading comment to drop the "Plan D wires them" clause.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p waml-editor`
Expected: PASS, no `dead_code` errors.

- [ ] **Step 6: Run the full gate**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: all green. This gate covers Tasks 2, 3, 4 and 5 together.

- [ ] **Step 7: Commit**

```bash
git add crates/waml-editor/src/class_diagram_view.rs crates/waml-editor/src/classifier_preview_view.rs
git commit -m "feat(view-bar): wire the four camera actions to the canvas"
```

---

### Task 6: Interactive verification

**Files:**
- Modify: none (verification only)

**Interfaces:**
- Consumes: everything above.
- Produces: a per-pid visual sign-off.

**Context:** Launch the **worktree's own** `scripts/run-native.ps1` — it builds the checkout the script lives in, so main's stale binary starts otherwise. Capture by the specific launched pid in one PowerShell call; never `Stop-Process` by name.

- [ ] **Step 1: Launch on the mini fixture**

Run the worktree's `scripts/run-native.ps1` with no fixture argument (defaults to `crates/waml-editor/tests/fixtures/mini`), capturing the launched pid.

- [ ] **Step 2: Zoom holds the viewport centre**

- Note which card sits at the middle of the canvas.
- Press the zoom-in button several times: the view magnifies and that card stays at the centre — it must **not** drift toward a corner the way cursor-anchored scroll zoom does.
- Press zoom-out the same number of times: the view returns to (visually) where it started.
- Hold zoom-in until it stops magnifying (the `MAX_ZOOM` clamp): the button stays enabled and simply does nothing further — no jump, no flicker. Same for zoom-out at `MIN_ZOOM`.

- [ ] **Step 3: Fit to size frames the whole diagram**

- Pan and zoom somewhere arbitrary.
- Press the `maximize` button: every node comes into view, centred, with a visible margin on all four sides.
- Press it again: nothing moves (it is already fitted).

- [ ] **Step 4: Fit to selection**

- With nothing selected, the `scan-search` button is **dim** and clicking it does nothing at all.
- Click a node: the button un-dims immediately.
- Press it: the view frames that one node, centred, with a margin.
- Click empty canvas to deselect: the button goes dim again.

- [ ] **Step 5: Check the other tabs**

- Open a classifier preview tab: the `scan-search` button is dim there.
- Switch back to the diagram tab and re-select a node: it un-dims.

- [ ] **Step 6: Screenshot for the record and close by pid**

Capture by the pid from Step 1, in a single PowerShell call.

---

## Done when

- `FIT_PAD` is the only pad literal and there is exactly one `Camera::fit` call site in `canvas.rs`.
- `cargo test -p waml-editor canvas::tests::the_scene_fit_helper_uses_the_shared_pad` and `::zoom_step_is_a_symmetric_pair` are green.
- `cargo test -p waml-editor view_bar::` is green (5 tests).
- The full gate (`cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`) is green.
- Interactively: zoom holds the viewport centre and round-trips, fit-to-size frames the whole diagram with a margin, fit-to-selection frames one node, and the fit-to-selection button is dim and inert with nothing selected — with the bar's width and button positions unchanged between the two states.
