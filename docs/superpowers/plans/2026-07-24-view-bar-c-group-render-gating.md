# ViewBar Plan C — Group Render Gating + Hidden Borders Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop the native canvas drawing chrome for every group. Only `Shape::Frame` groups draw; `Box`/`Shrink` groups draw nothing unless the view bar's hidden-borders x-ray is on, in which case they draw as a dashed hairline with a dim title.

**Architecture:** `SolvedGroup.shape` already reaches the native renderer and is simply never read (`canvas.rs:1390-1418` draws all groups). This plan introduces a pure tri-state predicate `group_draw_mode(shape, show_hidden) -> GroupDraw`, a new `GroupDashed` DSL pen for the x-ray outline, a `show_hidden_borders` flag on `GraphCanvas` with a `set_show_hidden_borders` setter mirroring `set_constraint_vis`, and rewrites the group draw loop around the predicate. Purely a render decision — no model, solver, or wasm-ABI change.

**Tech Stack:** Rust, makepad `Sdf2d` shaders + script DSL (`script_mod!`), `cargo test`.

## Global Constraints

- **Spec:** `docs/superpowers/specs/2026-07-24-canvas-view-bar-design.md` §3.
- **Depends on Plans A and B**, both landed on `origin/main` first. Plan A supplies `Icon::SquareDashed`; Plan B supplies `ViewBar`, `ViewOption::ShowHiddenBorders`, and the `ViewBarAction` match block in `ClassDiagramView::handle` that this plan extends.
- **Plan order:** A → B → {C, D}. C and D both edit `class_diagram_view.rs`'s `ViewBar` match block but in *different arms* (C: `ShowHiddenBorders`; D: the four camera one-shots), and different regions of `canvas.rs` (C: the group draw loop and pens; D: camera methods).
- **This is a real, intended visible change.** A diagram with no `frame` group loses all group chrome by default. That is what the spec says (`docs/uaml-spec.md:674-681`: `frame` = visible titled box; `box`/`shrink` = layout only), what the web renderer already does, and the "grey mess" fix this work exists for. Do not soften it.
- **Out of scope:** no model or wasm-ABI change (`SolvedGroup`, `Shape` stay as they are); no new WAML syntax (`frame`/`box`/`shrink` already parse and already reach the renderer); no solver work.
- **Shader constraint:** an `if` on a uniform **silently no-ops** in this fork's shader VM (see the `EdgeMarker` comment at `canvas.rs:106-109`). Every new `pixel:` fn must be branch-free — select with arithmetic (multiply by a 0/1 mask), never with control flow.
- **Colour convention:** no RGBA crosses Rust. Tints come from DSL atlas-token holders whose `.color` is copied per draw (the `IconButton` `draw_icon_lit`/`draw_icon_idle` pattern).
- **`-D warnings` promotes rustc `dead_code` to a hard error.** Anything added must have a consumer or an explicit `#[allow(dead_code)]` with a reason.
- **Never edit the main checkout.** Work in a git worktree. `Edit`/`Write` take absolute paths and have no cwd — a main-root path edits main while the worktree build "passes" against a stale copy. Tell: a new test missing from the worktree's `cargo test -- --list`.
- **`scripts/run-native.ps1` builds the checkout the script lives in** (`$PSScriptRoot`), not your cwd. Launch the worktree's own copy.
- **Screenshot by specific pid, in one PowerShell call.** By-name capture grabs the user's own open editor; `Stop-Process` by name kills their session.
- **Do not mutate the `mini` or `sixkind` fixtures.** Splitting a shared diagram fixture into groups has previously broken `scene.rs` (cross-box `Layout` placements drop to `LayoutConflict`). Task 4 adds a *new* fixture instead.
- **Full gate before each commit:** `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`.

---

### Task 1: The pure draw-mode predicate and the untitled-group label

