# WAML OKF/UML architecture documentation

**Date:** 2026-07-27  
**Status:** Approved design

## Purpose and scope

Create a product-architecture documentation bundle at `docs/waml/architecture/`.
The bundle explains the currently implemented WAML system as a set of observable
concepts and interactions: what an OKF bundle is, how it becomes a model and a
view, how users author and validate it, how edits round-trip, how models move
between people and tools, how layout is solved, and how the native editor is
delivered on GitHub Pages.

The audience is a product engineer, technical writer, or agent who needs to
understand or communicate WAML without reading the implementation. The bundle
describes responsibilities and boundaries, not internal code organization. It
is a description of the implemented state, not a roadmap.

## Non-goals

- Documenting crates, packages, functions, files, or source-code ownership.
- Specifying unimplemented capabilities, alternative architectures, or future
  deployment options.
- Replacing the OKF specification, the WAML authoring guide, or end-user help.
- Teaching the complete WAML grammar; views use only the forms already
  supported by the product and link to the authoring reference for syntax.
- Treating browser-canvas behavior as evidence that it is the GitHub Pages
  deployment target. The deployed Pages editor is the native editor compiled to
  WebAssembly.

## Documentation model

The bundle is an OKF-native knowledge bundle. Reusable concept documents state
facts once. Focused view documents link to those concepts and use diagrams only
to show a relationship or a sequence that prose alone would obscure. An index
at every level enables progressive disclosure.

The root index begins with three paths:

1. **Understand the model** — model, validation, and authoring concepts.
2. **Follow a workflow** — authoring, editing, exchange, and layout views.
3. **Run the product** — system context and native GitHub Pages delivery.

A view must not restate a concept's rules or guarantees. It names the involved
concepts, links to them, and explains only the interaction shown by that view.
Conversely, a concept document does not reproduce a workflow diagram.

## Proposed bundle structure

```text
docs/waml/architecture/
├── index.md
├── concepts/
│   ├── index.md
│   ├── okf-bundle.md
│   ├── waml-model-and-views.md
│   ├── validation-and-diagnostics.md
│   ├── editing-and-round-trip.md
│   ├── exchange-and-sharing.md
│   ├── layout-solving.md
│   └── native-web-delivery.md
└── views/
    ├── index.md
    ├── domain-model.md
    ├── system-context.md
    ├── authoring-and-validation.md
    ├── editing-round-trip.md
    ├── import-export-and-share.md
    ├── layout-solving.md
    └── github-pages-deployment.md
```

`index.md` files have no concept frontmatter and list only their direct contents
with the linked document's one-line description. Every other document has OKF
frontmatter with a `type`, `title`, and `description`. Cross-directory links use
explicit relative paths and are checked from the document that owns each link.

## Shared vocabulary and concept documents

### OKF bundle

`concepts/okf-bundle.md` defines the exchange unit: a hierarchical collection
of Markdown documents with YAML frontmatter, standard Markdown links, and
optional indexes. A document's path supplies its bundle identity; frontmatter
declares its kind and display metadata. Documents outside the diagram grammar
remain part of the bundle and are preserved as bundle content.

### WAML model and views

`concepts/waml-model-and-views.md` defines the distinction between the authored
bundle, the resolved model, and views. UML classifiers and relationships describe
the model. A `Diagram` is a curated view over selected members. Behavioral
documents are views of their own behavior: `uml.Sequence` describes an ordered
interaction, while `uml.Activity` and `uml.StateMachine` describe a directed
flow. A view can omit related model elements to stay focused; this does not
remove the underlying relationship from the bundle.

The document also names the implemented UML vocabulary used by the architecture
bundle: classes, interfaces, data types, enums, actors, use cases, packages,
notes, associations, instance specifications, relationships, diagrams,
lifelines, messages, flow nodes, and transitions. It does not add a new
taxonomy.

### Validation and diagnostics

`concepts/validation-and-diagnostics.md` explains that validation evaluates the
bundle as a whole, not merely one open document. It reports malformed supported
syntax, unresolved or inconsistent references, invalid relationship usage,
invalid diagram membership or layout references, and invalid behavioral
references. Diagnostics distinguish errors from warnings. Unknown or
unrecognized content is retained for graceful consumption rather than silently
discarded.

