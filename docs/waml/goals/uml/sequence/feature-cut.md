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
| `calls` — synchronous message | done | yes | unverified |
| `calls ... async` — asynchronous message | planned | yes | in the approved slice |
| `returns` — reply message | partial | yes | replaces the old `replies` spelling |
| `signals` — signal message | planned | yes | in the approved slice |
| `creates` — create message | planned | no | in the approved slice |
| `destroys` — destroy message | planned | no | in the approved slice |
| Self message | partial | yes | unverified |
| Message arguments | partial | no | unverified |
| Message return value | partial | no | unverified |
| Lost and found message | horizon | no | excluded from the approved slice |

## Structure

| Feature | Status | MVP | Evidence |
| --- | --- | --- | --- |
| `alt` fragment — one or more `when`, optional final `else` | planned | yes | in the approved slice |
| `opt` fragment — exactly one `when` | planned | yes | in the approved slice |
| `loop` fragment — exactly one `when` | planned | yes | in the approved slice |
| `break` fragment — exactly one `when` | planned | no | in the approved slice |
| `par` fragment — two or more `branch` | planned | no | in the approved slice |
| `critical` fragment — exactly one `branch` | planned | no | in the approved slice |
| `assert` fragment — exactly one `branch` | planned | no | in the approved slice |
| `neg` fragment — exactly one `branch` | planned | no | in the approved slice |
| Nested fragments | planned | no | in the approved slice |
| Guard on a fragment operand | planned | yes | in the approved slice |
| `ref` — interaction use of another interaction | planned | no | in the approved slice |
| Gate at the interaction boundary, with bindings | planned | no | `## Gates` section, in the approved slice |
| `outside` as a boundary endpoint | planned | no | in the approved slice |
| Note anchored to a message | partial | no | unverified |
| Time and duration constraint | horizon | no | excluded from the approved slice |
| Coregion, continuation, general ordering | horizon | no | excluded from the approved slice |
| `strict`, `seq`, `ignore`, `consider` fragments | horizon | no | excluded from the approved slice |
| Part decomposition, state invariant, execution specification | horizon | no | excluded from the approved slice |

## Notes

- Almost every `planned` row above is already specified. An approved plan,
  `docs/superpowers/plans/2026-08-02-waml-sequence-language-completeness.md`,
  covers the whole sequence language as one vertical slice from lossless syntax
  through solver output. Rows marked `in the approved slice` are that plan's
  scope; rows marked `excluded` are constructs it deliberately refuses. This
  cut should not diverge from that plan — change the plan first.
- That plan also fixes the message vocabulary to exactly `calls`, `returns`,
  `signals`, `creates`, and `destroys`, with `async` valid only after the
  target of `calls`. `replies`, `sends`, and the colon call form are rejected
  with `UnsupportedSequenceForm`.
- Combined fragments are the largest single gap, and the reason most real
  sequence diagrams cannot yet be drawn. `alt`, `opt`, and `loop` are the three
  that matter for the dogfood bar.
- Fragments are as much a layout problem as a model problem — a fragment is a
  box that must enclose a vertical span across a horizontal set of lifelines.
  Expect work to land in [Shared](../shared/) as much as here.
- The interaction substrate is separate from the flow substrate that Activity
  and State Machine share, so nothing here comes free from those kinds.
- The plan explicitly does not change the visual sequence editor. Authoring
  these constructs on the canvas is a later goal, not part of that slice.
