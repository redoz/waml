# New Package Flow

**Status:** Design
**Date:** 2026-07-16

## Problem

The app began life as one canvas / one diagram. It now holds a hierarchy of
OKF bundles (WAML packages), but the creation surface still reflects the old
model:

- The template library applies a template by dumping its baked-in folder into
  the canvas, offering only **Replace** vs **Merge** (`TemplateApplyDialog`).
- There is no way to choose *where* in the package tree new content lands, or
  to name the package it creates.
- The TS merge helper (`mergeBundles`) dedupes incoming docs by **bare filename
  slug, globally**, which is inconsistent with the Rust core's identity model
  (full path) and can silently drop a template's doc when an unrelated doc of
  the same filename exists in another package, leaving its links dangling.

## Goal

Replace the "apply template" flow with a **New Package** flow. Creating a
package is the single act; a template is just one of three ways to seed that
package's contents. The user always chooses the tree location and the package
name. All bundle manipulation moves into the Rust core, where path identity is
already correct, so the TS layer only orchestrates the UI.

## Non-goals

- Cross-package linking by bare document name. The core resolves links
  directory-relative (`resolve_href`); this design does not change that and
  does not introduce name-based cross-package references.
- Reworking the OKF *import* dialog's UX. Its merge path is re-pointed at the
  new Rust op (removing the global-slug bug) but its dialog stays as-is.

## Background: identity is path-based

The Rust core (authoritative, via `build_model`) keys every node by its full
bundle path: `id_of("Sales/Orders/order.md") == "Sales/Orders/order"`. Links
written as `./order.md` / `../shop/order.md` resolve relative to the referring
document's own directory (`okf.rs:resolve_href`). Therefore
`Sales/Orders/order` and `Billing/order` are distinct nodes and neither can
reach the other through `./order.md`. Re-rooting a template's whole folder to a
new package path keeps every `./`-relative link inside it valid.

The only component out of step is the TS `mergeBundles`, which dedupes by
basename globally. This design removes that helper's merge role.

## Design

### 1. The New Package dialog

Repurposes `TemplateApplyDialog` into a single modal with three stacked zones.

**Tier selector (top):** three cards.

- **Empty** - a package with no contents.
- **Diagram** - a package holding one empty diagram of a chosen kind.
- **Template** - a package seeded with a full example (today's templates:
  diagram + sample data).

**Contextual middle (depends on tier):**

- Empty: nothing.
- Diagram: a 4-kind chooser - **Class / Domain**, **Use-case**, **Activity**,
  **Sequence**.
- Template: the existing gallery (the `TEMPLATES` list, name + description).

**Placement footer (always shown):**

- Inline **package-tree picker** - a compact, selectable rendering of the
  current package tree (the same tree the Navigator shows). The tree's root is
  the project itself (the implicit root package / project name) and is a valid
  target. Selecting a package sets the insert parent.
- **Name** field - the new package's name.
- **Add** button.

**Name defaults:** Empty -> "New package"; Diagram -> the chosen kind's label
(e.g. "Activity"); Template -> a cleaned version of the template name.

**Collision:** if `<parentPath>/<name>` already exists, **Add is blocked**
(disabled, inline message "name already used here"). No auto-suffix; the user
picks another name or location.

The Navigator's existing inline "New package" quick-add remains as a shortcut
for the Empty tier; the dialog is the one full path covering all three tiers.

### 2. Rust core: pkg.insert op + seed generator

All bundle manipulation lives in the core and is applied through the existing
`apply_ops` pipeline (same mechanism as `pkg.move` / `pkg.rename` /
`pkg.delete`).

**New op `pkg.insert`** with input `{ parentPath, name, docs }`:

1. Compute the target prefix `parentPath/name/` (or `name/` at root).
2. If a package already exists at that path, return an `OpError` (surfaced to
   the dialog as the collision block; the UI also checks up front to disable
   Add, but the op is the authority).
3. Re-root every doc in `docs`: strip its incoming top-level folder and prepend
   the target prefix. `./`-relative links are unchanged and stay valid.
4. Append into the working bundle. Identity is the full path, so distinct paths
   never collide; re-adding the identical path is the only true collision and
   is prevented by step 2.

**Seed generator `new_diagram_doc(kind, name)`** (Rust): returns the single
diagram document's markdown for a kind, with empty members:

- Class / Domain: `type: "Diagram"`, `profile: "uml-domain"`.
- Use-case: `type: "Diagram"`, use-case profile.
- Activity: `type: "uml.Activity"`.
- Sequence: `type: "uml.Sequence"`.

Exposed via a `#[wasm_bindgen]` entry (or an `OpDto` variant consumed by
`apply_ops`, whichever fits the existing DTO surface).

**Empty tier** uses the existing create-package / ghost-package op; no new code.

### 3. Remove the TS merge path

`mergeBundles`' role (global-basename dedup) is deleted. The template/new-package
insert goes through `pkg.insert`. The OKF-import "merge" mode is re-pointed at
the same op so the global-slug bug is fixed at the source rather than per
caller. `mergeGraphs` (graph-level remap for a different path) is reviewed; if
it shares the defect it is migrated too, otherwise left untouched.

### 4. Create-new project

A TopBar "Create new" action, folded into this spec:

- Opens a confirm dialog: "This will close the current project - your work is
  saved." (Everything autosaves; the confirm only guards the context switch,
  reusing the existing confirm-dialog pattern, e.g. Clear Canvas.)
- On confirm: `store.load([])` and reset the project name to the default.

## Data flow

1. User opens New Package dialog, picks tier (+ kind or template), a parent in
   the tree, and a name.
2. TS builds the op input: Empty -> create-package op; Diagram ->
   `new_diagram_doc(kind, name)` as the single doc for `pkg.insert`; Template ->
   the template's bundle as `docs` for `pkg.insert`.
3. `apply_ops` runs the op in wasm, returning the new bundle.
4. `store.load(newBundle)`, then lay out the newly-added keys.

## Error handling

- Package-name collision: dialog disables Add with an inline message; the
  `pkg.insert` op independently returns `OpError` as a backstop.
- Empty / whitespace name: Add disabled.
- Unknown / missing profile for a diagram kind falls back per the core's
  existing `getProfile` behavior (never errors).

## Testing

- Rust: `pkg.insert` re-roots paths, preserves `./` links, errors on collision,
  keeps distinct same-filename docs across packages. `new_diagram_doc` emits
  valid frontmatter for each of the 4 kinds.
- Regression: a bundle containing two `order.md` in different packages round-
  trips through insert without either being dropped (the old `mergeBundles`
  bug).
- Svelte: dialog tier switching, tree selection sets parent, name-collision
  disables Add, each tier calls the right op with the right input.
- Create-new: confirm gate; on confirm the model resets and name defaults.

## Open implementation details (resolved at plan time)

- Exact use-case profile name string.
- Whether `pkg.insert` is a new `OpDto` variant or a standalone wasm export.
- The compact tree-picker component: extract from `NavigatorBody` vs a slim
  purpose-built selectable tree.
