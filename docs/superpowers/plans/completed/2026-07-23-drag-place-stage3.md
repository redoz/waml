# Drag-to-place Stage 3 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make corner drops author a single diagonal placement relation (moving a node diagonally with zero solver conflicts), draw the placement relations touching the dragged node + hovered target during a drag, and paint any would-conflict compass zone red before the drop commits.

**Architecture:** A new pure primitive in the `waml` crate — four diagonal `Direction` variants that separate a pair on **both** axes and center-align **neither** — replaces the corner gesture's old "two conflicting cardinals" emission. The `waml-editor` canvas + `ClassDiagramView` then plumb the diagram's existing placement relations into the render seam (`Scene`) once, and reuse that same projection for (a) a drag-time relation overlay and (b) a speculative-solve conflict oracle that clones the diagram, applies the hypothetical placement, re-solves, and reports whether the solver emits a `LayoutConflict`.

**Tech Stack:** Rust (workspace crates `waml`, `waml-editor`), Makepad UI (canvas widget, immediate-mode draw), `cargo test` as the gate. No web / pnpm work — this change does not touch the TS/wasm surface.

## Global Constraints

- Gate for **every** task: `cargo test --workspace` must be green (compile + all unit tests). Run it before each commit. Web (`pnpm`) is NOT touched, so no web gate applies.
- The implement-plan gate promotes `rustc`/`clippy` warnings to hard errors (`-D warnings`). Leave **zero** warnings: no unused variables, no unused `mut`, no dead code. When an edit removes the last use of a local, remove the local.
- One relation per pair: authoring a placement for a `(subject, reference)` ordered pair replaces whatever placement that exact ordered pair already had. A diagonal is a **single** `Direction` (one placement), not two cardinals.
- Diagonal DSL surface strings are exactly: `AboveLeft => "above left of"`, `AboveRight => "above right of"`, `BelowLeft => "below left of"`, `BelowRight => "below right of"`.
- Do NOT put UI/oracle logic in `app.rs`. The view (`class_diagram_view.rs`) owns the canvas + gesture and emits ops via `ViewOutcome`; the shell applies them.
- Commit trailer on every commit:
  `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`

---

## File Structure

- `crates/waml/src/syntax.rs` — `Direction` enum gains 4 diagonal variants.
- `crates/waml/src/solve/geometry.rs` — 4 diagonal arms in the `Constraint::Place` match (both-axis separation, no center-align).
- `crates/waml/src/validate.rs` — 4 diagonal arms in the cycle-detection match (a diagonal contributes an edge to BOTH the horizontal and vertical graphs).
- `crates/waml/src/layout.rs` — serialize (`dir_str`) + parse (`eat_direction`) the 4 diagonals.
- `crates/waml/src/ops/mod.rs` — no production change; a new characterization test for the diagonal pair-scoped replace.
- `crates/waml-editor/src/canvas.rs` — `dir_word` diagonal arms (Task 1, for workspace compile); `Placed` → single `Option<Direction>` + `zone_placed` corner→diagonal (Task 2); drag-time relation overlay (Task 5); conflict-red compass + `CompassArmed` action + `set_conflict_zones` (Task 6).
- `crates/waml-editor/src/scene.rs` — `SceneRelation` + `Scene.relations` projection (Task 3); `placement_would_conflict` speculative-solve oracle (Task 4).
- `crates/waml-editor/src/class_diagram_view.rs` — handle `CompassArmed` → compute per-zone verdicts via the oracle → push to canvas (Task 6).

---

### Task 1: Diagonal placement primitive (`waml` crate)

Adds the four diagonal `Direction` variants and every match arm they force across the `waml` crate (solver, validator, serialize, parse), plus the one `waml-editor` arm (`dir_word`) required to keep the workspace compiling. This is the pure, cleanly-TDD-able primitive that gates every later task; it lands **fully and first**.

**Files:**
- Modify: `crates/waml/src/syntax.rs:187-192` (the `Direction` enum)
- Modify: `crates/waml/src/solve/geometry.rs:110-127` (the `Constraint::Place` `match dir`)
- Modify: `crates/waml/src/validate.rs:358-365` (cycle-detection `match dir`)
- Modify: `crates/waml/src/layout.rs:108-115` (`dir_str` serialize) and `crates/waml/src/layout.rs:257-289` (`eat_direction` parse)
- Modify: `crates/waml-editor/src/canvas.rs:432-440` (`dir_word` — keep workspace compiling)
- Test: `crates/waml/src/solve/geometry.rs` (tests module, near `contradiction_warns_and_still_renders` at ~:628), `crates/waml/src/layout.rs` (tests module, near the round-trip tests at ~:755), `crates/waml/src/ops/mod.rs` (tests module, near `place_set_rewrites_a_different_axis_placement` at ~:2129)

**Interfaces:**
- Produces: `waml::syntax::Direction::{AboveLeft, AboveRight, BelowLeft, BelowRight}` — four new unit variants of the existing `Copy` enum. Serialize/parse round-trip via `waml::layout::{render_layout_line, parse_layout_line}`. Solver honors them in `solve::geometry` (both-axis separation). `waml::ops::Op::PlaceSet { directions: Vec<Direction>, .. }` already carries them (no op change).

- [ ] **Step 1: Write the failing solver test**

Add to the tests module in `crates/waml/src/solve/geometry.rs` (alongside `contradiction_warns_and_still_renders`):

