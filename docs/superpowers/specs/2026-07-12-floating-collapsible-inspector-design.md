# Floating collapsible inspector — design

**Date:** 2026-07-12
**Scope:** Restyle the inspector panel to match the diagram properties dialog, float it off the top/right corner, and make it collapsible.
**Target file:** `packages/web/src/components/inspector/InspectorPanel.svelte` (the `<aside>` at lines 94–103 and its header). No new components.

## Problem

The inspector is an edge-pinned, full-height white slab (`absolute top-0 bottom-0 right-0`, `border-l`, no radius). The diagram properties dialog (`Dock.svelte:243-247`) is a detached, rounded, all-sides-shadowed floating card. The two should read as the same design language. The inspector should also be collapsible so it can be tucked out of the way without losing the "what's selected" context.

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

Because the card is content-height, the body scroll region (currently line 136) needs `min-h-0` so `overflow-y-auto` engages when content exceeds the `max-h` cap.

### 2. Collapse

- New local `collapsed` state (`$state(false)`) in `InspectorPanel.svelte`.
- A chevron toggle button in the header, in the control cluster to the **left of** the pin button.
- Collapsed:
  - Body (`{@render children()}` region) is hidden (`{#if !collapsed}` or `class:hidden`).
  - Left-edge resize drag handle is hidden.
  - Header bottom-border is dropped (card is just the header bar).
- Because the card anchors top-right, hiding the body shrinks the card upward — the content "collapses up" into the header bar.
- Chevron rotates 180° between states (`transition-transform`).
- Collapse state is component-local and not persisted (resets when the panel unmounts/reopens). Persistence is out of scope.

### 3. Header — kind-icon + name, no close

- Replace the plain title with a **kind-icon + name**:
  - A small square icon chip (accent color on `#e6f1fb`, `rounded-md`) before the title.
  - Icon reflects the element kind: a box/table glyph for an object node, a relationship glyph for an edge. The consumer already distinguishes node vs. edge (`Inspector.svelte:32-42`) — it passes a `kind` (`"node" | "edge"` or similar) alongside the existing `title` prop so `InspectorPanel` can pick the icon. Fallback icon when nothing meaningful is selected.
  - So the collapsed header bar still shows *what* is selected.
- **Remove the close button.** Collapse is the only "tuck away" affordance now.
- **Keep the pin button** and its behavior unchanged (toggles the translucent-when-idle fade).
- Resulting header control cluster, left→right: `[kind-icon] [title ………] [⋯ collapse chevron] [📌 pin]`.

## Interface changes

`InspectorPanel.svelte` props:
- **Add** `kind` (e.g. `"node" | "edge" | undefined`) so the header can pick the icon.
- **Remove** `onClose` and the close button markup.
- Everything else (`open`, `pinned`, `title`, `onTogglePin`, `hideDelay`, `width`, `children`) unchanged.

Consumer (`Inspector.svelte`, and any other `InspectorPanel` host):
- Pass the new `kind` prop derived from the current selection.
- Drop the `onClose` wiring for this panel (the panel no longer closes itself; closing, if still needed anywhere, is the host's concern via its own affordance — out of scope here).

## Non-goals / YAGNI

- No element context-menu / kebab (`...`) button.
- No collapse-state persistence.
- No changes to the older `Inspector.svelte` slab chrome beyond passing the new prop / dropping close wiring for `InspectorPanel`.
- No changes to resize math, translucency logic, or the body inspector components (`ObjectInspector`, `RelationshipInspector`).

## Testing

- Existing `InspectorPanel` behavior (open, pin/translucency, resize) must still pass.
- Add coverage: collapse toggle hides body + resize handle and keeps the header; chevron aria/label reflects state; kind-icon renders per `kind`.

## Reference

Interactive mockup: floating card, collapse toggle, kind-icon + name, pin, no close — matches this spec.
