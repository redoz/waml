# Markdown editor: squiggle underlines and end-of-row diagnostic messages

## Problem

The markdown editor surfaces diagnostics as a flat 2px quad under the offending
range (`widget.rs`, `DecorationRole::DiagnosticUnderline`), painted in a single
`diagnostic_color` regardless of severity. The diagnostic's `message` field
already exists on `PresentedDiagnostic` and is never drawn, so the reader sees
that something is wrong but never what.

Two changes:

1. Replace the flat underline with the classic antialiased squiggle, coloured by
   severity.
2. Draw the diagnostic's message at the end of the row it ends on.

## Scope

Both surfaces that drive the presentation pipeline get this: the markdown editor
widget and the WAML source view. There is no opt-in flag; one implementation
serves both.

## Design

### 1. Squiggle rendering

A new `#[live] draw_squiggle` field on `MarkdownEditor`, backed by a dedicated
`DrawQuad` shader. `draw_decoration` stays a plain `DrawColor` and continues to
serve link underlines and strikethroughs untouched.

The pixel shader computes the analytic distance from the fragment to a sine
curve and strokes it with `smoothstep`:

- amplitude ~1.5px
- period ~4px
- stroke ~1px
- antialiased

It must not use `sdf.box(..., 0)` — at radius zero that floods the quad in this
fork. Analytic distance only.

The wave phase is locked to absolute document x, carried on the instance, so the
squiggle does not crawl when the viewport scrolls or when text reflows around it.

`paint_command` splits the decoration arm:

- `DecorationRole::LinkUnderline` — flat quad via `underline_rect`, unchanged.
- `DecorationRole::DiagnosticUnderline(severity)` — `draw_squiggle` over a band
  from a new `squiggle_rect` helper. The squiggle needs roughly 4px of vertical
  room, where `underline_rect` yields 2px.

The single `diagnostic_color` is replaced by three theme colours — error,
warning, information. Both the squiggle and the message text read them.

### 2. Message emission

Message placement is computed in `build_draw_commands`, not at paint time, so it
lives in the pure, unit-tested draw-command layer alongside every other command.

A new command variant:

```rust
DrawCommand::DiagnosticMessage {
    line: TextRange,
    rect: Rect,
    text: Arc<str>,
    severity: PresentedDiagnosticSeverity,
}
```

It sits in `DrawLayer::Decoration`, which is already ordered after
`DrawLayer::Text`.

The emission rule:

1. Bucket `frame.diagnostics` by the visual line (`layout.visual_lines()`)
   containing the diagnostic's `range.end()`.
2. Per line, pick the worst severity — Error > Warning > Information. Ties break
   on the earliest `range.start()`, which makes the order total: two diagnostics
   never contend for the same slot.
3. `text` is the winning diagnostic's message. When the line holds N other
   diagnostics, the message is suffixed `" +N"`.
4. `rect.pos.x` is the maximum right edge of that line's glyph clusters plus
   `MESSAGE_GAP` (12px). The message therefore follows the text and moves as the
   row is edited.
5. `rect` takes its y and height from the visual line's rect; the widget centres
   the mono run inside it.
6. The message is ellipsized to `layout.viewport_width()`: characters are
   dropped from the end and `…` appended. No wrapping, no row growth, no hard
   clip.

The message is decoration, never document text. It is not selectable, and caret
motion and hit-testing ignore it entirely.

### 3. Measuring inside a pure function

`build_draw_commands` has no font access, but ellipsizing needs a width. Since
the message renders in the mono face, its width is `chars * advance`. So
`PresentationStyles` gains a `diagnostic_message_advance: f64`, measured once by
the widget through the same path `gutter_metrics` already uses and installed with
the styles.

The consequence, stated plainly: a message containing CJK or other wide
characters overruns the computed width slightly. This is accepted — diagnostic
messages are ASCII parser text.

### 4. Message styling

Mono face at the gutter's size convention (~0.75 scale), coloured by severity. It
reads as chrome rather than prose. No leading glyph — colour carries the severity.

## Testing

`draw_layers.rs` and `presentation_model.rs` cover the emission rule:

- diagnostics bucket onto the visual line holding their end offset
- worst severity wins a contested line
- ties break on earliest start
- `+N` counts the losers on that line
- placement sits `MESSAGE_GAP` past the last cluster's right edge
- ellipsize fires exactly at the viewport-width boundary

The squiggle shader gets no unit test. A visual check is owed and must be
scheduled explicitly: in this codebase an implicit visual check does not happen.
