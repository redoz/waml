# Navigator Panel Promotion — Design

Date: 2026-07-16

## Goal

Promote the model Navigator (the bundle/package structure tree) from a small,
dismissable dropdown popup into a persistent, near-full-height panel. The popup
currently feels like it "is going to disappear any second," which makes tree
manipulation — reordering members, moving nodes between packages, restructuring
the model — feel fragile. A tall, stable panel gives the tree real vertical room
and removes the fear of an accidental dismiss.

## Current State

- `packages/web/src/components/Navigator.svelte` — the tree body. Today it renders
  its own fixed chrome: a `w-[300px]` rounded card with an internal
  `max-h-[420px]` scroll region. It is prop-driven and presentational (all
  mutations are callbacks), so it unit-tests in isolation.
- Mounted as a **dropdown** inside `TopBar.svelte` (~line 184): the centered
  diagram-title switcher button (`switcherOpen` state) toggles it, and a
  `fixed inset-0 z-40` click-catcher backdrop dismisses it on any outside click.
- Two existing panel-chrome patterns to draw from:
  - `InspectorPanel.svelte` — floating docked card, `absolute top-3 right-3`,
    near-full-height (`max-h-[calc(100%-24px)]`), **no scrim** (canvas stays
    interactive), left-edge resize drag, translucent-when-idle (pinned + not
    engaged), collapse chevron. State (`inspectorPinned`, `inspectorWidth`) lives
    in `CanvasInner.svelte` as session-local `$state` — **not** persisted to
    localStorage; it resets on reload.
  - `CentralEditPanel.svelte` — centered modal card over a dismissing scrim,
    `fullHeight` → `h-[95vh]`, header with title + close button, dismiss via
    close / scrim-click / two-stage Esc (first Esc blurs a focused input to
    protect an in-progress edit, second Esc closes).

## Design

### Two-mode panel

The promoted Navigator has two mutually-exclusive display modes, toggled by a
**pin button** in the panel header:

1. **Unpinned — centered (default).** Exactly the `CentralEditPanel` treatment:
   a centered card over a dismissing scrim, near-full-height (`h-[95vh]`), header
   bar with the Navigator's scope title, close button, and the new pin button.
   Dismiss via close button, scrim click, or Esc (two-stage, protecting
   in-progress inline create/rename inputs). This is the "focus mode" for
   restructuring the tree.

2. **Pinned — left-docked.** A left-edge mirror of `InspectorPanel`: anchored
   `absolute top-3 left-3`, near-full-height, **no scrim** so the canvas stays
   interactive. Full Inspector parity, mirrored to the left:
   - **Resize** by dragging the panel's **right** edge (Inspector drags its left
     edge; this is the mirror).
   - **Translucent-when-idle** — solid while the pointer is over it or focus is
     inside; dims when idle. (Docked mode is inherently the "pinned" state, so
     translucency is always active in this mode; there is no separate
     translucency toggle inside the docked mode.)
   - **Collapse chevron** — folds the body to just the header bar.

### Controls & state transitions

- The existing TopBar diagram-title **switcher button toggles the panel
  open/closed** (unchanged trigger, new target).
- The header **pin button toggles centered ↔ left-docked**.
- **Esc closes** the panel (centered mode uses `CentralEditPanel`'s two-stage
  Esc; docked mode closes on Esc when focus is inside the panel).
- **Unpin while docked → returns to centered mode** (the inverse of pin; does not
  close).
- **Reopen after closing → remembers the last mode** (centered vs docked) for the
  session.
- **State is session-local**, mirroring the Inspector exactly: mode, width, open,
  and collapsed are in-memory `$state` and reset on page reload. No localStorage.

### Coexistence

The left dock and the right-side Inspector can be open at once; the canvas sits
between the two rails. Both are non-modal in their docked/floating states, so
this is fine. (The centered mode's scrim covers the canvas — expected for a focus
mode.)

## Architecture

The change is primarily a **chrome/ownership refactor**; the tree logic in
`Navigator.svelte` is preserved.

1. **Extract a chrome-agnostic Navigator body.** Split `Navigator.svelte` so the
   search box, type chip, breadcrumb, tree, context/action menus, inline
   create/rename forms, and delete prompt live in a body component that **fills
   its host** — a flex column whose tree scroll region is `flex-1 min-h-0`
   instead of the current fixed `w-[300px]` / `max-h-[420px]`. All existing props
   and callbacks are preserved so its unit tests continue to pass (adjusting only
   for the chrome split). The absolutely-positioned menus (`actionMenu`,
   `ctxMenu`, `deletePrompt`) currently anchor to fixed `top-[120px]` offsets
   inside the 300px sheet; they must be re-anchored to work inside a tall,
   variable-width host in both modes.

2. **Two host wrappers around the shared body:**
   - **Centered:** reuse `CentralEditPanel` (pass the Navigator body as its
     `children`, the scope title as `title`). Add the pin button to its header —
     via the existing optional `header` snippet, or a small extension for a
     header-action slot. Esc / scrim / close come for free.
   - **Left-dock:** a left-mirrored Inspector chrome. Prefer parameterizing
     `InspectorPanel` (or a shared docked-panel primitive) with a `side: "left" |
     "right"` prop that flips the anchor (`left-3` vs `right-3`) and the resize
     handle edge, rather than duplicating the resize/translucency/collapse logic.
     If parameterizing risks the Inspector's behavior, extract the docked chrome
     into a shared component both consume.

3. **Move Navigator mounting from `TopBar` into `CanvasInner`'s canvas overlay
   layer.** To dock over the canvas (and to render the centered modal above it),
   the Navigator must render in the canvas layer alongside `InspectorPanel`, not
   inside the TopBar dropdown. `TopBar`'s switcher button becomes a pure trigger
   that emits an `onToggleNav` callback and reflects the open state
   (`aria-expanded`); the `switcherOpen` state and the `<Navigator>` mount move up
   to `CanvasInner`, which owns the new session-local state:
   `navOpen`, `navMode` (`"centered" | "docked"`), `navWidth`, `navCollapsed`.
   All the existing Navigator callbacks (`onScope`, `onSelectDiagram`,
   `onReorder`, `onCreatePackage`, `onCreateNode`, `onCreateDiagram`, `onRename`,
   `onSort`, `onDelete`, `onViewInDiagram`, `onAddToNewDiagram`,
   `onEditProperties`) are already wired at the TopBar level; they move with the
   mount to `CanvasInner` (which is where their handlers already originate).

## Testing

- Preserve/adapt `Navigator.test.ts` for the extracted body (props/callbacks
  unchanged; assertions that depend on the old fixed chrome updated).
- New tests: pin toggles centered↔docked; unpin-while-docked returns to centered;
  switcher toggles open/closed; Esc closes; reopen remembers last mode; docked
  resize clamps like the Inspector; docked translucency toggles on
  engage/disengage.
- Manual/visual verify: tree drag-reorder works in both modes; context menus
  anchor correctly in the tall centered card and the resizable left dock;
  left dock + right Inspector coexist without overlap.

## Out of Scope (YAGNI)

- Cross-reload persistence of panel state (matches Inspector: session-local only).
- A separate translucency pin inside docked mode (docked is inherently pinned).
- Resize of the centered mode (fixed near-full-height, like `CentralEditPanel`).
- Any change to the tree's behavior, search, or context-menu actions themselves.
