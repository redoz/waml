# UAML Rust Tooling — Design

**Date:** 2026-07-11
**Status:** Approved (design), pending implementation plan
**Author:** Patrik Husfloen (redoz) with Claude

## Summary

Build native Rust tooling for authoring and checking **UAML** documents. UAML is
the UML-profile authoring format this project already uses — plain Markdown, one
file per classifier (`type: uml.*`) plus optional `Diagram` documents — layered
on **OKF (Open Knowledge Format)**. The reference implementation is the
TypeScript `@mc/okf` package (`packages/okf/src/`); the human-facing spec is
`packages/web/public/okf-format.md`.

The first cut delivers three things, all native (no browser/WASM shipped yet):

1. A **core library crate** (`uaml`) — parse, serialize, and validate UAML.
2. A **`check` CLI** — parse a bundle and report all diagnostics.
3. A **`fmt` CLI** — canonicalize UAML documents.

Not in scope for the first cut: convert/export (Mermaid/PlantUML/images/graph
JSON), zip handling, and any legacy-format import.

## Guiding invariant

> **A UAML document is valid, cleanly-rendering CommonMark.** The
> machine-readable structure is a set of conventions layered on top of markdown —
> never syntax that breaks markdown or renders wrong.

Consequences:

- The docs stay readable on GitHub, in any previewer, and in a plain editor.
- We parse with a **standard markdown parser** (`pulldown-cmark`), not a bespoke
  parser only this project understands. We recognize *our* conventions in the
  parser's output; we never re-implement markdown.
- Enforcing the invariant becomes a first-class **lint** (see Validation), so the
  guarantee is machine-checked, not just a principle.

The one construct that isn't plain CommonMark is **YAML frontmatter** (`---` …
`---`): by strict CommonMark rules a `key: value` line followed by `---` is a
setext heading. Frontmatter is nonetheless *the* established convention for
machine-readable markdown metadata (Jekyll, Hugo, Obsidian, GitHub), and
`pulldown-cmark` supports it as a first-class extension. Enabling that extension
is consistent with the invariant: we use a widely-supported markdown extension
rather than inventing syntax.

## Scope decisions

- **UML-only.** The Rust parser accepts only the current `type: uml.*` classifier
  and `Diagram` format. No OWOX "Data Mart" schema/joins import. No "Google OKF
  v0.1" bullet fallback (deferred — grammar kept extensible, but not
  implemented now).
- **Native-only, WASM-friendly.** The core crate must stay WASM-compatible (no
  filesystem/OS/threads in the parse/serialize/validate layer), but no `wasm32`
  target is built or shipped in this cut. The TS `@mc/okf` continues to power the
  web app.
- **No OWOX branding** in any output. The TS serializer's OWOX footer is not
  carried over.
- **Location:** an in-repo Cargo workspace under `/crates`, alongside the pnpm
  workspace in `/packages`.

## Related (separate) work

Removing the OWOX "Data Mart" legacy format from the TS `@mc/okf` code is part of
the same de-fork effort but is **not** part of this Rust tooling. Tracked
separately (the Google OKF v0.1 fallback stays in TS for now).

## Architecture

### Workspace layout

```
/crates
  uaml/            # core library crate (no I/O): model + parse + serialize + validate
  uaml-cli/        # binary `uaml`: check, fmt (filesystem/stdin/rendering here only)
Cargo.toml         # [workspace], members = ["crates/*"]
```

`/target` is added to `.gitignore`. The Cargo workspace coexists with the pnpm
workspace; neither manages the other.

### Parsing pipeline

`pulldown-cmark` is a pull parser (event stream), not a tree. The pipeline:

```
markdown source
  → pulldown-cmark event stream    (Start(Item), Text, Start(Link), End, …)
  → Document AST                    (our typed, per-file, near-lossless structure)
  → Model (graph)                   (bundle-wide, cross-refs resolved)
```

