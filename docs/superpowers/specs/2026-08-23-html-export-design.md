# `waml export html` — a read-only HTML rendering of a bundle

Design, 2026-08-23. Closes audit row **A04** ("Accessibility — ship a plain-HTML
export beside the canvas reader"), and settles a product question the audit
raised separately in **A03** ("who adopts this and why").

## The product decision this rests on

The audit's sharpest accessibility finding was that the flagship "zero-install
reader" paints every glyph into one HTML `<canvas>` through makepad's WebGL
backend. Nothing on the page is DOM text, so screen readers, browser find, and
crawlers all see an empty document. The prose reading view (`reading_view.rs`)
is affected exactly as much as the diagrams are — this was never only a diagram
problem.

**The decision: HTML is the reading surface. The editor is what you link to.**

That inverts the current story, and it is a better one. Today the wasm build is
the front door and its inaccessibility is a defect to be patched. After this,
reading a model needs no install, no JavaScript, and no GPU — it is a web page —
and the wasm editor becomes what it actually is: the application you open when
you want to *change* something.

`waml export site` is not deleted and not changed. It stops being the front
door and becomes the editor's entry point.

### Why this shape and not a fallback

Three shapes were considered:

1. A separate export deployed *instead of* the wasm build. **Chosen.**
2. A separate export deployed *alongside*, at a second path. Rejected: crawlers
   and screen readers only benefit from what is at the address people actually
   reach, so a sidecar fixes archival and print but not reach.
3. One artifact with HTML as the ground layer and the wasm reader hydrating
   over it. Rejected for now: it needs real per-document URLs, which the editor
   has never had — it boots one bundle via `?bundle=` plus a fragment — so it
   pulls a routing redesign in on top of a renderer that does not exist yet.

There is a fourth benefit, and it is the reason to build this before deciding
anything else about the product's scope. **The export is a scope oracle.**
Static HTML can express a concept, its prose, its diagram, and links to other
concepts. It cannot express infinite scroll, live view chains, or projection
masks — there is no JavaScript and nothing to project against. So whatever
survives into HTML is the model as a reader can receive it, and whatever cannot
be represented is precisely the machinery that grew past it (`waml/src/view/*`
at 5,211 lines, `waml-editor/src/book_*` at 2,321, `folder_*` at ~2,245 — about
9,800 lines, of which roughly 4,000 sit in the core `waml` crate rather than the
editor). This design does not propose cutting any of it. It proposes building
the thing that will tell us.

## What already exists

Three measurements were taken before designing, and two of them changed the
design.

**A conformant CommonMark + GFM HTML renderer already exists — in a test file.**
`crates/waml-syntax/tests/markdown_conformance.rs` holds a `Renderer` that emits
HTML and is asserted against the expected HTML of **652 CommonMark 0.31.2
examples and 24 GFM 0.29 examples**, with both corpus sizes pinned by
`conformance_corpus_is_complete` so the oracle cannot silently shrink. The suite
is green.

Crucially, its input is not a test fiction. `conformance_events` calls the
production `parse_markdown`, walks `snapshot.tree().root()` with
`snapshot.queries()`, and maps `node.kind()` through `role_for_kind` into a
three-variant event stream (`Source`, `Start { role, range, metadata }`, `End`).
It is a thin adapter over the production green tree. Promoting the renderer is
therefore a move, not a rewrite — and it corrects a real oddity: a 676-example
specification oracle currently validates code that ships to nobody.

**Diagram geometry is available outside the GPU canvas.** `waml/src/solve/`
computes positions, orthogonally routed edges, and label boxes;
`crates/waml/examples/stress_dump.rs` already writes rects and edge lines to SVG
in 135 lines. Feasibility is not the question — fidelity is.

**Diagram label geometry is welded to IBM Plex.** `waml/src/solve/sizing.rs`
sizes boxes by summing glyph advances from embedded IBM Plex Sans / Mono faces
via `ttf-parser`. Card widths and label boxes are Plex-specific numbers. This is
a hard constraint on typography, recorded below.

**No HTML generation exists in `src/` anywhere.** The only `render_*` functions
in the shipped crates are bundle-envelope JSON/TS emitters and frontmatter YAML
serializers.

## Scope

`waml export html <bundle> --out <dir>` writes a static tree:

- one HTML page per bundle document, `<a href>` between them
- diagrams as inline `<svg>`
- one stylesheet
- embedded web fonts
- **no JavaScript at all**

Out of scope, deliberately:

- **Search.** Browser find and crawlers cover a static site. The export-time
  search index asset stays what it is — an editor optimisation.
- **Any interactivity.** No collapsing, no filtering, no view chains. See "scope
  oracle" above; the absence is the point.
- **Editing.** That is what the link is for.

### The editor link is optional, and one flag gates two outputs

The editor boots from `?bundle=<url>`, so a link into it is only meaningful if a
Bundle Envelope is published beside the HTML. One flag therefore controls both:

- **`--editor <url>` omitted (the default):** pure HTML, CSS, SVG and fonts.
  No link, no `bundle.waml`. The output is self-contained and has no dependency
  on `export site` having been run at all.
- **`--editor <url>` given:** `<url>` is the base the editor is published at,
  absolute or relative to the export root (e.g. `./editor/`). Each page carries
  an "Open in editor" link built from it, and `bundle.waml` is emitted for the
  editor to fetch.

A default export is a folder you can email. Nothing in it can dangle.

## Units

Four units. **A, B and C together ship a complete, useful artifact**; D only
sharpens an optional link, so it can land later without blocking anything.

### Unit A — promote the renderer into `waml-syntax`

Move `Renderer`, `collect_events`, `role_for_kind` and `metadata` from
`tests/markdown_conformance.rs` into `crates/waml-syntax/src/`, behind a public
entry point that takes a parsed snapshot and returns HTML. The conformance test
keeps calling it, unchanged, so the 676-example corpus now gates the shipped
renderer instead of a private copy.

No behaviour change and no new specification risk: the oracle is already
written, already exhaustive, and already passing.

### Unit B — page assembly in `waml::export::html`

Model-aware, sits above Unit A:

- frontmatter to `<title>` and `<meta>`
- inter-document links resolved to output paths through the model's existing
  link resolution, not by string munging
- diagram SVG embedded at its place in the document
- the bundle's `index.md` rendered as the root page
- the optional editor link

This split respects the layering row **A12** just put guards on
(`crates/waml/tests/analysis_layering.rs`): `waml-syntax` knows markdown and
nothing about the model; `waml` knows the model.

### Unit C — diagrams in `waml::export::svg`

Solver geometry to SVG: cards, compartment rules, orthogonal edge polylines,
arrowheads, labels. `stress_dump.rs` is the throwaway proof; this is the real
one, styled deliberately rather than dumped.

**It will not match the editor's rendering, and should not try.** The editor
draws through makepad SDF shaders — the pen ladder, rounded cards, stroke
weights. Reproducing that in SVG is a reimplementation, not a port, and it would
weld two renderers together for every future styling change. The target is a
clean, well-styled UML diagram that is honestly a different rendering of the
same geometry.

**Diagram labels stay IBM Plex Sans / Mono.** Not an aesthetic choice: the boxes
are sized to those advances by `solve/sizing.rs`, so any other face mis-fits
every label. Making diagram labels serif would mean adding a `Serif` variant to
that module's `Font` enum and re-measuring, which changes every diagram's
geometry and would be caught by the ink-comparison rendering gates. Reachable,
mechanical, and not proposed here.

### Unit D — link the editor to a specific document

The only change to existing code. Today `select_initial_document(bundle, uml,
wanted_diagram)` accepts only a *diagram* name and otherwise falls back to the
first concept in `/`; all three web boot paths pass `None`, so `wanted_diagram`
is a native-CLI feature that never reached the browser. Consequently the editor
cannot be linked to a page — it has no page concept at all.

This unit adds a URL parameter on the web boot and widens
`wanted_diagram: Option<&str>` to name a concept as well as a diagram. Touches
`browser_boot.rs`, `cli.rs`, `workspace.rs`.

Without it, an "Open in editor" link still works — it lands on the bundle's
first concept rather than the one being read.

## Typography

**Prose is serif; diagram labels are not.** The constraint above forces the
split, so the design leans into it rather than hiding it.

**Noto Serif** for prose. It is already vendored, at
`crates/waml-editor/resources/fonts/Noto_Serif/` — 74 files with the full weight
range and italics — and it carries its own `OFL.txt` and `README.txt`, as each
of the four vendored families does (Cascadia Code, IBM Plex Mono, IBM Plex Sans,
Noto Serif). So there is **no font to vendor and no new attribution to write**.

Better still, nothing renders with it today. `scripts/prune-web-fonts.mjs` notes
that "the app names exactly 8 of them -- Noto_Serif alone is 36 MB of weights",
and its test asserts the entire `Noto_Serif` directory is deleted from the web
artifact. It is currently dead weight that is never distributed. Two
consequences: using it here cannot change how the editor renders anything, and
the export is the first thing to give those files a purpose.

One doc follow-up: `THIRD-PARTY.md`'s Fonts section describes only the subsets
pruned from the makepad distribution. Once the export ships Noto Serif from
waml-editor's own resources tree, that section needs a line saying so.

**Fonts are embedded as subset woff2** rather than left to a system stack. The
export is meant to be hosted or sent, and a system serif stack renders
differently on every machine, which defeats the point of styling it at all. Three
faces are enough — Regular, Italic, and one bold cut — subset to the glyphs the
bundle actually uses. `scripts/prune-web-fonts.mjs` is prior art for the pruning
step, though it prunes by "what the app names" and the export needs its own
subsetting.

## Testing

| Unit | Gate |
|---|---|
| A | The existing conformance corpus, unchanged — 652 CommonMark + 24 GFM, sizes pinned. |
| B | Golden-file tests over a fixture bundle: page set, link targets, frontmatter projection, the `--editor` on/off split. |
| C | Golden-file SVG over a fixture diagram, asserting structure rather than exact path data where the solver is free to move. |
| D | `select_browser_boot` is pure and host-testable; the existing boot tests are the pattern. |

The CLI surface gets an end-to-end test in `waml-cli/tests/` in the shape of the
existing export tests, including the "no `--editor`, no `bundle.waml`" case,
since a dangling link is the one failure a reader would actually hit.

## Risks

**Two renderers of the same model can disagree.** The editor and the export will
draw the same diagram differently, and prose styling will differ too. This is
accepted (see Unit C), but it means a visual difference is not automatically a
bug, and neither renderer can serve as the other's oracle.

**Unit A's move is only as safe as the corpus is broad.** 676 examples is a
strong oracle for CommonMark and GFM, but the WAML dialect adds semantic roles
on top. The conformance suite's own role-rejection tests
(`canonical_renderer_rejects_wrong_semantic_role`,
`..._missing_semantic_roles`) cover that seam and must move with it.

**The scope-oracle argument cuts both ways.** If a substantial part of the model
turns out not to be expressible as static HTML, that is information, not a
failure of this design — but it will feel like one mid-implementation. It should
be recorded rather than worked around.

## Open

Nothing blocking. The deployment question — whether the HTML export replaces the
wasm build at the Pages root — is deliberately left until the artifact exists
and can be looked at.
