# Use-Case Edge Routing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Produce deterministic, orthogonal, obstacle-aware use-case routes that are materially easier to trace in the three shipped diagrams.

**Architecture:** Keep semantic ordering in the use-case solver, reusable routing mechanics in the generic router, and measured editor geometry in the scene projection. Feed heading strips and measured ports into routing without changing rendering or notation.

**Tech Stack:** Rust, WAML solve pipeline, Makepad editor scene, native screenshot scripts.

## Global Constraints

- Work only in `C:\dev\waml\.worktrees\use-case-edge-routing`.
- Preserve authored layout constraints, actor order, band order, containment, and measured ports.
- Use deterministic bounded algorithms and ASD-STE100 text.
- Use TDD for each behavior change.

---

### Task 1: Adjacency-aware default member order

**Files:**
- Modify: `crates/waml/src/solve/use_case.rs`
- Test: `crates/waml/tests/use_case_layout.rs`

**Interfaces:**
- Consumes: authored band members, ordered actor ranks, relationship endpoint pairs.
- Produces: stable generated `Constraint::Place` order inside each band.

- [ ] Add failing tests for one actor fan-out, two actors with interleaved targets, direct boundary members, and repeated resolves.
- [ ] Run `cargo test -p waml --test use_case_layout` and confirm the new assertions fail for adjacency order.
- [ ] Replace smallest-rank ordering with a stable full adjacency signature while keeping authored ties.
- [ ] Run the focused layout test and existing semantics tests.

### Task 2: Hard obstacles and orthogonal measured ports

**Files:**
- Modify: `crates/waml/src/solve/route.rs`
- Test: `crates/waml/tests/use_case_routing.rs`
- Test: inline tests in `crates/waml/src/solve/route.rs`

**Interfaces:**
- Produces: a route entry point that accepts sorted hard obstacle rectangles.
- Produces: terminal clipping that preserves an orthogonal first and last segment for `PortGeometry`.

- [ ] Add failing tests for hard heading obstacles, actor fan-out lanes, ellipse clipping, obstacle-safe lane separation, and determinism.
- [ ] Run the exact tests and confirm expected failures.
- [ ] Add hard obstacles to the OVG obstacle list and label reroute context.
- [ ] Add deterministic terminal clipping with the minimum elbow needed for orthogonality.
- [ ] Run router and use-case routing tests.

### Task 3: Editor scene integration and real workflow regression

**Files:**
- Modify: `crates/waml-editor/src/scene.rs`
- Test: inline tests in `crates/waml-editor/src/scene.rs`

**Interfaces:**
- Consumes: `SceneGroup::heading_bounds`, measured node `PortGeometry`, router hard-obstacle API.
- Produces: scene edges and labels with stable geometry.

- [ ] Add a crossing counter and failing real Editor Workflows threshold test.
- [ ] Add failing tests for nested heading avoidance, label rerouting, disconnected members, marker direction, and repeated scene solves.
- [ ] Pass use-case heading strips into initial routing and label rerouting.
- [ ] Apply measured terminal clipping after both route passes.
- [ ] Run focused editor scene and relationship tests.

### Task 4: Native visual and full verification

**Files:**
- Update: `crates/waml-editor/tests/screenshots/use-case/*.png`

**Interfaces:**
- Produces: three reviewed native-pixel baselines.

- [ ] Launch through `run.ps1` with the manifest titles and capture with `scripts/capture-window.ps1`.
- [ ] Run the screenshot script with `-Update`, inspect all images, then rerun comparison without `-Update`.
- [ ] Run focused tests, workspace tests, format, Clippy, upgrade check, and `git diff --check`.
- [ ] Request staff review and resolve concrete findings.
- [ ] Rebase on `origin/main`, rerun required checks, and fast-forward local `main` without pushing.
