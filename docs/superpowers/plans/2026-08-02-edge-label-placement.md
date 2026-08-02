# Edge Label Placement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move edge label placement out of the renderer and into the solver as a world-space layout stage, so labels stop overlapping each other and stop vanishing under node cards.

**Architecture:** A new `waml::solve::label` module measures label text with the existing `sizing` module, generates a small discrete candidate set per label (slide along the route x side of the stroke), rejects candidates that collide with hard obstacles (node cards, group title bands, already-placed labels), scores the rest, and assigns greedily in a fixed order. The editor composes label *strings* (display policy stays editor-side) and consumes placed world rects. Node spacing floors gain a `MIN_SEP` for unconnected neighbours and a label-aware `MIN_ASSOC` for connected ones.

**Tech Stack:** Rust, `waml` crate (pure, wasm-clean, no rendering backend), `ttf-parser` via `waml::solve::sizing`, `waml-editor` (makepad) as the consuming frontend.

## Global Constraints

- The `waml` crate is pure and wasm-clean. No rendering backend, no platform APIs, no `Date`/RNG. All new code must hold this.
- All placement must be deterministic: same input, same output, byte-identical golden dumps. No hash-map iteration order, no floating-point accumulation that depends on traversal order.
- Label geometry is **world space**. The only screen-space behaviour that remains in the renderer is the adornment head clearance and the legibility cutoff.
- Label font is `sizing::Font::Sans` at world size `8.0`. This matches the renderer's `target_size = 8.0 * zoom`.
- Existing route goldens must not move in this plan. This plan does not touch `route.rs`.
- The workspace gate is `cargo test --workspace` plus `cargo clippy --all-targets` with no new warnings. `dead_code` is promoted to a hard error by the gate, so do not leave unused variants or fields behind.
- Run `cargo fmt -p <crate>` (not `--all`) before committing, to avoid sweeping up pre-existing formatting drift in unrelated files.

---

### Task 1: Floor unconnected neighbours at MIN_SEP

`geometry.rs` floors the facing-border gap for *connected* pairs at `MIN_ASSOC = 72`, but unconnected pairs fall through to the plain margin (`Medium = 16`), so unrelated boxes read 4.5x tighter than related ones. This task adds the symmetric floor.

**Files:**
- Modify: `crates/waml/src/solve/geometry.rs:14` (add const), `crates/waml/src/solve/geometry.rs:190-196` (apply floor)
- Test: `crates/waml/src/solve/geometry.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `const MIN_SEP: f64 = 40.0` in `geometry.rs`, private to that module.

- [ ] **Step 1: Write the failing test**

Add to the `mod tests` block at the bottom of `crates/waml/src/solve/geometry.rs`. Model it on the existing `MIN_ASSOC` test near line 1077, which shows how to get at the solved rects.

```rust
#[test]
fn unconnected_neighbours_are_floored_at_min_sep() {
    // `a left of b` with NO edge between them: the facing-border gap must be
    // floored at MIN_SEP, not left at the plain Medium margin (16), or two
    // unrelated boxes read tighter than two related ones.
    let edges: Vec<(BoxId, BoxId)> = vec![];
    let (_solved, rects, _diags, _dropped) = solve_with_rects(
        &two_box_scene_placed(Direction::LeftOf),
        &edges,
        &two_box_sizes(),
        &SolveConfig::default(),
    );
    let a = rects[&BoxId::Node("a".into())];
    let b = rects[&BoxId::Node("b".into())];
    let gap = b.x - (a.x + a.w);
    assert_eq!(gap, MIN_SEP, "unconnected pair should be floored at MIN_SEP");
}
```

If `two_box_scene_placed` / `two_box_sizes` helpers do not already exist in that test module, read the existing `MIN_ASSOC` test at `geometry.rs:1069-1083` and build the scene inline exactly the way it does, changing only the `edges` vector to be empty.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p waml unconnected_neighbours_are_floored_at_min_sep`
Expected: FAIL — `cannot find value MIN_SEP in this scope`.

- [ ] **Step 3: Add the constant**

In `crates/waml/src/solve/geometry.rs`, directly below the existing `MIN_ASSOC` declaration at line 14:

```rust
/// Minimum facing-border gap between two boxes with NO edge between them.
/// Without this, unconnected neighbours fall through to the plain margin (16)
/// while connected pairs are floored at `MIN_ASSOC` (72), so unrelated boxes
/// read as a tighter pair than related ones -- the opposite of the truth.
const MIN_SEP: f64 = 40.0;
```

- [ ] **Step 4: Apply the floor**

In `crates/waml/src/solve/geometry.rs`, replace the gap computation at lines 190-195:

```rust
let gap = cfg.margin(max_margin(ma, mb));
let gap = if connected.contains(&pair(a, b)) {
    gap.max(MIN_ASSOC)
} else {
    gap.max(MIN_SEP)
};
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p waml unconnected_neighbours_are_floored_at_min_sep`
Expected: PASS

- [ ] **Step 6: Re-baseline the layout goldens**

Raising a spacing floor moves every golden that has an unconnected placed pair. This is expected — the diffs are the review artifact, not a failure.

Run: `cargo test -p waml 2>&1 | tail -40`

For each failing golden, read the diff and confirm the only change is boxes moving further apart (never overlapping, never reordered). Then update the expected fixtures. Do **not** blanket-accept: if any golden shows a box *order* change or an overlap, stop and report it — that means the floor interacted with a constraint rather than just widening a gap.

- [ ] **Step 7: Run the full gate**

Run: `cargo test --workspace 2>&1 | grep -E "^(test result|error)" | grep -v "0 failed"`
Expected: no output (every suite reports 0 failed).

Run: `cargo clippy -p waml --all-targets 2>&1 | grep -E "^(warning|error)"`
Expected: no new warnings.

- [ ] **Step 8: Commit**

```bash
cargo fmt -p waml
git add crates/waml/src/solve/geometry.rs
git add -u
git commit -m "fix(solve): floor unconnected neighbours at MIN_SEP

Connected pairs were floored at MIN_ASSOC (72) so their connector could
carry adornments and a label; unconnected pairs fell through to the plain
Medium margin (16). Unrelated boxes therefore read as a TIGHTER pair than
related ones, which is backwards. Floors them at 40 -- clearly separated,
still well under the connected floor so connectedness stays legible."
```

---

### Task 2: Measure a label in world units

