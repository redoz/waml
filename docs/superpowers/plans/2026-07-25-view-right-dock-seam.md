# View-owned right dock Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the floating three-state inspector into the right-hand twin of the project tree — a flush, binary open/closed column in `right_slot`, toggled by an `[I]` `IconButton` anchored at the right of the caption's tab row — and make *which* right-hand dock exists a per-view declaration on the `DocView` seam rather than app chrome.

**Architecture:** Four moving parts, in dependency order. (1) The shared `IconButton` splits its single `lit` flag into two channels so a resting-active toggle reads as a bare accent glyph rather than a hover wash. (2) The existing `DocView` seam grows one declaration member (`BodyChrome.right_dock: Option<Icon>`) and one upward request member (`ViewOutcome::open_right_dock: bool`); all three concrete views return `Some(Icon::InspectionPanel)` because all three drive the one shared `inspector` widget today. (3) The `Inspector` widget loses its flag spine, pin button and `Peek` auto-collapse timer and becomes a `Flag`⇄`Pinned` column mounted inside `right_slot` — a literal mirror of what `ProjectTree` already is; `peek_layer` / `right_peek_wrap` are then deleted. (4) The caption gains the `[I]` toggle, wired exactly like `[T]`: visibility+glyph from `BodyChrome`, click → `Inspector::toggle_dock`, lit state from `slot_width()` in `sync_dock_slots`, and its own `WindowDragQuery` arm.

**Tech Stack:** Rust, the `waml-editor` binary crate, the redoz makepad fork (`Widget` / `View` / `Turtle` / `DrawStep`, the `script_mod!` DSL), the pure `crate::dock` state model.

**Source spec:** `docs/superpowers/specs/2026-07-25-view-right-dock-seam-design.md` (APPROVED). Its `## Decisions` section is settled — do not relitigate it.

## Global Constraints

- **The per-task gate is `cargo test --workspace` AND clippy with `-D warnings`** (`cargo clippy --workspace --all-targets -- -D warnings`). The workflow's gate also runs `pnpm -r test && pnpm lint && pnpm build`; this change touches no TypeScript, so those legs must simply stay green.
- **`dead_code` is promoted to a HARD error** by the clippy `-D warnings` leg. A new `pub` item on a *binary* crate with no caller reds the whole run. `doc_view.rs` carries `#![allow(dead_code)]` at line 11 and `dock.rs` at line 10 — new members in those two files are safe. `inspector_panel.rs`, `app.rs`, `doc_tabs.rs` and `icon_button.rs` do **not**; every item added there must have a caller in the same task (or an explicit `#[allow(dead_code)]`, which `Inspector::dock_state` at `inspector_panel.rs:1081` already models).
- **Unused imports are also errors** under `-D warnings`. Task 3 deletes the last `IconButton` use from `inspector_panel.rs` — its `use crate::icon_button::IconButtonWidgetRefExt;` (line 31) and the `DockEdge`/`PeekTimer` names in line 30 must go in the same commit.
- **A docked panel's `Flag` draw branch MUST loop its inner draw to completion:** `while self.view.draw_walk(cx, scope, fw).step().is_some() {}`. A one-shot `let _ = view.draw_walk(..)` leaves the turtle begun-and-never-ended, unbalances the window's turtle stack, and silently blanks the caption **and both side panels**. Clean stderr; the whole automated gate is blind to it. This exact bug shipped once on the tree side (fixed in e62ad58). Both existing Flag branches (`tree_panel.rs:595`, `inspector_panel.rs:649`) already loop — keep it that way.
- **A caption-bar control needs its own `Event::WindowDragQuery` arm** (`app.rs:2593-2640`). Without it the button sits inside the OS caption drag region, every press becomes a window drag, and the toggle is silently dead. This bit `[T]` once already.
- **`waml-editor` chrome bans inline `font_size:` / `FontMember` in DSL.** No task here needs a font role; do not add one. (A new role would mean `fonts.rs` + `script_gate.rs` + `fonts_overlay.rs` move together.)
- **A makepad `mod.X` namespace must be created by ONE object-literal assignment**, never field-by-field. No task here creates a namespace.
- **No new widget type is introduced.** `[I]` reuses the already-registered `IconButton` (registered in `app.rs`'s `script_mod` chain), so the "custom widget mounted as a DSL child is dead+invisible unless its `script_mod(vm)` registers first" trap does not apply — do not introduce one.
- **Do NOT reformat or re-sort untouched code.** Diffs stay surgical.
- Out of scope, do not implement: tab overflow against `[I]`; moving `windows_buttons`; per-tab dock memory; a first real caller for `open_right_dock`; replacing the shared `inspector` widget with a per-view panel.

## File map

| File | Role after this change |
| --- | --- |
| `crates/waml-editor/src/icon_button.rs` | Two independent light channels (`wash_and_ink`), plus its unit tests. Task 1. |
| `crates/waml-editor/src/doc_view.rs` | The seam: `DocView::right_dock()`, `BodyChrome.right_dock`, `ViewOutcome::open_right_dock`, `right_dock_open_requested`. Tasks 2 + 5. |
| `crates/waml-editor/src/class_diagram_view.rs`, `classifier_preview_view.rs`, `source_view.rs` | Each declares `right_dock() -> Some(Icon::InspectionPanel)`. Task 2. |
| `crates/waml-editor/src/inspector_panel.rs` | Binary `Flag`⇄`Pinned` flush column: no flag spine, no pin button, no peek timer, flat `field_bg`. Tasks 3 + 5. |
| `crates/waml-editor/src/dock.rs` | Pure state model; gains `DockEvent::Open`. Task 5. |
| `crates/waml-editor/src/app.rs` | Layout move (`inspector` into `right_slot`, `peek_layer` deleted), `[I]` DSL node + `INSPECTOR_BTN_W`, all `[I]` wiring, the outcome relay. Tasks 3 + 4 + 5. |
| `crates/waml-editor/src/doc_tabs.rs` | Top-rule right overshoot gains `INSPECTOR_BTN_W`. Task 4. |
| `crates/waml-editor/src/tree_panel.rs` | Comment-only: two stale references to the inspector's floating card/ring. Task 3. |

## Ordering, and the one degraded intermediate state

Tasks are ordered 1 → 5 and each is independently committable and gate-green. **Between Task 3 and Task 4 the inspector has no open affordance**: Task 3 deletes the flag spine and the pin button, and Task 4 is what adds the `[I]` toggle that replaces them. The panel defaults to `Flag` (collapsed) so an intermediate build shows no inspector at all. This is expected, is exactly the shape the tree migration had, and is resolved by Task 4 — do not "fix" it inside Task 3 by keeping the flag spine alive.

---

### Task 1 — `IconButton`: split the wash from the tint

**Intent.** `IconButton` computes one `lit` flag and uses it for both the 16% accent wash and the accent glyph tint, so a resting-active toggle is visually identical to a hovered one. Split it into two channels: the wash follows hover only; the tint follows hover OR active. Disabled (`dim`) still lights neither. The rule is extracted into a pure function so the gate can actually test it.

**Blast radius (visual, not compile).** Every `set_active` caller changes appearance: `tool_dock.rs:185` (selected tool), `view_bar.rs:249` (lit toggle), `app.rs:1636-1639` (transient burger glow), `app.rs:823-826` (`[T]`), and `inspector_panel.rs:1055` (`element_bar.pin_btn`, which Task 3 deletes). This is intentional and global — per the spec's Decisions, it is **not** an opt-in style enum. Nothing to change in those files.

**Files:**
- Modify: `crates/waml-editor/src/icon_button.rs` — module doc (lines 1-15), DSL wash comment (lines 32-43), `active` field doc (lines 108-115), `draw_walk` (lines 154-182)
- Test: `crates/waml-editor/src/icon_button.rs` (new `#[cfg(test)] mod tests` at end of file — the file has none today)

**Interfaces:**
- Consumes: nothing new.
- Produces: `fn wash_and_ink(hovered: bool, active: bool, dim: bool) -> (bool, bool)` — private to `icon_button.rs`; `.0` = paint the wash, `.1` = tint the glyph accent. No public API changes; `set_active` / `set_dim` / `set_icon` keep their signatures.

- [ ] **Step 1: Write the failing tests**

Append to the end of `crates/waml-editor/src/icon_button.rs` (after the closing `}` of `impl IconButtonRef`, currently line 273):

```rust
#[cfg(test)]
mod tests {
    use super::wash_and_ink;

    // The whole point of the split: an active-but-unhovered toggle reads as a
    // bare accent glyph, so "on" no longer looks identical to "hovered".
    #[test]
    fn resting_active_tints_the_glyph_without_the_wash() {
        assert_eq!(wash_and_ink(false, true, false), (false, true));
    }

    // An active button that is also hovered looks like any other hovered
    // button -- both channels lit.
    #[test]
    fn hover_lights_both_channels() {
        assert_eq!(wash_and_ink(true, false, false), (true, true));
        assert_eq!(wash_and_ink(true, true, false), (true, true));
    }

    #[test]
    fn idle_lights_neither_channel() {
        assert_eq!(wash_and_ink(false, false, false), (false, false));
    }

    // Disabled never lights, however it is hovered or flagged active.
    #[test]
    fn dim_never_lights_either_channel() {
        assert_eq!(wash_and_ink(true, true, true), (false, false));
        assert_eq!(wash_and_ink(false, true, true), (false, false));
        assert_eq!(wash_and_ink(true, false, true), (false, false));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-editor icon_button`
Expected: FAIL to compile — `cannot find function 'wash_and_ink' in this scope`.

- [ ] **Step 3: Add the pure helper**

Insert immediately above `impl Widget for IconButton {` (currently line 129):

```rust
/// The two independent light channels of an icon button, split so a resting
/// `active` toggle does not read as a hovered one:
/// `.0` = the 16% accent **wash** behind the glyph (hover only);
/// `.1` = the accent glyph **tint** (hover OR active).
/// A `dim` (disabled) button lights neither, however it is hovered or flagged.
/// Pure so the rule is unit-testable without a `Cx`.
fn wash_and_ink(hovered: bool, active: bool, dim: bool) -> (bool, bool) {
    if dim {
        return (false, false);
    }
    (hovered, hovered || active)
}
```

- [ ] **Step 4: Use it in `draw_walk`**

In `draw_walk` (lines 154-170), replace:

```rust
        // A dim button never lights, however it is hovered or flagged active.
        let lit = (self.hovered || self.active) && !self.dim;
        self.view
            .draw_bg
            .set_uniform(cx, live_id!(lit), &[if lit { 1.0 } else { 0.0 }]);
```

with:

```rust
        // Two channels (see `wash_and_ink`): `hot` paints the wash, `ink` tints
        // the glyph. A dim button never lights either.
        let (hot, ink) = wash_and_ink(self.hovered, self.active, self.dim);
        self.view
            .draw_bg
            .set_uniform(cx, live_id!(lit), &[if hot { 1.0 } else { 0.0 }]);
```

and in the tint pick just below, replace `let tint = if lit {` with `let tint = if ink {`. Leave the `else if self.dim` / `else` arms and the `live_id!(lit)` uniform NAME untouched — the DSL uniform keeps its name, only the value it is fed changes.

- [ ] **Step 5: Move the three prose statements of the old rule**

The file states the old "OR'd into `lit` + accent glyph tint" rule verbatim in three places. Update all three.

a) Module doc, lines 2-4. Replace:

```rust
//! rounded accent hover/active wash. The shared recipe already proven by the
//! tool dock: a hover (or an `active` flag) lights the wash and tints the glyph
//! `atlas.text` -> `atlas.accent`. The glyph is picked at runtime (`set_icon`),
```

with:

```rust
//! rounded accent hover wash. The shared recipe already proven by the tool
//! dock, split across two channels (`wash_and_ink`): a hover lights the wash,
//! and hover OR an `active` flag tints the glyph `atlas.text` ->
//! `atlas.accent`. A resting-active toggle is therefore a bare accent glyph
//! with no wash, so "on" never reads as "hovered". The glyph is picked at
//! runtime (`set_icon`),
```

b) DSL wash comment, lines 33-34. Replace:

```rust
        // Rounded accent wash behind the glyph, faded in by `lit` (hover ||
        // active). A centred 28px square, the SAME accent @16% the tool dock /
```

with:

```rust
        // Rounded accent wash behind the glyph, faded in by `lit` -- HOVER
        // only now; an `active` button tints its glyph and paints no wash (see
        // `wash_and_ink`). A centred 28px square, the SAME accent @16% the tool dock /
```

c) `active` field doc, lines 112-113. Replace:

```rust
    /// Persistent lit state (e.g. an active tool / a pinned panel). OR'd with
    /// `hovered` into the `lit` uniform + accent glyph tint.
```

with:

```rust
    /// Persistent lit state (e.g. an active tool / a pinned panel). Tints the
    /// glyph `atlas.accent` on its own; it does NOT paint the hover wash (see
    /// `wash_and_ink`), so resting-on and hovered are distinguishable states.
```

- [ ] **Step 6: Run the gate**

Run: `cargo test --workspace`
Expected: PASS, including the four new `icon_button::tests` cases.

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean (no `dead_code` — `wash_and_ink` has a caller in `draw_walk`).

- [ ] **Step 7: Commit**

```bash
git add crates/waml-editor/src/icon_button.rs
git commit -m "feat(icon-button): split the hover wash from the active glyph tint"
```

**Done means:** `wash_and_ink` exists, is used by `draw_walk` for both channels, the four tests pass, all three prose statements of the old rule are updated, and no other file changed.

---

### Task 2 — the `DocView` right-dock seam

**Intent.** Grow the existing per-view chrome seam by one declaration member and one upward-request member. The view says *whether* it has a right-hand dock and *which glyph* its toggle wears — nothing else. Open/closed state, slot width, button placement and lit state stay the app's. Nothing consumes either member in this task; both land wired and tested, exactly as `ops` and `open_preview` did before them.

**Files:**
- Modify: `crates/waml-editor/src/doc_view.rs:67-75` (imports), `:77-93` (`ViewOutcome`), `:188-190` (trait, after `wants_view_bar`), `:214-221` (`BodyChrome`), `:228-242` (`body_chrome`)
- Modify: `crates/waml-editor/src/class_diagram_view.rs:4-12` (imports) and `:529-535` (trait impl)
- Modify: `crates/waml-editor/src/classifier_preview_view.rs:4-10` (imports) and `:127-129` (trait impl)
- Modify: `crates/waml-editor/src/source_view.rs:8-13` (imports) and `:58-60` (trait impl)
- Test: `crates/waml-editor/src/doc_view.rs:264-401` (existing `mod tests`)

**Interfaces:**
- Consumes: `crate::icons::Icon` (a `Copy + Debug + PartialEq + Eq` fieldless enum, `icons.rs:4023-4025`); `Icon::InspectionPanel` already exists in the catalog at `icons.rs:4119`.
- Produces, for Tasks 4 and 5:
  - `DocView::right_dock(&self) -> Option<Icon>` — defaulted `None` on the trait.
  - `BodyChrome { pub tool_dock: bool, pub view_bar: bool, pub right_dock: Option<Icon> }` — `body_chrome(None)` reports `right_dock: None`.
  - `ViewOutcome::open_right_dock: bool` — defaults false.

- [ ] **Step 1: Write the failing tests**

In `crates/waml-editor/src/doc_view.rs`'s `mod tests`, extend the three existing cases and add one.

In `view_outcome_default_is_all_empty` (line 282), after `assert!(!o.statusbar_dirty);` add:

```rust
        assert!(!o.open_right_dock);
```

In `no_active_tab_hides_every_piece_of_body_chrome` (line 364), replace the asserted struct literal with:

```rust
        assert_eq!(
            body_chrome(None),
            BodyChrome {
                tool_dock: false,
                view_bar: false,
                right_dock: None,
            }
        );
```

In `body_chrome_follows_the_active_view` (line 378), add `right_dock: Some(Icon::InspectionPanel),` as the third field of all three expected `BodyChrome` literals, e.g.:

```rust
        assert_eq!(
            body_chrome(Some(&tab(TabKind::Diagram, TreeKind::Diagram))),
            BodyChrome {
                tool_dock: true,
                view_bar: true,
                right_dock: Some(Icon::InspectionPanel),
            }
        );
        assert_eq!(
            body_chrome(Some(&tab(TabKind::Classifier, TreeKind::Class))),
            BodyChrome {
                tool_dock: false,
                view_bar: false,
                right_dock: Some(Icon::InspectionPanel),
            }
        );
        assert_eq!(
            body_chrome(Some(&tab(TabKind::Source, TreeKind::Class))),
            BodyChrome {
                tool_dock: false,
                view_bar: false,
                right_dock: Some(Icon::InspectionPanel),
            }
        );
```

Then add a new case at the end of `mod tests` (before its closing `}` at line 401):

```rust
    #[test]
    fn every_open_tab_kind_declares_the_inspector_right_dock() {
        // All three concrete views drive the one shared `inspector` widget
        // today, so all three wear the same glyph. The seam earns its keep on
        // the `None` path (no open tab -> no toggle) and on views yet written.
        for (kind, node_kind) in [
            (TabKind::Diagram, TreeKind::Diagram),
            (TabKind::Classifier, TreeKind::Class),
            (TabKind::Source, TreeKind::Class),
        ] {
            assert_eq!(
                body_chrome(Some(&tab(kind, node_kind))).right_dock,
                Some(Icon::InspectionPanel),
                "a {kind:?} tab must declare the inspector dock"
            );
        }
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-editor doc_view`
Expected: FAIL to compile — `struct 'BodyChrome' has no field named 'right_dock'`, `no field 'open_right_dock' on type 'ViewOutcome'`, `cannot find value 'Icon' in this scope`.

- [ ] **Step 3: Import `Icon` in `doc_view.rs`**

In the crate-import block (lines 70-75), after `use crate::doc_tabs::{DocTab, TabKind};` add:

```rust
use crate::icons::Icon;
```

- [ ] **Step 4: Add `ViewOutcome::open_right_dock`**

In `ViewOutcome` (lines 78-93), after the `statusbar_dirty` field, add:

```rust
    /// Ask the shell to open the right-hand docked panel -- a view-side user
    /// action that needs the panel visible (select a node, hit a body control).
    /// Request-only: a view never asks for a collapse, so a user who closed the
    /// panel isn't fought by the next click. Ignored when the active view
    /// declares no right dock (`DocView::right_dock() == None`).
    ///
    /// Nothing sets this yet. Like `ops` and `open_preview` before it, it lands
    /// as a wired and tested channel whose first real caller comes later.
    pub open_right_dock: bool,
```

- [ ] **Step 5: Add `DocView::right_dock`**

In the `DocView` trait, immediately after `wants_view_bar` (which ends at line 190), add:

```rust
    /// The right-hand docked panel this view drives, and the glyph its caption
    /// toggle wears. `None` -> no right dock; the shell hides the toggle.
    ///
    /// The view declares *whether* and *which glyph*, and nothing else:
    /// open/closed state, slot width, button placement and lit state are all
    /// the app's. Shaped so a later view can name its own panel without any
    /// shell change.
    fn right_dock(&self) -> Option<Icon> {
        None
    }
```

- [ ] **Step 6: Add `BodyChrome.right_dock` and fill it in `body_chrome`**

`BodyChrome` (lines 216-221) becomes:

```rust
pub struct BodyChrome {
    /// The left tool dock (`tool_dock_wrap`).
    pub tool_dock: bool,
    /// The bottom-centre view bar (`view_bar_wrap`).
    pub view_bar: bool,
    /// The right-hand docked panel the active view drives, and the glyph its
    /// caption toggle wears (`None` = no dock, so the toggle is hidden).
    pub right_dock: Option<Icon>,
}
```

`body_chrome` (lines 228-242) becomes:

```rust
pub fn body_chrome(active: Option<&DocTab>) -> BodyChrome {
    match active {
        None => BodyChrome {
            tool_dock: false,
            view_bar: false,
            right_dock: None,
        },
        Some(tab) => {
            let view = make_view(tab);
            BodyChrome {
                tool_dock: view.wants_tooldock(),
                view_bar: view.wants_view_bar(),
                right_dock: view.right_dock(),
            }
        }
    }
}
```

- [ ] **Step 7: Declare the dock on all three concrete views**

`class_diagram_view.rs` — add `use crate::icons::Icon;` after `use crate::doc_view::{...};` (line 9), and insert after `wants_view_bar` (ends line 535, before `fn as_any_mut`):

```rust
    /// The shared `inspector` widget: this view feeds it the diagram's element
    /// picker, so its caption toggle wears the inspection-panel glyph.
    fn right_dock(&self) -> Option<Icon> {
        Some(Icon::InspectionPanel)
    }
```

`classifier_preview_view.rs` — add `use crate::icons::Icon;` after `use crate::doc_view::{...};` (line 8), and insert the same method after `wants_tooldock` (ends line 129), with the doc comment reading:

```rust
    /// The shared `inspector` widget: this view points it at the previewed
    /// classifier (picker hidden), so its caption toggle wears the
    /// inspection-panel glyph.
    fn right_dock(&self) -> Option<Icon> {
        Some(Icon::InspectionPanel)
    }
```

`source_view.rs` — add `use crate::icons::Icon;` after `use crate::doc_view::{...};` (line 12), and insert after `wants_tooldock` (ends line 60):

```rust
    /// The shared `inspector` widget: a source tab still points it at the
    /// subject classifier (picker hidden), so its caption toggle wears the
    /// inspection-panel glyph.
    fn right_dock(&self) -> Option<Icon> {
        Some(Icon::InspectionPanel)
    }
```

- [ ] **Step 8: Run the gate**

Run: `cargo test --workspace`
Expected: PASS, including `every_open_tab_kind_declares_the_inspector_right_dock` and the three amended cases.

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean. `open_right_dock` has no reader yet — that is fine because `doc_view.rs:11` carries `#![allow(dead_code)]`. **Do not remove that attribute.**

- [ ] **Step 9: Commit**

```bash
git add crates/waml-editor/src/doc_view.rs crates/waml-editor/src/class_diagram_view.rs crates/waml-editor/src/classifier_preview_view.rs crates/waml-editor/src/source_view.rs
git commit -m "feat(doc-view): declare a per-view right dock on the shell seam"
```

**Done means:** `body_chrome(None).right_dock` is `None`, all three tab kinds report `Some(Icon::InspectionPanel)`, `ViewOutcome::default().open_right_dock` is false, the gate is green, and no app-side code consumes either member yet.

---

### Task 3 — the inspector becomes a binary right column

**Intent.** Make the `Inspector` the right-hand mirror of `ProjectTree`: no flag spine, no pin button, no peek timer, no floating card ring or margin — a flush `Flag`⇄`Pinned` column mounted directly inside `right_slot`. `peek_layer` / `right_peek_wrap` then hold nothing and are deleted.

**Read `tree_panel.rs` first.** `ProjectTree::draw_walk` (`tree_panel.rs:570-633`), `apply_dock`/`toggle_dock`/`slot_width` (`tree_panel.rs:918-952`) and its flat `draw_bg` (`tree_panel.rs:66-85`) are the exact shapes to mirror. Do not invent a different one.

