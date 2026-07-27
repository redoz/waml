# WAML OKF/UML Architecture Documentation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a navigable OKF-native documentation bundle that explains WAML's implemented product domain, workflows, system context, and GitHub Pages delivery without documenting source code.

**Architecture:** Every modeled UML element is one focused OKF document. Structural `Diagram` documents reuse those elements, while `uml.Sequence` and `uml.Activity` documents show ordered interactions and process flows. Nested indexes provide progressive disclosure without duplicating concept definitions.

**Tech Stack:** OKF Markdown with YAML frontmatter, WAML UML document grammar, the repository's `target/debug/waml.exe` validator and formatter, PowerShell link checks, Git.

## Global Constraints

- Work only in `C:\dev\waml\.worktrees\docs-okf-architecture`.
- Publish the bundle only under `docs/waml/architecture/`.
- Describe current, observable product and system responsibilities only.
- Do not document crates, packages, functions, source files, internal code ownership, or planned architecture.
- Do not include source-code links, symbol references, or code-layout citations.
- State each substantive fact in one concept document; views link to that owner instead of repeating it.
- Use only supported `Diagram`, `uml.Sequence`, and `uml.Activity` forms.
- Every non-index document has `type`, `title`, and a one-sentence `description`.
- Every diagram member and sequence lifeline is a real, linkable OKF document.
- Indexes contain direct-child navigation and descriptions, with no concept frontmatter.
- Use explicit relative Markdown links and keep every link resolvable.
- Use present tense and no `TBD`, `TODO`, `FIXME`, placeholders, or speculative language.

---

### Task 1: Model vocabulary and domain view

**Files:**
- Create: `docs/waml/architecture/concepts/model/index.md`
- Create: `docs/waml/architecture/concepts/model/okf-bundle.md`
- Create: `docs/waml/architecture/concepts/model/authored-document.md`
- Create: `docs/waml/architecture/concepts/model/waml-model.md`
- Create: `docs/waml/architecture/concepts/model/model-element.md`
- Create: `docs/waml/architecture/concepts/model/classifier.md`
- Create: `docs/waml/architecture/concepts/model/relationship.md`
- Create: `docs/waml/architecture/concepts/model/diagram.md`
- Create: `docs/waml/architecture/concepts/model/behavioral-view.md`
- Create: `docs/waml/architecture/concepts/model/diagnostic.md`
- Create: `docs/waml/architecture/views/domain-model.md`

**Interfaces:**
- Consumes: `docs/specs/OKF_SPEC.md` and `docs/uaml-spec.md` as format references only; neither is linked as a code citation.
- Produces: stable concept paths under `../concepts/model/` for later workflow and system views.
- Produces: the definitions of authored bundle, resolved model, model element, classifier, relationship, diagram, behavioral view, and diagnostic.

- [ ] **Step 1: Create the model concept documents**

  Use one UML node per file. Use `uml.Class` for the conceptual types, `abstract: true` for `Model Element`, and restrained stereotypes such as `document`, `model`, or `view` only when they clarify the box. Each file must include:

  ```markdown
  ---
  type: uml.Class
  title: <Exact conceptual title>
  description: <One sentence>
  ---

  # <Exact conceptual title>

  <A concise responsibility and boundary description.>
  ```

  Add only supported `## Relationships` bullets. Establish these meanings without implementation detail:

  - an OKF Bundle composes one-or-more Authored Documents;
  - a WAML Model depends on an OKF Bundle and contains resolved Model Elements;
  - Classifier and Relationship specialize Model Element;
  - Diagram and Behavioral View depend on the WAML Model but do not replace it;
  - Diagnostic identifies a problem associated with authored content.

  Use valid relationship grammar, including ends only for association-family verbs:

  ```markdown
  ## Relationships
  - composes [Authored Document](./authored-document.md): 1 bundle to 1..* documents
  - depends [OKF Bundle](./okf-bundle.md)
  ```

