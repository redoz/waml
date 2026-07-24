# Recents Pinning + VS-Tight Rows Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Visual-Studio-style per-row pin to the start screen's recents list (pinned models stay on the list and sort to a top block) and tighten the row to VS anatomy, sizing the list box to exactly five rows.

**Architecture:** `config.rs` gains a `pinned_at: Option<u64>` on `Recent`, a pure `sort_recents` (pinned block first, oldest-pin-on-top; unpinned MRU), a pin-exempt cap, and a `set_pinned` API. `recent_row.rs` grows a left `Package` glyph and a hand-rolled pin (drawn immediate over a child anchor, hover tracked by rect containment, its own `TogglePin` action). `start_screen.rs` pushes pin state per row, sizes the list box from a new `RecentRowView::ROW_HEIGHT` const, and routes `TogglePin`. `app.rs` drops `.take(5)`, maps pin state, and handles the toggle by persisting + reloading.

**Tech Stack:** Rust, Makepad (`script_mod!` DSL widgets, `#[deref] View` hybrid, `IconSet` SDF glyphs), serde JSON config.

## Global Constraints

- Never edit the main checkout (`C:\dev\waml`) and never the unrelated `C:\dev\waml\.worktrees\start-recents-5` checkout. All paths below are relative to YOUR OWN worktree — the one this run was cut into. Verify with `git rev-parse --show-toplevel` before the first edit; Edit/Write take absolute paths with no cwd, so a main-root path silently edits main while your build keeps using the stale worktree copy.
- Baseline note: this plan was drafted against a working copy that carried two uncommitted tweaks which are NOT on `main` — a `.take(5)` in `app.rs::show_start_screen` and the `recent_row.rs` `spacing: -2.0` / `text_label` / `text_menu` font swap. The code blocks below are the intended END state and already include the row tweaks; where a step says to remove `.take(5)`, expect it to be absent already and just land the block as written.
- `mod.fonts` semantic scale only — no ad-hoc `font_size:` / `FontMember` (a `fonts.rs` gate test forbids it). Rows use `fonts.text_label` (title, when) and `fonts.text_menu` (path).
- No RGBA crosses Rust: glyph tints are copied from DSL `DrawColor` holder fields (`atlas.*` tokens), exactly as `icon_button.rs` does.
- The cargo/clippy gate is headless (never boots the script VM), so DSL/runtime correctness is proven by a pid-specific screenshot, never by green tests alone. Capture by a specific pid; never kill processes by name.
- `#[serde(default)]` on every new persisted field so old `editor.json` files load unchanged.

---

### Task 1: config.rs — pinned data model, sort, pin-exempt cap, set_pinned

**Files:**
- Modify: `crates/waml-editor/src/config.rs`
- Test: `crates/waml-editor/src/config.rs` (its `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: existing `Recent`, `add_or_promote`, `prune_missing`, `canonical_key`, `now_unix`, `recents`.
- Produces:
  - `Recent::pinned(&self) -> bool`
  - `fn sort_recents(recents: &mut [Recent])` (private)
  - `fn apply_pin(recents: Vec<Recent>, path: &Path, pinned: bool, now: u64) -> Vec<Recent>` (private, pure)
  - `pub fn set_pinned(path: &Path, pinned: bool)`
  - `recents()` now returns the list already run through `sort_recents`.

- [ ] **Step 1: Add the `Ordering` import**

At the top of `crates/waml-editor/src/config.rs`, add to the `use std::...` block:

```rust
use std::cmp::Ordering;
```

- [ ] **Step 2: Add `pinned_at` to `Recent` and a `pinned()` getter**

Change the `Recent` struct (currently `crates/waml-editor/src/config.rs:111-119`) to:

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Recent {
    /// The OKF directory.
    path: PathBuf,
    /// Display name (the model's root name; see `push_recent` caller).
    title: String,
    /// Unix seconds, last time opened.
    opened_at: u64,
    /// Unix seconds when pinned; `None` when unpinned. Pinned recents sort to a
    /// block at the top (oldest pin first) and are exempt from the MRU cap.
    /// `#[serde(default)]` keeps files written before pinning existed loadable.
    #[serde(default)]
    pinned_at: Option<u64>,
}
```

Add to the `impl Recent` block (after `opened_at`, `crates/waml-editor/src/config.rs:133-135`):

```rust
    /// Whether this recent is pinned (kept on the list, sorted to the top block).
    pub fn pinned(&self) -> bool {
        self.pinned_at.is_some()
    }
```

- [ ] **Step 3: Add the `pinned_at` field to the one struct literal in `add_or_promote`**

In `add_or_promote` (`crates/waml-editor/src/config.rs:163-170`), the `recents.insert(0, Recent { ... })` gains the new field. A freshly opened model is unpinned:

```rust
    recents.insert(
        0,
        Recent {
            path: path.to_path_buf(),
            title: title.to_string(),
            opened_at,
            pinned_at: None,
        },
    );
