# Central Edit Panel — Design

**Date:** 2026-07-12
**Status:** Draft (design)
**Related:** `2026-07-11-diagram-properties-panel-design.md`, tree-nav workstream (selection model), diagram-solver-wasm-bridge (Phase 2, complete)

## Problem

The canvas is a **diagram** surface. But two edit contexts have no diagram to render onto:

1. **A bare element or package is selected** (via the tree navigator, outside any diagram). Today the app fakes a synthetic `__all__` diagram and drops the node on a canvas. That fake view is being removed by the tree-nav workstream — leaving no home for "edit this one thing."
2. **Diagram properties** — editing a diagram's own settings (title, profile, display) is a diagram-level concern, not a node/edge selection.

We want one **central edit panel**: a centered, enlarged surface (wider than the docked inspector, expanded to the bottom of the screen) that hosts single-item editing outside a diagram, and doubles as the diagram-properties editor.

## Goal

Introduce a **central edit panel** — a centered, large edit surface — used in two contexts:

- **Single-item edit** (element / package selected, no diagram active): panel centered over a neutral backdrop.
- **Diagram properties** (a diagram is active): same panel, with the **live diagram still rendered behind it** (you are already "in" the diagram).

Reuse the existing inspector field bodies (`ObjectInspector`, diagram display fields). This is a new **host/chrome**, not new field editors.

## Non-Goals

- **Tree navigation & selection model** — owned by the separate tree-nav workstream. This spec *consumes* its selection signal; it does not build it.
- **Solver-driven layout (Phase 3 / Feature B)** — independent; separate spec. The central panel never lays anything out.
- **Deleting the synthetic `__all__` diagram** — tree-nav's job. This panel is what replaces its *purpose* for non-diagram selection, but the deletion lands there.
- Rewriting `ObjectInspector` / `RelationshipInspector` field bodies. They are reused verbatim.

## Architecture

### Components

- **`CentralEditPanel.svelte`** (new) — the centered host chrome. Owns: centered positioning, enlarged sizing (wider than docked; full height to screen bottom), optional scrim/backdrop, dismissal (Esc / scrim click / close button), and a `<slot>`/snippet for the body. Purely presentational + dismissal wiring; knows nothing about *what* it hosts.
- **Body content** routed by the current selection:
  - **element (classifier node)** → existing `ObjectInspector` fields.
  - **package node** → `ObjectInspector` fields (a package is a `Node`; start with the same title/description/type body — see Open Questions on whether to trim).
  - **diagram** → diagram display/properties fields. **These exist today** as a 268px popover in `Dock.svelte` (sliders button, `role="dialog"` "Diagram properties"): `showAttributes`, `attributeDetail`, `associationLabels`, `emphasizeMultiplicity`, `showStereotype`, etc. — display toggles only, no title/profile. The panel body reuses these same controls (lifted out of the popover); the Dock **sliders button stays as the trigger** but opens the central panel instead of the inline popover.

### Two invocation contexts

| Context | Trigger | Backdrop behind panel |
|---|---|---|
| Single-item edit | selection is a bare element/package (from tree-nav), no active diagram | neutral empty app backdrop (whatever tree-nav shows when no diagram is open) |
| Diagram properties | active diagram + user opens "diagram properties" | the **live diagram**, dimmed by a scrim so the panel reads as focused |

Same `CentralEditPanel` chrome in both; only the backdrop and body differ.

### Relationship to the existing docked inspector

The right-docked `InspectorPanel` **stays** for **in-diagram** element/edge selection — editing a node while looking at its diagram is unchanged. The central panel is strictly the **out-of-diagram** (single-item) and **diagram-level** (properties) surface. Two hosts, **shared field bodies**. (Open Question: whether the two hosts should later converge — out of scope here.)

## Data Flow

1. Selection changes (from tree-nav for bare items; from a "diagram properties" affordance for the diagram case).
2. A derived `centralPanel` state resolves to one of: `{ kind: "node", node }`, `{ kind: "diagram", diagram }`, or `null` (closed).
3. `CentralEditPanel` renders when non-null, routing to the matching body.
4. Edits dispatch the **existing** update paths: `onUpdateNode` (→ `store.updateNode`) for nodes; `handleDisplayChange` / `store.updateDiagram` for diagram properties. No new persistence path.
5. Dismiss → clear `centralPanel` → return to prior view (diagram stays put in the properties case).

## Error / Edge Handling

- Selection points at a since-deleted key → resolve to `null` (panel closes), same guard as today's `focused`.
- Opening diagram properties for the implicit/absent diagram → no-op (mirrors today's `updateDiagram` no-op on `__all__`), until tree-nav removes that case.
- Esc while a field input is focused → blur first, second Esc closes (don't lose an in-progress edit to a stray keypress).

## Testing

- **Unit (vitest + Svelte):** `CentralEditPanel` renders body per `kind`; scrim/Esc/close all clear the panel; edits call the injected update callback.
- **Body reuse:** assert `ObjectInspector` mounts inside the central host and title/description edits fire `onUpdateNode` (reuse existing ObjectInspector test patterns).
- **Diagram-properties context:** panel mounts over a rendered diagram (backdrop present); dismiss leaves the diagram intact.
- No golden/visual test required for layout; positioning verified manually.

## Open Questions (resolve in planning or with a stated default)

1. **Package body fields** — reuse full `ObjectInspector` (title/description/type/attributes) or a trimmed package body? **Default: reuse `ObjectInspector` unchanged** for now; trim later if noisy.
2. **Diagram-properties trigger** — RESOLVED: exists today as the sliders button + 268px popover in `Dock.svelte` (`propsOpen`). Plan reuses the button, routes it to the central panel, and lifts the popover's display controls into the panel body. Decide during planning whether the small popover is deleted outright or kept as a compact alternative.
3. **Selection interface with tree-nav** — RESOLVED against landed main. The tree-nav navigator exposes a single-item edit action as the `onEditProperties(key: string)` callback (`Navigator.svelte` → `TopBar.svelte` → `CanvasInner.svelte`). `key` is a model node key (element/classifier); the Navigator fires it from the row action menu (see `Navigator.test.ts:87`, called with `"customer"`). Today `CanvasInner.svelte:518` wires it to the synthetic path `selectionSet = { nodes: [key], edges: [] }`, which drives the docked `InspectorPanel` over the implicit `__all__` diagram — this is exactly the fake single-item view this spec replaces. C rewires `onEditProperties(key)` to open the central panel on that node instead. Note: `__all__` / `effectiveDiagrams` still exist on main (deletion remains tree-nav's job), so a truly "no active diagram" backdrop does not exist yet; the single-item panel opens over whatever is currently rendered until that lands. Package-kind rows do not currently fire `onEditProperties` (they use the inline rename/create/delete context menu), so the package-body context is latent until tree-nav routes packages through this callback.
4. **Coexistence vs convergence** — docked `InspectorPanel` and `CentralEditPanel` share bodies but are separate hosts. Confirm they coexist (this spec's assumption) rather than merging now.
5. **Backdrop for single-item context** — is there literally an empty neutral canvas behind, or is the panel effectively full-screen? Default: neutral empty backdrop, panel centered (not edge-to-edge).
