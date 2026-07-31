# Markdown presentation and motion — design

**Date:** 2026-07-31
**Status:** Approved in conversation; written-spec review pending
**Sequence:** 3 of 4
**Depends on:** Incremental Markdown syntax platform; Markdown editor foundation

## Problem

The target is neither a split preview nor WYSIWYG with hidden source. Users must
edit the real Markdown document while seeing its semantic formatting in place.
That requires source delimiters and rendered hierarchy to share one layout.

Mixed metrics also cause surrounding content to move when syntax changes. Hard
snaps would make headings, wrapping, images, and tables feel unstable compared
with Makepad's motion language.

## Goal

Present canonical Markdown source as a balanced formatted document:

- every source character remains visible and editable;
- syntax markers are visually dim but never hidden;
- semantic content receives Markdown typography and decoration;
- non-text constructs render alongside their literal source;
- displaced content transitions smoothly to new geometry.

## Visual direction

Use the approved **balanced** treatment:

- moderate heading scale and block spacing;
- readable proportional body text;
- stronger hierarchy than a code editor without editorial-scale layout churn;
- 24 logical pixels of padding around the editing column;
- no centered maximum-width column in the initial version: content fills the
  available surface inside the inset.

Syntax markers use a low-contrast text role. Markers in the active construct
gain modest contrast so users can inspect the exact Markdown without changing
layout. Formatting remains applied while the caret is inside a construct.

## Presentation plan

Convert syntax queries from spec 1 into immutable presentation items:

```rust
pub enum PresentationItem {
    TextRun {
        range: TextRange,
        role: TextRole,
        style: TextStyle,
    },
    BlockDecoration {
        owner: SyntaxIdentity,
        kind: BlockDecorationKind,
    },
    EmbeddedBlock {
        owner: SyntaxIdentity,
        source_range: TextRange,
        kind: EmbeddedBlockKind,
    },
}
```

Items are keyed by syntax identity, source range, semantic role, and fragment
ordinal. The layout and motion systems use that identity; the source range
remains the authority for editing and hit-testing.

No presentation code recognizes Markdown with regexes.

## Construct treatment

- **Headings:** marker dim; content scaled and weighted by level.
- **Emphasis/strong/strikethrough:** delimiters dim; content styled.
- **Links:** brackets and destination syntax dim; label receives link styling.
  Normal clicks edit; Ctrl/Cmd-click requests navigation.
- **Lists:** source marker remains in flow; content uses hanging indentation.
- **Task lists:** brackets remain visible; the non-interactive checkbox
  decoration mirrors state but does not replace the source characters. Clicking
  the source edits it normally.
- **Block quotes:** `>` remains dim; the block receives an inset rule and quote
  color.
- **Inline code:** backticks remain dim; content uses code typography and fill.
- **Fenced code:** fences and info string remain visible; content uses a code
  block surface and registered language highlighting.
- **Tables:** pipes, delimiter row, and alignment colons remain visible; cells
  align from parsed column structure and the header receives emphasis.
- **Images:** the literal `![alt](path)` remains as an editable source line; the
  resolved image renders as a block immediately beneath it.
- **Thematic breaks:** source markers remain visible and a subtle rule is drawn
  without replacing them.
- **Raw HTML:** remains visible and source-styled. It is not executed.
- **Invalid/incomplete syntax:** remains ordinary editable text with diagnostics;
  presentation never guesses destructive structure.

## Code highlighting

Fenced-code language identifiers route content to a registered highlighter.
WAML-owned languages consume their existing syntax snapshots directly. Other
supported languages may use WAML-owned lexical highlighters adapted from
Makepad, with recorded provenance.

Unknown languages render as unclassified code. The native editor never starts
or round-trips through an LSP server merely to color a code block.

## Asset behavior

Image resolution is asynchronous and revision-bound. While loading, a compact
placeholder occupies the embedded block. Success replaces it with the measured
image; failure shows a compact error placeholder. The literal source line is
unchanged in all states.

Relative paths resolve through the host document's bundle path. Remote asset
policy remains controlled by the application host. Presentation code cannot
silently fetch arbitrary network resources.

## Motion model

Each committed edit creates previous and target layout snapshots.

- Newly typed or deleted source responds immediately.
- Surviving glyphs and blocks with matching stable identities interpolate from
  previous to target geometry.
- Caret and selection are drawn from the same interpolated geometry, so they
  remain attached to moving text.
- Scroll anchoring keeps the active caret at a stable viewport position when
  upstream content changes height.
- Image arrival, heading conversion, wrapping, list indentation, and table
  remeasurement use the same transition system.

The default transition is a 100 ms ease-out using Makepad's animation timing
primitives. Duration and curve are live design tokens, not hard-coded
throughout layout.

Animation cuts directly to the target when:

- reduced motion is enabled;
- the document is initially loaded or externally replaced;
- a large paste or bulk rewrite exceeds the configured visible-change budget;
- old and new snapshots do not share a safe identity mapping;
- the affected geometry is outside the viewport.

## Draw order

Within each visible block:

1. block backgrounds and quote/table decorations;
2. selection;
3. text and syntax markers;
4. diagnostics and link decoration;
5. embedded blocks;
6. caret and IME composition.

All layers consume the same interpolated layout snapshot. Independent geometry
reconstruction by selection, diagnostics, or caret drawing is prohibited.

## Error handling

- A missing style role falls back to body text without losing source mapping.
- A failed highlighter renders unclassified code.
- A missing image renders an error placeholder beneath intact source.
- A motion identity mismatch cuts to target geometry.
- Layout recovery for one block does not disable editing elsewhere.
- Raw HTML and unsafe links are never executed as a side effect of drawing.

## Testing

### Presentation

- Golden presentation plans for every CommonMark/GFM construct.
- Every source byte belongs to exactly one editable text mapping.
- Marker/content ranges remain correct for nesting and malformed input.
- Active-marker contrast changes style only, never geometry.
- Raw HTML is visible and never executed.

### Layout

- Balanced typography and 24-pixel inset at supported DPI scales.
- Hanging lists, nested quotes, code blocks, tables, and image blocks.
- Source-to-screen round trips across every delimiter boundary.
- Viewport resize, wrapping, and off-screen virtualization.

### Motion

- Deterministic interpolation at start, midpoint, and target time.
- Stable identities move surviving content rather than recreating it.
- Caret, selection, diagnostics, and embedded blocks share interpolated
  geometry.
- Reduced-motion and bulk-edit cutovers.
- Scroll anchoring through upstream height changes.

### Visual verification

Capture native, HiDPI-correct screenshots for headings, nested inline syntax,
lists, quotes, code, tables, images, invalid Markdown, active selection, and
motion end states. Motion itself receives a deterministic frame sequence or
short capture in addition to static screenshots.

## Success criteria

- Users always edit visible literal Markdown.
- Balanced formatting clearly communicates rendered structure.
- Syntax markers are dim, selectable, and never hidden.
- Images and other decorations do not replace source.
- Every moving layer follows one geometry transition.
- Common editing remains responsive while reflow appears continuous.
