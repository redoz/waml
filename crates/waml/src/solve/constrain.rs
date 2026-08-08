//! `Scene` → stress-solve constraint compiler.
//!
//! Lowers authored `Constraint::Place`/`Constraint::Align` statements (plus
//! `Box::axis` row/column chains) into per-axis `vpsc::Sep` lists over leaf
//! indices, with boundary variables standing in for group operands. This is
//! the single source of truth the unified stress path (`stress::layout_constrained`)
//! consumes; `geometry::solve_cluster`'s edge-blind strip packer stays in
//! place for the wasm/CLI callers this plan does not touch.

use super::geometry::{axis_constraints, edge_axes, off_x, off_y, pair_gap, DroppedPlacement};
use super::stress::{GroupSpec, SepSpecs, StressConfig};
use super::vpsc::Sep;
use super::{Box, BoxId, Constraint, FlagSet, Scene, Size, SolveConfig};
use crate::layout::{Direction, Edge, Margin, Shape};
use std::collections::{BTreeMap, BTreeSet};

/// Everything the unified stress path needs from a resolved `Scene`.
#[derive(Debug, Clone, PartialEq)]
pub struct Compiled {
    /// Leaf keys in solve order — index `i` here is sep/layout index `i`.
    pub keys: Vec<String>,
    /// One `GroupSpec` per `Group` box (scene order), members = descendant
    /// leaf indices — replaces the editor's `flatten_groups` on this path.
    pub group_specs: Vec<GroupSpec>,
    /// `(title, depth, shape)` per group, same order as `group_specs`. Shape
    /// carries the authored `with frame`/etc. hint (default `Shape::Shrink`,
    /// same as `resolve::add_group`'s default) so the unified path renders
    /// the same chrome the authored-layout path always has.
    pub group_meta: Vec<(Option<String>, u8, Shape)>,
    /// One `GroupSpec` per `Inline` box (scene order): cohesion only, no
    /// hull — inline groups cluster without a frame.
    pub inline_specs: Vec<GroupSpec>,
    pub seps: SepSpecs,
    /// `FlagSet` per leaf index (collapsed/emphasized survive unification).
    pub flags: Vec<FlagSet>,
    /// Constraints dropped at compile time (unknown operand, no shared
    /// axis), with `conflicts_with` empty.
    pub dropped: Vec<DroppedPlacement>,
    /// Sep index (per axis) -> originating `Constraint`, so solver-dropped
    /// seps map back to `DroppedPlacement { relation, .. }`.
    pub provenance_x: Vec<Option<Constraint>>,
    pub provenance_y: Vec<Option<Constraint>>,
}

/// One endpoint of a `Place`/`Align` relation once resolved against the
/// scene: a leaf (with its effective size — chip-sized when collapsed) or a
/// group standing in for its not-yet-allocated boundary variables.
enum Endpoint {
    Leaf { idx: usize, size: Size },
    Group(BoxId),
}

/// Which axis (`true` = x) each `Direction` constrains, and whether operand
/// `a` plays the "left"/"top" role (occupies the space before the gap) on
/// that axis. Diagonals combine the two orthogonal directions they are named
/// after (`AboveLeft = Above + LeftOf`, etc. — see `geometry::place_deltas`,
/// whose per-direction deltas this mirrors).
fn axis_components(dir: Direction) -> Vec<(bool, bool)> {
    use Direction::*;
    match dir {
        LeftOf => vec![(true, true)],
        RightOf => vec![(true, false)],
        Above => vec![(false, true)],
        Below => vec![(false, false)],
        AboveLeft => vec![(false, true), (true, true)],
        AboveRight => vec![(false, true), (true, false)],
        BelowLeft => vec![(false, false), (true, true)],
        BelowRight => vec![(false, false), (true, false)],
    }
}

/// Descendant leaf indices of `id` (recursive; a leaf's own index is itself).
fn descendant_leaves(
    id: &BoxId,
    boxes_by_id: &BTreeMap<BoxId, &Box>,
    leaf_index: &BTreeMap<BoxId, usize>,
    out: &mut Vec<usize>,
) {
    if let Some(&idx) = leaf_index.get(id) {
        out.push(idx);
        return;
    }
    if let Some(b) = boxes_by_id.get(id) {
        for c in &b.children {
            descendant_leaves(c, boxes_by_id, leaf_index, out);
        }
    }
}