The solver needs label sizes before it can place anything. `sizing` already measures text headlessly; this wraps it in the label-specific font and size so no call site repeats the constants.

**Files:**
- Create: `crates/waml/src/solve/label.rs`
- Modify: `crates/waml/src/solve/mod.rs` (add `pub mod label;`)
- Test: inline `mod tests` in `crates/waml/src/solve/label.rs`

**Interfaces:**
- Consumes: `waml::solve::sizing::{text_width, line_height, Font}`, `waml::solve::wire::Size`.
- Produces:
  - `pub struct LabelConfig { pub font_size: f64, pub gap: f64, pub slack: f64 }` with `Default` = `{ font_size: 8.0, gap: 3.0, slack: 24.0 }`
  - `pub fn measure(text: &str, cfg: &LabelConfig) -> Size`

- [ ] **Step 1: Write the failing test**

Create `crates/waml/src/solve/label.rs` containing only the test module for now:

```rust
//! World-space edge label placement.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_label_measures_wider_than_it_is_tall_and_scales_with_text() {
        let cfg = LabelConfig::default();
        let short = measure("1", &cfg);
        let long = measure("settledBy {0..*}", &cfg);
        assert!(long.w > short.w, "more text is wider");
        assert_eq!(short.h, long.h, "one line is one line height");
        assert!(short.h > 0.0);
    }

    #[test]
    fn an_empty_label_still_has_height() {
        // A zero-height rect would be invisible to every collision test, so an
        // empty label would silently stop being an obstacle.
        let m = measure("", &LabelConfig::default());
        assert_eq!(m.w, 0.0);
        assert!(m.h > 0.0);
    }
}
```

- [ ] **Step 2: Register the module**

In `crates/waml/src/solve/mod.rs`, add alongside the other `mod` declarations at the top:

```rust
pub mod label;
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p waml -- label::tests`
Expected: FAIL — `cannot find function measure`.

- [ ] **Step 4: Implement the config and measurement**

Add above the test module in `crates/waml/src/solve/label.rs`:

```rust
use super::sizing::{self, Font};
use super::wire::Size;

/// The face edge labels are drawn in. The renderer's `target_size` is
/// `8.0 * zoom`, so 8.0 is the world-space size and both agree at zoom 1.
const LABEL_FONT: Font = Font::Sans;

/// Tunables for label geometry, in world units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabelConfig {
    /// World-space font size for label text.
    pub font_size: f64,
    /// Clearance between a route and the label riding alongside it.
    pub gap: f64,
    /// Extra room reserved between the two terminal labels of one edge, so
    /// they do not touch in the middle when the gap is sized to hold both.
    pub slack: f64,
}

impl Default for LabelConfig {
    fn default() -> Self {
        LabelConfig {
            font_size: 8.0,
            gap: 3.0,
            slack: 24.0,
        }
    }
}

/// World-space box a label's text occupies. Height is a full line height even
/// for empty text -- a zero-height rect is invisible to every collision test,
/// which would silently stop an empty label from acting as an obstacle.
pub fn measure(text: &str, cfg: &LabelConfig) -> Size {
    Size {
        w: sizing::text_width(text, cfg.font_size, LABEL_FONT),
        h: sizing::line_height(cfg.font_size, LABEL_FONT),
    }
}
```

Check the exact name and signature of the line-height helper in `crates/waml/src/solve/sizing.rs` before writing this — the module has `ascent`, `descent`, and a line-height function around line 80. Use whichever exists rather than inventing one.

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p waml -- label::tests`
Expected: PASS, 2 tests.

- [ ] **Step 6: Commit**

```bash
cargo fmt -p waml
git add crates/waml/src/solve/label.rs crates/waml/src/solve/mod.rs
git commit -m "feat(solve): measure edge labels in world units

First piece of moving label placement into the solver. Wraps the existing
ttf-parser sizing in the label font and world size so no call site repeats
the constants. Empty text still measures a full line height -- a zero-height
rect is invisible to collision tests, which would silently stop an empty
label from acting as an obstacle."
```

---

### Task 3: Generate placement candidates

Each label gets a small enumerable candidate set: slide position along its route, times side of the stroke. Discrete and bounded, so placement stays deterministic and golden-testable.

**Files:**
- Modify: `crates/waml/src/solve/label.rs`
- Test: inline `mod tests` in `crates/waml/src/solve/label.rs`

**Interfaces:**
- Consumes: `measure`, `LabelConfig` from Task 2.
- Produces:
  - `pub enum LabelSlot { TerminalFrom, TerminalTo, MidRoute }`
  - `pub struct LabelRequest { pub edge: usize, pub slot: LabelSlot, pub text: String }`
  - `pub struct Candidate { pub rect: Rect, pub attach: (f64, f64), pub side_is_canonical: bool, pub slide_cost: f64 }`
  - `pub fn candidates(points: &[(f64, f64)], slot: LabelSlot, size: Size, cfg: &LabelConfig) -> Vec<Candidate>`

- [ ] **Step 1: Write the failing tests**

Add to the test module in `crates/waml/src/solve/label.rs`:

```rust
#[test]
fn terminal_candidates_stay_near_their_own_endpoint() {
    let cfg = LabelConfig::default();
    let size = Size { w: 40.0, h: 10.0 };
    let route = [(0.0, 0.0), (200.0, 0.0)];

    let from = candidates(&route, LabelSlot::TerminalFrom, size, &cfg);
    let to = candidates(&route, LabelSlot::TerminalTo, size, &cfg);

    assert!(!from.is_empty() && !to.is_empty());
    // Each terminal set clusters at its OWN end, never past the middle.
    for c in &from {
        assert!(c.attach.0 < 100.0, "from candidates stay in the near half");
    }
    for c in &to {
        assert!(c.attach.0 > 100.0, "to candidates stay in the far half");
    }
}

#[test]
fn every_candidate_clears_the_stroke_by_the_gap() {
    let cfg = LabelConfig::default();
    let size = Size { w: 40.0, h: 10.0 };
    // Horizontal route along y = 50: no candidate rect may touch that line.
    for c in candidates(&[(0.0, 50.0), (200.0, 50.0)], LabelSlot::MidRoute, size, &cfg) {
        let clears_above = c.rect.y + c.rect.h <= 50.0 - cfg.gap + 1e-9;
        let clears_below = c.rect.y >= 50.0 + cfg.gap - 1e-9;
        assert!(clears_above || clears_below, "rect sits on the stroke: {:?}", c.rect);
    }
}

