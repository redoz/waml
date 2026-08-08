//! Semi-smart default layout via stress majorization (SMACOF).
//!
//! Edge-aware placement: connected nodes are pulled toward each other so the
//! result reflects the model's relationships instead of an edge-blind
//! left-to-right strip. Fully deterministic (circular seed, fixed iteration
//! order, no RNG) — same input, same pixels. Authored `## Layout` statements
//! enter through `layout_constrained`'s compiled seps (`constrain::compile`).
//!
//! See docs/superpowers/specs/2026-07-21-default-layout-stress-majorization-design.md.

use super::crossing::segment_crossings_touching;
use super::{BoxId, Rect, Size, SolveConfig};
use crate::layout::Margin;
use std::collections::{HashMap, HashSet, VecDeque};
use std::f64::consts::PI;

/// Tunables for the stress solve. Defaults are a first pass; the spec calls for
/// tuning `edge_len`/`gap` from a real-model screenshot (Phase 2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StressConfig {
    /// Ideal pixels per graph hop (the SMACOF target-distance unit, `L`).
    pub edge_len: f64,
    /// Hard cap on majorization iterations per component.
    pub max_iter: u32,
    /// Convergence threshold on the absolute stress delta between iterations.
    pub epsilon: f64,
    /// Minimum pixels between node boxes after overlap removal.
    pub gap: f64,
    /// Ideal co-member separation for grouped nodes (the group's target-distance
    /// unit). Shorter than `edge_len` so groups pull tighter than a bare edge.
    pub group_len: f64,
    /// Weight multiplier applied to co-member pairs, per group-nesting depth.
    pub group_weight: f64,
    /// Padding from a group's member bounding box out to its hull.
    pub hull_pad: f64,
    /// Cap on `reduce_crossings` hill-climb *sweeps* over the candidate move
    /// set -- not on accepted moves: one sweep applies every improving move it
    /// finds, and the climb stops early as soon as a whole sweep improves
    /// nothing, so this only bounds the worst case on a large graph. `0`
    /// disables the pass entirely -- the escape hatch and the A/B mechanism
    /// for judging it (see `docs/superpowers/plans/2026-08-07-crossing-aware-placement.md`).
    pub crossing_passes: u32,
}

impl Default for StressConfig {
    fn default() -> Self {
        StressConfig {
            edge_len: 120.0,
            max_iter: 300,
            epsilon: 1e-4,
            gap: SolveConfig::default().min_sep,
            // Tuned against the WAML Domain Model view (13 nodes, 4 groups) by
            // rendering weight = 4 / 12 / 30 side by side. 4 and 12 both left
            // the `Views` group split — Behavioral View drifted away from
            // Diagram and Profile — and only at 30 did all three tiers read as
            // distinct blocks.
            //
            // PROVISIONAL. That comparison judged cluster separation ALONE.
            // The router does not minimize edge crossings (see
            // docs/superpowers/specs/2026-08-06-edge-crossing-reduction-design.md),
            // and tighter clusters bundle more edges into the same corridors,
            // so crossing behavior is the next thing to change and these two
            // numbers should be re-judged when it does.
            group_len: 120.0 * 0.625,
            group_weight: 30.0,
            hull_pad: SolveConfig::default().margin(Margin::Medium),
            // The pass ships enabled; see the field doc for the opt-out.
            crossing_passes: 8,
        }
    }
}

/// One diagram group's membership for the cohesion force.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupSpec {
    /// Indices into `ids`/`sizes` of every member, including members of nested
    /// children — a nested group's members are also listed in its ancestors.
    pub members: Vec<usize>,
    /// Nesting depth; 0 is top level. Deeper groups bind tighter.
    pub depth: u8,
}

/// Per-axis separation constraints over layout indices. Indices `0..n-1` are
/// the nodes (`ids` order); indices `n..` address the boundary variables
/// appended by `constrain.rs`'s `boundary_vars`, in that same order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SepSpecs {
    pub x: Vec<super::vpsc::Sep>,
    pub y: Vec<super::vpsc::Sep>,
    /// Extra positionless variables (group boundaries): this many are
    /// appended after the node variables on BOTH axes.
    pub extra_vars: usize,
}

/// Guard against division by zero when two points coincide (Guttman denom).
const COINCIDENT_EPS: f64 = 1e-9;

/// Scalar half-extent proxy for a box: the mean of its half-width and
/// half-height. Used to inflate target distances so adjacent boxes leave room
/// for their own footprints before the final scan-line push-apart.
fn half_extent(s: &Size) -> f64 {
    (s.w + s.h) / 4.0
}

/// Lay out `ids` (with matching `sizes`) under undirected `edges` (index pairs
/// into `ids`/`sizes`). Returns one `Rect` per input id, in input order, with
/// the min corner translated to the origin (matching `assemble`'s convention).
pub fn layout(
    ids: &[BoxId],
    sizes: &[Size],
    edges: &[(usize, usize)],
    cfg: &StressConfig,
) -> Vec<Rect> {
    layout_grouped(ids, sizes, edges, &[], cfg).0
}

/// Lay out `ids` under undirected `edges` plus a soft cohesion force from
/// `groups`: co-members are pulled toward a shorter target distance and
/// weighted more heavily in the SMACOF solve, without hard-constraining them —
/// a strong outside edge can still pull a member away. Returns `(node rects,
/// one hull rect per group in `groups` order)`. With `groups` empty this is
/// exactly `layout`'s behavior (the regression guard for "groups change
/// nothing when there are none").
pub fn layout_grouped(
    ids: &[BoxId],
    sizes: &[Size],
    edges: &[(usize, usize)],
    groups: &[GroupSpec],
    cfg: &StressConfig,
) -> (Vec<Rect>, Vec<Rect>) {
    let (rects, hulls, _) =
        layout_grouped_inner(ids, sizes, edges, groups, &SepSpecs::default(), cfg);
    (rects, hulls)
}

/// `layout_grouped` + hard constraints: stress-solve, pack components, then
/// project `seps` and re-run overlap removal with the authored seps folded
/// in so it cannot un-satisfy them. Returns `(node rects, hull rects,
/// (dropped x-sep indices, dropped y-sep indices))`. With `seps` empty this
/// is byte-identical to `layout_grouped` (see
/// `layout_constrained_empty_seps_matches_layout_grouped`).
#[allow(clippy::type_complexity)]
pub fn layout_constrained(
    ids: &[BoxId],
    sizes: &[Size],
    edges: &[(usize, usize)],
    groups: &[GroupSpec],
    seps: &SepSpecs,
    cfg: &StressConfig,
) -> (Vec<Rect>, Vec<Rect>, (Vec<usize>, Vec<usize>)) {
    layout_grouped_inner(ids, sizes, edges, groups, seps, cfg)
}

/// Project authored `seps` onto already-packed global coordinates, then run
/// overlap removal with those seps folded in so it cannot undo them. Returns
/// the authored-sep drop report (see `remove_overlaps_with`). No-op (aside
/// from the trivial empty-seps projection) when `seps` is empty; callers
/// only invoke this when at least one axis is non-empty.
fn apply_seps_and_overlap(
    rects: &mut [Rect],
    seps: &SepSpecs,
    cfg: &StressConfig,
) -> (Vec<usize>, Vec<usize>) {
    let n = rects.len();
    let total = n + seps.extra_vars;
    let mut w = vec![1.0_f64; total];
    for wv in w.iter_mut().skip(n) {
        *wv = 0.01;
    }
    let mut xs: Vec<f64> = rects.iter().map(|r| r.x).collect();
    xs.resize(total, 0.0); // constrain.rs emits containment seps that position these
    super::vpsc::project(&mut xs, &w, &seps.x);
    for (r, x) in rects.iter_mut().zip(xs.iter().take(n)) {
        r.x = *x;
    }
    let mut ys: Vec<f64> = rects.iter().map(|r| r.y).collect();
    ys.resize(total, 0.0);
    super::vpsc::project(&mut ys, &w, &seps.y);
    for (r, y) in rects.iter_mut().zip(ys.iter().take(n)) {
        r.y = *y;
    }
    remove_overlaps_with(rects, cfg.gap, &seps.x, &seps.y, seps.extra_vars)
}

#[allow(clippy::type_complexity)]
fn layout_grouped_inner(
    ids: &[BoxId],
    sizes: &[Size],
    edges: &[(usize, usize)],
    groups: &[GroupSpec],
    seps: &SepSpecs,
    cfg: &StressConfig,
) -> (Vec<Rect>, Vec<Rect>, (Vec<usize>, Vec<usize>)) {
    let has_seps = !(seps.x.is_empty() && seps.y.is_empty());
    let n = ids.len();
    assert_eq!(n, sizes.len(), "ids and sizes length mismatch");
    if n == 0 {
        return (vec![], vec![], (Vec::new(), Vec::new()));
    }
    if n == 1 {
        let mut rects = vec![Rect {
            x: 0.0,
            y: 0.0,
            w: sizes[0].w,
            h: sizes[0].h,
        }];
        let dropped = if has_seps {
            apply_seps_and_overlap(&mut rects, seps, cfg)
        } else {
            (Vec::new(), Vec::new())
        };
        normalize_to_origin(&mut rects);
        let hulls = group_hulls(&rects, groups, cfg);
        return (rects, hulls, dropped);
    }

    let clean = dedup_edges(n, edges);
    let co_depth = comembership_depths(groups);
    let co_edges: Vec<(usize, usize)> = co_depth.keys().copied().collect();
    // Merged adjacency: real edges plus a clique per group, so a group can
    // never be split across two independently shelf-packed components, and
    // `bfs_hops` defines a distance for every co-member pair.
    let merged: Vec<(usize, usize)> = {
        let mut m = clean.clone();
        m.extend(co_edges.iter().copied());
        dedup_edges(n, &m)
    };
    if merged.is_empty() {
        // No meaningful distances — degenerate. Fall back to the grid, but still
        // honor the hull contract: one hull per group, in `groups` order. This
        // arm is reachable with non-empty `groups` whenever every group has
        // fewer than two members (no clique edges) and there are no real edges.
        let mut rects = grid_pack(ids, sizes, cfg);
        let dropped = if has_seps {
            apply_seps_and_overlap(&mut rects, seps, cfg)
        } else {
            separate_hulls(&mut rects, groups, cfg);
            (Vec::new(), Vec::new())
        };
        normalize_to_origin(&mut rects);
        let hulls = group_hulls(&rects, groups, cfg);
        debug_assert_eq!(hulls.len(), groups.len());
        return (rects, hulls, dropped);
    }

    let adj = adjacency(n, &merged);
    let comps = components(n, &adj);

    // Solve each component independently, normalizing its min corner to the
    // origin and recording its bounding box for packing.
    struct Laid {
        comp: Vec<usize>,
        rects: Vec<Rect>, // local, min corner at (0,0)
        w: f64,
        h: f64,
    }
    let mut laid: Vec<Laid> = Vec::with_capacity(comps.len());
    for comp in comps {
        let mut rects = component_layout(&comp, sizes, &adj, cfg, &co_depth);
        remove_overlaps(&mut rects, cfg.gap);
        let (min_x, min_y) = rects
            .iter()
            .fold((f64::INFINITY, f64::INFINITY), |(mx, my), r| {
                (mx.min(r.x), my.min(r.y))
            });
        let (mut w, mut h) = (0.0_f64, 0.0_f64);
        for r in &mut rects {
            r.x -= min_x;
            r.y -= min_y;
            w = w.max(r.x + r.w);
            h = h.max(r.y + r.h);
        }
        laid.push(Laid { comp, rects, w, h });
    }

    // Shelf-pack the components toward a roughly landscape aspect rather than a
    // single left-to-right row (which strings singletons into a long tail).
    // Target row width = sqrt(total area) biased wide; deterministic order.
    let total_area: f64 = laid.iter().map(|l| l.w * l.h).sum();
    let widest = laid.iter().fold(0.0_f64, |m, l| m.max(l.w));
    let target_w = (total_area.sqrt() * 1.4).max(widest);

    let zero = Rect {
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
    };
    let mut out = vec![zero; n];
    let (mut cursor_x, mut cursor_y, mut shelf_h) = (0.0_f64, 0.0_f64, 0.0_f64);
    for l in &laid {
        if cursor_x > 0.0 && cursor_x + l.w > target_w {
            cursor_x = 0.0;
            cursor_y += shelf_h + cfg.gap;
            shelf_h = 0.0;
        }
        for (local, r) in l.rects.iter().enumerate() {
            out[l.comp[local]] = Rect {
                x: r.x + cursor_x,
                y: r.y + cursor_y,
                w: r.w,
                h: r.h,
            };
        }
        cursor_x += l.w + cfg.gap;
        shelf_h = shelf_h.max(l.h);
    }
    // Crossings are a whole-diagram property (an edge can span components), so
    // this runs here on the fully assembled `out` rather than per-component --
    // the only point in this function with both global rects and global
    // `edges` in scope. The pass cleans up after itself: with groups present a
    // move is rejected outright if it would worsen top-level hull overlap and
    // `separate_hulls` below re-runs `remove_overlaps`; with *no* groups that
    // hull guard is vacuous, so `reduce_crossings` runs `remove_overlaps`
    // itself before returning (see its docs). Nothing else here does. The pass
    // measures against the eventual output, not this mid-pipeline snapshot.
    //
    // With authored seps present, `reduce_crossings` and `separate_hulls` are
    // both skipped: their moves are sep-blind (whole-group/whole-node bulk
    // translations) and could un-satisfy a constraint that
    // `apply_seps_and_overlap`'s per-pair projection just enforced. A
    // diagram with authored hints already has a human's opinion baked in.
    let dropped = if has_seps {
        apply_seps_and_overlap(&mut out, seps, cfg)
    } else {
        reduce_crossings(&mut out, groups, &clean, cfg);
        separate_hulls(&mut out, groups, cfg);
        (Vec::new(), Vec::new())
    };

    // Normalize the min corner to the origin, matching `layout`'s convention;
    // the separation pass can push rects negative.
    normalize_to_origin(&mut out);

    let hulls = group_hulls(&out, groups, cfg);
    debug_assert_eq!(hulls.len(), groups.len());
    (out, hulls, dropped)
}