/// Allocate (or reuse) the `(min, max)` boundary-variable pair for `group`
/// on one axis (`axis_x` selects x vs y). First allocation emits the
/// containment seps pinning every member leaf between them (`min + pad <=
/// member`, `member + extent + pad <= max`), tagged with the same
/// provenance as the relation that triggered the allocation — allocation
/// order is the deterministic constraint scan order (first reference wins).
#[allow(clippy::too_many_arguments)]
fn boundary_pair(
    group: &BoxId,
    axis_x: bool,
    boundary_vars: &mut BTreeMap<(BoxId, u8), (usize, usize)>,
    next_extra: &mut usize,
    group_members_by_id: &BTreeMap<BoxId, Vec<usize>>,
    leaf_sizes: &[Option<Size>],
    hull_pad: f64,
    seps: &mut SepSpecs,
    provenance_x: &mut Vec<Option<Constraint>>,
    provenance_y: &mut Vec<Option<Constraint>>,
    origin: &Constraint,
) -> (usize, usize) {
    let axis: u8 = if axis_x { 0 } else { 1 };
    if let Some(&pair) = boundary_vars.get(&(group.clone(), axis)) {
        return pair;
    }
    let l = *next_extra;
    let r = *next_extra + 1;
    *next_extra += 2;
    boundary_vars.insert((group.clone(), axis), (l, r));
    let members = group_members_by_id.get(group).cloned().unwrap_or_default();
    for m in members {
        // A member missing from `sizes` (no test forces this path) falls
        // back to the same default `geometry::solve_box` uses for a leaf
        // with no measured size, so containment stays sane rather than
        // panicking on an index.
        let size = leaf_sizes[m].unwrap_or(Size { w: 100.0, h: 40.0 });
        let extent = if axis_x { size.w } else { size.h };
        let near = Sep {
            left: l,
            right: m,
            gap: hull_pad,
            equality: false,
        };
        let far = Sep {
            left: m,
            right: r,
            gap: extent + hull_pad,
            equality: false,
        };
        if axis_x {
            seps.x.push(near);
            provenance_x.push(Some(origin.clone()));
            seps.x.push(far);
            provenance_x.push(Some(origin.clone()));
        } else {
            seps.y.push(near);
            provenance_y.push(Some(origin.clone()));
            seps.y.push(far);
            provenance_y.push(Some(origin.clone()));
        }
    }
    (l, r)
}

