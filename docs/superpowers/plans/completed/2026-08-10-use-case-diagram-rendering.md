# Use-Case Diagram Rendering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render declared `uml.UseCaseDiagram` documents with UML actor, use-case, boundary, band, and relationship notation while the editor keeps its existing structural interaction system.

**Architecture:** Validate group roles once from resolved members and store the role in the projected model. Select a structural visual policy from the declared diagram kind. The policy supplies measurement, default placement, ports, drawing, and relationship notation. The class and use-case policies share the current scene solver, camera, selection, drag, focus, stale projection, and edit transaction systems.

**Tech Stack:** Rust, `waml`, `waml-editor`, Makepad, deterministic layout tests, native Windows screenshot tests, Markdown fixtures.

**Dependency:** Complete `2026-08-10-canonical-uml-diagram-types-and-upgrade.md` first. This plan starts from canonical `DiagramKind::UseCase` dispatch.

## Global Constraints

- Work only in `C:/dev/waml/.worktrees/use-case-diagram-rendering`.
- Use ASD-STE100 Simplified Technical English in code messages and documentation.
- Prefix every shell command with `rtk`.
- Use test-driven development. Run each red test before implementation.
- Do not change the WAML parser or add a lane keyword. Use the existing `Members` and `Layout` grammar.
- Do not infer a diagram kind from its contents. Dispatch from `Diagram.kind`.
- Do not infer group roles from English group titles.
- A top-level group is either an actor group or a system boundary. Do not add a third role.
- An actor group has at least one resolved actor. Its other resolved members can only be notes or packages that resolve to actors, notes, and actor packages.
- A system boundary has at least one resolved use case in its direct members or bands. Its direct members can only be use cases and notes. Its nested groups are bands.
- A note-only or empty top-level group is invalid. A band has at least one resolved use case and can contain only use cases and notes.
- Actor groups cannot have child groups. Bands cannot have child groups.
- Every actor is outside all system boundaries. Every use case belongs to exactly one system boundary.
- Authored layout constraints have priority. Add a default only when the same placement decision is absent.
- Keep top-level group order, band order, and actor member order stable. A crossing-reduction pass can reorder use cases inside one band; for equal scores, use authored member order and then key order.
- Use one measured geometry result for drawing, obstacles, ports, hit testing, focus, and selection outlines.
- Do not draw outside the measured rectangle.
- Keep the current stale valid projection when a use-case edit has a semantic error.
- Do not add relationship kinds, extension-point compartments, automatic category bands, or another interaction system.
- Give every manual editor launch a unique `-Title` value.

---

### Task 1: Validate resolved use-case group roles

**Files:**
- Create: `crates/waml/src/uml/use_case.rs`
- Modify: `crates/waml/src/uml.rs`
- Modify: `crates/waml/src/uml/analysis.rs`
- Modify: `crates/waml/src/model.rs`
- Modify: `crates/waml/src/diagnostic.rs`
- Create: `crates/waml/tests/use_case_semantics.rs`

- [ ] Add red semantic tests for a valid actor group, a valid direct system boundary, a boundary whose use cases are all in valid bands, an actor package, a note-only top-level group, an empty top-level group, a band without a use case, a child group under an actor group, a child group under a band, an actor inside a boundary, a use case outside a boundary, a use case in two boundaries, an incompatible member, an unresolved layout reference, and a valid diagram whose group titles are not English role names.
- [ ] Run `rtk cargo test -p waml --test use_case_semantics`.
  Expected result: invalid groups project or no role information exists.
- [ ] Add this projected model type:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DiagramGroupRole {
    Generic,
    ExternalActors,
    SystemBoundary,
    Band,
}

pub struct DiagramGroup {
    pub name: String,
    pub role: DiagramGroupRole,
    pub members: Vec<String>,
    pub children: Vec<DiagramGroup>,
}
```

- [ ] Add this pure validation interface in `uml/use_case.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UseCaseMemberKind {
    Actor,
    UseCase,
    Note,
    ActorPackage,
    Incompatible,
}

pub(crate) struct UseCaseGroupInput {
    pub name: String,
    pub depth: usize,
    pub members: Vec<(String, UseCaseMemberKind)>,
    pub children: Vec<UseCaseGroupInput>,
}

