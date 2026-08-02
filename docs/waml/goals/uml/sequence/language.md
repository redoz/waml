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

## Scenarios

Each scenario has an identifier. A test that covers a scenario must show that
identifier in its name or in a comment. A scenario without a test is an
intention. A rule in this document without a scenario is not yet testable.

Each scenario applies to a document with the type `uml.Sequence`.

### Messages

#### SEQ-MSG-1 — a call makes a synchronous message

**Given** a document with the lifelines `A` and `B`
**And** the item `- calls B: fetch` below `A`
**When** the tool analyses the document
**Then** the model contains one message from `A` to `B`
**And** the message kind is `calls`
**And** the tool reports no diagnostic

#### SEQ-MSG-2 — a return matches an earlier call

**Given** a call from `A` to `B`
**And** a return from `B` to `A`
**When** the tool analyses the document
**Then** the return correlates to that call
**And** the tool reports no diagnostic

#### SEQ-MSG-3 — a return without a call is an error

**Given** a return from `B` to `A`
**And** no call from `A` to `B` before it
**When** the tool analyses the document
**Then** the tool reports one diagnostic at the return
**And** the model keeps the declared return

#### SEQ-MSG-4 — async is valid only after a call target

**Given** the item `- calls B async: fetch`
**When** the tool analyses the document
**Then** the message is asynchronous
**And** the tool reports no diagnostic

#### SEQ-MSG-5 — async on another verb is an error

**Given** the item `- signals B async: ping`
**When** the tool analyses the document
**Then** the tool reports one diagnostic at the word `async`

#### SEQ-MSG-6 — an old verb is not a message

**Given** the item `- replies to A: result`
**When** the tool analyses the document
**Then** the tool reports an unsupported sequence form
**And** the model contains no message for that item
**And** the source round-trips without a change

#### SEQ-MSG-7 — a create message starts the lifeline

**Given** a create message from `A` to `C`
**When** the solver solves the interaction
**Then** the head of `C` is at the row of that message
**And** no part of `C` is above that row

#### SEQ-MSG-8 — a destroy message ends the lifeline

**Given** a destroy message from `A` to `C`
**When** the solver solves the interaction
**Then** the line of `C` stops at the row of that message

### Order

#### SEQ-ORD-1 — source order is behavior order

**Given** three messages in the order `m1`, `m2`, `m3`
**When** the solver solves the interaction
**Then** the row of `m1` is above the row of `m2`
**And** the row of `m2` is above the row of `m3`

#### SEQ-ORD-2 — par removes the order between branches

**Given** a `par` fragment with two branches
**When** the tool analyses the document
**Then** the model gives no order between the two branches
**And** the model keeps the order inside each branch

### Fragments

#### SEQ-FRAG-1 — alt accepts one when

**Given** an `alt` fragment with one `when` operand
**When** the tool analyses the document
**Then** the model contains one fragment with one operand
**And** the tool reports no diagnostic

#### SEQ-FRAG-2 — alt accepts a final else

**Given** an `alt` fragment with `when` and then `else`
**When** the tool analyses the document
**Then** the model contains two operands
**And** the tool reports no diagnostic

#### SEQ-FRAG-3 — else must be last

**Given** an `alt` fragment with `when`, `else`, and then `when`
**When** the tool analyses the document
**Then** the tool reports one diagnostic at the `else`
**And** the source round-trips without a change

#### SEQ-FRAG-4 — else occurs one time only

**Given** an `alt` fragment with `when`, `else`, and `else`
**When** the tool analyses the document
**Then** the tool reports one diagnostic at the second `else`

#### SEQ-FRAG-5 — opt, loop, and break accept one when

**Given** a fragment with the head `opt`, `loop`, or `break`
**And** exactly one `when` operand
**When** the tool analyses the document
**Then** the tool reports no diagnostic

#### SEQ-FRAG-6 — a wrong operand count is an error

**Given** an `opt` fragment with two `when` operands
**When** the tool analyses the document
**Then** the tool reports one diagnostic at the second operand