**Files:**
- Modify: `crates/waml-editor/src/canvas.rs` (add after `relations_for_visibility`, around `:512`)
- Test: `crates/waml-editor/src/canvas.rs` `mod tests` (`:2270`)

**Interfaces:**
- Consumes: `waml::syntax::Shape` (`Frame` | `Box` | `Shrink`, `#[derive(Debug, Clone, Copy, PartialEq)]` at `crates/waml/src/syntax.rs:277`).
- Produces:
  - `pub enum GroupDraw { Chrome, Dashed, Skip }` (`Clone, Copy, PartialEq, Eq, Debug`).
  - `fn group_draw_mode(shape: waml::syntax::Shape, show_hidden: bool) -> GroupDraw`.
  - `fn untitled_label(n: usize) -> String`.

**Context:** The spec's §"Testing" names this predicate `group_draws_chrome(shape, show_hidden) -> bool`. A boolean cannot express the three outcomes the renderer actually needs (full chrome / dashed outline / draw nothing), so this plan ships a tri-state `group_draw_mode` instead and covers all six `(Shape, bool)` combinations, which was the point of the spec's test. `SolvedGroup.title` is `None` for unnamed (inline) groups (`resolve.rs:92-96`); the fallback label is renderer-side only.

- [ ] **Step 1: Write the failing tests**

In `crates/waml-editor/src/canvas.rs`, inside `mod tests`, add:

```rust
    #[test]
    fn only_frame_groups_draw_chrome() {
        use waml::syntax::Shape;
        // Frame always draws its chrome, x-ray or not.
        assert_eq!(group_draw_mode(Shape::Frame, false), GroupDraw::Chrome);
        assert_eq!(group_draw_mode(Shape::Frame, true), GroupDraw::Chrome);
        // Box/Shrink are layout-only: invisible by default...
        assert_eq!(group_draw_mode(Shape::Box, false), GroupDraw::Skip);
        assert_eq!(group_draw_mode(Shape::Shrink, false), GroupDraw::Skip);
        // ...and dashed under the hidden-borders x-ray.
        assert_eq!(group_draw_mode(Shape::Box, true), GroupDraw::Dashed);
        assert_eq!(group_draw_mode(Shape::Shrink, true), GroupDraw::Dashed);
    }

    #[test]
    fn untitled_groups_get_a_one_based_label() {
        assert_eq!(untitled_label(1), "Untitled 1");
        assert_eq!(untitled_label(2), "Untitled 2");
        assert_eq!(untitled_label(12), "Untitled 12");
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-editor canvas::tests::only_frame_groups_draw_chrome canvas::tests::untitled_groups_get_a_one_based_label`
Expected: FAIL to compile — `cannot find function 'group_draw_mode' in this scope`.

- [ ] **Step 3: Write the predicate**

In `crates/waml-editor/src/canvas.rs`, after `relations_for_visibility` (i.e. after `:512`) and before `reframe_to_selected`, add:

```rust
/// What chrome a group gets this frame (spec §3). Three outcomes, not two: a
/// group either draws its full chrome, draws only the x-ray outline, or draws
/// nothing at all.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GroupDraw {
    /// Tinted fill + title — a `frame` group, exactly as the canvas drew every
    /// group before this change.
    Chrome,
    /// Dashed hairline + dim title, no fill — a layout-only group revealed by
    /// the hidden-borders x-ray.
    Dashed,
    /// Nothing. A layout-only group with the x-ray off.
    Skip,
}

/// Per-group render decision. Only `Shape::Frame` opts into chrome
/// (`docs/uaml-spec.md:674-681`); `box`/`shrink` reserve space without drawing,
/// which is what the web renderer already does. The hidden-borders toggle is an
/// x-ray that brings the invisible ones back as dashed outlines. Pure, GPU-free.
fn group_draw_mode(shape: waml::syntax::Shape, show_hidden: bool) -> GroupDraw {
    match (shape, show_hidden) {
        (waml::syntax::Shape::Frame, _) => GroupDraw::Chrome,
        (_, true) => GroupDraw::Dashed,
        (_, false) => GroupDraw::Skip,
    }
}

/// Display name for an unnamed group under the x-ray. `SolvedGroup.title` is
/// `None` for inline groups; `n` is a 1-based counter over the untitled groups
/// in `scene.groups` order, so the labels are stable across redraws of the same
/// scene. Renderer-side only — no model change.
fn untitled_label(n: usize) -> String {
    format!("Untitled {n}")
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p waml-editor canvas::tests::only_frame_groups_draw_chrome canvas::tests::untitled_groups_get_a_one_based_label`
Expected: PASS, 2 tests. (`dead_code` will not fire yet — both items are consumed by the tests in the same crate build; if it does, land Task 3 before gating.)