pub(crate) struct UseCaseGroupVerdict {
    pub role: Option<DiagramGroupRole>,
    pub violations: Vec<UseCaseViolation>,
}

pub(crate) fn classify_group(input: &UseCaseGroupInput) -> UseCaseGroupVerdict;
```

- [ ] Classify a package as `ActorPackage` only when its resolved recursive contents contain at least one actor and contain no use case or incompatible classifier.
- [ ] Classify a system boundary from the recursive use-case membership of its direct members and bands. Do not require a direct use-case member when valid bands contain the use cases.
- [ ] Use these diagnostic codes: `InvalidUseCaseGroup`, `ActorInsideSystemBoundary`, `UseCaseOutsideSystemBoundary`, `UseCaseInMultipleSystemBoundaries`, and `EmptyUseCaseBand`. Keep the existing unresolved-layout-reference diagnostic for bad `Layout` links.
- [ ] Run validation after member link resolution. Store a role only after the verdict is valid. Store `Generic` for class diagrams.
- [ ] Reject the invalid projection so the editor can retain its last valid scene.
- [ ] Run `rtk cargo test -p waml --test use_case_semantics` and `rtk cargo test -p waml --test semantic_diagnostics`.
  Expected result: all role and diagnostic tests pass.
- [ ] Commit with `rtk git add crates/waml/src/uml/use_case.rs crates/waml/src/uml.rs crates/waml/src/uml/analysis.rs crates/waml/src/model.rs crates/waml/src/diagnostic.rs crates/waml/tests/use_case_semantics.rs` and `rtk git commit -m "feat(uml): validate use-case group roles"`.

### Task 2: Add explicit structural visual policy dispatch

**Files:**
- Create: `crates/waml-editor/src/canvas/class/visual.rs`
- Modify: `crates/waml-editor/src/canvas/class/mod.rs`
- Modify: `crates/waml-editor/src/class_diagram_view.rs`
- Modify: `crates/waml-editor/src/uml_documents.rs`
- Modify: `crates/waml-editor/src/doc_view.rs`
- Modify: `crates/waml-editor/src/document_host.rs`
- Modify: `crates/waml-editor/src/scene.rs`
- Create: `crates/waml-editor/tests/use_case_dispatch.rs`

- [ ] Add a red test that opens equal member sets under `uml.ClassDiagram` and `uml.UseCaseDiagram` and gets different structural visual kinds. Add an empty-use-case case.
- [ ] Add a red session test that makes a valid use-case document invalid and confirms that the same use-case view identity keeps the last valid scene.
- [ ] Run `rtk cargo test -p waml-editor --test use_case_dispatch`.
  Expected result: the structural surface has no explicit use-case policy.
- [ ] Add these interfaces:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StructuralVisualKind {
    Class,
    UseCase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeVisualKind {
    ClassCard,
    Actor,
    UseCase,
    Note,
    Package,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupVisualKind {
    Generic,
    ActorRail,
    SystemBoundary,
    Band,
}

pub struct StructuralVisualPolicy {
    pub kind: StructuralVisualKind,
}

impl StructuralVisualPolicy {
    pub fn node_kind(&self, ty: &ElementType) -> NodeVisualKind;
    pub fn group_kind(&self, role: DiagramGroupRole) -> GroupVisualKind;
}
```

- [ ] Make `ClassDiagramView::new` take `(key: String, visual_kind: StructuralVisualKind)`.
- [ ] Put `StructuralVisualKind` in the document view identity and scene. Do not replace a use-case surface with a class surface when the projection is stale.
- [ ] Keep camera, drag, selection, focus, hover, expansion, transaction, and stale-scene ownership in the existing structural surface.
- [ ] Run `rtk cargo test -p waml-editor --test use_case_dispatch` and `rtk cargo test -p waml-editor navigation`.
  Expected result: dispatch and stale-state tests pass.
