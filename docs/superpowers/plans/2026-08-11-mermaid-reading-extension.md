# Mermaid Reading Extension Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render existing fenced Markdown blocks whose language is the ASCII case-insensitive word `mermaid` as safe, read-only SVG diagrams in native and browser/WASM reading views.

**Architecture:** Keep Markdown parsing generic: the presentation model preserves fenced-code metadata, and the reading model routes only registered language keys to a generic block-extension lifecycle. `waml-editor` owns the registry and platform scheduler; all Mermaid grammar, Merman calls, error translation, and cache policy stay below `markdown_extensions/mermaid/`. Native builds use worker threads, while every `wasm32` build uses a one-cache-miss-per-deferred-turn cooperative queue so non-atomics WASM never depends on Makepad's no-op thread spawn.

**Tech Stack:** Rust, Makepad `Image`/`DrawSvg`, Merman `=0.8.0-alpha.5` with `svg`, usvg with bundled fonts for Makepad-compatible paint and label paths, native `std::thread` plus `SignalToUI`, cooperative Makepad `NextFrame` scheduling on WASM.

## Global Constraints

- Work only in the isolated worktree created from `origin/main`; never edit `C:\dev\waml` directly.
- Prefix every shell command with `rtk`.
- Use ASD-STE100 Simplified Technical English in code comments, diagnostics, fixture copy, and documentation.
- Keep the workspace Rust version at `1.80`; set only `crates/waml-editor/Cargo.toml` to `rust-version = "1.95"`.
- Pin Merman exactly as `merman = { version = "=0.8.0-alpha.5", default-features = false, features = ["svg"] }`.
- Only Rust files below `crates/waml-editor/src/markdown_extensions/mermaid/` may import `merman` or name a Merman type.
- Do not add Mermaid grammar, family detection, aliases, authoring support, completion, hover, linting, formatting, or refactoring to WAML.
- Route `mermaid`, `MERMAID`, and other ASCII case variants; leave all other fenced languages as normal code blocks.
- Accept every diagram family that the pinned Merman build detects; do not add a WAML family list.
- Use `HeadlessRenderer::render_resvg_compatible_svg_sync`; do not use Mermaid JavaScript, browser `foreignObject` labels, a PNG intermediate, or a second SVG renderer.
- Enforce 256 KiB maximum source, 8 MiB maximum SVG, finite positive dimensions, and 16,384 logical units maximum per side.
- Do not fetch network resources, run scripts, activate external links, accept event-handler attributes, or install unsanitized user CSS.
- Keep a failed diagram local to its block and show the original source plus `Cannot render Mermaid: <message>`.
- Reject completions whose lease, document revision, item ID, request ID, or source range does not match the live request.
- Run native and WASM verification. Always launch the editor with `run.ps1 -Title mermaid-reading`; add `-Color <hex>` if another task window is open.

---

## File Structure

### Create

- `crates/waml-markdown-editor/src/reading/extensions.rs` — generic registered-language set, request/event contract, per-revision state reconciliation, and immutable viewer frame.
- `crates/waml-markdown-editor/tests/reading_extensions.rs` — generic routing, state, cancellation, and stale-event tests.
- `crates/waml-editor/src/markdown_extensions/mod.rs` — generic renderer registry, lease isolation, native worker scheduler, cooperative WASM queue, and test seams.
- `crates/waml-editor/src/markdown_extensions/mermaid/mod.rs` — the only registry entry point for the Mermaid adapter.
- `crates/waml-editor/src/markdown_extensions/mermaid/renderer.rs` — Merman configuration, SVG call, usvg normalization with bundled fonts, output security checks, Makepad SVG validation, and dimensions.
- `crates/waml-editor/src/markdown_extensions/mermaid/cache.rs` — bounded result/failure cache and exact cache key.
- `crates/waml-editor/src/markdown_extensions/mermaid/error.rs` — stable adapter errors and safe viewer messages.
- `crates/waml-editor/tests/fixtures/mermaid-reading/index.md` — native and browser acceptance fixture with flowchart, sequence, class, state, invalid, and wide diagrams.
- `scripts/mermaid-browser-check.mjs` — non-mutating Playwright check that owns the `waml serve` process until browser assertions and screenshot capture finish.

### Modify

- `Cargo.lock` — lock the exact Merman graph.
- `crates/waml-editor/Cargo.toml` — application-only Rust 1.95 floor and exact Merman dependency.
- `crates/waml-editor/src/lib.rs` — register `markdown_extensions`.
- `crates/waml-editor/src/okf_documents.rs` — create one shared extension host beside the shared Markdown image host and pass it to reading views.
- `crates/waml-editor/src/reading_view.rs` — own an extension lease and state set, reconcile requests, drive completions, and install extension frames.
- `crates/waml-editor/src/generic_okf_view.rs` — forward UI/deferred events to `ReadingView` and preserve source handoff.
- `crates/waml-editor/tests/markdown_integration.rs` — end-to-end reading/source behavior and local failure coverage.
- `crates/waml-markdown-editor/src/presentation/mod.rs` — add generic fenced-code metadata to code blocks and a generic embedded role identity.
- `crates/waml-markdown-editor/src/presentation/compile.rs` — copy `FencedCodeInfo` metadata into the presentation block without routing it.
- `crates/waml-markdown-editor/src/reading/mod.rs` — export the generic extension contract.
- `crates/waml-markdown-editor/src/reading/model.rs` — route registered fenced languages and preserve item/source/content identity.
- `crates/waml-markdown-editor/src/reading/widget.rs` — draw loading, ready SVG, and failed fallback blocks; track visual source rectangles.
- `crates/waml-markdown-editor/tests/presentation_model.rs` — exact fenced metadata and source-partition tests.
- `crates/waml-markdown-editor/tests/reading_model.rs` — case-insensitive route and non-Mermaid fallback tests.
- `crates/waml-markdown-editor/tests/reading_source_map.rs` — visual-block search and source-handoff mapping tests.
- `crates/waml-markdown-editor/tests/reading_widget_draw.rs` — ready sizing, loading height, and failed fallback draw tests.