/// Translate `rects` so their bounding box's min corner sits at the origin.
fn normalize_to_origin(rects: &mut [Rect]) {
    let (min_x, min_y) = rects
        .iter()
        .fold((f64::INFINITY, f64::INFINITY), |(mx, my), r| {
            (mx.min(r.x), my.min(r.y))
        });
    if min_x.is_finite() {
        for r in rects {
            r.x -= min_x;
            r.y -= min_y;
        }
    }
}

/// Bounded separation pass: sibling group hulls that overlap are pushed apart
/// by translating every member of the deeper/later group, and any *ungrouped*
/// node that lands inside a hull is pushed out through its nearest edge.
/// Group pairs that share any member — nested (ancestor/descendant) pairs, and
/// siblings that merely intersect — are left alone: containment is expected for
/// the former, and neither can be separated by translating one set without
/// dragging the shared nodes out of the other. Deterministic: groups are always visited deepest-first
/// then by index, and the loop is capped at 6 passes, exiting early once a
/// pass makes no translation.
///
/// Node-node overlap removal runs at the *start* of each pass, before the
/// checks, so a pass that reports "nothing moved" leaves the rects both
/// overlap-free and hull-separated — the post-condition the callers assert.
/// Exhausting the pass cap is best-effort: the last batch of translations never
/// gets its start-of-pass cleanup, so a final `remove_overlaps` runs after the
/// loop when (and only when) it left node rects overlapping. Hull separation is
/// therefore only guaranteed on the converged exit.
/// Only nodes that belong to no group at all are pushed out of foreign hulls:
/// translating a member of some other group in isolation would skew that
/// group's own hull and undo its just-established separation.
fn separate_hulls(rects: &mut [Rect], groups: &[GroupSpec], cfg: &StressConfig) {
    if groups.is_empty() {
        return;
    }
    let mut order: Vec<usize> = (0..groups.len()).collect();
    order.sort_by(|&a, &b| groups[b].depth.cmp(&groups[a].depth).then(a.cmp(&b)));

    let member_sets: Vec<std::collections::BTreeSet<usize>> = groups
        .iter()
        .map(|g| g.members.iter().copied().collect())
        .collect();
    // Only groups with *disjoint* member sets can be pulled apart: translating
    // one of two sets that share a node drags that node out of the other group
    // too, so the pair can never separate and the passes just fight until the
    // cap. Nested (ancestor/descendant) pairs share by definition and are
    // expected to overlap; merely-intersecting siblings (the same element under
    // two `###` headings) are the same story geometrically — the frontend
    // reports those as an `entangled-groups` warning rather than pretending
    // the hulls came out separated.
    let is_entangled =
        |i: usize, j: usize| -> bool { !member_sets[i].is_disjoint(&member_sets[j]) };

    // A node in any group is moved only with its group, never in isolation.
    let ungrouped: Vec<bool> = (0..rects.len())
        .map(|i| !member_sets.iter().any(|s| s.contains(&i)))
        .collect();

    // Node-pair overlap-removal state lives OUTSIDE the pass loop and is never
    // reset: a fresh, amnesiac `remove_overlaps` call every pass is exactly
    // the reactive-greedy scheme whose cross-call cycling caused unbounded
    // hull growth (a group-hull translation nudges a formerly-clear pair back
    // into violation; re-deciding that pair's axis from scratch can then
    // nudge it right back, forever). Keeping the accumulator alive means a
    // node-pair separation decided in an early pass is never re-opened by a
    // later one.
    //
    // Excluded from the accumulator entirely (see `overlap_removal_pass`'s
    // `skip_pair` docs): pairs separated by a DIFFERENT external mover than
    // `overlap_removal_pass` itself. Covering such a pair would merge its two
    // rects into the same rigid VPSC block for the life of this call; when
    // the OTHER mover later repositions just one side (a bulk hull
    // translation, or the ungrouped-node-vs-hull push below), the shared
    // block drags the untouched side right along with it -- an unbounded
    // tug-of-war, observed directly on this exact codepath for both movers:
    //   - cross-group pairs (different, non-overlapping groups): separated by
    //     the bulk group-hull translation loop.
    //   - ungrouped-vs-grouped pairs: the ungrouped side is separated by the
    //     "push clear of every hull" loop below.
    // Same-group pairs and ungrouped-vs-ungrouped pairs have no such external
    // mover (group translation moves a whole group RIGIDLY, preserving
    // within-group relationships) and stay in the accumulator normally.
    let m = rects.len();
    let skip_pair = |i: usize, j: usize| -> bool {
        let gi: Vec<usize> = (0..groups.len())
            .filter(|&g| member_sets[g].contains(&i))
            .collect();
        let gj: Vec<usize> = (0..groups.len())
            .filter(|&g| member_sets[g].contains(&j))
            .collect();
        match (gi.is_empty(), gj.is_empty()) {
            (true, true) => false,                                // both ungrouped
            (true, false) | (false, true) => true,                // ungrouped vs grouped
            (false, false) => !gi.iter().any(|g| gj.contains(g)), // cross-group
        }
    };
    let mut node_overlap = OverlapAccum::new(m);
    let max_overlap_passes = (m * m.saturating_sub(1) / 2).max(4);
    let mut run_overlap_removal = |rects: &mut [Rect]| {
        for _ in 0..max_overlap_passes {
            if !overlap_removal_pass(rects, cfg.gap, &mut node_overlap, skip_pair) {
                break;
            }
        }
    };

    let mut capped = true;
    for _ in 0..6 {
        let mut moved = false;
        run_overlap_removal(rects);

        // Recomputed after every translation below, not once per pass: moving
        // `gj` invalidates its own hull and every ancestor's, so a snapshot taken
        // before the loop would have each pair after the first comparing stale
        // geometry — which is how a pass could report itself settled while hulls
        // it had already shifted still overlapped.
        let mut hulls = group_hulls(rects, groups, cfg);
        for (oi, &gi) in order.iter().enumerate() {
            for &gj in &order[oi + 1..] {
                if is_entangled(gi, gj) {
                    continue;
                }
                let a = &hulls[gi];
                let b = &hulls[gj];
                let ox = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
                let oy = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
                if ox <= 0.0 || oy <= 0.0 {
                    continue;
                }
                moved = true;
                let a_cx = a.x + a.w / 2.0;
                let a_cy = a.y + a.h / 2.0;
                let b_cx = b.x + b.w / 2.0;
                let b_cy = b.y + b.h / 2.0;
                if ox < oy {
                    let dir = if b_cx >= a_cx { 1.0 } else { -1.0 };
                    let delta = dir * (ox + cfg.gap);
                    for &m in &groups[gj].members {
                        if let Some(r) = rects.get_mut(m) {
                            r.x += delta;
                        }
                    }
                } else {
                    let dir = if b_cy >= a_cy { 1.0 } else { -1.0 };
                    let delta = dir * (oy + cfg.gap);
                    for &m in &groups[gj].members {
                        if let Some(r) = rects.get_mut(m) {
                            r.y += delta;
                        }
                    }
                }
                hulls = group_hulls(rects, groups, cfg);
            }
        }

        // Hulls stay valid across this loop: only ungrouped nodes move, and no
        // hull depends on them. Each node is re-tested against every hull until
        // it is clear of all of them (bounded), so being pushed out of one hull
        // and straight into another resolves within the same pass.
        if push_ungrouped_clear(rects, groups, cfg, &ungrouped) {
            moved = true;
        }

        if !moved {
            capped = false;
            break;
        }
    }

    // Exhausting the cap means the loop stopped right after a batch of
    // translations that never got the start-of-pass cleanup. Node rects must be
    // overlap-free regardless, so clean up once more here. (Only on the capped
    // path: a converged run is already overlap-free AND hull-separated.)
    // Both movers run, alternating: `run_overlap_removal` covers within-group
    // and ungrouped-vs-ungrouped pairs (its own accumulator can shift a
    // group's hull), `push_ungrouped_clear` covers ungrouped-vs-grouped —
    // exactly the categories `overlap_removal_pass`'s `skip_pair` leaves for
    // each OTHER mover to finish. Bounded: this is last-mile mop-up after the
    // main loop above already did the real work.
    if capped && any_overlap(rects) {
        for _ in 0..3 {
            run_overlap_removal(rects);
            if !push_ungrouped_clear(rects, groups, cfg, &ungrouped) {
                break;
            }
        }
    }
}

/// Push every ungrouped rect (per `ungrouped`, indexed like `rects`) clear of
/// every group hull it currently overlaps, by `cfg.gap`. Grouped rects and
/// hull positions are untouched — only ungrouped rects move. Each node is
/// re-tested against every hull until clear (bounded by group count), so
/// being pushed out of one hull and straight into another resolves within
/// one call. Returns whether anything moved.
fn push_ungrouped_clear(
    rects: &mut [Rect],
    groups: &[GroupSpec],
    cfg: &StressConfig,
    ungrouped: &[bool],
) -> bool {
    let mut moved = false;
    let hulls = group_hulls(rects, groups, cfg);
    // Generous and escalating, not just `groups.len()` retries at a constant
    // push: clearing hull A can land squarely inside hull B, whose own
    // clearance push can then land right back inside A -- an EXACT ping-pong
    // when two hulls happen to sit `cfg.gap` apart (a real, observed case,
    // not hypothetical). A constant push repeats the same two positions
    // forever; growing the push every retry means no two retries can land on
    // the same spot, so the node is eventually shoved clear of every hull it
    // was caught between instead of oscillating in place until the budget
    // runs out.
    let max_retries = 2 * groups.len().max(1) + 4;
    for (idx, r) in rects.iter_mut().enumerate() {
        if !ungrouped[idx] {
            continue;
        }
        for retry in 0..max_retries {
            let mut hit = false;
            let escalation = 1.0 + retry as f64;
            for h in &hulls {
                let ox = (r.x + r.w).min(h.x + h.w) - r.x.max(h.x);
                let oy = (r.y + r.h).min(h.y + h.h) - r.y.max(h.y);
                if ox <= 0.0 || oy <= 0.0 {
                    continue;
                }
                hit = true;
                moved = true;
                if ox < oy {
                    let dir = if r.x + r.w / 2.0 >= h.x + h.w / 2.0 {
                        1.0
                    } else {
                        -1.0
                    };
                    r.x += dir * (ox + cfg.gap) * escalation;
                } else {
                    let dir = if r.y + r.h / 2.0 >= h.y + h.h / 2.0 {
                        1.0
                    } else {
                        -1.0
                    };
                    r.y += dir * (oy + cfg.gap) * escalation;
                }
            }
            if !hit {
                break;
            }
        }
    }
    moved
}

/// True when any two rects have positive intersection area.
fn any_overlap(rects: &[Rect]) -> bool {
    for i in 0..rects.len() {
        for j in i + 1..rects.len() {
            let a = &rects[i];
            let b = &rects[j];
            if a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h {
                return true;
            }
        }
    }
    false
}