**Files:**
- Modify: `crates/waml-editor/src/inspector_panel.rs` — imports `:30-31`; DSL `draw_bg` `:63-82`, `element_bar` `:84-101`, `flag_btn` `:103-114`; fields `:372-383`; `FLAG_SQUARE` `:402-405`; `handle_event` `:519-586`; `draw_walk` Flag branch `:625-651` and expanded prologue `:652-668`; `sync_bar_buttons` `:1038-1056`; `apply_dock` `:1058-1071`; `arm_frame` `:1073-1076`; `slot_width` `:1086-1089`; module doc `:1-27`
- Modify: `crates/waml-editor/src/app.rs` — DSL `:265-273` (comment), `:416-417` (`right_slot`), `:419-442` (`peek_layer` block, deleted)
- Modify: `crates/waml-editor/src/tree_panel.rs` — comments only, `:19` and `:73-74`
- Test: `crates/waml-editor/src/inspector_panel.rs:1326-1466` (existing `mod tests`)

**Interfaces:**
- Consumes: `crate::dock::{next, slot_width, body_visible, DockEvent::Toggle, DockState}` (unchanged API).
- Produces, for Task 4: `Inspector::toggle_dock(&mut self, cx: &mut Cx)` — binary `Flag`⇄`Pinned`, the twin of `ProjectTree::toggle_dock` (`tree_panel.rs:936`). `Inspector::slot_width(&self) -> f64` keeps its signature and still returns 320 open / 0 closed.

- [ ] **Step 1: Write the failing tests**

Append to `inspector_panel.rs`'s `mod tests` (before its closing `}` at line 1466):

```rust
    // The caption `[I]` toggle is the panel's only affordance now, so the state
    // machine it drives must be strictly binary -- landing in `Peek` would
    // self-collapse the column out from under the user.
    #[test]
    fn the_caption_toggle_moves_the_column_between_flag_and_pinned() {
        use crate::dock::{next, DockEvent, DockState};
        assert_eq!(next(DockState::Flag, DockEvent::Toggle), DockState::Pinned);
        assert_eq!(next(DockState::Pinned, DockEvent::Toggle), DockState::Flag);
        assert_ne!(next(DockState::Flag, DockEvent::Toggle), DockState::Peek);
        assert_ne!(next(DockState::Pinned, DockEvent::Toggle), DockState::Peek);
    }

    #[test]
    fn the_inspector_column_reserves_320_open_and_nothing_closed() {
        use crate::dock::{slot_width, DockState};
        assert_eq!(slot_width(DockState::Pinned, INSPECTOR_W), 320.0);
        assert_eq!(slot_width(DockState::Flag, INSPECTOR_W), 0.0);
    }

    // `DockState::default()` is spelled `Flag` on the enum precisely because
    // the inspector depends on it (the tree seeds its own `Pinned` at the
    // field). The inspector still wants to start collapsed.
    #[test]
    fn the_inspector_starts_collapsed() {
        use crate::dock::DockState;
        assert_eq!(DockState::default(), DockState::Flag);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-editor inspector_panel`
Expected: FAIL to compile — `cannot find value 'INSPECTOR_W' in this scope`.

- [ ] **Step 3: Name the column width and use it in `slot_width`**

In `inspector_panel.rs`, next to the other geometry consts (after `BAR_H` at line 400), add:

```rust
/// Body width of the docked inspector column (px) -- what `right_slot` reserves
/// while `Pinned`, and therefore what the center shrinks by. The right-hand
/// twin of the tree's 280.
const INSPECTOR_W: f64 = 320.0;
```

and change `slot_width` (lines 1086-1089) to:

```rust
    /// The layout width the app must reserve in the right slot for this panel.
    pub fn slot_width(&self) -> f64 {
        crate::dock::slot_width(self.dock, INSPECTOR_W)
    }
```

Delete `FLAG_SQUARE` and its 4-line doc comment (lines 402-405) — the flag tab is gone.

- [ ] **Step 4: Delete the peek machinery from the widget state**

In the `Inspector` struct, replace the `dock` field's doc and delete the three peek fields (lines 371-383) so the block reads:

```rust
    /// Dock visual state, binary here: `Pinned` (a flush 320px column) or
    /// `Flag` (zero pixels, nothing drawn). The app reads `slot_width()` to
    /// drive the right reservation slot. Seeded from `DockState::default()`
    /// (`Flag`), so the panel starts collapsed.
    #[rust]
    dock: DockState,
```

Delete: `peek_timer: PeekTimer` (+ its 2-line doc), `dock_frame: NextFrame` (+ its doc), `dock_last_time: f64`.

Fix the import at line 30:

```rust
use crate::dock::{DockEvent, DockState};
```

- [ ] **Step 5: Delete the peek + flag/pin event handling**

In `handle_event`, delete outright:
- the whole `if let Event::MouseMove(e) = event { match self.dock { ... } }` block (lines 525-548);
- the whole `if let Some(ne) = self.dock_frame.is_event(event) { ... }` block (lines 549-564);
- the whole `if let Event::Actions(actions) = event { ... }` block including its `flag_btn` and `element_bar.pin_btn` branches and its 3-line lead comment (lines 565-586) — nothing else in this method reads `actions`.

**Keep** `let panel_rect = ...` / `let hit_off = ...` (lines 519-520) and the `hits_with_capture_overload` match below them. The panel is a laid-out column now, so the offset is normally zero, but the translation stays correct either way. Amend the tail of that comment block (the sentence beginning "This panel lives in a right-aligned parent") to:

```rust
        // ... The panel used to live in a right-aligned parent; it is a
        // laid-out `right_slot` column now, so this offset is normally zero --
        // the translation is kept because it stays correct either way and the
        // rects are still captured mid-draw.
```

- [ ] **Step 6: Rewrite the `Flag` draw branch as a zero walk — LOOP IT**

Replace the whole `if !crate::dock::body_visible(self.dock) { ... }` branch in `draw_walk` (lines 626-651) with the tree's shape (`tree_panel.rs:571-597`):

```rust
        // Flag rest state: the panel is gone, not shrunk -- there is no flag
        // spine any more, the caption bar's `[I]` toggle is the only
        // affordance. Hide every child and draw into a zero-size, margin-free
        // walk.
        //
        // Drawing a zero walk rather than returning early is deliberate: it
        // costs one invisible 0x0 quad but leaves `self.view.area()` freshly
        // stamped as an empty rect, so `handle_event`'s hit tests can't keep
        // matching the last expanded rect and swallow clicks meant for the
        // canvas underneath.
        if !crate::dock::body_visible(self.dock) {
            let mut fw = walk;
            fw.width = Size::Fixed(0.0);
            fw.height = Size::Fixed(0.0);
            fw.margin = Inset::default();
            self.view.view(cx, ids!(element_bar)).set_visible(cx, false);
            self.view.widget(cx, ids!(body)).set_visible(cx, false);
            // `View::draw_walk` is a multi-step `DrawStep` machine: it opens the
            // view's turtle on the first call and only closes it once the loop
            // runs it to `done`. Calling it once and dropping the result leaves
            // the turtle begun-but-never-ended, unbalancing the whole window's
            // turtle stack -- every later draw (the sibling panel and the window
            // caption/frame) then silently aborts. Drive it to completion,
            // exactly like the expanded path below.
            while self.view.draw_walk(cx, scope, fw).step().is_some() {}
            return DrawStep::done();
        }
```

**This loop is load-bearing.** A one-shot `let _ = self.view.draw_walk(cx, scope, fw);` blanks the caption and both side panels, silently, with the whole automated gate blind to it.

Then delete the now-orphaned line `self.view.widget(cx, ids!(flag_btn)).set_visible(cx, false);` (line 652) from the expanded path.

- [ ] **Step 7: Delete the flag spine and pin button from the DSL**

