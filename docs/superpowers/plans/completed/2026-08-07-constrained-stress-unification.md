# Constrained-Stress Unification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One placement engine for every diagram: the stress solve becomes the single layout path, authored `## Layout` statements compile into hard separation/alignment constraints projected inside it (IPSep-CoLa pattern), and the edge-blind `geometry::solve_cluster` strip-packing path stops being what the editor ships.

**Architecture:** Three new pieces in `crates/waml/src/solve/`: a VPSC solver (`vpsc.rs`, minimal-displacement projection onto separation constraints — the port the whole research briefing hinges on), a constraint compiler (`constrain.rs`, `Scene` constraints → per-axis separation specs with group boundary variables), and a constrained entry point on the stress module (`layout_constrained`). The editor's `build_scene` then routes **all** diagrams through stress; `use_stress_default` and the authored-path branch disappear. `waml::solve::geometry` stays in the crate untouched for the wasm/CLI callers (follow-up migration), but the native editor no longer calls it for placement.

**Tech Stack:** Pure Rust, `std` only (BTreeMap/Vec — no HashMap iteration in any new code path). No new dependencies.

## Global Constraints

- **Determinism:** same input, same pixels. No RNG, no time, no HashMap iteration order. Ties broken by index (`usize` order) or `f64::total_cmp`. Every new public fn gets a determinism test (call twice, assert equal).
- **wasm-compatible:** no threads, no filesystem, no `SystemTime`.
- **Gate (every task, before every commit):** `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`, then `pnpm test && pnpm lint && pnpm build` in `editors/vscode`. Clippy `-D warnings` promotes `dead_code` to a hard error: every task below leaves its new code reachable (pub API + tests), never half-wired.
- **Commits:** conventional style matching the repo (`feat(solve): …`), subject + body only. **No Co-Authored-By / generated-with trailers.**
- **Never commit `proptest-regressions` files.**
- **Visual verification is explicitly DEFERRED.** Task 5 changes what the editor draws for every authored-layout diagram. The gate for this plan is golden/unit tests only; a human visual pass over the preset fixtures (start screen → presets, esp. the Connectors/CI-CD entity diagram that motivated this) is owed after landing and is NOT a task here.

## Context (read before Task 1)

Two placement pipelines exist today, selected in `crates/waml-editor/src/scene.rs`:

- `use_stress_default(diagram)` (scene.rs:270) returns `diagram.layout.is_empty()`. **One authored `place` statement flips the whole diagram** to `waml::solve::solve_diagram_routed` → `geometry::solve_with_rects_labeled` → `solve_cluster` (geometry.rs:156), which treats each `Place{a,b,dir}` as a **rigid exact offset** (union-find `Potentials` per axis) and packs unconstrained/rigid components **left-to-right in a strip** (geometry.rs:289-310) — edge-blind by design. This is the bug class the user screenshotted: same-group entities flung across the full canvas width.
- Diagrams with no `## Layout` go through `stress_default` (scene.rs:622) → `stress::layout_grouped` (stress.rs:113): SMACOF + soft group cohesion + `remove_overlaps` (stress.rs:1100, greedy **x-only** scanline push) + `separate_hulls` + `reduce_crossings` (stress.rs:472).

