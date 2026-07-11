# UAML behavioral substrates — covering the four core UML families

## Goal

Extend UAML from structural-only (class/structure diagrams) to cover the
minimum set of core UML diagram families:

- class diagrams
- use case diagrams
- activity diagrams
- state machine diagrams
- sequence diagrams

...while preserving UAML's existing architecture: a **closed metaclass set**
per family for renderer dispatch, all domain vocabulary as **open stereotypes**,
presentation via **profiles**, and **graceful degradation** everywhere.

**Status:** metaclass/relationship model is settled. The concrete surface
grammar in this document is a **first pass** — good enough to start building,
expected to be refined. Parser-grade BNF is deferred to the implementation plan.

## The two-tier decision

UML elements split cleanly by whether they have independent identity:

- **Entity tier** — elements that exist on their own and are *curated* into
  views: classes, interfaces, enums, actors, use cases. These keep UAML's
  existing rule: **one node = one document**, assembled into `Diagram` views.
- **Behavior tier** — elements that exist only inside one ordered behavior: a
  state, an action, a message. These do **not** get their own document. A
  behavior is authored as **one self-contained document** whose body *is* the
  ordered behavior. The document is **both model and view** (there is no
  separate `Diagram` curation step — ordering and time cannot be re-curated).

Behavior documents **link into** the entity tier: a lifeline *is* a Class or
Actor; a state machine *describes* a Class; a call-behavior action *refines*
another behavior. All such references use UAML's existing markdown-link
mechanism.

## The three substrates

Above the existing `family.Metaclass` dispatch key sits a **substrate** layer.
Every metaclass belongs to exactly one substrate. The substrate decides
document shape and the renderer's top-level dispatch; family, stereotype and
profile keep their existing open/closed roles.

| substrate | tier | document shape | UML families |
|---|---|---|---|
| **structure** | entity | frontmatter + sections; node=doc; curated into `Diagram` | class, use case |
| **flow** | behavior | one doc = one directed graph; self-rendering | activity, state machine |
| **interaction** | behavior | one doc = one ordered interaction; self-rendering | sequence |

Four families collapse onto three grammars: **activity and state machine are
the same directed-graph substrate**, differing only by a document-level flavor
that tunes rendering (labels, final-node shape, swimlanes vs orthogonal
regions).

Document `type` for a behavior doc names the whole behavior and selects the
flavor: `uml.Activity`, `uml.StateMachine`, `uml.Sequence`. Structure-tier docs
keep per-node `type: uml.Class`, `uml.Actor`, etc.

## Surface grammar — one sentence, three markers

Every statement across all four families is the same sentence shape, so a reader
who learns one substrate can read the rest:

```
[fronted clauses] <verb> <target> [: <detail>]
```

- The **verb** is a closed-set, third-person-singular word (`associates`,
  `composes`, `includes`, `transitions`, `calls`, …). The **subject** is
  implicit — the enclosing document (structure) or the enclosing node (flow) —
  and is stated explicitly only where document order forbids grouping by subject
  (sequence messages: `Sender calls Receiver`).
- **Fronted clauses** are optional English adverbials that precede the verb
  (`on <trigger>`, `when <guard>`); a trailing `: <detail>` carries the payload
  (an effect, a signature, the ends of a relationship).

Three visual markers tell the reader what kind of token they are looking at.
This is the single rule that governs every bullet:

| marker | means | examples |
|---|---|---|
| **bare word** | UAML grammar keyword, **or** a local model reference | `transitions`, `to`, `on`; state target `Placed`; attribute type `OrderId`; role `order` |
| **`` `backtick` ``** | an **expression** — behavior-language, opaque to the model | event `` `place` ``, guard `` `items > 0` ``, effect `` `reserveStock` ``, signature `` `place(items)` `` |
| **`[link](path)`** | a **cross-document** reference | `refines [SubFlow](./sub.md)`, `carries [Order](./order.md)`, `describes: [Order](./order.md)` |