/// Deterministic hill-climb that lowers `segment_crossings` over `rects`
/// without disturbing any group's cohesion. Every candidate move is
/// cohesion-preserving *by construction*:
///
/// 1. Swap two members' centers within the same leaf group (or the virtual
///    "no group" pool of ungrouped nodes) -- neither node changes which
///    group(s) contain it. Pools are additionally split by connected
///    component, so an ungrouped node can never be teleported out of its
///    component's packed region (which would break the disjoint-regions
///    invariant the shelf packing establishes).
/// 2. Swap two whole top-level groups by translating every member of each so
///    their hull centers exchange -- a rigid translation, so every nested
///    group inside moves as one block and keeps its internal shape exactly.
/// 3. Reflect one top-level group's members horizontally or vertically about
///    their hull center -- again rigid, so nested structure is untouched.
///
/// Group-level moves (2)/(3) are restricted to top-level (`depth == 0`)
/// groups: a nested group's members are a subset of its top-level ancestor's,
/// so moving the ancestor already moves it, and moving a nested group in
/// isolation would *not* be cohesion-preserving for the ancestor.
///
/// Candidates are enumerated in a fixed order every sweep; every move that
/// strictly decreases the crossing count is applied immediately and the sweep
/// carries on down the list (it does not restart). On a tie the existing
/// arrangement is kept (never churn). Stops when a full sweep finds no
/// improving move, or after `cfg.crossing_passes` sweeps, whichever comes
/// first.
///
/// `edges` should be real diagram edges only (not the group-clique edges used
/// internally for the stress solve) -- crossings are measured over what is
/// actually drawn.
///
/// Overlap: with top-level groups present, a move that would *increase* the
/// total pairwise top-level hull overlap is rejected (relative, not absolute
/// -- this pass runs before `separate_hulls`, where hulls commonly still
/// overlap), and the caller's `separate_hulls` re-runs `remove_overlaps`. With
/// *no* groups that guard is vacuous and no cleanup follows, so this function
/// runs `remove_overlaps` itself -- but only when it actually accepted a move,
/// keeping the no-op path byte-identical.
///
/// Cost: each candidate is delta-evaluated -- only edges incident to the nodes
/// the move displaces are re-tested (`segment_crossings_touching`) -- and the
/// pool swaps alone are O(n^2) candidates, so the candidate list is capped to
/// a fixed whole-pass evaluation budget (see `PASS_BUDGET`) -- the pass degrades to
/// "improve what fits in the budget" on large diagrams rather than stalling a
/// layout that reruns on every edit.
fn reduce_crossings(
    rects: &mut [Rect],
    groups: &[GroupSpec],
    edges: &[(usize, usize)],
    cfg: &StressConfig,
) {
    if cfg.crossing_passes == 0 || rects.len() < 2 || edges.len() < 2 {
        return;
    }

    // Deepest group each node belongs to (ties broken by smaller group index),
    // for the intra-pool swap move. Nodes in no group map to `None`, the
    // virtual "no group" pool.
    let n = rects.len();
    // Every path below indexes per-node arrays (`centers`, `dims`, `moved`) by
    // group member, so out-of-range members are filtered ONCE here rather than
    // guarded per use -- a `GroupSpec` carrying a stale index must be ignored
    // everywhere consistently, not panic in whichever path forgot the check.
    let members: Vec<Vec<usize>> = groups
        .iter()
        .map(|g| g.members.iter().copied().filter(|&m| m < n).collect())
        .collect();
    let mut leaf_group: Vec<Option<usize>> = vec![None; n];
    for (gi, g) in members.iter().enumerate() {
        for &m in g {
            match leaf_group[m] {
                None => leaf_group[m] = Some(gi),
                Some(cur)
                    if groups[gi].depth > groups[cur].depth
                        || (groups[gi].depth == groups[cur].depth && gi < cur) =>
                {
                    leaf_group[m] = Some(gi);
                }
                _ => {}
            }
        }
    }
    // Connected component of each node under the same merged adjacency the
    // packing used (real edges plus a clique per group), so component ids line
    // up with the shelf-packed regions. Swapping two nodes in different
    // components would move one into the other's region.
    let comp_of: Vec<usize> = {
        let mut merged: Vec<(usize, usize)> = edges.to_vec();
        merged.extend(comembership_depths(groups).keys().copied());
        let adj = adjacency(n, &dedup_edges(n, &merged));
        let mut of = vec![0usize; n];
        for (ci, comp) in components(n, &adj).into_iter().enumerate() {
            for m in comp {
                of[m] = ci;
            }
        }
        of
    };

    let mut pools: HashMap<(Option<usize>, usize), Vec<usize>> = HashMap::new();
    for (node, key) in leaf_group.iter().enumerate() {
        pools.entry((*key, comp_of[node])).or_default().push(node);
    }
    let mut pool_keys: Vec<(Option<usize>, usize)> = pools.keys().copied().collect();
    pool_keys.sort_unstable_by_key(|(g, c)| (g.map(|g| g as isize).unwrap_or(-1), *c));

    // Group-level moves need at least one in-range member to have a hull
    // center at all (an empty member list yields a NaN center), so drop those.
    let top_level: Vec<usize> = (0..groups.len())
        .filter(|&g| groups[g].depth == 0 && !members[g].is_empty())
        .collect();

    // Evaluation budget for the *whole* pass -- all sweeps, not one -- in
    // elementary tests. Each candidate costs one delta objective evaluation
    // (`segment_crossings_touching`: `O(affected_edges * E)` geometric
    // predicates, with `affected_edges` bounded by ~twice the average degree
    // for a node swap) plus the hull guard, which rescans every top-level
    // member and tests every top-level hull pair. The same candidate list is
    // re-swept up to `crossing_passes` times, so the cap divides by that too.
    // Only the O(n^2) pool swaps are capped (group moves are O(g^2) and few),
    // and the truncation is a prefix of the same deterministic enumeration
    // order, so a capped run stays reproducible.
    const PASS_BUDGET: usize = 20_000_000;
    let avg_deg = (2 * edges.len() / n.max(1)).max(1);
    let hull_members: usize = top_level.iter().map(|&g| members[g].len()).sum();
    let per_candidate =
        (2 * avg_deg * edges.len() + hull_members + top_level.len() * top_level.len()).max(1);
    let swap_cap = (PASS_BUDGET / (per_candidate * cfg.crossing_passes.max(1) as usize)).max(1);

    let mut candidates: Vec<Move> = Vec::new();
    'pools: for key in &pool_keys {
        let members = &pools[key];
        for i in 0..members.len() {
            for j in (i + 1)..members.len() {
                if candidates.len() >= swap_cap {
                    break 'pools;
                }
                candidates.push(Move::SwapMembers(members[i], members[j]));
            }
        }
    }
    candidates.extend(group_level_moves(&members, &top_level, &comp_of));
    if candidates.is_empty() {
        return;
    }

    fn hull_center(centers: &[(f64, f64)], members: &[usize]) -> (f64, f64) {
        let (mut min_x, mut min_y) = (f64::INFINITY, f64::INFINITY);
        let (mut max_x, mut max_y) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
        for &m in members {
            let (x, y) = centers[m];
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
        ((min_x + max_x) / 2.0, (min_y + max_y) / 2.0)
    }

    fn apply(centers: &mut [(f64, f64)], members: &[Vec<usize>], mv: Move) {
        match mv {
            Move::SwapMembers(a, b) => centers.swap(a, b),
            Move::SwapGroups(g1, g2) => {
                let c1 = hull_center(centers, &members[g1]);
                let c2 = hull_center(centers, &members[g2]);
                let (dx, dy) = (c2.0 - c1.0, c2.1 - c1.1);
                for &m in &members[g1] {
                    centers[m].0 += dx;
                    centers[m].1 += dy;
                }
                for &m in &members[g2] {
                    centers[m].0 -= dx;
                    centers[m].1 -= dy;
                }
            }
            Move::ReflectGroup(g, horizontal) => {
                let c = hull_center(centers, &members[g]);
                for &m in &members[g] {
                    if horizontal {
                        centers[m].0 = 2.0 * c.0 - centers[m].0;
                    } else {
                        centers[m].1 = 2.0 * c.1 - centers[m].1;
                    }
                }
            }
        }
    }

    // `separate_hulls` afterwards is only a best-effort, capped-pass separator
    // that assumes a reasonably-converged starting layout, so don't lean on it
    // to undo damage this pass does: score a candidate's total top-level hull
    // overlap and reject any move that makes it *worse*. The comparison must
    // be relative, not absolute -- this pass runs BEFORE `separate_hulls`,
    // precisely where hulls routinely still overlap (that is why
    // `separate_hulls` exists), so an absolute "no hulls may overlap" guard
    // rejected every candidate on every such diagram and made the whole pass a
    // silent no-op. Every move type needs the check: group-level moves
    // obviously reposition a hull, but even an intra-group `SwapMembers` can
    // change *that* group's own bbox when the swapped members have different
    // sizes (a large member's half-extent now reaches further at the
    // position a small member used to occupy).
    fn top_level_hull_overlap(
        centers: &[(f64, f64)],
        dims: &[(f64, f64)],
        members: &[Vec<usize>],
        top_level: &[usize],
        hull_pad: f64,
    ) -> f64 {
        // Mirror `group_hulls`' construction exactly (rect extents, not bare
        // centers, then padded): a center-only bbox check would pass two
        // groups that are close but not touching, whose *padded, rect-extent*
        // hulls (what `separate_hulls`/rendering actually use) do overlap.
        let bbox = |g: usize| -> (f64, f64, f64, f64) {
            let (mut min_x, mut min_y) = (f64::INFINITY, f64::INFINITY);
            let (mut max_x, mut max_y) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
            for &m in &members[g] {
                let (x, y) = centers[m];
                let (w, h) = dims[m];
                min_x = min_x.min(x - w / 2.0);
                min_y = min_y.min(y - h / 2.0);
                max_x = max_x.max(x + w / 2.0);
                max_y = max_y.max(y + h / 2.0);
            }
            (
                min_x - hull_pad,
                min_y - hull_pad,
                max_x + hull_pad,
                max_y + hull_pad,
            )
        };
        // Summed pairwise intersection area: a scalar that is 0 iff no two
        // top-level hulls touch, and that shrinks monotonically as they pull
        // apart, so "did this move worsen hull overlap?" has an answer even
        // when the starting layout is already tangled.
        let mut area = 0.0;
        for i in 0..top_level.len() {
            let (a_min_x, a_min_y, a_max_x, a_max_y) = bbox(top_level[i]);
            for &gj in &top_level[i + 1..] {
                let (b_min_x, b_min_y, b_max_x, b_max_y) = bbox(gj);
                let w = a_max_x.min(b_max_x) - a_min_x.max(b_min_x);
                let h = a_max_y.min(b_max_y) - a_min_y.max(b_min_y);
                if w > 0.0 && h > 0.0 {
                    area += w * h;
                }
            }
        }
        area
    }

    let dims: Vec<(f64, f64)> = rects.iter().map(|r| (r.w, r.h)).collect();
    let mut centers: Vec<(f64, f64)> = rects
        .iter()
        .map(|r| (r.x + r.w / 2.0, r.y + r.h / 2.0))
        .collect();
    // Which nodes a move displaces -- the delta objective only has to re-test
    // edges incident to these.
    fn moved_nodes(n: usize, members: &[Vec<usize>], mv: Move) -> Vec<bool> {
        let mut moved = vec![false; n];
        match mv {
            Move::SwapMembers(a, b) => {
                moved[a] = true;
                moved[b] = true;
            }
            Move::SwapGroups(g1, g2) => {
                for &m in members[g1].iter().chain(members[g2].iter()) {
                    moved[m] = true;
                }
            }
            Move::ReflectGroup(g, _) => {
                for &m in &members[g] {
                    moved[m] = true;
                }
            }
        }
        moved
    }

    // Hull-overlap areas are px^2 on layouts hundreds of px across; this only
    // has to absorb float noise from re-deriving an unchanged bbox.
    const OVERLAP_EPS: f64 = 1e-6;

    let mut current_overlap =
        top_level_hull_overlap(&centers, &dims, &members, &top_level, cfg.hull_pad);
    let mut improved_any = false;
    let mut trial = centers.clone();
    // Each sweep applies *every* improving move it finds rather than
    // restarting on the first one, and stops the climb as soon as a whole
    // sweep improves nothing.
    for _ in 0..cfg.crossing_passes {
        let mut improved = false;
        for &mv in &candidates {
            trial.copy_from_slice(&centers);
            apply(&mut trial, &members, mv);
            let overlap = top_level_hull_overlap(&trial, &dims, &members, &top_level, cfg.hull_pad);
            if overlap > current_overlap + OVERLAP_EPS {
                continue;
            }
            let moved = moved_nodes(n, &members, mv);
            let before = segment_crossings_touching(&centers, edges, &moved);
            let after = segment_crossings_touching(&trial, edges, &moved);
            if after < before {
                centers.copy_from_slice(&trial);
                current_overlap = overlap;
                improved = true;
                improved_any = true;
            }
        }
        if !improved {
            break;
        }
    }

    if !improved_any {
        return;
    }
    for (i, r) in rects.iter_mut().enumerate() {
        r.x = centers[i].0 - r.w / 2.0;
        r.y = centers[i].1 - r.h / 2.0;
    }
    // With no groups the hull guard above is vacuous and no later pass cleans
    // up: `separate_hulls` returns immediately on an empty `groups`. A
    // `SwapMembers` of two differently-sized rects can leave node rects
    // overlapping, so clear that here.
    if groups.is_empty() {
        remove_overlaps(rects, cfg.gap);
    }
}

/// One candidate rearrangement in `reduce_crossings`' hill-climb. Every
/// variant is cohesion-preserving by construction: it permutes positions
/// inside one pool, or moves a whole group rigidly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Move {
    SwapMembers(usize, usize),
    SwapGroups(usize, usize),
    ReflectGroup(usize, bool), // true = horizontal (flip x), false = vertical (flip y)
}

/// The group-level half of `reduce_crossings`' candidate list: swap any two
/// top-level groups, or mirror one about its own hull center.
///
/// Both move EVERY member of a group at once, which is cohesion-preserving
/// only under two conditions, enforced here:
///
/// * **Disjoint membership.** On entangled siblings (the same element under
///   two headings -- the case `separate_hulls` handles explicitly) a shared
///   member would take group A's offset and then group B's opposite one and
///   stay put while both hulls translate around it, or be mirrored out of the
///   sibling it also belongs to. Either way both hulls scramble, so an
///   entangled group gets no group-level moves at all; its members still move
///   via the intra-pool swaps.
/// * **Same connected component.** A group's members are one component by
///   construction (the packing merges a clique per group into the adjacency),
///   so a group has a single component id. Swapping across components would
///   teleport each group into the other's shelf-packed region -- the same
///   invariant the pool swaps guard.
fn group_level_moves(members: &[Vec<usize>], top_level: &[usize], comp_of: &[usize]) -> Vec<Move> {
    let member_sets: Vec<HashSet<usize>> = top_level
        .iter()
        .map(|&g| members[g].iter().copied().collect())
        .collect();
    let entangled: Vec<bool> = (0..top_level.len())
        .map(|i| {
            (0..top_level.len()).any(|j| j != i && !member_sets[i].is_disjoint(&member_sets[j]))
        })
        .collect();
    let group_comp: Vec<usize> = top_level.iter().map(|&g| comp_of[members[g][0]]).collect();

    let mut moves = Vec::new();
    for i in 0..top_level.len() {
        for j in (i + 1)..top_level.len() {
            if entangled[i] || entangled[j] || group_comp[i] != group_comp[j] {
                continue;
            }
            moves.push(Move::SwapGroups(top_level[i], top_level[j]));
        }
    }
    for (i, &g) in top_level.iter().enumerate() {
        if entangled[i] {
            continue;
        }
        moves.push(Move::ReflectGroup(g, true));
        moves.push(Move::ReflectGroup(g, false));
    }
    moves
}

/// For every pair of co-members across `groups`, the depth of the *deepest*
/// group that contains both (0 = top level). Pairs never sharing a group are
/// absent. Keyed with the smaller index first. Groups are small, so the O(k^2)
/// clique per group is fine.
fn comembership_depths(groups: &[GroupSpec]) -> HashMap<(usize, usize), u8> {
    let mut depths: HashMap<(usize, usize), u8> = HashMap::new();
    for g in groups {
        for (i, &a) in g.members.iter().enumerate() {
            for &b in &g.members[i + 1..] {
                if a == b {
                    continue;
                }
                let key = if a < b { (a, b) } else { (b, a) };
                let entry = depths.entry(key).or_insert(g.depth);
                *entry = (*entry).max(g.depth);
            }
        }
    }
    depths
}

/// Bounding box of each group's member rects, grown by `cfg.hull_pad`. A
/// group with no members (all keys missing from `sizes`, or genuinely empty)
/// gets a degenerate zero-size hull at the origin.
fn group_hulls(rects: &[Rect], groups: &[GroupSpec], cfg: &StressConfig) -> Vec<Rect> {
    groups
        .iter()
        .map(|g| {
            let mut min_x = f64::INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            for &m in &g.members {
                let Some(r) = rects.get(m) else { continue };
                min_x = min_x.min(r.x);
                min_y = min_y.min(r.y);
                max_x = max_x.max(r.x + r.w);
                max_y = max_y.max(r.y + r.h);
            }
            if !min_x.is_finite() {
                return Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 0.0,
                    h: 0.0,
                };
            }
            Rect {
                x: min_x - cfg.hull_pad,
                y: min_y - cfg.hull_pad,
                w: (max_x - min_x) + 2.0 * cfg.hull_pad,
                h: (max_y - min_y) + 2.0 * cfg.hull_pad,
            }
        })
        .collect()
}