## Task Interfaces

The following WAML-owned interfaces are fixed for all tasks. Keep these names and field types consistent.

```rust
// waml-markdown-editor::presentation
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationFence {
    pub language: Option<Arc<str>>,
    pub content_range: TextRange,
}

pub enum PresentationBlockKind {
    // existing variants
    Code { fence: Option<PresentationFence> },
}

// waml-markdown-editor::reading
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BlockExtensionAppearance { Light, Dark }

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FencedBlockExtension {
    pub id: PresentationItemId,
    pub language: Arc<str>,
    pub source_range: TextRange,
    pub content_range: TextRange,
}

pub enum ReadingBlockKind {
    // existing variants
    FencedExtension(FencedBlockExtension),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BlockExtensionRequestId(pub u64);

#[derive(Clone, Debug)]
pub struct BlockExtensionRequest {
    pub request_id: BlockExtensionRequestId,
    pub revision: DocumentRevision,
    pub item: PresentationItemId,
    pub source_range: TextRange,
    pub content_range: TextRange,
    pub language: Arc<str>,
    pub content: Arc<str>,
    pub appearance: BlockExtensionAppearance,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderedBlockSvg {
    pub data: Arc<[u8]>,
    pub logical_size: (f64, f64),
}

pub enum BlockExtensionEvent {
    Ready { request_id: BlockExtensionRequestId, revision: DocumentRevision,
            item: PresentationItemId, source_range: TextRange, svg: RenderedBlockSvg },
    Failed { request_id: BlockExtensionRequestId, revision: DocumentRevision,
             item: PresentationItemId, source_range: TextRange, message: Arc<str> },
}

pub trait MarkdownBlockExtensionHost {
    fn request(&mut self, request: BlockExtensionRequest);
    fn cancel(&mut self, request_id: BlockExtensionRequestId);
    fn drain_events(&mut self) -> Vec<BlockExtensionEvent>;
}
```

The application-only renderer seam is:

```rust
pub(super) trait FencedBlockRenderer: Send + Sync {
    fn language(&self) -> &'static str;
    fn cached(&self, request: &BlockExtensionRequest) -> Option<BlockRenderResult>;
    fn render_and_cache(&self, request: &BlockExtensionRequest) -> BlockRenderResult;
}

pub(super) type BlockRenderResult = Result<RenderedBlockSvg, Arc<str>>;
```

### Task 1: Preserve generic fenced-code metadata

**Files:**
- Modify: `crates/waml-markdown-editor/src/presentation/mod.rs`
- Modify: `crates/waml-markdown-editor/src/presentation/compile.rs`
- Modify: `crates/waml-markdown-editor/tests/presentation_model.rs`

**Interfaces:**
- Consumes: existing `MarkdownSyntaxQueries::fenced_code(owner) -> Option<&FencedCodeInfo>`.
- Produces: `PresentationFence` and `PresentationBlockKind::Code { fence: Option<PresentationFence> }` as defined above. `compile_presentation` keeps its current signature.

- [ ] **Step 1: Write failing metadata tests**

Add tests that compile ```` ```MeRmAiD\nflowchart TD\nA-->B\n``` ```` and assert that the code block keeps `language == Some("MeRmAiD")`, the full block range, and the exact content-only range. Add an indented-code test that asserts `fence == None`. Keep `plan.validate_source_partition()` in both tests.

```rust
let code = plan.blocks.iter().find(|block| matches!(block.kind, PresentationBlockKind::Code { .. })).unwrap();
let PresentationBlockKind::Code { fence: Some(fence) } = &code.kind else { panic!("fenced metadata") };
assert_eq!(fence.language.as_deref(), Some("MeRmAiD"));
assert_eq!(&source[fence.content_range.start().to_usize()..fence.content_range.end().to_usize()], "flowchart TD\nA-->B\n");
plan.validate_source_partition().unwrap();
```

- [ ] **Step 2: Run the tests and confirm the expected failure**

Run: `rtk cargo test -p waml-markdown-editor --test presentation_model fenced_code`

Expected: FAIL because `PresentationFence` and the structured `Code` variant do not exist.

- [ ] **Step 3: Implement generic metadata preservation**

Add `PresentationFence`. Change only `Kind::FencedCodeBlock` to read `snapshot.queries().fenced_code(owner)` and copy `language` plus `content_range`. Map indented code and frontmatter to `Code { fence: None }`. Remove `Copy` only from types that now contain `Arc`; update match sites to borrow or clone instead of copying.

Do not normalize the language and do not mention Mermaid in this crate.

- [ ] **Step 4: Run presentation and reading-model tests**

Run: `rtk cargo test -p waml-markdown-editor --test presentation_model --test reading_model`

Expected: PASS after existing `PresentationBlockKind::Code` matches use `Code { .. }`.

- [ ] **Step 5: Commit**

```text
rtk git add crates/waml-markdown-editor/src/presentation crates/waml-markdown-editor/tests/presentation_model.rs crates/waml-markdown-editor/tests/reading_model.rs
rtk git commit -m "feat(markdown): preserve fenced block metadata"
```

### Task 2: Add the generic reading-extension model and lifecycle

