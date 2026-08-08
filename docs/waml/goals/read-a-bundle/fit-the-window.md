# Fit the Window

**Goal:** The editor keeps its responsive controls usable in narrow and wide
viewports.

**Why:** A reader can open the editor on devices with different viewport
widths.

**Done when:** The shell enters and leaves narrow mode with hysteresis, and the
tree toggle stays at the responsive shell edge in both layouts.

**Status:** done
**MVP:** yes

## Shipped behavior

#### NATIVE-006 — shell width changes use hysteresis

**Applies to:** native

**Given** the editor is in a wide or narrow shell mode
**When** the viewport width crosses a responsive threshold
**Then** the editor enters narrow mode below 640 pixels and leaves it above 680 pixels
**And** the current mode remains stable inside the hysteresis band

**Evidence:** `crates/waml-editor/src/app/tests/menus.rs::breakpoint_enters_below_640_and_leaves_above_680` `crates/waml-editor/src/app/tests/menus.rs::breakpoint_preserves_mode_through_the_hysteresis_band`

#### NATIVE-007 — the tree toggle follows the responsive shell edge

**Applies to:** native

**Given** the editor presents the responsive shell
**When** the shell changes between wide and narrow layout
**Then** the tree toggle stays at the column edge or the row head for that layout

**Evidence:** `crates/waml-editor/src/app/tests/shell.rs::the_toggle_rides_the_column_edge_and_falls_back_to_the_row_head`

## Notes

- Dock state and non-width shell behavior are owned by
  [Use the Shell](./use-the-shell.md).
- The current prose workaround for responsive view anchors and post-draw
  results is recorded in
  [FG-003](../../waml-feature-gaps.md#fg-003--view-anchors-and-eventual-draw-results).
