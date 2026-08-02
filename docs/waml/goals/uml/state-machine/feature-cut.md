# State Machine Feature Cut

**Goal:** A state machine diagram in WAML expresses everything an architecture
document needs to say about an object's lifecycle.

**Done when:** Every row below is `done` or `horizon`, and no `planned` row is
`MVP: yes`.

**Status:** partial — unverified
**MVP:** no

Every status in this table is a first-pass guess. `Evidence` reads
`unverified` until an audit replaces it with a `file:line` or a test name.

## States

| Feature | Status | MVP | Evidence |
| --- | --- | --- | --- |
| Simple state | done | no | unverified |
| Initial pseudostate | done | no | unverified |
| Final state | done | no | unverified |
| Entry behavior | done | no | unverified |
| Exit behavior | partial | no | unverified |
| Do behavior | planned | no | unverified |
| Choice pseudostate | partial | no | unverified |
| Junction pseudostate | partial | no | unverified |
| Composite state | horizon | no | unverified |
| Submachine state | horizon | no | unverified |
| History pseudostate | horizon | no | unverified |

## Transitions

| Feature | Status | MVP | Evidence |
| --- | --- | --- | --- |
| Transition on a trigger | done | no | unverified |
| Guard on a transition | done | no | unverified |
| Effect on a transition | done | no | unverified |
| Else transition | done | no | unverified |
| Self transition | partial | no | unverified |
| Internal transition | planned | no | unverified |
| Completion transition | partial | no | unverified |

## Structure

| Feature | Status | MVP | Evidence |
| --- | --- | --- | --- |
| `describes` link to the classifier whose lifecycle this is | done | no | unverified |
| Note anchored to a state | partial | no | unverified |
| Author-controlled state ordering | partial | no | unverified |

## Notes

- This kind is further along than it looks. `uml.StateMachine` parses states
  and `on TRIGGER when GUARD transitions to TARGET: EFFECT`, solves through the
  flow solver under `FlowFlavor::StateMachine`, renders in the behavior view,
  and has golden and property coverage. Most of it came free from the shared
  flow substrate.
- The whole kind is `MVP: no`. The dogfood bar does not need a lifecycle
  diagram. Nothing here should be built before the class and sequence gaps
  close.
- Composite and submachine states are the real ceiling: they need a nested flow
  layout that the current solver does not attempt.
