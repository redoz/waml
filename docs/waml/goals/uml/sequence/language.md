# Sequence Language

The authored form of a sequence interaction. This document states the intended
language, not the current implementation. Where the parser disagrees with this
document, one of the two is a defect — and this document is what a test is
written against.

## Messages

A message is one list item under a lifeline's section. Exactly five verbs are
accepted:

| Verb | Means | Drawn as |
| --- | --- | --- |
| `calls` | A synchronous call. The sender waits. | Solid line, filled arrowhead |
| `returns` | A reply to an earlier `calls`. | Dashed line, open arrowhead |
| `signals` | An asynchronous signal with no reply. | Solid line, open arrowhead |
| `creates` | Brings the target lifeline into existence. | Dashed line to the target's head |
| `destroys` | Ends the target lifeline. | Solid line to a cross |

`async` is valid only after the target of `calls`, and makes that call
non-blocking. It is not valid on any other verb.

`replies`, `sends`, and the colon call form are not part of the language. A
document that uses them is reported as an unsupported sequence form, not
silently reinterpreted.

## Order

Source order is behavior order. The single exception is `par`, whose branches
are unordered relative to each other. Everything else — including the contents
of any one branch — happens in the order it is written.

## Fragments

A fragment groups messages and carries a condition. Each head takes a fixed
number of operands:

| Head | Operands | Means |
| --- | --- | --- |
| `alt` | One or more `when`, with an optional final `else` | Exactly one branch runs |
| `opt` | Exactly one `when` | The branch runs or does not |
| `loop` | Exactly one `when` | The branch repeats while the condition holds |
| `break` | Exactly one `when` | The branch runs and the enclosing interaction ends |
| `par` | Two or more `branch` | The branches run without a defined order between them |
| `critical` | Exactly one `branch` | The branch must not be interleaved |
| `assert` | Exactly one `branch` | The branch is the only valid continuation |
| `neg` | Exactly one `branch` | The branch is invalid behavior |

Fragments nest. A nested fragment stays inside the operand that contains it,
and the item after a fragment is a sibling of that fragment, not of its last
child.

An `else` operand is valid only on `alt`, only once, and only last.

## Boundaries

`outside` is the endpoint for a message that crosses the interaction's
boundary. It is a reserved word: no lifeline may be named `outside`, and no
lifeline alias may contain `@`.

A `## Gates` section names those boundary points so that an enclosing
interaction can bind to them. A `ref` to another interaction draws as a single
frame and binds its gates to local lifelines; it never copies the referenced
interaction's messages into the referring one.

## Activations

An activation is derived, never authored. A lifeline is active from the arrival
of a `calls` until its correlated `returns`. Correlation is by message
identity, not by position, so an unmatched call is a reportable defect rather
than a guess.

## Deliberately excluded

`strict`, `seq`, `ignore`, and `consider` fragments; coregions; continuations;
general orderings; part decomposition; execution specifications as an authored
construct; state invariants; and time and duration constraints.

These are excluded because each adds authored syntax that a reader must learn
in exchange for a case this project has not met. Excluded is a decision, not a
backlog: adding one requires changing this document first.

## Notes

- Every row in the [feature cut](./feature-cut.md) that is not `done` is
  measured against this document.
- A test for this language should be readable as a table of authored source and
  expected outcome. Where a rule above has no such test, the rule is an
  intention rather than a guarantee.
