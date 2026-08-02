# Edge Label Route Pressure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the router prefer paths that leave room for their labels, and give a leader line to any label that still cannot be placed — so no label is ever drawn on top of something.

**Architecture:** `route.rs`'s hardcoded A\* penalties become a named `RouteCost` struct — the seam future layout tuning extends. A new `label_pressure` weight penalises OVG edges whose label-height band collides with a hard obstacle, so the router is *continuously* steered toward routes with label room rather than being hard-blocked. A bounded loop reroutes only the edges whose labels failed and re-places them. Whatever survives gets a leader line into free space, found by expanding-ring search — total, because space outside the content bounding box is always empty.

**Tech Stack:** Rust, `waml` crate, `waml-editor` (makepad).

> **Gate:** rust-only

## Prerequisite

**This plan requires `docs/superpowers/plans/2026-08-02-edge-label-placement.md` to be fully landed first.** It builds directly on `solve::label::{place, Placement, LabelRequest, Obstacles, LabelConfig}` and on `Solved.labels`, none of which exist until that plan is done.

Before starting, confirm the prerequisite landed:

```bash
git log --oneline --grep="size connected gaps to hold their terminal labels"
```

If that returns nothing, stop — this plan cannot be implemented yet.

## Global Constraints

- The `waml` crate is pure and wasm-clean. No rendering backend, no platform APIs, no `Date`/RNG.
- All routing and placement must stay deterministic: same input, same output.
- **Existing route goldens must be byte-identical after Task 1.** That is the regression gate for the riskiest part of this work, and it is not negotiable — if they move, the refactor changed behaviour and must be fixed before proceeding.
- The green gate is `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`. Note clippy runs with `-D warnings` across the whole workspace: any `dead_code` left behind is a hard build failure, not a warning.
- The gate runs `cargo fmt --all --check`; run `cargo fmt --all` before committing.

---

### Task 1: Extract RouteCost as a no-op refactor

`astar` bakes its length and bend penalties in as constants. This task lifts them into a struct without changing a single routed pixel — the whole point is to prove the seam is inert before anything is built on it.

**Files:**
- Modify: `crates/waml/src/solve/route.rs` (`astar` around line 455, `route_keyed` around line 37)
- Test: inline `mod tests` in `crates/waml/src/solve/route.rs`

**Interfaces:**
- Produces:
  - `pub struct RouteCost { pub length: f64, pub bend: f64, pub label_pressure: f64 }`
  - `impl Default for RouteCost` reproducing today's constants exactly
  - `pub fn route_keyed_with(boxes, rects, edges, cfg, cost: &RouteCost) -> Vec<Route>`

- [ ] **Step 1: Capture the current behaviour as a golden**

Before touching anything, pin what the router does today so the refactor is provably inert.

```rust
#[test]
fn route_cost_default_reproduces_the_legacy_router() {
    let (boxes, rects, edges) = three_box_bent_case();
    let legacy = route(&boxes, &rects, &edges, &SolveConfig::default());
    let via_cost = route_keyed_with(
        &boxes,
        &rects,
        &edges.iter().map(|(s, t)| (s.clone(), t.clone(), None)).collect::<Vec<_>>(),
        &SolveConfig::default(),
        &RouteCost::default(),
    );
    assert_eq!(legacy, via_cost, "default weights must not move a single point");
}
```

Build `three_box_bent_case()` from the existing route tests near `route.rs:1149` — pick a case that actually bends, since a straight route would not exercise the bend penalty at all.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p waml route_cost_default_reproduces_the_legacy_router`
Expected: FAIL — `cannot find struct RouteCost`.

- [ ] **Step 3: Read the current constants**

Read `crates/waml/src/solve/route.rs::astar` (from line 455) and write down the exact literal values used for step cost and bend penalty. The `Default` impl must use these values verbatim — a "tidier" round number here silently moves every route in the repo.

- [ ] **Step 4: Add the struct**

```rust
/// Weights for the router's A* cost function.
///
/// Lifted out of `astar`'s hardcoded constants so layout tuning has one place
/// to live instead of each change inventing its own mechanism. `Default`
/// reproduces the legacy constants exactly: adding a weight must never move a
/// route until that weight is deliberately changed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RouteCost {
    /// Cost per unit of path length.
    pub length: f64,
    /// Cost per direction change.
    pub bend: f64,
    /// Cost per unit of label band that collides with a hard obstacle. Zero in
    /// `Default`, and applied only to edges that actually carry a label, so
    /// unlabelled edges route byte-identically to before this existed.
    pub label_pressure: f64,
}
```

Write the `Default` impl with the literals from Step 3 and `label_pressure: 0.0`.

- [ ] **Step 5: Thread it through**

Add `route_keyed_with(..., cost: &RouteCost)`, make `route_keyed` delegate to it with `RouteCost::default()`, and pass `cost` down into `astar`. Replace the hardcoded literals in `astar` with `cost.length` / `cost.bend`.

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p waml route_cost_default_reproduces_the_legacy_router`
Expected: PASS

