# Start-screen recents card: immediate-mode → makepad layout engine

**Date:** 2026-07-19
**Status:** Approved (design)
**Scope:** `crates/waml-editor/src/start_screen.rs` (+ one new `recent_row.rs`, widget
`RecentRowView`). Only the
launcher/start-screen card. Sibling immediate-mode widgets (`tool_dock.rs`, `canvas.rs`,
`doc_tabs.rs`, `inspector_panel.rs`) are **not** touched.

## Problem

`start_screen.rs` hand-rolls the launcher card in immediate mode: the card body (header +
recents list + two action buttons) is drawn as absolute `Rect`s, and every position is
manual pixel math — card centering, the pane split, header vertical centering (`+ 20.0`,
`+ 54.0`), per-row `y += ROW_H`, and the last-opened timestamp right-anchored by measuring
its width with `waml::solve::sizing::text_width`. Any font-size or content change forces a
new round of pixel tuning. The user wants the card rebuilt on the makepad layout engine so
alignment and sizing fall out of `flow` / `align` / `Fill`, not arithmetic.

Immediate mode was a *deliberate* original choice for this widget (colocated draw + hit-test,
per its module docstring) and remains correct for the canvas (custom UML nodes, zoom, ripple).
It is the wrong tool for a standard card UI. We refactor **only** the card.

## Key finding: FlatList is the fork's blessed dynamic list

The pinned makepad fork (`redoz/makepad` rev `4f9ce7a`) ships two list widgets:

- `portal_list.rs` (~106K) — virtualized/recycling scroller (thousands of rows, flick,
  pull-to-refresh). Overkill for a ≤6-row recents list.
- `flat_list.rs` (~7K) — **lightweight non-virtualized list**. Declares row *templates* in
  the DSL; at draw time you loop your data, call `list.item(cx, item_id, id!(Template))` to
  get-or-create a real child widget per row, push data in (`set_text`), and `draw_all`. It
  owns flow layout + scrollbars; drives child events; bubbles child actions up grouped.

`flat_list.rs` is the correct vehicle: rows become real flow-layout widgets, so font-size or
content changes reflow automatically — no `ROW_H`, no measure, no `y + N`.

**Reference consumer** (exact idiom we copy): `old/studio/src/run_list.rs` in the fork.

## Architecture

`StartScreen` becomes a container widget deref'ing a `View` (the `inspector_panel.rs`
pattern), with the card shell declared as a `script_mod!` `View` tree and the recents list as
a `FlatList` inside it. Only the dynamic recents rows go through `FlatList`; the fixed card
chrome and the two action buttons are static tree nodes.

### Widget field shape

```rust
#[derive(Script, ScriptHook, Widget)]
pub struct StartScreen {
    #[deref] view: View,          // card shell + FlatList, from the DSL tree
    #[rust] rows: Vec<RecentRow>, // render-copy of config recents (unchanged struct)
    #[rust] visible: bool,        // self-managed show/hide (unchanged semantics)
    // btn_new / btn_open move INTO the DSL tree as static WamlButton children;
    // draw_* immediate-mode fields (draw_title/draw_dim/draw_marker/…) are removed
    // except any still needed for chrome drawn manually over the View (see below).
}
```

### DSL tree (card shell)

```
StartScreen = View  (width Fill, height Fill)   // full-window backdrop keeps its gradient bg
  card := View  flow:Down                       // centered; align on the parent centers it
    header := View  flow:Right  align:{y:0.5}    // logo (Fit) + subtitle column (Fill)
    rule  := <hairline>                          // header divider
    body  := View  flow:Right                    // recents (Fill) | actions (Fixed 260)
      recents_col := View  flow:Down             // eyebrow "RECENT" + list frame
        recents_list := FlatList  flow:Down
          Row := mod.widgets.RecentRowView { }   // the row template (new widget, below)
      actions_col := View  flow:Down  (width 260) // eyebrow "START" + two buttons
        btn_new  := mod.widgets.WamlButton { }
        btn_open := mod.widgets.WamlButton { }
```

