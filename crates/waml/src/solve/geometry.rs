//! Layer B: solve a `Scene` into absolute rectangles via weighted union-find.

use super::potentials::Potentials;
use super::{
    Box, BoxId, BoxKind, Constraint, Rect, Scene, Size, SizeMap, SolveConfig, Solved, SolvedGroup,
};
use crate::diagnostic::{DiagCode, Diagnostic};
use crate::syntax::{Axis, Direction, Edge, Margin, Shape};
use std::collections::{BTreeMap, BTreeSet};

/// Minimum facing-border gap between two edge-connected boxes, so the connector
/// between them can carry arrowheads and a short label. Floors the `Place` gap
/// for connected pairs; unconnected neighbours keep the plain margin gap.
const MIN_ASSOC: f64 = 72.0;

/// Order-independent key for an unordered box pair.
fn pair(a: &BoxId, b: &BoxId) -> (BoxId, BoxId) {
    if a <= b {
        (a.clone(), b.clone())
    } else {
        (b.clone(), a.clone())
    }
}

fn margin_rank(m: Margin) -> u8 {
    match m {
        Margin::No => 0,
        Margin::Small => 1,
        Margin::Medium => 2,
        Margin::Large => 3,
    }
}
fn max_margin(a: Margin, b: Margin) -> Margin {
    if margin_rank(a) >= margin_rank(b) {
        a
    } else {
        b
    }
}

/// Which axes an alignment edge constrains: (x, y).
fn edge_axes(e: Edge) -> (bool, bool) {
    match e {
        Edge::Left | Edge::Right => (true, false),
        Edge::Top | Edge::Bottom => (false, true),
        Edge::Center => (true, true),
    }
}
fn off_x(e: Edge, w: f64) -> f64 {
    match e {
        Edge::Left => 0.0,
        Edge::Right => w,
        Edge::Center => w / 2.0,
        _ => 0.0,
    }
}
fn off_y(e: Edge, h: f64) -> f64 {
    match e {
        Edge::Top => 0.0,
        Edge::Bottom => h,
        Edge::Center => h / 2.0,
        _ => 0.0,
    }
}

fn eq(p: &mut Potentials, a: usize, b: usize, delta: f64, diags: &mut Vec<Diagnostic>) {
    if p.union(a, b, delta).is_err() {
        diags.push(Diagnostic::warn(
            DiagCode::LayoutConflict,
            "conflicting layout constraint dropped",
            "",
            0,
        ));
    }
}

