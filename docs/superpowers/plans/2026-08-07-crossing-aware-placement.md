# Crossing-aware placement in the stress default layout

## Context

Class diagrams on the stress path show edges crossing for no reason. The user
hit it on the WAML Domain Model view: two edges running roughly parallel invert
their order right before their endpoints, so they must cross.

There are two independent causes, and only one of them is the router's:

1. **Route-level** — endpoints in the wrong relative order on a node's border,
   or a silly corridor choice. Fixable with node positions untouched. Covered by
   `docs/superpowers/specs/2026-08-06-edge-crossing-reduction-design.md`, which
   confirmed `route_keyed_with` (`crates/waml/src/solve/route.rs:125`) routes
   edge-by-edge with no other route in scope, and that `RouteCost` has only
   `length`/`bend`/`label_pressure` terms.
2. **Placement-level** — the node positions themselves make the crossing
   unavoidable. No router can fix these; only moving nodes can.

This plan is about (2). Stress majorization optimizes distance fidelity and
knows nothing about crossings, so it will happily produce a layout whose best
possible routing still crosses many times.

There is also a feedback loop worth naming: group cohesion was just raised to
`group_weight: 30` (`d56da727`), which packs clusters tighter and forces more
edges through narrower corridors between them. Tighter clusters plausibly
*increase* crossings even as they improve legibility, which is why those
constants are marked PROVISIONAL in `stress.rs`. This work is what lets them be
judged honestly.

**Nobody has measured anything yet.** We do not know whether the domain model
has 4 removable crossings or 40, nor what fraction are placement-level. Task 1
exists to answer that before any optimization is written, and its baseline
numbers decide whether the rest of the plan is even worth landing.

The load-bearing design insight: **moves that permute nodes within one group, or
relocate a whole group, preserve group cohesion exactly.** Restricting the
optimizer to those moves means crossing reduction and cohesion never fight —
no weighing one against the other, no risk of undoing `d56da727`.

Outcome: a deterministic post-pass over the stress solve that lowers the
crossing count without disturbing cluster structure, plus the measurement to
prove it did.

## Approach

### Task 1 — crossing counters and a baseline report

`crates/waml/src/solve/crossing.rs` (new module, exported from `solve/mod.rs`).

Two counters, because the optimizer and the ground truth need different things:

```rust
/// Exact crossings over routed polylines. Ground truth; reporting and tests.
pub fn route_crossings(routes: &[Route]) -> usize;

/// Cheap proxy over straight node-center segments. The optimizer's objective.
pub fn segment_crossings(centers: &[(f64, f64)], edges: &[(usize, usize)]) -> usize;
```

Both must handle the cases that are NOT crossings, or every count is noise:

- Edges sharing an endpoint (a node's own fan-out) touch at that point — not a
  crossing.
- Collinear overlapping segments — that is edge bundling, a separate defect. Do
  not count it here; note it in the report so it is visible.
- Orthogonal routes touch at corners constantly; only count proper transversal
  intersections of segment interiors.

Then a reporting harness (an `examples/` binary or an ignored test — match
whatever `crates/waml/examples/stress_dump.rs` does) that prints, per fixture:
node count, edge count, `route_crossings`, and `segment_crossings`. Run it over
`crates/waml-editor/tests/fixtures/{mini,groups,groups-linked,sixkind}` and
`docs/waml/architecture/views/domain-model.md`.

**Record those baseline numbers in the commit message.** Also record them at
`group_weight` 4 vs 30 — that answers whether the cohesion raise made crossings
worse, which is currently an open question blocking the constants.

**Report the proxy's fidelity, but do NOT stop on it.** `segment_crossings` is
only a legitimate objective insofar as it tracks `route_crossings`, so compare
the two across every fixture and state the correlation plainly in the commit
message — including if it is poor. Proceed to the remaining tasks either way;
the decision to keep or discard the pass is the human's, and it is made from the
Task 3 numbers, not by aborting here. Routing every candidate placement is not
an option; it is an A* per edge per candidate.

### Task 2 — the improvement pass

`crates/waml/src/solve/stress.rs`, after `component_layout` and before the
existing `remove_overlaps` / `separate_hulls` calls in `layout_grouped`.

```rust
fn reduce_crossings(
    rects: &mut [Rect],
    groups: &[GroupSpec],
    edges: &[(usize, usize)],
    cfg: &StressConfig,
);
```

Hill-climb on `segment_crossings`. Candidate moves, all cohesion-preserving by
construction:

1. **Swap two members within one group** — exchange their center positions.
2. **Swap two whole groups** — translate each group's member set so their hull
   centers exchange.
3. **Reflect one group's members** horizontally or vertically about their hull
   center.

Ungrouped nodes participate in (1) as a virtual "no group" pool.

Accept a move iff `segment_crossings` strictly decreases. On equal counts, keep
the existing arrangement — never churn for a tie. Enumerate candidates in a
fixed index order, apply the first improving move, repeat until a full sweep
finds none or a pass cap (`cfg.crossing_passes`, default 8) is hit. No RNG, no
simulated annealing — determinism is a hard requirement of this solver
(`stress.rs:1-7`) and a golden-tested property.

Sizes differ between nodes, so a swap can reintroduce overlap; the existing
`remove_overlaps` and `separate_hulls` still run afterwards and clean up. Note
that they may perturb positions enough to reintroduce a crossing — measure the
final count, not the mid-pass one.

Add `crossing_passes: u32` to `StressConfig`, **defaulting to 8 — the pass ships
enabled**. Setting it to `0` disables the pass entirely, which is both the
escape hatch and the A/B mechanism.

### Task 3 — wire it up and re-measure

The pass sits inside `layout_grouped`, so `crates/waml-editor/src/scene.rs`
needs no change — but re-run the Task 1 report with `crossing_passes` at 0 vs 8
and record the delta in the commit message. That number is the entire
justification for this plan. If it turns out negligible, still land the work
(the pass is opt-out via `crossing_passes: 0`) and say so plainly in the commit
message rather than dressing up a null result.

### Task 4 — tests

- Unit tests for both counters, covering shared endpoints, collinear overlap,
  corner touches, and a plain transversal crossing.
- A hand-built layout with one obviously avoidable crossing (two groups, two
  cross edges inverted) where the pass removes it — assert the count drops.
- **Cohesion preservation**: after the pass, every group's members are still the
  same set inside its hull, and the existing softness golden
  (`grouped_layout_lets_a_strong_outside_edge_pull_a_member_out` in
  `crates/waml/tests/stress_golden.rs`) still passes.
- **Determinism**: identical input twice yields byte-identical rects.
- A regression guard that `crossing_passes: 0` reproduces today's exact output,
  so the pass is provably opt-in.

## Verification

```
cargo test --workspace
cd editors/vscode && npm install && npm run build && npm test && npm run lint
```

Note `npm install` is required in a fresh worktree — there is no
`package-lock.json`, so `npm ci` fails outright and `tsc` then reports
`Cannot find module 'node:path'`, which looks exactly like a real typecheck
break and is not.

**The visual pass below is DEFERRED to a human and is NOT part of any task's
completion criteria.** Every task here is complete when the gate is green and
the measured numbers are recorded; no task may block on a screenshot, a running
editor, or a rendered frame.

Then, for a human: run the editor on the domain model and confirm the crossings
actually look reduced, and re-judge `group_weight`/`group_len` now that
crossings move.

```
pwsh run.ps1 docs/waml -Title crossing-aware
```