**Files:**
- Create: `crates/waml-markdown-editor/src/reading/extensions.rs`
- Create: `crates/waml-markdown-editor/tests/reading_extensions.rs`
- Modify: `crates/waml-markdown-editor/src/presentation/mod.rs`
- Modify: `crates/waml-markdown-editor/src/reading/mod.rs`
- Modify: `crates/waml-markdown-editor/src/reading/model.rs`
- Modify: `crates/waml-markdown-editor/tests/reading_model.rs`

**Interfaces:**
- Consumes: `PresentationFence`, `PresentationItemId`, `PresentationRole`, `EmbeddedBlockRole`, and unchanged text pieces.
- Produces: every `waml-markdown-editor::reading` type in the Task Interfaces section plus `RegisteredBlockExtensions`, `BlockExtensionState`, `BlockExtensionFrame`, and `BlockExtensionStates` below.

```rust
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RegisteredBlockExtensions { languages: BTreeSet<Arc<str>> }

impl RegisteredBlockExtensions {
    pub fn from_languages(languages: impl IntoIterator<Item = Arc<str>>) -> Self;
    pub fn contains(&self, language: &str) -> bool;
}

#[derive(Clone, Debug, PartialEq)]
pub enum BlockExtensionState {
    Loading,
    Ready(RenderedBlockSvg),
    Failed(Arc<str>),
}

#[derive(Clone, Debug)]
struct BlockExtensionEntry {
    request_id: BlockExtensionRequestId,
    source_range: TextRange,
    state: BlockExtensionState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockExtensionEventOutcome { Applied, IgnoredStale }

#[derive(Clone, Debug, PartialEq)]
pub struct BlockExtensionFrame {
    pub revision: DocumentRevision,
    pub items: Arc<[(PresentationItemId, BlockExtensionState)]>,
}

#[derive(Debug, Default)]
pub struct BlockExtensionStates {
    revision: Option<DocumentRevision>,
    entries: BTreeMap<PresentationItemId, BlockExtensionEntry>,
    next_request: u64,
}

impl BlockExtensionStates {
    pub fn reconcile(&mut self, host: &mut dyn MarkdownBlockExtensionHost,
        revision: DocumentRevision, document: &ReadingDocument, source: Arc<str>,
        appearance: BlockExtensionAppearance);
    pub fn apply_event(&mut self, event: BlockExtensionEvent) -> BlockExtensionEventOutcome;
    pub fn pending_count(&self) -> usize;
    pub fn frame(&self, revision: DocumentRevision) -> Arc<BlockExtensionFrame>;
}
```

- [ ] **Step 1: Write routing tests**

Test `mermaid`, `MERMAID`, and `MeRmAiD` against `RegisteredBlockExtensions::from_languages([Arc::from("mermaid")])`. Assert that each becomes `ReadingBlockKind::FencedExtension`, with the original full/content ranges and `PresentationItemId { role: PresentationRole::Embedded(EmbeddedBlockRole::FencedExtension), fragment_ordinal: 0, .. }`. Assert that `rust`, `mermaid-js`, no language, indented code, and an empty registry stay `ReadingBlockKind::Code`.

- [ ] **Step 2: Run the routing tests and confirm failure**

Run: `rtk cargo test -p waml-markdown-editor --test reading_model fenced_extension`

Expected: FAIL because `build_reading_document` does not accept a registry and has no extension block.

- [ ] **Step 3: Implement routing without changing the source partition**

Change the signature to:

```rust
pub fn build_reading_document(
    plan: &PresentationPlan,
    extensions: &RegisteredBlockExtensions,
) -> Result<ReadingDocument, ReadingError>
```

Use `eq_ignore_ascii_case` only after rejecting non-ASCII keys in `RegisteredBlockExtensions::from_languages`. Keep every `ReadingPiece`, including hidden fences and info text, so `ReadingDocument::validate_source_partition` remains exact.

- [ ] **Step 4: Write lifecycle tests with a fake host**

Cover: one request per live block/revision; exact fence content in `request.content`; removal cancellation; revision cancellation; ready/failed transitions; stale request ID, revision, item, and source range rejection; a frame containing only live states; and `pending_count()` decreasing only when a live loading entry becomes ready or failed. Assert the concise message is stored without debug formatting.

- [ ] **Step 5: Run the lifecycle tests and confirm failure**

Run: `rtk cargo test -p waml-markdown-editor --test reading_extensions`

Expected: FAIL because `BlockExtensionStates` does not exist.

- [ ] **Step 6: Implement the lifecycle**

Mirror the proven `EmbeddedAssets` rules, but reconcile `ReadingBlockKind::FencedExtension` entries. Allocate monotonically increasing request IDs. Before slicing, validate `content_range` against the shared source; produce a local failed state if it is invalid. `apply_event` must compare all five identity fields and return `IgnoredStale` without state changes on any mismatch. Implement `pending_count()` as the number of live entries whose state is `BlockExtensionState::Loading`; failed and ready entries return zero work.

- [ ] **Step 7: Run all Markdown widget tests**

Run: `rtk cargo test -p waml-markdown-editor`

Expected: PASS after all callers pass either the registered set or `RegisteredBlockExtensions::default()`.

- [ ] **Step 8: Commit**

```text
rtk git add crates/waml-markdown-editor/src/reading crates/waml-markdown-editor/tests/reading_extensions.rs crates/waml-markdown-editor/tests/reading_model.rs
rtk git commit -m "feat(markdown): add block extension lifecycle"
```

### Task 3: Draw extension blocks with Makepad's dynamic SVG path