/// Position a flat set of boxes (given size + margin per id) under a constraint
/// list. Returns one absolute `Rect` per input id.
pub(super) fn solve_cluster(
    ids: &[BoxId],
    dims: &BTreeMap<BoxId, (Size, Margin)>,
    constraints: &[Constraint],
    connected: &BTreeSet<(BoxId, BoxId)>,
    cfg: &SolveConfig,
    diags: &mut Vec<Diagnostic>,
) -> BTreeMap<BoxId, Rect> {
    let n = ids.len();
    let index: BTreeMap<BoxId, usize> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.clone(), i))
        .collect();
    let mut px = Potentials::new(n);
    let mut py = Potentials::new(n);

    for c in constraints {
        match c {
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
                match dir {
                    Direction::LeftOf => {
                        eq(&mut px, ia, ib, sa.w + gap, diags);
                        eq(&mut py, ia, ib, (sa.h - sb.h) / 2.0, diags);
                    }
                    Direction::RightOf => {
                        eq(&mut px, ia, ib, -(sb.w + gap), diags);
                        eq(&mut py, ia, ib, (sa.h - sb.h) / 2.0, diags);
                    }
                    Direction::Above => {
                        eq(&mut py, ia, ib, sa.h + gap, diags);
                        eq(&mut px, ia, ib, (sa.w - sb.w) / 2.0, diags);
                    }
                    Direction::Below => {
                        eq(&mut py, ia, ib, -(sb.h + gap), diags);
                        eq(&mut px, ia, ib, (sa.w - sb.w) / 2.0, diags);
                    }
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
                }
            }
            Constraint::Align {
                a,
                a_edge,
                b,
                b_edge,
            } => {
                let (Some(&ia), Some(&ib)) = (index.get(a), index.get(b)) else {
                    continue;
                };
                let (sa, _) = dims[a];
                let (sb, _) = dims[b];
                let (ax, ay) = edge_axes(*a_edge);
                let (bx, by) = edge_axes(*b_edge);
                let mut shared = false;
                if ax && bx {
                    eq(
                        &mut px,
                        ia,
                        ib,
                        off_x(*a_edge, sa.w) - off_x(*b_edge, sb.w),
                        diags,
                    );
                    shared = true;
                }
                if ay && by {
                    eq(
                        &mut py,
                        ia,
                        ib,
                        off_y(*a_edge, sa.h) - off_y(*b_edge, sb.h),
                        diags,
                    );
                    shared = true;
                }
                if !shared {
                    diags.push(Diagnostic::warn(
                        DiagCode::LayoutConflict,
                        "alignment edges share no axis",
                        "",
                        0,
                    ));
                }
            }
        }
    }

    // Resolve relative coordinates + roots per axis.
    let mut relx = vec![0.0; n];
    let mut rootx = vec![0usize; n];
    let mut rely = vec![0.0; n];
    let mut rooty = vec![0usize; n];
    for i in 0..n {
        let (rx, dx) = px.find(i);
        let (ry, dy) = py.find(i);
        rootx[i] = rx;
        relx[i] = dx;
        rooty[i] = ry;
        rely[i] = dy;
    }
    let w_of = |i: usize| dims[&ids[i]].0.w;

    // X components packed left-to-right by first-member list order.
    let mut xcomps: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, &root) in rootx.iter().enumerate() {
        xcomps.entry(root).or_default().push(i);
    }
    let mut order: Vec<(usize, Vec<usize>)> = xcomps.into_iter().collect();
    order.sort_by_key(|(_, v)| *v.iter().min().unwrap());
    let gap = cfg.margin(Margin::Medium);
    let mut originx: BTreeMap<usize, f64> = BTreeMap::new();
    let mut cursor = 0.0;
    for (root, members) in &order {
        let minrel = members
            .iter()
            .map(|&i| relx[i])
            .fold(f64::INFINITY, f64::min);
        let maxend = members
            .iter()
            .map(|&i| relx[i] + w_of(i))
            .fold(f64::NEG_INFINITY, f64::max);
        originx.insert(*root, cursor - minrel);
        cursor += (maxend - minrel) + gap;
    }

    // Y components normalized so each top sits at 0 (shared band).
    let mut ycomps: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, &root) in rooty.iter().enumerate() {
        ycomps.entry(root).or_default().push(i);
    }
    let mut originy: BTreeMap<usize, f64> = BTreeMap::new();
    for (root, members) in &ycomps {
        let minrel = members
            .iter()
            .map(|&i| rely[i])
            .fold(f64::INFINITY, f64::min);
        originy.insert(*root, -minrel);
    }

    let mut out = BTreeMap::new();
    for i in 0..n {
        let (sz, _) = dims[&ids[i]];
        let x = originx[&rootx[i]] + relx[i];
        let y = originy[&rooty[i]] + rely[i];
        out.insert(
            ids[i].clone(),
            Rect {
                x,
                y,
                w: sz.w,
                h: sz.h,
            },
        );
    }
    out
}

/// A solved subtree in local coordinates: the box's outer size, every
/// descendant leaf rect, and every descendant group hull.
struct Laid {
    size: Size,
    rects: BTreeMap<BoxId, Rect>,
    groups: Vec<SolvedGroup>,
}

fn endpoints(c: &Constraint) -> (&BoxId, &BoxId) {
    match c {
        Constraint::Place { a, b, .. } => (a, b),
        Constraint::Align { a, b, .. } => (a, b),
    }
}