- [ ] Commit with `rtk git add crates/waml-editor/src/canvas/class/visual.rs crates/waml-editor/src/canvas/class/mod.rs crates/waml-editor/src/class_diagram_view.rs crates/waml-editor/src/uml_documents.rs crates/waml-editor/src/doc_view.rs crates/waml-editor/src/document_host.rs crates/waml-editor/src/scene.rs crates/waml-editor/tests/use_case_dispatch.rs` and `rtk git commit -m "feat(editor): select structural visual policies"`.

### Task 3: Measure actors and use cases once

**Files:**
- Create: `crates/waml-editor/src/canvas/class/use_case_geometry.rs`
- Modify: `crates/waml-editor/src/canvas/class/mod.rs`
- Modify: `crates/waml-editor/src/scene.rs`
- Create: `crates/waml-editor/tests/use_case_geometry.rs`

- [ ] Add red tests for a short actor name, a long actor name, a short use-case title, a wrapped title, the maximum line count, ellipse growth, geometry-contained drawing primitives, hit bounds, and deterministic results.
- [ ] Run `rtk cargo test -p waml-editor --test use_case_geometry`.
  Expected result: actor and ellipse measurements do not exist.
- [ ] Add these types:

```rust
pub enum MeasuredNodeGeometry {
    ClassCard,
    Actor(ActorGeometry),
    UseCase(UseCaseGeometry),
    Note,
    Package,
}

pub struct ActorGeometry {
    pub bounds: Rect,
    pub head_center: Point,
    pub head_radius: f64,
    pub body: Segment,
    pub arms: [Segment; 2],
    pub legs: [Segment; 2],
    pub title_bounds: Rect,
}

pub struct UseCaseGeometry {
    pub bounds: Rect,
    pub title_bounds: Rect,
    pub title_lines: Vec<String>,
}

pub fn measure_node(
    policy: StructuralVisualPolicy,
    node: &Node,
    text: &dyn TextMeasurer,
) -> MeasuredNodeGeometry;
```

- [ ] Put the actor title below the stick figure. Include the complete figure and title in `bounds`.
- [ ] Center bounded wrapped text in the ellipse. Use a fixed maximum line count. Grow the ellipse when the wrapped text needs more room.
- [ ] Ellipsize the final visible line when a title exceeds the maximum line count. Keep the ellipsis and every glyph inside `title_bounds`.
- [ ] Measure every node once before solving. Use `geometry.bounds()` to build the solver size map. After placement, translate that same geometry result to world coordinates and store it on `SceneNode`; do not measure the node again.
- [ ] Derive `SceneNode.rect` from the translated `geometry.bounds()` so drawing, layout, routing, hit testing, and outlines consume the same measurement.
- [ ] Assert in tests that all line segments, the actor title, and the ellipse title are inside `bounds`.
- [ ] Run `rtk cargo test -p waml-editor --test use_case_geometry`.
  Expected result: all geometry and containment tests pass.
- [ ] Commit with `rtk git add crates/waml-editor/src/canvas/class/use_case_geometry.rs crates/waml-editor/src/canvas/class/mod.rs crates/waml-editor/src/scene.rs crates/waml-editor/tests/use_case_geometry.rs` and `rtk git commit -m "feat(editor): measure UML actor and use-case nodes"`.

### Task 4: Add deterministic use-case placement defaults

**Files:**
- Create: `crates/waml/src/solve/use_case.rs`
- Modify: `crates/waml/src/solve/mod.rs`
- Modify: `crates/waml/src/solve/resolve.rs`
- Modify: `crates/waml/src/solve/constrain.rs`
- Create: `crates/waml/tests/use_case_layout.rs`

- [ ] Add red layout tests for actors left of boundaries, stable actor stacks, stable band order, a single row, a balanced multi-row grid, direct boundary members without bands, authored overrides, disconnected members, and equal results across ten runs.
- [ ] Add a crossing test where connected use cases can reorder inside one band but bands never reorder.
- [ ] Run `rtk cargo test -p waml --test use_case_layout`.
  Expected result: generic class placement does not meet the use-case rules.
- [ ] Add this interface:

```rust
pub struct UseCaseLayoutDefaults {
    pub group_shapes: BTreeMap<BoxId, Shape>,
    pub constraints: Vec<Constraint>,
}

pub fn defaults(
    diagram: &Diagram,
    resolved: &solve::Scene,
    relationships: &[(BoxId, BoxId)],
) -> UseCaseLayoutDefaults;

pub fn resolve_use_case(
    diagram: &Diagram,
    relationships: &[(BoxId, BoxId)],
) -> (solve::Scene, Vec<Diagnostic>);
```

