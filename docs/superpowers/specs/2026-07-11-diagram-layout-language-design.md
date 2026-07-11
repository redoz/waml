# Diagram Layout Language

**Date:** 2026-07-11
**Product:** OKF-UML / Model Canvas (`docs/okf-uml.md`, `packages/okf`, `packages/web`)
**Scope:** The declarative layout + grouping language for **Diagram** documents. Replaces per-element saved coordinates with human-readable, relation-based positioning.

## Context

A **Diagram** document (`docs/okf-uml.md` — "Diagram documents") is a curated,
profiled *view* over a set of nodes. Today it carries a flat `## Members` list
and a `## Render hints` section whose positioning support is a single optional
saved coordinate per node:

```markdown
## Render hints
- [Order](./order.md) at 0,0          # optional saved position
```

Coordinates are the wrong authoring surface. A readable diagram of this kind
always needs human judgement to arrange, but nobody should hand-compute `x,y`
for every node, and coordinates are opaque to both a reviewer and an LLM reading
the document. We want to say *how things sit relative to each other* in language,
let a solver produce the pixels, and let the editor round-trip direct
manipulation back into that same language.

## Goals

- Position nodes and groups **relative to one another**, never by coordinate.
- Read as close to **natural English** as practical — the stored form is what an
  LLM and a human reviewer read and write.
- **Groups** bundle related nodes (e.g. everything about the user) and are laid
  out uniformly with individual elements at this stage.
- A **deterministic solver** turns the relations into a layout; the UI editor
  turns drags into relations (see Editing round-trip).
- A **formal grammar (BNF)** the Rust and TypeScript parsers are written against.

## Non-goals

- Absolute coordinates anywhere in the stored document. (The solver produces
  pixels at render time; they are never persisted.)
- Auto-layout that needs *zero* human input. Human arrangement is expected and
  supported — it just produces relations, not coordinates.
- A general constraint solver with numeric spacing. Spacing is qualitative
  (levels), not measured.

## Model

Two orthogonal concerns, kept separate:

### 1. Structure — groups are headings

A **group** is a markdown heading with a member list under it. Nesting is heading
depth. The heading declares *membership only* — no visual treatment rides on it.

```markdown
## Users
- [Customer](./customer.md)
- [Account](./account.md)

### VIP                       # nested group = deeper heading
- [Platinum](./platinum.md)

## Orders
- [Order](./order.md)
- [OrderLine](./order-line.md)
```

- Groups and elements are **operands of the same kind** — anything you can say
  about an element you can say about a group.
- **Member list order within a group implies a top-to-bottom stack** (a column)
  by default. An `as row` / `as column` hint (see Render hints) flips the axis;
  an explicit relation overrides individual placements.
- A diagram with no group headings is a single implicit top-level group.

### 2. Layout — the relation language

Relations and render treatment live in a separate **`## Layout`** section (it
supersedes the positional `at x,y` hint; `emphasize` / `collapse` move here too).
Each line is one statement in the language below.

```markdown
## Layout
- Users left of Orders
- column of Customer, Account with roomy
- top of VIP aligned with top of Orders
- Orders with frame
```

#### Relations

Two families, both plain English:

**Placement** — one operand on a side of another; chainable:

```
Users left of Orders
Order above OrderLine above Payment
```

- directions: `left of` · `right of` · `above` · `below`
- **adjacency (tight, aligned) is the default** — that is how you build rows and
  columns. There is no "loose / far" variant; separation is controlled by
  spacing hints, not by the relation.

**Alignment** — line up edges or centers, independent of ordering, with a named
anchor on each side:

```
top of X aligned with bottom of Y
center of X aligned with center of Y
X aligned with Y                       # bare = center-to-center
```

- edges: `top` · `bottom` · `left` · `right` · `center`
- **anchor → axis** mapping: `top`/`bottom` constrain the **Y** position;
  `left`/`right` constrain the **X** position; `center` constrains **both**
  (concentric). `left of` / `above` etc. are the ergonomic path; anchor-align is
  the precise escape hatch (e.g. `bottom of X aligned with top of Y` stacks X on
  Y explicitly).

#### Operands

An operand is any of:

- an **element** name (or link) — e.g. `Customer` / `[Customer](./customer.md)`
- a **group** (heading) name — e.g. `Users`
- an inline **`column of …`** — ordered vertical stack, adjacency
- an inline **`row of …`** — ordered horizontal run, adjacency
- a parenthesized operand — `( … )` for nesting / disambiguation

Inline `row`/`column` are anonymous groups usable anywhere a name is; they nest:

```
row of (column of Customer, Account), Orders
```

#### Render hints (`with …`)

Any operand may carry a `with` clause. Named groups and elements may instead be
targeted by-reference on their own `## Layout` line (`Orders with frame`); an
**anonymous** inline group can *only* be treated inline, since it has no name to
reference.

- **axis** (groups only): `as row` · `as column` — lays the group's members
  along that axis in list order. Default is `as column` (the list-order stack);
  this is the only way to set the internal axis of a *named* heading group, since
  its members aren't restated inline. `Users as row` flows Customer, Account
  horizontally instead of stacking them.
