//! Eyeball harness for the standalone stress-majorization layout.
//!
//! Builds a synthetic ER-ish graph (~12 nodes, varied sizes, a couple of
//! disconnected nodes plus one isolated pair), runs `stress::layout`, prints the
//! resulting positions, and writes a plain SVG (rects + edge lines) so geometry
//! can be judged without the makepad editor.
//!
//! Run: `cargo run -p waml --example stress_dump`
//!
//! The real ocuro model at C:\dev\ocuro\docs\kd\domain\ was NOT loadable (the
//! directory does not exist in this checkout), so only the synthetic graph is
//! exercised. Wire up a Model adapter here once that model is available.

use waml::solve::stress::{self, StressConfig};
use waml::solve::{BoxId, Rect, Size};

const SVG_PATH: &str = "C:\\Users\\redoz\\AppData\\Local\\Temp\\claude\\C--dev-waml--claude-worktrees-icons\\107d8d46-de11-4c4e-8ccb-2f028be4408a\\scratchpad\\stress_out.svg";

/// (label, w, h) for each synthetic node.
fn synthetic() -> (Vec<BoxId>, Vec<Size>, Vec<(usize, usize)>) {
    // 0..=9 form a connected ER-ish blob; 10-11 are an isolated pair; 12 alone.
    let spec: &[(&str, f64, f64)] = &[
        ("customer", 200.0, 120.0),
        ("account", 180.0, 90.0),
        ("order", 220.0, 140.0),
        ("order_line", 200.0, 160.0),
        ("product", 190.0, 110.0),
        ("category", 160.0, 80.0),
        ("payment", 200.0, 100.0),
        ("invoice", 210.0, 130.0),
        ("address", 180.0, 90.0),
        ("shipment", 200.0, 110.0),
        ("audit_log", 240.0, 70.0),    // disconnected pair with tag
        ("tag", 120.0, 60.0),          // disconnected pair with audit_log
        ("feature_flag", 160.0, 60.0), // fully isolated
    ];
    let ids: Vec<BoxId> = spec
        .iter()
        .map(|(k, _, _)| BoxId::Node((*k).into()))
        .collect();
    let sizes: Vec<Size> = spec.iter().map(|(_, w, h)| Size { w: *w, h: *h }).collect();
    let edges = vec![
        (0, 1),   // customer - account
        (0, 8),   // customer - address
        (0, 2),   // customer - order
        (2, 3),   // order - order_line
        (3, 4),   // order_line - product
        (4, 5),   // product - category
        (2, 6),   // order - payment
        (6, 7),   // payment - invoice
        (2, 9),   // order - shipment
        (9, 8),   // shipment - address
        (10, 11), // audit_log - tag (own component)
    ];
    (ids, sizes, edges)
}

fn write_svg(
    path: &str,
    ids: &[BoxId],
    rects: &[Rect],
    edges: &[(usize, usize)],
) -> std::io::Result<()> {
    let pad = 40.0;
    let (min_x, min_y, max_x, max_y) = rects.iter().fold(
        (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ),
        |(a, b, c, d), r| (a.min(r.x), b.min(r.y), c.max(r.x + r.w), d.max(r.y + r.h)),
    );
    let w = (max_x - min_x) + 2.0 * pad;
    let h = (max_y - min_y) + 2.0 * pad;

    let mut s = String::new();
    s.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.0}\" height=\"{h:.0}\" viewBox=\"{:.0} {:.0} {w:.0} {h:.0}\">\n",
        min_x - pad,
        min_y - pad
    ));
    s.push_str(
        "<rect x=\"-100000\" y=\"-100000\" width=\"200000\" height=\"200000\" fill=\"#0f1419\"/>\n",
    );

    // Edges (center to center) first, so rects sit on top.
    for &(a, b) in edges {
        let (ra, rb) = (&rects[a], &rects[b]);
        let (x1, y1) = (ra.x + ra.w / 2.0, ra.y + ra.h / 2.0);
        let (x2, y2) = (rb.x + rb.w / 2.0, rb.y + rb.h / 2.0);
        s.push_str(&format!(
            "<line x1=\"{x1:.1}\" y1=\"{y1:.1}\" x2=\"{x2:.1}\" y2=\"{y2:.1}\" stroke=\"#5c6773\" stroke-width=\"2\"/>\n"
        ));
    }

    for (id, r) in ids.iter().zip(rects) {
        let label = match id {
            BoxId::Node(k) => k.clone(),
            BoxId::Group(g) => format!("group{g}"),
            BoxId::Inline(i) => format!("inline{i}"),
        };
        s.push_str(&format!(
            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"6\" fill=\"#1c2530\" stroke=\"#7aa2f7\" stroke-width=\"2\"/>\n",
            r.x, r.y, r.w, r.h
        ));
        s.push_str(&format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"#c0caf5\" font-family=\"monospace\" font-size=\"14\" text-anchor=\"middle\">{label}</text>\n",
            r.x + r.w / 2.0,
            r.y + r.h / 2.0 + 5.0
        ));
    }
    s.push_str("</svg>\n");
    std::fs::write(path, s)
}

fn main() {
    let (ids, sizes, edges) = synthetic();
    let cfg = StressConfig::default();
    let rects = stress::layout(&ids, &sizes, &edges, &cfg);

    println!(
        "stress layout of {} synthetic nodes, {} edges:\n",
        ids.len(),
        edges.len()
    );
    print!("{}", stress::pretty(&ids, &rects));

    match write_svg(SVG_PATH, &ids, &rects, &edges) {
        Ok(()) => println!("\nwrote SVG: {SVG_PATH}"),
        Err(e) => eprintln!("\nSVG write failed: {e}"),
    }
}
