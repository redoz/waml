# Markdown reading-view typesetting

Theory baseline for the `MarkdownViewer` surface, and the mapping from that
theory onto the makepad `TextFlow` machinery the viewer drives. Written for
the 2026-08 typesetting pass; update it when the viewer's spacing model
changes.

## How markdown prose should be set

The canon here is uncontroversial — Butterick's *Practical Typography*, the
USWDS type guidance, and GitHub's `markdown-body` stylesheet all converge on
the same numbers.

### The four levers that matter

1. **Point size.** Screen body text wants 15–25 CSS px (≈ 11–19 pt). Below
   that, prose reads as UI chrome.
2. **Leading (line-height).** 120–145 % of the point size for body prose;
   GitHub uses 1.5 for markdown. Headings run *tighter* (1.1–1.25) because
   large sizes exaggerate the gap and multi-line headings must read as one
   unit.
3. **Measure (line length).** 45–90 characters per line, ideal ≈ 66. An
   unconstrained full-width column on a wide window is the single fastest way
   to make text unreadable — the eye loses the return sweep.
4. **Consistent rhythm.** Vertical space must be *hierarchical and
   deliberate*: the gap above a heading is larger than the gap below it
   (a heading binds to what follows, not what precedes); paragraphs are
   separated by a fraction of the line height, not by nothing and not by a
   full blank line of glyph height.

### Reference values (GitHub `markdown-body`, 16 px em)

| Element | Size | Line-height | Space above | Space below |
|---|---|---|---|---|
| body/p | 1 em | 1.5 | 0 | 16 px (1 em) |
| h1 | 2 em | 1.25 | 24 px | 16 px |
| h2 | 1.5 em | 1.25 | 24 px | 16 px |
| h3 | 1.25 em | 1.25 | 24 px | 16 px |
| h4 | 1 em | 1.25 | 24 px | 16 px |
| h5 | 0.875 em | 1.25 | 24 px | 16 px |
| h6 | 0.85 em | 1.25 | 24 px | 16 px |
| list item | 1 em | 1.5 | 0.25 em between items | — |
| code block | 0.85 em mono | 1.45 | 16 px | 16 px |

First child of the document drops its top margin (`>*:first-child
{margin-top: 0}`) so the page doesn't start with a hole.

Inline code must **never change the line's height** — a run of `code` inside
prose keeps the surrounding leading; its background box hugs the glyphs and
is allowed to be slightly shorter than the line box.

## What the viewer was doing wrong (2026-08 audit)

The `MarkdownViewer` widget (`crates/waml-markdown-editor/src/reading/widget.rs`)
drives makepad's `TextFlow`. Audit findings against the theory above:

1. **Zero block spacing.** Every block ended with `new_line_collapsed(cx)`,
   which is `turtle_new_line_with_spacing(0.0)` — no paragraph gap, no
   heading margins at all. The fork's `Html` widget applies
   `heading_margin`/`paragraph_margin` via `new_line_collapsed_with_spacing`;
   the viewer never did. All perceived "spacing" was incidental glyph height.
2. **Inter-line (wrap) gap is ascender-derived and sticky.** In the fork,
   `wrap_spacing = ascender × (line_spacing − 1)` per drawn run
   (`draw_text.rs`), and `Turtle::set_wrap_spacing` takes the **max** seen on
   the turtle. Any run with a different `line_spacing` or size (bold, inline
   code at 0.85×, a heading) perturbs the gap for the lines around it —
   the literal "inconsistent line spacing" symptom. Mitigation on the viewer
   side: give **all five text styles the same `line_spacing`**.
3. **Inline code inflated its line.** `TextFlow::draw_text` grows the row by
   `inline_code_padding + inline_code_margin` vertical extents
   (`allocate_height(code_pad_h)`), so any line containing inline code was
   taller than its neighbours. Mitigated by shrinking the vertical padding to
   1 px and zeroing the vertical margin in the viewer's `TextFlow` config.
4. **No measure control.** `flow_body` was `width: Fill` — 200+ character
   lines on a wide window.
5. **Heading ladder was ad-hoc** (1.8/1.5/1.3/1.15/1.05/1.0, all bold, body
   leading) with no margins, so a heading was just a slightly bigger line
   glued to its neighbours.

## The model the viewer implements now

Spacing is **gap-before-block**, emitted between siblings only (never before
the first block of a container — the `:first-child` rule falls out for free,
and nesting works because gaps are emitted inside whatever turtle the
container opened):

- before a heading: `1.5 em` (of body size, in lpx)
- between list items: `0.25 em`
- between anything else (paragraphs, code, quotes, images, breaks): `0.75 em`

A heading's "space below" is the following block's own `0.75 em` gap — no
after-margins anywhere, which sidesteps CSS-style margin-collapsing
questions entirely.

Sizes: GitHub ladder — h1 2.0×, h2 1.5×, h3 1.25×, h4 1.0×, h5 0.875×,
h6 0.85×, all bold. Headings temporarily tighten the bold styles'
`line_spacing` to 1.25 while they draw.

Leading: all five `TextFlow` text styles pinned to `line_spacing: 1.5` so
mixed bold/italic/code runs cannot perturb the wrap gap (finding 2).

Measure: `max_measure_em` (default 38 em of the body size ≈ 70 characters)
clamps the column; leftover width becomes symmetric side margins, centering
the column.

Every spacing emission records a `None` (structural) piece in the
`SourceMap`, exactly like the pre-existing newline bookkeeping, so
selection→source mapping is unaffected.

## Follow-ups (out of scope for the viewer pass)

- Fork-side: derive `wrap_spacing` from `(ascender + descender)` (or the em)
  rather than ascender alone, and reset the sticky max per paragraph.
- Serif family for viewer prose (goes through the `fonts.rs` role trio).
- Real table rendering (rows still flatten to paragraph flow).

Sources: [Butterick — summary of key rules](https://practicaltypography.com/summary-of-key-rules.html),
[Butterick — line length](https://practicaltypography.com/line-length.html),
[UXPin — optimal line length](https://www.uxpin.com/studio/blog/optimal-line-length-for-readability/),
[github-markdown-css](https://github.com/sindresorhus/github-markdown-css),
[USWDS typography](https://designsystem.digital.gov/components/typography/).