// --- helpers -------------------------------------------------------------

/// Drop self-edges, dedup (undirected), and clamp indices in range. Output is
/// sorted for a fully deterministic downstream ordering.
fn dedup_edges(n: usize, edges: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut seen: Vec<(usize, usize)> = edges
        .iter()
        .filter(|&&(a, b)| a != b && a < n && b < n)
        .map(|&(a, b)| if a < b { (a, b) } else { (b, a) })
        .collect();
    seen.sort_unstable();
    seen.dedup();
    seen
}

/// Undirected adjacency lists, each sorted ascending.
fn adjacency(n: usize, edges: &[(usize, usize)]) -> Vec<Vec<usize>> {
    let mut adj = vec![Vec::new(); n];
    for &(a, b) in edges {
        adj[a].push(b);
        adj[b].push(a);
    }
    for a in &mut adj {
        a.sort_unstable();
        a.dedup();
    }
    adj
}

/// Connected components, each sorted ascending, ordered by smallest member.
fn components(n: usize, adj: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut seen = vec![false; n];
    let mut comps = Vec::new();
    for start in 0..n {
        if seen[start] {
            continue;
        }
        let mut comp = Vec::new();
        let mut queue = VecDeque::from([start]);
        seen[start] = true;
        while let Some(node) = queue.pop_front() {
            comp.push(node);
            for &nb in &adj[node] {
                if !seen[nb] {
                    seen[nb] = true;
                    queue.push_back(nb);
                }
            }
        }
        comp.sort_unstable();
        comps.push(comp);
    }
    // `start` scans ascending so comps already order by smallest member.
    comps
}

/// BFS hop counts from `src` over the whole graph; `None` for unreachable.
fn bfs_hops(n: usize, adj: &[Vec<usize>], src: usize) -> Vec<Option<u32>> {
    let mut dist = vec![None; n];
    dist[src] = Some(0);
    let mut queue = VecDeque::from([src]);
    while let Some(node) = queue.pop_front() {
        let d = dist[node].unwrap();
        for &nb in &adj[node] {
            if dist[nb].is_none() {
                dist[nb] = Some(d + 1);
                queue.push_back(nb);
            }
        }
    }
    dist
}

/// Deterministic circular seed: node `k` of `m` at angle `2*PI*k/m`, radius set
/// so the ring circumference is about `edge_len * m`.
fn circular_seed(m: usize, edge_len: f64) -> Vec<(f64, f64)> {
    let radius = (edge_len * m as f64 / (2.0 * PI)).max(edge_len);
    (0..m)
        .map(|k| {
            let theta = 2.0 * PI * k as f64 / m as f64;
            (radius * theta.cos(), radius * theta.sin())
        })
        .collect()
}

/// Weighted raw stress: sum over a<b of w_ab * (d_ab - dist(p_a, p_b))^2.
fn stress_value(pos: &[(f64, f64)], dist: &[Vec<f64>], w: &[Vec<f64>]) -> f64 {
    let m = pos.len();
    let mut s = 0.0;
    for a in 0..m {
        for b in (a + 1)..m {
            let dx = pos[a].0 - pos[b].0;
            let dy = pos[a].1 - pos[b].1;
            let actual = dx.hypot(dy);
            let e = dist[a][b] - actual;
            s += w[a][b] * e * e;
        }
    }
    s
}

/// Run the Guttman-transform majorization to convergence. Returns the final
/// positions and the per-iteration stress trace (trace[0] = seed stress).
fn majorize(
    seed: &[(f64, f64)],
    dist: &[Vec<f64>],
    w: &[Vec<f64>],
    wsum: &[f64],
    cfg: &StressConfig,
) -> (Vec<(f64, f64)>, Vec<f64>) {
    let m = seed.len();
    let mut pos = seed.to_vec();
    let mut trace = vec![stress_value(&pos, dist, w)];
    for _ in 0..cfg.max_iter {
        // Simultaneous (Jacobi) update — the standard SMACOF majorizer; stress
        // is guaranteed non-increasing.
        let mut next = vec![(0.0, 0.0); m];
        for a in 0..m {
            if wsum[a] <= 0.0 {
                next[a] = pos[a];
                continue;
            }
            let (mut sx, mut sy) = (0.0, 0.0);
            for b in 0..m {
                if b == a {
                    continue;
                }
                let dx = pos[a].0 - pos[b].0;
                let dy = pos[a].1 - pos[b].1;
                let actual = dx.hypot(dy);
                let inv = if actual < COINCIDENT_EPS {
                    0.0
                } else {
                    dist[a][b] / actual
                };
                sx += w[a][b] * (pos[b].0 + inv * dx);
                sy += w[a][b] * (pos[b].1 + inv * dy);
            }
            next[a] = (sx / wsum[a], sy / wsum[a]);
        }
        pos = next;
        let s = stress_value(&pos, dist, w);
        let prev = *trace.last().unwrap();
        trace.push(s);
        if (prev - s).abs() < cfg.epsilon {
            break;
        }
    }
    (pos, trace)
}

/// Solve one connected component to node-centered `Rect`s (in `comp` order).
/// `co_depth` maps a co-member pair (global indices, smaller first) to the
/// depth of its deepest common group, for the cohesion override below.
fn component_layout(
    comp: &[usize],
    sizes: &[Size],
    adj: &[Vec<usize>],
    cfg: &StressConfig,
    co_depth: &HashMap<(usize, usize), u8>,
) -> Vec<Rect> {
    let m = comp.len();
    if m == 1 {
        let s = sizes[comp[0]];
        return vec![Rect {
            x: -s.w / 2.0,
            y: -s.h / 2.0,
            w: s.w,
            h: s.h,
        }];
    }

    // Target distances: hops * edge_len, inflated by combined half-extents so
    // boxes have room for their footprints. Co-member pairs are then pulled in
    // to `group_len` (whichever is tighter) — cohesion is soft: an edge-adjacent
    // pair that is also co-member keeps the tighter of the two distances.
    // Weights w = 1 / d^2, with co-member pairs weighted up by
    // `group_weight^(depth+1)` using the deepest group containing both.
    let n = adj.len();
    let mut dist = vec![vec![0.0; m]; m];
    let mut w = vec![vec![0.0; m]; m];
    for (la, &ga) in comp.iter().enumerate() {
        let hops = bfs_hops(n, adj, ga);
        for (lb, &gb) in comp.iter().enumerate() {
            if la == lb {
                continue;
            }
            let h = hops[gb].expect("connected component is fully reachable") as f64;
            let mut d = h * cfg.edge_len + half_extent(&sizes[ga]) + half_extent(&sizes[gb]);
            let key = if ga < gb { (ga, gb) } else { (gb, ga) };
            let depth = co_depth.get(&key).copied();
            if depth.is_some() {
                let group_d = cfg.group_len + half_extent(&sizes[ga]) + half_extent(&sizes[gb]);
                d = d.min(group_d);
            }
            dist[la][lb] = d;
            let mut weight = 1.0 / (d * d);
            if let Some(depth) = depth {
                weight *= cfg.group_weight.powi(depth as i32 + 1);
            }
            w[la][lb] = weight;
        }
    }
    let wsum: Vec<f64> = (0..m).map(|a| w[a].iter().sum()).collect();

    let seed = circular_seed(m, cfg.edge_len);
    let (pos, _trace) = majorize(&seed, &dist, &w, &wsum, cfg);

    // Centers → top-left rects.
    comp.iter()
        .enumerate()
        .map(|(local, &g)| {
            let s = sizes[g];
            Rect {
                x: pos[local].0 - s.w / 2.0,
                y: pos[local].1 - s.h / 2.0,
                w: s.w,
                h: s.h,
            }
        })
        .collect()
}

/// Minimal-displacement overlap removal (Dwyer-Marriott-Stuckey GD 2005,
/// simplified): each overlapping pair contributes one separation constraint
/// on the axis that needs the smaller move; both axes then solve exactly via
/// `vpsc::project`.
///
/// The per-pair axis choice is made ONCE, the first time a pair is found
/// overlapping, and never revisited: a pair whose Sep has already been
/// generated is never re-examined, so its constraint always stays in the set
/// handed to `project`. This matters because re-deciding "cheapest axis"
/// fresh every iteration can cycle forever on chained/contended triples (A
/// pushed off B re-violates A-vs-C; fixing A-vs-C re-violates A-vs-B; ...),
/// which is a real, observed non-termination, not merely a slow-convergence
/// worry -- a purely reactive greedy scheme drifts the whole chain sideways
/// indefinitely instead of settling. Accumulating every discovered Sep and
/// re-projecting the FULL set each pass instead lets `vpsc::project`'s block
/// merge resolve the whole chain simultaneously and correctly, and previously
/// satisfied Seps are never un-satisfied by a later pass (`project`
/// guarantees every live Sep it is given holds on return). Bounded: every
/// pass that makes progress covers at least one previously-uncovered pair, so
/// the loop cannot run longer than there are pairs (`m*(m-1)/2`); in practice
/// 2-3 passes surface every violated pair and the cap only guards
/// pathological input.
fn remove_overlaps(rects: &mut [Rect], gap: f64) {
    let m = rects.len();
    if m < 2 {
        return;
    }
    let mut st = OverlapAccum::new(m);
    let max_passes = (m * m.saturating_sub(1) / 2).max(4);
    for _ in 0..max_passes {
        if !overlap_removal_pass(rects, gap, &mut st, |_, _| false) {
            return;
        }
    }
}

/// Accumulator threaded through repeated `overlap_removal_pass` calls.
/// Tracks, per accumulated Sep, which rect pair it came from (`xsep_pairs`/
/// `ysep_pairs`, parallel to `xsep`/`ysep`), so a Sep that `vpsc::project`
/// drops as unsatisfiable can be un-covered and retried against fresh
/// geometry on a later pass instead of leaving that pair's gap permanently
/// unenforced (a real, observed failure mode: an accumulated Sep can become
/// contradictory once later Seps are layered on top of it).
#[derive(Default)]
struct OverlapAccum {
    xsep: Vec<super::vpsc::Sep>,
    xsep_pairs: Vec<(usize, usize)>,
    ysep: Vec<super::vpsc::Sep>,
    ysep_pairs: Vec<(usize, usize)>,
    /// covered[i][j] (i < j): this pair's axis has been decided and its Sep
    /// already lives in `xsep`/`ysep` — never re-examined while `true`.
    covered: Vec<Vec<bool>>,
    /// banned_axes[i][j] (i < j): bitset (`AXIS_X`/`AXIS_Y`) of axes whose
    /// generated Sep for this pair `vpsc::project` dropped as unsatisfiable.
    /// Re-discovery must not re-decide a banned axis: with an authored
    /// equality pinning the pair on its "cheaper" axis, re-deciding would
    /// regenerate the same contradicted Sep every pass until the pass cap,
    /// leaving the pair overlapping forever (see `remove_overlaps_with`).
    /// Instead the OTHER axis is committed; with both axes banned the pair
    /// is abandoned (marked covered with no Sep) so the loop terminates.
    banned_axes: Vec<Vec<u8>>,
}

/// `OverlapAccum::banned_axes` bits.
const AXIS_X: u8 = 1;
const AXIS_Y: u8 = 2;

impl OverlapAccum {
    fn new(m: usize) -> Self {
        OverlapAccum {
            covered: vec![vec![false; m]; m],
            banned_axes: vec![vec![0; m]; m],
            ..Default::default()
        }
    }
}

/// One discovery-then-project step of minimal-displacement overlap removal:
/// scan every not-yet-covered, not-`skip_pair`-filtered pair, commit any
/// newly-overlapping pair's Sep on its cheaper axis (marking it covered so
/// it is never re-decided — re-deciding "cheapest axis" fresh every call is
/// what cycles forever on chained/contended triples, see
/// `remove_overlaps`), then UNCONDITIONALLY re-project both accumulated axis
/// lists — even when no new pair was found this call. That unconditional
/// re-projection matters for `separate_hulls`'s persistent accumulator: a
/// group-hull translation between two calls can shove a rect that already
/// has a covered, previously-satisfied Sep; since that pair is never
/// re-examined, only re-running `project` on the existing set — not
/// re-discovery — can restore it. Any Sep `project` drops as unsatisfiable
/// is un-covered and removed from the accumulator so its pair gets a fresh
/// look (and possibly a different axis) on a later pass.
///
/// `skip_pair(i, j)`: pairs this returns `true` for are left alone forever
/// (never covered, never given a Sep) — `separate_hulls` uses this to keep
/// cross-group pairs OUT of the accumulator entirely, because covering one
/// merges the two rects into the same rigid VPSC block for the life of the
/// call; a later bulk hull-translation moving one group then drags the
/// OTHER group's member right along with it through that shared block,
/// which is exactly the unbounded tug-of-war this accumulator exists to
/// avoid. Cross-group separation is the hull-translation loop's job, not
/// this one's. `remove_overlaps` (no group concept) passes `|_, _| false`.
///
/// Returns whether any NEW pair was found (callers use this only to decide
/// whether another discovery pass is worthwhile, not whether projection
/// ran).
fn overlap_removal_pass(
    rects: &mut [Rect],
    gap: f64,
    st: &mut OverlapAccum,
    skip_pair: impl Fn(usize, usize) -> bool,
) -> bool {
    use super::vpsc::project;
    let added_any = overlap_discover(rects, gap, st, skip_pair);
    let m = rects.len();
    let w = vec![1.0; m];
    let mut xs: Vec<f64> = rects.iter().map(|r| r.x).collect();
    let dropped_x = project(&mut xs, &w, &st.xsep);
    for (r, x) in rects.iter_mut().zip(&xs) {
        r.x = *x;
    }
    // Highest index first: swap_remove would otherwise invalidate later
    // indices still pending removal.
    for &di in dropped_x.iter().rev() {
        let (i, j) = st.xsep_pairs[di];
        st.covered[i][j] = false;
        st.xsep.swap_remove(di);
        st.xsep_pairs.swap_remove(di);
    }
    let mut ys: Vec<f64> = rects.iter().map(|r| r.y).collect();
    let dropped_y = project(&mut ys, &w, &st.ysep);
    for (r, y) in rects.iter_mut().zip(&ys) {
        r.y = *y;
    }
    for &di in dropped_y.iter().rev() {
        let (i, j) = st.ysep_pairs[di];
        st.covered[i][j] = false;
        st.ysep.swap_remove(di);
        st.ysep_pairs.swap_remove(di);
    }
    added_any
}