fn axis_constraints(b: &Box) -> Vec<Constraint> {
    let dir = match b.axis {
        Some(Axis::Row) => Direction::LeftOf,
        Some(Axis::Column) => Direction::Above,
        None => return vec![],
    };
    b.children
        .windows(2)
        .map(|w| Constraint::Place {
            a: w[0].clone(),
            b: w[1].clone(),
            dir,
        })
        .collect()
}

/// Union of child rects → (minX, minY, maxX, maxY). Empty → all zero.
fn bounds(rects: &BTreeMap<BoxId, Rect>, ids: &[BoxId]) -> (f64, f64, f64, f64) {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for id in ids {
        let r = rects[id];
        min_x = min_x.min(r.x);
        min_y = min_y.min(r.y);
        max_x = max_x.max(r.x + r.w);
        max_y = max_y.max(r.y + r.h);
    }
    if ids.is_empty() {
        (0.0, 0.0, 0.0, 0.0)
    } else {
        (min_x, min_y, max_x, max_y)
    }
}

/// Place a set of already-sized children under `cons`, then translate each
/// child's own subtree into this frame. `inset` pads the content; `hull`, if
/// given, appends this box's own group rectangle.
#[allow(clippy::too_many_arguments)]
fn assemble(
    children: &[BoxId],
    child_laid: &BTreeMap<BoxId, Laid>,
    child_margins: &BTreeMap<BoxId, Margin>,
    cons: &[Constraint],
    connected: &BTreeSet<(BoxId, BoxId)>,
    inset: f64,
    hull: Option<(Shape, Option<String>, u8)>,
    cfg: &SolveConfig,
    diags: &mut Vec<Diagnostic>,
) -> Laid {
    let mut dims: BTreeMap<BoxId, (Size, Margin)> = BTreeMap::new();
    for c in children {
        dims.insert(c.clone(), (child_laid[c].size, child_margins[c]));
    }
    let placed = solve_cluster(children, &dims, cons, connected, cfg, diags);
    let (min_x, min_y, max_x, max_y) = bounds(&placed, children);
    let dx = inset - min_x;
    let dy = inset - min_y;

    let mut rects = BTreeMap::new();
    let mut groups = Vec::new();
    for c in children {
        let pr = placed[c];
        let ox = pr.x + dx;
        let oy = pr.y + dy;
        let cl = &child_laid[c];
        for (k, r) in &cl.rects {
            rects.insert(
                k.clone(),
                Rect {
                    x: r.x + ox,
                    y: r.y + oy,
                    w: r.w,
                    h: r.h,
                },
            );
        }
        for g in &cl.groups {
            groups.push(SolvedGroup {
                rect: Rect {
                    x: g.rect.x + ox,
                    y: g.rect.y + oy,
                    w: g.rect.w,
                    h: g.rect.h,
                },
                shape: g.shape,
                title: g.title.clone(),
                depth: g.depth,
            });
        }
    }

    let outer = Size {
        w: (max_x - min_x) + 2.0 * inset,
        h: (max_y - min_y) + 2.0 * inset,
    };
    if let Some((shape, title, depth)) = hull {
        groups.push(SolvedGroup {
            rect: Rect {
                x: 0.0,
                y: 0.0,
                w: outer.w,
                h: outer.h,
            },
            shape,
            title,
            depth,
        });
    }
    Laid {
        size: outer,
        rects,
        groups,
    }
}