- [ ] **Step 7: Prove it against every existing golden**

Run: `cargo test --workspace 2>&1 | grep -E "^test result" | grep -v "0 failed"`
Expected: **no output, with zero golden files modified.**

Run: `git status --short`
Expected: only `route.rs` modified. If any golden fixture changed, the refactor was not inert — find the discrepancy rather than re-baselining. Re-baselining here would destroy the one regression gate this plan has.

- [ ] **Step 8: Commit**

Commit this unit. Suggested message:
```text
refactor(solve): lift A* penalties into RouteCost

No behaviour change -- default weights are the previous literals, proven
by every existing route golden being byte-identical. This is the seam
label pressure and future layout tuning hang off, so it lands on its own
where 'nothing moved' is a checkable claim.
```

The harness appends the `Plan:` / `Plan-Tasks:` trailers and the attribution footer; do not write them by hand, and do not run `git commit` yourself if the harness commits for you.

---

### Task 2: Add the label_pressure term

**Files:**
- Modify: `crates/waml/src/solve/route.rs`
- Test: inline `mod tests` in `crates/waml/src/solve/route.rs`

**Interfaces:**
- Consumes: `RouteCost` from Task 1.
- Produces: `route_keyed_with` honours `cost.label_pressure`; edges opt in via a per-edge label height.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn label_pressure_steers_a_route_toward_room_for_its_label() {
    // A corridor with two viable paths of near-equal length: one hugs an
    // obstacle so tightly no label band fits beside it, the other has clearance.
    // With label_pressure at zero the router may take either; with it weighted,
    // it must take the roomy one.
    let (boxes, rects, edges) = corridor_with_tight_and_roomy_paths();
    let roomy = route_keyed_with(
        &boxes,
        &rects,
        &labelled(&edges, 12.0),
        &SolveConfig::default(),
        &RouteCost { label_pressure: 50.0, ..RouteCost::default() },
    );
    assert!(
        clearance_beside(&roomy[0].points) >= 12.0,
        "route should leave a label band's worth of room"
    );
}

#[test]
fn an_unlabelled_edge_is_untouched_by_label_pressure() {
    let (boxes, rects, edges) = corridor_with_tight_and_roomy_paths();
    let keyed: Vec<_> = edges.iter().map(|(s, t)| (s.clone(), t.clone(), None)).collect();
    let baseline = route_keyed_with(&boxes, &rects, &keyed, &SolveConfig::default(), &RouteCost::default());
    let pressured = route_keyed_with(
        &boxes,
        &rects,
        &keyed,
        &SolveConfig::default(),
        &RouteCost { label_pressure: 50.0, ..RouteCost::default() },
    );
    assert_eq!(baseline, pressured, "no label means no pressure");
}
```

You will need to build `corridor_with_tight_and_roomy_paths()` and the `clearance_beside` helper. Construct the corridor deliberately: two obstacles forming a narrow channel and a wide one, with the wide detour only slightly longer, so the length term alone would pick the narrow channel. If the two paths differ too much in length, the test proves nothing — the length weight would dominate regardless.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml label_pressure`
Expected: FAIL — the edge tuple has no label-height field.

- [ ] **Step 3: Give edges an optional label height**

Extend the keyed-edge tuple to carry `Option<f64>` (the label band height for that edge, `None` when it has no label). Update `route_keyed` to pass `None` for every edge, keeping its behaviour identical.

- [ ] **Step 4: Implement the band check**

In `astar`'s edge relaxation, when `cost.label_pressure > 0.0` **and** the edge under routing has a label height, compute the blocked fraction: form a band of that height alongside the OVG edge on each side, test it against the same inflated obstacle list A\* already holds, and add `cost.label_pressure * blocked_fraction * segment_length`. Take the *better* of the two sides — a label only needs one clear side.

