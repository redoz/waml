# The MVP Definition

This document consolidates the MVP into one page set: the scope, the
area-by-area completeness matrix, the ordered gap backlog, the explicit
deferrals, and the definition of done for each area. The [Root
Goal](./root-goal.md) states the bar. The goal tree holds the detail. This
document orders the remaining work against that bar. If this document and a
leaf goal do not agree, the leaf goal is correct; then correct this document.

Every status here comes from the goal tree or from a direct reading of the
code. A claim that no reading supports has the word `UNVERIFIED`.

## Scope statement

**The MVP is:** an author writes and reads `docs/waml` fully in the native
editor, with no text editor, and sends it as a link that a reader opens in a
browser with no installation and no account.

The MVP therefore **is**:

- One bundle format: plain Markdown on disk, lossless through the parser,
  canonical when written, diffable in review.
- One editor with two delivery forms, native and web, that draw the same views.
- Reading: tree, preview tabs, formatted documents, diagrams, navigation
  history, a responsive window down to a telephone width.
- Authoring in the native form: create/rename/move/delete documents, prose
  editing of every construct `docs/waml` uses, typed model edits, direct
  canvas manipulation, save, undo.
- Three diagram kinds at full cut quality: class, sequence, activity — with
  the shared layout, routing, and label machinery below them.
- Trust: lossless round-trip, canonical formatting, correct indexes, resolved
  references, and diagnostics from every layer at every surface.
- Sharing: pack-to-link, the published web artifact, the pages workflow.

The MVP explicitly **is not**:

- State machine and use case diagrams at full quality (they draw through what
  exists; their cuts are `MVP: no`).
- Authoring in the browser, `waml serve`, or any web save-to-disk path.
- Image export (SVG/PNG), the first post-MVP function.
- Layout overrides authored from the canvas, keyboard-only authoring,
  templates beyond the empty tier, effort budgets.
- The language server and the VS Code extension beyond their current state;
  marketplace publication.
- Multi-user, comments, search, cross-bundle links, non-UML typed projections
  (all `horizon`, see [Beyond UML](./beyond-uml.md)).
- Curved edges (orthogonal routing is a decision, not a gap), a third theme,
  other Markdown dialects.

## Completeness matrix

`Bar` states what "complete for MVP" means for the area. `Gaps` lists what
stands between the current state and that bar. Statuses repeat the goal tree;
`unverified` marks survive until the audit pass replaces them with evidence.