Expressions are new to the behavioral tier (structural documents carry only
references and grammar), so backticks introduce no change to the structural
surface: an attribute type or a role name is a *reference*, not an expression,
and stays bare. Backticks also bound arbitrary content unambiguously — a guard
`` `x > 0` `` or an event literally named `` `to` `` cannot collide with the
clause keywords — the same disambiguation job `{…}` does for multiplicity.

## Substrate 1 — structure (class + use case)

Keeps all seven existing structural metaclasses (`uml.Class`, `uml.Interface`,
`uml.Enum`, `uml.DataType`, `uml.Package`, `uml.Association`, `uml.Note`) and
adds two:

| `type` | renders as |
|---|---|
| `uml.Actor` | stick figure; node; `## Attributes` optional (usually none); may `specializes` another actor |
| `uml.UseCase` | ellipse; node; may `includes` / `extends` / `specializes` other use cases |

**System boundary** introduces no metaclass. It is a `frame` group in the
use-case `Diagram`'s `## Members` — the existing layout frame supplies the
titled box, and group membership already means "inside the system."

Relationship vocabulary extends the existing table with two dependency verbs.
The existing rule holds: **category fixes the line, verb fixes the end
adornment** — no new line logic.

| verb | category | line | end adornment | ends? | near → far |
|---|---|---|---|---|---|
| `associates` | association | solid | none (communication link) | optional | actor ↔ use case |
| `includes` | dependency | dashed | open → + `«include»` | no | base → included |
| `extends` | dependency | dashed | open → + `«extend»` | no | extension → base |
| `specializes` | generalization | solid | hollow ▷ | no | child → parent |

`associates` and `specializes` are existing verbs, reused. One context rule
differs by metaclass: an `associates` ends clause (`: <near> to <far>`) is
**required between classifiers** (as in `uaml-spec.md`) but **optional on an
actor↔use-case communication link** — a bare `- associates [Customer](./customer.md)`
implies no multiplicity. This is a rule keyed on the participating metaclasses,
not a contradiction of the structural requirement.

Actor and use-case
documents are ordinary node documents (frontmatter + optional `## Attributes` +
`## Relationships` + `## Notes`) curated into a `Diagram` exactly like classes.
**Class and use case diagrams share one substrate and one renderer path** — they
differ only in which metaclasses appear.

```markdown
---
type: uml.UseCase
title: Place Order
---
# Place Order

## Relationships
- includes [Authenticate](./authenticate.md)
- extends [Apply Coupon](./apply-coupon.md)
- associates [Customer](./customer.md)
```

## Substrate 2 — flow (activity + state machine)

One directed graph per document. `type: uml.Activity` or `uml.StateMachine`
selects the flavor; both parse identically. One frontmatter field, `describes:`,
links the behavior to the entity it belongs to for both flavors — a state
machine describes a Class, an activity describes (realizes) the use case or
operation it carries out.

### Nodes

Each node is a `###` heading, making it a mini-classifier that can own its own
sub-sections (transitions, internals, `#### Notes`).

Node kind is a **closed set**, marked by a leading keyword on the heading; an
absent keyword means a plain action (activity) or state (state machine). A
node's **identity** is its heading text with the leading kind-keyword removed —
so `### decision Ready to ship?` has identity *Ready to ship?*, and
`### object [Order](./order.md)` has identity *Order* (the link title). A
keyword-only heading (`### initial`) uses the keyword as its identity. That
identity is the name a `transitions to <target>` bullet resolves against.
Control nodes come in matched split/join pairs:

- `initial`, `final`
- `decision` (1→N, guarded branch) / `merge` (N→1, rejoin) — both render as a diamond ◇
- `fork` (1→N, concurrent split) / `join` (N→1, concurrent sync) — both render as a bar
- `object` — an object/data node, typed by a link
  (`### object [Order](./order.md)`); renders as a rectangle. Edges touching it
  are **object flows** (see below)
