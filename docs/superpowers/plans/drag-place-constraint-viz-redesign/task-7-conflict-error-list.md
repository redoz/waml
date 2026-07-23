### Task 7: Off-canvas conflict error list

**Files:**
- Modify: `crates/waml-editor/src/scene.rs` (pure `dir_keyword`, `conflict_statement`, `conflict_participants` + tests)
- Modify: `crates/waml-editor/src/canvas.rs` (`conflict_count`, `conflicts`, `set_conflict_focus`; fade-the-rest draw)
- Create: `crates/waml-editor/src/conflict_badge.rs` (red `! N` toolbar counter widget)
- Modify: `crates/waml-editor/src/main.rs` (add `mod conflict_badge;`)
- Modify: `crates/waml-editor/src/app.rs` (register in order; mount; sync count; click → popup; row → focus-fade)

**Interfaces:**
- Consumes: `crate::scene::{SceneConflict, SceneRelation}` (Task 2); `GraphCanvas` veil/fade path (Tasks 4/6); `PopupRoot`/`PopupSpec::Menu` (existing).
- Produces:
  - `crate::scene::conflict_statement(&SceneConflict) -> String`, `crate::scene::conflict_participants(&SceneConflict) -> Vec<String>`, `crate::scene::dir_keyword(Direction) -> &'static str`.
  - `GraphCanvas::conflict_count(&self) -> usize`, `GraphCanvas::conflicts(&self) -> Vec<SceneConflict>`, `GraphCanvas::set_conflict_focus(&mut self, cx, Option<usize>)`.
  - `crate::conflict_badge::ConflictBadge` + `ConflictBadge::set_count(&mut self, cx, usize)`, `ConflictBadge::clicked(&self, &Actions) -> bool`.

**⚠️ Registration-order gotcha (Global Constraints).** `ConflictBadge` is mounted in the app DSL, so `crate::conflict_badge::script_mod(vm)` must run BEFORE `self::script_mod(vm)`. Its only child is a prelude `Label` (already registered), so no extra child-dep ordering is needed. Step 9 verifies the line position.

---

- [ ] **Step 1: Write the failing scene tests**

Add to `scene.rs`'s `#[cfg(test)] mod tests`:

```rust
#[test]
fn conflict_statement_reads_as_dsl() {
    use waml::syntax::Direction;
    let c = SceneConflict {
        dropped: SceneRelation { subject: "order".into(), reference: "customer".into(), dir: Direction::LeftOf },
        conflicts_with: vec![
            SceneRelation { subject: "customer".into(), reference: "order".into(), dir: Direction::LeftOf },
        ],
    };
    let s = conflict_statement(&c);
    assert!(s.contains("order left of customer"), "dropped statement missing: {s}");
    assert!(s.contains("customer left of order"), "conflict statement missing: {s}");
    assert!(s.to_lowercase().contains("contradict"), "missing the 'contradict' note: {s}");
}

#[test]
fn conflict_participants_lists_every_involved_node() {
    use waml::syntax::Direction;
    let c = SceneConflict {
        dropped: SceneRelation { subject: "order".into(), reference: "customer".into(), dir: Direction::LeftOf },
        conflicts_with: vec![
            SceneRelation { subject: "customer".into(), reference: "order".into(), dir: Direction::LeftOf },
        ],
    };
    let mut p = conflict_participants(&c);
    p.sort();
    p.dedup();
    assert_eq!(p, vec!["customer".to_string(), "order".to_string()]);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml-editor --lib scene::tests::conflict`
Expected: FAIL to compile — the three functions are undefined.

- [ ] **Step 3: Add the pure conflict-text helpers in `scene.rs`**

Add near `SceneConflict`:

```rust
/// DSL keyword for a placement direction (matches the `## Layout` surface form).
pub fn dir_keyword(d: waml::syntax::Direction) -> &'static str {
    use waml::syntax::Direction::*;
    match d {
        LeftOf => "left of",
        RightOf => "right of",
        Above => "above",
        Below => "below",
        AboveLeft => "above left of",
        AboveRight => "above right of",
        BelowLeft => "below left of",
        BelowRight => "below right of",
    }
}