#[allow(clippy::too_many_arguments)]
fn solve_box(
    id: &BoxId,
    boxes: &BTreeMap<BoxId, &Box>,
    sizes: &SizeMap,
    cfg: &SolveConfig,
    cfor: &BTreeMap<Option<BoxId>, Vec<Constraint>>,
    connected: &BTreeSet<(BoxId, BoxId)>,
    diags: &mut Vec<Diagnostic>,
) -> Laid {
    let b = boxes[id];
    if b.kind == BoxKind::Leaf {
        let key = match id {
            BoxId::Node(k) => k.clone(),
            _ => String::new(),
        };
        let sz = if b.flags.collapsed {
            cfg.chip
        } else {
            sizes
                .get(&key)
                .copied()
                .unwrap_or(Size { w: 100.0, h: 40.0 })
        };
        let mut rects = BTreeMap::new();
        rects.insert(
            id.clone(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: sz.w,
                h: sz.h,
            },
        );
        return Laid {
            size: sz,
            rects,
            groups: vec![],
        };
    }

    let mut child_laid = BTreeMap::new();
    let mut child_margins = BTreeMap::new();
    for c in &b.children {
        child_laid.insert(
            c.clone(),
            solve_box(c, boxes, sizes, cfg, cfor, connected, diags),
        );
        child_margins.insert(c.clone(), boxes[c].margin);
    }
    let mut cons = axis_constraints(b);
    if let Some(list) = cfor.get(&Some(id.clone())) {
        cons.extend(list.iter().cloned());
    }
    let inset = cfg.margin(b.margin);
    let mut laid = assemble(
        &b.children,
        &child_laid,
        &child_margins,
        &cons,
        connected,
        inset,
        Some((b.shape, b.title.clone(), b.depth)),
        cfg,
        diags,
    );
    // Key this group's own frame by its BoxId so the router can use group rects
    // as containment-aware obstacles. Behavior-preserving: Solved.nodes only ever
    // extracts BoxId::Node entries, so group keys are invisible to existing output.
    laid.rects.insert(
        id.clone(),
        Rect {
            x: 0.0,
            y: 0.0,
            w: laid.size.w,
            h: laid.size.h,
        },
    );
    laid
}

pub fn solve(scene: &Scene, sizes: &SizeMap, cfg: &SolveConfig) -> (Solved, Vec<Diagnostic>) {
    let (solved, _rects, diags) = solve_with_rects(scene, &[], sizes, cfg);
    (solved, diags)
}

pub(super) fn solve_with_rects(
    scene: &Scene,
    edges: &[(BoxId, BoxId)],
    sizes: &SizeMap,
    cfg: &SolveConfig,
) -> (Solved, BTreeMap<BoxId, Rect>, Vec<Diagnostic>) {
    let mut diags = vec![];
    let boxes: BTreeMap<BoxId, &Box> = scene.boxes.iter().map(|b| (b.id.clone(), b)).collect();
    // Unordered node<->node edge-connected pairs, so the layout can floor the
    // `Place` gap between associated boxes. Group-as-endpoint edges are ignored.
    let mut connected: BTreeSet<(BoxId, BoxId)> = BTreeSet::new();
    for (a, b) in edges {
        if matches!(a, BoxId::Node(_)) && matches!(b, BoxId::Node(_)) {
            connected.insert(pair(a, b));
        }
    }
    // parent[child] = its group; roots have no parent.
    let mut parent: BTreeMap<BoxId, BoxId> = BTreeMap::new();
    for b in &scene.boxes {
        for c in &b.children {
            parent.insert(c.clone(), b.id.clone());
        }
    }
    let roots: Vec<BoxId> = scene
        .boxes
        .iter()
        .filter(|b| !parent.contains_key(&b.id))
        .map(|b| b.id.clone())
        .collect();

    // Assign each constraint to the cluster whose direct children are both
    // endpoints. Non-siblings warn and drop.
    let mut cfor: BTreeMap<Option<BoxId>, Vec<Constraint>> = BTreeMap::new();
    for c in &scene.constraints {
        let (a, b) = endpoints(c);
        let pa = parent.get(a).cloned();
        let pb = parent.get(b).cloned();
        if pa == pb {
            cfor.entry(pa).or_default().push(c.clone());
        } else {
            diags.push(Diagnostic::warn(
                DiagCode::LayoutConflict,
                "layout relates operands that are not siblings; dropped",
                "",
                0,
            ));
        }
    }

    // Solve every root subtree, then assemble the roots as a top-level clump
    // (no hull, no inset).
    let mut child_laid = BTreeMap::new();
    let mut child_margins = BTreeMap::new();
    for r in &roots {
        child_laid.insert(
            r.clone(),
            solve_box(r, &boxes, sizes, cfg, &cfor, &connected, &mut diags),
        );
        child_margins.insert(r.clone(), boxes[r].margin);
    }
    let root_cons = cfor.get(&None).cloned().unwrap_or_default();
    let laid = assemble(
        &roots,
        &child_laid,
        &child_margins,
        &root_cons,
        &connected,
        0.0,
        None,
        cfg,
        &mut diags,
    );

    let mut nodes = BTreeMap::new();
    for (id, r) in &laid.rects {
        if let BoxId::Node(key) = id {
            nodes.insert(key.clone(), *r);
        }
    }
    let mut groups = laid.groups;
    groups.sort_by(|a, b| {
        a.depth
            .cmp(&b.depth)
            .then(a.rect.x.total_cmp(&b.rect.x))
            .then(a.rect.y.total_cmp(&b.rect.y))
    });

    let mut flags = BTreeMap::new();
    for b in &scene.boxes {
        if let BoxId::Node(key) = &b.id {
            if b.flags.emphasized || b.flags.collapsed {
                flags.insert(key.clone(), b.flags);
            }
        }
    }

    (
        Solved {
            nodes,
            groups,
            flags,
            routes: Vec::new(),
        },
        laid.rects,
        diags,
    )
}

