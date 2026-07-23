### Task 5: Visibility toggle (None / Selected / All)

**Files:**
- Modify: `crates/waml-editor/src/canvas.rs` (add `ConstraintVisibility` enum + field + setter; pure `relations_for_visibility`; gate `draw_relations_overlay`)
- Create: `crates/waml-editor/src/constraint_toggle.rs` (new `ConstraintToggle` segmented widget, modeled on `ToolDock`)
- Modify: `crates/waml-editor/src/main.rs` (add `mod constraint_toggle;`)
- Modify: `crates/waml-editor/src/app.rs` (register `constraint_toggle::script_mod` in the correct order; mount the widget; wire its action to the canvas)

**Interfaces:**
- Consumes (from Task 2/4): `crate::scene::SceneRelation`; `GraphCanvas::draw_veil_for`; `waml::syntax::Direction`.
- Produces:
  - `crate::canvas::ConstraintVisibility { None, Selected, All }` (`Default = Selected`), plus `GraphCanvas::set_constraint_vis(&mut self, cx, ConstraintVisibility)`.
  - `crate::constraint_toggle::ConstraintToggle` widget + `ConstraintToggle::toggle_action(&self, &Actions) -> Option<ConstraintVisibility>`.

**⚠️ CRITICAL registration-order gotcha (from Global Constraints).** `ConstraintToggle` mounts `IconButton` children AND is itself mounted in `app.rs`'s DSL. It is a DEAD + INVISIBLE, unqueryable node (glyphs blank, `clicked` no-op) unless BOTH hold:
1. `crate::icon_button::script_mod(vm)` runs BEFORE `crate::constraint_toggle::script_mod(vm)` (its child dep — already true; icon_button is registered ~app.rs:1593).
2. `crate::constraint_toggle::script_mod(vm)` runs BEFORE `self::script_mod(vm)` (app's own module, which mounts `ConstraintToggle`).
Green tests will NOT catch a violation — Step 8 is a manual checklist verification of the registration line's position.

---

- [ ] **Step 1: Write the failing `relations_for_visibility` test**

Add to `canvas.rs`'s `#[cfg(test)] mod tests`:

```rust
#[test]
fn visibility_gates_which_relations_draw() {
    use crate::scene::SceneRelation;
    use waml::syntax::Direction;
    let rels = vec![
        SceneRelation { subject: "order".into(), reference: "customer".into(), dir: Direction::LeftOf },
        SceneRelation { subject: "payment-gateway".into(), reference: "order".into(), dir: Direction::Below },
        SceneRelation { subject: "a".into(), reference: "b".into(), dir: Direction::LeftOf },
    ];
    // None: nothing, regardless of selection.
    assert!(relations_for_visibility(&rels, ConstraintVisibility::None, Some("order")).is_empty());
    // Selected with nothing selected: nothing.
    assert!(relations_for_visibility(&rels, ConstraintVisibility::Selected, None).is_empty());
    // Selected on `order`: the two relations touching it (as subject OR reference),
    // not the unrelated a-b relation.
    let sel = relations_for_visibility(&rels, ConstraintVisibility::Selected, Some("order"));
    assert_eq!(sel.len(), 2);
    assert!(sel.iter().all(|r| r.subject == "order" || r.reference == "order"));
    // All: every relation.
    assert_eq!(relations_for_visibility(&rels, ConstraintVisibility::All, None).len(), 3);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml-editor --lib canvas::tests::visibility_gates`
Expected: FAIL to compile — `ConstraintVisibility` and `relations_for_visibility` undefined.

- [ ] **Step 3: Add `ConstraintVisibility` + the pure selector**

Add to `canvas.rs` (near the top-level pure helpers / enums, e.g. after `Zone`):

```rust
/// What constraint veils the canvas draws (spec §1). Persisted in view state and
/// driven by the toolbar segmented control.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ConstraintVisibility {
    /// No constraint marks — pure diagram.
    None,
    /// Selecting a node lights every constraint touching it (sticky). Default.
    #[default]
    Selected,
    /// Every constraint, viewed through the layer scrubber (Task 6).
    All,
}

impl ConstraintVisibility {
    pub const ALL: [ConstraintVisibility; 3] =
        [ConstraintVisibility::None, ConstraintVisibility::Selected, ConstraintVisibility::All];
}

/// The relations that should be drawn under a visibility mode + sticky selection
/// (spec §1). `None` ⇒ empty; `Selected` ⇒ relations touching `selected_key` as
/// subject OR reference (empty if nothing selected); `All` ⇒ every relation. Pure,
/// GPU-free (mirrors `node_at` selection logic).
fn relations_for_visibility<'a>(
    relations: &'a [crate::scene::SceneRelation],
    mode: ConstraintVisibility,
    selected_key: Option<&str>,
) -> Vec<&'a crate::scene::SceneRelation> {
    match mode {
        ConstraintVisibility::None => Vec::new(),
        ConstraintVisibility::All => relations.iter().collect(),
        ConstraintVisibility::Selected => {
            let Some(key) = selected_key else { return Vec::new() };
            relations
                .iter()
                .filter(|r| r.subject == key || r.reference == key)
                .collect()
        }
    }
}
```

- [ ] **Step 4: Add the canvas field, setter, and gate the overlay**

Add the field to `GraphCanvas` (next to `selected_key`):

```rust
    /// Which constraint veils to draw (spec §1). Default `Selected`.
    #[rust]
    constraint_vis: ConstraintVisibility,
```

Add the setter to `impl GraphCanvas` (near `set_conflict_zones`):

```rust
    /// Set the constraint-veil visibility mode and repaint.
    pub fn set_constraint_vis(&mut self, cx: &mut Cx, mode: ConstraintVisibility) {
        self.constraint_vis = mode;
        self.draw_bg.redraw(cx);
    }
```

Replace the body of `draw_relations_overlay` (from Task 4) with the gated version:

```rust
    /// Persistent constraint overlay, gated by the visibility mode + sticky
    /// selection (spec §1): None draws nothing, Selected draws only relations
    /// touching the selected node, All draws every relation.
    fn draw_relations_overlay(&mut self, cx: &mut Cx2d) {
        let selected_key = self.selected_key.clone();
        let chosen: Vec<(usize, usize, waml::syntax::Direction)> =
            relations_for_visibility(&self.scene.relations, self.constraint_vis, selected_key.as_deref())
                .into_iter()
                .filter_map(|rel| {
                    let si = self.scene.nodes.iter().position(|n| n.key == rel.subject)?;
                    let ri = self.scene.nodes.iter().position(|n| n.key == rel.reference)?;
                    Some((si, ri, rel.dir))
                })
                .collect();
        for (si, ri, dir) in chosen {
            self.draw_veil_for(cx, si, ri, dir);
        }
    }
```

- [ ] **Step 5: Create the `ConstraintToggle` widget**

Create `crates/waml-editor/src/constraint_toggle.rs` (a stripped `ToolDock`: a `#[deref] View` of three `IconButton` children, one per mode; the active one is lit; a click emits the picked mode):

```rust
//! Toolbar segmented control for constraint-veil visibility (spec §1): a
//! three-cell None / Selected / All picker. Modeled on `ToolDock` — a
//! `#[deref] View` laying out three shared `IconButton` children; `draw_walk`
//! syncs each child's glyph + lit state from `active`, `handle_event` reads each
//! child's `clicked` and emits the picked `ConstraintVisibility`.

use makepad_widgets::*;

use crate::canvas::ConstraintVisibility;
use crate::icon_button::IconButtonWidgetRefExt;
use crate::icons::Icon;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*

    mod.widgets.ConstraintToggleBase = #(ConstraintToggle::register_widget(vm))

    mod.widgets.ConstraintToggle = set_type_default() do mod.widgets.ConstraintToggleBase{
        width: 110.0
        height: 36.0
        flow: Right
        align: Align{x: 0.5, y: 0.5}
        padding: Inset{left: 4.0, right: 4.0, top: 2.0, bottom: 2.0}
        spacing: 2.0
        show_bg: true
        // Same Atlas HUD frame as ToolDock.
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

        none_btn := IconButton {}
        selected_btn := IconButton {}
        all_btn := IconButton {}
    }
}