/// Render one relation as its `A <dir> B` DSL form.
fn relation_statement(r: &SceneRelation) -> String {
    format!("{} {} {}", r.subject, dir_keyword(r.dir), r.reference)
}

/// Human-readable error-list text for a dropped constraint: the dropped
/// statement, the statements it contradicts, and a one-line "these contradict"
/// note (spec §4).
pub fn conflict_statement(c: &SceneConflict) -> String {
    let mut lines = vec![relation_statement(&c.dropped)];
    for w in &c.conflicts_with {
        lines.push(relation_statement(w));
    }
    format!("{}  —  these contradict", lines.join("; "))
}

/// Every node key involved in a conflict (dropped + all contradicting relations),
/// for the fade-the-rest focus (spec §4). Not deduped — callers dedup as needed.
pub fn conflict_participants(c: &SceneConflict) -> Vec<String> {
    let mut out = vec![c.dropped.subject.clone(), c.dropped.reference.clone()];
    for w in &c.conflicts_with {
        out.push(w.subject.clone());
        out.push(w.reference.clone());
    }
    out
}
```

- [ ] **Step 4: Add the canvas conflict readers + focus-fade**

Add a field to `GraphCanvas` (near `conflict_zones`):

```rust
    /// Index into `scene.conflicts` whose participants should stay lit while every
    /// other card fades (spec §4 "fade the rest"). `None` = no focus. Reset on
    /// scene replace.
    #[rust]
    conflict_focus: Option<usize>,
```

Reset it in `set_scene`/`set_focus` (add `self.conflict_focus = None;` beside the other resets).

Add readers + setter to `impl GraphCanvas`:

```rust
    /// Number of unsatisfiable constraints in the current scene (toolbar counter).
    pub fn conflict_count(&self) -> usize {
        self.scene.conflicts.len()
    }

    /// Clone of the current scene's conflicts, for the toolbar popup list.
    pub fn conflicts(&self) -> Vec<crate::scene::SceneConflict> {
        self.scene.conflicts.clone()
    }

    /// Focus a conflict (or clear): every card except the conflict's participants
    /// fades. Clamped to the conflict count. Repaints.
    pub fn set_conflict_focus(&mut self, cx: &mut Cx, idx: Option<usize>) {
        self.conflict_focus = idx.filter(|&i| i < self.scene.conflicts.len());
        self.draw_bg.redraw(cx);
    }
```

In `draw_walk`, AFTER `self.draw_relations_overlay(cx);` (and before the drag overlay), add the fade-the-rest pass:

```rust
        // Conflict focus (spec §4): fade every card except the focused conflict's
        // participants, so the contradiction is locatable off the error list.
        if let Some(i) = self.conflict_focus {
            if let Some(conflict) = self.scene.conflicts.get(i).cloned() {
                let keep: std::collections::HashSet<String> =
                    crate::scene::conflict_participants(&conflict).into_iter().collect();
                let dims: Vec<(usize, bool)> = self
                    .scene
                    .nodes
                    .iter()
                    .enumerate()
                    .map(|(idx, n)| (idx, keep.contains(&n.key)))
                    .collect();
                for (idx, kept) in dims {
                    if !kept {
                        let s = self.node_screen_rect(idx);
                        self.fill_rect(cx, s.pos.x, s.pos.y, s.size.x, s.size.y, vec4(0.62, 0.65, 0.70, 0.55));
                    }
                }
            }
        }