pub fn compile(
    scene: &Scene,
    sizes: &BTreeMap<String, Size>,
    label_widths: &BTreeMap<(BoxId, BoxId), f64>,
    connected: &BTreeSet<(BoxId, BoxId)>,
    cfg: &SolveConfig,
) -> Compiled {
    let boxes_by_id: BTreeMap<BoxId, &Box> =
        scene.boxes.iter().map(|b| (b.id.clone(), b)).collect();

    // Rule 1: leaf enumeration, scene order, dedup (first occurrence wins).
    let mut keys: Vec<String> = Vec::new();
    let mut leaf_index: BTreeMap<BoxId, usize> = BTreeMap::new();
    let mut flags: Vec<FlagSet> = Vec::new();
    for b in &scene.boxes {
        if let BoxId::Node(k) = &b.id {
            if leaf_index.contains_key(&b.id) {
                continue;
            }
            leaf_index.insert(b.id.clone(), keys.len());
            keys.push(k.clone());
            flags.push(b.flags);
        }
    }
    let n = keys.len();

    // Collapsed leaves report the chip size instead of their (possibly
    // absent) measured size — matches what the caller substitutes into
    // `dims` before solving, so the gaps this module computes agree with
    // what actually gets rendered. `None` marks a leaf whose size is
    // genuinely unknown; any constraint naming it drops (Rule 7).
    let leaf_sizes: Vec<Option<Size>> = (0..n)
        .map(|i| {
            if flags[i].collapsed {
                Some(cfg.chip)
            } else {
                sizes.get(&keys[i]).copied()
            }
        })
        .collect();

    // Rule 2: groups + inline groups (recursive descendant-leaf walk).
    //
    // Two cases are filtered out of `group_specs`/`group_meta` (cohesion +
    // hull), matching the editor's old `flatten_groups`:
    //   - the implicit wrapper the parser synthesizes for a diagram's flat
    //     `## Members` list (unnamed, and — by construction — never has a
    //     nested `### ` subgroup child): not an authored cluster, and
    //     including it would give every groupless diagram whole-diagram
    //     cohesion plus a phantom whole-canvas hull.
    //   - a group whose resolved membership has no geometry (every member
    //     key either doesn't resolve to a leaf box at all, or resolves to one
    //     with no measured size): its hull would be a degenerate `Rect` at
    //     the origin, a phantom route obstacle. A leaf box gets created for
    //     ANY declared member key regardless of whether it names a real sized
    //     node (`resolve::add_group` doesn't check), so "resolved" here means
    //     "has an entry in `leaf_sizes`", matching the old `flatten_groups`'
    //     `collect_members` filtering against the sizes-derived `index`.
    let mut group_specs: Vec<GroupSpec> = Vec::new();
    let mut group_meta: Vec<(Option<String>, u8, Shape)> = Vec::new();
    let mut inline_specs: Vec<GroupSpec> = Vec::new();
    let mut group_members_by_id: BTreeMap<BoxId, Vec<usize>> = BTreeMap::new();
    for b in &scene.boxes {
        match &b.id {
            BoxId::Group(_) => {
                let mut members = Vec::new();
                descendant_leaves(&b.id, &boxes_by_id, &leaf_index, &mut members);
                group_members_by_id.insert(b.id.clone(), members.clone());
                let is_trivial_wrapper =
                    b.title.is_none() && !b.children.iter().any(|c| matches!(c, BoxId::Group(_)));
                let sized_members: Vec<usize> = members
                    .iter()
                    .copied()
                    .filter(|&m| leaf_sizes[m].is_some())
                    .collect();
                if !is_trivial_wrapper && !sized_members.is_empty() {
                    group_specs.push(GroupSpec {
                        members: sized_members,
                        depth: b.depth,
                    });
                    group_meta.push((b.title.clone(), b.depth, b.shape));
                }
            }
            BoxId::Inline(_) => {
                let mut members = Vec::new();
                descendant_leaves(&b.id, &boxes_by_id, &leaf_index, &mut members);
                group_members_by_id.insert(b.id.clone(), members.clone());
                inline_specs.push(GroupSpec {
                    members,
                    depth: b.depth,
                });
            }
            BoxId::Node(_) => {}
        }
    }

    // Rule 3: axis chains compile exactly like authored constraints; scanned
    // first so index order stays deterministic (scene.boxes, then
    // scene.constraints — the split between the two lists never affects
    // correctness, only which index a given sep lands at).
    let mut all: Vec<Constraint> = Vec::new();
    for b in &scene.boxes {
        all.extend(axis_constraints(b));
    }
    all.extend(scene.constraints.iter().cloned());

    let hull_pad = StressConfig::default().hull_pad;
    let mut seps = SepSpecs::default();
    let mut provenance_x: Vec<Option<Constraint>> = Vec::new();
    let mut provenance_y: Vec<Option<Constraint>> = Vec::new();
    let mut dropped: Vec<DroppedPlacement> = Vec::new();
    let mut boundary_vars: BTreeMap<(BoxId, u8), (usize, usize)> = BTreeMap::new();
    let mut next_extra = n;

    let margin_of = |id: &BoxId| -> Margin {
        boxes_by_id
            .get(id)
            .map(|b| b.margin)
            .unwrap_or(Margin::Medium)
    };
    let resolve_endpoint = |id: &BoxId| -> Option<Endpoint> {
        if let Some(&idx) = leaf_index.get(id) {
            return leaf_sizes[idx].map(|size| Endpoint::Leaf { idx, size });
        }
        if boxes_by_id.contains_key(id) {
            return Some(Endpoint::Group(id.clone()));
        }
        None
    };

    for c in &all {
        match c {
            Constraint::Place { a, b, dir } => {
                let (Some(ea), Some(eb)) = (resolve_endpoint(a), resolve_endpoint(b)) else {
                    dropped.push(DroppedPlacement {
                        relation: c.clone(),
                        conflicts_with: Vec::new(),
                    });
                    continue;
                };
                let dir_is_horizontal = matches!(dir, Direction::LeftOf | Direction::RightOf);
                let sa = match &ea {
                    Endpoint::Leaf { size, .. } => *size,
                    Endpoint::Group(_) => Size { w: 0.0, h: 0.0 },
                };
                let sb = match &eb {
                    Endpoint::Leaf { size, .. } => *size,
                    Endpoint::Group(_) => Size { w: 0.0, h: 0.0 },
                };
                let base_gap = pair_gap(
                    a,
                    b,
                    (sa, margin_of(a)),
                    (sb, margin_of(b)),
                    dir_is_horizontal,
                    connected,
                    label_widths,
                    cfg,
                );
                for (axis_x, a_is_left) in axis_components(*dir) {
                    let (left_ep, right_ep) = if a_is_left { (&ea, &eb) } else { (&eb, &ea) };
                    let (var_left, width_left) = match left_ep {
                        Endpoint::Leaf { idx, size } => {
                            (*idx, if axis_x { size.w } else { size.h })
                        }
                        Endpoint::Group(gid) => {
                            let (_l, r) = boundary_pair(
                                gid,
                                axis_x,
                                &mut boundary_vars,
                                &mut next_extra,
                                &group_members_by_id,
                                &leaf_sizes,
                                hull_pad,
                                &mut seps,
                                &mut provenance_x,
                                &mut provenance_y,
                                c,
                            );
                            (r, 0.0)
                        }
                    };
                    let var_right = match right_ep {
                        Endpoint::Leaf { idx, .. } => *idx,
                        Endpoint::Group(gid) => {
                            let (l, _r) = boundary_pair(
                                gid,
                                axis_x,
                                &mut boundary_vars,
                                &mut next_extra,
                                &group_members_by_id,
                                &leaf_sizes,
                                hull_pad,
                                &mut seps,
                                &mut provenance_x,
                                &mut provenance_y,
                                c,
                            );
                            l
                        }
                    };
                    let sep = Sep {
                        left: var_left,
                        right: var_right,
                        gap: base_gap + width_left,
                        equality: false,
                    };
                    if axis_x {
                        seps.x.push(sep);
                        provenance_x.push(Some(c.clone()));
                    } else {
                        seps.y.push(sep);
                        provenance_y.push(Some(c.clone()));
                    }
                }
            }
            Constraint::Align {
                a,
                a_edge,
                b,
                b_edge,
            } => {
                let (Some(ea), Some(eb)) = (resolve_endpoint(a), resolve_endpoint(b)) else {
                    dropped.push(DroppedPlacement {
                        relation: c.clone(),
                        conflicts_with: Vec::new(),
                    });
                    continue;
                };
                let (ax, ay) = edge_axes(*a_edge);
                let (bx, by) = edge_axes(*b_edge);
                // Per-axis (var, offset) for one endpoint: a leaf aligns at
                // `min corner + off(edge)`; a group aligns at its boundary-var
                // proxy (Rule 6: Lg/Rg on x, Tg/Bg on y). A group CENTER has
                // no proxy -- (Lg+Rg)/2 is not a single VPSC variable -- so
                // it returns None and the constraint stays a recorded
                // compile-time drop. Checked BEFORE `boundary_pair` so a
                // dropped align never leaks half-allocated boundary vars.
                let representable = |ep: &Endpoint, edge: Edge| -> bool {
                    matches!(ep, Endpoint::Leaf { .. })
                        || matches!(edge, Edge::Left | Edge::Top | Edge::Right | Edge::Bottom)
                };
                let mut shared = false;
                for axis_x in [true, false] {
                    let covered = if axis_x { ax && bx } else { ay && by };
                    if !covered || !representable(&ea, *a_edge) || !representable(&eb, *b_edge) {
                        continue;
                    }
                    let mut var_of = |ep: &Endpoint, edge: Edge| -> Option<(usize, f64)> {
                        match ep {
                            Endpoint::Leaf { idx, size } => {
                                let off = if axis_x {
                                    off_x(edge, size.w)
                                } else {
                                    off_y(edge, size.h)
                                };
                                Some((*idx, off))
                            }
                            Endpoint::Group(gid) => {
                                let want_min = matches!(edge, Edge::Left | Edge::Top);
                                let want_max = matches!(edge, Edge::Right | Edge::Bottom);
                                if !want_min && !want_max {
                                    return None;
                                }
                                let (l, r) = boundary_pair(
                                    gid,
                                    axis_x,
                                    &mut boundary_vars,
                                    &mut next_extra,
                                    &group_members_by_id,
                                    &leaf_sizes,
                                    hull_pad,
                                    &mut seps,
                                    &mut provenance_x,
                                    &mut provenance_y,
                                    c,
                                );
                                Some((if want_min { l } else { r }, 0.0))
                            }
                        }
                    };
                    let (Some((va, off_a)), Some((vb, off_b))) =
                        (var_of(&ea, *a_edge), var_of(&eb, *b_edge))
                    else {
                        continue;
                    };
                    let sep = Sep {
                        left: va,
                        right: vb,
                        gap: off_a - off_b,
                        equality: true,
                    };
                    if axis_x {
                        seps.x.push(sep);
                        provenance_x.push(Some(c.clone()));
                    } else {
                        seps.y.push(sep);
                        provenance_y.push(Some(c.clone()));
                    }
                    shared = true;
                }
                if !shared {
                    dropped.push(DroppedPlacement {
                        relation: c.clone(),
                        conflicts_with: Vec::new(),
                    });
                }
            }
        }
    }
    seps.extra_vars = next_extra - n;

    Compiled {
        keys,
        group_specs,
        group_meta,
        inline_specs,
        seps,
        flags,
        dropped,
        provenance_x,
        provenance_y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{Axis, Edge, Margin, Shape};

    fn leaf(k: &str) -> Box {
        Box {
            id: BoxId::Node(k.into()),
            kind: super::super::BoxKind::Leaf,
            children: vec![],
            axis: None,
            shape: Shape::Shrink,
            margin: Margin::Medium,
            flags: FlagSet::default(),
            title: None,
            depth: 0,
        }
    }

    fn collapsed_leaf(k: &str) -> Box {
        let mut b = leaf(k);
        b.flags.collapsed = true;
        b
    }

    fn group(id: u32, children: Vec<BoxId>, axis: Option<Axis>, title: &str) -> Box {
        Box {
            id: BoxId::Group(id),
            kind: super::super::BoxKind::Group,
            children,
            axis,
            shape: Shape::Frame,
            margin: Margin::Medium,
            flags: FlagSet::default(),
            title: Some(title.into()),
            depth: 0,
        }
    }

    fn sizes(pairs: &[(&str, f64, f64)]) -> BTreeMap<String, Size> {
        pairs
            .iter()
            .map(|(k, w, h)| ((*k).to_string(), Size { w: *w, h: *h }))
            .collect()
    }

    fn place(a: &str, b: &str, dir: Direction) -> Constraint {
        Constraint::Place {
            a: BoxId::Node(a.into()),
            b: BoxId::Node(b.into()),
            dir,
        }
    }

    #[test]
    fn place_left_of_compiles_to_an_x_sep_with_the_connected_gap() {
        let scene = Scene {
            boxes: vec![leaf("a"), leaf("b")],
            constraints: vec![place("a", "b", Direction::LeftOf)],
        };
        let sz = sizes(&[("a", 100.0, 50.0), ("b", 100.0, 50.0)]);
        let mut connected = BTreeSet::new();
        connected.insert(super::super::geometry::pair(
            &BoxId::Node("a".into()),
            &BoxId::Node("b".into()),
        ));
        let compiled = compile(
            &scene,
            &sz,
            &BTreeMap::new(),
            &connected,
            &SolveConfig::default(),
        );
        assert_eq!(compiled.seps.x.len(), 1);
        assert!(compiled.seps.y.is_empty());
        let sep = compiled.seps.x[0];
        assert_eq!(sep.left, 0);
        assert_eq!(sep.right, 1);
        assert!((sep.gap - 172.0).abs() < 1e-9, "gap = {}", sep.gap);
        assert!(!sep.equality);
        assert_eq!(
            compiled.provenance_x[0].as_ref(),
            Some(&scene.constraints[0])
        );
        assert!(compiled.dropped.is_empty());
    }

    #[test]
    fn place_above_right_emits_one_sep_per_axis() {
        // a AboveRight b: b is left of a (x), a is above b (y). Unconnected
        // pair, so the additive base gap is `min_sep` = 40 on BOTH axes
        // (place_deltas' "same gap spent on both axes" — the size added on
        // top of it differs per axis because a's height and b's width do).
        let scene = Scene {
            boxes: vec![leaf("a"), leaf("b")],
            constraints: vec![place("a", "b", Direction::AboveRight)],
        };
        let sz = sizes(&[("a", 100.0, 50.0), ("b", 80.0, 60.0)]);
        let compiled = compile(
            &scene,
            &sz,
            &BTreeMap::new(),
            &BTreeSet::new(),
            &SolveConfig::default(),
        );
        assert_eq!(compiled.seps.x.len(), 1, "one x-sep");
        assert_eq!(compiled.seps.y.len(), 1, "one y-sep");
        let xsep = compiled.seps.x[0];
        let ysep = compiled.seps.y[0];
        assert_eq!((xsep.left, xsep.right), (1, 0), "x-sep: b -> a");
        assert_eq!((ysep.left, ysep.right), (0, 1), "y-sep: a -> b");
        assert!(
            (xsep.gap - (40.0 + 80.0)).abs() < 1e-9,
            "x gap = {}",
            xsep.gap
        );
        assert!(
            (ysep.gap - (40.0 + 50.0)).abs() < 1e-9,
            "y gap = {}",
            ysep.gap
        );
    }

    #[test]
    fn align_left_edges_compiles_to_a_zero_gap_equality() {
        let align = |a_edge, b_edge| Constraint::Align {
            a: BoxId::Node("a".into()),
            a_edge,
            b: BoxId::Node("b".into()),
            b_edge,
        };
        let sz = sizes(&[("a", 100.0, 50.0), ("b", 100.0, 50.0)]);

        let scene = Scene {
            boxes: vec![leaf("a"), leaf("b")],
            constraints: vec![align(Edge::Left, Edge::Left)],
        };
        let compiled = compile(
            &scene,
            &sz,
            &BTreeMap::new(),
            &BTreeSet::new(),
            &SolveConfig::default(),
        );
        assert_eq!(compiled.seps.x.len(), 1);
        assert!(compiled.seps.y.is_empty());
        let sep = compiled.seps.x[0];
        assert!(sep.equality);
        assert!(sep.gap.abs() < 1e-9);
        assert!(compiled.dropped.is_empty());

        // Center (a, 100 wide) vs Left (b): gap = 50.0 - 0.0.
        let scene2 = Scene {
            boxes: vec![leaf("a"), leaf("b")],
            constraints: vec![align(Edge::Center, Edge::Left)],
        };
        let compiled2 = compile(
            &scene2,
            &sz,
            &BTreeMap::new(),
            &BTreeSet::new(),
            &SolveConfig::default(),
        );
        assert_eq!(compiled2.seps.x.len(), 1);
        assert!((compiled2.seps.x[0].gap - 50.0).abs() < 1e-9);
    }

    #[test]
    fn align_top_to_left_is_dropped_no_shared_axis() {
        let scene = Scene {
            boxes: vec![leaf("a"), leaf("b")],
            constraints: vec![Constraint::Align {
                a: BoxId::Node("a".into()),
                a_edge: Edge::Top,
                b: BoxId::Node("b".into()),
                b_edge: Edge::Left,
            }],
        };
        let sz = sizes(&[("a", 100.0, 50.0), ("b", 100.0, 50.0)]);
        let compiled = compile(
            &scene,
            &sz,
            &BTreeMap::new(),
            &BTreeSet::new(),
            &SolveConfig::default(),
        );
        assert!(compiled.seps.x.is_empty());
        assert!(compiled.seps.y.is_empty());
        assert_eq!(compiled.dropped.len(), 1);
    }

    #[test]
    fn row_axis_group_chains_left_of_seps() {
        let children = vec![
            BoxId::Node("a".into()),
            BoxId::Node("b".into()),
            BoxId::Node("c".into()),
        ];
        let scene = Scene {
            boxes: vec![
                leaf("a"),
                leaf("b"),
                leaf("c"),
                group(1, children, Some(Axis::Row), "g"),
            ],
            constraints: vec![],
        };
        let sz = sizes(&[("a", 100.0, 50.0), ("b", 100.0, 50.0), ("c", 100.0, 50.0)]);
        let compiled = compile(
            &scene,
            &sz,
            &BTreeMap::new(),
            &BTreeSet::new(),
            &SolveConfig::default(),
        );
        assert_eq!(compiled.seps.x.len(), 2, "two adjacent pairs");
        assert!(compiled.seps.y.is_empty());
        assert_eq!((compiled.seps.x[0].left, compiled.seps.x[0].right), (0, 1));
        assert_eq!((compiled.seps.x[1].left, compiled.seps.x[1].right), (1, 2));
    }

    #[test]
    fn cross_group_node_constraint_is_kept_not_dropped() {
        let scene = Scene {
            boxes: vec![
                leaf("a"),
                leaf("b"),
                group(1, vec![BoxId::Node("a".into())], None, "g1"),
                group(2, vec![BoxId::Node("b".into())], None, "g2"),
            ],
            constraints: vec![place("a", "b", Direction::LeftOf)],
        };
        let sz = sizes(&[("a", 100.0, 50.0), ("b", 100.0, 50.0)]);
        let compiled = compile(
            &scene,
            &sz,
            &BTreeMap::new(),
            &BTreeSet::new(),
            &SolveConfig::default(),
        );
        assert_eq!(
            compiled.seps.x.len(),
            1,
            "non-sibling constraint is honored, not dropped"
        );
        assert!(compiled.dropped.is_empty());
    }

    #[test]
    fn group_operand_allocates_boundary_vars_with_containment() {
        let scene = Scene {
            boxes: vec![
                leaf("m1"),
                leaf("m2"),
                leaf("n"),
                group(
                    1,
                    vec![BoxId::Node("m1".into()), BoxId::Node("m2".into())],
                    None,
                    "g",
                ),
            ],
            constraints: vec![Constraint::Place {
                a: BoxId::Group(1),
                b: BoxId::Node("n".into()),
                dir: Direction::LeftOf,
            }],
        };
        let sz = sizes(&[("m1", 100.0, 50.0), ("m2", 100.0, 50.0), ("n", 100.0, 50.0)]);
        let compiled = compile(
            &scene,
            &sz,
            &BTreeMap::new(),
            &BTreeSet::new(),
            &SolveConfig::default(),
        );
        assert_eq!(
            compiled.seps.extra_vars, 2,
            "boundary pair allocated on x only"
        );
        assert_eq!(
            compiled.seps.x.len(),
            5,
            "2 containment seps per member + 1 relation sep"
        );
        assert!(
            compiled.seps.y.is_empty(),
            "y untouched -- the constraint never touches it"
        );
        for p in &compiled.provenance_x {
            assert_eq!(
                p.as_ref(),
                Some(&scene.constraints[0]),
                "every emitted x-sep traces to the Place"
            );
        }
    }

    #[test]
    fn align_group_edge_to_node_compiles_via_boundary_vars() {
        // Task 4 Rule 6: a group operand in an `Align` compiles against its
        // boundary-var proxies (Lg/Rg on x, Tg/Bg on y) instead of dropping
        // -- the old authored path honored group-vs-sibling alignment.
        let scene = Scene {
            boxes: vec![
                leaf("m1"),
                leaf("m2"),
                leaf("n"),
                group(
                    1,
                    vec![BoxId::Node("m1".into()), BoxId::Node("m2".into())],
                    None,
                    "g",
                ),
            ],
            constraints: vec![Constraint::Align {
                a: BoxId::Group(1),
                a_edge: Edge::Left,
                b: BoxId::Node("n".into()),
                b_edge: Edge::Left,
            }],
        };
        let sz = sizes(&[("m1", 100.0, 50.0), ("m2", 100.0, 50.0), ("n", 100.0, 50.0)]);
        let compiled = compile(
            &scene,
            &sz,
            &BTreeMap::new(),
            &BTreeSet::new(),
            &SolveConfig::default(),
        );
        assert!(
            compiled.dropped.is_empty(),
            "group-edge align must be honored, not dropped: {:?}",
            compiled.dropped
        );
        assert_eq!(
            compiled.seps.extra_vars, 2,
            "x boundary pair only -- the constraint never touches y"
        );
        assert_eq!(
            compiled.seps.x.len(),
            5,
            "2 containment seps per member + 1 alignment equality"
        );
        assert!(compiled.seps.y.is_empty());
        let eq = compiled.seps.x[4];
        assert!(eq.equality, "the relation sep is an equality");
        assert_eq!(
            (eq.left, eq.right),
            (3, 2),
            "Lg proxy (first extra var) aligns against the node"
        );
        assert!(eq.gap.abs() < 1e-9);
        for p in &compiled.provenance_x {
            assert_eq!(
                p.as_ref(),
                Some(&scene.constraints[0]),
                "every emitted x-sep traces to the Align"
            );
        }
    }

    #[test]
    fn align_group_center_is_recorded_as_dropped_without_allocating() {
        // A group's center is (Lg+Rg)/2 -- not a single VPSC variable, so no
        // boundary proxy exists for it (Rule 6 names only Lg/Rg/Tg/Bg). It
        // stays a compile-time drop, recorded so the conflict list hears it,
        // and must not leak half-allocated boundary vars.
        let scene = Scene {
            boxes: vec![
                leaf("m1"),
                leaf("n"),
                group(1, vec![BoxId::Node("m1".into())], None, "g"),
            ],
            constraints: vec![Constraint::Align {
                a: BoxId::Group(1),
                a_edge: Edge::Center,
                b: BoxId::Node("n".into()),
                b_edge: Edge::Center,
            }],
        };
        let sz = sizes(&[("m1", 100.0, 50.0), ("n", 100.0, 50.0)]);
        let compiled = compile(
            &scene,
            &sz,
            &BTreeMap::new(),
            &BTreeSet::new(),
            &SolveConfig::default(),
        );
        assert_eq!(compiled.dropped.len(), 1);
        assert!(compiled.seps.x.is_empty());
        assert!(compiled.seps.y.is_empty());
        assert_eq!(compiled.seps.extra_vars, 0);
    }

    #[test]
    fn unknown_operand_is_reported_not_panicking() {
        let scene = Scene {
            boxes: vec![leaf("a")],
            constraints: vec![place("a", "ghost", Direction::LeftOf)],
        };
        let sz = sizes(&[("a", 100.0, 50.0)]);
        let compiled = compile(
            &scene,
            &sz,
            &BTreeMap::new(),
            &BTreeSet::new(),
            &SolveConfig::default(),
        );
        assert!(compiled.seps.x.is_empty());
        assert!(compiled.seps.y.is_empty());
        assert_eq!(compiled.dropped.len(), 1);
    }

    #[test]
    fn compile_is_deterministic() {
        let scene = Scene {
            boxes: vec![
                leaf("a"),
                leaf("b"),
                leaf("c"),
                group(
                    1,
                    vec![BoxId::Node("a".into()), BoxId::Node("b".into())],
                    None,
                    "g",
                ),
            ],
            constraints: vec![
                place("a", "b", Direction::LeftOf),
                Constraint::Place {
                    a: BoxId::Group(1),
                    b: BoxId::Node("c".into()),
                    dir: Direction::Above,
                },
            ],
        };
        let sz = sizes(&[("a", 100.0, 50.0), ("b", 100.0, 50.0), ("c", 100.0, 50.0)]);
        let one = compile(
            &scene,
            &sz,
            &BTreeMap::new(),
            &BTreeSet::new(),
            &SolveConfig::default(),
        );
        let two = compile(
            &scene,
            &sz,
            &BTreeMap::new(),
            &BTreeSet::new(),
            &SolveConfig::default(),
        );
        assert_eq!(one, two);
    }

    #[test]
    fn collapsed_leaf_reports_its_flag() {
        let scene = Scene {
            boxes: vec![collapsed_leaf("a"), leaf("b")],
            constraints: vec![place("a", "b", Direction::LeftOf)],
        };
        // "a" deliberately has no entry in `sizes` -- collapsed leaves report
        // the chip size instead, so the Place still compiles.
        let sz = sizes(&[("b", 100.0, 50.0)]);
        let compiled = compile(
            &scene,
            &sz,
            &BTreeMap::new(),
            &BTreeSet::new(),
            &SolveConfig::default(),
        );
        assert!(compiled.flags[0].collapsed);
        assert!(!compiled.flags[1].collapsed);
        assert!(
            compiled.dropped.is_empty(),
            "collapsed leaf still has an effective size"
        );
        assert_eq!(compiled.seps.x.len(), 1);
        let cfg = SolveConfig::default();
        let expected_gap = cfg.margin(Margin::Medium).max(cfg.min_sep) + cfg.chip.w;
        assert!((compiled.seps.x[0].gap - expected_gap).abs() < 1e-9);
    }

    #[test]
    fn trivial_unnamed_childless_group_is_excluded_from_cohesion_and_hulls() {
        // Mirrors the parser's implicit wrapper for a flat `## Members` list:
        // unnamed, no nested `### ` subgroup children. Not an authored
        // cluster -- must not become whole-diagram cohesion + a phantom hull.
        let scene = Scene {
            boxes: vec![
                leaf("a"),
                leaf("b"),
                Box {
                    id: BoxId::Group(1),
                    kind: super::super::BoxKind::Group,
                    children: vec![BoxId::Node("a".into()), BoxId::Node("b".into())],
                    axis: None,
                    shape: Shape::Shrink,
                    margin: Margin::Medium,
                    flags: FlagSet::default(),
                    title: None,
                    depth: 0,
                },
            ],
            constraints: vec![],
        };
        let sz = sizes(&[("a", 100.0, 50.0), ("b", 100.0, 50.0)]);
        let compiled = compile(
            &scene,
            &sz,
            &BTreeMap::new(),
            &BTreeSet::new(),
            &SolveConfig::default(),
        );
        assert!(compiled.group_specs.is_empty());
        assert!(compiled.group_meta.is_empty());
    }

    #[test]
    fn memberless_named_group_is_excluded() {
        // Every member key fails to resolve (no leaf box at all) -> its hull
        // would be a degenerate zero-size rect at the origin.
        let scene = Scene {
            boxes: vec![group(1, vec![BoxId::Node("ghost".into())], None, "Ghost")],
            constraints: vec![],
        };
        let compiled = compile(
            &scene,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeSet::new(),
            &SolveConfig::default(),
        );
        assert!(compiled.group_specs.is_empty());
        assert!(compiled.group_meta.is_empty());
    }

    #[test]
    fn group_with_only_sizeless_members_is_excluded() {
        // The member key resolves to a real leaf BOX -- a declared group
        // membership always creates one, `resolve::add_group` never checks
        // `sizes` -- but has no entry in `sizes`: no geometry, so the hull
        // would still be a degenerate zero-size rect at the origin.
        let scene = Scene {
            boxes: vec![
                leaf("not-a-node"),
                group(1, vec![BoxId::Node("not-a-node".into())], None, "Ghost"),
            ],
            constraints: vec![],
        };
        let compiled = compile(
            &scene,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeSet::new(),
            &SolveConfig::default(),
        );
        assert!(compiled.group_specs.is_empty());
        assert!(compiled.group_meta.is_empty());
    }
}