- (no keyword) → action / state

Under a node heading, each bullet leads with its own keyword or verb:

- **transitions** — the outgoing-edge verb. The source is the enclosing node
  (implicit, mirroring how `## Relationships` attach to their declaring
  classifier). Grammar:
  `- [on <trigger>] [when <guard>] transitions to <target> [: <effect>]`.
  All firing conditions front the verb: `on <trigger>` names the event,
  `when <guard>` the guard. The `transitions to <target>` core is the anchor,
  and the effect trails after `:`. A completion transition (no event) starts at
  the verb: `- transitions to final`. Trigger, guard, and effect are
  **expressions** and are backticked; the target is a **reference** and stays
  bare (or a link when it lives in another document). The guard is delimited by
  the word `when` — **never `[...]`**, which collides with markdown link syntax.
- **object flow** — a transition may carry a typed object with `carries <link>`:
  `- transitions to Ship carries [Order](./order.md)`. (`carries`, not `as` —
  `as` is reserved for names, e.g. lifeline aliases.)
- **state internals** — `- entry: <effect>`, `- do: <effect>`,
  `- exit: <effect>`; each effect is a backticked expression, and the colon
  reuses the "detail follows" idiom (attribute type, message signature).
- **composite / call behavior** — `- refines [SubFlow](./sub.md)`. A composite
  state or structured/call-behavior action never inlines its interior; it always
  points at a **separate** flow document via `refines` (a submachine state or
  call-behavior action). This keeps every flow document one flat graph.
- **`#### Notes`** — free-text notes on the node.

A transition **target** follows the general reference rule: a bare label for a
local vertex (the editor resolves it to the matching `###` heading and makes it
navigable), or a link when the target is a real element in another document.

```markdown
---
type: uml.StateMachine
title: Order Lifecycle
describes: [Order](./order.md)
---
# Order Lifecycle

## Nodes

### initial
- transitions to Draft

### Draft
- on `place` when `items > 0` transitions to Placed
- on `cancel` transitions to Cancelled

#### Notes
- Auto-expires after 24h.

### Placed
- entry: `reserveStock`
- on `ship` transitions to Shipped: `notify`

### Shipped
- on `deliver` transitions to final

### Cancelled
- transitions to final

### final
```

The same document with `type: uml.Activity` reads the plain headings as
actions, uses `decision` nodes for branches, and renders an activity-final
shape — identical grammar. Activity edges lean on guards rather than events, and
`else` marks a decision's default branch:

```markdown
### decision Ready to ship?
- when `paid and inStock` transitions to Ship
- else transitions to Hold

### Ship
- transitions to Deliver carries [Order](./order.md)
```

(`transitions` is used for both flavors even though UML names activity edges
"flows" and state-machine edges "transitions" — the flow substrate is one
grammar, so it gets one verb.)

**Swimlanes / regions** — optional `partition: <name>` on a node; the renderer
draws activity swimlanes or state-machine orthogonal regions. Detail beyond
this is deferred.

## Substrate 3 — interaction (sequence)

`type: uml.Sequence` + optional `describes:`. Two sections.

### `## Lifelines`

Participants. Each *is* a real Class or Actor, so each is a **link**. An
optional `as <alias>` gives a short handle for terse message lines.

```markdown
## Lifelines
- [Customer](./customer.md)
- [Order](./order.md) as order
- [Warehouse](./warehouse.md) as wh
```

### `## Messages`

Ordered — **document order is time order**, which is the whole reason
interaction is its own substrate. Each message is a binary
`Sender <verb> Receiver`, reusing the relationship-verb idiom: the **verb is
the message kind** and fixes the line and arrowhead.

| verb | UML message | renders |
|---|---|---|
| `calls` | synchronous | solid line, filled ▶ |
| `sends` | asynchronous | solid line, open → |
| `replies` | reply / return | dashed line, open → |
| `creates` | create | dashed → to new lifeline head |
| `destroys` | delete | → ending in ✕ |

