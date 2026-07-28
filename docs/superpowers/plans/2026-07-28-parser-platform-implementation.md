# Parser Platform Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the legacy semantic/recovering WAML parser with one lossless, revision-scoped Roslyn-lite parser platform that derives OKF and statically composed UML analyses from the authoritative `SourceBundle`, preserves malformed source, routes native-editor edits through atomic `EditorSession::apply`, and gives the CLI/LSP pure host transactions over the same preparation boundary.

**Architecture:** Add a domain-neutral `waml-syntax` crate for UTF-8 text, immutable language-parameterized green/red trees, exact writing, recovery, and the lossless Markdown/frontmatter shell. The `waml` crate owns source/document identity, pure candidate preparation, OKF analysis, static specialization composition, UML syntax/declared/validated projections, actions, formatting, and Lowerers. `EditorSession::apply` is the native editor's sole mutation/commit boundary; the CLI performs one ephemeral prepare/validate/write transaction and the LSP atomically swaps one prepared host snapshot, so neither becomes a parser workspace or independent parser revision authority. Full parsing is the initial oracle and fallback; measured incremental reuse follows the single-parser cutover without retaining an unbounded chain of source allocations.

**Tech Stack:** Rust 2021 with MSRV 1.80, `Arc<String>` copy-on-write source, `pulldown-cmark 0.12`, a new workspace crate `waml-syntax`, `proptest`, `cargo-fuzz`/libFuzzer, existing Makepad native UI, `tower-lsp 0.20`, and the retained TypeScript VS Code stdio client.

## Global Constraints

- Treat `docs/superpowers/specs/2026-07-26-parser-platform-design.md` at reconciled commit `298dab68` as the approved architecture and `docs/superpowers/plans/2026-07-27-first-class-okf-documents.md` as landed prerequisite authority.
- Start from local `main` commit `4252b9c5`; the clean baseline is `rtk cargo test --workspace --all-features` with 1,237 passing tests.
- Keep `source::{BundlePath, SourceDocument, SourceSlice, SourceBundle}` as the sole validated source/document authority; raw `SourceBundle` mutation helpers remain crate-private and are used only by domain Lowerers plus the narrow pure host-ingress candidate functions in Task 19.
- Keep the acyclic pipeline `SourceBundle -> lossless shell -> okf::Bundle -> statically composed specialization analyses`.
- Syntax sets are derived, immutable, lookup-only, and revision-scoped. They expose no insert, remove, rename, text replacement, save, dirty, persistence, or independent revision API.
- Keep `okf::Bundle::{Concept, Index, Log, Directory}`. Reserved `index.md` and `log.md` are structural and never become Concepts or specialization inputs.
- UML is a built-in sibling specialization, not a runtime plugin, registry entry, kernel default, global family enum, or OKF-core privilege.
- A future built-in sibling must require no change to `waml-syntax`, OKF core types, `DomainAnalysisContext`, or `DocumentHost`; only the explicit static composition root gains one call and one session field.
- Unknown, missing, arbitrary, and unknown `uml.*` types remain usable, lossless Generic OKF Concepts.
- `EditorSession::apply` is the native editor's sole mutation and atomic commit boundary. Native specialization operations, syntax rewrites, formatting, and code actions lower to candidate source and re-enter it. The CLI uses one process-local host transaction and the LSP uses atomic host ingress/snapshot replacement through `prepare_candidate`; neither exposes parser workspace mutation or an independent parser revision clock.
- One successful transaction analyzes the final shell and OKF once, analyzes each specialization once, validates disjoint claims once, installs all candidate state once, increments the session revision once, and marks dirty once.
- A failed native session transaction preserves source, persisted source, shell, OKF, all specialization analyses/claim sets, revision, dirty state, and every pre-existing shared allocation identity. Failed CLI/LSP candidate preparation publishes/writes nothing and leaves the prior host snapshot unchanged.
- Parser recovery diagnostics remain visible but do not reject an otherwise structurally valid candidate; only failure to construct source, shell, OKF, required specialization invariants, or disjoint claims rejects the transaction.
- Every text position is a checked half-open UTF-8 byte range. Oversized input and non-boundary ranges fail; no position arithmetic wraps.
- Untouched syntax writes byte-for-byte exact source, including malformed input. Every source byte belongs to exactly one token text or concrete token trivia.
- Missing lexical items use their expected token kind, empty spelling, zero width, and a missing flag. `BadToken` is non-empty; misplaced valid tokens live in `SkippedTokensSyntax`.
- Green elements contain no parent, absolute position, product document identity, revision, or diagnostics. Red identity is tree instance plus complete child-index path.
- Current syntax and semantics share current source allocations; clean persisted state shares them too. An unchanged document revision may share its whole tree. After a text change, source-backed green tokens and every ancestor containing one are rebuilt/rebased onto the current `SourceText`; only source-independent static/owned greens may retain identity. Incremental reuse may not retain an unbounded chain of historical whole-document allocations.
- Preserve the current broad canonical formatter behavior first. Exact syntax writing and canonical formatting remain distinct operations.
- Keep classifier `## Operations` out of this plan; it requires a separate approved syntax/model design.
- Keep FULL LSP sync until incremental source-change mapping is proven. Keep the VS Code extension as a TypeScript client that launches `waml lsp --stdio`.
- Do not restore web, WASM, browser, Svelte, generated TypeScript-domain, or JavaScript semantic compatibility.
- Every task compiles and passes its focused gate before the next task begins.
- Except for Task 1 characterization, Task 20's test-only transport lock, and Task 24 verification, perform the first test-writing checkbox before that task's production edits, immediately run the task's first focused test command, and require a red result naming the missing interface or old behavior described in the checkbox. After each implementation checkbox, rerun that focused command and require PASS before continuing; the final listed gate must also PASS. Do not weaken an assertion to turn red green.
- Every shell command in this plan starts with `rtk`, per `RTK.md`.

---

## File Structure

```text
Cargo.toml
├── adds crates/waml-syntax and shared proptest dev dependency
crates/waml-syntax/
├── Cargo.toml                    # domain-neutral syntax crate
├── src/lib.rs                    # public re-exports only
├── src/text.rs                   # TextSize/TextRange/SourceText/LineIndex
├── src/green.rs                  # green nodes/tokens/text/trivia/factory/writer
├── src/red.rs                    # trees, positioned facades, locators, identity
├── src/ast.rs                    # typed casts, slots, visitor, rewriter
├── src/annotation.rs             # annotations and occurrence tracking
├── src/markdown.rs               # CommonMark structure map
├── src/shell.rs                  # OKF Markdown language and shell API
├── src/shell/parser.rs           # lossless shell/frontmatter parser and recovery
└── src/incremental.rs            # TextChange, safe windows, reuse/rebase
crates/waml/src/
├── analysis.rs                   # document catalog, OKF analysis, domain context/claims
├── action.rs                     # versioned edits/actions and SyntaxChangeBatch
├── host.rs                       # pure add/replace/remove candidate ingress for retained hosts
├── okf.rs / okf/shell.rs         # semantic OKF derivation from shell
├── uml.rs / uml/analysis.rs      # recognizer and complete sibling analysis
├── uml/syntax/{mod,kind,ast,parser}.rs
├── uml/declared.rs               # tolerant declared UML fields/bundle
├── uml/format.rs                 # explicit canonical formatter
├── okf/lower.rs / uml/lower.rs   # syntax-native Lowerer internals
├── edit.rs                       # parser-era EditContext
├── frontmatter.rs                # semantic frontmatter value helpers only after cutover
├── validate.rs                   # located domain diagnostics
└── grammar.rs/parse.rs/syntax.rs/serialize.rs
                                      # deleted after all retained consumers migrate
crates/waml-editor/src/
├── editor_session.rs             # owns source + OkfAnalysis + uml::Analysis
├── documents.rs / uml_documents.rs / okf_documents.rs
├── document_host.rs              # still consumes provider-prepared OpenDocument
├── generic_okf_view.rs / source_view.rs
└── app.rs / app/actions.rs       # syntax diagnostics/actions and session routing
crates/waml-cli/src/
├── commands.rs / main.rs         # short-lived shared analysis for CLI
└── lsp/{bundle,map,server}.rs     # catalog/range/action adapter, no parser fork
packages/vscode/src/              # unchanged stdio ownership; contract tests expand
fuzz/
├── Cargo.toml
└── fuzz_targets/{outer_mapping,uml_islands,syntax_edits,parse_write}.rs
```

`waml-syntax` never imports `waml`, source paths, OKF/UML semantic types, product diagnostics, sessions, persistence, IO, URIs, or host lifecycle. `waml::analysis` is the sole allocator of `DocumentId` and `DocumentRevision`; each specialization stores the exact catalog and document-version `Arc`s it receives.

### Task 1: Freeze the Post-OKF Differential and Resource Baseline

**Files:**
- Create: `crates/waml/tests/fixtures/parser-platform/` with `generic.md`, `unknown-uml.md`, `index.md`, `log.md`, `class.md`, `enum.md`, `object.md`, `diagram.md`, `activity.md`, `state-machine.md`, `sequence.md`, and malformed CRLF/Unicode variants
- Create: `crates/waml/examples/parser_platform_baseline.rs`
- Create: `docs/superpowers/baselines/2026-07-28-parser-platform-method.json`
- Create execution evidence outside the repository: `C:\tmp\parser-platform-baseline\post-okf.json`
- Modify: `crates/waml/src/parse.rs` inline tests
- Modify: `crates/waml/src/serialize.rs` inline tests
- Modify: `crates/waml/src/validate.rs` inline tests
- Modify: `crates/waml/tests/golden.rs`
- Modify: `crates/waml-editor/src/editor_session.rs` inline tests
- Modify: `crates/waml-cli/tests/lsp_e2e.rs`

**Interfaces:**
- Consumes: landed `SourceBundle`, `okf::Bundle::parse`, `uml::project`, legacy `parse_document`, `serialize_document`, validation, Lowerers, editor transaction, and LSP range mapping.
- Produces: a checked-in semantic/recovery/serializer golden corpus and method record containing the ordered fixture list, dependency-free FNV-1a-64 corpus identity algorithm, warmup/sample counts, measurement fields, `enforcement: "report-only"`, and hardware-fingerprint fields captured in each untracked local observation.
- Produces an untracked local observation containing hardware/OS/Rust metadata, 30-run median/p95 parse nanoseconds, median absolute deviation, total peak live bytes, allocated bytes, and allocation count. A generic allocator reports process allocations only; it makes no claim about which live allocations are `Arc<String>`.
- Correctness and whole-source retention are hardware-independent test gates. Task 22's crate-private weak-reference tests are the authoritative whole-source retention gate. Latency comparison remains report-only until a separately reviewed stable budget is checked into the method record; absent/mismatched hardware prints a skip and exits successfully.
- Corpus identity requires no dependency: start from FNV-1a-64 offset basis `0xcbf29ce484222325`, process each normalized fixture path's UTF-8 bytes, one zero separator byte, source bytes, and one `0xff` separator byte in listed order, XOR each byte, then multiply wrapping by prime `0x100000001b3`; serialize as 16 lowercase hex digits.

- [ ] **Step 1: Add source and semantic fixtures**

Write the exact retained grammar families into the named files: supported classifier frontmatter and Attributes/Slots/Values/Relationships, diagram Members/Layout, activity/state nodes and transitions, sequence Lifelines/Messages, nested headings, arbitrary/missing type, unknown `uml.*`, reserved documents, broken opening frontmatter, missing punctuation, bad links/layout, CRLF, astral Unicode, combining marks, and trailing whitespace. Keep `## Operations` as raw Markdown in `class.md`; do not give it semantic assertions.

- [ ] **Step 2: Add differential tests before changing parser code**

For every fixture, assert the current OKF membership, selective UML claims, projection values, diagnostics `(code,severity,line,span)`, canonical `serialize_document(parse_document(src))`, Lowerer output, UTF-16 LSP positions, and failed/successful session atomicity. Normalize only deliberately unstable diagnostic display prose; paths, codes, ranges, semantic values, and bytes remain exact.

- [ ] **Step 3: Run the characterization suite**

Run:

```powershell
rtk cargo test -p waml parser_platform_baseline
rtk cargo test -p waml --test golden
rtk cargo test -p waml-editor editor_session::tests
rtk cargo test -p waml-cli --test lsp_e2e
```

Expected: PASS against the landed parser; every later differential mismatch names the fixture and semantic field.

- [ ] **Step 4: Implement and record the resource baseline**

The example must read only the method record's ordered checked-in corpus, verify the FNV-1a-64 identity above, warm up five runs, measure 30 isolated runs, use a counting global allocator scoped to the single-threaded measurement process, and emit stable-key JSON. It does not inspect or classify `Arc<String>` liveness. `--compare` compares only matching corpus and hardware fingerprints, always reports the delta, and exits zero with `LATENCY_REPORT_ONLY`; a hardware mismatch exits zero with `LATENCY_SKIPPED_HARDWARE_MISMATCH`.

Run:

```powershell
rtk proxy pwsh -NoProfile -Command 'New-Item -ItemType Directory -Force -Path "C:\tmp\parser-platform-baseline" | Out-Null'
rtk cargo run -p waml --example parser_platform_baseline --release -- --method docs/superpowers/baselines/2026-07-28-parser-platform-method.json --record C:\tmp\parser-platform-baseline\post-okf.json
rtk cargo run -p waml --example parser_platform_baseline --release -- --method docs/superpowers/baselines/2026-07-28-parser-platform-method.json --compare C:\tmp\parser-platform-baseline\post-okf.json
```

Expected: the observation contains complete non-zero measurements; the comparison prints `LATENCY_REPORT_ONLY` on the same machine or `LATENCY_SKIPPED_HARDWARE_MISMATCH` elsewhere and exits zero.

- [ ] **Step 5: Commit the baseline**

```powershell
rtk git add crates/waml/tests/fixtures/parser-platform crates/waml/examples/parser_platform_baseline.rs docs/superpowers/baselines/2026-07-28-parser-platform-method.json crates/waml/src/parse.rs crates/waml/src/serialize.rs crates/waml/src/validate.rs crates/waml/tests/golden.rs crates/waml-editor/src/editor_session.rs crates/waml-cli/tests/lsp_e2e.rs
rtk git commit -m "test: freeze parser platform baseline"
```

### Task 2: Add Checked UTF-8 Text and the Green Syntax Kernel

**Files:**
- Create: `crates/waml-syntax/Cargo.toml`
- Create: `crates/waml-syntax/src/lib.rs`
- Create: `crates/waml-syntax/src/text.rs`
- Create: `crates/waml-syntax/src/green.rs`
- Create: `crates/waml-syntax/tests/text_green.rs`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`

**Interfaces:**
- Produces:

```rust
pub struct TextSize(u32);
pub struct TextRange { start: TextSize, end: TextSize }
pub struct SourceText { source: Arc<String> }
pub struct LineIndex { line_starts: Arc<[TextSize]> }
pub struct LineColumn { pub line: u32, pub byte_column: u32 }
pub enum MarkdownDialect { CommonMarkCurrent }
pub enum TextError {
    SourceTooLarge { bytes: usize },
    WidthOverflow { left: TextSize, right: TextSize },
    ReversedRange { start: TextSize, end: TextSize },
    OutOfBounds { range: TextRange, len: TextSize },
    NonUtf8Boundary { offset: TextSize },
}

pub trait SyntaxLanguage: Send + Sync + 'static {
    type Kind: Copy + Eq + Hash + Debug + Send + Sync;
    type DiagnosticCode: Copy + Eq + Hash + Debug + Send + Sync;
}

