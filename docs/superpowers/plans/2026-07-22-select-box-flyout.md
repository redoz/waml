# SelectBox + SelectFlyout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the inspector element-picker's `MenuPopup` routing with a reusable `SelectBox` combo control whose open list is a new third `PopupRoot` surface (`SelectFlyout`, `ActiveKind::Select`), so the card is at least as wide as the control, marks the current selection, renders per-row badges/splines, and shows the control active while open.

**Architecture:** The work splits along the popup-authority seam the codebase already enforces. `SelectBox` owns the *closed control* and lives in the consumer's own widget tree; it cannot open a cross-tree surface itself. `SelectFlyout` is the *open list*, a third surface kind hosted by `PopupRoot` beside `MenuPopup`/`RadialPopup`. They are glued by the established emit-request → `App`-relay → tag-filtered-close pattern (identical to burger/logo/node): the box emits `SelectBoxAction::OpenRequested`, `App` relays it to `PopupRoot::show_at(PopupSpec::Select{…})`, and the close comes back through the tag-filtered `PopupRoot::closed` queue.

**Tech Stack:** Rust, makepad fork (redoz `waml-svg-stroked-bounds` branch), the `script_mod!` DSL macro, the shared `MarkingCore` state machine, the Atlas HUD material (`AccentFrame{field_bg}` + `IconSet` SDF glyphs).

## Global Constraints

