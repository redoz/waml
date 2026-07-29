# Native behavior canvases: activity, state machine, sequence

Date: 2026-07-30
Status: approved (design sections 1–2 approved interactively; 3–6 completed under the
delegated-autonomy instruction below)

## Why

Activity and sequence diagrams do not render in the editor. They never rendered
*natively*: their only renderers lived in the retired Svelte web stack and were deleted by
`ef618e76 refactor: retire legacy web and WASM stack` (2026-07-28), which removed
`packages/web/src/components/canvas/flow/*`, `.../sequence/SequenceView.svelte`,
`packages/web/src/canvas/flowGraph.ts`, `sequenceLayout.ts`, `flowTypes.ts`, and the
`orders-checkout-activity` / `orders-checkout-sequence` template bundles.

The native editor has exactly one diagram surface — `ClassDiagramSurface`
(`crates/waml-editor/src/canvas/mod.rs:1-14`); `canvas/class/` is the only renderer
directory. The dead seam is precise: `crates/waml-editor/src/uml_documents.rs:98` gives a
canvas only to `NavCategory::Diagram`, so `NavCategory::Behavior` and
`NavCategory::Sequence` fall through to `ClassifierPreviewView`. Opening an activity or
sequence document therefore shows a classifier preview, never a diagram.

The runtime model is already complete and needs no change:

| concept | type | location |
| --- | --- | --- |
| behavior document (activity / state machine) | `FlowDoc` | `crates/waml/src/model.rs:583` |
| pooled flow node | `ActivityNode` | `model.rs:466` |
| pooled flow edge | `FlowEdge` | `model.rs:530` |
| node kind | `FlowNodeKind` (`Initial`, `Final`, `Decision`, `Merge`, `Fork`, `Join`, `Object`, `Plain`) | `model.rs:413` |
| flavor | `FlowFlavor` (`Activity`, `StateMachine`) | `model.rs:455` |
| interaction document | `SequenceDoc` | `model.rs:747` |
| lifeline / fragment / operand | `SeqNode` | `model.rs:701` |
| message | `SeqEdge` | `model.rs:679` |
| ordered stream item | `SeqChild` | `model.rs:668` |
| message kind | `MessageVerb` (`Calls`, `Sends`, `Replies`, `Creates`, `Destroys`) | `model.rs:603` |
| fragment kind | `FragmentKind` (`Alt`, `Opt`, `Loop`) | `model.rs:637` |

`waml::uml::analysis` already populates `model.activity_nodes` (`analysis.rs:1707`) and
`model.interactions` (`analysis.rs:1972`). The data path is alive; only the consumer is
missing. This spec builds that consumer from first principles — the deleted TypeScript is
**not** a source, and is not to be consulted or ported.

## Decisions taken (locked)

1. **Read-only surfaces with selection.** Draw, pan, zoom, hover, select. Selection drives
   the inspector and the View Source tab. **No** drag-to-place, radial drop dial, conflict
   list, or authoring DSL emission in this project.
2. **One spec, three units.** Unit 1 the shared kind-agnostic surface seam; unit 2 flow
   layout + render; unit 3 interaction layout + render.
3. **Layout lives in `waml::solve`.** New `solve::flow` and `solve::interaction`: pure
   `model → geometry` functions in the core crate, golden-testable headlessly. The editor
   only draws the solved result.
4. **Canonical UML shapes in the Atlas skin.** Real UML glyph vocabulary, drawn with the
   existing SDF pens, Atlas colors, accent buckets, and font roles.
5. **Flow layout is layered, top-down, orthogonal, deterministic.**
6. **Interaction layout derives execution occurrences** (activation bars) from the message
   stream by pairing `calls` with `replies` on a per-lifeline stack.
7. **Done bar = fixtures + text goldens + per-pid visual sign-off** for each of the three
   diagram kinds.

