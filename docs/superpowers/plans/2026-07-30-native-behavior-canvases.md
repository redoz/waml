# Native Behavior Canvases Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Native, read-only-with-selection rendering of activity, state-machine, and sequence diagrams in the makepad editor, per the approved spec `docs/superpowers/specs/2026-07-30-native-behavior-canvases-design.md`.

**Architecture:** Two new pure layout modules in the core crate (`waml::solve::flow`, `waml::solve::interaction`) that turn the already-populated runtime model into geometry, golden-tested headlessly; one new editor surface family (`canvas/behavior/` + `behavior_doc_view.rs`) that draws the solved geometry through promoted shared SDF pens, reusing the existing viewport/hit machinery, wired in at the dead seam `uml_documents.rs:99`.

**Tech Stack:** Rust, makepad (redoz fork), ttf-parser sizing (`solve::sizing`), existing orthogonal router (`solve::route`).

**The spec is the authority.** Every task below references spec sections; the implementer must read the referenced section before implementing. Do not consult or port the deleted Svelte/TypeScript renderers — they are not a reference (spec §8).

## Global Constraints

- Gate after EVERY task: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`. Run it from the worktree root before every commit.
- The gate promotes `dead_code` to a hard error **in the editor binary crate**. No task may land an editor-side item nothing consumes. Items in `crates/waml` (a lib crate) are exempt when `pub` — this is why the solver tasks (2–5) can land before their editor consumers (7–8).
- Layout is deterministic: fixed iteration counts, stable sorts, `BTreeMap` ordering, no randomness, no HashMap iteration order anywhere in solver output (spec §2.3, §3).
- Edge routing is straight + orthogonal only, never splines.
- makepad landmines (all from repo memory, restated in the tasks where they bite):
  - A custom widget used as a DSL child is dead+invisible unless its `script_mod(vm)` registers BEFORE the consuming module's.
  - A `mod.X` script namespace must be created by ONE object-literal assignment, never field-by-field.
  - Inline `font_size:` / `FontMember` is gate-banned in chrome; a new text style needs a `mod.fonts` role, which moves `fonts.rs` + `script_gate.rs` + `fonts_overlay.rs` together. **This plan reuses existing font roles only** — no new role is needed.
  - `sdf.box(..., 0)` degenerates and floods; sharp corners use `sdf.rect`.
  - A one-shot `draw_walk` on a child view must be looped to done (`while view.draw_walk(cx, scope, walk).is_step() {}` pattern) or it leaves a begun-never-ended turtle that silently blanks sibling widgets.
  - Manual hit rects captured during `draw_walk` are PRE-alignment; events arrive POST-alignment. Translate by the event-area-to-draw-area delta (spec §5.1).
- Visual sign-off (tasks 6–9): launch THIS worktree's own `scripts/run-native.ps1`, capture and kill **by specific pid in one PowerShell call** — never by process name (it kills the user's own editor).
- Commit after each green task with a conventional-commit message. Do not push (the dispatcher/workflow owns git transport).

---

## File map (who owns what)

| Path (repo-relative) | Role |
| --- | --- |
| `crates/waml/src/solve/flow.rs` | flow solver: resolve, measure, rank/order/x, partitions, routing glue, diagnostics, `pretty_flow` |
| `crates/waml/src/solve/interaction.rs` | interaction solver: columns, rows, activations, fragments, `pretty_interaction`, all `Solved*` interaction types |
| `crates/waml/tests/fixtures/behavior/{activity,state-machine,sequence-nested}/*.md` | authored-fresh fixtures (spec §6.1) |
| `crates/waml/tests/flow_solver_golden.rs`, `interaction_solver_golden.rs` | goldens + invariant tests (spec §6.2) |
| `crates/waml-editor/src/canvas/primitives.rs` | promoted shared SDF pens (from `class/render/primitives.rs`) |
| `crates/waml-editor/src/canvas/behavior/mod.rs` | `BehaviorSurface` widget + `BehaviorSurfaceAction` + `script_mod` |
| `crates/waml-editor/src/canvas/behavior/scene.rs` | `BehaviorScene` (`Empty` / `Flow` / `Interaction`) |
| `crates/waml-editor/src/canvas/behavior/hit.rs` | `BehaviorTarget` + pure hit-test |
| `crates/waml-editor/src/canvas/behavior/render/{mod,flow,interaction}.rs` | glyph renderers (spec §4) |
| `crates/waml-editor/src/behavior_doc_view.rs` | `BehaviorDocView: DocView` |
| `crates/waml-editor/src/uml_documents.rs` | two new `open` arms (the dead seam, line 99) |
| `crates/waml-editor/src/app.rs` | DSL slot `behavior_canvas_wrap` + `script_mod` registration |
| `crates/waml-editor/src/doc_view.rs` | `BodyWidgets` behavior-canvas accessors |
| `crates/waml-editor/src/edge_labels.rs` | small pure mid-route/label helper reused by flow render |

---

### Task 1: Promote shared SDF primitives to canvas/primitives.rs

**Goal:** Pure refactor — move the kind-agnostic drawing helpers out of `canvas/class/render/primitives.rs` up to `canvas/primitives.rs` so a second surface can consume them, with zero behavior change (spec §1.2).

**Files:**
- Create: `crates/waml-editor/src/canvas/primitives.rs`
- Modify: `crates/waml-editor/src/canvas/mod.rs` (add `mod primitives;` + `pub(crate) use`), `crates/waml-editor/src/canvas/class/render/primitives.rs` (shrinks to class-coupled leftovers or is deleted), every `class/render/*` importer of the moved items.

**Interfaces:**
- Consumes: nothing new.
- Produces (for tasks 7–8): `pub(in crate::canvas)` (or `pub(crate)`) `font_raster_size(target_size: f32) -> f32`, `fill_rect(cx, pen, rect, color)`, `world_rect_to_screen(viewport: ViewportSnapshot, rect: waml::solve::Rect) -> Rect`, plus every other helper in the current file that does not reference class-only types.

**Split rule (this is the whole task):** anything in the current `class/render/primitives.rs` that references `crate::canvas::class::*` (e.g. `ClassDrawResources`, which borrows the class widget's pens, and `node_screen_rect`, which takes `PlacementSnapshot`) or `crate::scene::Scene` **stays under `class/`** (move it into `class/render/mod.rs` or a small `class/render/resources.rs`). Everything else — the generic viewport/pixel helpers and SDF pen utilities — moves to `canvas/primitives.rs` with visibility widened from `pub(in crate::canvas::class)`/`pub(super)` to `pub(in crate::canvas)`.

- [ ] **Step 1: Read the full current file** `crates/waml-editor/src/canvas/class/render/primitives.rs` and list generic vs class-coupled items per the split rule.
- [ ] **Step 2: Create `canvas/primitives.rs`** with the generic items, register `mod primitives;` in `canvas/mod.rs`.
- [ ] **Step 3: Repoint every import** in `canvas/class/render/*` (grep for the moved names) to `crate::canvas::primitives::…`. Leave class-coupled items where the split rule says.
- [ ] **Step 4: Run the gate.** `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`. dead_code will fire if any moved item lost its consumer — fix by keeping the item's existing consumers pointed at the new path, never by adding `#[allow]`.
- [ ] **Step 5: Commit** `refactor(editor): promote shared canvas SDF primitives out of class/`

**Done when:** gate green; `git diff --stat` shows a move + import repoints and no logic edits; the class diagram surface is untouched behaviorally (no golden or widget test changed).

---

### Task 2: Behavior fixtures + solve::flow layout (rank, order, lanes, pretty_flow)

**Goal:** The flow solver's layout half — resolve/measure/rank/order/x/partitions per spec §2.1–§2.4 — plus the activity and state-machine fixtures and layout goldens (spec §6.1–§6.2). Routing is Task 3.

**Files:**
- Create: `crates/waml/src/solve/flow.rs`; `crates/waml/tests/fixtures/behavior/activity/*.md`; `crates/waml/tests/fixtures/behavior/state-machine/*.md`; `crates/waml/tests/flow_solver_golden.rs`
- Modify: `crates/waml/src/solve/mod.rs` (add `pub mod flow;` and the `pretty_flow` free function next to `pretty` at `mod.rs:157`)

**Interfaces:**
- Consumes: `Solved`, `SolvedGroup`, `Rect`, `Size`, `SizeMap`, `FlagSet` (`solve/mod.rs:74-85`); `FlowDoc`/`ActivityNode`/`FlowEdge`/`FlowNodeKind`/`FlowFlavor` (`model.rs:583/466/530/413/455`); `solve::sizing::{text_width, ascent, descent, Font, PT_TO_LPX}`.
- Produces (for tasks 3, 7):

```rust
// crates/waml/src/solve/flow.rs
pub struct FlowConfig {
    pub row_gap: f64,        // 56.0
    pub node_gap: f64,       // 32.0
    pub lane_pad: f64,       // 24.0
    pub pad_x: f64,          // 14.0  text padding inside Plain/Object
    pub pad_y: f64,          // 10.0
    pub font_size: f64,      // 13.0 * sizing::PT_TO_LPX
    pub line_height: f64,    // 18.0
    pub diamond_min: f64,    // 36.0
    pub bar_thickness: f64,  // 6.0
    pub bar_min_len: f64,    // 80.0
    pub initial_r: f64,      // 9.0
    pub final_r: f64,        // 11.0
}
impl Default for FlowConfig { /* the values above */ }