- [ ] Start with the existing resolver so authored `Layout` atoms keep their provenance and order.
- [ ] Add a default only when the authored constraints do not decide that axis or group shape.
- [ ] Put actor groups left of system boundaries. Stack actors by authored member order.
- [ ] Put bands in authored child order. Put use cases in a stable row for small sets and a balanced stable grid for larger sets.
- [ ] Pass the already resolved drawable relationship endpoint pairs into the use-case resolver. Use their adjacency to reduce crossings inside a band. Use authored order, then key order, for equal scores. Never change top-level group order or band order.
- [ ] Run `rtk cargo test -p waml --test use_case_layout` and `rtk cargo test -p waml --test layout_atom_api`.
  Expected result: defaults, authored-priority, and determinism tests pass.
- [ ] Commit with `rtk git add crates/waml/src/solve/use_case.rs crates/waml/src/solve/mod.rs crates/waml/src/solve/resolve.rs crates/waml/src/solve/constrain.rs crates/waml/tests/use_case_layout.rs` and `rtk git commit -m "feat(layout): add use-case placement defaults"`.

### Task 5: Project measured nodes, group roles, and shape ports into the scene

**Files:**
- Modify: `crates/waml-editor/src/scene.rs`
- Modify: `crates/waml/src/solve/route.rs`
- Modify: `crates/waml/src/solve/label.rs`
- Create: `crates/waml/tests/use_case_routing.rs`
- Modify: `crates/waml-editor/tests/use_case_geometry.rs`

- [ ] Add red tests that route to actor and ellipse boundaries, keep routes outside measured node bounds, and keep labels outside system-boundary and band headings.
- [ ] Run `rtk cargo test -p waml --test use_case_routing`.
  Expected result: ports clip only to rectangles and headings are not label obstacles.
- [ ] Add these routing types:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum PortGeometry {
    Rectangle(Rect),
    Ellipse(Rect),
    Actor {
        bounds: Rect,
        head_center: (f64, f64),
        head_radius: f64,
        stroke_radius: f64,
        segments: Vec<((f64, f64), (f64, f64))>,
    },
}