- [ ] **Step 5: Commit**

```bash
git add crates/waml-editor/src/canvas.rs
git commit -m "feat(canvas): add the pure group draw-mode predicate"
```

---

### Task 2: The dashed hidden-border pen and the canvas flag

**Files:**
- Modify: `crates/waml-editor/src/canvas.rs` — `script_mod!` pens (insert after the `ConstraintVeil` block, `:160`), the `GraphCanvas` DSL instance (`:162-166`), the struct's draw fields (`:263-265`), the struct's `#[rust]` state fields (near `:376`), a new setter beside `set_constraint_vis` (`:2231-2235`)

**Interfaces:**
- Consumes: nothing new.
- Produces:
  - DSL pen `mod.draw.GroupDashed` with uniforms `dash_px` (f32) and `stroke_w` (f32).
  - `GraphCanvas` fields `draw_group_dashed: DrawColor`, `draw_group_title_dim: DrawColor`, `show_hidden_borders: bool`.
  - `pub fn set_show_hidden_borders(&mut self, cx: &mut Cx, on: bool)`.

**Context:** `draw_group` today is a plain `DrawColor` tinted `atlas.group_fill` — it fills the group rect, and the title is drawn separately with `draw_text`. The dashed pen strokes the rect border with no fill. Parameterising the dash off `(pos.x + pos.y)` makes the pattern continuous across all four sides and around the corners, which stamping per-side segments does not. `draw_group_title_dim` is a colour-only holder — never drawn; its `.color` is copied into `draw_text.color` for the dim title, so no RGBA is written in Rust.

- [ ] **Step 1: Add the pen to the DSL**

In `crates/waml-editor/src/canvas.rs`, after the `mod.draw.ConstraintVeil` block (which closes at `:160`), add:

```
    // Hidden-group border pen (the x-ray, spec §3): a dashed hairline on the
    // group rect with NO fill. The dash rides the (x+y) diagonal so the pattern
    // stays continuous across all four sides and around the corners, unlike
    // per-side stamping. `dash_px` is pushed per draw so the dash grows with
    // zoom. Branch-free: an `if` on a uniform silently no-ops in this fork's
    // shader VM (see EdgeMarker above), so the on/off duty is a 0..1 mask
    // multiplied into the stroke alpha.
    mod.draw.GroupDashed = mod.draw.DrawColor{
        dash_px: uniform(6.0)
        stroke_w: uniform(1.0)
        pixel: fn() {
            let p = self.pos * self.rect_size
            let sdf = Sdf2d.viewport(p)
            let inset = self.stroke_w * 0.5
            sdf.rect(inset, inset, self.rect_size.x - inset * 2.0, self.rect_size.y - inset * 2.0)
            // 50% duty cycle with ~1px of antialiasing on the leading edge.
            let f = fract((p.x + p.y) / self.dash_px)
            let mask = clamp((0.5 - f) * self.dash_px, 0.0, 1.0)
            sdf.stroke(
                vec4(self.color.x, self.color.y, self.color.z, self.color.w * mask),
                self.stroke_w
            )
            return sdf.result
        }
    }
```

- [ ] **Step 2: Instance the pen and the title-ink holder on `GraphCanvas`**

