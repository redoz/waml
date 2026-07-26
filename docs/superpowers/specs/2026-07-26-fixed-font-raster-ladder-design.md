# Fixed Font Raster Ladder

## Decision

Large canvas text uses a fixed approximately 1.25× geometric ladder of raster
sizes:

`32, 40, 50, 63, 79, 99, 124, 155, 194, 243, 304`

The nearest rung is assigned to `DrawText.text_style.font_size`.
`DrawText.font_scale` remains `target_size / raster_size`, preserving the exact
visual size.

## Scope

- Remove the mutable font-size LRU, zoom dwell timer, and application-side
  hit/miss logging.
- Apply the ladder to canvas text whose target size exceeds 32 points.
- Preserve the existing direct-size path at or below 32 points.
- Do not change `node_design_editor.rs`; it is not in use.
- Do not modify Makepad.

## Rationale

The application-side LRU does not own or evict Makepad atlas entries, so it
cannot bound the actual atlas. A fixed ladder bounds the large raster sizes the
canvas requests and is deterministic across draw order and zoom history.
Fibonacci spacing is rejected because adjacent sizes are too far apart for
reliable visual scaling.

## Verification

- Unit tests cover nearest-rung selection, midpoint behavior, and the upper
  bound.
- The full `waml-editor` test suite and strict Clippy must pass.