#[test]
fn both_sides_of_the_stroke_are_offered() {
    let cfg = LabelConfig::default();
    let size = Size { w: 40.0, h: 10.0 };
    let cs = candidates(&[(0.0, 50.0), (200.0, 50.0)], LabelSlot::MidRoute, size, &cfg);
    assert!(cs.iter().any(|c| c.rect.y < 50.0), "some candidate above");
    assert!(cs.iter().any(|c| c.rect.y > 50.0), "some candidate below");
    assert!(cs.iter().any(|c| c.side_is_canonical), "canonical side offered");
}

#[test]
fn candidate_generation_is_deterministic() {
    let cfg = LabelConfig::default();
    let size = Size { w: 40.0, h: 10.0 };
    let route = [(0.0, 0.0), (60.0, 0.0), (60.0, 90.0)];
    let a = candidates(&route, LabelSlot::MidRoute, size, &cfg);
    let b = candidates(&route, LabelSlot::MidRoute, size, &cfg);
    assert_eq!(a, b);
}

#[test]
fn a_degenerate_route_yields_no_candidates() {
    let cfg = LabelConfig::default();
    let size = Size { w: 40.0, h: 10.0 };
    assert!(candidates(&[], LabelSlot::MidRoute, size, &cfg).is_empty());
    assert!(candidates(&[(1.0, 1.0)], LabelSlot::MidRoute, size, &cfg).is_empty());
    assert!(candidates(&[(1.0, 1.0), (1.0, 1.0)], LabelSlot::MidRoute, size, &cfg).is_empty());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml -- label::tests`
Expected: FAIL — `cannot find function candidates`.

- [ ] **Step 3: Implement candidate generation**

Add to `crates/waml/src/solve/label.rs`. Note `Rect` needs importing from `super::wire`.

```rust
/// Which label on an edge this is. The two terminals carry role/multiplicity
/// text and belong to one end; the mid-route label carries the relationship
/// name and belongs to the whole route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelSlot {
    TerminalFrom,
    TerminalTo,
    MidRoute,
}

/// One label to place: which edge (index into the route list), which slot, and
/// the already-composed text. Text composition is display policy and stays with
/// the frontend; the solver only ever sees the final string.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelRequest {
    pub edge: usize,
    pub slot: LabelSlot,
    pub text: String,
}

/// One possible position for a label.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    /// World box the text would occupy.
    pub rect: Rect,
    /// Point on the route this candidate hangs off, for the leader-line stage
    /// and for the renderer's own head clearance.
    pub attach: (f64, f64),
    /// True when this is the preferred side (above a horizontal run, right of
    /// a vertical one). Used to break near-ties so labels do not flip sides
    /// between two layouts that score the same.
    pub side_is_canonical: bool,
    /// How far this slid from the slot's ideal position, in world units.
    pub slide_cost: f64,
}

/// Fraction-of-band slide positions offered to a terminal label, measured from
/// its own endpoint. Bounded: slid too far and the text stops reading as
/// belonging to that end.
const TERMINAL_SLIDES: [f64; 4] = [0.0, 0.15, 0.30, 0.45];
/// Arc-length positions offered to a mid-route label, as a fraction of total
/// route length. Centred on the true middle.
const MID_SLIDES: [f64; 5] = [0.50, 0.42, 0.58, 0.35, 0.65];

pub fn candidates(
    points: &[(f64, f64)],
    slot: LabelSlot,
    size: Size,
    cfg: &LabelConfig,
) -> Vec<Candidate> {
    let total = polyline_length(points);
    if points.len() < 2 || total <= f64::EPSILON {
        return Vec::new();
    }

    let mut out = Vec::new();
    let slides: &[f64] = match slot {
        LabelSlot::MidRoute => &MID_SLIDES,
        _ => &TERMINAL_SLIDES,
    };

    for &s in slides {
        // Terminal slots measure their fraction from their own end of the
        // route; mid-route measures from the start.
        let t = match slot {
            LabelSlot::TerminalFrom => s,
            LabelSlot::TerminalTo => 1.0 - s,
            LabelSlot::MidRoute => s,
        };
        let Some((attach, tangent)) = point_at_fraction(points, t) else {
            continue;
        };
        let horizontal = tangent.0.abs() >= tangent.1.abs();
        for canonical in [true, false] {
            let rect = rect_for(attach, tangent, horizontal, canonical, size, slot, cfg);
            out.push(Candidate {
                rect,
                attach,
                side_is_canonical: canonical,
                slide_cost: s * total,
            });
        }
    }
    out
}
```

Then write the three private helpers in the same file:

- `polyline_length(points) -> f64` — sum of segment hypots.
- `point_at_fraction(points, t) -> Option<((f64, f64), (f64, f64))>` — walk arc length to `t * total`, return the point and the **unit** tangent of the segment it landed on. Return `None` for a degenerate polyline. This is the same walk as `polyline_midpoint` in `waml-editor/src/edge_labels.rs`; port it rather than reinventing, and return the tangent alongside the point.
- `rect_for(attach, tangent, horizontal, canonical, size, slot, cfg) -> Rect` — place the box one `cfg.gap` off the stroke on the chosen side, and along the route axis: for `TerminalFrom` grow forward from `attach`, for `TerminalTo` grow backward, for `MidRoute` centre on `attach`. This mirrors the `LabelAlign` logic already in `waml-editor/src/edge_labels.rs::aligned_text_pos` — the growth direction exists so a terminal box never grows back over its own endpoint and under the card there.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p waml -- label::tests`
Expected: PASS, 7 tests.

- [ ] **Step 5: Commit**

```bash
cargo fmt -p waml
git add crates/waml/src/solve/label.rs
git commit -m "feat(solve): generate edge label placement candidates

Slide along the route x side of the stroke, as a small discrete set rather
than a continuous optimisation, so placement stays deterministic and
golden-testable. Terminal slots slide from their OWN endpoint and are
bounded to a short band; slid further the text stops reading as belonging
to that end. Every candidate clears the stroke by the configured gap."
```

---

### Task 4: Reject candidates that collide

Placement is only useful if it can say no. This task builds the obstacle set and the hard-collision test.

**Files:**
- Modify: `crates/waml/src/solve/label.rs`
- Test: inline `mod tests` in `crates/waml/src/solve/label.rs`

