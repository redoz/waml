# Incremental Markdown Syntax Platform Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make one lossless, revisioned `waml-syntax` tree the incremental authority for CommonMark 0.31.2, the five named GFM extensions, WAML frontmatter, and WAML section islands.

**Architecture:** Replace the opaque Markdown-region shell with a two-phase block/inline parser that writes the existing green/red tree and derives references, the compatibility `MarkdownStructureMap`, diagnostics, and presentation queries from that tree. Wrap each full or incremental result in one immutable `MarkdownSyntaxSnapshot`; local reparsing works at safe block roots and expands the affected-range set when a changed reference definition changes non-contiguous inline blocks.

**Tech Stack:** Rust 2021, MSRV 1.80, existing `waml-syntax` green/red trees and `SourceText`, `pulldown-cmark 0.12.2` for CommonMark block/event recognition, custom lossless delimiter segmentation and GFM extended-autolink/tag-filter classification, `proptest = 1.8.0`, and existing cargo-fuzz/libFuzzer targets.

## Global Constraints

- Conform to CommonMark 0.31.2.
- Enable tables, task-list items, strikethrough, extended autolinks, and tag filtering together in `MarkdownDialect::WAML_DEFAULT`.
- If a GFM fixture conflicts with CommonMark core, use the CommonMark 0.31.2 core result and use the GFM fixture only for its named extension.
- Preserve every source byte and delimiter token. Exact source recovery must succeed for every published snapshot.
- Keep one outer `SyntaxTree<OkfMarkdownLanguage>` for document, frontmatter, all Markdown blocks and inlines, and WAML section ownership.
- Keep specialized WAML/UML trees as embedded analyses keyed by their owning Markdown `SyntaxIdentity` and `TextRange`. They must not parse surrounding Markdown.
- Do not add footnotes, math, Mermaid, definition lists, heading attributes, smart punctuation, GFM alerts, or another product-specific syntax flag.
- Do not execute raw HTML. Parse it, and classify the GFM-disallowed tags `title`, `textarea`, `style`, `xmp`, `iframe`, `noembed`, `noframes`, `script`, and `plaintext`.
- Keep all positions as checked half-open UTF-8 byte `TextRange` values.
- Keep snapshots immutable and tagged with `DocumentRevision`. Reject a same or older requested revision with `ParseError::NonMonotonicRevision`.
- Express every `MarkdownSyntaxUpdate::affected_ranges` in the new snapshot. Sort, de-duplicate, and merge touching ranges.
- Set `MarkdownReparseOutcome::Incremental::reparsed_range` to `Some(range)` only when the normalized affected set contains one range. Use `None` for non-contiguous reference fan-out.
- Keep full parsing as the oracle and explicit correctness fallback. Every fallback records one `FullReparseReason`.
- Derive `MarkdownStructureMap` from the completed outer tree. Do not scan raw Markdown a second time.
- Expose framework-free presentation queries. Native UI and LSP code must not classify Markdown with regular expressions.
- Keep `waml-syntax` independent of Makepad and of the `waml` domain crate. `waml` can depend on and re-export `waml-syntax` types.
- Use only explicit `pulldown_cmark::Options::ENABLE_TABLES`, `ENABLE_STRIKETHROUGH`, and `ENABLE_TASKLISTS`. Do not use `Options::all()` or `Options::ENABLE_GFM`, because those enable syntax outside this spec.
- Every shell command in this plan starts with `rtk`, as required by `RTK.md`.
- In each task, write the listed failing test first, run its focused command, implement only the named behavior, rerun the command, and commit only after the focused command passes.

---

## File Structure

```text
crates/waml-syntax/
├── src/lib.rs                         # public re-exports only
├── src/text.rs                        # SourceText, ranges, dialect flags, revision
├── src/incremental.rs                 # generic TextChange/ChangeMap/green rebasing
├── src/shell.rs                       # temporary low-level compatibility delegates
├── src/shell/parser.rs                # delete after the full parser replaces it
├── src/markdown/mod.rs                # full-parse orchestration and module boundary
├── src/markdown/kind.rs               # Markdown syntax and diagnostic kinds
├── src/markdown/block.rs              # line/block phase and safe block roots
├── src/markdown/inline.rs             # delimiter and inline phase
├── src/markdown/reference.rs          # normalized link-reference definitions/backlinks
├── src/markdown/gfm.rs                # five named GFM extensions and HTML tag filter
├── src/markdown/projection.rs         # tree-derived MarkdownStructureMap/island projection
├── src/markdown/query.rs              # presentation spans, metadata, stable identities
├── src/markdown/snapshot.rs           # immutable revisioned public product
└── src/markdown/reparse.rs            # incremental block/reference/island scheduling
crates/waml-syntax/tests/
├── markdown_blocks.rs                 # complete block shapes and lossless markers
├── markdown_inlines.rs                # complete inline shapes and reference resolution
├── markdown_gfm.rs                    # five GFM extension contracts
├── markdown_queries.rs                # public framework-free query contract
├── markdown_snapshot.rs               # revision and ownership contract
├── markdown_incremental.rs            # deterministic incremental/full oracle cases
├── markdown_conformance.rs            # imported CommonMark/GFM fixtures
├── markdown_recovery.rs               # malformed and Unicode recovery matrix
├── fixtures/commonmark-0.31.2/
│   ├── spec.json                      # official 652 CommonMark examples
│   ├── LICENSE                        # CC-BY-SA-4.0 fixture license
│   └── SOURCE.md                      # URL, 0.31.2 revision, import command, digest
└── fixtures/gfm-0.29/
    ├── spec.txt                       # official GFM specification examples
    ├── LICENSE                        # CC-BY-SA-4.0 fixture license
    └── SOURCE.md                      # URL, 0.29-gfm revision, import command, digest
crates/waml/
├── src/analysis.rs                    # consume MarkdownSyntaxSnapshot per document
├── src/okf/shell.rs                   # consume tree-derived structure projection
└── src/uml/syntax/{mod.rs,parser.rs}  # key islands by Markdown SyntaxIdentity/range
fuzz/
├── fuzz_targets/parse_write.rs        # full Markdown exact-write invariant
├── fuzz_targets/syntax_edits.rs       # full/incremental snapshot oracle
└── seeds/{parse_write,syntax_edits}/  # GFM, reference, malformed, Unicode seeds
```

### Task 1: Freeze the Public Dialect, Revision, Kind, and Snapshot Contract

**Files:**
- Create: `crates/waml-syntax/src/markdown/mod.rs`
- Create: `crates/waml-syntax/src/markdown/kind.rs`
- Create: `crates/waml-syntax/src/markdown/snapshot.rs`
- Create: `crates/waml-syntax/tests/markdown_snapshot.rs`
- Modify: `crates/waml-syntax/src/text.rs`
- Modify: `crates/waml-syntax/src/lib.rs`
- Modify: `crates/waml-syntax/src/shell.rs`
- Delete: `crates/waml-syntax/src/markdown.rs`

**Interfaces:**
- Consumes: existing `SourceText`, `TextRange`, `SyntaxTree<OkfMarkdownLanguage>`, `TreeDiagnostic<OkfSyntaxDiagnosticCode>`, and `MarkdownStructureMap`.
- Produces: `DocumentRevision`, stable `SyntaxIdentity`, explicit `MarkdownDialect` profiles, full Markdown syntax kinds, `MarkdownSyntaxSnapshot`, `MarkdownSyntaxUpdate`, `MarkdownReparseOutcome`, `parse_markdown`, and `reparse_markdown`.

- [ ] **Step 1: Write compile-time public-contract tests**

Create `markdown_snapshot.rs` with a test that constructs `DocumentRevision::INITIAL`, checks `checked_next`, parses `"# one\n"`, and destructures an update:

```rust
use waml_syntax::{
    parse_markdown, reparse_markdown, DocumentRevision, MarkdownDialect,
    MarkdownReparseOutcome, SourceText, TextChange, TextRange, TextSize,
};

#[test]
fn snapshot_is_revisioned_immutable_and_query_ready() {
    let revision = DocumentRevision::INITIAL.checked_next().unwrap();
    let first = parse_markdown(
        revision,
        SourceText::new("# one\n").unwrap(),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    assert_eq!(first.revision(), revision);
    assert_eq!(first.text().shared(), "# one\n");
    assert_eq!(first.tree().write().unwrap(), "# one\n");
    assert_eq!(first.diagnostics().as_ref(), first.tree().diagnostics());

    let update = reparse_markdown(
        &first,
        revision.checked_next().unwrap(),
        SourceText::new("# two\n").unwrap(),
        &[TextChange {
            old_range: TextRange::new(TextSize::new(2), TextSize::new(5)).unwrap(),
            replacement: "two".into(),
        }],
    )
    .unwrap();
    assert_eq!(update.snapshot.text().shared(), "# two\n");
    assert!(!update.affected_ranges.is_empty());
    assert!(matches!(
        update.outcome,
        MarkdownReparseOutcome::Incremental { .. }
            | MarkdownReparseOutcome::Full { .. }
    ));
}
```

- [ ] **Step 2: Run the contract test and verify the red result**

Run: `rtk cargo test -p waml-syntax --test markdown_snapshot snapshot_is_revisioned_immutable_and_query_ready`

Expected: FAIL with unresolved imports for `DocumentRevision`, `parse_markdown`, and `MarkdownSyntaxSnapshot`.

- [ ] **Step 3: Replace the dialect enum with explicit profiles and add revision identity**

In `text.rs`, define:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MarkdownDialect {
    bits: u8,
}

impl MarkdownDialect {
    const TABLES: u8 = 1 << 0;
    const TASK_LISTS: u8 = 1 << 1;
    const STRIKETHROUGH: u8 = 1 << 2;
    const EXTENDED_AUTOLINKS: u8 = 1 << 3;
    const TAG_FILTER: u8 = 1 << 4;
    const WAML_FRONTMATTER: u8 = 1 << 5;
    const WAML_SECTIONS: u8 = 1 << 6;

    pub const COMMONMARK_0_31_2: Self = Self { bits: 0 };
    pub const WAML_DEFAULT: Self = Self {
        bits: Self::TABLES
            | Self::TASK_LISTS
            | Self::STRIKETHROUGH
            | Self::EXTENDED_AUTOLINKS
            | Self::TAG_FILTER
            | Self::WAML_FRONTMATTER
            | Self::WAML_SECTIONS,
    };

