# Markdown Editor Emphasis Design

## Purpose

Make the markdown editor suitable for code-focused editing without removing its
rich markdown presentation. The widget exposes the caller's intent as an
emphasis profile. It does not expose independent font and row-spacing controls.

## Public Model

Add `EditorEmphasis` with two values:

- `Code` is the default.
- `Layout` is the explicit page-like alternative.

The markdown editor widget accepts one emphasis value. The widget resolves that
value centrally into its internal typography and layout settings. Callers do not
combine individual density and font settings.

Backward compatibility is not a requirement. Existing widget instances adopt
the `Code` default unless they select `Layout`.

## Code Emphasis

`Code` has these properties:

- Use the project's mono font family for all markdown text.
- Set the base vertical padding of each source row to zero.
- Keep the existing horizontal inset between the gutter and row content.
- Keep rows variable-height. Font metrics, semantic font sizes, wrapping, and
  inline decorations can increase a row's height.
- Preserve bold, italic, bold-italic, underline, strikethrough, links, inline
  code, diagnostics, and other rendered decorations.

Zero row padding does not impose a fixed line height. It removes only the
additional top and bottom space around the row's content.

## Layout Emphasis

`Layout` retains the current page-like typography and vertical rhythm. It uses
the existing proportional and mono face selection, semantic sizes, block
spacing, and row padding.

The two profiles use the same markdown document, presentation plan, selection,
input, and decoration pipelines. Only resolved presentation settings differ.

## Resolution Boundary

One resolver maps `EditorEmphasis` to the complete internal presentation
settings used by layout, shaping, and drawing. The widget supplies the resolved
settings consistently when it builds or rebuilds layout.

Changing emphasis invalidates cached layout and drawing data that depends on
typography or row geometry. The next draw rebuilds those values from the same
document session. It does not change document content, selection ranges, or
revision authority.

Future presentation knobs can join the resolver without expanding the public
widget API. A new public option is necessary only when it represents a new user
intent that cannot fit `Code` or `Layout`.

## Errors and Fallbacks

Emphasis resolution is total and does not return an error. Font loading follows
the widget's existing font fallback behavior. A missing preferred mono face must
not prevent layout or editing.

## Verification

Automated tests cover:

- `Code` is the default emphasis.
- `Code` resolves to the mono family and zero vertical row padding.
- `Layout` resolves to the current page-like settings.
- Both profiles keep the existing horizontal inset.
- A plain code-emphasis row loses only its vertical padding.
- A heading, styled run, wrapped run, or inline decoration can still make a
  code-emphasis row taller.
- Bold, italic, bold-italic, underline, strikethrough, links, inline code, and
  diagnostics remain in the presentation and draw output under `Code`.
- Changing emphasis invalidates dependent layout and produces updated geometry
  without changing document or selection state.

The editor is also launched with a unique `-Title` slug and captured at native
pixels. The visual check compares `Code` and `Layout` with the same markdown
fixture and confirms compact code-editor rhythm plus variable-height semantic
and decorated rows.

## Scope

This change does not add a general-purpose typography editor, per-document
preference storage, a user-facing settings control, or fixed-height rows. It
does not change markdown parsing, editing semantics, or decoration meaning.
