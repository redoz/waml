# UI behavior and architecture documentation design

## Purpose

Update `docs/waml` so that it is the current, testable product contract. The
goal tree shall describe all user-visible behavior. The architecture tree
shall describe the current implementation boundaries and data flows. The
product-use-case tree shall show which external roles participate in each
workflow. The content shall use OKF v0.2 and WAML diagrams where a model
explains the subject better than prose.

## Scope

The audit covers every interactive surface:

- the native editor;
- the browser editor and viewer;
- browser boot, share, site, and local-serve interactions;
- editor integrations where they expose distinct user-visible behavior; and
- CLI and language-server interactions that are part of a user workflow.

The native application is the primary test surface. Shared native and browser
behavior has one normative scenario and is tested natively. Browser tests cover
only browser-specific behavior and explicit parity seams.

## Status contract

Shipped behavior on `origin/main` is normative. Stable shipped behavior shall
have Given-When-Then scenarios and implementation or test evidence.

Planned behavior remains in the goal hierarchy, but it shall be marked
`planned` or `horizon`. It shall not read as a passing acceptance contract.

If the implementation and documentation disagree, the audit shall record the
discrepancy. The audit shall not silently change the claimed product behavior
or treat an unimplemented control as shipped.

The allowed goal status values remain `done`, `partial`, `planned`, and
`horizon`. Goal status shall follow scenario coverage and evidence. It shall
not be a first-reading estimate. The out-of-vocabulary value `implemented`
shall be removed.

## Delivery approach

Work has three phases.

### Phase 1: traceability inventory

Create a temporary audit inventory that maps each behavior to:

- a stable behavior identifier;
- its user workflow and owning goal document;
- its applicability: `shared`, `native`, or `browser`;
- shipped, planned, unsupported, or discrepant state;
- implementation and existing-test evidence;
- the required scenario identifier;
- the native or browser verification boundary; and
- a WAML feature gap when the behavior is difficult to express.

The inventory is coordination scaffolding. It is not a second product source
of truth. The final source of truth remains `docs/waml`.

### Phase 2: goal-tree update

Use the inventory to divide the goal tree into non-overlapping subtrees. Each
subtree uses the same template, status rules, scenario style, and evidence
rules. Cross-cutting behavior has one owner. Related documents link to that
owner instead of copying scenarios.

Separate work streams update the implementation architecture, OKF v0.2
metadata conventions, documentation gates, and the WAML feature-gap ledger.

### Phase 3: semantic product-use-case model

After the inventory and documentation contract are frozen and the goal leaves
contain their GWT scenarios, create a permanent model under
`docs/waml/use-cases`. This model shall show external roles, product workflows,
and system boundaries. It shall link to the goal tree and shall not become a
second copy of the behavior contract.

## Scenario design

Each stable scenario shall use this form:

```md
#### <AREA-N> — <lower-case behavior>

**Applies to:** <shared|native|browser>

**Given** <one observable state>
**And** <one additional state, when required>
**When** <one user action>
**Then** <one observable result>
**And** <one additional result, when required>

**Evidence:** <test name or file and line>
```

Scenario identifiers shall be stable and visible in the corresponding test
name or test comment when a test exists. Sentences shall use active voice,
present tense, and ASD-STE100 Simplified Technical English. Scenarios shall use
semantic targets and observable state. They shall not depend on internal Rust
actions or fixed pixel coordinates unless the pixel value is itself the
contract.

The first inventory shall cover at least these behavior areas:

- start, recents, open, close, save, and export;
- responsive shell, docks, splitters, overlays, popups, and theme;
- tree, folders, breadcrumbs, external links, reveal, and view history;
- preview tabs, pinned tabs, document switching, and presentation switching;
- Markdown reading, editing, selection, clipboard, multi-caret, and IME;
- undo, redo, savepoints, dirty state, diagnostics, and status feedback;
- class-diagram selection, tools, direct manipulation, properties, layout,
  conflicts, and solver feedback;
- activity and sequence rendering, hit testing, selection, and camera behavior;
- browser boot, download, URL, share, site, and API-specific behavior; and
- user-visible CLI, LSP, and editor-integration workflows.

## Product use-case design

The permanent product-use-case layer shall use this structure:

```text
docs/waml/use-cases/
  actors/
  workflows/
  views/
```

Create one `uml.Actor` document for each distinct user role or external system
role that interacts across a product boundary. Do not create separate actors
for two names that describe the same role. Use `specializes` from a narrower
actor to a broader actor only when the narrower actor can do all interactions
of the broader actor.

Create one `uml.UseCase` document for each distinct shipped product workflow.
One use-case document shall have one owning goal document. Its `## Owning goal`
section shall link to that goal. Its `## Scenarios` section shall link to every
shipped GWT scenario that the workflow owns. The link target shall be the
scenario heading in the owning goal document. A use-case document shall not
contain `Given`, `When`, or `Then` lines. Planned-only behavior remains in its
goal document until it has a shipped scenario.

Use these relationship meanings:

- `associates` connects an external actor to a use case in which the actor
  participates;
- `includes` points from a use case to another use case that always runs as a
  required part of it;
- `extends` points from an optional or conditional use case to the base use
  case that it extends; and
- `specializes` points from a narrower actor or use case to its broader parent.

Do not add a relationship only because two documents link to the same goal or
because two actions occur in sequence. Author each actor-to-use-case
association once. All relationship targets shall resolve.

Create WAML `Diagram` documents under `docs/waml/use-cases/views`. Each view
shall group external actors separately from the use cases inside its named
system boundary. Use `## Members` groups to express this semantic boundary.
Do not add a `## Layout` section, coordinates, sizes, edge routes, shape names,
or ordering constraints. The model shall not constrain the specialized
use-case view that is under separate implementation.