Delegated-autonomy note: the user approved sections 1–2 interactively, then handed the
remainder over for unattended completion with the standing instruction to take the
thorough, correct option on every judgment call. Sections 3–6 were completed under that
instruction; every lever they turn was already pinned by decisions 1–7.

## 1. Architecture and seams

### 1.1 Core crate: two new pure modules

`waml::solve::flow`

```rust
pub struct FlowConfig { /* row gap, lane padding, glyph metrics, min sizes */ }

/// Off-page connector stub for a cross-document edge (§2.1).
pub struct OffPageStub {
    pub edge_key: String,
    pub points: Vec<(f64, f64)>,
    pub target_title: String,
}

pub struct FlowSolution {
    pub solved: Solved,
    pub diagnostics: Vec<Diagnostic>,
    /// Edge keys reversed for ranking; the renderer still draws the arrowhead at
    /// the TRUE target (§2.3).
    pub reversed: std::collections::BTreeSet<String>,
    pub off_page: Vec<OffPageStub>,
}

pub fn solve_flow(
    doc: &FlowDoc,
    nodes: &[ActivityNode],
    edges: &[FlowEdge],
    sizes: &SizeMap,
    cfg: &FlowConfig,
) -> FlowSolution;
```

One entry point, not two. `Solved` alone cannot carry the reversed-edge set or the off-page
stubs that the renderer needs, and a bare `(Solved, Vec<Diagnostic>)` tuple would force a
second parallel entry point for the same computation.

The `solved` field reuses the existing `Solved` (`solve/mod.rs:74`) verbatim: `nodes` maps flow-node pool key
→ `Rect`, `groups` carries partition lane bands as `SolvedGroup`, `routes` carries
orthogonal flow-edge polylines.

Golden dumping needs one addition. The existing `solve::pretty` (`mod.rs:157-181`) emits
nodes, groups, and flags but **not** `routes`, so it cannot witness the routing invariants.
Rather than change `pretty` — which would churn every existing class-diagram golden — add:

```rust
/// Deterministic dump of a solved flow: `pretty(solved)` output plus one line per route.
pub fn pretty_flow(solved: &Solved) -> String;
```

`pretty_flow` delegates to `pretty` for node/group/flag lines and appends
`route <source> -> <target> : x,y x,y …` lines in `routes` order. Existing class goldens are
untouched.

`waml::solve::interaction`

```rust
pub struct InteractionConfig { /* column gap, row gap, bar width, nesting step, frame inset */ }

pub struct SolvedLifeline {
    pub id: String,
    pub head: Rect,
    pub stem_x: f64,
    pub stem_top: f64,
    pub stem_bottom: f64,
    pub destroyed: bool,
}

pub struct SolvedActivation {
    pub lifeline: String,
    pub rect: Rect,
    pub depth: u8,
    pub unclosed: bool,
}

pub struct SolvedMessage {
    pub id: String,
    pub verb: MessageVerb,
    pub from_x: f64,
    pub to_x: f64,
    pub y: f64,
    pub self_loop: Option<Rect>,
    pub label: Option<Rect>,
}

pub struct SolvedFragment {
    pub id: String,
    pub kind: FragmentKind,
    pub rect: Rect,
    pub depth: u8,
    pub operands: Vec<SolvedOperand>, // divider_y + guard label rect
}

pub struct SolvedInteraction {
    pub lifelines: Vec<SolvedLifeline>,
    pub activations: Vec<SolvedActivation>,
    pub messages: Vec<SolvedMessage>,
    pub fragments: Vec<SolvedFragment>,
    pub size: Size,
}

pub fn solve_interaction(
    doc: &SequenceDoc,
    sizes: &SizeMap,
    cfg: &InteractionConfig,
) -> (SolvedInteraction, Vec<Diagnostic>);

/// Deterministic dump for goldens, mirroring `solve::pretty`.
pub fn pretty_interaction(solved: &SolvedInteraction) -> String;
```

