use makepad_widgets::{dvec2, DVec2, Rect};
use waml::adornment::Marker;

pub(crate) const ELBOW_MIN_DEVICE_PX: f64 = 6.0;

/// Draw-ready geometry for an orthogonal corner fillet.
///
/// The quarter-circle band and the two tangent bar stubs share one snapped
/// centerline and stroke width. The stubs overlap the straight bars just beyond
/// each tangent, keeping the join solid without changing the routed path.
pub(crate) struct CornerFillet {
    pub(crate) quad: Rect,
    pub(crate) bar_in: [f32; 4],
    pub(crate) bar_out: [f32; 4],
    pub(crate) gate: [f32; 4],
    pub(crate) center: DVec2,
    pub(crate) radius: f64,
    pub(crate) hw: f64,
}

/// Draw-ready polygon geometry for a relationship terminal marker.
///
/// The vertices are packed as two pairs in the marker quad's local coordinates;
/// the marker tip remains exactly on the routed endpoint.
pub(crate) struct MarkerGeometry {
    pub(crate) quad: Rect,
    pub(crate) v01: [f32; 4],
    pub(crate) v23: [f32; 4],
    pub(crate) hollow: f32,
    pub(crate) filled: f32,
}

pub(crate) fn intersect_rect(a: Rect, b: Rect) -> Rect {
    let x0 = a.pos.x.max(b.pos.x);
    let y0 = a.pos.y.max(b.pos.y);
    let x1 = (a.pos.x + a.size.x).min(b.pos.x + b.size.x);
    let y1 = (a.pos.y + a.size.y).min(b.pos.y + b.size.y);
    Rect {
        pos: dvec2(x0, y0),
        size: dvec2((x1 - x0).max(0.0), (y1 - y0).max(0.0)),
    }
}

fn segment_quad(a: DVec2, b: DVec2, thickness: f64) -> Rect {
    let mut min = dvec2(a.x.min(b.x), a.y.min(b.y));
    let mut size = dvec2((a.x - b.x).abs(), (a.y - b.y).abs());
    if size.x < thickness {
        min.x -= (thickness - size.x) / 2.0;
        size.x = thickness;
    }
    if size.y < thickness {
        min.y -= (thickness - size.y) / 2.0;
        size.y = thickness;
    }
    Rect { pos: min, size }
}

pub(crate) fn elbow_radius(a: DVec2, v: DVec2, b: DVec2, r_base: f64) -> f64 {
    let din = dvec2(v.x - a.x, v.y - a.y);
    let dout = dvec2(b.x - v.x, b.y - v.y);
    let lin = (din.x * din.x + din.y * din.y).sqrt();
    let lout = (dout.x * dout.x + dout.y * dout.y).sqrt();
    if lin < 1e-6 || lout < 1e-6 {
        return 0.0;
    }
    let dot = (din.x * dout.x + din.y * dout.y) / (lin * lout);
    if dot.abs() > 1e-3 {
        return 0.0;
    }
    r_base.min(lin * 0.5).min(lout * 0.5)
}

const CORNER_STUB_OVERLAP: f64 = 1.0;
const CORNER_STUB_SEAL: f64 = 0.5;

