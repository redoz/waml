use waml::solve::stress::{self, StressConfig};
use waml::solve::{BoxId, Size};

fn node(k: &str) -> BoxId {
    BoxId::Node(k.into())
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
