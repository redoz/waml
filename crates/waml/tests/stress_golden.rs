use waml::solve::stress::{self, GroupSpec, StressConfig};
use waml::solve::{BoxId, Rect, Size};

fn node(k: &str) -> BoxId {
    BoxId::Node(k.into())
}

fn rects_overlap(a: &Rect, b: &Rect) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
}

fn rect_inside(inner: &Rect, outer: &Rect) -> bool {
    inner.x >= outer.x - 1e-6
        && inner.y >= outer.y - 1e-6
        && inner.x + inner.w <= outer.x + outer.w + 1e-6
        && inner.y + inner.h <= outer.y + outer.h + 1e-6
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

fn center(r: &Rect) -> (f64, f64) {
    (r.x + r.w / 2.0, r.y + r.h / 2.0)
}

/// A small fixed graph pinned to an exact pixel dump. Regenerate the expected
/// string only on a deliberate algorithm change (same discipline as
/// `solver_golden.rs`).
#[test]
fn stress_layout_pins_to_expected_pixels() {
    // a-b-c-d path with a b-e branch; five varied node sizes.
    let ids = [node("a"), node("b"), node("c"), node("d"), node("e")];
    let sizes = [
        Size { w: 200.0, h: 90.0 },
        Size { w: 160.0, h: 60.0 },
        Size { w: 220.0, h: 100.0 },
        Size { w: 180.0, h: 80.0 },
        Size { w: 140.0, h: 50.0 },
    ];
    let edges = [(0, 1), (1, 2), (2, 3), (1, 4)];

    let rects = stress::layout(&ids, &sizes, &edges, &StressConfig::default());
    assert_eq!(
        stress::pretty(&ids, &rects),
        "node a @ 522,323 200x90\n\
         node b @ 360,192 160x60\n\
         node c @ 135,304 220x100\n\
         node d @ 0,115 180x80\n\
         node e @ 436,0 140x50\n"
    );
}

/// (a) Two groups joined by a single cross-group edge cluster their own
/// members and never let their hulls overlap.
#[test]
fn grouped_layout_clusters_members_with_disjoint_hulls() {
    let ids = [
        node("a1"),
        node("a2"),
        node("a3"),
        node("b1"),
        node("b2"),
        node("b3"),
    ];
    let sizes = [Size { w: 100.0, h: 50.0 }; 6];
    let edges = [(2, 3)]; // a3 -- b1, the only cross-group pull
    let groups = [
        GroupSpec {
            members: vec![0, 1, 2],
            depth: 0,
        },
        GroupSpec {
            members: vec![3, 4, 5],
            depth: 0,
        },
    ];
    let (rects, hulls) =
        stress::layout_grouped(&ids, &sizes, &edges, &groups, &StressConfig::default());
    assert_eq!(hulls.len(), 2);
    assert!(
        !rects_overlap(&hulls[0], &hulls[1]),
        "sibling hulls overlap: {:?} vs {:?}",
        hulls[0],
        hulls[1]
    );
    for (gi, group) in groups.iter().enumerate() {
        for &m in &group.members {
            assert!(
                rect_inside(&rects[m], &hulls[gi]),
                "member {m} not inside its group hull {:?}",
                hulls[gi]
            );
        }
    }
}

/// (b) A group member with a strong outside edge is pulled toward the
/// outside node rather than held rigidly inside its group — proving
/// cohesion is a soft force, not a hard container.
#[test]
fn grouped_layout_lets_a_strong_outside_edge_pull_a_member_out() {
    let ids = [node("a1"), node("a2"), node("a3"), node("outside")];
    let sizes = [Size { w: 100.0, h: 50.0 }; 4];
    // a3 is edge-adjacent to `outside`; a1/a2 are not.
    let edges = [(2, 3)];
    let groups = [GroupSpec {
        members: vec![0, 1, 2],
        depth: 0,
    }];
    let (rects, _hulls) =
        stress::layout_grouped(&ids, &sizes, &edges, &groups, &StressConfig::default());
    let outside = center(&rects[3]);
    let d_a3 = dist(center(&rects[2]), outside);
    let d_a1 = dist(center(&rects[0]), outside);
    let d_a2 = dist(center(&rects[1]), outside);
    assert!(
        d_a3 < d_a1 && d_a3 < d_a2,
        "a3 (edge-adjacent to outside) should sit closer to it than its \
         edge-less groupmates: d_a3={d_a3} d_a1={d_a1} d_a2={d_a2}"
    );
}

/// (c) A nested group's hull stays inside its parent's hull.
#[test]
fn grouped_layout_nests_hulls_by_depth() {
    let ids = [node("a"), node("b"), node("c")];
    let sizes = [Size { w: 100.0, h: 50.0 }; 3];
    let edges: [(usize, usize); 0] = [];
    let groups = [
        GroupSpec {
            members: vec![0, 1, 2],
            depth: 0,
        },
        GroupSpec {
            members: vec![0, 1],
            depth: 1,
        },
    ];
    let (_rects, hulls) =
        stress::layout_grouped(&ids, &sizes, &edges, &groups, &StressConfig::default());
    assert!(
        rect_inside(&hulls[1], &hulls[0]),
        "inner hull {:?} not inside outer hull {:?}",
        hulls[1],
        hulls[0]
    );
}

/// (d) Two runs of the same input are byte-identical — no RNG, no
/// iteration-order nondeterminism.
#[test]
fn grouped_layout_is_deterministic() {
    let ids = [node("a1"), node("a2"), node("b1"), node("b2")];
    let sizes = [Size { w: 90.0, h: 45.0 }; 4];
    let edges = [(1, 2)];
    let groups = [
        GroupSpec {
            members: vec![0, 1],
            depth: 0,
        },
        GroupSpec {
            members: vec![2, 3],
            depth: 0,
        },
    ];
    let cfg = StressConfig::default();
    let one = stress::layout_grouped(&ids, &sizes, &edges, &groups, &cfg);
    let two = stress::layout_grouped(&ids, &sizes, &edges, &groups, &cfg);
    assert_eq!(one, two);
}
