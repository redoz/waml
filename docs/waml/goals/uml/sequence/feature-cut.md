# Sequence Feature Cut

**Goal:** A sequence diagram in WAML expresses everything an architecture
document needs to say about an interaction over time.

**Done when:** Every row below is `done` or `horizon`, and no `planned` row is
`MVP: yes`.

**Status:** partial — unverified
**MVP:** yes

Every status in this table is a first-pass guess. `Evidence` reads
`unverified` until an audit replaces it with a `file:line` or a test name.

## Participants

| Feature | Status | MVP | Evidence |
| --- | --- | --- | --- |
| Lifeline | done | yes | unverified |
| Lifeline typed by a classifier | partial | yes | unverified |
| Actor as a participant | partial | no | unverified |
| Lifeline ordering under author control | partial | no | unverified |
| Activation bar | partial | yes | unverified |
| Nested activation | planned | no | unverified |
| Create message and delayed lifeline start | planned | no | unverified |
| Destroy message and lifeline end | planned | no | unverified |

## Messages

| Feature | Status | MVP | Evidence |
| --- | --- | --- | --- |
| Synchronous message | done | yes | unverified |
| Asynchronous message | partial | yes | unverified |
| Reply message | partial | yes | unverified |
| Self message | partial | yes | unverified |
| Message arguments | partial | no | unverified |
| Message return value | partial | no | unverified |
| Lost and found message | horizon | no | unverified |

## Structure

| Feature | Status | MVP | Evidence |
| --- | --- | --- | --- |
| `alt` fragment, with an else branch | planned | yes | unverified |
| `opt` fragment | planned | yes | unverified |
| `loop` fragment, with a bound | planned | yes | unverified |
| `par` fragment | planned | no | unverified |
| `critical` fragment | horizon | no | unverified |
| `ref` to another interaction | planned | no | unverified |
| Guard on a fragment operand | planned | yes | unverified |
| Nested fragments | planned | no | unverified |
| Gate at the diagram boundary | horizon | no | unverified |
| Time and duration constraint | horizon | no | unverified |
| Note anchored to a message | partial | no | unverified |

## Notes

- Combined fragments are the largest single gap in this cut, and they are the
  reason most real sequence diagrams cannot yet be drawn. `alt`, `opt`, and
  `loop` are the three that matter for the dogfood bar; the rest can wait.
- Fragments are as much a layout problem as a model problem — a fragment is a
  box that must enclose a vertical span across a horizontal set of lifelines.
  Expect the work to land in [Shared](../shared/) as much as here.
- The interaction substrate is separate from the flow substrate that Activity
  and State Machine share, so nothing here comes free from those kinds.
