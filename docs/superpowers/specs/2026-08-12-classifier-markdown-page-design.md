# Classifier markdown page

The classifier preview surface stops drawing a single class-diagram card and
renders a documentation page instead: prose identity, a definition list of
properties, and associations written as directional sentences.

## Why

`ClassifierPreviewView` currently shows one card on a focus canvas plus the
inspector. The card repeats the inspector's facts in a worse form: it cannot
show a description, it cannot show incoming relationships, and reading an
association off an arrowhead requires knowing which end the arrow decorates.
A page can say `SpecialOrder specializes Car.` in words.

## Surface

`ClassifierPreviewView::sync` swaps `body.show_canvas` for
`body.show_markdown_viewer`. The inspector keeps its subject and its
no-picker rule, so field editing is unchanged. Chrome is unchanged:
`tool_dock: false`, `view_bar: false`, breadcrumb plus right dock.

`build_focus_scene` (`crates/waml-editor/src/scene.rs`) loses its only caller.
Remove it, and any card-sizing path that no longer has a caller once it is
gone.

The view serves all four classifier kinds (`Class`, `Interface`, `Enum`,
`DataType`) through `NavCategory`; the page shape below covers all four.

## Page generator

A pure function in the `waml` crate — model in, markdown out, no editor
dependency, so a CLI subcommand can emit the identical page later:

```rust
pub fn classifier_page(model: &Model, key: &str) -> Option<String>
```

`None` when `key` names no node. Sections emit in this fixed order, each
omitted when it would be empty:

1. `# {title}` — `concept.title`, falling back to `key`.
2. Dek line — kind label, stereotypes as `«name»`, and `abstract` when
   `abstract_` is set, joined by ` · `.
3. Description paragraph — `concept.description`.
4. `## Properties` — one bullet per `node.attributes` entry (see below).
   Replaced by `## Values` for `uml.Enum`.
5. `## Associations` — outgoing edges, subject elided.
6. `## Referenced by` — incoming edges, far classifier as subject.
7. `concept.body` verbatim, last, when non-empty.

### Properties

One bullet per attribute. Name and type always; multiplicity only when it is
not `1`; visibility only when declared. `Attribute::description`, when
present, is an indented continuation line under the bullet.

```markdown
## Properties

- `id` · `OrderId`
- `total` · `Decimal` — private
- `lines` · `OrderLine` `1..*`
  The line items on the order.
```

Properties keep UML multiplicity notation and suppress a bare `1`; only the
association sentences spell multiplicity out in words. The two differ
deliberately: a definition list is reference material read by scanning down a
column, where `1..*` is denser and a repeated `1` on every row is noise,
while a sentence has to be read as English.

A `uml.Enum` renders `## Values` from `node.values` instead — one bullet per
literal, no type or multiplicity.

### Associations

Every relationship is a sentence. Direction is carried by word order, never
by passive voice or a glyph: under `## Associations` the subject is this
classifier and is elided; under `## Referenced by` the far classifier is
named and takes subject position.

| `RelationshipKind` | Under `## Associations` | Under `## Referenced by` |
| --- | --- | --- |
| `Associates` | Associated with one or more Foo. | Bar is associated with Foo. |
| `Aggregates` | Aggregates one or more Wheel. | Bar aggregates Foo. |
| `Composes` | Composes one engine (Engine). | Bar composes Foo. |
| `Specializes` | Specializes Vehicle. | Bar specializes Foo. |
| `Implements` | Implements Drivable. | Bar implements Foo. |
| `Depends` | Depends on Fuel. | Bar depends on Foo. |
| `Includes` | Includes Checkout. | Bar includes Foo. |
| `Extends` | Extends Order. | Bar extends Foo. |
| `InstanceOf` | Instance of Template. | Bar is an instance of Foo. |
| `Links` | Links to Runbook. | Bar links to Foo. |

`Associates` is the one kind that shifts register. It is not a transitive verb
in ordinary English — "Associates one Customer" reads as a typo — so the
elided form is the participial "Associated with one Customer", and the
named-subject form is "Bar is associated with Foo." Every other kind stays a
plain transitive verb in both forms.