In `script_mod!`:
- Delete the whole `flag_btn := IconButton { ... }` node and its 5-line lead comment (lines 103-114).
- In `element_bar`, delete `pin_btn := IconButton { visible: false }` (line 100) and rewrite the block's lead comment (lines 84-92) to drop the fold-caret/pin prose:

```rust
        // The element-picker bar. Hosts the real `SelectBox` child widget
        // (badge + selected label + caret, its own click handling and open
        // request). The fold-caret + pin `IconButton`s that used to sit at its
        // right are gone -- the caption bar's `[I]` toggle owns collapse/expand
        // now, exactly as `[T]` does for the tree. Hidden (`visible: false`)
        // until a diagram feeds the picker. The dropped list is the shared
        // `SelectFlyout` surface (routed through `PopupRoot`), so each
        // association row still carries the real `IconSpline` SDF.
```

- Delete `use crate::icon_button::IconButtonWidgetRefExt;` (line 31) — it is now unused and would red the gate.

- [ ] **Step 8: Replace the floating card's frame ring with a flat fill**

Replace the `draw_bg +: { ... }` block and its lead comment (lines 63-82) with the tree's flat fill (`tree_panel.rs:66-85`), reworded:

```rust
        // Flat, opaque `field_bg` -- no ring, no corner radius, no divider. The
        // panel used to inline the `frame.rs` / `AccentFrame` material (a 1.5px
        // accent stroke round the fill) because it floated as a HUD card over
        // the canvas; it is a flush column now, butted to the window's right
        // edge and to the caption band above it, so the ring had nothing left
        // to separate and only cut the two apart. Chrome mass versus canvas
        // ground carries the edge instead -- the same call `tree_panel.rs`
        // made, and the two are now deliberately symmetric.
        //
        // A flat fill rather than an SDF one: nothing here needs coverage or
        // antialiasing now that the ring and the radius are gone. The body is
        // still inlined onto the `DrawQuad` because this widget derefs `View`,
        // whose `draw_bg` is a `DrawQuad` a `DrawColor` object can't swap onto --
        // so this repeats `mod.draw.DrawColor`'s own pixel fn verbatim, including
        // its premultiply (the render pass blends premultiplied alpha).
        draw_bg +: {
            color: atlas.field_bg
            pixel: fn() {
                return vec4(self.color.rgb * self.color.a, self.color.a)
            }
        }
```

Keep the expanded path's `walk.margin.right/top/bottom = 0.0` stripping (lines 664-668) — `tree_panel.rs:615-618` keeps the mirror-image stripping even though its mount site carries no margin either; it is a cheap belt-and-braces against a future mount-site margin. Amend its comment to say the mount site no longer carries a float margin.

- [ ] **Step 9: Delete `sync_bar_buttons` and narrow `apply_dock`; add `toggle_dock`**

Delete the whole `fn sync_bar_buttons` (lines 1038-1056) and `fn arm_frame` (lines 1073-1076). Delete its four remaining call sites: `set_subject` (line 1034), `apply_dock` (line 1069), `set_diagram_elements` (line 1172), `set_picker_visible` (line 1181).

`apply_dock` becomes the exact twin of `ProjectTree::apply_dock`:

```rust
    /// Apply a dock event: transition, then redraw. No-op if the state is
    /// unchanged.
    ///
    /// The panel has no controls of its own now; every caller comes in through
    /// [`Inspector::toggle_dock`] below.
    fn apply_dock(&mut self, cx: &mut Cx, ev: DockEvent) {
        let next = crate::dock::next(self.dock, ev);
        if next == self.dock {
            return;
        }
        self.dock = next;
        self.view.redraw(cx);
    }

    /// Expand <-> collapse, driven by the caption bar's `[I]` toggle. Binary by
    /// construction: `DockEvent::Toggle` never routes through `Peek`, so the
    /// column is either a full `INSPECTOR_W` or zero pixels.
    pub fn toggle_dock(&mut self, cx: &mut Cx) {
        self.apply_dock(cx, DockEvent::Toggle);
    }
```

Leave `dock_state()` (lines 1078-1084) and its `#[allow(dead_code)]` exactly as they are.

Update the module doc (lines 1-27): in the "Top bar (`element_bar`)" paragraph, drop the "plus fold-caret + pin `IconButton`s that drive the panel's `dock: DockState` (Flag/Peek/Pinned; see `dock.rs`) -- pin docks the column as a flush sidebar, the caret always collapses to Flag" clause and replace it with: "The panel's dock state is binary -- `Pinned` (a flush `INSPECTOR_W` column) or `Flag` (zero pixels, nothing drawn) -- and the caption bar's `[I]` toggle is its sole affordance; it never enters `Peek`, so it carries no flag spine and no auto-collapse timer."

- [ ] **Step 10: Move the inspector into `right_slot` and delete `peek_layer`**

In `app.rs`, replace the `right_slot` line (line 417):

```rust
                            right_slot := View{ width: 0.0, height: Fill }
```

with:

```rust
                            // Right (Inspector) column, the mirror of
                            // `left_slot`: not a bare spacer any more, but a
                            // real layout child, since the inspector no longer
                            // peeks. Its flush top (the body's y=66), flush
                            // right and full height all fall out of the layout
                            // for free, and the shared `field_bg` merges it
                            // with the caption band into one chrome mass.
                            //
                            // Width is set at runtime by `sync_dock_slots` (320
                            // when Pinned, 0 when collapsed), which is what
                            // shrinks the `Fill` center. Starts at 0 so the
                            // first frame can't flash a column before the slot
                            // sync runs -- and the inspector rests collapsed.
                            right_slot := View{
                                width: 0.0
                                height: Fill
                                inspector := Inspector{
                                    width: Fill
                                    height: Fill
                                }
                            }
```

Then delete the entire `peek_layer := View{ ... }` block including `right_peek_wrap` and the old `inspector := Inspector{ width: 320.0 ... margin: Inset{right: 28.0, top: 12.0, bottom: 12.0} }` mount, plus its 9-line lead comment (lines 419-442).

`dock_body` keeps its `flow: Overlay` and now wraps `dock_row` alone; update its lead comment (lines 265-273) to:

```rust
                    // Body: a docked split. `dock_row` is flow:Right so a
                    // pinned slot shrinks the Fill `center_stack` automatically
                    // (no margin math). Both side panels are real layout
                    // children of their slots now -- neither peeks -- so the
                    // old `peek_layer` overlay sibling is gone and `dock_body`
                    // wraps `dock_row` alone. Both slot widths are driven from
                    // each panel's DockState (`sync_dock_slots`).
```

- [ ] **Step 11: Fix the two stale cross-references in `tree_panel.rs`**

`tree_panel.rs:19` — replace "Unlike the inspector it never enters `Peek`," with "Like the inspector it never enters `Peek`,".

`tree_panel.rs:73-74` — replace:

```rust
        // instead. The inspector still floats and still keeps its ring -- the
        // asymmetry is deliberate, so do NOT sync this shader back to `frame.rs`.
```

with:

```rust
        // instead. The inspector's right column now carries the identical flat
        // fill -- the two are deliberately symmetric, so keep them in step with
        // each other and do NOT sync either back to `frame.rs`.
```

- [ ] **Step 12: Run the gate**

Run: `cargo test --workspace`
Expected: PASS, including the three new `inspector_panel::tests` cases.

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean. Watch specifically for: unused `use crate::icon_button::IconButtonWidgetRefExt` (must be deleted in Step 7), unused `DockEdge`/`PeekTimer` imports (Step 4), and unused `FLAG_SQUARE` (Step 3). `crate::dock::{PeekTimer, peek_hover_span, DockEdge, header_controls_visible}` now have no callers anywhere — that is fine and intentional: `dock.rs:10` carries `#![allow(dead_code)]` and its own tests still exercise them. **Do not delete anything from `dock.rs` in this task.**

- [ ] **Step 13: Commit**

