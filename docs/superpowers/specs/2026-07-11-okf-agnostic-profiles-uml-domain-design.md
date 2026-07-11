# OKF Canvas — agnostic modeling core + UML domain-model profile

**Date:** 2026-07-11
**Status:** Approved design + post-approval revisions (see Changelog), pre-implementation
**Builds on:** `2026-07-10-okf-canvas-static-spa-design.md` (static SPA pivot, `type` made opaque)

## Goal

Turn OKF Canvas from a data-mart/ERD tool into a **profile-agnostic modeling
canvas**. OKF stays an open markdown format; the canvas renders any node kind by
dispatching on a small closed set of **metaclasses**, with everything
domain-specific carried as **data** (stereotypes + profiles) rather than code.

The **first profile** is a **UML class-diagram / domain model** (DDD-flavored).
The design must make adding later profiles (ERD, BPMN, C4, …) a data exercise,
not a renderer rewrite.

## Non-goals

- Not implementing every UML diagram type. Scope = **class/structure** diagrams.
- No operations/methods modeling yet (deferred; see Deferred).
- No change to URL sharing (gzipped JSON `ModelGraph`) or localStorage plumbing.
- Not deleting the existing data-mart capability in this spec — but the node data
  model is generalized so the mart notion becomes one possible shape, not the core.

## Core concepts

### Three doc roles

A doc's role is structural, not a free-text `type`:

| role | how identified | is a canvas node? |
|---|---|---|
| **index** | filename `index.md` | no — navigation only (unchanged from prior spec) |
| **diagram** | `type: Diagram` + a `## Members` list | no — it's a *view* over nodes |
| **classifier (node)** | anything else | yes |

### `type` = `family.Metaclass` (structured dispatch key)

The prior spec made `type` opaque. This spec **supersedes that for the render
layer**: `type` becomes a structured dispatch key of the form `family.Metaclass`.

- **family** (`uml`, later `erd`, `bpmn`, `c4`) selects the renderer + palette.
- **Metaclass** is a member of that family's **closed** metaclass set.

**Graceful degradation is mandatory:** an unknown family or metaclass renders as a
generic labelled box (name + attributes). The canvas never errors on an
unrecognized `type`; agnosticism is preserved.

### Metaclasses vs stereotypes (the extensibility hinge)

We use UML's own extension mechanism. The renderer knows a handful of real
**metaclasses**; every domain vocabulary term is a **stereotype** — pure data.

**Core UML metaclasses (closed set — renderer has code per entry):**

| `type` | renders as |
|---|---|
| `uml.Class` | 3-compartment box (name / attributes / operations); `abstract: true` → italic name |
| `uml.Interface` | box with «interface» keyword |
| `uml.Enum` | box with «enumeration» + literal list |
| `uml.DataType` | box with «dataType» |
| `uml.Package` | tabbed-folder box |
| `uml.Association` | association class — classifier box (name / attributes) dashed-connected to an association line |
| `uml.Note` | dog-eared comment box; body markdown; dashed anchor(s) to annotated element(s) |

**Stereotypes (open set — data, no code):** `entity`, `valueObject`,
`aggregateRoot`, `repository`, `service`, `domainEvent`, `controller`, … invent
any. Rendered as a `«guillemet»` label above the name plus optional style. A node
may carry **multiple** stereotypes (UML allows it).

Where stereotype *styling* comes from: the **profile** (below). Adding «saga»
tomorrow = one profile line, no renderer change.

### Profiles

A **profile** (e.g. `uml-domain`) is named by a diagram and does three jobs:

1. **Render lens / emphasis** — which adornments to surface. `uml-domain` shows
   multiplicity, composition diamonds, generalization; hides operations. A future
   `uml-class` profile would show operations + visibility.
2. **Stereotype → style map** — `stereotype: aggregateRoot` → gold header, thick
   border, etc.
3. **Palette** — which metaclasses + stereotypes the "add node" UI offers.

```yaml
# uml-domain profile (illustrative)
emphasize: [multiplicity, aggregation/composition diamonds, generalization, realization]
hide: [operations, visibility]
stereotypes:
  aggregateRoot: { header: gold, border: thick }
  valueObject:   { header: slate }
  domainEvent:   { shape: hexagon }
palette:
  metaclasses: [uml.Class, uml.Interface, uml.Enum, uml.DataType]
  stereotypes: [entity, valueObject, aggregateRoot, service, domainEvent]
```

The same node docs can be drawn by different profiles → different emphasis. The
"what matters here" logic lives on the **diagram/profile, never on the node**.

## Node (classifier) document format

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

- `type` — `family.Metaclass` (required for known-family nodes; opaque string
  tolerated and rendered generically).