We own the AST. The markdown parser provides CommonMark correctness (lists,
inline links, the frontmatter block); we fold its events into our structures and
recognize our conventions (e.g. "a list item whose text matches the relationship
pattern").

### Two-tier AST

The formatter and the validator want different things, so there are two tiers.

**1. `Document` (`uaml::syntax`)** — faithful, near-lossless view of *one* `.md`
file: frontmatter fields, the recognized sections (`## Attributes`,
`## Relationships`, `## Values`, `## Members`, `## Body`, `## Render hints`,
`## Notes`), the typed lines within them, **and any unrecognized sections
preserved verbatim**. This is what `fmt` round-trips through, and it is what makes
the format's *graceful degradation* rule hold (unknown `##` sections carried
through, never dropped).

**2. `Model` (`uaml::model`)** — the resolved semantic layer built by walking all
`Document`s in a bundle: nodes keyed by slug, edges with resolved `ref` targets,
diagrams. Analogue of the TS `ModelGraph`. This is what `validate` (and any future
export) consumes.

Rationale for the split: `fmt` must not lose a user's unknown sections or reorder
content destructively (needs fidelity); `check` needs cross-document resolution
(needs the resolved graph). One AST doing both compromises each.

```
uaml::syntax    → Document, Section, AttributeLine, RelationshipLine, …   (per-file)
uaml::model     → Model, Node, Edge, Diagram, TypeRef, RelEnd            (bundle-wide, resolved)
uaml::parse     → events → Document ; Documents → Model
uaml::validate  → &Model (+ &[Document]) → Vec<Diagnostic>
uaml::serialize → Document → markdown string
```

## Data model (`uaml::model`)

Mirrors the TS `ModelGraph` (`packages/okf/src/types.ts`) closely so parity is
checkable, but idiomatic Rust — enums where TS used string unions.

```rust
struct Model { nodes: Vec<Node>, edges: Vec<Edge>, diagrams: Vec<Diagram> }
// nodes resolvable by `key` (slug); expose a by-key lookup.

struct Node {
    key: String,              // slug
    ty: ClassifierType,       // parsed "family.Metaclass"
    title: String,
    stereotypes: Vec<String>,
    abstract_: bool,
    attributes: Vec<Attribute>,
    values: Vec<String>,      // enum literals (uml.Enum)
    // note body / notes-sugar as applicable
}

enum ClassifierType {         // graceful degradation as a type-level guarantee
    Uml(UmlMetaclass),        // Class, Interface, Enum, DataType, Package, Note, Association
    Diagram,
    Unknown(String),          // any other family.Metaclass or opaque token → generic box
}

struct Attribute {
    name: String,
    ty: TypeRef,
    multiplicity: Multiplicity,
    visibility: Option<Visibility>,
    description: Option<String>,
}
struct TypeRef { name: String, ref_: Option<String> }   // ref_ = resolved slug
enum Visibility { Public, Private, Protected, Package }  // + - # ~

enum RelationshipKind { Associates, Aggregates, Composes, Specializes, Implements, Depends, Annotates }
// Associates/Aggregates/Composes are "ended"; the rest forbid ends.
// Encoded as a method on the enum (single source of truth), not duplicated data.

struct Edge {
    source: String,           // declaring doc slug
    target: String,           // resolved target slug
    kind: RelationshipKind,
    name: Option<AssocName>,  // plain label, or link to a uml.Association (association class)
    from_end: RelEnd,
    to_end: RelEnd,
}
struct RelEnd { multiplicity: Option<Multiplicity>, role: Option<String>, navigable: Option<bool> }

struct Diagram { key: String, title: String, profile: String, members: Vec<Member>, render_hints: RenderHints }
```

Two deliberate choices:

- **`Multiplicity` is a validated type**, parsed against the BNF
  (`bound | lower..bound`, lower `0|posint`, bound `posint|*`, and `lower <= upper`).
  An out-of-range range like `5..2` is a parse-time error, not a lurking bug.
- **`ClassifierType::Unknown`** makes graceful degradation a type-level property
  rather than scattered `if` checks.

## Validation & diagnostics (`uaml::validate`)