After this plan: `stress_default` (renamed in spirit, kept in place) handles every diagram; authored statements arrive as `vpsc::Sep` constraints via `constrain::compile`, are enforced by projection, and the `DroppedPlacement` conflict-report contract (consumed by the editor's conflict error list) is preserved for genuinely contradictory hints.

Key existing types (do not redefine — import):

```rust
// crates/waml/src/solve/mod.rs
pub enum BoxId { Node(String), Group(u32), Inline(u32) }
pub struct Box { pub id: BoxId, pub kind: BoxKind, pub children: Vec<BoxId>,
    pub axis: Option<Axis>, pub shape: Shape, pub margin: Margin,
    pub flags: FlagSet, pub title: Option<String>, pub depth: u8 }
pub enum Constraint {
    Place { a: BoxId, b: BoxId, dir: Direction },
    Align { a: BoxId, a_edge: Edge, b: BoxId, b_edge: Edge },
}
pub struct Scene { pub boxes: Vec<Box>, pub constraints: Vec<Constraint> }

// crates/waml/src/layout.rs
pub enum Direction { LeftOf, RightOf, Above, Below, AboveLeft, AboveRight, BelowLeft, BelowRight }
pub enum Edge { Top, Bottom, Left, Right, Center }
pub enum Axis { Row, Column }

// crates/waml/src/solve/geometry.rs
pub struct DroppedPlacement { pub relation: Constraint, pub conflicts_with: Vec<Constraint> }

// crates/waml/src/solve/stress.rs
pub struct StressConfig { pub edge_len: f64, pub max_iter: u32, pub epsilon: f64,
    pub gap: f64, pub group_len: f64, pub group_weight: f64, pub hull_pad: f64 }
pub struct GroupSpec { pub members: Vec<usize>, pub depth: u8 }
pub fn layout_grouped(ids: &[BoxId], sizes: &[Size], edges: &[(usize, usize)],
    groups: &[GroupSpec], cfg: &StressConfig) -> (Vec<Rect>, Vec<Rect>)
```

## File Structure

- Create: `crates/waml/src/solve/vpsc.rs` — separation solver. Zero knowledge of diagrams; operates on `&mut [f64]`.
- Create: `crates/waml/src/solve/constrain.rs` — `Scene` → `SepSpecs` compiler + provenance for conflict reporting.
- Modify: `crates/waml/src/solve/stress.rs` — `SepSpecs` type, `layout_constrained` entry, overlap removal swap.
- Modify: `crates/waml/src/solve/mod.rs` — `pub mod vpsc; pub mod constrain;`.
- Modify: `crates/waml/src/solve/geometry.rs` — extract `pair_gap` helper (shared gap policy), make `off_x`/`off_y`/`edge_axes` `pub(super)`.
- Modify: `crates/waml-editor/src/scene.rs` — delete the dispatch, feed compiled seps into the stress path, carry flags.
- Tests: module tests in each new file + `crates/waml/tests/stress_golden.rs` additions; regenerate `crates/waml/tests/solver_golden.rs` expectation.

---

### Task 1: VPSC solver (`vpsc.rs`)

**Files:**
- Create: `crates/waml/src/solve/vpsc.rs`
- Modify: `crates/waml/src/solve/mod.rs` (add `pub mod vpsc;` after line 16 `pub mod sizing;`)

**Interfaces:**
- Consumes: nothing project-specific.
- Produces (later tasks rely on these exact signatures):

```rust
/// pos[left] + gap <= pos[right]  (== when `equality`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sep { pub left: usize, pub right: usize, pub gap: f64, pub equality: bool }

/// Move `pos` the least (weighted squared displacement) so every surviving
/// sep holds. Returns indices into `seps` that were DROPPED as unsatisfiable
/// (constraint cycles with positive total gap / contradictory equalities),
/// in ascending order. Deterministic.
pub fn project(pos: &mut [f64], weight: &[f64], seps: &[Sep]) -> Vec<usize>
```

- [ ] **Step 1: Write the failing tests**

Create `crates/waml/src/solve/vpsc.rs` with the module doc and the test module first (implementation stubbed with `todo!()` is NOT allowed to be committed — this step and Step 2 happen in your working tree, the commit lands only at Step 5 when green):

```rust
//! Variable Placement with Separation Constraints (VPSC).
//!
//! Minimal-displacement projection of scalar positions onto separation
//! constraints `pos[left] + gap <= pos[right]` — the primitive behind
//! IPSep-CoLa constrained stress layout (Dwyer, Koren, Marriott, TVCG 2006)
//! and minimal-displacement overlap removal (Dwyer, Marriott, Stuckey,
//! GD 2005). Ported from the WebCola `vpsc.ts` block merge/split solver.
//! Deterministic: ties break on constraint index; no RNG, no maps.

#[cfg(test)]
mod tests {
    use super::*;

    fn sep(left: usize, right: usize, gap: f64) -> Sep {
        Sep { left, right, gap, equality: false }
    }

    #[test]
    fn satisfied_input_is_untouched() {
        let mut pos = vec![0.0, 10.0, 25.0];
        let dropped = project(&mut pos, &[1.0, 1.0, 1.0], &[sep(0, 1, 5.0), sep(1, 2, 5.0)]);
        assert!(dropped.is_empty());
        assert_eq!(pos, vec![0.0, 10.0, 25.0]);
    }

    #[test]
    fn two_var_violation_splits_the_difference() {
        // minimize (a-4)^2 + (b-0)^2  s.t.  a + 2 <= b  =>  active: b = a + 2,
        // Lagrange gives a = 1, b = 3.
        let mut pos = vec![4.0, 0.0];
        let dropped = project(&mut pos, &[1.0, 1.0], &[sep(0, 1, 2.0)]);
        assert!(dropped.is_empty());
        assert!((pos[0] - 1.0).abs() < 1e-9, "a = {}", pos[0]);
        assert!((pos[1] - 3.0).abs() < 1e-9, "b = {}", pos[1]);
    }

    #[test]
    fn weights_shift_the_merge_point() {
        // Same constraint, weight(a) = 3, weight(b) = 1: minimize
        // 3(a-4)^2 + (b-0)^2 s.t. b = a + 2  =>  a = 2.5, b = 4.5.
        let mut pos = vec![4.0, 0.0];
        let dropped = project(&mut pos, &[3.0, 1.0], &[sep(0, 1, 2.0)]);
        assert!(dropped.is_empty());
        assert!((pos[0] - 2.5).abs() < 1e-9);
        assert!((pos[1] - 4.5).abs() < 1e-9);
    }

    #[test]
    fn chain_merges_transitively() {
        // a<=b<=c all violated, equal weights: block of three at the mean of
        // (desired - offset): offsets 0,5,10 against desireds 10,5,0
        // => centre vars a = ((10-0)+(5-5)+(0-10))/3 = 0 => a=0, b=5, c=10.
        let mut pos = vec![10.0, 5.0, 0.0];
        let dropped = project(&mut pos, &[1.0; 3], &[sep(0, 1, 5.0), sep(1, 2, 5.0)]);
        assert!(dropped.is_empty());
        assert!((pos[0] - 0.0).abs() < 1e-9);
        assert!((pos[1] - 5.0).abs() < 1e-9);
        assert!((pos[2] - 10.0).abs() < 1e-9);
    }

    #[test]
    fn split_pass_reaches_the_true_optimum() {
        // Regression for merge-only greediness. Desireds 0, 9, 3, unit
        // weights, seps a+3<=b (satisfied), b+3<=c (violated).
        // Greedy merge of {b,c} gives b=4.5, c=7.5 and a stays 0 — which is
        // optimal here; now tighten with a+3<=b: still satisfied (0+3<=4.5).
        // True optimum therefore keeps a free: check exactly that the solver
        // does NOT drag `a` into the block.
        let mut pos = vec![0.0, 9.0, 3.0];
        let dropped = project(&mut pos, &[1.0; 3], &[sep(0, 1, 3.0), sep(1, 2, 3.0)]);
        assert!(dropped.is_empty());
        assert!((pos[0] - 0.0).abs() < 1e-9, "a must not move: {}", pos[0]);
        assert!((pos[1] - 4.5).abs() < 1e-9);
        assert!((pos[2] - 7.5).abs() < 1e-9);
    }

    #[test]
    fn equality_sep_pins_the_offset_both_ways() {
        // b - a == 10 exactly, desireds 0 and 0: minimize a^2 + (b)^2
        // s.t. b = a + 10 => a = -5, b = 5.
        let mut pos = vec![0.0, 0.0];
        let dropped = project(
            &mut pos,
            &[1.0, 1.0],
            &[Sep { left: 0, right: 1, gap: 10.0, equality: true }],
        );
        assert!(dropped.is_empty());
        assert!((pos[0] + 5.0).abs() < 1e-9);
        assert!((pos[1] - 5.0).abs() < 1e-9);
    }

    #[test]
    fn positive_cycle_drops_the_highest_index_sep_and_reports_it() {
        // a+10<=b, b+10<=a is unsatisfiable; the LATER sep (index 1) loses,
        // matching the authored-statement-order policy of solve_cluster.
        let mut pos = vec![0.0, 0.0];
        let dropped = project(&mut pos, &[1.0, 1.0], &[sep(0, 1, 10.0), sep(1, 0, 10.0)]);
        assert_eq!(dropped, vec![1]);
        // The surviving sep still holds.
        assert!(pos[0] + 10.0 <= pos[1] + 1e-9);
    }

    #[test]
    fn zero_gap_cycle_is_satisfiable_and_collapses_to_equality() {
        // a<=b, b<=a with gap 0 is just a == b; nothing must be dropped.
        let mut pos = vec![4.0, 0.0];
        let dropped = project(&mut pos, &[1.0, 1.0], &[sep(0, 1, 0.0), sep(1, 0, 0.0)]);
        assert!(dropped.is_empty());
        assert!((pos[0] - pos[1]).abs() < 1e-9);
        assert!((pos[0] - 2.0).abs() < 1e-9, "meets at the mean");
    }

    #[test]
    fn projection_is_deterministic() {
        let seps: Vec<Sep> = (0..8).map(|i| sep(i, i + 1, 12.5)).collect();
        let desired: Vec<f64> = (0..9).rev().map(|i| i as f64 * 3.0).collect();
        let mut a = desired.clone();
        let mut b = desired.clone();
        let da = project(&mut a, &[1.0; 9], &seps);
        let db = project(&mut b, &[1.0; 9], &seps);
        assert_eq!(a, b);
        assert_eq!(da, db);
    }

    #[test]
    fn projection_is_idempotent() {
        let seps = [sep(0, 1, 5.0), sep(1, 2, 5.0), sep(0, 2, 20.0)];
        let mut pos = vec![9.0, 1.0, 4.0];
        project(&mut pos, &[1.0; 3], &seps);
        let once = pos.clone();
        project(&mut pos, &[1.0; 3], &seps);
        assert_eq!(pos, once, "already-feasible positions must not move");
    }

    #[test]
    fn disjoint_pairs_solve_independently() {
        let mut pos = vec![4.0, 0.0, 100.0, 90.0];
        project(&mut pos, &[1.0; 4], &[sep(0, 1, 2.0), sep(2, 3, 2.0)]);
        assert!((pos[0] - 1.0).abs() < 1e-9 && (pos[1] - 3.0).abs() < 1e-9);
        assert!((pos[2] - 94.0).abs() < 1e-9 && (pos[3] - 96.0).abs() < 1e-9);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail to compile**

Run: `cargo test -p waml vpsc`
Expected: compile error — `Sep`/`project` not defined.

- [ ] **Step 3: Implement the solver**

Add above the test module. This is the block merge/split algorithm; keep it exactly this shape (deterministic scans, index tie-breaks):

```rust
/// pos[left] + gap <= pos[right]  (== when `equality`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sep {
    pub left: usize,
    pub right: usize,
    pub gap: f64,
    pub equality: bool,
}

const EPS: f64 = 1e-9;

/// One merged block: member variables move rigidly, each at a fixed offset
/// from the block's reference position.
struct Block {
    /// Optimal reference position: weighted mean of (desired - offset).
    posn: f64,
    /// (var, offset) pairs; a var appears in exactly one live block.
    members: Vec<(usize, f64)>,
    /// Indices into `seps` of the constraints made ACTIVE by merges inside
    /// this block, in activation order — the split pass scans these.
    active: Vec<usize>,
}