```rust
#[test]
fn diagonal_above_left_separates_both_axes_without_conflict() {
    // a AboveLeft b: a's bottom-right corner sits above-and-left of b's
    // top-left. Both axes are separated; NEITHER is center-aligned (that is
    // the whole point of the diagonal primitive).
    let scene = Scene {
        boxes: vec![leaf("a"), leaf("b")],
        constraints: vec![Constraint::Place {
            a: BoxId::Node("a".into()),
            b: BoxId::Node("b".into()),
            dir: Direction::AboveLeft,
        }],
    };
    let (solved, diags) = solve(
        &scene,
        &sizes(&["a", "b"], 200.0, 90.0),
        &SolveConfig::default(),
    );
    assert!(diags.is_empty(), "diagonal must not conflict: {diags:?}");
    let a = solved.nodes.iter().find(|(k, _)| k == "a").unwrap().1;
    let b = solved.nodes.iter().find(|(k, _)| k == "b").unwrap().1;
    // a is left of b (a right edge <= b left edge)...
    assert!(a.x + a.w <= b.x + 1e-6, "a not left of b: {a:?} {b:?}");
    // ...and a is above b (a bottom edge <= b top edge).
    assert!(a.y + a.h <= b.y + 1e-6, "a not above b: {a:?} {b:?}");
}

#[test]
fn diagonal_below_right_separates_both_axes_without_conflict() {
    let scene = Scene {
        boxes: vec![leaf("a"), leaf("b")],
        constraints: vec![Constraint::Place {
            a: BoxId::Node("a".into()),
            b: BoxId::Node("b".into()),
            dir: Direction::BelowRight,
        }],
    };
    let (solved, diags) = solve(
        &scene,
        &sizes(&["a", "b"], 200.0, 90.0),
        &SolveConfig::default(),
    );
    assert!(diags.is_empty(), "diagonal must not conflict: {diags:?}");
    let a = solved.nodes.iter().find(|(k, _)| k == "a").unwrap().1;
    let b = solved.nodes.iter().find(|(k, _)| k == "b").unwrap().1;
    // a is right of b and below b.
    assert!(b.x + b.w <= a.x + 1e-6, "a not right of b: {a:?} {b:?}");
    assert!(b.y + b.h <= a.y + 1e-6, "a not below b: {a:?} {b:?}");
}
```

> Note: `solved.nodes` is a `BTreeMap<String, Rect>` in this module's `solve` return — the `.iter().find(|(k, _)| k == "a")` form works against its `(&String, &Rect)` items; adjust to `solved.nodes["a"]` if the local `pretty`/`solve` helpers expose a map index instead. Use whichever form the neighboring tests use.

- [ ] **Step 2: Run the test to verify it FAILS (compile error)**

Run: `cargo test -p waml --lib solve::geometry`
Expected: FAIL — compile error `no variant named AboveLeft`/`BelowRight` on `Direction`.

- [ ] **Step 3: Add the four diagonal enum variants**

In `crates/waml/src/syntax.rs`, extend the enum (replace lines 187-192):

```rust
pub enum Direction {
    LeftOf,
    RightOf,
    Above,
    Below,
    AboveLeft,
    AboveRight,
    BelowLeft,
    BelowRight,
}
```

- [ ] **Step 4: Add the four diagonal solver arms**

In `crates/waml/src/solve/geometry.rs`, inside the `match dir { .. }` (after the `Direction::Below` arm at :123-126), add:

```rust
Direction::AboveLeft => {
    // a is above-and-left of b: separate on both axes, center-align neither.
    eq(&mut py, ia, ib, sa.h + gap, diags);
    eq(&mut px, ia, ib, sa.w + gap, diags);
}
Direction::AboveRight => {
    eq(&mut py, ia, ib, sa.h + gap, diags);
    eq(&mut px, ia, ib, -(sb.w + gap), diags);
}
Direction::BelowLeft => {
    eq(&mut py, ia, ib, -(sb.h + gap), diags);
    eq(&mut px, ia, ib, sa.w + gap, diags);
}
Direction::BelowRight => {
    eq(&mut py, ia, ib, -(sb.h + gap), diags);
    eq(&mut px, ia, ib, -(sb.w + gap), diags);
}
```

- [ ] **Step 5: Add the four diagonal cycle-detection arms**

In `crates/waml/src/validate.rs`, replace the tuple-returning `match dir` + push (lines 358-364):

```rust
// Edge points from the operand that must come first to the one after it.
// A diagonal constrains both axes, so it contributes an edge to BOTH graphs.
match dir {
    Direction::LeftOf => {
        horizontal.entry(a).or_default().push(b);
    }
    Direction::RightOf => {
        horizontal.entry(b).or_default().push(a);
    }
    Direction::Above => {
        vertical.entry(a).or_default().push(b);
    }
    Direction::Below => {
        vertical.entry(b).or_default().push(a);
    }
    Direction::AboveLeft => {
        vertical.entry(a.clone()).or_default().push(b.clone());
        horizontal.entry(a).or_default().push(b);
    }
    Direction::AboveRight => {
        vertical.entry(a.clone()).or_default().push(b.clone());
        horizontal.entry(b).or_default().push(a);
    }
    Direction::BelowLeft => {
        vertical.entry(b.clone()).or_default().push(a.clone());
        horizontal.entry(a).or_default().push(b);
    }
    Direction::BelowRight => {
        vertical.entry(b.clone()).or_default().push(a.clone());
        horizontal.entry(b).or_default().push(a);
    }
}
```

- [ ] **Step 6: Add the four diagonal serialize arms**

In `crates/waml/src/layout.rs`, extend `dir_str` (after `Direction::Below => "below",` at :113):

```rust
Direction::AboveLeft => "above left of",
Direction::AboveRight => "above right of",
Direction::BelowLeft => "below left of",
Direction::BelowRight => "below right of",
```

- [ ] **Step 7: Keep the workspace compiling — `dir_word` arms**

In `crates/waml-editor/src/canvas.rs`, extend `dir_word` (the exhaustive `match d` at :434, after `Below => "below",`):

```rust
AboveLeft => "above left of",
AboveRight => "above right of",
BelowLeft => "below left of",
BelowRight => "below right of",
```

- [ ] **Step 8: Run the solver tests to verify they PASS**

Run: `cargo test -p waml --lib solve::geometry`
Expected: PASS (both `diagonal_*` tests green; existing `solves_a_row_of_three` etc. still green).

- [ ] **Step 9: Write the failing parse/serialize round-trip test**

Add to the tests module in `crates/waml/src/layout.rs` (near the `- A left of B` round-trip tests):

