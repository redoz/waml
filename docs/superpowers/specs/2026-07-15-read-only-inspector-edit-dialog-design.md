# Read-Only Docked Inspector + Element Edit Dialog — Design

**Date:** 2026-07-15
**Status:** Draft (design)
**Related:** `2026-07-12-central-edit-panel-design.md` (introduces `CentralEditPanel`/`CentralEditPanelHost`), `2026-07-12-floating-collapsible-inspector-design.md` (prior `InspectorPanel` restyle, superseded intent)

## Problem

The docked `InspectorPanel` (floating card, top-right of the canvas) is editable inline today: selecting a node or edge shows live editable fields directly in the docked panel. This makes editing a two-purpose surface — "what is this element" and "change this element" are the same view, with no separation.

Separately, `CentralEditPanel` already exists as a centered, dialog-style host (currently used for "diagram properties" and for the Navigator's "Edit properties" context-menu action on nodes) — but the docked `InspectorPanel` and `CentralEditPanel` are not connected: editing from the canvas selection still happens inline in the dock, not through the dialog.

We want: selecting an element on canvas shows a **read-only** summary in the docked panel; editing happens in a dialog that mirrors the diagram-properties dialog's layout, with a live cropped preview of the element on top of the edit fields.

## Goal

1. Docked `InspectorPanel` becomes **read-only** for both nodes and edges — it displays the selected element's fields as plain text, not inputs.
2. An **Edit** button in the docked panel's header opens `CentralEditPanel` for the selected element (node or edge).
3. `CentralEditPanel`, when opened for element/edge editing, gets a **live cropped preview** at the top of the dialog body, above the existing editable fields.
4. `CentralEditPanel` width stays the diagram-properties width (`560px`); height grows toward full-viewport for the element/edge edit variant only — diagram properties keeps its current `85vh` cap.

## Non-Goals

- Changing `CentralEditPanel`'s diagram-properties sizing — stays at `max-h-[85vh]`, `560px` wide, unchanged.
- Interactive preview (pan/zoom/click inside the preview) — the preview is a static, live-updating render only.
- Removing the Navigator's existing "Edit properties" entry point — it continues to open `CentralEditPanel` for nodes; this spec adds a second entry point (the docked panel's Edit button) and extends the host to also cover edges.
- Rewriting `ObjectInspector` / `RelationshipInspector` editable field bodies — reused verbatim inside the dialog.
- Removing `packages/web/src/components/inspector/Inspector.svelte` — flagged as dead code below, not touched by this work.

## Architecture

### Docked InspectorPanel (read-only)

`InspectorPanel.svelte` keeps its existing chrome unchanged: floating card (`absolute top-3 right-3`), resizable via left-edge drag handle, collapse toggle, pin toggle, element-picker `<select>`, translucency-on-idle-when-pinned.

Body content changes:
- Currently: `children` snippet renders `ObjectInspector` (nodes) or `RelationshipInspector` (edges) — both editable, calling `onUpdate` on every keystroke.
- New: body renders new **read-only** presentational variants of the same field sets — Title, Description, Type, `abstract` flag, Stereotypes, Attributes/Values for nodes; endpoints, label, multiplicity for edges — as static text/labels, no inputs, no `onUpdate` wiring.

Header gains an **Edit** button (icon button, placed alongside the existing collapse/pin buttons), visible whenever `hasSelection` is true. Click opens the edit dialog for the currently selected element:
- Node selected → `centralPanel = { kind: "element", nodeKey }`
- Edge selected → `centralPanel = { kind: "edge", edgeKey }` (new state kind, see below)

Selection-driven behavior is otherwise unchanged: choosing an element via canvas click or the panel's own `<select>` still drives which element the (now read-only) body shows.

### CentralEditPanel sizing variant

`CentralEditPanel.svelte` gets a boolean prop, e.g. `fullHeight = false`:
- `fullHeight = false` (default, used by diagram properties): unchanged — `max-w-[560px] max-h-[85vh]`, `p-8` scrim inset.
- `fullHeight = true` (used by element/edge edit): same `max-w-[560px]`, height cap raised toward the viewport (e.g. `max-h-[95vh]`, reduced scrim inset e.g. `p-4`) so the dialog reads as full height. Width is unchanged from the diagram-properties dialog — same 560px, per explicit direction to keep width consistent for now.

`CentralEditPanelHost.svelte` passes `fullHeight={true}` for both the `"element"` and new `"edge"` branches, and leaves the `"diagram"` branch unchanged.

### Live cropped preview

New presentational component (e.g. `ElementPreview.svelte`), rendered at the top of the dialog body for `"element"` and `"edge"` kinds only (not for `"diagram"`):