Cache the per-OVG-edge band result in a map keyed by the edge index; the same edge is relaxed many times during the search and recomputing the rect queries each visit is the difference between "a bit slower" and "unusable on a large diagram".

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p waml label_pressure`
Expected: PASS, 2 tests.

- [ ] **Step 6: Check the performance cost honestly**

Run the largest fixture available and time routing before and after:

```bash
cargo test -p waml --release stress_layout_pins_to_expected_pixels -- --nocapture
```

If routing time on a realistic diagram more than doubles, do not tune the cache further in this task — instead record the measured numbers in the commit message and note that evaluating `label_pressure` only on the reroute pass (Task 3) rather than the first pass is the available knob. That is a documented tradeoff, not a silent regression.

- [ ] **Step 7: Run the full gate and commit**

Run: `cargo test --workspace 2>&1 | grep -E "^test result" | grep -v "0 failed"`
Expected: no output. Existing goldens must still be unchanged, because `Default` still has `label_pressure: 0.0`.

Commit this unit. Suggested message:
```text
feat(solve): add a label_pressure term to the router

Penalises OVG edges whose label-height band collides with a hard obstacle,
so the router PREFERS paths with room for their labels rather than only
being hard-blocked out of bad ones. Weighted zero by default and applied
only to edges that carry a label, so nothing routes differently until it
is deliberately switched on.
```

The harness appends the `Plan:` / `Plan-Tasks:` trailers and the attribution footer; do not write them by hand, and do not run `git commit` yourself if the harness commits for you.

---

### Task 3: Bounded reroute-and-replace loop

**Files:**
- Modify: `crates/waml/src/solve/mod.rs` (`place_labels`)
- Test: inline `mod tests` in `crates/waml/src/solve/mod.rs`

**Interfaces:**
- Consumes: `route_keyed_with`, `RouteCost`, `label::place`.
- Produces: `Solved.label_reroutes: usize`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn a_label_that_cannot_be_placed_triggers_a_bounded_reroute() {
    let mut solved = crowded_scene_where_one_label_cannot_fit();
    let before = solved.routes.clone();
    place_labels(&mut solved, &requests_for(&solved), &label::LabelConfig::default());

    assert!(solved.label_reroutes >= 1, "the failure should have been retried");
    assert_ne!(before, solved.routes, "the blocked edge should have moved");
    // Every other edge is untouched: rerouting is surgical, not global.
    assert_eq!(before.len(), solved.routes.len());
}

#[test]
fn rerouting_stops_after_the_bound_even_when_it_never_succeeds() {
    // A scene with no possible label position at all must terminate, not spin.
    let mut solved = scene_with_no_free_space();
    place_labels(&mut solved, &requests_for(&solved), &label::LabelConfig::default());
    assert!(solved.label_reroutes <= MAX_REROUTE_ROUNDS);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml reroute`
Expected: FAIL — `Solved` has no field `label_reroutes`.

- [ ] **Step 3: Implement the loop**

```rust
/// How many times a failed label may ask the router to try again. Bounded
/// because the loop is not guaranteed to converge: a reroute that frees one
/// label can block another, and without a ceiling that oscillates.
const MAX_REROUTE_ROUNDS: usize = 2;
```

In `place_labels`, after the first `label::place` call: while there are unplaced labels and rounds remain, re-route **only the edges those labels belong to**, with `label_pressure` weighted and the edge's label height supplied; then re-run placement for the affected edges only. Increment `solved.label_reroutes` per edge actually rerouted.

Leaving every other edge alone is what keeps existing routes stable — assert it in the test rather than trusting it.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p waml reroute`
Expected: PASS, 2 tests.

- [ ] **Step 5: Run the full gate and commit**

Run: `cargo test --workspace 2>&1 | grep -E "^test result" | grep -v "0 failed"`
Expected: no output.

Commit this unit. Suggested message:
```text
feat(solve): reroute edges whose labels could not be placed

