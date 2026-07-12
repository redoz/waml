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

Grow the center **diagram title switcher** (`TopBar.svelte`) into a **navigator sheet** that drops down from the top bar. It is a roomy **single-column** panel (not a tiny menu) and **stays open while working** so it can host file-manager interactions. This **replaces** the earlier left-panel idea.

### Visual reference

Full mockups (app colors, all screens): [`assets/2026-07-12-navigator-mockups.html`](assets/2026-07-12-navigator-mockups.html). The prose below is the behavior of record; where pixels and prose disagree, the prose wins.

**Main sheet** — scope `sales`, diagrams floated to the top of each package, on-hover drag grips (`⠿`):

```
┌────────────────────────────────────────┐
│ [ Search…                    ] [ All ▾ ]│  ← live search + type chip (Ctrl-T rotates)
├────────────────────────────────────────┤
│ acme-model / sales                      │  ← scope header (breadcrumb), glued to tree
├────────────────────────────────────────┤
│ ◫ ✓ Sales overview                    ⠿ │  ┐ sales' own diagrams float to top
│ ◫   Fulfilment flow                   ⠿ │  ┘
│ 📂 orders                             ⠿ │  ┐ then members in `members` order
│ │  ◫ Order lifecycle                    │  │   (sub-package diagrams float too)
│ │  ◻ Order                              │  │
│ 📂 refunds                            ⠿ │  │
│ │  ◻ RefundRequest                      │  │
│ ◻ Customer                            ⠿ │  │
│ ◻ Invoice                             ⠿ │  │
│ 🗒 Billing rules                      ⠿ │  ┘
└────────────────────────────────────────┘
```

**Left-click classifier → action menu · Right-click row → context menu:**

```
  Customer  ▸ ┌───────────────────────┐      orders ▸ ┌───────────────────────┐
              │ ◫ View in diagram  3 ›│               │ ↳ New package         │
              │ ＋ Add to new diagram │               │ ◻ New Class           │
              │ ───────────────────── │               │ ◫ New diagram         │
              │ ✎ View / edit props   │               │ ───────────────────── │
              └───────────────────────┘               │ ✎ Rename              │
                                                       │ ↕ Sort A–Z            │
                                                       │ ───────────────────── │
                                                       │ 🗑 Delete…            │
                                                       └───────────────────────┘
```

**Search — zero in scope, matches elsewhere** (filtered tree, matched substring `⟨…⟩`):

```
│ [ payment ]                    [ All ▾ ]│
│ acme-model / sales                      │
│           No matches in sales           │  ← short, centered, no icon
│ ──────── Elsewhere in model ─────────── │  ← divider
│ 📂 billing                              │  ← ancestor kept (full strength)
│ │  ◻ ⟨Payment⟩                          │
│ │  ◫ ⟨Payment⟩ flow                     │
│ 📂 catalog                              │
│ │  ◻ ⟨Payment⟩Method                    │
```

### Layout (top to bottom)

1. **Search + type filter row** — a live search field (filters as you type, no Enter — the dataset is small) and, beside it, a **type chip** showing the active metaclass filter (`All ▾`). `Ctrl-T` rotates the chip through the available types; the key hint is **not** rendered inline (it surfaces via the separate keyboard-shortcuts spec).
2. **Scope header** — glued to the top of the tree, reading as "*this is the root; you're viewing its children*": a styled ancestry breadcrumb (`root / … / current`). Click any crumb to pop back out to that scope. Root is the default scope; the root title tracks `ModelGraph.path`.
3. **Model tree** — the current scope's members, **fully expanded, no manual collapse**. Scoping (drill-in) is the narrowing mechanism, not collapse.

There is **no separate diagrams zone.** A diagram is an ordinary tree member (it lives in exactly one package, though its `members` may reference nodes anywhere), so it appears inline in the tree like any classifier or note.

### Tree rendering

- Every package member is a row: **sub-package · classifier · note · diagram**, each with a distinct icon.
- **Diagrams float to the top** of each package's listing (a soft split — diagrams first, then the rest), and the rule **recurses** into every sub-package. Not a labelled zone, just an ordering rule.
- **Order = the `members` array** (author-controlled). Below the floated diagrams, members render in `members` order.
- **Reorder** via on-hover drag grips; the new order **persists to `members`**. New members always **append to the end** — reordering is manual afterward.
- **Sort A–Z** (context-menu action) rewrites a package's `members` alphabetically. It is also the **default order applied when a package is first discovered from a folder**.
- The **active diagram** is checkmarked; clicking a diagram switches the active view.

