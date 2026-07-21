### Task 7: App collapse — wire `popup_root`, delete `radial.rs` + `app_menu.rs`, burger glow caller-local

Replace the two hand-wired popup surfaces in `App` with one `popup_root`. The two `is_open()`-gated `handle()` blocks + two outcome matches + `MenuOwner` + `set_app_menu_owner` + the manual reset all collapse to `popup_root.route` + tag-filtered `closed` reads. The three openers call `show_at`. `radial.rs` and `app_menu.rs` are deleted. This is the integration task — the mechanic is driven in-app at the end.

**Files:**
- Modify: `crates/waml-editor/src/app.rs` (DSL, struct, openers, route, drag-query, item builders, script_mod registration)
- Modify: `crates/waml-editor/src/main.rs` (remove `mod radial;` + `mod app_menu;`)
- Delete: `crates/waml-editor/src/radial.rs`
- Delete: `crates/waml-editor/src/app_menu.rs`

**Interfaces:**
- Consumes: `crate::popup::root::{PopupRoot, PopupSpec, MenuOpen, RadialOpen, PopupRootAction}`; `crate::popup::base::{PopupItem, PopupResult}`.
- Produces: no new public surface — this rewires existing `App` internals.

**Tags (opaque caller tokens — one per opener):** `live_id!(logo)`, `live_id!(burger)`, `live_id!(node_menu)`. These identify a popup's opener across the queue; they are NOT the committed item ids.

---

- [ ] **Step 1: Point the item builders at `PopupItem`**

In `app.rs`, the three builders `node_radial_items` (`:725-758`), `logo_menu_items` (`:763-789`), `burger_menu_items` (`:794-805`) each `use crate::radial::RadialItem;` and return `Vec<crate::radial::RadialItem>`. Change all three: `use crate::popup::base::PopupItem;`, return `Vec<PopupItem>`, and build `PopupItem { .. }` (fields are identical, so only the type name changes). Leave the `live_id!(..)` ids, labels, icons, `danger`, `enabled` values exactly as-is.

- [ ] **Step 2: Swap the DSL — two surfaces become one `popup_root`**

In the `script_mod!` App DSL:
- Replace the imports `use mod.widgets.Radial` + `use mod.widgets.AppMenu` (`app.rs:26-27`) with `use mod.widgets.PopupRoot`.
- Replace the two overlay children (`app.rs:253-267`):
  ```
  radial := Radial{ width: Fill height: Fill }
  app_menu := AppMenu{ width: Fill height: Fill }
  ```
  with one:
  ```
  // Single-active popup authority: last overlay child so it paints above the
  // canvas + every panel. Hosts the wedge + linear-card surfaces; each paints
  // nothing while closed. Replaces the old `radial` + `app_menu` children.
  popup_root := PopupRoot{ width: Fill height: Fill }
  ```

- [ ] **Step 3: Swap the `script_mod` registrations**

Replace `crate::radial::script_mod(vm);` + `crate::app_menu::script_mod(vm);` (`app.rs:1226-1227`) with the popup module registrations, children before the parent (the DSL parent references the child widget types):

```rust
        crate::popup::menu::script_mod(vm);
        crate::popup::radial::script_mod(vm);
        crate::popup::root::script_mod(vm);
```

- [ ] **Step 4: Delete `MenuOwner` + `set_app_menu_owner` + the struct field**

- Delete the `app_menu_owner: MenuOwner` field from `struct App` (`app.rs:291-296`, including its doc comment).
- Delete the `MenuOwner` enum (`app.rs:299-306`).
- Delete the `set_app_menu_owner` method (`app.rs:310-321`, including its doc comment).

- [ ] **Step 5: Rewrite the three openers to `show_at`**

Each opener needs the window client rect as `bounds`. Factor the existing radial computation (`app.rs:1059-1063`) into a helper on `App`:

```rust
    /// The main window's client rect in main-window coords (popup clip bounds).
    fn window_bounds(&mut self, cx: &mut Cx) -> Rect {
        let sz = self.ui.window(cx, ids!(main_window)).get_inner_size(cx);
        Rect { pos: dvec2(0.0, 0.0), size: dvec2(sz.x, sz.y) }
    }
```

**Burger** (`app.rs:906-937`): replace the `menu.open(cx, anchor, press, ..)` + `set_app_menu_owner(Burger)` block with:

```rust
            let btn = self.ui.widget(cx, ids!(menu_btn)).as_caption_button().rect();
            let anchor = dvec2(
                btn.pos.x + crate::popup::menu::MENU_INDENT_X,
                btn.pos.y + btn.size.y + crate::popup::menu::MENU_GAP,
            );
            let bounds = self.window_bounds(cx);
            if let Some(mut pr) = self.ui.widget(cx, ids!(popup_root)).borrow_mut::<PopupRoot>() {
                pr.show_at(cx, PopupSpec::Menu {
                    tag: live_id!(burger),
                    anchor,
                    bounds,
                    items: burger_menu_items(),
                    open: MenuOpen::Press(press),
                });
            }
            // Caller-local glow: light the burger now; it drops when we see this
            // tag's Closed (dismiss OR commit) in handle_actions (Step 7).
            self.ui.widget(cx, ids!(menu_btn)).as_caption_button().set_held(cx, true);
```

**Logo** (`app.rs:1084-1106`): replace `menu.open_popup(..)` + `set_app_menu_owner(Logo)` with:

```rust
            let anchor = dvec2(
                logo_rect.pos.x,
                (logo_rect.pos.y + logo_rect.size.y + crate::popup::menu::MENU_GAP)
                    .max(crate::popup::menu::CAPTION_H),
            );
            let bounds = self.window_bounds(cx);
            if let Some(mut pr) = self.ui.widget(cx, ids!(popup_root)).borrow_mut::<PopupRoot>() {
                pr.show_at(cx, PopupSpec::Menu {
                    tag: live_id!(logo),
                    anchor,
                    bounds,
                    items: logo_menu_items(),
                    open: MenuOpen::Popup,
                });
            }
            return;
```

(Opening the logo supersedes an open burger; the burger's `Closed{tag=burger, Dismissed}` fires from `show_at`'s supersede path and drops the burger glow via Step 7. The logo needs no glow.)

**Node radial** (`app.rs:1055-1071`): replace the `radial.open(..)` block with:

```rust
            let bounds = self.window_bounds(cx);
            if let Some(mut pr) = self.ui.widget(cx, ids!(popup_root)).borrow_mut::<PopupRoot>() {
                pr.show_at(cx, PopupSpec::Radial {
                    tag: live_id!(node_menu),
                    center: abs,
                    bounds,
                    items: node_radial_items(),
                    open: RadialOpen::Marking,
                });
            }
            return;
```

- [ ] **Step 6: Replace the two `handle()` blocks with one `route`**

Delete the whole radial-outcome block (`app.rs:1299-1326`) AND the app_menu-outcome block + reset (`app.rs:1328-1360`). In their place, before `self.ui.handle_event(cx, event, ..)` (`app.rs:1362`):

```rust
        // Single popup seam: light-dismiss + active-surface routing + emission.
        if let Some(mut pr) = self.ui.widget(cx, ids!(popup_root)).borrow_mut::<PopupRoot>() {
            pr.route(cx, event);
        }
```

- [ ] **Step 7: Move the commit/close handling into tag-filtered `closed` reads (handle_actions)**

The committed-outcome mapping that lived in the two matches moves into `App`'s action handler (the `match_event`/`handle_actions` path where `.picked(actions)` etc. are read — same block as `app.rs:906` sits in). Add, reading `popup_root.closed` per tag:

```rust
        // Popup outcomes (tag-filtered off the single action queue).
        if let Some(pr) = self.ui.widget(cx, ids!(popup_root)).borrow::<PopupRoot>() {
            let logo_closed = pr.closed(actions, live_id!(logo));
            let burger_closed = pr.closed(actions, live_id!(burger));
            let node_closed = pr.closed(actions, live_id!(node_menu));
            drop(pr);

            // Burger caller-local glow: any close of the burger tag drops it.
            if burger_closed.is_some() {
                self.ui.widget(cx, ids!(menu_btn)).as_caption_button().set_held(cx, false);
            }
            if let Some(PopupResult::Invoked(id)) = burger_closed {
                if id == live_id!(close_model) {
                    self.show_start_screen(cx);
                }
            }
            if let Some(PopupResult::Invoked(id)) = logo_closed {
                if let Some(cmd) = logo_command_for(id) {
                    match cmd {
                        LogoCommand::Properties => log!("logo command: Properties (stub)"),
                        LogoCommand::About => cx.open_url("https://github.com/redoz/waml", OpenUrlInPlace::No),
                        LogoCommand::Exit => cx.quit(),
                    }
                }
            }
            if let Some(PopupResult::Invoked(id)) = node_closed {
                if let Some(cmd) = crate::canvas::node_command_for(id) {
                    log!("node command: {cmd:?}");
                }
            }
        }
```