```rust
#[test]
fn diagonals_parse_and_serialize_round_trip() {
    use crate::syntax::{Direction, LayoutStatement};
    for (text, dir) in [
        ("- A above left of B", Direction::AboveLeft),
        ("- A above right of B", Direction::AboveRight),
        ("- A below left of B", Direction::BelowLeft),
        ("- A below right of B", Direction::BelowRight),
    ] {
        let stmt = parse_layout_line(text).unwrap();
        let LayoutStatement::Placement {
            operands,
            directions,
        } = &stmt
        else {
            panic!("expected a placement for {text}");
        };
        assert_eq!(operands.len(), 2, "{text}");
        assert_eq!(directions.as_slice(), &[dir], "{text}");
        let rendered = render_layout_line(&stmt);
        let phrase = match dir {
            Direction::AboveLeft => "above left of",
            Direction::AboveRight => "above right of",
            Direction::BelowLeft => "below left of",
            Direction::BelowRight => "below right of",
            _ => unreachable!(),
        };
        assert!(
            rendered.contains(phrase),
            "serialize lost the diagonal: {rendered}"
        );
    }
}

#[test]
fn bare_above_below_still_parse_as_cardinals() {
    use crate::syntax::{Direction, LayoutStatement};
    for (text, dir) in [
        ("- A above B", Direction::Above),
        ("- A below B", Direction::Below),
    ] {
        let LayoutStatement::Placement { directions, .. } =
            parse_layout_line(text).unwrap()
        else {
            panic!()
        };
        assert_eq!(directions.as_slice(), &[dir], "{text}");
    }
}
```

- [ ] **Step 10: Run the round-trip test to verify it FAILS**

Run: `cargo test -p waml --lib layout::`
Expected: FAIL — `diagonals_parse_and_serialize_round_trip` fails: `directions` is `[Above]`/`[Below]` (parser fell through to the cardinal; the trailing `left of`/`right of` was not consumed).

- [ ] **Step 11: Add the diagonal parse logic**

In `crates/waml/src/layout.rs`, replace the `"above"` and `"below"` arms of `eat_direction` (lines 259-266):

```rust
"above" => {
    cur.bump(); // consume "above"
    let save = cur.pos;
    match cur.peek_word().map(|w| w.to_ascii_lowercase()).as_deref() {
        Some("left") => {
            cur.bump();
            if cur.eat_word("of") {
                Some(Direction::AboveLeft)
            } else {
                cur.pos = save;
                Some(Direction::Above)
            }
        }
        Some("right") => {
            cur.bump();
            if cur.eat_word("of") {
                Some(Direction::AboveRight)
            } else {
                cur.pos = save;
                Some(Direction::Above)
            }
        }
        _ => Some(Direction::Above),
    }
}
"below" => {
    cur.bump(); // consume "below"
    let save = cur.pos;
    match cur.peek_word().map(|w| w.to_ascii_lowercase()).as_deref() {
        Some("left") => {
            cur.bump();
            if cur.eat_word("of") {
                Some(Direction::BelowLeft)
            } else {
                cur.pos = save;
                Some(Direction::Below)
            }
        }
        Some("right") => {
            cur.bump();
            if cur.eat_word("of") {
                Some(Direction::BelowRight)
            } else {
                cur.pos = save;
                Some(Direction::Below)
            }
        }
        _ => Some(Direction::Below),
    }
}
```

- [ ] **Step 12: Run the round-trip test to verify it PASSES**

Run: `cargo test -p waml --lib layout::`
Expected: PASS (both new tests green; existing `- A below B` etc. round-trips still green).

- [ ] **Step 13: Write the op-layer diagonal pair-scoped-replace test**

Add to the tests module in `crates/waml/src/ops/mod.rs` (near `place_set_rewrites_a_different_axis_placement`). It reuses the module's existing `layout_diagram`, `apply`, and `placeset` helpers:

```rust
#[test]
fn place_set_diagonal_replaces_a_cardinal_for_the_same_pair() {
    // A single diagonal Direction is ONE placement. Re-dragging Order onto a
    // corner of PaymentGateway rewrites the prior cardinal for that ordered
    // pair, not stacking a conflicting second relation.
    let b = layout_diagram(
        "- [Order](./order.md) left of [PaymentGateway](./payment-gateway.md)\n",
    );
    let out = apply(
        &b,
        &[placeset(
            ("Order", "order"),
            ("PaymentGateway", "payment-gateway"),
            vec![Direction::AboveLeft],
        )],
    )
    .unwrap();
    assert!(
        out[0].1.contains(
            "- [Order](./order.md) above left of [PaymentGateway](./payment-gateway.md)"
        ),
        "authored diagonal present: {}",
        out[0].1
    );
    // The prior bare `... .md) left of ...` line is gone. (The diagonal line
    // reads `... .md) above left of ...`, so `.md) left of` distinguishes it.)
    assert!(
        !out[0].1.contains("md) left of"),
        "prior cardinal replaced, not kept: {}",
        out[0].1
    );
}
```

- [ ] **Step 14: Run the op test (expected PASS — `op_place_set` is unchanged)**

Run: `cargo test -p waml --lib ops::`
Expected: PASS. `op_place_set` already loops `for dir in &directions` and `placement_matches` already keys on the ordered pair, so a single diagonal round-trips with no production change. If it fails, the parse/serialize wiring from Steps 6/11 is wrong — fix there, not in `ops`.

- [ ] **Step 15: Run the full gate**

Run: `cargo test --workspace`
Expected: PASS (workspace compiles — `dir_word` arms cover the new variants — and all tests green).

- [ ] **Step 16: Commit**

```bash
git add crates/waml/src/syntax.rs crates/waml/src/solve/geometry.rs \
  crates/waml/src/validate.rs crates/waml/src/layout.rs \
  crates/waml/src/ops/mod.rs crates/waml-editor/src/canvas.rs
git commit -m "feat(solve): diagonal Direction variants — both-axis placement primitive"
```

---

### Task 2: Corner zone authors one diagonal Direction

Collapses the compass gesture's corner emission from two conflicting cardinals to a single diagonal `Direction`, so a corner drop moves the node diagonally (Verify #1). `Placed` becomes a single optional `Direction`.

**Files:**
- Modify: `crates/waml-editor/src/canvas.rs` — `Placed` struct (:326-330), `zone_placed` (:416-429), the drop emission flatten (:835-838), the DSL readout (:1148-1153)
- Test: `crates/waml-editor/src/canvas.rs` tests module (:1431)