- [ ] **Step 2: Create the structural domain view**

  `views/domain-model.md` must use:

  ```markdown
  ---
  type: Diagram
  title: WAML Domain Model
  description: Structural view of WAML's authored bundle, resolved model, model elements, views, and diagnostics.
  profile: uml-domain
  ---

  # WAML Domain Model

  ## Members
  ```

  List all Task 1 model concepts with `../concepts/model/<slug>.md` links. Add a short reading guide that links to the owning concepts but does not restate their definitions.

- [ ] **Step 3: Create the model index**

  `concepts/model/index.md` must have no frontmatter. List each direct child once, grouped under a concise heading, using its exact title and one-sentence description.

- [ ] **Step 4: Validate the Task 1 slice**

  Run:

  ```powershell
  target\debug\waml.exe check docs\waml\architecture\concepts\model docs\waml\architecture\views\domain-model.md
  target\debug\waml.exe fmt --check docs\waml\architecture\concepts\model docs\waml\architecture\views\domain-model.md
  rg -n "TBD|TODO|FIXME|PLACEHOLDER|crate|package|function|source file" docs\waml\architecture\concepts\model docs\waml\architecture\views\domain-model.md
  git diff --check
  ```

  Expected: WAML check exits 0; formatter check exits 0; the content scan has no implementation-documentation or placeholder hits; Git reports no whitespace errors.

- [ ] **Step 5: Commit**

  ```powershell
  git add docs/waml/architecture/concepts/model docs/waml/architecture/views/domain-model.md
  git commit -m "docs: add WAML domain model"
  ```

---

### Task 2: Workflow concepts and behavioral views

**Files:**
- Create: `docs/waml/architecture/concepts/workflows/index.md`
- Create: `docs/waml/architecture/concepts/workflows/author.md`
- Create: `docs/waml/architecture/concepts/workflows/editor.md`
- Create: `docs/waml/architecture/concepts/workflows/model-projection.md`
- Create: `docs/waml/architecture/concepts/workflows/validation-and-diagnostics.md`
- Create: `docs/waml/architecture/concepts/workflows/editing-and-round-trip.md`
- Create: `docs/waml/architecture/concepts/workflows/canonical-serialization.md`
- Create: `docs/waml/architecture/concepts/workflows/exchange-and-sharing.md`
- Create: `docs/waml/architecture/concepts/workflows/layout-solving.md`
- Create: `docs/waml/architecture/views/authoring-and-validation.md`
- Create: `docs/waml/architecture/views/editing-round-trip.md`
- Create: `docs/waml/architecture/views/import-export-and-share.md`
- Create: `docs/waml/architecture/views/layout-solving.md`

**Interfaces:**
- Consumes: Task 1 paths `../concepts/model/okf-bundle.md`, `authored-document.md`, `waml-model.md`, `diagram.md`, and `diagnostic.md`.
- Produces: workflow participant paths reused by Task 3, especially `author.md`, `editor.md`, and `exchange-and-sharing.md`.
- Produces: four behavioral views using only supported lifeline, message, node, and transition syntax.

- [ ] **Step 1: Create workflow participant and responsibility documents**

  Create `Author` as `uml.Actor`; create the remaining concepts as `uml.Class` or `uml.DataType` according to whether they represent a responsibility or an information form. Define:

  - the Author creates or imports an OKF bundle and responds to diagnostics;
  - the Editor presents derived views and applies semantic edits to authored documents;
  - Model Projection derives a model/view representation from the current bundle;
  - Validation and Diagnostics evaluates the bundle as a whole, reports errors and warnings, and retains unknown content;
  - Editing and Round Trip makes authored documents authoritative and rebuilds derived views after canonical serialization;
  - Canonical Serialization produces stable supported document form without regenerating source from the model;
  - Exchange and Sharing covers supported import, merge/replace preview, SVG/PNG output, and complete-bundle URL-fragment sharing;
  - Layout Solving turns diagram membership, dimensions, relationships, and declarative layout constraints into view geometry without changing domain semantics.

  Cross-link the Task 1 model concepts with supported relationships. Keep file-format details in `exchange-and-sharing.md` and do not repeat them in its activity view.