Notes:
- Mirror the exact borrow idiom the sibling readers use (`.borrow::<PopupRoot>()` returns a guard; `drop(pr)` before the `set_held`/`show_start_screen` calls avoids a double-borrow of `self.ui`). If `borrow` + explicit `drop` fights the borrow checker, read the three `closed` values into locals inside a tight scope `{ let pr = ...; (pr.closed(..), ..) }` then act outside it.
- The old radial block also mapped `logo_command_for(id)` for the radial (dead path — the logo is a menu now); it is dropped. `node_command_for` stays.
- `use crate::popup::root::{PopupRoot, PopupSpec, MenuOpen, RadialOpen}` and `use crate::popup::base::PopupResult` at the top of `app.rs` (or fully-qualify).

- [ ] **Step 8: Point the `WindowDragQuery` client-ize hook at `popup_root`**

Replace the `menu_open` computation (`app.rs:1404-1409`) with:

```rust
            let menu_open = self
                .ui
                .widget(cx, ids!(popup_root))
                .borrow::<PopupRoot>()
                .map(|pr| pr.is_open())
                .unwrap_or(false);
```

The `if over_tab || over_logo || over_btn || menu_open` guard is unchanged.

- [ ] **Step 9: Delete the old files + module declarations**

- Delete `crates/waml-editor/src/radial.rs` and `crates/waml-editor/src/app_menu.rs`.
- Remove `mod radial;` and `mod app_menu;` from `crates/waml-editor/src/main.rs`.
- Grep for any straggler references and fix them: `grep -rn "crate::radial\|crate::app_menu\|RadialItem\|RadialOutcome\|MenuOwner\|::Radial\b\|::AppMenu\b" crates/waml-editor/src`. The only expected hits are the ones this task already rewrote; anything else (e.g. a test module, `logo.rs`, `canvas.rs`) must be repointed to `crate::popup::*`.

- [ ] **Step 10: Build + run the full test suite**

Run: `taskkill //IM waml-editor.exe //F ; cargo test -p waml-editor && cargo build -p waml-editor`
Expected: PASS (all tests, including every ported `popup::*` test) + clean build, zero new warnings.

- [ ] **Step 11: End-to-end verification — drive the app**

Run the app: `taskkill //IM waml-editor.exe //F ; cargo run -p waml-editor` (or the project's run recipe). Open a model so the editor chrome shows, then confirm EACH:
1. **Node radial:** right-press a canvas node → the wedge disc blooms at the press point. Drag onto a wedge + release → commits (check the `node command:` log). Esc closes it.
2. **Logo menu:** left-click the top-left WAML wordmark → the drop-down card appears. Click About → opens the URL; the card closes.
3. **Burger menu:** press the caption burger → the card drops AND the burger lights (held glow). Pick "Close model" → returns to the start screen and the glow drops.
4. **Single-active:** open the burger (glow on), then click the logo → the burger card dies, the burger glow drops, the logo card shows. Only ever one popup visible.
5. **Universal dismiss on the active one:** with any popup open — press Esc → closes; click empty canvas → closes (outside-click); Alt-Tab away → closes (window-blur).

If any of 1–5 misbehaves, STOP and debug before committing (superpowers:systematic-debugging). Do not claim completion without observing all five.

- [ ] **Step 12: Commit**

```bash
git add -A
git commit -m "refactor(editor): collapse radial + app_menu into one PopupRoot seam

Replace the two hand-wired popup state machines + MenuOwner glow + two
handle()/outcome blocks + the drag-query hook with a single popup_root:
one route() per event, show_at from the three openers (logo/burger/node),
tag-filtered closed() reads for commits, burger glow caller-local. Delete
radial.rs + app_menu.rs (all logic ported to popup/*). Single-active +
universal light-dismiss now guaranteed by PopupRoot.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```
