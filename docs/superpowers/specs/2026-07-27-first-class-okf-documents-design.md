# First-Class OKF Documents and UML Projection — Design

**Date:** 2026-07-27

**Status:** Approved in conversation; written-spec review pending

**Builds on:** `2026-07-11-okf-agnostic-profiles-uml-domain-design.md`,
`2026-07-23-diagram-view-seam-design.md`, and
`2026-07-27-editor-architecture-ownership-design.md`

## Problem

WAML already uses Open Knowledge Format (OKF) Markdown as its source format, but
its internal and editor models still treat UML as the effective product domain:

- every Markdown file projects to `okf::Concept`, including reserved
  `index.md` and `log.md` files;
- `parse::build_model` also coerces unrecognized, non-UML documents into
  `model::Node` values with `ElementType::Unknown`;
- directory structure is synthesized as `uml.Package` nodes;
- the navigator obtains its hierarchy from the UML-oriented `Model`;
- `TreeKind::Unknown` rows are deliberately not openable;
- package/index operations rebuild the UML model to discover directory
  membership and titles;
- the operation vocabulary combines OKF source/directory operations and UML
  modeling operations in one `Op` enum.

Consequently, an ordinary OKF Concept is preserved by the parser but is not a
first-class document in the editor. It appears internally as an unsuccessful
UML classification and has no document view.

## Goal

Promote OKF to a first-class supported document domain while retaining UML as a
specialization layered on the OKF Knowledge Bundle:

```text
OKF Knowledge Bundle
├── recognized UML document -> UML projection and UML views
└── unclaimed Concept        -> Generic OKF Markdown view
```

The design must:

1. make the OKF Knowledge Bundle the domain-neutral semantic root;
2. project UML only from recognized OKF Concepts;
3. make every otherwise-unclaimed Concept openable as a Generic OKF document;
4. give Generic OKF documents a dedicated Markdown-only `DocView`;
5. remove cross-domain document-kind dispatch from the editor host;
6. split OKF and UML operation ownership without building a plugin system;
7. preserve one atomic, source-authoritative edit transaction; and
8. avoid retaining multiple complete copies of the bundle's Markdown.

## OKF terminology

This design follows the
[Open Knowledge Format specification](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md):

- **Knowledge Bundle** (or Bundle): the self-contained hierarchical unit of
  distribution.
- **Concept**: one unit of knowledge represented by a non-reserved Markdown
  document.
- **Concept ID**: the Concept file's path within the Bundle, with the `.md`
  suffix removed.
- **Directory**: a physical Bundle subdirectory used to organize Concepts.
- **Index**: the reserved `index.md` document for a directory.
- **Log**: the reserved `log.md` document for a directory.

The core layer must not call directories or indexes "packages." `uml.Package`
remains valid UML terminology only for explicitly recognized UML elements.

## Chosen approach

Use statically composed domain projections and document providers.

There is no global `DocumentKind` enum and no runtime provider registry. Each
domain owns its closed vocabulary:

- OKF owns Bundle, Concept, Directory, Index, Log, and generic Markdown
  presentation.
- UML owns its classifiers, diagrams, behaviors, relationships, and UML view
  implementations.
- one composition point tries the UML provider first and falls back to the
  Generic OKF provider.

Adding a future built-in specialization would add another explicit projector at
that composition point. It would not require changing OKF core types, a global
family enum, or `DocumentHost`.

### Rejected alternatives

**Make `TreeKind::Unknown` open a source tab.** This is a small UI patch but
retains the incorrect representation of Generic OKF Concepts as malformed UML
nodes.

**Introduce a global document-family enum.** This moves coupling rather than
removing it: every new specialization changes the enum and every exhaustive
match.

**Build a plugin/provider registry.** Two statically known domains do not
justify runtime registration, discovery, or plugin lifecycle machinery.

## Core OKF Knowledge Bundle

### Source and semantic representations

The opened content has two related representations:

```text
SourceBundle       exact paths and Markdown used for operations and persistence
okf::Bundle        parsed semantic Knowledge Bundle sharing the same source text
uml::Projection    recognized UML semantics derived from okf::Bundle
```

`okf::Bundle` replaces the proposed `okf::Project` name. It is the semantic
root Bundle, not a UI project or persistence container.