- **Never edit the main checkout.** All work happens in the worktree `C:\dev\waml\.claude\worktrees\icons`; run all git from there.
- **Full gate per task (all four must pass before commit):**
  `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
- **`dead_code` is a hard error.** The gate's clippy runs `-D warnings`, which promotes rustc `dead_code` to an error. Every new symbol must be either wired/used within its task OR carry an `#[allow(dead_code)]`. The sibling popup files establish the sanctioned pattern: `base.rs` and `marking.rs` use a module-level `#![allow(dead_code)]`; `menu.rs`/`root.rs` use item-level `#[allow(dead_code)]` on forward-declared types. New files in this plan (`popup/select.rs`, `select_box.rs`) MUST carry a module-level `#![allow(dead_code)]` at the top, exactly like `base.rs`, so forward-declared symbols land green before Task 5 wires them live.
- **RGBA stays in the DSL where practical.** Glyph tints are copied from a DSL atlas-token `DrawColor` holder before drawing (the tool-dock idiom, `IconSet::draw(cx, icon, rect, tint)`). The one sanctioned exception already in the codebase is the per-kind **badge** fill, which is Rust-computed via `bucket_color(accent_bucket(&ty))` and assigned to `draw_badge.color` at draw time — reuse that exact precedent for flyout badges.
- **Load-bearing order invariant (do NOT touch it in this plan).** The `Icon` enum / `IconSet` field / DSL / `get` / `ALL` / `label` order invariant must stay intact. This plan adds **no** new catalog glyph — the selected-row check mark is drawn as a small dedicated SDF `DrawColor` in the flyout DSL (like the inspector's badge), and the box caret reuses the existing `Icon::ChevronsUpDown`.
- **Widget registration.** Every new widget's `script_mod` MUST be added to `App::script_mod` in `crates/waml-editor/src/app.rs` (the central list, ~lines 1388-1421). A registered-but-unmounted widget is fine and precedented (`node_design_editor`).
- **New top-level module** `select_box` MUST be declared in `crates/waml-editor/src/main.rs` (the `mod …;` block, alphabetical, ~lines 4-34). New popup submodule `select` MUST be declared in `crates/waml-editor/src/popup/mod.rs`.

---

## File Structure

- `crates/waml-editor/src/popup/select.rs` — **new**. The item model (`SelectLead`, `SelectItem`), the pure width-clamp function `select_width`, and the `SelectFlyout` surface (DSL + widget + `Popup` impl). One file: the model, its geometry helper, and the only surface that consumes them change together.
- `crates/waml-editor/src/select_box.rs` — **new**. The reusable closed control `SelectBox` + `SelectBoxAction`. Top-level (not under `popup/`) because it lives in consumer trees, not the popup authority.
- `crates/waml-editor/src/popup/mod.rs` — **modify**. Register `pub mod select;`.
- `crates/waml-editor/src/popup/root.rs` — **modify**. `ActiveKind::Select`, `PopupSpec::Select`, the `select :=` DSL child, and `show_at`/`route`/`reset` arms.
- `crates/waml-editor/src/app.rs` — **modify**. Register the two new `script_mod`s; relay `OpenRequested` → `PopupSpec::Select`; route `picker_closed` into the inspector.
- `crates/waml-editor/src/inspector_panel.rs` — **modify**. Drop the hand-drawn picker field + `OpenPicker`/`PopupItem` path; host a `SelectBox`; build `SelectItem`s from diagram elements; keep `picker_ids` + `apply_pick`.
- `crates/waml-editor/src/main.rs` — **modify**. Declare `mod select_box;`.

---

### Task 1: Item model + pure width clamp (`popup/select.rs`)

Create the new file with the item descriptors and the one pure geometry helper that the flyout will feed into `LinearGeom::set_width`. This task is TDD: the width-clamp formula is pure arithmetic and gets a real failing-then-passing unit test.

**Files:**
- Create: `crates/waml-editor/src/popup/select.rs`
- Modify: `crates/waml-editor/src/popup/mod.rs` (register `pub mod select;`)

**Interfaces:**
- Consumes: `crate::icons::Icon` (for `SelectLead::Icon`); `makepad_widgets::*` (`LiveId`, `Vec4`).
- Produces (later tasks rely on these exact shapes):
  - `pub enum SelectLead { None, Icon(Icon), Badge { color: Vec4, letter: String } }`
  - `pub struct SelectItem { pub id: LiveId, pub lead: SelectLead, pub label: String, pub selected: bool, pub enabled: bool }` — both `#[derive(Clone, Debug)]`.
  - `pub fn select_width(label_hug: f64, min_width: f64, cap: f64) -> f64` — returns `label_hug.max(min_width).min(cap.max(min_width))`.
  - `pub const SELECT_MAX_W: f64 = 320.0;` `pub const LEAD_GUTTER: f64 = 42.0;` `pub const PAD_R: f64 = 18.0;` `pub const SELECT_GAP: f64 = 2.0;`

- [ ] **Step 1: Create the file skeleton with the model and constants**

Create `crates/waml-editor/src/popup/select.rs`:

```rust
//! `SelectFlyout` — the combo/select-box open list, a third `PopupRoot` surface
//! beside `MenuPopup` and `RadialPopup`. Same Atlas HUD material (`AccentFrame
//! {field_bg}` card + `IconSet` glyph rows), driven by the shared `MarkingCore`
//! in popup mode. Unlike `MenuPopup` it is at least as wide as the control that
//! opened it (`min_width`), marks the current selection, and renders each row's
//! own `SelectLead` visual. Item model + pure width clamp live here too; the
//! clamp is unit-tested directly. See
//! `docs/superpowers/specs/2026-07-22-select-box-flyout-design.md`.
#![allow(dead_code)]

use crate::icons::Icon;
use makepad_widgets::*;

/// Safety cap on flyout width (lpx). The card hugs its widest label but never
/// grows past this — unless the control itself is wider (`min_width` wins).
pub const SELECT_MAX_W: f64 = 320.0;
/// Left offset where a row label starts, past the leading `SelectLead` gutter
/// (lpx). Matches `menu::LABEL_X` so the badge/icon share the menu's 14px inset.
pub const LEAD_GUTTER: f64 = 42.0;
/// Trailing margin right of the widest label before the frame edge (lpx).
pub const PAD_R: f64 = 18.0;
/// Gap between the control's bottom edge and the card top (lpx). Tight, flush
/// left — the card sits just under the control, no horizontal indent.
pub const SELECT_GAP: f64 = 2.0;

/// A leading visual for one row. Closed set; extend with a new arm when a new
/// row shape appears (YAGNI over an open-ended draw callback).
#[derive(Clone, Debug)]
pub enum SelectLead {
    None,
    /// Edge rows lead with `Icon(Icon::Spline)`.
    Icon(Icon),
    /// Node rows lead with a per-type coloured square + kind initial.
    Badge { color: Vec4, letter: String },
}

/// One selectable row. `id` is opaque to the surface — the opener resolves it on
/// commit (same contract as `PopupItem.id`).
#[derive(Clone, Debug)]
pub struct SelectItem {
    pub id: LiveId,
    pub lead: SelectLead,
    pub label: String,
    /// Current value → trailing check mark + subtle persistent fill.
    pub selected: bool,
    /// Disabled rows draw dimmed and never arm or commit.
    pub enabled: bool,
}

/// The flyout width: hug the widest label, but never narrower than the control
/// (`min_width`) and never wider than the cap — except a control wider than the
/// cap is never clipped (`cap` floors to `min_width`).
pub fn select_width(label_hug: f64, min_width: f64, cap: f64) -> f64 {
    label_hug.max(min_width).min(cap.max(min_width))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder() {
        // Replaced by real tests in Step 3.
        assert!(true);
    }
}
```

- [ ] **Step 2: Register the module**

In `crates/waml-editor/src/popup/mod.rs`, add `pub mod select;` in alphabetical order (after `pub mod root;` — note `select` sorts after `root`, so append it):

```rust
pub mod base;
pub mod marking;
pub mod menu;
pub mod presenter;
pub mod radial;
pub mod root;
pub mod select;
```

- [ ] **Step 3: Write the failing tests for `select_width`**

Replace the `placeholder` test in `select.rs`'s `mod tests` with:

```rust
    #[test]
    fn hug_wins_when_widest() {
        // Label hug (200) beats a narrow control (120), under the cap (320).
        assert_eq!(select_width(200.0, 120.0, 320.0), 200.0);
    }

    #[test]
    fn min_width_floors_a_short_hug() {
        // A wide control (260) floors a short label hug (140).
        assert_eq!(select_width(140.0, 260.0, 320.0), 260.0);
    }

    #[test]
    fn cap_clamps_a_pathological_hug() {
        // A runaway label (900) is capped at 320.
        assert_eq!(select_width(900.0, 120.0, 320.0), 320.0);
    }

    #[test]
    fn control_wider_than_cap_is_never_clipped() {
        // A control wider than the cap (400 > 320) raises the effective cap so
        // the card is never narrower than the control it drops from.
        assert_eq!(select_width(140.0, 400.0, 320.0), 400.0);
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p waml-editor popup::select`
Expected: PASS — 4 tests (`hug_wins_when_widest`, `min_width_floors_a_short_hug`, `cap_clamps_a_pathological_hug`, `control_wider_than_cap_is_never_clipped`). The function is already implemented in Step 1, so these go green immediately (this is the "write the assertion against real arithmetic" form of TDD; if any fails, the formula in Step 1 is wrong — fix `select_width`, do not edit the test).

- [ ] **Step 5: Run the full gate**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: all green. (The module-level `#![allow(dead_code)]` keeps the as-yet-unused `SelectItem`/`SelectLead`/constants from tripping clippy `-D warnings`.)

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/popup/select.rs crates/waml-editor/src/popup/mod.rs
git commit -m "feat(popup): select item model + pure width clamp"
```

---

### Task 2: `SelectFlyout` surface (`popup/select.rs`)

Add the surface to the same file: the DSL card material, the widget struct (own overlay draw list like `MenuPopup`), the `draw` that computes width via `select_width` + measures labels with makepad's text engine, per-row lead rendering (icon / badge / none), the transient-hover vs persistent-selected fills, the trailing check mark, and the `Popup` trait impl driving `MarkingCore` in popup mode. Register its `script_mod`. Rendering is UI-driven, so this task's runtime proof is deferred to Task 5's `/run`; the gate here is compile + existing tests.

**Files:**
- Modify: `crates/waml-editor/src/popup/select.rs`
- Modify: `crates/waml-editor/src/app.rs` (add `crate::popup::select::script_mod(vm);` to `App::script_mod`)

**Interfaces:**
- Consumes: `crate::popup::menu::{PAD_V, PAD_H, ROW_H, DRAG_THRESHOLD, LinearGeom}`; `crate::popup::base::{Popup, PopupResult, PopupVerdict, PopupItem}`; `crate::popup::marking::{MarkOutcome, MarkingCore}`; `crate::icons::IconSet`; `select_width`, `SELECT_MAX_W`, `LEAD_GUTTER`, `PAD_R`, `SelectItem`, `SelectLead` from this file.
- Produces (Task 3 relies on these):
  - `pub struct SelectFlyout` (a `#[derive(Script, ScriptHook, Widget)]` widget).
  - `pub fn open_select(&mut self, cx: &mut Cx, anchor: DVec2, min_width: f64, items: Vec<SelectItem>)`.
  - `pub fn is_open(&self) -> bool`.
  - `impl Popup for SelectFlyout` (`handle`, `reset`).
  - `pub fn script_mod(vm: &mut ScriptVm) -> ScriptValue` (generated by the `script_mod!` macro).

**Design note — MarkingCore stays PopupItem-based.** `MarkingCore` hardcodes `Vec<PopupItem>` and only reads `id` + `enabled` from each slot (see `marking.rs::release`). The flyout keeps its own `Vec<SelectItem>` for *rendering* and feeds `MarkingCore` a parallel `Vec<PopupItem>` carrying only the load-bearing `{ id, enabled }` (label empty, `icon: Icon::Spline` placeholder, `danger: false`). This reuses `MarkingCore` unchanged (the spec's intent) — render data and commit data are simply split. `armed()`/`items().len()` still index the same rows.

- [ ] **Step 1: Add the DSL card material**

Append a `script_mod!` block to `select.rs` (place it after the `select_width` fn, before `#[cfg(test)]`). Mirror `menu.rs`'s block but with the select-specific holders — a persistent selected fill, and a check-mark SDF drawn inline (no catalog glyph):

```rust
script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    mod.widgets.SelectFlyoutBase = #(SelectFlyout::register_widget(vm))

    mod.widgets.SelectFlyout = set_type_default() do mod.widgets.SelectFlyoutBase{
        width: Fill
        height: Fill
        // Same source-bright Atlas frame + field-bg fill as MenuPopup, so the
        // flyout reads as one HUD material with the control it drops from.
        draw_frame: mod.draw.AccentFrame{ color: atlas.field_bg }
        // Transient hover wash (matches MenuPopup) and the subtle *persistent*
        // fill on the currently-selected row (a fainter accent so it reads as a
        // marked value, not a hover).
        draw_hover: mod.draw.DrawColor{ color: atlas.selection }
        draw_selected: mod.draw.DrawColor{ color: atlas.accent_soft }
        // Row glyph tints (copied per row; no RGBA crosses Rust for icons).
        draw_icon_idle +: { color: atlas.text }
        draw_icon_accent +: { color: atlas.accent }
        // Per-type badge: solid coloured square (colour set at draw time from
        // the row's SelectLead::Badge) with the kind initial (white) on top.
        draw_badge: mod.draw.DrawColor{ color: atlas.bucket_slate }
        draw_badge_text +: {
            color: #xffffff
            text_style: theme.font_regular{ font_size: 10 }
        }
        // Trailing check mark on the selected row — a small inline SDF stroke,
        // NOT a catalog glyph (keeps the Icon order invariant untouched).
        draw_check: mod.draw.DrawColor{
            color: atlas.accent
            pixel: fn() {
                let s = self.rect_size.x
                let w = s * 0.10
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.move_to(s * 0.20, s * 0.52)
                sdf.line_to(s * 0.42, s * 0.74)
                sdf.line_to(s * 0.80, s * 0.28)
                sdf.stroke(self.color, w)
                return sdf.result
            }
        }
        draw_label +: {
            color: atlas.text
            text_style: theme.font_regular{ font_size: 10 line_spacing: 1.2 }
        }
    }
}
```

- [ ] **Step 2: Add the widget struct**

After the `script_mod!` block add the struct. Mirror `MenuPopup`'s field set (own overlay `draw_list`, the mark/geom state) plus the select holders and the `items: Vec<SelectItem>` render vector:

```rust
#[derive(Script, ScriptHook, Widget)]
pub struct SelectFlyout {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    /// Own draw list into the WINDOW OVERLAY (`begin_overlay_reuse`), so the
    /// card escapes the body clip — identical idiom to `MenuPopup`.
    #[live]
    draw_list: DrawList2d,

    #[redraw]
    #[live]
    draw_frame: DrawColor,
    #[redraw]
    #[live]
    draw_hover: DrawColor,
    #[redraw]
    #[live]
    draw_selected: DrawColor,
    #[redraw]
    #[live]
    draw_icon_idle: DrawColor,
    #[redraw]
    #[live]
    draw_icon_accent: DrawColor,
    #[redraw]
    #[live]
    draw_badge: DrawColor,
    #[redraw]
    #[live]
    draw_badge_text: DrawText,
    #[redraw]
    #[live]
    draw_check: DrawColor,
    #[redraw]
    #[live]
    draw_label: DrawText,
    /// Shared project-tree SDF glyph set (for `SelectLead::Icon`).
    #[live]
    icons: IconSet,

    #[rust]
    mark: MarkingCore,
    #[rust]
    geom: LinearGeom,
    /// Render rows, parallel to `mark`'s `{id,enabled}` PopupItems (see the
    /// design note): the flyout draws from these by index.
    #[rust]
    items: Vec<SelectItem>,
    /// The control width passed at open, floored into `select_width`.
    #[rust]
    min_width: f64,
}
```

- [ ] **Step 3: Add the `Widget` impl (event-passive, overlay draw)**

Copy `MenuPopup`'s `Widget` impl verbatim except the type name:

```rust
impl Widget for SelectFlyout {
    // Event-passive: `PopupRoot` drives this through the inherent methods.
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {}

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, _walk: Walk) -> DrawStep {
        if !self.is_open() {
            return DrawStep::done();
        }
        self.draw_list.begin_overlay_reuse(cx);
        let size = cx.current_pass_size();
        cx.begin_root_turtle(size, Layout::flow_overlay());
        self.draw(cx);
        cx.end_pass_sized_turtle();
        self.draw_list.end(cx);
        DrawStep::done()
    }
}
```

- [ ] **Step 4: Add `open_select` + `is_open` + `draw` (inherent impl)**

```rust
impl SelectFlyout {
    pub fn is_open(&self) -> bool {
        self.mark.is_open()
    }

    /// Latched popup open dropping from `anchor`; `min_width` is the control's
    /// width (the card is never narrower than it).
    pub fn open_select(
        &mut self,
        cx: &mut Cx,
        anchor: DVec2,
        min_width: f64,
        items: Vec<SelectItem>,
    ) {
        use crate::popup::menu::DRAG_THRESHOLD;
        // Parallel commit vector: MarkingCore only needs {id, enabled}.
        let marks: Vec<PopupItem> = items
            .iter()
            .map(|it| PopupItem {
                id: it.id,
                label: String::new(),
                icon: Icon::Spline,
                danger: false,
                enabled: it.enabled,
            })
            .collect();
        self.geom = LinearGeom::new(anchor, items.len());
        self.items = items;
        self.min_width = min_width;
        self.mark.begin_popup(marks, DRAG_THRESHOLD);
        self.draw_frame.redraw(cx);
    }

    pub fn draw(&mut self, cx: &mut Cx2d) {
        use crate::popup::menu::{PAD_H, PAD_V, ROW_H};
        if !self.is_open() {
            return;
        }
        let hovered = self.mark.armed();

        // Width: hug the widest measured label (makepad's own text engine, same
        // as MenuPopup), floored by min_width, capped by SELECT_MAX_W.
        let mut widest = 0.0_f64;
        for it in &self.items {
            if let Some(run) = self.draw_label.prepare_single_line_run(cx, &it.label) {
                widest = widest.max(run.width_in_lpxs as f64);
            }
        }
        let hug = LEAD_GUTTER + widest + PAD_R;
        self.geom
            .set_width(select_width(hug, self.min_width, SELECT_MAX_W));

        let panel = self.geom.panel_rect();
        self.draw_frame.set_uniform(cx, live_id!(zoom), &[0.6]);
        self.draw_frame.draw_abs(cx, panel);

        // Read tint holders before borrowing `self.icons`.
        let idle = self.draw_icon_idle.color;
        let accent = self.draw_icon_accent.color;

        for i in 0..self.items.len() {
            let it = self.items[i].clone();
            let row = self.geom.row_rect(i);
            let cy = row.pos.y + row.size.y * 0.5;

            // Persistent selected fill first (under any hover wash), inset off
            // both frame edges like the hover highlight.
            if it.selected {
                let fill = Rect {
                    pos: dvec2(panel.pos.x + PAD_H, row.pos.y),
                    size: dvec2(panel.size.x - PAD_H * 2.0, row.size.y),
                };
                self.draw_selected.draw_abs(cx, fill);
            }
            if hovered == Some(i) && it.enabled {
                let hi = Rect {
                    pos: dvec2(panel.pos.x + PAD_H, row.pos.y),
                    size: dvec2(panel.size.x - PAD_H * 2.0, row.size.y),
                };
                self.draw_hover.draw_abs(cx, hi);
            }

            // Leading visual.
            match &it.lead {
                SelectLead::None => {}
                SelectLead::Icon(icon) => {
                    let icon_rect = Rect {
                        pos: dvec2(row.pos.x + 14.0, cy - 8.0),
                        size: dvec2(16.0, 16.0),
                    };
                    let tint = if hovered == Some(i) && it.enabled {
                        accent
                    } else {
                        idle
                    };
                    self.icons.draw(cx, *icon, icon_rect, tint);
                }
                SelectLead::Badge { color, letter } => {
                    let badge = Rect {
                        pos: dvec2(row.pos.x + 12.0, cy - 10.0),
                        size: dvec2(20.0, 20.0),
                    };
                    self.draw_badge.color = *color;
                    self.draw_badge.draw_abs(cx, badge);
                    if !letter.is_empty() {
                        self.draw_badge_text
                            .draw_abs(cx, dvec2(badge.pos.x + 6.0, badge.pos.y + 3.0), letter);
                    }
                }
            }

            // Label.
            self.draw_label
                .draw_abs(cx, dvec2(row.pos.x + LEAD_GUTTER, cy - 6.0), &it.label);

            // Trailing check mark on the selected row.
            if it.selected {
                let check = Rect {
                    pos: dvec2(panel.pos.x + panel.size.x - PAD_R - 14.0, cy - 7.0),
                    size: dvec2(14.0, 14.0),
                };
                self.draw_check.draw_abs(cx, check);
            }
        }
    }
}
```

- [ ] **Step 5: Add the `Popup` trait impl + `map_outcome`**

Copy `MenuPopup`'s popup-mode `Popup` impl, minus the marking-drag branch (a select box is always click-latched):

```rust
impl Popup for SelectFlyout {
    fn handle(&mut self, cx: &mut Cx, event: &Event) -> PopupVerdict {
        if !self.mark.is_open() {
            return PopupVerdict::Consumed;
        }
        let verdict = match event {
            Event::MouseMove(e) => {
                self.mark.pointer_move(e.abs, self.geom.row_at(e.abs));
                self.draw_frame.redraw(cx);
                PopupVerdict::Consumed
            }
            Event::MouseUp(e) if e.button.is_primary() => {
                map_outcome(self.mark.release(self.geom.row_at(e.abs)))
            }
            // Popup mode: a press ON the card arms; a press OFF is the outside
            // click → Ignored (PopupRoot dismisses).
            Event::MouseDown(e) if e.button.is_primary() && self.mark.is_popup() => {
                if self.geom.panel_rect().contains(e.abs) {
                    self.mark.press(e.abs, self.geom.row_at(e.abs));
                    self.draw_frame.redraw(cx);
                    PopupVerdict::Consumed
                } else {
                    PopupVerdict::Ignored
                }
            }
            _ => PopupVerdict::Consumed,
        };
        if let PopupVerdict::Closed(_) = verdict {
            self.draw_frame.redraw(cx);
        }
        verdict
    }

    fn reset(&mut self) {
        self.mark.close();
    }
}

fn map_outcome(o: MarkOutcome) -> PopupVerdict {
    match o {
        MarkOutcome::Committed(id) => PopupVerdict::Closed(PopupResult::Invoked(id)),
        MarkOutcome::Cancelled => PopupVerdict::Closed(PopupResult::Dismissed),
        MarkOutcome::None => PopupVerdict::Consumed,
    }
}
```

Add the needed imports at the top of the file (below the existing `use crate::icons::Icon;`):

```rust
use crate::popup::base::{Popup, PopupItem, PopupResult, PopupVerdict};
use crate::popup::marking::{MarkOutcome, MarkingCore};
use crate::popup::menu::LinearGeom;
use crate::icons::IconSet;
```

- [ ] **Step 6: Register the surface's `script_mod`**

In `crates/waml-editor/src/app.rs`, in `App::script_mod` (~line 1400-1402, beside the other popup registrations), add:

```rust
        crate::popup::menu::script_mod(vm);
        crate::popup::radial::script_mod(vm);
        crate::popup::select::script_mod(vm);
        crate::popup::root::script_mod(vm);
```

- [ ] **Step 7: Verify it compiles**

Run: `cargo build -p waml-editor`
Expected: clean build. If `atlas.bucket_slate` / `atlas.accent_soft` don't resolve, confirm the token names against `theme_atlas.rs` (the inspector uses `atlas.bucket_slate` for `draw_badge` and `menu.rs` uses `atlas.accent_soft`, so both exist).

- [ ] **Step 8: Run the full gate**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: all green. `SelectFlyout` is registered but not yet mounted or opened — `#![allow(dead_code)]` covers the not-yet-called `open_select`/`is_open`.

- [ ] **Step 9: Commit**

```bash
git add crates/waml-editor/src/popup/select.rs crates/waml-editor/src/app.rs
git commit -m "feat(popup): SelectFlyout surface (badge/icon rows, selected mark, min-width)"
```

---

### Task 3: `PopupRoot` wiring — `ActiveKind::Select` (`popup/root.rs`)

Add the third surface kind to the authority, mirroring the `Menu`/`Radial` seams exactly: the `select :=` DSL child, `ActiveKind::Select`, `PopupSpec::Select`, and matching `show_at` / `route` / `reset` (supersede) arms. The pure `decide()` is unchanged — `Select` routes through it, and the existing `decide` tests already cover commit/dismiss/keep.

**Files:**
- Modify: `crates/waml-editor/src/popup/root.rs`

**Interfaces:**
- Consumes: `crate::popup::select::{SelectFlyout, SelectItem, SELECT_MAX_W}`; existing `Presenter::place`, `Popup` trait, `decide`.
- Produces (Task 5's `App` relay relies on this exact variant):
  - `PopupSpec::Select { tag: LiveId, anchor: DVec2, min_width: f64, bounds: Rect, items: Vec<SelectItem> }`.

- [ ] **Step 1: Import the new surface**

In `root.rs`, extend the imports near the top (beside the `menu`/`radial` imports, ~lines 9-11):

```rust
use crate::popup::menu::{MenuPopup, MENU_MAX_W, PAD_V, ROW_H};
use crate::popup::presenter::Presenter;
use crate::popup::radial::RadialPopup;
use crate::popup::select::{SelectFlyout, SelectItem, SELECT_MAX_W};
```

- [ ] **Step 2: Add the `PopupSpec::Select` variant**

In `enum PopupSpec` (~lines 36-51), add a third arm after `Radial`. No `open` field — a combo is always click-latched (`begin_popup`); there is no marking-open variant:

```rust
    Radial {
        tag: LiveId,
        center: DVec2,
        bounds: Rect,
        items: Vec<PopupItem>,
        open: RadialOpen,
    },
    Select {
        tag: LiveId,
        anchor: DVec2,
        min_width: f64,
        bounds: Rect,
        items: Vec<SelectItem>,
    },
}
```

- [ ] **Step 3: Add the `ActiveKind::Select` discriminant**

In `enum ActiveKind` (~lines 68-72):

```rust
#[derive(Clone, Copy, PartialEq)]
enum ActiveKind {
    Menu,
    Radial,
    Select,
}
```

- [ ] **Step 4: Add the `select :=` DSL child**

In the `script_mod!` `body: View{ … }` block (~lines 110-115), add the flyout as a third tree child. Import it in the `use` list too:

```rust
    use mod.prelude.widgets_internal.*
    use mod.widgets.*
```
(The `SelectFlyout` widget type resolves through `mod.widgets.*` once Task 2 registered it — no extra `use` line is needed, matching how `MenuPopup`/`RadialPopup` resolve.)

```rust
        body: View{
            width: Fill
            height: Fill
            menu := MenuPopup{ width: Fill height: Fill }
            radial := RadialPopup{ width: Fill height: Fill }
            select := SelectFlyout{ width: Fill height: Fill }
        }
```

- [ ] **Step 5: Add the supersede (`reset`) arm in `show_at`**

In `show_at`, the supersede block (~lines 161-178) has a `match kind` over the prior active surface. Add a `Select` arm:

```rust
                ActiveKind::Radial => {
                    if let Some(mut r) = self
                        .body
                        .widget(cx, ids!(radial))
                        .borrow_mut::<RadialPopup>()
                    {
                        r.reset();
                    }
                }
                ActiveKind::Select => {
                    if let Some(mut s) = self
                        .body
                        .widget(cx, ids!(select))
                        .borrow_mut::<SelectFlyout>()
                    {
                        s.reset();
                    }
                }
```

- [ ] **Step 6: Add the `Select` open arm in `show_at`**

After the `PopupSpec::Radial { … }` match arm (~lines 208-227), add:

```rust
            PopupSpec::Select {
                tag,
                anchor,
                min_width,
                bounds,
                items,
            } => {
                // Clamp on-screen. Width is unknown until draw measures the
                // label, so clamp with the widest possible width — the cap, or
                // the control if it is wider (matches `select_width`'s ceiling).
                let cap = SELECT_MAX_W.max(min_width);
                let size = dvec2(cap, PAD_V * 2.0 + items.len() as f64 * ROW_H);
                let placed = Presenter::place(anchor, size, bounds);
                if let Some(mut s) = self
                    .body
                    .widget(cx, ids!(select))
                    .borrow_mut::<SelectFlyout>()
                {
                    s.open_select(cx, placed, min_width, items);
                }
                self.active = Some((ActiveKind::Select, tag));
            }
```

- [ ] **Step 7: Add the `Select` arm in `route`**

In `route`, the `match kind` that dispatches `handle` (~lines 251-264) and the `match kind` that resets on close (~lines 268-284) each need a `Select` arm. In the handle dispatch:

```rust
                ActiveKind::Radial => self
                    .body
                    .widget(cx, ids!(radial))
                    .borrow_mut::<RadialPopup>()
                    .map(|mut r| r.handle(cx, ev))
                    .unwrap_or(PopupVerdict::Ignored),
                ActiveKind::Select => self
                    .body
                    .widget(cx, ids!(select))
                    .borrow_mut::<SelectFlyout>()
                    .map(|mut s| s.handle(cx, ev))
                    .unwrap_or(PopupVerdict::Ignored),
```

In the close-reset dispatch:

```rust
                ActiveKind::Radial => {
                    if let Some(mut r) = self
                        .body
                        .widget(cx, ids!(radial))
                        .borrow_mut::<RadialPopup>()
                    {
                        r.reset();
                    }
                }
                ActiveKind::Select => {
                    if let Some(mut s) = self
                        .body
                        .widget(cx, ids!(select))
                        .borrow_mut::<SelectFlyout>()
                    {
                        s.reset();
                    }
                }
```

- [ ] **Step 8: Verify the existing `decide` tests still pass**

Run: `cargo test -p waml-editor popup::root`
Expected: PASS — the 5 existing `decide` tests (`a_commit_closes_with_its_result`, `a_self_dismiss_closes_dismissed`, `an_ignored_primary_press_is_outside_click_dismiss`, `an_ignored_non_press_keeps_it_open`, `a_consumed_event_keeps_it_open`) are unchanged; `Select` reuses `decide` unmodified.

- [ ] **Step 9: Run the full gate**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: all green. `PopupSpec::Select` is not yet constructed by any caller — the enum's existing `#[allow(dead_code)]` covers the unused variant.

- [ ] **Step 10: Commit**

```bash
git add crates/waml-editor/src/popup/root.rs
git commit -m "feat(popup): wire SelectFlyout as ActiveKind::Select in PopupRoot"
```

---

### Task 4: `SelectBox` control widget (`select_box.rs`)

Create the reusable closed control: it renders the box (frame + selected lead + selected label + caret), holds `items`/`selected`/`open` state, emits `OpenRequested` on click, draws active while `open`, and exposes the reader/mutator API. State transitions are pure enough to TDD (`on_closed` clears `open`; `Invoked` updates `selected`, `Dismissed` does not).

**Files:**
- Create: `crates/waml-editor/src/select_box.rs`
- Modify: `crates/waml-editor/src/main.rs` (declare `mod select_box;`)
- Modify: `crates/waml-editor/src/app.rs` (register `crate::select_box::script_mod(vm);`)

**Interfaces:**
- Consumes: `crate::popup::select::{SelectItem, SelectLead, SELECT_GAP}`; `crate::popup::base::PopupResult`; `crate::icons::{Icon, IconSet}`; `makepad_widgets::*`.
- Produces (Task 5's inspector + App rely on these exact signatures):
  - `pub struct SelectBox` (a `Widget`), `pub enum SelectBoxAction { None, OpenRequested { anchor: Rect, min_width: f64, items: Vec<SelectItem> } }` (`#[derive(Clone, Debug, Default)]`, `None` is `#[default]`).
  - `pub fn set_items(&mut self, cx: &mut Cx, items: Vec<SelectItem>)`.
  - `pub fn set_selected(&mut self, cx: &mut Cx, selected: Option<usize>)`.
  - `pub fn picked(&self, actions: &Actions) -> Option<LiveId>` — reads the box's own `OpenRequested`? No: `picked` reads a *close*; see Step 5. It returns the id chosen after `on_closed(Invoked)`. Implemented as a plain getter over the last applied pick is unnecessary — `picked` is a convenience the consumer may skip; the inspector drives via `on_closed`. Provide it as: after `on_closed(cx, PopupResult::Invoked(id))`, `picked` is not action-based — instead expose `pub fn selected_id(&self) -> Option<LiveId>`. **Resolution:** implement `pub fn open_request(&self, actions: &Actions) -> Option<(Rect, f64, Vec<SelectItem>)>` (the load-bearing reader) and `pub fn on_closed(&mut self, cx: &mut Cx, result: PopupResult) -> Option<LiveId>` (returns the committed id on `Invoked`, else `None`). These two cover the spec's data flow; `picked`/`selected_id` are dropped as redundant (YAGNI — the inspector uses `on_closed`'s return + its own `apply_pick`).

- [ ] **Step 1: Create the file with the action + DSL + struct**

Create `crates/waml-editor/src/select_box.rs`:

```rust
//! `SelectBox` — the reusable closed combo control. Renders the box you see when
//! nothing is open (Atlas `AccentFrame{field_bg}` + the selected row's lead +
//! label + a trailing caret). It CANNOT open the list itself (popup authority
//! lives in `PopupRoot`): a click emits `SelectBoxAction::OpenRequested`, `App`
//! relays it to `PopupRoot::show_at(PopupSpec::Select{…})`, and the close comes
//! back through the tag-filtered queue into `on_closed`. See
//! `docs/superpowers/specs/2026-07-22-select-box-flyout-design.md`.
#![allow(dead_code)]

use crate::icons::{Icon, IconSet};
use crate::popup::base::PopupResult;
use crate::popup::select::{SelectItem, SelectLead};
use makepad_widgets::*;

/// Emitted by the box. `App` reads `open_request` and relays to `PopupRoot`.
#[derive(Clone, Debug, Default)]
pub enum SelectBoxAction {
    #[default]
    None,
    OpenRequested {
        anchor: Rect,
        min_width: f64,
        items: Vec<SelectItem>,
    },
}

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    mod.widgets.SelectBoxBase = #(SelectBox::register_widget(vm))

    mod.widgets.SelectBox = set_type_default() do mod.widgets.SelectBoxBase{
        width: Fill
        height: 32.0
        // Field material: the shared Atlas frame + field-bg fill.
        draw_frame: mod.draw.AccentFrame{ color: atlas.field_bg }
        // Active overlay RING drawn over the box while the list is open — a
        // source-bright accent border, the visual link to the open flyout.
        // Stroke-ONLY (no fill): a second `AccentFrame` would re-run
        // `sdf.fill_keep(self.color)` (see `frame.rs:50`) and re-paint the
        // interior, blanking the badge/label/caret drawn underneath (finding
        // H1). This variant strokes the accent edge and leaves the interior
        // untouched, so the open box keeps its content and gains an accent ring.
        draw_active: mod.draw.DrawColor{
            color: atlas.accent
            pixel: fn() {
                let inset = 1.5
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.rect(inset, inset, self.rect_size.x - inset * 2.0, self.rect_size.y - inset * 2.0)
                sdf.stroke(self.color, inset)
                return sdf.result
            }
        }
        draw_badge: mod.draw.DrawColor{ color: atlas.bucket_slate }
        draw_badge_text +: {
            color: #xffffff
            text_style: theme.font_regular{ font_size: 10 }
        }
        draw_icon_idle +: { color: atlas.text }
        draw_caret +: { color: atlas.text_dim }
        draw_label +: {
            color: atlas.text
            text_style: theme.font_regular{ font_size: 10 line_spacing: 1.2 }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct SelectBox {
    #[deref]
    view: View,

    #[redraw]
    #[live]
    draw_frame: DrawColor,
    #[redraw]
    #[live]
    draw_active: DrawColor,
    #[redraw]
    #[live]
    draw_badge: DrawColor,
    #[redraw]
    #[live]
    draw_badge_text: DrawText,
    #[redraw]
    #[live]
    draw_icon_idle: DrawColor,
    #[redraw]
    #[live]
    draw_caret: DrawColor,
    #[redraw]
    #[live]
    draw_label: DrawText,
    #[live]
    icons: IconSet,

    #[rust]
    items: Vec<SelectItem>,
    #[rust]
    selected: Option<usize>,
    #[rust]
    open: bool,
}
```

- [ ] **Step 2: Add the `Widget` impl (draw the box + emit on click)**

The box is a `View`-deref widget (its own tree node, so its area is real and hit-testable — unlike the inspector's manual-rect hand-drawn field). Draw the frame, the selected lead + label, and the caret; on a primary click over the box, set `open` and emit `OpenRequested` with an **event-time** anchor read from `self.view.area().rect(cx)`.

**Finding H2 — no `box_rect`, event-time anchor.** The earlier draft captured a `box_rect` in `draw_walk` and used it for both a `contains` hit-guard and the flyout anchor. That is a trap: the panel this box lives in is right-aligned, so the rect captured in `draw_walk` is PRE-alignment (`x≈0`), while event `abs` coordinates arrive POST-alignment. `box_rect.contains(fe.abs)` would be silently false (dead click) and the anchor would mis-place the flyout far-left. Fix: a `Hit::FingerUp` from `event.hits(cx, self.view.area())` *already* means the press landed on the box's real (post-alignment) area — no `contains` guard is needed. Read the anchor from `self.view.area().rect(cx)` at EVENT time, which is post-alignment and correct.

```rust
impl Widget for SelectBox {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        let uid = self.widget_uid();
        if let Hit::FingerUp(fe) = event.hits(cx, self.view.area()) {
            if fe.is_primary_hit() {
                // A hit on `view.area()` already means the press landed on the
                // box — NO `box_rect.contains` guard (that rect is pre-alignment;
                // the panel is right-aligned → event abs never matches → dead
                // click). See finding H2.
                self.open = true;
                self.view.redraw(cx);
                // Anchor from the EVENT-TIME area rect (post-alignment), never a
                // draw-captured rect (pre-alignment, x≈0 → mis-anchored far-left).
                let anchor = self.view.area().rect(cx);
                let min_width = anchor.size.x;
                let items = self.items.clone();
                cx.widget_action(
                    uid,
                    SelectBoxAction::OpenRequested {
                        anchor,
                        min_width,
                        items,
                    },
                );
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        while self.view.draw_walk(cx, scope, walk).step().is_some() {}
        let rect = self.view.area().rect(cx);
        let cy = rect.pos.y + rect.size.y * 0.5;

        // Card.
        self.draw_frame.set_uniform(cx, live_id!(zoom), &[0.6]);
        self.draw_frame.draw_abs(cx, rect);

        // Selected row's lead + label (or nothing selected → placeholder blank).
        let idle = self.draw_icon_idle.color;
        let mut label_x = rect.pos.x + 12.0;
        if let Some(sel) = self.selected.and_then(|i| self.items.get(i)).cloned() {
            match &sel.lead {
                SelectLead::None => {}
                SelectLead::Icon(icon) => {
                    let r = Rect { pos: dvec2(rect.pos.x + 10.0, cy - 8.0), size: dvec2(16.0, 16.0) };
                    self.icons.draw(cx, *icon, r, idle);
                    label_x = rect.pos.x + 34.0;
                }
                SelectLead::Badge { color, letter } => {
                    let b = Rect { pos: dvec2(rect.pos.x + 8.0, cy - 10.0), size: dvec2(20.0, 20.0) };
                    self.draw_badge.color = *color;
                    self.draw_badge.draw_abs(cx, b);
                    if !letter.is_empty() {
                        self.draw_badge_text
                            .draw_abs(cx, dvec2(b.pos.x + 6.0, b.pos.y + 3.0), letter);
                    }
                    label_x = rect.pos.x + 36.0;
                }
            }
            self.draw_label.draw_abs(cx, dvec2(label_x, cy - 6.0), &sel.label);
        }

        // Trailing caret (chevrons-up-down = the standard combo affordance).
        let caret = Rect {
            pos: dvec2(rect.pos.x + rect.size.x - 24.0, cy - 8.0),
            size: dvec2(16.0, 16.0),
        };
        let ct = self.draw_caret.color;
        self.icons.draw(cx, Icon::ChevronsUpDown, caret, ct);

        // Active accent RING over the box while the list is open — drawn LAST so
        // it sits atop the content. Stroke-only (no re-fill), so the badge/label/
        // caret stay visible; a full AccentFrame here would blank them (H1).
        if self.open {
            self.draw_active.draw_abs(cx, rect);
        }

        DrawStep::done()
    }
}
```

- [ ] **Step 3: Add the state API (`set_items`, `set_selected`, `open_request`, `on_closed`)**

```rust
impl SelectBox {
    pub fn set_items(&mut self, cx: &mut Cx, items: Vec<SelectItem>) {
        self.items = items;
        self.view.redraw(cx);
    }

    pub fn set_selected(&mut self, cx: &mut Cx, selected: Option<usize>) {
        self.selected = selected;
        self.view.redraw(cx);
    }

    /// `App` reads this to relay the open. `None` unless the box asked to open.
    pub fn open_request(
        &self,
        actions: &Actions,
    ) -> Option<(Rect, f64, Vec<SelectItem>)> {
        let item = actions.find_widget_action(self.widget_uid())?;
        if let SelectBoxAction::OpenRequested { anchor, min_width, items } = item.cast() {
            Some((anchor, min_width, items))
        } else {
            None
        }
    }

    /// The list closed. Always clears `open`; on `Invoked(id)` updates
    /// `selected` to that row and returns the id (else `None`).
    pub fn on_closed(&mut self, cx: &mut Cx, result: PopupResult) -> Option<LiveId> {
        self.open = false;
        let picked = match result {
            PopupResult::Invoked(id) => {
                if let Some(i) = self.items.iter().position(|it| it.id == id) {
                    self.selected = Some(i);
                }
                Some(id)
            }
            PopupResult::Dismissed => None,
        };
        self.view.redraw(cx);
        picked
    }

    pub fn is_open(&self) -> bool {
        self.open
    }
}
```

- [ ] **Step 4: Declare the module + register it**

In `crates/waml-editor/src/main.rs`, add `mod select_box;` in alphabetical order (after `mod scene;`, before `mod selection_toolbar;`):

```rust
mod scene;
mod select_box;
mod selection_toolbar;
```

In `crates/waml-editor/src/app.rs`, in `App::script_mod`, register it (e.g. beside `selection_toolbar`):

```rust
        crate::select_box::script_mod(vm);
        crate::selection_toolbar::script_mod(vm);
```

- [ ] **Step 5: Write the failing state-transition tests**

`SelectBox` needs a `Cx` to construct via the widget system, so drive `on_closed`'s pure state logic against a directly-constructed value. Add a `#[cfg(test)]` module. Because `SelectBox` derives `Widget` (no public `new`), extract the pure decision into a free helper and test THAT, then have `on_closed` call it — this keeps the test `Cx`-free:

Add above `impl SelectBox`:

```rust
/// Pure `on_closed` decision over prior selection + result: returns the new
/// `(open, selected)` and the committed id. `find_index` maps an invoked id to a
/// row index (the widget passes a closure over its own `items`).
fn decide_closed(
    result: &PopupResult,
    prior_selected: Option<usize>,
    find_index: impl Fn(LiveId) -> Option<usize>,
) -> (bool, Option<usize>, Option<LiveId>) {
    match result {
        PopupResult::Invoked(id) => {
            let sel = find_index(*id).or(prior_selected);
            (false, sel, Some(*id))
        }
        PopupResult::Dismissed => (false, prior_selected, None),
    }
}
```

Rewrite `on_closed` to use it:

```rust
    pub fn on_closed(&mut self, cx: &mut Cx, result: PopupResult) -> Option<LiveId> {
        let idx_of = |id: LiveId| self.items.iter().position(|it| it.id == id);
        let (open, selected, picked) = decide_closed(&result, self.selected, idx_of);
        self.open = open;
        self.selected = selected;
        self.view.redraw(cx);
        picked
    }
```

Add the tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dismiss_clears_open_and_keeps_selection() {
        let (open, sel, picked) =
            decide_closed(&PopupResult::Dismissed, Some(2), |_| None);
        assert!(!open);
        assert_eq!(sel, Some(2));
        assert_eq!(picked, None);
    }

    #[test]
    fn invoke_clears_open_and_updates_selection() {
        let (open, sel, picked) = decide_closed(
            &PopupResult::Invoked(live_id!(row_b)),
            Some(0),
            |id| if id == live_id!(row_b) { Some(3) } else { None },
        );
        assert!(!open);
        assert_eq!(sel, Some(3));
        assert_eq!(picked, Some(live_id!(row_b)));
    }

    #[test]
    fn invoke_of_unknown_id_keeps_prior_selection() {
        let (open, sel, picked) = decide_closed(
            &PopupResult::Invoked(live_id!(ghost)),
            Some(1),
            |_| None,
        );
        assert!(!open);
        assert_eq!(sel, Some(1));
        assert_eq!(picked, Some(live_id!(ghost)));
    }
}
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p waml-editor select_box`
Expected: PASS — 3 tests. (If `invoke_clears_open_and_updates_selection` fails, the `find_index`/`or(prior)` order in `decide_closed` is wrong — fix the helper, not the test.)

- [ ] **Step 7: Run the full gate**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: all green. `SelectBox` is registered but not yet mounted; `#![allow(dead_code)]` covers `set_items`/`set_selected`/`open_request`/`on_closed`/`is_open` until Task 5.

- [ ] **Step 8: Commit**

```bash
git add crates/waml-editor/src/select_box.rs crates/waml-editor/src/main.rs crates/waml-editor/src/app.rs
git commit -m "feat(editor): SelectBox reusable combo control"
```

---

### Task 5: Inspector consumer + App relay (`inspector_panel.rs`, `app.rs`)

Wire it live: mount a `SelectBox` in the inspector's picker bar, build `SelectItem`s from the diagram elements (node badges + edge splines, current subject marked selected, edges disabled), drop the hand-drawn field + `OpenPicker`/`PopupItem` path (keep `picker_ids` + `apply_pick`), and relay the box's `OpenRequested` → `PopupSpec::Select` in `App`, routing the close back into the inspector. This task's verification is `/run` on a diagram fixture (rendering + interaction are UI-driven).

**Files:**
- Modify: `crates/waml-editor/src/inspector_panel.rs`
- Modify: `crates/waml-editor/src/app.rs`

**Interfaces:**
- Consumes: `crate::select_box::SelectBox`; `crate::popup::select::{SelectItem, SelectLead, SELECT_GAP}`; `crate::popup::root::PopupSpec`; existing `bucket_color`, `accent_bucket`, `edge_target`, `subject_to_index`, `apply_pick`, `build_view`.
- Produces:
  - Inspector: `pub fn take_open_request(&self, cx: &mut Cx, actions: &Actions) -> Option<(Rect, f64, Vec<SelectItem>)>` and `pub fn on_picker_closed(&mut self, cx: &mut Cx, model: &Model, result: PopupResult)`.
  - `set_diagram_elements` gains a `model: &Model` parameter (needed to compute per-node badges — see the design note below).

**Design note — badges need the model.** `ElementRow` carries only `{ key, label, kind }` (no `ElementType`), which is exactly why the old `picker_items` fell back to a generic `Icon::PackageOpen` for nodes. To restore per-type badges the `SelectItem` build must look each node up in the model: `model.nodes.iter().find(|n| n.key == row.key)` → `bucket_color(accent_bucket(&node.ty))` for the colour, and the kind initial for the letter (reuse the exact derivation `set_subject` already uses: `build_view(model, &Subject::Classifier(key)).and_then(|v| v.kind_label.chars().next())` uppercased). So `set_diagram_elements` takes `&Model`.

- [ ] **Step 1: Mount the `SelectBox` in the inspector DSL**

In `inspector_panel.rs`, the `element_bar := View{ … }` (~lines 78-81) currently reserves an empty strip. Host the box inside it. Add `use mod.widgets.SelectBox` to the inspector `script_mod!` `use` block, and give the bar the box as a child:

```rust
        element_bar := View {
            width: Fill
            height: 56.0
            align: { x: 0.0, y: 0.5 }
            padding: { left: 16.0, right: 16.0 }
            select_box := SelectBox { width: Fill }
        }
```

**Finding M1 — de-risk the child-widget event routing FIRST.** `SelectBox` is the *first* interactive child widget mounted in the inspector's nested, aligned `element_bar` — and the inspector's picker was hand-drawn precisely because child-widget events historically misbehaved in this aligned/offset panel (see the aligned-parent hit-rect gotcha the codebase already fought). A real `Widget` *should* route events here, but that is unproven in this exact mount. **Before investing in Steps 2–9, prove routing works:** temporarily add a `dbg!`/`log!` (or a throwaway `warning!`) in `SelectBox::handle_event`'s `Hit::FingerUp` arm, mount the box as above, and `/run` on a fixture. Click the box and confirm the log fires (i.e. the click reaches the child). If it does NOT fire, STOP and escalate the routing problem (the H2 event-time-area approach, or a window-overlay hit rect per the aligned-parent memo) before continuing — do not build the full data flow on a dead click. Once confirmed, remove the temporary log and proceed.

- [ ] **Step 2: Drop the hand-drawn field state + build helpers, keep `picker_ids`/`apply_pick`**

In the `Inspector` struct, remove the `picker_field_rect` field (the box owns its own hit rect now). Keep `picker_ids`, `elements`, `show_picker`, `subject`, badge fields (still used by the collapsed-body path and `apply_pick`).

Delete `picker_items` (the `PopupItem` builder) and `open_picker_request`, and replace the `InspectorAction::OpenPicker` variant. In `enum InspectorAction`, remove `OpenPicker { anchor, items }` (it becomes the box's own action; the inspector no longer emits it):

```rust
#[derive(Clone, Debug, Default)]
pub enum InspectorAction {
    #[default]
    None,
    Edited(String),
}
```

- [ ] **Step 3: Remove the hand-drawn picker field draw + click branch**

In `draw_walk`, delete the picker-field label draw and `picker_field_rect` assignment (~lines 453-472) and the `else` branch that cleared `picker_field_rect`. Keep the badge/pin/caret cluster if still wanted — BUT the badge + label are now the box's job, so remove the left-badge draw (~lines 399-413) and the picker-field label; keep pin + fold caret (they are inspector chrome, not the picker). The body-offset math (`bar_h`) is unchanged.

In `handle_event`'s `Hit::FingerUp` branch (~lines 306-323), delete the `if self.picker_field_rect.contains(p)` block that emitted `OpenPicker`. The box handles its own click now; the inspector no longer opens the picker.

- [ ] **Step 4: Build `SelectItem`s and feed the box in `set_diagram_elements`**

Add a private helper and change `set_diagram_elements` to take `&Model` and push items into the child box. Reach the box via `self.view.widget(cx, ids!(element_bar, select_box)).borrow_mut::<SelectBox>()`.

```rust
/// Build the picker rows as `SelectItem`s and record their id→index map (for
/// `apply_pick`). Node rows lead with a per-type badge and are enabled; edge
/// rows lead with the spline glyph (target-end label) and are disabled;
/// diagram rows are disabled. Index 0 (placeholder) is skipped.
fn build_select_items(&mut self, model: &Model) -> Vec<SelectItem> {
    self.picker_ids.clear();
    let mut items = Vec::new();
    for idx in 1..self.elements.len() {
        let row = self.elements[idx].clone();
        let id = LiveId::from_str(&row.key);
        self.picker_ids.push((id, idx));
        let selected = subject_to_index(&self.elements, &self.subject) == idx;
        let (lead, label, enabled) = match row.kind {
            ElementKind::Node => {
                let (color, letter) = model
                    .nodes
                    .iter()
                    .find(|n| n.key == row.key)
                    .map(|n| {
                        let letter = build_view(model, &Subject::Classifier(row.key.clone()))
                            .and_then(|v| v.kind_label.chars().next())
                            .map(|c| c.to_uppercase().to_string())
                            .unwrap_or_default();
                        (bucket_color(accent_bucket(&n.ty)), letter)
                    })
                    .unwrap_or((bucket_color(AccentBucket::None), String::new()));
                (SelectLead::Badge { color, letter }, row.label.clone(), true)
            }
            ElementKind::Edge => (
                SelectLead::Icon(Icon::Spline),
                edge_target(&row.label).to_string(),
                false,
            ),
            _ => (SelectLead::None, row.label.clone(), false),
        };
        items.push(SelectItem { id, lead, label, selected, enabled });
    }
    items
}

pub fn set_diagram_elements(&mut self, cx: &mut Cx, model: &Model, rows: Vec<ElementRow>) {
    self.elements = rows;
    self.show_picker = true;
    let items = self.build_select_items(model);
    let sel = subject_to_index(&self.elements, &self.subject);
    let sel_in_items = if sel == 0 { None } else { Some(sel - 1) };
    if let Some(mut b) = self
        .view
        .widget(cx, ids!(element_bar, select_box))
        .borrow_mut::<SelectBox>()
    {
        b.set_items(cx, items);
        b.set_selected(cx, sel_in_items);
    }
    self.view.redraw(cx);
}
```

Note `sel_in_items` maps the elements-index (which includes the index-0 placeholder) to the box's item index (placeholder-excluded), so it is `sel - 1` for a real selection.

Add imports at the top of `inspector_panel.rs`: `use crate::select_box::SelectBox;`, `use crate::popup::select::{SelectItem, SelectLead};`, `use crate::popup::base::PopupResult;`, and `build_view` to the existing `use crate::inspector::{…}` list.

- [ ] **Step 5: Sync the box selection in `set_subject`**

`set_subject` already recomputes badges; also push the new selection into the box so a pick made elsewhere (canvas/tree) re-marks the box. Append to `set_subject`, before the final `self.view.redraw(cx)`:

```rust
        let sel = subject_to_index(&self.elements, &self.subject);
        let sel_in_items = if sel == 0 { None } else { Some(sel - 1) };
        if let Some(mut b) = self
            .view
            .widget(cx, ids!(element_bar, select_box))
            .borrow_mut::<SelectBox>()
        {
            b.set_selected(cx, sel_in_items);
        }
```

- [ ] **Step 6: Add the inspector's open-request reader + close handler**

Replace the deleted `open_picker_request` with a forwarder over the child box, and add the close handler:

```rust
    /// Forward the child `SelectBox`'s open request (App relays it to
    /// `PopupRoot`). `None` unless the box asked to open this pass.
    pub fn take_open_request(
        &self,
        cx: &mut Cx,
        actions: &Actions,
    ) -> Option<(Rect, f64, Vec<SelectItem>)> {
        self.view
            .widget(cx, ids!(element_bar, select_box))
            .borrow::<SelectBox>()?
            .open_request(actions)
    }

    /// The flyout closed. Clear the box's active state; on a committed node pick
    /// repoint the inspector via `apply_pick`.
    pub fn on_picker_closed(&mut self, cx: &mut Cx, model: &Model, result: PopupResult) {
        let picked = self
            .view
            .widget(cx, ids!(element_bar, select_box))
            .borrow_mut::<SelectBox>()
            .and_then(|mut b| b.on_closed(cx, result));
        if let Some(id) = picked {
            self.apply_pick(cx, model, id);
        }
    }
```

(`take_open_request` needs `&mut Cx` for the `widget(cx, …)` lookup even though it only reads; keep the `&self` receiver — `self.view.widget` takes `&self`.)

- [ ] **Step 7: Update `App` call sites for `set_diagram_elements`**

In `app.rs`, the two `set_diagram_elements` calls (~lines 462 and 632) now pass the model. Line 462 becomes:

```rust
                    inspector.set_diagram_elements(cx, &self.model, rows);
```

Line 632 (empty rows) becomes:

```rust
                    inspector.set_diagram_elements(cx, &self.model, vec![]);
```

Both sites already hold a `borrow_mut::<Inspector>()` and `&self.model` is accessible in that scope — if the borrow checker complains about `&self.model` while `inspector` is borrowed from `self.ui`, bind the rows/model before the borrow, or clone the elements build. (The existing burger/menu relays already interleave `&self.model` with `self.ui.widget` borrows, so this pattern compiles; mirror whichever ordering they use.)

- [ ] **Step 8: Replace the App relay: `OpenRequested` → `PopupSpec::Select`**

In `app.rs`, replace the `open_picker_request` relay block (~lines 1152-1179) with a read of the inspector's forwarded box request → `PopupSpec::Select`:

```rust
        // Element-picker: the SelectBox asked to open its flyout. Only `App` may
        // place a cross-tree popup, so relay through `popup_root`.
        let open_request = self
            .ui
            .widget(cx, ids!(inspector))
            .borrow_mut::<crate::inspector_panel::Inspector>()
            .and_then(|inspector| inspector.take_open_request(cx, actions));
        if let Some((anchor_rect, min_width, items)) = open_request {
            let anchor = dvec2(
                anchor_rect.pos.x,
                anchor_rect.pos.y + anchor_rect.size.y + crate::popup::select::SELECT_GAP,
            );
            let bounds = self.window_bounds(cx);
            if let Some(mut pr) = self
                .ui
                .widget(cx, ids!(popup_root))
                .borrow_mut::<PopupRoot>()
            {
                pr.show_at(
                    cx,
                    PopupSpec::Select {
                        tag: live_id!(element_picker),
                        anchor,
                        min_width,
                        bounds,
                        items,
                    },
                );
            }
            return;
        }
```

(`take_open_request` takes `&mut Cx`; the `.borrow_mut()` on the inspector already holds `cx` — bind the request in a scope so `cx` is free for the subsequent `pr.show_at`, mirroring how the old block dropped its inspector borrow before touching `popup_root`.)

- [ ] **Step 9: Replace the picker-close handling**

In `app.rs`, the `picker_closed` handling (~lines 1064-1072) currently only handles `Invoked`. Route ANY close (Invoked OR Dismissed) into the inspector so the box never sticks "active":

```rust
            // Element-picker: any close (commit or dismiss) clears the box's
            // active state; a node commit repoints the inspector.
            if let Some(result) = picker_closed {
                if let Some(mut inspector) = self
                    .ui
                    .widget(cx, ids!(inspector))
                    .borrow_mut::<crate::inspector_panel::Inspector>()
                {
                    inspector.on_picker_closed(cx, &self.model, result);
                }
            }
```

(`picker_closed` is already bound at ~line 1022 as `pr.closed(actions, live_id!(element_picker))`, an `Option<PopupResult>` — keep that binding.)

- [ ] **Step 10: Build and fix fallout**

Run: `cargo build -p waml-editor`
Expected: clean after the renames. Likely fixups: remove the now-dead `PopupItem` import in `inspector_panel.rs` if unused; ensure `AccentBucket` is imported (it is, via `crate::node_style::{accent_bucket, AccentBucket}` — the current `use` already brings `accent_bucket`; add `AccentBucket` to it). Delete the `edge_target`/`picker_items` unit-test only if it referenced removed code (the `edge_target` fn is KEPT, so its tests stay).

- [ ] **Step 11: Run the full gate**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: all green. Now that the box is mounted and driven, the `#![allow(dead_code)]` in `select.rs`/`select_box.rs` no longer masks anything real — every symbol is on a live path.

- [ ] **Step 12: Verify end-to-end with `/run` on a diagram fixture**

Launch the native editor on a diagram fixture (use the `run` skill / `scripts/run-native.ps1` with a preset that has nodes AND edges). Confirm by observation:
  1. The picker bar shows a `SelectBox` at least as wide as the panel content, with the current subject's badge + label + caret.
  2. Clicking the box opens the flyout flush under it; the box draws an accent-active stroke while open.
  3. The flyout card is at least as wide as the box (min-width), node rows show per-type coloured badges + letters, edge rows show the spline glyph + target-end labels.
  4. The current selection has a subtle persistent fill + trailing check mark, distinct from hover.
  5. Hovering arms a row (accent); clicking a node row commits — the inspector repoints, the box re-marks it, the flyout closes, the box active-stroke drops.
  6. Clicking outside dismisses; the box active-stroke drops (never sticks). Edge/diagram rows draw dimmed and do not commit.
  7. Opening the burger/logo menu while the flyout is open supersedes it (flyout closes `Dismissed`, box active-stroke drops).

- [ ] **Step 13: Commit**

```bash
git add crates/waml-editor/src/inspector_panel.rs crates/waml-editor/src/app.rs
git commit -m "feat(inspector): host SelectBox, drop hand-drawn picker field + MenuPopup routing"
```

---

## Self-Review

**Spec coverage:**
- Unit 1 (item model) → Task 1 (`SelectLead`, `SelectItem`, `select_width` + tests). ✓
- Unit 2 (SelectFlyout surface) → Task 2 (DSL, widget, `draw` with `select_width` + label measure + per-lead render + selected fill/check, `Popup` impl, `open_select`). ✓
- Unit 3 (PopupRoot wiring) → Task 3 (`ActiveKind::Select`, `PopupSpec::Select`, `select :=` child, `show_at`/`route`/`reset` arms). ✓
- Unit 4 (SelectBox control) → Task 4 (widget, `SelectBoxAction`, state, `set_items`/`set_selected`/`open_request`/`on_closed`, active draw + tests). ✓
- Unit 5 (inspector consumer) → Task 5 (mount box, build `SelectItem`s, drop field/`OpenPicker`, App relay + close routing). ✓
- Testing section: `select_width` clamp (Task 1), `SelectBox` state via `decide_closed` (Task 4), `decide()` reuse (Task 3 confirms existing tests), rendering via `/run` (Task 5). ✓
- Error/edge handling: empty list opens a zero-row card (works — `items` empty → no rows drawn, outside press dismisses); supersede routes `Closed` into `on_picker_closed` (Task 5 Step 9 handles ANY result); off-screen clamp via `Presenter::place` (Task 3 Step 6); disabled rows dimmed + no commit (Task 2 render + MarkingCore `enabled` gate). ✓
- File touch list: all six files covered. ✓

**Type consistency:** `SelectItem`/`SelectLead` shapes identical across Tasks 1/2/4/5. `PopupSpec::Select { tag, anchor, min_width, bounds, items }` identical in Task 3 (def) and Task 5 (construction). `open_request`/`take_open_request` both return `(Rect, f64, Vec<SelectItem>)`. `on_closed(cx, PopupResult) -> Option<LiveId>` consistent Task 4↔5. `set_diagram_elements(cx, &Model, Vec<ElementRow>)` consistent Task 5↔App.

**Resolved deviations from the spec (grounded in code):**
1. **No catalog check glyph.** The `Icon` enum has no plain checkmark (only `PackageCheck`/`SaveCheck`). Rather than touch the load-bearing `Icon` order invariant, the selected-row mark is a small inline SDF `draw_check: DrawColor` in the flyout DSL (Task 2 Step 1). The box caret reuses the existing `Icon::ChevronsUpDown`.
2. **`MarkingCore` is `PopupItem`-based, not `SelectItem`.** It only reads `{id, enabled}`. The flyout keeps a parallel `Vec<SelectItem>` for rendering and feeds `MarkingCore` derived `PopupItem`s — `MarkingCore` is reused unchanged (Task 2 design note).
3. **`ElementRow` lacks type data.** Node badges need a model lookup (`model.nodes.find` → `bucket_color(accent_bucket(&ty))`), so `set_diagram_elements` gains a `&Model` param (Task 5 design note). The badge letter reuses `set_subject`'s existing `build_view(...).kind_label.chars().next()` derivation. This is why the old picker fell back to a generic glyph.
4. **`select_width` clamp is a standalone pure fn**, not a `LinearGeom` method — `LinearGeom::set_width` just stores the value. The spec's "extend `set_width_drives_panel_and_hit_edges`" is honoured in spirit: the clamp formula gets its own dedicated tests (Task 1) and the existing `set_width` test already proves the value drives `panel_rect`/`row_rect`/`row_at`.
5. **`picked`/`selected_id` dropped (YAGNI).** The spec's Unit-4 API listed `picked(actions)`; the inspector drives entirely through `on_closed`'s returned id + its own `apply_pick`, so the redundant reader is omitted. `open_request` + `on_closed` are the load-bearing API.
6. **Inspector mediates App↔box access.** The box is a grandchild of `App` (inside the inspector's `element_bar`); rather than reach it by a fragile cross-deref id-path from `App`, the inspector exposes `take_open_request` / `on_picker_closed` forwarders (matching the proven inspector-mediated convention the old `open_picker_request` used).

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-22-select-box-flyout.md`.
