# Changed files

- `docs/waml/goals/uml/class/feature-cut.md`
- `docs/waml/goals/uml/class/interact-with-a-class-diagram.md`
- `docs/waml/goals/uml/class/index.md`
- `docs/waml/goals/uml/shared/select-and-inspect.md`
- `docs/waml/goals/uml/shared/solve-the-layout.md`
- `docs/waml/goals/uml/shared/route-the-edges.md`
- `docs/waml/goals/uml/shared/place-the-labels.md`
- `docs/waml/goals/uml/shared/keep-the-map-stable.md`
- `docs/waml/goals/uml/shared/theme-the-diagram.md`
- `docs/waml/goals/uml/shared/index.md`
- `docs/superpowers/audits/reports/class-shared.md`

# Scenario identifiers

- `NATIVE-033`, `NATIVE-035`, `NATIVE-058` through `NATIVE-060`,
  `NATIVE-065` through `NATIVE-067`, `NATIVE-069`, and `NATIVE-072` —
  `interact-with-a-class-diagram.md`
- `NATIVE-034`, `NATIVE-052`, `NATIVE-068`, `NATIVE-070`, and `NATIVE-071` —
  `select-and-inspect.md`
- `NATIVE-036` — `solve-the-layout.md`
- `NATIVE-051` — `route-the-edges.md`

The integrated inventory records these owner moves:

- `NATIVE-033` moves from `goals/uml/class/index.md` to
  `goals/uml/class/interact-with-a-class-diagram.md`.
- `NATIVE-035`, `NATIVE-058`, `NATIVE-059`, and `NATIVE-072` move from
  `goals/uml/class/feature-cut.md` to
  `goals/uml/class/interact-with-a-class-diagram.md`.
- `NATIVE-060`, `NATIVE-065`, `NATIVE-066`, `NATIVE-067`, and `NATIVE-069`
  move from `goals/author-in-the-editor/draw-on-the-canvas.md` to
  `goals/uml/class/interact-with-a-class-diagram.md`.

All ten `goal_document` values now name these owners. The authoring tree does
not copy the five class-interaction scenarios.

# Evidence

- `NATIVE-033` — implementation: `crates/waml-editor/src/scene.rs:233` (`project_scene_node_with_display`); test: none.
- `NATIVE-034` — implementation: `crates/waml-editor/src/canvas/class/selection.rs:97` (`select`); test: `crates/waml-editor/src/canvas/class/selection.rs::selecting_starts_a_lift_without_delaying_logical_selection`.
- `NATIVE-035` — implementation: `crates/waml-editor/src/diagram_properties.rs:445` (`DiagramProperties`); partial tests: `crates/waml-editor/src/diagram_properties.rs::changing_one_property_emits_the_complete_display` and `crates/waml-editor/src/diagram_properties.rs::changing_cardinality_emits_the_selected_enum`.
- `NATIVE-036` — implementation: `crates/waml-editor/src/scene.rs:403` (`conflict_statement`) and `crates/waml-editor/src/scene.rs:418` (`conflict_participants`); test: none.
- `NATIVE-051` — implementation: `crates/waml-editor/src/scene.rs:565` (`route_with_groups`); test: none.
- `NATIVE-052` — implementation: `crates/waml-editor/src/inspector_panel.rs:308` (`Inspector`); test: none.
- `NATIVE-058` — implementation: `crates/waml-editor/src/class_diagram_view.rs:246` (`apply_tool_action`); test: `crates/waml-editor/src/class_diagram_view.rs::diagram_properties_tool_toggles_the_view_instead_of_being_a_no_op`.
- `NATIVE-059` — implementation: `crates/waml-editor/src/class_diagram_view.rs:258` (`properties_actions_outcome`); test: `crates/waml-editor/src/class_diagram_view.rs::a_properties_change_returns_exactly_one_diagram_set`.
- `NATIVE-060` — implementation: `crates/waml-editor/src/canvas/class/placement.rs:202` (`begin_drag`), `crates/waml-editor/src/canvas/class/placement.rs:212` (`drag_to`), `crates/waml-editor/src/canvas/class/placement.rs:459` (`finish_pointer_up`), and `crates/waml-editor/src/canvas/class/placement.rs:479` (`cancel`); test: `crates/waml-editor/src/canvas/class/placement.rs::abrupt_cancel_restores_nodes_complete_edges_and_camera`.
- `NATIVE-065` — implementation: `crates/waml-editor/src/tool_dock.rs:283` (`ToolDock::set_active`); partial test: `crates/waml-editor/src/tool_dock.rs::default_active_tool_is_select`.
- `NATIVE-066` — implementation: `crates/waml-editor/src/tool_dock.rs:283` (`ToolDock::set_active`); test: none.
- `NATIVE-067` — implementation: `crates/waml-editor/src/tool_dock.rs:283` (`ToolDock::set_active`); test: none.
- `NATIVE-068` — implementation: `crates/waml-editor/src/canvas/class/selection.rs:114` (`clear`); test: none.
- `NATIVE-069` — implementation: `crates/waml-editor/src/canvas/class/placement.rs:459` (`finish_pointer_up`); test: none.
- `NATIVE-070` — implementation: `crates/waml-editor/src/canvas/class/interaction.rs:169` (`pointer_up`); test: none.
- `NATIVE-071` — implementation: `crates/waml-editor/src/canvas/class/interaction.rs:169` (`pointer_up`); test: none.
- `NATIVE-072` — implementation: `crates/waml-editor/src/canvas/class/interaction.rs:169` (`pointer_up`) and `crates/waml-editor/src/class_diagram_view.rs:488` (`handle`); test: none.

# Verification gaps

- `NATIVE-033` — target: native; No targeted native presentation assertion exists.
- `NATIVE-035` — target: native; The property-state tests assert emitted values, but no native test observes the visible cardinality and attribute-limit controls.
- `NATIVE-036` — target: native; Solver golden tests do not assert native conflict presentation.
- `NATIVE-051` — target: native; No native class-diagram test checks relation routing at the target boundary.
- `NATIVE-052` — target: native; No native test asserts the visible inspector content after selection.
- `NATIVE-065` — target: native; The test asserts only the default Select enum value, not that a user can select the active class-diagram tool.
- `NATIVE-066` — target: native; No native test checks visible Add-tool selection.
- `NATIVE-067` — target: native; No native test checks visible Connect-tool selection.
- `NATIVE-068` — target: native; No native test checks the Clear Selection tool result.
- `NATIVE-069` — target: native; No native test asserts the cleared placement fields and stopped dwell and preview commands after pointer-up.
- `NATIVE-070` — target: native; No native test checks pointer-up node selection.
- `NATIVE-071` — target: native; No native test checks pointer-up deselection.
- `NATIVE-072` — target: native; No native test checks pointer-up expansion toggling.

# Open discrepancies

None.

# Feature gaps

- `FG-004` — `select-and-inspect.md` uses prose for selected-state assertions.
- `FG-007` — `interact-with-a-class-diagram.md` uses prose for semantic canvas targets and drag paths.
- `FG-008` — `interact-with-a-class-diagram.md` uses prose for hit targets and selected z-order results.
