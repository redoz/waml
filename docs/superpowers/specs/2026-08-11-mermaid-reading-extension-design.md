# Mermaid Reading Extension Design

**Date:** 2026-08-11  
**Status:** Approved for autonomous implementation  
**Scope:** Read existing Mermaid fences in the rendered Markdown viewer

## Summary

WAML will render fenced Markdown blocks whose info-string language is
`mermaid`. The Markdown parser will continue to parse only Markdown. A generic
fenced-block extension boundary will route Mermaid source to a dedicated
Merman adapter. Merman will parse, lay out, and render the diagram as safe SVG.
The Markdown viewer will show that SVG as a read-only embedded block.

All Mermaid grammar, Merman API use, configuration, cache keys, and error
translation will stay in one module directory:

```text
crates/waml-editor/src/markdown_extensions/
|-- mod.rs
`-- mermaid/
    |-- mod.rs
    |-- renderer.rs
    |-- cache.rs
    `-- error.rs
```

No Merman type can cross the public boundary of this directory.

## Goals

- Render existing ```` ```mermaid ```` fenced blocks in `MarkdownViewer`.
- Route every Mermaid fence to the pinned Merman renderer. Merman owns diagram
  detection and supported-family decisions.
- Work in native desktop and browser/WASM builds.
- Keep Mermaid and Merman out of the Markdown syntax parser.
- Keep the source editor behavior unchanged.
- Keep rendering failures local to one block.
- Provide a generic extension seam that can support another fenced-block
  renderer later without adding its grammar to the Markdown parser.

## Non-goals

- Mermaid completion, hover, linting, formatting, or refactoring.
- A second Mermaid parser in WAML.
- A native Makepad implementation of Mermaid layout.
- Diagram editing or interaction inside the reading view.
- Link activation, scripts, remote assets, or Mermaid JavaScript.
- Export commands or a standalone Mermaid viewer.
- Support for language aliases other than the ASCII case-insensitive word
  `mermaid`.

## Dependency and Rust version

`waml-editor` will pin Merman exactly:

```toml
merman = { version = "=0.8.0-alpha.5", default-features = false, features = ["complete-svg"] }
```

The exact pin prevents an alpha release from changing the adapter contract on
a normal dependency update. Upgrades require an explicit change and a fixture
review.

Merman 0.8.0-alpha.5 declares Rust 1.95. The WAML workspace currently declares
Rust 1.80. The implementation will raise the `rust-version` of only the
`waml-editor` application crate to 1.95. `waml-syntax`,
`waml-markdown-editor`, and the other reusable workspace crates will keep the
workspace Rust 1.80 declaration. This keeps the Merman cost at the application
boundary.

## Ownership and dependency direction

### `waml-syntax`

The syntax crate will make no Mermaid-specific change. It already records a
fenced block's owner, source range, info range, content range, raw info string,
and parsed language in `FencedCodeInfo`.

### `waml-markdown-editor`

The Markdown widget crate will own only generic extension concepts:

- a fenced-extension presentation item;
- a stable item identity and source/content ranges;
- loading, ready, and failed resolution states;
- host request, cancellation, and completion messages;
- reading-model and widget support for a rendered SVG block.

It will not depend on Merman and will not contain a `Mermaid` AST or error type.

### `waml-editor`

The application crate will own the extension registry, request scheduling, and
the Mermaid adapter. Only files below `markdown_extensions/mermaid/` may import
`merman` or name a Merman type.

The generic registry maps an ASCII case-insensitive language key to a block
renderer. Version one registers one renderer:

```text
mermaid -> markdown_extensions::mermaid
```

An unregistered fence remains a normal code block.

## Generic extension contract

The presentation compiler will preserve generic fenced-code metadata after the
Markdown parse. It will not ask which extensions the application registered.
When the reading document is built, the caller supplies the generic set of
registered language keys. A matching fence becomes an embedded extension item;
an unmatched fence remains a normal code block. The extension item will
preserve:

- the presentation item ID;
- the syntax owner;
- the full fenced-block source range;
- the content range without the fence and info string;
- the normalized language key.

The host boundary will follow the existing Markdown image-host lifecycle. Its
public data will contain WAML-owned values only. The conceptual API is:

```rust
pub trait MarkdownBlockExtensionHost {
    fn request(&mut self, request: BlockExtensionRequest);
    fn cancel(&mut self, request_id: BlockExtensionRequestId);
    fn drain_events(&mut self) -> Vec<BlockExtensionEvent>;
}
```

A ready event contains sanitized SVG bytes and verified logical dimensions. A
failed event contains a stable user-facing message. The event also repeats the
document revision, item ID, request ID, and source range so the consumer can
reject stale or misrouted work.

The reading model will keep an embedded block as one source-addressable unit.
It will not copy Merman's semantic model. The viewer source map will associate
the rendered block with the original fenced source range so search, selection,
and source handoff remain coherent.

## Mermaid module

### `mod.rs`

This file exposes one WAML-owned adapter entry point and constructs the module's
cache. It is the only entry visible to the generic registry.

### `renderer.rs`

This file owns all Merman calls and configuration. It will:

1. accept the fence content, WAML theme inputs, and a stable diagram ID;
2. call the pinned Merman complete-SVG path;
3. select Merman's resvg-compatible/readable SVG pipeline so Makepad never has
   to support browser `foreignObject` labels;
4. disable network-dependent behavior and unsafe link/script behavior;
5. validate the returned SVG root and logical dimensions;
6. return WAML-owned SVG bytes and dimensions.

Merman will detect the diagram family. WAML will not maintain a family list or
perform a second parse.

### `cache.rs`

The cache key will include:

- the exact fence content;
- the effective light/dark Markdown theme inputs;
- the adapter schema version;
- the pinned Merman version.

The document revision is not part of the key. Identical diagrams can therefore
reuse a result across revisions and documents. The cache will be bounded by
entry count and total SVG bytes. It will cache deterministic failures for the
same key so an invalid fence does not render again on each frame.

### `error.rs`

This file will translate Merman errors and local validation failures into a
small stable error enum. Raw debug output and Merman types will not reach the
widget. Logs may keep detailed diagnostics, but the viewer message must be
short and safe to display.

## Data flow

1. `waml-syntax` parses the Markdown snapshot and records fenced-code metadata.
2. The presentation compiler sees the `mermaid` language and creates a generic
   fenced-extension item.
3. `ReadingView` builds and installs a reading document with an unresolved
   embedded-block placeholder.
4. A reading-view extension lease reconciles live items against the current
   presentation plan.
5. The application extension host checks the Mermaid cache.
6. A cache miss becomes a platform-specific render job.
7. The Mermaid module calls Merman and returns ready SVG or a translated error.
8. The host posts a completion signal to the UI.
9. `ReadingView` drains completions, rejects stale results, updates the block
   state, and requests a redraw.
10. `MarkdownViewer` draws the ready SVG, or the defined loading/failure state.

Changing or closing a document cancels its lease. Cancellation does not have to
stop a Merman call already in progress, but its late result cannot enter the
cache-to-view installation path for a retired request.

## Native and browser execution

The generic scheduler will have two executors behind one request/event API.

### Native

Native builds will run Merman on worker threads. Completion travels through a
channel and wakes the UI with `SignalToUI`. Rendering must never block a native
draw or event callback.

### Browser/WASM

The default Makepad browser target can run without WASM atomics. In that mode,
Makepad's `spawn_thread` implementation is a no-op. The implementation must not
use it as the browser execution guarantee.

Version one will use a cooperative WASM queue:

- enqueue requests during reconciliation;
- execute at most one cache miss during a deferred UI turn/frame;
- install the result through the same request/event validation path;
- yield before the next queued diagram.

One Merman render remains synchronous inside that turn, so a very large diagram
can consume one slow frame. Source and output limits below bound this risk. A
future Web Worker executor can replace this queue without changing the parser,
reading model, viewer, or Mermaid adapter.

Both executors must produce the same result and error types. Platform code must
not enter `markdown_extensions/mermaid/renderer.rs`.

## Display and sizing

The viewer will treat a rendered diagram as a block, not as inline text.

- The block uses the same readable content column as other reading blocks.
- It preserves the SVG aspect ratio.
- It scales down when its intrinsic width is greater than the available
  content width.
- It does not add horizontal scrolling in version one.
- It uses the document background and Merman theme chosen by the adapter.
- Loading uses a stable placeholder height to avoid repeated zero-height
  invalidation.
- Ready geometry replaces the placeholder and redraws the document.

The viewer will use Makepad's existing dynamic SVG image path:
`Image::load_svg_from_shared_data` followed by `Image::draw_walk_image`. That
path keeps shared SVG bytes by identity, parses them only when the allocation
changes, and draws through `DrawSvg`. The adapter returns SVG rather than PNG so
native and browser output share one source and avoid a fixed raster scale. The
feature must not add a second SVG parser.

## Failure behavior

A Mermaid failure is local to its fenced block. The rest of the document must
remain visible.

The failed block will show:

- the original fence content as a normal monospace code block; and
- one short line such as `Cannot render Mermaid: <message>`.

The viewer must not show Rust debug formatting, a backtrace, or internal Merman
type names. Switching to source must still place the caret in the original
fence. A stale result must not replace a newer revision's block.

If the language is not registered, the fence remains a normal code block and no
error appears.

## Resource and security policy

Markdown content can be untrusted. The host will enforce these rules before it
installs SVG:

- maximum Mermaid source size: 256 KiB per fence;
- maximum returned SVG size: 8 MiB per block;
- finite, positive logical dimensions with a maximum side of 16,384 units;
- no network fetches;
- no scripts or event-handler attributes;
- no active external links;
- no browser `foreignObject` requirement;
- no unsanitized user CSS injection.

Merman's own parser and nesting limits remain enabled. The WAML limits are host
limits and do not duplicate Mermaid grammar validation.

## Tests

### Syntax and presentation

- Existing fenced-code query tests remain unchanged.
- `mermaid`, `MERMAID`, and mixed-case language keys route to the extension.
- A fence with another language remains a code block.
- The extension item preserves exact full and content source ranges.
- Source partition validation remains exact.

### Mermaid module

- Flowchart, sequence, and class fixtures render through Merman.
- At least one additional Merman-supported family proves there is no WAML
  family allowlist.
- Invalid source returns a stable translated error.
- Theme changes produce distinct cache keys.
- Identical source and theme reuse one cached result.
- Source, SVG-size, and dimension limits reject oversized output safely.
- Output passes the selected Makepad SVG parser.

### Host lifecycle

- A revision requests each live block once.
- Removing a block cancels its request.
- A result for an older revision is ignored.
- A result for a closed lease is ignored.
- Native worker completion wakes the UI.
- The WASM queue processes at most one cache miss per deferred turn.
- Native and WASM executors use the same cache and result validation rules.

### Reading model and widget

- Loading, ready, and failed blocks preserve their source ranges.
- A ready SVG has non-zero recorded draw geometry.
- Wide diagrams fit the reading column and preserve aspect ratio.
- A failed block draws the original code and its concise message.
- Search/source handoff maps the diagram to its original fence.
- Switching between rendered and source views retains existing behavior.

### Build and visual verification

- `cargo test --workspace`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo check -p waml-editor --target wasm32-unknown-unknown`
- a native viewer fixture launched with `run.ps1 -Title mermaid-reading`
- a HiDPI-correct screenshot captured with
  `scripts/capture-window.ps1 -Out <path> -Process waml-editor`
- a browser fixture that renders the same Mermaid examples
- `cargo tree -i merman` confirms that only `waml-editor` depends on Merman

## Acceptance criteria

The feature is complete when:

1. An existing Markdown document with a valid `mermaid` fence renders a diagram
   in the reading view on native and browser/WASM builds.
2. WAML sends the fence content directly to Merman and has no diagram-family
   parser or allowlist.
3. Mermaid/Merman-specific Rust code exists only below
   `waml-editor/src/markdown_extensions/mermaid/`.
4. `waml-syntax` and `waml-markdown-editor` do not depend on Merman.
5. Invalid Mermaid source degrades to readable source plus a local message.
6. Old asynchronous results cannot replace newer document content.
7. The full workspace tests, clippy, native fixture, and WASM check pass.