/// Resolved (node, edge) views over the pools, in FlowDoc declaration order.
/// Public because tests and Task 3 both need the resolve step.
pub struct ResolvedFlow<'a> {
    pub nodes: Vec<&'a ActivityNode>,
    pub edges: Vec<&'a FlowEdge>,          // local-target edges only
    pub off_page: Vec<&'a FlowEdge>,        // to_ref set, no local target (spec §2.1)
}
pub fn resolve_flow<'a>(doc: &FlowDoc, nodes: &'a [ActivityNode], edges: &'a [FlowEdge])
    -> (ResolvedFlow<'a>, Vec<Diagnostic>);

/// Measured size per node pool key, from the §2.2 table. Both the editor and
/// the golden tests call this so metrics never drift.
pub fn measure_flow(nodes: &[&ActivityNode], flavor: FlowFlavor, cfg: &FlowConfig) -> SizeMap;

/// The whole result of solving a flow. ONE entry point — see the dispatcher
/// resolution of open question 4. `solved.routes` and `off_page` are empty in
/// this task and are populated by Task 3; `reversed` is populated here because
/// cycle-breaking happens here.
pub struct FlowSolution {
    pub solved: Solved,
    pub diagnostics: Vec<Diagnostic>,
    /// Edge keys whose direction was reversed for ranking; the renderer still
    /// draws the arrowhead at the TRUE target (spec §2.3).
    pub reversed: std::collections::BTreeSet<String>,
    pub off_page: Vec<OffPageStub>,
}

pub fn solve_flow(doc: &FlowDoc, nodes: &[ActivityNode], edges: &[FlowEdge],
                  sizes: &SizeMap, cfg: &FlowConfig) -> FlowSolution;
```

`FlowSolution` **and `OffPageStub`** are both declared in THIS task — `off_page:
Vec<OffPageStub>` cannot compile otherwise. `OffPageStub`'s shape is given in Task 3, which
populates it; declare it here exactly as shown there. Both are `pub` in the library crate,
so the gate's `dead_code` promotion does not apply (see the "why solvers land first" note
above). `solved.routes` and `off_page` stay empty until Task 3 fills them.

and in `solve/mod.rs`:

```rust
/// Deterministic dump of a solved flow: `pretty(solved)` plus one line per route:
/// `route <source> -> <target> : x,y x,y ...` (coords `{:.0}`), in `routes` order.
pub fn pretty_flow(solved: &Solved) -> String;
```

`pretty` itself is NOT changed (spec §1.1 — existing class goldens stay byte-identical).

#### Fixtures (spec §6.1)

Author fresh markdown behavior documents. **The behavior DSL syntax is demonstrated in `crates/waml/tests/uml_behavior_syntax.rs`** — read it first and copy its frontmatter/heading/transition forms exactly. Required content:

- `activity/` — one activity with: at least two named partitions plus one partition-less node; a decision with two guarded out-edges and an `else` edge; a fork/join pair; an `object` node typed by `object_ref`; a loop back-edge (an edge from a later node back to an earlier one).
- `state-machine/` — a `FlowFlavor::StateMachine` doc with: states carrying `entry`/`do`/`exit` lines; transitions with `trigger` and `guard`; one self-transition (`from == to`).

Tests load fixtures with `waml::source::SourceBundle` + `waml::analysis::prepare_candidate(source, None, 1)` (see the existing test at `crates/waml-editor/src/uml_documents.rs:124-138` for the incantation), then read `model.activity_nodes` / the flow docs off the uml analysis (populated at `analysis.rs:1707`).

#### Algorithm (implement exactly spec §2.3–§2.4)

1. Cycle-break: DFS from `Initial` nodes; else in-degree-0 nodes; else first declared node. Reversed back-edges are recorded in `FlowSolution.reversed` (a `BTreeSet<String>` of edge keys), which this task returns and Task 3 plus Task 7 consume.
2. Rank by longest path; ranks are rows top-to-bottom, y = accumulated row heights + `row_gap`.
3. In-rank order: DFS-discovery seed, then exactly 4 barycenter sweeps (down, up, down, up), then one adjacent-transpose pass. Stable sorts only.
4. x: pack left-to-right by measured width + `node_gap`, then a bounded (e.g. 3) number of priority passes toward adjacent-rank barycenters.
5. Partitions (spec §2.4): each distinct `ActivityNode.partition` → vertical lane band in first-appearance order, `partition: None` → trailing implicit unlabelled lane; node x clamped into its lane; lanes emitted as `SolvedGroup { rect, shape: <the Shape variant the class group renderer uses for frames>, title, depth: 0 }`.

Diagnostics in this task (spec §2.7 subset): unreachable node; `Decision` with zero guarded out-edges; empty flow document. Never fatal — always return the best `Solved` you have.

- [ ] **Step 1: Write the fixtures** under `crates/waml/tests/fixtures/behavior/activity/` and `.../state-machine/`, verifying against `uml_behavior_syntax.rs` that they parse and populate `model.activity_nodes` (write a smoke test first: load fixture, assert node/edge counts and kinds).
- [ ] **Step 2: Write the failing golden + invariant tests** in `crates/waml/tests/flow_solver_golden.rs`:

```rust
// Shared helper: fixture path -> (FlowDoc, Vec<ActivityNode>, Vec<FlowEdge>) via prepare_candidate.
// Then, per fixture:
#[test] fn activity_fixture_layout_golden() {
    let (doc, nodes, edges) = load("activity");
    let (rf, _) = resolve_flow(&doc, &nodes, &edges);
    let sizes = measure_flow(&rf.nodes, FlowFlavor::Activity, &FlowConfig::default());
    let sol = solve_flow(&doc, &nodes, &edges, &sizes, &FlowConfig::default());
    assert!(sol.diagnostics.is_empty(), "{:?}", sol.diagnostics);
    let solved = &sol.solved;
    // Inline expected string, matching the existing solver_golden.rs style (crates/waml/tests/solver_golden.rs:56-60):
    assert_eq!(pretty_flow(solved), EXPECTED_ACTIVITY_GOLDEN);
}
#[test] fn ranks_are_monotone_along_non_reversed_edges() { /* rank(from) < rank(to): recompute rank from y-band per node */ }
#[test] fn nodes_lie_inside_their_partition_band() { /* every node rect within its lane SolvedGroup rect */ }
#[test] fn no_overlapping_node_rects_within_a_rank() { /* pairwise x-interval disjointness per row */ }
#[test] fn solving_twice_is_byte_identical() { assert_eq!(pretty_flow(&a), pretty_flow(&b)); }
#[test] fn decision_without_guards_diagnoses_but_still_solves() { /* synthetic model, diags non-empty, solved.nodes non-empty */ }
#[test] fn empty_flow_doc_diagnoses_and_returns_empty_solved() { }
```

The golden strings follow the existing `solver_golden.rs` inline-expected style (`crates/waml/tests/solver_golden.rs:56-60`).
- [ ] **Step 3: Run tests, verify they fail** (module doesn't exist yet): `cargo test -p waml --test flow_solver_golden`.
- [ ] **Step 4: Implement** `solve/flow.rs` (resolve → measure → rank → order → x → lanes) and `pretty_flow` in `solve/mod.rs`, per the algorithm block above.
- [ ] **Step 5: Make all tests pass**, filling the golden expected strings from actual output ONLY after eyeballing the numbers for sanity (monotone y per rank, positive sizes, lanes ordered).
- [ ] **Step 6: Run the full gate**, then **commit** `feat(solve): flow solver layout + behavior fixtures + goldens`

**Done when:** gate green; both fixtures solve with zero diagnostics; all seven listed tests pass; `pretty` (class) untouched.

---

### Task 3: solve::flow routing — orthogonal routes, self-edges, back-edges, off-page stubs

**Goal:** Fill `Solved.routes` for flow diagrams via the existing router plus the two flow-specific route shapes, and the remaining flow diagnostics (spec §2.5, §2.7).

**Files:**
- Modify: `crates/waml/src/solve/flow.rs`, `crates/waml/tests/flow_solver_golden.rs` (routes appear in existing goldens — regenerate them deliberately), fixtures unchanged.

**Interfaces:**
- Consumes: `solve::route::route(boxes, rects, edges, cfg)` (`route.rs:21` — note it **skips self-edges itself**, so flow must route them); `FlowSolution.reversed`, populated by Task 2.
- Produces (for task 7): no new types and no new entry point. This task FILLS fields that
  Task 2 already declared — `FlowSolution.solved.routes` and `FlowSolution.off_page`. Both
  `FlowSolution` and `OffPageStub` (shape below, for reference) were declared in Task 2,
  because `FlowSolution.off_page: Vec<OffPageStub>` cannot compile without it:

```rust
/// Declared in Task 2, populated here. A short outbound route from the source
/// node border plus the target document title to letter the chevron with
/// (spec §2.1, §4.1).
pub struct OffPageStub {
    pub edge_key: String,
    pub points: Vec<(f64, f64)>,   // 2-3 points, leaving the source border
    pub target_title: String,       // resolved from to_ref's document title, else the raw `to` text
}
```

`solve_flow`'s signature does NOT change in this task — it already returns `FlowSolution`.
There is exactly one flow entry point across the whole plan (dispatcher resolution of open
question 4).

#### Behavior (spec §2.5)

- Normal edges: build the `Box`/`BoxId::Node` obstacle list from the solved node rects (mirroring how `solve_diagram_reported` calls `route::route` at `solve/mod.rs:209`) and route reversed edges **in their reversed direction** but store `Route{source,target}` as the TRUE direction so the renderer's arrowhead lands at the true target.
- Self-edges (`from == to`): synthesize a 5-point orthogonal loop out the node's right border into a side channel (`node.right + node_gap/2`) and back in — the router skips these, flow owns them.
- Reversed (loop) back-edges: route outside the rank stack — add the whole rank-stack bounding column as extra clearance by routing via a channel `x = max_right + node_gap` (left channel if the source is left of center). Keep it deterministic and simple; A* with the node obstacles already avoids nodes, the channel waypoint just biases it outside.
- Endpoint-on-border invariant: after routing, clamp/adjust both terminal points onto the respective node rect border (mirror the two-pass `connect_ends` lesson — verify endpoints land ON the border, not merely orthogonal to it).
- Remaining diagnostic: edge naming an unknown local id with no `to_ref` → dropped + `Diagnostic` (spec §2.1/§2.7).

- [ ] **Step 1: Write the failing tests** (extend `flow_solver_golden.rs`):

```rust
#[test] fn every_route_endpoint_lies_on_its_node_border() {
    // for each route: first point on border of solved.nodes[source], last on border of nodes[target]
    // border test: point on rect perimeter within 0.5px epsilon
}
#[test] fn self_transition_routes_out_and_back() { /* state-machine fixture: route exists for the self edge, >= 4 points, all outside the node interior except endpoints */ }
#[test] fn loop_back_edge_routes_outside_the_rank_stack() { /* activity fixture loop edge: some route x beyond all node rects, and Route.source/target are the TRUE direction */ }
#[test] fn unknown_target_without_to_ref_drops_with_diagnostic() { /* synthetic model */ }
#[test] fn cross_document_edge_becomes_off_page_stub() { /* synthetic: to_ref set -> in FlowSolution.off_page, not in routes */ }
```

- [ ] **Step 2: Verify they fail**, implement per the behavior block, **regenerate the Task 2 goldens** (routes now included via `pretty_flow`) with an explicit commit-message note.
- [ ] **Step 3: All flow tests pass; run the full gate.**
- [ ] **Step 4: Commit** `feat(solve): flow edge routing, self/back edges, off-page stubs`

**Done when:** gate green; endpoint-on-border test passes on BOTH fixtures; goldens contain `route` lines; determinism test still passes.

---

### Task 4: solve::interaction — columns, rows, messages, activations

**Goal:** The interaction solver's core: lifeline columns, time rows, message geometry, activation bars derived from the calls/replies stack, create/destroy lifetimes (spec §3.1–§3.4), with its own output type and dump.

**Files:**
- Create: `crates/waml/src/solve/interaction.rs`; `crates/waml/tests/fixtures/behavior/sequence-nested/*.md`; `crates/waml/tests/interaction_solver_golden.rs`
- Modify: `crates/waml/src/solve/mod.rs` (add `pub mod interaction;`)

**Interfaces:**
- Consumes: `SequenceDoc`/`SeqNode`/`SeqEdge`/`SeqChild`/`MessageVerb`/`FragmentKind` (`model.rs:747/701/679/668/603/637`); `solve::sizing`; `Rect`, `Size`, `SizeMap`.
- Produces (for tasks 5, 8): the exact types from spec §1.1 — `InteractionConfig`, `SolvedLifeline`, `SolvedActivation`, `SolvedMessage`, `SolvedFragment` (+ `SolvedOperand { divider_y: Option<f64>, guard: Option<String>, guard_rect: Rect }`), `SolvedInteraction`, and:

```rust
pub struct InteractionConfig {
    pub column_gap: f64,   // 48.0
    pub row_gap: f64,      // 40.0
    pub head_pad_x: f64,   // 14.0
    pub head_pad_y: f64,   // 10.0
    pub bar_width: f64,    // 12.0
    pub nesting_step: f64, // 6.0
    pub frame_inset: f64,  // 12.0
    pub font_size: f64,    // 13.0 * sizing::PT_TO_LPX
    pub line_height: f64,  // 18.0
}
pub fn measure_interaction(doc: &SequenceDoc, cfg: &InteractionConfig) -> SizeMap;
    // keys: lifeline id -> head box size; message id -> label size
pub fn solve_interaction(doc: &SequenceDoc, sizes: &SizeMap, cfg: &InteractionConfig)
    -> (SolvedInteraction, Vec<Diagnostic>);
pub fn pretty_interaction(solved: &SolvedInteraction) -> String;
```

`pretty_interaction` format (fix it now so goldens are stable): one line per lifeline `lifeline <id> head @ x,y wxh stem x=<stem_x> <top>..<bottom>[ destroyed]`, per activation `activation <lifeline> d<depth> @ x,y wxh[ unclosed]`, per message `message <id> <verb> <from_x>-><to_x> y=<y>[ self]`, per fragment `fragment <id> <kind> d<depth> @ x,y wxh` + per operand `  operand y=<divider_y|start> guard=<guard|else>`, all coords `{:.0}`, in the struct's natural (already deterministic) order. Fragments emit nothing until Task 5 fills them.

#### Fixture (spec §6.1)

`sequence-nested/`: four lifelines; nested `calls`/`replies` (A calls B, B calls C, C replies, B replies); one async `sends`; a `creates` of a fourth lifeline and a later `destroys` of it; an `alt` fragment containing a nested `opt`. Syntax reference: `crates/waml/tests/uml_behavior_syntax.rs`. Loaded the same way as Task 2's fixtures; interactions populate at `analysis.rs:1972`.

#### Algorithm (implement exactly spec §3.1–§3.4, §3.6)

Single pass, no iteration. Columns from lifeline declaration order (model guarantees lifelines first, `model.rs:755-757`); handle = alias else title. Rows by walking root `items`, descending into fragments (header row, then per operand a guard row + its items, then closing padding); row height = `max(row_gap, measured label height)`. Activations per the §3.3 stack rules (calls push on target, replies pop, sends none, creates places the head at the row, destroys ends the stem + flags `destroyed`, leftovers close at bottom flagged `unclosed`). Self-messages occupy two rows with `self_loop: Some(rect)` (spec §3.4). Diagnostics (§3.6 subset for this task): unknown lifeline handle (message dropped, diagnosed); `replies` with no open `calls` (drawn, diagnosed, nothing popped); uninvolved lifeline (column still drawn, no diagnostic needed unless spec says — it says column still drawn, list it as a diagnostic per §3.6).

- [ ] **Step 1: Write the fixture** + a parse smoke test (lifeline count 4, message verbs present, fragment kinds Alt+Opt).
- [ ] **Step 2: Write the failing tests:**

```rust
#[test] fn sequence_fixture_golden() { /* pretty_interaction vs expected string */ }
#[test] fn activation_nesting_is_contained_and_depth_matches_stack() {
    // child bar rect.y..y+h within parent's; depth == number of enclosing bars on the same lifeline
}
#[test] fn creates_target_stem_starts_at_its_row_and_destroys_ends_it() { /* SolvedLifeline stem_top/bottom vs the creates/destroys message y; destroyed flag set */ }
#[test] fn self_message_occupies_two_rows() { /* next message's y >= self y + 2*row height's worth */ }
#[test] fn reply_without_open_call_diagnoses_but_draws() { }
#[test] fn unknown_handle_message_is_dropped_with_diagnostic() { }
#[test] fn interaction_solve_is_deterministic() { /* byte-identical dumps */ }
```

- [ ] **Step 3: Verify failure, implement, pass.** Fragments may solve to empty `fragments: vec![]` this task (fields are `pub` in a lib crate — no dead_code risk).
- [ ] **Step 4: Full gate, commit** `feat(solve): interaction solver core — columns, rows, activations, lifetimes`

**Done when:** gate green; all seven tests pass; the golden's numbers eyeballed sane (columns strictly increasing x, rows strictly increasing y).

---

### Task 5: solve::interaction fragments — frames, operands, dividers, guards

**Goal:** Fragment rects computed bottom-up with enclosure, operand dividers and guard label rects, plus the remaining fragment diagnostics (spec §3.5, §3.6).

**Files:**
- Modify: `crates/waml/src/solve/interaction.rs`, `crates/waml/tests/interaction_solver_golden.rs` (goldens regenerate to include `fragment` lines).

**Interfaces:**
- Consumes/Produces: fills the already-declared `SolvedFragment` / `SolvedOperand`; no signature changes.

#### Behavior (spec §3.5)

After row assignment, bottom-up per fragment: rect vertically spans header row through last descendant row + closing padding; horizontally from leftmost to rightmost involved lifeline stem (transitively, through nested fragments), padded by `frame_inset`; nested frames inset by `frame_inset` relative to the parent when spans coincide, `depth` = nesting depth. Operand 2..n contribute a dashed divider at their start y (`divider_y`); every operand gets a guard label rect at its top-left, text = `[guard]` or `[else]` when `guard: None` (`model.rs:726`). Diagnostics: fragment with zero operands; operand with empty item stream.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test] fn fragment_encloses_every_descendant_message_and_nested_frame() {
    // for each fragment: every message row y of its subtree inside rect; every nested fragment rect inside, inset by >= frame_inset
}
#[test] fn alt_second_operand_has_divider_and_else_guard() { /* fixture's alt: operands[1].divider_y.is_some(), guard None -> renders "[else]" (assert guard field None + guard_rect non-empty) */ }
#[test] fn fragment_with_zero_operands_diagnoses() { /* synthetic */ }
#[test] fn empty_operand_stream_diagnoses() { /* synthetic */ }
```

