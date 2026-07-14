# Diagram Properties Panel

**Date:** 2026-07-11
**Status:** Implemented (2026-07-12, via `central-edit-panel-implementation` and
`floating-collapsible-inspector` plans — see `plans/completed/`). Stack migrated
from React to Svelte 5 in the interim; file paths below are historical.
**Product:** Model Canvas (`packages/web`, React + React Flow + TypeScript)
**Scope:** 1 of 5 UI-change specs. Independent; no dependency on the others.

## Context

The left floating dock (`packages/web/src/components/canvas/Dock.tsx`) currently
holds an **ERD view** toggle button (`Dock.tsx:245-261`) that flips a global
`viewMode` between `"compact"` and `"erd"`. That mode is a per-browser
localStorage preference (`packages/web/src/state/viewMode.ts`), read in many
places in `Canvas.tsx` (`toRFNode` line 102, `runDagreLayout` line 110,
`buildRfEdges` line 205, `erdAwareNodeSize`). A second per-browser preference,
`relLabelMode` (`packages/web/src/state/relLabels.ts`, `"all" | "hidden"`), plus
an `emphasizeMultiplicity` flag, control association-edge labels.

These are **global, browser-local** toggles. They should instead be **per-diagram
render settings** that live on the diagram and travel with the model.

## Goal

Replace the ERD toggle with a **"Diagram properties"** dock button that opens a
panel for configuring how the *active diagram* renders: whether attributes show,
how much attribute detail, what associations display, what class boxes display.
Settings persist per diagram (in the model / OKF), not per browser.

## Current state (concrete)

- ERD button + icon: `Dock.tsx:55-61` (`ErdIcon`), `Dock.tsx:245-261` (button).
- Toggle handler `handleToggleView`: `Canvas.tsx:298-304` (flips + persists `viewMode`).
- `viewMode` state + load: `Canvas.tsx:167`, `state/viewMode.ts`.
- `relLabelMode` state + Dock control: `Canvas.tsx` (`relLabelMode`), `Dock.tsx`,
  `state/relLabels.ts`.
- `Diagram` type: `packages/okf/src/types.ts:80-86` (`key`, `title`, `profile`,
  `members`, `hints?`).

## Changes

### Data model

Add an optional `display` field to `Diagram` (`packages/okf/src/types.ts`):

```ts
export interface DiagramDisplay {
  /** Show attribute rows on class boxes. Replaces viewMode erd/compact. */
  showAttributes: boolean;
  /** Detail shown per attribute row. */
  attributeDetail: "name-only" | "name-type";
  /** Association-edge labels. Replaces relLabelMode. */
  associationLabels: "all" | "hidden";
  /** Bold/emphasize multiplicity on association ends. Existing knob. */
  emphasizeMultiplicity: boolean;
  /** Show the profile stereotype line on class boxes. */
  showStereotype: boolean;
}

export interface Diagram {
  key: string;
  title: string;
  profile: string;
  members: string[];
  hints?: DiagramHints;
  display?: DiagramDisplay;   // NEW — absent ⇒ defaults below
}
```

`display` is optional; a missing value resolves to a `DEFAULT_DISPLAY` constant
(`showAttributes: true`, `attributeDetail: "name-type"`, `associationLabels:
"all"`, `emphasizeMultiplicity: false`, `showStereotype: true`). This keeps
existing OKF files valid and round-trips through OKF import/export.

### Rendering

- `toRFNode` / `erdAwareNodeSize` / `runDagreLayout` / `buildRfEdges` stop reading
  the global `viewMode`; they read the active diagram's resolved `display`.
- The node-data flag `_viewMode` becomes `_display` (the resolved `DiagramDisplay`),
  consumed by the OKF node renderer and edge components.
- Delete `state/viewMode.ts` and the `viewMode` state in `Canvas.tsx`. Fold
  `relLabelMode` handling into `display.associationLabels` and remove the separate
  Dock rel-label control + `state/relLabels.ts` (or keep the file only if used
  elsewhere — verify at plan time).

### UI

- Remove the ERD button from the Dock. Add a **"Diagram properties"** button
  (icon: a sliders/settings glyph) in its slot.
- Clicking opens a **left-anchored flyout panel** off the dock (a small popover,
  not the right sheet). **Superseded:** the later `central-edit-panel-implementation`
  plan replaced the flyout popover with routing into the central edit panel
  (`DiagramPropertiesBody.svelte`, mounted via `CentralEditPanelHost`); the field
  set below is unchanged, only the container moved. Panel fields, driven by the
  active diagram's `display`:
  - **Show attributes** — toggle (`showAttributes`).
  - **Attribute detail** — segmented / radio: *Name only* | *Name + type*
    (`attributeDetail`; disabled when Show attributes is off).
  - **Associations** — *Show labels* | *Hide labels* (`associationLabels`).
  - **Emphasize multiplicity** — toggle (`emphasizeMultiplicity`).
  - **Show stereotype** — toggle (`showStereotype`).
- Changing a field writes to the active diagram's `display` via the model store
  and re-renders immediately.

## Edge cases

- **Implicit diagram:** when `graph.diagrams` is empty the canvas shows one
  implicit diagram (`types.ts:92`). The panel then edits an in-memory display
  applied to that implicit view; if the user changes it, materialize a real
  diagram or store the display on a session default — pick at plan time, default
  to a session-level default that does not force diagram creation.
- **Migration:** existing localStorage `viewMode`/`relLabelMode` values can seed
  `DEFAULT_DISPLAY` once on load so current users keep their look; not required.

## Out of scope

- Diagram switching / rename / create (spec: diagram-title-switcher).
- Per-element show/hide (`hints.collapse` already exists; not touched here).

## Testing

- Unit: `DiagramDisplay` defaulting (missing `display` ⇒ defaults); OKF
  round-trip preserves `display`.
- Component: panel renders active diagram's values; toggling a field calls the
  store update; *Attribute detail* disabled when *Show attributes* off.
- Rendering: node size + attribute rows respond to `showAttributes` /
  `attributeDetail`; edge labels respond to `associationLabels`.
