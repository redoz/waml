# Read as scroll — a runtime book lens from the tree

**Status:** design, awaiting review
**Date:** 2026-08-11
**Follows:** `2026-08-11-book-mode-design.md` (Phase 1, landed)

## Problem

Book mode shipped, but the only way to open a folder as a book is to hand-write
`view: book` into its `index.md` frontmatter. That is a commitment — a
source edit, in git, shared with everyone — for what is often a passing
question: *what does this folder read like end to end?*

The reading lens should be reachable the way you reach any other view: right-click
the folder, pick it, look. Nothing written, nothing to undo.

## Goal

A **Read as scroll** entry on a folder's tree row that opens that folder as a
book, for any folder, without touching source.

## Non-goals

- **No frontmatter write.** This deliberately writes nothing. The shipped
  `view: book` declaration keeps its existing meaning (it decides what a
  *click* on a folder opens, via `App::primary_folder_locator`) and gains no
  new behavior here. Whether a runtime lens should ever be *persisted* is
  explicitly deferred — the point of this change is to use the lens for a
  while and find out.
- **No session mode, no override resolution order.** See "The shape this
  turned out to be" below — it needs neither.
- **No editing, bullets, or board lens.** Those remain Phases 2–4.

## The shape this turned out to be

The obvious design — a session-scoped "view override" per folder, consulted
ahead of the declaration when resolving a folder's surface — is not needed,
and building it would be a mistake.

A book tab is already a first-class document with its own locator:

```rust
DocumentLocator::new(RowTarget::Folder(directory), SurfaceId::book())
```

`book_documents::open` already opens **any** directory in the bundle, declared
or not — deliberately, so a stored book locator in history keeps working after
someone edits the declaration away. Its own module comment states the rule:
the declaration is a click-routing decision, not a capability gate. And
`tab_id_for` bakes the surface into the tab identity, so a directory's book tab
and its listing tab are already two different tabs that can both be open.

So "Read as scroll" is **not a mode at all — it is a navigation.** It opens a
locator that the system can already open. There is no new state, nothing to
persist, nothing to invalidate, and no second path to the book surface that
could drift from the declared one. Going "back to the list" is just that
folder's listing tab, which is a separate tab and may already be open.

This is the whole reason the feature is small. The work is almost entirely in
the tree's context menu, which cannot currently talk about folders.

## Design

### 1. Close the right-click asymmetry

Directory rows are already equal citizens on the **left-click** path.
`row_navigation` takes `is_directory` first and returns
`NavigationTarget::Directory { address }`; the row already carries both
`address` and `is_directory`, and directories open through the same
`NavigationIntent` machinery as everything else.

The **right-click** path is the one place that never got that treatment:

> Secondary button over a row: the node context menu. Openable,
> concept-carrying rows only — `App` dispatches the menu against a concept id,
> which no directory row has.

It reads `row.concept_id` and drops the row entirely when there isn't one, so
a directory row cannot open a menu even though the data to describe it is
sitting in the same struct.

The change is to make the secondary path speak the same vocabulary the primary
path already speaks: `ProjectTreeAction::ContextMenu` carries a
`NavigationTarget` (the tree's own currency, and what `row_navigation` already
produces) instead of a bare concept-id string. `App::handle_tree_context_menu`
then matches on it — `Document` keeps today's node menu and today's behavior,
`Directory` gets the folder menu below.

This is not a new abstraction; it is deleting a filter and reusing the type
next to it. It is worth doing properly rather than special-casing a folder
branch, because every future per-row action arrives through this same arm.

### 2. A folder row gets a folder menu

`popup::node_menu::base_items()` is concept-oriented; a folder needs its own
small item list. For this change that list has one entry:

- **Read as scroll** — icon: lucide `scroll`.

Adding one composed folder-item builder now (rather than special-casing a
single item) is what lets the projection entries, and later the bullets and
board lenses, land as items rather than as new menus.

### 3. The entry navigates

Committing the entry opens `DocumentLocator::new(RowTarget::Folder(dir),
SurfaceId::book())` through the ordinary navigation path — the same path a
stored book locator in history already takes. No new opening mechanism.

**One behavioral question to settle in the plan:** `handle_tree_context_menu`
currently calls `transition_document(cx, &key, false)` before showing the menu,
so right-clicking a concept row *opens* it. Doing that for folders means a
right-click opens the folder listing tab before you have picked anything, which
is rude for a menu you might dismiss. The folder path should show the menu
without opening a tab; the concept path keeps its current behavior, which is
established and expected.

### 4. The icon

Lucide `scroll` joins the icon catalog via `scripts/gen-icon.py`. The catalog
enforces one order across the enum, the fields, the DSL block, the getters, the
`ALL` list, and the labels, plus a count — the catalog test will fail loudly if
any of those disagree, which is the intended safety net rather than an
obstacle. Do not run `gen-all-icons.py`; it is stale.

### 5. Folders with no `index.md`

A folder that never declared anything is exactly the folder this feature is for,
so this case is the feature, not an edge of it.

Titles already work: `folder_documents::title_for` falls back to the last path
segment when there is no index title, and `book_documents` uses it.

What the plan must **verify rather than assume** is that
`analysis.bundle.directory(dir)` resolves for a directory that has files but no
`index.md`, since `book_documents::describe` gates on it and would otherwise
return `None` — the menu entry would appear and do nothing. If it does not
resolve, the fix belongs in the bundle/projection layer so that every consumer
sees such a folder consistently; it must **not** become a conditional inside
`build_book`, which would spread through Phases 2–4.

## Testing

Headless, in the shell-test style the book tasks used:

- A secondary press over a directory row emits a `ContextMenu` carrying
  `NavigationTarget::Directory` for that address; over a concept row it still
  emits one carrying `NavigationTarget::Document` for that concept, and the
  node menu it opens is unchanged.
- Committing **Read as scroll** on a folder row opens a document whose locator
  is that folder's book locator, and whose tab id differs from the folder
  listing's.
- The same folder opened via the menu and via a declared `view: book` produce
  the same model — one path, pinned.
- A folder with no `index.md` opens as a book and takes its title from the
  folder name.
- Right-clicking a folder row opens no tab.

Not headlessly verifiable, owed to a human: that the menu appears where the
pointer is, reads correctly, and that the glyph is the right one.

## Adjacent, deliberately not in this spec

The reveal-identity bug (a concept linked under two folders
captures tree clicks meant for its other row) touches the same navigation path
and was triaged as a bug, not a design question. It should be fixed, but it is
independent of this feature and should not ride along silently — it either gets
its own small change or a named task in this plan, not a quiet edit inside one.