```bash
git add crates/waml-editor/src/inspector_panel.rs crates/waml-editor/src/app.rs crates/waml-editor/src/tree_panel.rs
git commit -m "feat(inspector): make the panel a flush binary right column"
```

**Done means:** the inspector has no `flag_btn`, no `pin_btn`, no `PeekTimer`/`NextFrame`, no frame ring and no float margin; its `Flag` branch draws a looped zero walk; `toggle_dock` exists; the widget is mounted `Fill`/`Fill` inside `right_slot`; `peek_layer`/`right_peek_wrap` are gone; the gate is green. **Expected regression until Task 4: the inspector cannot be opened at all** (no affordance exists yet) — see "Ordering, and the one degraded intermediate state" above.

---

### Task 4 — the caption `[I]` toggle

**Intent.** Add the right-hand twin of `[T]`: an `IconButton` anchored as the LAST child of `tab_row`, whose visibility and glyph come from `BodyChrome.right_dock` (the active view declares it), whose click toggles the inspector column, whose lit state comes from the same `slot_width()` the layout uses, and which has its own `WindowDragQuery` arm.

**Do not move `windows_buttons`.** `tab_row` ends 138px (`WINDOW_BUTTONS_W`) inboard of the window's right edge, so `[I]` does not line up with the column it toggles and a bar-coloured void sits to its right. That cosmetic is **accepted** per the spec's Decisions. Likewise, tab overflow against `[I]` is out of scope.

**Files:**
- Modify: `crates/waml-editor/src/app.rs` — DSL `tab_row` `:159-202` (new last child after `doc_tabs`, `:198-201`); `TREE_BTN_W` `:483` (new sibling const); `sync_dock_slots` `:830-842`; `sync_active_tab` `:601-604`; `open_dir` chrome push `:1146-1149`; `show_start_screen` `:1249-1250`; tree-toggle click block `:1642-1660`; `WindowDragQuery` `:2616-2639`
- Modify: `crates/waml-editor/src/doc_tabs.rs:403-410` (comment) and `:676` (rule right overshoot)

**Interfaces:**
- Consumes: `BodyChrome.right_dock` (Task 2), `Inspector::toggle_dock` + `Inspector::slot_width` (Task 3), `IconButton::{set_icon, set_active, clicked, rect}` (`icon_button.rs:187-234`).
- Produces: `pub(crate) const INSPECTOR_BTN_W: f64` in `app.rs` (read by `doc_tabs.rs`); `App::sync_right_dock_btn(&mut self, cx: &mut Cx, glyph: Option<crate::icons::Icon>)`.

