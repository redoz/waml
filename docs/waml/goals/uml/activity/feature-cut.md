# Activity Feature Cut

**Goal:** An activity diagram in WAML expresses everything an architecture
document needs to say about a procedure.

**Done when:** Every row below is `done` or `horizon`, and no `planned` row is
`MVP: yes`.

**Status:** partial — unverified
**MVP:** yes

Every status in this table is a first reading of the code. `Evidence` shows
`unverified` until an audit replaces it with a `file:line` reference or the
name of a test.

## Nodes

| Feature | Status | MVP | Evidence |
| --- | --- | --- | --- |
| Action node | done | yes | unverified |
| Initial node | done | yes | unverified |
| Final node | done | yes | unverified |
| Flow final node | partial | no | unverified |
| Decision node | done | yes | unverified |
| Merge node | done | yes | unverified |
| Fork node | done | yes | unverified |
| Join node | done | yes | unverified |
| Object node | done | no | unverified |
| Call to another behavior | partial | no | unverified |
| Send signal and accept event | horizon | no | unverified |
| Pin on an action | horizon | no | unverified |

## Edges

| Feature | Status | MVP | Evidence |
| --- | --- | --- | --- |
| Control flow | done | yes | unverified |
| Guard on an outgoing edge | done | yes | unverified |
| Else branch | done | yes | unverified |
| Object flow | partial | no | unverified |
| Edge label | done | yes | unverified |
| Weight on an edge | horizon | no | unverified |

## Structure

| Feature | Status | MVP | Evidence |
| --- | --- | --- | --- |
| Partition or swimlane | planned | no | unverified |
| Nested partition | horizon | no | unverified |
| Interruptible region | horizon | no | unverified |
| Expansion region | horizon | no | unverified |
| Exception handler | horizon | no | unverified |
| Note anchored to a node | partial | no | unverified |
| `describes` link to the classifier that owns the behavior | done | yes | unverified |

## Notes

- Activity and [State Machine](../state-machine/feature-cut.md) use the same
  flow substrate and the same flow solver. Work in the substrate gives help to
  both kinds. A row with the status `done` here is frequently `done` there too.
- Partitions are the structural function that a real process document needs
  first. They have the flag `MVP: no` only because `docs/waml` has no process
  with more than one role.
- The regions for expansion, interruption, and exceptions have the status
  `horizon` on purpose. The list makes this cut complete. Do not build them for
  the bar.
