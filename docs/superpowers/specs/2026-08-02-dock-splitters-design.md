# Movable dock splitters

## Problem

The two dock columns are fixed at compile time: `PROJECT_TREE_W = 280.0`
(`crates/waml-editor/src/tree_panel.rs:30`) and `INSPECTOR_W = 320.0`
(`crates/waml-editor/src/inspector_panel.rs:398`). Both flow through
`dock::responsive_layout` (`dock.rs:176`) into the slot widths applied at
`app/shell.rs:343`. A model with deep nesting overflows the tree column; a small
model wastes it. The user cannot change either.

Add a draggable splitter to each of the two vertical dock edges, and remember the
result per project.

## Scope

In scope: the left (Model tree) and right (Inspector) dock edges against the
center canvas, in wide mode.

Out of scope: horizontal splits inside a panel, a general resizable dock tree,
and resizing in narrow mode (where the panel floats over the center and its width
is already viewport-capped).

## Behavior

### Dragging

The splitter live-updates its panel's width while the width stays at or above
`min`. Between `collapse` and `min` the panel sticks at `min` while the pointer
keeps travelling — that gap is deliberate resistance before the snap.

### Collapse and reopen (hysteresis)

Dragging below `collapse` transitions the panel to `DockState::Flag`
**immediately, mid-drag** — not on release. `DockMotion` (`dock.rs:101`, 180ms
cubic ease-out) animates the slot shut and the center canvas reclaims the space
with the same animation.

Pointer capture is retained after collapse. Dragging back out past `reopen`
(strictly greater than `collapse` — the "and a bit") returns the panel to
`Pinned` and live width resumes. Release commits whatever state the drag is in.

The persisted width is the last **non-collapsed** width, so a later caption
toggle reopens the panel where the user left it rather than at the collapse
threshold.

### Maximum width

There is no fixed maximum. Per drag frame:

```
max_w = viewport_w - other_panel_slot_w - MIN_CENTER_W
```

Dragging the tree wide stops when the canvas reaches `MIN_CENTER_W`, and stops
earlier when the inspector is also pinned. Both panels stay reachable and the
canvas never reaches zero.

Persisted widths are stored **unclamped**. If a stored width exceeds today's
maximum it is clamped for display only, so widening the window restores the size
the user asked for. Re-clamping happens at layout time in `responsive_layout`,
not at drag time, so opening the other panel later cannot produce an
over-committed row.

### Affordance

A 1px theme rule sits at the panel edge at all times. A ~6px hit strip covers it.
On hover or drag the rule lerps to the accent colour and the cursor becomes
`MouseCursor::ColResize`. Nothing else is drawn — no grip, no ridges.

## Architecture

### 1. `splitter.rs` — pure geometry and drag state machine

A new module beside `dock.rs`, under the same discipline: no makepad types, unit
tested standalone.

```rust
pub struct DockLimits { min: f64, collapse: f64, reopen: f64 }
pub fn max_width(viewport_w: f64, other_slot_w: f64) -> f64;
pub enum DragOutcome { Width(f64), Collapse, Reopen(f64) }
pub fn drag(
    edge: DockEdge,
    limits: DockLimits,
    pointer_x: f64,
    viewport_w: f64,
    other_slot_w: f64,
    collapsed: bool,
) -> DragOutcome;
```

`drag` is a pure function of pointer position plus whether the panel is currently
collapsed. That `collapsed` flag is where the hysteresis lives: when false, a
width below `collapse` yields `Collapse`; when true, only a width above `reopen`
yields `Reopen`. `edge` flips the sign — the left panel's width is `pointer_x`,
the right panel's is `viewport_w - pointer_x`.

### 2. `DockSplitter` widget

A real 6px-wide child widget with `height: Fill`, mounted as the **last** child of
`left_slot` and the **first** child of `right_slot`. The panel body beside it
becomes `Size::Fill`.

Deliberately a real child rather than a `draw_abs` overlay: a child's hit rect
comes from its own turtle, which avoids the aligned-parent hit-rect offset
problem where a rect stored during `draw_walk` is pre-alignment while events
arrive post-alignment.

Consequence: the slot width includes the 6px strip, so the body draws 6px
narrower than the stored number.

Emits `SplitterAction::{Dragged(f64), Released}` carrying the raw pointer x in
window coordinates.

