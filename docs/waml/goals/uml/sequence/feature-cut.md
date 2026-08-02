# Sequence Feature Cut

**Goal:** A sequence diagram in WAML expresses everything an architecture
document needs to say about an interaction over time.

**Done when:** Every row below is `done` or `horizon`, and no `planned` row is
`MVP: yes`.

**Status:** partial — unverified
**MVP:** yes

Every status in this table is a first reading of the code. `Evidence` shows
`unverified` until an audit replaces it with a `file:line` reference or the
name of a test.

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
| `calls ... async` — asynchronous message | planned | yes | [language](./language.md) |
| `returns` — reply message | partial | yes | [language](./language.md) |
| `signals` — signal message | planned | yes | [language](./language.md) |
| `creates` — create message | planned | no | [language](./language.md) |
| `destroys` — destroy message | planned | no | [language](./language.md) |
| Self message | partial | yes | unverified |
| Message arguments | partial | no | unverified |
| Message return value | partial | no | unverified |
| Lost and found message | horizon | no | [excluded](./language.md) |

## Structure

| Feature | Status | MVP | Evidence |
| --- | --- | --- | --- |
| `alt` fragment — one or more `when`, optional final `else` | planned | yes | [language](./language.md) |
| `opt` fragment — exactly one `when` | planned | yes | [language](./language.md) |
| `loop` fragment — exactly one `when` | planned | yes | [language](./language.md) |
| `break` fragment — exactly one `when` | planned | no | [language](./language.md) |
| `par` fragment — two or more `branch` | planned | no | [language](./language.md) |
| `critical` fragment — exactly one `branch` | planned | no | [language](./language.md) |
| `assert` fragment — exactly one `branch` | planned | no | [language](./language.md) |
| `neg` fragment — exactly one `branch` | planned | no | [language](./language.md) |
| Nested fragments | planned | no | [language](./language.md) |
| Guard on a fragment operand | planned | yes | [language](./language.md) |
| `ref` — interaction use of another interaction | planned | no | [language](./language.md) |
| Gate at the interaction boundary, with bindings | planned | no | [language](./language.md) |
| `outside` as a boundary endpoint | planned | no | [language](./language.md) |
| Note anchored to a message | partial | no | unverified |
| Time and duration constraint | horizon | no | [excluded](./language.md) |
| Coregion, continuation, general ordering | horizon | no | [excluded](./language.md) |
| `strict`, `seq`, `ignore`, `consider` fragments | horizon | no | [excluded](./language.md) |
| Part decomposition, state invariant, execution specification | horizon | no | [excluded](./language.md) |

## Notes

- The [Sequence Language](./language.md) states the meaning of each row above
  in authored source. A row here shows whether the product obeys the language.
  The language document shows what the product must do. Rows with the word
  `excluded` are constructs that the language refuses. To add one, change that
  document first.
- Fragments are the largest defect. They are the reason that the product cannot
  draw most real sequence diagrams. `alt`, `opt`, and `loop` are the three that
  the bar needs.
- A fragment is a layout problem and a model problem. A fragment is a box that
  must hold a vertical range across a horizontal set of lifelines. Expect work
  in [Shared](../shared/) and here.
- The interaction substrate is separate from the flow substrate that Activity
  and State Machine use. Work on those kinds gives no help here.
- To read is not to author. A construct can have the status `done` here, thus
  the parser, the model, and the renderer accept it, while an author cannot
  make it on the canvas. [Draw on the
  Canvas](../../author-in-the-editor/draw-on-the-canvas.md) controls canvas
  authoring.
