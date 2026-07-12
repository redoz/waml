# Keyboard Shortcut Hints — Design

**Date:** 2026-07-12
**Status:** Approved (brainstorming)

## Goal

Add a stateful "Show keyboard shortcuts" button to the left toolbar (Dock). When
enabled, visible key-hint badges appear on every keyboard-accessible action. The
same hints also appear on hover of an action's control even when the toggle is
off — so shortcuts are discoverable without committing to the always-on mode.

## Scope

Only actions that are **actually keyboard-accessible today** get hints. Nothing
gets a badge merely because it has a button.

Current registry:

| id | Action | Keys | Home (control) |
|----|--------|------|----------------|
| `tool.select`  | Select & move tool | `V`       | Dock button |
| `tool.add`     | Add object tool    | `N`       | Dock button |
| `tool.connect` | Connect tool       | `C`       | Dock button |
| `selection.delete` | Delete selection | `⌫` (Delete/Backspace) | SelectionToolbar Delete button |
| `hints.toggle` | Toggle shortcut hints | `?` (Shift+/) | New Dock button |

Layout, Clear, and Diagram-properties have no key today → no badge.

## Non-goals

- No new shortcuts for Layout / Clear / Diagram-properties.
- No floating legend panel.
- No remapping / customization UI.

## Architecture

### 1. Shortcut registry — `shortcuts.ts`

Single source of truth. Array of `{ id, keys: string[], label }`. Helpers:

- `keyLabel(id): string[]` — the display glyphs for an action.
- Key-matching helpers used by the actual handlers, so handler keys and displayed
  keys never drift.

Both the Dock `v/n/c` handler and the CanvasInner Delete handler are re-sourced
from this registry (they currently hardcode magic strings `"v"`/`"n"`/`"c"` and
`"Delete"`/`"Backspace"`).

### 2. Shared hint state — `shortcutsHint` module

A shared Svelte `$state` module exposing `showHints: boolean`.

- Toggled by the new Dock button and by the `?` key.
- Initialized from `localStorage` and written back on change, so the preference
  persists across reloads.

### 3. Visibility mechanism — CSS driven

A `<KeyHint>` component renders `keys` as `<kbd>` glyphs with class `keyhint`,
default `opacity-0 transition-opacity`. It has **no reactive `show` prop** —
visibility is driven entirely by ancestor CSS:

- Hover:  `.group:hover .keyhint { opacity: 1 }`  (reuses existing `group` wrappers)
- Toggle: `[data-show-shortcuts] .keyhint { opacity: 1 }`

The app root sets `data-show-shortcuts` from `showHints`. Two global CSS rules
cover both hover and toggle for every hint, on Dock and contextual UI alike.

### 4. Components + wiring

- **`KeyHint.svelte`** — presentational glyph renderer. `keys={["V"]}` → `<kbd>V</kbd>`.
- **Dock tool buttons** (Select/Add/Connect) — `KeyHint` overlaid at the button's
  top-right corner (absolute). The existing `dockTip` descriptive tooltip stays
  (full label on hover); `KeyHint` is the always-on-when-toggled glyph.
- **New Dock button** — lucide `Keyboard` icon, stateful active style matching the
  `propsOpen` pattern (blue when on), `aria-pressed={showHints}`. Toggles state.
  Carries its own `?` `KeyHint`.
- **SelectionToolbar** — inline `<KeyHint keys={["⌫"]}>` next to the Delete label.
- **Root** — reflect `showHints` onto `data-show-shortcuts`.

## Behavior

| Toggle | Hover control | Result |
|--------|---------------|--------|
| off    | no            | hint hidden |
| off    | yes           | hint visible (hover) |
| on     | either        | hint visible (persistent) |

## Edge cases

- `?` handler ignored when focus is in `INPUT` / `TEXTAREA` / `SELECT` (reuse
  Dock's existing guard). `?` is Shift+/ → match on `e.key === "?"`.
- `showHints` read from `localStorage` on init; missing/invalid → default off.
- Existing V/N/C behavior unchanged — only re-sourced from the registry.
- Context actions (Delete) only surface a hint while their contextual UI is
  present (SelectionToolbar shows only when something is selected). Acceptable:
  every action's hint lives on that action's home.

## Testing

- `shortcuts.test.ts` — registry shape, `keyLabel`, key-matching helpers.
- `KeyHint.test.ts` — renders the passed glyphs.
- `Dock.test.ts` — toggle button flips `showHints` + root `data-show-shortcuts`;
  `?` key toggles (and is ignored while typing in an input); tool badges present.
- `SelectionToolbar.test.ts` — Delete hint rendered.
- Hover-vs-toggle visibility is CSS, not computed in jsdom → assert presence of
  classes / `data-show-shortcuts` attr, not computed opacity.