- [ ] **Step 2: Create authoring and editing sequences**

  Both view documents use `type: uml.Sequence`, a `## Lifelines` list of real relative links, and supported `calls`, `sends`, and `replies` messages.

  `authoring-and-validation.md` shows:

  1. Author supplies authored content to Editor.
  2. Editor asks Validation and Diagnostics to evaluate the bundle.
  3. Validation and Diagnostics returns diagnostics.
  4. Editor asks Model Projection to derive the current model/view.
  5. Editor presents the view and diagnostics to Author.

  `editing-round-trip.md` shows:

  1. Author performs a semantic edit through Editor.
  2. Editor updates the OKF Bundle.
  3. Editor calls Canonical Serialization.
  4. Editor calls Model Projection.
  5. Editor returns the updated view to Author.

  Prose below each sequence links to its owning workflow concept and explains only how to read the interaction.

- [ ] **Step 3: Create exchange and layout activities**

  Both documents use `type: uml.Activity` and `## Nodes`. Use supported `initial`, `decision`, ordinary, and `final` headings with `transitions to`, `when`, and `else` bullets.

  `import-export-and-share.md` branches from the requested action:

  - import previews supported content and then replaces or merges the bundle;
  - export produces SVG of the current visual model;
  - image sharing produces PNG for copy/save;
  - link sharing encodes the complete bundle into a URL fragment that a recipient can open.

  `layout-solving.md` flows from selected Diagram through input collection, reference/constraint validation, solving, and either solved view geometry or diagnostics. It must state through links—not duplicated prose—that layout is a view concern.

- [ ] **Step 4: Create the workflows index**

  `concepts/workflows/index.md` has no frontmatter and lists only its direct concept children with their exact one-line descriptions.

- [ ] **Step 5: Validate the cumulative bundle**

  Run:

  ```powershell
  target\debug\waml.exe check docs\waml\architecture
  target\debug\waml.exe fmt --check docs\waml\architecture
  rg -n "TBD|TODO|FIXME|PLACEHOLDER|crate|package|function|source file" docs\waml\architecture
  git diff --check
  ```

  Expected: WAML and formatting checks exit 0; scans find no placeholders or code-documentation language; Git reports no whitespace errors.

- [ ] **Step 6: Commit**

  ```powershell
  git add docs/waml/architecture/concepts/workflows docs/waml/architecture/views/authoring-and-validation.md docs/waml/architecture/views/editing-round-trip.md docs/waml/architecture/views/import-export-and-share.md docs/waml/architecture/views/layout-solving.md
  git commit -m "docs: add WAML workflow views"
  ```

---

### Task 3: System context, deployment, navigation, and integrated verification

**Files:**
- Create: `docs/waml/architecture/index.md`
- Create: `docs/waml/architecture/concepts/index.md`
- Create: `docs/waml/architecture/concepts/runtime/index.md`
- Create: `docs/waml/architecture/concepts/runtime/native-editor.md`
- Create: `docs/waml/architecture/concepts/runtime/browser.md`
- Create: `docs/waml/architecture/concepts/runtime/local-bundle.md`
- Create: `docs/waml/architecture/concepts/runtime/share-recipient.md`
- Create: `docs/waml/architecture/concepts/runtime/wasm-web-artifact.md`
- Create: `docs/waml/architecture/concepts/runtime/github-pages.md`
- Create: `docs/waml/architecture/concepts/runtime/native-web-delivery.md`
- Create: `docs/waml/architecture/views/index.md`
- Create: `docs/waml/architecture/views/system-context.md`
- Create: `docs/waml/architecture/views/github-pages-deployment.md`

**Interfaces:**
- Consumes: Task 1's OKF Bundle and authored-document concepts.
- Consumes: Task 2's Author, Editor, and Exchange and Sharing concepts.
- Produces: the final three reader paths from the root index: understand the model, follow a workflow, and run the product.
- Produces: a self-contained, fully indexed bundle with exactly seven focused views.