#[derive(Clone, Debug, Default)]
pub enum ConstraintToggleAction {
    #[default]
    None,
    /// A mode cell was clicked; carries the picked visibility.
    Picked(ConstraintVisibility),
}

#[derive(Script, ScriptHook, Widget)]
pub struct ConstraintToggle {
    #[deref]
    view: View,
    #[rust]
    active: ConstraintVisibility,
}

impl Widget for ConstraintToggle {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        let uid = self.widget_uid();
        if let Event::Actions(actions) = event {
            for mode in ConstraintVisibility::ALL {
                if self.button(cx, mode).as_icon_button().clicked(actions) {
                    self.active = mode;
                    self.view.redraw(cx);
                    cx.widget_action(uid, ConstraintToggleAction::Picked(mode));
                    break;
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        for mode in ConstraintVisibility::ALL {
            let btn = self.button(cx, mode).as_icon_button();
            btn.set_icon(cx, Self::icon_for(mode));
            btn.set_active(cx, mode == self.active);
        }
        while self.view.draw_walk(cx, scope, walk).step().is_some() {}
        DrawStep::done()
    }
}

impl ConstraintToggle {
    fn button(&mut self, cx: &mut Cx, mode: ConstraintVisibility) -> WidgetRef {
        match mode {
            ConstraintVisibility::None => self.view.widget(cx, ids!(none_btn)),
            ConstraintVisibility::Selected => self.view.widget(cx, ids!(selected_btn)),
            ConstraintVisibility::All => self.view.widget(cx, ids!(all_btn)),
        }
    }

    /// Catalog glyph per mode. None = eye-off, Selected = eye, All = bounding box.
    fn icon_for(mode: ConstraintVisibility) -> Icon {
        match mode {
            ConstraintVisibility::None => Icon::EyeOff,
            ConstraintVisibility::Selected => Icon::Eye,
            ConstraintVisibility::All => Icon::VectorSquare,
        }
    }

    /// Set the active mode directly (App-driven), bypassing the click round-trip.
    pub fn set_active(&mut self, cx: &mut Cx, mode: ConstraintVisibility) {
        self.active = mode;
        self.view.redraw(cx);
    }

    /// Reader for `App`: the picked visibility this frame, if any.
    pub fn toggle_action(&self, actions: &Actions) -> Option<ConstraintVisibility> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.cast() {
            ConstraintToggleAction::Picked(mode) => Some(mode),
            ConstraintToggleAction::None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_active_is_selected() {
        assert_eq!(ConstraintVisibility::default(), ConstraintVisibility::Selected);
    }

    #[test]
    fn each_mode_maps_to_a_catalog_icon() {
        assert_eq!(ConstraintToggle::icon_for(ConstraintVisibility::None), Icon::EyeOff);
        assert_eq!(ConstraintToggle::icon_for(ConstraintVisibility::Selected), Icon::Eye);
        assert_eq!(ConstraintToggle::icon_for(ConstraintVisibility::All), Icon::VectorSquare);
    }
}
```

- [ ] **Step 6: Register the module**

In `crates/waml-editor/src/main.rs`, add (alpha position, after `mod config;`):

```rust
mod constraint_toggle;
```

- [ ] **Step 7: Mount the widget in the app DSL**

In `app.rs`'s `startup()` DSL, add `use mod.widgets.ConstraintToggle` to the `use mod.widgets.*` list at the top of the `script_mod!` block (near `use mod.widgets.ToolDock`).

Add a wrapper inside the same `flow: Overlay` body that holds `tool_dock_wrap` (right after the `tool_dock_wrap := View{ ... }` block):

```rust
                        // Constraint-veil visibility toggle: top-left, right of
                        // the tree, aligned with the tool dock's left edge.
                        constraint_toggle_wrap := View{
                            width: Fill
                            height: Fill
                            align: Align{x: 0.0, y: 0.0}
                            constraint_toggle := ConstraintToggle{
                                width: 110.0
                                height: 36.0
                                margin: Inset{left: 304.0, top: 12.0}
                            }
                        }
```

- [ ] **Step 8: Register `script_mod` in the correct order (CHECKLIST — the dead-node gotcha)**

In `App::script_mod` (`app.rs` ~:1603), add the registration line immediately AFTER `crate::tool_dock::script_mod(vm);`:

```rust
        crate::tool_dock::script_mod(vm);
        crate::constraint_toggle::script_mod(vm);
```

Manually VERIFY (this is the gotcha green tests miss):
1. `crate::icon_button::script_mod(vm);` (~:1593) appears ABOVE the new line — its `IconButton` children resolve.
2. The new line appears ABOVE `self::script_mod(vm)` (~:1614) — app's DSL mounts `ConstraintToggle`.
If either is false the widget silently mounts dead. This can only be truly confirmed at interactive sign-off (deferred), so double-check the source order now.

- [ ] **Step 9: Wire the toggle action to the canvas**

In `app.rs` `handle_actions`, add a reader block (e.g. near the diagram-switcher block):

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

- [ ] **Step 10: Run the tests**

Run: `cargo test -p waml-editor --lib`
Expected: PASS (`visibility_gates_which_relations_draw`, `default_active_is_selected`, `each_mode_maps_to_a_catalog_icon`).

- [ ] **Step 11: Run the full gate**

Run: `cargo test --workspace`
Expected: PASS.
Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 12: Commit**

```bash
git add crates/waml-editor/src/canvas.rs crates/waml-editor/src/constraint_toggle.rs crates/waml-editor/src/main.rs crates/waml-editor/src/app.rs
git commit -m "feat(canvas): None/Selected/All constraint visibility toggle"
```