**Interfaces:**
- Consumes: `waml::syntax::Direction::{AboveLeft, AboveRight, BelowLeft, BelowRight}` (Task 1).
- Produces: `zone_placed(z: Zone) -> Placed` where `Placed { pub dir: Option<Direction> }`; corner zones map to a single diagonal, edge zones to a single cardinal. The drop emits `AuthorPlacement { directions }` with `directions.len() == 1` for every drop.

- [ ] **Step 1: Write the failing `zone_placed` test**

Add to the tests module in `crates/waml-editor/src/canvas.rs`:

```rust
#[test]
fn corner_zones_author_a_single_diagonal_direction() {
    use waml::syntax::Direction::*;
    assert_eq!(zone_placed(Zone::TopLeft).dir, Some(AboveLeft));
    assert_eq!(zone_placed(Zone::TopRight).dir, Some(AboveRight));
    assert_eq!(zone_placed(Zone::BottomLeft).dir, Some(BelowLeft));
    assert_eq!(zone_placed(Zone::BottomRight).dir, Some(BelowRight));
}

#[test]
fn edge_zones_author_a_single_cardinal_direction() {
    use waml::syntax::Direction::*;
    assert_eq!(zone_placed(Zone::Left).dir, Some(LeftOf));
    assert_eq!(zone_placed(Zone::Right).dir, Some(RightOf));
    assert_eq!(zone_placed(Zone::Top).dir, Some(Above));
    assert_eq!(zone_placed(Zone::Bottom).dir, Some(Below));
}
```

- [ ] **Step 2: Run to verify it FAILS**

Run: `cargo test -p waml-editor --lib canvas::`
Expected: FAIL — compile error: `Placed` has no field `dir` (it has `h`/`v`).

- [ ] **Step 3: Redefine `Placed` as a single optional direction**

In `crates/waml-editor/src/canvas.rs`, replace the `Placed` struct (:326-330):

```rust
/// SPIKE (drag-place): the single placement a compass zone authors relative to
/// its target. An edge zone maps to a cardinal `Direction`, a corner zone to a
/// diagonal. `None` = no zone hovered (drop = cancel). `Direction` reuses the
/// DSL's own vocabulary so the readout maps 1:1 onto `A above left of B`.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct Placed {
    pub dir: Option<waml::syntax::Direction>,
}
```

- [ ] **Step 4: Rewrite `zone_placed` to return one diagonal per corner**

Replace `zone_placed` (:416-429):

```rust
/// The placement a compass `Zone` authors relative to the target: an edge zone
/// is a cardinal, a corner zone a single diagonal. Dropping A on B's *top-left*
/// zone reads `A above left of B`. Pure.
pub fn zone_placed(z: Zone) -> Placed {
    use waml::syntax::Direction::*;
    let dir = match z {
        Zone::Left => LeftOf,
        Zone::Right => RightOf,
        Zone::Top => Above,
        Zone::Bottom => Below,
        Zone::TopLeft => AboveLeft,
        Zone::TopRight => AboveRight,
        Zone::BottomLeft => BelowLeft,
        Zone::BottomRight => BelowRight,
    };
    Placed { dir: Some(dir) }
}
```

- [ ] **Step 5: Emit a single direction on drop**

Replace the flatten at :835-838:

```rust
                            let directions: Vec<_> =
                                self.drag_place.dir.into_iter().collect();
```

- [ ] **Step 6: Fix the DSL readout to draw the single relation**

Replace the readout loop at :1148-1153:

```rust
            if let Some(d) = place.dir {
                let line = format!("{a_key} {} {b_key}", dir_word(d));
                self.draw_mono_dim
                    .draw_abs(cx, dvec2(vx + 12.0, vy + 10.0), &line);
            }
```

- [ ] **Step 7: Run the tests to verify they PASS**

Run: `cargo test -p waml-editor --lib canvas::`
Expected: PASS (both new tests green; existing canvas tests — `node_at_*`, `handle_rect`, `compass_zone_of` — still green).

- [ ] **Step 8: Run the full gate**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add crates/waml-editor/src/canvas.rs
git commit -m "feat(canvas): corner drop authors one diagonal Direction (was two conflicting cardinals)"
```

---

### Task 3: Project placement relations into the `Scene`

Threads the diagram's existing 2-operand single-direction placement relations into `Scene` as `(subject_slug, reference_slug, Direction)` triples. This is the single projection reused by the drag overlay (Task 5) and the conflict oracle indirectly. Plumbed once, unit-tested against the `mini` fixture.

**Files:**
- Modify: `crates/waml-editor/src/scene.rs` — add `SceneRelation` + `Scene.relations`; populate in `build_scene`; add `relations: Vec::new()` to every other `Scene { .. }` literal (`build_focus_scene` at :358 and :398, the tests `Scene { .. }` at :616)
- Test: `crates/waml-editor/src/scene.rs` tests module (:429)

**Interfaces:**
- Produces: `pub struct SceneRelation { pub subject: String, pub reference: String, pub dir: waml::syntax::Direction }` and `Scene.relations: Vec<SceneRelation>`. Slugs equal `SceneNode.key`. `Scene` still derives `Default` (empty `relations`).

- [ ] **Step 1: Write the failing projection test**

Add to the tests module in `crates/waml-editor/src/scene.rs`:

```rust
#[test]
fn scene_projects_existing_placement_relations() {
    let model = mini();
    let (scene, _) = build_scene(
        &model,
        &model.diagrams[0],
        &std::collections::HashSet::new(),
    );
    use waml::syntax::Direction;
    // orders-diagram.md's ## Layout: `Order left of Customer` +
    // `PaymentGateway below Order`.
    let has = |subj: &str, refr: &str, dir: Direction| {
        scene
            .relations
            .iter()
            .any(|r| r.subject == subj && r.reference == refr && r.dir == dir)
    };
    assert!(
        has("order", "customer", Direction::LeftOf),
        "missing order left-of customer: {:?}",
        scene.relations
    );
    assert!(
        has("payment-gateway", "order", Direction::Below),
        "missing payment-gateway below order: {:?}",
        scene.relations
    );
}
```

- [ ] **Step 2: Run to verify it FAILS**

Run: `cargo test -p waml-editor --lib scene::`
Expected: FAIL — compile error: `Scene` has no field `relations`.

- [ ] **Step 3: Add the `SceneRelation` type and the `Scene.relations` field**

In `crates/waml-editor/src/scene.rs`, add above the `Scene` struct (:82):

```rust
/// A placement relation projected from the diagram's `## Layout` for drag-time
/// overlay + conflict prediction: a 2-operand single-direction placement, its
/// operands resolved to `SceneNode.key` slugs. Multi-operand / alignment
/// statements are not projected (the drag overlay + one-relation-per-pair
/// oracle only reason about 2-node placements).
#[derive(Debug, Clone, PartialEq)]
pub struct SceneRelation {
    pub subject: String,
    pub reference: String,
    pub dir: waml::syntax::Direction,
}
```

Add the field to `Scene` (inside the struct at :83-87):

```rust
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Scene {
    pub nodes: Vec<SceneNode>,
    pub groups: Vec<SolvedGroup>,
    pub edges: Vec<SceneEdge>,
    pub relations: Vec<SceneRelation>,
}
```

- [ ] **Step 4: Add a slug helper and project relations in `build_scene`**

In `crates/waml-editor/src/scene.rs`, add a free helper near `drawable_edges` (~:159):

```rust
/// The slug a placement operand refers to (`[Title](./slug.md)` or a bare
/// name). `None` for inline-group / paren operands, which the relation
/// projection skips.
fn operand_slug(op: &waml::syntax::Operand) -> Option<&str> {
    use waml::syntax::{NameRef, OperandRef};
    match &op.ref_ {
        OperandRef::Name(NameRef::Link { slug, .. }) => Some(slug.as_str()),
        OperandRef::Name(NameRef::Bare(s)) => Some(s.as_str()),
        _ => None,
    }
}

