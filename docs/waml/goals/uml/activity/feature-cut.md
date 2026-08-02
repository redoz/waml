# Activity Feature Cut

**Goal:** An activity diagram in WAML expresses everything an architecture
document needs to say about a procedure.

**Done when:** Every row below is `done` or `horizon`, and no `planned` row is
`MVP: yes`.

**Status:** partial — unverified
**MVP:** yes

Every status in this table is a first-pass guess. `Evidence` reads
`unverified` until an audit replaces it with a `file:line` or a test name.

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

- Activity shares the flow substrate and the flow solver with [State
  Machine](../state-machine/feature-cut.md). Anything landed in the substrate
  benefits both, and a row here that is `done` is often `done` there too.
- Swimlanes are the one structural gap that a real process document reaches for
  first. They are `MVP: no` only because `docs/waml` has no multi-role process
  documented yet.
- The exotic regions — expansion, interruptible, exception — are marked
  `horizon` on purpose. Listing them is what makes this a complete cut; nobody
  should build them for the bar.