/// Discovery-only half of `overlap_removal_pass`: scan every not-yet-
/// `covered`, not-`skip_pair`-filtered rect pair and commit any
/// newly-overlapping pair's Sep on its cheaper axis into `st` (marking it
/// covered). Does NOT project — extracted so `remove_overlaps_with` can
/// combine the discovered seps with authored ones in a single
/// `vpsc::project` call per axis instead of projecting twice per pass.
/// Returns whether any new pair was found.
fn overlap_discover(
    rects: &[Rect],
    gap: f64,
    st: &mut OverlapAccum,
    skip_pair: impl Fn(usize, usize) -> bool,
) -> bool {
    use super::vpsc::Sep;
    let m = rects.len();
    let mut added_any = false;
    for i in 0..m {
        for j in (i + 1)..m {
            if st.covered[i][j] || skip_pair(i, j) {
                continue;
            }
            let (a, b) = (&rects[i], &rects[j]);
            let ox = (a.x + a.w + gap).min(b.x + b.w + gap) - a.x.max(b.x);
            let oy = (a.y + a.h + gap).min(b.y + b.h + gap) - a.y.max(b.y);
            if ox <= 0.0 || oy <= 0.0 {
                continue; // clear (with gap) on at least one axis
            }
            st.covered[i][j] = true;
            let banned = st.banned_axes[i][j];
            if banned & AXIS_X != 0 && banned & AXIS_Y != 0 {
                // Both axes already proved unsatisfiable for this pair (e.g.
                // authored equalities pin it on both): abandon it — no Sep,
                // no `added_any` — instead of livelocking on re-discovery.
                continue;
            }
            added_any = true;
            // Resolve on the axis with the smaller required move, unless that
            // axis was already dropped as unsatisfiable for this pair.
            let use_x = if banned & AXIS_X != 0 {
                false
            } else if banned & AXIS_Y != 0 {
                true
            } else {
                ox <= oy
            };
            if use_x {
                let (l, r) = if a.x + a.w / 2.0 <= b.x + b.w / 2.0 {
                    (i, j)
                } else {
                    (j, i)
                };
                st.xsep.push(Sep {
                    left: l,
                    right: r,
                    gap: rects[l].w + gap,
                    equality: false,
                });
                st.xsep_pairs.push((i, j));
            } else {
                let (t, u) = if a.y + a.h / 2.0 <= b.y + b.h / 2.0 {
                    (i, j)
                } else {
                    (j, i)
                };
                st.ysep.push(Sep {
                    left: t,
                    right: u,
                    gap: rects[t].h + gap,
                    equality: false,
                });
                st.ysep_pairs.push((i, j));
            }
        }
    }
    added_any
}

/// Overlap removal that folds authored separation constraints in so the
/// per-pair minimal-displacement passes can never undo them: `extra_x`/
/// `extra_y` (already-compiled `constrain.rs` seps, or `layout_constrained`'s
/// own authored seps) are placed FIRST in the per-axis list handed to
/// `vpsc::project` every pass, ahead of the seps `overlap_discover` finds —
/// a dropped index `< extra_x.len()` (resp. `extra_y.len()`) therefore
/// always names an authored sep, never a generated overlap sep (those
/// regenerate on the next pass and are never reported).
///
/// The position vector is `rects.len() + extra_vars` long: indices
/// `0..rects.len()` are the node rects, `rects.len()..` are extra
/// positionless variables (group boundary vars) carried at low weight
/// (`0.01`) so they follow their members instead of dragging them; they
/// take no part in overlap discovery (they have no rect) but do get
/// projected by any authored sep that references them. Returns
/// `(dropped_x, dropped_y)` — indices into `extra_x`/`extra_y` that
/// `vpsc::project` ultimately could not satisfy against the final rects.
fn remove_overlaps_with(
    rects: &mut [Rect],
    gap: f64,
    extra_x: &[super::vpsc::Sep],
    extra_y: &[super::vpsc::Sep],
    extra_vars: usize,
) -> (Vec<usize>, Vec<usize>) {
    use super::vpsc::project;
    let m = rects.len();
    let total = m + extra_vars;
    let mut st = OverlapAccum::new(m);
    let mut ex_x = vec![0.0_f64; extra_vars];
    let mut ex_y = vec![0.0_f64; extra_vars];
    let mut weights = vec![1.0_f64; m];
    weights.resize(total, 0.01);
    let max_passes = (m * m.saturating_sub(1) / 2).max(4) + 1;
    let (mut dropped_x, mut dropped_y) = (Vec::new(), Vec::new());
    for _ in 0..max_passes {
        let discovered = if m >= 2 {
            overlap_discover(rects, gap, &mut st, |_, _| false)
        } else {
            false
        };

        let mut xs: Vec<f64> = rects
            .iter()
            .map(|r| r.x)
            .chain(ex_x.iter().copied())
            .collect();
        let mut xseps: Vec<super::vpsc::Sep> = extra_x.to_vec();
        let extra_x_len = xseps.len();
        xseps.extend(st.xsep.iter().copied());
        let dx = project(&mut xs, &weights, &xseps);
        for (r, x) in rects.iter_mut().zip(xs.iter().take(m)) {
            r.x = *x;
        }
        ex_x.copy_from_slice(&xs[m..]);
        // Highest index first: swap_remove would otherwise invalidate later
        // indices still pending removal.
        for &di in dx.iter().rev() {
            if di >= extra_x_len {
                let gi = di - extra_x_len;
                let (i, j) = st.xsep_pairs[gi];
                st.covered[i][j] = false;
                // An authored sep (equality) contradicted this generated
                // x-sep; re-discovery must take the y axis instead of
                // re-committing the same doomed choice forever.
                st.banned_axes[i][j] |= AXIS_X;
                st.xsep.swap_remove(gi);
                st.xsep_pairs.swap_remove(gi);
            }
        }
        dropped_x = dx.into_iter().filter(|&di| di < extra_x_len).collect();

        let mut ys: Vec<f64> = rects
            .iter()
            .map(|r| r.y)
            .chain(ex_y.iter().copied())
            .collect();
        let mut yseps: Vec<super::vpsc::Sep> = extra_y.to_vec();
        let extra_y_len = yseps.len();
        yseps.extend(st.ysep.iter().copied());
        let dy = project(&mut ys, &weights, &yseps);
        for (r, y) in rects.iter_mut().zip(ys.iter().take(m)) {
            r.y = *y;
        }
        ex_y.copy_from_slice(&ys[m..]);
        for &di in dy.iter().rev() {
            if di >= extra_y_len {
                let gi = di - extra_y_len;
                let (i, j) = st.ysep_pairs[gi];
                st.covered[i][j] = false;
                st.banned_axes[i][j] |= AXIS_Y;
                st.ysep.swap_remove(gi);
                st.ysep_pairs.swap_remove(gi);
            }
        }
        dropped_y = dy.into_iter().filter(|&di| di < extra_y_len).collect();

        if !discovered {
            break;
        }
    }
    (dropped_x, dropped_y)
}

/// Edgeless fallback: wrap the flat node list into a `ceil(sqrt(n))`-column
/// grid. Column widths are per-column maxima, row heights per-row maxima, with
/// `gap` between cells; each box is centered in its cell. Min corner at origin.
pub fn grid_pack(ids: &[BoxId], sizes: &[Size], cfg: &StressConfig) -> Vec<Rect> {
    let n = ids.len();
    if n == 0 {
        return vec![];
    }
    let cols = (n as f64).sqrt().ceil() as usize;
    let rows = n.div_ceil(cols);

    let mut col_w = vec![0.0_f64; cols];
    let mut row_h = vec![0.0_f64; rows];
    for (k, s) in sizes.iter().enumerate() {
        let (r, c) = (k / cols, k % cols);
        col_w[c] = col_w[c].max(s.w);
        row_h[r] = row_h[r].max(s.h);
    }

    // Cell origins from prefix sums plus inter-cell gaps.
    let mut col_x = vec![0.0_f64; cols];
    for c in 1..cols {
        col_x[c] = col_x[c - 1] + col_w[c - 1] + cfg.gap;
    }
    let mut row_y = vec![0.0_f64; rows];
    for r in 1..rows {
        row_y[r] = row_y[r - 1] + row_h[r - 1] + cfg.gap;
    }

    (0..n)
        .map(|k| {
            let (r, c) = (k / cols, k % cols);
            let s = sizes[k];
            Rect {
                x: col_x[c] + (col_w[c] - s.w) / 2.0,
                y: row_y[r] + (row_h[r] - s.h) / 2.0,
                w: s.w,
                h: s.h,
            }
        })
        .collect()
}

