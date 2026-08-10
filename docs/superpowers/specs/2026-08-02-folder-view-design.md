# Folder View — design

Date: 2026-08-02

## Problem

A folder in the project tree currently opens nothing. Clicking a folder row
only folds or unfolds it (`crates/waml-editor/src/tree_panel.rs:1565`). Yet
every directory already has an OKF `Index` (`crates/waml/src/okf.rs:235`) —
title, description, ordered members, body — backed by `<dir>/index.md`. That
document is unreachable from the editor.

Folders are also structurally untyped. `profile` exists only on concept
document frontmatter (`crates/waml/src/model.rs:950`, `crates/waml/src/seed.rs`
emits `type: uml.ClassDiagram` + `profile: uml-domain`). A directory cannot say what it
is, so it cannot say how it should be shown.

Goal: a folder declares what it is (`profile`) and how it is shown (`view`),
and opening a folder shows something useful. The end state is a Workflowy-style
outliner over OKF, reached in two steps.

## OKF spec posture

`docs/specs/OKF_SPEC.md` is an external standard, not ours. It stays
byte-identical. We do not amend it, extend it in place, or add reserved
filenames.

This design introduces exactly one deviation: **frontmatter in a non-root
`index.md`**. OKF §6 says index files contain no frontmatter, and §11 permits
it only in the bundle-root index for `okf_version`.

The deviation already exists. `parse_authored_index`
(`crates/waml/src/okf/shell.rs:434`) reads `shell.frontmatter.get_str("title")`
from any index.md today. This design extends that one deviation with two more
keys and does not add others.

Degradation for a strict OKF consumer: the frontmatter block renders as a YAML
block or is skipped. No member, link, or body content is affected. A bundle
authored by waml remains readable by any OKF consumer.

Explicitly **not** deviating:

- Members stay flat and link-only. No nested index lists.
- No plain-text (non-link) bullets in an index list. Every node is a real file
  or directory.
- No new reserved filenames, no sidecar files.

Our deviations are documented in `docs/specs/waml-okf-extensions.md` (new), one
entry per deviation, each stating its strict-consumer degradation.

## Model

### `okf::Index`

Three new fields:

```rust
pub struct Index {
    pub directory: DirectoryAddress,
    pub title: Option<String>,
    pub description: Option<String>,
    pub members: Vec<String>,      // unchanged: flat, link-backed ids
    pub body: Option<SourceSlice>,
    pub authored: bool,
    pub profile: Option<String>,   // new
    pub view: Option<ViewSpec>,    // new
    pub extra: Frontmatter,        // new: producer keys survive round-trip
}
```

`members` is unchanged. Hierarchy comes from `okf::Directory`
(`child_directories` + `concepts`), which already models it.

`profile` and `view` as stored are *local declarations only*. Inheritance and
fallback are computed by queries (below), never baked into the struct, so
"what is on disk" and "what is in effect" are never confused.

### `ViewSpec`

```rust
pub enum ViewSpec {
    Index,            // rendered listing (the fallback)
    Outline,          // Index plus editing
    Member(String),   // delegate to one member's own view
    Markdown,         // raw index.md in the markdown editor
}
```

Serialized in frontmatter as a single scalar string, never a nested mapping:
`view: index`, `view: outline`, `view: markdown`, or `view: member:./orders`
(the `member:` prefix followed by an href, no space, so the YAML value stays a
plain scalar). An unrecognized `view` value is treated as absent and falls
through to the next resolution step rather than erroring.

### `ProfileDef`

Profiles today are a bare frontmatter string with no definition anywhere. This
design adds a definition as a Rust data type, not a file format:

```rust
pub struct ProfileDef {
    pub name: &'static str,
    pub default_view: Option<ViewSpec>,
}

pub fn profile(name: &str) -> Option<&'static ProfileDef>;
```

A static table, no trait. Ships with `uml-domain` and `okf`, both
`default_view: None` — today's behavior is preserved and outline is opt-in per
folder until real use justifies a profile that assumes it.

The full profile system (legal element types, child templates, validation) is
out of scope and remains deferred. When it lands, `profile()` grows a bundle
argument and the static table becomes the fallback; call sites do not change.
No profile *file format* is specified or half-specified here.

