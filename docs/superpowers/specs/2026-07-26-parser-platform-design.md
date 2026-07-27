# WAML Roslyn-lite Parser Platform

**Status:** Approved in conversation; written-spec review complete
**Date:** 2026-07-26  
**Revised:** 2026-07-27
**Builds on:** `2026-07-27-first-class-okf-documents-design.md` and
`../plans/2026-07-27-first-class-okf-documents.md`
**Scope:** Lossless WAML parsing, authoring syntax, diagnostics, projections,
and their shared use by the native editor, CLI, Rust LSP, VS Code extension,
and automation consumers.

> This is an architecture design, not an implementation authorization. The
> first-class OKF implementation plan completes first. The parser platform then
> replaces the old parser in one coordinated cutover without replacing
> `SourceBundle`, `okf::Bundle`, `uml::Projection`, the domain Lowerers, or the
> atomic `EditorSession` transaction established by that work.

## Goal and scope

WAML must remain structurally useful while a document is incomplete or
malformed. An authoring surface must be able to identify an attribute, its name,
type, delimiters, and invalid multiplicity separately, then offer a targeted
repair without losing bytes or pretending the malformed field is absent.

The platform is deliberately **Roslyn-lite**: it retains the syntax and snapshot
mechanisms that make Roslyn useful while omitting compiler-scale peripherals and
remaining subordinate to the already-authoritative OKF source/session model.

It provides:

- immutable full-fidelity syntax trees for every byte of every retained Markdown
  source;
- true parentless/positionless green nodes and typed, parented/positioned red
  facades;
- tokens, deterministic trivia ownership, missing tokens, bad tokens, and
  structured skipped-token recovery;
- exact untouched round-trip, immutable syntax snapshots, structural sharing
  through syntax rewrites and later incremental reparse;
- typed traversal, visitors, rewriters, factories, annotations, and tracking;
- snapshot-scoped diagnostics, edits, and code actions;
- specialization-owned tolerant declared projections alongside validated
  projections, with UML as the first implementation;
- lossless generic Markdown syntax for unclaimed OKF Concepts;
- syntax snapshots derived from one authoritative `SourceBundle`; and
- a stable, domain-neutral analysis context through which statically composed
  specializations consume one immutable OKF Bundle revision.

It does **not** provide:

- a second workspace, source store, document lifecycle, or revision authority;
- a mutation path that bypasses `EditorSession::apply`;
- a replacement for `okf::Bundle` or `uml::Projection`;
- a global product document-family enum, runtime provider registry, generic
  operation visitor, or plugin system;
- compilation units, assemblies, emit, metadata, analyzer drivers, diagnostic
  suppression/localization, warning policies, or compiler-style symbol graphs;
- ropes, operational transform, collaboration, filesystem watching, filesystem
  or URI canonicalization, host lifecycle, or asynchronous scheduling in parser
  core;
- rich editable CommonMark syntax initially. Ordinary Markdown gets exact
  preservation, source ranges, and navigation, while claimed WAML/UML islands
  get typed editing;
- stable semantic IDs through arbitrary full reparses. Revision-scoped syntax
  handles are the default; or
- JavaScript, WASM, Svelte, browser-product, or generated TypeScript
  compatibility.

## Prerequisite architecture

Implementation starts only after the first-class OKF plan has satisfied its
completion criteria:

```text
EditorSession
├── current SourceBundle
├── persisted SourceBundle snapshot
├── okf::Bundle
├── uml::Projection
├── revision and dirty state
└── prepared document/view state
```

The following decisions are inherited and are not reopened by this design:

1. `SourceBundle` is the validated, source-authoritative, copy-on-write store.
2. `SourceDocument` owns one `Arc<String>` and one normalized `BundlePath`.
3. `okf::Bundle` is the semantic root with separate Concept, Index, Log, and
   Directory collections.
4. UML claims only recognized Concepts and is derived as `uml::Projection`.
5. Arbitrary types, missing types, and unknown `uml.*` metaclasses remain
   unclaimed and open as Generic OKF documents.
6. OKF and UML operations have separate batches and Lowerers.
7. `EditorSession::apply` is the only product mutation choke point.
8. One successful transaction parses the final OKF candidate once, projects UML
   once, increments the session revision once, and atomically commits all state.
9. A failed transaction changes no source, syntax, semantic projection,
   revision, dirty state, or shared allocation identity.
10. `DocumentHost` receives provider-prepared `OpenDocument` values and owns no
    semantic family dispatch.
11. The legacy web/WASM stack is absent. The VS Code extension remains an
    independent TypeScript client of the Rust LSP.
12. Future built-in specializations compose explicitly above OKF. Adding one
    does not change OKF core types, the syntax kernel, `DocumentHost`, or a
    global family enum.

The parser platform extends this state with **derived syntax state**. It does not
replace or wrap the session with another owning workspace.

## Design invariants

These are acceptance properties, not aspirations.

1. Writing an untouched tree produces source byte-for-byte, including invalid
   input: `tree.write_to_string() == source`.
2. Every source byte occurs exactly once in ordered leaf content: token text or
   trivia owned by a concrete token. Nodes add no text.
3. A required absent lexical item is its expected token kind with
   `is_missing = true`, empty spelling, and zero width. A missing colon is a
   missing `ColonToken`, never a generic missing-token kind.
4. Unclassifiable source is a non-empty `BadToken`; lexically valid but
   misplaced source is a navigable `SkippedTokensSyntax` node. Neither is
   silently discarded as trivia.