pub fn boundary_port(geometry: &PortGeometry, toward: (f64, f64)) -> (f64, f64);
```

- [ ] Translate the pre-solve measured geometry to world coordinates and pass the resulting port map into routing. Do not reconstruct actor or ellipse geometry from a generic solved rectangle.
- [ ] Project a scene group record with its role, bounds, and measured heading rectangle. Use no visible frame for `ActorRail`; use a named frame for `SystemBoundary`; use an ordered named subframe for `Band`.
- [ ] Use measured node bounds as obstacles. Use ellipse intersection for use cases. Compute actor endpoints against the measured head circle and stroked body, arm, and leg segments; do not attach to the actor title or the outer bounds rectangle. Keep the actor title bounds as an obstacle.
- [ ] Add boundary headings, band headings, and already placed edge labels to the label obstacle list.
- [ ] Run `rtk cargo test -p waml --test use_case_routing` and `rtk cargo test -p waml-editor --test use_case_geometry`.
  Expected result: port and obstacle tests pass.
- [ ] Commit with `rtk git add crates/waml-editor/src/scene.rs crates/waml/src/solve/route.rs crates/waml/src/solve/label.rs crates/waml/tests/use_case_routing.rs crates/waml-editor/tests/use_case_geometry.rs` and `rtk git commit -m "feat(scene): project use-case geometry and ports"`.

### Task 6: Draw actor, ellipse, boundary, and band visuals

**Files:**
- Create: `crates/waml-editor/src/canvas/class/render/use_case_nodes.rs`
- Create: `crates/waml-editor/src/canvas/class/render/use_case_groups.rs`
- Modify: `crates/waml-editor/src/canvas/class/render/mod.rs`
- Modify: `crates/waml-editor/src/canvas/class/widget.rs`
- Create: `crates/waml-editor/tests/use_case_render_commands.rs`

- [ ] Add red render-command tests for actor primitives and title, an ellipse and centered wrapped title, an unframed actor rail, a system boundary, ordered band frames, and selection/focus outlines from measured geometry.
- [ ] Run `rtk cargo test -p waml-editor --test use_case_render_commands`.
  Expected result: the renderer emits class cards and generic group frames.
- [ ] Dispatch node and group drawing through `StructuralVisualPolicy`. Keep class drawing unchanged.
- [ ] Draw actors with the stored head and segment coordinates. Draw the title at `title_bounds`.
- [ ] Draw use cases with the stored ellipse bounds and stored title lines.
- [ ] Draw system-boundary and band headings inside their measured heading rectangles. Do not draw an actor-group frame.
- [ ] Draw hover, selection, and focus outlines from the stored geometry. Assert that every emitted primitive stays inside its measured bounds.
- [ ] Run `rtk cargo test -p waml-editor --test use_case_render_commands` and `rtk cargo test -p waml-editor --test use_case_geometry`.
  Expected result: all visual command and bounds tests pass.
- [ ] Commit with `rtk git add crates/waml-editor/src/canvas/class/render/use_case_nodes.rs crates/waml-editor/src/canvas/class/render/use_case_groups.rs crates/waml-editor/src/canvas/class/render/mod.rs crates/waml-editor/src/canvas/class/widget.rs crates/waml-editor/tests/use_case_render_commands.rs` and `rtk git commit -m "feat(editor): draw UML use-case structures"`.

### Task 7: Draw UML use-case relationship notation

**Files:**
- Modify: `crates/waml-editor/src/canvas/class/visual.rs`
- Modify: `crates/waml-editor/src/canvas/class/render/edges.rs`
- Modify: `crates/waml-editor/src/canvas/class/widget.rs`
- Modify: `crates/waml-editor/src/edge_labels.rs`
- Modify: `crates/waml-editor/src/scene.rs`
- Modify: `crates/waml/tests/use_case_routing.rs`
- Create: `crates/waml-editor/tests/use_case_relationships.rs`

- [ ] Add red tests for all four exact notations:

```text
associates  -> solid line, no arrow
includes    -> dashed line, open dependency arrow, «include»
extends     -> dashed line, open dependency arrow, «extend»
specializes -> solid line, hollow triangle at the broader actor or use case
```

- [ ] Add red tests that association navigability does not add an arrow in a use-case view, labels avoid nodes and headings, and all markers meet measured actor or ellipse ports.
- [ ] Run `rtk cargo test -p waml-editor --test use_case_relationships`.
  Expected result: the class edge policy controls the notation.
- [ ] Add this policy result:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeLineStyle {
    Solid,
    Dashed,
}

pub struct EdgeNotation {
    pub line: EdgeLineStyle,
    pub from_marker: Marker,
    pub to_marker: Marker,
    pub middle_label: Option<&'static str>,
}

impl StructuralVisualPolicy {
    pub fn edge_notation(&self, edge: &SceneEdge) -> EdgeNotation;
}
```

- [ ] Reuse the existing `Marker::OpenArrow` for include and extend dependencies, and reuse the existing `Marker::HollowTriangle` for specialization. Keep the generic class-diagram adornment policy unchanged.
- [ ] Add dashed edge drawing to the edge shader. Keep dashed group outlines independent from dashed edges.
- [ ] Place forced middle labels through the existing edge-label collision solver. Treat previous edge-label rectangles as obstacles.
- [ ] Clip the last segment and marker at the stored actor or ellipse port. Point the specialization triangle at the broader endpoint.
- [ ] Run `rtk cargo test -p waml-editor --test use_case_relationships` and `rtk cargo test -p waml --test use_case_routing`.
  Expected result: notation, direction, port, and collision tests pass.
- [ ] Commit with `rtk git add crates/waml-editor/src/canvas/class/visual.rs crates/waml-editor/src/canvas/class/render/edges.rs crates/waml-editor/src/canvas/class/widget.rs crates/waml-editor/src/edge_labels.rs crates/waml-editor/src/scene.rs crates/waml/tests/use_case_routing.rs crates/waml-editor/tests/use_case_relationships.rs` and `rtk git commit -m "feat(editor): render use-case relationships"`.

