# Docked, collapsible panels (Model + Inspector)

**Date:** 2026-07-24
**Status:** Approved design, ready for implementation plan.

## Problem

The Model (project tree) and Inspector panels currently **float** over a
full-width `GraphCanvas` in a `Flow::Overlay` body (`app.rs` ~line 195),
rendered translucent by `panel_glass`. Floating panels waste the "glass over
diagram" trick, permanently occlude canvas corners, and force aux widgets
(`tool_dock`, `constraint_toggle`, `conflict_badge`, `selection_toolbar`) to
hard-code margins past the tree (`margin left:304`, `right:344`) — a brittle
coupling to the tree's pixel width.

Goal: replace floating panels with **traditional docked panels that collapse to
a thin vertical flag** at the body edge.

## State model

Each dock panel (left = **Model** / tree, right = **Inspector**) has three
visual states, driven by one enum that replaces today's separate
`collapsed` / `pinned` bools:

```
enum DockState {
    Flag,    // resting: thin sideways-label strip at the edge, no body drawn
    Peek,    // unpinned + expanded: body floats over center, auto-collapses
    Pinned,  // docked column: consumes layout, center shrinks, sticky
}
```

Transitions:

| From    | Trigger                                   | To      |
|---------|-------------------------------------------|---------|
| Flag    | hover flag, or click flag                 | Peek    |
| Peek    | pointer leaves flag **and** body + ~600ms | Flag    |
| Peek    | click header pin button                   | Pinned  |
| Pinned  | click header pin (unpin) / collapse       | Flag    |
| Pinned  | (never auto-collapses)                    | —       |

Rationale (from brainstorming): auto-collapsing a *docked* column would reflow
the diagram randomly under the user = confusing. So **unpinned panels must peek
as an overlay** (canvas frozen), and **only pinning promotes to a real column**
that shrinks the active view. Pin therefore flips the whole layout mode, not
just opacity.

## Layout architecture (Approach A — `flow: Right` split)

Body inner goes from `flow: Overlay` to:

```
peek_overlay := View { flow: Overlay              // top wrapper
    dock_row := View { flow: Right
        left_slot  := View { width: Fit }         // ModelPanel: 28px flag OR 280 column
        center_stack := View { width: Fill  flow: Overlay
            canvas, source_view,
            tool_dock_wrap, constraint_toggle_wrap,
            conflict_badge_wrap, selection_toolbar // RELOCATED here
        }
        right_slot := View { width: Fit }          // Inspector: 28px flag OR 320 column
    }
    peek_layer := View { flow: Overlay }           // Peek bodies draw here — zero layout cost
}
```

- **Slot width** is driven by `DockState`: `Flag` → 28px flag strip;
  `Pinned` → full panel width (280 / 320). Because `center_stack` is `Fill`,
  pinning a panel shrinks the center automatically (canvas / active tab reflows
  into the remaining width — no manual margin math).
- **Peek body** renders in `peek_layer` (an Overlay sibling above `dock_row`)
  at the flag's inner edge, so it overlaps the center **without consuming
  width**. The flag strip stays visible beside it as the spine.
- **Do NOT** use `draw_abs` past the narrow slot for the peek body — that hits
  the aligned-parent hit-rect offset bug (`draw_abs` rects stored pre-alignment,
  events arrive post-alignment → dead clicks). The dedicated `peek_layer` avoids
  it.

## Flag widget

Thin strip: `width: 28`, `height: Fill`, at the body edge. Contents, top-down:
icon (16px, ~12px from top) then a **rotated 90° label** in accent ink.

- Icons (ported from Lucide, see below):
  - Left / Model → `list-tree` (hierarchical structure — "the model is a tree")
  - Right / Inspector → `inspection-panel`
- Whole strip is **one hit target**: hover or click → `Peek`. Subtle accent
  wash on hover so it reads as pressable (reuse the `icon_button` wash idiom).
- Label text: "Model" (left), "Inspector" (right). If text rotation is painful
  in the fork's `DrawText`, fall back to a per-glyph vertical stack — decide at
  impl.

### Icon port (new glyphs)

`list-tree` and `inspection-panel` are not yet in `icons.rs`. Port via the
existing pipeline:

```
cp c:/dev/vendor/lucide-icons/.../list-tree.svg        resources/icons/
cp c:/dev/vendor/lucide-icons/.../inspection-panel.svg resources/icons/
python scripts/gen-icon.py resources/icons/list-tree.svg        # prints DSL body
python scripts/gen-icon.py resources/icons/inspection-panel.svg
```

Paste each printed `Icon<Name>` DSL body into `icons.rs`, then add the glyph in
**every** parallel list, preserving the invariant
`enum == field == DSL == get == ALL == label` order and bumping the counts
(memory: `keep-unused-catalog-icons`). Sanity-check size/stroke in `icon_harness`.

## panel_glass cleanup

- `PanelGlass` loses `pinned` (subsumed by `DockState`); keeps only opacity
  easing, used **for the Peek body**. `Pinned` = forced opaque; `Flag` = no body
  drawn.
- Repoint the existing header `IconButton`s: the **pin** button toggles
  `Peek ↔ Pinned`; the **collapse** (unpin) button goes straight to `Flag`.
  Today's body-fold-only semantics retire.
- Update the now-false module docs in `panel_glass.rs`, `tree_panel.rs`,
  `inspector_panel.rs` — they describe "float over the graph canvas
  (app `flow: Overlay`)", which no longer holds.

## Aux floater relocation

Move `tool_dock_wrap`, `constraint_toggle_wrap`, `conflict_badge_wrap`,
`selection_toolbar` out of the body Overlay and into `center_stack`. Their
`align` now anchors to the **real canvas rect**. Delete the hard-coded
`margin left:304` / `right:344`; they become `left:12` / `right:12` relative to
center and auto-track dock state for free.

## Testing / verification

- **Unit:** `DockState` transition table (Flag→Peek→Pinned→Flag); auto-collapse
  timer arm/cancel as a pure dt function (testable without a live `Cx`, same
  `NextFrame` dt pattern as `panel_glass`).
- **Layout:** assert center `Fill` rect shrinks by exactly the slot width when a
  panel pins; assert aux floaters anchor to the center rect (regression guard on
  the retired `margin:304`).
- **Manual visual pass (mandatory):** per-pid screenshot only, never kill-all
  (memory: `screenshot-verify-hits-user-editor`). Check flag legibility, peek
  slide-out + auto-collapse, pin reflow, and **both panels pinned at once**.

## Out of scope

- Overlay-opaque variant of the dock model (Approach B) — parked; may revisit.
- Top/bottom docks, drag-to-resize panel width, drag-to-reorder — not requested.
- Real Markdown in the Source view (separate deferred follow-up).