/// Constructs the tangent quarter-circle that replaces an orthogonal hard turn.
///
/// The returned arc has the same half-width as the adjacent bars. Its local
/// quadrant gate faces the route vertex, while short sealed stubs overlap both
/// bars to prevent a notch at either tangent. Degenerate, straight, or
/// zero-radius turns have no fillet.
pub(crate) fn corner_fillet(
    a: DVec2,
    v: DVec2,
    b: DVec2,
    in_bar: Rect,
    out_bar: Rect,
    r: f64,
) -> Option<CornerFillet> {
    if r <= 0.0 {
        return None;
    }
    let din = dvec2(v.x - a.x, v.y - a.y);
    let dout = dvec2(b.x - v.x, b.y - v.y);
    let lin = (din.x * din.x + din.y * din.y).sqrt();
    let lout = (dout.x * dout.x + dout.y * dout.y).sqrt();
    if lin < 1e-6 || lout < 1e-6 {
        return None;
    }
    let din = dvec2(din.x / lin, din.y / lin);
    let dout = dvec2(dout.x / lout, dout.y / lout);
    let (v_prime, t_snap) = if din.y.abs() < 1e-6 {
        let cy = in_bar.pos.y + in_bar.size.y * 0.5;
        let cx = out_bar.pos.x + out_bar.size.x * 0.5;
        (dvec2(cx, cy), in_bar.size.y)
    } else {
        let cx = in_bar.pos.x + in_bar.size.x * 0.5;
        let cy = out_bar.pos.y + out_bar.size.y * 0.5;
        (dvec2(cx, cy), in_bar.size.x)
    };
    let hw = t_snap * 0.5;
    let p1 = dvec2(v_prime.x - din.x * r, v_prime.y - din.y * r);
    let p2 = dvec2(v_prime.x + dout.x * r, v_prime.y + dout.y * r);
    let c = dvec2(
        v_prime.x - din.x * r + dout.x * r,
        v_prime.y - din.y * r + dout.y * r,
    );
    let m = t_snap * CORNER_STUB_OVERLAP;
    let q1 = dvec2(p1.x - din.x * m, p1.y - din.y * m);
    let q2 = dvec2(p2.x + dout.x * m, p2.y + dout.y * m);
    let seal = hw * CORNER_STUB_SEAL;
    let p1s = dvec2(p1.x + din.x * seal, p1.y + din.y * seal);
    let p2s = dvec2(p2.x - dout.x * seal, p2.y - dout.y * seal);
    let mut lo = dvec2(c.x.min(v_prime.x), c.y.min(v_prime.y));
    let mut hi = dvec2(c.x.max(v_prime.x), c.y.max(v_prime.y));
    for p in [q1, q2] {
        lo = dvec2(lo.x.min(p.x), lo.y.min(p.y));
        hi = dvec2(hi.x.max(p.x), hi.y.max(p.y));
    }
    lo = dvec2(lo.x - hw, lo.y - hw);
    hi = dvec2(hi.x + hw, hi.y + hw);
    let quad = Rect {
        pos: lo,
        size: dvec2(hi.x - lo.x, hi.y - lo.y),
    };
    let local = |seg: Rect| {
        [
            (seg.pos.x - lo.x) as f32,
            (seg.pos.y - lo.y) as f32,
            seg.size.x as f32,
            seg.size.y as f32,
        ]
    };
    let center_local = dvec2(c.x - lo.x, c.y - lo.y);
    let big = quad.size.x + quad.size.y;
    let gate_x = if v_prime.x >= c.x {
        center_local.x
    } else {
        center_local.x - big
    };
    let gate_y = if v_prime.y >= c.y {
        center_local.y
    } else {
        center_local.y - big
    };
    Some(CornerFillet {
        quad,
        bar_in: local(segment_quad(q1, p1s, t_snap)),
        bar_out: local(segment_quad(p2s, q2, t_snap)),
        gate: [gate_x as f32, gate_y as f32, big as f32, big as f32],
        center: center_local,
        radius: r,
        hw,
    })
}