- `title` — display name; slug (lowercase, spaces→hyphens) = filename + link target.
- `stereotype` — optional; scalar or list of stereotype names.
- `abstract` — optional boolean flag (renders italic for `uml.Class`).
- `description` — optional one-liner.

### `## Attributes` (list form)

One bullet per attribute: `- [visibility ]name: Type [multiplicity]`

- **name** — attribute name.
- **Type** — a bare token (primitive / unmodeled, e.g. `String`, `OrderId`) **or**
  a markdown link `[Money](./money.md)` to another classifier doc. Linked type →
  navigable; bare → plain text.
- **multiplicity** — optional trailing `[..]`, full UML strings (`1`, `0..1`, `*`,
  `1..*`, `0..*`, `2..5`). Absent → treated as `[1]`.
- **visibility** — optional leading `+ - # ~` (public/private/protected/package).
  Allowed but omittable; `uml-domain` profile hides it by default.

### `## Values` (Enum only)

`uml.Enum` uses a name-only list instead of `## Attributes`:

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

### `## Relationships`

One bullet per relationship. Grammar below. **Join keys are gone** — associations
are conceptual, not ERD (keys belong to a future data profile, not this one).

**Optional name.** Any relationship may carry an `as ...` label after the link
(before the `:` ends clause). It's the UML *association name* — a reading-label on
the line, distinct from the leading verb (which fixes line style) and from the
per-end roles. Rendered near the line's midpoint. Allowed on **all** verbs; no
reading-direction arrow. The name takes one of two forms:

- **String** — `as "places"`: a plain label. Gives the relationship an
  **identity** — referenceable as **(source doc, name)** by `uml.Note` anchors.
- **Link** — `as [Places](./places.md)`: the name links to a top-level
  `uml.Association` classifier that carries its own `## Attributes` — i.e. an
  **association class**. The inline bullet still declares the ends and keeps the
  direct `[far]` link, so class-to-class navigation is preserved; the association
  class is reached *via the `as` link*, not by rerouting the association through a
  middle doc. Being a classifier, it's annotated by notes like any other node.

**Taxonomy (drives line style).** UML has three relationship categories; the
verb's category fixes the **line**, the verb itself adds the **end adornment**:

| category | line | verbs | end adornment |
|---|---|---|---|
| **association** | solid | `associates`, `aggregates`, `composes` | none / hollow ◇ (aggregation) / filled ◆ (composition) |
| **dependency** | dashed | `depends`, `implements` (realization) | open → / hollow ▷ (realization) |
| **generalization** | solid | `specializes` | hollow ▷ |

These nest: **composition is a stronger aggregation, aggregation a stronger
association** (UML `AggregationKind`: none → shared → composite) — so all of
`associates`/`aggregates`/`composes` are associations (solid line), differing
only by end adornment. Likewise realization is a kind of *dependency* (dashed).
The renderer derives the line from category and the arrowhead/diamond from the
verb — new dependency kinds (e.g. «use») later need no new line logic.

**Verbs:**

| verb | UML | ends? | renders as |
|---|---|---|---|
| `associates` | association | yes | solid line, arrowhead on navigable end(s) |
| `aggregates` | shared aggregation | yes | solid line, hollow ◇ on this (whole) end |
| `composes` | composition | yes | solid line, filled ◆ on this (whole) end |
| `specializes` | generalization | no | solid line, hollow ▷ at parent |
| `implements` | realization | no | dashed line, hollow ▷ at interface |
| `depends` | dependency | no | dashed line, open → at target |

**Ends** (associates/aggregates/composes): `: <near> to <far>`, each end
`<multiplicity>[ <role>]`, near = this doc, far = target.

**Navigability = reciprocity.** A single line means "this end can reach the far
end" (one arrowhead at far). Both-navigable = **both** docs declare the reverse
line; the renderer merges the reciprocal pair into one edge with arrowheads on
both declared ends. Aggregation/composition are inherently directed (diamond fixed
on this/whole end) — no reciprocity needed.

#### BNF

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

**Context rules (parser-enforced, not in BNF):**

- `<ends>` **required** for `associates|aggregates|composes`; **forbidden** for
  `specializes|implements|depends`.
- `<name>` (`as "..."`) **optional** on every verb; when present it precedes
  `<ends>`. Names need not be unique, but a name that's referenced by a note
  should be unique within its source doc so the anchor resolves unambiguously.
- End order: **near** (this doc) `to` **far** (target).
- `*` = unbounded; bare `*` ≡ `0..*`; bare `n` ≡ exactly `n`; in `lower..bound`,
  lower ≤ bound (unless bound `*`).
- `<role>` optional per end; single token; follows multiplicity after one space.

### Association classes (`uml.Association`)