#[cfg(test)]
mod tests {
    use super::super::{pretty, FlagSet};
    use super::*;
    use crate::syntax::{Margin, Shape};

    fn leaf(k: &str) -> Box {
        Box {
            id: BoxId::Node(k.into()),
            kind: BoxKind::Leaf,
            children: vec![],
            axis: None,
            shape: Shape::Shrink,
            margin: Margin::Medium,
            flags: FlagSet::default(),
            title: None,
            depth: 0,
        }
    }

    fn sizes(keys: &[&str], w: f64, h: f64) -> SizeMap {
        let mut m = SizeMap::new();
        for k in keys {
            m.insert((*k).into(), Size { w, h });
        }
        m
    }

    #[test]
    fn solves_a_row_of_three() {
        let scene = Scene {
            boxes: vec![leaf("a"), leaf("b"), leaf("c")],
            constraints: vec![
                Constraint::Place {
                    a: BoxId::Node("a".into()),
                    b: BoxId::Node("b".into()),
                    dir: Direction::LeftOf,
                },
                Constraint::Place {
                    a: BoxId::Node("b".into()),
                    b: BoxId::Node("c".into()),
                    dir: Direction::LeftOf,
                },
            ],
        };
        let (solved, diags) = solve(
            &scene,
            &sizes(&["a", "b", "c"], 200.0, 90.0),
            &SolveConfig::default(),
        );
        assert!(diags.is_empty());
        assert_eq!(
            pretty(&solved),
            "node a @ 0,0 200x90\nnode b @ 216,0 200x90\nnode c @ 432,0 200x90\n"
        );
    }

