# WAML — UML domain models in OKF

WAML (formerly OKF-UML) is a profile of OKF (an open, markdown-based modeling
format) for representing UML models as a set of linked markdown documents. Each
classifier (class, interface, enum, …) is its own document; relationships
between classifiers are expressed as markdown links inside a
`## Relationships` section; a *diagram* document curates a subset of those
classifiers into a rendered view; and *behavior* documents (activity, state
machine, sequence) are self-rendering views over their own declared elements.
Everything the renderer must dispatch on is carried by a small closed set of
**metaclasses**, while all domain-specific vocabulary is carried as open
**data** (stereotypes and profiles).

This document specifies the on-disk format precisely enough to implement a
parser, a serializer, or to author WAML documents by hand. It describes the
format only — not any particular application, storage, or rendering technology.

A guiding principle throughout is **graceful degradation**: an unknown family,
metaclass, or section is passed through and rendered generically, never treated
as an error (see [Graceful degradation](#graceful-degradation)).

## Document roles

Every WAML document plays exactly one of four roles. The role is determined
structurally, not by a free-text label:

| role | how identified | is it a pool element? |
|---|---|---|
| **index** | filename is `index.md` | no — navigation only, generated (see [Packages and indexes](#packages-and-indexes)) |
| **diagram** | `type: Diagram` **and** a `## Members` list | no — it is a *view* over pooled nodes |
| **behavior** | `type: uml.Activity`, `uml.StateMachine`, or `uml.Sequence` | no — a self-rendering view (see [Behavioral substrates](#behavioral-substrates)) |
| **node** | anything else | yes |

- **Index** documents provide navigation and are not part of any model graph.
  They are regenerated from the bundle, one per directory.
- **Diagram** documents are curated, profiled views over a set of nodes (see
  [Diagram documents](#diagram-documents)).
- **Behavior** documents each hold one directed graph (flow substrate) or one
  ordered interaction (interaction substrate). They are the document — model
  and view at once — and never contribute a classifier node.
- **Node** documents describe a single modeling element — a class, interface,
  enum, data type, package, association class, actor, use case, instance
  specification, or note.

Within the pool, **classifier** is a narrower predicate than **node**: `Class`,
`Interface`, `Enum`, `DataType`, `Actor`, `UseCase`, `Association`, and the
behavior kinds are UML Classifiers; `Package`, `Note`, and
`InstanceSpecification` are pool elements but *not* classifiers (an instance is
never instantiated, a note is a comment, a package is a namespace).

## The `type` dispatch key: `family.Metaclass`

A node document's `type` frontmatter field is a structured dispatch key of the
form `family.Metaclass`:

- **family** (`uml`, and — outside this profile — potentially `erd`, `bpmn`,
  `c4`, …) selects the rendering family and palette.
- **Metaclass** is a member of that family's **closed** metaclass set.

For the `uml` family the metaclass set is closed and fixed. Each metaclass has a
defined rendering; a conforming renderer has explicit handling per entry:

| `type` | renders as |
|---|---|
| `uml.Class` | 3-compartment box (name / attributes / operations); `abstract: true` → italic name |
| `uml.Interface` | box with the `«interface»` keyword |
| `uml.Enum` | box with `«enumeration»` and a literal list |
| `uml.DataType` | box with the `«dataType»` keyword |
| `uml.Package` | tabbed-folder box |
| `uml.Association` | association class — a classifier box (name / attributes) dashed-connected to an association line |
| `uml.Actor` | stick figure (use-case structure tier) |
| `uml.UseCase` | ellipse (use-case structure tier) |
| `uml.InstanceSpecification` | object box — underlined `name : Classifier` header over a slot list |
| `uml.Note` | dog-eared comment box; markdown body; dashed anchor(s) to the annotated element(s) |

Three further `uml.*` tokens select the behavior substrates rather than node
metaclasses: `uml.Activity`, `uml.StateMachine`, and `uml.Sequence` (see
[Behavioral substrates](#behavioral-substrates)). A diagram's dispatch key is
the bare token `Diagram` (no family prefix).

The metaclass set is *closed*: authors do not invent new metaclasses. All
domain-specific meaning is expressed through **stereotypes** instead.

> Operations/methods on classifiers are out of scope for this profile. The
> three-compartment `uml.Class` box leaves room for an operations compartment,
> but no `## Operations` section is defined here.

## Metaclasses vs stereotypes

WAML uses UML's own extension mechanism to stay open without growing the
renderer:

- **Metaclasses** are the closed set above. The renderer knows each one.
- **Stereotypes** are an **open** set — pure data, no dedicated rendering code.
  Examples: `entity`, `valueObject`, `aggregateRoot`, `repository`, `service`,
  `domainEvent`, `controller`. Authors may invent any stereotype name.

A stereotype renders as a `«guillemet»` keyword label above the element name,
plus optional styling supplied by the active [profile](#profiles). A node may
carry **multiple** stereotypes (UML permits it); `stereotype` in frontmatter is
therefore a scalar or a list.

Adding a new domain term (e.g. `«saga»`) requires no format or renderer change —
it is one new stereotype name, optionally given styling in a profile.

## Profiles

A **profile** (for example `uml-domain`) is a named bundle of presentation data
that a diagram selects via its `profile` frontmatter key. It does not change
what a model *means*; it selects what is *emphasized* and how stereotyped
elements look. A profile does three jobs:

1. **Render lens / emphasis** — which adornments to surface. A `uml-domain`
   profile might show multiplicity, aggregation/composition diamonds,
   generalization, and realization while hiding operations and visibility. A
   different profile could instead surface operations and visibility.
2. **Stereotype → style map** — maps stereotype names to visual styles (header
   color, border weight, shape, …).
3. **Palette** — which metaclasses and stereotypes an authoring UI offers.

A profile is data. An illustrative shape:

```yaml
# uml-domain profile (illustrative)
emphasize: [multiplicity, aggregation/composition diamonds, generalization, realization]
hide: [operations, visibility]
stereotypes:
  aggregateRoot: { header: gold, border: thick }
  valueObject:   { header: slate }
  domainEvent:   { shape: hexagon }
palette:
  metaclasses: [uml.Class, uml.Interface, uml.Enum, uml.DataType, uml.Association, uml.Note]
  stereotypes: [entity, valueObject, aggregateRoot, service, domainEvent]
```

The same node documents may be drawn by different profiles, yielding different
emphasis. "What matters here" is a property of the **diagram/profile, never of
the node**. A diagram may additionally override individual display switches in
its own frontmatter (see [Display settings](#display-settings)).

## Node (classifier) documents

A classifier document carries YAML frontmatter and a set of markdown sections. A
representative `uml.Class`:

```markdown
---
type: uml.Class
stereotype: [aggregateRoot, entity]   # scalar or list; optional
abstract: false                       # optional flag, any metaclass
title: Order
description: A customer's placed order.
---
# Order

## Attributes
- id: OrderId {1}
- placedAt: Timestamp {1}
- status: [OrderStatus](./order-status.md) {1}
- shippingAddress: [Address](./address.md) {0..1}
- total: [Money](./money.md) {1}

## Relationships
- associates [Customer](./customer.md) as "places": 1 order to 1 customer
- composes [OrderLine](./order-line.md): 1 order to 1..* lines
- depends [PricingService](./pricing-service.md)
```

### Frontmatter

- `type` — the `family.Metaclass` dispatch key (required for known-family nodes;
  an opaque string is tolerated and rendered generically).
- `title` — display name. The **slug** (lowercase, spaces → hyphens) is both the
  filename (`order.md`) and the link target other documents use.
- `stereotype` — optional; a scalar or a list of stereotype names.
- `abstract` — optional boolean flag; renders the name italic for `uml.Class`.
- `description` — optional one-line description.

Generic OKF frontmatter (`tags`, `resource`, `timestamp`, and any keys this
profile does not read) is preserved losslessly on every document.

### Canonical sections

The recognised section headings are `Body`, `Attributes`, `Slots`, `Values`,
`Relationships`, `Notes`, `Nodes`, `Lifelines`, `Gates`, `Messages`, `Members`,
and `Layout`. Headings are matched case-insensitively and normalised to these
canonical titles by the formatter. Which sections are meaningful depends on the
document's role; an unrecognised section is carried through untouched (see
[Graceful degradation](#graceful-degradation)).

### `## Attributes`

One bullet per attribute, following the grammar:

`- [visibility ]name: Type {multiplicity}`

- **name** — the attribute name.
- **Type** — either a bare token (a primitive or otherwise unmodeled type, e.g.
  `String`, `OrderId`, `Timestamp`) **or** a markdown link to another classifier
  document (e.g. `[Money](./money.md)`). A linked type is navigable; a bare token
  is plain text.
- **multiplicity** — optional trailing `{…}` using full UML multiplicity strings
  (`1`, `0..1`, `*`, `1..*`, `0..*`, `2..5`). Absent multiplicity is treated as
  `{1}`. The braces avoid colliding with Markdown's `[…]` link/reference syntax;
  relationship-end multiplicities stay bare (see `## Relationships`). A parser
  tolerates `[…]` in this position, but `{…}` is the canonical serialization.
- **visibility** — optional leading `+`, `-`, `#`, or `~`
  (public / private / protected / package). Permitted but omittable; a
  domain-oriented profile typically hides it.

### `## Values` (enums only)

`uml.Enum` uses a name-only literal list under `## Values` instead of
`## Attributes`:

```markdown
---
type: uml.Enum
title: OrderStatus
---
# OrderStatus

## Values
- DRAFT
- PLACED
- SHIPPED
- CANCELLED
```

## Relationships

A classifier's `## Relationships` section lists one bullet per relationship. The
bullet's **verb** fixes the relationship category (and thus the line style); the
target is a markdown link; an optional `as …` clause names the relationship; and
an optional `: near to far` clause declares the ends.

Relationships are conceptual associations. There are no join keys, foreign keys,
or other data-persistence concerns in this profile — those are out of scope.

### Taxonomy → line style

UML has three relationship categories. The verb's category fixes the **line**;
the verb itself adds the **end adornment**:

| category | line | verbs | end adornment |
|---|---|---|---|
| **association** | solid | `associates`, `aggregates`, `composes`, `links` | none / hollow ◇ (aggregation) / filled ◆ (composition) |
| **dependency** | dashed | `depends`, `implements` (realization), `includes`, `extends`, `instance of` | open → / hollow ▷ (realization) / open → + `«include»`/`«extend»`/`«instanceOf»` label |
| **generalization** | solid | `specializes` | hollow ▷ |

These nest: composition is a stronger aggregation, and aggregation a stronger
association (UML `AggregationKind`: none → shared → composite). All of
`associates` / `aggregates` / `composes` are therefore associations (solid line),
differing only by end adornment. Likewise realization is a kind of dependency
(dashed line). The line derives from the category and the arrowhead/diamond from
the verb, so new dependency kinds added later need no new line logic.

### Verbs

| verb | UML meaning | ends? | renders as |
|---|---|---|---|
| `associates` | association | optional | solid line, arrowhead on navigable end(s) |
| `aggregates` | shared aggregation | required | solid line, hollow ◇ on this (whole) end |
| `composes` | composition | required | solid line, filled ◆ on this (whole) end |
| `specializes` | generalization | no | solid line, hollow ▷ at parent |
| `implements` | realization | no | dashed line, hollow ▷ at interface |
| `depends` | dependency | no | dashed line, open → at target |
| `includes` | use-case include | no | dashed line, open → + `«include»` label |
| `extends` | use-case extend | no | dashed line, open → + `«extend»` label |
| `instance of` | instance ↔ classifier typing | no | dashed line, open → at classifier (see [Instances](#instances-and-object-diagrams-umlinstancespecification)) |
| `links` | link — an instance of an association | no | solid line between two instances |
| `annotates` | comment anchor | no | plain dashed line, no arrowhead (`uml.Note` only) |

`specializes` reads near → far as child → parent (the child document declares its
parent). `annotates` is valid only in a `uml.Note`'s `## Relationships` (see
[Notes](#notes--comments-umlnote)); `instance of` and `links` are valid only on a
`uml.InstanceSpecification`.

### Ends

For `aggregates` / `composes` (required) and `associates` (optional), the ends
clause is `: <near> to <far>`, where each end is `<multiplicity>[ <role>]`.
**near** is the declaring document; **far** is the target.

Example: `- composes [OrderLine](./order-line.md): 1 order to 1..* lines`
means one `Order` (near, role `order`) composes one-or-more `OrderLine`s (far,
role `lines`).

A bare `associates` (no ends clause) is a plain communication/association link
with unstated ends — the idiomatic form between an `uml.Actor` and a
`uml.UseCase`, and permitted between any two classifiers.

### Navigability and reciprocity

A single relationship line means "this (near) end can reach the far end" — one
arrowhead at the far end. **Both-navigable** requires **both** documents to
declare the reverse line; a parser merges the reciprocal pair into a single edge
with arrowheads on both declared ends. Aggregation and composition are inherently
directed (the diamond is fixed on the whole/near end), so they need no reciprocal
declaration.

### Association names (`as …`)

Any relationship may carry an optional `as …` clause after the target link and
before the `:` ends clause. This is the UML *association name* — a reading-label
on the line, distinct from the leading verb (which fixes line style) and from the
per-end roles. It is rendered near the line's midpoint, with no reading-direction
arrow, and is allowed on **all** verbs. It takes one of two forms:

- **String** — `as "places"`: a plain label. It also gives the relationship an
  **identity**, referenceable as **(source document, name)** by `uml.Note`
  anchors.
- **Link** — `as [Places](./places.md)`: the name links to a top-level
  `uml.Association` classifier that carries its own `## Attributes` — i.e. an
  **association class** (see below). The inline bullet still declares the ends and
  keeps the direct link to the far classifier, so class-to-class navigation is
  preserved; the association class is reached *via the `as` link*, not by routing
  the relationship through an intermediate document.

On a `links` relationship, the link form of `as` names the **Association the
link instantiates** rather than an association class.

### BNF

```bnf
<relationship>  ::= "- " <verb> " " <link> <name>? <ends>?

<verb>          ::= "associates" | "aggregates" | "composes"
                  | "specializes" | "implements" | "depends"
                  | "includes" | "extends"
                  | "instance of" | "links"
                  | "annotates"          ; uml.Note only

<link>          ::= "[" <title> "](./" <slug> ".md)"

<name>          ::= " as " ( <quoted> | <link> )   ; UML association name
<quoted>        ::= "\"" <text> "\""             ; plain label; text free-form (no unescaped ")
                                                 ; <link> form → target is a uml.Association (association class)

<ends>          ::= ": " <end> " to " <end>
<end>           ::= <multiplicity> | <multiplicity> " " <role>

<multiplicity>  ::= <bound> | <lower> ".." <bound>
<lower>         ::= "0" | <posint>
<bound>         ::= <posint> | "*"
<posint>        ::= <digit-1-9> <digit>*

<role>          ::= <ident>            ; /[A-Za-z][A-Za-z0-9_]*/
<slug>          ::= <kebab>            ; lowercase, hyphen-separated
<title>         ::= target's display title
```

### Context rules (parser-enforced, not expressible in the BNF)

- `<ends>` is **required** for `aggregates` / `composes`, **optional** for
  `associates` (present with both ends, or absent entirely), and **forbidden**
  for every other verb.
- `<name>` (`as …`) is **optional** on every verb; when present it precedes
  `<ends>`. Names need not be globally unique, but a name referenced by a note
  should be unique within its source document so the anchor resolves
  unambiguously.
- End order is always **near** (the declaring document) `to` **far** (the
  target).
- Multiplicity: `*` is unbounded; bare `*` ≡ `0..*`; bare `n` ≡ exactly `n`; a
  bare `0` is invalid; in `lower..bound`, `lower ≤ bound` unless `bound` is `*`.
- `<role>` is optional per end; it is a single token following the multiplicity
  after one space.
- `annotates` is accepted only on `uml.Note` documents; `instance of` and
  `links` only on `uml.InstanceSpecification` documents.

## Instances and object diagrams (`uml.InstanceSpecification`)

An **instance specification** is UML's "object on an object diagram": a pool
element that is an *instance of* a classifier, carrying **slot** values rather
than declaring attributes. A **link** is the object-diagram counterpart of an
association — an edge connecting two instances, optionally naming the
Association it instantiates. An instance is a pool element but **not** a
classifier: you do not instantiate an instance.

### Standalone instance document

```markdown
---
type: uml.InstanceSpecification
title: order42
---
# order42

## Relationships
- instance of [Order](./order.md)
- links [order42-line](./order42-line.md) as [Places](./places.md)

## Slots
- id: "ORD-42"
- status: PLACED
```

- `instance of <link>` types the instance by a classifier. No ends, no `as`.
- `links <link>` connects this instance to another instance; the optional
  `as` clause (string or link) names the instantiated Association.
- `## Slots` lists one `- name: value` bullet per slot.

### Slot values

A slot **value** is one of:

- a **quoted string** — `"ORD-42"`;
- a **bare token** — an identifier or number (`PLACED`, `3`); a bare value must
  not contain a reserved word (`instance of`, `with`, `set to`, `and`, `as`,
  `links`) — quote it if it must;
- a **link** — `[order42-line](./order42-line.md)`: an instance-valued slot,
  navigable to the referenced element.

### Inline instance (in a diagram's `## Members`)

For sketching an object diagram without one file per object, a Diagram's
`## Members` list accepts an inline-instance bullet:

```markdown
## Members
- instance of [Order](./order.md) as order42 with id set to "ORD-42" and status set to PLACED
```

Grammar: `- instance of <link> as <name>[ with <slot> set to <value> { and <slot> set to <value> }]`.
The instance is promoted to a pool element keyed `{diagram}#{name}` and is
automatically a member of that diagram. Canonical serialization joins slots
with ` and ` (one canonical form), so parse → serialize is byte-identical.
Inlining is **instances-only** — classifiers are never declared inline.

### Conformance validation (warn-only)

Object diagrams are often sketched before their classifiers settle, so
conformance failures are **warnings**, never errors:

- **slot-unknown-attribute** — a slot name is not an attribute of the referenced
  classifier.
- **instance-of-non-classifier** — the `instance of` target is not a classifier
  (including another instance).
- **instance-of-unresolved** — the classifier reference is dangling.

## Behavioral substrates

Beyond the structure tier, WAML defines two self-rendering behavior substrates.
Full rationale and worked examples:
`docs/superpowers/specs/2026-07-11-uaml-behavioral-substrates-design.md`.

The structure tier participates through the `uml.Actor` and `uml.UseCase`
metaclasses and the `includes` / `extends` dependency verbs (all part of the
core sets above). A bare `associates` between an actor and a use case is a
communication link. A system boundary is a `frame` group in a Diagram's
`## Members` section — no new metaclass.

### Flow substrate (`uml.Activity`, `uml.StateMachine`)

One document is one directed graph — self-rendering, no `## Layout`. Optional
frontmatter `describes: [Title](./slug.md)` links the flow to the entity it
behaviorizes. `## Nodes` holds one `###` heading per vertex:

```bnf
flow-heading    ::= "###" SP node-kind? SP node-identity
node-kind       ::= "initial" | "final" | "decision" | "merge"
                   | "fork" | "join" | "object"
node-identity   ::= text                      ; heading text minus the keyword;
                                               ; an "object" node's identity is
                                               ; its link title
```

A heading with no keyword is a **plain** node — an action (activity flavor) or a
state (state-machine flavor). The flavor tunes rendering only; the grammar is
one. Each node owns zero or more bullets:

```bnf
flow-bullet     ::= transition | internal | refines | partition
transition      ::= "-" SP ("on" SP expr SP)? (("when" SP expr) | "else")? SP
                     "transitions to" SP target
                     (SP "carries" SP link)?
                     (":" SP expr)?
target          ::= local-name | link
internal        ::= "-" SP ("entry" | "do" | "exit") ":" SP expr
refines         ::= "-" SP "refines" SP link
partition       ::= "-" SP "partition:" SP text
expr            ::= "`" text "`"               ; opaque to the model
link            ::= "[" text "](" "./" slug ".md" ")"
```

`transitions` is the one edge verb for both flow flavors. Guards are
delimited by the word `when`, never `[...]`; `else` marks a decision's default
branch. A link target is a cross-document transition into another behavior. A
trailing `#### Notes` under a node is a plain bulleted list, same grammar as a
classifier's `## Notes`.

### Interaction substrate (`uml.Sequence`)

One document is one ordered interaction — self-rendering. Optional
`describes:` as above. `## Lifelines` declares participants, `## Gates`
declares local frame gates, and `## Messages` holds the ordered interaction.
Lifeline declaration order fixes the diagram columns. Message order is time
order, except that the operands of a `par` fragment execute concurrently.

```bnf
lifeline-line   ::= "-" SP link (SP "as" SP identifier)?
gate-line       ::= "-" SP identifier

sequence-item   ::= message | fragment | interaction-use
message         ::= call | return | signal | create | delete
call            ::= "-" SP endpoint SP "calls" SP endpoint
                     (SP "async")? (SP expr)?
                     (SP "as" SP identifier)?
return          ::= "-" SP endpoint SP "returns" (SP expr)?
                     (SP "to" SP endpoint)?
                     (SP "for" SP identifier)?
signal          ::= "-" SP endpoint SP "signals" SP endpoint (SP expr)?
create          ::= "-" SP endpoint SP "creates" SP identifier
                     (":" SP expr)?
delete          ::= "-" SP endpoint SP "destroys" SP identifier

endpoint        ::= identifier | "outside" | "@" identifier
                   | identifier "@" identifier

fragment        ::= "-" SP fragment-kind NEWLINE operand+
fragment-kind   ::= "alt" | "opt" | "loop" | "par" | "break"
                   | "critical" | "assert" | "neg"
operand         ::= "-" SP ("when" SP expr | "else"
                   | "branch" (SP expr)?) NEWLINE sequence-item*

interaction-use ::= "-" SP "ref" SP link SP "as" SP identifier
                     NEWLINE binding*
binding         ::= "-" SP "bind" SP identifier SP "to" SP identifier

expr            ::= "`" text "`"
identifier      ::= token other than "outside" without "@"
```

The message forms have these meanings:

| form | meaning | rendering |
|---|---|---|
| `calls` | synchronous call; `async` makes it asynchronous | solid line, filled arrow for sync and open arrow for async |
| `signals` | asynchronous signal | solid line, open arrow |
| `returns` | return from a call | dashed line, open arrow |
| `creates` | create a lifeline | dashed arrow to the new lifeline head |
| `destroys` | delete a lifeline | solid line that ends at an X |

`replies` and `sends` are recognised as *unsupported* verbs and rejected with a
diagnostic naming the supported forms — they never parse as messages.

An optional call identity follows `as`. A return can select that call with
`for`. Without `for`, the return must have exactly one eligible open call. The
solver derives activation bars from the selected call identity, not from a
lifeline stack.

An endpoint has one of four forms:

- A lifeline handle, such as `customer` (a lifeline's alias, else its title).
- `outside`, for a found or lost message. A message cannot have `outside` at
  both ends.
- A local gate, such as `@request`, declared in `## Gates`.
- A gate on an interaction use, such as `auth@request`.

The same lifeline at both ends creates self-message loopback geometry. This
rule also applies to a self-delete: the delete row ends that lifeline.

An interaction use references another `uml.Sequence` without copying its
messages. Its `bind` lines map local lifelines to target lifelines. A target
gate must exist and must have an inner connection. More than one outer message
can use the same gate. For example, a call and its return can share one gate:

```markdown
## Messages
- ref [Authorize](./authorize.md) as auth
- customer calls auth@request `authorize()` as authorization
- auth@request returns `accepted` to customer for authorization
```

Fragments own their indented operands. `when` has a guard, `else` is the
default `alt` operand, and `branch` identifies a `par` operand. All `par`
branches execute. Their rows can overlap, and their create and delete effects
join after all branches.

The parser preserves malformed declarations and recovers at the next sibling,
operand marker, section heading, or document end. Invalid identifiers also
stay in the declared syntax, but they do not enter runtime endpoint, gate,
interaction-use, or call-correlation pools.

## Association classes (`uml.Association`)

When an association itself needs attributes, name it with a **link** to a
`uml.Association` document rather than a bare string. The inline relationship
bullet keeps the ends and the direct link to the far classifier:

```markdown
# order.md — Relationships
- associates [Customer](./customer.md) as [Places](./places.md): 1 order to 1 customer
```

```markdown
---
type: uml.Association
title: Places
---
# Places

## Attributes
- placedAt: Timestamp {1}
- channel: [Channel](./channel.md) {1}
```

The ends live on the inline bullet, so `order.md` → `customer.md` remains a
direct link. The `uml.Association` document supplies only the association's
attributes and identity; it uses `## Attributes` like any classifier, may carry
stereotypes, and does **not** redeclare ends. It renders as a class box
dashed-connected to the association line, and it is annotated by notes like any
other classifier (by plain link).

## Notes / comments (`uml.Note`)

A `uml.Note` is UML's `Comment`: a dog-eared box carrying free text, attached by a
dashed anchor to one or more elements, with no semantic effect on the model.
There are two ways to author one.

### Standalone note document

A `uml.Note` is a metaclass node (not a classifier — it carries no attributes).
Its content is markdown under `## Body`, and it anchors its targets via an
`annotates` relationship:

```markdown
---
type: uml.Note
title: Domestic-only
---
# Domestic-only

## Body
Only valid for domestic customers; international goes through the broker flow.

## Relationships
- annotates [Order](./order.md)
- annotates [Order](./order.md) as "places"
```

`annotates` may target **any element except an attribute** (attributes are too
fine-grained to anchor):

- a **node** — any metaclass, via a plain link: `annotates [Order](./order.md)`,
  `annotates [OrderStatus](./order-status.md)` (enum), `annotates [Payments](./payments.md)`
  (package), even another `uml.Note`.
- an **association** — the source document's link **plus** the association name:
  `annotates [Order](./order.md) as "places"` means "the association named
  *places* declared on `order.md`". When the target association is unnamed, use
  the endpoint form instead:
  `annotates [Order](./order.md) associates [Customer](./customer.md)`
  (source + verb + target). Naming the association is preferred.

A single note may `annotate` several elements (multiple dashed anchors), and they
need not be the same kind. `annotates` is the only verb valid in a `uml.Note`'s
`## Relationships`. The anchor is a plain dashed line with **no arrowhead** (a UML
comment anchor, not a directed dependency).

### `## Notes` shorthand on a node

For the common case of a note pinned to a single class, a classifier may carry a
`## Notes` list. Each bullet **desugars** to a standalone `uml.Note` that
`annotates` the enclosing node — the same rendered result with less ceremony:

```markdown
## Notes
- Drafts expire after 24h.
- Total is derived from the order lines.
```

Every note is modeled internally as a `uml.Note` annotating something; the
shorthand is purely an authoring/serialization convenience. It must round-trip:
a note that anchors exactly its own enclosing node and nothing else serializes
back to a `## Notes` bullet.

## Diagram documents

A diagram is a curated, profiled **view** over nodes — not a classifier. It is
identified by `type: Diagram` together with a `## Members` list. It carries three
deliberately separate concerns: **membership** (`## Members`, optionally organised
into groups), **presentation lens** (`profile`, plus per-diagram display
switches), and **arrangement** (`## Layout`).

```markdown
---
type: Diagram
title: Orders Domain Model
profile: uml-domain
---
# Orders Domain Model

## Members

### Users
- [Customer](./customer.md)
- [Account](./account.md)

### Orders
- [Order](./order.md)
- [OrderLine](./order-line.md)
- [OrderStatus](./order-status.md)

## Layout
- Users as column with frame
- Users left of Orders
- top of Users aligned with top of Orders
- column of Order, OrderLine, OrderStatus with large margin
- [Money](./money.md) with collapsed
```

- **`## Members`** — the set of nodes drawn in this view (curated, reorderable),
  optionally organised into **groups** (see [Members and
  groups](#members-and-groups)).
- **`profile`** — selects the render lens, stereotype styles, and palette.
- **`## Layout`** (optional) — the arrangement statements. Positions are always
  expressed **relationally, never as coordinates** (see [The `## Layout`
  section](#the--layout-section)).

Arrangement of this kind always needs human judgement, but nobody should
hand-compute pixels: the author states *how things sit relative to one another* in
near-English, a deterministic solver produces the pixels at render time, and the
editor round-trips direct manipulation back into the same language. **No
coordinate is ever stored.**

### Display settings

A diagram's frontmatter may carry per-diagram display switches — a **partial**:
only keys present in the file take effect, everything else falls back to the
renderer's (profile-informed) defaults.

| key | value | meaning |
|---|---|---|
| `description` | string | one-line diagram description |
| `showAttributes` | bool | draw the attribute compartment |
| `showType` | bool | show attribute types (`attributeDetail: name-type` is an accepted legacy spelling) |
| `showAttributeVisibility` | bool | show `+`/`-`/`#`/`~` markers |
| `cardinality` | `off` \| `explicit` \| `all` | attribute-multiplicity policy: hide, show only authored values, or show all (supersedes the legacy boolean `showAttributeMultiplicity`) |
| `maxAttributes` | positive integer | truncate long attribute lists with a "more" footer |
| `showRoles` | bool | show relationship-end roles |
| `showCardinality` | bool | show relationship-end multiplicities |
| `showLabels` | bool | show association name labels |
| `showStereotype` | bool | show `«stereotype»` eyebrows |
| `stereotypeFilter` | list of names | allowlist of stereotypes to display (absent = show all; empty = show none) |
| `stereotypeColors` | list of `name:#rrggbb` | per-stereotype color overrides |

### Members and groups

`## Members` declares membership only. It may be a flat bullet list, or it may be
organised into **groups**:

- A **group** is a sub-heading under `## Members` with a member list beneath it.
  **Nesting is heading depth** — a deeper heading is a nested group. The heading
  **declares membership only**; it carries no visual treatment (treatment is a
  `## Layout` concern).
- A flat bullet list directly under `## Members`, with no group sub-headings, is a
  single **implicit top-level group**.
- Groups and elements are **operands of the same kind**: anything the layout
  language can say about an element it can also say about a group, referenced by
  the group's heading text.
- **By default a group's members clump** — the solver packs them compactly with
  no imposed axis or order. Member **list order carries no layout meaning** until
  the group is given an axis with an `as row` / `as column` clause (see [Treatment
  clauses](#treatment-clauses-as--with)).
- A member bullet is a link to a pooled node, or an [inline
  instance](#inline-instance-in-a-diagrams--members).

```markdown
## Members

### Users
- [Customer](./customer.md)
- [Account](./account.md)

#### VIP                       # nested group = deeper heading
- [Platinum](./platinum.md)

### Orders
- [Order](./order.md)
- [OrderLine](./order-line.md)
```

### The `## Layout` section

Positions are never stored as coordinates. The former `## Render hints` section
and its per-node saved coordinate (`[Order](./order.md) at 0,0`) are **removed**
from the format; the former per-node `emphasize` / `collapse` flags live here as
operand treatment (see [Treatment clauses](#treatment-clauses-as--with)).
Selecting *which adornments* a diagram surfaces remains a [profile](#profiles) /
[display-settings](#display-settings) concern.

Each bullet in `## Layout` is one statement — a **placement**, an **alignment**,
or a **standalone** treated operand. All arrangement is relative: the solver reads
the statements as constraints and produces the pixels.

#### Relations

Two families, both plain English.

**Placement** positions one operand on a side of another and is **chainable**:

```
- Users left of Orders
- Order above OrderLine above Payment
- Order above left of PaymentGateway
```

| direction | places left operand … |
|---|---|
| `left of` | to the left of the right operand |
| `right of` | to the right of the right operand |
| `above` | above the right operand |
| `below` | below the right operand |
| `above left of` | diagonally up-left of the right operand |
| `above right of` | diagonally up-right of the right operand |
| `below left of` | diagonally down-left of the right operand |
| `below right of` | diagonally down-right of the right operand |

Adjacency — **tight and aligned** — is the default, and is how rows and columns
are built. There is **no loose/far variant**; separation is controlled by margin
hints, not by the relation.

**Alignment** lines up an edge or a centre, independent of ordering, with a named
anchor on each side:

```
- top of VIP aligned with top of Orders
- center of X aligned with center of Y
- X aligned with Y                       # bare = center-to-center
```

The form is `[<edge> of] X aligned with [<edge> of] Y`, with `<edge>` one of
`top` / `bottom` / `left` / `right` / `center`. The anchor selects the axis it
constrains:

| anchor | constrains |
|---|---|
| `top` / `bottom` | the **Y** position |
| `left` / `right` | the **X** position |
| `center` | **both** (concentric) |

A bare `X aligned with Y` (no edges) is centre-to-centre. Placement (`left of`,
`above`, …) is the ergonomic path; anchor-alignment is the precise escape hatch
(e.g. `bottom of X aligned with top of Y` stacks X on Y explicitly).

#### Operands

An operand is any of:

- an **element** — a name or link, e.g. `Customer` or `[Customer](./customer.md)`;
- a **group** — its heading text, e.g. `Users`;
- an inline **`column of …`** — an anonymous ordered vertical stack (adjacency);
- an inline **`row of …`** — an anonymous ordered horizontal run (adjacency);
- a **parenthesized** operand — `( … )` for nesting and disambiguation.

Inline `row` / `column` are anonymous groups usable anywhere a name is, and they
nest:

```
- row of (column of Customer, Account), Orders
```

#### Treatment clauses (`as …` / `with …`)

An operand carries treatment through two optional clauses, in this order — an
**`as <axis>`** clause, then a **`with <hints>`** clause:

```
- Users as column with frame and large margin
```

A named group or element may be treated **by reference** on its own `## Layout`
line (a standalone statement, e.g. `Orders with frame`); an **anonymous** inline
group has no name, so it can only be treated **inline**.

**Axis** — `as row` / `as column`, groups only. Lays the group's members out in
**list order** along that axis. With no axis clause the members just clump (the
default). This is the only way to set the internal axis of a *named* heading
group, since its members are not restated inline: `Users as column` stacks
Customer over Account; `Users as row` flows them horizontally.

**`with` hints** are shape, margin, and flags, joined by `,` or `and`:

*Shape* (groups only) sets the group's keep-out geometry and whether it is drawn:

| shape | drawn? | reserves |
|---|---|---|
| `frame` | visible, titled box | a rectangle, drawn with the group's title |
| `box` | invisible | a square/rectangular bounding box (corner space wasted) |
| `shrink` *(default)* | invisible | the minimal concave hull hugging its members |

The **default is invisible `shrink`-wrap**: a group clusters its members without
drawing unless it opts into `frame` or `box`. Because `shrink` reserves the
minimal polygon, neighbouring groups tuck into its concave notches — the
compactness win; `box` reserves a full rectangle; `frame` reserves a rectangle and
draws it titled.

*Margin* (any operand) is qualitative breathing room around the operand — no
numbers: `no` / `small` / `medium` *(default)* / `large`, written `with large
margin` or `with no margin` (`margin` and `margins` both accepted). Shape and
margin are **orthogonal**: the old wide / thin / none idea is just `shrink` plus a
{large, small, no} margin, and splitting them lets margin apply to a `box` or a
bare element too.

*Flags* (any operand) are `emphasized` and `collapsed`. `collapsed` renders a node
as a reference chip rather than a full box; `emphasized` surfaces it.

**`with` binds greedily** to the nearest complete operand on its left. To attach a
`with` clause to a whole inline group rather than to its last member,
parenthesize:

```
- column of Customer, Account with large margin      # margin attaches to Account
- (column of Customer, Account) with large margin    # margin attaches to the column
```

The same rule governs a trailing relation: `(column of Customer, Account) left of
Orders` is unambiguous, whereas expressing "the large-margin column, left of
Orders" requires the parentheses.

#### BNF

Each `## Layout` bullet is one `<statement>`. Terminals are quoted; `{ … }` is
zero-or-more, `[ … ]` optional; `<link>`, `<quoted>`, and `<ident>` are as in the
[Relationships BNF](#bnf).

```bnf
<layout>        ::= { <statement> }
<statement>     ::= "- " ( <placement> | <alignment> | <standalone> )

<placement>     ::= <operand> " " <direction> " " <operand>
                    { " " <direction> " " <operand> }
<direction>     ::= "left of" | "right of" | "above" | "below"
                  | "above left of" | "above right of"
                  | "below left of" | "below right of"

<alignment>     ::= <anchored> " aligned with " <anchored>
<anchored>      ::= [ <edge> " of " ] <operand>
<edge>          ::= "top" | "bottom" | "left" | "right" | "center"

<standalone>    ::= <operand>          ; a lone operand — meaningful when it
                                       ; carries `with` hints: by-reference
                                       ; treatment of a named operand
                                       ; (`Orders with frame`), or treatment of
                                       ; an anonymous inline group

<operand>       ::= <ref> [ " as " <axis> ] [ " with " <hints> ]
<axis>          ::= "row" | "column"
<ref>           ::= <name> | <inline-group> | "(" <operand> ")"
<inline-group>  ::= ( "column" | "row" ) " of " <operand-list>
<operand-list>  ::= <operand> { ", " <operand> }

<hints>         ::= <hint> { ( ", " | " and " ) <hint> }
<hint>          ::= <shape> | <margin> | <flag>
<shape>         ::= "frame" | "box" | "shrink"
<margin>        ::= ( "no" | "small" | "medium" | "large" )
                    ( " margin" | " margins" )
<flag>          ::= "emphasized" | "collapsed"

<name>          ::= <ident> | <link> | <quoted>   ; element or group name
```

##### Context rules (parser-enforced, not expressible in the BNF)

- `as <axis>` is valid on **groups only** (a named heading group or an inline
  `row`/`column`); it orders members along the axis in **list order**. Absent, a
  group's members clump.
- `<shape>` (`frame` / `box` / `shrink`) applies to **groups only**; the default
  is invisible `shrink`.
- `<margin>` applies to **any** operand; the default is `medium`.
- `<flag>`s apply to any operand.
- Placement adjacency is always tight and aligned; qualitative separation is a
  `<margin>` concern, not a relation.
- The cardinal `above` / `below` take no trailing `of`; every other direction
  (including all four diagonals) ends in `of`.
- Anchor → axis: `top`/`bottom` → **Y**, `left`/`right` → **X**, `center` →
  **both**; a bare `<operand> aligned with <operand>` is centre-to-centre.
- A `with` clause binds to the **nearest complete operand on its left**;
  parenthesize to bind it to a whole inline group. The same rule disambiguates a
  trailing `<direction>`.

#### Worked example

```markdown
---
type: Diagram
title: Orders Domain Model
profile: uml-domain
---
# Orders Domain Model

## Members

### Users
- [Customer](./customer.md)
- [Account](./account.md)

### Orders
- [Order](./order.md)
- [OrderLine](./order-line.md)
- [OrderStatus](./order-status.md)

## Layout
- Users as column with frame
- Users left of Orders
- top of Users aligned with top of Orders
- column of Order, OrderLine, OrderStatus with large margin
- [Money](./money.md) with collapsed
```

Renders as a titled **Users** frame with Customer stacked over Account (`as
column` imposes the list-order stack) to the left of the **Orders** group; the two
groups' tops aligned; Orders' three members in a column with large margins; and
`Money` drawn as a reference chip. Without the `as column` clause Customer and
Account would simply clump inside the frame.

### Editing round-trip

The stored form is relations, and the UI editor is a relation generator, not a
coordinate store:

1. The user drags a node or group in the canvas.
2. On release the editor **infers** the relation(s) the new position implies —
   nearest neighbour plus side, or an alignment.
3. It **writes the sentence** into `## Layout`.
4. The solver **re-solves** and the node **snaps** into the solved position.

No coordinate is ever written. A human who never touches the text still produces
readable relations, and an LLM editing the text sees exactly what the human sees.

### External references

A member of a diagram may have relationships to nodes that are **not** in that
diagram's `## Members` (for example, a shared value object curated on another
diagram). Such off-diagram targets are not drawn as full members of the current
view. Instead, the other end of each such relationship is surfaced as a
**navigable external reference** — a link the reader can follow to a diagram that
does contain that node. This keeps each diagram a focused window while keeping
cross-document links discoverable and traversable.

## Packages and indexes

A bundle's directory tree **is** its package tree. Every directory holds an
`index.md` — a navigation document, generated and kept correct by tooling, never
part of the model graph. Its shape:

```markdown
# Sales

Sales bounded context.

* [orders](orders/)
* [Customer](./customer.md) - a buyer
```

- **No frontmatter.** The `#` heading is the package title (falling back to the
  directory basename); an optional intro paragraph is the description.
- One `*` bullet per member, in curated order: a sub-package links to its
  directory (`orders/`), a document links to its file (`./customer.md`), and a
  document's one-line description follows as a ` - ` blurb.
- Indexes are **regenerated** whenever the bundle changes (titles, blurbs, and
  membership are re-derived); a hand-edited custom title survives regeneration.

Independent of the directory tree, `uml.Package` remains a node metaclass for
*modeled* packages (a tabbed-folder box on a diagram) — a directory does not
have to be modeled, and a modeled package does not have to be a directory.

## Graceful degradation

Recognition failures never produce errors; they degrade to generic behavior:

- **Unknown family** (the part before the `.` in `type`) → the node renders as a
  generic labelled box (name plus attributes).
- **Unknown metaclass** within a known family → also a generic box.
- **Opaque / non-`family.Metaclass` `type`** → tolerated and rendered
  generically.
- **Unknown section** in a document → carried through and rendered generically,
  never dropped.
- **Malformed bullets** inside a known section are preserved in the syntax tree
  with a diagnostic; the parser recovers at the next bullet, heading, or
  document end.

Serialization is lossless: content a parser does not specifically understand is
preserved on round-trip rather than discarded.

## Conventions summary

- **Slug** — a classifier's slug is its `title` lowercased with spaces replaced
  by hyphens (kebab-case). The slug is the filename (`order.md`) and the link
  target used by other documents (`[Order](./order.md)`).
- **Title** — the human-readable display name, from frontmatter `title` and
  echoed as the document's `#` heading.
- **Multiplicity** — full UML strings (`1`, `0..1`, `*`, `1..*`, `0..*`,
  `2..5`); `*` is unbounded, bare `*` ≡ `0..*`, bare `0` invalid, and absent
  multiplicity on an attribute ≡ `{1}`.
- **Sections** — canonical headings are `Body`, `Attributes`, `Slots`, `Values`,
  `Relationships`, `Notes`, `Nodes`, `Lifelines`, `Gates`, `Messages`,
  `Members`, `Layout`; matching is case-insensitive and the formatter
  normalises to the canonical spelling.
- **Group** — a sub-heading under a diagram's `## Members`, declaring membership
  only; nesting is heading depth. A flat `## Members` list is one implicit
  top-level group. A group's members clump by default; list order carries no
  layout meaning until an `as row` / `as column` axis is set.
- **Layout** — positions are **relational, never coordinates**. A diagram's
  `## Layout` section holds placement (`left of` / `right of` / `above` /
  `below` plus the four diagonals), alignment (`[<edge> of] X aligned with
  [<edge> of] Y`), and per-operand treatment (`as` axis; `with` shape / margin /
  `emphasized` / `collapsed`). A solver produces the pixels at render time; none
  are stored.
- **Instances** — `uml.InstanceSpecification` + `## Slots` + `instance of` /
  `links`; conformance problems warn, never error.
- **Packages** — directories; each holds a generated, frontmatter-less
  `index.md` listing members with `*` bullets.
