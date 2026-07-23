### Task 1: Solver dropped-constraint report

**Files:**
- Modify: `crates/waml/src/solve/geometry.rs` (add `DroppedPlacement`, helpers, thread through `solve_cluster` → `assemble` → `solve_box` → `solve_with_rects`; add tests)
- Modify: `crates/waml/src/solve/mod.rs` (re-export `DroppedPlacement`; add native-only `solve_diagram_reported`; keep `solve_diagram` a 2-tuple wrapper)

**Interfaces:**
- Produces (consumed by Task 2):
  - `waml::solve::DroppedPlacement { pub relation: Constraint, pub conflicts_with: Vec<Constraint> }`
  - `waml::solve::solve_diagram_reported(diagram: &Diagram, edges: &[(BoxId, BoxId)], sizes: &SizeMap, cfg: &SolveConfig) -> (Solved, Vec<Diagnostic>, Vec<DroppedPlacement>)`
- Unchanged (Global Constraints): `waml::solve::solve_diagram(...) -> (Solved, Vec<Diagnostic>)` and `waml::solve::solve(...) -> (Solved, Vec<Diagnostic>)` keep their exact signatures. `Constraint`/`BoxId`/`Direction` unchanged.

**Background:** In `solve_cluster` today each `Constraint::Place` runs two `eq(...)` calls; `eq` calls `Potentials::union`, and on a contradiction pushes a generic `LayoutConflict` diagnostic and drops the equality silently. We add honest instrumentation: record, per axis, every equality that DID union; when one fails, emit a `DroppedPlacement` naming the dropped placement + the constraints in the connected component it could not join (its contradiction set). Diagnostics are UNCHANGED (existing tests that assert `diags.len()` stay green).

---

- [ ] **Step 1: Write the failing solver tests**

Add these two tests inside the existing `#[cfg(test)] mod tests` in `crates/waml/src/solve/geometry.rs` (the module already imports `super::*`, `pretty`, `FlagSet`, `Margin`, `Shape`, and has `leaf`/`sizes` helpers):

```rust
#[test]
fn satisfiable_set_drops_nothing() {
    // A clean row of three: no contradiction, so the dropped report is empty.
    let scene = Scene {
        boxes: vec![leaf("a"), leaf("b"), leaf("c")],
        constraints: vec![
            Constraint::Place { a: BoxId::Node("a".into()), b: BoxId::Node("b".into()), dir: Direction::LeftOf },
            Constraint::Place { a: BoxId::Node("b".into()), b: BoxId::Node("c".into()), dir: Direction::LeftOf },
        ],
    };
    let (_solved, diags, dropped) = solve_diagram_reported_from_scene(&scene);
    assert!(diags.is_empty());
    assert!(dropped.is_empty(), "satisfiable set must drop nothing: {dropped:?}");
}

#[test]
fn three_node_cycle_names_dropped_and_conflict_set() {
    // a left of b, b left of c, c left of a: the third placement cannot join the
    // rigid a-b-c x-component. It is dropped; its conflict set is the two prior
    // placements that formed the component.
    let scene = Scene {
        boxes: vec![leaf("a"), leaf("b"), leaf("c")],
        constraints: vec![
            Constraint::Place { a: BoxId::Node("a".into()), b: BoxId::Node("b".into()), dir: Direction::LeftOf },
            Constraint::Place { a: BoxId::Node("b".into()), b: BoxId::Node("c".into()), dir: Direction::LeftOf },
            Constraint::Place { a: BoxId::Node("c".into()), b: BoxId::Node("a".into()), dir: Direction::LeftOf },
        ],
    };
    let (_solved, _diags, dropped) = solve_diagram_reported_from_scene(&scene);
    assert_eq!(dropped.len(), 1, "exactly one placement is unsatisfiable: {dropped:?}");
    let d = &dropped[0];
    assert_eq!(
        d.relation,
        Constraint::Place { a: BoxId::Node("c".into()), b: BoxId::Node("a".into()), dir: Direction::LeftOf },
        "the third (cycle-closing) placement is the dropped one"
    );
    assert_eq!(d.conflicts_with.len(), 2, "conflict set is the two prior placements: {:?}", d.conflicts_with);
    assert!(d.conflicts_with.contains(
        &Constraint::Place { a: BoxId::Node("a".into()), b: BoxId::Node("b".into()), dir: Direction::LeftOf }));
    assert!(d.conflicts_with.contains(
        &Constraint::Place { a: BoxId::Node("b".into()), b: BoxId::Node("c".into()), dir: Direction::LeftOf }));
    // The dropped relation must NOT list itself.
    assert!(!d.conflicts_with.contains(&d.relation));
}

// Test-only shim: solve a bare `Scene` through the reported geometry path with
// default equal sizes, so the tests read like the existing `solve(...)` ones.
fn solve_diagram_reported_from_scene(scene: &Scene) -> (Solved, Vec<Diagnostic>, Vec<DroppedPlacement>) {
    let keys: Vec<&str> = scene.boxes.iter().filter_map(|b| match &b.id {
        BoxId::Node(k) => Some(k.as_str()),
        _ => None,
    }).collect();
    let (solved, _rects, diags, dropped) =
        solve_with_rects(scene, &[], &sizes(&keys, 200.0, 90.0), &SolveConfig::default());
    (solved, diags, dropped)
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml --lib solve::geometry`
Expected: FAIL to compile — `DroppedPlacement` is undefined and `solve_with_rects` returns a 3-tuple, not 4.

