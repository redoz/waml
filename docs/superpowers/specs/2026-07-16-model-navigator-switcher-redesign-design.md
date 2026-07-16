# Model Navigator — read-only switcher + three-state redesign

**Date:** 2026-07-16
**Status:** Approved for planning

## Problem

Clicking the TopBar diagram title jumps straight into the heavy model editor
(`NavigatorPanel`, either centered modal or left-docked rail). There is no light,
read-only "just switch diagrams" surface. The docked rail's pin currently only
swaps centered↔docked and never controls opacity, and it overlaps the canvas
tool `Dock`.

## Goal

Introduce a light **read-only dropdown** as the default click target, with two
escape hatches into the existing full editor (centered / docked). Unify the pin
so it means "force solid" on both the navigator and the inspector. Resolve the
docked-rail / tool-Dock overlap while the docked panel is being reworked.

## Three states

| State | Reached by | Surface |
|-------|-----------|---------|
| **Dropdown** (default) | click TopBar title button | read-only diagram list popover in TopBar |
| **Centered** | dropdown → Edit | full `NavigatorBody` editor in a centered modal |
| **Docked** | dropdown → Dock, or centered → stick-to-side | full `NavigatorBody` editor as a left rail |

There is **one** docked form — the full editor rail (today's rail). The dropdown
is only the lightweight entry point; docking always yields the full editor.

## Unit 1 — read-only switcher dropdown (TopBar)

Owner: `TopBar.svelte`. The existing center title button (`activeTitle` +
chevron) stops calling `onToggleNav`; instead it toggles a local
`switcherOpen` and renders a styled popover under itself — same anchoring
pattern as the existing Export menu in this file (invisible full-screen
click-catcher + absolutely positioned card).

Popover contents:
- **Diagram list** — the full `diagrams` array (curated diagrams + flow views +
  sequence views, already assembled in `CanvasInner`). Each row: a kind icon,
  the title, and a check on the active diagram. Row click → `onSelectDiagram(key)`,
  then close. Read-only: no rename / create / delete here.
- **Header controls** (top-right of the popover):
  - **Dock** — `PanelLeft` icon ("vertical tabs" affordance). Click → `onDockModel()`, close.
  - **Edit** — `Pencil` icon. Click → `onEditModel()`, close.

Dismissal: outside click (via the click-catcher) or `Escape`.

New `TopBar` props: `onSelectDiagram(key)`, `onDockModel()`, `onEditModel()`.
Removed prop: `onToggleNav`. `navOpen` prop is dropped from `TopBar` (the popover
owns its own open state; the button's `aria-expanded` reflects `switcherOpen`).

## Unit 2 — NavigatorPanel: centered + docked only

`NavigatorPanel.svelte` no longer owns the "click default." `mode` stays
`"centered" | "docked"`, reached only via the switcher.

- **Centered:** unchanged full editor + a stick-to-side (dock) button
  (`onToggleMode`, centered → docked) + close.
- **Docked:** full editor + **pin** (opacity, new) + collapse + close.
  - New props: `pinned: boolean`, `onTogglePin: () => void`.
  - Translucency becomes `mode === "docked" && !pinned && !engaged` — docked
    defaults to dims-when-idle (today's behavior); pinning forces it solid.
  - The former docked→centered toggle (PinOff) is **removed**. To return to
    centered, the user reopens from the switcher's Edit. This keeps the docked
    header to a single pin icon (no two pin-like buttons). Deliberate trade-off.

Docked position/overlap: see Unit 4.

## Unit 3 — pin flip (both panels)

Unify pin semantics so pinned = solid everywhere.

- Shared rule: `translucent = !pinned && !engaged`.
- **InspectorPanel.svelte:** invert the boolean (was `pinned && !engaged`).
  Swap the icon + labels: pinned now shows the "solid/pinned" state (`Pin`),
  unpinned shows `PinOff`; labels/titles updated to "keep solid" / "let it dim
  when idle". `aria-pressed` still reflects `pinned`.
- **CanvasInner.svelte:** default `inspectorPinned = $state(true)` so the
  inspector's current always-solid default is preserved — only the icon meaning
  inverts, not the out-of-the-box behavior.
- **Navigator docked:** default unpinned (`navPinned = $state(false)`) → dims
  when idle, matching today's docked default.

## Unit 4 — docked rail / tool-Dock overlap

The docked `NavigatorPanel` (`absolute top-3 left-3`, near-full-height) overlaps
the tool `Dock` (`left-[14px] top-[calc(50%-34px)]`, vertically centered on the
left edge). Fix: while the navigator is open **and** docked, slide the `Dock`
right by the rail's width — mirroring the existing pattern where the right-edge
flags slide left by `inspectorWidth`.

- `CanvasInner` already binds `navWidth`. Compute a left offset for the Dock:
  `navOpen && navMode === "docked" ? navWidth + gap : default`.
- Apply it via a prop/inline style on `Dock` (e.g. a `leftOffset` prop, or a
  wrapper), transitioning so it slides rather than jumps.

## CanvasInner wiring

- New `let navPinned = $state(false)`.
- `navMode` type unchanged (`"centered" | "docked"`).
- `TopBar` bindings: `onSelectDiagram` (switch active diagram + close, reusing
  the existing selection-reset logic in the current `onSelectDiagram`),
  `onDockModel` → `navOpen = true; navMode = "docked"`,
  `onEditModel` → `navOpen = true; navMode = "centered"`.
- `NavigatorPanel`: pass `pinned={navPinned}` and
  `onTogglePin={() => (navPinned = !navPinned)}`; drop the centered↔docked
  `onToggleMode` usage in docked mode (kept for centered's stick-to-side).
- `Dock`: pass the computed left offset.

## Testing

TDD, extending the existing co-located `.test.ts` files.

- **TopBar.test.ts:** clicking the title opens the popover; it lists every
  diagram/flow/sequence view with the active one checked; row click fires
  `onSelectDiagram`; Dock/Edit buttons fire `onDockModel`/`onEditModel`;
  outside-click and Escape close it; popover is read-only (no rename/create).
- **NavigatorPanel.test.ts:** docked shows pin + collapse + close (no
  center-toggle); `pinned` toggles the translucency class
  (closes the noted coverage gap); centered shows the stick-to-side + close.
- **InspectorPanel.test.ts:** pinned renders the solid state and is opaque;
  unpinned dims when idle (inverted from before); icon/label swap.
- **CanvasInner** (existing suites): docked Dock offset tracks `navWidth`;
  default inspector renders solid.

## Out of scope

- No slim read-only "vertical tab strip" rail (rejected: single full-editor rail).
- No persistence of pin/mode across sessions (stays session-local, as today).
- No changes to `NavigatorBody`'s editing behavior.
