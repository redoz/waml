# Theme the Diagram

**Goal:** Each diagram is legible in the light theme and in the dark theme, at
each zoom, in the native form and in the web form.

**Why:** A reader opens a share link with the theme of that reader's system. A
diagram that is correct in one theme only is incorrect for many readers.

**Done when:** Each diagram in this bundle is legible in the two themes. The
accent of each kind stays different from the other accents. A change of theme
draws the full view again and shows no frame with the previous theme.

**Status:** partial — unverified
**MVP:** no

## Notes

- There are two themes: light and dark. There is no third theme. Light is the
  reference.
- Accents for each kind and node styles operate. A theme atlas holds them.
- Zoom is the weak part, not colour. The tool makes a raster of the text at
  each zoom size. Thus one zoom step needs some hundreds of milliseconds.
- `MVP: no`. The bar needs a legible diagram. The bar does not need an
  attractive diagram. Change the flag to `yes` if a diagram is not legible in
  the dark theme.