- [ ] **Step 2: Verify failure, implement, regenerate the Task 4 golden** (now with `fragment` lines), keep the determinism test green.
- [ ] **Step 3: Full gate, commit** `feat(solve): interaction fragment frames, dividers, guards`

**Done when:** gate green; enclosure test passes on the nested alt/opt fixture; goldens include fragments.

---

### Task 6: Behavior surface scaffold, view seam, empty state

**Goal:** The kind-agnostic editor seam (spec §1.2–§1.3, §5.3): `BehaviorSurface` widget with pan/zoom + empty-state render, `BehaviorDocView`, the two `uml_documents.rs` arms, DSL slot + script_mod, and the regression test that started this. After this task, opening an activity/state-machine/sequence document shows a live (empty-state) canvas instead of a classifier preview.

**Files:**
- Create: `crates/waml-editor/src/canvas/behavior/mod.rs`, `behavior/scene.rs`, `behavior/hit.rs`, `behavior/render/mod.rs`, `crates/waml-editor/src/behavior_doc_view.rs`
- Modify: `crates/waml-editor/src/canvas/mod.rs` (`mod behavior;` + re-exports + include behavior's registration in `crate::canvas::script_mod`), `crates/waml-editor/src/uml_documents.rs:99` (two arms), `crates/waml-editor/src/app.rs` (DSL slot beside `canvas_wrap` at `app.rs:357`), `crates/waml-editor/src/doc_view.rs` (`BodyWidgets` accessors + visibility toggle), `crates/waml-editor/src/main.rs`/module registry (add `mod behavior_doc_view;`)

**Interfaces:**
- Consumes: `Camera`/viewport machinery (`canvas/viewport.rs` — reused unchanged, spec §1.2), `canvas/geometry.rs`, `canvas/primitives.rs` (Task 1), `DocView` trait (`doc_view.rs:294`), `BodyChrome`, `NavCategory::{Behavior,Sequence}` (`document.rs:28-29`).
- Produces (for tasks 7–9):

```rust
// behavior/scene.rs
pub(crate) enum BehaviorScene {
    Empty { message: String },                       // §5.3 explicit empty state
    Flow { flavor: FlowFlavor, solution: waml::solve::flow::FlowSolution,
           nodes: BTreeMap<String, ActivityNode>, edges: BTreeMap<String, FlowEdge> },
    Interaction { doc_key: String, solved: waml::solve::interaction::SolvedInteraction },
}
// behavior/hit.rs (spec §5.1)
pub(crate) enum BehaviorTarget {
    FlowNode(String), FlowEdge(String), Lifeline(String), Message(String), Fragment(String),
}
pub(crate) fn hit_test(scene: &BehaviorScene, world: (f64, f64)) -> Option<BehaviorTarget>;
// behavior/mod.rs
pub(crate) struct BehaviorSurface { /* Widget: camera, scene, hover, selected */ }
pub(crate) enum BehaviorSurfaceAction { Selected(Option<BehaviorTarget>), Cleared }
impl BehaviorSurface {
    pub(crate) fn set_scene(&mut self, cx: &mut Cx, scene: BehaviorScene);
    pub(crate) fn set_interaction_enabled(&mut self, cx: &mut Cx, enabled: bool);
}
// behavior_doc_view.rs
pub struct BehaviorDocView { /* key, kind */ }
impl BehaviorDocView {
    pub fn flow(key: String) -> BehaviorDocView;
    pub fn interaction(key: String) -> BehaviorDocView;
}
impl DocView for BehaviorDocView { /* sync/handle/chrome/... */ }
```

In THIS task `BehaviorScene::Flow`/`Interaction` variants are **not yet declared** — declare only `Empty` (a binary crate: an unconstructed variant/fields = dead_code). Tasks 7 and 8 each add their variant together with its renderer and constructor. `hit_test` on `Empty` returns `None` and is unit-tested as such.

#### Behavior

- **Widget:** mirror `ClassDiagramSurface`'s pan/zoom/event skeleton (mouse drag pan, wheel/pinch zoom via `Camera::zoom_at`, fit-on-scene via `Camera::fit` with `FIT_PAD`), drawing only the Atlas background + a centered empty-state message ("No renderable elements — N diagnostics" or the scene's `message`) through an existing body font role — **no new font role, no inline `font_size:`**.
- **DSL slot:** in `app.rs` add, as a sibling of `canvas_wrap` inside `center_stack` (`app.rs:346-365`):