- [ ] **Step 3: Add `DroppedPlacement` + the two pure helpers in `geometry.rs`**

Immediately after the `MIN_ASSOC` const (near the top of `geometry.rs`), add:

```rust
/// A placement the solver could not honor, plus the constraints it contradicts.
/// Native-only instrumentation (no wasm ABI); surfaced through
/// `solve_diagram_reported` to the editor's conflict error list. `relation` is
/// the dropped `Constraint::Place`; `conflicts_with` is the set of already-applied
/// constraints in the same-axis rigid component it could not join.
#[derive(Debug, Clone, PartialEq)]
pub struct DroppedPlacement {
    pub relation: Constraint,
    pub conflicts_with: Vec<Constraint>,
}

/// The (px, py) coordinate deltas a `Place` direction imposes: `coord[a] - coord[b]`
/// on each axis. Extracted from the per-direction match so the union path and the
/// dropped-report path agree on exactly what was attempted. Pure.
fn place_deltas(dir: Direction, sa: Size, sb: Size, gap: f64) -> (f64, f64) {
    use Direction::*;
    match dir {
        LeftOf => (sa.w + gap, (sa.h - sb.h) / 2.0),
        RightOf => (-(sb.w + gap), (sa.h - sb.h) / 2.0),
        Above => ((sa.w - sb.w) / 2.0, sa.h + gap),
        Below => ((sa.w - sb.w) / 2.0, -(sb.h + gap)),
        AboveLeft => (sa.w + gap, sa.h + gap),
        AboveRight => (-(sb.w + gap), sa.h + gap),
        BelowLeft => (sa.w + gap, -(sb.h + gap)),
        BelowRight => (-(sb.w + gap), -(sb.h + gap)),
    }
}

/// Constraint indices whose recorded same-axis edges form the rigid component
/// containing node `a` (fixpoint expansion over the undirected edge list). Called
/// only after a fresh union between `a` and some `b` FAILED — so `a`'s component
/// already pins `b`, and every edge in it participates in the contradiction.
fn component_constraints(edges: &[(usize, usize, usize)], a: usize) -> Vec<usize> {
    use std::collections::BTreeSet;
    let mut comp: BTreeSet<usize> = BTreeSet::new();
    comp.insert(a);
    let mut changed = true;
    while changed {
        changed = false;
        for &(_ci, u, v) in edges {
            if comp.contains(&u) != comp.contains(&v) {
                comp.insert(u);
                comp.insert(v);
                changed = true;
            }
        }
    }
    edges
        .iter()
        .filter(|&&(_ci, u, v)| comp.contains(&u) && comp.contains(&v))
        .map(|&(ci, _, _)| ci)
        .collect()
}

/// Union `coord[b] - coord[a] = delta` on one axis, recording the successful edge
/// (tagged with its constraint index `ci`) or pushing a `LayoutConflict` diag on a
/// contradiction. Returns whether it unioned. Preserves the exact diagnostic the
/// old `eq` emitted for `Place`.
fn apply_axis(
    p: &mut Potentials,
    edges: &mut Vec<(usize, usize, usize)>,
    ci: usize,
    ia: usize,
    ib: usize,
    delta: f64,
    diags: &mut Vec<Diagnostic>,
) -> bool {
    if p.union(ia, ib, delta).is_ok() {
        edges.push((ci, ia, ib));
        true
    } else {
        diags.push(Diagnostic::warn(
            DiagCode::LayoutConflict,
            "conflicting layout constraint dropped",
            "",
            0,
        ));
        false
    }
}
```