Bounded to 2 rounds -- the loop is not guaranteed to converge, since a
reroute that frees one label can block another. Only edges with a failed
label are rerouted, so every other route stays byte-identical; the test
asserts that rather than trusting it.
```

The harness appends the `Plan:` / `Plan-Tasks:` trailers and the attribution footer; do not write them by hand, and do not run `git commit` yourself if the harness commits for you.

---

### Task 4: Leader lines for the residue

**Files:**
- Modify: `crates/waml/src/solve/label.rs`, `crates/waml/src/solve/mod.rs`
- Modify: `crates/waml-editor/src/canvas/class/render/labels.rs` (draw the leader)
- Test: inline `mod tests` in `crates/waml/src/solve/label.rs`

**Interfaces:**
- Produces:
  - `PlacedLabel.leader: Option<[(f64, f64); 2]>`
  - `pub fn place_with_leaders(...) -> Placement` (no `unplaced` remains)
  - `Solved.label_leaders: usize`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_label_with_nowhere_to_go_gets_a_leader_into_free_space() {
    let obstacles = Obstacles {
        hard: vec![Rect { x: -200.0, y: -200.0, w: 800.0, h: 400.0 }],
        soft: vec![],
    };
    let reqs = vec![LabelRequest { edge: 0, slot: LabelSlot::MidRoute, text: "places".into() }];
    let out = place_with_leaders(&route_pair(), &reqs, &obstacles, &LabelConfig::default());

    assert!(out.unplaced.is_empty(), "leader lines make placement total");
    let placed = &out.placed[0];
    assert!(!collides(placed.rect, &obstacles.hard), "leader target must be free");
    let leader = placed.leader.expect("a displaced label carries a leader");
    assert_eq!(leader[0], placed.attach, "leader starts on the route");
}

#[test]
fn a_label_that_fits_normally_gets_no_leader() {
    let reqs = vec![LabelRequest { edge: 0, slot: LabelSlot::MidRoute, text: "places".into() }];
    let out = place_with_leaders(&route_pair(), &reqs, &Obstacles::default(), &LabelConfig::default());
    assert!(out.placed[0].leader.is_none());
}

#[test]
fn the_leader_search_terminates_on_a_hostile_scene() {
    // Obstacles cannot cover the whole plane, so an expanding ring always finds
    // free space eventually. This pins that the search actually exploits it.
    let obstacles = Obstacles {
        hard: (0..50)
            .map(|i| Rect { x: i as f64 * 40.0, y: 0.0, w: 40.0, h: 400.0 })
            .collect(),
        soft: vec![],
    };
    let reqs = vec![LabelRequest { edge: 0, slot: LabelSlot::MidRoute, text: "x".into() }];
    let out = place_with_leaders(&route_pair(), &reqs, &obstacles, &LabelConfig::default());
    assert_eq!(out.placed.len(), 1);
    assert!(out.unplaced.is_empty());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml leader`
Expected: FAIL — `cannot find function place_with_leaders`.

- [ ] **Step 3: Implement the ring search**

Add `leader: Option<[(f64, f64); 2]>` to `PlacedLabel` (this breaks its literals — fix each), then:

```rust
/// Place every request, falling back to a leader line for any that will not fit
/// beside their route.
///
/// This is TOTAL: obstacles are finite, so an expanding ring always reaches
/// empty space outside the content bounding box. That is what makes leader
/// lines a complete strategy rather than one more thing that can fail.
pub fn place_with_leaders(
    routes: &[Vec<(f64, f64)>],
    requests: &[LabelRequest],
    obstacles: &Obstacles,
    cfg: &LabelConfig,
) -> Placement {
    let mut out = place(routes, requests, obstacles, cfg);
    let residue = std::mem::take(&mut out.unplaced);
    let mut hard: Vec<Rect> = obstacles.hard.iter().copied()
        .chain(out.placed.iter().map(|p| p.rect))
        .collect();

    for request in residue {
        let size = measure(&request.text, cfg);
        let Some(points) = routes.get(request.edge) else { continue };
        let Some(anchor) = ideal_anchor(points, request.slot) else { continue };

        for ring in 1..=MAX_LEADER_RINGS {
            let radius = ring as f64 * size.h * 2.0;
            let mut found = None;
            for step in 0..LEADER_STEPS {
                let angle = std::f64::consts::TAU * step as f64 / LEADER_STEPS as f64;
                let rect = Rect {
                    x: anchor.0 + radius * angle.cos() - size.w * 0.5,
                    y: anchor.1 + radius * angle.sin() - size.h * 0.5,
                    w: size.w,
                    h: size.h,
                };
                if !collides(rect, &hard) {
                    found = Some(rect);
                    break;
                }
            }
            if let Some(rect) = found {
                hard.push(rect);
                out.placed.push(PlacedLabel {
                    edge: request.edge,
                    slot: request.slot,
                    text: request.text.clone(),
                    rect,
                    attach: anchor,
                    leader: Some([anchor, nearest_edge_point(rect, anchor)]),
                });
                break;
            }
        }
    }
    out
}
```

Constants and helpers this needs:

```rust
/// Ring count for the leader search. Each ring steps out by two label heights,
/// so this reaches well past any realistic diagram's bounding box -- which is
/// what makes the search total.
const MAX_LEADER_RINGS: usize = 64;
/// Positions sampled per ring. Fixed count in fixed angular order, so the
/// search is deterministic.
const LEADER_STEPS: usize = 16;
```

- `ideal_anchor(points, slot) -> Option<(f64, f64)>` — the un-displaced attach point for the slot, i.e. the first candidate's `attach` from `candidates`.
- `nearest_edge_point(rect, from) -> (f64, f64)` — the point on `rect`'s border closest to `from`, so the leader meets the box rather than its centre.