```

- [ ] **Step 4: Write the failing test for `sort_recents`**

Add to `#[cfg(test)] mod tests` in `crates/waml-editor/src/config.rs`. First extend the `rec` helper (`crates/waml-editor/src/config.rs:239-245`) to take a pin stamp, and add a sort test:

```rust
    fn rec(path: &str, opened_at: u64) -> Recent {
        Recent {
            path: PathBuf::from(path),
            title: format!("t:{path}"),
            opened_at,
            pinned_at: None,
        }
    }

    fn pinned_rec(path: &str, opened_at: u64, pinned_at: u64) -> Recent {
        Recent {
            path: PathBuf::from(path),
            title: format!("t:{path}"),
            opened_at,
            pinned_at: Some(pinned_at),
        }
    }

    #[test]
    fn sort_recents_pins_first_oldest_pin_on_top_then_mru() {
        let mut list = vec![
            rec("/u1", 10),
            pinned_rec("/p_late", 1, 200),
            rec("/u2", 30),
            pinned_rec("/p_early", 1, 100),
        ];
        sort_recents(&mut list);
        // Pinned block first, ascending pin time (a fresh pin lands directly
        // below the last-pinned item).
        assert_eq!(list[0].path, PathBuf::from("/p_early"));
        assert_eq!(list[1].path, PathBuf::from("/p_late"));
        // Then unpinned, newest opened_at first.
        assert_eq!(list[2].path, PathBuf::from("/u2"));
        assert_eq!(list[3].path, PathBuf::from("/u1"));
    }
```

- [ ] **Step 5: Run it to verify it fails**

Run: `cargo test -p waml-editor --lib config::tests::sort_recents_pins_first_oldest_pin_on_top_then_mru`
Expected: FAIL — `cannot find function `sort_recents`` (and the `pinned_at` field errors if Steps 2-3 not yet applied).

- [ ] **Step 6: Implement `sort_recents`**

Add above `add_or_promote` (near `crates/waml-editor/src/config.rs:152`):

```rust
/// Order the recents for display: pinned block first (ascending `pinned_at`,
/// so a freshly pinned entry lands directly below the last-pinned item), then
/// the unpinned tail in MRU order (newest `opened_at` first). Stable sort.
fn sort_recents(recents: &mut [Recent]) {
    recents.sort_by(|a, b| match (a.pinned_at, b.pinned_at) {
        (Some(ap), Some(bp)) => ap.cmp(&bp),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => b.opened_at.cmp(&a.opened_at),
    });
}
```

- [ ] **Step 7: Run it to verify it passes**

Run: `cargo test -p waml-editor --lib config::tests::sort_recents_pins_first_oldest_pin_on_top_then_mru`
Expected: PASS

- [ ] **Step 8: Write the failing test for the pin-exempt cap**

Add to the tests module:

```rust
    #[test]
    fn cap_exempts_pins_and_caps_only_unpinned() {
        // RECENTS_CAP unpinned already present, plus two pins.
        let mut list = vec![pinned_rec("/pin_a", 1, 10), pinned_rec("/pin_b", 1, 20)];
        for i in 0..RECENTS_CAP {
            list = add_or_promote(list, Path::new(&format!("/u{i}")), "t", i as u64);
        }
        // One more distinct unpinned open.
        list = add_or_promote(list, Path::new("/u-new"), "t", 999);
        // Both pins survive regardless of the cap.
        assert!(list.iter().any(|r| r.path == Path::new("/pin_a")));
        assert!(list.iter().any(|r| r.path == Path::new("/pin_b")));
        // Exactly RECENTS_CAP unpinned kept (the newest).
        let unpinned = list.iter().filter(|r| !r.pinned()).count();
        assert_eq!(unpinned, RECENTS_CAP);
        assert!(list.iter().any(|r| r.path == Path::new("/u-new")));
    }
```

- [ ] **Step 9: Run it to verify it fails**

Run: `cargo test -p waml-editor --lib config::tests::cap_exempts_pins_and_caps_only_unpinned`
Expected: FAIL — the current blanket `truncate(RECENTS_CAP)` drops pins / keeps the wrong count.

- [ ] **Step 10: Make the cap pin-exempt in `add_or_promote`**

Replace the tail of `add_or_promote` (`crates/waml-editor/src/config.rs:171-172`, the `recents.truncate(RECENTS_CAP); recents`) with:

```rust
    // Pinned entries are exempt from the cap; the cap trims only the unpinned
    // tail. Sort first so the retained unpinned are the newest.
    sort_recents(&mut recents);
    let mut unpinned_kept = 0usize;
    recents.retain(|r| {
        if r.pinned_at.is_some() {
            true
        } else {
            unpinned_kept += 1;
            unpinned_kept <= RECENTS_CAP
        }
    });
    recents
```

- [ ] **Step 11: Run both new tests to verify they pass**

Run: `cargo test -p waml-editor --lib config::tests::cap_exempts_pins_and_caps_only_unpinned config::tests::sort_recents_pins_first_oldest_pin_on_top_then_mru`
Expected: PASS (both)