- [ ] **Step 1: Create runtime and delivery concepts**

  Model Native Editor and GitHub Pages as conceptual responsibilities, Browser as the execution environment, Local Bundle and WASM Web Artifact as information/artifact types, and Share Recipient as `uml.Actor`.

  `native-web-delivery.md` owns these deployment facts:

  - the native editor runs as a desktop application and as WebAssembly;
  - pushes to `main` or manual dispatch start the Pages publication;
  - publication builds the native editor as non-threaded WebAssembly because Pages cannot supply shared-memory browser headers;
  - the static artifact includes WebAssembly, JavaScript glue, and required resources;
  - publication prunes unused fonts, brands the artifact, injects the loading/version runtime shell, uploads it, and deploys it to GitHub Pages.

  Other runtime concepts link to this owner rather than repeating the pipeline.

- [ ] **Step 2: Create the system-context diagram**

  Use `type: Diagram` and `profile: uml-domain`. Include real members for Author, Editor, OKF Bundle, Local Bundle, Share Recipient, Native Editor, Browser, GitHub Pages, and WASM Web Artifact.

  Show only conceptual dependencies and exchanged artifacts through relationships declared on the member documents. The reading guide links to `exchange-and-sharing.md` and `native-web-delivery.md`; it must not name implementation layers.

- [ ] **Step 3: Create the GitHub Pages deployment activity**

  Use `type: uml.Activity` and supported flow syntax. The exact flow is:

  1. initial;
  2. main-branch push or manual dispatch;
  3. build non-threaded native WebAssembly artifact;
  4. prune unused fonts;
  5. brand artifact;
  6. inject loading and deployed-version runtime shell;
  7. upload static Pages artifact;
  8. deploy GitHub Pages;
  9. browser loads the deployed native editor;
  10. final.

  Link to `native-web-delivery.md` for all explanations; the view supplies order only.

- [ ] **Step 4: Create progressive-disclosure indexes**

  `concepts/runtime/index.md` lists its direct runtime children. `concepts/index.md` links to the `model/`, `workflows/`, and `runtime/` indexes. `views/index.md` lists exactly:

  - WAML Domain Model
  - System Context
  - Authoring and Validation
  - Editing Round Trip
  - Import, Export, and Share
  - Layout Solving
  - GitHub Pages Deployment

  The root `index.md` opens with scope: current product architecture, no source-code map. It then presents:

  - **Understand the model** → model concepts and domain model;
  - **Follow a workflow** → authoring, editing, exchange, and layout views;
  - **Run the product** → system context, runtime concepts, and deployment view.

- [ ] **Step 5: Run WAML and formatting validation**

  Run:

  ```powershell
  target\debug\waml.exe check docs\waml\architecture
  target\debug\waml.exe fmt --check docs\waml\architecture
  rg -n "TBD|TODO|FIXME|PLACEHOLDER|crate|package|function|source file" docs\waml\architecture
  git diff --check
  ```

  Expected: both WAML commands exit 0; scans find no prohibited content; Git reports no whitespace errors.

- [ ] **Step 6: Verify every local Markdown link**

  Run this from the worktree root:

  ```powershell
  $docsRoot = (Resolve-Path 'docs\waml\architecture').Path
  $broken = @()
  Get-ChildItem $docsRoot -Recurse -Filter *.md | ForEach-Object {
      $source = $_
      $text = Get-Content -Raw $source.FullName
      [regex]::Matches($text, '\[[^\]]+\]\(([^)#]+)(?:#[^)]+)?\)') | ForEach-Object {
          $target = $_.Groups[1].Value
          if ($target -notmatch '^[a-z]+:' -and $target -notmatch '^#') {
              $resolved = [IO.Path]::GetFullPath((Join-Path $source.DirectoryName $target))
              if (-not (Test-Path $resolved)) { $broken += "$($source.FullName) -> $target" }
          }
      }
  }
  if ($broken.Count) { $broken; exit 1 }
  ```

  Expected: exit 0 with no broken-link output.

- [ ] **Step 7: Audit navigation and claim ownership**

  Confirm manually:

  - every index lists only direct children;
  - all seven view files are reachable from `views/index.md` and the root reader paths;
  - each substantive deployment, exchange, validation, round-trip, and layout claim has one concept owner;
  - views contain only interaction/order reading notes and links, not repeated definitions;
  - no document cites source paths or describes code organization.

- [ ] **Step 8: Commit**

  ```powershell
  git add docs/waml/architecture
  git commit -m "docs: complete WAML architecture bundle"
  ```