**Testability note (state it, don't fake it).** Nothing in this task is unit-testable: it is DSL layout plus `Cx`-bound widget wiring. The gate proves only that it compiles and that every existing test still passes. A tautological assertion like `INSPECTOR_BTN_W == 32.0` is **not** worth adding. This task's real verification is the interactive sign-off listed under "Verification" below.

- [ ] **Step 1: Add the `[I]` DSL node**

In `app.rs`, inside `tab_row`, immediately after the `doc_tabs := DocTabs{ ... }` block closes (line 201) and before `tab_row`'s own closing brace (line 202), add:

```rust
                            // The right-hand twin of `[T]`: the ACTIVE VIEW's
                            // right-dock toggle. LAST child, so it is anchored
                            // hard against the row's right edge and never
                            // moves -- the `Fill` tab strip absorbs every bit
                            // of slack between the two, so opening tabs or
                            // expanding the tree column slides only the cards.
                            // The same 30px box / 18px glyph as `menu_btn` and
                            // `tree_btn`, so all three caption glyphs read as
                            // one set, with `[T]`'s 2px inset mirrored to the
                            // right edge.
                            //
                            // Visibility AND glyph come from
                            // `BodyChrome.right_dock` (see
                            // `sync_right_dock_btn`), NOT from
                            // `show_editor`/`show_start_screen` the way
                            // `tree_btn` is: the button exists because the
                            // active view says it does. Counted into
                            // `INSPECTOR_BTN_W`, which `DocTabs` adds back to
                            // its top rule's right overshoot.
                            //
                            // Known cosmetic (accepted, see the design spec):
                            // `tab_row` ends `WINDOW_BUTTONS_W` (138px) inboard
                            // of the window's right edge because
                            // `windows_buttons` follows `caption_col` in the
                            // caption's `flow: Right`, so `[I]` does not line
                            // up with the column it toggles.
                            inspector_btn := IconButton{ width: 30.0 height: 30.0 icon_size: 18.0 margin: Inset{right: 2.0, top: 1.0} visible: false }
```

- [ ] **Step 2: Add `INSPECTOR_BTN_W`**

In `app.rs`, immediately after `const TREE_BTN_W: f64 = 32.0;` (line 483), add:

```rust
/// Footprint of the caption's right-dock toggle `[I]`: the `inspector_btn` DSL
/// `width` (30, the burger's size) plus its 2px right margin. The right-hand
/// twin of `TREE_BTN_W`. `pub(crate)` because `DocTabs` has the other consumer:
/// the tab strip's turtle is now shorter by exactly this, so its top rule has to
/// overshoot by this much more to still reach the window's right edge.
pub(crate) const INSPECTOR_BTN_W: f64 = 32.0;
```

- [ ] **Step 3: Drive visibility + glyph from `BodyChrome`**

In `app.rs`, add a method next to `sync_dock_slots` (insert after `sync_dock_slots` ends, line 843):

```rust
    /// Drive the caption's `[I]` toggle from the active view's declared right
    /// dock: visible and wearing the view's glyph exactly when the view has
    /// one, hidden otherwise -- including the no-active-tab case, where
    /// `body_chrome(None)` reports `None`. The view declares *whether* and
    /// *which glyph*; open/closed state, slot width and lit state stay the
    /// app's (see `sync_dock_slots`).
    fn sync_right_dock_btn(&mut self, cx: &mut Cx, glyph: Option<crate::icons::Icon>) {
        let btn = self.ui.widget(cx, ids!(inspector_btn));
        btn.set_visible(cx, glyph.is_some());
        if let Some(icon) = glyph {
            btn.as_icon_button().set_icon(cx, icon);
        }
    }
```

Call it from the two places that already compute `BodyChrome`:

- `sync_active_tab`, after line 604 (`body.set_view_bar_visible(cx, chrome.view_bar);`), add:

```rust
        self.sync_right_dock_btn(cx, chrome.right_dock);
```

- `open_dir`, after line 1149 (same call), add the identical line.

And in `show_start_screen`, after the `tree_btn` hide (line 1250), add:

```rust
        // No open model means no active tab, and `body_chrome(None)` declares
        // no right dock -- push that through the same seam rather than special-
        // casing the button, so the start screen can't strand a stale `[I]`
        // from the model that was just closed. (`show_start_screen` does not
        // run `sync_active_tab`, which is where the push otherwise happens.)
        self.sync_right_dock_btn(cx, crate::doc_view::body_chrome(None).right_dock);
```

- [ ] **Step 4: Wire the click**

In `handle_actions`, immediately after the tree-toggle block closes (line 1660), add:

```rust
        // Caption right-dock toggle `[I]`: the twin of `[T]`, and the active
        // view's only affordance for its right-hand panel now that the flag
        // spine and the pin button are gone. Same binary `DockEvent::Toggle`,
        // so one glyph covers both directions; `sync_dock_slots` picks the new
        // width up on this same event pass and relights the button.
        if self
            .ui
            .widget(cx, ids!(inspector_btn))
            .as_icon_button()
            .clicked(actions)
        {
            if let Some(mut panel) = self
                .ui
                .widget(cx, ids!(inspector))
                .borrow_mut::<crate::inspector_panel::Inspector>()
            {
                panel.toggle_dock(cx);
            }
        }
```

- [ ] **Step 5: Light the button from the layout's own number**

In `sync_dock_slots`, inside the right-hand change-guarded block (lines 836-842), after the `right_slot` width write and before `cx.redraw_all();`, add the mirror of the tree's push at lines 823-826:

```rust
            // `[I]` is lit exactly when the column occupies pixels -- the same
            // source of truth as the layout, so the glyph can't disagree with
            // the pixels.
            self.ui
                .widget(cx, ids!(inspector_btn))
                .as_icon_button()
                .set_active(cx, rw > 0.5);
```

(The guard means no push happens on the very first sync, when `rw` is already 0 — correct: `IconButton::active` defaults to `false`.)

- [ ] **Step 6: Add the fourth `WindowDragQuery` arm — REQUIRED, or the button is dead**

In the `if let Event::WindowDragQuery(dq) = event` block, after the `over_tree_btn` binding (ends line 2624), add:

```rust
            // Same for the tab row's right-dock toggle: it sits in the caption
            // drag region, so without this every press becomes a window drag
            // and the toggle is silently dead.
            let over_inspector_btn = self
                .ui
                .widget(cx, ids!(inspector_btn))
                .as_icon_button()
                .rect()
                .contains(dq.abs);
```

and extend the condition at line 2637:

```rust
            if over_tab || over_logo || over_btn || over_tree_btn || over_inspector_btn || menu_open {
```

- [ ] **Step 7: Let the tab strip's top rule reach the window edge again**

`doc_tabs`' turtle is now shorter by `INSPECTOR_BTN_W`. In `doc_tabs.rs`, change line 676:

```rust
        let x_end = (rect.pos.x + rect.size.x + WINDOW_BUTTONS_W).round();
```

to:

```rust
        let x_end =
            (rect.pos.x + rect.size.x + crate::app::INSPECTOR_BTN_W + WINDOW_BUTTONS_W).round();
```

and amend the far-end paragraph of the lead comment (lines 666-669) to note the extra addend:

```rust
        // At the far end the line overshoots the tab band by the caption's
        // right-dock toggle (`INSPECTOR_BTN_W`, which now trails the strip in
        // `tab_row`) plus the window-button gap, so it still reaches the
        // window's right edge, then dissolves over the last `EDGE_FADE` px --
        // faked as stacked 1px segments of falling alpha since a crisp plain
        // quad carries one flat colour (see `EDGE_FADE`).
```

Also extend `WINDOW_BUTTONS_W`'s own doc (lines 403-410) with one sentence: "Since the caption's `[I]` toggle now trails the strip inside `tab_row`, the rule adds `crate::app::INSPECTOR_BTN_W` on top of this reserve."

- [ ] **Step 8: Run the gate**

Run: `cargo test --workspace`
Expected: PASS (no new tests; every existing test still green).

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean. `INSPECTOR_BTN_W` has a caller (`doc_tabs.rs`) and `sync_right_dock_btn` has three, so no `dead_code`.

- [ ] **Step 9: Commit**

```bash
git add crates/waml-editor/src/app.rs crates/waml-editor/src/doc_tabs.rs
git commit -m "feat(caption): add the view-declared [I] right-dock toggle"
```

**Done means:** `inspector_btn` is the last child of `tab_row`; its visibility+glyph come only from `BodyChrome.right_dock`; clicking it calls `Inspector::toggle_dock`; `sync_dock_slots` lights it from `slot_width() > 0.5`; it has its own `WindowDragQuery` arm; the top rule's right overshoot includes `INSPECTOR_BTN_W`; the gate is green. Interactive sign-off is still owed (see Verification).

---

### Task 5 — apply `ViewOutcome::open_right_dock`

**Intent.** Close the request-to-open half of the seam: when a view sets `open_right_dock` **and** the active view declares a right dock, the shell drives the inspector to `Pinned`. Idempotent — an already-open panel is a no-op, so there is no redraw churn — and request-only, so a user who collapsed the panel is never fought by the next click. Because it rides `ViewOutcome`, `handle`, `on_popup_result` and `on_popup_armed` all get it for free.

**Nothing sets the flag in this change.** A first real caller is explicitly out of scope. This task lands the wiring and its tests.

**Files:**
- Modify: `crates/waml-editor/src/dock.rs:28-40` (`DockEvent`), `:45-61` (`next`)
- Modify: `crates/waml-editor/src/inspector_panel.rs` (new `open_dock` beside `toggle_dock`, added in Task 3)
- Modify: `crates/waml-editor/src/doc_view.rs` (new `right_dock_open_requested` beside `body_chrome`, ~line 242)
- Modify: `crates/waml-editor/src/app.rs:2284-2427` (`relay_outcome`)
- Test: `crates/waml-editor/src/dock.rs` `mod tests`; `crates/waml-editor/src/doc_view.rs` `mod tests`

**Interfaces:**
- Consumes: `ViewOutcome::open_right_dock` + `body_chrome` (Task 2), `Inspector::apply_dock` (Task 3).
- Produces:
  - `DockEvent::Open` — drives any state to `Pinned`, never collapses.
  - `Inspector::open_dock(&mut self, cx: &mut Cx)`.
  - `pub fn right_dock_open_requested(outcome: &ViewOutcome, active: Option<&DocTab>) -> bool` in `doc_view.rs`.

- [ ] **Step 1: Write the failing tests**

In `dock.rs`'s `mod tests`, after `toggle_skips_peek_in_both_directions` (ends line 195), add:

```rust
    #[test]
    fn open_is_idempotent_and_never_collapses() {
        use DockEvent::*;
        use DockState::*;
        // Request-only: a view can ask for its panel, never for its collapse,
        // so a user who closed it isn't fought by the next click. Every state
        // lands on Pinned, including Pinned itself (a no-op, so no redraw).
        assert_eq!(next(Flag, Open), Pinned);
        assert_eq!(next(Peek, Open), Pinned);
        assert_eq!(next(Pinned, Open), Pinned);
    }
```

In `doc_view.rs`'s `mod tests`, after `every_open_tab_kind_declares_the_inspector_right_dock`, add:

```rust
    #[test]
    fn an_open_request_needs_both_the_flag_and_a_declared_dock() {
        let asked = ViewOutcome {
            open_right_dock: true,
            ..Default::default()
        };
        let quiet = ViewOutcome::default();
        let diagram = tab(TabKind::Diagram, TreeKind::Diagram);
        assert!(right_dock_open_requested(&asked, Some(&diagram)));
        // No flag: never opens. This is the common case -- nothing sets it yet.
        assert!(!right_dock_open_requested(&quiet, Some(&diagram)));
        // No active tab: `body_chrome(None).right_dock` is `None`, so the
        // request is ignored rather than opening a panel with no view behind it.
        assert!(!right_dock_open_requested(&asked, None));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-editor dock::tests` then `cargo test -p waml-editor doc_view::tests`
Expected: both FAIL to compile — `no variant named 'Open' found for enum 'DockEvent'`, `cannot find function 'right_dock_open_requested'`.

- [ ] **Step 3: Add `DockEvent::Open` to the pure state model**

In `dock.rs`, add to `DockEvent` (after the `Toggle` variant, line 39):

```rust
    /// A view asked the shell to open its right-hand docked panel
    /// (`ViewOutcome::open_right_dock`). Request-only and idempotent: it drives
    /// ANY state to `Pinned` and never collapses, so a user who closed the
    /// panel isn't fought by the next click.
    Open,
```

In `next`, add one arm immediately **before** the `(s, _) => s` catch-all (line 59):

```rust
        (_, Open) => Pinned,
```

(Every arm above it names a specific event, so nothing earlier can shadow it.)

While here, fix the two stale "the tree is `Toggle`'s only sender" comments now that `[I]` sends it too:
- lines 36-39 (`Toggle`'s doc): change "The caption bar's tree toggle." to "A caption bar dock toggle (`[T]` for the tree, `[I]` for the right dock)."
- lines 55-58 (the `(Peek, Toggle)` arm comment): change "The tree, `Toggle`'s only sender, never enters Peek" to "Neither `Toggle` sender ever enters Peek".

- [ ] **Step 4: Add `Inspector::open_dock`**

In `inspector_panel.rs`, immediately after `toggle_dock` (added in Task 3), add:

```rust
    /// Open the panel, idempotently -- the shell's relay for a view-side
    /// request (`ViewOutcome::open_right_dock`). Never collapses: see
    /// `DockEvent::Open`. A no-op when already open, so there is no redraw
    /// churn on a repeated request.
    pub fn open_dock(&mut self, cx: &mut Cx) {
        self.apply_dock(cx, DockEvent::Open);
    }
```

- [ ] **Step 5: Add the pure gating helper**

In `doc_view.rs`, immediately after `body_chrome` (ends line 242), add:

```rust
/// Whether a returned `ViewOutcome` actually asks for the right-hand dock to
/// open: the flag is set AND the active tab's view declares a right dock at
/// all. Pure, so the gating rule is testable without a `Cx`; the shell calls it
/// once at the top of `relay_outcome`, before the owned outcome's fields are
/// moved out beneath it.
pub fn right_dock_open_requested(outcome: &ViewOutcome, active: Option<&DocTab>) -> bool {
    outcome.open_right_dock && body_chrome(active).right_dock.is_some()
}
```

- [ ] **Step 6: Apply it in the outcome relay**

In `app.rs`'s `relay_outcome`, immediately after `let mut consumed = false;` (line 2290), add:

```rust
        // Read the right-dock request BEFORE the owned `outcome`'s fields are
        // moved out below (`popup`, `promote_subject`, `open_preview` all move).
        let open_right_dock = crate::doc_view::right_dock_open_requested(&outcome, Some(active));
```

and immediately before the `if outcome.statusbar_dirty {` block (line 2422), add:

```rust
        // Request-only, and idempotent: the shell opens the panel and never
        // closes it, so a user who collapsed it isn't fought by the next click.
        // Nothing sets the flag yet -- this is the wired channel, not a caller.
        if open_right_dock {
            if let Some(mut panel) = self
                .ui
                .widget(cx, ids!(inspector))
                .borrow_mut::<crate::inspector_panel::Inspector>()
            {
                panel.open_dock(cx);
            }
        }
```

Do **not** set `consumed = true` here: an open request is a side effect on shell chrome, not a claim on the event, and every other chrome-ish branch (`statusbar_dirty`) leaves `consumed` alone too.

- [ ] **Step 7: Run the gate**

Run: `cargo test --workspace`
Expected: PASS, including `open_is_idempotent_and_never_collapses` and `an_open_request_needs_both_the_flag_and_a_declared_dock`.

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean. `open_dock` has a caller (`relay_outcome`); `right_dock_open_requested` has a caller; `DockEvent::Open` is constructed in `open_dock`.

- [ ] **Step 8: Commit**

```bash
git add crates/waml-editor/src/dock.rs crates/waml-editor/src/inspector_panel.rs crates/waml-editor/src/doc_view.rs crates/waml-editor/src/app.rs
git commit -m "feat(shell): relay a view's open-right-dock request to the inspector"
```

**Done means:** `DockEvent::Open` drives every state to `Pinned` and is unit-tested; `Inspector::open_dock` exists; `relay_outcome` opens the inspector exactly when the flag is set and the active view declares a dock; both new tests pass; the gate is green; no view sets the flag.

---

## Verification

**Per task:** `cargo test --workspace` green, and `cargo clippy --workspace --all-targets -- -D warnings` clean. The workflow gate also runs `pnpm -r test && pnpm lint && pnpm build`; no TypeScript changes here, so those must simply stay green.

**The automated gate is BLIND to the failure class this change is most exposed to.** The turtle-imbalance bug (a `Flag` draw branch that does not loop to completion) blanks the window caption and both side panels, and produces clean stderr and a fully green test run. It shipped once already on the tree side (fixed in e62ad58) and was caught only by launching the app and looking at it. No test added by this plan can catch it. The same is true for every layout and lit-state item below.

**Interactive sign-off owed by the human** (from the spec's Testing section — these are NOT automatable here, and no task may claim them as done):

1. `[I]` sits pinned at the tab row's right edge, and does not move when tabs are opened/closed or when the tree column expands/collapses.
2. Clicking `[I]` toggles the inspector column; the column is flush (no ring, no float margin, no window-bg gutter), full height, and **shrinks the canvas** rather than overlapping it.
3. The `IconButton` active-state read across all consumers — the tool dock's five-glyph column especially, plus `view_bar`, the burger glow, `[T]` and `[I]`. Per the spec's Decisions: if a selected tool now reads too weakly, the fix is a dock-side selection marker, **not** a second button mode.
4. The caption and **both** side panels still draw after the `peek_layer` deletion — i.e. the turtle stack is balanced. Check at both dock states, and with the tree open and closed.
5. `[I]` presses register as clicks, not window drags (proves the `WindowDragQuery` arm).
6. The tab strip's top rule still reaches the window's right edge and fades there (proves the `INSPECTOR_BTN_W` overshoot).
7. `[I]` is hidden on the start screen and when every tab is closed, and reappears when a tab opens.

Launch for sign-off with the worktree's own `scripts/run-native.ps1` (it builds the checkout the script lives in, not the cwd), and screenshot **by pid** — `pwsh -File scripts/capture-window.ps1 -Out shot.png` grabs a window by name and will otherwise capture the user's own open editor.

## Notes on conservative readings

Recorded so an implementer does not silently widen scope. None of these is an invitation to redesign.

- **`dock_body` is kept.** The spec says the `peek_layer` deletion "collaps[es] `dock_body`'s `flow: Overlay` down to `dock_row` alone". The conservative reading applied here is that `dock_body` keeps its wrapper `View` (now with a single child) and only its comment changes. Deleting the wrapper outright would re-nest `dock_row` under `main_column` for no functional gain; that is a trivial follow-up, deliberately left out to keep the diff surgical.
- **Nothing is removed from `dock.rs`'s `Peek` machinery.** After Task 3, `PeekTimer`, `peek_hover_span`, `DockEdge`, `header_controls_visible` and `DockState::Peek` have no production caller. The spec's Unit 3 deletion list is about the *inspector's use* of them; deleting the types would also mean deleting `DockState::Peek` from the transition table and half of `dock.rs`'s tests, a far larger blast radius than the spec asks for. `dock.rs:10`'s `#![allow(dead_code)]` keeps the gate green. Leave them.
- **The spec's Problem section mentions the inspector having "a scrim".** It does not — `grep` finds no scrim in `inspector_panel.rs`; the only scrims in the crate belong to the style-guide overlays. There is nothing to delete for that clause.
- **`hit_off` in `Inspector::handle_event` is kept.** With the panel now in an unaligned layout chain (`dock_row` → `right_slot` → panel), the pre/post-alignment offset should be zero, so the translation is a no-op — but it is still correct, and removing it risks reintroducing the aligned-parent hit-rect bug if any ancestor ever gains an `align`. Only its comment changes.
- **`INSPECTOR_BTN_W` lives in `app.rs`, not `doc_tabs.rs`.** The spec names it "the right-hand twin of `TREE_BTN_W`", which lives in `app.rs`; keeping the two caption-button footprints side by side is the reading taken, at the cost of one `pub(crate)` and a `crate::app::` path in `doc_tabs.rs`. (Defining it next to `WINDOW_BUTTONS_W` in `doc_tabs.rs` was the alternative and would also satisfy the spec.)
- **A hidden `IconButton` retains its last-drawn `rect()`,** so on the start screen `[I]`'s stale rect is still client-ized by the `WindowDragQuery` arm. This is pre-existing behaviour shared with `tree_btn` (`app.rs:2619-2624`) and is not addressed here.