/// Builds a standard UML terminal polygon at `ep`, oriented along `dir_raw`.
///
/// Open arrows, hollow triangles, and hollow/filled diamonds share one
/// four-vertex representation. The body extends backward from the routed
/// endpoint, so the glyph tip never drifts under zoom. `Marker::None` and
/// degenerate directions produce no geometry.
pub(crate) fn marker_geometry(
    marker: Marker,
    ep: DVec2,
    dir_raw: DVec2,
    size: f64,
) -> Option<MarkerGeometry> {
    if marker == Marker::None {
        return None;
    }
    let len = (dir_raw.x * dir_raw.x + dir_raw.y * dir_raw.y).sqrt();
    if len < 1e-6 {
        return None;
    }
    let d = dvec2(dir_raw.x / len, dir_raw.y / len);
    let n = dvec2(-d.y, d.x);
    let l = size;
    let w = size * 0.62;
    let half = 2.0 * l + w + 2.0;
    let quad = Rect {
        pos: dvec2(ep.x - half, ep.y - half),
        size: dvec2(half * 2.0, half * 2.0),
    };
    let o = quad.pos;
    let lp = |p: DVec2| [(p.x - o.x) as f32, (p.y - o.y) as f32];
    let base = dvec2(ep.x - d.x * l, ep.y - d.y * l);
    let bl = dvec2(base.x + n.x * w, base.y + n.y * w);
    let br = dvec2(base.x - n.x * w, base.y - n.y * w);
    let (v0, v1, v2, v3, hollow, filled) = match marker {
        Marker::HollowTriangle => (ep, bl, br, ep, 1.0, 0.0),
        Marker::FilledDiamond | Marker::HollowDiamond => {
            let far = dvec2(ep.x - d.x * 2.0 * l, ep.y - d.y * 2.0 * l);
            let sa = dvec2(ep.x - d.x * l + n.x * w, ep.y - d.y * l + n.y * w);
            let sb = dvec2(ep.x - d.x * l - n.x * w, ep.y - d.y * l - n.y * w);
            let filled = if marker == Marker::FilledDiamond {
                1.0
            } else {
                0.0
            };
            (ep, sa, far, sb, 1.0 - filled, filled)
        }
        Marker::OpenArrow => (bl, ep, br, ep, 0.0, 0.0),
        Marker::None => return None,
    };
    let a = lp(v0);
    let b = lp(v1);
    let c = lp(v2);
    let e = lp(v3);
    Some(MarkerGeometry {
        quad,
        v01: [a[0], a[1], b[0], b[1]],
        v23: [c[0], c[1], e[0], e[1]],
        hollow,
        filled,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_quad_centres_a_horizontal_bar() {
        let quad = segment_quad(dvec2(10.0, 20.0), dvec2(50.0, 20.0), 4.0);
        assert_eq!(quad.pos, dvec2(10.0, 18.0));
        assert_eq!(quad.size, dvec2(40.0, 4.0));
    }

    #[test]
    fn marker_none_has_no_geometry() {
        assert!(marker_geometry(Marker::None, dvec2(20.0, 30.0), dvec2(1.0, 0.0), 10.0,).is_none());
    }

    #[test]
    fn segment_quad_centers_the_stroke_on_the_routed_line() {
        let thickness = 2.0;
        let q = segment_quad(dvec2(10.0, 50.0), dvec2(30.0, 50.0), thickness);
        assert_eq!(q.pos.x, 10.0);
        assert_eq!(q.size.x, 20.0);
        assert_eq!(q.pos.y, 50.0 - thickness / 2.0);
        assert_eq!(q.size.y, thickness);
        assert_eq!(
            q.pos.y + q.size.y / 2.0,
            50.0,
            "Y center on the routed line"
        );
        let q = segment_quad(dvec2(70.0, 20.0), dvec2(70.0, 5.0), thickness);
        assert_eq!(q.pos.y, 5.0);
        assert_eq!(q.size.y, 15.0);
        assert_eq!(q.pos.x, 70.0 - thickness / 2.0);
        assert_eq!(q.size.x, thickness);
        assert_eq!(
            q.pos.x + q.size.x / 2.0,
            70.0,
            "X center on the routed line"
        );
        let q = segment_quad(dvec2(0.0, 0.0), dvec2(8.0, 6.0), thickness);
        assert_eq!(q.pos, dvec2(0.0, 0.0));
        assert_eq!(q.size, dvec2(8.0, 6.0));
    }

    #[test]
    fn elbow_radius_rounds_only_orthogonal_bends() {
        let a = dvec2(0.0, 0.0);
        let v = dvec2(10.0, 0.0);
        assert_eq!(elbow_radius(a, v, dvec2(10.0, -10.0), 4.0), 4.0);
        assert_eq!(elbow_radius(a, v, dvec2(10.0, -3.0), 4.0), 1.5);
        assert_eq!(elbow_radius(a, v, dvec2(20.0, 0.0), 4.0), 0.0);
        assert_eq!(elbow_radius(a, v, dvec2(20.0, -10.0), 4.0), 0.0);
        assert_eq!(elbow_radius(a, v, dvec2(0.0, 0.0), 4.0), 0.0);
    }

    #[test]
    fn corner_fillet_arc_meets_bars_at_equal_width() {
        let thickness = 2.0;
        let a = dvec2(0.0, 0.0);
        let v = dvec2(10.0, 0.0);
        let b = dvec2(10.0, -10.0);
        let in_bar = segment_quad(a, v, thickness);
        let out_bar = segment_quad(v, b, thickness);
        let f = corner_fillet(a, v, b, in_bar, out_bar, 4.0).unwrap();
        assert_eq!(f.radius, 4.0);
        assert_eq!(f.hw, thickness * 0.5);
        let to_local = |p: DVec2| dvec2(p.x - f.quad.pos.x, p.y - f.quad.pos.y);
        let p1 = to_local(dvec2(6.0, 0.0));
        let p2 = to_local(dvec2(10.0, -4.0));
        for p in [p1, p2] {
            let d = ((p.x - f.center.x).powi(2) + (p.y - f.center.y).powi(2)).sqrt();
            assert!((d - f.radius).abs() < 1e-9, "tangent off the arc: {}", d);
        }
        let bx0 = f.bar_in[0] as f64;
        let bx1 = bx0 + f.bar_in[2] as f64;
        let by0 = f.bar_in[1] as f64;
        let by1 = by0 + f.bar_in[3] as f64;
        assert!((by0 - (p1.y - f.hw)).abs() < 1e-9 && (by1 - (p1.y + f.hw)).abs() < 1e-9);
        let seal = f.hw * CORNER_STUB_SEAL;
        assert!(bx0 < p1.x && p1.x < bx1);
        assert!((bx1 - (p1.x + seal)).abs() < 1e-9);
        let ox0 = f.bar_out[0] as f64;
        let ox1 = ox0 + f.bar_out[2] as f64;
        assert!((ox0 - (p2.x - f.hw)).abs() < 1e-9 && (ox1 - (p2.x + f.hw)).abs() < 1e-9);
    }

    #[test]
    fn marker_geometry_puts_the_tip_on_the_endpoint() {
        let ep = dvec2(100.0, 100.0);
        let m = marker_geometry(Marker::HollowTriangle, ep, dvec2(1.0, 0.0), 10.0).unwrap();
        let near = |a: f64, b: f64| (a - b).abs() < 1e-3;
        let tip = dvec2(
            m.quad.pos.x + m.v01[0] as f64,
            m.quad.pos.y + m.v01[1] as f64,
        );
        assert!(
            near(tip.x, ep.x) && near(tip.y, ep.y),
            "apex on the endpoint"
        );
        let bl = dvec2(
            m.quad.pos.x + m.v01[2] as f64,
            m.quad.pos.y + m.v01[3] as f64,
        );
        assert!(
            near(bl.x, 90.0) && near(bl.y, 100.0 + 6.2),
            "base back along -dir, offset by w"
        );
        assert_eq!(
            (m.hollow, m.filled),
            (1.0, 0.0),
            "generalization triangle is hollow"
        );
    }

    #[test]
    fn marker_geometry_flags_match_the_glyph() {
        let ep = dvec2(0.0, 0.0);
        let d = dvec2(0.0, 1.0);
        assert_eq!(
            marker_geometry(Marker::FilledDiamond, ep, d, 8.0).map(|m| (m.hollow, m.filled)),
            Some((0.0, 1.0))
        );
        assert_eq!(
            marker_geometry(Marker::HollowDiamond, ep, d, 8.0).map(|m| (m.hollow, m.filled)),
            Some((1.0, 0.0))
        );
        assert_eq!(
            marker_geometry(Marker::OpenArrow, ep, d, 8.0).map(|m| (m.hollow, m.filled)),
            Some((0.0, 0.0))
        );
        assert!(marker_geometry(Marker::None, ep, d, 8.0).is_none());
        assert!(marker_geometry(Marker::OpenArrow, ep, dvec2(0.0, 0.0), 8.0).is_none());
    }
}
