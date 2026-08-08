//! Variable Placement with Separation Constraints (VPSC).
//!
//! Minimal-displacement projection of scalar positions onto separation
//! constraints `pos[left] + gap <= pos[right]` — the primitive behind
//! IPSep-CoLa constrained stress layout (Dwyer, Koren, Marriott, TVCG 2006)
//! and minimal-displacement overlap removal (Dwyer, Marriott, Stuckey,
//! GD 2005). Ported from the WebCola `vpsc.ts` block merge/split solver.
//! Deterministic: ties break on constraint index; no RNG, no maps.

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
                // An equality is violated in EITHER direction: `v < -EPS`
                // means the block's rigid offsets already hold the pair
                // further apart than this equality demands (e.g. two
                // contradictory `align` gaps, the larger merged first).
                // Without the negative check that contradiction sailed
                // through silently -- dropped=[] with the earlier-authored
                // equality broken in the output.
                if v > EPS || (s.equality && v < -EPS) {
                    // In-block violation = a cycle through active seps: `ci`
                    // plus the unique active-tree path s.right -> s.left.
                    // Victim candidates are restricted to exactly those seps
                    // -- a satisfiable constraint that merely merged into the
                    // same block (a spur off the cycle) must never be evicted
                    // (it would go silently unenforced AND be falsely
                    // reported as conflicting). Authored-order policy: the
                    // latest (max index) constraint on the cycle loses.
                    let actives: Vec<usize> = blocks[bl]
                        .active
                        .iter()
                        .copied()
                        .filter(|&a| live[a])
                        .collect();
                    let victim = tree_path(seps, &actives, n, s.right, s.left)
                        .into_iter()
                        .chain(std::iter::once(ci))
                        .max()
                        .unwrap();
                    live[victim] = false;
                    dropped.push(victim);
                    continue 'restart;
                }
                continue; // redundant, already satisfied inside the block
            }
            // Merge right block into left block so that
            // pos(left) + gap == pos(right) exactly.
            let off_l = blocks[bl]
                .members
                .iter()
                .find(|(m, _)| *m == s.left)
                .unwrap()
                .1;
            let off_r = blocks[br]
                .members
                .iter()
                .find(|(m, _)| *m == s.right)
                .unwrap()
                .1;
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
                        Block {
                            posn: num / den,
                            members,
                            active,
                        }
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

/// Sep indices on the unique path between vars `from` and `to` through the
/// undirected edges `actives` (indices into `seps`). One block's active seps
/// form a spanning tree over its members -- each merge adds exactly one
/// active edge joining two previously-disjoint variable sets -- so when
/// `from` and `to` share a block the path exists and is unique, and the
/// traversal order cannot affect the result (determinism). `n` bounds the
/// variable index space.
fn tree_path(seps: &[Sep], actives: &[usize], n: usize, from: usize, to: usize) -> Vec<usize> {
    // prev[v] = (previous var, sep index used to reach v).
    let mut prev: Vec<Option<(usize, usize)>> = vec![None; n];
    let mut visited = vec![false; n];
    visited[from] = true;
    let mut stack = vec![from];
    while let Some(v) = stack.pop() {
        if v == to {
            break;
        }
        for &ai in actives {
            let s = seps[ai];
            let next = if s.left == v {
                s.right
            } else if s.right == v {
                s.left
            } else {
                continue;
            };
            if !visited[next] {
                visited[next] = true;
                prev[next] = Some((v, ai));
                stack.push(next);
            }
        }
    }
    let mut path = Vec::new();
    let mut cur = to;
    while let Some((p, ai)) = prev[cur] {
        path.push(ai);
        cur = p;
    }
    path
}

fn satisfied(pos: &[f64], seps: &[Sep], live: &[bool]) -> bool {
    seps.iter().enumerate().all(|(i, s)| {
        !live[i] || {
            let d = pos[s.right] - pos[s.left] - s.gap;
            if s.equality {
                d.abs() <= EPS
            } else {
                d >= -EPS
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sep(left: usize, right: usize, gap: f64) -> Sep {
        Sep {
            left,
            right,
            gap,
            equality: false,
        }
    }

    #[test]
    fn satisfied_input_is_untouched() {
        let mut pos = vec![0.0, 10.0, 25.0];
        let dropped = project(
            &mut pos,
            &[1.0, 1.0, 1.0],
            &[sep(0, 1, 5.0), sep(1, 2, 5.0)],
        );
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
            &[Sep {
                left: 0,
                right: 1,
                gap: 10.0,
                equality: true,
            }],
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
    fn cycle_victim_is_on_the_cycle_not_an_innocent_spur() {
        // a+10<=b and b+10<=a are the contradiction; c+40<=a is a satisfiable
        // spur that merges into the same block before the cycle is detected.
        // Only the cycle's latest sep (index 1) may be evicted -- the spur
        // (index 2) must stay enforced and unreported.
        let mut pos = vec![0.0, 0.0, 0.0];
        let dropped = project(
            &mut pos,
            &[1.0; 3],
            &[sep(0, 1, 10.0), sep(1, 0, 10.0), sep(2, 0, 40.0)],
        );
        assert_eq!(dropped, vec![1]);
        assert!(pos[0] + 10.0 <= pos[1] + 1e-9, "surviving sep 0 holds");
        assert!(
            pos[2] + 40.0 <= pos[0] + 1e-9,
            "innocent spur sep 2 must stay enforced: {pos:?}"
        );
    }

    #[test]
    fn contradictory_equalities_drop_the_later_and_enforce_the_earlier() {
        // Two contradictory equalities on the same pair (gaps 5 and 10): the
        // LATER-authored one (index 1) loses and is reported; the earlier one
        // is enforced exactly. Regression: the in-block violation check only
        // caught `v > EPS`, so an equality violated in the NEGATIVE direction
        // sailed through -- dropped=[] with the earlier equality silently
        // broken in the output.
        let eq = |gap: f64| Sep {
            left: 0,
            right: 1,
            gap,
            equality: true,
        };
        let mut pos = vec![0.0, 0.0];
        let dropped = project(&mut pos, &[1.0, 1.0], &[eq(5.0), eq(10.0)]);
        assert_eq!(dropped, vec![1]);
        assert!(
            (pos[1] - pos[0] - 5.0).abs() < 1e-9,
            "earlier equality enforced exactly: {pos:?}"
        );
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
