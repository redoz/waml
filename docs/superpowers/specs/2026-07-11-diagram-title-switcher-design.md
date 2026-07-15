# Diagram Title & Switcher

**Date:** 2026-07-11
**Status:** Implemented (commit `e0a012c` "diagram title switcher replaces goal
button + DiagramTabs"; centered blue title + dropdown verified in
`packages/web/src/components/TopBar.svelte:172-209`. No `GoalDialog`/`state/goal`
or `DiagramTabs` remnants found. The dropdown later evolved into the fuller
Navigator sheet, plans `2026-07-12` series — a superset of the spec's simple
list/rename/create dropdown.)
**Product:** Model Canvas (`packages/web`, React + React Flow + TypeScript)
**Scope:** 2 of 5 UI-change specs. Independent.

## Context

Two top-of-screen surfaces deal with "which diagram, what's it called":

- The top bar has a **Business Goal** button (`TopBar.tsx:78-87`, `Target` icon +
  "Business goal" / "Set business goal", blue when set) that opens `GoalDialog`
  (`packages/web/src/components/GoalDialog.tsx`, rendered `Canvas.tsx:491-498`).
  Goal state lives in `packages/web/src/state/goal.ts`.
- A floating **DiagramTabs** pill (`packages/web/src/components/canvas/DiagramTabs.tsx`,
  rendered `Canvas.tsx:507-512`) sits centered at the top of the canvas: one button
  per diagram plus a trailing `+` that prompts for a name and calls
  `store.addDiagram(name)` (`Canvas.tsx:511`).

We are collapsing all of this into a single **centered Diagram title** in the top
bar that doubles as the diagram switcher. The business-goal feature is removed
entirely.

## Goal

- Replace the Business Goal button with a **centered Diagram title** in the top
  bar. Keep the blue background treatment; drop the `Target` circle icon.
- The title is a **dropdown**: click to switch diagrams, rename the current one,
  and create a new (empty) diagram.
- Delete DiagramTabs and the business-goal feature.

## Current state (concrete)

- Business Goal button: `TopBar.tsx:78-87`; `goalSet` prop wired `Canvas.tsx:453`.
- Goal dialog + state: `GoalDialog.tsx`, `state/goal.ts`, wiring `Canvas.tsx:491-498`.
- DiagramTabs: `DiagramTabs.tsx`, rendered `Canvas.tsx:507-512`; create at line 511.
- Diagram switching: `activeDiagramKey` / `setActiveDiagramKey` in `Canvas.tsx`;
  `store.addDiagram` (`state/model.ts:31`, `state/diagrams.ts`).
- `Diagram` type: `packages/okf/src/types.ts:80-86`.

## Changes

### Remove business goal

- Delete `GoalDialog.tsx` and `state/goal.ts`.
- Remove the goal button block (`TopBar.tsx:78-87`), the `onOpenGoal` / `goal` /
  `goalSet` props, and the `GoalDialog` render + goal wiring in `Canvas.tsx`.

### Centered diagram title (top bar)

- In the slot the goal button occupied, render a centered **title control** for
  the active diagram. Blue background retained (reuse `bg-[#e6f1fb]
  text-[#1e88e5]` treatment); no icon.
- Shows the active diagram's `title` (or a sensible label for the implicit
  single-diagram case when `graph.diagrams` is empty).

### Title dropdown

Clicking the title opens a dropdown menu:

- **Diagram list** — every diagram in `graph.diagrams`; selecting one calls
  `setActiveDiagramKey(d.key)`. Active diagram is checkmarked.
- **Rename current** — inline edit (or a small input) that updates the active
  diagram's `title` in the store. Replaces the old prompt-based flow.
- **+ New diagram** — creates an empty diagram (`store.addDiagram(name)`), sets it
  active. Replaces the DiagramTabs `+`. Name via inline input, not `window.prompt`.

### Remove DiagramTabs

- Delete `DiagramTabs.tsx` and its render at `Canvas.tsx:507-512`.

## Edge cases

- **Implicit diagram:** when `graph.diagrams` is empty, the title shows a default
  label (e.g. the model name or "Untitled diagram") and the list shows just that
  implicit entry. "+ New diagram" materializes the first real diagram.
- **Rename to empty:** reject empty/whitespace titles; keep the previous title.
- **Single diagram:** switcher still works (list of one); rename + new available.

## Out of scope

- "New diagram from selection" (spec: multiselect-selection-toolbar) — that path
  seeds members from the selection; this spec's "+ New diagram" is empty/current-nodes.
- Per-diagram render settings (spec: diagram-properties-panel).

## Testing

- Unit: rename updates `title`; empty rename rejected; new diagram becomes active.
- Component: dropdown lists all diagrams, checkmarks active, switches on click;
  goal button and DiagramTabs no longer render.
- Regression: removing `state/goal.ts` leaves no dangling imports (grep).