Card centering: the outer `StartScreen` view centers `card` via `align:{x:0.5, y:0.5}`; `card`
height is `Fit` so a short list never strands it. This replaces the `card_h` /
`(rect.size - CARD_W)*0.5` math. `CARD_W` survives as the card's fixed width; `RIGHT_PANE_W`
(260) survives as `actions_col` width.

### The new `RecentRowView` widget (`recent_row.rs`)

A dedicated small widget (named `RecentRowView` to avoid colliding with the existing
`RecentRow` render-copy struct) — a two-line, marker-led, right-anchored-timestamp row is more
than a `WamlButton` should carry, and it must be individually clickable.

```
RecentRowView = View  flow:Right  align:{y:0.5}
  marker := <accent square>            // Fixed (leading node-marker)
  textcol := View flow:Down            // width: Fill  ← this is what right-anchors the time
    title := <DrawText>                // draw_title style
    path  := <DrawText>                // draw_dim style
  when  := <DrawText>                  // width: Fit   ← Fill on textcol shoves it flush-right
```

The `Fill` on `textcol` consumes all slack, so `when` sits hard against the right edge with
**zero measurement** — this deletes the `sizing::text_width` call. `align:{y:0.5}` vertically
centers the timestamp against the title with no `y + 14.0`.

`RecentRowView` is a `#[deref] View` that:
- exposes setters the parent calls per row: `set_title`, `set_path`, `set_when` (via
  `self.view.label(cx, ids!(...)).set_text(...)`), plus a hover wash toggle;
- on `Hit::FingerUp` over its own area, emits a `RecentRowAction::Clicked` widget-action and
  sets `MouseCursor::Hand` on hover (so the parent needs no per-row hit rects).

### Draw drive (`StartScreen::draw_walk`)

Copy the `run_list.rs` interpose idiom exactly:

```rust
fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
    if !self.visible { return DrawStep::done(); }
    while let Some(item) = self.view.draw_walk(cx, scope, walk).step() {
        if let Some(mut list) = item.as_flat_list().borrow_mut() {
            for (i, row) in self.rows.iter().enumerate() {
                let item_id = LiveId::from_str(&row.path);          // stable per recent
                let mut roww = list.item(cx, item_id, id!(Row)).unwrap();
                roww.as_recent_row_view().set_row(cx, row, self.hovered == Some(i));
                roww.draw_all(cx, &mut Scope::empty());
            }
            // Empty state: when self.rows.is_empty(), populate exactly one Row from a
            // reserved placeholder item_id with title = "No recent projects", empty
            // path/when, and its clickable flag cleared, then draw_all it. (One code
            // path — no separate placeholder tree node to keep visible/hidden.)
        }
    }
    DrawStep::done()
}
```

The full-window gradient backdrop and the ambient card shadow (`CardShadow`) are the only
chrome still drawn abs — the shadow needs the card's post-layout rect, so it is drawn after
the tree is laid out, from `self.view.area().rect(cx)` (mirrors how `inspector_panel.rs` reads
`self.view.area().rect(cx)` after `draw_walk`). `SHADOW_PAD` / `SHADOW_DROP` survive.

### Event routing (`WidgetMatchEvent::handle_actions`)

Replaces `hot_rects` + `Rect::contains` entirely.

```rust
impl WidgetMatchEvent for StartScreen {
    fn handle_actions(&mut self, cx, actions, _scope) {
        let list = self.view.flat_list(cx, ids!(recents_list));
        for (item_id, item) in list.items_with_actions(actions) {
            if item.as_recent_row_view().clicked(actions) {
                // map item_id → row index → emit StartScreenAction::OpenRecent(i)
            }
        }
        // buttons keep their existing action path (WamlButton press/release/ripple),
        // now read via the standard button-clicked convention rather than hot_rects.
    }
}
```