/// Move `pos` the least (weighted squared displacement) so every surviving
/// sep holds. Returns indices into `seps` that were DROPPED as unsatisfiable,
/// ascending. Deterministic.
pub fn project(pos: &mut [f64], weight: &[f64], seps: &[Sep]) -> Vec<usize> {
    assert_eq!(pos.len(), weight.len());
    let desired: Vec<f64> = pos.to_vec();
    let mut dropped: Vec<usize> = Vec::new();
    let mut live: Vec<bool> = vec![true; seps.len()];
    // A split that leaves some OTHER sep violated restarts the solve; the
    // restart replays the same deterministic merges, so without a budget a
    // pathological active-set cycle could loop forever. When the budget runs
    // out we keep the merge-only answer — always feasible, possibly
    // suboptimal — instead of spinning.
    let mut split_budget: usize = seps.len() + 1;

    // Outer loop: solve; if an in-block violation proves a positive cycle,
    // drop the highest-index sep on it and re-solve from scratch. Each pass
    // drops at least one sep, so this terminates in <= seps.len() passes.
    'restart: loop {
        // block_of[v] = index into blocks; blocks are never removed, only
        // absorbed (absorbed blocks get members.clear()).
        let n = pos.len();
        let mut block_of: Vec<usize> = (0..n).collect();
        let mut blocks: Vec<Block> = (0..n)
            .map(|v| Block {
                posn: desired[v],
                members: vec![(v, 0.0)],
                active: Vec::new(),
            })
            .collect();
        let var_pos = |blocks: &[Block], block_of: &[usize], v: usize| -> f64 {
            let b = &blocks[block_of[v]];
            let off = b.members.iter().find(|(m, _)| *m == v).unwrap().1;
            b.posn + off
        };

        // Equality seps are permanently active: merge them first, in index
        // order. A contradiction inside one block (same block, wrong offset)
        // is unsatisfiable -> drop that sep and restart.
        // Then satisfy inequalities by most-violated-first merging.
        let mut merge_queue: Vec<usize> = (0..seps.len())
            .filter(|&i| live[i] && seps[i].equality)
            .collect();
        loop {
            let ci = match merge_queue.pop() {
                Some(ci) => ci,
                None => {
                    // Most violated live inequality; ties -> lowest index.
                    let mut worst: Option<(f64, usize)> = None;
                    for (i, s) in seps.iter().enumerate() {
                        if !live[i] || s.equality {
                            continue;
                        }
                        let v = var_pos(&blocks, &block_of, s.left) + s.gap
                            - var_pos(&blocks, &block_of, s.right);
                        if v > EPS && worst.map_or(true, |(wv, _)| v > wv + EPS) {
                            worst = Some((v, i));
                        }
                    }
                    match worst {
                        Some((_, i)) => i,
                        None => break, // all satisfied
                    }
                }
            };
            let s = seps[ci];
            let bl = block_of[s.left];
            let br = block_of[s.right];
            if bl == br {
                let v = var_pos(&blocks, &block_of, s.left) + s.gap
                    - var_pos(&blocks, &block_of, s.right);
                if v > EPS {
                    // In-block violation = positive cycle through active seps.
                    // Drop the highest-index live sep on the cycle: the cycle
                    // consists of `ci` plus actives in this block; authored
                    // order policy says the latest statement loses.
                    let worst_active = blocks[bl]
                        .active
                        .iter()
                        .copied()
                        .filter(|&a| live[a] && !seps[a].equality)
                        .max();
                    let victim = match worst_active {
                        Some(a) if a > ci => a,
                        _ => ci,
                    };
                    live[victim] = false;
                    dropped.push(victim);
                    continue 'restart;
                }
                continue; // redundant, already satisfied inside the block
            }
            // Merge right block into left block so that
            // pos(left) + gap == pos(right) exactly.
            let off_l = blocks[bl].members.iter().find(|(m, _)| *m == s.left).unwrap().1;
            let off_r = blocks[br].members.iter().find(|(m, _)| *m == s.right).unwrap().1;
            // Every member of br gets offset relative to bl's reference:
            // shift = off_l + gap - off_r.
            let shift = off_l + s.gap - off_r;
            let br_members = std::mem::take(&mut blocks[br].members);
            let br_active = std::mem::take(&mut blocks[br].active);
            for (m, o) in br_members {
                block_of[m] = bl;
                blocks[bl].members.push((m, o + shift));
            }
            blocks[bl].active.extend(br_active);
            blocks[bl].active.push(ci);
            // Recompute optimal reference position: weighted mean of
            // desired[m] - offset[m].
            let b = &mut blocks[bl];
            let mut num = 0.0;
            let mut den = 0.0;
            for &(m, o) in &b.members {
                num += weight[m] * (desired[m] - o);
                den += weight[m];
            }
            b.posn = num / den;
        }

        // Split pass: an active inequality with a negative Lagrange multiplier
        // is pinning its block below the optimum; deactivate it, split the
        // block, and re-run. Multiplier sign test via the standard "would the
        // two halves drift apart?" check: split members reachable from
        // `right` through OTHER active seps; if the right half's optimal posn
        // >= left half's + gap, splitting strictly improves. Bounded: each
        // split deactivates one active sep and we never re-activate inside
        // this pass; restart the satisfy loop after any split.
        let mut split_any = false;
        if split_budget == 0 {
            // Budget exhausted: accept the merge-only (feasible) answer.
            for b in &blocks {
                for &(m, o) in &b.members {
                    pos[m] = b.posn + o;
                }
            }
            dropped.sort_unstable();
            return dropped;
        }
        for bi in 0..blocks.len() {
            if blocks[bi].members.len() < 2 {
                continue;
            }
            // Scan actives newest-first so the most recent merge is
            // reconsidered first (WebCola order).
            let actives: Vec<usize> = blocks[bi].active.clone();
            for &ci in actives.iter().rev() {
                if !live[ci] || seps[ci].equality {
                    continue;
                }
                let s = seps[ci];
                // Partition members by reachability from s.right over active
                // seps excluding ci.
                let mut right_side: Vec<usize> = vec![s.right];
                let mut grew = true;
                while grew {
                    grew = false;
                    for &ai in &actives {
                        if ai == ci || !live[ai] {
                            continue;
                        }
                        let a = seps[ai];
                        let l_in = right_side.contains(&a.left);
                        let r_in = right_side.contains(&a.right);
                        if l_in != r_in {
                            right_side.push(if l_in { a.right } else { a.left });
                            grew = true;
                        }
                    }
                }
                if right_side.contains(&s.left) {
                    continue; // still connected without ci: not a cut edge
                }
                let (mut rnum, mut rden, mut lnum, mut lden) = (0.0, 0.0, 0.0, 0.0);
                let mut r_off = 0.0;
                let mut l_off = 0.0;
                for &(m, o) in &blocks[bi].members {
                    if right_side.contains(&m) {
                        rnum += weight[m] * (desired[m] - o);
                        rden += weight[m];
                        if m == s.right {
                            r_off = o;
                        }
                    } else {
                        lnum += weight[m] * (desired[m] - o);
                        lden += weight[m];
                        if m == s.left {
                            l_off = o;
                        }
                    }
                }
                let (rposn, lposn) = (rnum / rden, lnum / lden);
                // In split coordinates the constraint reads
                // (lposn + l_off) + gap <= (rposn + r_off).
                if lposn + l_off + s.gap <= rposn + r_off + EPS {
                    // Splitting is feasible AND weakly improving: do it by
                    // deactivating ci and restarting the whole solve pass
                    // (simple and deterministic; n is small).
                    let b = &mut blocks[bi];
                    b.active.retain(|&a| a != ci);
                    // Rebuild two blocks in place: keep left half here, put
                    // right half in a fresh block at its own optimum.
                    let members = std::mem::take(&mut b.members);
                    let actives_left: Vec<usize> = b
                        .active
                        .iter()
                        .copied()
                        .filter(|&ai| {
                            let a = seps[ai];
                            !right_side.contains(&a.left) && !right_side.contains(&a.right)
                        })
                        .collect();
                    let actives_right: Vec<usize> = b
                        .active
                        .iter()
                        .copied()
                        .filter(|&ai| {
                            let a = seps[ai];
                            right_side.contains(&a.left) && right_side.contains(&a.right)
                        })
                        .collect();
                    let mut left_members = Vec::new();
                    let mut right_members = Vec::new();
                    for (m, o) in members {
                        if right_side.contains(&m) {
                            right_members.push((m, o - r_off));
                        } else {
                            left_members.push((m, o - l_off));
                        }
                    }
                    // Renormalize offsets so each half's reference var (the
                    // sep endpoint) has offset 0; positions recomputed below.
                    let bl_new = blocks.len();
                    for &(m, _) in &right_members {
                        block_of[m] = bl_new;
                    }
                    let mk = |members: Vec<(usize, f64)>, active: Vec<usize>| -> Block {
                        let mut num = 0.0;
                        let mut den = 0.0;
                        for &(m, o) in &members {
                            num += weight[m] * (desired[m] - o);
                            den += weight[m];
                        }
                        Block { posn: num / den, members, active }
                    };
                    blocks[bi] = mk(left_members, actives_left);
                    blocks.push(mk(right_members, actives_right));
                    split_budget -= 1;
                    split_any = true;
                    break;
                }
            }
            if split_any {
                break;
            }
        }
        if split_any {
            // Some inequality may now be violated again; re-satisfy. Cheap at
            // our n: loop the whole pass. Terminates because each split
            // strictly lowers the objective and the active-set lattice is
            // finite; belt-and-braces bound below.
            // (Re-enter the satisfy loop by restarting the pass WITHOUT
            // clearing `dropped`/`live`.)
            let mut out_pos = vec![0.0; n];
            for b in &blocks {
                for &(m, o) in &b.members {
                    out_pos[m] = b.posn + o;
                }
            }
            pos.copy_from_slice(&out_pos);
            // Feed current positions back as the next pass's start; desired
            // stays the ORIGINAL desired (minimal displacement is measured
            // against the caller's input).
            if satisfied(pos, seps, &live) {
                dropped.sort_unstable();
                return dropped;
            }
            continue 'restart;
        }

        for b in &blocks {
            for &(m, o) in &b.members {
                pos[m] = b.posn + o;
            }
        }
        dropped.sort_unstable();
        return dropped;
    }
}