5. Green elements contain no parent, absolute position, product document
   identity, revision, or diagnostics.
6. Red equality and hashing use tree instance plus the complete child-index path
   from the root. Position, kind, and shared green identity are insufficient for
   distinct zero-width occurrences. Green sharing is a separate query and never
   promises red identity across revisions.
7. Ranges, locators, diagnostics, edits, and actions are valid only for their
   declared syntax/source revision.
8. Syntax state is derived from a specific `SourceBundle` and
   `EditorSession` revision. It has no independent public revision clock.
9. A parser edit cannot insert, remove, rename, or replace a source document
   directly. It must become an `EditBatch` applied by `EditorSession`.
10. Parser core performs no IO, host-path canonicalization, URI handling,
    persistence, dirty tracking, or host lifecycle work.
11. Every Markdown source is represented. Specialization syntax islands are
    typed only after that specialization claims the corresponding OKF Concept.
12. Index and Log sources never become Concepts or enter specialization
    claiming.
13. Declared projections distinguish absent, valid, incomplete, and invalid;
    validated projections may exclude invalid material.
14. A syntax snapshot always satisfies
    `snapshot.text() == snapshot.syntax().write_to_string()`.
15. Current semantic and syntax views share the current source allocation.
    When clean, persisted state shares it too; after an unsaved edit, persisted
    state may retain the previous allocation for touched documents. Parsing
    never copies a whole document.
16. Incremental reuse may not retain an unbounded chain of old whole-document
    allocations. Retention remains compatible with the source plan's clean and
    dirty allocation bounds.
17. Prepared syntax, candidate source, OKF Bundle, and every statically composed
    specialization analysis install atomically inside the session transaction
    or not at all.
18. The OKF shell and shared syntax kernel do not privilege UML. Each built-in
    specialization owns recognition, typed syntax, declared semantics,
    validation, lowering, and document presentation.

## Post-OKF support baseline

The prerequisite implementation leaves these responsibilities in place:

| Area | Authoritative owner after the prerequisite |
|---|---|
| Exact Markdown and normalized paths | `source::{SourceBundle, SourceDocument, BundlePath}` |
| Generic OKF meaning | `okf::{Bundle, Concept, Index, Log, Directory}` |
| Supported UML meaning | `uml::Projection` |
| OKF source transformations | `okf::ops::Lowerer` |
| UML source transformations | `uml::ops::Lowerer` |
| Ordered legacy Rust DTO compatibility | sealed `compat::Batch` |
| Transaction, revision, dirty state | `EditorSession::apply` |
| Document/view composition | static UML-first, Generic-OKF-fallback providers |
| Persistence | native `SourceBundle` adapters |

The old parser remains semantic and partially recovering rather than a CST:

- `Document { frontmatter, title, sections }` and section variants for
  attributes, slots, values, relationships, body, notes, nodes, lifelines,
  messages, members, layout, and unknown sections;
- `Line<T> = Parsed(T) | Error(ErrorNode)`, where `ErrorNode` preserves one raw
  line plus line and line-relative span;
- handwritten/regex section parsers and `pulldown-cmark` heading discovery;
- canonical reconstruction through `serialize.rs`, not a lossless writer; and
- malformed `Line::Error` values commonly filtered from validated UML
  projection.

The parser replacement must preserve the product semantics retained by the OKF
plan without restoring removed `ElementType::Unknown` nodes, structural UML
packages, `ConceptRole`, web/WASM adapters, or kind-based document dispatch.

### Known planned but unimplemented syntax

The platform preserves these as raw/unknown syntax but does not claim they are
implemented:

- classifier methods/operations: a future `## Operations` section, signatures,
  parameters, return types, and visibility;
- deferred sequence forms: `par`, self/found/lost messages, gates, and
  coregions;
- diagram render hints currently preserved as unknown;
- default/derived/read-only attribute adornments; and
- ERD/data profile constructs, BPMN, C4, and other non-UML families.

Classifier operations remain a later language milestone and require their own
syntax/model design.

## Architecture and dependency direction

```text
waml-syntax
  shared SourceText handle, text/ranges, green/red kernel, Markdown/OKF shell,
  annotations and shell syntax diagnostics

waml -> waml-syntax
  SourceBundle and BundlePath, OKF analysis, DomainAnalysisContext,
  static specialization composition, domain Lowerers, formatting,
  syntax-edit adapters

waml::uml -> DomainAnalysisContext + waml-syntax
  recognizer, uml::syntax language/parser/wrappers, declared UML,
  validated UML Projection

future built-in specialization -> DomainAnalysisContext
  its own recognizer, optional typed syntax, declared/validated projection,
  Lowerer and editor document provider

waml-editor -> waml
  EditorSession ownership and atomic commit, document providers and views

waml-cli -> waml
  CLI and Rust LSP

packages/vscode -> waml-cli LSP over stdio
```

`waml-syntax` imports no source-bundle, OKF, UML semantic, current diagnostic,
path, session, persistence, or host type. It owns:

```rust
pub struct TextSize(u32);
pub struct TextRange { pub start: TextSize, pub end: TextSize }

pub struct SourceText {
    source: Arc<String>,
}

pub struct LineIndex { /* byte line starts */ }
pub struct TreeInstanceId(/* opaque */);
pub enum MarkdownDialect { CommonMarkCurrent /* future explicit options */ }
pub struct SyntaxAnnotation { /* syntax-local annotation id/kind/data */ }
pub enum SyntaxSeverity { Error, Warning, Info }

pub trait SyntaxLanguage: Send + Sync + 'static {
    type Kind: Copy + Eq + Hash + Debug + Send + Sync;
    type DiagnosticCode: Copy + Eq + Hash + Debug + Send + Sync;
}

pub struct OkfMarkdownLanguage;
pub enum OkfMarkdownSyntaxKind { /* shell/raw Markdown kinds */ }
pub enum OkfSyntaxDiagnosticCode { /* shell/recovery codes */ }

impl SyntaxLanguage for OkfMarkdownLanguage {
    type Kind = OkfMarkdownSyntaxKind;
    type DiagnosticCode = OkfSyntaxDiagnosticCode;
}
```

`SourceText::from_shared(Arc<String>)` clones only the `Arc`, never the string.
The `waml` layer constructs it from `SourceDocument` and keeps `BundlePath`,
Concept identity, semantic classification, and session revision outside
`waml-syntax`.

The green/red kernel is language-parameterized. `waml-syntax` owns the kernel and
OKF shell only. `waml::uml::syntax` defines `UmlLanguage`, `UmlSyntaxKind`, UML
diagnostic codes, its parser, and typed wrappers over the shared kernel. A future
built-in specialization may define another `SyntaxLanguage`, consume the
lossless shell without a second typed tree, or use its own parser. Adding one
does not add kinds to a global enum or make OKF depend on its parser.

```rust
// waml::uml::syntax
pub struct UmlLanguage;
pub enum UmlSyntaxKind { /* UML islands plus embedded Markdown kinds */ }
pub enum UmlSyntaxDiagnosticCode { /* UML grammar/recovery codes */ }

impl SyntaxLanguage for UmlLanguage {
    type Kind = UmlSyntaxKind;
    type DiagnosticCode = UmlSyntaxDiagnosticCode;
}
```

`waml` owns `DocumentId`, `DocumentRevision`, session/bundle revision,
`BundlePath`, `DiagCode`/`Severity`/`Diagnostic`, declared and validated semantic
types, and the adapter from syntax changes into sealed `EditBatch` values.
Syntax-layer types may be re-exported for convenience.

## Text, green tree, and red facade

All core text positions are half-open UTF-8 byte ranges. `LineIndex` converts
them to line/column and UTF-16 only at LSP transport boundaries. Inputs too
large for `u32` widths fail explicitly rather than overflowing.

```rust
pub type GreenNode<L> = Arc<GreenNodeData<L>>;
pub type GreenToken<L> = Arc<GreenTokenData<L>>;

pub struct GreenNodeData<L: SyntaxLanguage> {
    kind: L::Kind,
    children: Arc<[GreenElement<L>]>,
    full_width: TextSize,
    annotations: Arc<[SyntaxAnnotation]>,
}

pub enum GreenElement<L: SyntaxLanguage> {
    Node(GreenNode<L>),
    Token(GreenToken<L>),
}

pub struct GreenTokenData<L: SyntaxLanguage> {
    kind: L::Kind,
    flags: TokenFlags,
    leading: Arc<[GreenTrivia]>,
    text: GreenText,
    trailing: Arc<[GreenTrivia]>,
    full_width: TextSize,
    annotations: Arc<[SyntaxAnnotation]>,
}

pub struct TokenFlags(/* MISSING bit */);
```

`GreenToken::<UmlLanguage>::missing(UmlSyntaxKind::ColonToken)` has colon kind,
`MISSING`, empty text/trivia, and zero width. `BadToken` has real non-empty
source spelling. `SkippedTokensSyntax` contains ordinary original tokens,
including their own trivia, and is a recovery node rather than trivia.

`GreenText` uses static spellings for punctuation/keywords and either compact
shared spelling or a validated range into `SourceText` for source-derived text.
The representation must satisfy both:

- parsing a document creates no second whole-document allocation; and
- retaining reused green nodes does not pin an unbounded sequence of historical
  `Arc<String>` values.

This is an objective memory gate, not only a benchmark question. Copying selected
small token spellings, rebasing ranges onto the current source, or another
measured strategy is acceptable if it preserves exact writing and bounded
retention.

Reds add tree occurrence context only:

```rust
pub struct SyntaxTree<L: SyntaxLanguage> {
    root_green: GreenNode<L>,
    context: Arc<RedContext>,
    diagnostics: Arc<[TreeDiagnostic<L::DiagnosticCode>]>,
    dialect: MarkdownDialect,
}

struct RedContext { tree_id: TreeInstanceId }

pub struct SyntaxNode<L: SyntaxLanguage>(Arc<RedNodeData<L>>);
struct RedNodeData<L: SyntaxLanguage> {
    green: GreenNode<L>,
    context: Arc<RedContext>,
    parent: Option<SyntaxNode<L>>,
    child_index: Option<u32>,
    position: TextSize,
}
```

`SyntaxTree::root()` creates a fresh root facade. `RedContext` has no backpointer
to the tree and the tree stores no red-root cache, so no cycle exists. Node and
token equality and hashing require the same tree ID and complete child-index
path from the root. The path distinguishes adjacent zero-width missing tokens
and empty recovery nodes even when they share a green and absolute position.

`same_green` is explicitly `Arc::ptr_eq` over identity-bearing green storage and
says only that immutable structure is shared. A serializable `SyntaxLocator`
contains tree ID, child-index path, and expected kind. `waml` wraps it with
document and revision information before exposing it to consumers.