`btn_new` / `btn_open` press-ripple: `WamlButton` already owns its ripple + next-frame loop.
As tree children their `handle_event` is driven by the container `View`; the manual
`tick`/`press`/`release` plumbing in the current `handle_event` is removed, and click intent is
read from their emitted actions.

## What dies / survives

**Deleted:** `ROW_H`, `VISIBLE_ROWS` (as height math), `BTN_H`, `BTN_GAP`, `PANE_PAD` (as
math), `BODY_PAD`, `ROW_MARGIN`/`ROW_PAD` (as offsets), `EYEBROW_H`, `MARKER` offset math,
`HEADER_H`/`LOGO_*` y-offsets, every `y + N` / `+ 20.0` literal, the `sizing::text_width`
import and call, `hot_rects`, `hovered: Option<Hot>` as a rect key (becomes row index),
`Hot` enum, the manual `btn_new.tick/press/release` plumbing.

**Survives:** `CARD_W`, `RIGHT_PANE_W`, `SHADOW_PAD`, `SHADOW_DROP`, the `RecentRow`
render-copy struct (feeds the new `RecentRowView` widget), `StartScreenAction` (+ `OpenRecent`/`NewProject`/`OpenProject`), `set_recents`,
`set_visible`, `screen_action`, the gradient `draw_bg` + `CardShadow` shaders, the `HudFrame`
card + list frames, the Atlas theme wiring.

## Testing

- Keep the existing unit test (`default_action_is_none`); add a `RecentRow` render-struct
  round-trip test if there's non-trivial formatting logic (there isn't much — mostly setters).
- The layout itself is verified by running the editor (there is no headless render test harness
  in this crate — every sibling widget is verified the same way).

## Manual verification (the acceptance check)

`cargo run -p waml-editor --bin waml-editor` → start screen shows →
1. Recents rows: project name left, path on the line below it, timestamp flush-right and
   vertically aligned with the name — **all from layout, no magic offsets**.
2. Buttons normal-sized (30px), sentence-case labels.
3. Click a recent row / New / Open → the corresponding `StartScreenAction` fires (routing
   survived the tree move).
4. Bump a font_size in the DSL by a few pt → rows/timestamp reflow correctly with **no** code
   change (the whole point of the refactor).
5. No new build warnings beyond the 2 known benign makepad dup-package warnings.

## Risks & mitigations

- **FlatList draw-drive wiring** (primary risk) — the interpose loop, `as_flat_list()`,
  `item()`, `draw_all` must be wired exactly. *Mitigation:* copied verbatim from the fork's own
  `run_list.rs` consumer; implement + run this first as the thin vertical slice before styling.
- **`RecentRowView` click bubbling through `items_with_actions`** — child action must be grouped so
  the parent sees it. *Mitigation:* `FlatList::handle_event` already wraps each item in
  `cx.group_widget_actions`; `run_list.rs` reads `CheckBox::changed` this way — same path.
- **Shadow needs post-layout card rect** — drawn from `self.view.area().rect(cx)` after the
  tree lays out, per the `inspector_panel.rs` precedent.
- **`WamlButton` as a tree child** — it was built for immediate-mode `draw_at`. If its
  `Widget::draw_walk`/`handle_event` don't behave as a tree child, fall back to keeping the two
  buttons immediate-mode inside `actions_col` (drawn abs from the laid-out column rect). This is
  a localized fallback that does not affect the recents refactor.
- **Makepad fork shader gotcha** — `sdf.box(...,0)` floods; the row hover / marker shaders must
  use `sdf.rect` for sharp corners (existing card shaders already do).

## Out of scope

Scroll behavior for long recents lists (FlatList supports it, but not needed this cut), the
parked cool-palette comment block in `theme_atlas.rs` (leave intact), and the `waml_button.rs`
font tweaks already in the worktree diff (leave as-is unless the tree-child fallback requires
touching them).