```
behavior_canvas_wrap := View{
    width: Fill height: Fill flow: Overlay visible: false
    behavior_canvas := BehaviorSurface{ width: Fill height: Fill }
}
```

- **script_mod landmine:** register the behavior widget's `script_mod(vm)` inside `crate::canvas::script_mod` (already called at `app.rs:2470`, BEFORE the app DSL evaluates) so the DSL child is alive. If behavior needs its own script namespace, create it with ONE object-literal assignment.
- **BodyWidgets:** add `behavior_canvas(&self, cx) -> WidgetRef`, `set_behavior_canvas_visible(cx, bool)` mirroring `canvas_wrap` handling, and extend `set_canvas_interaction_enabled` to also reach `BehaviorSurface` (it currently downcasts only `ClassDiagramSurface`, `doc_view.rs:76-83`). All consumed this task by `BehaviorDocView`.
- **BehaviorDocView:** `chrome()` = `BodyChrome { tool_dock: false, view_bar: true, canvas_overlays: false, document_header: DocumentHeaderChrome { breadcrumb: true, right_dock: Some(Icon::SlidersHorizontal) } }` (spec §1.3: tool dock hidden). `on_activate`/`on_deactivate` toggle `behavior_canvas_wrap` visible / class `canvas_wrap` hidden and back. `sync` sets `BehaviorScene::Empty` for now. Mirror `ClassDiagramView` (`class_diagram_view.rs`) structure minus every authoring path.
- **Seam:** `uml_documents.rs:99` becomes a `match presentation.category` with `Diagram` → existing, `Behavior` → `BehaviorDocView::flow`, `Sequence` → `BehaviorDocView::interaction`, else classifier preview.