**Interfaces:**
- Consumes: `Candidate` from Task 3.
- Produces:
  - `pub struct Obstacles { pub hard: Vec<Rect>, pub soft: Vec<[(f64, f64); 2]> }`
  - `pub fn collides(rect: Rect, hard: &[Rect]) -> bool`
  - `pub fn soft_crossings(rect: Rect, soft: &[[(f64, f64); 2]]) -> usize`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_rect_overlapping_a_card_is_rejected() {
    let card = Rect { x: 0.0, y: 0.0, w: 100.0, h: 50.0 };
    assert!(collides(Rect { x: 90.0, y: 40.0, w: 30.0, h: 20.0 }, &[card]));
    assert!(!collides(Rect { x: 101.0, y: 0.0, w: 30.0, h: 20.0 }, &[card]));
}

#[test]
fn touching_edges_do_not_count_as_a_collision() {
    // Exactly abutting is the common case when a floor put a box right at the
    // gap. Treating it as a collision would reject the best candidate.
    let card = Rect { x: 0.0, y: 0.0, w: 100.0, h: 50.0 };
    assert!(!collides(Rect { x: 100.0, y: 0.0, w: 30.0, h: 20.0 }, &[card]));
}

#[test]
fn soft_crossings_are_counted_not_fatal() {
    let rect = Rect { x: 0.0, y: 0.0, w: 100.0, h: 50.0 };
    let through = [(-10.0, 25.0), (110.0, 25.0)];
    let clear = [(-10.0, 200.0), (110.0, 200.0)];
    assert_eq!(soft_crossings(rect, &[through, clear]), 1);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml -- label::tests`
Expected: FAIL — `cannot find function collides`.

- [ ] **Step 3: Implement**

```rust
/// What a label may not sit on (`hard`) and what merely costs it (`soft`).
///
/// Node cards and already-placed labels are hard: text under a card is
/// invisible and text on text is unreadable. Group *title bands* are hard, but
/// group interiors are NOT -- a group box is a large translucent container that
/// legitimately holds edges and labels, so treating its whole rect as solid
/// would forbid every label inside a group.
///
/// Foreign edge strokes are soft. A label's OWN stroke is neither: the
/// perpendicular gap already clears it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Obstacles {
    pub hard: Vec<Rect>,
    pub soft: Vec<[(f64, f64); 2]>,
}

/// True when `rect` overlaps any hard obstacle. Abutting exactly is NOT a
/// collision -- that is the common case when a spacing floor put a box right at
/// the gap, and rejecting it would throw away the best candidate.
pub fn collides(rect: Rect, hard: &[Rect]) -> bool {
    hard.iter().any(|o| {
        rect.x < o.x + o.w && o.x < rect.x + rect.w && rect.y < o.y + o.h && o.y < rect.y + rect.h
    })
}

/// How many soft segments cross `rect`.
pub fn soft_crossings(rect: Rect, soft: &[[(f64, f64); 2]]) -> usize {
    soft.iter().filter(|s| segment_hits_rect(s[0], s[1], rect)).count()
}
```

Write `segment_hits_rect(a, b, rect) -> bool` as a private helper. The routes here are orthogonal, so handle the axis-aligned case directly: a segment hits the rect when its constant coordinate lies inside the rect's span on that axis and its varying coordinate's range overlaps the rect's other span. Fall back to returning `false` for a diagonal segment rather than pretending to handle it — the router only produces orthogonal routes, and a silently wrong diagonal test would be worse than an honest gap.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p waml -- label::tests`
Expected: PASS, 10 tests.

- [ ] **Step 5: Commit**

```bash
cargo fmt -p waml
git add crates/waml/src/solve/label.rs
git commit -m "feat(solve): hard and soft obstacle tests for label placement

Cards and placed labels are hard; group TITLE BANDS are hard but group
interiors are not, since a group legitimately contains edges and labels.
Foreign strokes are soft and merely counted. Abutting exactly is not a
collision -- that is what a spacing floor produces, and rejecting it would
discard the best candidate."
```

---

### Task 5: Score and assign

**Files:**
- Modify: `crates/waml/src/solve/label.rs`
- Test: inline `mod tests` in `crates/waml/src/solve/label.rs`

**Interfaces:**
- Consumes: `candidates`, `collides`, `soft_crossings`, `LabelRequest`.
- Produces:
  - `pub struct PlacedLabel { pub edge: usize, pub slot: LabelSlot, pub text: String, pub rect: Rect, pub attach: (f64, f64) }`
  - `pub struct Placement { pub placed: Vec<PlacedLabel>, pub unplaced: Vec<LabelRequest> }`
  - `pub fn place(routes: &[Vec<(f64, f64)>], requests: &[LabelRequest], obstacles: &Obstacles, cfg: &LabelConfig) -> Placement`

- [ ] **Step 1: Write the failing tests**