Red navigation creates one parent-chain facade and no persistent child cache.
Parent is O(1), ancestor walk is O(depth), `children_with_tokens` is one
O(children) pass, and sibling lookup is O(siblings). Rewriters operate on
greens. Add traversal-local weak caches only if measurement proves navigation
dominates.

Typed wrappers are checked red casts:

```rust
pub trait AstNode<L: SyntaxLanguage> {
    fn can_cast(kind: L::Kind) -> bool;
    fn cast(node: SyntaxNode<L>) -> Option<Self> where Self: Sized;
    fn syntax(&self) -> &SyntaxNode<L>;
}

pub struct AttributeSyntax(SyntaxNode<UmlLanguage>);
impl AttributeSyntax {
    pub fn colon_token(&self) -> SyntaxToken<UmlLanguage>; // possibly missing
    pub fn multiplicity(&self) -> Option<MultiplicitySyntax>;
}
```

Every typed node has a normative logical shape independent of recovery:

- fixed slots contain required or optional singular children;
- required absent tokens occupy their slot as zero-width expected-kind tokens;
- list slots contain ordered repetitions and separators;
- recovery slots contain `BadToken` or `SkippedTokensSyntax` without changing
  the meaning or index of typed slots; and
- typed accessors read declared slots, never search descendants by kind.

Parser, factory, rewriter, and typed facade share one slot definition.

## Markdown shell, domain claiming, and recovery

Parsing is explicitly staged:

```text
SourceDocument
  -> lossless Markdown/frontmatter shell
  -> OKF Concept / Index / Log / Directory derivation
  -> DomainAnalysisContext
       ├── UML recognizer -> UML typed islands and projection
       ├── future static recognizer -> its syntax/projection
       └── no recognizer -> lossless Generic OKF Markdown
```

`waml-syntax` exposes syntax functions, not product document-family values.
The `waml` composition layer constructs one domain-neutral context. Each
statically linked specialization decides whether it claims a Concept and, if
claimed, whether it needs typed islands beyond the shared shell. The syntax
crate does not import `okf::Concept`, `ElementType`, `uml::Projection`, or any
future specialization's semantic types.

This feature wires only UML plus the Generic OKF fallback, but the seam is not
UML-shaped. Adding a built-in specialization adds its analyzer and one explicit
call at the composition root; it does not change `okf::Bundle`,
`DomainAnalysisContext`, the shell parser, or `DocumentHost`. There is no
runtime registration, discovery, or plugin lifecycle.

Specialization recognizers must be disjoint for product document ownership.
Static composition tests exercise every built-in recognizer pair. If an
unexpected source is claimed by more than one specialization, analysis returns
an ambiguity error rather than silently selecting a different semantic owner.
The Generic OKF fallback applies only when no specialization claims the Concept.

The shell uses a `MarkdownStructureMap` derived from configured
`pulldown-cmark` event offsets and explicit container tracking:

```rust
pub struct MarkdownStructureMap {
    headings: Arc<[ConfirmedHeading]>,
    protected_ranges: Arc<[TextRange]>,
    dialect: MarkdownDialect,
}
```

Only CommonMark-confirmed headings outside block quotes, lists/items, code
blocks, HTML blocks, and other non-document containers are promoted. The shell
uses H1/H2; claimed WAML islands consume the same H3-H6 map for member groups,
flow nodes/notes, and related structure. Island parsers use explicit
indent/bullet grammar and protected ranges, never raw `#` heuristics. All other
source is exact `MarkdownRegion` content.

Clean fenced frontmatter is structured regardless of whether `type` is present
or recognized. Frontmatter retains arbitrary and missing type values for OKF.

Unclosed-frontmatter recovery remains conservative. Only an initial `---`
after an optional BOM can start a candidate. The parser scans contiguous
plausible flat entries and blank lines. A syntactically plausible `type:` entry
may confirm the candidate, but its value need not be a recognized UML type.
The parser synchronizes before the first confirmed top-level H1, inserts a
zero-width missing closing fence, emits `frontmatter-not-clean`, and continues.
An H2 fallback is allowed only when every preceding candidate line is plausible
frontmatter. Without enough OKF-shell evidence, an initial thematic rule remains
raw Markdown. Later `---` is never frontmatter.

Concepts unclaimed by UML do not acquire typed Attributes, Members, Layout,
Flow, or Sequence sections merely because their Markdown uses matching
headings. Another statically composed specialization may claim them through its
own recognizer; otherwise their source stays Generic OKF and lossless. Unknown
`uml.*` remains unclaimed by UML.

## Trivia, recovery, and exact writing

The writer visits tokens in source order and writes `leading`, `text`, then
`trailing`. Parser-produced typed-island whitespace follows one deterministic
leading-trivia convention:

- `NewlineToken` is explicit; indentation is explicit where it controls member,
  flow, or sequence structure;
- insignificant spaces/tabs between lexical tokens are leading trivia of the
  following token;
- spaces/tabs immediately before a newline are leading trivia of that newline;
- EOF whitespace is leading trivia of zero-width `EndOfFileToken`;
- parser-produced trailing trivia is empty in the first implementation; and
- raw Markdown regions own whitespace, comments, and HTML directly as raw token
  text. HTML comments are never generic WAML trivia.

For `foo   \r\n`, the sequence is `Identifier("foo")`, then
`Newline(leading = "   ", text = "\r\n")`; for `foo   ` it is
`Identifier("foo")`, then `EOF(leading = "   ")`. For `foo bar` when a colon
is expected, it is `Identifier("foo")`, missing `ColonToken`, then
`Identifier(leading = " ", text = "bar")`. Each writes exactly the input.

