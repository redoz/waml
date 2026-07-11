# Diagram Layout Solver

**Date:** 2026-07-12
**Product:** UAML / Model Canvas (`docs/uaml-spec.md`, `crates/uaml`, `packages/web`)
**Scope (this spec):** **Phase 1** — the headless Rust solver that turns parsed
`## Layout` relations + `## Members` groups into absolute pixel rectangles.
Phases 2 (WASM bridge) and 3 (web integration) are outlined here but get their
own spec → plan cycles.

## Context

The `## Layout` diagram-arrangement language (see
`docs/superpowers/specs/2026-07-11-diagram-layout-language-design.md` and the BNF
in `docs/uaml-spec.md`) is fully **parsed, serialized, and validated** in Rust
(`crates/uaml/src/layout.rs`, AST in `syntax.rs`). The resolved
`model::Diagram` already carries a `groups: Vec<DiagramGroup>` forest and a
`layout: Vec<LayoutStatement>` list.

What does not exist yet is the **solver**: the function that reads those
relations plus per-node sizes and produces the pixel coordinates a canvas draws.
The parser implementation plan explicitly scoped the solver out ("NO solver, NO
pixel/coordinate computation"). The layout-language design doc names it a
follow-on: "Solver + editor inference are follow-on implementation specs." This
is that spec.

Today `packages/web` ignores `## Layout` entirely and auto-arranges Diagram
documents with dagre over ERD edges (`packages/web/src/canvas/layout.ts`). The
solver replaces that path for Diagram documents.

## Goals

- **Deterministic**: same input → byte-identical output. Required for golden
  tests and stable WASM serialization.
- **Headless & pure**: no I/O, no text measurement. Node sizes are supplied by
  the caller. Testable without a browser.
- **Always renders**: every node gets a rectangle. Conflicting or unresolved
  relations degrade gracefully (warn + drop), never panic, never partial.
- **Syntax-free geometry**: the geometric core operates on a resolved IR of box
  ids and constraints, not on the `## Layout` syntax AST.
- **Reuses existing infra**: `Diagnostic`/`DiagCode` for warnings, existing
  golden-test style, no new heavyweight dependencies.

## Non-goals

- **No numeric constraint solver** (Cassowary/kiwi/linear programming). The
  layout-language design doc makes this an explicit non-goal: spacing is
  qualitative (levels), not measured. Adjacency + qualitative margins +
  alignment equality is exactly topological-order + union-find.
- **No concave shrink-hull** in Phase 1. `shrink` reserves a bounding rectangle
  like `box`; the "tuck neighbours into concave notches" compaction from the
  language design doc is a deliberate later optimization.
- **No drag-to-relation inference** here. The editor round-trip (drag → infer
  relation → write sentence) is Phase 3+.
- **No WASM or web code** in this spec. Phase 1 is `crates/uaml` (Rust) only.
- **No coordinate is ever stored.** The solver produces pixels at render time;
  they are never persisted back to the document.

## Architecture

Two layers with a clean, syntax-free IR between them. Both live under a new
`crates/uaml/src/solve/` module.

```
model::Diagram (groups + layout AST) + SizeMap
        │
        ▼   Layer A — resolve.rs
     Scene (BoxId tree + Constraint list, string-ref-free)   + Vec<Diagnostic>
        │
        ▼   Layer B — geometry.rs
     Solved (absolute Rects, group hulls, flags)             + Vec<Diagnostic>
```

### Layer A — Resolve (`crates/uaml/src/solve/resolve.rs`)

```rust
pub fn resolve(diagram: &Diagram, sizes: &SizeMap) -> (Scene, Vec<Diagnostic>);
```

Walks the `DiagramGroup` forest and the `LayoutStatement`s, turning every operand
reference (`NameRef::Bare`, `NameRef::Link`, inline `row of`/`column of`,
parenthesized operands) into a stable `BoxId`. Pulls `as <axis>` / `with <hints>`
treatment off the layout statements and onto the corresponding `Box` (shape,
axis, margin, flags — these do **not** live on `DiagramGroup`).

Output IR:

```rust
pub struct Scene {
    pub boxes: Vec<Box>,               // forest; index 0.. , tree via child ids
    pub constraints: Vec<Constraint>,  // over BoxIds, in document order
}

pub enum BoxId {
    Node(String),   // resolved node key / slug
    Group(u32),     // named heading group, by resolution index
    Inline(u32),    // anonymous inline row/column group
}

pub struct Box {
    pub id: BoxId,
    pub kind: BoxKind,                  // Leaf | Group
    pub children: Vec<BoxId>,           // groups only
    pub axis: Option<Axis>,             // Row | Column; None = clump
    pub shape: Shape,                   // Frame | Box | Shrink (default Shrink)
    pub margin: Margin,                 // default Medium
    pub flags: FlagSet,                 // emphasized / collapsed
    pub title: Option<String>,          // for a frame heading
    pub depth: u8,                      // heading depth (draw ordering)
}

pub enum Constraint {
    Place { a: BoxId, b: BoxId, dir: Direction },    // a <dir> b, adjacency
    Align { a: BoxId, a_edge: Edge, b: BoxId, b_edge: Edge },
}
```

Resolution rules:

- A **bare name** resolves against, in order: group heading names, then node
  titles/slugs in the diagram's membership. First match wins.
- A **link** resolves by its slug against diagram nodes.
- An **inline group** becomes a fresh `BoxId::Inline` box whose children are its
  resolved items; it nests.
- A **chained placement** `A left of B above C` expands to pairwise
  `Constraint::Place` (`A<-left->B`, `B<-above->C`).
- An **operand that carries treatment on its own line** (`Standalone` with
  `as`/`with`) contributes no positional constraint — it only sets that box's
  axis/shape/margin/flags.

Graceful degradation: an operand that resolves to nothing (`Users` matches no
group or node) emits `Diagnostic::warn` with a new `DiagCode`
(`UnknownLayoutOperand`) and the constraint carrying it is dropped. Mirrors the
`ClassifierType::Unknown` philosophy — an unknown reference never fails the
whole solve.

### Layer B — Solve (`crates/uaml/src/solve/geometry.rs`)

```rust
pub fn solve(scene: &Scene, sizes: &SizeMap, cfg: &SolveConfig) -> (Solved, Vec<Diagnostic>);
```

Pure geometry over `BoxId`s. No syntax types, no string references beyond box
ids. See **The solver algorithm** below.

### Top-level entry

```rust
pub fn solve_diagram(diagram: &Diagram, sizes: &SizeMap, cfg: &SolveConfig)
    -> (Solved, Vec<Diagnostic>);
```

Composes `resolve` then `solve`, concatenating diagnostics. This is what Phase 2
exports over WASM.

### Inputs supplied by the caller

```rust
pub type SizeMap = BTreeMap<String, Size>;   // node key -> intrinsic (w, h)
pub struct Size { pub w: f64, pub h: f64 }

pub struct SolveConfig {
    pub margin_px: [f64; 4],   // [No, Small, Medium, Large], default [0,8,16,32]
    pub chip_size: Size,       // fixed size used for a `collapsed` node
}
```

The solver never measures text. Web supplies sizes from `erdAwareNodeSize`
(`packages/core/src/canvas/layoutSize.ts`); tests supply fixed sizes. `margin_px`
values are tunable and not load-bearing.

## The solver algorithm

**Everything is a box.** A leaf box is an element (size from `SizeMap`, or
`cfg.chip_size` if `collapsed`). A composite box is a group (heading group or
inline group) whose size derives from its solved contents plus shape/margin.
Containment forms a tree; constraints are a flat set over boxes at any level.

**X and Y are solved independently.** A `center` alignment and an adjacency's
implied cross-axis alignment each contribute to *both* axes' structures — no 2D
coupling is needed. Per axis, two structures:

- **Ordering graph** (directed): `A left of B` ⇒ edge `A → B` on X; adjacency is
  tight, so `B.start = A.end + gap`. `above/below` ⇒ edges on Y.
- **Alignment classes** (union-find): `top of X aligned with top of Y` unions X
  and Y on the Y axis at their top edge. An adjacency additionally unions the
  **cross-axis centers** of its two boxes (the chosen "center-aligned"
  adjacency rule) — overridable by an explicit `aligned with`.

Adjacency cross-axis rule (decided): `A left of B` / `A right of B` align the two
boxes' **vertical centers**; `A above B` / `A below B` align their **horizontal
centers**. Consistent with bare `X aligned with Y` = center-to-center.

Solve order — post-order over the box tree:

1. **Size leaves** from `SizeMap` (or `cfg.chip_size` when `collapsed`).
2. **Arrange each group internally.** No axis ⇒ *clump*: deterministic
   flow-pack in list order (left-to-right rows wrapping to a bounded width, or a
   simple single-axis pack — deterministic by list order). `as row` / `as column`
   ⇒ adjacency + center-align constraints across members in list order.
   This yields the group's content bounds and thus its size.
3. **Build per-axis graphs** from placement constraints, alignment constraints,
   and group axes over all boxes.
4. **Cycle check** per axis. On a cycle, emit `Diagnostic::warn`
   (`LayoutConstraintCycle`), drop the edge that closes the cycle (deterministic:
   the later-in-document edge), continue.
5. **Assign coordinates**: topological walk over the ordering graph; each box's
   start = `max(pred.end + gap)` over predecessors; boxes in the same alignment
   class are forced to a single coordinate via the union-find representative. The
   gap between two adjacent boxes = `margin_px[max(a.margin, b.margin)]`.
6. **Unconstrained boxes** fall back to their group's clump position — nothing
   floats undefined. (An unconstrained axis is not a conflict; no diagnostic.)
7. **Composite sizing + nesting**: a solved group becomes a single box in its
   parent's graph; recurse outward, then translate all local coordinates to
   absolute pixels.

**Precedence**: explicit `## Layout` relations beat a group's implicit
list-order/axis. Explicit-vs-explicit conflicts take the drop-and-warn path.

**Why custom, not Cassowary**: the design doc rules out numeric constraint
solving; adjacency + qualitative margins + alignment equality *is*
topological-order + union-find, deterministic by construction and
golden-test-friendly.

## Groups, shapes, margins geometry

After a composite box's children are positioned, its **content bounds** = the
union of child rects. Shape + margin turn that into the outer rect + render
treatment:

| Shape             | Outer geometry (keep-out)              | Rendered                     |
| ----------------- | -------------------------------------- | ---------------------------- |
| `frame`           | content bounds + margin, rectangle     | titled box drawn (heading)   |
| `box`             | content bounds + margin, rectangle     | invisible (reserves rect)    |
| `shrink` (default)| **MVP: same rectangle** (hull deferred)| invisible                    |

- **Margin** = uniform padding on all four sides; level → px via
  `cfg.margin_px`. Applied both *inside* a group (content-to-hull inset) and
  *between* boxes (adjacency gap uses the larger of the two neighbours'
  margins). Default level = `Medium`.
- **Shrink concave hull** (tuck neighbours into notches) is **deferred**; Phase 1
  reserves the bounding rect for all three shapes. Frame/box/shrink already
  differ in *rendering*; the packing win comes later.
- **Axis** (`as row` / `as column`) drives internal arrangement (step 2 above);
  no axis = clump.
- **`collapsed`**: the node uses `cfg.chip_size` instead of its `SizeMap`
  entry (renders as a reference chip). **`emphasized`**: pure render flag, no
  geometric effect — passed through in `Solved.flags`.

## Conflict diagnostics (best-effort)

Every dropped constraint emits a `Diagnostic::warn` so authors see *why* a
relation didn't take. New `DiagCode`s:

- `UnknownLayoutOperand` — operand resolves to no node/group (Layer A); the
  constraint is dropped.
- `LayoutConstraintCycle` — an axis ordering cycle (`A left of B`, `B left of
  A`); drop the edge that closes the cycle.
- `LayoutAlignmentConflict` — an alignment would merge two classes already
  pinned to different coordinates by ordering; drop the later alignment.

An unconstrained axis is **not** a conflict — it falls back to clump, silently.

**Guarantee**: `solve_diagram` always returns a complete `Solved` (every diagram
node has a rect), never panics, never partial.

## Output contract

```rust
pub struct Solved {
    pub nodes:  BTreeMap<String, Rect>,   // every diagram node, absolute px
    pub groups: Vec<SolvedGroup>,         // draw order: outermost (lowest depth) first
    pub flags:  BTreeMap<String, FlagSet>,
}
pub struct Rect { pub x: f64, pub y: f64, pub w: f64, pub h: f64 }
pub struct SolvedGroup { pub rect: Rect, pub shape: Shape, pub title: Option<String>, pub depth: u8 }
pub struct FlagSet { pub emphasized: bool, pub collapsed: bool }
```

- Coordinates absolute, origin top-left, y-down (canvas / React-Flow
  convention).
- `BTreeMap` (not `HashMap`) → deterministic serialization for golden tests and
  stable WASM output.

## Testing

- **Golden tests** (`crates/uaml/tests/`, matching existing `golden.rs` style):
  fixture = a Diagram document (or hand-built `Scene`) + a fixed `SizeMap` →
  snapshot of `Solved` as pretty text. Deterministic output keeps snapshots
  stable.
- **Unit tests per layer**:
  - *resolve*: bare/link/inline resolution; unknown-operand warn + drop;
    treatment pulled onto the box; chained placement expansion.
  - *geometry*: each relation kind; chained placement; alignment classes; center
    adjacency; cycle detection + drop; clump fallback; nested groups; margin gap
    = expected px; `collapsed` uses `chip_size`.
- **Property-style checks** (existing suite uses plain loops): members of a
  `column of A, B, C` are non-overlapping and vertically ordered; adjacency gap
  equals the expected margin px.
- Test command: `cargo test -p uaml`.

## Phases 2 & 3 (outline — separate specs)

- **Phase 2 — WASM bridge**: a new `crates/uaml-wasm` (or a `wasm-bindgen`
  feature on `uaml`) exporting `solve_diagram(bundle, sizes) -> JsValue`, plus
  the `wasm-pack` build and a `@uaml/wasm` workspace package. This is where the
  new build pipeline lives — none exists today (`packages/web` depends only on
  TS `@uaml/core` and `@uaml/okf`).
- **Phase 3 — Web integration**: `packages/web` canvas consumes `Solved` for
  Diagram documents, replacing the dagre path
  (`packages/web/src/canvas/layout.ts`) with solved positions and drawing group
  hulls/frames. Drag-to-relation inference (write sentences back into
  `## Layout`) is a further follow-on.

This spec covers **Phase 1 only** (Layers A + B, golden-tested, headless).

## Open questions

- Clump packing exact shape: single-axis pack vs bounded-width wrapping grid.
  Either is deterministic; pick during implementation and pin with a golden.
- Whether `Scene` should be exposed as a public `crates/uaml` API (useful for a
  future non-UAML frontend) or kept `pub(crate)` until Phase 2 needs it.
- Default `margin_px` values (`[0,8,16,32]`) — confirm against real rendered
  diagrams once Phase 3 lands.
