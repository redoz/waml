# State Machine Feature Cut

**Goal:** A state machine diagram in WAML expresses the language and model that
an architecture document needs to describe an object's lifecycle.

**Done when:** Every language and model row below is `done` or `horizon`.

**Status:** partial
**MVP:** no

This document records language and model coverage. [Interact with a State
Machine Diagram](./interact-with-a-state-machine-diagram.md) owns
state-machine UI behavior.

## States

| Feature | Status | MVP |
| --- | --- | --- |
| Simple state | done | no |
| Initial pseudostate | done | no |
| Final state | done | no |
| Entry behavior | done | no |
| Exit behavior | done | no |
| Do behavior | done | no |
| Choice pseudostate | partial | no |
| Junction pseudostate | partial | no |
| Composite state | horizon | no |
| Submachine state | horizon | no |
| History pseudostate | horizon | no |

## Transitions

| Feature | Status | MVP |
| --- | --- | --- |
| Transition on a trigger | done | no |
| Guard on a transition | done | no |
| Effect on a transition | done | no |
| Else transition | done | no |
| Self transition | done | no |
| Internal transition | planned | no |
| Completion transition | done | no |

## Structure

| Feature | Status | MVP |
| --- | --- | --- |
| `describes` link to the classifier whose lifecycle this is | done | no |
| Note anchored to a state | partial | no |
| Author-controlled state ordering | partial | no |

## Evidence

- State-machine document claims and current flow forms are covered by
  `crates/waml/tests/uml_attribute_syntax.rs::catalog_claims_each_supported_uml_type_once_and_leaves_generic_types_unclaimed`
  and
  `crates/waml/tests/uml_behavior_syntax.rs::flow_fixed_slots_project_every_current_node_and_transition_form_losslessly`.
- Current state and pseudostate heading kinds are covered by
  `crates/waml/tests/uml_behavior_syntax.rs::every_flow_heading_kind_and_claimed_link_state_is_declared_without_byte_loss`.
- State-machine layout is covered by
  `crates/waml/tests/flow_solver_golden.rs::state_machine_fixture_layout_golden`.
- State behavior-line sizing is covered by
  `crates/waml/tests/flow_solver_golden.rs::a_state_box_fits_its_entry_do_exit_lines`.
- Self transitions are covered by
  `crates/waml/tests/flow_solver_golden.rs::self_transition_routes_out_and_back`.

## Notes

- State Machine and [Activity](../activity/feature-cut.md) use the same flow
  substrate and flow solver.
- Composite states, submachine states, and history pseudostates need a nested
  state model that is outside the current cut.
- [Interact with a State Machine
  Diagram](./interact-with-a-state-machine-diagram.md) owns rendering, hit
  testing, selection, camera retention, and refresh results.
