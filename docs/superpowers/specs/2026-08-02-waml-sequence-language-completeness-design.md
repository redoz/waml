# WAML practical UML sequence language completeness

Date: 2026-08-02
Status: approved for specification; implementation not started

## Why

WAML can describe and render a useful sequence diagram, but its authored language covers
only a small part of UML interactions. It supports lifelines, five message verbs, and the
`alt`, `opt`, and `loop` combined fragments. The parser rejects self messages and several
standard interaction constructs that the runtime will need for an editable sequence
canvas.

The next product step is an intent-first visual editor. That editor must not force a user
to choose an abstract UML term before the user knows what the term means. It must be able
to ask questions such as “Should Order wait for the response?” and then write precise
WAML. The language must therefore carry the full practical meaning before editor work
starts.

This design defines that language vertical slice. It is based on UML 2.5.1, chapter 17,
Interactions: <https://www.omg.org/spec/UML/>. It does not try to reproduce the complete
UML interchange metamodel.

## Goals

1. Express the sequence behavior that teams commonly need in software design.
2. Keep source text readable without knowledge of UML notation.
3. Preserve all valid sequence documents that WAML accepts today.
4. Give each accepted construct one unambiguous semantic representation.
5. Keep the lossless syntax tree, formatter, declared model, resolved model,
   diagnostics, and solver projections in agreement.
6. Give a later visual editor enough information to ask concrete intent questions and
   write WAML without a reduced intermediate notation.

## Non-goals

- Build or change the visual sequence editor.
- Implement every UML metamodel association or XMI interchange field.
- Add free-form arrow syntax as a second authoring language.
- Infer concurrency from vertical alignment or from different lifelines.
- Add explicit execution specifications, state invariants, time constraints, duration
  constraints, continuations, general orderings, part decomposition, or coregions in
  this project.
- Change class, activity, or state-machine syntax.

## Locked decisions

1. **Source order is behavior order.** Outside a `par` fragment, top-to-bottom item order
   is a total order. WAML does not use UML weak sequencing as its default.
2. **Concurrency is explicit.** A `par` fragment is the only new construct that removes
   order between branches. Each branch still has a total internal order.
3. **Message intent uses words.** The canonical verbs are `calls`, `returns`, `signals`,
   `creates`, and `destroys`. The `async` modifier applies only to `calls`. `returns` is
   WAML's readable spelling for the UML reply message sort.
4. **Signals are one-way.** `signals` is the canonical asynchronous Signal form and can
   carry a command or an event. Existing `sends` documents keep the same meaning as a
   compatibility spelling. `calls ... async` remains an operation call, not a signal.
5. **Boundary endpoints are structural.** `outside` represents a found or lost message.
   Named gates use `@gate` inside an interaction and `ref@gate` around an interaction
   use. Found and lost are not extra message verbs.
6. **Calls and returns can be correlated.** A call can declare a stable identity with
   `as`. A return can select that call with `for`, name the original caller with `to`, or
   omit both when exactly one open call is eligible. A matched return closes the called
   lifeline's activation.
7. **The useful combined-fragment set expands now.** Add `par`, `break`, `critical`,
   `assert`, and `neg`. Keep `alt`, `opt`, and `loop`.
8. **Interaction reuse is in scope.** Add interaction uses (`ref`), explicit lifeline
   bindings, and named gates.
9. **The parser must recover.** One malformed item must not hide later lifelines,
   messages, operands, fragments, or references.
10. **No second semantic path.** Parser actions, formatter output, editor actions, and
    direct source edits must all pass through the same syntax and semantic model.

## Coverage boundary

A UML interaction feature is in scope now when it adds useful behavior, cannot be stated
clearly with current WAML, can be explained with a concrete user question, and has a
coherent later visual form.