**Files:**
- Modify: `crates/waml-markdown-editor/src/reading/widget.rs`
- Modify: `crates/waml-markdown-editor/tests/reading_source_map.rs`
- Modify: `crates/waml-markdown-editor/tests/reading_widget_draw.rs`

**Interfaces:**
- Consumes: `BlockExtensionFrame`, `BlockExtensionState`, and `RenderedBlockSvg` from Task 2.
- Produces: `MarkdownViewer::install_document(cx, document, source, extensions)`, `SourceMap::push_visual`, and a per-item dynamic `Image` cache.

```rust
pub fn install_document(&mut self, cx: &mut Cx, document: Arc<ReadingDocument>,
    source: Arc<str>, extensions: Arc<BlockExtensionFrame>);

impl SourceMap {
    pub fn push_visual(&mut self, source: TextRange, rect: Rect);
    pub fn visual_rects_for_source(&self, source: TextRange) -> Vec<Rect>;
}
```

- [ ] **Step 1: Write failing source-map and geometry tests**

Test that a visual rectangle maps to the full fenced source range, contributes to search-highlight rectangles, and becomes the source handoff range after a pointer press. Add draw tests for: loading height `72.0`; ready non-zero geometry; a 2:1 SVG constrained from 1000 units to a 400-pixel column as 400 by 200; no upscale for a 200-unit SVG in a 400-pixel column; and failed source plus one concise error line.

- [ ] **Step 2: Run the focused tests and confirm failure**

Run: `rtk cargo test -p waml-markdown-editor --test reading_source_map --test reading_widget_draw`

Expected: FAIL because the viewer does not accept an extension frame or draw visual source units.

- [ ] **Step 3: Implement loading and failed drawing first**

Keep `LOADING_HEIGHT = 72.0`. Refactor the existing code-block branch into a helper that can append `Cannot render Mermaid: {message}` after the original monospace content. Do not expose raw renderer errors.

- [ ] **Step 4: Implement ready SVG drawing**

Keep `BTreeMap<PresentationItemId, ImageRef>` on `MarkdownViewer`. Construct a cache miss through the repository's existing dynamic-widget path:

```rust
let widget = WidgetRef::new_with_inner(Box::new(
    cx.with_vm(Image::script_new_with_default),
));
let image = widget.as_image();
images.insert(item, image.clone());
```

On new shared bytes, load through the `ImageRef`, then borrow the underlying `Image` only for the draw call:

```rust
image.load_svg_from_shared_data(cx, svg.data.clone())?;
let scale = (available_width / svg.logical_size.0).min(1.0);
let walk = Walk {
    width: Size::Fixed(svg.logical_size.0 * scale),
    height: Size::Fixed(svg.logical_size.1 * scale),
    ..Walk::default()
};
if let Some(mut inner) = image.borrow_mut() {
    inner.draw_walk_image(cx, walk);
}
```

Center the fixed-size walk inside the same readable column. Record the actual fixed rectangle with `SourceMap::push_visual`. Remove cached `Image` values for retired items when installing a frame. If Makepad rejects the SVG during `load_svg_from_shared_data`, draw the same safe failed fallback; never panic.

- [ ] **Step 5: Implement visual search and source handoff**

Extend `SourceMap` with visual rectangles. `draw_search_highlights` must include a diagram rectangle when the search source span overlaps its fence. On `FingerDown`, remember the hit visual's source range. `selected_source_span` must prefer an active text selection, then the remembered visual range, then `None`; changing documents clears the remembered visual.

- [ ] **Step 6: Run the focused and full crate tests**

Run: `rtk cargo test -p waml-markdown-editor --test reading_source_map --test reading_widget_draw`

Expected: PASS.

Run: `rtk cargo test -p waml-markdown-editor`

Expected: PASS.

- [ ] **Step 7: Commit**

```text
rtk git add crates/waml-markdown-editor/src/reading/widget.rs crates/waml-markdown-editor/tests/reading_source_map.rs crates/waml-markdown-editor/tests/reading_widget_draw.rs
rtk git commit -m "feat(markdown): draw rendered block extensions"
```

### Task 4: Pin Merman and prove its native/WASM compile contract

**Files:**
- Modify: `crates/waml-editor/Cargo.toml`
- Modify: `Cargo.lock`
- Create: `crates/waml-editor/src/markdown_extensions/mod.rs`
- Create: `crates/waml-editor/src/markdown_extensions/mermaid/mod.rs`
- Create: `crates/waml-editor/src/markdown_extensions/mermaid/renderer.rs`
- Create: `crates/waml-editor/src/markdown_extensions/mermaid/cache.rs`
- Create: `crates/waml-editor/src/markdown_extensions/mermaid/error.rs`
- Modify: `crates/waml-editor/src/lib.rs`

**Interfaces:**
- Consumes: the application-only `FencedBlockRenderer` seam and WAML-owned request/result types.
- Produces: `pub(super) fn renderer() -> Arc<dyn FencedBlockRenderer>` from `mermaid/mod.rs`. No Merman value appears in that signature.

- [ ] **Step 1: Add the exact dependency boundary**

Replace `rust-version.workspace = true` only in `crates/waml-editor/Cargo.toml` with `rust-version = "1.95"`, and add the exact Merman declaration from Global Constraints. Add `mod markdown_extensions;` in `lib.rs`.

- [ ] **Step 2: Add a compile-only renderer spike test**

In `renderer.rs`, add a test that uses the actual alpha API and asserts a flowchart returns an SVG root:

```rust
use merman::svg::{
    HeadlessRenderer, HostTheme, HostThemePreset, Presentation,
    RenderResourceProfile,
};

let renderer = HeadlessRenderer::new()
    .with_presentation(Presentation::new().with_theme(
        HostTheme::from_preset(HostThemePreset::EditorLight),
    ))
    .with_resource_profile(RenderResourceProfile::Constrained)
    .with_vendored_text_measurer()
    .with_diagram_id("waml-mermaid-spike");
let sealed = renderer
    .render_resvg_compatible_svg_sync("flowchart TD\nA-->B")
    .unwrap()
    .expect("diagram detected");
assert!(sealed.as_str().trim_start().starts_with("<svg"));
```

- [ ] **Step 3: Run native and WASM compile checks immediately**

Run: `rtk cargo test -p waml-editor markdown_extensions::mermaid::renderer::tests::pinned_merman_api_renders_resvg_compatible_svg`

Expected: PASS with the exact `0.8.0-alpha.5` API.

Run: `rtk cargo check -p waml-editor --target wasm32-unknown-unknown`

Expected: PASS. If this fails inside Merman for the non-atomics target, stop implementation and record the exact dependency/target error in the task report; do not hide Merman behind a native-only `cfg`, because browser support is an acceptance requirement.

- [ ] **Step 4: Confirm the dependency boundary**

Run: `rtk cargo tree -i merman`

Expected: the reverse tree has `waml-editor` as the only workspace dependent.

Run: `rtk cargo metadata --no-deps --format-version 1`

Expected: reusable crates still report the workspace Rust `1.80`; only `waml-editor` reports `1.95`.

- [ ] **Step 5: Commit the proven dependency spike**

```text
rtk git add crates/waml-editor/Cargo.toml Cargo.lock crates/waml-editor/src/lib.rs crates/waml-editor/src/markdown_extensions
rtk git commit -m "build(editor): pin Merman renderer"
```

### Task 5: Implement the isolated Mermaid adapter, validation, and cache

**Files:**
- Modify: `crates/waml-editor/src/markdown_extensions/mermaid/mod.rs`
- Modify: `crates/waml-editor/src/markdown_extensions/mermaid/renderer.rs`
- Modify: `crates/waml-editor/src/markdown_extensions/mermaid/cache.rs`
- Modify: `crates/waml-editor/src/markdown_extensions/mermaid/error.rs`

**Interfaces:**
- Consumes: `BlockExtensionRequest` and returns `BlockRenderResult` only.
- Produces: a renderer whose `cached` and `render_and_cache` methods satisfy the generic application trait.

```rust
const MAX_SOURCE_BYTES: usize = 256 * 1024;
const MAX_SVG_BYTES: usize = 8 * 1024 * 1024;
const MAX_LOGICAL_SIDE: f64 = 16_384.0;
const CACHE_MAX_ENTRIES: usize = 64;
const CACHE_MAX_SVG_BYTES: usize = 32 * 1024 * 1024;
const ADAPTER_SCHEMA_VERSION: u32 = 1;
const MERMAN_VERSION: &str = "0.8.0-alpha.5";
```

- [ ] **Step 1: Write adapter fixtures and failure tests**

Add unit fixtures for flowchart, sequence, class, and state diagrams. The state fixture proves there is no WAML family allowlist. Assert each result has non-zero dimensions and parses through `makepad_widgets::makepad_draw::svg::parse_svg`. Add invalid-source, 256 KiB + 1 source, 8 MiB + 1 synthetic output, zero/NaN/infinite/16,385 dimensions, `foreignObject`, script, `onload`, external `href`, external CSS URL, and source CSS override tests.

The security tests must assert the installed bytes contain no active element, event handler, remote resource, or external navigation. Fragment references such as `href="#node"` and `url(#paint)` remain valid.

- [ ] **Step 2: Run adapter tests and confirm failure**

Run: `rtk cargo test -p waml-editor markdown_extensions::mermaid`

Expected: FAIL because the adapter and validation policy are incomplete.

- [ ] **Step 3: Implement stable error translation**

Use a small `MermaidRenderError` enum with stable messages: `SourceTooLarge`, `DiagramNotDetected`, `InvalidDiagram`, `SvgTooLarge`, `UnsafeSvg`, `InvalidSvg`, and `InvalidDimensions`. Keep the Merman error only as a logged diagnostic in `renderer.rs`; `viewer_message()` returns short text such as `diagram syntax is invalid` and never uses `Debug`.

- [ ] **Step 4: Implement Merman rendering and host validation**

Choose `HostThemePreset::EditorLight` or `EditorDark` from `request.appearance`. Use `Presentation`, `RenderResourceProfile::Constrained`, Merman's default compatibility parsing, vendored text measurement, a stable diagram ID derived from `item.owner.get()` plus `fragment_ordinal`, and `render_resvg_compatible_svg_sync`. Convert `ResvgCompatibleSvg` with `into_string()`.

After Merman returns, enforce the 8 MiB output limit and reject active SVG/XML constructs and external URI-bearing attributes while allowing same-document fragment references. Do this before usvg parses the sealed result. Configure usvg image resolvers to reject decoded data and all string references. Load only the bundled IBM Plex Sans fonts, resolve CSS paint, and serialize labels as paths. Apply the size and safety gates again to the normalized output. Reject normalized output that still contains `<style>`, `<text>`, or `foreignObject`, then call Makepad `looks_like_svg` and `parse_svg(svg).logical_size()`. Validate dimensions before converting the string to `Arc<[u8]>`. This is an adapter normalization and validation pass over Merman's sealed SVG contract, not a Mermaid parser or second renderer.

- [ ] **Step 5: Implement the bounded cache**