```rust
fn route_pair() -> Vec<Vec<(f64, f64)>> {
    vec![vec![(0.0, 50.0), (300.0, 50.0)]]
}

#[test]
fn two_labels_on_one_route_never_overlap_each_other() {
    let reqs = vec![
        LabelRequest { edge: 0, slot: LabelSlot::TerminalFrom, text: "order {1}".into() },
        LabelRequest { edge: 0, slot: LabelSlot::TerminalTo, text: "customer {1}".into() },
    ];
    let out = place(&route_pair(), &reqs, &Obstacles::default(), &LabelConfig::default());
    assert_eq!(out.placed.len(), 2, "both placed: {:?}", out.unplaced);
    assert!(!collides(out.placed[0].rect, &[out.placed[1].rect]));
}

#[test]
fn a_label_never_lands_on_a_card() {
    let obstacles = Obstacles {
        hard: vec![Rect { x: 0.0, y: 0.0, w: 140.0, h: 100.0 }],
        soft: vec![],
    };
    let reqs = vec![LabelRequest {
        edge: 0,
        slot: LabelSlot::TerminalFrom,
        text: "order {1}".into(),
    }];
    let out = place(&route_pair(), &reqs, &obstacles, &LabelConfig::default());
    for p in &out.placed {
        assert!(!collides(p.rect, &obstacles.hard), "placed on a card: {:?}", p.rect);
    }
}

#[test]
fn an_impossible_label_is_reported_not_silently_dropped() {
    // One enormous obstacle covering the whole route: nothing can be placed,
    // and the caller must be able to tell.
    let obstacles = Obstacles {
        hard: vec![Rect { x: -500.0, y: -500.0, w: 2000.0, h: 2000.0 }],
        soft: vec![],
    };
    let reqs = vec![LabelRequest {
        edge: 0,
        slot: LabelSlot::MidRoute,
        text: "places".into(),
    }];
    let out = place(&route_pair(), &reqs, &obstacles, &LabelConfig::default());
    assert!(out.placed.is_empty());
    assert_eq!(out.unplaced, reqs, "unplaceable labels come back to the caller");
}

#[test]
fn placement_is_deterministic() {
    let reqs = vec![
        LabelRequest { edge: 0, slot: LabelSlot::TerminalFrom, text: "a".into() },
        LabelRequest { edge: 0, slot: LabelSlot::TerminalTo, text: "b".into() },
        LabelRequest { edge: 0, slot: LabelSlot::MidRoute, text: "c".into() },
    ];
    let cfg = LabelConfig::default();
    let one = place(&route_pair(), &reqs, &Obstacles::default(), &cfg);
    let two = place(&route_pair(), &reqs, &Obstacles::default(), &cfg);
    assert_eq!(one.placed, two.placed);
}

#[test]
fn the_canonical_side_wins_an_otherwise_tied_choice() {
    let reqs = vec![LabelRequest {
        edge: 0,
        slot: LabelSlot::MidRoute,
        text: "places".into(),
    }];
    let out = place(&route_pair(), &reqs, &Obstacles::default(), &LabelConfig::default());
    // Open space both sides: the label must take the canonical one (above a
    // horizontal run) rather than picking arbitrarily.
    assert!(out.placed[0].rect.y < 50.0, "should sit above the route");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml -- label::tests`
Expected: FAIL — `cannot find function place`.

- [ ] **Step 3: Implement scoring and greedy assignment**

```rust
/// Weight on how far a candidate slid from its slot's ideal position.
const W_SLIDE: f64 = 1.0;
/// Weight on taking the non-canonical side. Small: it only breaks near-ties,
/// so a genuinely better position still wins.
const W_SIDE: f64 = 8.0;
/// Weight per foreign stroke crossed.
const W_CROSSING: f64 = 40.0;

/// A label the solver found a home for.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedLabel {
    pub edge: usize,
    pub slot: LabelSlot,
    pub text: String,
    pub rect: Rect,
    pub attach: (f64, f64),
}

/// Result of a placement pass. `unplaced` is not an error: it is the input to
/// the reroute and leader-line stages, and its length is the instrumentation
/// that says whether those stages are earning their complexity.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Placement {
    pub placed: Vec<PlacedLabel>,
    pub unplaced: Vec<LabelRequest>,
}

pub fn place(
    routes: &[Vec<(f64, f64)>],
    requests: &[LabelRequest],
    obstacles: &Obstacles,
    cfg: &LabelConfig,
) -> Placement {
    let mut out = Placement::default();
    // Already-placed labels become hard obstacles for later ones, which is what
    // stops labels landing on each other.
    let mut hard = obstacles.hard.clone();

    let mut deferred: Vec<&LabelRequest> = Vec::new();
    for request in requests {
        if !try_place(request, routes, &mut hard, obstacles, cfg, &mut out) {
            deferred.push(request);
        }
    }
    // One retry pass. Greedy is order-dependent, so a label rejected early may
    // fit once the rest have settled into their own slots.
    for request in deferred {
        if !try_place(request, routes, &mut hard, obstacles, cfg, &mut out) {
            out.unplaced.push(request.clone());
        }
    }
    out
}

/// Score a single request against every candidate and take the best that has no
/// hard collision. Returns false when nothing fits.
fn try_place(
    request: &LabelRequest,
    routes: &[Vec<(f64, f64)>],
    hard: &mut Vec<Rect>,
    obstacles: &Obstacles,
    cfg: &LabelConfig,
    out: &mut Placement,
) -> bool {
    let Some(points) = routes.get(request.edge) else {
        return false;
    };
    let size = measure(&request.text, cfg);
    let mut best: Option<(f64, Candidate)> = None;
    for c in candidates(points, request.slot, size, cfg) {
        if collides(c.rect, hard) {
            continue;
        }
        let score = W_SLIDE * c.slide_cost
            + if c.side_is_canonical { 0.0 } else { W_SIDE }
            + W_CROSSING * soft_crossings(c.rect, &obstacles.soft) as f64;
        // Strict `<` keeps the FIRST candidate of a tie, and candidate order is
        // deterministic, so ties resolve the same way every run.
        if best.as_ref().is_none_or(|(b, _)| score < *b) {
            best = Some((score, c));
        }
    }
    match best {
        Some((_, c)) => {
            hard.push(c.rect);
            out.placed.push(PlacedLabel {
                edge: request.edge,
                slot: request.slot,
                text: request.text.clone(),
                rect: c.rect,
                attach: c.attach,
            });
            true
        }
        None => false,
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p waml -- label::tests`
Expected: PASS, 15 tests.

- [ ] **Step 5: Run the full gate and commit**

Run: `cargo test --workspace 2>&1 | grep -E "^test result" | grep -v "0 failed"`
Expected: no output.

```bash
cargo fmt -p waml
git add crates/waml/src/solve/label.rs
git commit -m "feat(solve): score and assign edge label placements

Greedy in a fixed order, with each placed label becoming a hard obstacle
for the next -- which is what stops labels landing on each other. One
retry pass covers the order-dependence: a label rejected early may fit
once the rest have settled. Anything still unplaced comes back to the
caller rather than being silently dropped; that count is the signal for
whether the later reroute stage is worth its complexity."
```

---

### Task 6: Carry placed labels on the solved scene

**Files:**
- Modify: `crates/waml/src/solve/mod.rs` (`Solved` gains a field; new placement entry point)
- Test: inline `mod tests` in `crates/waml/src/solve/mod.rs`

