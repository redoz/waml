# Interact with a Sequence Diagram

**Goal:** A reader sees a sequence interaction and uses its visible lifelines,
messages, activations, and fragments.

**Why:** Sequence-diagram interaction must give a visible result without
exposing widget identifiers or fixed pointer coordinates.

**Done when:** The editor renders a complete sequence scene, selects the
intended target, moves the camera, retains the camera and a surviving selection
during refresh, and shows a diagnostic empty state when the document is
unavailable.

**Status:** done
**MVP:** yes

## Shipped behavior

#### NATIVE-040 — the sequence diagram shows its interaction scene

**Applies to:** native

**Given** a sequence document contains lifelines, messages, activations, and fragments
**When** the reader opens its sequence diagram
**Then** the canvas shows the complete interaction scene

**Evidence:** `crates/waml-editor/src/behavior_doc_view.rs:289`

#### NATIVE-041 — a message wins a hit over its fragment

**Applies to:** native

**Given** a sequence message is inside a fragment
**When** the reader activates the message
**Then** the message becomes the interaction target before the enclosing fragment

**Evidence:** `crates/waml-editor/src/canvas/behavior/hit.rs::message_beats_enclosing_fragment`

#### NATIVE-042 — an activation hit selects its lifeline

**Applies to:** native

**Given** a sequence lifeline has an activation bar
**When** the reader activates the bar
**Then** the lifeline becomes the interaction target

**Evidence:** `crates/waml-editor/src/canvas/behavior/hit.rs::activation_bar_resolves_to_its_lifeline`

#### NATIVE-043 — a fragment border is selectable but its empty interior is not

**Applies to:** native

**Given** a sequence fragment has a visible border and empty interior space
**When** the reader activates the border and then the empty interior
**Then** the border targets the fragment and the empty interior returns no target

**Evidence:** `crates/waml-editor/src/canvas/behavior/hit.rs::fragment_border_hits_fragment_but_interior_empty_space_does_not`

#### NATIVE-044 — pan and zoom move the sequence camera

**Applies to:** native

**Given** a sequence scene is visible on the canvas
**When** the reader pans and zooms the scene
**Then** the camera moves and the canvas announces each distinct zoom percentage

**Evidence:** `crates/waml-editor/src/canvas/behavior/mod.rs:658`

#### NATIVE-054 — an unavailable behavior document shows diagnostic feedback

**Applies to:** native

**Given** the selected behavior document is unavailable
**And** the model reports a diagnostic for that document
**When** the reader opens the behavior view
**Then** the view shows a diagnostic empty-state message

**Evidence:** `crates/waml-editor/src/behavior_doc_view.rs:98` `crates/waml-editor/src/behavior_doc_view.rs:110`

## Verification gaps

- NATIVE-040 — target: native; No native test asserts the complete rendered sequence scene.
- NATIVE-044 — target: native; The tests assert zoom feedback and camera preservation during a scene update, but no test exercises pan input or observes the resulting camera movement.
- NATIVE-054 — target: native; No native test asserts behavior empty-state and diagnostic presentation.

## Notes

- [Sequence Language](./language.md) owns authored sequence syntax and model
  results. This document owns only user-visible interaction.
- The shared behavior canvas retains the camera and a surviving selection
  during a scene refresh. The implementation is covered by
  `crates/waml-editor/src/canvas/behavior/mod.rs::affected_scene_update_preserves_camera_and_surviving_selection`.
  This leaf does not copy the activity-owned `NATIVE-063` contract.
- [FG-003](../../../waml-feature-gaps.md#fg-003--view-anchors-and-eventual-draw-results)
  records the WAML expression gap for observable draw-cycle and camera results.
- [FG-008](../../../waml-feature-gaps.md#fg-008--hit-targets-tolerance-and-z-order)
  records the WAML expression gap for hit targets, tolerance, and priority.
