# Interact with an Activity Diagram

**Goal:** A reader sees an activity flow and interacts with its visible nodes
and edges.

**Why:** Activity-diagram interaction must give a visible result without
exposing widget identifiers or fixed pointer coordinates.

**Done when:** The editor renders an activity scene, selects the intended hit
target, preserves the camera and a surviving selection during refresh, and
returns no target for an empty scene.

**Status:** done
**MVP:** yes

## Shipped behavior

#### NATIVE-037 — the activity diagram shows its flow scene

**Applies to:** native

**Given** an activity document contains actions, control nodes, and flows
**When** the reader opens its activity diagram
**Then** the canvas shows the activity nodes and connecting edges

**Evidence:** `crates/waml-editor/src/behavior_doc_view.rs:158`

#### NATIVE-038 — a node wins a hit over an edge below it

**Applies to:** native

**Given** an activity node covers part of a crossing edge
**When** the reader activates the covered area
**Then** the node becomes the interaction target before the edge

**Evidence:** `crates/waml-editor/src/canvas/behavior/hit.rs::flow_hit_prefers_node_over_edge_under_it`

#### NATIVE-039 — an edge accepts a hit inside its tolerance band

**Applies to:** native

**Given** an activity edge is visible on the canvas
**When** the reader activates a point inside the edge tolerance band
**Then** the edge becomes the interaction target

**Evidence:** `crates/waml-editor/src/canvas/behavior/hit.rs::flow_edge_hits_within_tolerance_band`

#### NATIVE-053 — activity interaction shows the selected target

**Applies to:** native

**Given** an activity diagram contains a selectable node or edge
**When** the reader selects that target on the canvas
**Then** the canvas presents that node or edge as selected

**Evidence:** `crates/waml-editor/src/canvas/behavior/mod.rs:346`

#### NATIVE-063 — refresh keeps the camera and a surviving selection

**Applies to:** native

**Given** an activity diagram has a camera position and a selected target
**And** the target remains in the updated scene
**When** a document change refreshes the activity scene
**Then** the camera position and selected target remain

**Evidence:** `crates/waml-editor/src/canvas/behavior/mod.rs::affected_scene_update_preserves_camera_and_surviving_selection`

#### NATIVE-064 — an empty activity scene has no hit target

**Applies to:** native

**Given** an activity scene contains no nodes or edges
**When** the reader activates the empty canvas
**Then** the canvas returns no interaction target

**Evidence:** `crates/waml-editor/src/canvas/behavior/hit.rs::empty_scene_hits_nothing`

## Verification gaps

- NATIVE-037 — target: native; No native test asserts the rendered activity scene.
- NATIVE-053 — target: native; No native test asserts selected activity target presentation.

## Notes

- [FG-003](../../../waml-feature-gaps.md#fg-003--view-anchors-and-eventual-draw-results)
  records the WAML expression gap for observable refresh results after a draw
  cycle.
- [FG-008](../../../waml-feature-gaps.md#fg-008--hit-targets-tolerance-and-z-order)
  records the WAML expression gap for hit targets, tolerance, and priority.