- Fixed-height region (e.g. `h-[220px]`, `border-b` separating it from the fields below).
- Implementation: a second, read-only `SvelteFlow` instance (`nodesDraggable={false}`, `nodesConnectable={false}`, `panOnDrag={false}`, `zoomOnScroll={false}`, no selection/controls/minimap), fed a filtered subset of the current diagram's nodes/edges:
  - **Node edit**: the selected node plus its directly-connected neighbor nodes and the edges between them. Focal node full opacity; neighbors dimmed (e.g. reduced opacity/desaturated) for context without competing for attention.
  - **Edge edit**: the edge plus both endpoint nodes.
- Calls `fitView` (with padding) on mount and whenever the filtered node/edge set's geometry changes, so the view is always auto-cropped/zoomed to the relevant bounding box.
- "Live": the preview reads the same reactive model state (`nodes`/`display`) that the editable fields below write to via `onUpdateNode` — no separate sync path. Editing the title below updates the label rendered in the preview on the next reactive tick, same as the main canvas would.
- No pan/zoom/click/drag affordances — purely a view.

### Data model / wiring

`CentralPanelState` (`CentralEditPanelHost.svelte`) gains an edge variant:

```ts
type CentralPanelState =
  | { kind: "element"; nodeKey: string }
  | { kind: "edge"; edgeKey: string }
  | { kind: "diagram" };
```

`CentralEditPanelHost.svelte` gets a new `{#if state?.kind === "edge"}` branch, mirroring the existing `"element"` branch:
- Resolves the edge from the current edge list by key.
- Same since-deleted guard as the node effect (`$effect` closing the panel if the resolved edge becomes `undefined`).
- Renders `CentralEditPanel` with `fullHeight` set, `ElementPreview` (edge mode) + `RelationshipInspector` as children.

Only new trigger for `"edge"` kind is the docked panel's Edit button — there is no edge equivalent of the Navigator's context-menu action (edges aren't represented in the Navigator tree).

Node edit dialog continues to be reachable both from Navigator's "Edit properties" action and from the new docked-panel Edit button — both resolve to the same `{ kind: "element", nodeKey }` state and same dialog. The Navigator path does not currently need the preview (no diagram context when opened from the tree, per `2026-07-12-central-edit-panel-design.md`'s single-item-edit context) — **default: render the preview whenever a diagram is active/rendered behind the dialog; when there is no active diagram (Navigator's out-of-diagram single-item context), omit the preview section entirely** rather than showing an empty/broken canvas.

## Error / Edge Handling

- Selected node/edge deleted while the docked panel is open → existing `hasSelection`/`focusedKind` guards already blank the panel; unaffected by this change.
- Selected node/edge deleted while the edit dialog is open → existing since-deleted `$effect` guard (already present for `"element"`, mirrored for `"edge"`) closes the dialog.
- Preview's filtered node/edge subset resolves to just the focal element with no neighbors (isolated node, or a diagram with only one element) → `fitView` still crops/zooms to the single element; no special-casing needed.
- Edit dialog opened with no active diagram behind it (Navigator's out-of-diagram path) → preview section omitted (see above), only the editable fields render, matching current `CentralEditPanel` behavior for that path today.

## Testing

- **Unit (vitest + Svelte):**
  - Read-only `InspectorPanel` body variants render the same underlying node/edge field values as before, with no `<input>`/`<textarea>`/editable `<select>` elements present.
  - Edit button click sets `centralPanel` to the expected `{ kind: "element" | "edge", ... }` value (test via the parent `CanvasInner` wiring, mirroring existing `onEditProperties` tests).
  - `CentralEditPanelHost` renders the `"edge"` branch correctly, including the since-deleted-edge close guard (mirrors existing node test in `CentralEditPanelHost.test.ts`).
  - `CentralEditPanel` `fullHeight` prop toggles the expected size classes.
- **Preview component:** unit test asserts the filtered node/edge subset passed to the inner `SvelteFlow` includes the focal element + expected neighbors, and excludes unrelated diagram elements.
- No golden/visual test required for preview rendering; layout/cropping verified manually.

## Cleanup Note (flagged, out of scope)

`packages/web/src/components/inspector/Inspector.svelte` is dead code — an older full-height sidebar inspector variant, superseded by `InspectorPanel.svelte`, referenced only by its own test file. Not touched by this spec; worth a follow-up removal.

## Open Questions (resolve in planning or with a stated default)

1. **Neighbor-dimming exact styling** (opacity value, whether edges to non-included neighbors are hidden or also dimmed) — default: neighbors + their connecting edges at reduced opacity (e.g. `0.35`), consistent with any existing "dimmed" pattern elsewhere in the canvas (check `floating.ts`/`selection.ts` for an existing convention to reuse before inventing a new one).
2. **`fullHeight` exact max-height value** (`95vh` vs other) and reduced scrim inset — default `max-h-[95vh]`, `p-4`, finalize visually during implementation.
3. **Preview height** (`220px` suggested) — tune during implementation against the 560px dialog width so the crop reads well for both single nodes and small neighbor clusters.