**Interfaces:**
- Consumes: `label::{place, LabelRequest, Obstacles, LabelConfig, PlacedLabel}`.
- Produces:
  - `Solved.labels: Vec<PlacedLabel>` (serde `default`, so old payloads still deserialize)
  - `pub fn place_labels(solved: &mut Solved, requests: &[LabelRequest], cfg: &LabelConfig)`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn placed_labels_avoid_the_solved_node_rects() {
    let mut solved = Solved {
        nodes: BTreeMap::from([(
            "a".to_string(),
            Rect { x: 0.0, y: 0.0, w: 120.0, h: 80.0 },
        )]),
        groups: vec![],
        flags: BTreeMap::new(),
        routes: vec![Route {
            points: vec![(120.0, 40.0), (400.0, 40.0)],
            source: "a".into(),
            target: "b".into(),
            key: None,
        }],
        labels: vec![],
    };
    let requests = vec![label::LabelRequest {
        edge: 0,
        slot: label::LabelSlot::TerminalFrom,
        text: "order {1}".into(),
    }];

    place_labels(&mut solved, &requests, &label::LabelConfig::default());

    assert_eq!(solved.labels.len(), 1);
    let card = solved.nodes["a"];
    assert!(!label::collides(solved.labels[0].rect, &[card]));
}
```

Check `Route`'s exact field list in `crates/waml/src/solve/mod.rs` (the `wire` module, around line 68) before writing this literal — it has `points`, `source`, `target`, and `key`.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p waml placed_labels_avoid_the_solved_node_rects`
Expected: FAIL — `Solved` has no field `labels`.

- [ ] **Step 3: Add the field**

In the `wire` module of `crates/waml/src/solve/mod.rs`, add to `Solved`:

```rust
        /// Labels placed in world space by `place_labels`. `default` so a
        /// payload serialized before labels existed still deserializes.
        #[cfg_attr(feature = "serde", serde(default))]
        pub labels: Vec<crate::solve::label::PlacedLabel>,
```

`PlacedLabel` and everything it contains must derive `serde::Serialize`/`Deserialize` under the `serde` feature, matching the pattern the other `wire` types use. Add the `cfg_attr` derives to `PlacedLabel` and `LabelSlot` in `label.rs`.

Adding a field breaks every struct literal of `Solved` in the workspace. Run `cargo check -p waml --all-targets` and fix each one with `labels: vec![]` (or `..Default::default()` where the type allows).

- [ ] **Step 4: Add the entry point**

In `crates/waml/src/solve/mod.rs`:

```rust
/// Place `requests` against the already-solved geometry, filling `solved.labels`.
///
/// Kept separate from `solve_diagram_reported` on purpose: composing the label
/// TEXT is display policy (which toggles are on, how a role and a multiplicity
/// combine), and that belongs to the frontend. The solver only ever sees final
/// strings, so the display model does not have to move into this crate.
pub fn place_labels(solved: &mut Solved, requests: &[label::LabelRequest], cfg: &label::LabelConfig) {
    let obstacles = label::Obstacles {
        hard: solved.nodes.values().copied().collect(),
        soft: solved
            .routes
            .iter()
            .flat_map(|r| r.points.windows(2).map(|w| [w[0], w[1]]))
            .collect(),
    };
    let routes: Vec<Vec<(f64, f64)>> = solved.routes.iter().map(|r| r.points.clone()).collect();
    let placement = label::place(&routes, requests, &obstacles, cfg);
    solved.labels = placement.placed;
}
```

Group title bands are deliberately not in `hard` yet — `SolvedGroup` carries a `title` and a `rect`, but the title band's height is a renderer concern. Add it in a follow-up once the renderer exposes that metric; a comment in the code should say so rather than leaving it looking like an oversight.

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p waml placed_labels_avoid_the_solved_node_rects`
Expected: PASS

- [ ] **Step 6: Run the full gate and commit**

Run: `cargo test --workspace 2>&1 | grep -E "^test result" | grep -v "0 failed"`
Expected: no output.

```bash
cargo fmt -p waml
git add -u
git commit -m "feat(solve): carry placed labels on Solved

