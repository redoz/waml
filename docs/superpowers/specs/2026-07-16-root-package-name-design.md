# Root package name replaces the phantom "model name"

**Date:** 2026-07-16
**Status:** Design — approved, pending spec review

## Problem

The top bar shows a static `Model Canvas` subtitle beside the brand, and a
separate `modelName` string (localStorage `mc.modelName.v1`, default
`"My first WAML model"`) feeds the export filename, share-link name, and image
name. Neither is connected to the bundle. `modelName` is a leftover from the
"Model Canvas" origin — a phantom name with no home in the data.

The real name of a WAML bundle is its **root package**: the root `index.md` H1,
surfaced as `ModelGraph.path` (`crates/waml/src/model.rs` — *"Bundle/root name
(root index.md H1); '' when absent"*). The navigator root crumb and `nav/tree.ts`
already treat `path` as the root package name. `modelName` duplicates that badly.

## Goal

Delete `modelName` entirely. The bundle has one name: its root package title.
Everything — top-bar subtitle, export filename, share, image name — reads from
`$model.path`. The name is editable inline from the top bar.

## Guiding principle: a package is a package

Root, child, grandchild — the code must not special-case the root. The rename
operation, its DTO, and the store method are **generic over any package key**.
The root package (key `""`) is simply the instance the top bar wires up; the same
op can rename any package (e.g. from the navigator) later with no new code. The
only place the key `""` matters is the unavoidable path arithmetic that maps a
package key to its index file (root → `index.md`, else → `<key>/index.md`) — pure
path joining, already handled uniformly by `reindex_bundle`.

## Reading (display)

- Top-bar subtitle = `$model.path`.
- Unnamed bundle (`path === ""` — empty default, or a template with no root
  `index.md`): a muted `Untitled` placeholder that still invites naming.

## Writing — inline rename with hover pencil

- Hovering the brand reveals an edit pencil beside the name.
- Clicking the name or the pencil opens an inline text input seeded with the
  current name. Enter or blur commits; Esc cancels.
- Commit routes through the store to a new bundle op.

## New bundle op: `pkg.retitle { path, title }`

Threads through every layer, generic over `path` (the package key; root = `""`):

- Rust `Op::PkgRetitle` in `crates/waml/src/ops/pkg.rs`
- `waml-ops-dto` DTO variant
- `waml-wasm` op mapping
- core `retitlePackageOps(key, title)` in `state/ops-adapter.ts`
- store `retitlePackage(key, title)` in `state/model.ts`

### Behavior

- Resolve the package `path`, then write/create its index file
  (`<path>/index.md`, root → `index.md`) with H1 `# {title}`, **preserving the
  existing intro prose and member listing**. Reuse the `write_package_index`
  machinery.
- If the package has no `index.md` yet, create one: the H1 plus an
  auto-generated member listing (root members are discovered from top-level
  docs).
- Empty / whitespace-only `title` is rejected (an edit cannot blank the name).

### Fix `render_index` to be title-aware

`crates/waml/src/index_md.rs:24` currently derives every index.md H1 from the
directory name (hardcoded `"index"` for root). That means a later reorder/sort
would silently reset a custom title. Make `render_index` take the package's
current title (callers pass `pkg.concept.title`, which was parsed from the
existing H1) and emit it verbatim, falling back to the directory basename only
when no title is set. This fixes the latent clobber for **all** packages, not
just root.

## Removals / rewires

- Delete `packages/core/src/state/modelName.ts` and every import
  (`loadModelName`, `persistModelName`, `DEFAULT_MODEL_NAME`,
  `templateModelName`).
- `CanvasInner.svelte`: drop the `modelName` state and its persist effect;
  `imageName`, the export filename, and share all derive from `$model.path`
  (fallback `"waml-model"`).
- Share URL: drop the separate `&n=` name param, `readSharedName`, and
  `sharedModelName` (`packages/core/src/share/url.ts`, `bootstrap.ts`). The
  bundle now carries its own name in the root `index.md`, so the recipient reads
  it directly. Old share links that relied on `&n=` open as `Untitled` — old
  links are ephemeral; no back-compat plumbing is kept.
- Templates: drop `templateModelName`. A template's name comes from its own root
  `index.md`; templates without one open under the `Untitled` placeholder.
- No migration of `mc.modelName.v1` — the value is already invisible (the old
  editable field was removed when the diagram switcher landed), so it is simply
  ignored.

## Testing

- **Rust** (`ops/pkg.rs`, `index_md.rs`): retitle creates an index.md H1 when
  absent; retitle preserves members + intro; a custom title survives a
  subsequent reorder/sort; empty title rejected; retitle works identically for a
  nested package key and for root `""`.
- **core**: `retitlePackageOps` shape; store `retitlePackage` round-trips
  through `apply_ops` and updates `$model.path`.
- **TopBar.svelte**: renders the root package name; pencil appears on hover;
  clicking opens the input; Enter fires the rename callback; Esc cancels without
  firing.
- **CanvasInner**: export filename / image name / share read `$model.path`.

## Risks

- Making `render_index` title-aware changes reindex output for nested packages
  that carry a custom H1. Cover with golden tests; keep the directory-basename
  fallback so packages without a custom title are byte-identical to today.