- [ ] **Step 1: Write the failing regression test** in `uml_documents.rs` tests (spec §6.3), following the existing test at `:124`:

```rust
#[test]
fn behavior_and_sequence_open_as_behavior_views() {
    let source = SourceBundle::try_from_pairs([
        ("checkout.md", "---\ntype: Activity\n---\n# Checkout\n"),   // copy exact frontmatter from uml_behavior_syntax.rs
        ("ordering.md", "---\ntype: Sequence\n---\n# Ordering\n"),
    ]).unwrap();
    let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
    let doc = open(prepared.okf(), prepared.uml(), "checkout").unwrap();
    assert_eq!(doc.presentation.category, NavCategory::Behavior);
    // downcast-free check: BehaviorDocView reports view_bar-only chrome, unlike ClassifierPreviewView
    assert!(doc.view.chrome().view_bar);
    let seq = open(prepared.okf(), prepared.uml(), "ordering").unwrap();
    assert_eq!(seq.presentation.category, NavCategory::Sequence);
    assert!(seq.view.chrome().view_bar);
}
```

(Adjust the frontmatter to whatever `uml_behavior_syntax.rs` actually uses — verify before writing.)
- [ ] **Step 2: Write the hit-test-on-empty unit test** in `behavior/hit.rs` (`hit_test(&Empty{..}, (10.0, 10.0)) == None`).
- [ ] **Step 3: Verify both fail, then implement** everything in the Behavior block above.
- [ ] **Step 4: Full gate.** Expect dead_code pressure: every accessor/field added must already be consumed by `BehaviorDocView` or the widget itself.
- [ ] **Step 5: Visual sign-off (spec §6.4):** launch this worktree's `scripts/run-native.ps1` on a behavior fixture project, screenshot BY PID in one PowerShell call, confirm: opening an activity document shows the empty-state canvas (message text visible, Atlas background, no blank surface, sibling chrome intact — a begun-never-ended turtle blanks siblings, check the caption/panels), pan/zoom respond. Kill by that pid.
- [ ] **Step 6: Commit** `feat(editor): behavior surface scaffold + view seam + empty state`