## Derived syntax snapshots

Diagnostics are a `SyntaxTree` side table, never green fields:

```rust
pub struct TreeDiagnostic<C> {
    pub code: C,
    pub severity: SyntaxSeverity,
    pub message: Arc<str>,
    pub range: TextRange,
}
```

They explain recovery but do not represent source. Full reparse creates a new
table. Incremental reparse later discards diagnostics in the reparse window,
regenerates those, and translates unaffected ranges through a text-change map.
Reused greens therefore cannot carry stale absolute diagnostics.

The `waml` layer owns language-parameterized derived syntax sets:

```rust
pub struct DocumentVersion {
    id: DocumentId,
    revision: DocumentRevision,
    path: BundlePath,
    text: SourceText,
    line_index: Arc<LineIndex>,
}

pub struct DocumentCatalog {
    session_revision: u64,
    documents: Arc<BTreeMap<DocumentId, Arc<DocumentVersion>>>,
    paths: Arc<BTreeMap<BundlePath, DocumentId>>,
    next_document_id: u64,
}

pub struct SyntaxSnapshot<L: SyntaxLanguage> {
    document: Arc<DocumentVersion>,
    syntax: Arc<SyntaxTree<L>>,
}

pub struct SyntaxSet<L: SyntaxLanguage> {
    catalog: Arc<DocumentCatalog>,
    documents: Arc<BTreeMap<DocumentId, Arc<SyntaxSnapshot<L>>>>,
}
```

`DocumentCatalog` is the sole source identity/revision catalog.
`SyntaxSet` is not a public mutable workspace. It exposes syntax lookup only:

```rust
impl<L: SyntaxLanguage> SyntaxSet<L> {
    pub fn document(&self, id: DocumentId) -> Option<&Arc<SyntaxSnapshot<L>>>;
    pub fn catalog(&self) -> &Arc<DocumentCatalog>;
}
```

It deliberately has no public `insert`, `remove`, `rename`, `replace_text`,
`replace_syntax`, save, dirty, or persistence API.

OKF derivation is an internal composition in `waml`:

```rust
pub struct OkfAnalysis {
    pub catalog: Arc<DocumentCatalog>,
    pub shell: SyntaxSet<OkfMarkdownLanguage>,
    pub bundle: okf::Bundle,
}

pub fn analyze_okf(
    source: &SourceBundle,
    previous: Option<&OkfAnalysis>,
    session_revision: u64,
) -> Result<OkfAnalysis, AnalysisError>;

pub struct DomainAnalysisContext<'a> {
    pub source: &'a SourceBundle,
    pub catalog: &'a Arc<DocumentCatalog>,
    pub shell: &'a SyntaxSet<OkfMarkdownLanguage>,
    pub okf: &'a okf::Bundle,
    pub session_revision: u64,
}

pub struct ClaimSet {
    concept_ids: BTreeSet<String>,
}
```

`analyze_okf`:

1. receives an already-validated candidate `SourceBundle`;
2. derives lossless shell syntax for every Markdown source;
3. builds `okf::Bundle` from source plus shell/frontmatter syntax;
4. shares unchanged snapshots and `Arc<String>` allocations;
5. assigns document revisions only as derived metadata;
6. verifies the source/tree equality invariant; and
7. returns both OKF candidates without mutating previous state.

Document identity/revision rules are explicit:

- an unchanged normalized path reuses `DocumentId`;
- unchanged `Arc<String>` identity also reuses `DocumentRevision`;
- changed text at the same path preserves `DocumentId` and increments
  `DocumentRevision` once;
- a new path receives a fresh, never-reused ID and starts at revision one;
- removal drops the snapshot but does not rewind `next_document_id`; and
- a rename is remove-plus-add at the syntax layer and invalidates old locators.
  Stable identity through rename is not promised until the source model itself
  gains a session-local identity.

The candidate `DocumentCatalog` clones `next_document_id`; failed analysis
therefore does not consume IDs or advance any revision.

Each specialization consumes the same domain-neutral context through an
explicit function. UML, the first specialization, owns its complete analysis:

```rust
pub mod uml {
    pub struct Analysis {
        pub claims: ClaimSet,
        pub syntax: SyntaxSet<UmlLanguage>,
        pub declared: DeclaredBundle,
        pub projection: Projection,
    }

    pub fn analyze(
        context: DomainAnalysisContext<'_>,
        previous: Option<&Analysis>,
    ) -> Result<Analysis, AnalysisError>;
}
```

`uml::analyze` uses `okf::Bundle` as its only claim authority, refines only its
claimed Concepts, reuses the shell's `SourceText` and
`MarkdownStructureMap`, constructs declared UML once, and constructs validated
UML once. A future specialization owns an equivalent sibling analysis type; no
central `BundleAnalysis` accumulates every domain's declared or syntax types.
Every specialization syntax set uses the shell's `DocumentId` and
`DocumentRevision` for the same source path; only `analyze_okf` allocates source
document identities. Its `SyntaxSet` stores the exact
`Arc<DocumentCatalog>` supplied by `DomainAnalysisContext`, and every syntax
snapshot stores the catalog's exact `Arc<DocumentVersion>`. Language-specific
tree IDs remain independent.

`okf::Bundle::parse(&SourceBundle)` remains the public convenience boundary and
delegates to the same shell parser. The session calls `analyze_okf` once and
passes its context to every statically composed specialization.

