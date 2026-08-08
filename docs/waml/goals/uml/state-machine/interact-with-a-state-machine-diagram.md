# Interact with a State Machine Diagram

**Goal:** A reader sees a state-machine flow and interacts with its visible
states and transitions.

**Why:** State-machine interaction must give observable rendering, hit,
selection, camera, and refresh results.

**Done when:** The frozen behavior inventory owns stable state-machine
interaction scenarios for the shipped native results.

**Status:** partial
**MVP:** no

## Current product state

- The native behavior view builds and draws a flow scene for a state-machine
  document.
- The shared behavior canvas can return node and edge targets, present a
  selected target, and retain the camera and a surviving selection during a
  scene refresh.
- The frozen inventory has no state-machine row. This document therefore does
  not invent a stable scenario identifier or a GWT contract.

## Evidence

- State-machine flow scenes use
  `crates/waml-editor/src/behavior_doc_view.rs::build_flow_scene`.
- Flow hit testing uses
  `crates/waml-editor/src/canvas/behavior/hit.rs::hit_test`.
- Scene refresh retention is covered by
  `crates/waml-editor/src/canvas/behavior/mod.rs::affected_scene_update_preserves_camera_and_surviving_selection`.
- State-machine scene geometry is covered below the native target boundary by
  `crates/waml/tests/flow_solver_golden.rs::state_machine_fixture_layout_golden`.

## Notes

- Task 12 must reconcile the missing frozen ownership before this leaf can own
  a stable shipped scenario.
- [FG-003](../../../waml-feature-gaps.md#fg-003--view-anchors-and-eventual-draw-results)
  records the WAML expression gap for observable refresh results after a draw
  cycle.
- [FG-008](../../../waml-feature-gaps.md#fg-008--hit-targets-tolerance-and-z-order)
  records the WAML expression gap for hit targets, tolerance, and priority.