### 3. `project_settings.rs` — the `.waml/` store

```
<project>/.waml/settings.json
<project>/.waml/README.md
```

`settings.json`:

```json
{ "version": 1, "dock": { "tree_w": 280.0, "inspector_w": 320.0 } }
```

One `settings.json` to start rather than a file per concern; split it later if it
grows. The global `~/.waml/editor.json` (`config.rs`) keeps its current role for
user-level state such as theme and recents — this new file is strictly
project-scoped.

Reuses `config.rs`'s generic disk seam (`load_from` / `store_to`: atomic
temp-write plus rename, corrupt file preserved to `.bak`, never panics) by
promoting those two functions to `pub(crate)`. The seam is already
directory-injectable, so tests run against a temp dir.

`README.md` is written on first store only and never overwritten. It explains
what the directory holds and notes that most users will want to gitignore it. The
editor does **not** write a `.gitignore` — sharing the file is the user's
deliberate choice.

`read_bundle` (`load.rs:37`) currently takes every `*.md` under the project root
recursively, so `.waml/README.md` would be pulled into the source bundle and fail
analysis. `collect` (`load.rs:44`) must skip dot-directories. This is the correct
general rule for a project loader, not a workaround for this one file.

### 4. Shell wiring

At `app/shell.rs:343`, a runtime `self.dock_widths: DockWidths` replaces the two
constants passed at lines 348-349. Drag actions mutate it; `responsive_layout`
clamps it against the live viewport.

Everything downstream already follows: `left_slot`, `right_slot`, `tree_host`,
`inspector_host`, and `sync_tree_gap` (`shell.rs:496`), which keeps the caption
tab strip's left edge locked to the tree column's right edge — so the tabs track
the splitter live.

Collapse and reopen are routed through the existing `DockEvent::Close` and
`DockEvent::Open` transitions, so `DockMotion` animates the snap for free and
`DockState` remains the sole source of truth for open versus closed.

Persistence writes on `Released` only, never per drag frame.

### 5. Constants

| | tree | inspector |
|---|---|---|
| default | 280 | 320 |
| `min` | 180 | 220 |
| `collapse` | 140 | 170 |
| `reopen` | 200 | 240 |

`MIN_CENTER_W = 320`. All named and tunable in one place.

A project with no `.waml/settings.json` uses the defaults above. There is no
global fallback — new projects always start at 280/320.

## Data flow

```
FingerDown/Move on DockSplitter
  -> SplitterAction::Dragged(pointer_x)
  -> splitter::drag(...) -> DragOutcome
       Width(w)    -> dock_widths.<panel> = w
       Collapse    -> apply_dock_states(.., DockEvent::Close)
       Reopen(w)   -> apply_dock_states(.., DockEvent::Open); dock_widths.<panel> = w
  -> responsive_layout(narrow, viewport_w, motion values, dock_widths)
  -> slot / host walk widths -> redraw

FingerUp
  -> SplitterAction::Released
  -> project_settings::store(project_root, ..)
```

## Error handling

Disk failures follow the existing `config.rs` posture: a missing or unreadable
`settings.json` yields defaults; malformed JSON is renamed to `settings.json.bak`
and defaults are used; a failed write is logged and swallowed. Losing a panel
width is never worth an error dialog or a lost edit.

An unresolvable project root (an unsaved or in-memory model) simply skips
persistence; the drag still works for the session.

## Testing

Pure unit tests in `splitter.rs`:

- width clamps at `min` and at the dynamic maximum
- crossing `collapse` yields `Collapse`
- hysteresis in both directions: below `collapse` collapses, and while collapsed
  only a width above `reopen` reopens — a width between the two does neither
- `max_width` shrinks when the other panel is pinned
- left and right edges derive width from `pointer_x` with opposite signs

`project_settings.rs` against a temp dir: round-trip, missing file yields
defaults, corrupt file backs up and yields defaults, `README.md` written once and
not overwritten on a second store.

`load.rs`: `read_bundle` skips dot-directories.

Shell test: a synthesized drag moves `left_slot` / `right_slot` widths, and a
drag past the collapse threshold drives the panel to `Flag`.

Interactive per-pid visual sign-off is owed for cursor shape, hover accent, and
the collapse animation — none of those are visible to the gate.
