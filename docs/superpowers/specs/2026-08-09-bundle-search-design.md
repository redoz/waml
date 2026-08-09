# Bundle Search — Design

Date: 2026-08-09
Status: Approved design, not yet planned

## Problem

A bundle has no search. Finding anything means knowing where it is: walking the
tree, or remembering which document holds the sentence you wrote. Both
[`mvp.md`](../../waml/goals/mvp.md) and [`beyond-uml.md`](../../waml/goals/beyond-uml.md)
list search as horizon work, deliberately deferred. This design ends the
deferral.

The bar is "really good search", not "a search engine". Nothing here rebuilds
Lucene, and nothing here forecloses adopting one later.

## Goals

- Find a concept, a document, or a sentence from one keystroke path.
- Sweep every match of a term across a bundle and work through them in order.
- Find within the open document, including on a canvas.
- Behave identically in the native editor and on an exported static site.
- Keep the index backend replaceable without redesigning any surface.

## Non-goals

Phrase queries, typo tolerance, regex, wildcards, and nested boolean groups.
Cross-bundle or workspace-wide search. A persisted on-disk index. Split view or
multi-pane hosting. Preview panes. URL-addressable queries (`?q=`). Each is
reachable later; none is in v1.

## Corpus and fields

Every document contributes to four field groups. A document kind that has
nothing for a group simply contributes nothing.

| Field group | Contents | Example match |
| --- | --- | --- |
| `names` | Concept names, document titles, headings | `Payment` the class |
| `prose` | Projected, rendered text with markup removed | "the payment is captured after…" |
| `model` | Concept kind, relationship endpoints, tags | `kind:actor`, `Order ──▶ Payment` |
| `structure` | Ids, frontmatter keys, WAML keywords, link targets | `id: payment-capture` |

Raw source and projected content overlap for nearly all prose: the same word at
the same span. Those dedupe to one hit, keyed by document and span. What raw
source earns on its own is the `structure` group — the ids, keys, and link
targets that never appear in projected output. Those are a distinct group
precisely so they can be presented apart from prose rather than polluting it.

Ranking puts `names` above `model` above `prose` above `structure`.

## Engine boundary

One trait in `waml` core:

```
build(documents) -> Index
update_document(id, fields)
remove_document(id)
query(&str, scope) -> Vec<Hit>
snippet(hit, width) -> Snippet
```

A `Hit` carries the document, the field group that matched, a target (text span
or model element reference), and a score. Surfaces consume `Hit`; they never
see the index.

**v1 backend: a hand-rolled in-memory inverted index.** No new dependencies.
Compiles to `wasm32-unknown-unknown` under makepad's runtime with nothing to
work around — no threads, no filesystem, no clock, no `getrandom`. Indexes
today's largest real bundle (181 documents, 747 KB) in single-digit
milliseconds.

**Why not tantivy in v1.** Tantivy (0.26.0, April 2026) is the right answer at
the 100k-document tier and gives phrase, fuzzy, regex, and facets for free. It
is the wrong answer here: its wasm support is an open RFC rather than a
supported target, its `IndexWriter` wants threads, segment ids want
`getrandom`, date handling touches `SystemTime::now()` — which panics under
makepad wasm — and it adds multiple megabytes to a web boot path that was
expensively brought down to 1.7 s. Its persistence model also fights a bundle:
a directory of binary segments either gets committed to git alongside text
sources, or gets rebuilt on every open. Behind the trait, tantivy remains
available later as a **native-only** backend, with the hand-rolled index
continuing to serve wasm.

### Query language, v1

- Bare terms, ANDed: `payment capture`
- Prefix matching as you type: `paym` matches `payment`
- Field filters: `kind:actor`, `in:docs/guides/`
- BM25 ranking

Anything not parsed as a filter is a term. Unknown filter names are treated as
terms rather than erroring.

### Index lifecycle

The index builds when the bundle opens and updates per document on save, not
per keystroke. Both are `SearchIndex` calls; neither is visible in a bundle of
today's size.

At larger sizes the build stops being instant, so search must degrade rather
than block. Names and model facts come from the already-loaded projection, so
those sections are always available immediately. If the text index is still
building, the palette shows `Concepts`, `Documents`, and `Structure` normally
and renders the `Text` section with a quiet `indexing…` note, filling it in
when the build completes. Search is never unavailable, and it never lies about
what it has looked at.

## Surfaces

### Palette

The palette is a new surface — the editor has no command palette today. It is
not new *machinery*: it is a popup route over the existing `PopupRoot` and
linear `MenuPopup` path, which already handles dismissal and swallowing the
underlay.

One box, invoked by hotkey. You type; you get one blended, sectioned list:

```
CONCEPTS · 3
  class   Payment                           domain/billing.waml
  class   PaymentMethod                     domain/billing.waml
  actor   Payment Processor                 domain/actors.waml
DOCUMENTS · 1
  md      payment-flow.md                   guides/
TEXT · 12
          …the payment is captured only…    checkout.md:42
          Retrying a failed payment must…   checkout.md:51
          + 8 more
STRUCTURE · 2
  id      id: payment-capture               guides/checkout.md
  link    ](./payment-flow.md)              guides/index.md
```

Name hits always outrank body hits, so typing `payment` surfaces the `Payment`
class before prose about payments. The final row is always
`Search all text for "payment" — 18 results`, which opens the results tab.

No sigils, no modes, no prefix vocabulary. There is one thing to type and one
list to read.

### Results tab

Escalating opens a `Search: payment` tab in the existing tab strip. This adds
no chrome and invents no pattern: it is a tab, it participates in tab history
and navigation like any other, and two searches are two tabs you can compare.