- [ ] **Step 12: Fix the remaining `Recent` struct literals in existing tests**

Two existing tests build `Recent` by hand and now miss `pinned_at`. Update:

`prune_drops_missing_keeps_existing` (`crates/waml-editor/src/config.rs:303-314`) — add `pinned_at: None,` to both `Recent { ... }` literals.

`recent_getters_return_stored_fields` (`crates/waml-editor/src/config.rs:322-326`) — add `pinned_at: None,` to the `Recent { ... }` literal.

- [ ] **Step 13: Write the failing test for `apply_pin` + serde forward-compat**

Add to the tests module:

```rust
    #[test]
    fn apply_pin_sets_and_clears_stamp() {
        let list = vec![rec("/a", 1), rec("/b", 2)];
        let pinned = apply_pin(list, Path::new("/b"), true, 500);
        let b = pinned.iter().find(|r| r.path == Path::new("/b")).unwrap();
        assert_eq!(b.pinned_at, Some(500), "pin stamps now");
        assert!(pinned.iter().find(|r| r.path == Path::new("/a")).unwrap().pinned_at.is_none());

        let unpinned = apply_pin(pinned, Path::new("/b"), false, 999);
        assert!(unpinned.iter().find(|r| r.path == Path::new("/b")).unwrap().pinned_at.is_none(), "unpin clears");
    }

    #[test]
    fn old_recent_without_pinned_field_loads_unpinned() {
        let tmp = TempDir::new();
        std::fs::write(
            tmp.path().join(EDITOR_FILE),
            br#"{"version":1,"recents":[{"path":"/x","title":"t","opened_at":1}]}"#,
        )
        .unwrap();
        let cfg: EditorConfig = load_from(tmp.path(), EDITOR_FILE);
        assert_eq!(cfg.recents.len(), 1);
        assert!(!cfg.recents[0].pinned(), "absent pinned_at -> unpinned");
    }
```

- [ ] **Step 14: Run it to verify it fails**

Run: `cargo test -p waml-editor --lib config::tests::apply_pin_sets_and_clears_stamp`
Expected: FAIL — `cannot find function `apply_pin``

- [ ] **Step 15: Implement `apply_pin` and `set_pinned`**

Add `apply_pin` next to `add_or_promote` (pure, so the test above needs no filesystem):

```rust
/// Set or clear the pin on every recent whose canonical path matches `path`
/// (there is at most one after dedup). Pure over the vector; `set_pinned` wraps
/// it with load/store.
fn apply_pin(mut recents: Vec<Recent>, path: &Path, pinned: bool, now: u64) -> Vec<Recent> {
    let key = canonical_key(path);
    for r in recents.iter_mut() {
        if canonical_key(&r.path) == key {
            r.pinned_at = if pinned { Some(now) } else { None };
        }
    }
    recents
}
```

Add the public API next to `push_recent` (after `crates/waml-editor/src/config.rs:232`):

```rust
/// Set/clear the pin on the recent whose canonical path matches `path`, then
/// persist. Best-effort — a write failure is logged and swallowed. The caller
/// reloads via `recents()` to see the re-sorted list.
pub fn set_pinned(path: &Path, pinned: bool) {
    let mut config: EditorConfig = load(EDITOR_FILE);
    config.version = EDITOR_VERSION;
    config.recents = apply_pin(config.recents, path, pinned, now_unix());
    if let Err(e) = store(EDITOR_FILE, &config) {
        log!("waml-editor: failed to persist pin {:?}={pinned}: {e}", path);
    }
}
```

- [ ] **Step 16: Sort in `recents()` so all readers see the pinned block first**

Change `recents()` (`crates/waml-editor/src/config.rs:187-190`) to:

```rust
pub fn recents() -> Vec<Recent> {
    let config: EditorConfig = load(EDITOR_FILE);
    let mut list = prune_missing(config.recents);
    sort_recents(&mut list);
    list
}
```

- [ ] **Step 17: Run the whole config test module**

Run: `cargo test -p waml-editor --lib config::`
Expected: PASS (all — existing + 4 new).

- [ ] **Step 18: Commit**

```bash
git add crates/waml-editor/src/config.rs
git commit -m "feat(config): pin recents — pinned_at, sort_recents, pin-exempt cap, set_pinned"
```

---

### Task 2: recent_row.rs — VS-tight row, Package glyph, hand-rolled pin

