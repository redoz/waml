# Floating inspector — picker + never-close — design

**Date:** 2026-07-12
**Scope:** Restyle the inspector panel to match the diagram properties dialog, float it off the top/right corner, make it collapsible, replace its static title with an element **picker**, and make it **never close** — an empty selection shows a compact bar + hint instead.
**Primary files:** `packages/web/src/components/inspector/InspectorPanel.svelte` (card + header + collapse) and `packages/web/src/components/canvas/CanvasInner.svelte` (sole host: always-mounted, picker options, selection wiring, Inspect edge-flag removal).

## Problem

The inspector is an edge-pinned, full-height white slab (`absolute top-0 bottom-0 right-0`, `border-l`, no radius) that opens/closes via an "Inspect" edge-flag and a close (X) button, with a static `title="Inspect"`. We want it to:
1. Read as the same design language as the diagram properties dialog (`Dock.svelte:243-247`) — detached, rounded, all-sides shadow.
2. Float off the top/right corner.
3. Be collapsible.
4. Never close — instead, when nothing is selected it rests as a compact bar with a hint.
5. Carry an **element picker** in its header (in place of the static title) so you can choose what to inspect from the current diagram.

## Design

### 1. Floating card styling

The `<aside>` container changes from an edge slab to a detached card:

| Property | Now | After |
|---|---|---|
| position | `absolute top-0 bottom-0 right-0` | `absolute top-3 right-3` (12px gap off top + right) |
| height | full (top+bottom pin) | content-height, capped `max-h-[calc(100%-24px)]` |
| border | `border-l border-[#d8dee8]` | `border border-[#d8dee8]` (all sides) |
| radius | none | `rounded-xl` (12px) — matches dialog |
| shadow | `0_10px_15px_-3px_rgba(0,0,0,0.1)` | `0_8px_24px_rgba(15,23,42,0.14)` — dialog's |
| clip | — | `overflow-hidden` so the radius clips header/body corners |
| width | inline `width:{width}px`, resizable | **unchanged** |
| opacity | `opacity-40` when pinned+idle | **unchanged** |
| z | `z-[16]` | **unchanged** |

Body scroll region keeps `flex-1 min-h-0 overflow-y-auto` so content scrolls inside the `max-h` cap.

### 2. Never close — always mounted

- Remove the panel's `open` prop, the host's `inspectorOpen` state, and the **Inspect edge-flag tab** (`CanvasInner.svelte:606-615`). The panel is always rendered.
- Remove the close (X) button and the `onClose` prop.
- The right-edge flags that previously slid left only while open now always offset by the panel width (`rightOffset={inspectorWidth}`).

### 3. Header — kind-icon + element picker + collapse + pin

Header control cluster, left→right: `[kind-icon] [ ▾ element select box ] [⌃ collapse] [📌 pin]`.

- **Element picker** (native styled `<select>`) replaces the static `<h2>`:
  - Options = the active diagram's **member nodes** (objects + notes — both are nodes, so notes appear naturally). Option label = `node.title.trim() || "Untitled"`, value = `node.key`.
  - A placeholder option `"Select an element…"` is shown/selected when nothing is focused.
  - Choosing an option focuses that node: host sets `selectionSet = { nodes: [key], edges: [] }`.
  - Two-way: when the canvas selection changes, the picker's current value reflects the focused node.
- **Kind-icon** left of the picker reflects the focused element kind: node → `Box`, edge → `Spline`. Hidden when nothing is focused.
- **Collapse chevron**: only shown when something is focused (there's a body to collapse). Toggles the body; chevron rotates 180°. Local `collapsed` state, not persisted.
- **Pin** button + translucent-when-idle behavior: unchanged.

### 4. Body — selection-driven

- **Nothing focused** (resting state): header (picker) + a **slim hint** row ("Select an element to edit"). No body, no collapse chevron. This is the "compact" view.
- **Node focused**: `ObjectInspector` body (rendered by the existing `<Inspector>` child). Collapse chevron toggles it.
- **Edge focused** (via canvas — the picker does not list edges in v1): `RelationshipInspector` body still renders through the child; the picker shows its placeholder. No regression.

## Interface changes

`InspectorPanel.svelte` props:
- **Remove:** `open`, `onClose`, static `title`.
- **Add:**
  - `options: { key: string; label: string }[]` — picker entries (diagram member nodes).
  - `selectedKey: string | null` — focused node key (null when an edge or nothing is focused).
  - `focusedKind: "node" | "edge" | undefined` — drives the kind-icon + whether a body/collapse is offered.
  - `onSelect: (key: string | null) => void` — focus a node from the picker.
- **Keep:** `pinned`, `onTogglePin`, `hideDelay`, `width` (`$bindable`), `children`.
- The `<aside>` region `aria-label` is the constant `"Inspector"`.

Host (`CanvasInner.svelte`):
- Remove `inspectorOpen`, the Inspect edge-flag, and the `onClose` handler.
- Always render `<InspectorPanel>`.
- Derive and pass: `options` (member nodes), `selectedKey` (`focused?.type === "node" ? focused.id : null`), `focusedKind` (`focused?.type`), `onSelect` (`key => selectionSet = key ? {nodes:[key],edges:[]} : EMPTY_SELECTION`).
- Right-edge flag `rightOffset` becomes `inspectorWidth` (always).

## Non-goals / YAGNI

- No "diagram itself" picker entry / diagram-inspector body (later).
- No relationships in the picker (later); edge selection via canvas still works.
- No note-vs-object visual distinction in the picker (later).
- No element context-menu / kebab.
- No collapse-state persistence.
- No changes to resize math, translucency logic, or the body inspector components (`ObjectInspector`, `RelationshipInspector`).

## Testing

- `InspectorPanel` unit tests: picker renders options + reflects `selectedKey`; `onSelect` fires on change; kind-icon renders per `focusedKind`; collapse toggle only present when focused and hides the body; nothing-focused shows the hint and no body; pin/translucency/resize tests still pass (drop the `open:false` "renders nothing" and the close-button tests).
- `CanvasInner` / `Canvas` integration: panel is always present (no Inspect flag); selecting a node on canvas reflects in the picker; picking in the select focuses the node.

## Reference

Interactive mockup (v1 styling — floating card, collapse, kind-icon, pin): illustrates the card look. The picker + never-close behavior is described above.