### Editing and round-trip

`concepts/editing-and-round-trip.md` establishes the authoritative edit loop:
an edit changes the authored documents, the documents are serialized in their
canonical form, and the model and views are rebuilt from that bundle. The model
is derived for presentation and queries; it is not regenerated back into source
documents. A successful canonical round trip is stable after formatting, while
unrelated documents remain outside the edit's scope. This concept owns the
meaning of a round trip; workflow views only show where it occurs.

### Exchange and sharing

`concepts/exchange-and-sharing.md` covers three observable exchange modes.
Import accepts Markdown text, individual Markdown or text files, and ZIP
archives, including concatenated documents marked by HTML path comments.
Import can replace or merge the canvas after a model preview. Export creates an
SVG representation of the current visual model, and the share experience can
create a PNG for copying or saving. A share link carries the complete bundle in
the URL fragment, so opening it reconstructs that model without a sharing
service receiving the fragment.

### Layout solving

`concepts/layout-solving.md` states that layout turns a selected diagram's
members, relationships, dimensions, and authored layout constraints into solved
positions and grouping geometry. Authored layout is declarative: placement,
alignment, grouping, and display treatment express intent; the solver determines
the resulting arrangement. Contradictory or unresolved layout references are
diagnosed. Layout is a view concern and does not change domain semantics.

### Native web delivery

`concepts/native-web-delivery.md` describes the delivered surface. The native
WAML editor runs as a desktop application and as a WebAssembly application. The
GitHub Pages publication builds the native editor for the browser, publishes its
WebAssembly, JavaScript glue, and required resources as a static artifact, and
deploys that artifact on pushes to the main branch or by manual dispatch. The
Pages build is deliberately non-threaded because GitHub Pages cannot provide the
headers required for shared browser memory. A runtime shell supplies branded
loading and deployed-version checking.

## Focused UML views

### Domain model — `views/domain-model.md`

This is a `Diagram` view. It shows the stable conceptual relationships among
OKF bundle, WAML model, classifier, relationship, diagram, behavioral document,
and diagnostic. Its only job is to orient readers to the vocabulary and the
bundle-to-model-to-view distinction. It links to `okf-bundle`,
`waml-model-and-views`, and `validation-and-diagnostics` for all definitions.

### System context — `views/system-context.md`

This is a `Diagram` view. It shows the person who authors a bundle, the WAML
editor, the local/document bundle, a recipient of an exported artifact or share
link, and GitHub Pages as the static web host for the native WebAssembly editor.
It shows boundaries and information exchanged, not technical implementation
layers. It links to the exchange and delivery concepts.

### Authoring and validation — `views/authoring-and-validation.md`

This is a `uml.Sequence` view. Its lifelines are the author, the bundle, the
editor, the model/view projection, and diagnostics. The sequence shows authoring
or importing supported documents, bundle-wide validation, presentation of the
model/view when valid enough to build, and diagnostics returned to the author.
The accompanying prose makes clear that warnings can coexist with a built model
and that diagnostics identify the affected document location. It links to the
validation concept instead of enumerating validation rules.

### Editing round trip — `views/editing-round-trip.md`

This is a `uml.Sequence` view. It shows an editor action, the authoritative
bundle update, canonical serialization of the affected authored content, model
and view rebuild, and the updated visual result. Its single message is that
semantic editing starts and ends with documents; the visual model is derived.
It links to `editing-and-round-trip` for the guarantee and limitations.

### Import, export, and share — `views/import-export-and-share.md`

This is a `uml.Activity` flow view. It follows the choices an author can make:
import supported bundle content and choose replace or merge; export the current
visual model as SVG; or build a self-contained URL-fragment share link and later
open it. The flow includes preview before import application and the distinct
PNG sharing branch. It links to `exchange-and-sharing` for file types, output
meaning, and URL-fragment behavior.

### Layout solving — `views/layout-solving.md`

