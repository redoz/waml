# Use the Shell

**Goal:** A reader controls dock state and uses the document shell without
losing the presented layout.

**Why:** The shell keeps navigation, history, documents, and supporting panels
available around the active document.

**Done when:** A dock keeps its presented geometry through close motion, dock
areas move to their wide or narrow positions, and history controls stay between
the tree column and the tab strip.

**Status:** done
**MVP:** yes

## Shipped behavior

#### NATIVE-008 — dock close motion keeps the presented geometry

**Applies to:** native

**Given** a dock is open in the document shell
**When** the reader closes the dock
**Then** the dock keeps its presented geometry until the close motion completes

**Evidence:** `crates/waml-editor/src/app/tests/shell.rs::mounted_dock_close_keeps_presented_geometry_until_motion_completes`

#### NATIVE-009 — dock areas follow the responsive layout

**Applies to:** native

**Given** the document shell has dock areas
**When** the shell uses its wide or narrow layout
**Then** each dock area appears in the position for that layout

**Evidence:** `crates/waml-editor/src/app/tests/shell.rs::mounted_dock_areas_follow_wide_and_narrow_production_layout`

#### NATIVE-055 — history controls stay ahead of the tab strip

**Applies to:** native

**Given** the tree column and document tabs are visible
**When** the editor presents the document shell
**Then** the Back and Forward controls appear after the tree column and before the tab strip

**Evidence:** `crates/waml-editor/src/app/tests/shell.rs::mounted_history_buttons_lead_the_tab_strip_past_the_tree_column`

## Notes

- Responsive threshold and tree-toggle behavior are owned by
  [Fit the Window](./fit-the-window.md).