Results are grouped by document under collapsible headers carrying a hit count.
Ranking orders rows within a group. Each row shows what matched (`name`, `rel`,
`id`, or a line number for prose) and a snippet with the matched term
highlighted.

```
🔍 payment                                    18 in 6 documents

▾ billing.waml            docs/waml/domain/                    4
    name   class Payment
    name   class PaymentMethod
    rel    Order ──▶ Payment
    doc    Captures a payment against an order total.
▾ checkout.md             docs/waml/guides/                    7
    42     …the payment is captured only after the reservation…
    51     Retrying a failed payment must not re-reserve stock.
▸ refunds.waml            docs/waml/domain/                    3
▾ legacy-gateway.md       docs/waml/archive/                   2
    hidden …the old payment gateway shim…
```

Activating a row opens that document through the normal open path — the same
one the tree uses. There is no preview pane, no transient tab slot, and no new
tab semantics. The results tab stays put; you navigate back to it.

### Find strip

`Ctrl+F` opens a thin strip over the open document: query, `3 of 12`, next,
previous, close. Scoped to that document only.

In text surfaces it scrolls and highlights. **On a canvas, matching nodes stay
lit and everything else dims**, and next/previous pans between them — find as a
spotlight rather than a scroll position. Same session machinery as global
search, pre-scoped to one document.

## Search session and reveal

A query stays live after you land on a hit:

- Every other match in the open document is highlighted.
- `F3` / `Shift+F3` walk to the next and previous hit, **across document
  boundaries**, following results-tab order.
- The results tab marks the current position.
- `Esc` ends the session and clears every highlight.

This is what makes a rename or an audit feel like one motion instead of
eighteen round trips.

### `DocView::reveal`

The session needs exactly one new piece of trait surface:

```
fn reveal(&mut self, cx: &mut Cx, body: &BodyWidgets, target: RevealTarget)
```

`RevealTarget` is a text span or a model element reference. The results tab,
the find strip, and `F3` traversal all call it — there is no second path.

Both implementations sit on machinery that already exists. Markdown reveals
over [`DecorationRole`](../../../crates/waml-markdown-editor/src/presentation/draw.rs).
The canvas reveals over the `set_focus` path that
[`ClassifierPreviewView`](../../../crates/waml-editor/src/classifier_preview_view.rs)
already uses to frame a single classifier.

### Activation per document kind

A hit's target, not its document, decides where you land:

- **Model hits** (`names`, `model`) open the canvas with the node selected and
  the camera centred on it, inspector populated. A hit on a class is a hit on a
  class, not on line 14.
- **Prose and structure hits** open the text surface scrolled to the span.

A document containing both kinds of hit therefore lands you in different places
depending on the row you picked — which is what the row already told you it
was.

## Projection-masked content

Matches in content the active projection mask hides **do surface**, as muted
rows carrying a `hidden by projection` badge. Activating one offers to reveal
the masked content.

Nothing is ever silently missing, and search doubles as a way to find what you
masked away. The alternative — matching only what is currently visible — fails
the "I know it is in this bundle" query with no explanation, which is the worst
possible failure for a search feature.

## Published static site

The exported site runs the same runtime, so it gets the same behaviour: same
palette, same sections, same results tab, same find strip, same hidden badges.
One behaviour to design, document, and test, and no seam for a reader who also
authors. The index is built at export and shipped as an asset, so it is fast
and never stale.

**Masked content and the export boundary.** The rule is: *the index is built
from what the export ships.* If the export strips projection-masked content,
that content is not in the shipped index and hidden hits simply do not exist on
the published site. If the export ships masked source, search adds no exposure
that view-source did not already have. Either way search cannot leak what the
export withheld, and no separate published-site search policy is needed.

## States

- **Empty query** — palette shows recently opened documents, reusing the
  recents the start screen already tracks.
- **No results** — a single row naming the query, plus the active scope filters
  so the user can see why (`in:docs/guides/` is a common self-inflicted zero).
- **Hidden-only results** — the muted rows show normally; the count line says
  how many of the hits are hidden.
- **Index building** — as described under Index lifecycle.

## Testing

- Unit tests on the index: tokenising, field-group assignment, prefix matching,
  filter parsing, dedupe of overlapping raw/projected spans, ranking order
  across field groups.
- Golden tests over the `docs/waml/` bundle for a fixed set of queries, so
  ranking changes are visible in a diff.
- Typed UI regression tests per the
  [typed UI regression testing design](2026-08-08-typed-ui-regression-testing-design.md)
  for the palette sections, the results tab grouping, and the find strip
  counter.
- `reveal` tested per `DocView` implementation: a text span scrolls into view
  and decorates; a model element selects and centres.
- A wasm build check, since the index is core and must stay wasm-clean.

## Risks

- **Ranking is the part that makes search feel good or bad**, and it is the
  part with no compile error when it is wrong. The golden query set is the
  defence; it needs to be written early, not last.
- **`structure` matches can be noisy** in bundles with many ids and links.
  Section separation contains it, but if the section is routinely ignored it
  should be collapsed by default rather than removed.
- **Four new key bindings** — palette, find strip, and `F3`/`Shift+F3` — none
  of which the editor claims today. They need checking against existing
  bindings and against what makepad consumes before the surfaces are built, not
  after.
- **Canvas dimming for the find strip is new visual state** on a surface that
  already carries selection, hover, and conflict states. It must compose with
  them rather than compete.

## Documentation updates

Search is currently listed as out of scope in two places. Both need to change
when this lands: [`mvp.md`](../../waml/goals/mvp.md) and
[`beyond-uml.md`](../../waml/goals/beyond-uml.md).