## Resolution

Two pure queries on `Bundle`, unit-testable with no editor involved.

`resolved_profile(dir) -> Option<&str>` — nearest declaring ancestor, self
first. Walking stops at the first index that declares a `profile`. An explicit
declaration always wins over an inherited one, so a child can opt out of a
parent's profile by declaring its own.

`resolved_view(dir) -> ViewSpec`:

1. the index's own `view:`, if declared
2. else `resolved_profile(dir)`'s `default_view`, if any
3. else `ViewSpec::Index`

Three steps, no special cases. Note step 2 uses the *inherited* profile: marking
`/sales` with a profile gives every folder beneath it that profile's default
view without restating it.

There is no auto-detection. A folder holding exactly one diagram does not
silently resolve to `Member`; you write `view: member: ./orders`.

## Round-trip

`render_index` (`crates/waml/src/index_md.rs:42`) emits no frontmatter today.
Any write path that re-renders an index would therefore silently erase a
folder's `profile:` and `view:`. Emitting frontmatter — including preserving
unknown keys via `extra` — is part of this change, not a follow-up. A
round-trip test (parse an index with frontmatter, render it, reparse, assert
equality) is a required unit.

## Views

`Index` and `Outline` are **one widget in two modes**, not two widgets. Both
render the directory's members as titled rows with blurbs; `Outline` sets
`editable: true`. Building them separately would produce two layouts that
drift.

Rows come from `okf::Directory` — child directories and concepts — ordered by
the index's authored member order, with unlisted items appended. Each row shows
a bullet, the title, and an optional blurb taken from the concept's frontmatter
`description`.

**`Index` (read-only).** Clicking a row opens that concept, or that child
folder's own resolved view.

**`Outline` (editing).** Same rows, plus:

- Enter — create a new concept `.md` in this directory, inserted into the
  index's members at that position
- typing in a row — retitle the concept (H1 and frontmatter title); not a file
  rename
- Tab / Shift-Tab — move the concept into the preceding sibling directory, or
  out to the parent; rewrites both index.md files
- drag a row — reorder within the index's members; no file is touched
- click a bullet — zoom: a child folder opens its resolved view, a concept
  opens the concept

Every edit maps to an existing OKF op or a small composite of them. Nothing
bypasses the model to write files directly.

**`Member(id)`.** Opens that member's own view in the folder's tab slot.

**`Markdown`.** The existing markdown editor over index.md, for raw editing.

## Tree behavior

Folder rows gain a second action. The chevron folds and unfolds; the row body
opens the folder's resolved view as a tab. Folder tabs use an icon distinct
from file tabs.

This is a behavior change to an existing surface — today the whole row folds —
and needs its own verification.

## Delivery order

1. Frontmatter on `Index`: parse `profile`/`view`/`extra`, emit them in
   `render_index`, round-trip test. No UI.
2. `ProfileDef` table and the two resolution queries. No UI.
3. `Index` view, read-only, plus the tree row-versus-chevron split. Folders
   open something for the first time.
4. `Outline` mode: editing on the proven surface.

Steps 1 and 2 are pure model work with no editor dependency. Step 3 is
immediately useful on its own. Step 4 adds editing to a surface that already
works rather than introducing a new surface and editing at once.

## Open question

**Tab on a concept with no preceding sibling directory.** Workflowy indents
under the bullet above. Here that means promoting a concept to a directory:
`orders.md` becomes `orders/index.md`. Legal in OKF and reversible, but it
turns a keystroke into a structural change. The alternative is for Tab to
refuse unless a real directory precedes.

Undecided. It only affects step 4 and does not block steps 1 through 3.

## Testing

- `DirectoryAddress`/`Index` parse: frontmatter keys promoted, unknown keys
  land in `extra`, an index with no frontmatter parses as it does today.
- Round-trip: parse, render, reparse, assert equality, including unknown keys.
- `resolved_profile`: self wins over ancestor; nearest ancestor wins over
  further; none declared yields `None`.
- `resolved_view`: each of the three steps in isolation, and an explicit local
  `view` beating an inherited profile default.
- Outline ops: each edit produces the expected OKF op batch and leaves both
  affected index files consistent.
- Tree: chevron folds without opening; row body opens without folding.
