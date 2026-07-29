# Document Header Visual Polish

## Goal

Refine WAML's shared document header so it reads as a compact developer-tool
breadcrumb, using the supplied reference for typography and spacing while
retaining WAML's Atlas palette and existing navigation architecture.

## Visual treatment

- Paint the header with `atlas.canvas_ground`, matching the diagram canvas.
- Separate the header from document content with a one-pixel bottom rule using
  `atlas.surface_border`; do not use a contrasting filled strip.
- Retain the existing 30-pixel header height.
- Use the existing IBM Plex Sans roles: `fonts.text_menu` with `atlas.text_dim`
  for ancestors and `fonts.text_label` with `atlas.text` for the current
  segment.
- Pixel-snap text placement and apply a one-pixel downward optical correction
  for the shared `asc: -0.1` font trim.
- Start breadcrumb content at a 14-pixel visual inset.
- Place a small, dim Lucide `chevron-right` between segments with approximately
  ten pixels of breathing room on both sides.

## Separator

WAML's curated `Icon` enum does not contain `ChevronRight`. Adding it would
change a public API, while the existing `ArrowRight` has a shaft and reads as
an action rather than hierarchy.

The header will therefore use Lucide's single-chevron geometry as a private
SDF draw primitive local to `document_header.rs`. It will follow WAML's
existing generated Lucide draw conventions, use `atlas.text_dim`, and avoid
font-dependent Unicode. The double `chevrons-right` glyph is intentionally not
used because it suggests skipping or fast-forwarding.

## Layout and interaction

Layout remains owned by `layout_header`. It will account for:

- the leading content inset;
- horizontal padding inside every segment hit rectangle;
- a fixed separator slot;
- the unchanged 30-pixel right-dock reservation.

Each visible segment's full padded rectangle remains mapped to its original
`NavigationTarget`. Separator slots are not navigation targets. Ancestors
continue to elide oldest-first, and the current segment remains represented at
every positive available width. The header's start-screen collapse, inspector
placement, wide/narrow geometry, and drag-query behavior remain unchanged.

No ownership boundary, navigation behavior, or public Rust API changes.

## Verification

Focused unit tests will measure:

- symmetric separator spacing and leading inset;
- pixel-snapped geometric centering plus the one-pixel optical correction;
- padded, positive-area hit rectangles mapped to their original targets;
- current-segment retention under narrow and right-dock-constrained widths;
- unchanged content clipping at the right-dock edge.

Matching 1440×900 and 820×900 screenshots will be captured before and after.
The requested focused tests, editor test binary, Clippy, formatting check, and
Git whitespace check will run before the implementation commit. A fresh
reviewer will inspect the final diff and screenshots for visual or layout
regressions.