### Task 8: Reuse structural hit testing and editing behavior

**Files:**
- Modify: `crates/waml-editor/src/canvas/class/interaction.rs`
- Modify: `crates/waml-editor/src/canvas/class/widget.rs`
- Modify: `crates/waml-editor/src/class_diagram_view.rs`
- Create: `crates/waml-editor/tests/use_case_interaction.rs`

- [ ] Add red tests for clicking an actor figure, clicking its title, clicking the edge of an ellipse, dragging a use case, selecting a relationship, keyboard focus, zoom, pan, and stale-scene interaction after a bad edit.
- [ ] Run `rtk cargo test -p waml-editor --test use_case_interaction`.
  Expected result: actor-title or ellipse-boundary hits fail, or a second code path is required.
- [ ] Make `node_at` use `MeasuredNodeGeometry::hit_bounds`. For actor and use-case nodes, this is the same complete measured rectangle used by obstacles and focus.
- [ ] Keep the existing drag transaction and authored layout update. Do not add a use-case-specific edit protocol.
- [ ] Keep the current edge selection, camera, pan, zoom, keyboard focus, and stale projection paths.
- [ ] Run `rtk cargo test -p waml-editor --test use_case_interaction` and `rtk cargo test -p waml-editor navigation`.
  Expected result: all structural interactions work for use-case geometry.
- [ ] Commit with `rtk git add crates/waml-editor/src/canvas/class/interaction.rs crates/waml-editor/src/canvas/class/widget.rs crates/waml-editor/src/class_diagram_view.rs crates/waml-editor/tests/use_case_interaction.rs` and `rtk git commit -m "feat(editor): reuse interactions for use-case views"`.

### Task 9: Add real-document screenshot regression coverage

**Files:**
- Modify: `docs/waml/use-cases/views/editor-workflows.md`
- Verify: `docs/waml/use-cases/views/browser-and-publishing-workflows.md`
- Verify: `docs/waml/use-cases/views/tooling-workflows.md`
- Create: `scripts/check-use-case-diagram-screenshots.ps1`
- Create: `crates/waml-editor/tests/screenshots/use-case/editor-workflows.png`
- Create: `crates/waml-editor/tests/screenshots/use-case/browser-and-publishing-workflows.png`
- Create: `crates/waml-editor/tests/screenshots/use-case/tooling-workflows.png`
- Modify: `crates/waml-editor/tests/README.md`
- Modify: `crates/waml-editor/tests/ui.rs`

- [ ] Confirm that each real workflow document declares `type: uml.UseCaseDiagram` and keeps its existing semantic members.
- [ ] In `editor-workflows.md`, author the two approved nested bands `Create and change` and `Find and understand` under `WAML editor boundary`, and move every existing workflow link into exactly one of them. Keep the existing link order within each band. Do not create bands in the browser/publishing or tooling documents; those two documents deliberately cover direct boundary members.

```text
Create and change:
  Edit Prose
  Interact with an Activity Diagram
  Interact with a Class Diagram
  Interact with a Sequence Diagram
  Open a Bundle
  Route the Edges
  Save and Undo
  Sequence Language
  Solve the Layout
  Use the Shell

Find and understand:
  Browse the Tree
  Fit the Window
  Navigate and Return
  Read a Document
  Report Every Problem
  Select and Inspect
  Work with Tabs
```
- [ ] Add a red native screenshot manifest test in `ui.rs` that requires exactly these three source paths, titles, and baseline paths.
- [ ] Run `rtk cargo test -p waml-editor --test ui use_case_screenshot_manifest`.
  Expected result: the manifest and baselines do not exist.
- [ ] Add this PowerShell interface:

```powershell
param(
    [switch] $Update,
    [double] $MaxChangedPixelRatio = 0.001
)
```