## Architecture design

The architecture documentation shall separate product concepts from software
implementation. Existing domain-model views remain useful, but they shall not
stand in for the runtime architecture.

Add or revise WAML views for:

1. The six-crate dependency and ownership map.
2. The preparation pipeline from immutable `SourceBundle` through Markdown
   syntax and catalog analysis, OKF lowering, UML analysis and projection,
   `PreparedCandidate`, and atomic snapshot installation.
3. The incremental-update flow, including exact edits, incremental or full
   reparse, affected closure, per-island freshness, quarantine, commit, and
   rejection.
4. Editor ownership across the app shell, `EditorSession`, document host,
   navigation and tabs, Markdown editor, diagram renderers, and platform
   adapters.
5. Deployment and user surfaces across desktop, static WASM, share and site,
   local serve and API, CLI, LSP, and VS Code.

Correct at least these known stale claims:

- The language server is not diagnostics-only. It also provides document
  symbols, links, definitions, and semantic tokens.
- Analysis failure is not one global binary result. The implementation can
  quarantine malformed documents and keep unrelated projections current.
- Semantic edits use prepare-then-commit. They do not mutate the live bundle
  and then restore it after failure.
- Model projection has more than a plain-document and UML two-stage flow.
- The editor and CLI share core services, but they do not provide the same
  operations.

WAML class diagrams shall describe ownership and dependencies. WAML sequence
diagrams shall describe revisioned transactions and commit or reject paths.
WAML activity diagrams shall describe incremental analysis and deployment
flows. Notes and stereotypes shall express invariants that have no first-class
notation.

## OKF v0.2 design

The content shall retain the current OKF-compatible document identities and
typed WAML frontmatter. The update shall define and consistently use OKF v0.2
provenance and freshness fields where they add evidence:

- `sources` for implementation, test, or normative-document provenance;
- `generated` and `verified` for derived trust tiers;
- `status` and `stale_after` for review state and freshness; and
- the retained v0.1 `timestamp` field when required by the format.

Goal status and OKF trust metadata are different concepts. Goal status states
product completion. OKF metadata states document provenance and freshness. The
implementation plan shall define one canonical mapping and shall not duplicate
the same status in conflicting locations.

## WAML feature-gap ledger

Create one linked ledger under `docs/waml` for language features that would
make the contract easier to express. Each entry shall contain:

- the documentation problem;
- a minimal desired notation;
- the current workaround;
- affected scenario or architecture documents; and
- whether the feature is syntax, semantics, rendering, or tooling.

Seed the ledger with the audit findings:

- scenario-level platform and capability predicates;
- reusable typed gestures and input-consumption assertions;
- named view anchors and eventual assertions after a draw cycle;
- ordered collection and state assertions for tabs and selections;
- semantic text positions, multi-caret actions, and IME composition;
- transaction groups and saved-state markers;
- semantic canvas targets and coordinate-space-aware drag paths;
- hit-target, tolerance, and z-order assertions;
- component ports and explicit asynchronous or compare-and-swap notation; and
- traceable links from scenario IDs to tests and evidence.

The ledger records opportunities. This documentation task shall not expand the
WAML language. Specialized stick-figure actor rendering, ellipse use-case
rendering, and system-boundary rendering are under separate implementation by
the user. Do not record them as requirements of this documentation task or as
new language gaps. Scenario-to-use-case-to-test traceability remains a WAML
tooling opportunity.

## Automation and validation

Add the minimum repository gates that keep the contract current:

- `waml check docs/waml`;
- `waml fmt --check docs/waml`; and
- a deterministic generated-index freshness check.

The implementation shall reuse existing CLI and index-generation behavior. It
shall not add a second parser or formatter. CI failure output shall identify
the document and reason. Local contributor instructions shall list the same
commands that CI runs.

Content verification shall include:

- all `docs/waml` documents parse and validate;
- formatting is canonical;
- generated indexes are current;
- scenario identifiers are unique;
- every shipped scenario has evidence and an applicability value;
- planned scenarios cannot be mistaken for passing contracts;
- shared scenarios map to native verification;
- browser scenarios describe browser-only behavior or a parity seam; and
- architecture diagrams resolve their referenced concepts;
- every product use case links to one owning goal and all of its shipped GWT
  scenarios;
- use-case documents contain no copied GWT bodies; and
- product-use-case diagrams contain semantic system-boundary groups and no
  renderer-specific geometry.

## Delegation and integration

After the traceability inventory is complete, parallel agents may own separate
goal subtrees. Agents shall not edit outside their assigned files. One agent
shall own cross-cutting scenario identifiers and deduplication. Separate agents
shall own architecture diagrams, OKF conventions, automation, and feature-gap
documentation. A separate agent shall own the permanent product-use-case model
and shall not edit goal scenario bodies.

Each work stream shall report changed files, scenario identifiers, evidence,
open discrepancies, and feature gaps. A final integration review shall check
cross-tree consistency before the full validation commands run.

## Completion criteria

The work is complete when:

- every discovered user-visible behavior has an owning goal or an explicit
  unsupported, planned, or discrepant record;
- all stable shipped behavior has normative GWT coverage;
- native and browser test ownership follows the approved boundary;
- architecture documents match the current crate boundaries and revisioned
  data flows;
- diagrams replace structural prose where they communicate more clearly;
- OKF v0.2 provenance and freshness conventions are consistent;
- the WAML feature-gap ledger links to the documented problems;
- the product-use-case layer links each external role and shipped workflow to
  its system boundary, owning goal, and GWT scenarios without copying scenario
  bodies;
- the documentation gates pass locally and in CI; and
- a final reviewer finds no unowned behaviors, duplicate contracts, stale
  architecture claims, or unresolved validation errors.