```

- [ ] **Step 5: Create the `ConflictBadge` widget**

Create `crates/waml-editor/src/conflict_badge.rs`:

```rust
//! Toolbar conflict counter (spec §4): a red `! N` pill, shown only when the
//! solver dropped constraints. Clicking it opens the error-list popup (wired in
//! `app.rs`). A `#[deref] View` with a red `draw_bg` and a `Label`; a
//! `FingerDown` on its area emits `Clicked`.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas

    mod.widgets.ConflictBadgeBase = #(ConflictBadge::register_widget(vm))

    mod.widgets.ConflictBadge = set_type_default() do mod.widgets.ConflictBadgeBase{
        width: Fit
        height: 28.0
        flow: Right
        align: Align{x: 0.5, y: 0.5}
        padding: Inset{left: 10.0, right: 10.0}
        show_bg: true
        draw_bg +: {
            color: vec4(0.80, 0.22, 0.22, 0.95)
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(0.0, 0.0, self.rect_size.x, self.rect_size.y, 6.0)
                sdf.fill(self.color)
                return sdf.result
            }
        }
        label := Label{
            text: ""
            draw_text +: {
                color: #FFF
                text_style: theme.font_bold{font_size: 12}
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum ConflictBadgeAction {
    #[default]
    None,
    Clicked,
}

#[derive(Script, ScriptHook, Widget)]
pub struct ConflictBadge {
    #[deref]
    view: View,
}

impl Widget for ConflictBadge {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        let uid = self.widget_uid();
        match event.hits(cx, self.view.area()) {
            Hit::FingerDown(_) => cx.widget_action(uid, ConflictBadgeAction::Clicked),
            Hit::FingerHoverIn(_) => cx.set_cursor(MouseCursor::Hand),
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl ConflictBadge {
    /// Set the counter text + show/hide by count (`0` hides the pill). Uses the
    /// `self.view.widget(...).set_text(...)` accessor (same shape ToolDock uses for
    /// child lookup) rather than a `.label()` convenience that `View` may not expose.
    pub fn set_count(&mut self, cx: &mut Cx, n: usize) {
        self.view.widget(cx, ids!(label)).set_text(cx, &format!("! {n}"));
        self.view.set_visible(cx, n > 0);
        self.view.redraw(cx);
    }

    /// Reader for `App`: whether the badge was clicked this frame.
    pub fn clicked(&self, actions: &Actions) -> bool {
        actions
            .find_widget_action(self.widget_uid())
            .map(|a| matches!(a.cast(), ConflictBadgeAction::Clicked))
            .unwrap_or(false)
    }
}
```

(If `theme.font_bold` is not the exact alias in this codebase, use the same `text_style` shape the `statusbar.rs` labels use — check `statusbar.rs` for the live font alias and mirror it.)

- [ ] **Step 6: Register the module**

In `main.rs`, add (alpha, after `mod config;`/`mod constraint_toggle;`):

```rust
mod conflict_badge;
```

- [ ] **Step 7: Mount the badge in the app DSL**

Add `use mod.widgets.ConflictBadge` to the `use mod.widgets.*` list in `app.rs`'s `script_mod!` block. Mount it top-right, left of the inspector, inside the same overlay body:

```rust
                        // Conflict counter: top area, right side (spec §4).
                        conflict_badge_wrap := View{
                            width: Fill
                            height: Fill
                            align: Align{x: 1.0, y: 0.0}
                            conflict_badge := ConflictBadge{
                                margin: Inset{right: 344.0, top: 14.0}
                                visible: false
                            }
                        }
```

- [ ] **Step 8: Register `script_mod` in order (CHECKLIST)**

In `App::script_mod`, add after `crate::constraint_toggle::script_mod(vm);`:

```rust
        crate::conflict_badge::script_mod(vm);
```

Verify it sits ABOVE `self::script_mod(vm)` (~:1614). (Its `Label` child is a prelude widget already registered, so no child-dep ordering applies.)

- [ ] **Step 9: Sync the count after every solve**

Add a helper to `impl App`:

```rust
    /// Push the canvas's current conflict count onto the toolbar badge.
    fn sync_conflict_badge(&mut self, cx: &mut Cx) {
        let n = self
            .ui
            .widget(cx, ids!(canvas))
            .borrow::<crate::canvas::GraphCanvas>()
            .map(|c| c.conflict_count())
            .unwrap_or(0);
        if let Some(mut badge) = self
            .ui
            .widget(cx, ids!(conflict_badge))
            .borrow_mut::<crate::conflict_badge::ConflictBadge>()
        {
            badge.set_count(cx, n);
        }
    }
```

Call `self.sync_conflict_badge(cx);` at the end of `sync_active_tab` (after `self.sync_statusbar(cx);`) and after the re-solve tail in `handle_actions` (right after the `v.resolve_active(cx, &body, &self.model);` block that follows `waml::ops::apply`).

- [ ] **Step 10: Wire the badge click → popup, and rows → focus-fade**

Add an `App` field (near `nav_scope_ids`):

```rust
    /// Maps each conflict-list popup item id back to its `scene.conflicts` index,
    /// so the committed `LiveId` resolves to a conflict to focus-fade.
    #[rust]
    conflict_row_ids: Vec<(LiveId, usize)>,
```

In `handle_actions`, add a badge-click block (e.g. after the constraint-toggle block from Task 5):

```rust
        // Conflict badge -> error-list popup (spec §4).
        let badge_clicked = self
            .ui
            .widget(cx, ids!(conflict_badge))
            .borrow::<crate::conflict_badge::ConflictBadge>()
            .map(|b| b.clicked(actions))
            .unwrap_or(false);
        if badge_clicked {
            let conflicts = self
                .ui
                .widget(cx, ids!(canvas))
                .borrow::<crate::canvas::GraphCanvas>()
                .map(|c| c.conflicts())
                .unwrap_or_default();
            self.conflict_row_ids.clear();
            let items: Vec<crate::popup::base::PopupItem> = conflicts
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let id = LiveId::from_str(&format!("conflict:{i}"));
                    self.conflict_row_ids.push((id, i));
                    crate::popup::base::PopupItem {
                        id,
                        label: crate::scene::conflict_statement(c),
                        icon: crate::icons::Icon::CircleX,
                        danger: true,
                        enabled: true,
                    }
                })
                .collect();
            if !items.is_empty() {
                let btn = self.ui.widget(cx, ids!(conflict_badge)).area().rect(cx);
                let anchor = dvec2(btn.pos.x, btn.pos.y + btn.size.y + crate::popup::menu::MENU_GAP);
                let bounds = self.window_bounds(cx);
                if let Some(mut pr) = self.ui.widget(cx, ids!(popup_root)).borrow_mut::<PopupRoot>() {
                    pr.show_at(cx, PopupSpec::Menu {
                        tag: live_id!(conflict_list),
                        anchor,
                        bounds,
                        items,
                        open: MenuOpen::Popup,
                    });
                }
            }
            return;
        }
```

In the existing popup-outcomes block (where `logo_closed`/`burger_closed`/... are read), add:

```rust
            let conflict_closed = pr.closed(actions, live_id!(conflict_list));
```

(before `drop(pr);`), and after `drop(pr);` add the handler:

```rust
            if let Some(PopupResult::Invoked(id)) = conflict_closed {
                if let Some((_, idx)) = self.conflict_row_ids.iter().find(|(i, _)| *i == id) {
                    let idx = *idx;
                    if let Some(mut canvas) = self
                        .ui
                        .widget(cx, ids!(canvas))
                        .borrow_mut::<crate::canvas::GraphCanvas>()
                    {
                        canvas.set_conflict_focus(cx, Some(idx));
                    }
                }
            }
```

(`PopupResult`, `PopupRoot`, `PopupSpec`, `MenuOpen`, `MENU_GAP` are already imported/used in `app.rs`.)

- [ ] **Step 11: Run the tests**

Run: `cargo test -p waml-editor --lib`
Expected: PASS (`conflict_statement_reads_as_dsl`, `conflict_participants_lists_every_involved_node`).

- [ ] **Step 12: Run the full gate**

Run: `cargo test --workspace`
Expected: PASS.
Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean (no unused imports; if `area().rect(cx)` needs the badge's `#[deref] View` area, confirm `ConflictBadge` exposes `.area()` via `#[deref]` — it does through the `View`).

- [ ] **Step 13: Commit**

```bash
git add crates/waml-editor/src/scene.rs crates/waml-editor/src/canvas.rs crates/waml-editor/src/conflict_badge.rs crates/waml-editor/src/main.rs crates/waml-editor/src/app.rs
git commit -m "feat(editor): off-canvas conflict error list with fade-to-focus"
```