**Done when:** gate green; regression test passes; sign-off screenshot shows the empty state and intact chrome.

---

### Task 7: Flow render — UML glyphs, routes, labels, hit-test, hover/select

**Goal:** Draw solved flows (both flavors) per the spec §4.1 glyph table, with hover/click selection (spec §5.1–§5.2 minus inspector wiring, which is Task 9). Adds `BehaviorScene::Flow` together with its renderer and constructor.

**Files:**
- Create: `crates/waml-editor/src/canvas/behavior/render/flow.rs`
- Modify: `behavior/scene.rs` (add `Flow` variant), `behavior/hit.rs` (FlowNode/FlowEdge targets), `behavior/mod.rs` (draw + hover/select events, emit `BehaviorSurfaceAction::Selected`), `behavior_doc_view.rs` (`sync` builds the flow scene), `crates/waml-editor/src/edge_labels.rs` (one small pure helper)

**Interfaces:**
- Consumes: `waml::solve::flow::{solve_flow, measure_flow, resolve_flow, FlowSolution, FlowConfig, OffPageStub}` (tasks 2–3); `canvas/primitives.rs` pens (Task 1); `accent::tree_kind_color(NavCategory::Behavior)` bucket; existing chrome font roles.
- Produces (for task 9): selection state on `BehaviorSurface` + `BehaviorSurfaceAction::Selected(Option<BehaviorTarget>)` actions.
- New pure helper in `edge_labels.rs` (spec §2.6 — reuse this code path rather than duplicating placement math):

```rust
/// Mid-route label anchor for a plain polyline (kind-agnostic; the class path
/// keeps its SceneEdge-typed entry points).
pub fn mid_route_label(points: &[(f64, f64)], text: String) -> Option<EdgeLabel>;
```

consumed immediately by the flow renderer for `trigger [guard] / effect` / `else` / carried-type text.

#### Behavior

- `BehaviorDocView::sync` (Behavior kind): find the `FlowDoc` for `self.key` in the uml analysis model; `resolve_flow` → `measure_flow` (flavor from the doc) → `solve_flow` → `BehaviorScene::Flow`. Empty/fully-diagnosed doc → `BehaviorScene::Empty` with a diagnostic-count message (spec §5.3).
- Renderer (`render/flow.rs`), one pass over the scene in world space through `world_rect_to_screen`, per the §4.1 table:
  - `Plain`: rounded rect (SDF `sdf.box` with a REAL radius, e.g. 6.0 — never 0), Behavior accent-bucket fill wash, title in the existing heading role; `entry:`/`do:`/`exit:` as body-role lines (state-machine flavor labels them; activity flavor rarely has them but draws the same).
  - `Object`: sharp rect via `sdf.rect`, title + `:Type` line (type name resolved from `object_ref` via the model, else omit).
  - `Decision`/`Merge`: diamond via an explicit 4-point SDF path (move_to/line_to/close) — never a zero-radius box.
  - `Fork`/`Join`: solid filled bar (accent ink).
  - `Initial`: filled disc (SDF circle). `Final`: bullseye — ring + filled inner disc.
  - Partition lanes: `SolvedGroup` bands styled like the existing class group renderer (dim border + title).
  - Routes: solid orthogonal polylines through the class edge-pen technique; open arrowhead at the TRUE target (respect `FlowSolution.reversed`). Object-flow edges add the carried-type label via `mid_route_label`.
  - Off-page stubs: short stub polyline + chevron + `target_title` text.
  - Refinement affordance: small footer marker (e.g. a rake/glyph via primitives) on `Plain` nodes with `refines.is_some()`.