When an association itself needs attributes, name it with a **link** to a
`uml.Association` doc rather than a bare string:

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

The ends live on the inline bullet (so `order.md` → `customer.md` stays a direct
link); `places.md` supplies only the association's attributes/identity. It renders
as a class box dashed-connected to the association line. A `uml.Association` doc
uses `## Attributes` like any classifier and may carry stereotypes; it does **not**
redeclare ends (those belong to the bullet). Notes anchor it by plain link.

## Notes / comments (`uml.Note`)

UML `Comment` — a dog-eared box carrying free text, attached by a dashed anchor to
one or more elements, with no semantic effect on the model. Two ways to author one:

### Standalone note document

A `uml.Note` is a metaclass node (not a classifier — carries no attributes/type
beyond its body). Its body is markdown; it anchors targets via an `annotates`
relationship:

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

`annotates` may target **any element except an attribute** — attributes are too
fine-grained to anchor. Concretely:

- a **node** — any metaclass, via a plain link: `annotates [Order](./order.md)`,
  `annotates [OrderStatus](./order-status.md)` (enum), `annotates [Payments](./payments.md)`
  (package), even another `uml.Note`.
- an **association** — the source doc's link **plus** the association name:
  `annotates [Order](./order.md) as "places"` = "the association named *places*
  declared on `order.md`". When the target association is unnamed, fall back to the
  endpoint form `annotates [Order](./order.md) associates [Customer](./customer.md)`
  (source + verb + target); naming the association is preferred.

A single note may `annotate` several elements (multiple dashed anchors), and they
need not be the same kind. `annotates` is the only verb valid in a `uml.Note`'s
`## Relationships`. The anchor is a plain dashed line with **no arrowhead** (a UML
comment anchor, not a directed dependency).

### `## Notes` shorthand on a node

For the common "a note pinned to this one class" case, a classifier may carry a
`## Notes` list. Each bullet **desugars** to a standalone `uml.Note` that
`annotates` the enclosing node — same rendered result, less ceremony:

```markdown
## Notes
- Drafts expire after 24h.
- Total is derived from the order lines.
```

Desugaring keeps one internal model (every note is a `uml.Note` annotating
something); the shorthand is purely an authoring/serialization convenience and must
round-trip back to `## Notes` for notes that anchor exactly their own node with no
other targets.

## Diagram document format

A diagram is a curated, profiled **view** over nodes — not a classifier.

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
- collapse [Money](./money.md)        # show as ref chip, not full box
- [Order](./order.md) at 0,0          # optional saved position
```

- **`## Members`** — the node set for this view (curated, reorderable).
- **`profile`** — selects the render lens/stereotype-styles/palette.
- **`## Render hints`** (optional) — per-diagram emphasis overrides, collapse
  flags, saved positions.

### External references (the "isolate a domain, still see other sources" behavior)

A member of a diagram may have relationships to nodes **not** in that diagram's
`## Members` (e.g. `Order` referenced by a `checkout-flow` UX diagram, or pointing
at a shared `Money` value object curated elsewhere). Behavior:

- On-canvas, external targets are not drawn as full members.
- The **inspector**, when a node is selected, lists its **external references** —
  incoming and outgoing relationships whose other end is off-diagram — as
  **navigable** chips. Clicking navigates to a diagram that contains that node.
- This keeps each diagram a focused window while making cross-source links
  discoverable and traversable.

## Data-model impact (`packages/okf`, `packages/web`)

The current `ModelNode` is data-mart shaped (`inputSource`, `schema:
SchemaField[]` with `pk`, `JoinKey`). Generalize:

- **Node** gains: `metaclass`/`type` (`family.Metaclass`), `stereotypes: string[]`,
  `abstract?: boolean`. `schema` generalizes to **attributes** (`name`, `type`
  token-or-ref, `multiplicity`, `visibility?`) and, for enums, **values**
  (`string[]`). PK/FK/`inputSource`/`definition` become **data-profile-only**
  concerns, not core fields.
- **Edge** generalizes from join to **relationship**: `kind` (the verb),
  optional `name` — either a string label or a ref to a `uml.Association` node
  (association class); the name is also the annotation handle — per-end
  `{ multiplicity, role, navigable }`, `bidirectional` derived from reciprocity.
  `keys` drop out of the UML profile (retained only if/when a data profile is
  reintroduced). A `uml.Association` node is a classifier (attributes, stereotypes)
  linked to its edge; it does not carry ends.
- **Note** is a node whose `metaclass` is `uml.Note`, holding a markdown `body`
  and a set of anchor targets (each a classifier key **or** an edge ref
  `{ sourceKey, name | verb+targetKey }`). The `## Notes` shorthand parses into
  the same shape (a note anchored to its enclosing node).
