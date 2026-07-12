# UAML Packages + Model Navigator — Design

**Date:** 2026-07-12
**Scope:** Spec 1 of a decomposed effort — parts **A** (package model) and **B** (navigator sheet). Parts **C** (classifier document editor) and **D** (LSP-in-editor) are captured in `2026-07-12-classifier-document-editor-design.md` and deferred.

## Goal

Give the model first-class **packages** (UML-style namespaces), backed by OKF directories + `index.md`, and a **navigator sheet** — the existing center diagram switcher grown into a searchable, type-filterable tree — to browse and reorganize packages, classifiers, and diagrams.

Non-goals for this spec: the rich classifier document editor (A4 modal, markdown-aware fields, raw-markdown escape hatch) and LSP integration. The navigator's *View/edit properties* action **stubs to the existing Inspector**.

## Background — current state

- `ModelGraph` is flat: `nodes`, `edges`, `diagrams` (`packages/okf/src/types.ts`). Diagrams are curated **views** over nodes; a node can appear in many. There is no namespace/ownership grouping.
- The OKF serializer (`packages/okf/src/serialize.ts`) already emits a bundle: a folder of concept `.md` files plus a frontmatter-less `index.md`. `index.md` is a **reserved filename**, excluded from nodes, and lists documents.
- OKF spec (`docs/specs/OKF_SPEC.md`): a bundle is a **directory tree of markdown** (§3); **subdirectories organize concepts into groups** (§3, line 82); `index.md` is optional, contains **no frontmatter** (§6, line 278), enumerates a directory's contents for **progressive disclosure**, with entries `* [Title](url) - description`, and consumers **MAY synthesize** it when absent (§6, line 293).
- The center control today is the **diagram title switcher** (`packages/web/src/components/TopBar.svelte`) — switch / rename / create diagrams.

---

## Part A — Package model

### Core concept

- **Package = a directory. 1:1.** Runtime node type `uml.Package`. To split into two packages, create two folders. No intra-folder sub-grouping.
- **Discovery is bottom-up from children.** A directory with children *is* a package; its children *are* its members. `index.md` is the **written record** — regenerated, never the source of truth.
- **Root package** = the bundle root directory. Its `title` = `ModelGraph.path` (the model/bundle name). Top-level documents are its members.
- **Nesting = the tree.** A package member serializes to a subdirectory; a classifier member to a concept `.md`; a `uml.Note` doc is a member (a leaf). **Edges are not tree members** — they ride their source document's `## Relationships` section (existing behavior).
- **Exclusivity is structural** — a file lives in exactly one directory, so a node belongs to exactly one package. Enforced in all tooling.
- **Empty package = runtime-only ghost.** It shows in the tree but is not written until it gains its first child (nothing to discover yet). First child → materialize (directory + `index.md`). Last child leaves → de-materialize (prune directory + `index.md`). Ghosts do not survive reload.

### Runtime data model

- Add `uml.Package` as a metaclass in the `uml-domain` profile palette (`packages/core/src/profiles/umlDomain.ts`). It is already anticipated by the Rust tooling design.
- Packages reuse `ModelNode`:
  - `type: "uml.Package"`, `title`, `description`.
  - `members: string[]` — owned node keys (classifiers + sub-packages), **ordered** (order is progressive-disclosure order). Meaningful only on `uml.Package` nodes.
- `ModelGraph` gains `path: string` — the bundle/root name; export label. The **root package** node (`title` = `path`) owns all top-level nodes.
- **Keys remain the primary reference** everywhere (edges, diagram members, package members) — rename-safe. **Paths are derived** from package nesting; the serializer's `slugByKey` map extends from single-segment (`order`) to multi-segment (`sales/order`).
- Tree helpers (new, in `packages/core/src/state/` or `okf`): build a child→parent index from `members`; resolve a package's ancestry chain; resolve a package by path.

### Serialization / round-trip (OKF-compliant)

**Save** (`serializeBundle`):
- Each `uml.Package` node → a directory.
- Its `index.md` = optional **intro prose** (from the package `description`) followed by a **frontmatter-less listing**: `* [Title](relative-url) - blurb` per member, subfolders as `* [Sub](sub/) - blurb`. Blurb = the member's `description` first line (OKF §6, line 292).
- Classifier members → concept `.md` via existing `renderNode`. Package members → subdirectory (recurse). Edges remain under their source document.

**Load** (`parseBundle`):
- Walk the directory tree. Each directory → a `uml.Package` node (`title` = directory name; `description` = its `index.md` intro prose, if any).
- **Children are discovered from actual directory contents** (authoritative). `index.md` is parsed only for **member order and blurbs**, then reconciled: add newly-present docs, drop absent ones, preserve order/blurbs for survivors. A stale `index.md` is never an error.
- Concept `.md` → nodes via existing parse. Directory nesting → `members`.