A time axis has no equivalent in `Solved`, so interaction gets its own output type rather
than being bent into a node map.

Both take a `SizeMap` measured by the existing `solve::sizing` (ttf-parser + IBM Plex
Sans), so text metrics never drift between the class surface and the behavior surfaces.

### 1.2 Editor: `canvas/behavior/`

```
crates/waml-editor/src/canvas/
  primitives.rs          <- promoted from class/render/primitives.rs; shared SDF pens
  behavior/
    mod.rs               BehaviorSurface widget + BehaviorSurfaceAction
    scene.rs             BehaviorScene = Flow(Solved) | Interaction(SolvedInteraction)
    hit.rs               kind-agnostic hit-test -> BehaviorTarget
    render/
      mod.rs
      flow.rs            UML flow glyphs
      interaction.rs     lifelines, activation bars, fragment frames
```

`canvas/viewport.rs` and `canvas/geometry.rs` are already kind-agnostic and are reused
unchanged. `canvas/class/` is otherwise untouched; the only edit to it is moving
`class/render/primitives.rs` up to `canvas/primitives.rs` and re-pointing its imports.

The boundary is drawn where the sameness actually ends: pan/zoom/selection/hit-testing is
identical between the two kinds and error-prone to reimplement (see the
aligned-parent hit-rect offset trap: manual `draw_abs` rects stored during `draw_walk` are
pre-alignment while events arrive post-alignment). Shape drawing genuinely differs, so it
splits per kind. Both surfaces stay clear of the class canvas's ~3.3k lines of authoring
machinery (`placement.rs`, dial, conflict list), which a read-only surface must not
inherit — and `class/widget.rs` (1597 lines) does not grow a kind discriminator.

### 1.3 View seam

`uml_documents.rs:98` grows two arms beside the existing `NavCategory::Diagram` arm:

- `NavCategory::Behavior` → `BehaviorDocView::flow(concept_id)`
- `NavCategory::Sequence` → `BehaviorDocView::interaction(concept_id)`

One new `crates/waml-editor/src/behavior_doc_view.rs` implements `DocView`
(`doc_view.rs:271`), mirroring `ClassDiagramView`'s responsibilities — body chrome, ViewBar,
inspector wiring, View Source tab, conflict badge — minus every authoring path. Tool dock
hidden; `set_canvas_interaction_enabled` used for pan/zoom/select only.

State-machine documents are `FlowDoc { flavor: StateMachine }` and arrive through the same
`Behavior` arm at no extra cost: flavor only tunes glyphs and vocabulary, never layout.

## 2. Flow solver (`waml::solve::flow`)

### 2.1 Resolve

`FlowDoc.nodes` / `FlowDoc.edges` hold pool keys; resolve each to its `ActivityNode` /
`FlowEdge`. An edge whose `to` matches no local node key is **not** drawn as a normal edge
— that is the model's own documented cross-document rule (`model.rs:536-548`), mirroring the
class-diagram edge rule. When such an edge carries `to_ref`, it renders as an off-page
connector stub (chevron + target document title). Without `to_ref` it is dropped with a
diagnostic.

### 2.2 Measure

Per kind, via `solve::sizing`:

| `FlowNodeKind` | Activity flavor | StateMachine flavor | size |
| --- | --- | --- | --- |
| `Plain` | action | state | measured title, plus `entry`/`do_`/`exit` lines when present, padded |
| `Object` | object node | object node | measured title + `:Type` line from `object_ref` |
| `Decision` | decision | choice | diamond; side derived from measured label, floored at a minimum |
| `Merge` | merge | junction | diamond, same rule |
| `Fork` | fork bar | fork bar | fixed thickness; length spans its branch fan |
| `Join` | join bar | join bar | same |
| `Initial` | filled disc | filled disc | fixed |
| `Final` | bullseye | final state | fixed |

A `Plain` node whose `refines` is set (composite / call behavior) gets a refinement
affordance in its footer; the drill-in navigation itself is out of scope here.

