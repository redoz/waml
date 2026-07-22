# HUD Turtle Migration — ProjectTree Header

**Date:** 2026-07-23
**Status:** Approved (converged in brainstorming)
**Scope:** `crates/waml-editor/src/tree_panel.rs` header band only.

## Problem

The editor's HUD panels grew a class of layout bugs from absolute
positioning. Interactive glyphs and text are drawn with `DrawColor::draw_abs`
/ `IconSet::draw` into hand-computed `Rect`s captured during `draw_walk`, then
hit-tested in `handle_event` against those same cached rects. Because the rects
are captured **pre-alignment** but pointer events arrive **post-alignment**,
every hit path needs a manual `hit_off` translate (memory
`aligned-parent-hit-rect-offset`), and any layout tweak means re-deriving pixel
offsets by hand. redoz@: *"a lot of layout issues because of all the absolute
stuff."*

`ToolDock` and `Inspector` have already been converted to the target pattern
(shared `IconButton` DSL children laid out by a `View`'s `flow`). `ProjectTree`
is the **last** panel still carrying a fully abs-drawn overlay header:
`tree_panel.rs:590-740` draws the title trigger, pin glyph, collapse glyph,
search field, and type chip absolutely; `handle_event:800-835` hit-tests six
cached rects (`header_rect`, `title_rect`, `collapse_rect`, `pin_rect`,
`search_rect`, `chip_rect`) through `hit_off`.

## Goals

- Kill the abs-positioning bug class in `ProjectTree`'s header.
- The two interactive **glyph** controls — collapse and pin — become real
  shared `IconButton` children that own their own hover/click/active state and
  hit-test their own `view.area()`. Drops their manual hit rects and the
  `hit_off` dance for those two controls.
- The header becomes a real laid-out `View` (`flow: Down`) stacked above the
  `FileTree` body, not an overlay drawn over the panel's whole area.
- Visual parity: the header renders identically; every click target
  (title/scope trigger, collapse, pin, search field, type chip) plus the
  just-shipped filter dropdown still land.
- `cargo test -p waml-editor` stays green (309+; no test asserts layout).

## Non-Goals

- No change to `Inspector` or `ToolDock` (already migrated — they are the
  reference templates, not targets).
- No widget-ification of the bespoke composite controls (scope-title
  trigger, editable search field, type-filter chip). They stay immediate-mode.
  See "Rejected — Approach B."
- No behavior change: same actions (`ScopeRequest`, `Query`, `FilterRequest`),
  same glass/opacity easing, same `Elsewhere`/`Empty` notes, same row glyphs.
- No new public API on `ProjectTree` beyond what the migration requires.

## Chosen Approach — A (turtle-laid header + `IconButton` glyph children)

Two-part, matching the `Inspector::element_bar` precedent
(`inspector_panel.rs:90-99`), where a real `View` hosts `IconButton` children
**plus** bespoke immediate-mode draws in the same bar:

1. **Interactive glyphs → `IconButton` children.** `collapse_btn` and
   `pin_btn` become `IconButton {}` children declared in the header View's DSL,
   laid out by `flow`. Their glyph + lit state are pushed each `draw_walk`
   (`set_icon` / `set_active`), and their clicks are read from `Event::Actions`
   via `IconButton::clicked` — exactly as `ToolDock` drives its five buttons
   (`tool_dock.rs:163-176, 182-186`) and `Inspector` drives `pin_btn`/`fold_btn`.
   `IconSet::draw` already takes a rect, so the glyphs need **no** wrapping —
   the widget renders itself.

2. **Bespoke composites stay immediate-mode**, but drawn into the header
   View's **own turtle area** rather than the panel's full rect. The
   scope-title trigger (label + `⌄`), the editable search field (glass pill +
   magnifier + caret text), and the type-filter chip (glass pill + leading
   glyph + label + chevrons) remain hand-drawn — they are composite, stateful,
   and not glyph-shaped, so wrapping them buys nothing. Their hit rects are
   still captured in `draw_walk` and tested in `handle_event`, but relative to
   the header View's area, keeping the existing `ScopeRequest` / `Query` /
   `FilterRequest` action seams unchanged.

### New widget tree (DSL)

The current spacer `header := View { width: Fill, height: 64 }` becomes a real
`flow: Down` container with two rows:

```
header := View { flow: Down, height: 64
    title_row := View { flow: Right, height: 34, align:{y:0.5}
        // bespoke title trigger drawn into the leading turtle space,
        // then a Fill spacer, then the glyph cluster:
        collapse_btn := IconButton {}
        pin_btn := IconButton {}
    }
    search_row := View { flow: Right, height: 30, align:{y:0.5}
        // bespoke search field (Fill) + type chip drawn into this row
    }
}
```

Exact child ordering / spacer mechanics are an implementation detail for the
plan; the constraint is: the two `IconButton`s are real DSL children in a
`flow` View, and the bespoke draws target their row's turtle rect. `IconButton`
default is 32×32; the header uses `icon_size` / box sizing consistent with the
inspector's bar buttons.

### `draw_walk` changes

- Drop the abs draws of the pin glyph and collapse glyph
  (`tree_panel.rs:607-629`). Instead, before the header View lays out, push
  each button's glyph + lit:
  - `collapse_btn.set_icon(ListExpand if collapsed else ListCollapse)`
  - `pin_btn.set_icon(Pin if pinned else PinOff)`; `pin_btn.set_active(pinned)`
    so a pinned panel reads lit (matches inspector).
- The title trigger, search field, and chip draws stay, but anchor off the
  header / row View area rects (captured from `.area().rect(cx)`), not off the
  panel rect + hand offsets.
- Keep the glass-opacity push into `draw_field_bg` (the pills still ease with
  the panel).
- `note_band` and the `Elsewhere` / `Empty` / `Browse` / `Results` notes are
  untouched.

### `handle_event` changes

- Let the header View handle its children first (`self.view.handle_event`
  already runs), then read `collapse_btn.clicked(actions)` /
  `pin_btn.clicked(actions)` from the `Event::Actions` arm — toggling
  `collapsed` / `panel.toggle_pin` there. Remove the `pin_rect` /
  `collapse_rect` `contains(p)` branches and the two fields.
- The title / search / chip branches remain in the `Hit::FingerUp` arm, still
  tested against their captured rects. `hit_off` stays **only** for those
  bespoke rects (they are still hand-drawn); it is no longer needed for the two
  glyphs.

### Fields removed / added

- Remove: `pin_rect`, `collapse_rect` (`#[rust]` fields), plus the `IconSet`
  abs-draw of pin & collapse.
- Keep: `search_rect`, `chip_rect`, `title_rect`, `header_rect` (bespoke draws
  still need them), `draw_title`, `draw_dim`, `draw_field_bg`, `icons`, glass
  `panel`.
- The `IconButton` children are addressed via `self.view.widget(cx, ids!(...))`
  / a small `WidgetRef` helper, mirroring `ToolDock::button`.

## Rejected — Approach B (full child-widget tree)

Convert *every* header element — title trigger, search field, chip — into
bespoke child widgets too. Rejected as the total endpoint: multi-week rewrite
for marginal gain. `IconSet` is a rect-draw helper, not a widget (no
`draw_walk` / `Walk`), so the composite controls would each need a new wrapper
widget with its own event plumbing (text editing, popup anchoring). Approach A
converts exactly the two elements that map cleanly onto the existing shared
`IconButton` and leaves the composites as the same immediate-mode hybrid the
inspector already ships.

## Testing & Verification

- **Unit:** `cargo test -p waml-editor` stays green. Existing tests cover tree
  data + icon map + `note_band_height`, not layout — so they guard against
  regression in the untouched logic, not the visual change.
- **Visual parity (manual, per-dedicated-pid):** launch a dedicated editor
  instance from the worktree's own `run-native`, screenshot by that specific
  pid (memory `screenshot-verify-hits-user-editor` — never grab the user's open
  editor, never kill-all). Confirm: header renders identically; title / scope
  trigger opens the scope dropdown; collapse toggles the body; pin toggles glass
  + reads lit; search field takes focus + edits; type chip opens the filter
  `SelectFlyout`; the shipped per-kind filter dropdown still lands.
- **Collapsed state:** header stays, body hides, frame hugs the header — same as
  today.

## Risks & Gotchas

- **Never hand-draw an `IconButton` in a manual `begin_turtle`** — no widget in
  this repo does; buttons must be `flow`-laid DSL children of a View. This is
  what killed the earlier abs-turtle-overlay WIP.
- **`Flow::Right` cross-axis (y) align is per-child; main-axis (x) is
  whole-block** (memory `makepad-turtle-align-shifts-whole-block`) — the glyph
  cluster's vertical centering is per-child, horizontal packing is the block.
- **Pre / post-alignment hit rects** (memory
  `aligned-parent-hit-rect-offset`): the point of moving the glyphs to
  `IconButton` is that real child widgets own their `view.area()`, making
  `hit_off` moot for them. The remaining bespoke rects still need it (the panel
  is left-aligned so `hit_off ≈ 0`, but keep the translate per the memory).
- **Stay in the worktree**, integrate per-chunk, push origin/main (memories
  `always-work-in-worktree`, `integrate-often-push-origin`). `run-native`
  builds `$PSScriptRoot`'s checkout — launch the worktree's own copy.
- Layout has no unit-test coverage → parity is visual-only. Preserve current
  rects where practical so the header looks byte-identical.