Define `CacheKey { content: Arc<str>, appearance: BlockExtensionAppearance, adapter_schema: u32, merman_version: &'static str }`. Store `Result<RenderedBlockSvg, Arc<str>>`, including deterministic failures. Use insertion-order eviction with `HashMap` plus `VecDeque`; refresh an existing key on access. Evict until both `len() <= 64` and total successful SVG bytes `<= 32 MiB`. Do not include document revision, request ID, item ID, source range, or available width in the key.

`MermaidRenderer` owns `cache: Mutex<MermaidCache>`. Use this exact locking policy:

```rust
fn cached(&self, request: &BlockExtensionRequest) -> Option<BlockRenderResult> {
    let key = CacheKey::from_request(request);
    self.cache.lock().expect("Mermaid cache poisoned").get(&key)
}

fn render_and_cache(&self, request: &BlockExtensionRequest) -> BlockRenderResult {
    let key = CacheKey::from_request(request);
    if let Some(hit) = self.cache.lock().expect("Mermaid cache poisoned").get(&key) {
        return hit;
    }
    // The lock is not held while Merman parses, lays out, or renders.
    let rendered = render_uncached(request);
    let mut cache = self.cache.lock().expect("Mermaid cache poisoned");
    if let Some(winner) = cache.get(&key) {
        return winner;
    }
    cache.insert(key, rendered.clone());
    rendered
}
```

`MermaidCache::get` returns a cloned WAML-owned result and refreshes recency. `MermaidCache::insert` replaces an existing key without double-counting bytes, then enforces both caps before it returns. Never hold the mutex while Merman renders or Makepad parses SVG. Two native workers may render the same cold key concurrently; this is acceptable. The second worker uses the first inserted result if it finds that result during the insertion lock.

- [ ] **Step 6: Test cache identity and bounds**

Assert identical content and appearance render once across different revisions/items, light and dark do not collide, deterministic failure renders once, a 65th small entry evicts the oldest, and byte pressure evicts until the 32 MiB bound holds.

- [ ] **Step 7: Run adapter tests and checks**

Run: `rtk cargo test -p waml-editor markdown_extensions::mermaid`

Expected: PASS for all four families, security/limit tests, Makepad parsing, and cache tests.

Run: `rtk cargo check -p waml-editor --target wasm32-unknown-unknown`

Expected: PASS.

- [ ] **Step 8: Commit**

```text
rtk git add crates/waml-editor/src/markdown_extensions/mermaid
rtk git commit -m "feat(editor): render safe Mermaid SVG"
```

### Task 6: Add the registry, leases, native workers, and cooperative WASM queue

**Files:**
- Modify: `crates/waml-editor/src/markdown_extensions/mod.rs`

**Interfaces:**
- Consumes: `mermaid::renderer()` and `MarkdownBlockExtensionHost`.
- Produces: `SharedMarkdownExtensionHost`, `MarkdownExtensionLease`, `registered_languages()`, and the deferred-work seam below.

```rust
pub type SharedMarkdownExtensionHost = Rc<RefCell<EditorMarkdownExtensionHost>>;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MarkdownExtensionLeaseId(u64);

impl EditorMarkdownExtensionHost {
    pub fn shared() -> SharedMarkdownExtensionHost;
    pub fn open_lease(shared: &SharedMarkdownExtensionHost) -> MarkdownExtensionLease;
}

impl MarkdownExtensionLease {
    pub fn registered_languages(&self) -> RegisteredBlockExtensions;
    pub fn has_deferred_work(&self) -> bool;
    pub fn run_one_deferred(&mut self) -> bool; // true when more queued work remains
}

#[cfg(target_arch = "wasm32")]
struct QueuedRender {
    lease: MarkdownExtensionLeaseId,
    renderer: Arc<dyn FencedBlockRenderer>,
    request: BlockExtensionRequest,
}
```

- [ ] **Step 1: Write host lifecycle and executor tests**

Use a counting fake renderer registered as `mermaid`. Test case-insensitive lookup, an unregistered-language failure that does not invoke any renderer, immediate cache-hit completion, cancellation before completion, closed-lease completion rejection, and result identity preservation.

Under `#[cfg(not(target_arch = "wasm32"))]`, assert a cache miss runs off the caller thread, reaches the channel, and calls the injected wake function once. Under a platform-independent cooperative-queue test seam, enqueue two misses, call `run_one_deferred`, and assert exactly one renderer call and one remaining job; the second call drains the queue. Run cached work immediately without charging the one-miss budget.

- [ ] **Step 2: Run host tests and confirm failure**

Run: `rtk cargo test -p waml-editor markdown_extensions::tests`

Expected: FAIL because the registry, lease maps, and executors are absent.

- [ ] **Step 3: Implement registry and lease identity**

Register one renderer from `mermaid::renderer()`. `MarkdownExtensionLease::registered_languages` borrows the shared host and returns its registry keys as `RegisteredBlockExtensions`; no public host method exposes this set. Keep completed, canceled, and pending maps keyed by `(lease_id, request_id)`, and store the expected revision, item, and source range with each pending entry. `Drop` for a lease retires all its pending work. `drain_events` removes canceled or closed-lease results before they can reach the reading state.

- [ ] **Step 4: Implement native scheduling**

On native cache miss, clone the `Arc<dyn FencedBlockRenderer>`, request, completion sender, and lease ID into `std::thread::spawn`. Send the WAML-owned event and call `SignalToUI::set_ui_signal()` only after a successful send. Never render in `request`, `drain_events`, a draw callback, or an event callback.

- [ ] **Step 5: Implement non-atomics-safe WASM scheduling**

Under `#[cfg(target_arch = "wasm32")]`, push cache misses into `VecDeque<QueuedRender>`. `run_one_deferred` pops and synchronously renders at most one miss, sends it through the same completion admission function, and reports whether another job remains. Do not call `spawn_thread` on WASM. Check cancellation and lease liveness before and after the render.