### 2.3 Rank and order

1. **Cycle-break.** DFS from `Initial` nodes; if none, from in-degree-0 nodes; if none,
   from the first declared node. Each back-edge found is reversed for ranking purposes and
   flagged `reversed`, so the renderer still draws the arrowhead at the true target.
2. **Rank.** Longest-path on the resulting DAG: `rank(n) = 1 + max(rank(preds))`, ranks are
   rows top to bottom.
3. **Order within a rank.** Seed with DFS discovery order, then a **fixed** number of
   barycenter sweeps (down, up, down, up) followed by one adjacent-transpose crossing
   reduction pass. Fixed iteration counts, stable sorts, no randomness — the same model
   always produces the same picture, so goldens cannot flake.
4. **x positions.** Pack each rank left to right by measured width plus gap, then run a
   bounded number of priority passes nudging each node toward the barycenter of its
   neighbors in adjacent ranks.

### 2.4 Partitions (swimlanes)

Each distinct `ActivityNode.partition` becomes a vertical lane band, ordered by first
appearance in document order. A node's x is clamped to its lane's interval; ranks remain
global so rows read straight across lanes. Lane bands are emitted as `SolvedGroup`
(rect + title + depth), which the existing group renderer already understands. Nodes with
`partition: None` occupy an implicit unlabelled lane placed after the named ones.

### 2.5 Route

Reuse `solve::route` — orthogonal polylines with bend penalties is exactly its job and
matches the standing edge-routing rule (straight and orthogonal only, no splines). Two
flow-specific additions layered on top:

- **Self-edges** route out and back through a side channel beside the node.
- **Reversed (loop) back-edges** route outside the rank stack rather than through it.

Every route must satisfy the endpoint-on-border invariant: both endpoints land *on* the
respective node's border, not merely orthogonal to it.

### 2.6 Edge labels

The solver returns routes only. `trigger` / `guard` / `else` text is placed by the
editor's existing `edge_labels.rs`, the same code path as class relationship labels.

### 2.7 Diagnostics (never fatal)

Unreachable node (no path from any `Initial`); `Decision` with zero guarded out-edges; edge
naming an unknown local id and carrying no `to_ref`; empty flow document. Each emits a
`Diagnostic` and the diagram still renders whatever it can — matching the graceful
degradation already guaranteed at the type level.

## 3. Interaction solver (`waml::solve::interaction`)

Single-pass and fully deterministic: no relaxation, no iteration. The output is a pure
function of document order, which is time order (`model.rs:758`).

### 3.1 Columns

Lifelines are the `SeqNode::Lifeline` entries of `SequenceDoc.nodes`, which the model
guarantees appear first and in declaration order (`model.rs:755-757`) — that is the column
order. Column width is the measured head-box width (title, plus `:Ref` when `ref_`
resolves to a pool classifier); x advances by width plus column gap. `stem_x` is the column
center. A lifeline's messaging handle is its `alias`, else its `title`, and that handle is
what `SeqEdge.from` / `to` reference.

### 3.2 Time rows

Walk the root `items` stream in order, descending into fragments, assigning each message a
monotonically increasing row. A fragment contributes a header row, then per operand a guard
row followed by that operand's items, then closing padding. Row height is the maximum of
the base row gap and the measured label height for that row, so long signatures never
collide.

### 3.3 Activation bars (execution occurrences)

Maintain a per-lifeline stack while walking the stream:

- `calls` A → B: push an activation on B opening at this row's y. Stack depth at push time
  becomes the bar's `depth`, offsetting it horizontally by `depth * nesting_step` so nested
  calls draw as classic nested bars.
- `replies` B → A: pop B's top activation and close it at this row's y.
- `sends` (async): opens no bar.
- `creates`: the target lifeline's head is placed at this row rather than at the top, and
  its stem starts there.