In the `mod.widgets.GraphCanvas` block, immediately after `        draw_group +: { color: atlas.group_fill }` (`:166`), add:

```
        // Hidden-group x-ray outline; dim ink so it reads as secondary chrome.
        draw_group_dashed: mod.draw.GroupDashed{ color: atlas.text_dim }
        // Colour-only holder (never drawn): the dim ink copied onto `draw_text`
        // for a hidden group's title, so no RGBA crosses Rust.
        draw_group_title_dim +: { color: atlas.text_dim }
```

- [ ] **Step 3: Add the matching struct fields**

In `pub struct GraphCanvas`, immediately after the `draw_group` field (`:263-265`):

```rust
    #[redraw]
    #[live]
    draw_group: DrawColor,
```

add:

```rust
    #[redraw]
    #[live]
    draw_group_dashed: DrawColor,
    /// Colour-only holder for a hidden group's dim title ink (never drawn).
    #[live]
    draw_group_title_dim: DrawColor,
```

- [ ] **Step 4: Add the state flag**

In the same struct, immediately after the `constraint_vis` field (`:374-376`), add:

```rust
    /// X-ray for groups that opt out of chrome (spec §3): when on, `box`/
    /// `shrink` groups draw a dashed hairline instead of nothing. Default off.
    #[rust]
    show_hidden_borders: bool,
```

- [ ] **Step 5: Add the setter**

In `impl GraphCanvas`, immediately after `set_constraint_vis` (`:2231-2235`), add:

```rust
    /// Toggle the hidden-group-border x-ray and repaint.
    pub fn set_show_hidden_borders(&mut self, cx: &mut Cx, on: bool) {
        self.show_hidden_borders = on;
        self.draw_bg.redraw(cx);
    }
```

- [ ] **Step 6: Verify the crate builds**

Run: `cargo build -p waml-editor`
Expected: FAIL with `dead_code` on `set_show_hidden_borders`, `show_hidden_borders`, `draw_group_dashed`, and `draw_group_title_dim` — they have no consumer until Tasks 3 and 5. Expect this build to be red here and green after Task 3 (fields) and Task 5 (setter). Treat Tasks 2, 3 and 5 as one working set and gate once at the end of Task 5.