| UML concept | Decision | WAML form |
| --- | --- | --- |
| synchronous call | keep | `calls` |
| asynchronous operation call | add | `calls <target> async` |
| asynchronous signal | keep with readable UML spelling | `signals` |
| reply/operation return | keep with readable spelling | `returns` |
| create message | keep | `creates` |
| delete message | keep | `destroys` |
| self and recursive message | add | same endpoint on both sides |
| found and lost message | add | `outside` endpoint |
| alternatives and optional behavior | keep | `alt`, `opt` |
| repetition | keep | `loop` |
| parallel behavior | add | `par` with `branch` operands |
| break, critical, assertion, invalid trace | add | `break`, `critical`, `assert`, `neg` |
| interaction use | add | `ref`, `bind`, and gates |
| execution specification | defer | derived activations remain sufficient for now |
| state invariant | defer | needs a real authoring and editor use case |
| time and duration constraint | defer | needs stable occurrence names and unit rules |
| `strict` | defer | redundant with WAML total order |
| `seq` | defer | conflicts with the chosen total-order default |
| `ignore` and `consider` | defer | trace filtering is uncommon in design diagrams |
| coregion | defer | `par` is the clear practical concurrency form |
| continuation and general ordering | defer | partial-order notation would weaken clarity |
| part decomposition | defer | interaction use gives practical reuse |

Deferred means “not accepted by the grammar.” A diagnostic must name the unsupported
construct when the parser can identify it. The semantic model must not contain unused
placeholder variants for deferred features.

## Current WAML baseline

The current core model uses `SequenceDoc`, `SeqNode`, `SeqEdge`, and `SeqChild`.
`MessageVerb` contains `Calls`, `Sends`, `Replies`, `Creates`, and `Destroys`.
`FragmentKind` contains `Alt`, `Opt`, and `Loop`. The ordered `SequenceDoc.items` stream
already makes document order the time order.

The implementation must preserve these properties:

- lifeline `title`, `ref`, `alias`, and `order` fields;
- message source, target, optional signature, and source order;
- nested combined fragments and their operand guards;
- call/return activation derivation;
- create and destroy stem boundaries;
- lossless parse and print behavior for unchanged source.

The parser currently rejects self messages and has explicit unsupported paths for
`par`, gates, found messages, and lost messages. These paths must become normal parsed
constructs only for the features accepted by this design.

## Authoring syntax

### Lifelines

The existing `## Lifelines` syntax does not change. A lifeline handle is its explicit
alias when present. Otherwise, the existing handle derivation rules apply.

```markdown
## Lifelines

- [Customer](../actors/customer.md) as customer
- [Order](../domain/order.md) as order
- [Event bus](../infra/event-bus.md) as bus
```

`outside` is reserved and cannot be a lifeline alias. An alias cannot contain `@` because
that character separates an interaction-use alias from a gate name.

### Message grammar

Calls, returns, and signals have sentence-like canonical forms. Calls and returns expose
operation identity and correlation. Create and destroy messages retain their current
endpoint-first shape until their separate syntax review is complete.

The canonical grammar is:

```text
message        := call-message | return-message | signal-message | other-message
call-message   := "-" endpoint "calls" endpoint ("async")? operation? call-tag?
return-message := "-" endpoint "returns" value? return-target? call-match?
signal-message := "-" endpoint "signals" endpoint signal?
other-message  := "-" endpoint other-verb endpoint (":" payload)?
operation      := inline-code
value          := inline-code
signal         := inline-code
call-tag       := "as" call-id
return-target  := "to" endpoint
call-match     := "for" call-id
call-id        := name
endpoint       := lifeline-handle | "outside" | local-gate | use-gate
local-gate     := "@" name
use-gate       := use-alias "@" name
other-verb     := "creates" | "destroys"
```

The `async` modifier occurs after the target and before the operation. It is valid only on
a `calls` message. `as` occurs after the operation. A return value occurs directly after
`returns`; `to` then `for` is the fixed order when both clauses are present. Inline code
delimits operations and values, so `as`, `to`, and `for` inside those values are not
grammar tokens.

`signals` is inherently asynchronous and therefore does not accept the `async` modifier.
Its optional inline-code value names the command or event delivered to the target.

The existing colon forms, `replies` verb, and `sends` verb remain accepted compatibility
spellings. They keep their existing meaning and lossless source representation. New
editor actions and documentation emit the canonical forms above.

### Message meaning

```markdown
- customer calls order `submit()` as submission
- order returns `accepted` for submission
- order calls order async `recalculate()` as recalculation
- order signals bus `OrderPlaced`
- order creates worker: `OrderWorker`
- order destroys worker
```

