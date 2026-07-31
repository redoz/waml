# Markdown Presentation and Motion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Present the canonical Markdown source as an editable, balanced document with visible syntax, semantic typography, embedded blocks, and one deterministic 100 ms geometry transition shared by text, selection, diagnostics, images, caret, and IME.

**Architecture:** Extend the `waml-markdown-editor` crate from the foundation plan. A pure presentation compiler consumes only `MarkdownSyntaxSnapshot` queries and produces immutable source-partitioned presentation items plus neutral layout hints. The foundation layout engine remains the only source-to-screen and hit-test authority; a motion controller interpolates its stable-identity snapshots once per frame, and every draw layer consumes that same interpolated snapshot. Application-owned hosts provide typed code highlighting and approved image data without adding a parser, LSP loop, file-system policy, or network policy to the editor crate.

**Tech Stack:** Rust 2021 with MSRV 1.80, `waml-syntax`, the Spec 2 `waml-markdown-editor` layout/session/widget foundation, Makepad `DrawText`/`DrawQuad`/`Image`/`NextFrame`/`Play`/`Ease`, existing WAML Atlas colors and IBM Plex font resources, and native HiDPI screenshot capture through `scripts/capture-window.ps1`.

## Global Constraints

- Implement after the incremental Markdown syntax platform and Markdown editor foundation plans. Do not add an application integration path; Spec 4 replaces the current Source surface.
- Canonical raw Markdown remains the sole editable source. Every source byte is visible, selectable, and mapped exactly once by text presentation.
- Presentation uses `MarkdownSyntaxSnapshot::queries()` only. Do not classify Markdown with regexes, `pulldown-cmark`, line-prefix checks, or a second parser.
- Syntax markers are dim, not hidden. Active markers change color only; they do not change font metrics, spacing, wrapping, or geometry.
- Use balanced document styling with 24 logical pixels on all four sides. Fill the available width inside that inset. Do not add a centered maximum-width column.
- Keep moderate heading sizes: H1 22 pt, H2 19 pt, H3 17 pt, H4 15.5 pt, H5 14.5 pt, H6 14 pt, against a 14 pt body.
- Keep literal task-list brackets, image source, table pipes, fences, info strings, raw HTML, malformed text, and all other source characters in normal edit flow.
- Raw HTML is never executed. Drawing never fetches a URL, opens a link, runs an LSP request, or writes a file.
- Fenced-code highlighting uses a registered application host. Unknown languages and failed highlighters use unclassified code style.
- Image resolution is application-authorized, asynchronous, and revision-bound. The editor accepts approved bytes or an approved canonical local path; it never calls Makepad's HTTP image loader.
- Use one motion clock and one interpolated layout snapshot. Draw order is block backgrounds, selection, text and markers, diagnostics/link decoration, embedded blocks, then caret/IME.
- The default transition is `0.100` seconds with Makepad `Ease::OutCubic`. Duration and ease remain live widget properties.
- Cut directly to target geometry for reduced motion, initial load, accepted external replacement, viewport resize, more than 4,096 changed source bytes, more than 256 changed visible geometry elements, unsafe or duplicate identity mapping, or changes wholly outside the viewport.
- Newly inserted target elements appear at target geometry immediately. Deleted elements disappear immediately. Only surviving elements with safe stable identities interpolate.
- Scroll anchoring derives every frame from the interpolated primary-caret geometry. Do not animate scroll independently from caret geometry.
- A failure in one block uses editable plain-text presentation for that block. It does not disable editing or presentation in other blocks.
- Keep `waml-markdown-editor` independent of Makepad's upstream `CodeEditor`, `Markdown`, and `TextFlow` widgets. Low-level Makepad draw, font, image, input, and timing primitives are allowed.
- Each task starts with a focused failing test, confirms the expected red result, adds the smallest production implementation, confirms green, and creates one review-sized commit. Do not weaken an assertion to obtain green.
- Prefix every shell command with `rtk`, as required by `RTK.md`.

---

## File Structure

```text
crates/waml-markdown-editor/
├── Cargo.toml
├── src/
│   ├── lib.rs                         # re-exports the public presentation/motion host seams
│   ├── widget.rs                      # foundation widget; gains presentation, draw, and frame scheduling
│   ├── layout/
│   │   ├── mod.rs                     # neutral block-layout contract added to foundation types
│   │   ├── engine.rs                  # consumes document inset and parsed block constraints
│   │   ├── geometry.rs                # unique geometry keys and interpolatable snapshots
│   │   └── makepad.rs                 # DrawText shaping adapter and matching paint cache
│   ├── presentation/
│   │   ├── mod.rs                     # immutable public presentation model
│   │   ├── compile.rs                 # syntax-query-to-presentation compiler
│   │   ├── style.rs                   # balanced metric/color/block style roles
│   │   ├── layout.rs                  # PresentationPlan -> foundation LayoutDocument
│   │   ├── highlight.rs               # revisioned registered code-highlighter host
│   │   ├── assets.rs                  # revision-bound approved image state machine
│   │   └── draw.rs                    # strict six-layer renderer over one frame snapshot
│   └── motion.rs                      # cut policy, stable matching, easing, scroll anchor
└── tests/
    ├── presentation_model.rs          # exact source partition and stable item identities
    ├── presentation_constructs.rs     # CommonMark/GFM treatment and malformed-source goldens
    ├── presentation_style.rs          # metric tokens, active markers, inset, DPI invariants
    ├── presentation_layout.rs         # lists, quotes, code, tables, images, resize, virtualization
    ├── highlighting.rs                # registry, clipping, unknown/failure fallback
    ├── assets.rs                      # async/revision policy and measurement invalidation
    ├── motion.rs                      # deterministic 0/50/100 ms frames and cutovers
    ├── draw_layers.rs                 # shared geometry and strict draw-order command model
    ├── fixtures/
    │   ├── presentation-all.md
    │   ├── malformed.md
    │   ├── motion-before.md
    │   ├── motion-after.md
    │   └── checker.svg
    └── goldens/
        ├── presentation-all.plan
        └── malformed.plan
crates/waml-editor/
├── Cargo.toml                         # adds the new crate for the visual harness
├── src/bin/markdown_presentation_harness.rs
└── tests/README.md                    # fixed visual cases and capture commands
```

The existing foundation files remain responsible for editing, revision checks, Unicode navigation, selection, IME, hit-testing, scroll state, and base layout. Presentation files do not mutate source or own a second selection/caret model.

### Cross-Plan Interfaces

The syntax plan supplies these crate-root exports:

```rust
pub struct SyntaxIdentity(NonZeroU64);
impl SyntaxIdentity {
    pub fn get(self) -> u64;
}

pub struct MarkdownSyntaxSnapshot { /* immutable */ }
impl MarkdownSyntaxSnapshot {
    pub fn revision(&self) -> DocumentRevision;
    pub fn text(&self) -> &SourceText;
    pub fn queries(&self) -> &MarkdownSyntaxQueries;
}

pub struct MarkdownSyntaxUpdate {
    pub snapshot: Arc<MarkdownSyntaxSnapshot>,
    pub affected_ranges: Arc<[TextRange]>,
    pub outcome: MarkdownReparseOutcome,
}

impl MarkdownSyntaxQueries {
    pub fn spans(&self, range: TextRange)
        -> impl Iterator<Item = &MarkdownSyntaxSpan>;
    pub fn heading(&self, owner: SyntaxIdentity) -> Option<&MarkdownHeading>;
    pub fn list(&self, owner: SyntaxIdentity) -> Option<&MarkdownList>;
    pub fn table_cell(&self, owner: SyntaxIdentity) -> Option<&MarkdownTableCell>;
    pub fn link(&self, owner: SyntaxIdentity) -> Option<&MarkdownLink>;
    pub fn image(&self, owner: SyntaxIdentity) -> Option<&MarkdownImage>;
    pub fn fenced_code(&self, owner: SyntaxIdentity) -> Option<&FencedCodeInfo>;
    pub fn island(&self, owner: SyntaxIdentity) -> Option<&WamlLanguageIsland>;
    pub fn diagnostics(&self, range: TextRange)
        -> impl Iterator<Item = &TreeDiagnostic<OkfSyntaxDiagnosticCode>>;
    pub fn has_recovery(&self, range: TextRange) -> bool;
}
```

The foundation plan supplies these `waml-markdown-editor` interfaces:

```rust
pub struct LayoutElementId {
    pub owner: SyntaxIdentity,
    pub fragment_ordinal: u32,
}

pub struct TextMetrics {
    pub font: FontKey,
    pub font_size: f32,
    pub line_spacing: f32,
    pub weight: FontWeight,
    pub italic: bool,
}

pub struct LayoutTextRun {
    pub id: LayoutElementId,
    pub range: TextRange,
    pub metrics: TextMetrics,
}

pub struct MeasuredBlock {
    pub id: LayoutElementId,
    pub source_range: TextRange,
    pub size: DVec2,
    pub baseline: Option<f64>,
}

pub struct LayoutViewport {
    pub width: f64,
    pub height: f64,
    pub scroll_y: f64,
    pub overscan: f64,
}

pub enum LayoutInvalidation {
    Document,
    SyntaxUpdate(MarkdownSyntaxUpdate),
    ViewportWidth,
    BlockMeasurement(LayoutElementId),
}

pub struct ScrollAnchor {
    pub position: TextPosition,
    pub viewport_y: f64,
}
```

Spec 3 extends `LayoutDocument` with neutral block constraints in Task 3. That extension belongs in the foundation layout module because layout remains foundation authority; the presentation compiler only creates values.

---

### Task 1: Add the Immutable Presentation Model and Exact Source Partition

**Files:**
- Create: `crates/waml-markdown-editor/src/presentation/mod.rs`
- Create: `crates/waml-markdown-editor/tests/presentation_model.rs`
- Modify: `crates/waml-markdown-editor/src/lib.rs`