fn satisfied(pos: &[f64], seps: &[Sep], live: &[bool]) -> bool {
    seps.iter().enumerate().all(|(i, s)| {
        !live[i] || {
            let d = pos[s.right] - pos[s.left] - s.gap;
            if s.equality { d.abs() <= EPS } else { d >= -EPS }
        }
    })
}
```

Implementation notes for the engineer:
- The `'restart` structure trades speed for simplicity — fine at diagram scale (n ≤ a few hundred, seps ≤ a few hundred). Do NOT "optimize" it into incremental block bookkeeping in this task.
- If the split-pass bookkeeping fights you, a correct fallback that still passes every test above: after the merge loop, recompute from scratch with the active set minus the one deactivated sep (full re-solve). Keep the tests as the contract; simplify internals freely.
- Watch `members.contains` / `right_side.contains` — Vec scans are fine here; do not introduce HashSet (iteration-order nondeterminism risk in future edits).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p waml vpsc`
Expected: all 11 tests PASS.

- [ ] **Step 5: Gate and commit**

Run the full gate (see Global Constraints). Then:

```bash
git add crates/waml/src/solve/vpsc.rs crates/waml/src/solve/mod.rs
git commit -m "feat(solve): VPSC minimal-displacement separation solver

Block merge/split projection of scalar positions onto separation and
equality constraints (Dwyer-Marriott-Stuckey GD 2005; WebCola vpsc.ts).
Foundation for constrained stress layout: overlap removal, authored
layout hints, and top-down inheritance all express as Sep lists.
Unsatisfiable cycles drop the latest-index constraint and report it,
matching solve_cluster's authored-order conflict policy."
```

---

### Task 2: Minimal-displacement overlap removal in the stress path

**Files:**
- Modify: `crates/waml/src/solve/stress.rs` — replace the body of `remove_overlaps` (stress.rs:1100) and its call sites' expectations; add tests.

**Interfaces:**
- Consumes: `vpsc::{Sep, project}` from Task 1.
- Produces: `fn remove_overlaps(rects: &mut [Rect], gap: f64)` — same signature as today (callers unchanged), new semantics: two-axis minimal-displacement instead of x-only greedy push.

Why: the greedy x-only push (sort by center-x, shove right) is what smears layouts into wide horizontal strips and destroys whatever vertical structure the stress solve produced. The replacement resolves each overlapping pair on the axis needing the smaller displacement, then solves both axes exactly.

- [ ] **Step 1: Write the failing tests**

Add to `stress.rs` `mod tests`:

```rust
#[test]
fn overlap_removal_prefers_the_cheap_axis() {
    // Two boxes overlapping 10px in x but 60px in y: the old x-push moved
    // one box 10+gap px right — correct. But two boxes overlapping 60px in
    // x and 10px in y must separate VERTICALLY (10+gap), not horizontally
    // (60+gap). Total displacement must be the smaller option.
    let gap = 8.0;
    let mut rects = vec![
        Rect { x: 0.0, y: 0.0, w: 100.0, h: 50.0 },
        Rect { x: 40.0, y: 40.0, w: 100.0, h: 50.0 }, // 60px x-overlap, 10px y-overlap
    ];
    remove_overlaps(&mut rects, gap);
    assert!(!overlaps(&rects[0], &rects[1]));
    // Vertical resolution: x positions unchanged.
    assert_eq!(rects[0].x, 0.0);
    assert_eq!(rects[1].x, 40.0);
    let y_gap = rects[1].y - (rects[0].y + rects[0].h);
    assert!(y_gap >= gap - 1e-9, "y gap {y_gap}");
}

#[test]
fn overlap_removal_distributes_displacement_across_both_boxes() {
    // The old scanline moved only the RIGHT box; minimal displacement moves
    // both toward each other's clear side (weighted equally).
    let mut rects = vec![
        Rect { x: 0.0, y: 0.0, w: 100.0, h: 50.0 },
        Rect { x: 90.0, y: 10.0, w: 100.0, h: 50.0 },
    ];
    let before = rects.clone();
    remove_overlaps(&mut rects, 8.0);
    assert!(!overlaps(&rects[0], &rects[1]));
    assert!(rects[0].x < before[0].x, "left box shares the displacement");
    assert!(rects[1].x > before[1].x);
}

#[test]
fn overlap_removal_two_axis_is_deterministic_and_idempotent() {
    let mk = || {
        vec![
            Rect { x: 0.0, y: 0.0, w: 80.0, h: 40.0 },
            Rect { x: 30.0, y: 10.0, w: 80.0, h: 40.0 },
            Rect { x: 60.0, y: 20.0, w: 80.0, h: 40.0 },
            Rect { x: 10.0, y: 35.0, w: 80.0, h: 40.0 },
        ]
    };
    let mut a = mk();
    let mut b = mk();
    remove_overlaps(&mut a, 8.0);
    remove_overlaps(&mut b, 8.0);
    assert_eq!(a, b);
    let once = a.clone();
    remove_overlaps(&mut a, 8.0);
    assert_eq!(a, once, "already-separated input must not move");
}
```

(`overlaps` already exists in the test module at stress.rs:1206.)

- [ ] **Step 2: Run to verify the new tests fail**

Run: `cargo test -p waml overlap_removal`
Expected: `overlap_removal_prefers_the_cheap_axis` and `..._distributes_displacement...` FAIL against the x-only push (`overlap_removal_leaves_no_overlaps` still passes).

- [ ] **Step 3: Replace the implementation**

Replace `fn remove_overlaps` (stress.rs:1100-1127) with:

```rust
/// Minimal-displacement overlap removal (Dwyer-Marriott-Stuckey GD 2005,
/// simplified): each overlapping pair contributes one separation constraint
/// on the axis that needs the smaller move; both axes then solve exactly via
/// `vpsc::project`, so displacement is spread over both boxes instead of
/// shoving everything rightward. Loops until clean because resolving one
/// pair can create a new overlap; bounded, each pass strictly reduces
/// total overlap area.
fn remove_overlaps(rects: &mut [Rect], gap: f64) {
    use super::vpsc::{project, Sep};
    let m = rects.len();
    // Bounded: in practice 2-3 passes; the cap only guards pathological input.
    for _ in 0..m.max(4) {
        let mut xsep: Vec<Sep> = Vec::new();
        let mut ysep: Vec<Sep> = Vec::new();
        for i in 0..m {
            for j in (i + 1)..m {
                let (a, b) = (&rects[i], &rects[j]);
                let ox = (a.x + a.w + gap).min(b.x + b.w + gap) - a.x.max(b.x);
                let oy = (a.y + a.h + gap).min(b.y + b.h + gap) - a.y.max(b.y);
                if ox <= 0.0 || oy <= 0.0 {
                    continue; // clear (with gap) on at least one axis
                }
                // Resolve on the axis with the smaller required move.
                if ox <= oy {
                    let (l, r) = if a.x + a.w / 2.0 <= b.x + b.w / 2.0 { (i, j) } else { (j, i) };
                    xsep.push(Sep { left: l, right: r, gap: rects[l].w + gap, equality: false });
                } else {
                    let (t, u) = if a.y + a.h / 2.0 <= b.y + b.h / 2.0 { (i, j) } else { (j, i) };
                    ysep.push(Sep { left: t, right: u, gap: rects[t].h + gap, equality: false });
                }
            }
        }
        if xsep.is_empty() && ysep.is_empty() {
            return;
        }
        let w = vec![1.0; m];
        let mut xs: Vec<f64> = rects.iter().map(|r| r.x).collect();
        project(&mut xs, &w, &xsep);
        for (r, x) in rects.iter_mut().zip(&xs) {
            r.x = *x;
        }
        let mut ys: Vec<f64> = rects.iter().map(|r| r.y).collect();
        project(&mut ys, &w, &ysep);
        for (r, y) in rects.iter_mut().zip(&ys) {
            r.y = *y;
        }
    }
}
```

- [ ] **Step 4: Run the crate tests; inspect stress golden churn**

Run: `cargo test -p waml`
Expected: the three new tests PASS. Existing stress tests assert **properties** (no overlap, determinism, cohesion, hull nesting) not exact pixels — they should pass unchanged. `crates/waml/tests/stress_golden.rs` and `crates/waml-editor` tests may pin coordinates; if any fail, read the diff: coordinates may shift but every property assertion (disjoint hulls, no overlaps, determinism) must hold. Update pinned coordinates ONLY after confirming the property assertions pass — mention each regenerated expectation in the commit body.

- [ ] **Step 5: Gate and commit**

```bash
git add crates/waml/src/solve/stress.rs
git commit -m "feat(solve): minimal-displacement two-axis overlap removal

