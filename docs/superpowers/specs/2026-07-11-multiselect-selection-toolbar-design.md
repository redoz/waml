# Multi-Select & Floating Selection Toolbar

**Date:** 2026-07-11
**Product:** Model Canvas (`packages/web`, React + React Flow + TypeScript)
**Scope:** 5 of 5 UI-change specs. Independent.

## Context

The canvas is **single-select only** today: selection is tracked as a `Selection`
union (node *or* edge) in `Canvas.tsx` (~line 157), set by `onNodeClick` /
`onEdgeClick` and cleared by `onPaneClick`. A `useEffect` (~`Canvas.tsx:161`)
auto-opens the Inspector when something is selected. There is **no marquee, no
shift-click accumulation, and no floating selection toolbar**. Delete-key handling
for the current selection is at `Canvas.tsx:307-315`.

New-diagram creation today spawns a diagram from *all current nodes* via
`store.addDiagram(name)` (`Canvas.tsx:511`, `state/model.ts:31` seeds members with
every node). There is no way to create a diagram scoped to a chosen subset.

## Goal

- Enable **full multi-select**: shift/ctrl-click to add to the selection and a
  **drag marquee** to box-select multiple elements.
- Show a **floating toolbar** anchored to the selection bounds whenever ≥1 element
  is selected. Its primary action is **New diagram from selection** (create a
  diagram seeded with exactly the selected nodes). Secondary: **Delete selection**.
- Selecting no longer auto-opens the Inspector (the Inspector opens via its flag —
  see right-edge-flags spec).

## Current state (concrete)

- Selection state + handlers: `Canvas.tsx` ~line 157 (`Selection`), `onNodeClick`
  / `onEdgeClick` / `onPaneClick`.
- Auto-open Inspector on select: `Canvas.tsx` ~line 161 (`if (selection)
  panel.open("inspect")`).
- Delete handling: `Canvas.tsx:307-315`.
- Diagram creation: `store.addDiagram` (`state/model.ts:31`, `state/diagrams.ts`).
- `Diagram.members` seeding: `state/model.ts:31` (all node keys today).

## Changes

### Multi-select (React Flow native)

- Enable React Flow's built-in multi-selection on the `ReactFlow` instance in
  `Canvas.tsx`:
  - `multiSelectionKeyCode` (Shift / Ctrl) for additive click selection.
  - `selectionOnDrag` + an appropriate `panOnDrag` config for **marquee box-select**
    (reconcile with the current pan/tool behavior in the Dock so panning still works —
    e.g. marquee on the select tool, pan on space/middle-drag).
- Replace the single `Selection` union with a **selection set** (arrays of selected
  node ids and edge ids), sourced from React Flow's `onSelectionChange` (or the
  nodes'/edges' `selected` flags).
- Update delete handling (`Canvas.tsx:307-315`) to remove **all** selected nodes
  and edges.

### Floating selection toolbar

- New `components/canvas/SelectionToolbar.tsx`, rendered when the selection set is
  non-empty. Position it at the **bounding box of the selected elements** (use
  React Flow coordinate helpers to convert graph → screen space; consider React
  Flow's `NodeToolbar` if it fits the multi-node case, otherwise a manually
  positioned overlay).
- Actions:
  - **New diagram from selection** — creates a diagram whose `members` are exactly
    the selected node ids, sets it active. This needs a store method that accepts
    explicit members (extend `addDiagram` to take an optional `members` array, or
    add `addDiagramFromMembers(name, memberIds)`), since today `addDiagram` seeds
    from *all* nodes.
  - **Delete selection** — removes the selected nodes/edges (shares the delete
    path above).
- Toolbar stays clear of the elements it acts on (offset above the bounding box);
  hides on empty selection.

### Selection no longer auto-opens Inspector

- Remove the `if (selection) panel.open("inspect")` effect (`Canvas.tsx` ~line
  161). The Inspector opens only from its flag (right-edge-flags spec). If that
  spec is not yet merged, this change can ship independently — the Inspector rail
  button still works.

## Edge cases

- **Mixed selection (nodes + edges):** *New diagram from selection* uses only the
  node ids (edges between included nodes come along implicitly via membership);
  disable the action if no nodes are selected.
- **Empty name / cancel:** creating a diagram from selection uses an inline name
  input; empty/whitespace rejected.
- **Selection during pan:** ensure marquee and pan don't conflict (tool-dependent).
- **Single selection:** toolbar still appears for one element (the "or more" case
  degrades gracefully to one).

## Out of scope

- The diagram title/switcher UI that lists/creates diagrams (spec:
  diagram-title-switcher) — this spec only adds the *from-selection* creation path
  and its store method.
- Inspector pin/translucent behavior (spec: right-edge-flags).

## Testing

- Unit: `addDiagramFromMembers` (or extended `addDiagram`) seeds `members` with
  exactly the passed ids; rejects empty name.
- Interaction: shift-click accumulates selection; marquee selects enclosed
  elements; delete removes all selected.
- Component: toolbar appears on non-empty selection at the selection bounds; "New
  diagram from selection" creates a diagram with the selected members and activates
  it; disabled when only edges are selected.
- Regression: selecting an element no longer auto-opens the Inspector.
