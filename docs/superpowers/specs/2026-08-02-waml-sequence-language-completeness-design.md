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
3. **Message intent uses words.** The canonical verbs are `calls`, `invokes`, `sends`,
   `replies`, `creates`, and `destroys`.
4. **Existing `sends` documents keep their meaning.** `sends` is the asynchronous
   signal or event form. `invokes` is the new asynchronous operation-call form.
5. **Boundary endpoints are structural.** `outside` represents a found or lost message.
   Named gates use `@gate` inside an interaction and `ref@gate` around an interaction
   use. Found and lost are not extra message verbs.
6. **Existing activation inference remains the default.** A synchronous `calls` message
   opens an activation and its `replies` message closes it. Other message kinds do not
   create an open-ended activation.
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
| asynchronous operation call | add | `invokes` |
| asynchronous signal | keep and clarify | `sends` |
| reply | keep | `replies` |
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
- call/reply activation derivation;
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

The canonical message shape stays sentence-like:

```text
- <source> <verb> <target>[: <signature-or-signal>]
```

The grammar is:

```text
message       := "-" endpoint verb endpoint (":" payload)?
endpoint      := lifeline-handle | "outside" | local-gate | use-gate
local-gate    := "@" name
use-gate      := use-alias "@" name
verb          := "calls" | "invokes" | "sends" | "replies"
               | "creates" | "destroys"
```

The colon and payload remain optional for source compatibility. The formatter does not
invent a payload.

### Message meaning

```markdown
- customer calls order: `submit()`
- order replies customer: `accepted`
- order invokes order: `recalculate()`
- order sends bus: `OrderPlaced`
- order creates worker: `OrderWorker`
- order destroys worker
```

| Verb | Meaning | Sender waits | Activation effect |
| --- | --- | --- | --- |
| `calls` | synchronous operation call | yes | open activation on target |
| `invokes` | asynchronous operation call | no | none |
| `sends` | asynchronous signal or event | no | none |
| `replies` | return from a synchronous call | n/a | close matching activation |
| `creates` | create target lifeline instance | n/a | start target stem at message |
| `destroys` | destroy target lifeline instance | n/a | end target stem at message |

A self message is valid when source and target are the same lifeline. A self `calls`
message opens a nested activation. A later self `replies` message closes the most recent
matching activation. The solver draws these messages as loopbacks; the semantic model
does not use a special self-message variant.

A reply is matched by the current per-lifeline activation stack. An explicit correlation
identifier is not part of this project. An unmatched reply is kept in the model and gets
a diagnostic.

### Found and lost messages

`outside` is an anonymous endpoint at the interaction-frame boundary:

```markdown
- outside sends order: `OrderImported`
- order sends outside: `AuditRecord`
```

An `outside` source makes a found message. An `outside` target makes a lost message.
`outside` to `outside` is invalid. `creates` and `destroys` require a local lifeline as
their target. Other verbs can use `outside` on one side, subject to normal reply
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
    - order calls payment: `charge()`
  - else
    - order replies customer: `out of stock`

- opt
  - when `receipt requested`
    - order sends bus: `ReceiptRequested`

- loop
  - when `an item remains`
    - order calls inventory: `reserveNext()`
```

`alt` requires one or more `when` operands and permits one final `else`. `opt`, `loop`,
and `break` each require exactly one `when` operand. Loop bounds are deferred; the guard
states the continuation condition.

Parallel and unconditional fragments use `branch` operands:

```markdown
- par
  - branch `payment`
    - order calls payment: `charge()`
  - branch `fulfilment`
    - order invokes inventory: `reserve()`

- critical
  - branch
    - order calls ledger: `record()`

- assert
  - branch
    - ledger replies order: `recorded`

- neg
  - branch
    - order sends bus: `ChargeWithoutOrder`
```

`par` requires two or more `branch` operands. A branch label is optional and is not a
guard. `critical`, `assert`, and `neg` each require exactly one `branch` operand.

`break` uses a guard because it runs only when its condition is true:

```markdown
- break
  - when `customer cancels`
    - order replies customer: `cancelled`
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
- approved
```

Gate names use the same identifier rules as aliases and must be unique in the document.
Inside that document, a gate endpoint is written as `@name`:

```markdown
- @request calls payment: `authorize()`
- payment replies @approved: `approved`
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
- order calls auth@request: `authorize()`
- auth@approved replies order: `approved`
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

- `invokes` messages;
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

`DeclaredFragmentKind` contains `Alt`, `Opt`, `Loop`, `Par`, `Break`, `Critical`,
`Assert`, and `Neg`. A declared operand records either a guard, an optional branch label,
or `else`, plus its ordered child items.

`DeclaredInteractionUse` records its syntax node, link, alias, ordered bindings, and any
authored gate endpoint references. It exists even when its target document does not
resolve.

### Resolved/runtime layer

The runtime replaces authored handles with typed identifiers. `SeqEdge.from` and
`SeqEdge.to` must become endpoint references rather than unqualified lifeline strings.
The message enum adds `AsyncCall`; existing `Calls` maps to `SyncCall` and existing
`Sends` maps to `AsyncSignal`. Serialization names must be explicit so an enum rename does
not change external data by accident.

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
- a malformed gate endpoint;
- an invalid fragment operand word;
- a missing `ref` link or alias;
- a malformed `bind` line.

The formatter emits the canonical forms in this document. It preserves valid existing
`calls`, `sends`, `replies`, `creates`, `destroys`, `alt`, `opt`, and `loop` source. It
must not rewrite `sends` to `invokes`, because the two forms have different meaning.

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
- unmatched or structurally impossible replies;
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
- `sends` remains asynchronous and does not open an activation;
- reply matching uses the existing activation stack rule;
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
- A synchronous call/reply pair opens and closes the correct activation, including
  recursion.
- Async calls and signals do not create unmatched activations.
- Fragment operand rules and `par` partial order are deterministic.
- Interaction references resolve bindings and gates without flattening target documents.
- All required invalid cases produce stable diagnostics and retain recoverable siblings.

### Formatter and serialization tests

- Every new construct is formatter-idempotent.
- Parse → format → parse preserves the declared model.
- Existing source verbs and fragments keep their spelling and meaning.
- Existing serialized `SequenceDoc` fixtures still decode.
- New endpoint, fragment, and interaction-use values round-trip through serialization.

### Solver contract tests

This project does not redesign the sequence renderer, but the solver must accept the new
runtime model without silent loss. Text goldens must prove:

- self-message loopbacks and nested activations;
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

- “Should Order wait for the response?” → `calls` or `invokes`.
- “Is this an operation call or an event notification?” → `invokes` or `sends`.
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