Replace the x-only greedy scanline push with per-pair cheap-axis
separation constraints solved exactly by vpsc::project on both axes.
Displacement is shared between boxes and the vertical structure the
stress solve produced survives, instead of smearing into a strip."
```

(Include a `Regenerated:` list in the body if any golden expectations changed.)

---

### Task 3: `SepSpecs` + `layout_constrained` on the stress module

**Files:**
- Modify: `crates/waml/src/solve/stress.rs`

**Interfaces:**
- Consumes: `vpsc::{Sep, project}`.
- Produces (Task 4/5 rely on these exact signatures):

```rust
/// Per-axis separation constraints over layout indices. Indices 0..n-1 are
/// the nodes (`ids` order); indices n.. address the boundary variables
/// appended by `boundary_vars` (constrain.rs), in that same order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SepSpecs {
    pub x: Vec<crate::solve::vpsc::Sep>,
    pub y: Vec<crate::solve::vpsc::Sep>,
    /// Extra positionless variables (group boundaries): (axis extent proxy)
    /// count appended after the node variables on BOTH axes.
    pub extra_vars: usize,
}

/// `layout_grouped` + hard constraints: stress-solve, pack components, then
/// project `seps` and re-run overlap removal with the authored seps folded
/// in so it cannot un-satisfy them. Returns (node rects, hull rects,
/// (dropped x-sep indices, dropped y-sep indices)).
pub fn layout_constrained(
    ids: &[BoxId],
    sizes: &[Size],
    edges: &[(usize, usize)],
    groups: &[GroupSpec],
    seps: &SepSpecs,
    cfg: &StressConfig,
) -> (Vec<Rect>, Vec<Rect>, (Vec<usize>, Vec<usize>))
```

Design decisions (locked):
- Projection is **post-hoc**: after per-component SMACOF + component packing, once, globally — not per-majorization-iteration. Sparse authored hints don't warrant per-iteration projection; cross-component seps just work because projection runs on the packed global coordinates. (Per-iteration IPSep-CoLa refinement is an explicit follow-up, out of scope.)
- Coordinates projected are rect **min-corners** (`r.x`, `r.y`), matching Task 2 and the `Sep.gap` conventions in Task 4.
- `reduce_crossings` is **skipped when any authored sep exists** (`!seps.x.is_empty() || !seps.y.is_empty()`): its group-relocation moves know nothing about seps and would un-satisfy them; a diagram with authored hints has a human's opinion in it already. Groupless/hint-free diagrams keep today's behavior exactly.
- Order inside `layout_constrained`: stress+pack (existing `layout_grouped` internals) → project x, project y (node vars + `extra_vars` boundary vars, boundary vars initialized from their group's current member bounds, weight `0.01` so they follow members instead of dragging them) → `remove_overlaps_with` (overlap constraints **plus** authored seps, so separation can't undo them) → hulls → (conditionally) `reduce_crossings`.

- [ ] **Step 1: Write the failing tests**

Add to `stress.rs` tests:

```rust
#[test]
fn layout_constrained_empty_seps_matches_layout_grouped() {
    let ids = ids(&["a", "b", "c"]);
    let sz = sizes(3, 100.0, 50.0);
    let edges = vec![(0usize, 1usize), (1, 2)];
    let cfg = StressConfig::default();
    let (r1, h1) = layout_grouped(&ids, &sz, &edges, &[], &cfg);
    let (r2, h2, dropped) = layout_constrained(&ids, &sz, &edges, &[], &SepSpecs::default(), &cfg);
    assert_eq!(r1, r2);
    assert_eq!(h1, h2);
    assert!(dropped.0.is_empty() && dropped.1.is_empty());
}

#[test]
fn layout_constrained_enforces_a_y_separation() {
    // "a above b" as a y-sep: y_a + h_a + 40 <= y_b — regardless of where
    // stress wanted them.
    use crate::solve::vpsc::Sep;
    let ids = ids(&["a", "b"]);
    let sz = sizes(2, 100.0, 50.0);
    let seps = SepSpecs {
        y: vec![Sep { left: 0, right: 1, gap: 50.0 + 40.0, equality: false }],
        ..SepSpecs::default()
    };
    let (rects, _, dropped) =
        layout_constrained(&ids, &sz, &[(0, 1)], &[], &seps, &StressConfig::default());
    assert!(dropped.0.is_empty() && dropped.1.is_empty());
    assert!(
        rects[0].y + rects[0].h + 40.0 <= rects[1].y + 1e-6,
        "a must sit above b: {:?}", rects
    );
}

#[test]
fn layout_constrained_enforces_an_alignment_equality() {
    use crate::solve::vpsc::Sep;
    // Align left edges: x_a == x_b (equality sep, gap 0).
    let ids = ids(&["a", "b"]);
    let sz = sizes(2, 100.0, 50.0);
    let seps = SepSpecs {
        x: vec![Sep { left: 0, right: 1, gap: 0.0, equality: true }],
        ..SepSpecs::default()
    };
    let (rects, _, _) =
        layout_constrained(&ids, &sz, &[(0, 1)], &[], &seps, &StressConfig::default());
    assert!((rects[0].x - rects[1].x).abs() < 1e-6);
    // Overlap removal must have separated them on Y instead of breaking the
    // x-equality.
    assert!(!overlaps(&rects[0], &rects[1]));
}

#[test]
fn layout_constrained_cross_component_sep_holds() {
    use crate::solve::vpsc::Sep;
    // Two disconnected components; a sep between them still holds because
    // projection runs on packed global coordinates.
    let ids = ids(&["a", "b", "c", "d"]);
    let sz = sizes(4, 100.0, 50.0);
    let edges = vec![(0usize, 1usize), (2, 3)]; // components {a,b} and {c,d}
    let seps = SepSpecs {
        x: vec![Sep { left: 3, right: 0, gap: 100.0 + 40.0, equality: false }],
        ..SepSpecs::default()
    };
    let (rects, _, dropped) =
        layout_constrained(&ids, &sz, &edges, &[], &seps, &StressConfig::default());
    assert!(dropped.0.is_empty());
    assert!(rects[3].x + 100.0 + 40.0 <= rects[0].x + 1e-6, "d left of a");
}

#[test]
fn layout_constrained_reports_contradictory_seps() {
    use crate::solve::vpsc::Sep;
    let ids = ids(&["a", "b"]);
    let sz = sizes(2, 100.0, 50.0);
    let seps = SepSpecs {
        x: vec![
            Sep { left: 0, right: 1, gap: 140.0, equality: false },
            Sep { left: 1, right: 0, gap: 140.0, equality: false },
        ],
        ..SepSpecs::default()
    };
    let (_, _, (dx, _)) =
        layout_constrained(&ids, &sz, &[(0, 1)], &[], &seps, &StressConfig::default());
    assert_eq!(dx, vec![1], "the later authored sep loses, and is reported");
}

#[test]
fn layout_constrained_is_deterministic() {
    use crate::solve::vpsc::Sep;
    let ids = ids(&["a", "b", "c", "d", "e"]);
    let sz = sizes(5, 100.0, 50.0);
    let edges = vec![(0usize, 1), (1, 2), (2, 3), (3, 4), (0, 4)];
    let seps = SepSpecs {
        x: vec![Sep { left: 0, right: 2, gap: 140.0, equality: false }],
        y: vec![Sep { left: 1, right: 3, gap: 90.0, equality: false }],
        ..SepSpecs::default()
    };
    let one = layout_constrained(&ids, &sz, &edges, &[], &seps, &StressConfig::default());
    let two = layout_constrained(&ids, &sz, &edges, &[], &seps, &StressConfig::default());
    assert_eq!(one, two);
}