**Files:**
- Modify: `crates/waml-editor/src/recent_row.rs`
- Test: `crates/waml-editor/src/recent_row.rs` (new `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `crate::icons::{Icon, IconSet}`; the existing `RecentRowView` `#[deref] View` hybrid + `set_title/set_path/set_when/set_clickable/clicked` and their `RecentRowViewRef` mirrors.
- Produces:
  - `RecentRowViewAction::TogglePin` (new variant)
  - `RecentRowView::ROW_HEIGHT: f64` (const, `= 30.0`)
  - `RecentRowView::set_pinned(&mut self, cx: &mut Cx, pinned: bool)` + Ref mirror
  - `RecentRowView::pin_toggled(&self, actions: &Actions) -> bool` + Ref mirror

- [ ] **Step 1: Add the icons import**

At the top of `crates/waml-editor/src/recent_row.rs`, after `use makepad_widgets::*;`:

```rust
use crate::icons::{Icon, IconSet};
```

- [ ] **Step 2: Rework the DSL — Package glyph anchor, tight padding, pin anchor, tint holders**

Replace the whole `mod.widgets.RecentRowView = set_type_default() do ...` block body (`crates/waml-editor/src/recent_row.rs:26-111`) with:

```
    mod.widgets.RecentRowView = set_type_default() do mod.widgets.RecentRowViewBase{
        width: Fill
        height: Fit
        flow: Right
        align: Align{y: 0.5}
        // Tighter than before (was top/bottom 5) to reach VS's compact pitch.
        padding: Inset{left: 12.0, right: 12.0, top: 3.0, bottom: 3.0}
        spacing: 10.0
        show_bg: true

        // Row hover wash (unchanged): a subtle premultiplied accent fill faded
        // by the `hover` uniform the widget pushes from its rect-containment
        // hover tracking each draw.
        draw_bg +: {
            color: atlas.accent
            hover: uniform(0.0)
            pixel: fn() {
                let a = 0.12 * self.hover
                return vec4(self.color.x * a, self.color.y * a, self.color.z * a, a)
            }
        }

        // Colour-only holders (never drawn): the immediate-mode glyphs copy
        // `color` from these per draw, so no RGBA crosses Rust (icon_button.rs
        // pattern). `draw_pkg` tints the left document glyph; the pin uses
        // `draw_pin_lit` when pinned or pin-hovered, else `draw_pin_idle`.
        draw_pkg +: { color: atlas.text_dim }
        draw_pin_idle +: { color: atlas.text_dim }
        draw_pin_lit +: { color: atlas.accent }

        // Left document glyph anchor: a 16x16 spacer reserving the flow slot;
        // `Icon::Package` is drawn immediate over this rect in `draw_walk`.
        glyph := View {
            width: 16.0
            height: 16.0
        }

        // Title over path, stacked. The timestamp rides the title line; `title`
        // is `Fill` inside `titlerow`, shoving the `Fit` `when` flush right.
        textcol := View {
            width: Fill
            height: Fit
            flow: Down
            spacing: -2.0

            titlerow := View {
                width: Fill
                height: Fit
                flow: Right
                align: Align{y: 0.5}
                title := Label {
                    width: Fill
                    text: ""
                    draw_text +: {
                        color: atlas.text
                        text_style: fonts.text_label
                    }
                }
                when := Label {
                    text: ""
                    draw_text +: {
                        color: atlas.text_dim
                        text_style: fonts.text_label
                    }
                }
            }

            path := Label {
                text: ""
                draw_text +: {
                    color: atlas.text_dim
                    text_style: fonts.text_menu
                }
            }
        }

        // Pin anchor: a 20x20 spacer at the row's right edge. `Icon::Pin` is
        // drawn immediate over this rect (centered 16px) only when the row is
        // hovered or the row is pinned (VS on-hover reveal). Its own FingerUp is
        // hit-tested first in `handle_event` so a pin click toggles without
        // opening the model.
        pin := View {
            width: 20.0
            height: 20.0
        }
    }
```

- [ ] **Step 3: Add the `TogglePin` action variant**

Change the action enum (`crates/waml-editor/src/recent_row.rs:117-122`) to:

```rust
#[derive(Clone, Debug, Default)]
pub enum RecentRowViewAction {
    #[default]
    None,
    /// The row body was clicked — open this recent.
    Clicked,
    /// The pin button was clicked — toggle this recent's pinned state.
    TogglePin,
}
```

- [ ] **Step 4: Add the new struct fields**

Replace the `RecentRowView` struct (`crates/waml-editor/src/recent_row.rs:124-139`) with:

```rust
#[derive(Script, ScriptHook, Widget)]
pub struct RecentRowView {
    /// The row container: glyph anchor + stacked text + pin anchor.
    #[deref]
    view: View,

    /// SDF icon set (shared Atlas material), drawn via `IconSet::draw`.
    #[live]
    icons: IconSet,
    /// Colour-only holders, copied into the glyph tint per draw.
    #[live]
    draw_pkg: DrawColor,
    #[live]
    draw_pin_idle: DrawColor,
    #[live]
    draw_pin_lit: DrawColor,

    /// Row pointer-over state, tracked by rect containment (a child area would
    /// steal `Hit::FingerHover`; the containment test keeps the pin inside the
    /// row's hover). Fed to the `hover` uniform each `draw_walk`.
    #[rust]
    hovered: bool,
    /// Pointer-over the pin anchor specifically (lights the pin tint).
    #[rust]
    pin_hovered: bool,
    /// Whether this row responds to hover/click. The empty-state row leaves it
    /// false so it neither washes, fires, nor draws a pin.
    #[rust]
    clickable: bool,
    /// Whether this recent is pinned (drives the pin's always-visible + accent
    /// tint). Pushed per row from `StartScreen`.
    #[rust]
    pinned: bool,
    /// Cached absolute rects from the last `draw_walk`, for containment hover.
    #[rust]
    row_rect: Rect,
    #[rust]
    glyph_rect: Rect,
    #[rust]
    pin_rect: Rect,
    /// The pin anchor's `Area`, for hit-testing its FingerUp before the row's.
    #[rust]
    pin_area: Area,
}
```

- [ ] **Step 5: Rewrite `handle_event` — containment hover + pin-first click**

Replace the whole `impl Widget for RecentRowView { fn handle_event ... }` (`crates/waml-editor/src/recent_row.rs:141-162`) with:

```rust
impl Widget for RecentRowView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if !self.clickable {
            return;
        }
        let uid = self.widget_uid();

        // Hover by rect containment: a child area (the pin) would claim
        // Hit::FingerHover and drop the row's hover the moment the pointer
        // crosses onto the pin, so track both off raw MouseMove instead
        // (the scrim-hover fix). The row rect encloses the pin, so hovering the
        // pin keeps the row hovered -> the revealed pin never flickers away.
        if let Event::MouseMove(e) = event {
            let over_row = self.row_rect.contains(e.abs);
            let over_pin = self.pin_rect.contains(e.abs);
            if over_row != self.hovered || over_pin != self.pin_hovered {
                self.hovered = over_row;
                self.pin_hovered = over_pin;
                self.view.redraw(cx);
            }
        }

        // Pin claims its FingerUp first (toggles without opening). The pin area
        // is topmost over its rect, so the row body's hit below bails there.
        if let Hit::FingerUp(fe) = event.hits(cx, self.pin_area) {
            if fe.is_primary_hit() && fe.is_over {
                cx.widget_action(uid, RecentRowViewAction::TogglePin);
                return;
            }
        }

        match event.hits(cx, self.view.area()) {
            Hit::FingerUp(fe) if fe.is_primary_hit() && fe.is_over => {
                cx.widget_action(uid, RecentRowViewAction::Clicked);
            }
            Hit::FingerHoverIn(_) => {
                cx.set_cursor(MouseCursor::Hand);
            }
            _ => {}
        }
    }
```

- [ ] **Step 6: Rewrite `draw_walk` — push hover, cache rects, draw glyphs**

Replace the `draw_walk` method (`crates/waml-editor/src/recent_row.rs:166-171`, still inside `impl Widget`) with:

```rust
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view
            .draw_bg
            .set_uniform(cx, live_id!(hover), &[if self.hovered { 1.0 } else { 0.0 }]);
        let step = self.view.draw_walk(cx, scope, walk);

        // Cache post-layout rects for containment hover + immediate glyphs.
        self.row_rect = self.view.area().rect(cx);
        self.glyph_rect = self.view.widget(cx, ids!(glyph)).area().rect(cx);
        self.pin_area = self.view.widget(cx, ids!(pin)).area();
        self.pin_rect = self.pin_area.rect(cx);

        // Left document glyph, filling its 16x16 anchor.
        self.icons
            .draw(cx, Icon::Package, self.glyph_rect, self.draw_pkg.color);

        // Pin: VS on-hover reveal — draw only when the row is hovered or pinned.
        // Accent when pinned or pin-hovered, else dim.
        if self.clickable && (self.hovered || self.pinned) {
            let lit = self.pinned || self.pin_hovered;
            let tint = if lit {
                self.draw_pin_lit.color
            } else {
                self.draw_pin_idle.color
            };
            let sz = 16.0;
            let g = Rect {
                pos: dvec2(
                    (self.pin_rect.pos.x + (self.pin_rect.size.x - sz) * 0.5).round(),
                    (self.pin_rect.pos.y + (self.pin_rect.size.y - sz) * 0.5).round(),
                ),
                size: dvec2(sz, sz),
            };
            self.icons.draw(cx, Icon::Pin, g, tint);
        }
        step
    }
}
```

- [ ] **Step 7: Add `ROW_HEIGHT`, `set_pinned`, `pin_toggled` to `impl RecentRowView`**

Add these to the existing `impl RecentRowView` block (alongside `set_title` etc., `crates/waml-editor/src/recent_row.rs:174-201`):

```rust
    /// Intrinsic drawn pitch of one row (the 16px glyph / stacked text plus the
    /// 2x3 vertical padding). `StartScreen` sizes its list box to
    /// `5 * ROW_HEIGHT + list padding` so exactly five rows fit; verify the fit
    /// by screenshot after any font/padding change and retune if a 6th peeks in.
    pub const ROW_HEIGHT: f64 = 30.0;

    /// Drive the pinned state (pin glyph visibility + accent tint), redrawing
    /// only on a change.
    pub fn set_pinned(&mut self, cx: &mut Cx, pinned: bool) {
        if self.pinned != pinned {
            self.pinned = pinned;
            self.view.redraw(cx);
        }
    }

    /// True when this row's pin emitted a toggle in `actions`.
    pub fn pin_toggled(&self, actions: &Actions) -> bool {
        actions
            .find_widget_action(self.widget_uid())
            .is_some_and(|a| matches!(a.cast(), RecentRowViewAction::TogglePin))
    }
```

- [ ] **Step 8: Mirror the new methods on `RecentRowViewRef`**

Add to the `impl RecentRowViewRef` block (`crates/waml-editor/src/recent_row.rs:203-231`):

```rust
    pub fn set_pinned(&self, cx: &mut Cx, pinned: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_pinned(cx, pinned);
        }
    }
    /// See [`RecentRowView::pin_toggled`].
    pub fn pin_toggled(&self, actions: &Actions) -> bool {
        self.borrow().is_some_and(|inner| inner.pin_toggled(actions))
    }
```

- [ ] **Step 9: Add a shape-gate test module**

Append to `crates/waml-editor/src/recent_row.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_default_is_none() {
        assert!(matches!(
            RecentRowViewAction::default(),
            RecentRowViewAction::None
        ));
    }

    #[test]
    fn row_height_is_positive() {
        // The list box height (5 * ROW_HEIGHT + padding) must be well-formed.
        assert!(RecentRowView::ROW_HEIGHT > 0.0);
    }
}
```

- [ ] **Step 10: Build + run the new tests**

Run: `cargo test -p waml-editor --lib recent_row::`
Expected: PASS. (The build also confirms the DSL struct fields resolve.)

- [ ] **Step 11: Commit**

```bash
git add crates/waml-editor/src/recent_row.rs
git commit -m "feat(recents): VS-tight row — Package glyph, hand-rolled pin, ROW_HEIGHT"
```

---

### Task 3: start_screen.rs — pin state per row, 5-row box, TogglePin routing

**Files:**
- Modify: `crates/waml-editor/src/start_screen.rs`
- Test: `crates/waml-editor/src/start_screen.rs` (its `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `RecentRowView::ROW_HEIGHT`, `RecentRowViewRef::{set_pinned, pin_toggled}` (Task 2); `RecentRow`, `row_index_for`, `StartScreenAction` (existing).
- Produces: `RecentRow.pinned: bool`; `StartScreenAction::TogglePin(usize)`; the list box sized to five rows.

- [ ] **Step 1: Add `pinned` to `RecentRow`**

Change `RecentRow` (`crates/waml-editor/src/start_screen.rs:248-254`) to:

```rust
pub(crate) struct RecentRow {
    pub title: String,
    pub path: String,
    /// Preformatted local "M/D/YYYY h:mm AM/PM" last-opened stamp.
    pub when: String,
    /// Whether this recent is pinned (drives the row's pin glyph).
    pub pinned: bool,
}
```

- [ ] **Step 2: Add the `TogglePin` action variant**

Change `StartScreenAction` (`crates/waml-editor/src/start_screen.rs:256-264`) to:

```rust
#[derive(Clone, Debug, Default)]
pub enum StartScreenAction {
    #[default]
    None,
    /// A recent row was clicked; indexes the rows last passed to `set_recents`.
    OpenRecent(usize),
    /// A recent row's pin was toggled; indexes the rows passed to `set_recents`.
    TogglePin(usize),
    NewProject,
    OpenProject,
}
```

- [ ] **Step 3: Change the DSL `list_frame` height to `Fit` (Rust sizes it)**

In the `list_frame := View { ... }` block, change the fixed height (`crates/waml-editor/src/start_screen.rs:186`, `height: 320.0`) to:

```
                        // Height is set from Rust in `draw_walk` to exactly five
                        // `RecentRowView::ROW_HEIGHT` rows plus this box's Inset
                        // padding, so the box fits five rows and scrolls beyond.
                        height: Fit
```

- [ ] **Step 4: Size the list box from `ROW_HEIGHT` in `draw_walk`**

In `StartScreen::draw_walk` (`crates/waml-editor/src/start_screen.rs:309-348`), immediately after the `self.seat_subtitle_baseline(cx);` line (`:314`) and before the `while let Some(item) = ...` loop, insert:

```rust
        // Fix the list box to exactly five rows plus its own Inset-5 top/bottom
        // padding, so the recents box always shows five rows and scrolls beyond.
        let box_h = 5.0 * crate::recent_row::RecentRowView::ROW_HEIGHT + 2.0 * 5.0;
        if let Some(mut frame) = self.view.view(cx, ids!(list_frame)).borrow_mut() {
            frame.walk.height = Size::Fixed(box_h);
        }
```

- [ ] **Step 5: Push pin state into each real row**

In the `else` branch of the interpose loop (`crates/waml-editor/src/start_screen.rs:332-343`), after `rv.set_clickable(true);` add:

```rust
                        rv.set_pinned(cx, row_data.pinned);
```

- [ ] **Step 6: Route the pin toggle in `handle_actions`**

In `StartScreen::handle_actions`, inside the `for (item_id, item) in list.items_with_actions(actions)` loop (`crates/waml-editor/src/start_screen.rs:366-372`), add a second check after the `clicked` block:

```rust
            if item.as_recent_row_view().pin_toggled(actions) {
                if let Some(i) = row_index_for(&self.rows, item_id) {
                    cx.widget_action(uid, StartScreenAction::TogglePin(i));
                }
            }
```

- [ ] **Step 7: Fix the two `RecentRow` literals in the test module**

The test helper `row` (`crates/waml-editor/src/start_screen.rs:434-440`) and the empty-vs-default test now need `pinned`. Change the helper to:

```rust
    fn row(path: &str) -> RecentRow {
        RecentRow {
            title: "t".into(),
            path: path.into(),
            when: "w".into(),
            pinned: false,
        }
    }
```

- [ ] **Step 8: Add a test locking `TogglePin` into the action enum**

Add to the `#[cfg(test)] mod tests` in `crates/waml-editor/src/start_screen.rs`:

```rust
    #[test]
    fn toggle_pin_indexes_a_row() {
        // A pinned row carries the flag, and TogglePin round-trips an index.
        let r = RecentRow {
            title: "t".into(),
            path: "/p".into(),
            when: "w".into(),
            pinned: true,
        };
        assert!(r.pinned);
        assert!(matches!(
            StartScreenAction::TogglePin(3),
            StartScreenAction::TogglePin(3)
        ));
    }
```

- [ ] **Step 9: Build + run the start_screen tests**

Run: `cargo test -p waml-editor --lib start_screen::`
Expected: PASS (existing + new; the build confirms `Size::Fixed`, `view(...).borrow_mut().walk`, and the new Ref methods resolve).

- [ ] **Step 10: Commit**

```bash
git add crates/waml-editor/src/start_screen.rs
git commit -m "feat(start-screen): pin state per row, 5-row list box, TogglePin routing"
```

---

### Task 4: app.rs — drop take(5), map pin state, handle TogglePin

**Files:**
- Modify: `crates/waml-editor/src/app.rs`

**Interfaces:**
- Consumes: `config::recents`, `config::set_pinned`, `Recent::{path, pinned}` (Task 1); `RecentRow.pinned`, `StartScreenAction::TogglePin` (Task 3); the existing `show_start_screen`, `start_recents`, `screen_action` match.
- Produces: no new public interface; wires the toggle end-to-end.

- [ ] **Step 1: Drop `.take(5)` and map the pin flag into each `RecentRow`**

In `show_start_screen` (`crates/waml-editor/src/app.rs`, around `:740-765` — locate by the `let rows: Vec<crate::start_screen::RecentRow>` binding, not by line number), remove the `.take(5)` iterator step and its comment **if present** (it was never committed to `main`, so most likely there is nothing to remove), and add `pinned` to the constructed `RecentRow`. The block becomes:

```rust
        let rows: Vec<crate::start_screen::RecentRow> = self
            .start_recents
            .iter()
            // All recents feed the list; the box (Task 3) bounds the visible
            // count to five and scrolls beyond, so pins past five stay reachable.
            // Click/toggle indices still map 1:1 (rows[i] == start_recents[i]).
            .map(|r| crate::start_screen::RecentRow {
                title: r.title().to_string(),
                path: r.path().display().to_string(),
                when: format_opened(r.opened_at()),
                pinned: r.pinned(),
            })
            .collect();
```

- [ ] **Step 2: Handle `TogglePin` in the start-screen action match**

In the `match action { ... }` block (`crates/waml-editor/src/app.rs:1606-1621`), add a `TogglePin` arm after the `OpenRecent` arm:

```rust
                    crate::start_screen::StartScreenAction::TogglePin(i) => {
                        if let Some(recent) = self.start_recents.get(i).cloned() {
                            // Flip persisted pin, then reload the start screen so
                            // the list re-sorts (pinned block floats to the top).
                            crate::config::set_pinned(recent.path(), !recent.pinned());
                            self.show_start_screen(cx);
                        }
                    }
```

- [ ] **Step 3: Build the whole editor**

Run: `cargo build -p waml-editor`
Expected: builds clean (the two benign fork dup-package warnings are expected; no errors).

- [ ] **Step 4: Run the full editor test suite**

Run: `cargo test -p waml-editor --lib`
Expected: PASS (all).

- [ ] **Step 5: Commit**

```bash
git add crates/waml-editor/src/app.rs
git commit -m "feat(app): feed all recents + wire pin toggle (persist + reload)"
```

---

### Task 5: Visual verification + verification note

**Files:**
- Add: `docs/superpowers/plans/notes/2026-07-24-recents-pinning-verification.md`

The gate never boots the script VM, so the DSL row layout, the Package glyph, the pin reveal/tint, and the 5-row box fit are only provable by eye. Verify against the VS reference (tight pitch, doc glyph left, pin right).

**Unattended-run rules for this task (the human is AFK and cannot drive the mouse):**
- Static checks (Steps 1, 2, 4) ARE doable headlessly: launch, screenshot by pid, read the pixels. Do them.
- Hover/click checks (Step 3) need a real pointer. Do NOT fake, skip silently, or synthesize input. Record them as **owed interactive sign-off** in the verification note.
- Never leave the launched editor process running: close the window (or stop that one pid — never by process name; the human may have their own editor open).
- A screenshot mismatch is a real failure: fix it (e.g. retune `ROW_HEIGHT`) and re-verify. Do not lower the bar to close the task.

- [ ] **Step 1: Launch the worktree's own build and capture by pid**

Launch YOUR worktree's own binary (NOT main's stale exe — `scripts/run-native.ps1` builds the checkout the `.ps1` itself lives in, so run your worktree's own copy of the script), let it reach the start screen with several recents, and capture a screenshot by the launched process's specific pid in one PowerShell call. Do not capture by window name (it would grab the user's own open editor) and do not kill processes by name.

- [ ] **Step 2: Verify the row anatomy + spacing**

Confirm each row reads `[Package glyph] [title / path tight] [when, right] [pin]`, the pitch is VS-tight (no loose gap between title and path), and the fonts match (title `text_label`, path `text_menu`).

- [ ] **Step 3: Pin reveal + toggle (pointer-driven — record as owed)**

The full check is: hover a row → pin appears dim; hover the pin → it lights accent; click the pin → the row does NOT open, the model pins and jumps to the top block; re-open the start screen and the pin is lit inside the pinned block; unpin → returns to MRU order.

Unattended, only the *pinned-state rendering* is provable: hand-write a `pinned_at` into the editor config's `editor.json` recents for one entry (or seed one via the config API in a test binary), relaunch, and screenshot — the pinned row must render lit-accent pin and sort into the top block. The pointer half (hover reveal, pin-click-wins-over-row-click) goes into the note as owed interactive sign-off.

- [ ] **Step 4: Verify the 5-row box fit**

With 6+ recents, confirm exactly five rows are visible in the box with no partial sixth row peeking, and the list scrolls to reach the rest. If a sixth peeks in or the fifth is clipped, bump `RecentRowView::ROW_HEIGHT` (Task 2) and re-verify.

- [ ] **Step 5: Write the verification note and commit it**

Write `docs/superpowers/plans/notes/2026-07-24-recents-pinning-verification.md` covering: what was checked headlessly and the verdict per check (row anatomy, fonts, 5-row fit, pinned-state render), the final measured `ROW_HEIGHT` (and whether it was retuned from 30.0), and an explicit **Owed interactive sign-off** list (hover reveal, pin-hover tint, pin-click-does-not-open-the-model, live re-sort on toggle). Be plain about anything that failed or could not be checked.

```bash
git add docs/superpowers/plans/notes/2026-07-24-recents-pinning-verification.md
git commit -m "docs(recents): visual verification note + owed interactive sign-off"
```

---

## Self-Review

**Spec coverage:**
- §1 config data model → Task 1 (pinned_at, pinned(), sort_recents, pin-exempt cap, set_pinned, recents() sorts). ✓
- §2 recent_row VS anatomy → Task 2 (Package glyph, padding 3, spacing -2 retained, fonts unchanged, pin reveal states, pin-first FingerUp, TogglePin, set_pinned/pin_toggled, ROW_HEIGHT). ✓ Deviation from spec §2: the pin is hand-rolled inside `RecentRowView` (not a mounted `IconButton`) and hover is tracked by rect containment — required because `RecentRowView::handle_event` does not forward to children and a child button would lose the row's hover to the documented hover-arbiter bug. Same VS behavior, no shared-widget churn.
- §3 start_screen → Task 3 (drop fixed 320, box = 5·ROW_HEIGHT + 2·5, RecentRow.pinned, TogglePin routing). ✓
- §4 action routing → Task 3 (StartScreenAction::TogglePin) + Task 4 (App set_pinned + reload, drop take(5)). ✓
- Testing (pure config + shape-gate) → Task 1 (sort/cap/apply_pin/serde), Task 2 (action default, ROW_HEIGHT), Task 3 (TogglePin/pinned). ✓
- Risks (ROW_HEIGHT drift, pin-vs-row order, serde forward-compat) → Task 5 fit verification, Task 2 pin-first hit test, Task 1 serde-default test. ✓

**Placeholder scan:** No TBD/TODO; every code step carries full code. ✓

**Type consistency:** `set_pinned` (config free fn, `&Path`), `RecentRowView::set_pinned`/`RecentRowViewRef::set_pinned` (`cx, bool`), `pin_toggled` (both), `ROW_HEIGHT`, `RecentRow.pinned`, `StartScreenAction::TogglePin(usize)`, `RecentRowViewAction::TogglePin` — names match across Tasks 1-4. ✓
