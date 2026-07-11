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

`associates` and `specializes` are existing verbs, reused. Actor and use-case
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
selects the flavor; both parse identically. Optional frontmatter `describes:`
links the behavior to the entity it belongs to (a state machine describes a
Class; an activity realizes a use case or operation).

### Nodes

Each node is a `###` heading, making it a mini-classifier that can own its own
sub-sections (transitions, internals, `#### Notes`). The heading text is the
node's label and local identity.

Node kind is a **closed set**, marked by a leading keyword on the heading;
absent keyword means a plain action (activity) or state (state machine).
Control nodes come in matched split/join pairs:

- `initial`, `final`
- `decision` (1→N, guarded branch) / `merge` (N→1, rejoin) — both render as a diamond ◇
- `fork` (1→N, concurrent split) / `join` (N→1, concurrent sync) — both render as a bar
- `object` — an object/data node, typed by a link
  (`### object [Order](./order.md)`); renders as a rectangle. Edges touching it
  are **object flows** (see below)
- (no keyword) → action / state

Under a node heading:

- **transitions** — bullets leading with `to` (outgoing edges; source is the
  enclosing node, implicit, mirroring how `## Relationships` attach to their
  declaring classifier). Grammar:
  `- to <target> [as <name>] [on <trigger>] [when <guard>] [/ <effect>]`.
  `as`/`on`/`when`/`/` clauses are each optional; the guard is delimited by the
  word `when` — **never `[...]`**, which collides with markdown link syntax.
  `as <name>` is an optional edge label (reusing the association-name idiom);
  when the name is a **link**, the edge carries that object type — an **object
  flow** (`- to Ship as [Order](./order.md)`).
- **state internals** — `- entry / <effect>`, `- do / <effect>`,
  `- exit / <effect>`.
- **composite / call behavior** — `- refines [SubFlow](./sub.md)`. A composite
  state or structured/call-behavior action never inlines its interior; it always
  points at a **separate** flow document via `refines` (a submachine state or
  call-behavior action). This keeps every flow document one flat graph.
- **`#### Notes`** — free-text notes on the node.

A transition **target** follows the same rule as node references generally:
a bare label for a local vertex, or a link when the target is a real element.

```markdown
---
type: uml.StateMachine
title: Order Lifecycle
describes: [Order](./order.md)
---
# Order Lifecycle

## Nodes

### initial
- to Draft

### Draft
- to Placed on place when items > 0
- to Cancelled on cancel

#### Notes
- Auto-expires after 24h.

### Placed
- entry / reserveStock
- to Shipped on ship / notify

### Shipped
- to final on deliver

### Cancelled
- to final

### final
```

The same document with `type: uml.Activity` reads the plain headings as
actions, uses `decision` nodes for branches, and renders an activity-final
shape — identical grammar.

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

Sender and Receiver are lifeline aliases or links; the signature trails after
`:`.

**Combined fragments** (`alt`, `opt`, `loop`, `par`) are keyword bullets with a
`when <guard>` clause; their messages nest underneath; `else` splits `alt`
operands — mirroring the nested-operand style of the layout language.

```markdown
## Messages
- Customer calls order: place(items)
- alt when paid
  - order calls wh: ship()
  - else
  - order sends Customer: paymentFailed()
- order replies Customer: confirmation
```

Execution/activation bars derive automatically from `calls`/`replies` nesting —
no syntax. Self, found, and lost messages are deferred.

## Coverage summary

| family | substrate | key constructs |
|---|---|---|
| class | structure | existing seven metaclasses |
| use case | structure | `uml.Actor`, `uml.UseCase`; `includes`/`extends`; `frame` = system boundary |
| activity | flow | heading nodes + `to` transitions; `when` guards; `decision`/`merge`, `fork`/`join`; `object` nodes + object flows (`as` link) |
| state machine | flow | same grammar; `entry`/`do`/`exit`; `describes` a Class |
| sequence | interaction | `## Lifelines` links + ordered `## Messages` verbs + fragments |

Four families, three grammars. Every construct is a recombination of grammar
UAML already has: frontmatter `type` dispatch, bare-name lists, verb/keyword-led
bullets, binary `X <op> Y`, links as references, `when`/`as`/`with`/`to`
word-clauses, and graceful degradation for anything unrecognized.

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

- **Surface grammar polish.** The concrete syntax above is a first pass. Message
  verbs, transition clause order, and the entry/do/exit bullet-vs-bare-line
  choice are candidates for refinement before BNF is frozen.
- **Swimlanes / partitions.** Only sketched (`partition:` field).
- **Object flow detail.** Input/output **pins** on actions, and rendering an
  object flow when it appears on a *class* diagram (the same directed,
  nameable, no-roles/no-multiplicity edge primitive, surfaced as a stereotyped
  edge). Only object nodes + `as`-link object-flow edges are specified now.
- **Advanced sequence features.** Self/found/lost messages, gates, coregions.
- **State machine details.** History pseudostates, deferred events,
  entry/exit points on composite states, and cross-boundary transitions
  (targeting a specific substate inside a `refines`d submachine from outside).
- **BNF.** Parser-grade grammar and context rules for all three substrates,
  to be written in the implementation plan and folded into `uaml-spec.md`.