### Separate Concept, Index, and Log types

Reserved files are not Concepts. Replace `ConceptRole` with separate domain
types:

```rust
pub struct Bundle {
    // Logical shape; lookup indexes may differ in the implementation.
    pub concepts: Vec<Concept>,
    pub indexes: Vec<Index>,
    pub logs: Vec<Log>,
    pub directories: Vec<Directory>,
}
```

An `Index` belongs to one directory and represents either an authored
`index.md` or a synthesized index view when that file is absent. Mutating a
synthesized Index materializes the corresponding `index.md`.

A `Log` belongs to one directory and preserves the reserved log document
separately from ordinary Concepts. Parsing detailed log entries is not required
by this feature; retaining its source and identity is sufficient.

`ConceptRole` and code paths that treat Index or Log as a Concept are removed.
Wire compatibility may use transitional deserialization or re-exports, but new
internal code must use the separate types.

### Identity and lookup

Concept IDs remain bundle-relative, case-preserving, slash-normalized paths
without the final `.md`:

```text
order.md                  -> order
sales/orders/order.md     -> sales/orders/order
```

They are never absolute filesystem paths. IDs must be unique after
normalization.

Directories use rooted Bundle addresses:

```text
/             -> Bundle root
/sales        -> sales/
/sales/orders -> sales/orders/
```

The root directory always exists. Bundle lookup resolves its root address to
the root Index:

```text
bundle.index("/")             -> index.md or synthesized root Index
bundle.index("/sales")        -> sales/index.md or synthesized Index
bundle.concept("sales/order") -> sales/order.md
```

This keeps `/` out of the Concept-ID namespace while providing the expected
root lookup behavior.

### Directory hierarchy

Directory membership, title, description, Index ordering, and child-directory
relationships move from UML `Model::packages` into `okf::Bundle`.

An explicitly authored `type: uml.Package` document remains a UML element. A
directory does not become `uml.Package` merely because it exists.

`index_md::reindex_bundle` and OKF directory/index operations use the OKF Bundle
instead of calling `parse::build_model`.

## UML projection

Expose the UML specialization through a clear facade:

```rust
pub fn project(bundle: &okf::Bundle) -> uml::Projection;
```

The existing `model::Model` may remain the implementation type during
migration, but editor-facing naming and APIs must say `uml` or
`uml_projection`, not the ambiguous `model`.

The UML recognizer claims only supported UML documents:

- recognized UML classifiers;
- recognized UML diagrams;
- recognized UML behavior documents; and
- explicitly authored UML packages.

Anything else remains unclaimed and therefore opens as Generic OKF. This
includes:

- arbitrary OKF `type` values;
- missing or empty `type`;
- unknown families; and
- unknown `uml.*` metaclasses.

The previous "unknown type becomes a generic canvas box" behavior is superseded
at the document boundary: graceful degradation now means a Generic OKF Markdown
document. Unclaimed Concepts are not UML nodes and cannot become diagram
members.

`ElementType::Unknown` may remain temporarily for wire/backward compatibility,
but the normal Bundle-to-UML projection does not produce it.

## Static document composition

The composition root resolves a Concept without a global family enum:

```rust
uml_documents::open(bundle, uml_projection, concept_id)
    .unwrap_or_else(|| okf_documents::open(bundle, concept_id))
```

Each provider prepares an open document:

```rust
pub struct OpenDocument {
    pub tab_id: LiveId,
    pub concept_id: String,
    pub title: String,
    pub presentation: DocumentPresentation,
    pub view: Box<dyn DocView>,
}
```

The exact presentation fields are editor-local data such as icon, accent, and
filter category. They are not a semantic document-family enum.

`DocumentHost` accepts prepared `OpenDocument` values. It owns preview
replacement, persistent tabs, live-view lifecycle, activation, synchronization,
and chrome, but it never matches on UML, OKF, classifier, diagram, or source
kinds.

`DocTab` stores identity and presentation data. The current `TabKind` factory
dispatch is removed. `TreeKind` is either reduced to presentation data or
replaced by open navigator category records; it no longer selects a document
view.

## Native editor behavior

### Generic OKF view

Add `GenericOkfView`, a dedicated `DocView` for unclaimed Concepts.

It:

- reads the Concept and original source by Concept ID;
- renders the Markdown on the shared Markdown surface;
- does not mount or synchronize the diagram canvas;
- hides the tool dock, view bar, canvas overlays, and right inspector;
- has a stable OKF tab identity distinct from UML and Source views; and
- emits no edits in this feature.

`GenericOkfView` and `SourceView` may share a `MarkdownSurface`/body helper, but
they remain different document views with different intent and identity.
Shared body methods and widget IDs should use Markdown-neutral names rather
than calling the reusable surface `source_view`.

### Navigator

The navigator is built from the OKF directory hierarchy, then decorated by the
statically composed providers:

- claimed Concepts receive UML-specific presentation;
- unclaimed Concepts receive Generic OKF presentation;
- directories receive directory/index presentation from OKF; and
- Index and Log remain structural/reserved entries rather than Generic
  Concepts.

Generic OKF rows are clickable and participate in an OKF/document filter
category. UML-only context menus and classifier editing actions are not offered
for them.

### Initial tab

Opening a mixed Bundle preserves the current preference for the requested or
first UML diagram.

When no diagram exists, the editor opens the first navigable Concept. If it is
unclaimed, its `GenericOkfView` becomes the initial preview. An empty Bundle
continues to open with no tab.

### View Source

Generic OKF documents continue to permit the current explicit View Source
action in this feature, even though it is redundant with their Markdown view.
Removing or hiding that action is deferred. Separate view identity ensures that
the later change does not require merging `GenericOkfView` and `SourceView`.

## Editing architecture

### Source-authoritative transaction

Semantic projections are never mutated directly:

```text
User interaction
        |
        v
OKF or UML semantic operation batch
        |
        v
domain Lowerer validates and rewrites Markdown
        |
        v
candidate SourceBundle
        |
        +--> parse okf::Bundle
        `--> derive uml::Projection
                    |
                    v
        atomically replace EditorSession state
