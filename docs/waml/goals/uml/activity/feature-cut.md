# Activity Feature Cut

**Goal:** An activity diagram in WAML expresses the language and model that an
architecture document needs to describe a procedure.

**Done when:** Every MVP language and model row below is `done`, and every
other row is `done` or `horizon`.

**Status:** partial
**MVP:** yes

This document records language and model coverage. [Interact with an Activity
Diagram](./interact-with-an-activity-diagram.md) owns activity-diagram UI
behavior.

## Nodes

| Feature | Status | MVP |
| --- | --- | --- |
| Action node | done | yes |
| Initial node | done | yes |
| Final node | done | yes |
| Flow final node | planned | no |
| Decision node | done | yes |
| Merge node | done | yes |
| Fork node | done | yes |
| Join node | done | yes |
| Object node | done | no |
| Call to another behavior | done | no |
| Send signal and accept event | horizon | no |
| Pin on an action | horizon | no |

## Edges

| Feature | Status | MVP |
| --- | --- | --- |
| Control flow | done | yes |
| Guard on an outgoing edge | done | yes |
| Else branch | done | yes |
| Object flow | done | no |
| Edge label | done | yes |
| Weight on an edge | horizon | no |

## Structure

| Feature | Status | MVP |
| --- | --- | --- |
| Partition or swimlane | done | no |
| Nested partition | horizon | no |
| Interruptible region | horizon | no |
| Expansion region | horizon | no |
| Exception handler | horizon | no |
| Note anchored to a node | partial | no |
| `describes` link to the classifier that owns the behavior | done | yes |

## Evidence

- Current node, transition, guard, effect, object-flow, partition, and
  cross-document forms are covered by
  `crates/waml/tests/uml_behavior_syntax.rs::flow_fixed_slots_project_every_current_node_and_transition_form_losslessly`.
- All current flow-node heading kinds are covered by
  `crates/waml/tests/uml_behavior_syntax.rs::every_flow_heading_kind_and_claimed_link_state_is_declared_without_byte_loss`.
- Activity layout is covered by
  `crates/waml/tests/flow_solver_golden.rs::activity_fixture_layout_golden`.
- Partition layout is covered by
  `crates/waml/tests/flow_solver_golden.rs::nodes_lie_inside_their_partition_band`.
- Calls to another behavior are covered by
  `crates/waml/tests/flow_solver_golden.rs::cross_document_edge_becomes_off_page_stub`.

## Notes

- Activity and [State Machine](../state-machine/feature-cut.md) use the same
  flow substrate and the same flow solver.
- Flow finals have no current model kind. Send and accept events, pins, nested
  partitions, interruptible regions, expansion regions, exception handlers,
  and edge weights are outside the current cut.
- [Interact with an Activity
  Diagram](./interact-with-an-activity-diagram.md) owns rendering, hit testing,
  selection, camera retention, and refresh results.
