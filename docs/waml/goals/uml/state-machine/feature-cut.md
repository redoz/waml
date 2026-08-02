# State Machine Feature Cut

**Goal:** A state machine diagram in WAML expresses everything an architecture
document needs to say about an object's lifecycle.

**Done when:** Every row below is `done` or `horizon`, and no `planned` row is
`MVP: yes`.

**Status:** partial — unverified
**MVP:** no

Every status in this table is a first reading of the code. `Evidence` shows
`unverified` until an audit replaces it with a `file:line` reference or the
name of a test.

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

- This kind operates more than the row statuses show. The parser accepts states
  and the form `on TRIGGER when GUARD transitions to TARGET: EFFECT`. The flow
  solver solves it. The behavior view draws it. Golden tests and property tests
  cover it. The shared flow substrate gives most of this behavior.
- The full kind has the flag `MVP: no`. The bar does not need a lifecycle
  diagram. Do not build work here before the class defects and the sequence
  defects are complete.
- Composite states and submachine states are the limit of this kind. They need
  a nested flow layout. The current solver does not make one.