- [ ] **Step 6: Run host, native, and WASM checks**

Run: `rtk cargo test -p waml-editor markdown_extensions::tests`

Expected: PASS.

Run: `rtk cargo check -p waml-editor --target wasm32-unknown-unknown`

Expected: PASS with the cooperative executor selected.

- [ ] **Step 7: Commit**

```text
rtk git add crates/waml-editor/src/markdown_extensions/mod.rs
rtk git commit -m "feat(editor): schedule Markdown block renderers"
```

### Task 7: Integrate extension leases with the reading view

**Files:**
- Modify: `crates/waml-editor/src/okf_documents.rs`
- Modify: `crates/waml-editor/src/reading_view.rs`
- Modify: `crates/waml-editor/src/generic_okf_view.rs`
- Modify: `crates/waml-editor/tests/markdown_integration.rs`

**Interfaces:**
- Consumes: `SharedMarkdownExtensionHost`, `MarkdownExtensionLease`, `BlockExtensionStates`, and the four-argument `MarkdownViewer::install_document`.
- Produces: `ReadingView::handle_event(&mut self, cx, body, event)` and a complete per-document request lifecycle.

- [ ] **Step 1: Write failing integration tests**

Build a snapshot with valid and invalid Mermaid fences. Assert: valid fences enter loading then ready; invalid fences enter failed without hiding sibling prose; the source text stays byte-for-byte unchanged; a newer revision cancels old work; an old event cannot replace the new block; dropping `ReadingView` closes its lease; switching to source after selecting a diagram places the caret at the full fence start; and a non-Mermaid fence never requests the host.

- [ ] **Step 2: Run the integration tests and confirm failure**

Run: `rtk cargo test -p waml-editor --test markdown_integration mermaid`

Expected: FAIL because `ReadingView` does not own or drive extensions.

- [ ] **Step 3: Open and reconcile the lease**

Create one `SharedMarkdownExtensionHost` in `OkfDocuments` beside its existing shared Markdown image host, then pass clones to every `GenericOkfView`. Change construction so `GenericOkfView` receives both hosts; keep source-editor image behavior unchanged. In `ReadingView`, store `plan`, `source`, `BlockExtensionStates`, `MarkdownExtensionLease`, and `NextFrame`. Build the reading document with `lease.registered_languages()`, reconcile after each installed revision, and install `states.frame(revision)` with the document. Sharing the host shares the Mermaid cache across open documents.

Map `crate::config::theme()` to `BlockExtensionAppearance::Light` or `Dark`. If appearance changes without a document revision, cancel and reconcile the extension states so the theme cache key and rendered SVG change.

- [ ] **Step 4: Drain native completions and drive WASM turns**

Add:

```rust
pub fn handle_event(&mut self, cx: &mut Cx, body: &BodyWidgets, event: &Event)
```

On every routed event, drain admitted events, apply them, install a new frame, and redraw only if state changed. On WASM, if the stored `NextFrame` matches, run exactly one deferred miss, drain its event, and arm one more `cx.new_next_frame()` only when work remains. After reconciliation, arm the first frame when `has_deferred_work()` is true. Native work relies on `SignalToUI` and does not arm a polling frame.

Add an opt-in WASM trace for the browser acceptance check. Under `#[cfg(all(target_arch = "wasm32", feature = "browser-test-trace"))]`, store `browser_trace_generation: u64` and the last traced `(generation, ready, failed, loading)` totals in `ReadingView`. Increment the generation after each document reconciliation. After each extension frame is installed, emit exactly `WAML_TEST_EXTENSION_PENDING generation=<u64> count=<usize> ready=<usize> failed=<usize> loading=<usize>` through Makepad's browser console log only when that tuple changes. Compute all totals from the installed generation frame, and keep `count` equal to `loading`. Emit zero Loading only after the ready/failed frame is installed. Do not compile this trace or its state fields unless `browser-test-trace` is enabled. The fixture contract starts with positive Loading and settles at exactly 8 Ready, 1 Failed, and 0 Loading.

- [ ] **Step 5: Preserve source handoff**

Change `toggle_source` to accept `cx` and `body`. Before switching from reading to source, get `body.markdown_viewer().caret_for_handoff(cx)` and reveal that offset in the read-only `SourceView`. Keep existing text-selection behavior; the visual source range added in Task 3 supplies the fence start for a clicked diagram.

- [ ] **Step 6: Route events and run integration tests**

Call `self.reading.handle_event(cx, body, event)` from `GenericOkfView::route_ui_event` before forwarding to the source view.

Run: `rtk cargo test -p waml-editor --test markdown_integration mermaid`

Expected: PASS.

Run: `rtk cargo test -p waml-editor generic_okf_view`

Expected: PASS for existing reading/source switching and the new source caret test.

- [ ] **Step 7: Commit**

```text
rtk git add crates/waml-editor/src/okf_documents.rs crates/waml-editor/src/reading_view.rs crates/waml-editor/src/generic_okf_view.rs crates/waml-editor/tests/markdown_integration.rs
rtk git commit -m "feat(editor): connect Mermaid to reading view"
```

### Task 8: Add the shared native/browser fixture and visual checks

**Files:**
- Create: `crates/waml-editor/tests/fixtures/mermaid-reading/index.md`
- Create: `scripts/mermaid-browser-check.mjs`

**Interfaces:**
- Consumes: completed reading-view integration.
- Produces: one fixture used without alteration by native `run.ps1` and browser `waml serve`, plus a non-mutating browser check that keeps the server alive for the complete browser observation.

