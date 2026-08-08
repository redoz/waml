# Changed files

- `docs/waml/goals/uml/activity/feature-cut.md`
- `docs/waml/goals/uml/activity/interact-with-an-activity-diagram.md`
- `docs/waml/goals/uml/activity/index.md`
- `docs/waml/goals/uml/sequence/feature-cut.md`
- `docs/waml/goals/uml/sequence/language.md`
- `docs/waml/goals/uml/sequence/interact-with-a-sequence-diagram.md`
- `docs/waml/goals/uml/sequence/index.md`
- `docs/waml/goals/uml/state-machine/feature-cut.md`
- `docs/waml/goals/uml/state-machine/interact-with-a-state-machine-diagram.md`
- `docs/waml/goals/uml/state-machine/index.md`
- `docs/waml/goals/uml/use-case/feature-cut.md`
- `docs/waml/goals/uml/use-case/interact-with-a-use-case-diagram.md`
- `docs/waml/goals/uml/use-case/index.md`
- `docs/superpowers/audits/reports/behavior-diagrams.md`

# Scenario identifiers

- `NATIVE-037` through `NATIVE-039`, `NATIVE-053`, `NATIVE-063`, and
  `NATIVE-064` — `interact-with-an-activity-diagram.md`
- `NATIVE-040` through `NATIVE-044` and `NATIVE-054` —
  `interact-with-a-sequence-diagram.md`
- Existing `SEQ-MSG-1` through `SEQ-MSG-8`, `SEQ-ORD-1` and `SEQ-ORD-2`, and
  `SEQ-FRAG-1` through `SEQ-FRAG-11` remain byte-for-byte stable.
- Existing sequence boundary, activation, excluded-construct, and recovery IDs
  also remain stable. They now use the same applicability, evidence, and
  verification-gap contract.
- The frozen inventory has no state-machine or use-case row. Their interaction
  leaves contain no invented scenario identifier and no GWT contract.

The ownership plan moves these frozen rows without changing the inventory:

- `NATIVE-037` and `NATIVE-064` move from `goals/uml/activity/index.md` to
  `goals/uml/activity/interact-with-an-activity-diagram.md`.
- `NATIVE-038`, `NATIVE-039`, `NATIVE-053`, and `NATIVE-063` move from
  `goals/uml/activity/feature-cut.md` to
  `goals/uml/activity/interact-with-an-activity-diagram.md`.
- `NATIVE-040` moves from `goals/uml/sequence/index.md` to
  `goals/uml/sequence/interact-with-a-sequence-diagram.md`.
- `NATIVE-041` through `NATIVE-044` and `NATIVE-054` move from
  `goals/uml/sequence/feature-cut.md` to
  `goals/uml/sequence/interact-with-a-sequence-diagram.md`.

Task 12 must reconcile these 12 `goal_document` values. It must also reconcile
the absence of frozen state-machine and use-case interaction rows before those
leaves can own stable shipped scenarios.

# Evidence

- `NATIVE-037` — implementation: `crates/waml-editor/src/behavior_doc_view.rs:158` (`build_flow_scene`); test: none.
- `NATIVE-038` — implementation: `crates/waml-editor/src/canvas/behavior/hit.rs:59` (`hit_test`); test: `crates/waml-editor/src/canvas/behavior/hit.rs::flow_hit_prefers_node_over_edge_under_it`.
- `NATIVE-039` — implementation: `crates/waml-editor/src/canvas/behavior/hit.rs:59` (`hit_test`); test: `crates/waml-editor/src/canvas/behavior/hit.rs::flow_edge_hits_within_tolerance_band`.
- `NATIVE-053` — implementation: `crates/waml-editor/src/canvas/behavior/mod.rs:346` (`hover_target_at`); test: none.
- `NATIVE-063` — implementation: `crates/waml-editor/src/canvas/behavior/mod.rs:570` (`update_scene`); test: `crates/waml-editor/src/canvas/behavior/mod.rs::affected_scene_update_preserves_camera_and_surviving_selection`.
- `NATIVE-064` — implementation: `crates/waml-editor/src/canvas/behavior/hit.rs:59` (`hit_test`); test: `crates/waml-editor/src/canvas/behavior/hit.rs::empty_scene_hits_nothing`.
- `NATIVE-040` — implementation: `crates/waml-editor/src/behavior_doc_view.rs:289` (`build_interaction_scene`); test: none.
- `NATIVE-041` — implementation: `crates/waml-editor/src/canvas/behavior/hit.rs:59` (`hit_test`); test: `crates/waml-editor/src/canvas/behavior/hit.rs::message_beats_enclosing_fragment`.
- `NATIVE-042` — implementation: `crates/waml-editor/src/canvas/behavior/hit.rs:59` (`hit_test`); test: `crates/waml-editor/src/canvas/behavior/hit.rs::activation_bar_resolves_to_its_lifeline`.
- `NATIVE-043` — implementation: `crates/waml-editor/src/canvas/behavior/hit.rs:59` (`hit_test`); test: `crates/waml-editor/src/canvas/behavior/hit.rs::fragment_border_hits_fragment_but_interior_empty_space_does_not`.
- `NATIVE-044` — implementation: `crates/waml-editor/src/canvas/behavior/mod.rs:658` (`zoom_step`); partial tests: `crates/waml-editor/src/canvas/behavior/mod.rs::affected_scene_update_preserves_camera_and_surviving_selection` and `crates/waml-editor/src/canvas/behavior/mod.rs::a_zoom_announces_once_per_distinct_percentage`.
- `NATIVE-054` — implementation: `crates/waml-editor/src/behavior_doc_view.rs:98` (`empty_message`) and `crates/waml-editor/src/behavior_doc_view.rs:110` (`diagnostics_status`); test: none.

# Verification gaps

- `NATIVE-037` — target: native; No native test asserts the rendered activity scene.
- `NATIVE-053` — target: native; No native test asserts selected activity target presentation.
- `NATIVE-040` — target: native; No native test asserts the complete rendered sequence scene.
- `NATIVE-044` — target: native; The tests assert zoom feedback and camera preservation during a scene update, but no test exercises pan input or observes the resulting camera movement.
- `NATIVE-054` — target: native; No native test asserts behavior empty-state and diagnostic presentation.
- The stable sequence-language scenarios use exact source evidence and native
  verification-gap records. Task 15 owns their test-marker insertion.

# Open discrepancies

None in the frozen activity and sequence rows. The missing state-machine and
use-case rows are an inventory-coverage concern for Task 12, not an invented
discrepancy record.

# Feature gaps

- `FG-003` — activity, sequence, and state-machine interaction leaves use prose
  for observable results after a draw cycle.
- `FG-008` — activity, sequence, and state-machine interaction leaves use prose
  for hit targets, tolerance, and priority.
- No feature-gap entry or geometry constraint was added for specialized actor,
  use-case, system-boundary, or use-case-layout rendering.