/// Deterministic, `solve::pretty`-style dump: one `node <id> @ x,y wxh` line per
/// box, sorted by id. Used by tests and the harness.
pub fn pretty(ids: &[BoxId], rects: &[Rect]) -> String {
    let mut pairs: Vec<(&BoxId, &Rect)> = ids.iter().zip(rects.iter()).collect();
    pairs.sort_by(|a, b| a.0.cmp(b.0));
    let mut out = String::new();
    for (id, r) in pairs {
        let name = match id {
            BoxId::Node(k) => k.clone(),
            BoxId::Group(g) => format!("group{g}"),
            BoxId::Inline(i) => format!("inline{i}"),
        };
        out.push_str(&format!(
            "node {name} @ {:.0},{:.0} {:.0}x{:.0}\n",
            r.x, r.y, r.w, r.h
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::crossing::segment_crossings;
    use super::*;

    fn node(k: &str) -> BoxId {
        BoxId::Node(k.into())
    }
    fn ids(keys: &[&str]) -> Vec<BoxId> {
        keys.iter().map(|k| node(k)).collect()
    }
    fn sizes(n: usize, w: f64, h: f64) -> Vec<Size> {
        vec![Size { w, h }; n]
    }
    fn overlaps(a: &Rect, b: &Rect) -> bool {
        a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
    }

    #[test]
    fn layout_constrained_empty_seps_matches_layout_grouped() {
        let ids = ids(&["a", "b", "c"]);
        let sz = sizes(3, 100.0, 50.0);
        let edges = vec![(0usize, 1usize), (1, 2)];
        let cfg = StressConfig::default();
        let (r1, h1) = layout_grouped(&ids, &sz, &edges, &[], &cfg);
        let (r2, h2, dropped) =
            layout_constrained(&ids, &sz, &edges, &[], &SepSpecs::default(), &cfg);
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
            y: vec![Sep {
                left: 0,
                right: 1,
                gap: 50.0 + 40.0,
                equality: false,
            }],
            ..SepSpecs::default()
        };
        let (rects, _, dropped) =
            layout_constrained(&ids, &sz, &[(0, 1)], &[], &seps, &StressConfig::default());
        assert!(dropped.0.is_empty() && dropped.1.is_empty());
        assert!(
            rects[0].y + rects[0].h + 40.0 <= rects[1].y + 1e-6,
            "a must sit above b: {:?}",
            rects
        );
    }

    #[test]
    fn layout_constrained_enforces_an_alignment_equality() {
        use crate::solve::vpsc::Sep;
        // Align left edges: x_a == x_b (equality sep, gap 0).
        let ids = ids(&["a", "b"]);
        let sz = sizes(2, 100.0, 50.0);
        let seps = SepSpecs {
            x: vec![Sep {
                left: 0,
                right: 1,
                gap: 0.0,
                equality: true,
            }],
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
    fn layout_constrained_x_equality_on_tall_nodes_separates_on_y() {
        use crate::solve::vpsc::Sep;
        // 50x120 cards are taller than wide, so overlap removal's "cheaper
        // axis" for an overlapping pair is X — exactly the axis the authored
        // equality pins. Re-discovery must flip the pair to Y instead of
        // re-committing the same contradicted X sep every pass until the
        // pass cap (the observed livelock: both rects returned at the
        // identical position, nothing reported).
        let idv = ids(&["a", "b"]);
        let sz = sizes(2, 50.0, 120.0);
        let seps = SepSpecs {
            x: vec![Sep {
                left: 0,
                right: 1,
                gap: 0.0,
                equality: true,
            }],
            ..SepSpecs::default()
        };
        // Both the edgeless grid arm and the connected stress arm.
        for edges in [vec![], vec![(0usize, 1usize)]] {
            let (rects, _, dropped) =
                layout_constrained(&idv, &sz, &edges, &[], &seps, &StressConfig::default());
            assert!(
                dropped.0.is_empty() && dropped.1.is_empty(),
                "authored equality must not be reported dropped: {:?}",
                dropped
            );
            assert!(
                (rects[0].x - rects[1].x).abs() < 1e-6,
                "x-equality must hold: {:?}",
                rects
            );
            assert!(
                !overlaps(&rects[0], &rects[1]),
                "pair must be separated (on Y): {:?}",
                rects
            );
        }
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
            x: vec![Sep {
                left: 3,
                right: 0,
                gap: 100.0 + 40.0,
                equality: false,
            }],
            ..SepSpecs::default()
        };
        let (rects, _, dropped) =
            layout_constrained(&ids, &sz, &edges, &[], &seps, &StressConfig::default());
        assert!(dropped.0.is_empty());
        assert!(
            rects[3].x + 100.0 + 40.0 <= rects[0].x + 1e-6,
            "d left of a"
        );
    }

    #[test]
    fn layout_constrained_reports_contradictory_seps() {
        use crate::solve::vpsc::Sep;
        let ids = ids(&["a", "b"]);
        let sz = sizes(2, 100.0, 50.0);
        let seps = SepSpecs {
            x: vec![
                Sep {
                    left: 0,
                    right: 1,
                    gap: 140.0,
                    equality: false,
                },
                Sep {
                    left: 1,
                    right: 0,
                    gap: 140.0,
                    equality: false,
                },
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
            x: vec![Sep {
                left: 0,
                right: 2,
                gap: 140.0,
                equality: false,
            }],
            y: vec![Sep {
                left: 1,
                right: 3,
                gap: 90.0,
                equality: false,
            }],
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
        let groups = vec![
            GroupSpec {
                members: vec![0, 1],
                depth: 0,
            },
            GroupSpec {
                members: vec![2, 3],
                depth: 0,
            },
        ];
        let edges = vec![(0usize, 2usize), (1, 3)];
        let seps = SepSpecs {
            x: vec![Sep {
                left: 0,
                right: 2,
                gap: 140.0,
                equality: false,
            }],
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

    #[test]
    fn overlap_removal_prefers_the_cheap_axis() {
        // Two boxes overlapping 10px in x but 60px in y: the old x-push moved
        // one box 10+gap px right — correct. But two boxes overlapping 60px in
        // x and 10px in y must separate VERTICALLY (10+gap), not horizontally
        // (60+gap). Total displacement must be the smaller option.
        let gap = 8.0;
        let mut rects = vec![
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 50.0,
            },
            Rect {
                x: 40.0,
                y: 40.0,
                w: 100.0,
                h: 50.0,
            }, // 60px x-overlap, 10px y-overlap
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
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 50.0,
            },
            Rect {
                x: 90.0,
                y: 10.0,
                w: 100.0,
                h: 50.0,
            },
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
                Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 80.0,
                    h: 40.0,
                },
                Rect {
                    x: 30.0,
                    y: 10.0,
                    w: 80.0,
                    h: 40.0,
                },
                Rect {
                    x: 60.0,
                    y: 20.0,
                    w: 80.0,
                    h: 40.0,
                },
                Rect {
                    x: 10.0,
                    y: 35.0,
                    w: 80.0,
                    h: 40.0,
                },
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

    #[test]
    fn bfs_hops_on_a_path() {
        // 0-1-2-3 path.
        let adj = adjacency(4, &[(0, 1), (1, 2), (2, 3)]);
        let d = bfs_hops(4, &adj, 0);
        assert_eq!(d, vec![Some(0), Some(1), Some(2), Some(3)]);
    }

    #[test]
    fn bfs_hops_marks_unreachable() {
        // 0-1 and isolated 2.
        let adj = adjacency(3, &[(0, 1)]);
        assert_eq!(bfs_hops(3, &adj, 0), vec![Some(0), Some(1), None]);
    }

    #[test]
    fn dedup_edges_drops_self_and_duplicates() {
        let e = dedup_edges(3, &[(0, 1), (1, 0), (2, 2), (0, 1)]);
        assert_eq!(e, vec![(0, 1)]);
    }

    #[test]
    fn components_split_and_order_by_smallest_member() {
        // {0,2} and {1,3}, discovered so smallest member leads.
        let adj = adjacency(4, &[(0, 2), (1, 3)]);
        let comps = components(4, &adj);
        assert_eq!(comps, vec![vec![0, 2], vec![1, 3]]);
    }

    #[test]
    fn circular_seed_places_first_node_on_positive_x_axis() {
        let seed = circular_seed(4, 120.0);
        assert_eq!(seed.len(), 4);
        // Ring circumference formula gives 76.4 < edge_len, so the min clamp
        // pins the radius at edge_len.
        let radius = (120.0 * 4.0 / (2.0 * PI)).max(120.0);
        assert_eq!(radius, 120.0);
        assert!((seed[0].0 - radius).abs() < 1e-9);
        assert!(seed[0].1.abs() < 1e-9);
        // Quarter turn → (0, radius).
        assert!(seed[1].0.abs() < 1e-9);
        assert!((seed[1].1 - radius).abs() < 1e-9);
    }

    #[test]
    fn majorization_monotonically_decreases_stress() {
        // Square graph 0-1-2-3-0 plus a diagonal.
        let adj = adjacency(4, &[(0, 1), (1, 2), (2, 3), (3, 0), (0, 2)]);
        let cfg = StressConfig::default();
        let szs = sizes(4, 100.0, 40.0);
        // Rebuild the dist/weight matrices the same way component_layout does.
        let comp = [0usize, 1, 2, 3];
        let m = comp.len();
        let mut dist = vec![vec![0.0; m]; m];
        let mut w = vec![vec![0.0; m]; m];
        for (la, &ga) in comp.iter().enumerate() {
            let hops = bfs_hops(4, &adj, ga);
            for (lb, &gb) in comp.iter().enumerate() {
                if la == lb {
                    continue;
                }
                let h = hops[gb].unwrap() as f64;
                let d = h * cfg.edge_len + half_extent(&szs[ga]) + half_extent(&szs[gb]);
                dist[la][lb] = d;
                w[la][lb] = 1.0 / (d * d);
            }
        }
        let wsum: Vec<f64> = (0..m).map(|a| w[a].iter().sum()).collect();
        let seed = circular_seed(m, cfg.edge_len);
        let (_pos, trace) = majorize(&seed, &dist, &w, &wsum, &cfg);
        assert!(trace.len() >= 2);
        for pair in trace.windows(2) {
            assert!(
                pair[1] <= pair[0] + 1e-6,
                "stress rose: {} -> {}",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn separation_leaves_no_node_overlap_even_at_the_cap() {
        // This pile of overlapping sibling groups never converges: all 6
        // passes translate, so the loop exits by exhausting its cap right
        // after a batch of moves that never got the start-of-pass
        // `remove_overlaps`. Node rects must still come out overlap-free.
        let cfg = StressConfig::default();
        let mut rects = vec![
            Rect {
                x: 0.0,
                y: 0.0,
                w: 220.0,
                h: 70.0,
            },
            Rect {
                x: 70.0,
                y: 40.0,
                w: 40.0,
                h: 190.0,
            },
            Rect {
                x: 140.0,
                y: 80.0,
                w: 70.0,
                h: 110.0,
            },
            Rect {
                x: 49.0,
                y: 25.0,
                w: 100.0,
                h: 30.0,
            },
            Rect {
                x: 119.0,
                y: 65.0,
                w: 130.0,
                h: 150.0,
            },
            Rect {
                x: 28.0,
                y: 10.0,
                w: 160.0,
                h: 70.0,
            },
            Rect {
                x: 98.0,
                y: 50.0,
                w: 190.0,
                h: 190.0,
            },
        ];
        let groups = vec![
            GroupSpec {
                members: vec![0, 5],
                depth: 0,
            },
            GroupSpec {
                members: vec![1, 6],
                depth: 1,
            },
            GroupSpec {
                members: vec![2],
                depth: 2,
            },
            GroupSpec {
                members: vec![3],
                depth: 0,
            },
            GroupSpec {
                members: vec![4],
                depth: 1,
            },
        ];
        separate_hulls(&mut rects, &groups, &cfg);
        for i in 0..rects.len() {
            for j in i + 1..rects.len() {
                assert!(
                    !overlaps(&rects[i], &rects[j]),
                    "rects {i} and {j} overlap after separation: {:?} vs {:?}",
                    rects[i],
                    rects[j]
                );
            }
        }
    }

    #[test]
    fn overlap_removal_leaves_no_overlaps() {
        // Five boxes clustered on nearly the same point.
        let mut rects = vec![
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 40.0
            };
            5
        ];
        for (i, r) in rects.iter_mut().enumerate() {
            r.x = i as f64 * 5.0;
            r.y = i as f64 * 3.0;
        }
        remove_overlaps(&mut rects, 24.0);
        for i in 0..rects.len() {
            for j in (i + 1)..rects.len() {
                assert!(!overlaps(&rects[i], &rects[j]), "boxes {i},{j} overlap");
            }
        }
    }

    #[test]
    fn empty_input_is_empty() {
        assert!(layout(&[], &[], &[], &StressConfig::default()).is_empty());
    }

    #[test]
    fn single_node_sits_at_origin() {
        let r = layout(
            &ids(&["a"]),
            &sizes(1, 200.0, 90.0),
            &[],
            &StressConfig::default(),
        );
        assert_eq!(
            r,
            vec![Rect {
                x: 0.0,
                y: 0.0,
                w: 200.0,
                h: 90.0
            }]
        );
    }

    #[test]
    fn no_edges_falls_back_to_grid() {
        let cfg = StressConfig::default();
        let g = ids(&["a", "b", "c", "d"]);
        let szs = sizes(4, 100.0, 40.0);
        let via_layout = layout(&g, &szs, &[], &cfg);
        let via_grid = grid_pack(&g, &szs, &cfg);
        assert_eq!(via_layout, via_grid);
        // ceil(sqrt(4)) = 2 columns, 2 rows.
        assert_eq!(
            via_grid[0],
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 40.0
            }
        );
        assert_eq!(
            via_grid[1],
            Rect {
                x: 140.0,
                y: 0.0,
                w: 100.0,
                h: 40.0
            }
        );
        assert_eq!(
            via_grid[2],
            Rect {
                x: 0.0,
                y: 80.0,
                w: 100.0,
                h: 40.0
            }
        );
    }

    #[test]
    fn self_and_duplicate_edges_do_not_crash() {
        let cfg = StressConfig::default();
        let g = ids(&["a", "b"]);
        let szs = sizes(2, 100.0, 40.0);
        // Only self/dup edges → collapses to no-edge grid.
        let r = layout(&g, &szs, &[(0, 0), (1, 1)], &cfg);
        assert_eq!(r, grid_pack(&g, &szs, &cfg));
    }

    #[test]
    fn output_has_no_overlaps_and_is_normalized() {
        let cfg = StressConfig::default();
        let g = ids(&["a", "b", "c", "d", "e"]);
        let szs = sizes(5, 160.0, 80.0);
        let edges = [(0, 1), (1, 2), (2, 3), (3, 4), (4, 0)];
        let r = layout(&g, &szs, &edges, &cfg);
        let (min_x, min_y) = r
            .iter()
            .fold((f64::INFINITY, f64::INFINITY), |(mx, my), q| {
                (mx.min(q.x), my.min(q.y))
            });
        assert!(min_x.abs() < 1e-6, "min x normalized to 0, got {min_x}");
        assert!(min_y.abs() < 1e-6, "min y normalized to 0, got {min_y}");
        for i in 0..r.len() {
            for j in (i + 1)..r.len() {
                assert!(!overlaps(&r[i], &r[j]), "nodes {i},{j} overlap");
            }
        }
    }

    #[test]
    fn layout_is_deterministic() {
        let cfg = StressConfig::default();
        let g = ids(&["a", "b", "c", "d", "e", "f"]);
        let szs = sizes(6, 120.0, 60.0);
        let edges = [(0, 1), (1, 2), (2, 0), (3, 4), (4, 5)];
        let a = layout(&g, &szs, &edges, &cfg);
        let b = layout(&g, &szs, &edges, &cfg);
        assert_eq!(a, b);
    }

    #[test]
    fn disconnected_components_occupy_disjoint_regions() {
        // Two triangles; the shelf-packer must place the components in disjoint
        // regions (side-by-side or stacked), never overlapping.
        let cfg = StressConfig::default();
        let g = ids(&["a", "b", "c", "d", "e", "f"]);
        let szs = sizes(6, 100.0, 40.0);
        let edges = [(0, 1), (1, 2), (2, 0), (3, 4), (4, 5), (5, 3)];
        let r = layout(&g, &szs, &edges, &cfg);
        let bbox = |sl: &[Rect]| {
            sl.iter().fold(
                (
                    f64::INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::NEG_INFINITY,
                ),
                |(x0, y0, x1, y1), q| {
                    (
                        x0.min(q.x),
                        y0.min(q.y),
                        x1.max(q.x + q.w),
                        y1.max(q.y + q.h),
                    )
                },
            )
        };
        let (ax0, ay0, ax1, ay1) = bbox(&r[0..3]);
        let (bx0, by0, bx1, by1) = bbox(&r[3..6]);
        let disjoint =
            bx0 >= ax1 - 1e-6 || ax0 >= bx1 - 1e-6 || by0 >= ay1 - 1e-6 || ay0 >= by1 - 1e-6;
        assert!(
            disjoint,
            "component bounding boxes overlap: a=({ax0},{ay0},{ax1},{ay1}) b=({bx0},{by0},{bx1},{by1})"
        );
    }

    /// The equivalence is structural, not incidental: `layout` is literally
    /// `layout_grouped(.., &[], ..).0`, so every pass -- `reduce_crossings`
    /// included -- runs identically on both paths by construction. This guards
    /// against that delegation being replaced by a divergent copy.
    #[test]
    fn layout_grouped_with_no_groups_matches_layout() {
        let cfg = StressConfig::default();
        let g = ids(&["a", "b", "c", "d", "e"]);
        let szs = sizes(5, 160.0, 80.0);
        let edges = [(0, 1), (1, 2), (2, 3), (3, 4), (4, 0)];
        let plain = layout(&g, &szs, &edges, &cfg);
        let (grouped, hulls) = layout_grouped(&g, &szs, &edges, &[], &cfg);
        assert_eq!(plain, grouped);
        assert!(hulls.is_empty());
    }

    #[test]
    fn layout_grouped_edgeless_with_no_groups_matches_grid() {
        let cfg = StressConfig::default();
        let g = ids(&["a", "b", "c", "d"]);
        let szs = sizes(4, 100.0, 40.0);
        let via_grid = grid_pack(&g, &szs, &cfg);
        let (grouped, hulls) = layout_grouped(&g, &szs, &[], &[], &cfg);
        assert_eq!(grouped, via_grid);
        assert!(hulls.is_empty());
    }

    #[test]
    fn group_hulls_bound_members_with_padding() {
        let cfg = StressConfig::default();
        let rects = vec![
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 40.0,
            },
            Rect {
                x: 200.0,
                y: 100.0,
                w: 100.0,
                h: 40.0,
            },
        ];
        let groups = vec![GroupSpec {
            members: vec![0, 1],
            depth: 0,
        }];
        let hulls = group_hulls(&rects, &groups, &cfg);
        assert_eq!(hulls.len(), 1);
        let h = hulls[0];
        assert!((h.x - (0.0 - cfg.hull_pad)).abs() < 1e-9);
        assert!((h.y - (0.0 - cfg.hull_pad)).abs() < 1e-9);
        assert!((h.w - (300.0 + 2.0 * cfg.hull_pad)).abs() < 1e-9);
        assert!((h.h - (140.0 + 2.0 * cfg.hull_pad)).abs() < 1e-9);
    }

    #[test]
    fn comembership_depths_uses_deepest_shared_group() {
        // Outer group {0,1,2} depth 0, inner group {0,1} depth 1.
        let groups = vec![
            GroupSpec {
                members: vec![0, 1, 2],
                depth: 0,
            },
            GroupSpec {
                members: vec![0, 1],
                depth: 1,
            },
        ];
        let d = comembership_depths(&groups);
        assert_eq!(d.get(&(0, 1)), Some(&1)); // deepest shared group wins
        assert_eq!(d.get(&(0, 2)), Some(&0));
        assert_eq!(d.get(&(1, 2)), Some(&0));
    }

    #[test]
    fn cohesion_pulls_co_members_closer_than_bare_hops_would() {
        // Star: hub 0 connects to a (1) and to the group {b(2), c(3), d(4)} via b
        // only — so c and d have no real edge at all, just co-membership. Without
        // cohesion c/d would sit far apart (2+ hops through b); with cohesion they
        // must land within a `group_len`-ish target distance of each other.
        let cfg = StressConfig::default();
        let g = ids(&["hub", "a", "b", "c", "d"]);
        let szs = sizes(5, 80.0, 40.0);
        let edges = [(0, 1), (0, 2)];
        let groups = vec![GroupSpec {
            members: vec![2, 3, 4], // b, c, d
            depth: 0,
        }];
        // c (3) and d (4) are unreachable via `edges` alone.
        let plain_adj = adjacency(5, &dedup_edges(5, &edges));
        assert!(bfs_hops(5, &plain_adj, 3)[4].is_none());

        let (rects, hulls) = layout_grouped(&g, &szs, &edges, &groups, &cfg);
        assert_eq!(hulls.len(), 1);
        let c = &rects[3];
        let d = &rects[4];
        let cx = c.x + c.w / 2.0;
        let cy = c.y + c.h / 2.0;
        let dx = d.x + d.w / 2.0;
        let dy = d.y + d.h / 2.0;
        let dist = (cx - dx).hypot(cy - dy);
        // c and d have no edge at all — without cohesion their target distance
        // through the merged graph would be 2 hops (b-c, b-d), i.e. ~2*edge_len.
        // Cohesion pulls the *target* distance down to ~group_len instead, so the
        // solved distance should land well under the bare-hop distance.
        assert!(
            dist < 2.0 * cfg.edge_len,
            "c/d too far apart: {dist} (expected well under {})",
            2.0 * cfg.edge_len
        );

        // Sibling group members' bboxes must sit inside the emitted hull.
        let hull = hulls[0];
        for m in [2usize, 3, 4] {
            let r = &rects[m];
            assert!(
                r.x >= hull.x - 1e-6
                    && r.y >= hull.y - 1e-6
                    && r.x + r.w <= hull.x + hull.w + 1e-6
                    && r.y + r.h <= hull.y + hull.h + 1e-6,
                "member {m} rect not inside hull"
            );
        }
    }

    #[test]
    fn strong_outside_edge_still_pulls_a_member_away() {
        // Two members b,c grouped, but c also carries a direct edge to an
        // outside anchor `x` — cohesion is soft, so c should sit closer to its
        // real edge-neighbor x than an ungrouped member would to nothing, i.e.
        // adding the group must not force c on top of b regardless of the edge.
        let cfg = StressConfig::default();
        let g = ids(&["b", "c", "x"]);
        let szs = sizes(3, 80.0, 40.0);
        let edges = [(1, 2)]; // c -- x
        let groups = vec![GroupSpec {
            members: vec![0, 1], // b, c
            depth: 0,
        }];
        let (rects, _hulls) = layout_grouped(&g, &szs, &edges, &groups, &cfg);
        let b = &rects[0];
        let c = &rects[1];
        let x = &rects[2];
        let dist = |p: &Rect, q: &Rect| {
            let px = p.x + p.w / 2.0;
            let py = p.y + p.h / 2.0;
            let qx = q.x + q.w / 2.0;
            let qy = q.y + q.h / 2.0;
            (px - qx).hypot(py - qy)
        };
        // c's real edge to x still shapes the result: c is not glued to b.
        assert!(dist(c, x) > 0.0);
        assert!(dist(b, c) > 0.0);
    }

    fn rects_overlap(a: &Rect, b: &Rect) -> bool {
        let ox = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
        let oy = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
        ox > 0.0 && oy > 0.0
    }

    #[test]
    fn sibling_hulls_never_overlap() {
        // Two groups with a single cross-group edge pulling them together —
        // without separation their hulls would collide.
        let cfg = StressConfig::default();
        let g = ids(&["a1", "a2", "a3", "b1", "b2", "b3"]);
        let szs = sizes(6, 100.0, 50.0);
        let edges = [(2, 3)]; // a3 -- b1, the only cross-group pull
        let groups = vec![
            GroupSpec {
                members: vec![0, 1, 2],
                depth: 0,
            },
            GroupSpec {
                members: vec![3, 4, 5],
                depth: 0,
            },
        ];
        let (rects, hulls) = layout_grouped(&g, &szs, &edges, &groups, &cfg);
        assert_eq!(hulls.len(), 2);
        assert!(
            !rects_overlap(&hulls[0], &hulls[1]),
            "sibling hulls overlap: {:?} vs {:?}",
            hulls[0],
            hulls[1]
        );
        // Members still sit inside their own hull.
        for (gi, group) in groups.iter().enumerate() {
            let hull = hulls[gi];
            for &m in &group.members {
                let r = &rects[m];
                assert!(
                    r.x >= hull.x - 1e-6
                        && r.y >= hull.y - 1e-6
                        && r.x + r.w <= hull.x + hull.w + 1e-6
                        && r.y + r.h <= hull.y + hull.h + 1e-6,
                    "member {m} rect not inside its own hull"
                );
            }
        }
    }

    #[test]
    fn nested_group_hull_stays_inside_parent_hull() {
        let cfg = StressConfig::default();
        let g = ids(&["a", "b", "c"]);
        let szs = sizes(3, 100.0, 50.0);
        let edges: [(usize, usize); 0] = [];
        let groups = vec![
            GroupSpec {
                members: vec![0, 1, 2],
                depth: 0,
            },
            GroupSpec {
                members: vec![0, 1],
                depth: 1,
            },
        ];
        let (_rects, hulls) = layout_grouped(&g, &szs, &edges, &groups, &cfg);
        let outer = hulls[0];
        let inner = hulls[1];
        assert!(
            inner.x >= outer.x - 1e-6
                && inner.y >= outer.y - 1e-6
                && inner.x + inner.w <= outer.x + outer.w + 1e-6
                && inner.y + inner.h <= outer.y + outer.h + 1e-6,
            "inner hull {inner:?} not inside outer hull {outer:?}"
        );
    }

    #[test]
    fn separate_hulls_is_deterministic_with_multiple_groups() {
        let cfg = StressConfig::default();
        let g = ids(&["a1", "a2", "b1", "b2", "c1", "c2"]);
        let szs = sizes(6, 90.0, 45.0);
        let edges = [(1, 2), (3, 4)];
        let groups = vec![
            GroupSpec {
                members: vec![0, 1],
                depth: 0,
            },
            GroupSpec {
                members: vec![2, 3],
                depth: 0,
            },
            GroupSpec {
                members: vec![4, 5],
                depth: 0,
            },
        ];
        let one = layout_grouped(&g, &szs, &edges, &groups, &cfg);
        let two = layout_grouped(&g, &szs, &edges, &groups, &cfg);
        assert_eq!(one, two);
    }

    #[test]
    fn singleton_groups_without_edges_still_emit_one_hull_each() {
        // Every group has a single member and there are no edges, so the merged
        // graph is empty and the grid fallback fires — the hull contract ("one
        // hull per group, in `groups` order") must still hold.
        let cfg = StressConfig::default();
        let g = ids(&["a", "b", "c"]);
        let szs = sizes(3, 100.0, 40.0);
        let groups = vec![
            GroupSpec {
                members: vec![0],
                depth: 0,
            },
            GroupSpec {
                members: vec![1],
                depth: 0,
            },
        ];
        let (rects, hulls) = layout_grouped(&g, &szs, &[], &groups, &cfg);
        assert_eq!(hulls.len(), groups.len(), "one hull per group");
        for (gi, group) in groups.iter().enumerate() {
            let hull = hulls[gi];
            for &m in &group.members {
                let r = &rects[m];
                assert!(
                    r.x >= hull.x - 1e-6
                        && r.y >= hull.y - 1e-6
                        && r.x + r.w <= hull.x + hull.w + 1e-6
                        && r.y + r.h <= hull.y + hull.h + 1e-6,
                    "member {m} rect not inside its own hull"
                );
            }
        }
        assert!(
            !rects_overlap(&hulls[0], &hulls[1]),
            "sibling hulls overlap: {:?} vs {:?}",
            hulls[0],
            hulls[1]
        );
    }

    #[test]
    fn three_interleaved_groups_end_separated_and_overlap_free() {
        // Three groups chained by cross-group edges, plus two ungrouped nodes
        // seeded between them. The post-conditions must hold on the *returned*
        // rects: sibling hulls disjoint, no node-node overlap, and no ungrouped
        // node left sitting inside a hull.
        let cfg = StressConfig::default();
        let g = ids(&[
            "a1", "a2", "b1", "b2", "c1", "c2", "loose1", "loose2", "loose3",
        ]);
        let szs = sizes(9, 120.0, 60.0);
        let edges = [(1, 2), (3, 4), (5, 6), (6, 0), (7, 2), (8, 4)];
        let groups = vec![
            GroupSpec {
                members: vec![0, 1],
                depth: 0,
            },
            GroupSpec {
                members: vec![2, 3],
                depth: 0,
            },
            GroupSpec {
                members: vec![4, 5],
                depth: 0,
            },
        ];
        let (rects, hulls) = layout_grouped(&g, &szs, &edges, &groups, &cfg);
        assert_eq!(hulls.len(), groups.len());
        for i in 0..hulls.len() {
            for j in (i + 1)..hulls.len() {
                assert!(
                    !rects_overlap(&hulls[i], &hulls[j]),
                    "hulls {i},{j} overlap: {:?} vs {:?}",
                    hulls[i],
                    hulls[j]
                );
            }
        }
        for i in 0..rects.len() {
            for j in (i + 1)..rects.len() {
                assert!(!overlaps(&rects[i], &rects[j]), "nodes {i},{j} overlap");
            }
        }
        for loose in [6usize, 7, 8] {
            for (gi, hull) in hulls.iter().enumerate() {
                assert!(
                    !rects_overlap(&rects[loose], hull),
                    "ungrouped node {loose} sits inside hull {gi}"
                );
            }
        }
    }

    /// The separation post-conditions ("sibling hulls never overlap" and "no
    /// node-node overlap") must hold on the *returned* rects for every shape of
    /// input, not just the hand-picked fixtures. Deterministic sweep; the case
    /// n=11 wk=1 ek=4 gk=1 used to fail when overlap removal ran *after* the
    /// last separation check.
    #[test]
    fn hull_separation_post_conditions_hold_across_configurations() {
        let cfg = StressConfig::default();
        for n in 6..12usize {
            for wk in 0..4 {
                let szs: Vec<Size> = (0..n)
                    .map(|i| Size {
                        w: 60.0 + (wk * 60) as f64 + (i % 3) as f64 * 40.0,
                        h: 30.0 + (i % 2) as f64 * 50.0,
                    })
                    .collect();
                for ek in 0..6usize {
                    let edges: Vec<(usize, usize)> = (0..n)
                        .map(|i| (i, (i * (ek + 1) + 1) % n))
                        .filter(|(a, b)| a != b)
                        .collect();
                    for gk in 1..4usize {
                        let mut groups = Vec::new();
                        let per = 2 + gk;
                        let mut i = 0;
                        while i + per <= n {
                            groups.push(GroupSpec {
                                members: (i..i + per).collect(),
                                depth: 0,
                            });
                            i += per;
                        }
                        if groups.len() < 2 {
                            continue;
                        }
                        let g = ids(&vec!["x"; n]);
                        let (rects, hulls) = layout_grouped(&g, &szs, &edges, &groups, &cfg);
                        for a in 0..hulls.len() {
                            for b in (a + 1)..hulls.len() {
                                assert!(
                                    !rects_overlap(&hulls[a], &hulls[b]),
                                    "n={n} wk={wk} ek={ek} gk={gk}: hulls {a},{b} overlap"
                                );
                            }
                        }
                        for a in 0..rects.len() {
                            for b in (a + 1)..rects.len() {
                                assert!(
                                    !overlaps(&rects[a], &rects[b]),
                                    "n={n} wk={wk} ek={ek} gk={gk}: nodes {a},{b} overlap"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn layout_grouped_is_deterministic() {
        let cfg = StressConfig::default();
        let g = ids(&["a", "b", "c", "d", "e"]);
        let szs = sizes(5, 100.0, 40.0);
        let edges = [(0, 1), (2, 3)];
        let groups = vec![GroupSpec {
            members: vec![0, 1, 2],
            depth: 0,
        }];
        let one = layout_grouped(&g, &szs, &edges, &groups, &cfg);
        let two = layout_grouped(&g, &szs, &edges, &groups, &cfg);
        assert_eq!(one, two);
    }

    /// Task 4: a hand-built layout with one obviously avoidable crossing --
    /// two 2-member groups, side by side, whose cross edges are inverted
    /// (0-2 and 1-3 form an X). Swapping the two members *within* group A is
    /// cohesion-preserving by construction and untangles it. `reduce_crossings`
    /// is private, so this calls it directly rather than going through the
    /// full `layout_grouped` pipeline (whose overlap/hull passes would be
    /// incidental to what this test is checking).
    #[test]
    fn reduce_crossings_untangles_a_hand_built_inverted_x() {
        let mut rects = vec![
            Rect {
                x: 0.0,
                y: 0.0,
                w: 20.0,
                h: 20.0,
            }, // node 0, group A, top
            Rect {
                x: 0.0,
                y: 100.0,
                w: 20.0,
                h: 20.0,
            }, // node 1, group A, bottom
            Rect {
                x: 200.0,
                y: 0.0,
                w: 20.0,
                h: 20.0,
            }, // node 2, group B, top
            Rect {
                x: 200.0,
                y: 100.0,
                w: 20.0,
                h: 20.0,
            }, // node 3, group B, bottom
        ];
        let groups = vec![
            GroupSpec {
                members: vec![0, 1],
                depth: 0,
            },
            GroupSpec {
                members: vec![2, 3],
                depth: 0,
            },
        ];
        // 0 (top-left) -- 2 (bottom-right... no, top-right) and 1 (bottom-left)
        // -- 3 (top-right) is the inversion: 0-2 and 1-3 form an X.
        let edges = [(0, 3), (1, 2)];
        let cfg = StressConfig::default();

        let centers = |rs: &[Rect]| -> Vec<(f64, f64)> {
            rs.iter()
                .map(|r| (r.x + r.w / 2.0, r.y + r.h / 2.0))
                .collect()
        };
        let before = segment_crossings(&centers(&rects), &edges);
        assert_eq!(before, 1, "fixture must start with exactly one crossing");

        reduce_crossings(&mut rects, &groups, &edges, &cfg);

        let after = segment_crossings(&centers(&rects), &edges);
        assert_eq!(
            after, 0,
            "the hill-climb should untangle the one avoidable crossing"
        );

        // Cohesion preservation: each group's member set is still exactly the
        // same two node indices (the pass permutes positions, never
        // membership), and they are still each other's nearest groupmate --
        // i.e. still closer to each other than to either member of the other
        // group, so the swap did not smuggle a node across group lines.
        for g in &groups {
            let (m0, m1) = (g.members[0], g.members[1]);
            let d_within = dist(centers(&rects)[m0], centers(&rects)[m1]);
            for g2 in &groups {
                if g2.members == g.members {
                    continue;
                }
                for &other in &g2.members {
                    let d_across = dist(centers(&rects)[m0], centers(&rects)[other]);
                    assert!(
                        d_within < d_across,
                        "group member {m0} ended up closer to a member of another group"
                    );
                }
            }
        }
    }

    /// Review fix (high): the group-level moves are cohesion-preserving only
    /// while group membership is disjoint and both groups sit in the same
    /// shelf-packed component. Two top-level groups that SHARE a member (the
    /// same element under two headings -- the case `separate_hulls` handles
    /// explicitly) must get NO group-level moves: the shared member would take
    /// group A's offset and then group B's opposite one and stay put while
    /// both hulls translate around it, or be mirrored out of the sibling it
    /// also belongs to. Asserted on the candidate generator directly -- the
    /// intra-pool swaps can reach many of the same arrangements, so a
    /// whole-pass behavioural test does not reliably exercise this branch.
    #[test]
    fn group_level_moves_skip_entangled_groups() {
        // Groups 0 and 1 share member 2; group 2 is disjoint. All one component.
        let members = vec![vec![0, 1, 2], vec![2, 3, 4], vec![5, 6]];
        let top_level = vec![0, 1, 2];
        let comp_of = vec![0; 7];

        let moves = group_level_moves(&members, &top_level, &comp_of);

        assert_eq!(
            moves,
            vec![Move::ReflectGroup(2, true), Move::ReflectGroup(2, false)],
            "only the disjoint group may move as a group; an entangled group              gets neither a swap nor a reflection"
        );
    }

    /// Review fix (high): group-level swaps carry the same connected-component
    /// restriction as the pool swaps -- components are shelf-packed into
    /// disjoint regions, so swapping two groups across components teleports
    /// each into the other's region. Reflections stay legal (a group is
    /// mirrored inside its own hull, never leaving its region).
    #[test]
    fn group_level_moves_do_not_swap_across_components() {
        let members = vec![vec![0, 1], vec![2, 3]];
        let top_level = vec![0, 1];
        let comp_of = vec![0, 0, 1, 1];

        let moves = group_level_moves(&members, &top_level, &comp_of);

        assert_eq!(
            moves,
            vec![
                Move::ReflectGroup(0, true),
                Move::ReflectGroup(0, false),
                Move::ReflectGroup(1, true),
                Move::ReflectGroup(1, false),
            ],
            "groups in different components must not be swapped"
        );

        // Same component: the swap comes back.
        let same = group_level_moves(&members, &top_level, &[0, 0, 0, 0]);
        assert!(same.contains(&Move::SwapGroups(0, 1)));
    }

    /// Review fix (high): every other `reduce_crossings` test calls the
    /// private fn directly on a hand-built, already-separated fixture, so the
    /// pass being a no-op in *production* was invisible. This drives the real
    /// `layout_grouped` pipeline, where the pass runs before `separate_hulls`
    /// and top-level hulls therefore routinely still overlap -- the exact
    /// situation in which the old absolute hull guard rejected every candidate
    /// and the pass changed nothing. Fails on that guard; passes on the
    /// relative one.
    #[test]
    fn reduce_crossings_changes_the_real_pipeline_output() {
        let g = ids(&["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l"]);
        let szs = sizes(12, 120.0, 40.0);
        let groups = vec![
            GroupSpec {
                members: vec![0, 1, 2],
                depth: 0,
            },
            GroupSpec {
                members: vec![3, 4, 5],
                depth: 0,
            },
            GroupSpec {
                members: vec![6, 7, 8],
                depth: 0,
            },
            GroupSpec {
                members: vec![9, 10, 11],
                depth: 0,
            },
        ];
        // Interleaved inter-group edges: every group reaches into every other,
        // so no arrangement is crossing-free and there is room to improve.
        let edges = [
            (0, 1),
            (1, 2),
            (3, 4),
            (4, 5),
            (6, 7),
            (7, 8),
            (9, 10),
            (10, 11),
            (0, 4),
            (1, 5),
            (2, 3),
            (3, 7),
            (4, 8),
            (5, 6),
            (6, 10),
            (7, 11),
            (8, 9),
            (0, 11),
            (1, 9),
            (2, 10),
        ];

        let off = StressConfig {
            crossing_passes: 0,
            ..StressConfig::default()
        };
        let on = StressConfig::default();
        assert!(on.crossing_passes > 0);

        let (rects_off, _) = layout_grouped(&g, &szs, &edges, &groups, &off);
        let (rects_on, _) = layout_grouped(&g, &szs, &edges, &groups, &on);

        assert_ne!(
            rects_off, rects_on,
            "the crossing pass must actually reach the layout through the real \
             pipeline -- if these match, it is a silent no-op in production"
        );

        let centers = |rs: &[Rect]| -> Vec<(f64, f64)> {
            rs.iter()
                .map(|r| (r.x + r.w / 2.0, r.y + r.h / 2.0))
                .collect()
        };
        let before = segment_crossings(&centers(&rects_off), &edges);
        let after = segment_crossings(&centers(&rects_on), &edges);
        assert!(
            after < before,
            "the pass must lower the crossing count end to end ({before} -> {after})"
        );
    }

    fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
        ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
    }

    /// Determinism: identical input run through `reduce_crossings` twice
    /// yields byte-identical rects, matching the golden `layout_grouped`
    /// determinism guard above.
    #[test]
    fn reduce_crossings_is_deterministic() {
        let base = vec![
            Rect {
                x: 0.0,
                y: 0.0,
                w: 20.0,
                h: 20.0,
            },
            Rect {
                x: 0.0,
                y: 100.0,
                w: 20.0,
                h: 20.0,
            },
            Rect {
                x: 200.0,
                y: 0.0,
                w: 20.0,
                h: 20.0,
            },
            Rect {
                x: 200.0,
                y: 100.0,
                w: 20.0,
                h: 20.0,
            },
        ];
        let groups = vec![
            GroupSpec {
                members: vec![0, 1],
                depth: 0,
            },
            GroupSpec {
                members: vec![2, 3],
                depth: 0,
            },
        ];
        let edges = [(0, 3), (1, 2)];
        let cfg = StressConfig::default();

        let mut one = base.clone();
        reduce_crossings(&mut one, &groups, &edges, &cfg);
        let mut two = base.clone();
        reduce_crossings(&mut two, &groups, &edges, &cfg);
        assert_eq!(one, two);
    }

    /// Regression guard: `crossing_passes: 0` is a provable no-op -- the
    /// pass's opt-out escape hatch reproduces the exact pre-pass rects.
    #[test]
    fn crossing_passes_zero_leaves_rects_untouched() {
        let base = vec![
            Rect {
                x: 0.0,
                y: 0.0,
                w: 20.0,
                h: 20.0,
            },
            Rect {
                x: 0.0,
                y: 100.0,
                w: 20.0,
                h: 20.0,
            },
            Rect {
                x: 200.0,
                y: 0.0,
                w: 20.0,
                h: 20.0,
            },
            Rect {
                x: 200.0,
                y: 100.0,
                w: 20.0,
                h: 20.0,
            },
        ];
        let groups = vec![
            GroupSpec {
                members: vec![0, 1],
                depth: 0,
            },
            GroupSpec {
                members: vec![2, 3],
                depth: 0,
            },
        ];
        let edges = [(0, 3), (1, 2)];
        let cfg = StressConfig {
            crossing_passes: 0,
            ..StressConfig::default()
        };

        let mut rects = base.clone();
        reduce_crossings(&mut rects, &groups, &edges, &cfg);
        assert_eq!(rects, base, "crossing_passes: 0 must be a provable no-op");
    }

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect { x, y, w, h }
    }

    /// Review fix: with no groups the `top_level_hull_overlap` guard is
    /// vacuous and `separate_hulls` returns immediately, so a `SwapMembers` of
    /// two differently-sized rects used to leave node rects overlapping with
    /// nothing to clean up. Node 1 is far wider than node 0; swapping their
    /// centers untangles the X but parks node 1 on top of node 2.
    #[test]
    fn reduce_crossings_leaves_ungrouped_rects_overlap_free() {
        // Centers: 0 (0,0), 1 (0,100), 2 (300,0), 3 (300,100).
        let mut rects = vec![
            rect(-10.0, -10.0, 20.0, 20.0),
            rect(-300.0, 90.0, 600.0, 20.0),
            rect(290.0, -10.0, 20.0, 20.0),
            rect(290.0, 90.0, 20.0, 20.0),
        ];
        // (2,3) keeps all four in one connected component; (0,3)/(1,2) cross.
        let edges = [(0, 3), (1, 2), (2, 3)];
        let cfg = StressConfig::default();

        let centers = |rs: &[Rect]| -> Vec<(f64, f64)> {
            rs.iter()
                .map(|r| (r.x + r.w / 2.0, r.y + r.h / 2.0))
                .collect()
        };
        assert_eq!(segment_crossings(&centers(&rects), &edges), 1);

        reduce_crossings(&mut rects, &[], &edges, &cfg);

        for i in 0..rects.len() {
            for j in (i + 1)..rects.len() {
                let (a, b) = (&rects[i], &rects[j]);
                let overlap =
                    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
                assert!(
                    !overlap,
                    "rects {i} and {j} overlap after the pass: {a:?} {b:?}"
                );
            }
        }
    }

    /// Review fix: a `GroupSpec` naming a node index past the end of `rects`
    /// used to be skipped by the `leaf_group`/`moved_nodes` paths but indexed
    /// unchecked by `hull_center`/the hull-overlap guard, so it panicked
    /// instead of being ignored. Members are now filtered once up front, so the
    /// stale index is inert and the real crossing is still untangled.
    #[test]
    fn reduce_crossings_ignores_out_of_range_group_members() {
        let mut rects = vec![
            rect(-10.0, -10.0, 20.0, 20.0),
            rect(-10.0, 90.0, 20.0, 20.0),
            rect(190.0, -10.0, 20.0, 20.0),
            rect(190.0, 90.0, 20.0, 20.0),
        ];
        let groups = vec![
            GroupSpec {
                // 99 does not exist.
                members: vec![0, 1, 99],
                depth: 0,
            },
            GroupSpec {
                members: vec![2, 3],
                depth: 0,
            },
            // A group whose every member is out of range must not produce a
            // NaN hull center either.
            GroupSpec {
                members: vec![42],
                depth: 0,
            },
        ];
        let edges = [(0, 3), (1, 2)];
        let cfg = StressConfig::default();

        let centers = |rs: &[Rect]| -> Vec<(f64, f64)> {
            rs.iter()
                .map(|r| (r.x + r.w / 2.0, r.y + r.h / 2.0))
                .collect()
        };
        assert_eq!(segment_crossings(&centers(&rects), &edges), 1);

        reduce_crossings(&mut rects, &groups, &edges, &cfg);

        assert_eq!(
            segment_crossings(&centers(&rects), &edges),
            0,
            "the stale member must not stop the pass from untangling the X"
        );
        for (i, r) in rects.iter().enumerate() {
            assert!(
                r.x.is_finite() && r.y.is_finite(),
                "rect {i} went non-finite"
            );
        }
    }

    /// Review fix: the virtual "no group" pool used to be global, so a swap
    /// could exchange two ungrouped nodes from *different* connected
    /// components, teleporting one out of its shelf-packed region. Here the
    /// only crossing-reducing swap (0 <-> 2) is cross-component; the only
    /// same-component swaps (0<->1, 2<->3) merely reverse a segment and change
    /// nothing, so the pass must leave the layout alone.
    #[test]
    fn reduce_crossings_never_swaps_across_components() {
        let base = vec![
            rect(-10.0, -10.0, 20.0, 20.0), // 0, center (0,0)
            rect(90.0, 90.0, 20.0, 20.0),   // 1, center (100,100)
            rect(-10.0, 90.0, 20.0, 20.0),  // 2, center (0,100)
            rect(90.0, -10.0, 20.0, 20.0),  // 3, center (100,0)
        ];
        // Two disjoint components whose segments cross geometrically.
        let edges = [(0, 1), (2, 3)];
        let cfg = StressConfig::default();

        let centers = |rs: &[Rect]| -> Vec<(f64, f64)> {
            rs.iter()
                .map(|r| (r.x + r.w / 2.0, r.y + r.h / 2.0))
                .collect()
        };
        assert_eq!(segment_crossings(&centers(&base), &edges), 1);

        let mut rects = base.clone();
        reduce_crossings(&mut rects, &[], &edges, &cfg);
        assert_eq!(
            rects, base,
            "the only improving swap crosses component lines and must be rejected"
        );
    }
}