### Interactions

- **Package row → rescope** the sheet to that package (drill in); scope header updates.
- **Diagram row → switch** the active diagram (reuses `onSelectDiagram`).
- **Classifier row (left-click) → action menu:**
  - **View in diagram** — in one diagram, go; in several, a submenu to pick; in none, fall through to *Add to new diagram*.
  - **Add to new diagram** — existing create-diagram flow, seeded with this classifier.
  - **View/edit properties** — **STUB (Spec 1): opens the existing Inspector.** Replaced by the A4 document editor in Spec C.
- **Right-click any row → context menu (file-manager ops):** *New package / New \<metaclass\> / New diagram* (created under this package), *Rename*, *Sort A–Z*, *Delete…*.
- **Tree editing:** create package, create a classifier under the current scope, drag-move between packages (reassign membership, enforce exclusivity, re-index), inline rename (materializes a ghost as needed), delete, multi-drag, reorder within a package.
- Package lifecycle (materialize/de-materialize) applies to all mutations.

### Create vocabulary (no "classifier" in the UI)

The word *classifier* stays internal (spec/code). The *New \<metaclass\>* items list **concrete metaclasses from the active profile's palette** (`packages/core/src/profiles`), labelled by de-prefixing the token — `uml.Class` → **Class**, `uml.Interface` → **Interface**, `uml.Enum` → **Enum**, `uml.DataType` → **DataType**. The navigator is **profile-agnostic**: it reads whatever palette is active. Binding a *profile/kind to a package* (so the palette narrows and the vocabulary re-dresses — e.g. an ERD package offering *New Table / New View*) is a **distinct capability deferred to its own spec (E)**; the palette-driven menu here is the forward hook.

### Search behaviour (filtered tree)

Search does **not** produce a flat list — it renders the **tree filtered in place**: matching rows shown **within their structure**, ancestor packages **kept (at full strength)** so a hit keeps its home, non-matching siblings pruned, and the matched substring **highlighted**. Default scope = the current package.

- **Matches in scope** → the current subtree, filtered.
- **Zero in scope, matches elsewhere** → a short, centered "**No matches in \<scope\>**", then an "**Elsewhere in model**" divider, then the elsewhere matches as their **own filtered tree** (real, clickable rows — not a teaser hint). This **supersedes** the earlier "subtle hint that never poses as a result" design: the out-of-scope matches are now first-class filtered-tree results, kept honest by the divider and the empty in-scope line above it.
- **Zero everywhere** → a bare, centered "**No matches found**"; no divider, no rows.
- **Clicking a package in search results** rescopes to it **and clears the search**.

### Wiring to existing surfaces

- Diagram rows → existing TopBar diagram handlers (`onSelectDiagram` / `onRenameDiagram` / `onCreateDiagram`).
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
- **Navigator (component, mirroring `TopBar.test.ts`):** breadcrumb rescope (+ clears search); **filtered-tree search** — matches render in-structure with ancestors kept and substring highlighted; zero-in-scope + matches-elsewhere → "No matches in \<scope\>" plus an "Elsewhere in model" filtered subtree; zero-everywhere → bare "No matches found"; `Ctrl-T` rotates the type filter; **diagrams float to the top** of each package; **reorder** persists to `members` and **new members append**; **Sort A–Z** rewrites `members`; classifier action-menu routing (view / add / edit-stub); create menu lists de-prefixed metaclass labels from the active palette; file-manager ops (create / move / rename / delete / reorder) mutate the model and re-index; **delete of a non-empty package** covers all three prompt branches (cascade / move-to-parent / cancel).

## Deferred (own specs)

- **C — Classifier document editor:** A4 modal, markdown-aware fields with live preview, raw-markdown escape hatch. See `2026-07-12-classifier-document-editor-design.md`. Stubbed here → Inspector.
- **D — LSP-in-editor:** completions/diagnostics under `uaml serve`. Rides the LSP track (`2026-07-12-uaml-lsp-design.md`).
- **E — Package profiles / kinds:** bind a profile (domain-model, ERD, …) to a package so the create palette narrows and its vocabulary re-dresses (e.g. ERD → *New Table / New View*). Spec 1's palette-driven create menu is the forward hook; the binding, the label maps, and additional profiles are all deferred here.