pub type GreenNode<L> = Arc<GreenNodeData<L>>;
pub type GreenToken<L> = Arc<GreenTokenData<L>>;
pub enum GreenElement<L: SyntaxLanguage> { Node(GreenNode<L>), Token(GreenToken<L>) }
pub enum GreenText {
    Static(&'static str),
    SourceSlice { source: SourceText, range: TextRange },
    Owned(Arc<str>),
}
pub struct GreenTrivia { pub kind: TriviaKind, pub text: GreenText }
pub struct TokenFlags(u8);
pub struct GreenFactory<L: SyntaxLanguage>(PhantomData<L>);

impl TextSize {
    pub fn try_from_usize(value: usize) -> Result<Self, TextError>;
    pub fn to_usize(self) -> usize;
    pub fn checked_add(self, rhs: TextSize) -> Result<Self, TextError>;
}
impl TextRange {
    pub fn new(start: TextSize, end: TextSize) -> Result<Self, TextError>;
    pub fn start(self) -> TextSize;
    pub fn end(self) -> TextSize;
    pub fn len(self) -> TextSize;
}
impl SourceText {
    pub fn from_shared(source: Arc<String>) -> Result<Self, TextError>;
    pub fn len(&self) -> TextSize;
    pub fn slice(&self, range: TextRange) -> Result<&str, TextError>;
}
```

- `SourceText::from_shared(Arc<String>) -> Result<SourceText, TextError>` clones only the `Arc`, rejects byte lengths above `u32::MAX`, and exposes checked slice/range/UTF-8-boundary operations.
- `TextError: Display + std::error::Error`; `TextSize::try_from_usize` and `SourceText::from_shared` return `SourceTooLarge`, `checked_add` returns `WidthOverflow`, and `TextRange::new` returns `ReversedRange`. `SourceText::slice` checks `end <= len` first (`OutOfBounds`), then start and end UTF-8 boundaries in that order (`NonUtf8Boundary`); it never panics or truncates.
- `LineIndex::line_col` and `LineIndex::utf16_column` accept `&SourceText` and checked offsets; UTF-16 conversion has no LSP dependency.
- `GreenFactory::token`, `missing_token`, `bad_token`, and `node` validate widths. Missing tokens have expected kind, empty text/trivia, zero width; bad tokens reject empty spelling.
- `write_green_to<W: fmt::Write>` emits ordered token `leading + text + trailing`; nodes emit no bytes.
- `GreenText::SourceSlice` is source-revision-bound. `GreenTokenData::is_source_independent()` is true only when token text and every trivia item are `Static` or `Owned`; `GreenNodeData::is_source_independent()` is true only when every descendant is. Across a changed `DocumentRevision`, only elements for which this predicate is true may satisfy `same_green`. Source-backed leaves are rebuilt with ranges into the current `SourceText`, and their ancestors are rebuilt even when authored bytes are unchanged.

- [ ] **Step 1: Write failing checked-text tests**

Test zero/end offsets, multibyte boundaries in `"aé𝄞\r\n"`, half-open ranges, checked addition overflow, CRLF line starts, UTF-16 columns, shared `Arc<String>` identity, and explicit oversized-width construction through a test-only checked-length helper.

Run `rtk cargo test -p waml-syntax --test text_green`.

Expected: FAIL because the workspace crate and checked text/green interfaces do not exist.

- [ ] **Step 2: Implement text primitives and line indexing**

Use private scalar fields, `TryFrom<usize>`, checked `Add`, `TextRange::new`, `contains`, `cover`, and `SourceText::slice`. Do not expose the underlying string mutably and do not copy it while constructing `LineIndex`.

- [ ] **Step 3: Write failing green/trivia/writer tests**

Define a test language and assert deterministic output for identifier + spaces-before-newline + CRLF, EOF leading whitespace, source-slice text, static punctuation, owned replacement text, missing colon, non-empty bad token, and parent width equal to checked child-width sum.

- [ ] **Step 4: Implement green storage, factory, and exact writer**

Keep every green type parentless/positionless and diagnostic-free. Use `Arc<[T]>` children/trivia/annotations and compute widths in constructors; reject overflow rather than truncating.

Add predicate tests proving a static missing colon and owned punctuation are source-independent, a source-slice identifier is not, and any parent containing that identifier is not. These predicates are the only cross-text-change green-sharing permission used by Task 22.

- [ ] **Step 5: Run and commit**

```powershell
rtk cargo test -p waml-syntax --test text_green
rtk cargo check --workspace
rtk git add Cargo.toml Cargo.lock crates/waml-syntax
rtk git commit -m "feat: add lossless syntax kernel"
```

Expected: PASS; `waml-syntax` has no dependency on `waml`, serde, tower-lsp, Makepad, filesystem, or URI libraries.

### Task 3: Add Red Identity, Typed Slots, Traversal, Rewriting, and Tracking

**Files:**
- Create: `crates/waml-syntax/src/red.rs`
- Create: `crates/waml-syntax/src/ast.rs`
- Create: `crates/waml-syntax/src/annotation.rs`
- Create: `crates/waml-syntax/tests/red_ast.rs`
- Modify: `crates/waml-syntax/src/lib.rs`
- Modify: `crates/waml-syntax/src/green.rs`

**Interfaces:**
- Produces:

```rust
pub struct TreeInstanceId(NonZeroU64);
pub struct SyntaxTree<L: SyntaxLanguage> {
    root_green: GreenNode<L>,
    context: Arc<RedContext>,
    diagnostics: Arc<[TreeDiagnostic<L::DiagnosticCode>]>,
    dialect: MarkdownDialect,
}
pub enum SyntaxSeverity { Error, Warning, Info }
pub struct TreeDiagnostic<C> {
    pub code: C, pub severity: SyntaxSeverity, pub message: Arc<str>, pub range: TextRange,
}
pub struct SyntaxNode<L: SyntaxLanguage>(Arc<RedNodeData<L>>);
pub struct SyntaxToken<L: SyntaxLanguage>(Arc<RedTokenData<L>>);
pub enum SyntaxElement<L: SyntaxLanguage> { Node(SyntaxNode<L>), Token(SyntaxToken<L>) }
pub struct SyntaxPath(Arc<[u32]>);
pub struct SyntaxLocator<L: SyntaxLanguage> {
    tree: TreeInstanceId,
    path: SyntaxPath,
    expected_kind: L::Kind,
}
pub trait AstNode<L: SyntaxLanguage>: Sized {
    fn can_cast(kind: L::Kind) -> bool;
    fn cast(node: SyntaxNode<L>) -> Option<Self>;
    fn syntax(&self) -> &SyntaxNode<L>;
}
pub trait SyntaxVisitor<L: SyntaxLanguage> { fn visit(&mut self, element: SyntaxElement<L>); }
pub trait SyntaxRewriter<L: SyntaxLanguage> {
    fn rewrite_node(&mut self, node: &GreenNode<L>) -> GreenNode<L>;
    fn rewrite_token(&mut self, token: &GreenToken<L>) -> GreenToken<L>;
}
pub struct SyntaxAnnotation { id: NonZeroU64, kind: Arc<str>, data: Option<Arc<str>> }
pub enum RewriteError<K> {
    WrongTree { expected: TreeInstanceId, actual: TreeInstanceId },
    InvalidPath { depth: usize, child_index: u32 },
    ExpectedNode { path: SyntaxPath },
    ExpectedToken { path: SyntaxPath },
    KindMismatch { expected: K, actual: K },
    Text(TextError),
}
pub fn annotate_occurrence<L: SyntaxLanguage>(
    tree: &SyntaxTree<L>,
    locator: &SyntaxLocator<L>,
    annotation: SyntaxAnnotation,
) -> Result<GreenNode<L>, RewriteError<L::Kind>>;
pub fn find_annotation<L: SyntaxLanguage>(
    tree: &SyntaxTree<L>, id: NonZeroU64
) -> Vec<SyntaxNode<L>>;

impl<L: SyntaxLanguage> SyntaxTree<L> {
    pub fn root(&self) -> SyntaxNode<L>;
    pub fn diagnostics(&self) -> &[TreeDiagnostic<L::DiagnosticCode>];
    pub fn write_to_string(&self) -> String;
    pub fn resolve(
        &self, locator: &SyntaxLocator<L>
    ) -> Result<SyntaxElement<L>, RewriteError<L::Kind>>;
}
impl<L: SyntaxLanguage> SyntaxNode<L> {
    pub fn locator(&self) -> SyntaxLocator<L>;
}
impl<L: SyntaxLanguage> SyntaxToken<L> {
    pub fn locator(&self) -> SyntaxLocator<L>;
}
impl<L: SyntaxLanguage> SyntaxElement<L> {
    pub fn locator(&self) -> SyntaxLocator<L>;
}
impl<L: SyntaxLanguage> SyntaxLocator<L> {
    pub fn tree_id(&self) -> TreeInstanceId;
    pub fn path(&self) -> &SyntaxPath;
    pub fn expected_kind(&self) -> L::Kind;
}
impl SyntaxAnnotation {
    pub fn new(
        id: NonZeroU64,
        kind: impl Into<Arc<str>>,
        data: Option<Arc<str>>,
    ) -> Self;
    pub fn id(&self) -> NonZeroU64;
    pub fn kind(&self) -> &str;
    pub fn data(&self) -> Option<&str>;
}
```

- Red equality/hash uses `(TreeInstanceId, complete SyntaxPath)`. `same_green` is a separate `Arc::ptr_eq` query.
- `TreeInstanceId: Copy + Eq + Hash + Debug`; `SyntaxPath: Clone + Eq + Hash + Debug`; and `SyntaxLocator<L>: Clone + Eq + Hash + Debug`.
- `SyntaxLocator` has no public constructor and all fields remain private. Each red occurrence's `locator()` internally copies its `TreeInstanceId`, complete child-index path, and current kind; the three locator accessors are read-only. Callers never fabricate or mutate locator identity.
- `SyntaxTree::resolve` checks tree identity first and returns `WrongTree { expected: tree_instance_id, actual: locator.tree_id() }`; it then walks the complete path (`InvalidPath`) and compares the resolved kind (`KindMismatch`). The successful value is the exact node/token occurrence at that path.
- `SyntaxTree` owns no red-root cache; `RedContext` has no tree backpointer. Parent is O(1); child enumeration is one pass; no persistent child cache.
- `AstSlots` reads fixed node/token/list/recovery slots by declared child index; typed accessors never descendant-search by kind.
- `SyntaxAnnotation::new(id: NonZeroU64, kind: impl Into<Arc<str>>, data: Option<Arc<str>>) -> Self` is the only annotation constructor; its fields are private and the accessors above are read-only. `SyntaxPath::from_indices(impl IntoIterator<Item = u32>) -> SyntaxPath` validates only during resolution. `RewriteError<K>: Display + std::error::Error` for `K: Debug + Send + Sync + 'static`, formatting kinds with `Debug`; `From<TextError>` maps to `RewriteError::Text`.

- [ ] **Step 1: Write red occurrence and navigation tests**

In external `tests/red_ast.rs`, build one tree containing two adjacent shared zero-width missing-colon tokens at the same absolute position. Obtain both locators only through `SyntaxToken::locator`; assert equal tree IDs/kinds but unequal complete paths and locators, unequal red identities, `same_green == true`, and correct parent/sibling/range navigation. Resolve each locator to its original occurrence. Build a second tree from the same green root and assert both `SyntaxTree::resolve` and `annotate_occurrence` return exact `RewriteError::WrongTree`. Keep this integration test limited to public constructors/accessors/resolution. A compile-fail documentation example must prove private locator fields and the absence of a public constructor prevent literal construction/mutation.

In `src/red.rs`'s inline `#[cfg(test)] mod tests`, add `forged_expected_kind_is_rejected`: obtain a valid locator from a token, use module-private construction to copy its tree/path with a different `expected_kind`, and assert `SyntaxTree::resolve` returns exact `RewriteError::KindMismatch { expected: forged_kind, actual: resolved_kind }`.

Run `rtk cargo test -p waml-syntax --test red_ast`.

Expected: FAIL because red facades, locators, and path-based identity do not exist.

Run:

```powershell
rtk cargo test -p waml-syntax red::tests::forged_expected_kind_is_rejected -- --exact
```

Expected: FAIL because private locator construction and checked kind resolution do not exist.

- [ ] **Step 2: Implement red facades and locator resolution**

Construct one parent-chain facade per navigation request. Have each public red `locator()` call one crate-private constructor with its context tree ID, complete path, and kind. Resolve locators only when tree ID, every child index, and expected kind match, returning the exact `RewriteError` variant above instead of `Option`.

Run:

```powershell
rtk cargo test -p waml-syntax red::tests::forged_expected_kind_is_rejected -- --exact
```

Expected: PASS with the exact `KindMismatch` assertion; the external test never forges private locator state.

- [ ] **Step 3: Write typed-slot/traversal/rewriter tests**

Define a test typed node with required name/colon, optional value, repeated list, and recovery slot. Insert skipped tokens and prove all declared accessor indices remain stable. Verify pre-order visitor order and structural sharing of untouched siblings after a one-token rewrite.

- [ ] **Step 4: Implement slots, visitor, factory-facing rewriter, and annotations**

`annotate_occurrence(tree, locator, annotation)` first delegates to the tree-bound checked resolution above, then rebuilds only the path to that exact occurrence. Keep its bare-`SyntaxPath` green rebuild helper crate-private so no public rewrite can bypass tree/kind validation. `find_annotation` returns current-tree occurrences and makes no cross-full-reparse promise. Rewriting preserves existing annotations unless the replaced element is intentionally discarded.

- [ ] **Step 5: Run and commit**

```powershell
rtk cargo test -p waml-syntax --test red_ast
rtk cargo test -p waml-syntax
rtk git add crates/waml-syntax/src crates/waml-syntax/tests/red_ast.rs
rtk git commit -m "feat: add typed red syntax facades"
```

### Task 4: Parse a Lossless Markdown and Frontmatter OKF Shell

**Files:**
- Create: `crates/waml-syntax/src/markdown.rs`
- Create: `crates/waml-syntax/src/shell.rs`
- Create: `crates/waml-syntax/src/shell/parser.rs`
- Create: `crates/waml-syntax/tests/shell_roundtrip.rs`
- Create: `crates/waml-syntax/tests/fixtures/shell/`
- Modify: `crates/waml-syntax/src/lib.rs`
- Modify: `crates/waml-syntax/Cargo.toml`

**Interfaces:**
- Produces:

```rust
pub struct OkfMarkdownLanguage;
pub enum OkfMarkdownSyntaxKind {
    Root, Frontmatter, FrontmatterOpenFence, FrontmatterEntry,
    FrontmatterKey, ColonToken, FrontmatterValue, FrontmatterCloseFence,
    Heading, HeadingMarkerToken, HeadingText, MarkdownRegion, RawTextToken,
    NewlineToken, EndOfFileToken, BadToken, SkippedTokensSyntax,
}
pub enum OkfSyntaxDiagnosticCode {
    FrontmatterNotClean, MissingFrontmatterFence, MalformedFrontmatterEntry,
    InvalidUtf8Boundary, ParserStalled,
}
pub struct ConfirmedHeading { pub level: u8, pub range: TextRange, pub text_range: TextRange }
pub struct MarkdownStructureMap {
    pub headings: Arc<[ConfirmedHeading]>,
    pub protected_ranges: Arc<[TextRange]>,
    pub dialect: MarkdownDialect,
}
pub struct ShellParse {
    pub tree: Arc<SyntaxTree<OkfMarkdownLanguage>>,
    pub structure: Arc<MarkdownStructureMap>,
}
pub enum ParseError {
    SourceTooLarge { bytes: usize },
    InvalidRange { range: TextRange },
    WidthOverflow,
    StructuralInvariant { reason: Arc<str> },
    ParserStalled { offset: TextSize },
}
pub fn parse_okf_markdown(
    text: SourceText, dialect: MarkdownDialect
) -> Result<ShellParse, ParseError>;
```

- `MarkdownStructureMap` uses `pulldown-cmark` event offsets plus explicit container depth. Only confirmed top-level H1/H2 outside quotes, lists/items, code blocks, HTML blocks, and other protected containers become shell headings; H3-H6 remain available to claimed islands.
- Clean fenced frontmatter is structured for arbitrary, missing, and unknown type. Only an initial fence after optional BOM can start frontmatter.
- Unclosed recovery scans plausible flat entries/blank lines, synchronizes before confirmed top-level H1, inserts missing close-fence, emits `FrontmatterNotClean`, and permits H2 fallback only when all preceding candidate lines are plausible. Insufficient evidence leaves the thematic rule raw.
- Malformed authored Markdown/frontmatter never returns `ParseError`; it returns `ShellParse` with recovery syntax/diagnostics. `ParseError` is reserved for representational or parser-progress invariants.
- `ParseError: Display + std::error::Error`.
- Trivia ownership is normative: inter-token horizontal whitespace leads the following token; whitespace before newline leads `NewlineToken`; EOF whitespace leads missing-width EOF; parser trailing trivia is empty.

- [ ] **Step 1: Write shell exactness and structure-map tests**

Cover BOM, LF/CRLF, Unicode, arbitrary/missing type, clean and broken frontmatter, later thematic rules, H1/H2 inside every protected container, H3-H6 offsets, HTML comments as raw Markdown, and trailing spaces. For each fixture assert `tree.write_to_string() == source`, leaf concatenation equals source exactly once, all ranges are bounded, widths sum, and parser progress.

Run `rtk cargo test -p waml-syntax --test shell_roundtrip`.

Expected: FAIL because `parse_okf_markdown` and `MarkdownStructureMap` do not exist.

- [ ] **Step 2: Implement the CommonMark structure mapper**

Convert `usize` offsets through checked `TextSize`, track containers explicitly, sort/non-overlap protected ranges, and return `ParseError::SourceTooLarge` for unrepresentable offsets.

- [ ] **Step 3: Implement shell/frontmatter parsing and recovery**

Build raw `MarkdownRegion` tokens between promoted structures. Never interpret Attributes/Members/Layout headings in the shell and never use recognized UML types to decide whether frontmatter exists.

- [ ] **Step 4: Add deterministic recovery goldens**

Golden-print kind, range, missing flag, trivia bytes escaped, child path, and diagnostic code/range for every shell fixture. Review the golden so every source byte appears once and recovery nodes do not own duplicate text.

- [ ] **Step 5: Run and commit**

```powershell
rtk cargo test -p waml-syntax --test shell_roundtrip
rtk cargo test -p waml-syntax
rtk git add crates/waml-syntax
rtk git commit -m "feat: parse lossless OKF markdown shell"
```

### Task 5: Derive the Revision-Scoped Catalog and OKF Analysis

**Files:**
- Create: `crates/waml/src/analysis.rs`
- Create: `crates/waml/src/okf/shell.rs`
- Create: `crates/waml/tests/analysis_catalog.rs`
- Modify: `crates/waml/Cargo.toml`
- Modify: `crates/waml/src/lib.rs`
- Modify: `crates/waml/src/okf.rs`
- Modify: `crates/waml/src/source.rs`

**Interfaces:**
- Produces:

```rust
pub struct DocumentId(u64);
pub struct DocumentRevision(u64);
pub struct DocumentVersion {
    id: DocumentId, revision: DocumentRevision, path: BundlePath,
    text: SourceText, line_index: Arc<LineIndex>,
}
pub struct DocumentCatalog {
    session_revision: u64,
    documents: Arc<BTreeMap<DocumentId, Arc<DocumentVersion>>>,
    paths: Arc<BTreeMap<BundlePath, DocumentId>>,
    next_document_id: u64,
}
pub struct SyntaxSnapshot<L: SyntaxLanguage> {
    document: Arc<DocumentVersion>, syntax: Arc<SyntaxTree<L>>,
}
pub struct SyntaxSet<L: SyntaxLanguage> {
    catalog: Arc<DocumentCatalog>,
    documents: Arc<BTreeMap<DocumentId, Arc<SyntaxSnapshot<L>>>>,
}
pub struct OkfAnalysis {
    pub catalog: Arc<DocumentCatalog>,
    pub shell: SyntaxSet<OkfMarkdownLanguage>,
    pub structures: Arc<BTreeMap<DocumentId, Arc<MarkdownStructureMap>>>,
    pub bundle: okf::Bundle,
}
pub fn analyze_okf(
    source: &SourceBundle, previous: Option<&OkfAnalysis>, session_revision: u64
) -> Result<OkfAnalysis, AnalysisError>;
pub struct DomainAnalysisContext<'a> {
    pub source: &'a SourceBundle,
    pub catalog: &'a Arc<DocumentCatalog>,
    pub shell: &'a SyntaxSet<OkfMarkdownLanguage>,
    pub structures: &'a Arc<BTreeMap<DocumentId, Arc<MarkdownStructureMap>>>,
    pub okf: &'a okf::Bundle,
    pub session_revision: u64,
}
pub struct ClaimSet { concept_ids: BTreeSet<String> }
pub enum AnalysisStage { Shell, Okf, Specialization(&'static str), Claims }
pub enum AnalysisError {
    SourceTooLarge { path: BundlePath, bytes: usize },
    Shell { path: BundlePath, source: waml_syntax::ParseError },
    Okf(okf::BundleError),
    CatalogInvariant { reason: Arc<str> },
    Specialization { name: &'static str, reason: Arc<str> },
    AmbiguousClaim { concept_id: String, first: &'static str, second: &'static str },
    StructuralInvariant { stage: AnalysisStage, reason: Arc<str> },
}

impl DocumentVersion {
    pub fn id(&self) -> DocumentId;
    pub fn revision(&self) -> DocumentRevision;
    pub fn path(&self) -> &BundlePath;
    pub fn text(&self) -> &SourceText;
    pub fn line_index(&self) -> &Arc<LineIndex>;
}
impl<L: SyntaxLanguage> SyntaxSnapshot<L> {
    pub fn document(&self) -> &Arc<DocumentVersion>;
    pub fn syntax(&self) -> &Arc<SyntaxTree<L>>;
}
impl DocumentCatalog {
    pub fn session_revision(&self) -> u64;
    pub fn document(&self, id: DocumentId) -> Option<&Arc<DocumentVersion>>;
    pub fn id_for_path(&self, path: &BundlePath) -> Option<DocumentId>;
    pub fn path_for_id(&self, id: DocumentId) -> Option<&BundlePath>;
}
impl<L: SyntaxLanguage> SyntaxSet<L> {
    pub fn catalog(&self) -> &Arc<DocumentCatalog>;
    pub fn document(&self, id: DocumentId) -> Option<&Arc<SyntaxSnapshot<L>>>;
}
impl ClaimSet {
    pub fn from_concept_ids(ids: impl IntoIterator<Item = String>) -> Self;
    pub fn contains(&self, id: &str) -> bool;
    pub fn iter(&self) -> impl Iterator<Item = &str>;
}
trait PreparationHooks {
    fn before(&mut self, stage: AnalysisStage) -> Result<(), AnalysisError>;
}
struct NoopPreparationHooks;
```

- `SyntaxSet::document` and `catalog` are the only public state accessors. Catalog allocation/update constructors are `pub(crate)`.
- `DocumentId` and `DocumentRevision` are `Copy + Eq + Ord + Hash`; their numeric constructors remain crate-private. Public consumers convert only through catalog lookup, never guessed integers.
- Malformed authored input produces a recovered `SyntaxTree` plus diagnostics and is not an `AnalysisError`. `StructuralInvariant` is reserved for impossible width/source/tree/catalog mismatches and is injectable under `#[cfg(test)]` for rollback tests.
- `AnalysisError: Display + std::error::Error`.
- `analyze_okf` delegates to private `analyze_okf_inner(..., hooks: &mut impl PreparationHooks)` with `NoopPreparationHooks`. The trait and inner function are module-private. `analysis.rs` inline tests use a private counting/failing implementation; integration tests cannot and do not reference hooks.
- Same normalized path preserves `DocumentId`; identical `Arc<String>` preserves revision and full snapshot; changed text preserves ID and increments document revision once; add gets never-reused ID at revision one; remove drops it; rename is remove-plus-add. Failed analysis consumes neither IDs nor revisions.
- `analyze_okf` shell-parses every Markdown source, verifies exact source/tree equality, and derives OKF from shell frontmatter/body ranges. `Bundle::parse(&SourceBundle)` delegates to this same shell route with no previous analysis.

- [ ] **Step 1: Write failing identity and lookup-only API tests**

Test unchanged/change/add/remove/rename candidates, exact shared `Arc<DocumentCatalog>`/`Arc<DocumentVersion>` identity, and compile-fail documentation examples proving no public mutation/revision methods exist. Malformed authored source must succeed with recovery diagnostics; structural-failure rollback belongs to the inline tests below.

Run `rtk cargo test -p waml --test analysis_catalog`.

Expected: FAIL because catalog, syntax-set, error, and lookup-only APIs do not exist.

- [ ] **Step 2: Implement catalog and syntax-set construction**

Clone the prior catalog counters into local candidate state; install nothing globally. Construct `SourceText` by cloning `SourceDocument`'s `Arc<String>` through crate-private access.

- [ ] **Step 3: Derive OKF from shell**

Move frontmatter/body-range decisions into `okf/shell.rs`. Preserve arbitrary/missing type, Index/Log/Directory separation, links/citations/extras, zero-copy `SourceSlice`, and current deterministic order. Do not import UML.

- [ ] **Step 4: Prove semantic and allocation parity**

Run the Task 1 corpus through old and new paths and compare OKF exactly. Assert shell snapshots and semantic `SourceSlice`s share current document allocation; parsing creates no second whole-document allocation.

- [ ] **Step 5: Test structural failures inline**

In `analysis.rs`'s inline test module, use private hooks to fail before Shell and OKF installation and count both phases. Assert malformed fixture source returns `Ok` with diagnostics, while injected `StructuralInvariant` returns `Err` without consuming document IDs or mutating/replacing the previous public analysis value.

Run `rtk cargo test -p waml analysis::tests::candidate_failure_is_non_mutating`.

Expected: PASS after the private hook-backed implementation; no integration test imports a private hook.

- [ ] **Step 6: Run and commit**

```powershell
rtk cargo test -p waml --test analysis_catalog
rtk cargo test -p waml okf::tests
rtk cargo test -p waml --test serde_shape
rtk git add crates/waml/Cargo.toml crates/waml/src/lib.rs crates/waml/src/analysis.rs crates/waml/src/okf.rs crates/waml/src/okf/shell.rs crates/waml/src/source.rs crates/waml/tests/analysis_catalog.rs
rtk git commit -m "feat: derive revision scoped OKF analysis"
```

### Task 6: Add the UML Language and Representative Attribute Projection

**Files:**
- Create: `crates/waml/src/uml/syntax/mod.rs`
- Create: `crates/waml/src/uml/syntax/kind.rs`
- Create: `crates/waml/src/uml/syntax/ast.rs`
- Create: `crates/waml/src/uml/syntax/parser.rs`
- Create: `crates/waml/src/uml/declared.rs`
- Create: `crates/waml/src/uml/analysis.rs`
- Create: `crates/waml/tests/uml_attribute_syntax.rs`
- Modify: `crates/waml/src/uml.rs`
- Modify: `crates/waml/src/model.rs`
- Modify: `crates/waml/src/diagnostic.rs`

**Interfaces:**
- Produces:

```rust
pub struct UmlLanguage;
pub enum UmlSyntaxKind {
    Root, FrontmatterRegion, MarkdownRegion,
    AttributesSection, Attribute, ValuesSection, Value,
    SlotsSection, Slot, RelationshipsSection, Relationship,
    MembersSection, MemberGroup, Member, InlineInstance,
    LayoutSection, LayoutStatement,
    FlowSection, FlowNode, FlowTransition, FlowBlock,
    LifelinesSection, Lifeline, MessagesSection, Message, SequenceOperand,
    Multiplicity, TypeReference, Link, SkippedTokensSyntax,
    BulletToken, VisibilityToken, IdentifierToken, ColonToken, TypeToken,
    OpenBracketToken, CloseBracketToken, CommaToken, LinkTextToken,
    LinkTargetToken, RelationshipKindToken, ArrowToken, EqualsToken,
    LayoutKeywordToken, FlowKeywordToken, MessageKeywordToken,
    HeadingMarkerToken, NewlineToken, RawMarkdownToken, BadToken,
    EndOfFileToken,
}
pub enum UmlSyntaxDiagnosticCode { MissingColon, MissingType, InvalidMultiplicity, UnexpectedToken }
pub enum ExpectedSyntax {
    ColonToken, TypeReference, ValidMultiplicity, LinkTarget,
    RelationshipTarget, LayoutOperand, FlowTarget, MessageTarget,
}
pub struct AttributeSyntax(SyntaxNode<UmlLanguage>);
impl AttributeSyntax {
    pub fn visibility_token(&self) -> Option<SyntaxToken<UmlLanguage>>;
    pub fn name_token(&self) -> SyntaxToken<UmlLanguage>;
    pub fn colon_token(&self) -> SyntaxToken<UmlLanguage>;
    pub fn type_syntax(&self) -> Option<TypeReferenceSyntax>;
    pub fn multiplicity(&self) -> Option<MultiplicitySyntax>;
    pub fn recovery(&self) -> impl Iterator<Item = SyntaxElement<UmlLanguage>>;
}
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
pub struct DeclaredConcept {
    pub concept_id: String,
    pub attributes: Arc<[DeclaredAttribute]>,
}
pub struct DeclaredBundle {
    concepts: BTreeMap<String, DeclaredConcept>,
}
impl DeclaredBundle {
    pub fn concept(&self, id: &str) -> Option<&DeclaredConcept>;
    pub fn concepts(&self) -> impl Iterator<Item = &DeclaredConcept>;
}
// In module `waml::uml`
pub struct Analysis {
    pub claims: ClaimSet,
    pub syntax: SyntaxSet<UmlLanguage>,
    pub declared: DeclaredBundle,
    pub projection: Projection,
    pub diagnostics: Arc<[Diagnostic]>,
}
pub fn analyze(
    context: DomainAnalysisContext<'_>, previous: Option<&Analysis>
) -> Result<Analysis, AnalysisError>;
```

- UML recognition still uses only supported built-in OKF Concept types. Claimed parsing reuses the shell `SourceText` and `MarkdownStructureMap`.
- Attribute slots are normative: bullet, optional visibility, required name, required colon, optional type node, optional multiplicity, newline, recovery. Missing colon remains `ColonToken`; malformed present multiplicity is `Invalid`, not `Absent`.
- Located diagnostics use bundle path, document/revision, byte range, `DiagCode`, severity, and message; no semantic model stores red/green nodes.
- `ExpectedSyntax` is closed over the currently implemented grammar only; classifier Operations adds no variant in this plan.

- [ ] **Step 1: Write failing attribute syntax/recovery tests**

Cover complete, missing colon, missing type, malformed/open multiplicity, stray tokens, tabs/spaces/CRLF/Unicode, and an unclaimed Concept with an `## Attributes` heading. Assert typed slots, exact writing, diagnostics, declared field states, and that the unclaimed Concept has no UML syntax snapshot.

Add one table-driven recognizer/analysis acceptance test with exact rows:

```rust
[
    ("uml.Class", "classifier"), ("uml.Interface", "classifier"),
    ("uml.Enum", "classifier"), ("uml.DataType", "classifier"),
    ("uml.Package", "concept"), ("uml.Note", "concept"),
    ("uml.Association", "classifier"), ("uml.Actor", "classifier"),
    ("uml.UseCase", "classifier"), ("uml.InstanceSpecification", "concept"),
    ("uml.Activity", "behavior"), ("uml.StateMachine", "behavior"),
    ("uml.Sequence", "behavior"), ("Diagram", "diagram"),
]
```

For every row assert `recognizes == true`, exactly one claim, a syntax snapshot, and the expected validated projection category. A companion table `["", "vendor.Widget", "uml.FutureThing", "diagram"]` asserts zero claims and Generic OKF fallback.

Run:

```powershell
rtk cargo test -p waml --test uml_attribute_syntax
```

Expected: FAIL because `UmlLanguage`, `AttributeSyntax`, `ExpectedSyntax`, and `uml::Analysis` do not exist.

- [ ] **Step 2: Implement UML kinds, fixed slots, and the attribute island parser**

Use shell-confirmed H2 and explicit bullet/indent grammar. Reuse raw shell regions outside the claimed island, insert expected-kind missing tokens, and make parser progress by consuming bad or skipped input.

- [ ] **Step 3: Build declared attributes and validated lowering**

Create a `DeclaredAttribute` whenever the production is recognized. Add it to the validated projection only when required fields are valid and optional fields are absent/valid. Keep invalid authored bytes and located diagnostics accessible.

- [ ] **Step 4: Implement the initial sibling analysis**

Claim supported Concepts from `context.okf`, create syntax/declared/projection once, use the exact catalog/document-version Arcs, and return local candidate state. Unknown/missing/arbitrary/unknown-`uml.*`, Index, Log, and Directory never enter claims.

- [ ] **Step 5: Run differential and commit**

```powershell
rtk cargo test -p waml --test uml_attribute_syntax
rtk cargo test -p waml uml::tests
rtk cargo test -p waml parser_platform_baseline
rtk git add crates/waml/src/uml.rs crates/waml/src/uml crates/waml/src/model.rs crates/waml/src/diagnostic.rs crates/waml/tests/uml_attribute_syntax.rs
rtk git commit -m "feat: add tolerant UML attribute analysis"
```

### Task 7: Complete Classifier, Value, Slot, and Relationship Grammar

**Files:**
- Modify: `crates/waml/src/uml/syntax/kind.rs`
- Modify: `crates/waml/src/uml/syntax/ast.rs`
- Modify: `crates/waml/src/uml/syntax/parser.rs`
- Modify: `crates/waml/src/uml/declared.rs`
- Modify: `crates/waml/src/uml/analysis.rs`
- Modify: `crates/waml/src/model.rs`
- Create: `crates/waml/tests/uml_classifier_syntax.rs`
- Create: `crates/waml/tests/fixtures/parser-platform/recovery/classifier.md`

**Interfaces:**
- Produces checked wrappers `ValueSyntax`, `SlotSyntax`, `RelationshipSyntax`, `MemberSyntax`, `MemberGroupSyntax`, and `InlineInstanceSyntax`.
- Produces `DeclaredValue`, `DeclaredSlot`, `DeclaredRelationship`, `DeclaredMember`, and `DeclaredInlineInstance`, each using `DeclaredField` per authored field.
- Implements the complete currently claimed classifier/object grammar from `grammar.rs`: Attributes, Values, Slots, Relationships, Members, grouped members, and inline instances. Classifier `## Operations` remains an ordinary embedded Markdown region.
- Required tokens occupy fixed slots; list separators and recovery occupy dedicated slots. Accessors never search descendants.

- [ ] **Step 1: Write failing per-production syntax tests**

For every production, include one valid line, every required-token omission, invalid-present field, unexpected trailing token, malformed link, nested indentation error, CRLF, and Unicode identifier. Assert exact leaf sequence, fixed accessor results, declared state, located diagnostics, and validated projection inclusion/exclusion.

Run `rtk cargo test -p waml --test uml_classifier_syntax`.

Expected: FAIL because the classifier/value/slot/relationship wrappers and declared forms are incomplete.

- [ ] **Step 2: Implement Values and Slots**

Port the exact accepted forms from legacy `parse_value_line`, `classify_slot_value`, and `parse_slot_line`. Preserve raw spelling in tokens while declared values normalize only where the current semantic model already does.

- [ ] **Step 3: Implement Relationships**

Parse kind/name/end multiplicities/link target into distinct slots. Resolve targets against the complete claimed-concept index for the immutable UML analysis; a target that is Generic OKF or invalid remains authored declared syntax plus a located diagnostic and is absent from validated edges.

- [ ] **Step 4: Implement Members, groups, and inline instances**

Use shell-confirmed H3-H6 plus explicit indentation/bullet rules. Malformed nested content becomes skipped/bad syntax under the owning member recovery slot and cannot shift typed slots.

- [ ] **Step 5: Compare projection and canonical source with the baseline**

For all valid Task 1 classifier fixtures, assert new validated UML equals the old semantic projection and the future formatter input contains equivalent declared values. For malformed fixtures, assert new syntax adds information without losing legacy diagnostics.

- [ ] **Step 6: Run and commit**

```powershell
rtk cargo test -p waml --test uml_classifier_syntax
rtk cargo test -p waml parser_platform_baseline
rtk git add crates/waml/src/uml crates/waml/src/model.rs crates/waml/tests/uml_classifier_syntax.rs crates/waml/tests/fixtures/parser-platform/recovery/classifier.md
rtk git commit -m "feat: complete UML classifier syntax"
```

### Task 8: Complete Diagram Membership and Layout Grammar

**Files:**
- Modify: `crates/waml/src/uml/syntax/kind.rs`
- Modify: `crates/waml/src/uml/syntax/ast.rs`
- Modify: `crates/waml/src/uml/syntax/parser.rs`
- Modify: `crates/waml/src/uml/declared.rs`
- Modify: `crates/waml/src/uml/analysis.rs`
- Modify: `crates/waml/src/layout.rs`
- Modify: `crates/waml/src/model.rs`
- Create: `crates/waml/tests/uml_diagram_syntax.rs`
- Create: `crates/waml/tests/fixtures/parser-platform/recovery/diagram.md`

**Interfaces:**
- Produces `DiagramMembersSyntax`, `MemberLineSyntax`, `LayoutSectionSyntax`, and one typed wrapper for every current `LayoutStatement` variant:

```rust
pub enum DeclaredLayoutStatement {
    Placement { operands: Arc<[DeclaredOperand]>, directions: Arc<[DeclaredField<UmlLanguage, Direction>]> },
    Alignment { left: DeclaredField<UmlLanguage, Anchored>, right: DeclaredField<UmlLanguage, Anchored> },
    Standalone(DeclaredField<UmlLanguage, Operand>),
}
```

- The parser covers every nested current layout value used by those three variants: `Anchored`, `Edge`, `Operand`, `Axis`, `OperandRef`, `NameRef`, `Hint`, `Shape`, `Margin`, and `Flag`. No fourth layout-statement variant is implied.
- Diagram membership and layout resolve only claimed Concepts. Physical OKF directories never become UML packages; explicitly authored `uml.Package` remains an ordinary claimed Concept.
- Unknown render hints remain exact recovery/raw syntax and do not gain semantic meaning.

- [ ] **Step 1: Write failing layout token/slot tests**

Cover every accepted direction/operator/operand/flag from `layout.rs`, malformed references, missing operands, duplicate/trailing tokens, unknown hints, indentation, CRLF, and Generic OKF members. Assert exact source, declared field state, ranges, and validated placement.

Run `rtk cargo test -p waml --test uml_diagram_syntax`.

Expected: FAIL because all three `DeclaredLayoutStatement` variants and nested typed values are not implemented.

- [ ] **Step 2: Parse diagram members from the shared structure map**

Recognize only claimed diagram Concepts and their confirmed Members section. Keep unresolved/unclaimed links declared and diagnosed; do not synthesize projection nodes.

- [ ] **Step 3: Parse the complete current layout grammar**

Factor semantic conversion helpers out of `layout.rs` so token parsing lives in UML syntax and semantic value construction remains domain code. A parser failure consumes at least one non-empty bad/skipped token.

- [ ] **Step 4: Build declared and validated diagram projections**

Populate existing diagram/group/layout model values only from valid declared fields. Preserve the current diagnostics and serializer behavior for valid fixtures.

- [ ] **Step 5: Run and commit**

```powershell
rtk cargo test -p waml --test uml_diagram_syntax
rtk cargo test -p waml layout::tests
rtk cargo test -p waml parser_platform_baseline
rtk git add crates/waml/src/uml crates/waml/src/layout.rs crates/waml/src/model.rs crates/waml/tests/uml_diagram_syntax.rs crates/waml/tests/fixtures/parser-platform/recovery/diagram.md
rtk git commit -m "feat: complete UML diagram syntax"
```

### Task 9: Complete Flow, State, and Sequence Grammar

**Files:**
- Modify: `crates/waml/src/uml/syntax/kind.rs`
- Modify: `crates/waml/src/uml/syntax/ast.rs`
- Modify: `crates/waml/src/uml/syntax/parser.rs`
- Modify: `crates/waml/src/uml/declared.rs`
- Modify: `crates/waml/src/uml/analysis.rs`
- Modify: `crates/waml/src/model.rs`
- Create: `crates/waml/tests/uml_behavior_syntax.rs`
- Create: `crates/waml/tests/fixtures/parser-platform/recovery/flow.md`
- Create: `crates/waml/tests/fixtures/parser-platform/recovery/sequence.md`

**Interfaces:**
- Produces `FlowNodeSyntax`, `FlowTransitionSyntax`, `FlowBlockSyntax`, `LifelineSyntax`, `MessageSyntax`, `SequenceOperandSyntax`, and `MessagesBlockSyntax`.
- Implements current claimed activity/state/sequence forms from legacy `parse_flow_*`, `parse_lifeline_line`, `parse_message_line`, and `parse_messages_block`.
- Deferred `par`, self/found/lost messages, gates, and coregions remain navigable recovery/raw syntax with located unsupported-form diagnostics; they are not silently accepted or discarded.

- [ ] **Step 1: Write failing flow/state grammar tests**

Cover headings with linked targets, all current node kinds, notes, transitions, nested flow blocks, invalid bullets, missing links, malformed indentation, and recovery synchronization at the next confirmed heading.

Run `rtk cargo test -p waml --test uml_behavior_syntax`.

Expected: FAIL because flow/state/sequence typed syntax and recovery are incomplete.

- [ ] **Step 2: Implement flow/state parsing and projections**

Use the shared H3-H6 structure map and explicit indentation. Resolve links against the current immutable claimed index, distinguish invalid-present from absent, and retain all unprojected bytes.

- [ ] **Step 3: Write failing sequence grammar tests**

Cover lifelines, directed messages, current operands/guards, nesting, missing arrows/targets, malformed blocks, CRLF, Unicode, and each explicitly deferred sequence form.

- [ ] **Step 4: Implement sequence parsing and projections**

Build typed nested slots and recovery children without a recursive parser stall. Lower only currently supported valid declared messages/operands into `Projection`.

- [ ] **Step 5: Run complete claimed-grammar differential gate**

```powershell
rtk cargo test -p waml --test uml_behavior_syntax
rtk cargo test -p waml --test uml_classifier_syntax
rtk cargo test -p waml --test uml_diagram_syntax
rtk cargo test -p waml parser_platform_baseline
```

Expected: PASS; every currently claimed grammar family has typed/recovery coverage and `## Operations` remains raw.

- [ ] **Step 6: Commit**

```powershell
rtk git add crates/waml/src/uml crates/waml/src/model.rs crates/waml/tests/uml_behavior_syntax.rs crates/waml/tests/fixtures/parser-platform/recovery
rtk git commit -m "feat: complete UML behavior syntax"
```

### Task 10: Prove Static Sibling Composition and Generic OKF Boundaries

**Files:**
- Modify: `crates/waml/src/analysis.rs`
- Modify: `crates/waml/src/uml/analysis.rs`
- Create: `crates/waml/tests/specialization_composition.rs`
- Modify: `crates/waml-editor/src/documents.rs` inline tests
- Modify: `crates/waml-editor/src/document_host.rs` inline tests

**Interfaces:**
- Produces:

```rust
pub fn validate_disjoint_claims<'a>(
    claims: impl IntoIterator<Item = (&'a str, &'a ClaimSet)>
) -> Result<(), AnalysisError>;
pub struct PreviousAnalyses<'a> {
    pub okf: &'a OkfAnalysis,
    pub uml: &'a uml::Analysis,
}
pub struct PreparedCandidate {
    source: SourceBundle,
    okf: OkfAnalysis,
    uml: uml::Analysis,
    revision: u64,
}
pub fn prepare_candidate(
    candidate_source: SourceBundle,
    previous: Option<PreviousAnalyses<'_>>,
    candidate_revision: u64,
) -> Result<PreparedCandidate, AnalysisError>;
impl PreparedCandidate {
    pub fn source(&self) -> &SourceBundle;
    pub fn okf(&self) -> &OkfAnalysis;
    pub fn uml(&self) -> &uml::Analysis;
    pub fn revision(&self) -> u64;
    pub fn into_parts(self) -> (SourceBundle, OkfAnalysis, uml::Analysis, u64);
}
```

- The product composition root calls `uml::analyze` explicitly, then validates `[("uml", &uml.claims)]`. There is no runtime registry, trait object, discovery, priority, plugin lifecycle, or central enum containing domain analyses.
- `prepare_candidate` is the one pure, non-global candidate-preparation boundary shared by native session, one-shot CLI, and LSP. It consumes an already validated/lowered candidate source, derives OKF once, constructs one `DomainAnalysisContext`, calls each statically linked specialization exactly once, validates disjoint claims, and returns owned candidate state without mutating an owner.
- `PreviousAnalyses` is reuse input only; it owns no source, clock, dirty state, or lifecycle. `candidate_revision` is supplied by the caller's existing owner. A failure returns `AnalysisError` and leaves caller state/IDs/revisions untouched.
- Implement production `prepare_candidate` as a call to private `prepare_candidate_inner(candidate, previous, revision, hooks: &mut impl PreparationHooks)` with `NoopPreparationHooks`. Static product claims are assembled explicitly inside that function and passed to public `validate_disjoint_claims`; there is no injected analyzer/claim list in production.
- A test-only `FutureLanguage`, `FutureSyntaxKind`, `FutureDeclared`, and `FutureAnalysis` claim `future.Widget` through a sibling `analyze_future(context)` function defined wholly in the integration test.

- [ ] **Step 1: Write the future-sibling acceptance test**

Construct one bundle containing supported UML, `future.Widget`, arbitrary, missing type, unknown `uml.*`, Index, and Log. The test-only analyzer consumes `DomainAnalysisContext`, shares the exact catalog/document versions, produces its own syntax/declared projection, and requires no edit to OKF types, shell kinds, UML kinds, or editor document-family state.

Run `rtk cargo test -p waml --test specialization_composition`.

Expected: FAIL because disjoint claim validation and the shared prepared-candidate composition boundary do not exist.

- [ ] **Step 2: Implement disjoint claim validation**

Return `AnalysisError::AmbiguousClaim { concept_id, first, second }` in deterministic concept/analyzer order. Generic OKF is a fallback for zero claims and is not itself a claim set.

- [ ] **Step 3: Add ambiguity and boundary tests**

Make test UML and future recognizers both claim one Concept and assert candidate analysis fails before any session installation. Assert Index/Log never reach either recognizer, structural directories never become UML packages, and unclaimed Concepts expose no UML typed accessors.

- [ ] **Step 4: Reconfirm editor neutrality**

Keep `documents::open` explicit UML-first then Generic-OKF fallback. Add a test-only descriptor for the sibling outside `DocumentHost` and prove the host accepts prepared `OpenDocument` without family dispatch or new enum variants.

- [ ] **Step 5: Test the shared pure preparation boundary**

In `analysis.rs`'s inline test module, use the module-private `PreparationHooks` implementation to count Shell/OKF/UML/Claims phases and inject `StructuralInvariant` before each phase. Assert `prepare_candidate_inner` runs each successful phase once and leaves the previous candidate untouched on injected failure. Test claim ambiguity separately by passing UML plus the test sibling `ClaimSet` to public `validate_disjoint_claims`. Recoverable malformed syntax must return `Ok(PreparedCandidate)` with diagnostics.

Run:

```powershell
rtk cargo test -p waml analysis::tests::prepare_candidate_runs_static_phases_once
rtk cargo test -p waml analysis::tests::prepare_candidate_failure_is_non_mutating
rtk cargo test -p waml --test specialization_composition
```

Expected: PASS; the integration test uses only public context/claim APIs and no private counter/failpoint.

- [ ] **Step 6: Run and commit**

```powershell
rtk cargo test -p waml --test specialization_composition
rtk cargo test -p waml-editor documents::tests
rtk cargo test -p waml-editor document_host::tests
rtk git add crates/waml/src/analysis.rs crates/waml/src/uml/analysis.rs crates/waml/tests/specialization_composition.rs crates/waml-editor/src/documents.rs crates/waml-editor/src/document_host.rs
rtk git commit -m "test: prove static specialization composition"
```

### Task 11: Add Revision-Bound Syntax Edits and Code Actions

**Files:**
- Create: `crates/waml/src/action.rs`
- Create: `crates/waml/tests/syntax_actions.rs`
- Modify: `crates/waml/src/lib.rs`
- Modify: `crates/waml/src/edit.rs`
- Modify: `crates/waml/src/source.rs`
- Modify: `crates/waml/src/analysis.rs`
- Modify: `crates/waml/src/uml/analysis.rs`

**Interfaces:**
- Produces:

```rust
pub struct TextEdit { pub range: TextRange, pub replacement: Arc<str> }
pub struct VersionedDocumentChange {
    pub document: DocumentId,
    pub base_document_revision: DocumentRevision,
    pub edits: Arc<[TextEdit]>,
}
pub enum ActionBasis {
    Document { document: DocumentId, document_revision: DocumentRevision, session_revision: u64 },
    Bundle { session_revision: u64 },
}
pub struct CodeAction {
    pub title: String,
    pub basis: ActionBasis,
    pub changes: Arc<[VersionedDocumentChange]>,
}
pub struct VersionedSyntaxLocator<L: SyntaxLanguage> {
    document: DocumentId,
    document_revision: DocumentRevision,
    session_revision: u64,
    locator: SyntaxLocator<L>,
}
impl<L: SyntaxLanguage> VersionedSyntaxLocator<L> {
    pub fn for_node(
        document: DocumentId,
        document_revision: DocumentRevision,
        session_revision: u64,
        node: &SyntaxNode<L>,
    ) -> Self;
    pub fn for_token(
        document: DocumentId,
        document_revision: DocumentRevision,
        session_revision: u64,
        token: &SyntaxToken<L>,
    ) -> Self;
    pub fn document(&self) -> DocumentId;
    pub fn document_revision(&self) -> DocumentRevision;
    pub fn session_revision(&self) -> u64;
    pub fn locator(&self) -> &SyntaxLocator<L>;
    pub fn resolve_in(
        &self, tree: &SyntaxTree<L>
    ) -> Result<SyntaxElement<L>, RewriteError<L::Kind>>;
}
pub struct SyntaxChangeBatch { action: CodeAction }
impl SyntaxChangeBatch { pub fn new(action: CodeAction) -> Result<Self, ActionError>; }
pub enum ActionError {
    UnknownDocument { document: DocumentId },
    StaleSession { expected: u64, actual: u64 },
    StaleDocument { document: DocumentId, expected: DocumentRevision, actual: DocumentRevision },
    DifferentTree { document: DocumentId },
    InvalidRange { document: DocumentId, range: TextRange },
    NonUtf8Boundary { document: DocumentId, offset: TextSize },
    Overlap { document: DocumentId, first: TextRange, second: TextRange },
    BasisScope { document: DocumentId },
    MismatchedCatalog,
    MismatchedAnalysisRevision { catalog: u64, requested: u64 },
    StructuralInvariant { reason: Arc<str> },
}
impl From<ActionError> for EditError {
    fn from(error: ActionError) -> Self {
        EditError { index: 0, op: "syntax.action".into(), selector: None, reason: error.to_string() }
    }
}
impl From<AnalysisError> for EditError {
    fn from(error: AnalysisError) -> Self {
        EditError { index: 0, op: "analysis.prepare".into(), selector: None, reason: error.to_string() }
    }
}
```

- Parser-era `EditContext` becomes:

```rust
pub struct EditContext<'a> {
    pub source: &'a SourceBundle,
    pub okf_analysis: &'a OkfAnalysis,
    pub session_revision: u64,
    pub uml: &'a uml::Analysis,
}
```

- `VersionedSyntaxLocator` has private fields and no generic public constructor: `for_node`/`for_token` copy the exact occurrence locator through Task 3's public `locator()` API, while its accessors are read-only. `resolve_in` delegates to `SyntaxTree::resolve` and preserves exact `RewriteError::WrongTree`, `InvalidPath`, or `KindMismatch`.
- A versioned locator is producer-side targeting metadata, not serialized inside `CodeAction`. Formatter/repair/action producers construct it from the typed red occurrence, validate its document/session revisions against the current catalog, call `resolve_in` against that document's exact syntax tree, and only then derive the `TextEdit`. A wrong-tree result becomes `ActionError::DifferentTree { document }`; other locator invariant failures become `StructuralInvariant` with the original `RewriteError` display text.
- `SyntaxChangeBatch: EditBatch` validates action basis, IDs, revisions, UTF-8 boundaries, and sorted non-overlap; it copy-on-write edits only touched documents and returns a candidate `SourceBundle`.
- Document-basis actions may touch only that document. Cross-document resolution uses bundle basis even for one output document.
- Error conversion preserves the source variant in `EditError.reason`; `op` is exactly `"syntax.action"` for `ActionError` and `"analysis.prepare"` for `AnalysisError`, `index` is zero until a domain batch supplies a step index, and no conversion treats recovery diagnostics as errors.
- `ActionError: Display + std::error::Error`; retained `EditError` remains the sealed Lowerer/session error and implements the same traits.

- [ ] **Step 1: Write constructor validation tests**

Test reversed/out-of-bounds/non-UTF-8 ranges, overlaps, duplicate document changes, non-ascending edit input, document-basis cross-document changes, and an empty no-op action. Require ascending non-overlapping storage, then apply from highest range to lowest.

Run `rtk cargo test -p waml --test syntax_actions`.

Expected: FAIL because versioned action values, `ActionError`, and `SyntaxChangeBatch` do not exist.

- [ ] **Step 2: Implement value validation and source lowering**

Resolve `DocumentId -> BundlePath` only through the current catalog; clone the bundle once; call crate-private `SourceDocument::text_mut` once per touched document; never mutate the catalog or syntax set.

- [ ] **Step 3: Write stale-action and locator tests**

Generate actions at revision N, then change the same document, another document, and the bundle. Because both basis variants carry session revision, assert every session advance rejects the old action; also assert a same-session document-revision mismatch is rejected. Construct `VersionedSyntaxLocator` values only with `for_node`/`for_token`, assert their read-only accessors match the catalog and occurrence locator, and resolve them in the original tree. Rebuild a different tree over the same green root and assert `resolve_in` returns exact `RewriteError::WrongTree` even when kind/range/green identity match; assert the action producer maps it to `ActionError::DifferentTree` without producing a `CodeAction`.

- [ ] **Step 4: Prove atomic source behavior**

For two-document changes, make the second range invalid and assert the returned error leaves the original bundle and all `Arc<String>` identities unchanged. On success, touched documents detach once and untouched documents remain shared.

- [ ] **Step 5: Run and commit**

```powershell
rtk cargo test -p waml --test syntax_actions
rtk cargo test -p waml edit::tests
rtk git add crates/waml/src/action.rs crates/waml/src/lib.rs crates/waml/src/edit.rs crates/waml/src/source.rs crates/waml/src/analysis.rs crates/waml/src/uml/analysis.rs crates/waml/tests/syntax_actions.rs
rtk git commit -m "feat: lower revision bound syntax actions"
```

### Task 12: Make EditorSession Prepare and Commit Every Analysis Atomically

**Files:**
- Modify: `crates/waml-editor/src/editor_session.rs`
- Modify: `crates/waml-editor/src/load.rs`
- Modify: `crates/waml-editor/src/doc_view.rs`
- Modify: `crates/waml-editor/src/app.rs`
- Modify: `crates/waml-editor/src/app/actions.rs`
- Modify: `crates/waml/src/edit.rs`
- Create: `crates/waml-editor/tests/fixtures/parser-actions/`

**Interfaces:**
- `EditorSession` fields become:

```rust
pub struct EditorSession {
    source: SourceBundle,
    persisted_source: SourceBundle,
    okf_analysis: OkfAnalysis,
    uml: uml::Analysis,
    revision: u64,
    dirty_revision: Option<u64>,
}
```

- `EditorSession::replace(&mut self, source: SourceBundle) -> Result<SessionChange, EditError>` analyzes and atomically installs a clean replacement; callers no longer supply a separately built projection.
- `EditorSession::apply<B: EditBatch>(&mut self, batch: B) -> Result<SessionChange, EditError>` lowers once, calls shared `waml::analysis::prepare_candidate(candidate_source, previous, next_revision)` once, then assigns all fields and revision/dirty state.
- Accessors are `source`, `persisted_bundle`, `okf_analysis`, `okf`, `uml_analysis`, `uml_projection`, `revision`, and `is_dirty`. There is no public install-syntax/install-analysis path.
- Factor the assignment-free body through private `apply_with_preparer<B, F>(&mut self, batch: B, prepare: F)` where `F: for<'a> FnOnce(SourceBundle, Option<PreviousAnalyses<'a>>, u64) -> Result<PreparedCandidate, AnalysisError>`. Production `apply` passes `prepare_candidate`; inline editor tests pass a closure that returns each `StructuralInvariant` stage. This helper is private and creates no alternative product mutation API.

- [ ] **Step 1: Rewrite session tests around complete snapshots**

Capture source, persisted source, shell/catalog/tree IDs, OKF, UML syntax/declared/projection/claims/diagnostics, revision, dirty revision, and all document allocation identities. Inject test-only builders that count OKF and UML analysis invocations and can fail after each preparation phase.

Run `rtk cargo test -p waml-editor editor_session::tests`.

Expected: FAIL because `EditorSession` does not own `OkfAnalysis`/`uml::Analysis` or call shared `prepare_candidate`.

- [ ] **Step 2: Implement private prepare and atomic replace**

Compute `next_revision` locally and call `prepare_candidate` with `PreviousAnalyses { okf: &self.okf_analysis, uml: &self.uml }`. Destructure `PreparedCandidate` only after success and assign no `self` field before it is complete. `replace` passes `None`; CLI and LSP use the same boundary in Tasks 18 and 19.

- [ ] **Step 3: Extend apply and stale-action coverage**

Apply an OKF batch, UML batch, syntax code action, and sealed compatibility batch. Each successful batch lowers/analyzes/projects once and bumps once. Fail lowering, shell/OKF analysis, UML analysis, ambiguity, and stale validation and assert every captured value/identity is unchanged.

Also apply malformed-but-recoverable source and assert the transaction succeeds atomically, advances once, and exposes the new recovery diagnostic instead of treating diagnostics as transaction failure.

- [ ] **Step 4: Preserve save/allocation bounds**

Clean replace shares current/persisted/syntax/semantic source. One unsaved touched document permits exactly current and persisted whole-source allocations for that path; untouched documents share all views. Saving the current revision collapses back to one; saving an old revision changes nothing.

- [ ] **Step 5: Route editor outcomes only through apply**

`ViewOutcome` continues returning `PendingEdit`; syntax diagnostics/code actions wrap `SyntaxChangeBatch` in `PendingEdit`. Remove any editor-side projection construction or candidate syntax installation.

- [ ] **Step 6: Run and commit**

```powershell
rtk cargo test -p waml-editor editor_session::tests
rtk cargo test -p waml-editor app::actions::tests
rtk cargo test -p waml --test syntax_actions
rtk git add crates/waml-editor/src/editor_session.rs crates/waml-editor/src/load.rs crates/waml-editor/src/doc_view.rs crates/waml-editor/src/app.rs crates/waml-editor/src/app/actions.rs crates/waml-editor/tests/fixtures/parser-actions crates/waml/src/edit.rs
rtk git commit -m "refactor: atomically install parser analyses"
```

### Task 13: Produce Revision-Bound Formatting and Targeted UML Repairs

**Files:**
- Create: `crates/waml/src/uml/format.rs`
- Create: `crates/waml/src/uml/repair.rs`
- Create: `crates/waml/tests/formatter_actions.rs`
- Create: `crates/waml/tests/uml_repair_actions.rs`
- Modify: `crates/waml/src/uml.rs`
- Modify: `crates/waml/src/serialize.rs` inline differential tests

**Interfaces:**
- Produces:

```rust
pub struct Formatter;
pub struct ActionContext<'a> {
    okf: &'a OkfAnalysis,
    uml: &'a uml::Analysis,
    session_revision: u64,
}
impl<'a> ActionContext<'a> {
    pub fn new(
        okf: &'a OkfAnalysis,
        uml: &'a uml::Analysis,
        session_revision: u64,
    ) -> Result<Self, ActionError>;
    pub fn from_prepared(candidate: &'a PreparedCandidate) -> Result<Self, ActionError>;
    pub fn okf(&self) -> &'a OkfAnalysis;
    pub fn uml(&self) -> &'a uml::Analysis;
    pub fn session_revision(&self) -> u64;
}
impl Formatter {
    pub fn format(
        &self, context: ActionContext<'_>, document: DocumentId
    ) -> Result<CodeAction, FormatError>;
}
pub enum FormatError {
    Action(ActionError),
    UnknownDocument { document: DocumentId },
    NotClaimed { document: DocumentId },
    StructuralInvariant { reason: Arc<str> },
}
impl From<ActionError> for FormatError {
    fn from(error: ActionError) -> Self { Self::Action(error) }
}
impl From<FormatError> for EditError {
    fn from(error: FormatError) -> Self {
        EditError { index: 0, op: "uml.format".into(), selector: None, reason: error.to_string() }
    }
}
pub fn repair_actions(
    context: ActionContext<'_>, document: DocumentId
) -> Result<Vec<CodeAction>, ActionError>;
```

- Exact tree writing remains `snapshot.syntax().write_to_string()` and never canonicalizes.
- Formatter output is a revision-bound `CodeAction`, preserves current broad canonical behavior for claimed valid syntax, is idempotent, and leaves raw Generic OKF Markdown plus malformed recovery bytes untouched unless an explicit repair owns the range.
- `ActionContext` is read-only and cannot be assembled with public fields. `new` requires `Arc::ptr_eq(&okf.catalog, uml.syntax.catalog())`, requires both catalog accessors to report `session_revision`, and otherwise returns `MismatchedCatalog` or `MismatchedAnalysisRevision`. Only after validation do its accessors expose references; formatter and repairs never apply source.
- `FormatError: Display + std::error::Error`; conversion to `EditError` uses the stable `"uml.format"` operation tag shown above.
- `repair_actions` is UML-owned and produces exactly: insert `": "` at a missing `ColonToken`; insert the canonical placeholder type `String` at missing type; and replace an invalid-present multiplicity with `{n}` when the invalid slot contains a first ASCII-decimal integer `n`, otherwise `{1}`. Each action targets the diagnostic's typed slot and carries `ActionBasis::Document`.

- [ ] **Step 1: Write formatter differential/idempotence tests**

For every valid Task 1 fixture compare formatter action applied to source with current `serialize_document` canonical output. Format twice and assert the second action has no edits. For malformed and Generic OKF fixtures assert recovery/raw ranges are byte-identical.

Run:

```powershell
rtk cargo test -p waml --test formatter_actions
```

Expected: FAIL because `ActionContext`, revision-bound `Formatter::format`, and `FormatError` do not exist.

- [ ] **Step 2: Implement canonical token/section formatting**

Encode current heading order, blank lines, bullets, indentation, links, multiplicity, layout, flow, and sequence rendering in `uml/format.rs`. Produce minimal ordered text edits over owned valid regions while preserving all regions outside formatter ownership.

- [ ] **Step 3: Write and run targeted repair action tests**

For missing colon, missing type, and invalid multiplicity, assert exact title, `ActionBasis`, document revision, byte edit, fixed source, reanalysis diagnostic removal, and stale rejection after same/other-document session changes.

Construct two prepared candidates at the same numeric revision and assert mixing their OKF/UML analyses returns `MismatchedCatalog`. Use one candidate with a different requested revision and assert `MismatchedAnalysisRevision`. Assert `from_prepared` succeeds and every produced action uses that exact revision/catalog.

Run:

```powershell
rtk cargo test -p waml --test uml_repair_actions
```

Expected: FAIL because `repair_actions` does not exist.

- [ ] **Step 4: Implement UML-owned targeted repairs**

Locate only typed missing/invalid slots in the requested claimed document. Do not offer a repair for bad/skipped content with no unambiguous slot. Return actions only; application remains `SyntaxChangeBatch -> EditorSession::apply`.

- [ ] **Step 5: Run formatter/repair gates**

```powershell
rtk cargo test -p waml --test formatter_actions
rtk cargo test -p waml --test uml_repair_actions
rtk cargo test -p waml serialize::tests
```

Expected: PASS; canonical bytes match the baseline, raw/recovery source stays untouched, and all actions carry the current session/document revisions.

- [ ] **Step 6: Commit**

```powershell
rtk git add crates/waml/src/uml/format.rs crates/waml/src/uml/repair.rs crates/waml/src/uml.rs crates/waml/src/serialize.rs crates/waml/tests/formatter_actions.rs crates/waml/tests/uml_repair_actions.rs
rtk git commit -m "feat: produce revision bound UML actions"
```

### Task 14: Move the OKF Lowerer onto Cumulative Shell State

**Files:**
- Modify: `crates/waml/src/okf/lower.rs`
- Modify: `crates/waml/src/okf/ops.rs`
- Modify: `crates/waml/src/index_md.rs`
- Create: `crates/waml/tests/okf_lowering_order.rs`
- Modify: `crates/waml/tests/ops_golden.rs`

**Interfaces:**
- Produces the crate-private ordered cursor:

```rust
pub(crate) struct OkfLoweringCursor<'a> {
    original: EditContext<'a>,
    candidate: SourceBundle,
    state: OkfLoweringState,
}
pub(crate) struct OkfLoweringState {
    touched_shell: BTreeMap<BundlePath, ShellParse>,
    structural_paths: BTreeSet<BundlePath>,
}
impl OkfLoweringState {
    pub(crate) fn from_context(context: &EditContext<'_>) -> Self;
    pub(crate) fn invalidate_text(&mut self, path: &BundlePath);
    pub(crate) fn inserted(&mut self, path: BundlePath) -> Result<(), EditError>;
    pub(crate) fn removed(&mut self, path: &BundlePath);
    pub(crate) fn renamed(&mut self, from: &BundlePath, to: BundlePath) -> Result<(), EditError>;
}
impl<'a> OkfLoweringCursor<'a> {
    pub(crate) fn new(context: EditContext<'a>) -> Self;
    pub(crate) fn apply(&mut self, index: usize, op: &okf::Op) -> Result<(), EditError>;
    pub(crate) fn finish(self) -> SourceBundle;
}
pub(crate) fn apply_step(
    candidate: &mut SourceBundle,
    state: &mut OkfLoweringState,
    index: usize,
    op: &okf::Op,
) -> Result<(), EditError>;
```

- Every step reads the cumulative candidate, not `original.source`. After a text edit, the cursor reparses that touched document's shell before the next step; after add/remove/rename, it updates `structural_paths` and invalidates affected shell entries. It does not construct a complete `okf::Bundle` or any specialization analysis; shared `prepare_candidate` does that once after the batch.
- `OkfLoweringCursor::new` clones `context.source` once into `candidate` and initializes `state` with `OkfLoweringState::from_context(&context)` before storing `original`; `finish` returns that candidate without semantic preparation.
- `original` seeds revision validation and initial derived state only. Once a path/document is touched, no later step may query its source or shell through `original`.
- `apply_step` performs normalized/collision-free `SourceBundle` mutation before calling these state methods. `inserted`/`renamed` reject duplicate entries in their derived state without consulting the original bundle; `removed` clears cached shell; `invalidate_text` drops only the affected parse so the next access reparses cumulative source.

- [ ] **Step 1: Write and run ordered OKF regression tests**

Test import-then-retitle (add-then-set), directory-rename-then-retitle (rename-then-edit), two edits to one synthesized Index, and a late collision rollback. Assert the second operation observes the first operation's path/text and untouched `Arc<String>` identities remain shared.

Run:

```powershell
rtk cargo test -p waml --test okf_lowering_order
```

Expected: FAIL because the existing Lowerer reads stale initial semantic/syntax state for the second operation.

- [ ] **Step 2: Implement cumulative structural/source state**

Move shell/index mutation behind `OkfLoweringCursor::apply`. Reparse only touched Index/frontmatter documents and maintain a candidate path set for move/collision decisions.

- [ ] **Step 3: Route `okf::Batch` through the cursor**

Create one cursor, enumerate operations with stable error indices, stop on first error, and return only `finish()` after all steps succeed. Never mutate input context.

- [ ] **Step 4: Run and commit**

```powershell
rtk cargo test -p waml --test okf_lowering_order
rtk cargo test -p waml okf::ops::tests
rtk cargo test -p waml --test ops_golden
rtk git add crates/waml/src/okf/lower.rs crates/waml/src/okf/ops.rs crates/waml/src/index_md.rs crates/waml/tests/okf_lowering_order.rs crates/waml/tests/ops_golden.rs
rtk git commit -m "refactor: lower ordered OKF edits cumulatively"
```

### Task 15: Move the UML Lowerer onto Cumulative Typed Islands

**Files:**
- Modify: `crates/waml/src/uml/lower.rs`
- Modify: `crates/waml/src/uml/ops.rs`
- Modify: `crates/waml/src/uml/rename.rs`
- Modify: `crates/waml/src/uml/selector.rs`
- Create: `crates/waml/tests/uml_lowering_order.rs`
- Modify: `crates/waml/tests/ops_golden.rs`

**Interfaces:**
- Produces:

```rust
pub(crate) struct UmlLoweringCursor<'a> {
    original: EditContext<'a>,
    candidate: SourceBundle,
    state: UmlLoweringState,
}
pub(crate) struct UmlLoweringState {
    current_paths: BTreeMap<String, BundlePath>,
    touched_islands: BTreeMap<BundlePath, Arc<SyntaxTree<UmlLanguage>>>,
}
impl UmlLoweringState {
    pub(crate) fn from_context(context: &EditContext<'_>) -> Self;
    pub(crate) fn invalidate_text(&mut self, path: &BundlePath);
    pub(crate) fn inserted_concept(&mut self, id: String, path: BundlePath) -> Result<(), EditError>;
    pub(crate) fn removed_concept(&mut self, id: &str);
    pub(crate) fn renamed_concept(
        &mut self, from: &str, to: String, path: BundlePath
    ) -> Result<(), EditError>;
}
impl<'a> UmlLoweringCursor<'a> {
    pub(crate) fn new(context: EditContext<'a>) -> Self;
    pub(crate) fn apply(&mut self, index: usize, op: &uml::Op) -> Result<(), EditError>;
    pub(crate) fn finish(self) -> SourceBundle;
}
pub(crate) fn apply_step(
    candidate: &mut SourceBundle,
    state: &mut UmlLoweringState,
    index: usize,
    op: &uml::Op,
) -> Result<(), EditError>;
```

- Each operation resolves IDs/paths and typed slots against the cumulative cursor. After editing a document, reparse its claimed island before the next operation. Classifier add/remove/rename updates `current_paths`; cross-document reference edits use cumulative rebased ranges. No complete OKF/UML analysis is rebuilt inside the Lowerer.
- `UmlLoweringCursor::new` clones `context.source` once into `candidate` and initializes `state` with `UmlLoweringState::from_context(&context)` before storing `original`; `finish` returns that candidate without semantic preparation.
- `original` seeds the initial claim/path/index only. Once a concept or document is touched, later selectors and slots resolve exclusively through cumulative `state`.
- `apply_step` performs normalized/collision-free `SourceBundle` mutation first. Concept insert/rename methods then reject duplicate IDs/paths in `current_paths` before state change; remove and text invalidation drop affected typed-island caches so subsequent access reparses cumulative source.

- [ ] **Step 1: Write and run ordered UML regression tests**

Test classifier-new-then-set, classifier-new-then-attribute-add (add-then-set), classifier-rename-then-attribute-add (rename-then-edit), placement-set-then-remove, and late invalid-selector rollback. Assert exact final source and error index.

Run:

```powershell
rtk cargo test -p waml --test uml_lowering_order
```

Expected: FAIL because the second operation resolves against the original path/tree.

- [ ] **Step 2: Implement cumulative path and island state**

Replace `Document`/`Line<T>` mutation with typed syntax lookup and candidate source edits. Reparse only the touched claimed island after each step, and translate later ranges through the cumulative edit map.

- [ ] **Step 3: Route `uml::Batch` through the cursor**

Preserve public operation vocabulary/order and existing collision/cascade behavior. Return candidate source only after all operations succeed.

- [ ] **Step 4: Run and commit**

```powershell
rtk cargo test -p waml --test uml_lowering_order
rtk cargo test -p waml uml::ops::tests
rtk cargo test -p waml --test ops_golden
rtk git add crates/waml/src/uml/lower.rs crates/waml/src/uml/ops.rs crates/waml/src/uml/rename.rs crates/waml/src/uml/selector.rs crates/waml/tests/uml_lowering_order.rs crates/waml/tests/ops_golden.rs
rtk git commit -m "refactor: lower ordered UML edits cumulatively"
```

### Task 16: Preserve Mixed Compatibility Order on One Candidate

**Files:**
- Modify: `crates/waml/src/compat.rs`
- Modify: `crates/waml/src/ops/mod.rs`
- Create: `crates/waml/tests/compat_lowering_order.rs`
- Modify: `crates/waml-ops-dto/src/lib.rs`

**Interfaces:**
- `compat::Batch` remains sealed and compatibility-only. Its crate-private cursor is exact:

```rust
struct MixedLoweringCursor<'a> {
    original: EditContext<'a>,
    candidate: SourceBundle,
    okf: OkfLoweringState,
    uml: UmlLoweringState,
}
impl<'a> MixedLoweringCursor<'a> {
    fn new(context: EditContext<'a>) -> Self;
    fn apply(&mut self, index: usize, step: &compat::Step) -> Result<(), EditError>;
    fn finish(self) -> SourceBundle;
    fn propagate(&mut self, event: CandidateInvalidation) -> Result<(), EditError>;
}
enum CandidateInvalidation {
    TextChanged(BundlePath),
    Inserted { id: Option<String>, path: BundlePath },
    Removed { id: Option<String>, path: BundlePath },
    Renamed { id_from: Option<String>, id_to: Option<String>, from: BundlePath, to: BundlePath },
}
```

- It calls `okf::lower::apply_step(&mut candidate, &mut okf, ...)` or `uml::lower::apply_step(&mut candidate, &mut uml, ...)` and propagates path/text invalidation to the sibling state after every successful step.
- `MixedLoweringCursor::new` clones `context.source` exactly once, constructs both state values from the same untouched context, and then stores `original`; `finish` returns the single cumulative candidate.
- `propagate` is called exactly once for every successful text/path mutation before the next mixed step. `TextChanged(path)` calls both `invalidate_text` methods. `Inserted` calls `okf.inserted`; for UML it calls `inserted_concept` when `id` is present and otherwise `invalidate_text`. `Removed` calls `okf.removed`; for UML it calls `removed_concept` when `id` is present and always invalidates the old path. `Renamed` calls `okf.renamed`; UML calls `renamed_concept` for `Some`→`Some`, `removed_concept` plus destination invalidation for `Some`→`None`, `inserted_concept` for `None`→`Some`, and invalidates both paths for `None`→`None`. If propagation fails, `apply` reports the current step index and the outer sealed batch discards the cursor.
- It rebuilds neither complete OKF nor complete UML between steps. Final `prepare_candidate` remains the sole complete semantic construction.

- [ ] **Step 1: Write and run mixed-order tests**

Cover OKF import -> UML classifier set, UML classifier new -> OKF directory retitle, OKF rename -> UML attribute add (rename-then-edit), UML rename -> placement set, and a final collision. Assert inter-domain order, original DTO error index, exact rollback, and one final analysis build in the caller.

Run:

```powershell
rtk cargo test -p waml --test compat_lowering_order
```

Expected: FAIL because the compatibility adapter delegates later steps with stale original context.

- [ ] **Step 2: Implement one mixed cumulative cursor**

Share candidate path/source invalidations between domain cursors. Reparse only the next step's touched shell/island and retain current path mappings across domain transitions.

- [ ] **Step 3: Preserve wire compatibility**

Keep every `OpDto` mapping/tag/version unchanged and add DTO-driven versions of add-then-set and rename-then-edit tests.

- [ ] **Step 4: Run and commit**

```powershell
rtk cargo test -p waml --test compat_lowering_order
rtk cargo test -p waml-ops-dto
rtk cargo test -p waml --test ops_golden
rtk git add crates/waml/src/compat.rs crates/waml/src/ops/mod.rs crates/waml/tests/compat_lowering_order.rs crates/waml-ops-dto/src/lib.rs
rtk git commit -m "fix: preserve mixed batch candidate order"
```

### Task 17: Cut the Native Editor onto Shared Syntax and Analyses

**Files:**
- Modify: `crates/waml-editor/src/editor_session.rs`
- Modify: `crates/waml-editor/src/doc_view.rs`
- Modify: `crates/waml-editor/src/documents.rs`
- Modify: `crates/waml-editor/src/uml_documents.rs`
- Modify: `crates/waml-editor/src/okf_documents.rs`
- Modify: `crates/waml-editor/src/generic_okf_view.rs`
- Modify: `crates/waml-editor/src/source_view.rs`
- Modify: `crates/waml-editor/src/document_host.rs`
- Modify: `crates/waml-editor/src/app.rs`
- Modify: `crates/waml-editor/src/app/actions.rs`
- Modify: `crates/waml-editor/src/inspector.rs`
- Modify: `crates/waml-editor/src/tree.rs`
- Modify: `crates/waml-editor/src/nav.rs`
- Modify: `crates/waml-editor/src/load.rs`
- Modify: `crates/waml-editor/src/native_save.rs`

**Interfaces:**
- `ViewData` supplies `source`, `okf_analysis`, `uml_analysis`, and `revision`; old bare-model/parser inputs disappear.
- UML provider reads validated projection for rendering and declared syntax/located diagnostics for authored-state inspection/actions. Generic provider reads unclaimed OKF and shell syntax only.
- UML views call `ActionContext::new(session.okf_analysis(), session.uml_analysis(), session.revision())?`, pass the validated context to `uml::repair_actions`, and present missing-colon/type/invalid-multiplicity actions; invoking one wraps it in `SyntaxChangeBatch` and `PendingEdit`.
- `documents::open` remains explicit `uml_documents::open(...).or_else(okf_documents::open(...))`; `DocumentHost` still receives prepared `OpenDocument` and contains no semantic-family match.
- Generic OKF stays read-only. Source view may navigate shell ranges but cannot install source directly.

- [ ] **Step 1: Update provider and host contract tests**

Test supported UML, arbitrary/missing/unknown-`uml.*` Generic fallback, Index/Log non-openability, invalid claimed UML remaining owned by UML, persistent/preview/source tab identities, and provider-prepared document replacement after revision change.

Run:

```powershell
rtk cargo test -p waml-editor documents::tests
```

Expected: FAIL because editor providers still consume the bare projection and cannot produce revision-bound repairs.

- [ ] **Step 2: Migrate view data and diagnostics**

Show declared invalid-present fields and located syntax/domain diagnostics without pretending fields are absent. Resolve all actions to `PendingEdit` carrying domain batches or versioned `SyntaxChangeBatch`.

- [ ] **Step 3: Migrate navigation and source lookup**

Build navigation from `session.okf()` plus `session.uml_projection()`, preserve UML-first decoration and Generic fallback, and use catalog IDs/ranges for syntax navigation. Keep Index order and keep structural documents out of Concept tabs.

- [ ] **Step 4: Remove editor-local parsing/projection**

`load.rs` returns validated `SourceBundle`; `EditorSession::replace` owns analysis. Search editor source and remove calls to `parse_document`, `build_model`, `serialize_document`, legacy `Line`, or direct `uml::project`.

- [ ] **Step 5: Run native focused gates**

```powershell
rtk cargo test -p waml-editor editor_session::tests
rtk cargo test -p waml-editor documents::tests
rtk cargo test -p waml-editor uml_documents::tests
rtk cargo test -p waml-editor okf_documents::tests
rtk cargo test -p waml-editor document_host::tests
rtk cargo test -p waml-editor generic_okf_view::tests
rtk cargo test -p waml-editor nav::tests
rtk cargo check -p waml-editor
rtk rg 'parse_document|build_model|serialize_document|Line<' crates/waml-editor/src
```

Expected: tests/check PASS; final scan returns no matches.

- [ ] **Step 6: Commit**

```powershell
rtk git add crates/waml-editor/src
rtk git commit -m "refactor: migrate editor to parser analyses"
```

### Task 18: Migrate the One-Shot CLI Adapter

**Files:**
- Modify: `crates/waml-cli/src/commands.rs`
- Modify: `crates/waml-cli/src/io.rs`
- Modify: `crates/waml-cli/src/main.rs`
- Modify: `crates/waml-cli/tests/cli_e2e.rs`
- Modify: `crates/waml-ops-dto/src/lib.rs`

**Interfaces:**
- CLI parse/check uses `prepare_candidate(source, None, 0)` as an ephemeral immutable analysis. CLI format/operations lower against that state, call `prepare_candidate(candidate_source, Some(previous), 1)` to validate the complete candidate, and write files only after success.
- Revisions zero/one are invocation-local stale-validation tokens, not a persisted CLI clock. The process stores no dirty/persisted/session state, exposes no mutation service, and drops both states at command exit.

- [ ] **Step 1: Write and run failing one-shot CLI transaction tests**

Replace legacy parse/serialize calls with analysis and `uml::Formatter`. Add e2e cases for Generic OKF, malformed claimed UML with diagnostics, exact no-format output, canonical format idempotence, and a late multi-file action failure that writes nothing.

Run:

```powershell
rtk cargo test -p waml-cli --test cli_e2e
```

Expected: FAIL because CLI still calls legacy parser/serializer and bypasses `prepare_candidate`.

- [ ] **Step 2: Migrate read-only and mutating commands**

Use ephemeral revision zero for the input. Formatter receives `uml::ActionContext` from that state. Lower the action/batch, prepare revision one, and only then call filesystem writers.

- [ ] **Step 3: Preserve DTO compatibility**

Keep every `OpDto` wire tag/version/nullable field and sealed ordered compatibility lowering. Add failure tests proving neither partial files nor a reusable revision authority escapes the invocation.

- [ ] **Step 4: Run and commit**

```powershell
rtk cargo test -p waml-cli --test cli_e2e
rtk cargo test -p waml-ops-dto
rtk rg 'parse_document|build_model|serialize_document|Line<' crates/waml-cli crates/waml-ops-dto
rtk git add crates/waml-cli/src/commands.rs crates/waml-cli/src/io.rs crates/waml-cli/src/main.rs crates/waml-cli/tests/cli_e2e.rs crates/waml-ops-dto/src/lib.rs
rtk git commit -m "refactor: migrate one shot CLI analysis"
```

Expected: PASS; legacy-authority scan returns no matches.

### Task 19: Give the Rust LSP One Atomic Analysis Snapshot

**Files:**
- Create: `crates/waml/src/host.rs`
- Modify: `crates/waml/src/lib.rs`
- Modify: `crates/waml-cli/src/lsp/bundle.rs`
- Modify: `crates/waml-cli/src/lsp/map.rs`
- Modify: `crates/waml-cli/src/lsp/server.rs`
- Modify: `crates/waml-cli/tests/lsp_e2e.rs`

**Interfaces:**
- Produces exact LSP ownership:

```rust
struct LspAnalysisState {
    host: LspHostIndex,
    source: SourceBundle,
    okf: OkfAnalysis,
    uml: uml::Analysis,
    revision: u64,
}
struct LspHostIndex {
    root: Option<PathBuf>,
    disk_by_physical: BTreeMap<PathBuf, SourceDocument>,
    open_by_physical: BTreeMap<PathBuf, BundlePath>,
}
struct Backend {
    client: tower_lsp::Client,
    current: RwLock<Arc<LspAnalysisState>>,
}
```

- Produces narrow, pure host-ingress APIs in `waml::host`:

```rust
pub enum HostIngressError {
    ExistingDocument { path: BundlePath },
    MissingDocument { path: BundlePath },
    Source(SourceError),
}
pub fn add_document(
    current: &SourceBundle, document: SourceDocument
) -> Result<SourceBundle, HostIngressError>;
pub fn replace_document(
    current: &SourceBundle, document: SourceDocument
) -> Result<SourceBundle, HostIngressError>;
pub fn remove_document(
    current: &SourceBundle, path: &BundlePath
) -> Result<SourceBundle, HostIngressError>;
```

- These `#[doc(hidden)]` functions clone the bundle structure, call crate-private source mutation helpers, preserve every untouched `Arc<String>`, and return a validated candidate. They own no source, install/swap, revision, syntax, analysis, save, or dirty state. `SyntaxChangeBatch` remains unable to add/remove documents; these functions are host lifecycle ingress, not parser workspace mutation.
- `HostIngressError: Display + std::error::Error`; `add_document` rejects an existing normalized path, `replace_document`/`remove_document` reject a missing path, and no error changes the input bundle or its allocation identities.
- `logical_path(root, physical)` is exact: a path under root becomes its slash-normalized relative `.md` path; otherwise normalize physical separators, drop empty/`.`/`..` segments, replace `:` with `_`, prefix `__external__/`, and validate with `BundlePath::parse`. This preserves current external-document behavior without admitting absolute paths to `SourceBundle`.
- Initialization reads disk once, creates each `SourceDocument`, stores its clone in `disk_by_physical`, and builds revision-zero source from the same shared documents. `open_by_physical` starts empty.
- Every ingress clones the current `Arc`, host index, and source outside the write lock, calls the host API, computes the next revision with `checked_add(1)` (overflow rejects with no swap), calls `prepare_candidate(candidate, Some(previous), next_revision)`, then reacquires the write lock and swaps the whole `Arc<LspAnalysisState>` only if the current revision still equals the base. A race discards/retries the entire ingress.
- `did_open`: derive physical/logical path; if logical exists in current source call `replace_document`, otherwise `add_document`; insert physical→logical in `open_by_physical`. Thus an unsaved/external document absent from disk seed becomes a validated Concept/source candidate.
- Before replacement, require any existing disk/open owner of that logical path to have the same physical path; a normalized logical collision from a different physical URI rejects the ingress with no swap.
- `did_change`: require the physical path in `open_by_physical`, create a replacement `SourceDocument` at that logical path, and call `replace_document`.
- `did_close`: remove the open mapping; when `disk_by_physical` contains the physical path, restore that shared disk `SourceDocument` through `replace_document`; otherwise remove the overlay-only logical document through `remove_document`.
- A change for a non-open physical path logs one LSP warning and leaves state/revision unchanged. Closing a non-open path is an idempotent no-op with no revision bump. Reopening an already-open physical path follows the `did_change` replacement path.
- Disk/open precedence is therefore deterministic: open text wins while open, disk text returns on close, and overlay-only source disappears on close. Same-path open/change/restore preserves `DocumentId` and increments `DocumentRevision`; overlay-only close drops the ID, and a later reopen receives a fresh never-reused ID.
- Revision is owned only by the LSP snapshot and advances on successful atomic swap. LSP has no dirty/persisted/save authority. Diagnostics/actions are computed from one `Arc<LspAnalysisState>` and carry its revision.

- [ ] **Step 1: Write and run failing atomic LSP state tests**

Test disk-backed open/change/close restore, overlay-only/external open/change/close removal, normalized disk/open precedence, add collision, missing change/close, structural analysis failure, two racing FULL changes, stale document/bundle actions, diagnostics during a later failed change, and exact equality of host/source/catalog/UML revision in each observed snapshot.

Run:

```powershell
rtk cargo test -p waml-cli --test lsp_e2e
```

Expected: FAIL because host add/remove ingress and one atomic host/source/analysis snapshot do not exist.

- [ ] **Step 2: Implement candidate-then-compare-and-swap**

Implement the three pure `waml::host` functions first, including untouched-allocation tests. Then implement the ingress steps and compare-and-swap flow above; never publish host maps, source, diagnostics, analysis, or revision separately.

- [ ] **Step 3: Add UTF-16/CRLF/Unicode tests**

Use ASCII, `é`, combining mark, and astral characters before ranges on LF/CRLF. Convert through the snapshot catalog `LineIndex`; assert broken frontmatter recovery and no diagnostic/action from a superseded revision.

- [ ] **Step 4: Run and commit**

```powershell
rtk cargo test -p waml-cli --test lsp_e2e
rtk cargo test -p waml-cli lsp
rtk cargo test -p waml host::tests
rtk git add crates/waml/src/host.rs crates/waml/src/lib.rs crates/waml-cli/src/lsp crates/waml-cli/tests/lsp_e2e.rs
rtk git commit -m "refactor: atomically swap LSP analyses"
```

### Task 20: Verify the Independent VS Code Stdio Client

**Files:**
- Modify tests only: `packages/vscode/src/serverPath.test.ts`
- Verify unchanged implementation contract: `packages/vscode/src/extension.ts`
- Verify unchanged implementation contract: `packages/vscode/src/serverPath.ts`

**Interfaces:**
- VS Code launches the configured Rust executable as `waml lsp --stdio` through `vscode-languageclient`. It owns no parser, syntax kinds, source/analysis state, revisions, semantic model, WASM fallback, or generated TypeScript domain.

- [ ] **Step 1: Write and run failing transport-isolation tests**

Assert default/configured executable, exact stdio arguments, restart configuration, and absence of imports from retired `@waml/*`, WASM, or parser-domain packages.

Run:

```powershell
rtk pnpm --filter @waml/vscode test
```

Expected: PASS if the retained stdio client already satisfies the expanded characterization; any failure identifies a launch-contract defect to fix before the commit.

- [ ] **Step 2: Update tests without adding client semantics**

Keep `extension.ts` and `serverPath.ts` unchanged unless a test exposes an actual launch-contract defect. Do not mirror Rust diagnostics/actions or revision state in TypeScript.

- [ ] **Step 3: Run and commit**

```powershell
rtk pnpm --filter @waml/vscode build
rtk pnpm --filter @waml/vscode test
rtk rg -n '^import .*(@waml/|wasm|parser|syntax)|^export .*(@waml/|wasm|parser|syntax)' packages/vscode/src/extension.ts packages/vscode/src/serverPath.ts
rtk git add packages/vscode/src/serverPath.test.ts
rtk git commit -m "test: lock VS Code stdio isolation"
```

Expected: build/tests PASS; exact production-import scan returns no matches.

### Task 21: Delete the Final Legacy Parser and Serializer Authority

**Files:**
- Delete: `crates/waml/src/grammar.rs`
- Delete: `crates/waml/src/parse.rs`
- Delete: `crates/waml/src/syntax.rs`
- Delete: `crates/waml/src/serialize.rs`
- Modify: `crates/waml/src/lib.rs`
- Modify: `crates/waml/src/frontmatter.rs`
- Modify: `crates/waml/src/layout.rs`
- Modify: `crates/waml/src/validate.rs`
- Modify: `crates/waml/src/model.rs`
- Modify: `crates/waml/src/uml.rs`
- Modify: `crates/waml/src/uml/lower.rs`
- Modify: `crates/waml/src/okf.rs`
- Modify: `crates/waml/src/ops/mod.rs`
- Modify: `crates/waml/src/seed.rs`
- Modify: `crates/waml/tests/golden.rs`
- Modify: `crates/waml/tests/serde_shape.rs`
- Modify: `crates/waml/tests/layout_serde_roundtrip.rs`
- Create: `crates/waml/tests/no_legacy_authority.rs`

**Interfaces:**
- Removes production `Document`, `Section`, `Line<T>`, `ErrorNode`, handwritten line parsers/renderers, `parse_document`, `build_model`, `build_model_from_source`, `serialize_document`, and temporary old/new differential adapters.
- Keeps semantic model/value types, `okf::Bundle::parse`, `uml::{recognizes, analyze, Projection}`, domain batches/Lowerers, sealed Rust DTO compatibility, formatter, validation, seed generation, serde shapes, and retained public Rust contracts that do not create a second parser authority.
- `parse_document`/serializer compatibility is not retained under another name. Automation consumers use analysis/formatter APIs.

- [ ] **Step 1: Write and run the failing one-authority architecture test**

The test resolves `CARGO_MANIFEST_DIR`, asserts `src/{grammar,parse,syntax,serialize}.rs` are absent, and asserts `src/lib.rs` contains none of `pub mod grammar;`, `pub mod parse;`, `pub mod syntax;`, or `pub mod serialize;`.

Run:

```powershell
rtk cargo test -p waml --test no_legacy_authority
```

Expected: FAIL listing all four still-present authority modules.

- [ ] **Step 2: Inventory every legacy authority reference**

Run:

```powershell
rtk rg -n 'parse_document|build_model|build_model_from_source|project_okf|serialize_document|Document\b|Section\b|Line<|ErrorNode|parse_(attribute|value|slot|relationship|members|flow|lifeline|message)|render_(attribute|slot|relationship|members|flow|lifeline|message)' crates packages/vscode --glob '!crates/waml/src/model.rs'
```

Expected: matches are confined to the four deletion targets, semantic helper moves named in this task, and temporary differential tests.

- [ ] **Step 3: Move remaining semantic-only helpers**

Move `Direction` and layout semantic enums to `layout.rs`, frontmatter value types to `frontmatter.rs`, and any seed/canonical helpers to `uml/format.rs` or `seed.rs`. Do not move recovery or parsing structs into semantic modules.

- [ ] **Step 4: Delete old modules and differential code**

Remove module exports and update tests to use shell/UML syntax, declared values, validated projection, or formatter as appropriate. Preserve Task 1 golden files as new-parser expectations and keep the checked-in measurement-method record.

- [ ] **Step 5: Run the one-authority scan and workspace tests**

```powershell
rtk rg -n 'parse_document|build_model|build_model_from_source|project_okf|serialize_document|Line<|ErrorNode' crates packages/vscode
rtk cargo test -p waml --test no_legacy_authority
rtk cargo test --workspace --all-features
rtk cargo check --workspace --all-features
```

Expected: scan returns no production or test matches; tests/check PASS with one parser, writer distinction, source authority, and mutation boundary.

- [ ] **Step 6: Commit**

```powershell
rtk git add -A -- crates/waml/src crates/waml/tests
rtk git commit -m "refactor: retire legacy parser authority"
```

### Task 22: Add Incremental Reparse with Bounded Source Retention

**Files:**
- Create: `crates/waml-syntax/src/incremental.rs`
- Create: `crates/waml-syntax/tests/incremental.rs`
- Modify: `crates/waml-syntax/src/lib.rs`
- Modify: `crates/waml-syntax/src/shell/parser.rs`
- Modify: `crates/waml/src/source.rs`
- Modify: `crates/waml/src/analysis.rs`
- Modify: `crates/waml/src/uml/analysis.rs`
- Create: `crates/waml/tests/incremental_analysis.rs`
- Modify: `crates/waml-editor/src/editor_session.rs` inline tests

**Interfaces:**
- Produces:

```rust
pub struct TextChange { pub old_range: TextRange, pub replacement: Arc<str> }
pub struct ChangeSegment {
    pub old: TextRange,
    pub new: TextRange,
}
pub struct ChangeMap {
    old_len: TextSize,
    new_len: TextSize,
    segments: Arc<[ChangeSegment]>,
}
pub enum FullReparseReason {
    NoPreviousSnapshot,
    OverlappingChanges,
    InvalidUtf8Boundary,
    FrontmatterBoundaryChanged,
    MarkdownContainerBoundaryChanged,
    HeadingBoundaryChanged,
    IslandBoundaryChanged,
    UnsafeSynchronization,
}
pub enum ReparseOutcome<L: SyntaxLanguage> {
    Incremental {
        tree: Arc<SyntaxTree<L>>,
        shared_source_independent_green: usize,
        reparsed_range: TextRange,
    },
    Full { tree: Arc<SyntaxTree<L>>, reason: FullReparseReason },
}
pub fn reparse_okf_markdown(
    previous: &SyntaxTree<OkfMarkdownLanguage>,
    new_text: SourceText,
    changes: &[TextChange],
) -> Result<ReparseOutcome<OkfMarkdownLanguage>, ParseError>;
#[cfg(test)]
pub(crate) fn source_text_weak(document: &SourceDocument) -> Weak<String>;
```

- The smallest safe window is the containing frontmatter, raw Markdown region, confirmed heading section, or whole document when container/heading/frontmatter boundaries may change.
- UML reparses the smallest claimed island whose boundary survives; otherwise it full-parses that claimed document. Full parse is always available and is the semantic/exactness oracle.
- Diagnostics inside the window regenerate; unaffected ranges translate through `ChangeMap`. Green diagnostics never exist.
- An unchanged document revision reuses the exact snapshot/tree/green graph. In a changed document, every `GreenText::SourceSlice` leaf is rebuilt with the current `SourceText`, and every ancestor containing one is rebuilt. Only `GreenNodeData::is_source_independent()` / `GreenTokenData::is_source_independent()` elements may be `same_green` across changed revisions.
- Annotations on mapped unchanged occurrences are copied onto rebuilt greens through `ChangeMap`; annotation continuity does not imply green identity. No green in the current graph may retain a historical whole-document `Arc<String>`.

- [ ] **Step 1: Write full-vs-incremental oracle tests**

For edits at start/end, token middle, multibyte boundary, frontmatter fence/type, heading marker, indentation, nested flow/sequence, and multiple non-overlapping changes, compare exact text, full tree structural value, diagnostics, declared UML, and validated projection with a clean full parse.

Run:

```powershell
rtk cargo test -p waml-syntax --test incremental
```

Expected: FAIL because `TextChange`, `ChangeMap`, `FullReparseReason`, and incremental reparse do not exist.

- [ ] **Step 2: Implement checked change maps and safe windows**

Reject overlap, unsorted changes, old-range mismatch, and non-UTF-8 boundaries. Fall back with a named reason when synchronization cannot be proven.

- [ ] **Step 3: Implement shell and UML reuse**

Outside the safe window, rebase every source-backed leaf to the current `SourceText` and rebuild its ancestors. Share only static/owned source-independent tokens/subtrees. Tests must assert: unchanged document root is `same_green`; changed-document source-backed identifier/raw region and ancestors are not; static missing tokens may be; every current source slice points to the current allocation.

- [ ] **Step 4: Integrate previous analyses**

`analyze_okf(previous)` shares unchanged snapshots and incrementally reparses changed paths. `uml::analyze(previous)` shares unchanged claimed snapshots and uses shell structure changes to choose island/full fallback. No specialization allocates document IDs. Copy annotations for mapped unchanged occurrences even though source-backed green identity changes; reject old locators because tree/revision changes.

- [ ] **Step 5: Prove bounded historical retention**

In `analysis.rs`'s crate-internal test module, simulate 1,000 alternating one-byte candidates while retaining only current analysis plus a baseline `SourceBundle`. Capture `source_text_weak` before replacing each whole-source allocation: at most current and baseline weak handles upgrade for the touched path; after replacing the baseline with current, exactly current upgrades. Assert untouched paths retain one shared allocation. These pure lower-crate tests do not import or call binary-only `EditorSession`.

- [ ] **Step 6: Test actual editor apply/save/annotation integration**

In `editor_session.rs` inline tests, perform the same repeated edits through `EditorSession::apply`, verify current/persisted bounds before and after `mark_saved`, and annotate an unchanged occurrence outside the reparse window. After atomic install, the annotation resolves in the new tree, its source-backed green is not `same_green`, and the old locator is stale.

- [ ] **Step 7: Run and commit**

```powershell
rtk cargo test -p waml-syntax --test incremental
rtk cargo test -p waml --test incremental_analysis
rtk cargo test -p waml-editor editor_session::tests
rtk git add crates/waml-syntax/src/incremental.rs crates/waml-syntax/src/lib.rs crates/waml-syntax/src/shell/parser.rs crates/waml-syntax/tests/incremental.rs crates/waml/src/source.rs crates/waml/src/analysis.rs crates/waml/src/uml/analysis.rs crates/waml/tests/incremental_analysis.rs crates/waml-editor/src/editor_session.rs
rtk git commit -m "perf: add bounded incremental syntax reuse"
```

### Task 23: Add Property, Fuzz, Golden, Performance, and Allocation Gates

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `crates/waml-syntax/Cargo.toml`
- Create: `crates/waml-syntax/tests/properties.rs`
- Modify: `crates/waml/tests/golden.rs`
- Create: `crates/waml/tests/parser_platform_properties.rs`
- Modify: `crates/waml/examples/parser_platform_baseline.rs`
- Create: `fuzz/Cargo.toml`
- Create: `fuzz/fuzz_targets/outer_mapping.rs`
- Create: `fuzz/fuzz_targets/uml_islands.rs`
- Create: `fuzz/fuzz_targets/syntax_edits.rs`
- Create: `fuzz/fuzz_targets/parse_write.rs`

**Interfaces:**
- `proptest` generates arbitrary UTF-8 Markdown, frontmatter-like prefixes, nested headings/containers, UML token lines, and valid/invalid edit sequences.
- Fuzz targets assert progress/no panic, bounded ranges, checked widths, exact parse/write, no duplicate bytes, full-vs-incremental equality, and action atomicity.
- The release example records a local post-platform observation and optionally compares it with the untracked post-OKF observation. Matching hardware/corpus prints deltas with `LATENCY_REPORT_ONLY`; absent or mismatched evidence prints `LATENCY_SKIPPED_*` and exits zero. Only separately reviewed numeric budgets added later to the method record may turn latency into a failing gate.
- Correctness, exact writing, parser progress, and the explicit weak-reference whole-source retention bounds from Task 22 are always failing gates and do not depend on hardware.

- [ ] **Step 1: Add syntax invariants as property tests**

For arbitrary UTF-8 input assert parse completes, exact writer equality, ordered leaves concatenate exactly once, all ranges and UTF-8 boundaries are valid, child widths sum, red navigation terminates, and full parse equals incremental parse after generated valid edits.

Run:

```powershell
rtk cargo test -p waml-syntax --test properties
```

Expected: FAIL before the property strategies and invariant coverage exist.

- [ ] **Step 2: Add domain/boundary properties**

Generate arbitrary type strings and assert only supported UML types are claimed; Index/Log never become Concepts; unclaimed headings never expose UML wrappers; declared invalid-present differs from absent; two identical claim sets always yield deterministic ambiguity.

- [ ] **Step 3: Add fuzz targets**

Each target has one narrow entry: outer CommonMark/frontmatter mapping, claimed UML islands, versioned edit validation/application, and parse/write plus incremental oracle. Seed libFuzzer corpora from Task 1 fixtures.

- [ ] **Step 4: Add allocation and performance checks**

Count full-document allocations outside parallel test execution. Assert full parse shares input, one source edit allocates one replacement whole string for each touched document, unchanged snapshots allocate no whole strings, and the 1,000-edit retention bound from Task 22. Record/report a local candidate observation; do not turn an observed latency/allocation maximum into a portable correctness limit.

- [ ] **Step 5: Run deterministic CI gates and bounded local fuzz smoke**

```powershell
rtk cargo test -p waml-syntax --test properties
rtk cargo test -p waml --test parser_platform_properties
rtk cargo test -p waml --test golden
rtk proxy pwsh -NoProfile -Command 'New-Item -ItemType Directory -Force -Path "C:\tmp\parser-platform-baseline" | Out-Null'
rtk cargo run -p waml --example parser_platform_baseline --release -- --method docs/superpowers/baselines/2026-07-28-parser-platform-method.json --record C:\tmp\parser-platform-baseline\post-platform.json
rtk cargo run -p waml --example parser_platform_baseline --release -- --method docs/superpowers/baselines/2026-07-28-parser-platform-method.json --compare-if-present C:\tmp\parser-platform-baseline\post-okf.json C:\tmp\parser-platform-baseline\post-platform.json
rtk cargo fuzz run outer_mapping -- -max_total_time=60
rtk cargo fuzz run uml_islands -- -max_total_time=60
rtk cargo fuzz run syntax_edits -- -max_total_time=60
rtk cargo fuzz run parse_write -- -max_total_time=60
```

Expected: property/golden/retention gates PASS; latency prints `LATENCY_REPORT_ONLY`, `LATENCY_SKIPPED_BASELINE_ABSENT`, or `LATENCY_SKIPPED_HARDWARE_MISMATCH` and exits zero; every fuzz target completes 60 seconds without crash, timeout, OOM, or invariant failure.

- [ ] **Step 6: Commit**

```powershell
rtk git add Cargo.toml Cargo.lock crates/waml-syntax/Cargo.toml crates/waml-syntax/tests/properties.rs crates/waml/tests/golden.rs crates/waml/tests/parser_platform_properties.rs crates/waml/examples/parser_platform_baseline.rs fuzz
rtk git commit -m "test: harden parser platform gates"
```

### Task 24: Run Full Architecture, Automated, Host, and Native Verification

**Files:**
- Modify: none expected; fix a failure in the owning Task 1-23 commit, rerun its focused gate, then restart this task.
- Create execution evidence outside the repository under `C:\tmp\parser-platform-verification\`.

**Interfaces:**
- Consumes: the complete one-authority parser platform.
- Produces: workspace, lint, property, fuzz-smoke, performance/allocation, static architecture, retained-host, and native visual evidence.

- [ ] **Step 1: Run formatting and complete Rust gates**

```powershell
rtk cargo fmt --check
rtk cargo test --workspace --all-features
rtk cargo clippy --workspace --all-targets --all-features -- -D warnings
rtk cargo doc --workspace --all-features --no-deps
```

Expected: PASS with no warning; no aggregate test-count comparison is used as a gate.

- [ ] **Step 2: Run parser hardening and resource gates**

```powershell
rtk cargo test -p waml-syntax --test properties
rtk cargo test -p waml --test parser_platform_properties
rtk cargo test -p waml --test incremental_analysis
rtk cargo test -p waml-editor editor_session::tests
rtk proxy pwsh -NoProfile -Command 'New-Item -ItemType Directory -Force -Path "C:\tmp\parser-platform-baseline" | Out-Null'
rtk cargo run -p waml --example parser_platform_baseline --release -- --method docs/superpowers/baselines/2026-07-28-parser-platform-method.json --record C:\tmp\parser-platform-baseline\post-platform-final.json
rtk cargo run -p waml --example parser_platform_baseline --release -- --method docs/superpowers/baselines/2026-07-28-parser-platform-method.json --compare-if-present C:\tmp\parser-platform-baseline\post-okf.json C:\tmp\parser-platform-baseline\post-platform-final.json
rtk cargo fuzz run outer_mapping -- -runs=10000
rtk cargo fuzz run uml_islands -- -runs=10000
rtk cargo fuzz run syntax_edits -- -runs=10000
rtk cargo fuzz run parse_write -- -runs=10000
```

Expected: correctness and hardware-independent retention tests PASS; latency reports or skips successfully without enforcing observed local maxima; fuzz targets complete without findings.

- [ ] **Step 3: Run retained CLI/LSP/VS Code gates**

```powershell
rtk cargo test -p waml-cli
rtk cargo test -p waml-ops-dto
rtk pnpm install --frozen-lockfile
rtk pnpm --filter @waml/vscode build
rtk pnpm --filter @waml/vscode test
```

Expected: PASS; VS Code still launches Rust LSP over stdio.

- [ ] **Step 4: Run static authority and dependency scans**

```powershell
rtk rg -n 'parse_document|build_model|build_model_from_source|project_okf|serialize_document|Line<|ErrorNode' crates packages/vscode
rtk rg -n 'pub fn (insert|remove|rename|replace_text|replace_syntax|save|mark_dirty|set_revision)\b' crates/waml/src/analysis.rs
rtk cargo tree -p waml-syntax --edges normal
rtk rg -n 'pub (trait|struct|enum) (SpecializationPlugin|SpecializationRegistry|DocumentFamily)\b' crates/waml/src crates/waml-editor/src
rtk rg -n 'match .*NavCategory|match .*Uml|match .*GenericOkf' crates/waml-editor/src/document_host.rs
rtk cargo test -p waml --test analysis_catalog
rtk cargo test -p waml --test specialization_composition
rtk cargo test -p waml --test no_legacy_authority
rtk cargo test -p waml host::tests
rtk cargo test -p waml-editor editor_session::tests
rtk cargo test -p waml-cli --test cli_e2e
rtk cargo test -p waml-cli --test lsp_e2e
rtk rg -n 'waml-wasm|wasm-bindgen|tsify' Cargo.toml crates --glob 'Cargo.toml'
rtk rg --files -g '*.svelte'
rtk proxy pwsh -NoProfile -Command '$removed = @("packages/web","packages/core","packages/okf","packages/wasm","crates/waml-wasm"); $present = $removed | Where-Object { Test-Path -LiteralPath $_ }; if ($present) { Write-Error ("Retired paths present: " + ($present -join ", ")); exit 1 }; "RETIRED_PATHS_ABSENT"'
```

Expected:

- legacy-authority, forbidden-public-mutation, runtime-specialization declaration, host-family-match, retired dependency, and Svelte scans return no matches; explicit path check prints `RETIRED_PATHS_ABSENT`;
- `rtk cargo tree -p waml-syntax --edges normal` contains only the standard library and explicitly approved Markdown dependency graph, never `waml`, editor, CLI, LSP, Makepad, serde, or IO/URI libraries;
- lookup-only compile-fail/API tests, pure preparation tests, editor transaction tests, ephemeral CLI tests, and atomic LSP snapshot tests pass;
- `DocumentHost` has no semantic family dispatch.

- [ ] **Step 5: Build and launch the native editor**

```powershell
rtk cargo build -p waml-editor
rtk proxy pwsh -NoProfile -Command 'New-Item -ItemType Directory -Force -Path "C:\tmp\parser-platform-verification" | Out-Null'
```

Launch the worktree-built executable against a disposable copy of the parser-platform mixed corpus. Do not stop or reuse a user-owned editor process.

- [ ] **Step 6: Capture required native states**

```powershell
rtk pwsh -File scripts/capture-window.ps1 -Out C:\tmp\parser-platform-verification\declared-invalid.png -Process waml-editor
```

Capture separate native-pixel images for valid UML, incomplete attribute with targeted repair, invalid multiplicity, malformed frontmatter recovery, Generic arbitrary type, unknown `uml.*` Generic fallback, mixed navigator, explicit Source view, and persistent UML/Generic/Source tabs. Verify Index/Log have no Concept tab and Generic OKF remains Markdown-only.

- [ ] **Step 7: Exercise transaction and stale-action paths manually**

In the disposable corpus, prepare a code action, edit the document, then invoke the stale action and confirm no source/UI partial change. Exercise one multi-file syntax action whose final validation fails, a valid formatter action, save, reopen, and confirm allocation-independent visible state and stable provider selection.

- [ ] **Step 8: Inspect final diff**

```powershell
rtk git status --short
rtk git diff --check
rtk git log --oneline --decorate -20
```

Expected: only planned implementation files are present, no whitespace errors, and Tasks 1-23 have review-sized commits in dependency order.

## Completion Criteria

- `SourceBundle` remains the only source/document authority; syntax/catalog state is immutable, derived, revision-scoped, and lookup-only.
- Every retained Markdown byte has lossless shell syntax; claimed UML has complete current typed grammar, deterministic trivia, expected-kind missing tokens, bad/skipped recovery, declared fields, validated projection, and located diagnostics.
- Generic OKF, unknown `uml.*`, arbitrary/missing type, Index/Log separation, and test-only future sibling/ambiguity acceptance tests pass.
- OKF and every specialization consume one shared immutable candidate revision through a domain-neutral context and disjoint static composition.
- All native product edits, code actions, formatters, and domain operations lower to source and enter the sole atomic `EditorSession` path. The CLI uses one ephemeral prepare/validate/write transaction; the LSP owns one atomically swapped immutable snapshot. All three reuse `prepare_candidate`.
- Exact writing, broad canonical formatting, and syntax-native Lowerers are distinct and tested.
- Full parse remains the oracle/fallback; unchanged documents share complete trees, while changed documents share only source-independent greens, copy annotations across rebuilt occurrences, and retain no unbounded historical source allocations.
- Native editor, CLI, Rust LSP, and VS Code use the shared platform; web/WASM/TypeScript-domain stacks remain absent.
- Legacy parser/serializer authorities and temporary differential adapters are deleted.
- Golden, property, fuzz, hardware-independent allocation/retention, report-only local latency, full workspace, clippy, documentation, VS Code, static architecture, and native visual gates pass.