- `destroys`: the target lifeline's stem ends at this row and is marked `destroyed` (drawn
  with an X).
- Any activation still open at the end of the interaction closes at the interaction bottom
  and is flagged `unclosed`.

### 3.4 Self-messages

`from == to`: the message renders as a loop out to the right of the stem and back,
occupying two rows of vertical space so the return leg has somewhere to land.

### 3.5 Fragment frames

After row assignment, compute each fragment's rect bottom-up so a frame always encloses
every descendant message row. Horizontal extent spans from the leftmost to the rightmost
lifeline stem involved in the fragment's subtree, padded; nested fragments inset by the
frame inset. Each operand after the first contributes a dashed divider at its start y, and
every operand carries a guard label rect at its top-left (`guard: None` renders as
`[else]`, per `model.rs:726`).

### 3.6 Diagnostics (never fatal)

Message referencing an unknown lifeline handle (dropped, diagnosed); `replies` with no
matching open `calls` (drawn, diagnosed, no bar closed); fragment with zero operands;
operand with an empty item stream; lifeline never involved in any message (column still
drawn). `unclosed` activations are geometry, not an error — an interaction may legitimately
end mid-call.

## 4. Render: canonical UML in the Atlas skin

Both renderers draw through the promoted `canvas/primitives.rs` pens, use Atlas colors, the
`accent::tree_kind_color` buckets, and the chrome font roles. Two standing constraints
apply: inline `font_size:` / `FontMember` is gate-banned in chrome, so any new text style
goes through a `mod.fonts` role (which means `fonts.rs` + `script_gate.rs` +
`fonts_overlay.rs` move together); and `sdf.box(..., 0)` degenerates and floods, so sharp
corners use `sdf.rect`.

### 4.1 Flow glyphs

| element | drawing |
| --- | --- |
| `Plain` (action / state) | rounded rect, Behavior accent bucket, title in the heading role; `entry` / `do` / `exit` as body-role lines |
| `Object` | square-cornered rect (`sdf.rect`) with title and `:Type` |
| `Decision` / `Merge` | diamond via an explicit SDF path, never a zero-radius box |
| `Fork` / `Join` | solid bar |
| `Initial` | filled disc |
| `Final` | bullseye (ring plus filled inner disc) |
| partition lane | group band with title, styled like the existing group renderer |
| control flow | solid orthogonal polyline, open arrowhead at the true target |
| object flow | solid polyline plus the carried-type label |
| off-page connector | chevron stub plus the target document title |
| refinement affordance | footer marker on a `Plain` node whose `refines` is set |

### 4.2 Interaction glyphs

| element | drawing |
| --- | --- |
| lifeline head | compact card: title, `:Ref` when resolved, accent bucket of the referenced classifier |
| stem | dashed vertical line |
| activation bar | filled thin rect, accent at low alpha, offset by nesting depth |
| `calls` | solid line, solid filled arrowhead |
| `sends` | solid line, open arrowhead |
| `replies` | dashed line, open arrowhead |
| `creates` | dashed line terminating at the created head box |
| `destroys` | solid line terminating in an X on the stem |
| message label | `signature` when present, else the verb, centered above the line |
| fragment frame | stroked rect with a pentagon tab carrying the kind, dashed operand dividers, guards in brackets |

## 5. Selection, inspector, and degradation

### 5.1 Targets

```rust
pub enum BehaviorTarget {
    FlowNode(String),   // pool key
    FlowEdge(String),   // pool key
    Lifeline(String),   // handle
    Message(String),    // SeqEdge id
    Fragment(String),   // SeqNode id
}
```

`hit.rs` resolves a viewport point to a target, topmost-first (a message beats the fragment
frame containing it; an activation bar resolves to its lifeline). Hit rects are stored in
viewport space and translated by the event-area-to-draw-area delta, per the
aligned-parent offset rule, rather than trusting raw `draw_walk` rects.

### 5.2 Behavior