/// Project the diagram's `## Layout` into 2-operand single-direction relation
/// triples (subject_slug, reference_slug, dir). Mirrors `ops::placement_matches`'
/// shape: only 2-operand, 1-direction placements qualify.
fn project_relations(diagram: &Diagram) -> Vec<SceneRelation> {
    use waml::syntax::LayoutStatement;
    let mut out = Vec::new();
    for stmt in &diagram.layout {
        if let LayoutStatement::Placement {
            operands,
            directions,
        } = stmt
        {
            if operands.len() == 2 && directions.len() == 1 {
                if let (Some(subject), Some(reference)) =
                    (operand_slug(&operands[0]), operand_slug(&operands[1]))
                {
                    out.push(SceneRelation {
                        subject: subject.to_string(),
                        reference: reference.to_string(),
                        dir: directions[0],
                    });
                }
            }
        }
    }
    out
}
```

Then set `relations` in the `Scene` `build_scene` returns (:343-348). Replace that `Scene { nodes, groups: solved.groups.clone(), edges }` literal with:

```rust
    (
        Scene {
            nodes,
            groups: solved.groups.clone(),
            edges,
            relations: project_relations(diagram),
        },
        diags,
    )
```

- [ ] **Step 5: Add `relations: Vec::new()` to the other `Scene` literals**

`build_focus_scene` has two `Scene { .. }` literals (the empty early return ~:358 and the final ~:398) — add `relations: Vec::new(),` to each. The tests-module `Scene { nodes: vec![], groups: vec![], edges: vec![] }` at ~:616 (in `bounding_box_none_for_empty_scene`) — add `relations: vec![],`.

- [ ] **Step 6: Run the projection test to verify it PASSES**

Run: `cargo test -p waml-editor --lib scene::`
Expected: PASS (new test green; existing scene tests green).

- [ ] **Step 7: Run the full gate**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/waml-editor/src/scene.rs
git commit -m "feat(scene): project 2-node placement relations into Scene for drag overlay + oracle"
```

---

### Task 4: Speculative-solve conflict oracle

Adds a pure function that predicts whether authoring a hypothetical placement would make the solver emit a `LayoutConflict`: clone the diagram, apply the one-relation-per-pair replace against the clone's layout, re-solve via `build_scene`, and inspect the returned diagnostics. Unit-tested against `mini`.

**Files:**
- Modify: `crates/waml-editor/src/scene.rs` — add `placement_would_conflict`
- Test: `crates/waml-editor/src/scene.rs` tests module (:429)

**Interfaces:**
- Consumes: `Scene.relations` shape and `build_scene` (Task 3).
- Produces: `pub fn placement_would_conflict(model: &Model, diagram: &Diagram, subject_slug: &str, reference_slug: &str, dir: waml::syntax::Direction, expanded: &std::collections::HashSet<String>) -> bool`.

- [ ] **Step 1: Write the failing oracle tests**

Add to the tests module in `crates/waml-editor/src/scene.rs`:

```rust
#[test]
fn oracle_flags_a_contradictory_placement() {
    // mini has `Order left of Customer`. Authoring the REVERSED ordered pair
    // `Customer left of Order` is a different pair (so the existing relation is
    // NOT replaced) — both coexist, the solver cannot satisfy them, and emits a
    // LayoutConflict.
    let model = mini();
    let diagram = &model.diagrams[0];
    assert!(
        placement_would_conflict(
            &model,
            diagram,
            "customer",
            "order",
            waml::syntax::Direction::LeftOf,
            &std::collections::HashSet::new(),
        ),
        "reversed cardinal on an existing pair must be predicted conflicting"
    );
}

#[test]
fn oracle_accepts_a_clean_diagonal_placement() {
    // Placing PaymentGateway above-left of Customer contradicts nothing in
    // mini (order left-of customer; payment-gateway below order) — the solver
    // is satisfiable, so no LayoutConflict.
    let model = mini();
    let diagram = &model.diagrams[0];
    assert!(
        !placement_would_conflict(
            &model,
            diagram,
            "payment-gateway",
            "customer",
            waml::syntax::Direction::AboveLeft,
            &std::collections::HashSet::new(),
        ),
        "a non-contradictory diagonal must NOT be predicted conflicting"
    );
}
```