- **shape** (groups only): `frame` (visible titled box) · `box` (square bounding
  box, invisible) · `shrink` (shrink-wrapped hull, invisible). **Default =
  invisible shrink-wrap** — a group clusters its members without drawing unless
  it opts into `frame`/`box`.
- **spacing** (any operand): qualitative levels — `snug` · (normal, default) ·
  `roomy`. Additive breathing room around the operand; no numbers.
- **emphasize** · **collapse** — carried over from the existing render hints
  (`collapse` renders a node as a reference chip rather than a full box).

**Shape vs spacing are orthogonal.** The old `wide` / `thin` / `none` idea was
really `shrink` + {more, less, no} spacing; splitting them means spacing applies
to a `box` or a bare element too, not just to a hull.

**Keep-out geometry.** `box` reserves a rectangle (wastes corner space); `shrink`
reserves the minimal polygon hugging its members, so neighbouring groups can tuck
into the concave notches — the compactness win. `frame` reserves a rectangle and
draws it with the group's title.

### 3. Editing round-trip

The stored form is relations; the UI is a relation generator.

1. User drags a node/group in the canvas.
2. On release the editor **infers the relation(s)** the new position implies
   (nearest neighbour + side, or an alignment).
3. It **writes the sentence** into `## Layout`.
4. The solver **re-solves** and the node **snaps** into the solved position.

No coordinate is ever written. A human who never touches the text still produces
readable relations; an LLM editing the text sees exactly what the human sees.

## Grammar (BNF)

Informal but parser-targetable. Terminals in quotes; `{ }` = zero-or-more,
`[ ]` = optional. Whitespace between tokens; list separators are commas.

```bnf
layout        ::= { statement }
statement     ::= placement | alignment | standalone

placement     ::= operand direction operand { direction operand }
direction     ::= "left of" | "right of" | "above" | "below"

alignment     ::= anchored "aligned with" anchored
anchored      ::= [ edge "of" ] operand
edge          ::= "top" | "bottom" | "left" | "right" | "center"

standalone    ::= operand                     ; lone operand — meaningful when it
                                              ; carries `with` hints: a name with
                                              ; hints is by-reference treatment
                                              ; (`Orders with frame`), an inline
                                              ; group with hints is an anonymous
                                              ; group treatment

operand       ::= ref [ "with" hints ]
ref           ::= name
                | inline-group
                | "(" operand ")"
inline-group  ::= ("column" | "row") "of" operand-list
operand-list  ::= operand { "," operand }

hints         ::= hint { ("," | "and") hint }
hint          ::= axis | shape | spacing | flag
axis          ::= "as row" | "as column"
shape         ::= "frame" | "box" | "shrink"
spacing       ::= "snug" | "roomy"
flag          ::= "emphasized" | "collapsed"

name          ::= identifier | markdown-link | quoted-string
```

**`with` disambiguation rule.** A `with` clause binds greedily to the nearest
complete operand to its left. To attach a `with` clause to an entire inline
group rather than to its last member, parenthesize:

```
column of Customer, Account with roomy          ; roomy attaches to Account
(column of Customer, Account) with roomy         ; roomy attaches to the column
```

The parser applies the same rule to a trailing relation:
`(column of Customer, Account) left of Orders` is unambiguous; a bare
`column of Customer, Account with roomy left of Orders` requires the parens to
express "the roomy column, left of Orders".

## Worked example

```markdown
---
type: Diagram
title: Orders Domain Model
profile: uml-domain
---
# Orders Domain Model

## Users
- [Customer](./customer.md)
- [Account](./account.md)

## Orders
- [Order](./order.md)
- [OrderLine](./order-line.md)
- [OrderStatus](./order-status.md)

## Layout
- Users with frame
- Users left of Orders
- top of Users aligned with top of Orders
- column of Order, OrderLine, OrderStatus with roomy
- collapse [Money](./money.md)
```

Renders: a titled **Users** frame (Customer stacked on Account by list order) to
the left of the **Orders** group; the two groups' tops aligned; Orders' three
members in a roomy column; Money shown as a reference chip.

## Integration with existing spec

- `docs/okf-uml.md` "Diagram documents": `## Members` gains optional group
  sub-headings; the flat list remains valid as one implicit group.
- `## Render hints` positional `at x,y` is **removed**; its `emphasize` /
  `collapse` semantics move into `## Layout` (inline `with` or by-reference).
- `packages/okf` types (`Diagram`, `hints`) grow a parsed `layout` model;
  `packages/web` (React Flow) consumes solved positions and emits relations on
  drag. Solver + editor inference are follow-on implementation specs.

## Open questions

- Exact spelling of qualitative spacing beyond `snug` / `roomy` (need a third
  "very roomy"? make it repeatable?).
- Whether `## Layout` is the final section name or folds back into a renamed
  `## Render hints`.
- Conflict handling when inferred/authored relations over-constrain (solver
  precedence: last statement wins? explicit beats implicit list-order?).
