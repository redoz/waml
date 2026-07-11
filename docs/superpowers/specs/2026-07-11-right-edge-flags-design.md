# Right-Edge Flag Tabs (Feedback + Inspect) & Rail Removal

**Date:** 2026-07-11
**Product:** Model Canvas (`packages/web`, React + React Flow + TypeScript)
**Scope:** 4 of 5 UI-change specs.
**Depends on:** share-topbar-and-dialog (Share must leave the right rail before the
rail is deleted).

## Context

- The **Feedback** link is a small absolutely-positioned anchor at the
  bottom-left of the canvas (`Canvas.tsx:544-554`, `MessageSquare` + "Feedback"),
  currently pointing at a Google Form and sitting under the React Flow zoom
  `Controls`.
- The **right icon rail** (`packages/web/src/components/rail/RightRail.tsx`) holds
  two entries, `share` and `inspect` (`RightRail.tsx:7-10`). Share is being moved
  to the top bar (spec 3), leaving only Inspect.
- **Inspect** opens the Inspector (`packages/web/src/components/inspector/*`) inside
  the sliding **ModelSheet** (`ModelSheet.tsx`, rendered `Canvas.tsx:572-607`).
  Panel id state: `packages/web/src/components/rail/useRightPanel.ts`.

## Goal

- Turn both **Feedback** and **Inspect** into **vertical "flag" tabs** on the right
  edge, middle height, sharing one reusable tab component.
- **Feedback flag** links to the GitHub new-issue page.
- **Inspect flag** toggles the Inspector. The Inspector is **pinnable**; when
  pinned but unfocused it becomes **translucent**, and **fades back to opaque on
  hover**.
- Delete the right icon rail (`RightRail`) — Share already left it (spec 3), so
  nothing else needs it.

## Current state (concrete)

- Feedback anchor: `Canvas.tsx:544-554` (bottom-left, Google Form URL).
- Right rail: `RightRail.tsx` (`share` + `inspect`), rendered `Canvas.tsx:608`.
- Inspector host: `ModelSheet.tsx`; Inspector dispatcher
  `components/inspector/Inspector.tsx`.
- Panel state: `useRightPanel.ts` (`RightPanelId`, open/close).

## Changes

### Reusable flag-tab component

New `components/chrome/EdgeFlag.tsx` — a vertical tab pinned to the right edge at
mid-height, with rotated (vertical) label text, a small icon, and a "little flag /
branding tag" silhouette. Props: `label`, `icon`, `onClick` (or `href`), and a
stacking offset so multiple flags stack without overlap. Style is a starting idea,
not a fixed design; tune during implementation.

### Feedback flag

- Remove the bottom-left feedback anchor (`Canvas.tsx:544-554`); the zoom
  `Controls` can drop back to its normal position (undo the `bottom: 60` nudge).
- Render an `EdgeFlag` labeled **Feedback** on the right edge, linking to
  `https://github.com/redoz/uaml/issues/new` (opens in a new tab).

### Inspect flag + pinnable Inspector

- Render an `EdgeFlag` labeled **Inspect** on the right edge, stacked with the
  Feedback flag (define a consistent order, e.g. Inspect above Feedback).
- Clicking it toggles the Inspector open/closed.
- **Pin behavior:** the Inspector gets a **pin** control. Two states:
  - *Unpinned* (default): behaves as before — opens over the canvas, closes on
    outside interaction / when its trigger is toggled.
  - *Pinned:* stays open. When it **does not have focus / the pointer is not over
    it**, it renders **translucent** (e.g. `opacity: ~0.4`) so the canvas shows
    through. On **hover (or focus)** it animates back to **opaque**. Pointer
    events remain enabled (hovering to read it restores it); click-through is out
    of scope.
- Selection no longer auto-opens the Inspector — that behavior moves to the
  selection toolbar spec. (If that spec is not yet built, keeping the existing
  auto-open temporarily is acceptable; the two specs converge on "flag opens
  inspector, selection shows toolbar.")

### Remove the rail

- Delete `RightRail.tsx` and its render at `Canvas.tsx:608`.
- The Inspector may keep using `ModelSheet` as its host, or the pin/translucent
  behavior may warrant a dedicated inspector container — decide at plan time;
  either way `RightPanelId` reduces to just the inspector concept (Share is gone).

## Edge cases

- **Both flags visible with panel open:** the open Inspector should not be
  obscured by its own flag; position the flags clear of the panel or let the panel
  overlay them.
- **Translucent + interacting:** while the pointer is over the panel it is opaque;
  moving away re-triggers the fade after a short delay to avoid flicker.
- **Keyboard access:** flags are focusable buttons/links; focus opens/activates.

## Out of scope

- Share (spec 3 — already moved to the top bar).
- Selection toolbar / multi-select (spec 5).

## Testing

- Component: two `EdgeFlag`s render on the right edge, stacked without overlap;
  Feedback links to the issues URL; Inspect toggles the Inspector.
- Behavior: pin keeps the Inspector open; unfocused pinned panel is translucent;
  hover restores full opacity.
- Regression: `RightRail` removed with no dangling imports; feedback no longer at
  bottom-left; zoom controls reposition correctly.