- Hit-test (spec §5.1): store world-space rects (node rects, route segments with a tolerance band, e.g. 6px world) in the scene; `hit_test` checks nodes topmost-first then edges. Events translate local→world through the camera; the widget translates event coords by the event-area-to-draw-area delta before hit-testing (aligned-parent trap).
- Hover: track under-cursor target off `MouseMove` (not `FingerHover` — child-claiming trap) and tint; click sets `selected`, emits `Selected(Some(t))`; click on empty space emits `Selected(None)`.

- [ ] **Step 1: Write the failing hit-test tests** (pure, headless — spec §6.3) in `behavior/hit.rs`:

```rust
#[test] fn flow_hit_prefers_node_over_edge_under_it() { /* build a tiny BehaviorScene::Flow by hand with one node rect + a route passing under it */ }
#[test] fn flow_edge_hits_within_tolerance_band() { /* point 4px off a segment -> FlowEdge; 20px off -> None */ }
```

- [ ] **Step 2: Verify failure; implement scene variant, renderer, hit, hover/select, `sync`, and the `mid_route_label` helper (with its own small unit test asserting the anchor is the route midpoint).**
- [ ] **Step 3: Full gate.** dead_code check: every new pub(crate) item is consumed (renderer by widget, helper by renderer, action by doc view's `handle` — have `handle` at least consume `Selected` into `statusbar_dirty` for now so nothing dangles).
- [ ] **Step 4: Visual sign-off (spec §6.4), BOTH flavors:** run this worktree's editor on projects containing the Task 2 activity and state-machine fixtures; per kind: screenshot by pid, confirm every glyph class from the §4.1 table that the fixture exercises actually draws (partitions visible, diamond is a diamond, fork bar, bullseye final, routes orthogonal with arrowheads at true targets, guard/else labels, self-transition loop, off-page stub if present), hover tint + click selection visibly change state. Kill by pid. Layout goldens can NOT stand in for this — they pass on a surface that draws nothing.
- [ ] **Step 5: Commit** `feat(editor): flow render — UML glyphs, routes, labels, selection`

**Done when:** gate green; hit tests pass; two-flavor visual sign-off screenshots confirm real drawing.

---

### Task 8: Interaction render — lifelines, activations, messages, fragments, hit-test

**Goal:** Draw solved interactions per the spec §4.2 glyph table with the same hover/select behavior. Adds `BehaviorScene::Interaction` with its renderer and constructor.

**Files:**
- Create: `crates/waml-editor/src/canvas/behavior/render/interaction.rs`
- Modify: `behavior/scene.rs` (add `Interaction` variant), `behavior/hit.rs` (Lifeline/Message/Fragment targets), `behavior_doc_view.rs` (`sync` for the Sequence kind)

**Interfaces:**
- Consumes: `waml::solve::interaction::{solve_interaction, measure_interaction, SolvedInteraction, InteractionConfig}` (tasks 4–5); primitives; `accent::tree_kind_color` for the referenced classifier's bucket (lifeline `ref_` resolved via the model, fallback bucket when unresolved).
- Produces: nothing new beyond the variant; Task 9 consumes the same `Selected` actions.

#### Behavior

- `sync` (Sequence kind): find the `SequenceDoc` for `self.key` (`model.interactions`, populated at `analysis.rs:1972`) → `measure_interaction` → `solve_interaction` → `BehaviorScene::Interaction`; empty → `Empty` state.
- Renderer per §4.2: head cards (compact rect, title + `:Ref`, accent bucket); dashed vertical stems (dash by segment loop through the shared pens — check `primitives.rs`/class edge pens for an existing dashed technique first); activation bars as thin filled rects, accent at low alpha, x offset `depth * nesting_step`; `destroyed` stems end in an X (two SDF strokes). Messages: `calls` solid line + solid filled arrowhead; `sends` solid + open head; `replies` dashed + open head; `creates` dashed terminating AT the created head box; `destroys` solid terminating in the X; self-loops draw the `self_loop` rect's three outer sides; label = `signature` else the verb, centered above the line (label rect comes solved). Fragment frames: stroked `sdf.rect` outline, pentagon tab (explicit 5-point SDF path) carrying the kind keyword, dashed operand dividers, guards as `[g]` / `[else]` at the solved guard rects. `unclosed` activations draw with an open (strokeless) bottom edge.
- Hit-test priority (spec §5.1): Message beats Fragment; an activation bar resolves to its **Lifeline**; head + stem (tolerance band) → Lifeline; frame border band → Fragment; else None.

- [ ] **Step 1: Write the failing hit-test tests:**

```rust
#[test] fn message_beats_enclosing_fragment() { /* hand-built SolvedInteraction: point on a message y inside a fragment rect -> Message */ }
#[test] fn activation_bar_resolves_to_its_lifeline() { }
#[test] fn fragment_border_hits_fragment_but_interior_empty_space_does_not() { }
```

- [ ] **Step 2: Verify failure; implement variant, renderer, hit arms, `sync`.**
- [ ] **Step 3: Full gate** (dead_code: all consumed as in Task 7).
- [ ] **Step 4: Visual sign-off (spec §6.4):** run on a project containing the `sequence-nested` fixture; screenshot by pid; confirm four columns, nested activation bars visibly offset, dashed reply vs solid call, the created lifeline's head mid-diagram, the destroy X, the alt frame with pentagon tab + divider + `[else]`, the nested opt inset, self-loop if present; hover + select respond. Kill by pid.
- [ ] **Step 5: Commit** `feat(editor): interaction render — lifelines, activations, messages, fragments`

**Done when:** gate green; hit tests pass; sequence visual sign-off confirms real drawing of every §4.2 glyph the fixture exercises.

---

### Task 9: Selection wiring — inspector, View Source, Esc, diagnostics surfacing

**Goal:** Complete spec §5.2–§5.3: selection drives the inspector panel and the View Source tab; `Esc` clears; solver diagnostics reach the existing diagnostic channel/status bar; final all-three-kinds visual sign-off.

**Files:**
- Modify: `crates/waml-editor/src/behavior_doc_view.rs` (the bulk), `crates/waml-editor/src/canvas/behavior/mod.rs` (`clear_selection`, `Esc`/`Cleared` path), possibly `crates/waml-editor/src/inspector.rs` (only if `subject_from`/`Subject` needs a behavior arm — prefer reusing the existing `Subject` machinery over inventing one)

**Interfaces:**
- Consumes: `BehaviorSurfaceAction::Selected` (tasks 7–8); `crate::inspector::{subject_from, Subject}` and the inspector sync pattern used by `ClassDiagramView` (read its `sync`/`handle` in full first); the View Source path the node context menu already uses (memory: View Source tab renders subject markdown — find it from `ClassDiagramView`'s handling of the context-menu action and `markdown_surface`); `ViewOutcome::navigation` / `promote_subject` for tab behavior.
- Produces: the finished feature; nothing downstream.

#### Behavior

- `BehaviorDocView::handle`: consume `Selected(target)` → map the target to the underlying subject key (FlowNode → its `ActivityNode.key` pool key; FlowEdge → edge pool key; Lifeline → resolved `ref_` classifier key when present else the interaction doc; Message/Fragment → the interaction doc key with the element noted) → push the inspector subject exactly the way `ClassDiagramView` does; `Selected(None)`/`Cleared` empties it.
- View Source: selection's subject markdown renders through the same code path the node context menu already uses (spec §5.2), but **do NOT add a context menu to this surface** (dispatcher resolution 2 — a right-click menu is scope the spec does not grant a read-only surface). Reach that rendering path from the selection/inspector affordance instead: a selected target sets the View Source subject exactly as the class view does once its context menu has fired. If the class view's only entry to that path is its context-menu action handler, factor the action's *body* into a small function and call it from the selection handler; do not replicate the menu.
- `DocView::on_escape` → `clear_selection` on the surface.
- Diagnostics: forward solver `Vec<Diagnostic>` through the same channel the class view uses (find where `ClassDiagramView`/session pushes diagnostics to the status bar) so the count in the empty-state message and the status bar agree.
- No mutation reaches the document from this surface — assert by construction: `BehaviorDocView` never sets `ViewOutcome::edit`.

- [ ] **Step 1: Read `class_diagram_view.rs` in full** (the pattern to mirror, minus authoring) and locate the inspector-subject push and the View Source path.
- [ ] **Step 2: Write the failing tests:** a unit test on the target→subject-key mapping function (pure, all five `BehaviorTarget` arms), and a test that `BehaviorDocView::handle` never returns an `edit` for any `Selected` action.
- [ ] **Step 3: Implement, pass, full gate.**
- [ ] **Step 4: Final visual sign-off, all three kinds (spec decision 7):** one editor run per kind by pid — select a node/lifeline and confirm the inspector populates; open View Source and confirm the subject markdown; press Esc and confirm the selection clears; confirm the status bar shows solver diagnostics on a deliberately-broken document (e.g. an edge to an unknown target). Kill each pid.
- [ ] **Step 5: Commit** `feat(editor): behavior selection — inspector, view source, esc, diagnostics`

**Done when:** gate green; mapping + no-edit tests pass; three-kind interactive sign-off complete. The whole spec §6 bar is now met: fixtures (T2/T4), per-invariant goldens (T2–T5), hit tests (T7/T8), `uml_documents::open` regression (T6), per-pid visual sign-offs (T6–T9).

---

## Open questions — RESOLVED by the dispatcher

All four were resolved before this plan was handed to `implement-plan`. The plan body above
has been amended to match; these resolutions are authoritative where any stale wording
remains.

1. **`edge_labels.rs` reuse — ACCEPTED as proposed.** Confirmed: its only entry point is
   `edge_end_labels(edge: &SceneEdge, display: &ResolvedDiagramDisplay)` (`edge_labels.rs:25`),
   which is class-typed. Add the small kind-agnostic `mid_route_label` helper in that file and
   route flow labels through it. Do not bend the class types.
2. **View Source exposure — OVERRIDDEN. Do NOT add a context menu.** The spec grants these
   surfaces selection plus inspector plus View Source, and nothing more (§1 decision 1, §8).
   A new right-click menu on a read-only surface is scope the spec does not grant. Wire View
   Source from the selection/inspector affordance only. Task 9's context-menu sub-item is
   dropped.
3. **Fixture syntax — ACCEPTED as proposed.** `crates/waml/tests/uml_behavior_syntax.rs` is
   ground truth for the DSL, and the analysis is authoritative over the spec's model table if
   they ever disagree.
4. **Single flow entry point — CHANGED.** Do not add `solve_flow_full`. Nothing consumes
   `solve_flow` yet, so there is no compatibility argument for two entry points; two would be
   strictly worse. `solve_flow` returns `FlowSolution { solved, diagnostics, reversed,
   off_page }` from Task 2 onward. Task 2 declares `FlowSolution` and `OffPageStub` and
   populates `solved` plus `reversed`; Task 3 fills `solved.routes` and `off_page`. The spec's
   §1.1 signature was amended to match in the same commit as this plan.

## Original open questions (superseded — kept for the record)

1. **`edge_labels.rs` reuse is not literal.** Spec §2.6 says flow edge text is placed by "the editor's existing `edge_labels.rs`, the same code path as class relationship labels" — but that file's entry point is typed against class-only `SceneEdge`/`ResolvedDiagramDisplay`. Assumption taken (Task 7): add one small kind-agnostic pure helper (`mid_route_label`) inside `edge_labels.rs` and route flow labels through it, leaving the class-typed API untouched. This honors the "same code path" intent without bending class types.
2. **View Source exposure surface.** Spec §5.2 says selection drives the View Source tab "through the same path the node context menu already uses", but a read-only surface has no context menu specified. Assumption taken (Task 9): mirror exactly how the class surface exposes View Source (context menu with only the View Source item, no authoring entries). If the dispatcher prefers no context menu at all on behavior surfaces, drop that sub-item and wire View Source solely from the selection/inspector affordance the class view already has.
3. **Behavior fixture frontmatter/DSL details** (exact `type:` tokens, transition/message syntax) are taken from `crates/waml/tests/uml_behavior_syntax.rs` as ground truth, not restated in this plan. If that file and the analysis disagree with the spec's model table, the analysis is authoritative and the fixture should follow it.
4. **`solve_flow_full` wrapper (Task 3)** extends the spec §1.1 `solve_flow` signature (which returns only `(Solved, Vec<Diagnostic>)`) because the renderer needs `reversed` edge keys and off-page stubs that `Solved` cannot carry. The spec-shaped `solve_flow` is kept and delegates, so nothing in the spec is contradicted — flagging it because it is a public-API addition the spec didn't enumerate.