- [ ] **Step 1: Add the fixture**

Create `index.md` with a heading and prose before, between, and after these fences: flowchart, sequence, class, state, a deliberately wide flowchart, invalid Mermaid, `MERMAID`, mixed-case `MeRmAiD`, and a non-Mermaid `rust` fence. Keep each example small enough for fast cooperative WASM turns.

- [ ] **Step 2: Run the native fixture**

Run: `rtk pwsh -File run.ps1 crates/waml-editor/tests/fixtures/mermaid-reading -Title mermaid-reading`

Expected: all valid Mermaid fences render; the wide diagram fits the reading column without horizontal scrolling; invalid Mermaid shows source plus one concise line; the Rust fence stays code; prose after the failure remains visible.

- [ ] **Step 3: Capture and inspect a HiDPI-correct native screenshot**

Run: `rtk pwsh -File scripts/capture-window.ps1 -Out target/mermaid-reading.png -Process waml-editor`

Expected: the screenshot is native pixel size, labels are readable, aspect ratios are correct, the document background matches the diagram theme, and no block overlaps its neighbors.

- [ ] **Step 4: Write the non-mutating browser-lifetime check**

Create `scripts/mermaid-browser-check.mjs` with this command shape:

```text
node scripts/mermaid-browser-check.mjs <path-to-waml(.exe)> <fixture-dir> <screenshot-path>
```

The script must: spawn `<waml> serve <fixture> --port 0 --no-open`; parse the printed URL; keep the child process alive; open that URL with the repository's existing `playwright-core` Chromium; fail on console panic, page error, early server exit, missing canvas, malformed trace, or a 60-second completion timeout; and parse console messages that match `WAML_TEST_EXTENSION_PENDING generation=<u64> count=<usize> ready=<usize> failed=<usize> loading=<usize>`. Wait for positive Loading for one generation, then take the early loading canvas screenshot. Continue until the same generation reports exactly 8 Ready, 1 Failed, and 0 Loading, with `count == loading`. After zero, wait until three consecutive animation-frame canvas screenshots have the same SHA-256 digest, use that stable frame for the final comparison, and assert that its pixels differ from the early loading frame. Save the final full-page screenshot at the requested path; read the fixture file before and after and assert byte equality; close Chromium; then terminate and await the server child. It must not pass based on elapsed frame count alone, call the save API, or write into the fixture directory.

- [ ] **Step 5: Build and run the browser fixture**

Run: `rtk cargo build -p waml-cli --features embed-web`

Run: `rtk node scripts/mermaid-browser-check.mjs target/debug/waml.exe crates/waml-editor/tests/fixtures/mermaid-reading target/mermaid-reading-browser.png`

Expected: `mermaid-browser-check: PASS`, one fixture generation traces positive Loading and then exactly 8 Ready, 1 Failed, and 0 Loading. Three post-zero canvas frames are stable, the stable canvas differs from the loading canvas, fixture bytes are unchanged, and `target/mermaid-reading-browser.png` is present. Open that screenshot and verify the same valid, invalid, wide, and non-Mermaid cases as native. Confirm that valid diagrams are readable, no loading placeholder remains, the invalid block is local, and the Rust fence stays code.

- [ ] **Step 6: Commit**

```text
rtk git add crates/waml-editor/tests/fixtures/mermaid-reading/index.md scripts/mermaid-browser-check.mjs
rtk git commit -m "test(editor): add Mermaid reading fixture"
```

### Task 9: Final boundary, regression, and quality verification

**Files:**
- Modify only files required to correct failures found by this task.

**Interfaces:**
- Consumes: all prior tasks.
- Produces: verified acceptance evidence with no dependency or module leaks.

- [ ] **Step 1: Format and inspect the diff**

Run: `rtk cargo fmt --all -- --check`

Expected: PASS. If it fails, run `rtk cargo fmt --all`, then repeat the check.

Run: `rtk git diff --check`

Expected: PASS with no whitespace errors.

- [ ] **Step 2: Run the full test suite**

Run: `rtk cargo test --workspace`

Expected: PASS.

- [ ] **Step 3: Run strict linting**

Run: `rtk cargo clippy --workspace --all-targets --all-features -- -D warnings`

Expected: PASS with no warnings.

- [ ] **Step 4: Re-run the browser target check**

Run: `rtk cargo check -p waml-editor --target wasm32-unknown-unknown`

Expected: PASS.

- [ ] **Step 5: Prove the Merman boundary**

Run: `rtk cargo tree -i merman`

Expected: only `waml-editor` depends on Merman.

Run: `rtk rg -n "\bmerman\b|merman::" crates --glob "*.rs" --glob "Cargo.toml"`

Expected: Rust references appear only below `crates/waml-editor/src/markdown_extensions/mermaid/`; the sole manifest reference is `crates/waml-editor/Cargo.toml`.

Run: `rtk rg -n "Mermaid|mermaid" crates/waml-syntax crates/waml-markdown-editor/src/presentation crates/waml-markdown-editor/src/reading`

Expected: no Mermaid-specific production type, grammar, family list, or branch appears in reusable crates. Generic test fixture names may contain the registered string.

- [ ] **Step 6: Re-run native and browser visual acceptance**

Repeat Task 8 Steps 2 through 4 after the final code changes. Expected: both platforms show the same content and local failure behavior.

- [ ] **Step 7: Commit final corrections**

```text
rtk git add Cargo.lock crates/waml-editor crates/waml-markdown-editor
rtk git commit -m "fix(editor): complete Mermaid reading verification"
```

If Step 1 through Step 6 required no correction, do not create an empty commit.