| Verb | Meaning | Sender waits | Activation effect |
| --- | --- | --- | --- |
| `calls` | synchronous operation call | yes | open activation on target |
| `calls ... async` | asynchronous operation call | no | open activation on target |
| `signals` | asynchronous one-way command or event | no | none |
| `returns` | completion of a synchronous or asynchronous call | n/a | close matching activation |
| `creates` | create target lifeline instance | n/a | start target stem at message |
| `destroys` | destroy target lifeline instance | n/a | end target stem at message |

`signals` delivers one asynchronous stimulus and has no operation-return relationship.
The signal can describe a one-way command such as `StartJob` or an event such as
`OrderPlaced`. A response is another independent `signals` message in the opposite
direction. WAML does not correlate the two signals with `as` or `for`; a protocol-level
correlation value belongs in the signal payload.

A self message is valid when source and target are the same lifeline. A self `calls`
message opens a nested activation. A later matched `returns` message closes it. The solver
draws these messages as loopbacks; the semantic model does not use a special self-message
variant.

Every call has operation-completion semantics. The authored trace can omit the return when
the completion is outside the detail shown by the diagram. A return with no value is a
void completion:

```markdown
- payment returns for paymentRequest
```

Call identities use the same identifier rules as aliases and are unique within one
sequence document. They are optional on synchronous and asynchronous calls. An identity
is necessary when an authored return cannot otherwise select exactly one open call. The
visual editor should add identities when it creates overlapping calls.

These forms are equivalent when each resolves to one call:

```markdown
- order calls payment async `charge()` as paymentRequest
- payment returns `approved` for paymentRequest

- payment returns `approved` to order

- payment returns `approved`
```

The three return lines above are alternative examples, not three returns for the same
call. Resolution uses this deterministic algorithm:

1. If `for` is present, resolve that exact authored call identity. Derive its original
   caller and callee.
2. Otherwise, collect eligible preceding calls whose callee is the returning endpoint and
   whose explicit return has not already occurred.
3. If `to` is present, keep only calls whose caller is that endpoint.
4. One candidate resolves the return. No candidates produce an unmatched-return
   diagnostic. More than one candidate produces an ambiguous-return diagnostic that
   suggests `for <call-id>`.

When both `to` and `for` are present, `for` identifies the call and `to` validates the
derived caller. A conflicting source, `to` endpoint, or call identity is an error. The
invalid return stays in the declared model.

### Found and lost messages

`outside` is an anonymous endpoint at the interaction-frame boundary:

```markdown
- outside signals order `OrderImported`
- order signals outside `AuditRecord`
```

An `outside` source makes a found message. An `outside` target makes a lost message.
`outside` to `outside` is invalid. `creates` and `destroys` require a local lifeline as
their target. Other verbs can use `outside` on one side, subject to normal return
validation.

The declared and resolved models represent this as `EndpointRef::Outside`, not as a
synthetic lifeline. The solver places the endpoint on the nearest suitable frame edge and
keeps the authored source order.

### Combined fragments and operands

Fragment heads stay in the ordered message stream. Operand markers are one indentation
level below a fragment, and operand items are one level below the marker.

Conditional fragments keep the existing operand words:

```markdown
- alt
  - when `stock is available`
    - order calls payment `charge()`
  - else
    - order returns `out of stock` to customer

- opt
  - when `receipt requested`
    - order signals bus `ReceiptRequested`

- loop
  - when `an item remains`
    - order calls inventory `reserveNext()`
```

`alt` requires one or more `when` operands and permits one final `else`. `opt`, `loop`,
and `break` each require exactly one `when` operand. Loop bounds are deferred; the guard
states the continuation condition.

Parallel and unconditional fragments use `branch` operands:

```markdown
- par
  - branch `payment`
    - order calls payment `charge()`
  - branch `fulfilment`
    - order calls inventory async `reserve()` as reservation

- critical
  - branch
    - order calls ledger `record()`

- assert
  - branch
    - ledger returns `recorded` to order

- neg
  - branch
    - order signals bus `ChargeWithoutOrder`
```

`par` requires two or more `branch` operands. A branch label is optional and is not a
guard. `critical`, `assert`, and `neg` each require exactly one `branch` operand.

`break` uses a guard because it runs only when its condition is true:

```markdown
- break
  - when `customer cancels`
    - order returns `cancelled` to customer
```

The operators have these trace meanings:

- `alt`: exactly one true operand is selected; `else` is the fallback.
- `opt`: the operand runs when its guard is true.
- `loop`: the operand repeats while its guard is true.
- `par`: each branch keeps internal order; there is no order between branches.
- `break`: the operand runs and ends the nearest enclosing interaction or operand.
- `critical`: its operand cannot interleave with a sibling `par` branch.
- `assert`: the operand describes the only valid continuation at that point.
- `neg`: the operand describes an invalid trace.

Fragments can nest. A nested `par` creates a new concurrency scope. Source order after a
fragment resumes only after the fragment completes.

### Interaction uses and gates

A sequence can declare named gates after its lifelines:

```markdown
## Gates

- request
```

Gate names use the same identifier rules as aliases and must be unique in the document.
Inside that document, a gate endpoint is written as `@name`:

```markdown
- @request calls payment `authorize()` as authorization
- payment returns `approved` for authorization
```

An interaction use is an ordered item in `## Messages`:

```markdown
- ref [Authorize payment](./authorize-payment.md) as auth
  - bind order to caller
  - bind payment to payment
```

The left side of `bind` is a lifeline handle in the current interaction. The right side is
a lifeline handle in the referenced interaction. Bindings are explicit; WAML does not
silently bind equal names or equal links. Each referenced lifeline that participates in
the referenced trace must have exactly one binding unless it is created inside the
referenced interaction.

The use alias is required and must be unique among local lifeline aliases and interaction
use aliases. A message can connect to a gate on the interaction-use frame:

```markdown
- order calls auth@request `authorize()` as authorization
- auth@request returns `approved` for authorization
```

`auth@request` resolves the `request` gate in the document referenced by `auth`. An outer
message and an inner gate message describe the two sides of the same boundary. WAML does
not flatten the referenced interaction into the parent `SequenceDoc`; it resolves a typed
interaction-use node with a document reference, bindings, and gate references.

A missing document, missing gate, missing lifeline binding, duplicate binding, or binding
type mismatch produces a diagnostic. The interaction use remains in the declared model
so the formatter and editor can preserve and repair it. Direct or indirect reference
cycles are invalid.

## Ordering and concurrency model

Normal item order is deterministic:

```text
item 1 happens before item 2 happens before item 3
```

This rule applies even when adjacent messages use different lifelines. Vertical placement
therefore has one plain-language meaning. A later editor can ask “Can these happen at the
same time?” If the answer is yes, it wraps the items in separate `par` branches.

Within a `par`, WAML defines a partial order:

```text
all items in branch A keep A's order
all items in branch B keep B's order
no A-to-B or B-to-A order is implied
```

The join at the bottom of `par` completes after all non-broken branches complete. A
`critical` region in one branch is atomic relative to events in sibling branches. Nested
interaction uses are single ordered items in the parent, although their internal trace
keeps its own ordering and fragments.

This explicit model is intentionally more deterministic than UML weak sequencing. It is
also the reason that `strict`, `seq`, general ordering, and coregion are not accepted now.

## Syntax and semantic model

### Lossless syntax layer

The lossless syntax tree must add typed nodes for:

- the `async` modifier on `calls` messages;
- call identities introduced by `as`;
- return values and the optional `to` and `for` clauses;
- canonical `signals` messages while preserving legacy `sends` tokens;
- boundary and gate endpoints;
- the five new fragment heads;
- `branch` operands and their optional labels;
- gate declarations;
- interaction uses and `bind` entries.

Trivia, inline code, links, indentation, and malformed tokens remain lossless. Existing
syntax node kinds keep their identity. New constructs must not be represented as opaque
Markdown lists after they are recognized.

### Declared layer

The declared layer records authored references before resolution. Its conceptual shape is:

```rust
enum DeclaredEndpointRef {
    Lifeline(String),
    Outside,
    LocalGate(String),
    UseGate { use_alias: String, gate: String },
}

enum DeclaredMessageKind {
    SyncCall,
    AsyncCall,
    AsyncSignal,
    Reply,
    Create,
    Delete,
}

enum DeclaredSequenceItem {
    Message(DeclaredMessageId),
    Fragment(DeclaredFragmentId),
    InteractionUse(DeclaredInteractionUseId),
}
```