Hover highlights the target; click selects it; `Esc` clears. Selection drives the inspector
panel and the View Source tab, which renders the selected subject's markdown through the
same path the node context menu already uses. No mutation reaches the document from this
surface.

### 5.3 Degradation

The solver never panics. An empty or fully-diagnosed document renders an explicit
empty-state message on the canvas — never a blank surface, which in this codebase reads as
a rendering regression. Diagnostics surface through the existing diagnostic channel and
status bar.

## 6. Testing and verification

### 6.1 Fixtures

New bundles under `crates/waml/tests/fixtures/behavior/`:

- `activity/` — partitions, a decision with guards plus an `else`, a fork/join pair, an
  object node, and a loop back-edge.
- `state-machine/` — `entry`/`do`/`exit` states, triggers and guards on transitions, a
  self-transition.
- `sequence-nested/` — four lifelines, `calls`/`replies` nesting, an async `sends`, a
  `creates` and a `destroys`, and an `alt` fragment containing a nested `opt`.

These replace the deleted `orders-checkout-activity` / `orders-checkout-sequence` templates
and are authored fresh, not recovered.

### 6.2 Headless goldens

Flow goldens via `pretty_flow` (nodes, groups, flags, and routes); interaction goldens via
`pretty_interaction`. Asserted
invariants, each its own test:

- rank monotonicity: every non-reversed edge goes from a lower rank to a higher one;
- lane containment: every node's rect lies inside its partition band;
- no overlapping node rects within a rank;
- every route endpoint lies on its endpoint node's border;
- activation nesting: a child bar's span is contained by its parent's, and depth equals
  stack depth;
- fragment enclosure: a frame contains every descendant message row and every nested frame;
- lifeline lifetime: a `creates` target's stem starts at that row, a `destroys` target's
  ends there;
- determinism: solving the same fixture twice yields byte-identical dump output.

### 6.3 Widget-level

Hit-test tests mapping a known viewport point to the expected `BehaviorTarget` for each
kind, plus a test that `uml_documents::open` returns a `BehaviorDocView` (not a classifier
preview) for a `Behavior` and a `Sequence` concept — the regression that started this.

### 6.4 Visual sign-off (required per unit)

A per-pid screenshot pass on the running native editor for each of the three kinds before a
unit is called green: launch the worktree's own `scripts/run-native.ps1`, capture and kill
by specific pid in one call (never by process name — it hits the user's own editor), and
confirm the canvas actually draws. Layout goldens alone cannot catch a surface that lays out
correctly and draws nothing, which is exactly how the dock-chrome and IconButton
regressions reached main.

### 6.5 Gate

The repo gate per unit: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`.
Note that the gate promotes `dead_code` to a hard error, so no unit may land a
not-yet-consumed public helper — each unit must wire what it introduces.

## 7. Suggested unit breakdown

1. **Shared behavior surface seam.** `canvas/primitives.rs` promotion, `canvas/behavior/`
   scaffolding (widget, `BehaviorScene`, viewport, hit-test, targets),
   `behavior_doc_view.rs`, the two `uml_documents.rs:98` arms, and an empty-state render.
   Green with an empty scene, and visibly reachable by opening a behavior document.
2. **Flow layout and render.** `solve::flow`, the flow fixtures and goldens,
   `render/flow.rs`, partitions, routes, edge labels. Covers both flavors.
3. **Interaction layout and render.** `solve::interaction`, `pretty_interaction`, the
   sequence fixture and goldens, `render/interaction.rs`, activation bars, fragment frames.

## 8. Out of scope

Authoring on these surfaces (drag-to-place, radial dial, conflict list, DSL emission);
PNG/headless export; drill-in navigation into a `refines` target or an off-page connector;
`InstanceSpecification` lifelines (design spec §7.4); state-machine pseudostates beyond the
existing `FlowNodeKind` set; any web or wasm frontend. The deleted TypeScript renderers are
not a reference for any of this work.
