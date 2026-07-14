# WAML — OKF authoring guide for AI agents

Use this to generate a domain model in **OKF (Open Knowledge Format)** that imports cleanly into **WAML** (https://github.com/redoz/waml).

**How the user will use your output:** they open **Import** on the canvas, paste your text (or upload a `.zip`), and review the rendered UML diagram.

OKF is just **Markdown**: one file per classifier (a class, interface, enum, data type, package…), plus optional **diagram** documents that curate a view. When you deliver everything in one blob, separate the documents with `<!-- path/slug.md -->` markers. A document's **slug** is its title lowercased with spaces → hyphens (e.g. `Order Line` → `order-line.md`), and that slug is what every cross-reference links to.

---

## ⚠️ Read first — the 3 rules that make relationships work

1. **One file per classifier, separated by markers.** A single blob with no markers is read as ONE document, so nothing can relate. When pasting, put each document after a `<!-- path/slug.md -->` comment. Each file's name is the classifier's **slug** (title lowercased, spaces → hyphens).

2. **A relationship line points to the target's exact file name.** Format:
   `- <verb> [Target Title](./target-slug.md)`
   The `target-slug` in `(./target-slug.md)` MUST equal the target's file name. If `Customer` lives in `customer.md`, the link is `(./customer.md)` — not `(./customers.md)`.

3. **Ends are required for associations, forbidden for the rest.** `associates`, `aggregates` and `composes` REQUIRE a `: <near> to <far>` multiplicity clause. `specializes`, `implements` and `depends` FORBID it (they carry no ends).

✅ Correct:
```
## Relationships
- composes [OrderLine](./order-line.md): 1 to 1..* lines
- depends [PricingService](./pricing-service.md)
```
❌ Wrong (slug ≠ filename; ends on a dependency; missing ends on a composition):
```
## Relationships
- composes [OrderLine](./orderline.md)
- depends [PricingService](./pricing-service.md): 1 to 1
```

---

## The classifier document

```
---
type: uml.Class            # family.Metaclass
stereotype: [aggregateRoot, entity]
title: Order
description: "A customer's placed order."
---
# Order

## Attributes
- id: OrderId {1}
- status: [OrderStatus](./order-status.md) {1}
- total: [Money](./money.md) {1}

## Relationships
- associates [Customer](./customer.md): 1 order to 1 customer
- composes [OrderLine](./order-line.md): 1 to 1..* lines
```

### Frontmatter
- `type` — `family.Metaclass`. UML metaclasses: `uml.Class`, `uml.Interface`, `uml.Enum`, `uml.DataType`, `uml.Package`. An unknown `type` renders as a generic labelled box (see *Graceful degradation*).
- `stereotype` — a scalar (`stereotype: entity`) or a list (`stereotype: [aggregateRoot, entity]`). Open set; drives styling in the `uml-domain` profile.
- `abstract` — `true` for an abstract classifier (optional).
- `title` — the display name; its slug is the file name and what relationships link to, so **keep titles distinct**.
- `description` — one-line summary (optional).

### `## Attributes`
One bullet per attribute: `- [<visibility> ]<name>: <Type> {<multiplicity>}`.
- **Visibility** (optional, leading): `+` public, `-` private, `#` protected, `~` package.
- **Type** — either a bare token (`OrderId`, `String`, `Decimal`) or a link `[Title](./slug.md)` to another classifier (the canvas draws navigation to it).
- **Multiplicity** — trailing `{…}`: `1`, `0..1`, `*`, `1..*`, `2..5`. Absent ⇒ `{1}`.

### `## Values` (enums only)
For `type: uml.Enum`, list the literals — one `- LITERAL` per line:
```
## Values
- DRAFT
- PLACED
```

---

## `## Relationships` — the taxonomy

Every relationship line is `- <verb> [Target](./slug.md)[ as <name>][: <near> to <far>]`. The **line style** comes from the verb's *category*; the **end adornment** comes from the verb itself.

| verb | category | line | adornment (at far end) | ends |
|------|----------|------|------------------------|------|
| `associates` | association | solid | plain / open arrow if navigable | **required** |
| `aggregates` | association | solid | hollow diamond ◇ at the whole | **required** |
| `composes` | association | solid | filled diamond ◆ at the whole | **required** |
| `specializes` | generalization | solid | hollow triangle ▷ at the parent | forbidden |
| `implements` | dependency (realization) | dashed | hollow triangle ▷ at the interface | forbidden |
| `depends` | dependency | dashed | open arrow → at the dependency | forbidden |
| `annotates` | note anchor | dashed | none | forbidden (`uml.Note` only) |

Composition ⊂ aggregation ⊂ association — each is a stronger association, so they all share the solid line and all take ends.

**Reading direction.** `near` is the declaring document, `far` is the target. For `specializes`, **near → far = child → parent**: the document that declares `specializes [Parent]` *is* the subtype. There is no separate reading-direction arrow.

**Reciprocity → bidirectional.** An association is bidirectional when **both** documents declare the reverse line at each other. The first-parsed declaration's ends win; the reverse one only flips it bidirectional and marks both ends navigable. Mismatched reciprocal multiplicities are not an error.

---

## Association names & association classes

Any relationship may carry an optional name with `as …`, placed **after the link and before the `:` ends**:

- `as "places"` — a plain reading label (also the handle a note anchors by):
  `- associates [Customer](./customer.md) as "places": 1 to *`
- `as [Places](./places.md)` — a link to a `uml.Association` document, an **association class** that carries its own `## Attributes`. The ends stay on the inline bullet so class → class navigation stays direct.

---

## Notes / comments (`uml.Note`)

A `uml.Note` is a dog-eared comment. It has a `## Body` (free markdown) and `annotates` relationships to what it comments on:

```
---
type: uml.Note
title: Pricing rule
---
# Pricing rule

## Body
Totals are recomputed by the pricing service on every change.

## Relationships
- annotates [Order](./order.md)
```

Anchor forms: a classifier by plain link (above); a **named** association by its source link plus `as "name"`; or an **unnamed** association by endpoint — `annotates [Src](./src.md) associates [Tgt](./tgt.md)`.

Shorthand: a `## Notes` list on a classifier is sugar for a note that annotates just that classifier, and it round-trips back to `## Notes`.

---

## The diagram document

A diagram curates which classifiers appear in a view and how it's styled. `diagrams` are optional — with none, the canvas shows one implicit diagram of every node.

```
---
type: Diagram
title: Orders Domain
profile: uml-domain
---
# Orders Domain

## Members
- [Order](./order.md) at 40,80
- [Customer](./customer.md)

## Render hints
- emphasize: order, customer
- collapse [PricingService](./pricing-service.md)
```

- `profile` — the styling lens; `uml-domain` is the built-in one. An unknown profile falls back to `uml-domain`.
- `## Members` — the classifiers in this view, in curated order. Optional `at x,y` sets a position.
- `## Render hints` — `- emphasize: <slug>, …` highlights nodes; `- collapse [T](./slug.md)` draws a node as a reference chip.

---

## Graceful degradation

The canvas never errors on an unrecognized `type` — an unknown family/metaclass renders as a generic labelled box. Unknown `##` sections are carried through import → export unchanged, never dropped. So partial or forward-looking models still import.

---

## Complete worked example (copy-paste-ready)

Four classifiers, an enum, a value object and a diagram — a small DDD-flavored orders domain:

```
<!-- shop/order.md -->
---
type: uml.Class
stereotype: [aggregateRoot, entity]
title: Order
description: "A customer's placed order."
---
# Order

## Attributes
- id: OrderId {1}
- status: [OrderStatus](./order-status.md) {1}
- total: [Money](./money.md) {1}

## Relationships
- associates [Customer](./customer.md): 1 order to 1 customer
- composes [OrderLine](./order-line.md): 1 to 1..* lines

<!-- shop/order-line.md -->
---
type: uml.Class
stereotype: entity
title: OrderLine
---
# OrderLine

## Attributes
- quantity: Int {1}
- unitPrice: [Money](./money.md) {1}

<!-- shop/customer.md -->
---
type: uml.Class
stereotype: [aggregateRoot, entity]
title: Customer
---
# Customer

## Attributes
- id: CustomerId {1}
- name: String {1}

<!-- shop/order-status.md -->
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

<!-- shop/money.md -->
---
type: uml.DataType
stereotype: valueObject
title: Money
---
# Money

## Attributes
- amount: Decimal {1}
- currency: CurrencyCode {1}

<!-- shop/orders-domain.md -->
---
type: Diagram
title: Orders Domain
profile: uml-domain
---
# Orders Domain

## Members
- [Order](./order.md)
- [OrderLine](./order-line.md)
- [Customer](./customer.md)
- [OrderStatus](./order-status.md)
- [Money](./money.md)
```

That's a complete, valid model: five classifiers, two relationships (a `composes` and an `associates`) and one curated diagram. The user pastes it into **Import** and reviews the rendered graph.

---

## Legacy note

The older data-mart format (frontmatter `type: "OWOX Data Mart"`, a `# Schema` table and a `## Joins` section) still imports: schema columns become attributes and each join becomes an `associates` relationship. You don't need to author it for new models — prefer the UML classifier format above.