    #[test]
    fn contradiction_warns_and_still_renders() {
        let scene = Scene {
            boxes: vec![leaf("a"), leaf("b")],
            constraints: vec![
                Constraint::Place {
                    a: BoxId::Node("a".into()),
                    b: BoxId::Node("b".into()),
                    dir: Direction::LeftOf,
                },
                Constraint::Place {
                    a: BoxId::Node("b".into()),
                    b: BoxId::Node("a".into()),
                    dir: Direction::LeftOf,
                },
            ],
        };
        let (solved, diags) = solve(
            &scene,
            &sizes(&["a", "b"], 200.0, 90.0),
            &SolveConfig::default(),
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, DiagCode::LayoutConflict);
        assert_eq!(solved.nodes.len(), 2, "always renders every node");
    }

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
        let a = &solved.nodes["a"];
        let b = &solved.nodes["b"];
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
        let a = &solved.nodes["a"];
        let b = &solved.nodes["b"];
        // a is right of b and below b.
        assert!(b.x + b.w <= a.x + 1e-6, "a not right of b: {a:?} {b:?}");
        assert!(b.y + b.h <= a.y + 1e-6, "a not below b: {a:?} {b:?}");
    }

    fn group(
        id: u32,
        children: Vec<BoxId>,
        axis: Option<crate::syntax::Axis>,
        shape: Shape,
        title: &str,
    ) -> Box {
        Box {
            id: BoxId::Group(id),
            kind: BoxKind::Group,
            children,
            axis,
            shape,
            margin: Margin::Medium,
            flags: FlagSet::default(),
            title: Some(title.into()),
            depth: 0,
        }
    }

    #[test]
    fn column_group_with_frame_wraps_members_with_margin() {
        use crate::syntax::Axis;
        let scene = Scene {
            boxes: vec![
                leaf("a"),
                leaf("b"),
                group(
                    0,
                    vec![BoxId::Node("a".into()), BoxId::Node("b".into())],
                    Some(Axis::Column),
                    Shape::Frame,
                    "Users",
                ),
            ],
            constraints: vec![],
        };
        let (solved, diags) = solve(
            &scene,
            &sizes(&["a", "b"], 200.0, 90.0),
            &SolveConfig::default(),
        );
        assert!(diags.is_empty());
        assert_eq!(
            pretty(&solved),
            "node a @ 16,16 200x90\n\
             node b @ 16,122 200x90\n\
             group Frame \"Users\" d0 @ 0,0 232x228\n"
        );
    }

    #[test]
    fn non_sibling_constraint_warns() {
        use crate::syntax::Axis;
        // `a` lives inside group Users; relating it to top-level `c` is not a sibling relation.
        let scene = Scene {
            boxes: vec![
                leaf("a"),
                leaf("c"),
                group(
                    0,
                    vec![BoxId::Node("a".into())],
                    Some(Axis::Column),
                    Shape::Shrink,
                    "Users",
                ),
            ],
            constraints: vec![Constraint::Place {
                a: BoxId::Node("a".into()),
                b: BoxId::Node("c".into()),
                dir: Direction::LeftOf,
            }],
        };
        let (solved, diags) = solve(
            &scene,
            &sizes(&["a", "c"], 200.0, 90.0),
            &SolveConfig::default(),
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, DiagCode::LayoutConflict);
        assert_eq!(solved.nodes.len(), 2, "still renders both nodes");
    }

    #[test]
    fn collapsed_uses_chip_size_and_flags_reported() {
        let mut a = leaf("a");
        a.flags.collapsed = true;
        let mut b = leaf("b");
        b.flags.emphasized = true;
        let scene = Scene {
            boxes: vec![a, b],
            constraints: vec![Constraint::Place {
                a: BoxId::Node("a".into()),
                b: BoxId::Node("b".into()),
                dir: Direction::LeftOf,
            }],
        };
        let (solved, diags) = solve(
            &scene,
            &sizes(&["a", "b"], 200.0, 90.0),
            &SolveConfig::default(),
        );
        assert!(diags.is_empty());
        // a collapses to the 96x28 chip; `a left of b` gaps 96+16=112 in x,
        // centers align in y: (28-90)/2 = -31, normalized so the band top is 0.
        assert_eq!(
            pretty(&solved),
            "node a @ 0,31 96x28\n\
             node b @ 112,0 200x90\n\
             flags a emphasized=false collapsed=true\n\
             flags b emphasized=true collapsed=false\n"
        );
    }

    #[test]
    fn solve_with_rects_keys_group_frames_by_boxid() {
        use crate::syntax::Axis;
        let scene = Scene {
            boxes: vec![
                leaf("a"),
                leaf("b"),
                group(
                    0,
                    vec![BoxId::Node("a".into()), BoxId::Node("b".into())],
                    Some(Axis::Column),
                    Shape::Frame,
                    "Users",
                ),
            ],
            constraints: vec![],
        };
        let (_solved, rects, diags) = solve_with_rects(
            &scene,
            &[],
            &sizes(&["a", "b"], 200.0, 90.0),
            &SolveConfig::default(),
        );
        assert!(diags.is_empty());
        // Leaf rects present.
        assert!(rects.contains_key(&BoxId::Node("a".into())));
        assert!(rects.contains_key(&BoxId::Node("b".into())));
        // The group frame is keyed by its BoxId and equals the "Users" frame @ 0,0 232x228.
        let g = rects[&BoxId::Group(0)];
        assert_eq!((g.x, g.y, g.w, g.h), (0.0, 0.0, 232.0, 228.0));
    }

    #[test]
    fn connected_adjacent_pair_gets_min_assoc_gap() {
        let scene = Scene {
            boxes: vec![leaf("a"), leaf("b")],
            constraints: vec![Constraint::Place {
                a: BoxId::Node("a".into()),
                b: BoxId::Node("b".into()),
                dir: Direction::LeftOf,
            }],
        };
        let edges = vec![(BoxId::Node("a".into()), BoxId::Node("b".into()))];
        let (_solved, rects, diags) = solve_with_rects(
            &scene,
            &edges,
            &sizes(&["a", "b"], 200.0, 90.0),
            &SolveConfig::default(),
        );
        assert!(diags.is_empty());
        // `a left of b` with an a<->b edge: facing-border gap is floored at MIN_ASSOC (72),
        // so b.x = a.w + 72 = 272 rather than the 216 (16px margin) of the unconnected case.
        let a = rects[&BoxId::Node("a".into())];
        let b = rects[&BoxId::Node("b".into())];
        let gap = b.x - (a.x + a.w);
        assert!(gap >= MIN_ASSOC, "gap {gap} should be >= {MIN_ASSOC}");
        assert_eq!(gap, MIN_ASSOC);
    }

    #[test]
    fn unconnected_adjacent_pair_keeps_margin_gap() {
        let scene = Scene {
            boxes: vec![leaf("a"), leaf("b")],
            constraints: vec![Constraint::Place {
                a: BoxId::Node("a".into()),
                b: BoxId::Node("b".into()),
                dir: Direction::LeftOf,
            }],
        };
        let (_solved, rects, diags) = solve_with_rects(
            &scene,
            &[],
            &sizes(&["a", "b"], 200.0, 90.0),
            &SolveConfig::default(),
        );
        assert!(diags.is_empty());
        // No edge: the gap stays at the Medium margin value (16).
        let a = rects[&BoxId::Node("a".into())];
        let b = rects[&BoxId::Node("b".into())];
        let gap = b.x - (a.x + a.w);
        assert_eq!(gap, SolveConfig::default().margin(Margin::Medium));
    }

    #[test]
    fn group_flow_connected_siblings_spread() {
        use crate::syntax::Axis;
        // Two column-flow siblings inside a frame, connected by an edge. The
        // group-axis-flow Place chain floors the vertical gap at MIN_ASSOC.
        let scene = Scene {
            boxes: vec![
                leaf("a"),
                leaf("b"),
                group(
                    0,
                    vec![BoxId::Node("a".into()), BoxId::Node("b".into())],
                    Some(Axis::Column),
                    Shape::Frame,
                    "Users",
                ),
            ],
            constraints: vec![],
        };
        let edges = vec![(BoxId::Node("a".into()), BoxId::Node("b".into()))];
        let (_solved, rects, diags) = solve_with_rects(
            &scene,
            &edges,
            &sizes(&["a", "b"], 200.0, 90.0),
            &SolveConfig::default(),
        );
        assert!(diags.is_empty());
        let a = rects[&BoxId::Node("a".into())];
        let b = rects[&BoxId::Node("b".into())];
        let gap = b.y - (a.y + a.h);
        assert_eq!(gap, MIN_ASSOC);
    }

    #[test]
    fn min_assoc_layout_is_deterministic() {
        use crate::syntax::Axis;
        let scene = Scene {
            boxes: vec![
                leaf("a"),
                leaf("b"),
                group(
                    0,
                    vec![BoxId::Node("a".into()), BoxId::Node("b".into())],
                    Some(Axis::Column),
                    Shape::Frame,
                    "Users",
                ),
            ],
            constraints: vec![],
        };
        let edges = vec![(BoxId::Node("a".into()), BoxId::Node("b".into()))];
        let run = || {
            solve_with_rects(
                &scene,
                &edges,
                &sizes(&["a", "b"], 200.0, 90.0),
                &SolveConfig::default(),
            )
            .1
        };
        assert_eq!(run(), run());
    }
}