Sender and Receiver are lifeline aliases or links (bare references); the
**signature** is an expression and is backticked after the `:`.

**Combined fragments** (`alt`, `opt`, `loop`, `par`) are keyword bullets that
**own operands**. Each operand is a header bullet — `when <guard>` (a guarded
operand) or `else` (an `alt`'s default) — and the operand's messages nest under
*it*. `else` is therefore a proper operand sibling of `when`, not a bare
separator floating among the messages. `opt` / `loop` take a single guarded
operand; `par`'s concurrent operands defer to the advanced-sequence open
question.

```markdown
## Messages
- Customer calls order: `place(items)`
- alt
  - when `paid`
    - order calls wh: `ship()`
  - else
    - order sends Customer: `paymentFailed()`
- order replies Customer: `confirmation`
```

Execution/activation bars derive automatically from `calls`/`replies` nesting —
no syntax. Self, found, and lost messages are deferred.

## Coverage summary

| family | substrate | key constructs |
|---|---|---|
| class | structure | existing seven metaclasses |
| use case | structure | `uml.Actor`, `uml.UseCase`; `includes`/`extends`; `frame` = system boundary |
| activity | flow | heading nodes + `transitions to` edges; `when` guards; `decision`/`merge`, `fork`/`join`; `object` nodes + object flows (`carries` link) |
| state machine | flow | same grammar; `on`/`when`/`transitions to`/`: effect`; `entry`/`do`/`exit`; `describes` a Class |
| sequence | interaction | `## Lifelines` links + ordered `## Messages` verbs + operand-owning fragments |

Four families, three grammars. Every construct is a recombination of grammar
UAML already has: frontmatter `type` dispatch, bare-name lists, verb/keyword-led
bullets, the `[fronted clauses] <verb> <target> [: detail]` sentence, links as
references, `when`/`as`/`on`/`to` word-clauses, backticked expressions, and
graceful degradation for anything unrecognized.

## Design principles carried over unchanged

- **Closed metaclasses, open stereotypes.** Every new element kind
  (`uml.Actor`, `uml.UseCase`, flow node keywords, message verbs) is a closed,
  renderer-known set. Domain meaning stays in stereotypes.
- **Graceful degradation.** Unknown substrate, family, metaclass, node keyword,
  message verb, or section degrades to a generic render; nothing is dropped;
  serialization stays lossless.
- **Relational, never coordinates.** The flow and interaction substrates are
  self-rendering; a solver lays them out at render time. No coordinates stored.
- **Links are the connective tissue.** Cross-tier and cross-document references
  (`describes`, `refines`, lifelines) are markdown links, like structural
  relationships.

## Open questions (deferred, not blocking)

- **Surface grammar polish.** The behavioral sentence shape, three markers,
  clause order, and colon-effect are now settled (this revision). Remaining
  candidates: the message-verb set and whether edges take an optional `as
  <name>` label are open before BNF is frozen.
- **Swimlanes / partitions.** Only sketched (`partition:` field).
- **Object flow detail.** Input/output **pins** on actions, and rendering an
  object flow when it appears on a *class* diagram (the same directed,
  nameable, no-roles/no-multiplicity edge primitive, surfaced as a stereotyped
  edge). Only object nodes + `carries`-link object-flow edges are specified now.
- **`par` operands.** How concurrent operands of a `par` fragment are separated
  (the `alt`/`else` operand model covers the guarded fragments).
- **Advanced sequence features.** Self/found/lost messages, gates, coregions.
- **State machine details.** History pseudostates, deferred events,
  entry/exit points on composite states, and cross-boundary transitions
  (targeting a specific substate inside a `refines`d submachine from outside).
- **BNF.** Parser-grade grammar and context rules for all three substrates,
  to be written in the implementation plan and folded into `uaml-spec.md`.