    pub(crate) const fn contains(self, flag: u8) -> bool {
        self.bits & flag != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DocumentRevision(u64);

impl DocumentRevision {
    pub const INITIAL: Self = Self(0);
    pub const fn new(value: u64) -> Self { Self(value) }
    pub const fn get(self) -> u64 { self.0 }
    pub fn checked_next(self) -> Option<Self> { self.0.checked_add(1).map(Self) }
}

```

Make the seven dialect flag accessors `pub(crate)` with names `tables`, `task_lists`, `strikethrough`, `extended_autolinks`, `tag_filter`, `waml_frontmatter`, and `waml_sections`. Replace every `MarkdownDialect::CommonMarkCurrent` call with `MarkdownDialect::WAML_DEFAULT`.

In `markdown/kind.rs`, define the public identity plus the crate-private allocator used by every Markdown semantic-node builder:

```rust
static NEXT_MARKDOWN_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntaxIdentity(NonZeroU64);

impl SyntaxIdentity {
    pub fn get(self) -> u64 { self.0.get() }

    pub(crate) fn fresh() -> Result<Self, ParseError> {
        let value = NEXT_MARKDOWN_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| ParseError::StructuralInvariant {
                reason: "Markdown syntax identity space exhausted".into(),
            })?;
        Ok(Self(NonZeroU64::new(value).expect("identity starts at one")))
    }

    pub(crate) fn annotation(self) -> SyntaxAnnotation {
        SyntaxAnnotation::new(
            NonZeroU64::MAX,
            "waml.markdown.identity",
            Some(Arc::<str>::from(self.get().to_string())),
        )
    }
}
```

The annotation’s reserved ID and kind identify system metadata; the decimal data contains the per-node identity. User-created annotation IDs therefore cannot collide with a Markdown identity value.

- [ ] **Step 4: Define the complete outer-language kind set**

Move `OkfMarkdownLanguage`, `OkfMarkdownSyntaxKind`, and `OkfSyntaxDiagnosticCode` to `markdown/kind.rs`. Use node kinds for `Root`, `Frontmatter`, `FrontmatterEntry`, `BlockQuote`, `List`, `ListItem`, `Paragraph`, `AtxHeading`, `SetextHeading`, `ThematicBreak`, `IndentedCodeBlock`, `FencedCodeBlock`, `HtmlBlock`, `LinkReferenceDefinition`, `Table`, `TableHead`, `TableBody`, `TableRow`, `TableCell`, `Text`, `Escape`, `Entity`, `CodeSpan`, `Emphasis`, `StrongEmphasis`, `Strikethrough`, `Link`, `Image`, `Autolink`, `RawHtml`, `SoftLineBreak`, `HardLineBreak`, `WamlSection`, and `SkippedTokensSyntax`.

Use token kinds for `BomToken`, `FrontmatterFenceToken`, `FrontmatterKeyToken`, `ColonToken`, `FrontmatterValueToken`, `BlockQuoteMarkerToken`, `ListMarkerToken`, `TaskListMarkerToken`, `HeadingMarkerToken`, `SetextUnderlineToken`, `ThematicBreakToken`, `IndentToken`, `CodeFenceToken`, `InfoStringToken`, `CodeTextToken`, `HtmlToken`, `LinkLabelOpenToken`, `LinkLabelCloseToken`, `LinkDestinationOpenToken`, `LinkDestinationToken`, `LinkDestinationCloseToken`, `LinkTitleToken`, `TablePipeToken`, `TableAlignmentColonToken`, `TextToken`, `BackslashToken`, `EntityToken`, `CodeDelimiterToken`, `EmphasisDelimiterToken`, `StrikethroughDelimiterToken`, `ImageBangToken`, `AutolinkOpenToken`, `AutolinkCloseToken`, `WhitespaceToken`, `NewlineToken`, `EndOfFileToken`, and `BadToken`.

Keep the five current diagnostic codes and add `MalformedBlock`, `MalformedInline`, `UnclosedFence`, `UnclosedLink`, `MalformedTable`, and `FilteredHtmlTag`.

- [ ] **Step 5: Add the immutable snapshot structs and provisional delegates**

In `snapshot.rs`, define private snapshot fields plus getters:

```rust
pub struct MarkdownSyntaxSnapshot {
    revision: DocumentRevision,
    text: SourceText,
    tree: Arc<SyntaxTree<OkfMarkdownLanguage>>,
    structure: Arc<MarkdownStructureMap>,
    diagnostics: Arc<[TreeDiagnostic<OkfSyntaxDiagnosticCode>]>,
    queries: Arc<MarkdownSyntaxQueries>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MarkdownReparseOutcome {
    Incremental {
        shared_source_independent_green: usize,
        reparsed_range: Option<TextRange>,
    },
    Full { reason: FullReparseReason },
}

#[derive(Clone)]
pub struct MarkdownSyntaxUpdate {
    pub snapshot: Arc<MarkdownSyntaxSnapshot>,
    pub affected_ranges: Arc<[TextRange]>,
    pub outcome: MarkdownReparseOutcome,
}
```

Add `ParseError::NonMonotonicRevision { previous: DocumentRevision, requested: DocumentRevision }`. For this task, have `parse_markdown` wrap the current shell parse and have `reparse_markdown` wrap `reparse_okf_markdown_with_structure`; create an empty `MarkdownSyntaxQueries`. Normalize the old incremental range into a one-element `affected_ranges`. This bridge must be deleted in Task 8.

Define the bridge query type in `snapshot.rs` so Task 1 compiles:

```rust
#[derive(Default)]
pub struct MarkdownSyntaxQueries;
```

Move the current `ConfirmedHeading`, `MarkdownStructureMap`, and raw-map implementation from the deleted `markdown.rs` into `markdown/mod.rs` unchanged. Task 5 replaces that provisional raw-map implementation with the tree-derived projection.

- [ ] **Step 6: Export the contract and rerun tests**

Re-export all Task 1 types and functions from `lib.rs`. Keep `parse_okf_markdown` and `reparse_okf_markdown_with_structure` as hidden compatibility delegates until Task 9.

Run: `rtk cargo test -p waml-syntax --test markdown_snapshot && rtk cargo test -p waml-syntax --tests`

Expected: PASS; existing shell and incremental tests also pass under `WAML_DEFAULT`.

- [ ] **Step 7: Commit the public contract**

```bash
rtk git add crates/waml-syntax/src crates/waml-syntax/tests/markdown_snapshot.rs
rtk git commit -m "feat(markdown): define snapshot contract"
```

### Task 2: Build the Lossless CommonMark Block Phase

**Files:**
- Create: `crates/waml-syntax/src/markdown/block.rs`
- Create: `crates/waml-syntax/tests/markdown_blocks.rs`
- Modify: `crates/waml-syntax/src/markdown/mod.rs`
- Modify: `crates/waml-syntax/src/markdown/kind.rs`

**Interfaces:**
- Consumes: `MarkdownDialect`, `GreenFactory`, `SourceText`, and the Task 1 kinds.
- Produces: `BlockParse { root, diagnostics, inline_roots, definitions }`, exact block nodes/tokens, and `pulldown_options(MarkdownDialect)`.

- [ ] **Step 1: Add an exhaustive block-shape and explicit-option test**

Use one fixture containing a BOM, block quote, ordered and bullet lists, ATX and Setext headings, thematic break, paragraph, indented code, fenced code with info text, HTML block, and link definition. Assert exact write-back, ordered node kinds, and exact marker token spellings. Add:

```rust
#[test]
fn dialect_does_not_enable_unrequested_extensions() {
    let tree = parse("[^x]\n\n[^x]: note\n\nterm\n: definition\n\n$math$")
        .tree()
        .clone();
    let kinds = node_kinds(&tree);
    assert!(!kinds.iter().any(|kind| format!("{kind:?}").contains("Footnote")));
    assert!(!kinds.iter().any(|kind| format!("{kind:?}").contains("DefinitionList")));
    assert!(!kinds.iter().any(|kind| format!("{kind:?}").contains("Math")));
    assert_eq!(tree.write().unwrap(), "[^x]\n\n[^x]: note\n\nterm\n: definition\n\n$math$");
}
```

- [ ] **Step 2: Run the block test and verify it fails**

Run: `rtk cargo test -p waml-syntax --test markdown_blocks`

Expected: FAIL because the shell still emits `MarkdownRegion` and does not expose the required block kinds.

- [ ] **Step 3: Map only the approved parser options**

Implement:

```rust
fn pulldown_options(dialect: MarkdownDialect) -> pulldown_cmark::Options {
    let mut options = pulldown_cmark::Options::empty();
    if dialect.tables() { options.insert(pulldown_cmark::Options::ENABLE_TABLES); }
    if dialect.strikethrough() {
        options.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
    }
    if dialect.task_lists() {
        options.insert(pulldown_cmark::Options::ENABLE_TASKLISTS);
    }
    options
}
```

Do not use `ENABLE_GFM`: in pulldown-cmark 0.12.2 it also enables GFM alert block quotes, which this design excludes.

- [ ] **Step 4: Implement a bounded event-to-block stack**

Create `BlockFrame { kind, source_range, children, cursor }`. Consume `Parser::new_ext(source, options).into_offset_iter()` and map start/end events to the exact block kinds. For each child range, emit the uncovered source interval before the child as typed marker, whitespace, newline, or `BadToken`; then append the child. On frame close, emit the uncovered tail and verify that child widths equal the frame width.

Close each semantic block with `GreenFactory::node_with_annotations` and `Arc::from([SyntaxIdentity::fresh()?.annotation()])`. This gives Tasks 3-9 a real owner identity before the public query index exists.

Use these marker rules:

| Block | Marker token |
|---|---|
| block quote | `>` plus its following optional space |
| bullet/ordered list item | the bullet or digit/delimiter source slice |
| ATX heading | opening `#` run and optional closing `#` run |
| Setext heading | full `=` or `-` underline |
| thematic break | the confirmed complete break line |
| indented code | each four-space or tab indentation prefix |
| fenced code | opening and closing backtick/tilde runs |
| link definition | label brackets, colon, destination delimiters, and title delimiters |

Keep text source-backed. Use static/owned green text only for zero-width missing tokens.

- [ ] **Step 5: Enforce progress and recover all block bytes**

If an event range is reversed, outside the source, or starts before the current frame cursor, emit a non-empty `BadToken` for the next Unicode scalar and `MalformedBlock`. If EOF closes an open fence, keep the source bytes, omit a closing token, and add `UnclosedFence` at EOF. Return `ParserStalled` only when neither a token nor a Unicode scalar was consumed.

- [ ] **Step 6: Run focused and legacy lossless tests**

Run: `rtk cargo test -p waml-syntax --test markdown_blocks && rtk cargo test -p waml-syntax --test shell_roundtrip`

Expected: PASS with byte-exact write-back for all old shell fixtures and all new block forms.

- [ ] **Step 7: Commit the block phase**

```bash
rtk git add crates/waml-syntax/src/markdown crates/waml-syntax/tests/markdown_blocks.rs crates/waml-syntax/tests/shell_roundtrip.rs
rtk git commit -m "feat(markdown): parse lossless block structure"
```

### Task 3: Add Reference Definitions and the Lossless Inline Phase

**Files:**
- Create: `crates/waml-syntax/src/markdown/reference.rs`
- Create: `crates/waml-syntax/src/markdown/inline.rs`
- Create: `crates/waml-syntax/tests/markdown_inlines.rs`
- Modify: `crates/waml-syntax/src/markdown/block.rs`
- Modify: `crates/waml-syntax/src/markdown/mod.rs`

**Interfaces:**
- Consumes: `BlockParse::inline_roots` and link-definition nodes from Task 2.
- Produces: `MarkdownReferenceMap`, inline-bearing block replacement, and backlinks from normalized labels to owning inline blocks.

- [ ] **Step 1: Write inline and reference-resolution tests**

Cover text, escapes, named/decimal/hex entities, variable-length code spans, emphasis, strong emphasis, nested emphasis, links, images, full/collapsed/shortcut references, angle autolinks, raw HTML, soft breaks, two-space hard breaks, and backslash hard breaks. Assert that:

```rust
let snapshot = parse("[a][id] and ![b][]\n\n[id]: /one \"title\"\n");
let links: Vec<_> = snapshot.queries().links().collect();
assert_eq!(links[0].destination.as_ref(), "/one");
assert_eq!(links[0].destination_range, Some(range_of("/one")));
assert_eq!(links[0].kind, MarkdownLinkKind::Reference);
assert_eq!(snapshot.tree().write().unwrap(), snapshot.text().shared());
```

Add a duplicate-definition case that proves the first normalized definition wins and a Unicode/case/whitespace label case that proves CommonMark label normalization.

- [ ] **Step 2: Run the inline tests and verify they fail**

Run: `rtk cargo test -p waml-syntax --test markdown_inlines`

Expected: FAIL because paragraph and heading contents are still raw block children and `MarkdownReferenceMap` is missing.

- [ ] **Step 3: Build the immutable reference map before inline parsing**

Define:

```rust
#[derive(Clone, Debug)]
pub struct MarkdownReferenceDefinition {
    pub label: Arc<str>,
    pub owner: SyntaxIdentity,
    pub source_range: TextRange,
    pub destination: Arc<str>,
    pub destination_range: TextRange,
    pub title: Option<Arc<str>>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct MarkdownReferenceMap {
    definitions: Arc<HashMap<Arc<str>, MarkdownReferenceDefinition>>,
    backlinks: Arc<HashMap<Arc<str>, Arc<[SyntaxIdentity]>>>,
}
```

Normalize labels by trimming, collapsing internal Unicode whitespace to one ASCII space, applying Unicode lowercase, and rejecting labels longer than 999 characters. Insert only the first definition for a normalized label.

- [ ] **Step 4: Parse inline-bearing source ranges with a delimiter stack**

Use a cursor plus delimiter records `{ marker, range, can_open, can_close }`. Apply CommonMark left/right-flanking rules, the rule of three for `*` and `_`, link/image bracket activation, code-span equal-run matching, and entity decoding for semantic metadata while retaining source spelling in `EntityToken`.

Build nested green nodes only after delimiter matches are known. Every gap becomes `TextToken`, `WhitespaceToken`, or `NewlineToken`. Preserve unmatched delimiters as marker tokens under `Text`, not as deleted syntax.

Give each semantic inline node the same `waml.markdown.identity` annotation shape as Task 2 block nodes.

- [ ] **Step 5: Resolve links without changing their source ownership**

Inline links take their destination from their own source range. Full/collapsed/shortcut references take the semantic destination and title from `MarkdownReferenceMap` but keep label/delimiter tokens under the link node. Add the owning inline block identity to `backlinks[normalized_label]` for Task 8.

- [ ] **Step 6: Run inline, block, and exact-write tests**

Run: `rtk cargo test -p waml-syntax --test markdown_inlines && rtk cargo test -p waml-syntax --test markdown_blocks && rtk cargo test -p waml-syntax --test shell_roundtrip`

Expected: PASS. The inline structure differs from the old tree, but every recovered source string is unchanged.

- [ ] **Step 7: Commit reference and inline parsing**

```bash
rtk git add crates/waml-syntax/src/markdown crates/waml-syntax/tests/markdown_inlines.rs
rtk git commit -m "feat(markdown): parse lossless inline structure"
```

### Task 4: Implement the Five Named GFM Extensions

**Files:**
- Create: `crates/waml-syntax/src/markdown/gfm.rs`
- Create: `crates/waml-syntax/tests/markdown_gfm.rs`
- Modify: `crates/waml-syntax/src/markdown/block.rs`
- Modify: `crates/waml-syntax/src/markdown/inline.rs`
- Modify: `crates/waml-syntax/src/markdown/kind.rs`

**Interfaces:**
- Consumes: explicit Task 1 dialect flags and Task 2/3 block/inline builders.
- Produces: table alignment, task states, strikethrough nodes, extended links, and raw-HTML tag-filter classification.

- [ ] **Step 1: Write one positive and one negative case per extension**

Test escaped pipes and code spans in tables, `[ ]`/`[x]`/`[X]` task markers only at the start of list-item paragraph content, double-tilde strikethrough, `www.`, `http://`, `https://`, and email extended autolinks with punctuation trimming, plus case-insensitive disallowed HTML tags. Parse the same input with `COMMONMARK_0_31_2` and prove all five constructs remain ordinary CommonMark structure there.

- [ ] **Step 2: Run GFM tests and verify the red result**

Run: `rtk cargo test -p waml-syntax --test markdown_gfm`

Expected: FAIL on missing table metadata, extended autolinks, and HTML classification.

- [ ] **Step 3: Add exact metadata enums**

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TableAlignment { None, Left, Center, Right }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskListState { Unchecked, Checked }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HtmlTagFilter { Allowed, Disallowed }
```

Keep these values in parser metadata keyed by the owning node identity. Task 6 publishes that metadata through `MarkdownTableCell`, `MarkdownList`, and `MarkdownRawHtml`.

- [ ] **Step 4: Complete tables, tasks, and strikethrough**

Use pulldown table/task/strikethrough events only when their exact dialect flags are on. Segment every `|`, delimiter-row `:`, task bracket, and `~~` run into its named token. An unfinished table remains a paragraph; an unfinished task marker remains text; one `~` remains text.

- [ ] **Step 5: Add extended-autolink recognition**

Scan only text nodes after CommonMark inline matching. Use ASCII scheme/email/domain rules from the GFM spec, reject a candidate inside code/raw HTML/existing links, and trim trailing `?`, `!`, `.`, `,`, `:`, `*`, `_`, `~`, unmatched `)`, and unmatched entity-like semicolon tails. Wrap the exact retained source range in `Autolink`; use an implicit `http://` semantic destination for `www.` without changing source tokens.

- [ ] **Step 6: Add tag-filter classification without execution**

For each raw HTML node, read the first ASCII tag name after `<`, optional `/`, and ASCII whitespace. Compare case-insensitively with the nine disallowed names. Set `HtmlTagFilter::Disallowed` and emit `FilteredHtmlTag` at the tag-name range. Keep the HTML source unchanged and do not escape, remove, or execute it.

- [ ] **Step 7: Run GFM and full syntax tests**

Run: `rtk cargo test -p waml-syntax --test markdown_gfm && rtk cargo test -p waml-syntax --tests`

Expected: PASS with no footnote, math, definition-list, heading-attribute, or alert nodes.

- [ ] **Step 8: Commit GFM support**

```bash
rtk git add crates/waml-syntax/src/markdown crates/waml-syntax/tests/markdown_gfm.rs
rtk git commit -m "feat(markdown): add named GFM extensions"
```

### Task 5: Preserve Frontmatter and Mark WAML Section Islands

**Files:**
- Create: `crates/waml-syntax/src/markdown/projection.rs`
- Create: `crates/waml-syntax/tests/markdown_extensions.rs`
- Modify: `crates/waml-syntax/src/markdown/block.rs`
- Modify: `crates/waml-syntax/src/markdown/mod.rs`
- Modify: `crates/waml-syntax/src/shell.rs`
- Modify: `crates/waml-syntax/tests/shell_roundtrip.rs`
- Modify: `crates/waml-syntax/tests/properties.rs`

**Interfaces:**
- Consumes: complete outer tree from Tasks 2-4.
- Produces: root frontmatter nodes, `WamlSectionKind`, `WamlLanguageIsland`, and a tree-derived `MarkdownStructureMap`.

- [ ] **Step 1: Write extension-boundary tests**

Cover BOM plus clean/unclosed frontmatter, a later thematic rule, WAML section names inside and outside quotes/lists/fences/HTML, and the exact recognized section names: `Attributes`, `Values`, `Slots`, `Relationships`, `Members`, `Layout`, `Nodes`, `Lifelines`, and `Messages`. Assert only container-free headings produce islands and each island body ends at the next heading of equal or lower level or EOF.

- [ ] **Step 2: Run the extension tests and verify they fail**

Run: `rtk cargo test -p waml-syntax --test markdown_extensions`

Expected: FAIL because `WamlSectionKind`, island owner identities, and tree-derived structure do not exist.

- [ ] **Step 3: Parse frontmatter before CommonMark blocks**

When `dialect.waml_frontmatter()` and the first post-BOM line is `---`, reuse the current repository rules for clean close fences, plausible unclosed-frontmatter recovery, entry keys, colons, values, and diagnostics. Exclude its exact range from the CommonMark block phase. A later `---` always remains CommonMark.

- [ ] **Step 4: Mark recognized container-free sections**

Define:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum WamlSectionKind {
    Attributes, Values, Slots, Relationships, Members,
    Layout, Nodes, Lifelines, Messages,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WamlLanguageIsland {
    pub owner: SyntaxIdentity,
    pub kind: WamlSectionKind,
    pub heading_range: TextRange,
    pub content_range: TextRange,
}
```

Wrap a recognized heading plus its body ownership metadata in `WamlSection`. Do not move or duplicate child source. A heading under a Markdown container cannot become `WamlSection`.

The `WamlSection` wrapper receives its own identity annotation; `WamlLanguageIsland::owner` is that wrapper identity, not the heading child identity.

- [ ] **Step 5: Derive the compatibility projection from nodes**

Move `ConfirmedHeading` and `MarkdownStructureMap` to `projection.rs`. Traverse the finished red tree once. Fill headings, nested headings, protected ranges, list-item lines, tab-indented item lines, opaque ranges, dialect, and `islands: Arc<[WamlLanguageIsland]>`. Preserve the current top-level H1/H2 and nested H3-H6 contracts. Remove the pulldown scan formerly in `markdown.rs`.

- [ ] **Step 6: Prove projection parity and exact source**

Run: `rtk cargo test -p waml-syntax --test markdown_extensions && rtk cargo test -p waml-syntax --test shell_roundtrip && rtk cargo test -p waml-syntax --test properties`

Expected: PASS. Existing structure-map fixture values remain equal, and the new island list contains only the recognized unprotected sections.

- [ ] **Step 7: Commit WAML extensions and projection**

```bash
rtk git add crates/waml-syntax/src/markdown crates/waml-syntax/src/shell.rs crates/waml-syntax/tests
rtk git commit -m "feat(markdown): derive WAML section projection"
```

### Task 6: Add Stable Syntax Identity and Presentation Queries

**Files:**
- Create: `crates/waml-syntax/src/markdown/query.rs`
- Create: `crates/waml-syntax/tests/markdown_queries.rs`
- Modify: `crates/waml-syntax/src/annotation.rs`
- Modify: `crates/waml-syntax/src/markdown/{block.rs,inline.rs,gfm.rs,projection.rs,snapshot.rs}`
- Modify: `crates/waml-syntax/src/lib.rs`

**Interfaces:**
- Consumes: completed tree, annotations, semantic metadata, diagnostics, and islands.
- Produces: identity annotations for the Task 1 `SyntaxIdentity`, `MarkdownSyntaxSpan`, the final `MarkdownSyntaxQueries`, and typed metadata records for specs 2-4.

- [ ] **Step 1: Write the public query contract test**

Parse a document with a heading, task list, table, link, image, fenced code, filtered HTML, malformed link, and WAML section. Assert non-overlapping exact spans cover all non-empty tokens and call every metadata lookup by its owner `SyntaxIdentity`.

- [ ] **Step 2: Run query tests and verify they fail**

Run: `rtk cargo test -p waml-syntax --test markdown_queries`

Expected: FAIL with unresolved `SyntaxIdentity`, `MarkdownSyntaxQueries`, and metadata record imports.

- [ ] **Step 3: Decode and preserve semantic-node identities**

Add `syntax_identity(&SyntaxNode<OkfMarkdownLanguage>) -> Option<SyntaxIdentity>` that finds annotation kind `waml.markdown.identity`, parses its decimal data into a non-zero `u64`, and rejects a second matching annotation. Keep that annotation when `rebase_unchanged_green` reuses a node and when `transfer_mapped_annotations` maps an unchanged occurrence. Reject a semantic node without exactly one identity annotation as `ParseError::StructuralInvariant` while a snapshot query index is built.

- [ ] **Step 4: Define the exact span roles**

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkdownSourceRole { Content, SyntaxMarker }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkdownSemanticRole {
    Document, Frontmatter, BlockQuote, List, ListItem, Paragraph, Heading,
    ThematicBreak, IndentedCode, FencedCode, HtmlBlock, LinkDefinition,
    Table, TableHead, TableBody, TableRow, TableCell, Text, Escape, Entity,
    CodeSpan, Emphasis, Strong, Strikethrough, Link, Image, Autolink,
    RawHtml, SoftBreak, HardBreak, TaskMarker, Whitespace, Recovery,
    WamlSection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarkdownSyntaxSpan {
    pub owner: SyntaxIdentity,
    pub range: TextRange,
    pub source_role: MarkdownSourceRole,
    pub semantic_role: MarkdownSemanticRole,
}
```

- [ ] **Step 5: Define exact typed metadata**

Add `MarkdownHeading { owner, range, content_range, level }`, `MarkdownList { owner, range, kind: MarkdownListKind, task: Option<TaskListState> }`, `MarkdownTableCell { owner, range, alignment }`, `MarkdownRawHtml { owner, range, filter }`, and:

```rust
pub struct MarkdownLink {
    pub owner: SyntaxIdentity,
    pub source_range: TextRange,
    pub content_range: TextRange,
    pub destination: Arc<str>,
    pub destination_range: Option<TextRange>,
    pub title: Option<Arc<str>>,
    pub kind: MarkdownLinkKind,
}

pub struct MarkdownImage {
    pub owner: SyntaxIdentity,
    pub source_range: TextRange,
    pub alt_range: TextRange,
    pub source: Arc<str>,
    pub source_definition_range: Option<TextRange>,
    pub title: Option<Arc<str>>,
    pub kind: MarkdownLinkKind,
}

pub struct FencedCodeInfo {
    pub owner: SyntaxIdentity,
    pub source_range: TextRange,
    pub fence_range: TextRange,
    pub info_range: Option<TextRange>,
    pub content_range: TextRange,
    pub info: Arc<str>,
    pub language: Option<Arc<str>>,
}
```

`MarkdownLinkKind` is `Inline | Reference | Autolink | ExtendedAutolink`. `MarkdownListKind` is `Bullet | Ordered { start: u64 }`.

- [ ] **Step 6: Build one immutable query index**

Use these exact public signatures:

```rust
pub fn spans(&self, range: TextRange)
    -> impl Iterator<Item = &MarkdownSyntaxSpan> + '_;
pub fn links(&self) -> impl Iterator<Item = &MarkdownLink> + '_;
pub fn images(&self) -> impl Iterator<Item = &MarkdownImage> + '_;
pub fn heading(&self, owner: SyntaxIdentity) -> Option<&MarkdownHeading>;
pub fn list(&self, owner: SyntaxIdentity) -> Option<&MarkdownList>;
pub fn table_cell(&self, owner: SyntaxIdentity) -> Option<&MarkdownTableCell>;
pub fn link(&self, owner: SyntaxIdentity) -> Option<&MarkdownLink>;
pub fn image(&self, owner: SyntaxIdentity) -> Option<&MarkdownImage>;
pub fn raw_html(&self, owner: SyntaxIdentity) -> Option<&MarkdownRawHtml>;
pub fn fenced_code(&self, owner: SyntaxIdentity) -> Option<&FencedCodeInfo>;
pub fn island(&self, owner: SyntaxIdentity) -> Option<&WamlLanguageIsland>;
pub fn diagnostics(&self, range: TextRange)
    -> impl Iterator<Item = &TreeDiagnostic<OkfSyntaxDiagnosticCode>> + '_;
pub fn has_recovery(&self, range: TextRange) -> bool;
```

Use sorted `Arc<[T]>` storage plus identity maps built once per snapshot. Queries return references or iterators and never parse source.

- [ ] **Step 7: Run query and identity-transfer tests**

Run: `rtk cargo test -p waml-syntax --test markdown_queries && rtk cargo test -p waml-syntax --test incremental mapped_annotations_preserve_node_and_token_occurrences`

Expected: PASS. Unchanged semantic owners keep the same `SyntaxIdentity` after a mapped edit.

- [ ] **Step 8: Commit the query API**

```bash
rtk git add crates/waml-syntax/src crates/waml-syntax/tests/markdown_queries.rs
rtk git commit -m "feat(markdown): expose syntax presentation queries"
```

### Task 7: Publish Complete Immutable Snapshots and Revision Failures

**Files:**
- Modify: `crates/waml-syntax/src/markdown/snapshot.rs`
- Modify: `crates/waml-syntax/src/markdown/mod.rs`
- Modify: `crates/waml-syntax/tests/markdown_snapshot.rs`
- Modify: `crates/waml-syntax/src/lib.rs`

**Interfaces:**
- Consumes: full parser, structure projection, query index, and tree diagnostics.
- Produces: final `parse_markdown` snapshot ownership and monotonic revision contract.

- [ ] **Step 1: Extend snapshot tests for ownership and stale revisions**

Assert `snapshot.text()` shares the exact `SourceText` allocation used by every source-backed green, `snapshot.structure()` and `snapshot.queries()` describe that same tree, and `reparse_markdown` rejects equal and lower revisions:

```rust
assert!(matches!(
    reparse_markdown(&first, first.revision(), first.text().clone(), &[]),
    Err(ParseError::NonMonotonicRevision { previous, requested })
        if previous == requested
));
```

- [ ] **Step 2: Run snapshot tests and verify the ownership test fails**

Run: `rtk cargo test -p waml-syntax --test markdown_snapshot`

Expected: FAIL while the Task 1 bridge still builds an empty query index or accepts an equal revision.

- [ ] **Step 3: Replace the Task 1 bridge with one full-snapshot constructor**

Make `parse_markdown` call the full block/inline parser once, derive references, projection, diagnostics, and queries from that result, verify `tree.write() == text.shared()`, and return `Arc<MarkdownSyntaxSnapshot>`. Keep fields private and add getters with these exact returns:

```rust
pub fn revision(&self) -> DocumentRevision;
pub fn text(&self) -> &SourceText;
pub fn tree(&self) -> &Arc<SyntaxTree<OkfMarkdownLanguage>>;
pub fn structure(&self) -> &Arc<MarkdownStructureMap>;
pub fn diagnostics(&self) -> &Arc<[TreeDiagnostic<OkfSyntaxDiagnosticCode>]>;
pub fn queries(&self) -> &MarkdownSyntaxQueries;
```

- [ ] **Step 4: Run snapshot and full syntax tests**

Run: `rtk cargo test -p waml-syntax --test markdown_snapshot && rtk cargo test -p waml-syntax --tests`

Expected: PASS. No published snapshot has a source/tree mismatch.

- [ ] **Step 5: Commit immutable snapshot publication**

```bash
rtk git add crates/waml-syntax/src/markdown crates/waml-syntax/src/lib.rs crates/waml-syntax/tests/markdown_snapshot.rs
rtk git commit -m "feat(markdown): publish immutable syntax snapshots"
```

### Task 8: Reparse Safe Blocks and Non-Contiguous Reference Dependents

**Files:**
- Create: `crates/waml-syntax/src/markdown/reparse.rs`
- Create: `crates/waml-syntax/tests/markdown_incremental.rs`
- Modify: `crates/waml-syntax/src/incremental.rs`
- Modify: `crates/waml-syntax/src/markdown/{block.rs,inline.rs,reference.rs,snapshot.rs,mod.rs}`
- Modify: `crates/waml-syntax/tests/incremental.rs`
- Modify: `crates/waml-syntax/tests/properties.rs`

**Interfaces:**
- Consumes: `ChangeMap`, prior snapshot/tree/reference backlinks, safe block roots, and full parser oracle.
- Produces: final `reparse_markdown` and normalized new-snapshot `affected_ranges`.

- [ ] **Step 1: Write deterministic incremental oracle cases**

Cover local paragraph text, a list marker, heading boundary, fence boundary, table delimiter, frontmatter fence, WAML section boundary, one changed link definition used in two non-contiguous paragraphs, a Unicode replacement, insertion at EOF, overlapping changes, and a new-text/change mismatch. For every case compare incremental and clean full parse for source, structural fingerprint, diagnostics, structure, queries, reference destinations, and island ownership.

- [ ] **Step 2: Run incremental tests and verify they fail**

Run: `rtk cargo test -p waml-syntax --test markdown_incremental`

Expected: FAIL because the bridge can report only the old shell window and cannot fan out reference-dependent ranges.

- [ ] **Step 3: Select the smallest safe block roots**

Map each old change range through `ChangeMap`. Walk ancestors from the touched token to the first paragraph, heading, table cell, list item, code block, HTML block, or link-definition block whose start/end synchronization lines remain unchanged. Escalate through list/quote/table containers when a marker, indentation, blank-line, fence, or delimiter-row boundary changed. Escalate to full parse with the existing named reasons when no safe root exists.

- [ ] **Step 4: Reparse block structure and reference fan-out**

Reparse selected block roots against the new source. Rebuild `MarkdownReferenceMap` only if a definition node, label, destination, or title changed. Compare old/new normalized definitions. For each changed label, add every new-snapshot inline block from old and new backlinks to the inline reparse set. Reparse those inline blocks even when they do not intersect the direct text changes.

- [ ] **Step 5: Rebase and splice unchanged greens**

Use `rebase_unchanged_green` on untouched old subtrees, splice new block/inline greens by complete syntax path, and then run `transfer_mapped_annotations`. Rebuild projection, diagnostics, and queries from the final tree. Reparse only WAML islands whose owner range changed; keep the identity of every other island.

- [ ] **Step 6: Normalize affected ranges and outcome**

Collect direct new block ranges, reference-dependent inline ranges, changed diagnostic ranges, and changed island ranges. Sort by `(start, end)`, remove empty duplicates, and merge overlapping or touching ranges. Return:

```rust
MarkdownReparseOutcome::Incremental {
    shared_source_independent_green,
    reparsed_range: (affected_ranges.len() == 1).then(|| affected_ranges[0]),
}
```

For a full fallback, set `affected_ranges` to the complete new source range and keep the exact `FullReparseReason`.

- [ ] **Step 7: Prove local reuse and named fallbacks**

Run: `rtk cargo test -p waml-syntax --test markdown_incremental && rtk cargo test -p waml-syntax --test incremental && rtk cargo test -p waml-syntax --test properties`

Expected: PASS. The two-reference case returns at least two affected ranges and `reparsed_range: None`; normal local text edits return incremental outcomes and reuse source-independent greens.

- [ ] **Step 8: Commit incremental snapshots**

```bash
rtk git add crates/waml-syntax/src crates/waml-syntax/tests
rtk git commit -m "feat(markdown): incrementally reparse safe blocks"
```

### Task 9: Move WAML Analysis and Language Islands onto Snapshots

**Files:**
- Modify: `crates/waml/src/analysis.rs`
- Modify: `crates/waml/src/okf/shell.rs`
- Modify: `crates/waml/src/uml/syntax/mod.rs`
- Modify: `crates/waml/src/uml/syntax/parser.rs`
- Modify: `crates/waml/src/uml/analysis.rs`
- Modify: `crates/waml/tests/incremental_analysis.rs`
- Modify: `crates/waml/tests/parser_platform_properties.rs`
- Modify: `crates/waml-syntax/src/shell.rs`
- Modify: `crates/waml-syntax/src/lib.rs`
- Delete: `crates/waml-syntax/src/shell/parser.rs`

**Interfaces:**
- Consumes: `MarkdownSyntaxSnapshot`, `MarkdownSyntaxUpdate`, `DocumentRevision`, tree-derived structure, and `WamlLanguageIsland`.
- Produces: one snapshot per WAML document and embedded-analysis reuse keyed by `(SyntaxIdentity, content_range)`.

- [ ] **Step 1: Write cross-crate snapshot-promotion tests**

In `incremental_analysis.rs`, assert a one-character document edit promotes the exact `Arc<MarkdownSyntaxSnapshot>` created by the syntax update, unchanged documents preserve their snapshot `Arc`, unchanged UML islands preserve their syntax-tree `Arc`, and a broken edited island does not remove unrelated island analyses.

- [ ] **Step 2: Run the cross-crate test and verify it fails**

Run: `rtk cargo test -p waml --test incremental_analysis`

Expected: FAIL because `OkfAnalysis` still stores a separate shell syntax set and structures map.

- [ ] **Step 3: Move revision identity to `waml-syntax`**

Delete the local `analysis::DocumentRevision` tuple struct and `pub use waml_syntax::DocumentRevision` from `waml::analysis`. Keep `DocumentVersion::revision()` unchanged for callers. Convert the session’s integer increment through `DocumentRevision::checked_next`.

- [ ] **Step 4: Store Markdown snapshots as the syntax set**

Replace `SyntaxSnapshot<OkfMarkdownLanguage>` plus the parallel structures map with `Arc<MarkdownSyntaxSnapshot>` per `DocumentId`. Initial analysis calls `parse_markdown`; changed documents call `reparse_markdown`; unchanged documents clone the prior snapshot `Arc`. `DomainAnalysisContext` obtains structure through each snapshot.

- [ ] **Step 5: Key embedded parsing by outer island ownership**

Change UML island discovery to consume `snapshot.structure().islands`. Map each `WamlSectionKind` to the existing `UmlSyntaxKind`. Reuse an embedded tree only when both `SyntaxIdentity` and mapped `content_range` survive. Pass only the island source range to the embedded parser. Remove its duplicate heading-name and protected-container scans.

- [ ] **Step 6: Remove low-level shell authority**

Delete `shell/parser.rs`. Stop exporting `parse_okf_markdown`, `reparse_okf_markdown`, and `reparse_okf_markdown_with_structure`. Keep `OkfMarkdownLanguage` as the public language marker; make `shell.rs` a compatibility module that only re-exports the type names needed inside the crate, then fold it into `markdown/kind.rs` if no internal import remains.

- [ ] **Step 7: Run syntax, domain, CLI, and LSP gates**

Run: `rtk cargo test -p waml-syntax --tests && rtk cargo test -p waml --tests && rtk cargo test -p waml-cli --tests`

Expected: PASS. Syntax, lowerers, formatter, CLI, and LSP use the same snapshot tree and projection.

- [ ] **Step 8: Commit the one-authority migration**

```bash
rtk git add crates/waml-syntax crates/waml/src crates/waml/tests
rtk git commit -m "refactor(markdown): consume one syntax snapshot"
```

### Task 10: Import and Run CommonMark and GFM Conformance Fixtures

**Files:**
- Create: `crates/waml-syntax/tests/markdown_conformance.rs`
- Create: `crates/waml-syntax/tests/fixtures/commonmark-0.31.2/spec.json`
- Create: `crates/waml-syntax/tests/fixtures/commonmark-0.31.2/LICENSE`
- Create: `crates/waml-syntax/tests/fixtures/commonmark-0.31.2/SOURCE.md`
- Create: `crates/waml-syntax/tests/fixtures/gfm-0.29/spec.txt`
- Create: `crates/waml-syntax/tests/fixtures/gfm-0.29/LICENSE`
- Create: `crates/waml-syntax/tests/fixtures/gfm-0.29/SOURCE.md`
- Modify: `crates/waml-syntax/Cargo.toml`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`

**Interfaces:**
- Consumes: official fixture Markdown/expected HTML and the production syntax/query API.
- Produces: offline conformance tests and fixture provenance.

- [ ] **Step 1: Add the offline fixture loader and a deliberately selected failing example**

Add `serde`/`serde_json` as `waml-syntax` dev dependencies. Define:

```rust
#[derive(serde::Deserialize)]
struct CommonMarkExample {
    markdown: String,
    html: String,
    example: u32,
    section: String,
}
```

The test adapter traverses syntax/query roles into a test-only `ConformanceEvent` sequence, writes canonical HTML from those events, and compares it with the fixture `html`. It also asserts exact source recovery before comparison. The production parser must not call this adapter.

- [ ] **Step 2: Run the selected example and verify it fails before fixture import**

Run: `rtk cargo test -p waml-syntax --test markdown_conformance commonmark_example_1`

Expected: FAIL because `spec.json` is absent.

- [ ] **Step 3: Import CommonMark 0.31.2 with provenance**

Import `https://spec.commonmark.org/0.31.2/spec.json`. Put the CommonMark specification’s CC-BY-SA-4.0 license text in `LICENSE`. In `SOURCE.md`, record version `0.31.2`, publication date `2024-01-28`, the exact URL, the UTC import date, this command, and the SHA-256 printed by it:

```powershell
rtk proxy pwsh -NoProfile -Command "Invoke-WebRequest https://spec.commonmark.org/0.31.2/spec.json -OutFile crates/waml-syntax/tests/fixtures/commonmark-0.31.2/spec.json; Get-FileHash -Algorithm SHA256 crates/waml-syntax/tests/fixtures/commonmark-0.31.2/spec.json"
```

Do not write the digest in `SOURCE.md` until it has been printed from the imported bytes.

- [ ] **Step 4: Import the published GFM 0.29 spec with provenance**

Import `https://raw.githubusercontent.com/github/cmark-gfm/0.29.0.gfm.13/test/spec.txt`. Put its CC-BY-SA-4.0 specification license in `LICENSE`. Record revision tag `0.29.0.gfm.13`, the raw URL, UTC import date, this command, and the printed SHA-256 in `SOURCE.md`:

```powershell
rtk proxy pwsh -NoProfile -Command "Invoke-WebRequest https://raw.githubusercontent.com/github/cmark-gfm/0.29.0.gfm.13/test/spec.txt -OutFile crates/waml-syntax/tests/fixtures/gfm-0.29/spec.txt; Get-FileHash -Algorithm SHA256 crates/waml-syntax/tests/fixtures/gfm-0.29/spec.txt"
```

The test loader extracts only the five extension sections: tables, task list items, strikethrough, autolinks, and disallowed raw HTML.

- [ ] **Step 5: Run all conformance cases**

Run: `rtk cargo test -p waml-syntax --test markdown_conformance -- --nocapture`

Expected: PASS for all CommonMark 0.31.2 examples and all examples in the five GFM extension sections. When a GFM core case differs, the test names the CommonMark 0.31.2 example that takes precedence instead of accepting both structures.

- [ ] **Step 6: Commit fixtures, licenses, and runner**

```bash
rtk git add Cargo.toml Cargo.lock crates/waml-syntax/Cargo.toml crates/waml-syntax/tests/markdown_conformance.rs crates/waml-syntax/tests/fixtures
rtk git commit -m "test(markdown): add CommonMark and GFM conformance"
```

### Task 11: Complete Recovery, Property, and Fuzz Oracles

**Files:**
- Create: `crates/waml-syntax/tests/markdown_recovery.rs`
- Modify: `crates/waml-syntax/tests/properties.rs`
- Modify: `fuzz/fuzz_targets/parse_write.rs`
- Modify: `fuzz/fuzz_targets/syntax_edits.rs`
- Create: `fuzz/seeds/parse_write/gfm-mixed.md`
- Create: `fuzz/seeds/parse_write/malformed-inline.md`
- Create: `fuzz/seeds/syntax_edits/references.md`
- Create: `fuzz/seeds/syntax_edits/tables-unicode.md`

**Interfaces:**
- Consumes: full and incremental snapshot APIs plus structural/query fingerprints.
- Produces: deterministic malformed-source matrix, randomized edit oracle, and bounded fuzz targets.

- [ ] **Step 1: Write the recovery matrix**

Include BOM, CRLF, tabs, Unicode, combining characters, mixed line endings, malformed/unclosed fences, links, emphasis, tables, raw HTML, frontmatter, and WAML headings inside protected containers. For each source, assert full parse succeeds, exact source writes back, all red ranges are UTF-8 boundaries, all non-empty bytes have one token owner, diagnostics stay in range, and every recovery node is visible through `has_recovery`.

- [ ] **Step 2: Run recovery tests and verify the first uncovered case fails**

Run: `rtk cargo test -p waml-syntax --test markdown_recovery`

Expected: FAIL on the first malformed construct whose diagnostic or recovery span is absent.

- [ ] **Step 3: Close recovery gaps without changing source**

For each red case, add only the missing `BadToken`, expected-kind zero-width missing token, `SkippedTokensSyntax`, and typed diagnostic needed to make forward progress. Never synthesize a visible delimiter or discard a source byte.

- [ ] **Step 4: Expand the randomized full/incremental oracle**

Generate one to eight insert/delete/replace/paste edits at valid UTF-8 boundaries over documents that combine references, lists, tables, HTML, frontmatter, and WAML sections. After each edit, compare clean full and incremental snapshots for exact text, structural fingerprint excluding identity IDs, diagnostics, structure, query roles/metadata, reference resolution, islands, and fallback reason.

- [ ] **Step 5: Update fuzz targets**

`parse_write` must call `parse_markdown(DocumentRevision::INITIAL, ...)`, assert exact write and bounded ranges, and traverse all queries. `syntax_edits` must advance revisions, apply decoded `TextChange` values, call `reparse_markdown`, compare with a clean full parse, and assert that every full outcome has a named reason.

- [ ] **Step 6: Run deterministic and bounded fuzz gates**

Run: `rtk cargo test -p waml-syntax --test markdown_recovery && rtk cargo test -p waml-syntax --test properties && rtk cargo test -p waml-syntax --test markdown_incremental`

Run: `rtk cargo fuzz run parse_write -- -runs=10000`

Run: `rtk cargo fuzz run syntax_edits -- -runs=10000`

Expected: all deterministic tests PASS; each fuzz target completes 10,000 runs with no panic, mismatch, hang, or unnamed fallback.

- [ ] **Step 7: Commit recovery and fuzz coverage**

```bash
rtk git add crates/waml-syntax/tests fuzz
rtk git commit -m "test(markdown): prove recovery and edit oracles"
```

### Task 12: Verify the Single Markdown Authority

**Files:**
- Modify: `crates/waml/tests/no_legacy_authority.rs`
- Modify: `docs/superpowers/specs/2026-07-31-markdown-syntax-platform-design.md`

**Interfaces:**
- Consumes: all prior tasks.
- Produces: dependency/authority guard and recorded verification evidence.

- [ ] **Step 1: Extend the authority guard**

Scan Rust production source and Cargo manifests. Fail if `waml-editor` calls Makepad `Markdown` parsing APIs for source presentation, if any `waml`/editor/LSP module creates `pulldown_cmark::Parser`, if production code uses a regex Markdown classifier, or if `MarkdownStructureMap` is constructed outside `waml-syntax::markdown::projection`.

- [ ] **Step 2: Run the guard and verify it catches a seeded forbidden string**

Temporarily add `// pulldown_cmark::Parser::new` to the guard’s in-memory fixture, not to production source.

Run: `rtk cargo test -p waml --test no_legacy_authority`

Expected: PASS because the test proves the fixture is rejected and the repository source contains no forbidden second authority.

- [ ] **Step 3: Run formatting, lint, and full automated tests**

Run: `rtk cargo fmt --all -- --check`

Run: `rtk cargo clippy --workspace --all-targets --all-features -- -D warnings`

Run: `rtk cargo test --workspace --all-features`

Expected: all commands PASS with no warnings or failed tests.

- [ ] **Step 4: Record exact verification evidence**

In the design spec’s status line, change `written-spec review pending` to `implemented`. Append an `Implementation evidence` section with the implementation commit range, CommonMark/GFM passed example counts, the two 10,000-run fuzz commands, workspace test count, and the date. Use only values printed by the commands in Tasks 10-12.

- [ ] **Step 5: Commit the authority gate and evidence**

```bash
rtk git add crates/waml/tests/no_legacy_authority.rs docs/superpowers/specs/2026-07-31-markdown-syntax-platform-design.md
rtk git commit -m "test(markdown): enforce one syntax authority"
```

## Cross-Spec Handoff

- Spec 2 consumes `DocumentRevision`, `TextChange`, `MarkdownSyntaxSnapshot`, `MarkdownSyntaxUpdate`, `parse_markdown`, and `reparse_markdown`. Its widget-local document snapshot adds `LineIndex`, selection, undo, IME, and view state; it does not wrap a second parser.
- Spec 3 consumes `MarkdownSyntaxSnapshot::queries()`, `MarkdownSyntaxSpan`, `SyntaxIdentity`, and the typed heading/list/table/link/image/code/island metadata. A presentation fragment key is `(SyntaxIdentity, fragment_ordinal)`; `TextRange` remains edit and hit-test authority.
- Markdown query spans classify the outer WAML section and fenced-code ranges. They do not invent inner WAML token roles. Spec 3 gets inner WAML highlighting from the existing typed embedded `SyntaxTree<UmlLanguage>` that spec 4 publishes for the matching `WamlLanguageIsland`.
- Spec 4 promotes the exact `MarkdownSyntaxUpdate` proposed by spec 2 after it validates base revision and source identity. It does not call `parse_markdown` again for that accepted revision.
- `DocumentRevision` is per Markdown document. The application session revision remains a separate `u64`; no plan converts one identity into the other or compares them as if they shared a counter.
- There are exactly three syntax ingresses: `parse_markdown` for initial/application external replacement, `reparse_markdown` once in the widget-local edit transaction or for non-editor changed documents, and exact `Arc` promotion for an accepted editor proposal. These ingresses are mutually exclusive for one document revision.

## Plan Self-Review Record

- Spec coverage: Tasks 2-4 cover all named CommonMark/GFM blocks and inlines; Task 5 covers WAML extensions and tree-derived projection; Tasks 6-9 cover queries, snapshots, revisions, affected ranges, incremental reuse, and islands; Tasks 10-11 cover conformance, losslessness, recovery, randomized edits, and fuzzing; Task 12 covers single authority and final gates.
- Placeholder scan: the plan contains no deferred implementation marker or unspecified edge-case step. Fixture digests and test counts are deliberately recorded only from exact command output, so the plan does not invent values.
- Type consistency: `DocumentRevision`, `SyntaxIdentity`, `MarkdownSyntaxSnapshot`, `MarkdownSyntaxUpdate`, `MarkdownReparseOutcome`, `TextChange`, `TextRange`, and all query metadata use the same names and ownership in every task and handoff.
