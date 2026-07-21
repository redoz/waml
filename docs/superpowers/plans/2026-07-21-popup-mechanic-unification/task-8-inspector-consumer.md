### Task 8: inspector element-picker → first generic `PopupRoot` consumer

Rip the hand-drawn inline popup out of `inspector_panel.rs` and route the element-picker through `PopupRoot`/`MenuPopup`. This proves the mechanic generalizes to a caller that wasn't already a popup, and it stops the inspector frame growing to host its own list. The inspector emits an open-request action (it can't compute cross-tree placement itself — the aligned-parent offset); `App` relays it to `show_at`; the pick returns via the tag-filtered `closed` queue.

**Baseline assumption:** the current uncommitted inline-picker work in `inspector_panel.rs` is committed on its own first (the handoff's separate "element-picker dropdown+icon fix" commit). This task operates on that committed state and deliberately supersedes the inline drawing with the shared surface — `MenuPopup` draws per-row `IconSet` SDF (including `Icon::Spline`), which was the whole reason the inline popup existed, so that capability is preserved.

**Accepted minor regressions (plan 1; note in the commit):**
1. The inline popup accented the currently-**selected** row; `MenuPopup` has only hover/arm state, so the selected-row accent is dropped. The picker field label still shows the current selection, so this is cosmetic.
2. `MenuPopup` draws a leading icon on **every** row; the inline popup drew the `Spline` glyph only on edge rows. Non-edge rows now carry their kind glyph (or a neutral one). Acceptable.
Neither is worth extending `PopupItem`; revisit only if a design review rejects them.

**Files:**
- Modify: `crates/waml-editor/src/inspector_panel.rs` (rip inline popup; emit open-request; resolve pick)
- Modify: `crates/waml-editor/src/app.rs` (relay open-request → `show_at`; map `closed` → `set_subject`)

**Interfaces:**
- Consumes: `crate::popup::base::PopupItem`; `crate::popup::root::{PopupRoot, PopupSpec, MenuOpen}`; `crate::popup::base::PopupResult`.
- Produces:
  - `InspectorAction::OpenPicker { anchor: Rect, items: Vec<PopupItem> }` — a new variant replacing the inline open.
  - `pub fn open_picker_request(&self, actions: &Actions) -> Option<(Rect, Vec<PopupItem>)>` on `Inspector`.
  - `pub fn apply_pick(&mut self, cx: &mut Cx, model: &Model, id: LiveId) -> Option<Subject>` on `Inspector` — resolves a committed `PopupItem.id` back to its element and repoints; returns the new `Subject` (or `None` if the id wasn't a pickable element).
  - Tag: `live_id!(element_picker)`.

---

- [ ] **Step 1: Rip out the inline popup state, event handling, and draw**

In `inspector_panel.rs`:
- Delete the fields `picker_open` (`:243-244`), `picker_rects` (`:247-248`), `picker_hover` (`:249-250`). Keep `picker_field_rect` (still the click target), `show_picker`, `elements`.
- Delete the `#[live] draw_popup_bg` / `draw_row_hi` / `draw_row_text` / `draw_row_sel` fields and their DSL config (grep `draw_popup_bg`/`draw_row_hi`/`draw_row_text`/`draw_row_sel` — struct fields `:186-191`+, and the `script_mod!` `mod.draw`/holder lines near `:112-118`). These drew the inline sheet; `MenuPopup` owns row drawing now. Keep `draw_icon_edge` only if still used elsewhere (it tints the pin/caret via `draw_icon_edge.color` at `:478` — so KEEP `draw_icon_edge`).
- In `handle_event`, delete the `Hit::FingerHoverIn/Over ... if self.picker_open` arm (`:339-350`) and the `if self.picker_open { .. return; }` block inside `Hit::FingerUp` (`:353-366`). The remaining `Hit::FingerUp` "closed: the picker field opens the list" branch (`:367-371`) changes: instead of `self.open_picker(cx)`, emit the open-request (Step 3).
- In `draw_walk`, delete the `picker_open` frame-growth branch (`:430-433`) and the `if self.picker_open { self.draw_picker_list(..); return .. }` block (`:532-535`). Delete `self.picker_rects.clear();` (`:445`).
- Delete the methods `open_picker` (`:708-712`), `close_picker` (`:714-718`), `choose_element` (`:720-732`), `draw_picker_list` (`:734-784`), and the now-unused consts `POPUP_PAD`/`ROW_H_PICK`/`ROW_ICON` (`:285-287`) if nothing else references them (grep first).
- In `set_diagram_elements` (`:689-694`) and `set_picker_visible` (`:700-705`), delete the `self.picker_open = false;` lines (the field is gone). Keep the rest.
- Delete the `picked` reader (`:859-863`) — the pick now returns through `PopupRoot::closed` in `App`, resolved via `apply_pick` (Step 4). Keep `edited`.

- [ ] **Step 2: Build the picker items from `elements`**

Add a helper on `Inspector` that turns the diagram elements into `PopupItem`s (skipping the index-0 placeholder, matching the old list). It also records the id→element mapping for the reverse lookup on commit. Add a `#[rust] picker_ids: Vec<(LiveId, usize)>` field (id → index into `elements`).

```rust
    /// Build the picker rows as `PopupItem`s and record their id→index map.
    /// Node rows are enabled (a pick repoints the inspector); edge/diagram rows
    /// are disabled (they were no-ops in the inline list). Edge labels show only
    /// the target end (as before); edges lead with the `Spline` glyph.
    fn picker_items(&mut self) -> Vec<crate::popup::base::PopupItem> {
        use crate::popup::base::PopupItem;
        self.picker_ids.clear();
        let mut items = Vec::new();
        for idx in 1..self.elements.len() {
            let row = &self.elements[idx];
            let id = LiveId::from_str(&row.key); // runtime string hash; see note
            self.picker_ids.push((id, idx));
            let (label, icon) = match row.kind {
                ElementKind::Edge => (edge_target(&row.label).to_string(), Icon::Spline),
                ElementKind::Node => (row.label.clone(), Icon::PackageOpen),
                _ => (row.label.clone(), Icon::SquareMenu),
            };
            items.push(PopupItem {
                id,
                label,
                icon,
                danger: false,
                enabled: matches!(row.kind, ElementKind::Node),
            });
        }
        items
    }
```

Confirm `LiveId::from_str` is the fork's runtime string→`LiveId` hasher (grep the fork: `fn from_str` on `LiveId`, or `LiveId::from_str_with_lut`). If it takes a `&str` and returns a `LiveId` (not `Result`/`Option`), use as written; otherwise adapt. The map makes the id opaque + reversible-by-lookup, so the hash need not be reversible.

- [ ] **Step 3: Emit the open-request from the field click**

Add the action variant to `InspectorAction` (`:158-164`):

```rust
    OpenPicker { anchor: Rect, items: Vec<crate::popup::base::PopupItem> },
```

In `handle_event`'s `Hit::FingerUp` field-open branch (was `:367-371`), replace `self.open_picker(cx)` with the request. The anchor is the field rect translated back to screen space (undo the `hit_off` the panel applied — the field rect is stored in draw-time space, so add `hit_off` back to place the drop in real screen coords):

```rust
                if self.picker_field_rect.contains(p) {
                    let screen_field = Rect {
                        pos: self.picker_field_rect.pos + hit_off,
                        size: self.picker_field_rect.size,
                    };
                    let items = self.picker_items();
                    cx.widget_action(uid, InspectorAction::OpenPicker { anchor: screen_field, items });
                    return;
                }
```

Add the reader:

```rust
    /// The element-picker asked to open. `App` relays this to `PopupRoot` (only
    /// the composition root can place a cross-tree popup). Anchor is the field
    /// rect in screen coords; drop the card just below it.
    pub fn open_picker_request(&self, actions: &Actions) -> Option<(Rect, Vec<crate::popup::base::PopupItem>)> {
        let item = actions.find_widget_action(self.widget_uid())?;
        if let InspectorAction::OpenPicker { anchor, items } = item.cast() {
            Some((anchor, items))
        } else {
            None
        }
    }
```

- [ ] **Step 4: Resolve a committed pick back to a subject**

Replace the `picked` reader with `apply_pick`, using the recorded map. Node picks repoint the inspector via the existing `set_subject` path:

```rust
    /// Resolve a committed `PopupItem.id` (from `PopupRoot::closed`) back to its
    /// element and repoint the inspector. Returns the new subject, or `None` if
    /// the id wasn't a pickable (node) element in the current list.
    pub fn apply_pick(&mut self, cx: &mut Cx, model: &Model, id: LiveId) -> Option<Subject> {
        let idx = self.picker_ids.iter().find(|(i, _)| *i == id).map(|(_, x)| *x)?;
        let row = self.elements.get(idx)?;
        if !matches!(row.kind, ElementKind::Node) {
            return None;
        }
        let subject = Subject::Classifier(row.key.clone());
        self.set_subject(cx, model, subject.clone());
        Some(subject)
    }
```

(If `Subject` isn't `Clone`, return the `key` string instead and let `App` build the `Subject`. Match the existing `set_subject(cx, &self.model, Subject::Classifier(key))` call shape at `app.rs:1027`.)

- [ ] **Step 5: Relay open + resolve close in `App`**

In `app.rs`'s action handler, replace the old `picked` block (`:1013-1030`) with the relay + resolve. Place the relay near the other openers and the resolve near the other `closed` reads (Task 7 Step 7):

```rust
        // Element-picker: relay the inspector's open-request to PopupRoot.
        let picker_open = self
            .ui
            .widget(cx, ids!(inspector))
            .borrow::<crate::inspector_panel::Inspector>()
            .and_then(|ins| ins.open_picker_request(actions));
        if let Some((field, items)) = picker_open {
            let bounds = self.window_bounds(cx);
            let anchor = dvec2(field.pos.x, field.pos.y + field.size.y);
            if let Some(mut pr) = self.ui.widget(cx, ids!(popup_root)).borrow_mut::<PopupRoot>() {
                pr.show_at(cx, PopupSpec::Menu {
                    tag: live_id!(element_picker),
                    anchor,
                    bounds,
                    items,
                    open: MenuOpen::Popup,
                });
            }
            return;
        }

        // Element-picker commit: map the chosen id back to a subject.
        let picked_id = self
            .ui
            .widget(cx, ids!(popup_root))
            .borrow::<PopupRoot>()
            .and_then(|pr| match pr.closed(actions, live_id!(element_picker)) {
                Some(crate::popup::base::PopupResult::Invoked(id)) => Some(id),
                _ => None,
            });
        if let Some(id) = picked_id {
            if let Some(mut ins) = self
                .ui
                .widget(cx, ids!(inspector))
                .borrow_mut::<crate::inspector_panel::Inspector>()
            {
                ins.apply_pick(cx, &self.model, id);
            }
            return;
        }
```

(Watch the `self.model` borrow vs the `self.ui` widget borrow — take the `PopupRoot` read into a local and `drop` it before borrowing `inspector` mutably, same pattern as Task 7 Step 7. If the borrow checker fights `&self.model` while `self.ui` is borrowed, clone the needed key out first.)

- [ ] **Step 6: Build + test**

Run: `taskkill //IM waml-editor.exe //F ; cargo test -p waml-editor && cargo build -p waml-editor`
Expected: PASS + clean build. Grep for stragglers: `grep -rn "picker_open\|picker_rects\|picker_hover\|draw_picker_list\|choose_element\|InspectorAction::ElementPicked\|\.picked(" crates/waml-editor/src` — all should be gone (or the `ElementPicked` variant removed if nothing else uses it).

- [ ] **Step 7: End-to-end verification — drive the app**

Run the app and open a diagram tab (so the picker bar shows). Confirm:
1. Click the picker field → a `MenuPopup` card drops just below the field, listing the diagram's elements (edge rows lead with the `Spline` glyph, target-end label; node rows enabled, edge/diagram rows greyed/disabled).
2. Click a node row → the card closes and the inspector repoints to that node (same result as before).
3. Click a greyed edge/diagram row → no-op, card stays (disabled).
4. **Single-active across surfaces:** with the picker open, click the logo → the picker card dies, the logo card shows. Exactly one popup ever.
5. **Universal dismiss:** open the picker, then Esc / click empty space / Alt-Tab → each closes it. The inspector frame does NOT grow to host the list (the whole motivation).

STOP and debug (superpowers:systematic-debugging) if any of 1–5 fails. Do not claim completion without observing all five.

- [ ] **Step 8: Commit**

```bash
git add crates/waml-editor/src/inspector_panel.rs crates/waml-editor/src/app.rs
git commit -m "feat(inspector): route element-picker through PopupRoot (first consumer)

Rip the hand-drawn inline picker popup out of the inspector; emit an
OpenPicker request that App relays to popup_root.show_at(MenuPopup), and
resolve the tag-filtered commit back to a subject via apply_pick. The
inspector frame no longer grows to host its own list; the picker now gets
single-active + universal light-dismiss for free. Minor: selected-row accent
and edge-only icons dropped (MenuPopup draws per-row IconSet incl. Spline).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```
