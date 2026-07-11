# Spec change: attribute cardinality delimiter `[…]` → `{…}`

Status: proposed · Date: 2026-07-11 · Profile: OKF-UML

## Summary

OKF-UML currently writes an attribute's multiplicity as a trailing
bracketed token, e.g. `- total: [Money](./money.md) [1]`. The `[…]`
delimiter collides with Markdown's link/reference syntax. This spec
replaces the delimiter with curly braces — `{…}` — for attribute
multiplicity only. The multiplicity **grammar** (the strings between the
delimiters) is unchanged; only the surrounding delimiter changes.

Nothing else in OKF-UML changes. In particular, relationship-end
multiplicities are unaffected (they were never bracketed).

## Motivation — the clash

An attribute line is authored, stored, and displayed as raw Markdown. In
Markdown, `[…]` is link/reference territory:

- `[0..1]` is a *shortcut reference link*. With no matching reference
  definition it renders as literal text on most renderers, but the
  behaviour is renderer-dependent and fragile.
- `[1]` collides directly with footnote / numbered reference-link syntax.
  A document that defines a reference or footnote label `[1]` elsewhere
  silently changes how the multiplicity renders.
- Markdown linters, formatters, and WYSIWYG editors may "repair",
  reflow, or escape `[…]` runs they read as broken links, corrupting the
  attribute on a round-trip.

Curly braces carry no meaning in Markdown, so `{0..1}` survives every
renderer, linter, and editor untouched. UML itself already uses `{…}` for
property constraints, so the delimiter reads naturally in a UML context.

## Scope

**In scope:** the delimiter around an attribute's multiplicity, in the
`## Attributes` section of any classifier node (`uml.Class`,
`uml.Interface`, `uml.DataType`, `uml.Association`, and any generically
rendered node).

**Out of scope — unchanged:**

- The multiplicity grammar itself: `1`, `0..1`, `*`, `1..*`, `0..*`,
  `2..5`; `*` unbounded; bare `*` ≡ `0..*`; bare `n` ≡ exactly `n`; in
  `lower..bound`, `lower ≤ bound` unless `bound` is `*`.
- The default: an attribute with no multiplicity is still treated as
  exactly one.
- **Relationship ends** (`: <near> to <far>`). These already carry bare,
  unbracketed multiplicities (e.g. `1 order to 1..* lines`) and had no
  clash. They stay exactly as they are — do **not** wrap them in `{…}`.
- Enum `## Values` (name-only literals, never carried a multiplicity).
- Visibility, type tokens, type links, roles, association names, notes.

## Grammar change

The attribute-line grammar changes in exactly one production — the
delimiter around `<multiplicity>`.

Before:

```
<attribute>    ::= "- " <visibility>? <name> ": " <type> ( " [" <multiplicity> "]" )?
```

After:

```
<attribute>    ::= "- " <visibility>? <name> ": " <type> ( " {" <multiplicity> "}" )?
```

`<visibility>`, `<name>`, `<type>`, and `<multiplicity>` are all
unchanged. The multiplicity remains optional and, when present, remains a
single space-separated trailing token — now `{…}` instead of `[…]`.

### Examples

Before → after:

```
- id: OrderId [1]                              →  - id: OrderId {1}
- placedAt: Timestamp [1]                      →  - placedAt: Timestamp {1}
- status: [OrderStatus](./order-status.md) [1] →  - status: [OrderStatus](./order-status.md) {1}
- shippingAddress: [Address](./address.md) [0..1]
                                               →  - shippingAddress: [Address](./address.md) {0..1}
- tags: String [0..*]                          →  - tags: String {0..*}
```

A linked type keeps its `[title](./slug.md)` Markdown link — that `[…]`
is a genuine Markdown link and is **not** touched. Only the multiplicity
delimiter moves to `{…}`.

## Parsing and validation rules

- Multiplicity is recognised only as a `{…}` token at the **end** of the
  attribute line, separated from the type by whitespace.