- [ ] For each manifest entry, launch the real `docs/waml` bundle as a child process, keep its process ID, use the existing semantic UI-test navigation to open the exact concept, and pass a unique title: `use-case-editor-workflows`, `use-case-browser-workflows`, or `use-case-tooling-workflows`.
- [ ] Capture the window at native pixels through `scripts/capture-window.ps1 -ProcessId <id>`. Do not capture by process name when other editor windows can exist. Compare dimensions first, then compare pixels. In `-Update` mode, replace the matching baseline only after the target document is visible.
- [ ] Make a missing document, wrong active document, dimension change, or changed-pixel ratio above the limit fail the script.
- [ ] Document the Windows desktop prerequisite and these commands:

```powershell
pwsh -File scripts/check-use-case-diagram-screenshots.ps1 -Update
pwsh -File scripts/check-use-case-diagram-screenshots.ps1
```

- [ ] Run `rtk cargo test -p waml-editor --test ui use_case_screenshot_manifest`.
  Expected result: the manifest names and all three baseline files pass.
- [ ] On a native Windows desktop, run `rtk pwsh -File scripts/check-use-case-diagram-screenshots.ps1 -Update` and then `rtk pwsh -File scripts/check-use-case-diagram-screenshots.ps1`.
  Expected result: all three native HiDPI screenshots match their new baselines.
- [ ] Commit with `rtk git add docs/waml/use-cases/views scripts/check-use-case-diagram-screenshots.ps1 crates/waml-editor/tests` and `rtk git commit -m "test(editor): cover real use-case diagrams"`.

### Task 10: Verify the complete use-case renderer

**Files:**
- Verify: all files changed in Tasks 1-9

- [ ] Run `rtk cargo fmt --all -- --check`.
  Expected result: exit code 0.
- [ ] Run `rtk cargo test -p waml --test use_case_semantics --test use_case_layout --test use_case_routing`.
  Expected result: all semantic, layout, port, and route tests pass.
- [ ] Run `rtk cargo test -p waml-editor --test use_case_dispatch --test use_case_geometry --test use_case_render_commands --test use_case_relationships --test use_case_interaction`.
  Expected result: all editor use-case tests pass.
- [ ] Run `rtk cargo test --workspace`.
  Expected result: all workspace tests pass.
- [ ] Run `rtk cargo clippy --workspace --all-targets -- -D warnings`.
  Expected result: exit code 0 with no warning.
- [ ] Run `rtk pwsh -File scripts/check-use-case-diagram-screenshots.ps1` on a native Windows desktop.
  Expected result: all three screenshots match at native HiDPI resolution.
- [ ] Launch and inspect each real document one at a time with `rtk pwsh -File run.ps1 docs/waml -Title <unique-slug>`. Use `verify-use-case-editor`, `verify-use-case-browser`, and `verify-use-case-tooling` as the three slugs. In each launch, use semantic navigation to open the named workflow document. Close each window before the next launch. The screenshot script already performs native-pixel capture by exact process ID; do not put a capture command after the blocking `run.ps1` command.
- [ ] Confirm visually that actors are outside the boundary, titles do not clip, bands keep authored order, labels avoid headings, dashed arrows have the correct label, and no route crosses a node.
- [ ] Run `rtk git diff --check`.
  Expected result: no whitespace error.
- [ ] Review the diff and confirm that the parser did not change, group roles do not use English names, layout defaults do not replace authored atoms, and the renderer does not duplicate the interaction system.

## Plan Self-Review

- The plan uses the existing `Members` and `Layout` grammar and adds no parser rule or lane keyword.
- The plan has exactly two valid top-level roles. It rejects note-only and empty top-level groups and bands without use cases.
- The plan validates resolved types and containment. It does not use group-title words.
- The plan enforces actors outside boundaries and one boundary for each use case.
- The plan dispatches from `DiagramKind::UseCase`, including empty documents and stale projections.
- The plan uses one measured geometry source for drawing, ports, obstacles, hit testing, focus, and outlines.
- The plan preserves authored layout priority and stable group order. Defaults are deterministic.
- The plan keeps actor order stable and permits only deterministic crossing-driven reordering of use cases inside one band.
- The plan covers actor, ellipse, system-boundary, band, association, include, extend, and specialization notation.
- The plan reuses the structural interaction, camera, edit, and stale-projection systems.
- The plan uses the three real workflow documents for native HiDPI screenshot regression checks.
- The plan adds no new relationship kind, extension-point compartment, automatic band, or placeholder.