**Interfaces:**
- Consumes: `DocumentRevision`, `SyntaxIdentity`, `TextRange`, and `TextSize` from `waml-syntax`.
- Produces:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PresentationItemId {
    pub owner: SyntaxIdentity,
    pub role: PresentationRole,
    pub fragment_ordinal: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PresentationRole {
    Text(TextRole),
    Block(BlockDecorationRole),
    Embedded(EmbeddedBlockRole),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TextRole {
    Body,
    SyntaxMarker,
    Heading(u8),
    Emphasis,
    Strong,
    StrongEmphasis,
    Strikethrough,
    LinkLabel,
    LinkDestination,
    ListMarker,
    TaskMarker,
    QuoteMarker,
    InlineCode,
    CodeFence,
    CodeInfo,
    CodeContent,
    TableDelimiter,
    RawHtml,
    Frontmatter,
    Recovery,
    Whitespace,
    LineBreak,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextStyle {
    pub font: FontRole,
    pub size: FontSizeRole,
    pub weight: FontWeightRole,
    pub italic: bool,
    pub color: ColorRole,
    pub active_color: ColorRole,
    pub background: Option<ColorRole>,
    pub underline: bool,
    pub strikethrough: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PresentationItem {
    TextRun {
        id: PresentationItemId,
        range: TextRange,
        role: TextRole,
        style: TextStyle,
    },
    BlockDecoration {
        id: PresentationItemId,
        owner: SyntaxIdentity,
        source_range: TextRange,
        kind: BlockDecorationKind,
    },
    EmbeddedBlock {
        id: PresentationItemId,
        owner: SyntaxIdentity,
        source_range: TextRange,
        kind: EmbeddedBlockKind,
    },
}

#[derive(Clone, Debug)]
pub struct PresentationPlan {
    pub revision: DocumentRevision,
    pub source_len: TextSize,
    pub items: Arc<[PresentationItem]>,
    pub links: Arc<[PresentedLink]>,
}

impl PresentationPlan {
    pub fn validate_source_partition(&self) -> Result<(), PresentationError>;
    pub fn active_owners(&self, caret: TextSize) -> Arc<[SyntaxIdentity]>;
}
```

- `BlockDecorationKind` has exact variants `QuoteRule`, `InlineCodeFill`, `FencedCodeSurface`, `TableGrid { columns: u32 }`, `TableHeaderFill`, `TaskCheckbox { checked: bool }`, and `ThematicRule`.
- `EmbeddedBlockKind` initially has `Image { destination: Arc<str>, alt: Arc<str>, title: Option<Arc<str>> }`.
- `PresentationError` has `Gap { expected: TextSize, actual: TextSize }`, `Overlap { previous_end: TextSize, next: TextRange }`, `OutOfBounds { range: TextRange, len: TextSize }`, `DuplicateId(PresentationItemId)`, `UnsupportedHeadingLevel(u8)`, `RevisionMismatch { expected: DocumentRevision, actual: DocumentRevision, component: &'static str }`, and `Text(TextError)`. Implement `From<TextError>` so checked range construction can use `?`.

- [ ] **Step 1: Write the failing source-partition and identity tests**

Add exact tests that construct a three-run plan for `**a**\n`:

```rust
#[test]
fn text_runs_partition_every_source_byte_once() {
    let plan = plan_for_source(
        "**a**\n",
        [
            run(0..2, TextRole::SyntaxMarker, owner(1), 0),
            run(2..3, TextRole::Strong, owner(1), 1),
            run(3..5, TextRole::SyntaxMarker, owner(1), 2),
            run(5..6, TextRole::LineBreak, owner(2), 0),
        ],
    );
    assert_eq!(plan.validate_source_partition(), Ok(()));
}

#[test]
fn partition_rejects_gap_overlap_duplicate_and_out_of_bounds() {
    assert!(matches!(plan_with_ranges("abcd", [0..1, 2..4]).validate_source_partition(),
        Err(PresentationError::Gap { .. })));
    assert!(matches!(plan_with_ranges("abcd", [0..3, 2..4]).validate_source_partition(),
        Err(PresentationError::Overlap { .. })));
    assert!(matches!(plan_with_ranges("abcd", [0..5]).validate_source_partition(),
        Err(PresentationError::OutOfBounds { .. })));
    assert!(matches!(plan_with_duplicate_ids("abcd").validate_source_partition(),
        Err(PresentationError::DuplicateId(_))));
}
```

Also assert that block decorations and embedded blocks do not participate in the text partition and can overlap their owner's source range.

- [ ] **Step 2: Run the focused test and confirm red**

Run:

```powershell
rtk cargo test -p waml-markdown-editor --test presentation_model
```

Expected: FAIL because `presentation`, `PresentationPlan`, and `validate_source_partition` do not exist.

- [ ] **Step 3: Implement the exact model and validator**

Implement a single sorted pass over text runs:

```rust
pub fn validate_source_partition(&self) -> Result<(), PresentationError> {
    let mut expected = TextSize::try_from_usize(0)?;
    let mut ids = BTreeSet::new();
    for item in self.items.iter() {
        if !ids.insert(item.id()) {
            return Err(PresentationError::DuplicateId(item.id()));
        }
        let PresentationItem::TextRun { range, .. } = item else { continue };
        if range.end() > self.source_len {
            return Err(PresentationError::OutOfBounds {
                range: *range,
                len: self.source_len,
            });
        }
        if range.start() < expected {
            return Err(PresentationError::Overlap {
                previous_end: expected,
                next: *range,
            });
        }
        if range.start() > expected {
            return Err(PresentationError::Gap {
                expected,
                actual: range.start(),
            });
        }
        expected = range.end();
    }
    if expected != self.source_len {
        return Err(PresentationError::Gap {
            expected,
            actual: self.source_len,
        });
    }
    Ok(())
}
```

Use checked `TextRange` constructors in tests and production. Do not cast unchecked `usize` offsets into ranges.

- [ ] **Step 4: Run model tests and the crate gate**

```powershell
rtk cargo test -p waml-markdown-editor --test presentation_model
rtk cargo test -p waml-markdown-editor
```

Expected: PASS. The validator reports the exact first bad boundary for every invalid plan.

- [ ] **Step 5: Commit the model**

```powershell
rtk git add crates/waml-markdown-editor/src/lib.rs crates/waml-markdown-editor/src/presentation/mod.rs crates/waml-markdown-editor/tests/presentation_model.rs
rtk git commit -m "feat(markdown): add presentation model"
```

---

### Task 2: Compile Syntax Queries into Complete CommonMark/GFM Presentation

**Files:**
- Create: `crates/waml-markdown-editor/src/presentation/compile.rs`
- Create: `crates/waml-markdown-editor/tests/presentation_constructs.rs`
- Create: `crates/waml-markdown-editor/tests/fixtures/presentation-all.md`
- Create: `crates/waml-markdown-editor/tests/fixtures/malformed.md`
- Create: `crates/waml-markdown-editor/tests/goldens/presentation-all.plan`
- Create: `crates/waml-markdown-editor/tests/goldens/malformed.plan`
- Modify: `crates/waml-markdown-editor/src/presentation/mod.rs`

**Interfaces:**
- Consumes: `MarkdownSyntaxSnapshot::queries().spans(..)` and the heading/list/table/link/image/fenced-code/recovery metadata queries from Spec 1.
- Produces:

```rust
pub fn compile_presentation(
    snapshot: &MarkdownSyntaxSnapshot,
    styles: &PresentationStyles,
) -> Result<Arc<PresentationPlan>, PresentationError>;

pub fn render_plan_golden(plan: &PresentationPlan, text: &SourceText) -> String;
```

- `compile_presentation` never reads punctuation to infer Markdown. It may slice source only after a syntax query supplies the exact range.
- The compiler gathers all syntax span boundaries, sorts and deduplicates them, then emits one non-empty `TextRun` for each adjacent interval.
- Role precedence for overlapping query spans is exact: recovery, syntax marker, inline semantic role, block semantic role, document body. A syntax-marker span keeps its construct owner but always receives `TextRole::SyntaxMarker`, except code fences/info, list/task markers, quote markers, table delimiters, and line breaks, which receive their more specific visible marker role.
- Fragment ordinals increase in source order within one owner and presentation role. Decorations and embedded blocks use their own role ordinal and never alter text ordinals.

- [ ] **Step 1: Add the complete fixture and failing golden tests**

Write this exact construct coverage into `presentation-all.md`:

````markdown
# Heading *em* and **strong**

## Heading 2
### Heading 3 with ***strong emphasis***
#### Heading 4
##### Heading 5
###### Heading 6

Body with ~~strike~~, [label](./other.md#part), `inline`, and <kbd>raw</kbd>.

- bullet
  1. ordered
- [x] checked
- [ ] open

> quoted **text**

```waml
type: uml.class
```

| left | center | right |
| :--- | :----: | ----: |
| a | b | c |

![checker](checker.svg "fixture")

---
````

Write malformed cases into `malformed.md`: an unmatched `**`, incomplete link destination, incomplete image, unterminated fence, malformed table delimiter, and raw `<script>alert(1)</script>`.

The test must parse each fixture through `parse_markdown`, compile it, call `validate_source_partition`, and compare `render_plan_golden` byte-for-byte with the checked-in golden. The golden line format is:

```text
TEXT <start>..<end> owner=<u64> ordinal=<u32> role=<Debug TextRole> source=<escaped source>
BLOCK <start>..<end> owner=<u64> ordinal=<u32> kind=<Debug BlockDecorationKind>
EMBED <start>..<end> owner=<u64> ordinal=<u32> kind=<Debug EmbeddedBlockKind>
LINK <start>..<end> owner=<u64> destination=<escaped destination>
```

Assert these exact treatment rules in a table-driven test:

| Construct | Marker role | Content role | Extra item |
|---|---|---|---|
| H1-H6 | `SyntaxMarker` | `Heading(level)` | none |
| emphasis | `SyntaxMarker` | `Emphasis` | none |
| strong | `SyntaxMarker` | `Strong` | none |
| nested strong/emphasis | `SyntaxMarker` | `StrongEmphasis` | none |
| strikethrough | `SyntaxMarker` | `Strikethrough` | none |
| link brackets/destination | `SyntaxMarker` / `LinkDestination` | `LinkLabel` | `PresentedLink` |
| list | `ListMarker` | inherited content role | hanging block hint in Task 3 |
| task list | `TaskMarker` | inherited content role | non-interactive `TaskCheckbox` |
| quote | `QuoteMarker` | inherited content role | `QuoteRule` |
| inline code | `SyntaxMarker` | `InlineCode` | `InlineCodeFill` |
| fenced code | `CodeFence` / `CodeInfo` | `CodeContent` | `FencedCodeSurface` |
| table | `TableDelimiter` | inherited content role | `TableGrid` and header fill |
| image | normal source roles | normal source roles | one `EmbeddedBlock::Image` after source |
| thematic break | `SyntaxMarker` | none | `ThematicRule` |
| raw HTML | `RawHtml` | `RawHtml` | none |
| recovery | `Recovery` | `Recovery` | diagnostic only |

- [ ] **Step 2: Run construct tests and confirm red**

```powershell
rtk cargo test -p waml-markdown-editor --test presentation_constructs
```

Expected: FAIL because `compile_presentation` and the golden renderer do not exist.

- [ ] **Step 3: Implement boundary segmentation and role resolution**

Use the query spans as the only classifier:

```rust
let zero = TextSize::try_from_usize(0)?;
let full = TextRange::new(zero, snapshot.text().len())?;
let spans: Vec<_> = snapshot.queries().spans(full).collect();
let mut boundaries = vec![zero, snapshot.text().len()];
for span in &spans {
    boundaries.push(span.range.start());
    boundaries.push(span.range.end());
}
boundaries.sort_unstable();
boundaries.dedup();

for window in boundaries.windows(2) {
    let range = TextRange::new(window[0], window[1])?;
    if range.start() == range.end() {
        continue;
    }
    let classification = resolve_covering_spans(range, &spans, snapshot.queries());
    builder.push_text(range, classification);
}
```

Implement an exhaustive `match` over every `MarkdownSourceRole` and `MarkdownSemanticRole` from Spec 1. Map unknown future semantic roles to `Body` without losing the range. Do not use a wildcard for current variants; this keeps new syntax roles compile-visible.

- [ ] **Step 4: Add metadata-driven decorations, links, images, and recovery**

For each unique queried owner:

- add exactly one link record from `queries.link(owner)`;
- add exactly one image embed from `queries.image(owner)`;
- add quote/code/table/task/thematic decorations from semantic metadata;
- keep malformed or incomplete constructs as text; add no guessed link, image, table, or executable HTML item;
- use the table-cell alignment metadata rather than pipe text;
- use the fenced-code `language` and `content_range`; do not parse the info string.

Run `plan.validate_source_partition()` before returning success.

- [ ] **Step 5: Write and verify the two goldens**

Create the goldens manually from the query-produced ranges and roles. Then run:

```powershell
rtk cargo test -p waml-markdown-editor --test presentation_constructs
```

Expected: PASS. The raw `<script>` bytes appear as `RawHtml`; no embed or action represents them. Every malformed byte appears in a `Recovery` or ordinary text run.

- [ ] **Step 6: Commit the compiler**

```powershell
rtk git add crates/waml-markdown-editor/src/presentation crates/waml-markdown-editor/tests/presentation_constructs.rs crates/waml-markdown-editor/tests/fixtures crates/waml-markdown-editor/tests/goldens
rtk git commit -m "feat(markdown): compile semantic presentation"
```

---

### Task 3: Add Balanced Styles and Parsed Variable-Block Layout

**Files:**
- Create: `crates/waml-markdown-editor/src/presentation/style.rs`
- Create: `crates/waml-markdown-editor/src/presentation/layout.rs`
- Create: `crates/waml-markdown-editor/tests/presentation_style.rs`
- Create: `crates/waml-markdown-editor/tests/presentation_layout.rs`
- Modify: `crates/waml-markdown-editor/src/presentation/mod.rs`
- Modify: `crates/waml-markdown-editor/src/layout/mod.rs`
- Modify: `crates/waml-markdown-editor/src/layout/engine.rs`
- Modify: `crates/waml-markdown-editor/src/layout/geometry.rs`

**Interfaces:**
- Consumes and populates the exact neutral foundation layout types:

The following declarations repeat the foundation Task 7 contract for implementation clarity. Task 3 modifies their existing foundation definitions only to add behavior; it must not create presentation-owned duplicates or parallel geometry types.

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutBlock {
    pub id: LayoutElementId,
    pub source_range: TextRange,
    pub parent: Option<LayoutElementId>,
    pub spec: BlockLayoutSpec,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockLayoutSpec {
    pub flow: BlockFlow,
    pub insets: EdgeInsets,
    pub space_before: f64,
    pub space_after: f64,
    pub columns: Arc<[ColumnConstraint]>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BlockFlow {
    Paragraph,
    Hanging {
        marker_range: TextRange,
        content_indent: f64,
    },
    Quote,
    Code,
    Table,
    TableRow,
    TableCell { column: u32 },
    Embedded,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColumnConstraint {
    pub min_width: f64,
    pub max_width: Option<f64>,
    pub alignment: ColumnAlignment,
}

pub struct LayoutDocument {
    pub revision: DocumentRevision,
    pub content_insets: EdgeInsets,
    pub blocks: Arc<[LayoutBlock]>,
    pub text_runs: Arc<[LayoutTextRun]>,
    pub embedded_blocks: Arc<[MeasuredBlock]>,
}

#[derive(Clone, Debug, Default)]
pub struct EmbeddedMeasurements {
    pub blocks: Arc<[MeasuredBlock]>,
}

pub fn build_layout_document(
    plan: &PresentationPlan,
    styles: &PresentationStyles,
    measurements: &EmbeddedMeasurements,
) -> Result<LayoutDocument, PresentationError>;
```

`build_layout_document` copies `plan.revision` into `LayoutDocument::revision`. It rejects measurements from another revision before constructing blocks.

- `PresentationStyles::balanced()` uses these exact metrics:

| Role | Size | Weight | Italic | Line spacing |
|---|---:|---:|---|---:|
| body/markers/link/raw HTML | 14 pt | 400 | false | 1.35 |
| emphasis | 14 pt | 400 | true | 1.35 |
| strong | 14 pt | 600 | false | 1.35 |
| strong emphasis | 14 pt | 600 | true | 1.35 |
| H1 | 22 pt | 600 | false | 1.20 |
| H2 | 19 pt | 600 | false | 1.20 |
| H3 | 17 pt | 600 | false | 1.20 |
| H4 | 15.5 pt | 600 | false | 1.20 |
| H5 | 14.5 pt | 600 | false | 1.20 |
| H6 | 14 pt | 600 | false | 1.20 |
| inline/fenced code | 13 pt mono | 400 | false | 1.30 |

- Heading margins are `(before, after)` H1 `(20,10)`, H2 `(18,9)`, H3 `(16,8)`, H4 `(14,7)`, H5 `(12,6)`, H6 `(12,6)` logical pixels. Paragraph after-spacing is 12. Quote inset is 12 with a 3-pixel rule and 8-pixel content gap. List marker gap is 8. Code padding is 12. Table cell padding is 8 horizontal and 6 vertical.
- The document inset is always 24 logical pixels and is independent of DPI. The content width is `max(0, viewport.width - 48)`.

- [ ] **Step 1: Write failing style and layout-contract tests**

Add exact assertions:

```rust
#[test]
fn balanced_metrics_and_document_inset_are_fixed() {
    let styles = PresentationStyles::balanced();
    assert_eq!(styles.document_insets(), EdgeInsets { top: 24.0, right: 24.0, bottom: 24.0, left: 24.0 });
    assert_eq!(styles.text(TextRole::Body).metrics(), metrics(SANS, 14.0, 1.35, 400, false));
    assert_eq!(styles.text(TextRole::Heading(1)).metrics(), metrics(SANS, 22.0, 1.20, 600, false));
    assert_eq!(styles.text(TextRole::Heading(6)).metrics(), metrics(SANS, 14.0, 1.20, 600, false));
    assert_eq!(styles.text(TextRole::CodeContent).metrics(), metrics(MONO, 13.0, 1.30, 400, false));
}

#[test]
fn dpi_does_not_change_logical_inset() {
    for dpi in [1.0, 1.25, 1.5, 2.0] {
        let layout = layout_fixture(800.0, dpi);
        assert_eq!(layout.content_rect().pos.x, 24.0);
        assert_eq!(layout.content_rect().size.x, 752.0);
    }
}
```

Add layout tests for nested hanging lists, nested quotes, a wrapping mixed-style line, a three-column table with left/center/right alignment, a code block, and an image source line followed by its measured embed.

- [ ] **Step 2: Run focused tests and confirm red**

```powershell
rtk cargo test -p waml-markdown-editor --test presentation_style
rtk cargo test -p waml-markdown-editor --test presentation_layout
```

Expected: FAIL because balanced styles, block constraints, and `build_layout_document` do not exist.

- [ ] **Step 3: Implement style roles without Makepad objects**

Keep the pure style sheet deterministic and golden-testable. Map each `TextStyle` to foundation `TextMetrics` with fixed `FontKey` values:

```rust
pub const FONT_SANS: FontKey = FontKey(1);
pub const FONT_MONO: FontKey = FontKey(2);
pub const WEIGHT_REGULAR: FontWeight = FontWeight(400);
pub const WEIGHT_SEMIBOLD: FontWeight = FontWeight(600);
```

Color roles do not enter `TextMetrics`. `SyntaxMarker` uses `ColorRole::Marker` and `active_color: ColorRole::MarkerActive` with the same metrics. Link, diagnostic, selection, and background colors also stay out of layout cache keys.

Store the body style separately from the role map. `PresentationStyles::text(role)` returns that body style when a role has no configured entry. Add a test-only style builder that removes one role and prove that the affected range stays visible with body metrics instead of failing compilation.

- [ ] **Step 4: Build neutral block constraints from parsed owners**

Map presentation owners to foundation `BlockLayoutSpec` values:

- headings and paragraphs use `BlockFlow::Paragraph` with the exact spacing table;
- list items use `BlockFlow::Hanging`; set `marker_range` from syntax metadata and `content_indent` from the measured literal marker width plus the 8-pixel gap;
- quote nesting creates parented `Quote` blocks;
- fenced code uses `Code`;
- each parsed table creates one `Table`, parsed `TableRow` children, and parsed `TableCell { column }` children; put min/max widths and metadata alignment in the table's `columns`;
- an image owner keeps its literal source in a `Paragraph` block and adds a following child `Embedded` block with the matching `MeasuredBlock`;
- invalid table/list/image syntax stays `Paragraph`.

Do not inspect the text to decide a block kind.

- [ ] **Step 5: Extend the layout engine for the neutral constraints**

Apply document inset once at the root. Lay out each block from its parsed parent, metrics, and available width. Table layout uses:

1. each column's maximum measured min-content width;
2. equal distribution of remaining positive width;
3. proportional shrink down to min-content widths when constrained;
4. horizontal overflow only when the sum of min-content widths still exceeds content width.

Relayout from `LayoutInvalidation`'s first affected block and stop when the next unchanged `BlockSummary` has the same id, source range, width, y, and height. Keep only the visible block window plus 320 logical pixels of overscan as full glyph geometry; retain `BlockSummary` for the rest.

- [ ] **Step 6: Verify style, layout, hit-testing, resize, and virtualization**

```powershell
rtk cargo test -p waml-markdown-editor --test presentation_style
rtk cargo test -p waml-markdown-editor --test presentation_layout
rtk cargo test -p waml-markdown-editor --test layout_geometry
```

Expected: PASS. Source-to-screen-to-source round trips succeed at every delimiter boundary. Changing only marker active color produces byte-identical `LayoutSnapshot` geometry. Resizing invalidates wrapping but not presentation identity.

- [ ] **Step 7: Commit styles and block layout**

```powershell
rtk git add crates/waml-markdown-editor/src/presentation/style.rs crates/waml-markdown-editor/src/presentation/layout.rs crates/waml-markdown-editor/src/presentation/mod.rs crates/waml-markdown-editor/src/layout crates/waml-markdown-editor/tests/presentation_style.rs crates/waml-markdown-editor/tests/presentation_layout.rs
rtk git commit -m "feat(markdown): add balanced variable layout"
```

---

### Task 4: Add Registered, Snapshot-Based Code Highlighting

**Files:**
- Create: `crates/waml-markdown-editor/src/presentation/highlight.rs`
- Create: `crates/waml-markdown-editor/tests/highlighting.rs`
- Modify: `crates/waml-markdown-editor/src/presentation/compile.rs`
- Modify: `crates/waml-markdown-editor/src/presentation/mod.rs`
- Modify: `crates/waml-markdown-editor/src/lib.rs`

**Interfaces:**
- Produces the Spec 4 host seam:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeHighlightRequest {
    pub revision: DocumentRevision,
    pub owner: SyntaxIdentity,
    pub language: Arc<str>,
    pub content_range: TextRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodeTokenRole {
    Keyword,
    Type,
    Property,
    String,
    Number,
    Comment,
    Punctuation,
    Invalid,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeHighlightSpan {
    pub range: TextRange,
    pub role: CodeTokenRole,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeHighlightResult {
    pub revision: DocumentRevision,
    pub owner: SyntaxIdentity,
    pub spans: Arc<[CodeHighlightSpan]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodeHighlightError {
    OutOfBounds {
        content_range: TextRange,
        span: TextRange,
    },
    Overlap {
        previous_end: TextSize,
        next: TextRange,
    },
    InvalidUtf8Boundary(TextSize),
    StaleRevision {
        expected: DocumentRevision,
        actual: DocumentRevision,
    },
    WrongOwner {
        expected: SyntaxIdentity,
        actual: SyntaxIdentity,
    },
    Host(Arc<str>),
}

pub trait CodeHighlightHost {
    fn highlight(
        &self,
        request: &CodeHighlightRequest,
    ) -> Result<CodeHighlightResult, CodeHighlightError>;
}

pub struct HighlighterRegistry {
    hosts: BTreeMap<Arc<str>, Arc<dyn CodeHighlightHost>>,
}

impl HighlighterRegistry {
    pub fn register(
        &mut self,
        language: impl Into<Arc<str>>,
        host: Arc<dyn CodeHighlightHost>,
    ) -> Option<Arc<dyn CodeHighlightHost>>;
    pub fn highlight(
        &self,
        request: &CodeHighlightRequest,
        source: &SourceText,
    ) -> HighlightOutcome;
}
```

- Registry keys are trimmed ASCII-lowercase language identifiers supplied by parsed `FencedCodeInfo::language`. No text classifier computes them.
- `HighlightOutcome` is `Classified(Arc<[CodeHighlightSpan]>)`, `Unclassified`, or `Failed(CodeHighlightError)`.
- `PresentationStyles::code_token(CodeTokenRole)` maps each token role to colors and emphasis while preserving the base `CodeContent` font size, font family, and line spacing.
- Spec 4 implements WAML language hosts from the application-owned typed `SyntaxTree<UmlLanguage>` or equivalent immutable analysis snapshot, clipped to the requested content range. This crate does not parse the island and does not call LSP.

- [ ] **Step 1: Write failing registry, clipping, and fallback tests**

Test exact behavior:

```rust
#[test]
fn registered_host_receives_revision_owner_language_and_exact_content_range() {
    let host = RecordingHost::returning(result(revision(7), owner(3), [
        span(12..16, CodeTokenRole::Keyword),
    ]));
    let outcome = registry_with("waml", host.clone())
        .highlight(&request("WAML", 10..20), &source());
    assert_eq!(outcome, HighlightOutcome::Classified(arc([span(12..16, Keyword)])));
    assert_eq!(host.only_request(), request("waml", 10..20));
}

#[test]
fn unknown_failure_and_out_of_range_spans_use_unclassified_code() {
    assert_eq!(HighlighterRegistry::default()
        .highlight(&request("rust", 10..20), &source()),
        HighlightOutcome::Unclassified);
    assert!(matches!(failing_registry()
        .highlight(&request("waml", 10..20), &source()),
        HighlightOutcome::Failed(_)));
    assert!(matches!(out_of_range_registry()
        .highlight(&request("waml", 10..20), &source()),
        HighlightOutcome::Failed(CodeHighlightError::OutOfBounds { .. })));
}
```

Also assert that sorted non-overlapping spans are required and that gaps remain `CodeContent`.

- [ ] **Step 2: Run highlighting tests and confirm red**

```powershell
rtk cargo test -p waml-markdown-editor --test highlighting
```

Expected: FAIL because the registry and host seam do not exist.

- [ ] **Step 3: Implement registry normalization and result validation**

Reject a result for a different revision or owner. Reject highlight spans outside `content_range`, overlapping spans, reversed ranges, and non-UTF-8 boundaries. Keep errors local to the fenced block.

- [ ] **Step 4: Overlay valid highlight roles during presentation compilation**

Replace Task 2's compiler entry point with:

```rust
pub fn compile_presentation(
    snapshot: &MarkdownSyntaxSnapshot,
    styles: &PresentationStyles,
    highlighters: &HighlighterRegistry,
) -> Result<Arc<PresentationPlan>, PresentationError>;
```

Pass `HighlighterRegistry::default()` at callers that do not install a host. Add highlight boundaries to the same source segmentation pass from Task 2. Highlight roles override only `CodeContent`, never fences, info strings, syntax markers, recovery spans, or source mapping. On unknown language or any host error, emit the original `CodeContent` run and retain a non-fatal presentation diagnostic for that block.

- [ ] **Step 5: Run tests and architecture scans**

```powershell
rtk cargo test -p waml-markdown-editor --test highlighting
rtk cargo test -p waml-markdown-editor --test presentation_constructs
rtk rg -n 'pulldown_cmark::Parser|regex::|tower_lsp|lsp' crates/waml-markdown-editor/src/presentation
```

Expected: PASS. The scan returns no production match.

- [ ] **Step 6: Commit highlighting**

```powershell
rtk git add crates/waml-markdown-editor/src/lib.rs crates/waml-markdown-editor/src/presentation crates/waml-markdown-editor/tests/highlighting.rs
rtk git commit -m "feat(markdown): add snapshot code highlighting"
```

---

### Task 5: Add Revision-Bound Image Assets and Embedded Measurements

**Files:**
- Create: `crates/waml-markdown-editor/src/presentation/assets.rs`
- Create: `crates/waml-markdown-editor/tests/assets.rs`
- Create: `crates/waml-markdown-editor/tests/fixtures/checker.svg`
- Modify: `crates/waml-markdown-editor/src/presentation/layout.rs`
- Modify: `crates/waml-markdown-editor/src/lib.rs`
- Modify: `crates/waml-markdown-editor/src/widget.rs`

**Interfaces:**
- Produces the Spec 4 host seam:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AssetRequestId(pub u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageAssetRequest {
    pub request_id: AssetRequestId,
    pub revision: DocumentRevision,
    pub item: PresentationItemId,
    pub source_range: TextRange,
    pub destination: Arc<str>,
}

pub trait MarkdownAssetHost {
    fn request_image(&mut self, request: ImageAssetRequest);
    fn cancel_image(&mut self, request_id: AssetRequestId);
    fn drain_events(&mut self) -> Vec<ImageAssetEvent>;
}

#[derive(Clone)]
pub enum ApprovedImageSource {
    Bytes {
        cache_key: Arc<str>,
        media_type: ImageMediaType,
        data: Arc<[u8]>,
        pixel_size: (u32, u32),
    },
    CanonicalFile {
        path: Arc<PathBuf>,
        pixel_size: (u32, u32),
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageMediaType {
    Svg,
    Png,
    Jpeg,
}

pub enum ImageAssetEvent {
    Ready {
        request_id: AssetRequestId,
        revision: DocumentRevision,
        item: PresentationItemId,
        source: ApprovedImageSource,
    },
    Failed {
        request_id: AssetRequestId,
        revision: DocumentRevision,
        item: PresentationItemId,
        message: Arc<str>,
    },
}

pub enum EmbeddedState {
    Loading,
    Ready { source: ApprovedImageSource },
    Failed { message: Arc<str> },
}

pub enum AssetEventOutcome {
    Applied {
        invalidation: Option<LayoutInvalidation>,
    },
    IgnoredStale,
}

#[derive(Clone)]
pub struct EmbeddedAssetFrame {
    pub revision: DocumentRevision,
    pub items: Arc<[(PresentationItemId, EmbeddedState)]>,
}

impl EmbeddedAssets {
    pub fn reconcile(
        &mut self,
        host: &mut dyn MarkdownAssetHost,
        plan: &PresentationPlan,
    );
    pub fn retry(
        &mut self,
        host: &mut dyn MarkdownAssetHost,
        plan: &PresentationPlan,
        item: PresentationItemId,
    ) -> bool;
    pub fn apply_event(&mut self, event: ImageAssetEvent) -> AssetEventOutcome;
    pub fn measurements(&self, available_width: f64) -> EmbeddedMeasurements;
    pub fn frame(&self, plan: &PresentationPlan) -> Arc<EmbeddedAssetFrame>;
}
```

- Loading placeholder size is `min(240, available_width) x 72` logical pixels.
- Failure placeholder size is `min(320, available_width) x 48`.
- Loading and failure placeholders show the parsed alt text. Activating only the failure placeholder emits the foundation `EmbeddedBlockEvent::RetryRequested`. The widget emits the stable `LayoutElementId`; the application-owned `SourceView` maps it to `PresentationItemId` and calls `EmbeddedAssets::retry` with its application host. The widget does not own `EmbeddedAssets` or `MarkdownAssetHost`. The literal source remains independently editable.
- Ready images preserve aspect ratio, use intrinsic pixel dimensions as logical dimensions, clamp width to available content width, and clamp height to 480 logical pixels.
- An accepted size change returns the item's `LayoutInvalidation::BlockMeasurement`.

- [ ] **Step 1: Add the exact SVG fixture and failing asset-state tests**

Create `checker.svg`:

```svg
<svg xmlns="http://www.w3.org/2000/svg" width="96" height="48" viewBox="0 0 96 48">
  <rect width="96" height="48" fill="#1496dc"/>
  <rect x="48" width="48" height="24" fill="#f6f9fc"/>
  <rect y="24" width="48" height="24" fill="#f6f9fc"/>
</svg>
```

Tests must prove:

- reconciliation requests a parsed image once and cancels it when the item disappears;
- a completion for revision 7 is ignored after revision 8 is installed;
- a completion with the wrong item id is ignored;
- loading, failure, ready, max-width, and 480-height measurements are exact;
- activating a failed placeholder sends one new request, but activating a loading or ready block does not retry;
- the literal `![checker](checker.svg)` source run does not change in any asset state;
- no host method exists for arbitrary network fetch.

- [ ] **Step 2: Run asset tests and confirm red**

```powershell
rtk cargo test -p waml-markdown-editor --test assets
```

Expected: FAIL because asset host/state interfaces do not exist.

- [ ] **Step 3: Implement request reconciliation and revision checks**

Use `(revision, PresentationItemId)` as the authoritative state key. Allocate monotonically increasing request ids. Apply an event only when request id, revision, and item all match the current pending entry. All stale events return `AssetEventOutcome::IgnoredStale` without redraw or relayout. `drain_events` is the only completion ingress: application code drains it, passes each event to `EmbeddedAssets::apply_event`, and never mutates embedded state directly.

- [ ] **Step 4: Decode only host-approved sources in the widget adapter**

For approved bytes, call Makepad `ImageRef::load_svg_from_data`, `load_png_from_data`, or `load_jpg_from_data` according to `ImageMediaType`. For an approved canonical path, call `ImageRef::load_image_file_by_path_async`. Do not call `load_image_http_by_url_async`; remote hosts must return approved bytes.

Keep `ImageRef` and texture state in the widget adapter, not in the immutable presentation plan.

- [ ] **Step 5: Verify assets and layout invalidation**

```powershell
rtk cargo test -p waml-markdown-editor --test assets
rtk cargo test -p waml-markdown-editor --test presentation_layout
rtk rg -n 'load_image_http_by_url_async|http_request|TcpStream|reqwest' crates/waml-markdown-editor/src
```

Expected: PASS. The scan returns no match. A ready image with a changed size invalidates only its owner block and downstream geometry to the next stable summary.

- [ ] **Step 6: Commit assets**

```powershell
rtk git add crates/waml-markdown-editor/src/lib.rs crates/waml-markdown-editor/src/presentation/assets.rs crates/waml-markdown-editor/src/presentation/layout.rs crates/waml-markdown-editor/src/widget.rs crates/waml-markdown-editor/tests/assets.rs crates/waml-markdown-editor/tests/fixtures/checker.svg
rtk git commit -m "feat(markdown): add revisioned image blocks"
```

---

### Task 6: Add Stable-Identity Geometry Motion and Scroll Anchoring

**Files:**
- Create: `crates/waml-markdown-editor/src/motion.rs`
- Create: `crates/waml-markdown-editor/tests/motion.rs`
- Create: `crates/waml-markdown-editor/tests/fixtures/motion-before.md`
- Create: `crates/waml-markdown-editor/tests/fixtures/motion-after.md`
- Modify: `crates/waml-markdown-editor/src/layout/geometry.rs`
- Modify: `crates/waml-markdown-editor/src/lib.rs`

**Interfaces:**
- Consumes the foundation geometry identity. The foundation assigns `cluster_ordinal` in stable logical source order, independent of wrapping and bidi visual order:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GeometryElementId {
    pub layout: LayoutElementId,
    pub cluster_ordinal: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MotionElementId {
    GlyphCluster(GeometryElementId),
    Block(LayoutElementId),
    EmbeddedBlock(LayoutElementId),
}

pub struct GeometryEntry {
    pub id: MotionElementId,
    pub rect: Rect,
    pub baseline: Option<f64>,
}

pub struct MotionConfig {
    pub duration_seconds: f64,
    pub ease: Ease,
    pub max_changed_source_bytes: usize,
    pub max_changed_visible_elements: usize,
}

impl Default for MotionConfig {
    fn default() -> Self {
        Self {
            duration_seconds: 0.100,
            ease: Ease::OutCubic,
            max_changed_source_bytes: 4096,
            max_changed_visible_elements: 256,
        }
    }
}

pub enum LayoutChangeCause {
    LocalEdit { changes: Arc<[TextChange]> },
    ImageMeasurement(LayoutElementId),
    ViewportResize,
    InitialLoad,
    ExternalReplacement,
}

pub enum MotionCutReason {
    ReducedMotion,
    InitialLoad,
    ExternalReplacement,
    ViewportResize,
    SourceBudget,
    VisibleGeometryBudget,
    UnsafeIdentityMapping,
    OutsideViewport,
}

pub struct MotionFrame {
    pub layout: Arc<LayoutSnapshot>,
    pub scroll_y: f64,
    pub progress: f64,
    pub active: bool,
    pub cut_reason: Option<MotionCutReason>,
}

impl MotionController {
    pub fn commit(
        &mut self,
        now: f64,
        previous: Option<Arc<LayoutSnapshot>>,
        target: Arc<LayoutSnapshot>,
        cause: LayoutChangeCause,
        reduced_motion: bool,
        anchor: Option<ScrollAnchor>,
        config: MotionConfig,
    ) -> MotionFrame;
    pub fn sample(&mut self, now: f64) -> MotionFrame;
}
```

- Progress uses `Play::Forward { duration }.get_ended_time(elapsed)` and `Ease::map`.
- Matching key is `MotionElementId`. Its variants prevent a glyph cluster, block, and embedded block from colliding even when they share one `LayoutElementId`. A safe map has no duplicate keys and has a matched caret/selection/IME source anchor when that anchor survives the text change.
- `LayoutSnapshot::interpolate(previous, target, eased)` starts from the complete target snapshot and replaces only matched surviving rects, baselines, and caret stops with interpolated values. New target elements stay at target geometry. Deleted elements are absent.
- Only `LocalEdit` and `ImageMeasurement` are eligible to animate. `ViewportResize`, `InitialLoad`, and `ExternalReplacement` cut directly to target.
- Changed-source size is `sum(old_range.len().to_usize() + replacement.len())` across the accepted local edit. Changed visible geometry counts each new, deleted, or surviving moved `MotionElementId` once across the union of the previous and target visible windows.
- Evaluate cut policy in this exact order: reduced motion, initial load, external replacement, viewport resize, source budget, visible-geometry budget, unsafe identity, then outside viewport. The first match is the reported reason.

- [ ] **Step 1: Write deterministic start, midpoint, target, and cutover tests**

Use hand-built snapshots with one heading, one body cluster, one selection, one diagnostic, one image block, and one caret. Assert:

```rust
#[test]
fn out_cubic_transition_has_exact_start_midpoint_and_target() {
    let mut motion = controller(previous_at_y(20.0), target_at_y(60.0));
    assert_rect_y(motion.sample(10.000), 20.0);
    assert_rect_y(motion.sample(10.050), 55.0); // OutCubic(0.5) == 0.875
    assert_rect_y(motion.sample(10.100), 60.0);
    assert!(!motion.sample(10.100).active);
}
```

Also assert every cut reason, exact 4,096/4,097-byte boundary, exact 256/257 visible-element boundary, duplicate identity rejection, viewport-resize cut, outside-viewport cut, newly inserted target geometry, deleted omission, and image remeasurement through the same transition.

- [ ] **Step 2: Run motion tests and confirm red**

```powershell
rtk cargo test -p waml-markdown-editor --test motion
```

Expected: FAIL because `MotionController`, geometry ids, and interpolation do not exist.

- [ ] **Step 3: Add unique geometry ids and safe-map validation**

The layout engine assigns `cluster_ordinal` in logical source order within each `LayoutTextRun`. Wrapping may move a cluster to another visual line but must not change its id. A syntax identity or presentation fragment change may create a new id.

Build maps with `BTreeMap<MotionElementId, index>` and reject duplicates. Use the edit change map to verify that matched unchanged source clusters correspond across revisions.

- [ ] **Step 4: Implement interpolation with Makepad timing primitives**

Clamp time to `[0, duration]`. Interpolate `Rect.pos`, `Rect.size`, baseline, caret stops, block bounds, and embedded bounds with one eased scalar. Do not interpolate colors or reconstruct text layout during a frame.

Rebase an interrupted transition from the currently sampled frame, not from the obsolete original previous snapshot.

- [ ] **Step 5: Implement caret-bound scroll anchoring**

At commit, retain the caret's viewport y from `ScrollAnchor`. For each sampled interpolated snapshot:

```rust
let caret = frame_layout.source_to_point(anchor.position)?;
let scroll_y = (caret.rect.pos.y - anchor.viewport_y)
    .clamp(0.0, frame_layout.max_scroll_y(viewport_height));
```

Use the same sampled caret geometry that the caret layer draws. On a cut, use target caret geometry immediately.

- [ ] **Step 6: Run motion and geometry suites**

```powershell
rtk cargo test -p waml-markdown-editor --test motion
rtk cargo test -p waml-markdown-editor --test layout_geometry
rtk cargo test -p waml-markdown-editor --test presentation_layout
```

Expected: PASS. At midpoint, glyph, selection, diagnostic, image, caret, and IME test queries all report the same 0.875 eased displacement.

- [ ] **Step 7: Commit motion**

```powershell
rtk git add crates/waml-markdown-editor/src/lib.rs crates/waml-markdown-editor/src/motion.rs crates/waml-markdown-editor/src/layout/geometry.rs crates/waml-markdown-editor/tests/motion.rs crates/waml-markdown-editor/tests/fixtures/motion-before.md crates/waml-markdown-editor/tests/fixtures/motion-after.md
rtk git commit -m "feat(markdown): animate shared layout geometry"
```

---

### Task 7: Draw All Presentation Layers from One Interpolated Snapshot

**Files:**
- Create: `crates/waml-markdown-editor/src/presentation/draw.rs`
- Create: `crates/waml-markdown-editor/tests/draw_layers.rs`
- Modify: `crates/waml-markdown-editor/src/layout/makepad.rs`
- Modify: `crates/waml-markdown-editor/src/widget.rs`
- Modify: `crates/waml-markdown-editor/src/lib.rs`

**Interfaces:**
- Produces:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresentedDiagnosticSeverity {
    Error,
    Warning,
    Information,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentedDiagnostic {
    pub revision: DocumentRevision,
    pub range: TextRange,
    pub severity: PresentedDiagnosticSeverity,
    pub message: Arc<str>,
}

#[derive(Clone)]
pub struct InstalledPresentation {
    pub revision: DocumentRevision,
    pub plan: Arc<PresentationPlan>,
    pub styles: Arc<PresentationStyles>,
    pub layout_document: Arc<LayoutDocument>,
    pub diagnostics: Arc<[PresentedDiagnostic]>,
    pub assets: Arc<EmbeddedAssetFrame>,
}

impl InstalledPresentation {
    pub fn new(
        plan: Arc<PresentationPlan>,
        styles: Arc<PresentationStyles>,
        layout_document: Arc<LayoutDocument>,
        diagnostics: Arc<[PresentedDiagnostic]>,
        assets: Arc<EmbeddedAssetFrame>,
    ) -> Result<Arc<Self>, PresentationError>;
}

pub struct PresentationFrame {
    pub revision: DocumentRevision,
    pub layout: Arc<LayoutSnapshot>,
    pub active_owners: Arc<[SyntaxIdentity]>,
    pub diagnostics: Arc<[PresentedDiagnostic]>,
    pub assets: Arc<EmbeddedAssetFrame>,
}

pub enum DrawCommand {
    BlockBackground { id: LayoutElementId, rect: Rect, role: BlockDecorationRole },
    Selection { rect: Rect },
    Text { id: GeometryElementId, range: TextRange, rect: Rect, style: ResolvedTextStyle },
    Decoration { range: TextRange, rects: Arc<[Rect]>, role: DecorationRole },
    EmbeddedBlock { id: LayoutElementId, rect: Rect, state: EmbeddedState },
    CaretAndIme { caret: Rect, composition: Arc<[Rect]> },
}

pub fn build_draw_commands(
    frame: &PresentationFrame,
    plan: &PresentationPlan,
    styles: &PresentationStyles,
    selection: &SelectionSet,
    ime: Option<&ImeComposition>,
) -> Result<Arc<[DrawCommand]>, PresentationError>;

impl MarkdownEditorRef {
    pub fn install_presentation(
        &self,
        cx: &mut Cx,
        presentation: Arc<InstalledPresentation>,
        cause: LayoutChangeCause,
    );
    pub fn clear_presentation(&self, cx: &mut Cx);
    pub fn target_layout(&self) -> Option<Arc<LayoutSnapshot>>;
    pub fn frame_layout(&self) -> Option<Arc<LayoutSnapshot>>;
}
```

- `build_draw_commands` emits commands in foundation `DrawLayer` order only.
- Selection, diagnostics, links, caret, and IME call geometry queries on `frame.layout`; they do not shape text or calculate independent y positions.
- The widget gains live properties `motion_duration: 0.100`, `motion_ease: Ease::OutCubic`, `body_color`, `marker_color`, `marker_active_color`, `link_color`, `diagnostic_color`, block fills/rules, selection color, and caret color.
- The widget gains `set_reduced_motion(&mut self, cx: &mut Cx, reduced: bool)`.
- `InstalledPresentation::new` validates that its own revision, `plan.revision`, `layout_document.revision`, every diagnostic revision, and `assets.revision` are equal. A mismatch is a typed `PresentationError::RevisionMismatch`; partial installation is prohibited.
- `MarkdownEditor` owns the `LayoutEngine`, target/previous/frame `Arc<LayoutSnapshot>` values, `MotionController`, pending `LayoutChangeCause`, and `NextFrame`. `SourceView` owns source session, presentation compilation, diagnostics, and asset state, but never owns or installs a raw `LayoutSnapshot`.
- At each installed revision, `SourceView` calls only `install_presentation`. On the next draw with `MarkdownEditorScope`, the widget checks `presentation.revision == session.local_revision()`, lays out the target, commits or cuts motion with the supplied cause, and draws/hit-tests from the resulting frame snapshot. The foundation remains the only geometry authority.

- [ ] **Step 1: Write failing draw-order and shared-geometry tests**

Build a frame with all six layers. Assert the exact command discriminant order:

```rust
assert_eq!(
    commands.iter().map(DrawCommand::layer).collect::<Vec<_>>(),
    vec![
        DrawLayer::BlockBackground,
        DrawLayer::Selection,
        DrawLayer::Text,
        DrawLayer::Decoration,
        DrawLayer::EmbeddedBlock,
        DrawLayer::CaretAndIme,
    ],
);
```

Assert marker active/inactive commands have identical rects and metrics but different color roles. Put the caret inside strong emphasis and assert the strong-emphasis content keeps its semantic metrics while its delimiter runs use the active marker color. Assert selection, link underline, diagnostic underline, image, caret, and IME rects equal the matching queries from the same `Arc<LayoutSnapshot>`.

- [ ] **Step 2: Run draw tests and confirm red**

```powershell
rtk cargo test -p waml-markdown-editor --test draw_layers
```

Expected: FAIL because the frame command builder and renderer do not exist.

- [ ] **Step 3: Add the Makepad shaping and paint cache**

Resolve `FONT_SANS` and `FONT_MONO` to the widget's live `DrawText` fields. Shape through:

```rust
DrawText::layout(
    cx,
    first_row_indent,
    min_line_spacing_below,
    max_width,
    wrap,
    Align::default(),
    source_slice,
)
```

Preserve each `LaidoutGlyph.cluster` byte offset when creating foundation glyph clusters and caret stops. Since `TextMetrics` contains floats and is not an `Eq`/`Hash` key, define:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct TextLayoutKey {
    revision: DocumentRevision,
    id: LayoutElementId,
    font: FontKey,
    font_size_bits: u32,
    line_spacing_bits: u32,
    weight: FontWeight,
    italic: bool,
    width_bits: u64,
}
```

Build float fields with `to_bits()` after rejecting non-finite or negative style/width values. Cache `Rc<LaidoutText>` by this key. Paint cached glyphs at interpolated absolute positions through `draw_rasterized_glyphs_abs`; measurement and drawing must use the same `DrawText` font/style.

- [ ] **Step 4: Implement the strict command builder and renderer**

Use:

1. `DrawQuad` for quote/code/table/inline-code/background fills and rules;
2. `DrawColor` for selection rectangles;
3. role-resolved `DrawText` for source;
4. `DrawQuad` for link and diagnostic underlines;
5. `ImageRef` or placeholder/error `DrawQuad` plus label for images;
6. `DrawQuad` for caret and IME composition underline.

Raw HTML emits only text commands. Task-checkbox decorations do not accept pointer actions. The source brackets remain the hit target.

- [ ] **Step 5: Install presentation atomically, schedule frames, and route link activation**

Extend the foundation widget with `installed: Option<Arc<InstalledPresentation>>`, `motion: MotionController`, `frame_layout`, `pending_cause`, and `next_frame`. `install_presentation` validates the complete revision bundle before replacing `installed`; it never accepts separate plan/layout/diagnostic/asset setters. When target layout is available, commit motion from the currently sampled frame so interrupted transitions do not jump.

On an active transition, store `next_frame = cx.new_next_frame()`. On the matching `NextFrameEvent`, sample using `event.time`, update the scoped session scroll from the returned `MotionFrame`, redraw, and request one more frame only while `active`.

Normal clicks continue through foundation caret/selection handling. Ctrl on Windows/Linux or Command on macOS over a parsed `PresentedLink` emits the existing foundation action:

```rust
MarkdownEditorAction::NavigationRequested {
    position,
}
```

Here `position` is the foundation hit-test result already proven to lie in `link.source_range`. The Spec 4 application resolves that position through the installed syntax snapshot and opens only the parsed link destination. Do not open the destination in this crate.

- [ ] **Step 6: Verify draw, widget, parity, and reduced-motion behavior**

```powershell
rtk cargo test -p waml-markdown-editor --test draw_layers
rtk cargo test -p waml-markdown-editor --test widget_parity
rtk cargo test -p waml-markdown-editor --test motion
rtk cargo check -p waml-markdown-editor
```

Expected: PASS. Reduced motion prevents `NextFrame` rescheduling and returns target geometry on the edit event. Normal link clicks place the caret; only the platform navigation modifier emits `NavigationRequested`.

- [ ] **Step 7: Commit the renderer**

```powershell
rtk git add crates/waml-markdown-editor/src/lib.rs crates/waml-markdown-editor/src/widget.rs crates/waml-markdown-editor/src/layout/makepad.rs crates/waml-markdown-editor/src/presentation/draw.rs crates/waml-markdown-editor/tests/draw_layers.rs
rtk git commit -m "feat(markdown): draw animated presentation"
```

---

### Task 8: Add the Native Presentation Harness and Verification Record

**Files:**
- Create: `crates/waml-editor/src/bin/markdown_presentation_harness.rs`
- Modify: `crates/waml-editor/Cargo.toml`
- Modify: `crates/waml-editor/tests/README.md`

**Interfaces:**
- Consumes: public `waml_markdown_editor::script_mod`, `MarkdownEditor`, `PresentationPlan`, `MotionController`, the fixtures from Tasks 2/5/6, WAML `theme_atlas.rs`, WAML `fonts.rs`, and `scripts/capture-window.ps1`.
- Produces a fixed `1280 x 900` native harness with CLI:

```text
markdown_presentation_harness --case <headings|inline|lists|quotes|code|tables|images|invalid|selection|motion-start|motion-mid|motion-end>
```

- Motion cases use pure snapshot sampling at progress `0.0`, `0.5`, and `1.0`; they do not sleep and hope to catch a frame.
- The harness uses Atlas surface/text/text-dim/accent/selection roles and IBM Plex Sans/Mono resources. It sets 24 logical pixels through the editor style, not an outer compensating wrapper.

- [ ] **Step 1: Add a failing harness compile gate**

Add the bin dependency and a minimal compile test command before the bin exists:

```powershell
rtk cargo check -p waml-editor --bin markdown_presentation_harness
```

Expected: FAIL because the binary target does not exist.

- [ ] **Step 2: Implement the fixed-state harness**

Follow the existing `node_editor_harness.rs` pattern:

- import `theme_atlas.rs` and `fonts.rs` by path;
- register Makepad widgets, Atlas, fonts, `waml_markdown_editor::script_mod`, then the harness script;
- parse the selected checked-in fixture through Spec 1;
- create the foundation session and presentation plan;
- use a deterministic mock highlighter for the `waml` fence based on checked typed spans;
- make the `images` case show three literal image-source lines with loading, failed/retry, and approved `checker.svg` byte states;
- select all nested inline syntax for `selection`, place the caret inside a construct, install one diagnostic, and install one IME composition range;
- build static start/mid/end `PresentationFrame` values from the motion fixture for the three motion cases;
- set window size and title before first draw.

- [ ] **Step 3: Document the exact native capture matrix**

Append a “Markdown presentation and motion” section to `crates/waml-editor/tests/README.md`. Store evidence outside the repository at `C:\tmp\markdown-presentation-verification`. Use only the PID launched by the verification command:

```powershell
rtk cargo build -p waml-editor --bin markdown_presentation_harness --release
rtk proxy pwsh -NoProfile -Command '$out = "C:\tmp\markdown-presentation-verification"; New-Item -ItemType Directory -Force -Path $out | Out-Null; $cases = @("headings","inline","lists","quotes","code","tables","images","invalid","selection","motion-start","motion-mid","motion-end"); foreach ($case in $cases) { $p = Start-Process -FilePath "target\release\markdown_presentation_harness.exe" -ArgumentList @("--case",$case) -PassThru; try { $deadline = [DateTime]::UtcNow.AddSeconds(20); while ($p.MainWindowHandle -eq 0 -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 100; $p.Refresh() }; if ($p.MainWindowHandle -eq 0) { throw "window did not open for $case" }; & pwsh -NoProfile -File scripts/capture-window.ps1 -Out "$out\$case.png" -ProcessId $p.Id; if ($LASTEXITCODE -ne 0) { throw "capture failed for $case" } } finally { Stop-Process -Id $p.Id -ErrorAction SilentlyContinue } }'
```

Expected: twelve native-pixel PNGs. Each image is `1280 x 900` logical-window content at the host's native DPI capture. No user-owned WAML process is stopped or reused.

- [ ] **Step 4: Inspect the static and motion states**

Check these exact properties:

- all literal delimiters remain visible;
- markers are lower contrast and active markers keep identical geometry;
- heading hierarchy is moderate;
- left, right, top, and bottom document inset are 24 logical pixels;
- lists hang from literal markers;
- quotes, code, table, checkbox, thematic rule, and image decorations do not replace source;
- raw HTML is visible and inert;
- loading/error image states keep the source line;
- start/mid/end motion images show the same surviving identities, with midpoint at 87.5% of displacement;
- selection, diagnostics, image, caret, and IME remain attached to text at every sampled phase.

- [ ] **Step 5: Run the complete Spec 3 automated gate**

```powershell
rtk cargo fmt --check
rtk cargo test -p waml-markdown-editor
rtk cargo check -p waml-markdown-editor
rtk cargo check -p waml-editor --bin markdown_presentation_harness
rtk cargo clippy -p waml-markdown-editor --all-targets -- -D warnings
rtk cargo doc -p waml-markdown-editor --no-deps
rtk rg -n 'pulldown_cmark::Parser|regex::|load_image_http_by_url_async|tower_lsp|as_markdown|MarkdownRef|CodeEditorRef|TextFlow' crates/waml-markdown-editor/src
rtk git diff --check
```

Expected: PASS. Format, tests, checks, clippy, and docs pass. The architecture scan returns no match. `git diff --check` reports no whitespace error.

- [ ] **Step 6: Commit the harness and verification instructions**

```powershell
rtk git add crates/waml-editor/Cargo.toml crates/waml-editor/src/bin/markdown_presentation_harness.rs crates/waml-editor/tests/README.md
rtk git commit -m "test(markdown): add presentation harness"
```

---

## Completion Criteria

- Every source byte is present in exactly one editable `TextRun`; overlay items never remove or replace source.
- Presentation is derived only from immutable syntax queries and typed host products.
- All required CommonMark/GFM constructs, recovery, raw HTML, unknown code languages, failed highlighters, and image states have focused tests.
- The document has a 24-logical-pixel inset, no centered maximum-width column, and the exact balanced metric table.
- Variable layout handles mixed metrics, hanging lists, nested quotes, code, tables, embedded images, wrapping, resize, hit-testing, and off-screen summaries.
- Motion uses stable syntax-derived identity, Makepad `Play`/`Ease`, one 100 ms transition, one interpolated snapshot, deterministic tests, and all required cutovers.
- Selection, text, diagnostics, link decoration, images, caret, and IME consume one frame snapshot and follow the prescribed draw order.
- Asset and link policy stays in the application host. The editor does not execute HTML, fetch a network URL, open a link, write a file, or call LSP.
- The native harness captures all required static states and deterministic start/mid/end motion states at native HiDPI resolution.
- The crate has no dependency on Makepad's upstream `CodeEditor`, `Markdown`, or `TextFlow` widget implementation.