- **Diagram** becomes a first-class artifact: `{ profile, members: nodeKey[],
  hints }`. Today's single implicit graph = one default diagram.

Exact TypeScript shapes, migration of existing shares/templates, and how
`ModelGraph` holds multiple diagrams are **planning-stage** decisions — this spec
fixes the format and semantics, the implementation plan fixes the types.

### Parser / serializer

- `parse.ts` — add `## Attributes` (list), `## Values`, `## Relationships`
  (verb grammar + optional `as "..."` / `as [link]` name + ends + reciprocity
  merge), `## Body`
  (for `uml.Note`), and `## Notes` (shorthand → anchored `uml.Note`) parsing;
  resolve `annotates` targets to classifier keys or edge refs; keep
  frontmatter/title/slug handling. Drop the join-key requirement from relationships.
- `serialize.ts` — emit the new sections; round-trip must be lossless for the
  UML profile, including collapsing self-anchored notes back to `## Notes`.
- Unknown `type` / unknown section → carried through, rendered generically, never
  dropped.

### Rendering

- Metaclass renderers: a small registry keyed by `type` → React Flow node
  component. Unknown → generic box.
- Profile applies emphasis + stereotype styles + palette at the diagram level.
- Edge renderer maps verb → line style + end adornments (diamonds, triangles,
  arrowheads), reading per-end multiplicity/role and reciprocity, and draws the
  optional `name` at the line midpoint (or, for a `uml.Association` ref, a dashed
  connector from the line midpoint to the association-class box).
- `uml.Note` renders as a dog-eared box with a dashed anchor to each target
  (classifier box or edge midpoint); it participates in layout but has no
  attributes/operations compartments.

## Rendering dispatch summary

```
node.type ─split(".")→ (family, metaclass)
   known family?  ─no→  generic box
                  ─yes→ family renderer[metaclass]  (unknown metaclass → generic box)
   apply stereotype styles + emphasis from active diagram's profile
edge.kind ─→ line style + end adornments (per-end mult/role, reciprocity)
```

## Deferred (explicit YAGNI)

- **Operations/methods** on classifiers (would need a `## Operations` section and
  the `uml-class` profile). Format leaves room; not built now.
- **Non-UML families** (`erd`, `bpmn`, `c4`) — the point is they become possible
  as data/profile additions; none built now.
- **Data/ERD profile** (re-adding join keys, PK/FK, `inputSource`) as a distinct
  profile rather than the core.
- Default-value / derived / `{readOnly}` and other attribute adornments.

## Rollout (staged, `main` green each step)

1. Generalize the node/edge/diagram data model in `okf` types (keep existing
   tests compiling; adapt mart shape onto the new model).
2. Parser/serializer: Attributes / Values / Relationships (incl. `as "..."` /
   `as [link]` names) / Body / Notes sections + `annotates` resolution +
   round-trip tests.
3. Metaclass renderer registry (incl. `uml.Association` class box + dashed
   connector and `uml.Note` dog-eared box + dashed anchors) + generic fallback box.
4. Profile mechanism (`uml-domain`): emphasis + stereotype styles + palette.
5. Diagram doc as first-class view; multi-diagram in `ModelGraph`; external-refs
   in the inspector.
6. UML-domain template + author guide doc (replace/supplement `okf-format.md`).

Each stage: change → suite green → commit.

## Changelog

Revisions after the initial approval, from review comments:

- **Association names.** Relationships may carry an optional `as ...` label after
  the link (before the `:` ends), on **all** verbs. Two forms: `as "string"`
  (plain reading-label + note handle) or `as [Title](./slug.md)` (links to a
  `uml.Association` — an association class). No reading-direction arrow.
- **Association classes (`uml.Association`).** New top-level metaclass so an
  association can carry its own attributes. Bound to its edge via the `as [link]`
  name; ends stay on the inline bullet so class-to-class links remain direct (no
  two-hop fragmentation through a middle doc). Replaces the earlier "AssociationClass
  deferred" item.
- **Notes / comments (`uml.Note`).** New metaclass: dog-eared box, markdown `## Body`,
  anchored via an `annotates` relationship to **any element except an attribute** —
  any node (any metaclass, incl. another note) by plain link, or an association
  (source link + `as "name"`, or endpoint form when unnamed). Multiple, mixed-kind
  anchors allowed; the anchor is a dashed no-arrowhead comment line, not a directed
  dependency. Plus a `## Notes` shorthand on nodes that desugars to a self-anchored
  `uml.Note`.
- **`extends` → `specializes`.** Renamed the generalization verb. `extends` is a
  Java-ism and collides with UML's own «extend» (a use-case dependency);
  `specializes` matches the near→far = child→parent (child-declares-parent) reading.