The session revision is the authoritative clock. Document revisions identify
changed syntax snapshots within that clock; they cannot advance independently.

## Declared and validated projections

Syntax is authoritative for authored shape. Partial state belongs to fields, not
to an opaque whole-declaration success/failure wrapper:

```rust
pub enum DeclaredField<L: SyntaxLanguage, T> {
    Absent,
    Valid { value: T, syntax: SyntaxNode<L> },
    Incomplete { syntax: SyntaxNode<L>, expected: ExpectedSyntax },
    Invalid { syntax: SyntaxNode<L>, diagnostics: Arc<[DiagCode]> },
}

pub struct DeclaredAttribute {
    pub syntax: AttributeSyntax,
    pub visibility: DeclaredField<UmlLanguage, Visibility>,
    pub name: DeclaredField<UmlLanguage, String>,
    pub ty: DeclaredField<UmlLanguage, TypeRef>,
    pub multiplicity: DeclaredField<UmlLanguage, Multiplicity>,
}
```

An `AttributeSyntax` becomes a `DeclaredAttribute` once the claimed UML parser
recognizes the production, even if it cannot yet be lowered. It lowers into the
validated UML projection only when every required field is valid and every
optional field is absent or valid.

The projection pipeline is:

```text
SourceBundle
  -> lossless shell syntax for every source
  -> okf::Bundle (Concept, Index, Log, Directory)
  -> DomainAnalysisContext
       ├── uml::analyze
       │     -> SyntaxSet<UmlLanguage>
       │     -> uml::DeclaredBundle
       │     -> uml::Projection + located diagnostics
       └── future_specialization::analyze
             -> specialization-owned syntax/declared/projection state
```

Generic OKF Concepts stop before declared UML projection. Index, Log, and
Directory never enter specialization claiming. Bundle resolution builds a
transient index for the requested immutable Bundle revision; it does not create
a persistent compiler symbol database.

The semantic types do not store green/red nodes or revision-scoped syntax
locators. `okf::Bundle` remains parser-neutral; `uml::DeclaredBundle` is the
revision-scoped UML bridge from syntax to semantics; validated projections
contain only computation-ready semantic values.

Existing tests and the old parser remain a differential product-semantics oracle
during development. Deliberate changes from the first-class OKF plan—selective
UML claiming, no structural packages, separate reserved documents, and generic
fallback—are the new expected behavior rather than differences to suppress.

## Editing and formatting

Syntax edits are versioned values:

```rust
pub struct TextEdit {
    pub range: TextRange,
    pub replacement: Arc<str>,
}

pub struct VersionedDocumentChange {
    pub document: DocumentId,
    pub base_document_revision: DocumentRevision,
    pub edits: Arc<[TextEdit]>,
}

pub enum ActionBasis {
    Document {
        document: DocumentId,
        document_revision: DocumentRevision,
        session_revision: u64,
    },
    Bundle {
        session_revision: u64,
    },
}

pub struct CodeAction {
    pub title: String,
    pub basis: ActionBasis,
    pub changes: Arc<[VersionedDocumentChange]>,
}
```

Edits validate range ordering, overlap, UTF-8 boundaries, document identity,
document revision, and session revision. A stale action fails; it never applies
numeric offsets to changed source. A code action derived from cross-document
resolution uses `ActionBasis::Bundle` even when it edits one document.

`waml-syntax` does not apply these edits to product state. The `waml` layer wraps
validated syntax changes in a sealed batch:

```rust
pub struct EditContext<'a> {
    pub source: &'a SourceBundle,
    pub okf_analysis: &'a OkfAnalysis,
    pub session_revision: u64,
    pub uml: &'a uml::Analysis,
    // A future built-in specialization adds its analysis here at the one
    // explicit static composition point.
}

pub struct SyntaxChangeBatch {
    action: CodeAction,
}

impl EditBatch for SyntaxChangeBatch {
    fn lower(&self, context: EditContext<'_>)
        -> Result<SourceBundle, EditError>;
}
```

This is the parser-era extension of the prerequisite plan's `EditContext`; it
replaces the bare `&uml::Projection` with the owning UML analysis and adds the
shared catalog/shell/revision required for stale validation.

`SyntaxChangeBatch::lower` verifies its basis against
`context.okf_analysis.catalog`/`context.session_revision`, clones the candidate
`SourceBundle`, and copy-on-write edits only touched documents. A
domain-specific action also validates its language-specific `SyntaxLocator`
against that domain's analysis before producing the source edit. Application
remains:

```rust
session.apply(SyntaxChangeBatch::new(action))
```

Domain operations remain domain-owned:

- `okf::Batch` expresses Concept/Directory/Index/Bundle intent;
- `uml::Batch` expresses classifier, relationship, diagram, and placement
  intent;
- sealed `compat::Batch` exists only for retained ordered Rust DTO/CLI
  compatibility; and
- the parser platform supplies syntax-native implementation machinery to those
  Lowerers without recombining their vocabularies.

There are two distinct writing operations:

1. syntax-tree writing exactly preserves authored source; and
2. a specialization formatter such as
   `uml::Formatter::format(&SyntaxSnapshot<UmlLanguage>) -> CodeAction`
   proposes canonical domain edits.

The formatter leaves raw Generic OKF Markdown and malformed recovery content
alone unless an owned repair action explicitly changes it.

A green rewriter may prepare a replacement tree for performance or annotation
retention, but it cannot install that tree directly. The session accepts it only
alongside matching candidate source, verifies exact writing, derives candidate
OKF/UML state, and commits everything atomically.