`Annotates` is skipped — it anchors a `uml.Note` and is not a relationship,
matching `build_classifier_view` and the web renderer.

Under `## Referenced by` the same relationship runs with subject and object
swapped: `SpecialOrder specializes Car.`, `ShippingLabel depends on Order.`

### Multiplicity in words

Multiplicity is read from the far end and spelled out, never printed as UML
notation. It is always stated, including `1` — "composes one engine" is
clearer than a bare name, and suppressing the common case makes its absence
ambiguous.

| Multiplicity | Prose |
| --- | --- |
| `1` | one |
| `0..1` | zero or one |
| `1..*` | one or more |
| `0..*`, `*` | zero or more |
| `n` (exact) | exactly three |
| `a..b` | two to five |

Numbers spell out as words through ten and render as digits above that.
Structural kinds that carry no multiplicity — `Specializes`, `Implements`,
`Extends`, `InstanceOf` — omit the count entirely.

### Naming the far end

When the far end declares a role, the role leads and the classifier follows
in parentheses; the classifier name is the link target either way:

```markdown
Composes one or more lines (OrderLine).
Associated with one customer (Customer).
Aggregates one or more Wheel.
```

Class names are never inflected. A plural count next to a singular name
(`one or more Wheel`) is deliberate: the name is an identifier and must match
the model exactly, and pluralising it would need an English inflector that is
wrong on `Person`, `Analysis`, `Status`, and on any non-English name, with no
way for an author to correct it. Authors who want a plural noun declare a
role, which is their own text.

A bidirectional edge (`edge.bidirectional`, or both ends navigable) renders
once, under `## Associations`, with `(both ways)` trailing the sentence. It
must not also appear under `## Referenced by`.

## Link navigation

Classifier names in the page are links to that classifier's page. No markdown
link is clickable in the reading view today — including the
`[Customer](./customer.md)` links already present in every fixture document —
so this is real work in `waml-markdown-editor`, and it fixes reading-view
links everywhere, not only on this page.

`PresentationPlan` already carries `PresentedLink { source_range,
destination }`; `build_reading_document` drops it, and the reading widget
hits only `FingerScroll`.

1. Carry the plan's links into `ReadingDocument`, keyed by flow range. The
   widget already maps flow to source through `ReadingDocument::source_span`.
2. Add a `FingerUp` hit-test to the reading widget that emits a
   link-clicked action carrying the destination.
3. `ClassifierPreviewView::handle` feeds that destination to
   `navigation::resolve_link`, producing a `NavigationTarget`, and returns it
   through the existing `ViewOutcome` navigation path.

The generator emits ordinary markdown links, so it needs no knowledge of the
widget: a far classifier's name is written as a link to its document path.

## Testing

**Generator** — string snapshots, since the whole output is reviewable text:

- `sixkind/car.md` — all six relationship kinds on one classifier.
- `groups-linked/order.md` — outgoing associations with multiplicity.
- `groups-linked/customer.md` — a classifier with attributes and no
  relationships of its own, whose only associations are incoming; proves
  `## Referenced by` is derived across documents.
- `mini/customer.md` — no relationships in either direction; proves both
  association sections are omitted rather than emitted empty.
- A `uml.Enum` fixture — `## Values` in place of `## Properties`.
- A missing key returns `None`.

Plus a table-driven unit test over the multiplicity speller covering every
row of the prose table, both boundaries of the spell-out-through-ten rule,
and an unparseable multiplicity; and one over far-end naming covering role
present, role absent, and a role identical to the class name.

**Widget** — a link click in the reading view resolves to the expected
`NavigationTarget`.

**View** — `ClassifierPreviewView::sync` installs a document on the markdown
viewer and leaves the inspector's subject intact.

## Out of scope

- Operations. `Model::Node` carries no operations; the scene node's
  `operations` field is always empty.
- Editing from the page. The inspector remains the only editing surface.
- Any diagram or graphic. Associations are sentences.
- Package and directory surfaces. This is the classifier preview only.
