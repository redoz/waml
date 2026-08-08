# Select and Inspect

**Goal:** A selection in a diagram identifies its subject and makes the subject
available to the inspector.

**Why:** A reader must know which diagram subject is active before an author
changes it.

**Done when:** Every diagram kind can select and clear each drawn subject, and
the inspector shows the full property set of the selected subject.

**Status:** partial
**MVP:** yes

## Shipped behavior

#### NATIVE-034 — selecting a class target retains its selection state

**Applies to:** native

**Given** a class-diagram target is not selected
**When** the author selects that target
**Then** the target becomes selected immediately and starts its visual lift

**Evidence:** `crates/waml-editor/src/canvas/class/selection.rs::selecting_starts_a_lift_without_delaying_logical_selection`

#### NATIVE-052 — the inspector shows the selected class subject

**Applies to:** native

**Given** a class-diagram subject is selected
**When** the author views the inspector
**Then** the inspector identifies the selected subject and shows its property controls

**Evidence:** `crates/waml-editor/src/inspector_panel.rs::Inspector`

#### NATIVE-068 — clear selection removes the selected class subject

**Applies to:** native

**Given** a class-diagram subject is selected
**When** the author activates Clear Selection
**Then** no class-diagram subject remains selected

**Evidence:** `crates/waml-editor/src/canvas/class/selection.rs::clear`

#### NATIVE-070 — pointer-up selects a classifier

**Applies to:** native

**Given** a classifier is not selected
**When** the author releases the primary pointer on that classifier
**Then** the classifier becomes selected

**Evidence:** `crates/waml-editor/src/canvas/class/interaction.rs::pointer_up`

#### NATIVE-071 — pointer-up on empty canvas clears class selection

**Applies to:** native

**Given** a classifier is selected
**When** the author releases the primary pointer on empty canvas
**Then** the class-diagram selection is empty

**Evidence:** `crates/waml-editor/src/canvas/class/interaction.rs::pointer_up`

## Verification gaps

- NATIVE-052 — target: native; No native test asserts the visible inspector content after selection.
- NATIVE-068 — target: native; No native test checks the Clear Selection tool result.
- NATIVE-070 — target: native; No native test checks pointer-up node selection.
- NATIVE-071 — target: native; No native test checks pointer-up deselection.

## Notes

- Class-specific rendering, tools, properties, placement, and expansion are
  owned by [Interact with a Class Diagram](../class/interact-with-a-class-diagram.md).
- The frozen inventory has no shipped class scenario for member, edge, end,
  label, or note selection. These missing contracts keep this goal `partial`.
- Selected-state assertions use prose.
  [FG-004](../../../waml-feature-gaps.md#fg-004--ordered-collections-and-states)
  records this WAML expression gap.