Leader lines are **soft** obstacles for later labels, not hard — hard would cascade, each leader shrinking free space for the next, and they are hairlines where a crossing is untidy rather than unreadable. Note the loop above pushes only the label *rect* onto `hard`, never the leader segment; that is deliberate.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p waml leader`
Expected: PASS, 3 tests.

- [ ] **Step 5: Draw the leader**

In `crates/waml-editor/src/canvas/class/render/labels.rs`, when a placed label carries a leader, stroke the 2-point line from `attach` to the label rect using the existing edge stroke resources at hairline weight, before drawing the text.

- [ ] **Step 6: Visual sign-off (best-effort, NON-BLOCKING)**

Same rules as the prerequisite plan's Task 7 Step 7: attempt once, never block or retry, and record the outcome —
including failure to capture — in the commit body.

Build a fixture dense enough to actually trigger a leader — several relationships converging on one node with long role names — and screenshot the native editor by pid, per the procedure in the prerequisite plan's Task 7. Confirm the leader visibly connects the displaced label to its route and that the label is fully readable.

If no leader triggers on any realistic fixture, say so plainly in the commit message. That is a real and useful finding about whether this stage was needed.

- [ ] **Step 7: Run the full gate and commit**

Commit this unit. Suggested message:
```text
feat(solve): leader lines for labels with nowhere to sit

Expanding-ring search from the ideal anchor, first free position wins.
Total by construction: obstacles are finite, so the ring always reaches
empty space outside the content bbox -- which is why this needs no further
fallback. Leaders are soft obstacles for later labels, since making them
hard would cascade.
```

The harness appends the `Plan:` / `Plan-Tasks:` trailers and the attribution footer; do not write them by hand, and do not run `git commit` yourself if the harness commits for you.

---

### Task 5: Surface the instrumentation

Without these counts there is no way to tell whether Tasks 2-4 earn their complexity, and with a tunable cost function no way to see what a weight change did.

**Files:**
- Modify: `crates/waml/src/solve/mod.rs` (`pretty` dump)
- Test: `crates/waml/tests/` golden

**Interfaces:**
- Consumes: `Solved.label_reroutes`, `Solved.label_leaders`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn the_pretty_dump_reports_label_placement_effort() {
    let mut solved = crowded_scene_where_one_label_cannot_fit();
    place_labels(&mut solved, &requests_for(&solved), &label::LabelConfig::default());
    let dump = pretty(&solved);
    assert!(dump.contains("label-reroutes "), "dump: {dump}");
    assert!(dump.contains("label-leaders "), "dump: {dump}");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p waml the_pretty_dump_reports_label_placement_effort`
Expected: FAIL.

- [ ] **Step 3: Extend the dump**

In `pretty`, after the existing node/route lines, append one line each for placed labels (`label <edge> <slot> @ x,y wxh`, plus a `+leader` marker) and a trailing summary with the two counts. Keep the existing line formats byte-identical so current goldens do not move.

- [ ] **Step 4: Run the test and re-baseline**

Run: `cargo test -p waml 2>&1 | tail -30`

Goldens gain label lines. Confirm no existing line changed — only additions.

- [ ] **Step 5: Report the real numbers**

Run the dump across every fixture in the repo and record the actual reroute and leader counts.

```bash
cargo run -p waml --example stress_dump 2>&1 | grep -E "label-(reroutes|leaders)"
```

Put the totals in the commit message. If they are all zero on real diagrams, say so — that is evidence the spacing fix in the prerequisite plan did the real work and that Tasks 2-4 are insurance rather than load-bearing. That finding is worth more than a quiet green checkmark.

- [ ] **Step 6: Commit**

Commit this unit. Suggested message:
```text
feat(solve): report label reroute and leader counts

The objective function for whether route pressure and leader lines earn
their complexity. Measured across the repo's fixtures: <fill in actual
totals from Step 5>.
```

The harness appends the `Plan:` / `Plan-Tasks:` trailers and the attribution footer; do not write them by hand, and do not run `git commit` yourself if the harness commits for you.

---

## Notes for the implementer

- **Task 1 is the risk.** If existing route goldens move there, stop and find out why rather than re-baselining — that gate is the only thing standing between this plan and silently changing every diagram in the repo.
- **Work in a git worktree, never the main checkout.** Verify `git rev-parse --show-toplevel` before editing.
- **A `waml-syntax` proptest is intermittently red at HEAD** for reasons unrelated to this work (incremental vs full parse on trailing whitespace after an ATX heading). Confirm by stashing and re-running before blaming your diff.