- [ ] **Step 2: Run to verify it FAILS**

Run: `cargo test -p waml-editor --lib scene::oracle`
Expected: FAIL — compile error: `placement_would_conflict` not found.

- [ ] **Step 3: Implement the oracle**

In `crates/waml-editor/src/scene.rs`, add (reusing the `operand_slug` helper from Task 3):

```rust
/// The classifier title for a slug (for a `[Title](./slug.md)` operand), or the
/// slug itself when unknown.
fn title_for(model: &Model, slug: &str) -> String {
    model
        .nodes
        .iter()
        .find(|n| n.key == slug)
        .and_then(|n| n.concept.title.clone())
        .unwrap_or_else(|| slug.to_string())
}

/// True iff a placement matches the given ordered `(subject, reference)` pair
/// as a 2-operand single-direction relation (mirrors `ops::placement_matches`).
fn placement_is_pair(
    stmt: &waml::syntax::LayoutStatement,
    subject: &str,
    reference: &str,
) -> bool {
    use waml::syntax::LayoutStatement;
    if let LayoutStatement::Placement {
        operands,
        directions,
    } = stmt
    {
        operands.len() == 2
            && directions.len() == 1
            && operand_slug(&operands[0]) == Some(subject)
            && operand_slug(&operands[1]) == Some(reference)
    } else {
        false
    }
}

/// Speculatively author `subject <dir> reference` into a scratch clone of the
/// diagram (one-relation-per-pair replace: drop any existing placement for this
/// ordered pair, then push the hypothetical one), re-solve, and report whether
/// the solver emits a `LayoutConflict`. The solver is the ground truth — it
/// catches transitive / cycle contradictions a hand-rolled rule would miss.
pub fn placement_would_conflict(
    model: &Model,
    diagram: &Diagram,
    subject_slug: &str,
    reference_slug: &str,
    dir: waml::syntax::Direction,
    expanded: &std::collections::HashSet<String>,
) -> bool {
    use waml::diagnostic::DiagCode;
    use waml::syntax::{LayoutStatement, NameRef, Operand, OperandRef};

    let link = |slug: &str| Operand {
        ref_: OperandRef::Name(NameRef::Link {
            title: title_for(model, slug),
            slug: slug.to_string(),
        }),
        axis: None,
        hints: Vec::new(),
    };

    let mut scratch = diagram.clone();
    scratch
        .layout
        .retain(|s| !placement_is_pair(s, subject_slug, reference_slug));
    scratch.layout.push(LayoutStatement::Placement {
        operands: vec![link(subject_slug), link(reference_slug)],
        directions: vec![dir],
    });

    let (_scene, diags) = build_scene(model, &scratch, expanded);
    diags.iter().any(|d| d.code == DiagCode::LayoutConflict)
}
```

> If `waml::diagnostic::DiagCode` is not the correct path for the diag-code enum used by `build_scene`'s diagnostics, match the import to the one `solve::geometry` uses (`Diagnostic::warn(DiagCode::LayoutConflict, ..)` — same `DiagCode`). The `d.code` field is already asserted public in `geometry.rs`'s `contradiction_warns_and_still_renders` test.

- [ ] **Step 4: Run the oracle tests to verify they PASS**

Run: `cargo test -p waml-editor --lib scene::oracle`
Expected: PASS.

- [ ] **Step 5: Run the full gate**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/scene.rs
git commit -m "feat(scene): speculative-solve conflict oracle for drag-time placement prediction"
```

---

### Task 5: Drag-time relation overlay

Draws the placement relations that touch the dragged node or the hovered target while a drag is in flight (Verify #3). The scope filter is a pure, unit-tested helper; the draw itself is immediate-mode canvas code, sign-off is interactive (a screenshot cannot drive a drag).

**Files:**
- Modify: `crates/waml-editor/src/canvas.rs` — a pure `relations_in_scope` helper; a draw pass in the drag branch of the widget's `draw_walk` (the compass block, ~:1123-1127, where `self.drag_target` is known)
- Test: `crates/waml-editor/src/canvas.rs` tests module (:1431)

**Interfaces:**
- Consumes: `Scene.relations` (`SceneRelation`, Task 3); `SceneNode.key`/`.rect`.
- Produces: `fn relations_in_scope<'a>(relations: &'a [SceneRelation], dragged_key: &str, target_key: &str) -> Vec<&'a SceneRelation>` — relations whose subject or reference equals either key.

- [ ] **Step 1: Write the failing scope-filter test**

Add to the tests module in `crates/waml-editor/src/canvas.rs`:

```rust
#[test]
fn relations_in_scope_keeps_only_relations_touching_dragged_or_target() {
    use crate::scene::SceneRelation;
    use waml::syntax::Direction;
    let rels = vec![
        SceneRelation {
            subject: "order".into(),
            reference: "customer".into(),
            dir: Direction::LeftOf,
        },
        SceneRelation {
            subject: "payment-gateway".into(),
            reference: "order".into(),
            dir: Direction::Below,
        },
        SceneRelation {
            subject: "invoice".into(),
            reference: "shipment".into(),
            dir: Direction::Above,
        },
    ];
    // Dragging `payment-gateway` over target `customer`.
    let got = relations_in_scope(&rels, "payment-gateway", "customer");
    // order<->customer touches the target; payment-gateway<->order touches the
    // dragged node; invoice<->shipment touches neither.
    assert_eq!(got.len(), 2);
    assert!(got.iter().all(|r| r.subject != "invoice"));
}
```

- [ ] **Step 2: Run to verify it FAILS**

Run: `cargo test -p waml-editor --lib canvas::relations_in_scope`
Expected: FAIL — compile error: `relations_in_scope` not found.

- [ ] **Step 3: Implement the pure filter**

In `crates/waml-editor/src/canvas.rs`, add a free function near `zone_placed` (after :429). Import `SceneRelation` where the other `crate::scene::*` types are used, or reference it fully:

```rust
/// The placement relations in scope for the drag overlay: those touching either
/// the dragged node or the hovered target (Feature 2 scope — dragged-node +
/// hover-target only, not the whole diagram). Pure.
fn relations_in_scope<'a>(
    relations: &'a [crate::scene::SceneRelation],
    dragged_key: &str,
    target_key: &str,
) -> Vec<&'a crate::scene::SceneRelation> {
    relations
        .iter()
        .filter(|r| {
            r.subject == dragged_key
                || r.reference == dragged_key
                || r.subject == target_key
                || r.reference == target_key
        })
        .collect()
}
```

- [ ] **Step 4: Draw the in-scope relations during a drag**

In the widget's `draw_walk`, inside the `if let Some(ti) = self.drag_target {` compass block (~:1123-1127), before the `self.draw_compass(..)` call, draw a light connector for each in-scope relation. Look each endpoint's rect up by key in `self.scene.nodes`, transform world→screen the same way `node_screen_center`/`draw_compass` do, and stroke a thin line from reference-center to subject-center using the existing `fill_rect` pen (a 2px slate line via a degenerate rect per orthogonal leg) — reuse the L-shaped elbow pattern from `scene::fallback_route` for an orthogonal look. Concretely:

```rust
        if let Some(ti) = self.drag_target {
            let dragged_key = self.drag_node.map(|ni| self.scene.nodes[ni].key.clone());
            let target_key = self.scene.nodes[ti].key.clone();
            if let Some(dk) = dragged_key.as_deref() {
                let scope = relations_in_scope(&self.scene.relations, dk, &target_key);
                for rel in scope {
                    let (Some(si), Some(ri)) = (
                        self.scene.nodes.iter().position(|n| n.key == rel.subject),
                        self.scene.nodes.iter().position(|n| n.key == rel.reference),
                    ) else {
                        continue;
                    };
                    let a = self.node_screen_center(si);
                    let b = self.node_screen_center(ri);
                    // Orthogonal L from reference (b) to subject (a): horizontal
                    // leg then vertical leg, 2px slate.
                    let ind = vec4(0.55, 0.62, 0.72, 0.7);
                    self.fill_rect(cx, a.x.min(b.x), b.y, (a.x - b.x).abs(), 2.0, ind);
                    self.fill_rect(cx, a.x, a.y.min(b.y), 2.0, (a.y - b.y).abs(), ind);
                }
            }
            let center = self.node_screen_center(ti);
            self.draw_compass(cx, center, self.compass_zone);
        }
```

> `node_screen_center(i)` already exists (used at :1125). If its signature differs, mirror the world→local transform `draw_compass`'s caller uses. Exact weight/tint/arrow-vs-line is a polish detail to settle interactively — keep it visually distinct from routed edges.

- [ ] **Step 5: Run the scope test to verify it PASSES**

Run: `cargo test -p waml-editor --lib canvas::relations_in_scope`
Expected: PASS.

- [ ] **Step 6: Run the full gate**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/waml-editor/src/canvas.rs
git commit -m "feat(canvas): draw dragged-node + hover-target placement relations during a drag"
```

- [ ] **Step 8: Interactive sign-off (user)**

Final sign-off is the user running `scripts/run-native.ps1 -Optimized <bundle>` on the `orders-diagram.md` fixture, starting a node drag, and confirming the relations touching the dragged node and the hovered target are drawn (and cleared on drop / Escape). No automated check covers this — a screenshot cannot drive a drag.

---

### Task 6: Conflict-red compass highlight

Paints a compass zone red when the solver would reject its drop (Verify #4). The canvas emits `CompassArmed` when it arms a new target; the view computes the eight per-zone verdicts via the Task 4 oracle and pushes them back; `draw_compass` reddens the flagged zones. The oracle is already unit-tested (Task 4); this task's automated gate is compile + workspace tests, with interactive sign-off.

**Files:**
- Modify: `crates/waml-editor/src/canvas.rs` — add `conflict_zones: Vec<Zone>` state, `set_conflict_zones`, a `GraphCanvasAction::CompassArmed` variant + emission on arm, clear on drag end/cancel, and reddening in `draw_compass`
- Modify: `crates/waml-editor/src/class_diagram_view.rs` — handle `CompassArmed` in `handle` (which already receives `&Model`), compute per-zone verdicts, push to canvas

**Interfaces:**
- Consumes: `placement_would_conflict` (Task 4); `zone_placed`, `COMPASS_ZONES`, `Zone` (Tasks 2 / existing).
- Produces: `GraphCanvasAction::CompassArmed { subject_key: String, reference_key: String }`; `GraphCanvas::set_conflict_zones(&mut self, cx: &mut Cx, zones: Vec<Zone>)`.

- [ ] **Step 1: Add the `CompassArmed` action and `conflict_zones` state**

In `crates/waml-editor/src/canvas.rs`, add a variant to `GraphCanvasAction` (after `AuthorPlacement`, ~:644):

```rust
    /// A node-drag armed the compass on a (new) target: the view computes the
    /// per-zone conflict verdicts (speculative solve) and pushes them back via
    /// `set_conflict_zones`. `subject` = dragged node, `reference` = target.
    CompassArmed {
        subject_key: String,
        reference_key: String,
    },
```

Add a field to the `GraphCanvas` widget struct (near `drag_place: Placed` at :262):

```rust
    conflict_zones: Vec<Zone>,
```

Add the setter (near `update_scene`/other `&mut self, cx` methods):

```rust
    /// Store the per-zone conflict verdict pushed by the view; repaint so the
    /// compass reddens the flagged zones on the next frame.
    pub fn set_conflict_zones(&mut self, cx: &mut Cx, zones: Vec<Zone>) {
        self.conflict_zones = zones;
        self.draw_bg.redraw(cx);
    }
```

- [ ] **Step 2: Emit `CompassArmed` when the compass arms on a new target**

In `handle_event`, where the dwell timer fires and sets `self.drag_target` to a node (the `self.dwell_timer.is_event(event)` block ~:659), after the target is assigned, emit the action with the dragged + target keys (guarded by both being present and the target differing from the last-armed one — clear `conflict_zones` immediately so stale verdicts never paint):

```rust
            if let (Some(ni), Some(ri)) = (self.drag_node, self.drag_target) {
                let uid = self.widget_uid();
                let subject_key = self.scene.nodes[ni].key.clone();
                let reference_key = self.scene.nodes[ri].key.clone();
                self.conflict_zones.clear();
                cx.widget_action(
                    uid,
                    GraphCanvasAction::CompassArmed {
                        subject_key,
                        reference_key,
                    },
                );
            }
