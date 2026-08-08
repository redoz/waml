# Interact with a Class Diagram

**Goal:** A reader sees a class diagram, and an author uses its class-specific
controls.

**Why:** Class-diagram actions must give a visible result without exposing
widget identifiers or pointer coordinates.

**Done when:** The editor shows classifiers and members, controls the active
tool and diagram properties, cancels a drag without changing the diagram,
ends placement on pointer-up, and toggles classifier expansion.

**Status:** done
**MVP:** yes

## Shipped behavior

#### NATIVE-033 — the class diagram shows classifiers and displayed attributes

**Applies to:** native

**Given** a class document contains a classifier with displayed attributes
**When** the reader opens its class diagram
**Then** the canvas shows the classifier and its displayed attributes

**Evidence:** `crates/waml-editor/src/scene.rs::project_scene_node_with_display`

#### NATIVE-035 — diagram properties show cardinality and attribute limits

**Applies to:** native

**Given** a class diagram is active
**When** the author opens the diagram properties
**Then** the property controls show the cardinality and attribute-limit values

**Evidence:** `crates/waml-editor/src/diagram_properties.rs::DiagramProperties`

#### NATIVE-058 — the properties tool toggles the properties view

**Applies to:** native

**Given** the diagram properties view is closed
**When** the author selects the properties tool twice
**Then** the first selection opens the view and the second selection closes it

**Evidence:** `crates/waml-editor/src/class_diagram_view.rs::diagram_properties_tool_toggles_the_view_instead_of_being_a_no_op`

#### NATIVE-059 — a property change produces one updated diagram set

**Applies to:** native

**Given** the diagram properties show the current display values
**When** the author changes one property
**Then** the class diagram uses one complete updated set of display values

**Evidence:** `crates/waml-editor/src/class_diagram_view.rs::a_properties_change_returns_exactly_one_diagram_set`

#### NATIVE-060 — cancel restores a dragged classifier and the camera

**Applies to:** native

**Given** a class diagram has a classifier, complete edges, and a camera position
**When** the author drags the classifier and cancels the drag
**Then** the classifier, the complete edges, and the camera return to their prior state

**Evidence:** `crates/waml-editor/src/canvas/class/placement.rs::abrupt_cancel_restores_nodes_complete_edges_and_camera`

#### NATIVE-065 — the select tool becomes active

**Applies to:** native

**Given** another class-diagram tool is active
**When** the author selects the Select tool
**Then** the tool dock shows Select as the active tool

**Evidence:** `crates/waml-editor/src/tool_dock.rs:283`

#### NATIVE-066 — the add tool becomes active

**Applies to:** native

**Given** another class-diagram tool is active
**When** the author selects the Add tool
**Then** the tool dock shows Add as the active tool

**Evidence:** `crates/waml-editor/src/tool_dock.rs:283`

#### NATIVE-067 — the connect tool becomes active

**Applies to:** native

**Given** another class-diagram tool is active
**When** the author selects the Connect tool
**Then** the tool dock shows Connect as the active tool

**Evidence:** `crates/waml-editor/src/tool_dock.rs:283`

#### NATIVE-069 — pointer-up ends the active placement

**Applies to:** native

**Given** a classifier placement shows a preview and has active dwell behavior
**When** the author releases the primary pointer
**Then** the placement preview ends and its dwell behavior stops

**Evidence:** `crates/waml-editor/src/canvas/class/placement.rs::finish_pointer_up`

#### NATIVE-072 — the expansion handle toggles a classifier

**Applies to:** native

**Given** a classifier has an expansion handle
**When** the author activates that handle
**Then** the classifier changes between its expanded and collapsed presentation

**Evidence:** `crates/waml-editor/src/canvas/class/interaction.rs::pointer_up` `crates/waml-editor/src/class_diagram_view.rs::handle`

## Verification gaps

- NATIVE-033 — target: native; No targeted native presentation assertion exists.
- NATIVE-035 — target: native; The property-state tests assert emitted values, but no native test observes the visible cardinality and attribute-limit controls.
- NATIVE-065 — target: native; The test asserts only the default Select enum value, not that a user can select the active class-diagram tool.
- NATIVE-066 — target: native; No native test checks visible Add-tool selection.
- NATIVE-067 — target: native; No native test checks visible Connect-tool selection.
- NATIVE-069 — target: native; No native test asserts the cleared placement fields and stopped dwell and preview commands after pointer-up.
- NATIVE-072 — target: native; No native test checks pointer-up expansion toggling.

## Notes

- [Select and Inspect](../shared/select-and-inspect.md) owns shared selection and
  inspector results.
- [Solve the Layout](../shared/solve-the-layout.md), [Route the
  Edges](../shared/route-the-edges.md), [Place the
  Labels](../shared/place-the-labels.md), [Keep the Map
  Stable](../shared/keep-the-map-stable.md), and [Theme the
  Diagram](../shared/theme-the-diagram.md) own their shared outputs.
- Semantic canvas targets and drag paths use prose.
  [FG-007](../../../waml-feature-gaps.md#fg-007--semantic-canvas-targets-and-drag-paths)
  records this WAML expression gap.
- Hit targets and selected z-order results use prose.
  [FG-008](../../../waml-feature-gaps.md#fg-008--hit-targets-tolerance-and-z-order)
  records this WAML expression gap.