- The braces must wrap a valid multiplicity string. A `{…}` token whose
  contents are not a valid multiplicity is not a multiplicity — the line
  is treated as malformed rather than silently accepting arbitrary brace
  content.
- A trailing `[…]` token is **no longer** recognised as a multiplicity.
  Readers understand `{…}` only; there is no legacy-bracket fallback. All
  stored documents are migrated in one shot (see Migration).
- The bare-token type guard still rejects stray link/bracket punctuation
  in a type position; extend it so a stray `{` or `}` in a type position
  is likewise treated as malformed.

## Serialization

Serializers emit attribute multiplicity with `{…}`. As today, the
implicit default is not written: an attribute whose multiplicity is
exactly one serializes with no multiplicity token at all (no `{1}`),
matching current behaviour with brackets. All other multiplicities are
emitted, e.g. `{0..1}`, `{1..*}`, `{*}`.

## Migration

No back-compatibility. Readers understand `{…}` only. Every stored
document is rewritten once, before the new reader ships, with a single
find-and-replace.

**The rewrite:** a trailing bracketed multiplicity at the end of a line
becomes the same string in braces.

- Find (per line, multiline): `\[(<mult>)\]\s*$`
- Replace: `{$1}`

where `<mult>` is the multiplicity grammar spelled out as a regex:

```
(?:[1-9]\d*|\*|(?:0|[1-9]\d*)\.\.(?:[1-9]\d*|\*))
```

Full pattern:

```
\[((?:[1-9]\d*|\*|(?:0|[1-9]\d*)\.\.(?:[1-9]\d*|\*)))\]\s*$   →   {$1}
```

**Why it is safe — it only touches multiplicities:**

- **Anchored at end of line (`$`).** An attribute's multiplicity is the
  last token on its line; the pattern only fires there.
- **Contents must be a valid multiplicity.** The captured group is the
  multiplicity grammar itself, so a `[…]` whose contents are not a
  multiplicity is never matched.
- **Type links are untouched.** A linked type `[Address](./address.md)`
  ends in `)`, not `]`, and its label (`Address`) is not a valid
  multiplicity — it fails both the `$` anchor and the grammar.
- **Relationship ends are untouched.** Ends carry bare, unbracketed
  multiplicities (`: 1 order to 1..* lines`), so there is no `[…]` to
  match on those lines.

Apply the pattern line-by-line (multiline mode) across every stored
document. No document-structure awareness is required; the anchor plus the
grammar constraint make it self-limiting.

### Worked cases

```
- id: OrderId [1]                                 →  - id: OrderId {1}
- shippingAddress: [Address](./address.md) [0..1] →  - shippingAddress: [Address](./address.md) {0..1}
- status: [OrderStatus](./order-status.md)        →  (unchanged — no trailing multiplicity)
- composes [OrderLine](./order-line.md): 1 order to 1..* lines
                                                  →  (unchanged — relationship end, no brackets)
```

## Documentation impact

Every place that documents or demonstrates attribute multiplicity moves
from `[…]` to `{…}`:

- The OKF-UML format spec — the `## Attributes` grammar, the worked
  examples, and the conventions summary.
- Any author-facing / AI-facing format guide shipped alongside the app.
- Any built-in template or example document.

The multiplicity vocabulary description (`1`, `0..1`, `*`, `1..*`, …)
stays verbatim; only the delimiter shown around it changes. Reviewers
should confirm that relationship-end examples were **not** changed, since
those ends are bare by design.

## Rationale for `{…}` over alternatives

- **`{0..1}` (chosen):** no Markdown meaning; matches UML's constraint
  delimiter; visually distinct from the `[title](url)` link that may
  appear in the same line.
- **`` `0..1` `` (code span):** Markdown-safe but visually noisy and
  breaks the plain bare-token reading of an attribute line.
- **`(0..1)` (parens):** overlaps the `(…)` half of link syntax and reads
  as an aside; weaker separation from type links.
- **Escaping `\[1\]`:** Markdown-safe but ugly and harms round-trip
  fidelity.