#### SEQ-FRAG-7 — par needs two branches

**Given** a `par` fragment with one `branch` operand
**When** the tool analyses the document
**Then** the tool reports one diagnostic at the fragment head

#### SEQ-FRAG-8 — critical, assert, and neg accept one branch

**Given** a fragment with the head `critical`, `assert`, or `neg`
**And** exactly one `branch` operand
**When** the tool analyses the document
**Then** the tool reports no diagnostic

#### SEQ-FRAG-9 — a fragment stays in its parent operand

**Given** a `par` fragment inside an operand of an `alt` fragment
**When** the tool analyses the document
**Then** the parent of the `par` fragment is that operand

#### SEQ-FRAG-10 — the item after a fragment is a sibling

**Given** a message below a `par` fragment at the same level
**When** the tool analyses the document
**Then** that message is a sibling of the fragment
**And** that message is not a child of the last branch

#### SEQ-FRAG-11 — a fragment frame holds its content

**Given** a fragment that contains two messages
**When** the solver solves the interaction
**Then** the frame of the fragment holds both message rows
**And** the frame holds each lifeline that those messages touch

### Boundaries

#### SEQ-BND-1 — outside is a boundary endpoint

**Given** a message from `outside` to `A`
**When** the tool analyses the document
**Then** the source endpoint of the message is the boundary
**And** the tool reports no diagnostic

#### SEQ-BND-2 — outside is a reserved name

**Given** a lifeline with the name `outside`
**When** the tool analyses the document
**Then** the tool reports one diagnostic at that lifeline

#### SEQ-BND-3 — an alias must not contain an at sign

**Given** a lifeline alias that contains `@`
**When** the tool analyses the document
**Then** the tool reports one diagnostic at that alias

#### SEQ-BND-4 — a gate is on the frame boundary

**Given** an interaction with one gate
**When** the solver solves the interaction
**Then** the position of the gate is on the frame boundary

#### SEQ-BND-5 — a reference does not copy messages

**Given** a `ref` to an interaction that contains three messages
**When** the solver solves the referring interaction
**Then** the solver makes one frame for the reference
**And** the referring interaction contains no copy of those three messages

#### SEQ-BND-6 — a reference binds its gates

**Given** a `ref` with two gates and two bindings
**When** the solver solves the referring interaction
**Then** each outer message connects to the position of its gate

### Activations

#### SEQ-ACT-1 — a call starts an activation

**Given** a call from `A` to `B`
**And** a return from `B` to `A`
**When** the solver solves the interaction
**Then** `B` has one activation
**And** the activation starts at the call row
**And** the activation stops at the return row

#### SEQ-ACT-2 — correlation uses message identity

**Given** two calls from `A` to `B`
**And** two returns from `B` to `A`
**When** the solver solves the interaction
**Then** each return correlates to its own call
**And** the correlation does not change when a message moves in the source

### Excluded constructs

#### SEQ-EXC-1 — an excluded fragment head is not accepted

**Given** a fragment with the head `strict`, `seq`, `ignore`, or `consider`
**When** the tool analyses the document
**Then** the tool reports one diagnostic at the fragment head
**And** the model contains no fragment for that item
**And** the source round-trips without a change

### Recovery

#### SEQ-REC-1 — a bad item does not stop the next item

**Given** a malformed item between two valid messages
**When** the tool analyses the document
**Then** the tool reports one diagnostic at the malformed item
**And** the model contains both valid messages

#### SEQ-REC-2 — malformed source stays lossless

**Given** a document that contains a malformed item
**When** the tool reads the document and then writes it
**Then** the bytes do not change

## Notes

- Every row in the [feature cut](./feature-cut.md) that is not `done` is
  measured against this document.
- This document uses ASD-STE100 Simplified Technical English. Keep new
  scenarios in the same form: one idea for each sentence, an active verb, and
  the present tense.
- A new rule needs a new scenario in the same change. A rule without a scenario
  cannot fail a test, so nobody finds out when the product stops obeying it.