A declared call records an optional authored `call_id`. A declared return records its
optional value, authored `to` endpoint, and authored `for` call identity separately. The
declared layer does not guess a match. It preserves all three fields even when they
conflict.

`DeclaredFragmentKind` contains `Alt`, `Opt`, `Loop`, `Par`, `Break`, `Critical`,
`Assert`, and `Neg`. A declared operand records either a guard, an optional branch label,
or `else`, plus its ordered child items.

`DeclaredInteractionUse` records its syntax node, link, alias, ordered bindings, and any
authored gate endpoint references. It exists even when its target document does not
resolve.

### Resolved/runtime layer

The runtime replaces authored handles with typed identifiers. `SeqEdge.from` and
`SeqEdge.to` must become endpoint references rather than unqualified lifeline strings.
The message enum adds `AsyncCall`; an unmodified `calls` message maps to `SyncCall`, a
`calls ... async` message maps to `AsyncCall`, and canonical `signals` plus legacy `Sends`
map to `AsyncSignal`. Serialization names must be explicit so an enum rename does not
change external data by accident.

The semantic kind for canonical `returns` and legacy `replies` is `Reply`, matching UML.
Each resolved call has a stable message identity plus its optional authored call identity.
Each resolved return has `returns_call: Option<MessageId>`. Resolution fills this field
with the exact call selected by `for` or by unambiguous inference. The resolved source and
target must agree with the selected call's callee and caller. An unresolved return remains
in the ordered message pool with `returns_call: None`.

`SeqChild` adds an interaction-use reference. The fragment and operand pools keep the
ordered child structure. Interaction uses live in their own pool because they have a
document reference, frame identity, bindings, and gates; they are not messages and not
combined fragments.

The resolver does not expand a referenced sequence. It resolves and validates the
reference graph, bindings, and gate identities. Consumers can traverse into the target
document when they need a composed trace or a visual drill-down.

## Parsing, recovery, and formatting

The parser recognizes an item from its first reserved word and endpoint shape. It must use
the same indentation ownership rules as current fragments.

Recovery boundaries are the next sibling list item, operand marker, section heading, or
document end. A malformed nested item stays under its nearest valid fragment or
interaction use. The parser must continue after:

- an unknown message verb;
- a missing source or target;
- a malformed `as`, `to`, or `for` clause;
- a malformed gate endpoint;
- an invalid fragment operand word;
- a missing `ref` link or alias;
- a malformed `bind` line.

The formatter emits the canonical forms in this document for new edits. It preserves
valid existing colon-form calls, `replies`, and `sends` source, as well as existing
`creates`, `destroys`, `alt`, `opt`, and `loop` source. It must not rewrite legacy `sends`
to `calls ... async`, because the two forms have different meaning.

Formatting is idempotent. Parse → format → parse must preserve the declared semantic
model. Editing unrelated source must not normalize malformed interaction content that the
formatter cannot safely understand.

## Validation and diagnostics

Diagnostics must identify the source span and use the authored names. Required checks are:

- duplicate or reserved lifeline aliases;
- unknown lifelines, gates, or interaction-use aliases;
- `outside` on both sides of one message;
- non-lifeline target for `creates` or `destroys`;
- use of a lifeline before its create message or after its destroy message;
- duplicate create or destroy events;
- duplicate call identities;
- an unknown call identity in `for`;
- unmatched, ambiguous, already-completed, or structurally impossible returns;
- a return source or `to` endpoint that conflicts with the call selected by `for`;
- wrong operand word or operand count for a fragment kind;
- `else` that is not final or occurs more than once;
- fewer than two `par` branches;
- duplicate gate declarations;
- unresolved, cyclic, incomplete, or duplicate interaction-use bindings;
- an outer gate connection that has no matching referenced gate;
- a feature that this design explicitly defers.

Diagnostics are non-destructive. If enough data exists, invalid constructs remain in the
declared model and in source-order projections. The resolved model omits only links that
cannot be typed safely; it does not omit later valid siblings.

## Backward compatibility

Every sequence document accepted before this change must still parse with the same
meaning. In particular:

- `calls` remains synchronous;
- legacy `sends` maps to the same `AsyncSignal` kind as canonical `signals` and does not
  open an activation;
- legacy `replies` maps to the same UML `Reply` kind as canonical `returns`;
- untagged return matching retains the existing activation-stack result when that result
  is unambiguous;
- fragment ordering and guards remain unchanged;
- documents do not need a `## Gates` section;
- aliases and links keep their existing syntax.

This project needs no source migration. Internal enum changes need explicit serde
compatibility tests for existing serialized shapes. New enum variants must be additive.
The formatter must not create new syntax in an unchanged old document.

## Tests and acceptance criteria

The project is complete only when the whole language path agrees.

### Syntax tests

- Each new message, endpoint, fragment, operand, gate, reference, and binding has a
  positive parse snapshot.
- Self messages, found messages, lost messages, nested fragments, and nested references
  have focused snapshots.
- Each malformed form has a recovery snapshot that proves later valid items survive.
- Existing sequence snapshots remain unchanged unless a test intentionally exercises a
  former unsupported path.

### Semantic tests

- Each source verb maps to exactly one message kind.
- Endpoint references resolve to lifelines, outside, local gates, or use gates.
- A synchronous call/return pair opens and closes the correct activation, including
  recursion.
- An async call opens a callee activation without suspending the caller, and its matched
  return closes the correct activation.
- `for`, `to`, and source-only return forms resolve by the specified candidate algorithm.
- Multiple eligible open calls produce an ambiguity diagnostic instead of an inferred
  match.
- Fragment operand rules and `par` partial order are deterministic.
- Interaction references resolve bindings and gates without flattening target documents.
- All required invalid cases produce stable diagnostics and retain recoverable siblings.

### Formatter and serialization tests

- Every new construct is formatter-idempotent.
- Parse → format → parse preserves the declared model.
- Existing source verbs and fragments keep their spelling and meaning.
- Legacy `replies`, `sends`, and colon-form messages retain their spelling and semantic
  kind.
- Existing serialized `SequenceDoc` fixtures still decode.
- New endpoint, fragment, and interaction-use values round-trip through serialization.

### Solver contract tests

This project does not redesign the sequence renderer, but the solver must accept the new
runtime model without silent loss. Text goldens must prove:

- self-message loopbacks and nested activations;
- tagged and inferred call/return connections, including overlapping async calls;
- found and lost boundary endpoints;
- new fragment frame kinds and nested operand order;
- separate `par` branch order with a join after all branches;
- interaction-use frames, lifeline bindings, and gate connections;
- unchanged geometry semantics for existing documents.

Visual polish belongs to the later editor project. A minimal stable geometric projection
is sufficient for this language project.

### Done bar

The change is done when:

1. all valid examples in this design parse, resolve, format, serialize, and reach the
   interaction solver;
2. all listed invalid examples produce local diagnostics without hiding later valid
   items;
3. old sequence fixtures pass without source migration;
4. no deferred UML construct has a placeholder semantic variant;
5. the full relevant Rust test suite and formatting checks pass.

## Boundary for the later visual editor

The visual editor is a separate project. It consumes this language and does not define a
second notation. Its plain-language controls are projections over the message and
fragment kinds in this design.

Examples of later editor questions are:

- “Should Order wait for the response?” → `calls` or `calls ... async`.
- “Could more than one Payment call be in flight?” → add `as` to calls and `for` to
  returns.
- “Is this an operation call or a one-way signal?” → `calls ... async` or `signals`.
- “Does this message enter from outside this diagram?” → `outside` source.
- “Can these happen at the same time?” → separate `par` branches.
- “Does this condition select an alternative, repeat work, or stop the interaction?” →
  `alt`, `loop`, or `break`.

The editor should remain canvas-first. It can use a temporary sentence composer and
contextual questions, but it must write the same complete WAML syntax specified here.

## Implementation sequence for the next context

The implementation plan is not part of this document. The next context must first review
this specification, then create a file-level plan for this dependency order:

1. lossless syntax nodes and parser recovery;
2. formatter and round-trip fixtures;
3. declared types and lowering;
4. resolved/runtime types and validation;
5. solver projections and goldens;
6. compatibility and full-suite verification.

Do not start the visual editor until this vertical slice is complete.