This is a `uml.Activity` flow view. It begins with a selected diagram and its
member geometry, relationships, and authored constraints; it continues through
constraint validation and solving; and it ends in a laid-out view or diagnostics.
It treats explicit layout constraints as input to the solver, never as domain
relationships. It links to `layout-solving` for the semantics.

### GitHub Pages deployment — `views/github-pages-deployment.md`

This is a `uml.Activity` flow view. It begins with a main-branch push or manual
publication request, builds the non-threaded native WebAssembly web artifact,
prunes unused web fonts, brands the artifact, injects the runtime shell, uploads
the static artifact, and publishes it to GitHub Pages. The view ends with a
browser loading the deployed native editor and its version-aware runtime shell.
It links to `native-web-delivery` for the deployment constraints and observable
delivery result.

## Authoring rules

1. Use the smallest supported view form that expresses the relationship:
   `Diagram` for curated structural/context views, `uml.Sequence` for ordered
   interactions, and `uml.Activity` for decisions and process flow. Do not
   introduce ad-hoc UML diagram kinds or invent WAML frontmatter types.
2. Use the currently supported document structures: `## Members` for a Diagram;
   `## Lifelines` and `## Messages` for a Sequence; and `## Nodes` with supported
   node headings and transition bullets for an Activity. Use existing supported
   relationship verbs and message verbs only.
3. Use real, linkable concept documents as diagram members and lifelines. Do not
   use unsupported visual-only notation, freehand arrows, custom stereotypes, or
   prose pretending to be executable WAML grammar.
4. Keep view documents concise. Their body may explain the purpose and reading
   of the view, then link to shared concepts; it must not duplicate their claims.
5. Put a one-sentence `description` in every concept and view frontmatter so the
   parent index can remain a true progressive-disclosure directory.
6. Keep facts in present tense and supported by the currently implemented
   product. A fact that cannot be observed in the product, deployment workflow,
   or existing documentation is excluded.
7. Use a conceptual title and path. A document must never expose an internal
   source module, package, crate, function, or implementation directory as an
   architecture component.
8. Do not include source-code links, symbol references, or code-layout citations.
   Implementation may be consulted to verify a claim during authoring, but it is
   not part of the published architecture bundle.

## Validation strategy

Validation is layered so the written documentation and its embedded WAML views
are both reliable.

1. **Bundle conformance:** check that each non-index document has parseable OKF
   frontmatter with a non-empty `type`, and that indexes contain only
   progressive-disclosure listings.
2. **Link integrity:** verify each architecture-document link resolves within
   the bundle and every index entry points to a direct child. The source format
   can tolerate broken links generally; this curated architecture bundle does
   not introduce them.
3. **WAML view validation:** validate every Diagram, Sequence, and Activity
   against the current WAML validator. Resolve all member, lifeline, transition,
   and layout references; reject unsupported forms rather than weakening a view
   to approximate notation.
4. **Claim audit:** for every substantive assertion, identify the one shared
   concept document that owns it and the implemented behavior that supports it.
   Remove repeated explanations from all views.
5. **Reader-path review:** start at the root index and follow each of the three
   paths. A reader must reach the relevant concept or workflow without having to
   open unrelated documents.

## Acceptance criteria

- `docs/waml/architecture/` is a self-contained OKF-native documentation bundle
  with a root index and the concept/view indexes and documents listed above.
- The root index supports the three progressive-disclosure paths: model,
  workflow, and delivery.
- The bundle includes exactly the seven focused views described above: domain
  model, system context, authoring and validation, editing round trip, import /
  export / share, layout solving, and GitHub Pages deployment.
- Every shared fact has one owning concept document; views link to it instead of
  repeating it.
- The views use only implemented `Diagram`, `uml.Sequence`, and `uml.Activity`
  forms and pass current WAML validation without unsupported grammar.
- The content describes present, observable responsibilities and excludes source
  layout, future plans, and unimplemented behavior.
- The published bundle contains no source-code links, symbol references, or
  code-layout citations.
- The delivery documentation accurately identifies GitHub Pages as a static
  deployment of the non-threaded native WebAssembly editor and explains the
  browser-header constraint that requires this mode.
- The bundle distinguishes model semantics, visual views, canonical document
  round trips, exported images, and self-contained share links without
  conflating them.