- [ ] **Step 4: Thread `dropped` through `solve_cluster` and rewrite its `Place` arm**

Change the `solve_cluster` signature to accept the accumulator (add the last param):

```rust
pub(super) fn solve_cluster(
    ids: &[BoxId],
    dims: &BTreeMap<BoxId, (Size, Margin)>,
    constraints: &[Constraint],
    connected: &BTreeSet<(BoxId, BoxId)>,
    cfg: &SolveConfig,
    diags: &mut Vec<Diagnostic>,
    dropped: &mut Vec<DroppedPlacement>,
) -> BTreeMap<BoxId, Rect> {
```

Right after `let mut px = Potentials::new(n);` / `let mut py = Potentials::new(n);`, add the per-axis edge logs:

```rust
    // Per-axis successfully-applied equalities, tagged with their constraint
    // index, so a failed union can name the rigid component it collided with.
    let mut xedges: Vec<(usize, usize, usize)> = Vec::new();
    let mut yedges: Vec<(usize, usize, usize)> = Vec::new();
```

Change the constraint loop header to carry the index:

```rust
    for (ci, c) in constraints.iter().enumerate() {
```

Replace the entire `Constraint::Place { a, b, dir } => { ... }` arm (all eight direction branches) with:

```rust
            Constraint::Place { a, b, dir } => {
                let (Some(&ia), Some(&ib)) = (index.get(a), index.get(b)) else {
                    continue;
                };
                let (sa, ma) = dims[a];
                let (sb, mb) = dims[b];
                let gap = cfg.margin(max_margin(ma, mb));
                let gap = if connected.contains(&pair(a, b)) {
                    gap.max(MIN_ASSOC)
                } else {
                    gap
                };
                let (dx, dy) = place_deltas(*dir, sa, sb, gap);
                let okx = apply_axis(&mut px, &mut xedges, ci, ia, ib, dx, diags);
                let oky = apply_axis(&mut py, &mut yedges, ci, ia, ib, dy, diags);
                if !okx || !oky {
                    // The dropped placement's contradiction set = the rigid
                    // component on the failing axis it could not join (prefer x).
                    let edges_axis: &[(usize, usize, usize)] = if !okx { &xedges } else { &yedges };
                    let conflicts_with: Vec<Constraint> = component_constraints(edges_axis, ia)
                        .into_iter()
                        .filter(|&k| k != ci)
                        .map(|k| constraints[k].clone())
                        .collect();
                    dropped.push(DroppedPlacement {
                        relation: c.clone(),
                        conflicts_with,
                    });
                }
            }
```

Leave the `Constraint::Align { .. }` arm exactly as-is (it still uses `eq`, which stays). The old standalone `fn eq(...)` stays — `Align` still calls it.

- [ ] **Step 5: Thread `dropped` through `assemble`, `solve_box`, `solve_with_rects`**

In `assemble`, add `dropped: &mut Vec<DroppedPlacement>` as the final parameter and forward it to the `solve_cluster` call:

```rust
    let placed = solve_cluster(children, &dims, cons, connected, cfg, diags, dropped);
```

In `solve_box`, add `dropped: &mut Vec<DroppedPlacement>` as the final parameter; forward it to the recursive `solve_box(...)` call inside the `for c in &b.children` loop and to the `assemble(...)` call.

In `solve_with_rects`, create the accumulator and thread it, then return it as a 4th tuple element. Change the return type and body:

```rust
pub(super) fn solve_with_rects(
    scene: &Scene,
    edges: &[(BoxId, BoxId)],
    sizes: &SizeMap,
    cfg: &SolveConfig,
) -> (Solved, BTreeMap<BoxId, Rect>, Vec<Diagnostic>, Vec<DroppedPlacement>) {
    let mut diags = vec![];
    let mut dropped = vec![];
    // ... unchanged setup ...
```