```

> Match the exact place `self.drag_target` becomes `Some` on arm. If arming lives in the `FingerMove` target-selection path rather than the dwell block, emit there instead, gated so it fires only when the armed target *changes* (track a `last_armed: Option<usize>` to avoid re-solving every frame).

Also clear `conflict_zones` wherever the drag ends: in the `FingerUp`/drop reset blocks (:857-864, :870-877) and in `cancel_drag` (:1245-1256), add `self.conflict_zones.clear();`.

- [ ] **Step 3: Redden flagged zones in `draw_compass`**

In `draw_compass` (:1167), compute per-zone conflict and override the fill/arrow when set. Inside the `for z in COMPASS_ZONES` loop, after `let on = active == Some(z);`:

```rust
            let conflict = self.conflict_zones.contains(&z);
            let (fill, line, arrow) = if conflict {
                (
                    vec4(0.80, 0.22, 0.22, 0.94),
                    vec4(1.0, 0.72, 0.72, 1.0),
                    vec4(1.0, 0.90, 0.90, 1.0),
                )
            } else if on {
                (
                    vec4(0.37, 0.63, 1.0, 0.94),
                    vec4(0.75, 0.86, 1.0, 1.0),
                    vec4(1.0, 1.0, 1.0, 1.0),
                )
            } else {
                (
                    vec4(0.13, 0.17, 0.24, 0.86),
                    vec4(0.37, 0.63, 1.0, 0.60),
                    vec4(0.66, 0.79, 1.0, 0.95),
                )
            };
```

(Replace the existing `let (fill, line, arrow) = if on { .. } else { .. };` at :1171-1183 with the three-way form above.)

- [ ] **Step 4: Handle `CompassArmed` in the view — compute + push verdicts**

In `crates/waml-editor/src/class_diagram_view.rs`, add an arm to the canvas-action `match` in `handle` (alongside `AuthorPlacement`, ~:218). `handle` already has `model: &Model`:

```rust
            Some(crate::canvas::GraphCanvasAction::CompassArmed {
                subject_key,
                reference_key,
            }) => {
                if let Some(diagram) =
                    model.diagrams.iter().find(|d| d.key == self.active_key)
                {
                    let subject = strip_md_key(&subject_key);
                    let reference = strip_md_key(&reference_key);
                    let mut red = Vec::new();
                    for z in crate::canvas::COMPASS_ZONES {
                        if let Some(dir) = crate::canvas::zone_placed(z).dir {
                            if crate::scene::placement_would_conflict(
                                model,
                                diagram,
                                &subject,
                                &reference,
                                dir,
                                &self.expanded,
                            ) {
                                red.push(z);
                            }
                        }
                    }
                    if let Some(mut canvas) =
                        body.canvas(cx).borrow_mut::<crate::canvas::GraphCanvas>()
                    {
                        canvas.set_conflict_zones(cx, red);
                    }
                }
                return out;
            }
```

Add a small helper at the top of `class_diagram_view.rs` (mirroring the `strip_md` closure already used inline in the `AuthorPlacement` arm at :230) so both arms share it:

```rust
/// Strip a defensive `.md` tail from a node/diagram key.
fn strip_md_key(s: &str) -> String {
    s.strip_suffix(".md").unwrap_or(s).to_string()
}
```

Ensure `COMPASS_ZONES`, `Zone`, and `zone_placed` are `pub` in `canvas.rs` (they are `pub`/`pub const` already — `zone_placed` is `pub fn`, `Zone` is `pub enum`; make `COMPASS_ZONES` `pub const` if it is not already).

- [ ] **Step 5: Run the full gate**

Run: `cargo test --workspace`
Expected: PASS (compiles; Task 4's oracle tests + all prior tests green). No new unit test here — the oracle verdict is covered by Task 4; this task is wiring + draw.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/canvas.rs crates/waml-editor/src/class_diagram_view.rs
git commit -m "feat(canvas): conflict-red compass zones via speculative-solve oracle"
```

- [ ] **Step 7: Interactive sign-off (user)**

Final sign-off is the user running `scripts/run-native.ps1 -Optimized <bundle>` on `orders-diagram.md`, dragging a node so the compass arms on a target, and confirming: a corner drop moves the node diagonally with no leftover conflict; a zone whose drop the solver would reject is painted red before the drop; the red clears when the target changes or the drag ends. A screenshot cannot drive a drag, so no automated check covers the live gesture.

---

## Self-Review

**Spec coverage:**
- Feature 1 (diagonal primitive): Task 1 — enum, solver arms (both-axis separation, no center-align), validate arms, serialize + parse, op-layer characterization. Verify #1/#2 met (Task 1 solver test + Task 2 corner emission).
- Feature 2 (drag-time visibility): Task 3 (projection plumbing) + Task 5 (overlay draw). Verify #3.
- Feature 3 (conflict-red): Task 4 (oracle, unit-tested) + Task 6 (wiring + red draw). Verify #4.
- "No op / DTO change" honored (Task 1 Step 14 is a characterization test, not a production edit).
- Verify #5 (`cargo test --workspace` green + user live sign-off): every task's gate + Tasks 5/6 interactive steps.

**Ordering:** the pure `waml` primitive (Task 1) lands fully and first; canvas UI (2, 5, 6) and the projection/oracle (3, 4) follow, each a small committable green unit.

**Type consistency:** `Placed.dir` (Task 2) is consumed by the Task 2 emission + readout only. `SceneRelation`/`Scene.relations` (Task 3) is consumed by Tasks 4 (oracle build_scene reuse), 5 (`relations_in_scope`), and 6 (indirectly). `placement_would_conflict` signature (Task 4) matches its Task 6 call site exactly. `GraphCanvasAction::CompassArmed` + `set_conflict_zones` (Task 6) are defined and consumed within Task 6.