| Area | Status | Bar | Gaps |
| --- | --- | --- | --- |
| Language and spec (`docs/uaml-spec.md`, [sequence language](uml/sequence/language.md)) | partial | The written contract and the parser agree; every accepted construct has a scenario a test names. | Sequence language rows not yet implemented (see backlog #4); other kinds have no language document — acceptable, their cuts serve. |
| Parser and formatter (`waml-syntax`) | partial | [Round-Trip Losslessly](trust-the-content/round-trip-losslessly.md), [Format Canonically](trust-the-content/format-canonically.md). | One input format removes authored bytes (backlog #2). Line-end discipline across Windows/CI `UNVERIFIED`. |
| Diagnostics | partial | [Report Every Problem](trust-the-content/report-every-problem.md): every layer's diagnostics reach editor, CLI, and LSP with positions. | Shell- and frontmatter-layer diagnostics are discarded before the public boundaries (backlog #1). |
| References and indexes | partial | [Resolve References](trust-the-content/resolve-references.md), [Keep Indexes Correct](trust-the-content/keep-indexes-correct.md). | Index regeneration exists in the library, no product code calls it (backlog #3). Links in text and fragment links probably unchecked (backlog #12). |
| Class diagrams | partial | [Class cut](uml/class/feature-cut.md): no `planned` row `MVP: yes`. | Operation parameter lists and return types render `partial`; package nesting as containment `partial` (backlog #11). |
| Sequence diagrams | partial | [Sequence cut](uml/sequence/feature-cut.md) against the [language](uml/sequence/language.md). | The largest kind gap: `async`, `signals`, full `returns`, `alt`/`opt`/`loop` fragments, operand guards (backlog #4). Model-layer fragment types exist (`crates/waml/src/model.rs`, `FragmentKind`, `SeqChild`); the vertical slice is in flight. |
| Activity diagrams | partial | [Activity cut](uml/activity/feature-cut.md). | All `MVP: yes` rows read `done — unverified`; the audit pass is the remaining work. |
| Layout solving | partial | [Solve the Layout](uml/shared/solve-the-layout.md): every `docs/waml` diagram solves with no overlap, no clipped content, deterministically. | Verification against this bundle outstanding; sequence substrate must absorb fragments. |
| Layout stability | planned | [Keep the Map Stable](uml/shared/keep-the-map-stable.md): one edit moves only nearby nodes. | Nothing exists; solvers re-solve from scratch (backlog #6). `MVP: yes` and `planned` — a bar blocker. |
| Edge routing | partial | [Route the Edges](uml/shared/route-the-edges.md). | Crossing minimisation and parallel-edge spacing do not operate (backlog #7). |
| Label placement | partial | [Place the Labels](uml/shared/place-the-labels.md). | Solver-placed labels, reroute-under-pressure, and leader lines landed (`3d1dbadd`, `226b8460`, `404c06e4`); zoom-level verification outstanding (backlog #8). |
| Native editor — read | partial | [Read a Bundle](read-a-bundle/): all five leaves `done` with evidence. | Tables in the document view (backlog #12); failure messages on a bad bundle (backlog #15); back/forward scroll-position accuracy `UNVERIFIED`. |
| Native editor — prose | partial | [Edit Prose](author-in-the-editor/edit-prose.md): every construct this bundle uses, with caret, selection, keyboard, clipboard; unchanged bytes survive. | The largest single distance to the bar. Presentation landed; the edit foundation needs an audit before more work is planned (backlog #5). |
| Native editor — model and canvas | partial | [Edit the Model](author-in-the-editor/edit-the-model.md), [Draw on the Canvas](author-in-the-editor/draw-on-the-canvas.md). | Edge-endpoint re-drag unknown to operate; rubber-band multi-select absent (backlog #9). |
| Native editor — documents and save | partial | [Create and Delete Documents](author-in-the-editor/create-and-delete-documents.md), [Save and Undo](author-in-the-editor/save-and-undo.md). | Rename/move `UNVERIFIED`; index correction unwired (backlog #3); multi-file save atomicity weaker than the CLI path (backlog #10). |
| Web form | partial | [Run in a Browser](share-and-publish/run-in-a-browser.md): every native view draws; boot under ~2 s; failures show a message. | Renderer does not draw some diagonal content (backlog #14). Read-only by design for MVP. |
| Share and publish | partial | [Share a Link](share-and-publish/share-a-link.md) `done — unverified`; [Publish a Site](share-and-publish/publish-a-site.md). | Damaged-link message unverified (backlog #15); pages workflow does not gate defects before publication. Link-length ceiling is a watched risk, not a gap. |
| Persistence and bundle format | partial | Envelope v1 codec everywhere; export/open round-trip. | Envelope v1 landed (`ad6de8d8`, migrated `a0022383`). Native multi-file save is the remaining weakness (backlog #10). |
| CLI (`waml-cli`) | done — unverified | [Command-Line Tool](tooling-around-the-repo/command-line-tool.md); `export site` landed (`0c295aae`). | Shares the diagnostics defect (backlog #1). |
| Docs (`docs/waml` itself) | partial | The bundle validates, draws, and its indexes regenerate cleanly. | Architecture index holds hand-written sections a regeneration would remove; `unverified` audit pass across the tree. |

## Gap backlog

Ordered by dependency, then by value. Each item names the plan that covers it
or states that no plan exists. An item with no plan needs one before work
starts.

**Foundation first — trust and language:**

1. **Diagnostics through all layers.** Collect shell and frontmatter
   diagnostics across the parse layers; deliver at editor, CLI, LSP. Highest
   value in the tree: one change corrects three surfaces. *No plan exists.*
2. **Fix the one lossy input format.** Identify and correct the input format
   that removes authored bytes; add it to the round-trip property tests.
   *No plan exists* (which format it is: `UNVERIFIED`).
3. **Wire index regeneration into structure edits.** Call the existing
   library regeneration from create/rename/move/delete transactions; correct
   the architecture index so regeneration is safe. *No plan exists.*
4. **Sequence language completeness.** `signals`, `async`, `returns`
   pairing, `alt`/`opt`/`loop`, operand guards, `outside`, rejection of
   `replies`/`sends`/colon-form. *Plan:*
   `docs/superpowers/plans/2026-08-02-waml-sequence-language-completeness.md`
   (in flight; sequence fixes at the branch tip).

**Then authoring:**

5. **Prose editing to the bar.** Audit the edit foundation first (the goal
   itself warns it is the most probable error in the tree, in either
   direction), then close the distance. *Plans:*
   `2026-07-31-markdown-syntax-platform.md`, `-editor-foundation.md`,
   `-editor-integration.md`, `-bracket-activation.md`,
   `-presentation-motion.md`.
6. **Layout stability.** Seed each solve from the previous solution with a
   movement penalty. Blocks canvas authoring on real diagrams. *No plan
   exists.*
7. **Edge routing quality.** Crossing minimisation and parallel-edge
   spacing. *Plans:* `2026-07-22-orthogonal-edge-router.md`,
   `2026-07-12-straighten-edges-shared-band.md`,
   `2026-08-03-edge-label-route-pressure.md`.
8. **Label placement verification.** Verify no clipped or ownerless label in
   this bundle at every selectable zoom; leader-line fallback covers the
   rest. *Plan:* `2026-08-03-edge-label-route-pressure.md` (routing side).
9. **Canvas edit completeness.** Edge-endpoint re-drag to a new target;
   rubber-band selection; per-operation preview before release. *No plan
   exists.*
10. **Atomic multi-file save in the native editor.** Match the CLI path:
    write every changed file or none; report failure honestly. *No plan
    exists.*

**Then polish within scope:**

11. **Class cut remainders.** Operation parameters and return types;
    package nesting drawn as containment. *No plan exists.*
12. **Document-view tables and in-text link checking.** This tree is mostly
    tables; the bar needs them correct in both forms. *No plan exists.*
13. **Narrow-window verification.** ~390 px usability, threshold
    hysteresis, state preservation across the threshold. *Plan:*
    `2026-07-26-responsive-viewport-chrome.md` (landed in part; verification
    outstanding).
14. **Web renderer diagonal defect.** The web form skips some diagonal
    content; parity is the bar. *No plan exists* (makepad-fork side).
15. **Honest failure messages.** Bad bundle on open; damaged share link.
    Small, verifiable, trust-critical. *No plan exists.*
16. **The audit pass.** Replace every `unverified` in the tree with a
    `file:line` reference or a test name. The remaining marks are the task
    list. *No plan exists; this document is its index.*

## Explicit deferrals

Deferred means: named, out of the bar, and not half-abandoned — each item
below has an owner-goal that records the decision.

| Deferral | Where recorded | Reason |
| --- | --- | --- |
| State machine full cut | [cut](uml/state-machine/feature-cut.md) | The bar needs no lifecycle diagram; the kind draws today through the flow substrate. |
| Use case view (stick figure, ellipse, boundary) | [cut](uml/use-case/feature-cut.md) | Model complete, view absent — presentation-and-layout as one later task. |
| Canvas-authored layout overrides | [Arrange a Diagram](author-in-the-editor/arrange-a-diagram.md) | Constraints exist and solve; the solver default is judged sufficient for this bundle. |
| Template list (Empty/Diagram/Template tiers) | [Start from a Template](author-in-the-editor/start-from-a-template.md) | **Half-built and named:** the new-model and new-project commands log and do nothing. Either wire them to the empty tier or remove the menu rows before MVP; a dead command is worse than no command. |
| Keyboard-only authoring | [Author with the Keyboard](author-in-the-editor/author-with-the-keyboard.md) | The bar needs authoring, not fast authoring. |
| Effort budgets | [Reduce the Effort](author-in-the-editor/reduce-the-effort.md) | Measurement work; second stage. |
| `waml serve` and web save-to-disk | [Serve Locally](share-and-publish/serve-locally.md) | Web authoring is post-MVP; the web form is read-and-share. Plan `2026-07-25-waml-serve.md` stays parked. |
| Image export | [Export a Bundle](share-and-publish/export-a-bundle.md) | First post-MVP function, by decision. |
| LSP live-buffer fix, completion; extension publication | [Language Server](tooling-around-the-repo/language-server.md), [Text Editor Integration](tooling-around-the-repo/text-editor-integration.md) | `MVP: no`; the bar is the editor. The disk-vs-buffer defect is first when this area resumes. |
| Zoom performance and theming polish | [Theme the Diagram](uml/shared/theme-the-diagram.md), [Read a Diagram](read-a-bundle/read-a-diagram.md) | Legible beats fast; per-zoom rasterisation is a known cost. |
| Activity partitions | [Activity cut](uml/activity/feature-cut.md) | `docs/waml` has no multi-role process. |
| Selection pill, Peek machinery, hover-trace and other parked canvas feedback | plan corpus | Dead-but-kept by decision; do not resurrect for the bar. |
| Copy/paste of a subgraph | [Draw on the Canvas](author-in-the-editor/draw-on-the-canvas.md) | Needs single-element operations correct first. |

## Definition of done, per area

A criterion here is a check a person or a CI job can run. "Gate" means
`cargo test --workspace` plus the `editors/vscode` test, lint, and build.

| Area | Done when |
| --- | --- |
| Language/spec | Every scenario in the [sequence language](uml/sequence/language.md) has a passing test that names it. The parser rejects every `excluded` form with a diagnostic. |
| Parser/formatter | Parse-then-write of every document in `docs/waml` changes zero bytes (property test). Formatting canonical content is byte-idempotent on Windows and in CI. |
| Diagnostics | A fixture with one defect per layer (shell, frontmatter, syntax, model, layout) shows all five, with positions, in the editor, in `waml` CLI output, and over LSP. |
| References/indexes | A structure edit in the editor leaves `waml` validation of the bundle clean, including index agreement; a broken link in text produces a positioned diagnostic. |
| Class | Every `MVP: yes` row in the cut is `done` with a `file:line` or test-name evidence entry. |
| Sequence | Same, including `alt`/`opt`/`loop` drawn in both forms for a fixture that nests one fragment. |
| Activity | Same; the audit replaces `unverified` on the already-`done` rows. |
| Layout/routing/labels | Every diagram in `docs/waml` solves with no overlap, no clipped label, edges on borders, and identical output across two runs. One-node edits move only nearby nodes (stability fixture). |
| Native editor | An author performs the full loop on `docs/waml` — create a document, write prose with tables, edit the model, draw on the canvas, save, undo, close-with-prompt — without touching a text editor. |
| Web form | The published artifact draws every view of `docs/waml` that the native form draws (headed-browser screenshot check), boots in under ~2 s, and a damaged link shows a message, not a blank canvas. |
| Share/publish | Pack `docs/waml`, open the link, diff the content: identical. A push to main publishes or fails loudly; the workflow runs the gate first. |
| CLI | Validate/format/query/`export site` against `docs/waml` in CI; non-zero exit with positioned message on a seeded defect. |
| Docs | `docs/waml` itself passes all of the above; zero `unverified` marks remain in the goal tree. |
