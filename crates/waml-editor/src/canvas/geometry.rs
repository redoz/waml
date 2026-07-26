use makepad_widgets::{dvec2, DVec2, Rect};
use waml::adornment::Marker;

pub(crate) const ELBOW_MIN_DEVICE_PX: f64 = 6.0;

pub(crate) struct CornerFillet {
    pub(crate) quad: Rect,
    pub(crate) bar_in: [f32; 4],
    pub(crate) bar_out: [f32; 4],
    pub(crate) gate: [f32; 4],
    pub(crate) center: DVec2,
    pub(crate) radius: f64,
    pub(crate) hw: f64,
}

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

pub(crate) fn segment_quad(a: DVec2, b: DVec2, thickness: f64) -> Rect {
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

pub(crate) fn snap_bar_to_device(rect: Rect, dpi: f64) -> Rect {
    let snap = |v: f64| (v * dpi).round() / dpi;
    let size = |v: f64| ((v * dpi).round().max(1.0)) / dpi;
    Rect {
        pos: dvec2(snap(rect.pos.x), snap(rect.pos.y)),
        size: dvec2(size(rect.size.x), size(rect.size.y)),
    }
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
    fn snapping_respects_hidpi_device_pixels() {
        let snapped = snap_bar_to_device(
            Rect {
                pos: dvec2(10.3, 20.3),
                size: dvec2(0.4, 12.2),
            },
            2.0,
        );
        assert_eq!(snapped.pos, dvec2(10.5, 20.5));
        assert_eq!(snapped.size, dvec2(0.5, 12.0));
    }
}