Forward `&mut dropped` into both the `solve_box(...)` call (inside the `for r in &roots` loop) and the final root-level `assemble(...)` call, and change the final return to:

```rust
    (
        Solved { nodes, groups, flags, routes: Vec::new() },
        laid.rects,
        diags,
        dropped,
    )
}
```

- [ ] **Step 6: Update `solve()` and the geometry-internal test destructures**

In `solve()` (top of `geometry.rs`), update the destructure to ignore the new element:

```rust
pub fn solve(scene: &Scene, sizes: &SizeMap, cfg: &SolveConfig) -> (Solved, Vec<Diagnostic>) {
    let (solved, _rects, diags, _dropped) = solve_with_rects(scene, &[], sizes, cfg);
    (solved, diags)
}
```

Update every existing test in `geometry.rs` that destructures `solve_with_rects` to a 3-tuple to add `, _dropped`:
- `solve_with_rects_keys_group_frames_by_boxid`: `let (_solved, rects, diags, _dropped) = solve_with_rects(...)`
- `connected_adjacent_pair_gets_min_assoc_gap`: `let (_solved, rects, diags, _dropped) = solve_with_rects(...)`
- `unconnected_adjacent_pair_keeps_margin_gap`: `let (_solved, rects, diags, _dropped) = solve_with_rects(...)`
- `group_flow_connected_siblings_spread`: `let (_solved, rects, diags, _dropped) = solve_with_rects(...)`

The `.1`-indexed calls in `min_assoc_layout_is_deterministic` still compile unchanged (index 1 is still `rects`).

- [ ] **Step 7: Add `DroppedPlacement` re-export + `solve_diagram_reported` in `mod.rs`**

In `crates/waml/src/solve/mod.rs`, after the existing `pub use wire::{...}` line, add:

```rust
pub use geometry::DroppedPlacement;
```

Replace `solve_diagram` (near the bottom of `mod.rs`) so it wraps a new reported entry point (keeping its public 2-tuple signature intact for the wasm caller):

```rust
/// Top-level entry: resolve the diagram to a `Scene`, then solve it. Keeps the
/// 2-tuple shape the wasm crate depends on; drops the placement report.
pub fn solve_diagram(
    diagram: &crate::model::Diagram,
    edges: &[(BoxId, BoxId)],
    sizes: &SizeMap,
    cfg: &SolveConfig,
) -> (Solved, Vec<Diagnostic>) {
    let (solved, diags, _dropped) = solve_diagram_reported(diagram, edges, sizes, cfg);
    (solved, diags)
}

/// Native-only entry: like `solve_diagram` but also returns the solver's
/// dropped-placement report (unsatisfiable placements + their contradiction sets).
/// The editor's conflict error list consumes this; the wasm path uses
/// `solve_diagram` and never sees it.
pub fn solve_diagram_reported(
    diagram: &crate::model::Diagram,
    edges: &[(BoxId, BoxId)],
    sizes: &SizeMap,
    cfg: &SolveConfig,
) -> (Solved, Vec<Diagnostic>, Vec<DroppedPlacement>) {
    let (scene, mut diags) = resolve::resolve(diagram);
    let (mut solved, rects, mut geo_diags, dropped) =
        geometry::solve_with_rects(&scene, edges, sizes, cfg);
    diags.append(&mut geo_diags);
    solved.routes = route::route(&scene.boxes, &rects, edges, cfg);
    (solved, diags, dropped)
}
```

- [ ] **Step 8: Run the tests to verify they pass**

Run: `cargo test -p waml --lib solve`
Expected: PASS — including the two new tests and the untouched `contradiction_warns_and_still_renders` (still 1 diag) and `solves_a_row_of_three`.

- [ ] **Step 9: Run the full gate**

Run: `cargo test --workspace`
Expected: PASS (the golden test `crates/waml/tests/solver_golden.rs` calls `solve_diagram` — unchanged signature — so it is unaffected).
Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 10: Commit**

```bash
git add crates/waml/src/solve/geometry.rs crates/waml/src/solve/mod.rs
git commit -m "feat(solve): report dropped placements + contradiction sets"
```