```

The term **Lowerer** replaces the overloaded proposed domain "Editor." An
operation is already semantic intent; the Lowerer consumes that intent and
produces candidate source.

The current tolerant parser may return content diagnostics. Diagnostics alone
do not reject an edit. A lowering error, invalid path/selector, collision, or
failure to establish source/projection invariants leaves the session unchanged.

### OKF and UML operation ownership

Split the monolithic operation implementation into `okf::ops` and `uml::ops`.
There is no generic plugin operation registry.

Existing operations map as follows:

| Current operation | New ownership/name |
|---|---|
| `AttrAdd/Set/Rm` | `uml::Op::Attribute*` |
| `ValueAdd/Rm` | `uml::Op::Value*` |
| `RelAdd/Set/Rm` | `uml::Op::Relationship*` |
| `NodeNew/Set/Rm/Rename` | `uml::Op::Classifier*`, composed from OKF source/path primitives where appropriate |
| `PkgMove` | `okf::Op::ConceptMove` |
| `PkgRename` | split into `okf::Op::DirectoryRename` and `DirectoryMove` |
| `PkgDelete` | `okf::Op::DirectoryDelete` |
| `PkgReorder` | `okf::Op::IndexReorder` |
| `PkgSort` | `okf::Op::IndexSort` |
| `PkgRetitle` | `okf::Op::IndexRetitle` |
| `PkgInsert` | `okf::Op::BundleImport` |
| `DiagramSet` | `uml::Op::DiagramSet` |
| `PlaceSet/Rm` | `uml::Op::Placement*` |

The semantic distinctions are intentional:

- **rename** changes a final path segment while retaining its parent;
- **move** changes the parent while retaining the final segment;
- **retitle** changes human-facing content without changing path identity.

`BundleImport` means re-rooting an external Bundle beneath a directory in the
current Bundle. It is not called `BundleInsert`, which would not communicate the
external-Bundle boundary.

There is no `BundleRename` operation. The filesystem directory, repository, or
archive name containing a Bundle belongs to the storage adapter and is not OKF
content. Retitling the root Index is `IndexRetitle { directory: "/" }`.

There is no `BundleDelete` operation. Deleting the currently opened
distribution container is storage lifecycle and is outside a source
transformation. `DirectoryDelete` removes or flattens a nested directory within
the current Bundle.

Shared low-level Markdown, frontmatter, path, link, parse, and serialization
helpers live below both domain Lowerers without importing UML types into OKF.
A UML Lowerer may compose OKF primitives; the OKF Lowerer never depends on UML.

### One generic session apply

Avoid duplicate `apply_okf`/`apply_uml` methods with a sealed strategy/command
trait:

```rust
pub trait EditBatch: sealed::Sealed {
    fn lower(&self, context: EditContext<'_>)
        -> Result<SourceBundle, EditError>;
}

impl EditBatch for okf::Batch { /* delegates to OKF lowering */ }
impl EditBatch for uml::Batch { /* delegates to UML lowering */ }
```

`EditorSession` exposes one generic transaction:

```rust
pub fn apply<B: EditBatch>(
    &mut self,
    batch: B,
) -> Result<SessionChange, EditError>;
```

The trait is sealed because this is internal static composition, not a public
plugin extension point.

Heterogeneous `Box<dyn DocView>` values require type erasure at their common
outcome boundary. `ViewOutcome` therefore carries a `PendingEdit` wrapper around
an erased `EditBatch`. Direct callers retain statically dispatched generic
`EditorSession::apply`.

A visitor is not used: visitors add behaviors over one closed data structure,
whereas this boundary chooses one of several lowering strategies for different
semantic batch types.

### Atomic session update

`EditorSession` owns:

```text
current SourceBundle
persisted SourceBundle snapshot
okf::Bundle
uml::Projection
revision and dirty state
```

Applying a batch:

1. lowers against the current source and projections into a candidate source;
2. parses the candidate OKF Bundle;
3. derives the candidate UML projection;
4. commits source and both projections together;
5. increments revision once and marks it dirty; and
6. returns one `SessionChange`.

Any failure before step 4 leaves every session field unchanged. Operation
batches remain ordered and atomic.

## Memory model

The current editor already retains current and persisted bundles as separate
owned `String` collections. Naively adding `Concept.body: String` would retain
another near-complete copy.

Use shared, copy-on-write source documents:

```rust
pub struct SourceDocument {
    pub path: BundlePath,
    pub text: Arc<String>,
}
```

Current source, persisted baseline, candidate transactions, and semantic
documents share `Arc` pointers. A lowering operation uses copy-on-write and
allocates new text only for touched documents.

Parsed bodies use an owned slice abstraction rather than copying:

```rust
pub struct SourceSlice {
    source: Arc<String>,
    range: Range<usize>,
}

impl SourceSlice {
    pub fn as_str(&self) -> &str {
        &self.source[self.range.clone()]
    }
}
```

A plain `&str` cannot be stored safely inside a long-lived owner alongside the
`String` it borrows without a self-referential structure. `SourceSlice` is the
slice semantics with owned lifetime. Construction validates UTF-8 byte
boundaries; its fields remain private.

Expected source-text memory:

- clean Bundle: one text allocation per document, shared by all snapshots and
  projections;
- dirty Bundle: an old and new allocation only for documents changed since the
  last successful save; and
- OKF/UML projections: semantic metadata and graph structures, not copied
  Markdown bodies.

The whole Bundle remains indexed in memory because navigation, cross-document
resolution, diagrams, rename/cascade operations, and browser persistence are
global. A lazy disk-backed source store is a separate storage architecture and
is not part of this feature.

## Error handling

- Duplicate normalized Concept IDs reject Bundle construction.
- Invalid or traversal-bearing Bundle paths reject source construction.
- Missing Concepts/directories/selectors and destination collisions fail
  lowering without modifying the session.
- `/` cannot be renamed, moved, or deleted as a Directory.
- Missing Index files yield synthesized Index values; an Index mutation
  materializes the file.
- An unclaimed Concept always has the Generic OKF fallback provider.
- Missing source for a live Generic OKF tab renders the existing italic
  missing-source fallback and does not panic.
- Ordinary parser diagnostics remain visible but do not automatically roll back
  a successfully lowered edit.

## Migration sequence

1. Add characterization tests for current source-bundle identity, package/index
   operations, operation atomicity, unknown-document projection, and navigator
   behavior.
2. Introduce shared `SourceDocument`/`SourceBundle` and copy-on-write persisted
   snapshots without changing behavior.
3. Split `Concept`, `Index`, and `Log`; introduce OKF Directory and rooted Index
   lookup; move hierarchy/index reconciliation out of UML `Model`.
4. Add the `uml` projection facade; stop projecting unclaimed Concepts and
   structural directories into UML nodes/packages.
5. Split OKF and UML operation types/Lowerers, add sealed `EditBatch`, and route
   all edits through the one generic session transaction.
6. Replace semantic `TreeKind`/`TabKind` dispatch with provider-produced
   navigator presentation and prepared `OpenDocument` values.
7. Add `GenericOkfView`, Markdown-neutral shared surface naming, mixed-Bundle
   navigation, and OKF-only startup selection.
8. Update serde/WASM/DTO consumers and compatibility re-exports affected by the
   core type and operation split.
9. Run focused, workspace-wide, and native visual verification.

Each stage must compile and pass its focused tests before the next begins.

## Testing

### OKF core

- Concept IDs normalize separators, preserve directory paths, and strip `.md`.
- `/` resolves the root Index and nested directory addresses resolve their
  Index.
- missing indexes synthesize without creating source until edited;
- Concept, Index, and Log never enter one another's collections;
- directory ordering and titles do not require a UML projection;
- arbitrary and missing `type` values remain intact on Concepts;
- source slices share allocation and return exact body text; and
- duplicate/traversal paths are rejected.

### UML projection

- every supported UML document is claimed and projected;
- arbitrary OKF Concepts are not UML nodes;
- unknown `uml.*` metaclasses remain unclaimed;
- structural directories are not `uml.Package`;
- explicit `uml.Package` Concepts remain supported; and
- UML diagrams cannot resolve unclaimed Concepts as members.

### Operations and session

- each old operation maps to the intended OKF or UML batch;
- Directory rename, move, and Index retitle have distinct behavior;
- root Directory rename/move/delete is rejected;
- Bundle import re-roots an external Bundle without changing valid relative
  links;
- a failed Lowerer leaves source, projections, revision, and dirty state
  unchanged;
- a successful batch rebuilds each projection once and increments revision
  once;
- clean current/persisted snapshots share source allocations; and
- copy-on-write duplicates only touched document text.

### Editor

- a mixed Bundle shows both UML and Generic OKF rows;
- opening an unclaimed Concept constructs `GenericOkfView`;
- the Generic OKF view renders Markdown and hides all diagram/inspector chrome;
- UML documents retain their existing views and interactions;
- tab preview replacement and persistent-tab behavior are unchanged;
- the host contains no UML/OKF/source kind dispatch;
- an OKF-only Bundle opens its first Concept; and
- an empty Bundle remains safe.

### Verification

- `cargo fmt --check`
- focused `waml` and `waml-editor` tests after each migration stage
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- affected package tests for WASM/DTO compatibility
- native screenshots of a UML diagram, UML classifier preview, Generic OKF
  document, mixed navigator, OKF-only startup, source view, and tab switching

## Scope and non-goals

Initial user-facing support targets the native editor and shared Rust
projections. CLI, LSP, VS Code, and Svelte web behavior changes only where
required for core API/wire compatibility.

This feature does not include:

- editing within `GenericOkfView`;
- hiding View Source for Generic OKF documents;
- a runtime plugin/provider registry;
- a global document-family enum;
- a generic operation visitor framework;
- bundle-container rename/delete;
- lazy disk-backed document loading;
- undo/redo;
- incremental projection rebuilds; or
- new diagram types for Generic OKF.

## Success criteria

- The authoritative semantic root is an OKF Knowledge Bundle with separate
  Concept, Index, Log, and Directory representations.
- UML is derived from OKF and claims only recognized UML Concepts.
- Every unclaimed Concept opens in a dedicated Markdown-only Generic OKF view.
- Structural directories are never synthesized as UML packages.
- `DocumentHost` and navigator infrastructure do not dispatch on a global
  document-family enum.
- OKF and UML operations have separate ownership and Lowerers.
- `EditorSession` exposes one generic, atomic edit transaction.
- Source text is shared across current, persisted, candidate, and parsed
  representations rather than copied per projection.
- Existing UML editing, diagrams, persistence, tabs, and chrome continue to
  behave as before.