- [ ] **Step 7: Commit (after Task 5's gate is green)**

```bash
git add crates/waml-editor/src/canvas.rs
git commit -m "feat(canvas): add the dashed hidden-group pen and x-ray flag"
```

---

### Task 3: Rewrite the group draw loop around the predicate

**Files:**
- Modify: `crates/waml-editor/src/canvas.rs` — the group draw loop in `draw_walk` (`:1390-1418`)

**Interfaces:**
- Consumes: `group_draw_mode`, `untitled_label`, `GroupDraw` (Task 1); `draw_group_dashed`, `draw_group_title_dim`, `show_hidden_borders` (Task 2).
- Produces: nothing new — this is the render behaviour itself.

**Context:** The existing loop collects `(screen_rect, title)` pairs first so `self.draw_group` (which takes `&mut self`) can draw without holding the `self.scene.groups` borrow; keep that shape. Draw order stays shallow-first so inner groups land on top of outer ones. `draw_text.color` is shared with the node titles drawn later in the same `draw_walk`, so it must be restored after the group loop. The zoom-scaled title metrics (`12.0 * zoom` font size, `6.0 * zoom` / `4.0 * zoom` offsets) are unchanged.

- [ ] **Step 1: Replace the loop**

Replace `crates/waml-editor/src/canvas.rs:1390-1418` in full with:

```rust
        // Groups: only a `frame`-shaped group draws chrome -- `box`/`shrink` are
        // layout-only per docs/uaml-spec.md:674-681, which is what the web
        // renderer already does; this brings native into line. The view bar's
        // hidden-borders x-ray brings the invisible ones back as a dashed
        // hairline with a dim title (`Untitled N` when the group is unnamed).
        // Nesting is unchanged: draw-order (shallow first) leaves inner groups on
        // top. Collect screen rects first so the pens (&mut self) can draw
        // without holding the `self.scene.groups` borrow.
        let show_hidden = self.show_hidden_borders;
        let mut untitled_seen = 0usize;
        let group_draws: Vec<(Rect, Option<String>, GroupDraw)> = self
            .scene
            .groups
            .iter()
            .filter_map(|g| {
                let mode = group_draw_mode(g.shape, show_hidden);
                // Count every untitled group, drawn or not, so the labels are
                // stable regardless of which ones the x-ray is showing.
                let untitled = if g.title.is_none() {
                    untitled_seen += 1;
                    Some(untitled_seen)
                } else {
                    None
                };
                if mode == GroupDraw::Skip {
                    return None;
                }
                // A `frame` group with no name draws no title (as before); only
                // the x-ray outline gets the `Untitled N` fallback.
                let label = match (&g.title, untitled) {
                    (Some(t), _) => Some(t.clone()),
                    (None, Some(n)) if mode == GroupDraw::Dashed => Some(untitled_label(n)),
                    _ => None,
                };
                let (lx, ly) = self.camera.world_to_local(g.rect.x, g.rect.y);
                let screen = Rect {
                    pos: dvec2(rect.pos.x + lx, rect.pos.y + ly),
                    size: dvec2(g.rect.w * self.camera.zoom, g.rect.h * self.camera.zoom),
                };
                Some((screen, label, mode))
            })
            .collect();
        // Group titles borrow the shared body pen; stash its ink so the node
        // titles drawn later in this pass are unaffected by the dim override.
        let title_ink = self.draw_text.color;
        let dim_ink = self.draw_group_title_dim.color;
        // Dash period grows with zoom but stays legible at either extreme.
        let dash_px = (6.0 * zoom).clamp(3.0, 18.0) as f32;
        for (screen, label, mode) in group_draws {
            match mode {
                GroupDraw::Chrome => self.draw_group.draw_abs(cx, screen),
                GroupDraw::Dashed => {
                    self.draw_group_dashed
                        .set_uniform(cx, live_id!(dash_px), &[dash_px]);
                    self.draw_group_dashed.draw_abs(cx, screen);
                }
                GroupDraw::Skip => {}
            }
            if let Some(label) = &label {
                self.draw_text.color = if mode == GroupDraw::Dashed {
                    dim_ink
                } else {
                    title_ink
                };
                self.draw_text.text_style.font_size = (12.0 * zoom) as f32;
                self.draw_text.draw_abs(
                    cx,
                    dvec2(screen.pos.x + 6.0 * zoom, screen.pos.y + 4.0 * zoom),
                    label,
                );
            }
        }
        self.draw_text.color = title_ink;
```

- [ ] **Step 2: Verify the crate builds and tests pass**

Run: `cargo test -p waml-editor canvas::`
Expected: PASS. `dead_code` on `set_show_hidden_borders` remains until Task 5.

- [ ] **Step 3: Commit (after Task 5's gate is green)**

```bash
git add crates/waml-editor/src/canvas.rs
git commit -m "feat(canvas): gate group chrome on Shape::Frame, x-ray the rest"
```

---

### Task 4: A fixture with both a framed and a default group

**Files:**
- Create: `crates/waml-editor/tests/fixtures/groups/index.md`
- Create: `crates/waml-editor/tests/fixtures/groups/customer.md`
- Create: `crates/waml-editor/tests/fixtures/groups/account.md`
- Create: `crates/waml-editor/tests/fixtures/groups/order.md`
- Create: `crates/waml-editor/tests/fixtures/groups/invoice.md`
- Create: `crates/waml-editor/tests/fixtures/groups/groups-diagram.md`

**Interfaces:**
- Consumes: nothing.
- Produces: a bundle path (`crates/waml-editor/tests/fixtures/groups`) for `scripts/run-native.ps1` in Task 6.

**Context:** Named groups come from `### Heading` sub-sections under `## Members`; `## Layout` attaches hints with `- <Group> as column with frame`. A group with no shape hint defaults to `Shape::Shrink` (`resolve.rs:102`) — that is the case this change makes invisible. This is a **new** fixture: do not add groups to `mini` or `sixkind`, where cross-box `Layout` placements have previously collapsed into `LayoutConflict` and broken `scene.rs` tests.

- [ ] **Step 1: Write the four classifiers**

`crates/waml-editor/tests/fixtures/groups/customer.md`:

```markdown
---
type: uml.Class
title: Customer
---
# Customer

## Attributes
- id: CustomerId {1}
- name: String {1}
```

`crates/waml-editor/tests/fixtures/groups/account.md`:

```markdown
---
type: uml.Class
title: Account
---
# Account

## Attributes
- id: AccountId {1}
- balance: Decimal {1}
```

`crates/waml-editor/tests/fixtures/groups/order.md`:

```markdown
---
type: uml.Class
title: Order
---
# Order

## Attributes
- id: OrderId {1}
- total: Decimal {1}
```

`crates/waml-editor/tests/fixtures/groups/invoice.md`:

```markdown
---
type: uml.Class
title: Invoice
---
# Invoice

## Attributes
- id: InvoiceId {1}
- issued: Date {1}
```

- [ ] **Step 2: Write the index and the diagram**

`crates/waml-editor/tests/fixtures/groups/index.md`:

```markdown
# Groups
```

`crates/waml-editor/tests/fixtures/groups/groups-diagram.md`:

```markdown
---
type: Diagram
title: Groups
profile: uml-domain
---
# Groups

## Members

### Users
- [Customer](./customer.md)
- [Account](./account.md)

### Billing
- [Order](./order.md)
- [Invoice](./invoice.md)

## Layout
- Users as column with frame
- Billing as column
- Users left of Billing
```

- [ ] **Step 3: Verify the bundle loads and solves cleanly**

Run: `cargo run -p waml-editor --bin waml-editor -- crates/waml-editor/tests/fixtures/groups`
Expected: the editor opens on the Groups diagram with four cards. Watch the console for `diagnostic:` lines — any `LayoutConflict` means the placement lines need adjusting (drop the `- Users left of Billing` line and re-check; a cross-group placement is the known failure mode). Close by pid.

- [ ] **Step 4: Commit**

```bash
git add crates/waml-editor/tests/fixtures/groups
git commit -m "test(fixtures): add a groups fixture with a framed and a default group"
```

---

### Task 5: Wire `ShowHiddenBorders` from the view bar

**Files:**
- Modify: `crates/waml-editor/src/class_diagram_view.rs` — the `ViewBar` match block Plan B added after the tool-dock block

**Interfaces:**
- Consumes: `ViewBarAction::Toggled(ViewOption::ShowHiddenBorders, on)` (Plan B), `GraphCanvas::set_show_hidden_borders` (Task 2).
- Produces: nothing new. Plan D adds the four camera arms to the same `match`.

**Context:** Plan B left this action falling through to `other => log!("view bar: {other:?}")`. Adding an arm above that catch-all is the whole change; the surrounding block (the `body.view_bar(cx).borrow_mut(...)` read and the `return out;`) is untouched.

- [ ] **Step 1: Add the arm**

In `crates/waml-editor/src/class_diagram_view.rs`, inside the `match action { ... }` of the view-bar block, immediately after the `ShowConstraints` arm and **before** `other => log!("view bar: {other:?}")`, add:

```rust
                crate::view_bar::ViewBarAction::Toggled(
                    crate::view_bar::ViewOption::ShowHiddenBorders,
                    on,
                ) => {
                    if let Some(mut canvas) =
                        body.canvas(cx).borrow_mut::<crate::canvas::GraphCanvas>()
                    {
                        canvas.set_show_hidden_borders(cx, on);
                    }
                }
```

Also update the block's leading comment to drop the "Plan C wires the hidden borders" clause:

```rust
        // View bar: `ShowConstraints` drives the canvas veil mode and
        // `ShowHiddenBorders` the group x-ray. The camera one-shots are `log!`
        // no-ops here -- Plan D wires them.
```

- [ ] **Step 2: Run the tests**

Run: `cargo test -p waml-editor`
Expected: PASS, no `dead_code` errors.

- [ ] **Step 3: Run the full gate**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: all green. This gate covers Tasks 2, 3 and 5 together.

- [ ] **Step 4: Commit**

```bash
git add crates/waml-editor/src/class_diagram_view.rs
git commit -m "feat(view-bar): wire the hidden-borders x-ray to the canvas"
```

---

### Task 6: Interactive verification on the groups fixture

**Files:**
- Modify: none (verification only; a dashed-border retune lands back in Task 2's pen)

**Interfaces:**
- Consumes: everything above.
- Produces: a per-pid visual sign-off, plus a recorded decision on which dashed-border implementation shipped.

**Context:** The spec asks the shader approach to be tried first and the outcome recorded. If the diagonal parameterization reads badly at the corners (dashes bunching or a visible seam where the phase wraps), the documented fallback is stamping short axis-aligned quads along the four sides — the technique `segment_quad` already uses for edges. Launch the **worktree's own** `scripts/run-native.ps1`; capture by the specific pid in one PowerShell call.

- [ ] **Step 1: Launch on the groups fixture**

Run the worktree's `scripts/run-native.ps1` against `crates/waml-editor/tests/fixtures/groups`, capturing the launched pid.

- [ ] **Step 2: Confirm the default (x-ray off) render**

- The `Users` group (`with frame`) draws its tinted fill and its `Users` title.
- The `Billing` group (no shape hint → `shrink`) draws **nothing** — no fill, no outline, no title. Its four member cards are unchanged and still laid out as a column.
- The `square-dashed` button on the view bar is **unlit**.

- [ ] **Step 3: Confirm the x-ray**

Click the `square-dashed` button:
- it lights;
- `Billing` gains a dashed hairline outline with no fill, and its title draws in the dim ink;
- `Users` is **unchanged** — still filled, still solid, title still full-strength.

Click it again: `Billing`'s outline disappears and `Users` stays as it was.

- [ ] **Step 4: Confirm the dash under zoom and nesting**

- Zoom in and out (mouse wheel): the dash period grows and shrinks with the view and stays legible at both extremes.
- Inspect the four corners of the dashed rect: the dashes should run continuously around them with no bunching or phase seam.
- Check `mini` (`scripts/run-native.ps1` with no fixture argument): it has no groups, so nothing should change there.

- [ ] **Step 5: Record which implementation shipped, or fall back**

If the corners read badly, replace the pen's `pixel:` fn with per-side segment stamping (four axis-aligned quads per border, dash-masked along the single live axis) and re-run Steps 3-4. Either way, note in `canvas.rs`'s `mod.draw.GroupDashed` comment which approach shipped and why.

- [ ] **Step 6: Close by pid and commit any retune**

```bash
git add crates/waml-editor/src/canvas.rs
git commit -m "fix(canvas): retune the hidden-group dash after visual review"
```

---

## Done when

- `group_draw_mode` covers all six `(Shape, bool)` combinations and its test is green.
- `untitled_label` is green.
- A `frame` group renders exactly as every group did before this change; `box`/`shrink` groups render nothing by default.
- The view bar's `square-dashed` toggle brings `box`/`shrink` groups back as dashed hairlines with dim `Untitled N` (or real) titles, leaving `frame` groups untouched.
- `crates/waml-editor/tests/fixtures/groups` loads with no `LayoutConflict` diagnostics.
- The full gate (`cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`) is green.
- The shipped dashed-border approach (shader vs. segment stamping) is recorded in the pen's comment.