#[test]
fn layout_constrained_with_seps_skips_reduce_crossings_but_grouped_default_keeps_it() {
    // Contract pin: authored seps disable the crossing post-pass (its moves
    // are sep-blind). Empty seps must keep byte-identical behavior to
    // layout_grouped — that equivalence is already asserted above; here we
    // only pin that a sep-carrying solve still returns overlap-free rects.
    use crate::solve::vpsc::Sep;
    let ids = ids(&["a", "b", "c", "d"]);
    let sz = sizes(4, 100.0, 50.0);
    let groups = vec![GroupSpec { members: vec![0, 1], depth: 0 }, GroupSpec { members: vec![2, 3], depth: 0 }];
    let edges = vec![(0usize, 2usize), (1, 3)];
    let seps = SepSpecs {
        x: vec![Sep { left: 0, right: 2, gap: 140.0, equality: false }],
        ..SepSpecs::default()
    };
    let (rects, hulls, _) =
        layout_constrained(&ids, &sz, &edges, &groups, &seps, &StressConfig::default());
    for i in 0..rects.len() {
        for j in (i + 1)..rects.len() {
            assert!(!overlaps(&rects[i], &rects[j]));
        }
    }
    assert_eq!(hulls.len(), 2);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml layout_constrained`
Expected: compile error — `SepSpecs`/`layout_constrained` undefined.

- [ ] **Step 3: Implement**

In `stress.rs`:

1. Add the `SepSpecs` struct (doc comment + `Default` as in the Interfaces block).
2. Refactor: the current `layout_grouped` body becomes `fn layout_grouped_inner(..., seps: &SepSpecs) -> (Vec<Rect>, Vec<Rect>, (Vec<usize>, Vec<usize>))` with the projection stage inserted; `layout_grouped` delegates with `&SepSpecs::default()` and drops the third tuple element (preserving its public signature byte-for-byte); `layout_constrained` is a thin public wrapper.
3. The projection stage, placed after component packing / before hull work:

```rust
// --- authored-constraint projection (no-op when seps are empty) ---
let n = rects.len();
let total = n + seps.extra_vars;
let (mut dropped_x, mut dropped_y) = (Vec::new(), Vec::new());
if !(seps.x.is_empty() && seps.y.is_empty()) {
    // Node weight 1.0; boundary vars 0.01 so they trail their members.
    let mut w = vec![1.0; total];
    for wv in w.iter_mut().skip(n) {
        *wv = 0.01;
    }
    let mut xs: Vec<f64> = rects.iter().map(|r| r.x).collect();
    xs.resize(total, 0.0); // constrain.rs emits containment seps that position these
    dropped_x = vpsc::project(&mut xs, &w, &seps.x);
    for (r, x) in rects.iter_mut().zip(&xs) {
        r.x = *x;
    }
    let mut ys: Vec<f64> = rects.iter().map(|r| r.y).collect();
    ys.resize(total, 0.0);
    dropped_y = vpsc::project(&mut ys, &w, &seps.y);
    for (r, y) in rects.iter_mut().zip(&ys) {
        r.y = *y;
    }
    // Overlap removal that CANNOT undo authored seps: fold them into the
    // constraint set. Extend remove_overlaps into remove_overlaps_with,
    // taking extra per-axis seps + extra var count; remove_overlaps
    // delegates with empties.
    remove_overlaps_with(rects, cfg.gap, &seps.x, &seps.y, seps.extra_vars);
    normalize_to_origin(rects);
} else {
    remove_overlaps(rects, cfg.gap);
    // ... existing flow unchanged (reduce_crossings etc.)
}
```

4. `remove_overlaps_with(rects, gap, extra_x: &[vpsc::Sep], extra_y: &[vpsc::Sep], extra_vars: usize)`: Task 2's loop, but (a) the variable vector is `n + extra_vars` long (boundary vars carried along, weight 0.01), (b) each pass appends `extra_x`/`extra_y` to the generated overlap seps **after** them (so authored-sep indices stay stable for drop-reporting — dropped indices ≥ generated-count map back by subtraction; simpler: pass authored seps FIRST and generated seps after, then a dropped index `< extra.len()` names an authored sep — use this ordering and document it), (c) dropped generated-seps are fine to ignore (they regenerate next pass).
5. Gate `reduce_crossings` on `seps.x.is_empty() && seps.y.is_empty()`.
6. `normalize_to_origin` (stress.rs:244) runs LAST either way so the min corner lands at origin like every caller expects.

- [ ] **Step 4: Run tests**

Run: `cargo test -p waml`
Expected: new tests PASS; `layout_grouped`-based goldens byte-identical (the empty-seps path must not change even one branch — the equivalence test pins this).

- [ ] **Step 5: Gate and commit**

```bash
git add crates/waml/src/solve/stress.rs
git commit -m "feat(solve): layout_constrained — hard seps on the stress solve

SepSpecs carries per-axis vpsc constraints over node indices plus group
boundary variables. Projection runs post-hoc on packed global
coordinates, then overlap removal folds the authored seps in so it
cannot un-satisfy them. Empty seps stay byte-identical to
layout_grouped; authored seps skip reduce_crossings (its moves are
sep-blind). Contradictions drop latest-first and are reported by index
for DroppedPlacement mapping."
```

---

### Task 4: Constraint compiler (`constrain.rs`)

**Files:**
- Create: `crates/waml/src/solve/constrain.rs`
- Modify: `crates/waml/src/solve/mod.rs` (add `pub mod constrain;`)
- Modify: `crates/waml/src/solve/geometry.rs` — extract the gap policy into `pub(super) fn pair_gap(...)` (currently inlined at geometry.rs:185-209) and mark `edge_axes`, `off_x`, `off_y` (geometry.rs:118-139) `pub(super)`; `solve_cluster` calls the extracted helper so the two paths can never drift.

**Interfaces:**
- Consumes: `Scene`, `Constraint`, `Direction`, `Edge` (existing); `SepSpecs` (Task 3); `DroppedPlacement` (geometry).
- Produces:

```rust
/// Everything the unified stress path needs from a resolved Scene.
pub struct Compiled {
    /// Leaf keys in solve order — index i here is sep/layout index i.
    pub keys: Vec<String>,
    /// One GroupSpec per Group box (scene order), members = descendant leaf
    /// indices — replaces the editor's flatten_groups on this path.
    pub group_specs: Vec<GroupSpec>,
    /// (title, depth) per group, same order.
    pub group_meta: Vec<(Option<String>, u8)>,
    pub seps: SepSpecs,
    /// FlagSet per leaf index (collapsed/emphasized survive unification).
    pub flags: Vec<FlagSet>,
    /// Constraints dropped at compile time (unknown operand, no shared axis),
    /// with conflicts_with empty.
    pub dropped: Vec<DroppedPlacement>,
    /// Sep index (per axis) -> originating Constraint, so solver-dropped
    /// seps map back to DroppedPlacement { relation, .. }.
    pub provenance_x: Vec<Option<Constraint>>,
    pub provenance_y: Vec<Option<Constraint>>,
}

pub fn compile(
    scene: &Scene,
    sizes: &BTreeMap<String, Size>,
    label_widths: &BTreeMap<(BoxId, BoxId), f64>,
    connected: &BTreeSet<(BoxId, BoxId)>,
    cfg: &SolveConfig,
) -> Compiled
```

Compilation rules (each gets a test):

1. **Leaf enumeration:** walk `scene.boxes`, collect `BoxId::Node(k)` leaves in scene order (dedup, first occurrence wins); collapsed leaves (`flags.collapsed`) report size `cfg.chip` — the caller substitutes that size, the compiler only records the flag.
2. **Groups:** every `BoxKind::Group` box becomes a `GroupSpec` (descendant leaves via recursive walk, depth from `Box::depth`) + meta `(title, depth)`. `Inline` boxes are treated identically to groups (members cluster) but emit no hull meta — matching today's inline-group behavior of clustering without a frame: give them a `GroupSpec` and a `(None, depth)` meta with a `shape: Shape::Shrink`-style no-frame marker; concretely, return them in `group_specs` but with meta title `None`, and the editor (Task 5) skips hull emission for meta with `title == None && depth-marker`… **Simplification, locked:** inline boxes contribute a `GroupSpec` for cohesion and NO entry in `group_meta`; keep two parallel vecs `group_specs_all` (cohesion) and the hull-bearing prefix — implement as: `group_specs: Vec<GroupSpec>` for Group boxes only, plus `inline_specs: Vec<GroupSpec>` appended by the caller to the cohesion list. Add `pub inline_specs: Vec<GroupSpec>` to `Compiled`.
3. **Axis boxes:** a box with `axis: Some(Row)` emits `LeftOf` chains over adjacent children (`windows(2)`), `Some(Column)` emits `Above` chains — reuse `geometry::axis_constraints` (make it `pub(super)`), then compile those like authored constraints.
4. **`Place { a, b, dir }` with two Node operands** at indices `ia, ib` with sizes `sa, sb`: gap = `pair_gap(...)` (the extracted geometry helper: `margin(max(ma,mb))`, floored to `min_assoc` + label widths for connected horizontal pairs, else `min_sep`; margins come from the operands' `Box::margin`). Emit:
   - `LeftOf` → x-sep `{ left: ia, right: ib, gap: sa.w + gap }`
   - `RightOf` → x-sep `{ left: ib, right: ia, gap: sb.w + gap }`
   - `Above` → y-sep `{ left: ia, right: ib, gap: sa.h + gap }`
   - `Below` → y-sep `{ left: ib, right: ia, gap: sb.h + gap }`
   - Diagonals emit both components (`AboveLeft` = `Above` + `LeftOf`, etc.), each with the same `gap` — mirroring `place_deltas` (geometry.rs:25) spending the gap on both axes.
5. **`Align { a, a_edge, b, b_edge }`:** via `edge_axes`; on each shared axis emit an **equality** sep `{ left: ia, right: ib, gap: off(a_edge, sa) - off(b_edge, sb), equality: true }` using `off_x`/`off_y`. (Note sign: equality means `pos[ib] - pos[ia] == gap`; `pos` is the min corner, and `min_a + off_a == min_b + off_b` ⇔ `min_b - min_a == off_a - off_b`.) No shared axis → compile-time drop with the existing "alignment edges share no axis" wording left to the caller; the compiler records it in `dropped`.
6. **Group operands (`BoxId::Group`/`BoxId::Inline` in a constraint):** boundary variables. Per referenced group g and axis, allocate two extra vars `Lg`, `Rg` (x) / `Tg`, `Bg` (y) — indices `n + k` in allocation order (deterministic: constraint scan order, first reference allocates). Emit containment seps for every member leaf m: x: `{ left: Lg, right: m, gap: cfg' hull pad }` and `{ left: m, right: Rg, gap: size(m).w + pad }` (pad = `StressConfig::default().hull_pad` — pass it in as a param `hull_pad: f64` rather than reaching for StressConfig). Then compile the Place/Align against `Rg`/`Lg`/`Tg`/`Bg` as the operand's proxy: `Place{ g LeftOf b }` → x-sep `{ left: Rg, right: ib, gap: gap }` (the boundary var already sits past the members' width, so no `+w` term). Provenance maps every emitted sep back to the originating authored `Constraint`.
7. **Unknown operands** (a `BoxId` not in the scene / a leaf without a size): compile-time drop, recorded in `dropped`.
8. **Non-sibling constraints are now legal.** `solve_cluster` warned and dropped them; the flat solve honors any pair. The old "layout relates operands that are not siblings; dropped" diagnostic (geometry.rs:650) stays in geometry for the wasm/CLI path but the compiler intentionally accepts them. Pin this improvement with a test.

- [ ] **Step 1: Extract `pair_gap` + visibility changes in geometry.rs; run existing tests**

Extract geometry.rs:185-209 into:

```rust
/// Gap policy shared by solve_cluster and constrain::compile: margin of the
/// wider-margined operand, floored to min_assoc (+ terminal-label widths on
/// horizontally-related connected pairs) or min_sep for unconnected pairs.
pub(super) fn pair_gap(
    a: &BoxId, b: &BoxId,
    (sa, ma): (Size, Margin), (sb, mb): (Size, Margin),
    dir_is_horizontal: bool,
    connected: &BTreeSet<(BoxId, BoxId)>,
    label_widths: &BTreeMap<(BoxId, BoxId), f64>,
    cfg: &SolveConfig,
) -> f64
```

`solve_cluster` calls it; behavior identical. (`sa`/`sb` are unused in the body today — keep the params anyway, the signature documents the policy's inputs; if clippy objects, prefix with `_`.)

Run: `cargo test -p waml` — all green (pure refactor).

- [ ] **Step 2: Write the failing compiler tests**

`constrain.rs` test module — build small `Scene`s by hand (see geometry.rs:740-783 `leaf`/`group` helpers for the pattern; replicate locally, don't share test helpers across modules):

```rust
#[test]
fn place_left_of_compiles_to_an_x_sep_with_the_connected_gap() { /* two nodes,
    connected pair, assert one x-sep, gap == 100.0 (node w) + 72.0 (min_assoc),
    provenance_x[0] is the Place constraint */ }

#[test]
fn place_above_right_emits_one_sep_per_axis() { /* diagonal: x-sep b->a and
    y-sep a->b, same gap on both */ }

#[test]
fn align_left_edges_compiles_to_a_zero_gap_equality() { /* Edge::Left both:
    equality x-sep gap 0.0; Edge::Center vs Edge::Left on 100-wide boxes:
    gap == 50.0 - 0.0 signed correctly */ }

#[test]
fn align_top_to_left_is_dropped_no_shared_axis() { /* dropped.len()==1,
    seps empty */ }

#[test]
fn row_axis_group_chains_left_of_seps() { /* Box axis Some(Row), 3 children:
    2 x-seps in child order */ }

#[test]
fn cross_group_node_constraint_is_kept_not_dropped() { /* nodes under two
    different parent groups + root-level Place between them: 1 sep, 0 dropped
    — the old path warned and dropped */ }

#[test]
fn group_operand_allocates_boundary_vars_with_containment() { /* Place{ G
    LeftOf n }: extra_vars == 2 (x pair; y untouched -> still allocate the
    pair per axis ONLY on axes the constraint touches — assert extra_vars
    matches implementation, containment seps: 2 per member + 1 relation sep,
    provenance points at the Place */ }

#[test]
fn unknown_operand_is_reported_not_panicking() { /* Place with a BoxId::Node
    key absent from the scene: 0 seps, dropped.len()==1 */ }

#[test]
fn compile_is_deterministic() { /* same scene twice, assert Compiled
    (derive PartialEq where needed or compare fields) equal */ }

#[test]
fn collapsed_leaf_reports_its_flag() { /* Box with flags.collapsed: flags[i]
    .collapsed true */ }
```

Write them as real tests (each builds its scene inline with real `Box`/`Constraint` literals — the sketches above name the assertions; flesh out the fixture code following the geometry.rs test-module pattern).

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p waml constrain`
Expected: compile error — module doesn't exist.

- [ ] **Step 4: Implement `compile`**

Single top-down implementation: leaf/group enumeration walk, then a constraint loop with a `BoundaryVars` allocator (`BTreeMap<(BoxId /*group*/, u8 /*axis*/), (usize, usize)>` — BTreeMap keyed on the group's BoxId keeps allocation deterministic given the deterministic constraint scan). Every emitted sep pushes its provenance. Keep the whole module free of `HashMap`/`HashSet`.

- [ ] **Step 5: Run tests, gate, commit**

Run: `cargo test -p waml`

```bash
git add crates/waml/src/solve/constrain.rs crates/waml/src/solve/mod.rs crates/waml/src/solve/geometry.rs
git commit -m "feat(solve): compile Scene layout constraints to stress seps

constrain::compile lowers authored Place/Align statements (and Row/
Column axis chains) into per-axis vpsc seps over leaf indices, with
boundary variables for group operands and provenance back to the
authored Constraint for conflict reporting. Gap policy is the extracted
geometry::pair_gap, so the two paths cannot drift. Non-sibling
constraints — warned-and-dropped by solve_cluster — are now honored."
```

---

### Task 5: Editor unification — one layout path

**Files:**
- Modify: `crates/waml-editor/src/scene.rs`:
  - `use_stress_default` (scene.rs:270-272): delete fn.
  - `build_scene` dispatch (scene.rs:776-805): remove the branch; every diagram goes through the (renamed) stress path.
  - `stress_default` (scene.rs:622-715): rename to `stress_layout`, new signature below; group specs now come from `constrain::Compiled` (single source of truth) instead of `flatten_groups`; flags populated; collapsed sizes chipped.
- Modify/audit: `flatten_groups` + `GroupMeta` + `entangled_group_pairs` (callers change), `crossing_baseline_report` (scene.rs:2562) — keep it compiling against the new signature.
- Test: scene.rs test module + regenerate `crates/waml/tests/solver_golden.rs` — **read Step 5 before touching it**.

**Interfaces:**
- Consumes: `constrain::{compile, Compiled}`, `stress::{layout_constrained, SepSpecs}`, `resolve::resolve` (existing — already produces the `Scene` with constraints from `diagram.layout` AND the group forest from `diagram.groups`).
- Produces: `fn stress_layout(diagram: &Diagram, model_edges: &[&waml::model::Edge], sizes: &SizeMap) -> (Solved, SolvedRouting, Vec<(String, String)>, Vec<DroppedPlacement>, Vec<Diagnostic>)`

- [ ] **Step 1: Write the failing integration tests**

In scene.rs's test module (follow its existing fixture patterns for building a `Model`/`Diagram`):

```rust
#[test]
fn an_authored_place_hint_biases_the_stress_layout() {
    // Diagram: three connected entities, one authored `place a above b`.
    // Unified path: layout comes from stress (edges pull c near its
    // neighbors — no strip), AND a sits fully above b.
}

#[test]
fn a_diagram_with_hints_no_longer_strip_packs_unrelated_nodes() {
    // The screenshot regression: two connected nodes + one hint between two
    // OTHER nodes. Old path: the connected pair lands in separate rigid
    // components, packed into a horizontal strip at y-top 0. New path:
    // the connected pair's center distance is under 2 * edge_len.
}

#[test]
fn contradictory_hints_still_reach_the_conflict_report() {
    // `place a left-of b` + `place b left-of a` (via layout statements):
    // solve succeeds, exactly one DroppedPlacement comes back, and its
    // relation is the SECOND authored statement.
}

#[test]
fn hintless_diagrams_are_byte_identical_to_before() {
    // Golden pin: a no-layout diagram's Solved (pretty() dump) matches the
    // pre-unification stress_default output. Capture the expected string
    // BEFORE refactoring (run the old code, paste the dump).
}

#[test]
fn collapsed_flag_survives_the_unified_path() {
    // A collapsed node on a hint-carrying diagram: solved.flags carries
    // collapsed=true and the rect has chip size (96x28). This was silently
    // dead on the old stress path (flags: BTreeMap::new()) — pin the fix.
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml-editor scene`
Expected: the first three and the fifth FAIL (old dispatch still active); the fourth PASSES (it pins current behavior — keep it passing throughout).

- [ ] **Step 3: Implement the unification**

In `build_scene`, replace the `if use_stress_default(diagram) { … } else { … }` with a single call:

```rust
let (scene, resolve_diags) = waml::solve::resolve::resolve(diagram);
let compiled = waml::solve::constrain::compile(
    &scene,
    &sizes,
    &connected_label_widths_map, // same inputs solve_diagram_routed built
    &connected_pairs,
    &SolveConfig::default(),
);
let (mut solved, routing, entangled, dropped, mut diags) =
    stress_layout(&compiled, &sizes, &model_edges);
diags.extend(resolve_diags);
```

`stress_layout` body = old `stress_default` with these deltas:
1. `keys`/`ids` come from `compiled.keys` (scene order) — NOT `sizes.keys()`. Every sized key absent from the scene appends after (unknown/synthetic nodes still render, matching old behavior of solving every sized member).
2. Collapsed leaves (`compiled.flags[i].collapsed`) override their entry in `dims` with `SolveConfig::default().chip` before solving.
3. Cohesion groups = `compiled.group_specs` ++ `compiled.inline_specs`; hulls/meta from `compiled.group_specs` + `compiled.group_meta` (replaces `flatten_groups`; delete it and `GroupMeta` if no other caller remains — check `entangled_group_pairs`, adapt it to `(Option<String>, u8)` meta).
4. Solve via `layout_constrained(&ids, &dims, &pairs, &cohesion_groups, &compiled.seps, &cfg)`.
5. Dropped seps → `DroppedPlacement`: for each index in the returned `(dx, dy)`, look up `compiled.provenance_x/y[idx]`; `Some(c)` → `DroppedPlacement { relation: c, conflicts_with: vec![] }`, dedup by relation (a diagonal dropped on both axes reports once). Append `compiled.dropped`. These flow to `build_scene`'s existing `dropped` return slot (the old stress arm returned `Vec::new()` there — now it's real).
6. `solved.flags` from `compiled.flags` (keyed by `compiled.keys`) — no longer empty.
7. Routing/labels/hulls: unchanged from old `stress_default` (`route_with_groups`, entangled warning, `SolvedRouting` handback).

Then delete `use_stress_default`. `solve_diagram_routed` keeps existing callers (wasm/CLI/tests) — the editor just stops calling it. If that leaves `sizing_requests` (scene.rs:775) unused, feed it through to `stress_layout` for the label-width map the compiler consumes (that's where `connected_label_widths_map` comes from — reuse `waml::solve`'s `connected_label_widths` logic; it's private in mod.rs, make it `pub(crate)`… it's a different crate — instead re-derive in scene.rs with the same fold over `label::measure`, or promote the mod.rs helper to `pub`. **Locked: promote `connected_label_widths` to `pub` in `crates/waml/src/solve/mod.rs` with a doc comment; both callers share it.**)

- [ ] **Step 4: Run the editor + workspace tests**

Run: `cargo test --workspace`
Expected: Task-5 tests PASS, `hintless_diagrams_are_byte_identical_to_before` still PASSES.

- [ ] **Step 5: Regenerate the authored-path golden deliberately**

`crates/waml/tests/solver_golden.rs::orders_domain_diagram_solves_to_expected_layout` pins the **waml-crate** authored path (`solve_diagram`), which this plan does NOT change — it must still pass untouched. Any editor-crate golden that pinned authored-diagram pixel output WILL legitimately change: for each, run, eyeball the new `pretty()` dump for sanity (no overlaps, hints honored, no strip), then update the expectation. List every regenerated golden in the commit body.

- [ ] **Step 6: Gate and commit**

```bash
git add crates/waml-editor/src/scene.rs crates/waml/src/solve/mod.rs
git commit -m "feat(editor): one layout path — hints constrain the stress solve

Delete the use_stress_default dispatch. Every diagram now places via
layout_constrained: authored ## Layout statements compile to hard
separation/alignment constraints on the stress solve instead of
switching the whole diagram onto the edge-blind rigid-offset strip
packer. Contradictions surface as DroppedPlacement in the existing
conflict list; collapsed/emphasized flags now survive on all diagrams.
waml::solve::geometry remains for the wasm/CLI path (follow-up)."
```

---

## Follow-ups (explicitly OUT of scope, do not attempt)

1. **Visual sign-off** — owed to the user: preset fixtures + the Connectors/CI-CD diagram before/after. Plan gate is tests only.
2. Migrate wasm/CLI (`solve_diagram*`) onto `constrain` + `layout_constrained`, then retire `solve_cluster`'s strip packing.
3. Per-iteration IPSep-CoLa projection inside `majorize` (quality refinement; post-hoc is v1).
4. Inheritance auto-constraints (`y_sub ≥ y_super` from edge kinds, no authoring needed) — the DiG-CoLa quick win from the research briefing; trivially expressible as `SepSpecs.y` once this lands.
5. Sep-aware `reduce_crossings` (today: skipped when seps exist).

## Self-Review Notes

- Spec coverage: VPSC port (T1), minimal-displacement overlap removal (T2), constraint projection in stress (T3), hint compilation incl. groups/axis/align/diagonals/conflicts (T4), editor single-path + conflict-UI + flags contract (T5). The three goals from the discussion (VPSC first, overlap swap, hints-as-constraints) all land.
- Type consistency: `Sep{left,right,gap,equality}` and `project(pos,weight,seps)->Vec<usize>` used identically in T2/T3/T4; `SepSpecs{x,y,extra_vars}` in T3/T4/T5; `Compiled` fields consumed in T5 step 3 match T4's struct; `pair_gap` extracted in T4 step 1 before its T4 step 4 use.
- Known risk, accepted: T1's split pass is the subtlest code in the plan; its 11 tests are the contract and the step-3 note licenses internal simplification as long as they pass.