**Reserved names:** `index.md` and `log.md` are never concept documents; no package or classifier may slug to those. Slug uniqueness is enforced **per directory**.

### Migration

- On load, `migrateGraph` (`packages/okf/src/migrate.ts`) wraps an existing flat model in a **single root `uml.Package`** (`title` = current model name, from `modelName.ts`; sets `ModelGraph.path`), with all current nodes as its members.
- Runtime-only shape change; the existing serializer already emits the root `index.md`. **Zero behavior change** until a user creates a subfolder/package. Existing tests stay green.

### Invariants

1. Every non-root node key appears in **exactly one** package's `members`.
2. The root package is owned by none; `ModelGraph` locates it via `path`.
3. Empty package = ghost; materialize on first child; prune when empty.
4. Move = remove from old `members`, add to new, atomically; re-index both affected packages' `index.md`.

---

## Part B — Navigator sheet

### Placement

Grow the center **diagram title switcher** (`TopBar.svelte`) into a **navigator sheet** that drops down from the top bar. It is a roomy panel (not a tiny menu) and **stays open while working** so it can host file-manager interactions. This **replaces** the earlier left-panel idea.

### Layout (top to bottom)

- **Ancestry breadcrumb** — a styled chain of the currently scoped package (`root / … / current`). Click any crumb to rescope. Root is the default scope.
- **Search** — live-filters the tree by title. Default scope = the current package; a toggle widens to the whole model.
- **Type filter** — narrow to a single metaclass (e.g. only `uml.Class`, or only packages).
- **Model tree** — packages and classifiers under the current scope.
- **Diagrams zone** — the existing views; selecting one switches the active diagram (reuses `onSelectDiagram` / `onRenameDiagram` / `onCreateDiagram`).

### Interactions (full file-manager)

- **Package click → rescope** the sheet to that package (drill in); breadcrumb updates.
- **Classifier click → action menu:**
  - **View in diagram** — in one diagram, go; in several, disambiguate (pick which); in none, fall through to *Add to new diagram*.
  - **Add to new diagram** — existing create-diagram flow, seeded with this classifier.
  - **View/edit properties** — **STUB (Spec 1): opens the existing Inspector.** Replaced by the A4 document editor in Spec C.
- **Tree editing:** create package, create classifier under the current scope, drag-move between packages (reassign membership, enforce exclusivity, re-index), inline rename (materializes a ghost as needed), delete, context menus, multi-drag, reorder within a package (reorder persists as `index.md`/`members` order).
- Package lifecycle (materialize/de-materialize) applies to all mutations.

### Wiring to existing surfaces

- Diagrams zone → existing TopBar diagram handlers.
- Edit properties → existing Inspector.
- Add-to-diagram → existing diagram membership operations.

---

## Error handling / edge cases

- **Stale `index.md`** (lists a deleted doc, or misses a new one) → reconcile on load, not an error.
- **Broken cross-links** → tolerate (OKF §5).
- **Slug collisions within a directory** → existing `slugify` + counter, now scoped **per directory**.
- **Reserved-name collision** (`index` / `log`) → disambiguate the slug.
- **Empty scope** (package containing only ghosts) → empty-state in the sheet.
- **Delete a non-empty package** → **prompt** with three choices: **Delete children too** (cascade — remove the package and everything under it), **Move to parent** (reparent children to the package's parent, then remove the empty package), or **Cancel**. Never silent. Deleting an empty ghost just removes it, no prompt.
- **Root** is always a valid move target, so a move can never orphan a node.

## Testing

- **Round-trip:** model → bundle → model with nested packages; intro prose and blurbs preserved; reconcile against changed directory contents.
- **Discovery:** from a nested directory-tree fixture, membership equals directory children.
- **Exclusivity:** move updates both `members` lists; a node is never in two.
- **Lifecycle:** materialize on first child; de-materialize on last child out; ghost empty-package does not survive reload.
- **Index regen:** `index.md` regenerated with correct order; order reconcile on load.
- **Migration:** flat model → root package; existing serializer/tests unaffected.
- **Navigator (component, mirroring `TopBar.test.ts`):** breadcrumb rescope; search filter; type filter; classifier action-menu routing (view / add / edit-stub); file-manager ops (create / move / rename / delete / reorder) mutate the model and re-index; **delete of a non-empty package** covers all three prompt branches (cascade / move-to-parent / cancel).

## Deferred (own specs)

- **C — Classifier document editor:** A4 modal, markdown-aware fields with live preview, raw-markdown escape hatch. See `2026-07-12-classifier-document-editor-design.md`. Stubbed here → Inspector.
- **D — LSP-in-editor:** completions/diagnostics under `uaml serve`. Rides the LSP track (`2026-07-12-uaml-lsp-design.md`).
