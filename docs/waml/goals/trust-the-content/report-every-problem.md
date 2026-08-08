# Report Every Problem

**Goal:** Diagnostics from each analysis layer reach each applicable user
surface.

**Why:** A detected problem is not useful if the product hides it.

**Done when:** The editor, command line, and language server show each
applicable positioned diagnostic without discarding a layer.

**Status:** partial
**MVP:** yes

## Shipped behavior

#### NATIVE-032 — pending edits preserve only unchanged diagnostic ranges

**Applies to:** native

**Given** a document has diagnostics and pending source changes
**When** the editor maps a diagnostic range to the current source
**Then** an unchanged range keeps its current position
**And** a changed range is not presented as current

**Evidence:** `crates/waml-editor/src/editor_session/tests.rs::pending_source_changes_map_only_unchanged_diagnostic_ranges`

#### NATIVE-050 — source navigation publishes a status result

**Applies to:** native

**Given** source-range navigation changes the current selection
**When** the editor refreshes the affected document view
**Then** the selection remains at the resolved source range
**And** the editor publishes a visible status message

**Evidence:** `crates/waml-editor/src/app/tests/navigation.rs::changed_source_range_navigation_preserves_selection_and_publishes_status`

## Notes

- Analysis does not have one global valid or invalid result.
  `crates/waml/tests/incremental_analysis.rs::invalid_edited_island_keeps_unrelated_projection_current`
  proves that an invalid document projection can be stale while unrelated
  projections remain current.
- `crates/waml/tests/incremental_analysis.rs::valid_recovery_replaces_stale_projection_and_clears_diagnostic`
  proves that valid recovery replaces the stale projection and clears its
  diagnostic.
- `crates/waml/src/analysis.rs::shell_failed_document_is_quarantined_not_fatal`
  proves that one quarantined document does not make bundle analysis fatal.
- Command-line and language-server delivery are owned by their tooling goals.
  This goal stays partial until each applicable surface has complete evidence.