place_labels stays a separate entry point rather than folding into
solve_diagram_reported: composing label TEXT is display policy and belongs
to the frontend, so the solver only ever sees final strings and the display
model never has to move into this crate."
```

---

### Task 7: Consume placed labels in the editor

`waml-editor/src/edge_labels.rs` stops deciding geometry and becomes an adapter: it composes text (display policy) and reads back rects.

**Files:**
- Modify: `crates/waml-editor/src/edge_labels.rs`, `crates/waml-editor/src/scene.rs`, `crates/waml-editor/src/canvas/class/render/labels.rs`
- Test: inline tests in each

**Interfaces:**
- Consumes: `waml::solve::{place_labels, label::{LabelRequest, LabelSlot, PlacedLabel, LabelConfig}}`.
- Produces: `pub fn label_requests(edges: &[SceneEdge], display: &ResolvedDiagramDisplay) -> Vec<LabelRequest>`

- [ ] **Step 1: Write the failing test**

In `crates/waml-editor/src/edge_labels.rs`:

```rust
#[test]
fn requests_follow_the_display_switches_and_carry_slot_identity() {
    let mut edge = edge(vec![(20.0, 10.0), (100.0, 10.0)]);
    edge.from_end.role = Some("orders".into());
    edge.name = Some(AssocName::Label("places".into()));
    let mut display = display(CardinalityVisibility::Off);
    display.show_roles = false;
    display.show_cardinality = false;
    display.show_labels = false;
    assert!(label_requests(&[edge.clone()], &display).is_empty());

    display.show_roles = true;
    display.show_labels = true;
    let reqs = label_requests(&[edge], &display);
    let slots: Vec<_> = reqs.iter().map(|r| r.slot).collect();
    assert_eq!(slots, vec![LabelSlot::TerminalFrom, LabelSlot::MidRoute]);
    assert_eq!(reqs[0].text, "orders");
    assert_eq!(reqs[1].text, "places");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p waml-editor --bins requests_follow_the_display_switches`
Expected: FAIL — `cannot find function label_requests`.

- [ ] **Step 3: Extract request composition**

Refactor `edge_end_labels` into `label_requests`: keep the existing text-composition logic verbatim (the role/cardinality match, the `is_ended` guard, the `relationship_name` filter), but emit `LabelRequest { edge: index, slot, text }` instead of computing anchors. Delete the anchor/offset/align computation and every helper that only served it — `terminal_label`, `clear_of_route`, `midpoint_orientation`, `midpoint_segment`, `polyline_midpoint`, `marker_extent`. The gate promotes `dead_code` to a hard error, so leaving them behind fails the build.

Keep `mid_route_label` **only if** the behavior canvas still calls it (`canvas/behavior/render/flow.rs:113`); the flow renderer is out of scope for this plan and must keep working. If it does, leave `mid_route_label` and its helpers in place and note in a comment that the flow canvas has not yet moved to solver placement.

- [ ] **Step 4: Wire placement into scene building**

In `crates/waml-editor/src/scene.rs`, after routes are solved (near line 469 and near line 585 where `solved.routes` is consumed), call:

```rust
let requests = crate::edge_labels::label_requests(&edges, &display);
waml::solve::place_labels(&mut solved, &requests, &LabelConfig::default());
```

Then carry `solved.labels` onto the editor scene so the renderer can read it.

- [ ] **Step 5: Draw from placed rects**

In `crates/waml-editor/src/canvas/class/render/labels.rs`, replace the `edge_end_labels` loop with one over the scene's placed labels. For each: project `rect.x, rect.y` to screen, apply the screen-space head clearance (unchanged from the current implementation), and draw.

Add the legibility cutoff at the top of `draw_edge_labels`:

```rust
/// Below this drawn font size the text is unreadable, so labels are skipped
/// entirely rather than painted as illegible smears that still cost fill rate.
const MIN_LEGIBLE_PX: f64 = 5.0;

if 8.0 * viewport.camera.zoom < MIN_LEGIBLE_PX {
    return;
}
```

- [ ] **Step 6: Run the tests**

Run: `cargo test -p waml-editor --bins 2>&1 | tail -5`
Expected: PASS. Fix any test that asserted on the deleted anchor/offset/align API — those assertions are now the solver's, and the equivalent coverage lives in `label.rs`.

- [ ] **Step 7: Visual sign-off**

Unit tests cannot see occlusion, which is the entire defect class this plan exists to fix. Capture the native editor and look at it.

```bash
# from the worktree root
pwsh -File ./run.ps1 crates/waml-editor/tests/fixtures/mini -Title "LABELS"
```

Then, in ONE PowerShell call so the user's own editor is never captured or killed by name, find the process whose `Path` is under this worktree, capture it by pid via `scripts/capture-window.ps1 -ProcessId <pid>`, and read the PNG.

Confirm: no label overlaps a card; no two labels overlap each other; each terminal label sits clear of its adornment. Kill the process by pid when done.

- [ ] **Step 8: Run the full gate and commit**

Run: `cargo test --workspace 2>&1 | grep -E "^test result" | grep -v "0 failed"`
Expected: no output.

Run: `cargo clippy -p waml-editor --all-targets 2>&1 | grep -E "^(warning|error)"`
Expected: no new warnings.

```bash
cargo fmt -p waml-editor
git add -u
git commit -m "feat(editor): draw edge labels from solver placement

edge_labels.rs stops deciding geometry and becomes an adapter: it composes
label text (display policy) and reads back world rects. Adds a legibility
cutoff -- below 5px drawn the text is unreadable, so skip it rather than
paint illegible smears that still cost fill rate."
```

---

### Task 8: Make MIN_ASSOC label-aware

The motivating screenshot had `order {1}` and `customer {1}` at ~90px each sharing a 72px gap. No placement strategy can win that; the gap was never wide enough. This closes the loop by sizing the gap to the labels it must hold.

**Files:**
- Modify: `crates/waml/src/solve/geometry.rs`, `crates/waml/src/solve/mod.rs`
- Test: inline `mod tests` in `crates/waml/src/solve/geometry.rs`

**Interfaces:**
- Consumes: `label::{measure, LabelConfig}` from Task 2.
- Produces: `solve_with_rects` gains a `label_widths: &BTreeMap<(BoxId, BoxId), f64>` parameter.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn a_connected_pair_widens_to_hold_its_terminal_labels() {
    // Two labels of 90 each plus 24 slack needs 204 -- well past the plain
    // MIN_ASSOC floor of 72.
    let a = BoxId::Node("a".into());
    let b = BoxId::Node("b".into());
    let widths = BTreeMap::from([((a.clone(), b.clone()), 90.0 + 90.0 + 24.0)]);
    let (_solved, rects, _diags, _dropped) = solve_with_rects_labeled(
        &two_box_scene_placed(Direction::LeftOf),
        &[(a.clone(), b.clone())],
        &two_box_sizes(),
        &widths,
        &SolveConfig::default(),
    );
    let gap = rects[&b].x - (rects[&a].x + rects[&a].w);
    assert_eq!(gap, 204.0, "gap sized to hold both terminal labels");
}

#[test]
fn a_connected_pair_with_no_labels_keeps_the_plain_floor() {
    let a = BoxId::Node("a".into());
    let b = BoxId::Node("b".into());
    let (_solved, rects, _diags, _dropped) = solve_with_rects_labeled(
        &two_box_scene_placed(Direction::LeftOf),
        &[(a.clone(), b.clone())],
        &two_box_sizes(),
        &BTreeMap::new(),
        &SolveConfig::default(),
    );
    let gap = rects[&b].x - (rects[&a].x + rects[&a].w);
    assert_eq!(gap, MIN_ASSOC);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml a_connected_pair_widens`
Expected: FAIL — `cannot find function solve_with_rects_labeled`.

- [ ] **Step 3: Thread label widths through**

Rename the existing `solve_with_rects` to `solve_with_rects_labeled` with the extra `label_widths` parameter, and keep `solve_with_rects` as a thin wrapper passing an empty map — every existing caller and test keeps working unchanged.

At `geometry.rs:191`, replace the connected branch:

```rust
let gap = if connected.contains(&pair(a, b)) {
    // A connected pair's connector must hold its terminal labels. Falling back
    // to the bare MIN_ASSOC floor is what made labels unplaceable on short
    // edges: two ~90px labels cannot share a 72px gap, and no placement
    // strategy can rescue a gap that was never wide enough.
    let needed = label_widths
        .get(&pair(a, b))
        .copied()
        .unwrap_or(0.0);
    gap.max(MIN_ASSOC).max(needed)
} else {
    gap.max(MIN_SEP)
};
```

`pair(a, b)` already normalizes ordering; use it for the lookup key so the caller does not have to know which way round the edge was authored.

- [ ] **Step 4: Compute the widths at the call site**

In `crates/waml/src/solve/mod.rs::solve_diagram_reported`, build the map before solving. The width for a pair is `measure(from_text).w + measure(to_text).w + cfg.slack`. The texts come from the same composition the frontend uses, so this needs the label requests to be available *before* geometry solving — thread an optional `&[LabelRequest]` parameter into `solve_diagram_reported`, defaulting to empty.

Callers that do not have labels (the wasm path, `flow.rs`) pass an empty slice and get exactly today's behaviour.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p waml a_connected_pair`
Expected: PASS, 2 tests.

- [ ] **Step 6: Re-baseline goldens and verify visually**

Run: `cargo test --workspace 2>&1 | tail -40`

Update layout goldens whose gaps widened. As in Task 1, confirm each diff is boxes moving apart, not reordering.

Then repeat the Task 7 Step 7 screenshot procedure. This is the task whose effect should be plainly visible: the two terminal labels on the `Order`-`Customer` edge should both be fully readable with clear space between them.

- [ ] **Step 7: Commit**

```bash
cargo fmt -p waml
git add -u
git commit -m "feat(solve): size connected gaps to hold their terminal labels

MIN_ASSOC's flat 72 was the real cause of unplaceable labels on short
edges: two ~90px terminal labels cannot share a 72px gap, so no placement
strategy could win and rerouting would only have papered over it. The
floor now becomes max(72, from + to + slack)."
```

---

### Task 9: Make the floors tunable, and treat group titles as obstacles

Two loose ends from the spec: the spacing floors are still hardcoded constants, and group title bands were deliberately deferred in Task 6 with a comment rather than implemented.

**Files:**
- Modify: `crates/waml/src/solve/mod.rs` (`SolveConfig`), `crates/waml/src/solve/geometry.rs`, `crates/waml/src/solve/stress.rs`
- Test: inline `mod tests` in each

**Interfaces:**
- Consumes: `MIN_SEP` (Task 1), `place_labels` (Task 6).
- Produces: `SolveConfig` gains `pub min_sep: f64`, `pub min_assoc: f64`; `StressConfig::gap` reads its default from `SolveConfig`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn spacing_floors_are_tunable_without_a_recompile() {
    let mut cfg = SolveConfig::default();
    assert_eq!(cfg.min_sep, 40.0);
    assert_eq!(cfg.min_assoc, 72.0);

    cfg.min_sep = 100.0;
    let (_solved, rects, _diags, _dropped) = solve_with_rects(
        &two_box_scene_placed(Direction::LeftOf),
        &[],
        &two_box_sizes(),
        &cfg,
    );
    let a = rects[&BoxId::Node("a".into())];
    let b = rects[&BoxId::Node("b".into())];
    assert_eq!(b.x - (a.x + a.w), 100.0);
}
```

And in `crates/waml/src/solve/mod.rs`:

```rust
#[test]
fn a_group_title_band_is_a_hard_obstacle_but_its_interior_is_not() {
    let mut solved = solved_with_titled_group();
    let requests = vec![label::LabelRequest {
        edge: 0,
        slot: label::LabelSlot::MidRoute,
        text: "places".into(),
    }];
    place_labels(&mut solved, &requests, &label::LabelConfig::default());

    let group = solved.groups[0].rect;
    let title_band = Rect { h: GROUP_TITLE_BAND, ..group };
    let placed = solved.labels[0].rect;
    assert!(!label::collides(placed, &[title_band]), "must clear the title");
    // But the label IS allowed inside the group body -- a group legitimately
    // contains edges and their labels.
    assert!(placed.y > group.y);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml spacing_floors_are_tunable`
Expected: FAIL — `SolveConfig` has no field `min_sep`.

- [ ] **Step 3: Move the floors into config**

Add `min_sep: f64` and `min_assoc: f64` to `SolveConfig` in the `wire` module, defaulting to `40.0` and `72.0`. Delete the `MIN_SEP` and `MIN_ASSOC` consts from `geometry.rs` and read `cfg.min_sep` / `cfg.min_assoc` instead. The gate treats `dead_code` as an error, so the consts must actually go, not linger unused.

Update the tests from Tasks 1 and 8 that referenced the consts to use `SolveConfig::default().min_sep` / `.min_assoc`.

For `StressConfig::gap`, keep the field but change its `Default` to read `SolveConfig::default().min_sep` so the two paths cannot drift apart.

- [ ] **Step 4: Add the group title band**

Add `pub const GROUP_TITLE_BAND: f64` to `crates/waml/src/solve/label.rs` with the band height, and in `place_labels` extend the hard obstacle list:

```rust
        hard: solved
            .nodes
            .values()
            .copied()
            // A group's TITLE strip is solid; its interior is not. A group box is
            // a large translucent container that legitimately holds edges and
            // their labels, so treating the whole rect as hard would forbid every
            // label inside a group.
            .chain(solved.groups.iter().filter(|g| g.title.is_some()).map(|g| Rect {
                h: GROUP_TITLE_BAND.min(g.rect.h),
                ..g.rect
            }))
            .collect(),
```

Replace the "deferred" comment added in Task 6 Step 4 — leaving it would now be false.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p waml spacing_floors_are_tunable a_group_title_band`
Expected: PASS, 2 tests.

- [ ] **Step 6: Run the full gate and commit**

Run: `cargo test --workspace 2>&1 | grep -E "^test result" | grep -v "0 failed"`
Expected: no output.

```bash
cargo fmt -p waml
git add -u
git commit -m "feat(solve): make spacing floors tunable, honour group titles

MIN_SEP/MIN_ASSOC move into SolveConfig so layout can be tuned without
hunting constants, and StressConfig::gap reads its default from the same
place so the two layout paths cannot drift apart. Group title bands become
hard obstacles for labels; group INTERIORS deliberately do not, since a
group legitimately contains edges and their labels."
```

---

## Notes for the implementer

- **Work in a git worktree, never the main checkout.** Absolute paths in `Edit`/`Write` do not respect a worktree — verify `git rev-parse --show-toplevel` before editing, or you will silently edit `main` and the build will "pass" as baseline.
- **`cargo fmt -p <crate>`, not `--all`.** The repo has pre-existing formatting drift in `colors_overlay.rs` and `fonts_overlay.rs`; `--all` sweeps them into your diff.
- **The workspace gate is intermittently red from a pre-existing `waml-syntax` proptest bug** (incremental vs full parse disagree on trailing whitespace after an ATX heading). If that specific test fails, it is not your change. Confirm by stashing and re-running at HEAD.