Non-fail-fast: collect **all** diagnostics and return a `Vec<Diagnostic>`.

```rust
struct Diagnostic { severity: Severity, code: DiagCode, message: String, loc: Loc }
enum Severity { Error, Warning }
struct Loc { file: String, line: usize /* + span where cheap */ }
```

Rule groups:

- **Structural (Error)** — malformed attribute/relationship line; invalid
  multiplicity; `associates`/`aggregates`/`composes` missing the `: near to far`
  ends; `specializes`/`implements`/`depends` carrying forbidden ends; `annotates`
  on a non-`uml.Note`.
- **Cross-reference (Error)** — a `[X](./x.md)` link resolving to no document;
  slug/filename mismatch; duplicate slugs.
- **Markdown-invariant (Error/Warning)** — any construct that would not render as
  clean CommonMark or that parses ambiguously. Enforces the guiding invariant.
- **Consistency (Warning)** — reciprocal relationships with mismatched
  multiplicity (spec says this is *not* an error); unknown `type` / unknown
  `profile` (both degrade gracefully, so warn rather than error).

The library only returns the vector. Rendering (colored, `file:line: error[CODE]:
message`, and a JSON form) lives in `uaml-cli`, keeping the core WASM- and
embed-friendly.

## CLI surface (`uaml-cli`, binary `uaml`)

```
uaml check <path>...     # parse + validate; print diagnostics; exit 1 if any Error
uaml fmt <path>...       # canonicalize (in place by default)
```

**Input forms** (resolved by the CLI before strings reach the core lib):

- a **directory** of `.md` files → treated as one bundle;
- a single **`.md` file** — either one document, or a concatenated blob with
  `<!-- path/slug.md -->` markers (split on markers, same rule as
  `packages/web/src/okf/io.ts`);
- **`--stdin`** for piping;
- *(zip: deferred, matches the no-convert/export scope.)*

**`check`** — `--format human|json` (JSON for editors/CI); exit `1` on any
`Error`, `0` if only warnings.

**`fmt`** — default rewrites files to canonical form; `--check` exits non-zero if
any file isn't already formatted (CI gate); `--stdout` prints without writing.
Canonical form = `serialize(Document)`: normalized frontmatter key order,
slug-derived filenames, stable relationship/attribute ordering, **unknown
sections preserved verbatim**. No OWOX footer.

**Boundary:** the core lib never reads files or prints; the CLI owns all I/O and
rendering. This is what keeps the WASM door open for free.

## Testing & parity strategy

- **Round-trip** — `parse → serialize → parse` is stable; an already-canonical
  input is a `fmt` fixpoint.
- **Golden fixtures** — the `okf-format.md` worked example (orders domain),
  committed as a fixture and asserted to parse into the expected `Model`. This is
  the same artifact the TS `packages/okf/src/guideExample.test.ts` guards, so both
  implementations are pinned to the *same* spec artifact — a cross-language drift
  guard that does not require running the TS code.
- **Grammar unit tests** — port the meaningful cases from `packages/okf/test/`:
  multiplicity BNF edges, ends required/forbidden XOR, visibility markers,
  association names/classes, note anchors.
- **Markdown-invariant tests** — feed each fixture through `pulldown-cmark` and
  assert no ambiguous / heading-mangled output (the lint's own test bed).
- **Diagnostics snapshots** — malformed inputs produce the expected `(code, line)`
  set.
- **TDD** — follow test-driven-development throughout implementation.

## Open questions / deferred

- Markdown crate: `pulldown-cmark` chosen (fastest, strict CommonMark,
  WASM-proven, minimal deps). `comrak` is the fallback if a needed extension is
  missing.
- Syntax audit: before/while implementing, verify the *current* `okf-format.md`
  syntax has no genuine CommonMark collisions beyond frontmatter-needs-extension.
  Any found are fixed at the format level (per the invariant), not worked around
  in the parser.
- Deferred features: convert/export, zip I/O, Google OKF v0.1 fallback, and any
  WASM/`wasm-bindgen` bindings.
