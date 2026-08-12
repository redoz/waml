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

A `uml.Enum` renders `## Values` from `node.values` instead — one bullet per
literal, no type or multiplicity.

### Associations

Every relationship is a sentence. Direction is carried by word order, never
by passive voice or a glyph: under `## Associations` the subject is this
classifier and is elided; under `## Referenced by` the far classifier is
named and takes subject position. One verb per kind covers both.

| `RelationshipKind` | Verb | Example sentence (outgoing) |
| --- | --- | --- |
| `Associates` | associates | Associates Customer as `customer`. |
| `Aggregates` | aggregates | Aggregates 1..\* Wheel. |
| `Composes` | composes | Composes Engine. |
| `Specializes` | specializes | Specializes Vehicle. |
| `Implements` | implements | Implements Drivable. |
| `Depends` | depends on | Depends on Fuel. |
| `Includes` | includes | Includes Checkout. |
| `Extends` | extends | Extends Order. |
| `InstanceOf` | is an instance of | Instance of Template. |
| `Links` | links to | Links to Runbook. |

The elided-subject form capitalises the verb and drops the leading `is` where
one exists; the named-subject form under `## Referenced by` uses the verb
column verbatim: `Reading is an instance of Template.`

`Annotates` is skipped — it anchors a `uml.Note` and is not a relationship,
matching `build_classifier_view` and the web renderer.

Multiplicity is read from the far end and printed before the far classifier's
name, suppressed when it is `1` — the same rule the attribute rows use. A
role, when the far end declares one, trails as `` as `role` ``.

Under `## Referenced by` the same verb runs with the subject and object
swapped: `SpecialOrder specializes Car.`, `ShippingLabel depends on Order.`

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