`GenericOkfView` remains read-only in the prerequisite feature. Parser
capability does not silently expand its product editing scope.

## Atomic session integration

After parser integration, the prepare-then-commit flow becomes:

```text
OKF batch / UML batch / syntax code action
  -> sealed EditBatch::lower
  -> candidate SourceBundle
  -> candidate OkfAnalysis
       shell syntax -> OKF Bundle -> DomainAnalysisContext
  -> each statically composed specialization analysis
       ├── uml::Analysis
       └── future specialization analysis
  -> validate disjoint Concept ownership
  -> atomically commit session state
```

The composition preserves this dependency order. Internal work may be reused or
incremental as long as:

- each final candidate source is shell-parsed once;
- specialization parsers reuse the shared source and Markdown structure map
  rather than rediscovering document boundaries independently;
- the complete OKF Bundle is constructed once;
- each specialization constructs its declared and validated projection once;
- every recognizer uses the candidate OKF Bundle;
- claim ambiguity fails candidate analysis before commit;
- no `self` field mutates until every candidate succeeds; and
- revision and dirty state advance once.

Parser diagnostics remain visible but do not by themselves reject a successfully
lowered edit. Structural failure to construct source, syntax, OKF, or required
projection invariants rejects the transaction.

## Retained host integration

- The native editor owns the long-lived `EditorSession`, `OkfAnalysis`, and
  statically composed specialization analyses.
- The CLI may construct a short-lived session/syntax set for parse, validate,
  format, and operations.
- The Rust LSP retains FULL sync initially, maps host paths into `BundlePath`,
  and converts ranges through `LineIndex` to UTF-16.
- The LSP moves to incremental protocol changes only after edit/range tests and
  parser reuse are ready.
- The VS Code extension continues launching `waml lsp --stdio`; it owns no
  parser or semantic model.
- Automation uses retained Rust entry points.

There is no WASM host, web host, generated TypeScript model, browser storage,
deployment adapter, or `waml serve` browser-product compatibility target in this
design.

## Replacement and implementation sequence

0. **Prerequisite gate.** Complete and verify
   `2026-07-27-first-class-okf-documents.md`. Confirm shared `SourceBundle`,
   first-class OKF, selective UML, split Lowerers, one atomic session apply,
   static document providers, Generic OKF UX, retained CLI/LSP/VS Code, and
   removal of web/WASM.
1. **Post-OKF baseline.** Freeze retained core/editor fixtures, malformed
   documents, LSP UTF-16/CRLF/Unicode cases, directories/indexes/logs, generic
   Concepts, UML grammars, serializer output, diagnostics, parse latency,
   allocations, and retained snapshot memory. The old parser is a differential
   oracle, not a production compatibility layer.
2. **Syntax kernel.** Add `waml-syntax` text/range, shared `SourceText`,
   green/red trees, exact writer, annotations, Markdown structure map, recovery,
   and lossless raw Markdown.
3. **Shell and representative claimed island.** Implement domain-neutral
   frontmatter/document shell plus claimed-UML attributes, normative typed
   slots, recovery, `DeclaredAttribute`, and validated lowering. Prove arbitrary
   and missing types remain Generic OKF.
4. **Complete claimed UML grammar.** Implement values/slots/relationships,
   members/inline instances, layout, flows, and sequences. Keep Generic OKF,
   Index, and Log syntax lossless without projecting them into UML.
5. **Derived syntax/session integration.** Add `analyze_okf`,
   `DomainAnalysisContext`, language-parameterized lookup-only `SyntaxSet`,
   session-bound revisions, the UML sibling analysis, syntax code-action
   adapter, atomic candidate installation, and source-allocation tests. Do not
   add public workspace mutation APIs.
6. **Projection, formatting, and Lowerer internals.** Build declared UML
   projection, located diagnostics, canonical formatter, and syntax-native
   implementations beneath existing OKF/UML Lowerers.
7. **Retained host cutover.** Move core, native editor, CLI, Rust LSP, VS Code
   contract tests, and automation onto the new parser together. Do not restore
   removed compatibility surfaces.
8. **Convergence gate.** Inventory every call to old `parse_document`,
   `Line<T>`, grammar renderers, `serialize_document`, and legacy mutation entry
   points. Migrate remaining retained consumers, delete the old parser/semantic
   syntax/serializer paths and temporary differential adapters, and require one
   parser authority.
9. **Classifier operations.** After a separate approved syntax/model design,
   implement `## Operations` as another claimed UML island/projection/editing
   family.
10. **Incremental reuse.** Implement incremental lexing, smallest-safe island
    reparse, and green reuse. Full parse remains the oracle and fallback.
    Retention tests must prove repeated edits do not pin an unbounded chain of
    historical document allocations.

Each stage compiles and passes focused tests before the next begins.

## Verification and success criteria

### Syntax and recovery

- Exact source equality for every corpus input and arbitrary UTF-8 fuzz input.
- Parser progress/no panic; all red ranges in bounds; child full widths sum to
  parent width; token/trivia concatenation equals source.
- Recovery goldens for missing punctuation, malformed multiplicity/frontmatter,
  bad links/layout, malformed nested flow/sequence/member input, CRLF, and
  Unicode.
- Typed-slot tests proving recovery children never change accessor meaning.
- Annotation/tracking tests proving annotations survive an atomically installed
  green rewrite, locators reject a different tree/revision, adjacent zero-width
  nodes remain distinguishable, and full text reparse makes no false tracking
  promise.
