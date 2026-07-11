# OKF-UML — UML domain models in OKF

OKF-UML is a profile of OKF (an open, markdown-based modeling format) for
representing UML class and structure diagrams as a set of linked markdown
documents. Each classifier (class, interface, enum, …) is its own document;
relationships between classifiers are expressed as markdown links inside a
`## Relationships` section; and a *diagram* document curates a subset of those
classifiers into a rendered view. Everything the renderer must dispatch on is
carried by a small closed set of **metaclasses**, while all domain-specific
vocabulary is carried as open **data** (stereotypes and profiles).

This document specifies the on-disk format precisely enough to implement a
parser, a serializer, or to author OKF-UML documents by hand. It describes the
format only — not any particular application, storage, or rendering technology.

A guiding principle throughout is **graceful degradation**: an unknown family,
metaclass, or section is passed through and rendered generically, never treated
as an error (see [Graceful degradation](#graceful-degradation)).

## Document roles

Every OKF document plays exactly one of three roles. The role is determined
structurally, not by a free-text label:

| role | how identified | is it a node? |
|---|---|---|
| **index** | filename is `index.md` | no — navigation only |
| **diagram** | `type: Diagram` **and** a `## Members` list | no — it is a *view* over nodes |
| **classifier (node)** | anything else | yes |

- **Index** documents provide navigation and are not part of any model graph.
- **Diagram** documents are curated, profiled views over a set of nodes (see
  [Diagram documents](#diagram-documents)).
- **Classifier** (node) documents describe a single modeling element — a class,
  interface, enum, data type, package, association class, or note.

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
| `uml.Note` | dog-eared comment box; markdown body; dashed anchor(s) to the annotated element(s) |

The metaclass set is *closed*: authors do not invent new metaclasses. All
domain-specific meaning is expressed through **stereotypes** instead.

> Operations/methods on classifiers are out of scope for this profile. The
> three-compartment `uml.Class` box leaves room for an operations compartment,
> but no `## Operations` section is defined here.

## Metaclasses vs stereotypes

OKF-UML uses UML's own extension mechanism to stay open without growing the
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
that a diagram selects. It does not change what a model *means*; it selects what
is *emphasized* and how stereotyped elements look. A profile does three jobs:

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
the node**.

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
- id: OrderId [1]
- placedAt: Timestamp [1]
- status: [OrderStatus](./order-status.md) [1]
- shippingAddress: [Address](./address.md) [0..1]
- total: [Money](./money.md) [1]

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

### `## Attributes`

One bullet per attribute, following the grammar:

`- [visibility ]name: Type [multiplicity]`

- **name** — the attribute name.
- **Type** — either a bare token (a primitive or otherwise unmodeled type, e.g.
  `String`, `OrderId`, `Timestamp`) **or** a markdown link to another classifier
  document (e.g. `[Money](./money.md)`). A linked type is navigable; a bare token
  is plain text.
- **multiplicity** — optional trailing `[…]` using full UML multiplicity strings
  (`1`, `0..1`, `*`, `1..*`, `0..*`, `2..5`). Absent multiplicity is treated as
  `[1]`.
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
| **association** | solid | `associates`, `aggregates`, `composes` | none / hollow ◇ (aggregation) / filled ◆ (composition) |
| **dependency** | dashed | `depends`, `implements` (realization) | open → / hollow ▷ (realization) |
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
| `associates` | association | yes | solid line, arrowhead on navigable end(s) |
| `aggregates` | shared aggregation | yes | solid line, hollow ◇ on this (whole) end |
| `composes` | composition | yes | solid line, filled ◆ on this (whole) end |
| `specializes` | generalization | no | solid line, hollow ▷ at parent |
| `implements` | realization | no | dashed line, hollow ▷ at interface |
| `depends` | dependency | no | dashed line, open → at target |

`specializes` reads near → far as child → parent (the child document declares its
parent).

### Ends

For `associates` / `aggregates` / `composes`, the ends clause is
`: <near> to <far>`, where each end is `<multiplicity>[ <role>]`. **near** is the
declaring document; **far** is the target.

Example: `- composes [OrderLine](./order-line.md): 1 order to 1..* lines`
means one `Order` (near, role `order`) composes one-or-more `OrderLine`s (far,
role `lines`).

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

### BNF

```bnf
<relationship>  ::= "- " <verb> " " <link> <name>? <ends>?

<verb>          ::= "associates" | "aggregates" | "composes"
                  | "specializes" | "implements" | "depends"

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

- `<ends>` is **required** for `associates` / `aggregates` / `composes`, and
  **forbidden** for `specializes` / `implements` / `depends`.
- `<name>` (`as …`) is **optional** on every verb; when present it precedes
  `<ends>`. Names need not be globally unique, but a name referenced by a note
  should be unique within its source document so the anchor resolves
  unambiguously.
- End order is always **near** (the declaring document) `to` **far** (the
  target).
- Multiplicity: `*` is unbounded; bare `*` ≡ `0..*`; bare `n` ≡ exactly `n`; in
  `lower..bound`, `lower ≤ bound` unless `bound` is `*`.
- `<role>` is optional per end; it is a single token following the multiplicity
  after one space.

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
- placedAt: Timestamp [1]
- channel: [Channel](./channel.md) [1]
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
identified by `type: Diagram` together with a `## Members` list.

```markdown
---
type: Diagram
title: Orders Domain Model
profile: uml-domain
---
# Orders Domain Model

## Members
- [Order](./order.md)
- [OrderLine](./order-line.md)
- [Customer](./customer.md)
- [OrderStatus](./order-status.md)

## Render hints
- emphasize: multiplicity, composition
- collapse [Money](./money.md)        # show as a reference chip, not a full box
- [Order](./order.md) at 0,0          # optional saved position
```

- **`## Members`** — the set of nodes drawn in this view (curated, reorderable).
- **`profile`** — selects the render lens, stereotype styles, and palette.
- **`## Render hints`** (optional) — per-diagram emphasis overrides, collapse
  flags, and saved positions.

### External references

A member of a diagram may have relationships to nodes that are **not** in that
diagram's `## Members` (for example, a shared value object curated on another
diagram). Such off-diagram targets are not drawn as full members of the current
view. Instead, the other end of each such relationship is surfaced as a
**navigable external reference** — a link the reader can follow to a diagram that
does contain that node. This keeps each diagram a focused window while keeping
cross-document links discoverable and traversable.

## Graceful degradation

Recognition failures never produce errors; they degrade to generic behavior:

- **Unknown family** (the part before the `.` in `type`) → the node renders as a
  generic labelled box (name plus attributes).
- **Unknown metaclass** within a known family → also a generic box.
- **Opaque / non-`family.Metaclass` `type`** → tolerated and rendered
  generically.
- **Unknown section** in a document → carried through and rendered generically,
  never dropped.

Serialization is lossless: content a parser does not specifically understand is
preserved on round-trip rather than discarded.

## Conventions summary

- **Slug** — a classifier's slug is its `title` lowercased with spaces replaced
  by hyphens (kebab-case). The slug is the filename (`order.md`) and the link
  target used by other documents (`[Order](./order.md)`).
- **Title** — the human-readable display name, from frontmatter `title` and
  echoed as the document's `#` heading.
- **Multiplicity** — full UML strings (`1`, `0..1`, `*`, `1..*`, `0..*`,
  `2..5`); `*` is unbounded, bare `*` ≡ `0..*`, and absent multiplicity on an
  attribute ≡ `[1]`.
