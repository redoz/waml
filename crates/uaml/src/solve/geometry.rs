//! Layer B: solve a `Scene` into absolute rectangles via weighted union-find.

use std::collections::BTreeMap;
use crate::diagnostic::{DiagCode, Diagnostic};
use crate::syntax::{Direction, Edge, Margin};
use super::potentials::Potentials;
use super::{Box, BoxId, BoxKind, Constraint, Rect, Scene, Size, SizeMap, SolveConfig, Solved};

fn margin_rank(m: Margin) -> u8 {
    match m { Margin::No => 0, Margin::Small => 1, Margin::Medium => 2, Margin::Large => 3 }
}
fn max_margin(a: Margin, b: Margin) -> Margin {
    if margin_rank(a) >= margin_rank(b) { a } else { b }
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
    match e { Edge::Left => 0.0, Edge::Right => w, Edge::Center => w / 2.0, _ => 0.0 }
}
fn off_y(e: Edge, h: f64) -> f64 {
    match e { Edge::Top => 0.0, Edge::Bottom => h, Edge::Center => h / 2.0, _ => 0.0 }
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
    cfg: &SolveConfig,
    diags: &mut Vec<Diagnostic>,
) -> BTreeMap<BoxId, Rect> {
    let n = ids.len();
    let index: BTreeMap<BoxId, usize> =
        ids.iter().enumerate().map(|(i, id)| (id.clone(), i)).collect();
    let mut px = Potentials::new(n);
    let mut py = Potentials::new(n);

    for c in constraints {
        match c {
            Constraint::Place { a, b, dir } => {
                let (Some(&ia), Some(&ib)) = (index.get(a), index.get(b)) else { continue };
                let (sa, ma) = dims[a];
                let (sb, mb) = dims[b];
                let gap = cfg.margin(max_margin(ma, mb));
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
                }
            }
            Constraint::Align { a, a_edge, b, b_edge } => {
                let (Some(&ia), Some(&ib)) = (index.get(a), index.get(b)) else { continue };
                let (sa, _) = dims[a];
                let (sb, _) = dims[b];
                let (ax, ay) = edge_axes(*a_edge);
                let (bx, by) = edge_axes(*b_edge);
                let mut shared = false;
                if ax && bx {
                    eq(&mut px, ia, ib, off_x(*a_edge, sa.w) - off_x(*b_edge, sb.w), diags);
                    shared = true;
                }
                if ay && by {
                    eq(&mut py, ia, ib, off_y(*a_edge, sa.h) - off_y(*b_edge, sb.h), diags);
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
    for i in 0..n {
        xcomps.entry(rootx[i]).or_default().push(i);
    }
    let mut order: Vec<(usize, Vec<usize>)> = xcomps.into_iter().collect();
    order.sort_by_key(|(_, v)| *v.iter().min().unwrap());
    let gap = cfg.margin(Margin::Medium);
    let mut originx: BTreeMap<usize, f64> = BTreeMap::new();
    let mut cursor = 0.0;
    for (root, members) in &order {
        let minrel = members.iter().map(|&i| relx[i]).fold(f64::INFINITY, f64::min);
        let maxend = members
            .iter()
            .map(|&i| relx[i] + w_of(i))
            .fold(f64::NEG_INFINITY, f64::max);
        originx.insert(*root, cursor - minrel);
        cursor += (maxend - minrel) + gap;
    }

    // Y components normalized so each top sits at 0 (shared band).
    let mut ycomps: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..n {
        ycomps.entry(rooty[i]).or_default().push(i);
    }
    let mut originy: BTreeMap<usize, f64> = BTreeMap::new();
    for (root, members) in &ycomps {
        let minrel = members.iter().map(|&i| rely[i]).fold(f64::INFINITY, f64::min);
        originy.insert(*root, -minrel);
    }

    let mut out = BTreeMap::new();
    for i in 0..n {
        let (sz, _) = dims[&ids[i]];
        let x = originx[&rootx[i]] + relx[i];
        let y = originy[&rooty[i]] + rely[i];
        out.insert(ids[i].clone(), Rect { x, y, w: sz.w, h: sz.h });
    }
    out
}

pub fn solve(scene: &Scene, sizes: &SizeMap, cfg: &SolveConfig) -> (Solved, Vec<Diagnostic>) {
    let mut diags = vec![];
    // Task 5: flat — position leaf boxes only. Groups/shapes arrive in Task 6.
    let mut dims: BTreeMap<BoxId, (Size, Margin)> = BTreeMap::new();
    let mut ids = vec![];
    for b in &scene.boxes {
        if b.kind == BoxKind::Leaf {
            if let BoxId::Node(key) = &b.id {
                let sz = sizes.get(key).copied().unwrap_or(Size { w: 100.0, h: 40.0 });
                dims.insert(b.id.clone(), (sz, b.margin));
                ids.push(b.id.clone());
            }
        }
    }
    let rects = solve_cluster(&ids, &dims, &scene.constraints, cfg, &mut diags);
    let mut nodes = BTreeMap::new();
    for (id, r) in rects {
        if let BoxId::Node(key) = id {
            nodes.insert(key, r);
        }
    }
    (Solved { nodes, groups: vec![], flags: BTreeMap::new() }, diags)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::{Margin, Shape};
    use super::super::{FlagSet, pretty};

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
                Constraint::Place { a: BoxId::Node("a".into()), b: BoxId::Node("b".into()), dir: Direction::LeftOf },
                Constraint::Place { a: BoxId::Node("b".into()), b: BoxId::Node("c".into()), dir: Direction::LeftOf },
            ],
        };
        let (solved, diags) = solve(&scene, &sizes(&["a", "b", "c"], 200.0, 90.0), &SolveConfig::default());
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
                Constraint::Place { a: BoxId::Node("a".into()), b: BoxId::Node("b".into()), dir: Direction::LeftOf },
                Constraint::Place { a: BoxId::Node("b".into()), b: BoxId::Node("a".into()), dir: Direction::LeftOf },
            ],
        };
        let (solved, diags) = solve(&scene, &sizes(&["a", "b"], 200.0, 90.0), &SolveConfig::default());
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, DiagCode::LayoutConflict);
        assert_eq!(solved.nodes.len(), 2, "always renders every node");
    }
}