- Formatter idempotence and raw-Markdown/recovery preservation.
- `proptest` arbitrary input in CI plus `cargo-fuzz` targets for outer mapping,
  claimed islands, edits, and parse/write roundtrip.

### OKF/UML boundaries

- Clean frontmatter preserves arbitrary and missing type values.
- Malformed frontmatter recovery never requires a recognized UML metaclass.
- Arbitrary types and unknown `uml.*` remain lossless Generic OKF syntax.
- Unclaimed Concepts do not expose typed UML section accessors.
- Index and Log never become Concepts or typed UML documents.
- Structural directories never become UML packages.
- Supported UML Concepts retain current validated projection behavior.
- Invalid-present fields remain distinguishable from absent fields.
- A test-only sibling specialization defines its own syntax kinds, consumes
  `DomainAnalysisContext`, claims a non-UML type, and produces analysis without
  changing OKF types, the shell language, UML grammar, or `DocumentHost`.
- Two specialization recognizers claiming the same Concept produce an explicit
  ambiguity error before session commit.

### Authority and transactions

- Shell and specialization `SyntaxSet` values have no public source mutation,
  persistence, or independent revision API.
- Shell and specialization syntax sets share the exact `DocumentCatalog`; every
  common path has the same `DocumentId`, `DocumentRevision`, and
  `Arc<DocumentVersion>`, and specialization code cannot allocate IDs.
- Every syntax action enters through `EditorSession::apply`.
- Invalid/overlapping ranges and stale document/bundle actions fail.
- Failed syntax/lowering/OKF/specialization construction leaves source, shell
  syntax, every specialization analysis and claim set, revision, dirty state,
  and allocation identities unchanged. The current UML analysis is covered
  explicitly by this assertion.
- Successful changes rebuild OKF once, each specialization once, increment
  revision once, and mark dirty once.
- Clean current/persisted/syntax/semantic views share document allocations.
- One touched document receives one new source allocation; untouched documents
  remain shared.
- Repeated incremental edits do not retain unbounded historical source
  allocations.

### Retained consumers

- LSP UTF-16, CRLF, Unicode, stale-action, and broken-frontmatter tests pass.
- CLI, serde, and `waml-ops-dto` tests pass for retained Rust contracts.
- The VS Code extension builds/tests and launches the Rust LSP over stdio.
- No active code or test requires web, WASM, generated TypeScript, or browser
  compatibility.
- Focused `waml` and `waml-editor` tests pass after each migration stage.
- `cargo test --workspace`, clippy, VS Code, and native visual gates pass before
  integration.

No invented latency threshold is approved before the post-OKF baseline exists.
Record hardware and corpus, then set explicit budgets. Non-negotiable objective
gates are losslessness, progress, no panic, intentional product-semantic
coverage, one source authority, one mutation choke point, and bounded source
retention.

## Advantages, risks, and rejected alternatives

Advantages:

- structured edits survive partial syntax without polluting validated semantic
  types with recovery wrappers;
- OKF ownership and Generic OKF fallback remain intact;
- one source/syntax/projection foundation serves every retained host;
- source preservation and canonical formatting are distinct;
- ranges, revisions, stale actions, and cross-document boundaries are explicit;
  and
- incremental reuse has a natural later implementation path.

Risks and hotspots:

- Markdown boundary correctness and nested flow/sequence/member recovery;
- preserving the OKF/UML claim boundary while introducing typed islands;
- temporary parser duplication inside the development branch;
- token/trivia allocation and historical-source retention under reuse;
- coordinating retained native/CLI/LSP consumers without restoring dead
  adapters; and
- annotation survival must not be overstated before incremental reuse.

Rejected alternatives:

- a public Roslyn-style `DocumentSet` workspace: it creates a second source,
  lifecycle, and revision authority beside `EditorSession`;
- public parser-side insert/remove/rename/replace APIs: they bypass domain
  Lowerers and atomic session apply;
- parsing every Markdown H2 as WAML: it misclassifies unclaimed OKF Concepts;
- extending `Line<T> | ErrorNode`: it cannot represent partial fields/tokens or
  reliable structured edits;
- immutable ranged AST without green/red: it loses reuse, expected-token, and
  red-position properties;
- rope storage before measurement;
- a compiler workspace/symbol graph or host lifecycle in parser core;
- full editable CommonMark before WAML islands;
- fake stable IDs across full reparse;
- retaining web/WASM adapters for compatibility; and
- shipping parallel old/new parser, serializer, source, or mutation authorities.

## Remaining product decisions

Technical integration with the OKF architecture is resolved by this revision.
The remaining user decisions are:

1. Should classifier operations follow current-grammar parity immediately, or
   be a separately scheduled next language feature? Recommendation: keep the
   separate grammar design immediately after platform core.
2. Should the long-term explicit formatter preserve today's broad canonical
   behavior exactly, or later become minimally disruptive? Recommendation:
   preserve current behavior first; changing it is a separate user-visible
   format decision.

Incremental reuse remains a committed Roslyn-lite follow-up, with full reparse
as oracle and fallback.

## Relationship to diagram properties

Diagram properties remain on their narrow semantic path: valid multiplicity is
`Option<Multiplicity>`, `None` means not authored/default, and malformed source
remains in existing recovery behavior. This design introduces no temporary
generic wrapper into that work. Once implemented, properties can consume
declared attributes and targeted diagnostics for richer repair UI.
