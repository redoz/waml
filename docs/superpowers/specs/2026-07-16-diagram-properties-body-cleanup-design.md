# DiagramPropertiesBody cleanup

**Date:** 2026-07-16
**Status:** Design approved, spec under review

## Problem

`packages/web/src/components/canvas/DiagramPropertiesBody.svelte` renders the
diagram-properties view inside `CentralEditPanel`, alongside the peer inspectors
`ObjectInspector` and `RelationshipInspector`. It reads as a mess for three
reasons:

1. **No structure.** A flat scroll of ~12 toggles plus Title/Note plus two
   stereotype lists, split only by three anonymous hairline dividers. Nothing to
   scan by.
2. **Off-convention styling.** It ignores the pattern its peers share. Peers use
   uppercase `slate-500` section labels, `#d8dee8` input borders, and
   `focus:ring-2 focus:ring-[#e6f1fb]` focus rings. This file uses
   `font-medium slate-800` labels, bare `border-slate-300` inputs, and no focus
   rings. Title/Note inputs in particular look unfinished next to the peers.
3. **Repeated disable logic.** The chain `!display.showAttributes || disabledAll`
   is duplicated across four attribute controls.

## Non-goals

- **No design-token migration.** The project uses Tailwind fully but leaves
  `theme.extend` empty, so brand colors (`#1e88e5`, `#f1f3f7`, ...) live as
  repeated arbitrary values across 26 files. Introducing a token layer is a
  separate, project-wide effort. This work matches the existing convention, it
  does not replace it.
- **No behavior change.** Every field, the `onChange` / `onUpdateDiagram`
  contract, the not-editable warning note, and all disabled behavior stay
  exactly as they are.
- **No control-type change.** The switch and segmented controls stay — they are
  the right control for on/off display flags. Peers lack them only because they
  edit text, not because switches are wrong here.

## Changes

### 1. Titled sections

Replace the three anonymous hairline dividers with four titled sections, each
introduced by an uppercase section label in the peer style. Contents and order
are unchanged; only the grouping headers are added:

- **Identity** — Title, Note
- **Attributes** — Show attributes, Attribute detail, Show visibility, Show
  multiplicity, Max attributes
- **Relationships** — Show roles, Show cardinality, Show labels
- **Stereotypes** — Show stereotype, Stereotype filter, Stereotype colors

The not-editable warning note stays at the top, above Identity.

### 2. Convention-aligned styling

Adopt the peer inspectors' shared field styles:

- Title and Note inputs use the peer input class: `#d8dee8` border, focus ring
  (`focus:outline-none focus:border-[#1e88e5] focus:ring-2 focus:ring-[#e6f1fb]`),
  `rounded-lg`, `text-[13px]`.
- Section headers use the peer label class:
  `text-[11px] font-semibold text-slate-500 uppercase tracking-[0.3px]`.
- The switch and segmented controls keep their internal look; only the
  surrounding section rhythm and labels change to match.

Colors reuse the same hex literals already used across the app — no new values.

### 3. Shared field-style module

`inputCls` / `labelCls` are currently copy-pasted across the inspector folder,
already drifting:

- `labelCls` — **identical** in `ObjectInspector`, `ObjectInspectorReadonly`,
  `RelationshipInspectorReadonly`.
- `inputCls` — **identical** in `ObjectInspector` (`py-2`) and
  `RelationshipInspector` (`py-[8px]`, same computed value); a **smaller
  variant** in `AttributeEditor` (`text-[12.5px] px-[7px] py-[5px]`).

Add `packages/web/src/components/inspector/field-styles.ts` exporting the two
canonical constants (`inputCls` = the ObjectInspector/RelationshipInspector
value, `labelCls` = the shared value). `DiagramPropertiesBody` imports both.

Migrate the exact-duplicate definers to import instead of redefining:
`ObjectInspector`, `RelationshipInspector`, `ObjectInspectorReadonly`,
`RelationshipInspectorReadonly`. Leave `AttributeEditor`'s smaller variant
alone — it is deliberately denser for the attribute grid, not a duplicate. This
gives one source for the shared styling without forcing a size change on the
grid.

### 4. Dedupe disable logic

Add `let attrDisabled = $derived(!display.showAttributes || disabledAll);` and
use it for the four attribute-dependent controls (Attribute detail, Show
visibility, Show multiplicity, Max attributes), replacing the repeated inline
chain. `stereoDisabled` already follows this pattern.

## Verification

- `DiagramPropertiesBody.test.ts` stays green with no changes — same fields,
  same accessibility labels, same `onChange` / `onUpdateDiagram` calls.
- Visual check in the running app: open the central edit panel on a diagram,
  confirm the four sections render, Title/Note show focus rings, and every
  toggle/segment/color control still drives its field.
- The four migrated inspectors (`ObjectInspector`, `RelationshipInspector`,
  `ObjectInspectorReadonly`, `RelationshipInspectorReadonly`) import the same
  class strings they defined before, so rendered markup is byte-identical and
  their existing tests stay green.
