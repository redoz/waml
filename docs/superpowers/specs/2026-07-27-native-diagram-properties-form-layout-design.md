# Native Diagram Properties Form Layout

**Date:** 2026-07-27
**Status:** Approved in conversation

## Goal

Make the native Diagram Properties form read as a compact settings form rather
than a collection of controls stretched across the canvas. Preserve the
existing four-section information architecture and cardinality semantics.

## Form geometry

The properties view remains full-height and keeps its full-width header and
background. Its body contains one left-anchored form column:

- Maximum width: 620 logical pixels.
- Width below the maximum: available body width minus the normal left and
  right gutters.
- Left and right body gutters: 20–24 logical pixels.
- Alignment: left, not centered in the full canvas.

This keeps the form visually attached to the navigator and tab while avoiding
extreme control spans on ultrawide displays. Narrow windows shrink the form
fluidly rather than clipping it.

Within the bounded column:

- Toggle controls align to the column's right edge.
- The attribute-cardinality segmented control is 260–300 logical pixels wide
  and right-aligned; it does not fill the entire row.
- The Max attributes input remains compact.
- Text fields fill the bounded column.

## Typography

Do not alter the shared typography tokens globally. Diagram Properties gets a
compact, local control-label treatment:

- Page heading: existing 13-unit semibold heading.
- Section headings: existing 10-unit semibold eyebrow.
- Title and Note captions: 10–11-unit medium label.
- Property-row labels: 11-unit regular.
- Input text: 11–12-unit regular.
- Property rows: approximately 26 logical pixels high.

The important correction is that property-row labels no longer use the
12-unit global body style. The local style must preserve IBM Plex Sans and the
Atlas color tokens.

## Multiline Note

Use Makepad's existing multiline `TextInput` support:

- `is_multiline: true`.
- Fixed initial height of approximately 88 logical pixels.
- Normal text-field padding and focus treatment.
- Internal scrolling for overflow.
- Placeholder: `Optional note`.

Line breaks are data, not merely presentation. Remove the current
newline-to-space normalization and preserve CRLF/LF input as normalized LF.
Verify that multiline descriptions survive the full edit, operation,
serialization, parsing, and reopen cycle. If the WAML frontmatter serializer
cannot represent literal newlines safely in its current quoted-string form,
add the smallest compatible escaping or block-scalar support rather than
silently flattening content.

## Responsive behavior

- Ultrawide: form stays 620 pixels wide and left anchored.
- Standard desktop: form uses the same maximum width.
- Narrower than the maximum plus gutters: form becomes `Fill`.
- The form remains vertically scrollable.
- No horizontal scrollbar is introduced.

The diagram tool dock remains hidden while Diagram Properties is open, as
specified by the preceding alignment design.

## Verification

- Pure layout tests cover the width clamp and narrow-window behavior if the
  Makepad layout can be isolated without brittle rendering assertions.
- Native widget/state tests cover multiline Note preservation.
- Parser/operation round-trip tests cover embedded line breaks.
- A native screenshot is captured at a standard width and an ultrawide width.
- The ultrawide screenshot shows a bounded left-anchored form rather than
  full-width controls.
- Existing native, WAML, core, and web tests remain green.
