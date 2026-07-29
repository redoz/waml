use super::{primitives::ClassDrawResources, RenderSnapshot};
use crate::canvas::geometry::{
    corner_fillet, elbow_radius, marker_geometry, segment_quad, snap_bar_to_device,
    ELBOW_MIN_DEVICE_PX,
};
use crate::canvas::primitives::edge_point_to_screen;
use makepad_widgets::*;
use waml::adornment::{end_marker, End};

pub(super) fn draw_edges(
    cx: &mut Cx2d,
    snapshot: &RenderSnapshot<'_>,
    draws: &mut ClassDrawResources<'_>,
) {
    let camera = snapshot.viewport.camera;
    let rect = snapshot.viewport.view_rect;
    let zoom = camera.zoom;
    let thickness = snapshot.linework.edge_thickness;
    let marker_size = snapshot.linework.marker_size;
    draws.edge.set_uniform(cx, live_id!(zoom), &[zoom as f32]);
    draws.elbow.set_uniform(cx, live_id!(zoom), &[zoom as f32]);
    let r_base = thickness * 2.0;
    let dpi = cx.current_dpi_factor();
    let elbow_min = ELBOW_MIN_DEVICE_PX / dpi;

    for edge in &snapshot.scene.edges {
        let n = edge.points.len();
        let screen: Vec<DVec2> = edge
            .points
            .iter()
            .map(|point| edge_point_to_screen(&camera, rect.pos, *point))
            .collect();
        let mut radius = vec![0.0f64; n];
        for i in 1..n.saturating_sub(1) {
            let r = elbow_radius(screen[i - 1], screen[i], screen[i + 1], r_base);
            radius[i] = if r >= elbow_min { r } else { 0.0 };
        }

        let mut bars: Vec<Rect> = Vec::with_capacity(n.saturating_sub(1));
        for i in 0..n.saturating_sub(1) {
            let a = screen[i];
            let b = screen[i + 1];
            let segment = dvec2(b.x - a.x, b.y - a.y);
            let len = (segment.x * segment.x + segment.y * segment.y).sqrt();
            let (mut a, mut b) = (a, b);
            if len > 1e-6 {
                let unit = dvec2(segment.x / len, segment.y / len);
                a = dvec2(a.x + unit.x * radius[i], a.y + unit.y * radius[i]);
                b = dvec2(b.x - unit.x * radius[i + 1], b.y - unit.y * radius[i + 1]);
            }
            bars.push(snap_bar_to_device(segment_quad(a, b, thickness), dpi));
        }

        let mut vprime = vec![dvec2(0.0, 0.0); n];
        for i in 1..n.saturating_sub(1) {
            if radius[i] <= 0.0 {
                continue;
            }
            let (in_bar, out_bar) = (bars[i - 1], bars[i]);
            let incoming = dvec2(screen[i].x - screen[i - 1].x, screen[i].y - screen[i - 1].y);
            vprime[i] = if incoming.y.abs() < 1e-6 {
                dvec2(
                    out_bar.pos.x + out_bar.size.x * 0.5,
                    in_bar.pos.y + in_bar.size.y * 0.5,
                )
            } else {
                dvec2(
                    in_bar.pos.x + in_bar.size.x * 0.5,
                    out_bar.pos.y + out_bar.size.y * 0.5,
                )
            };
        }

        let snap = |value: f64| (value * dpi).round() / dpi;
        for i in 0..n.saturating_sub(1) {
            let a_fillet = radius[i] > 0.0;
            let b_fillet = radius[i + 1] > 0.0;
            let snapped_bar = bars[i];
            let segment = dvec2(screen[i + 1].x - screen[i].x, screen[i + 1].y - screen[i].y);
            let len = (segment.x * segment.x + segment.y * segment.y).sqrt();
            let quad = if len < 1e-6 {
                snapped_bar
            } else {
                let unit = dvec2(segment.x / len, segment.y / len);
                let a = if a_fillet {
                    dvec2(
                        vprime[i].x + unit.x * radius[i],
                        vprime[i].y + unit.y * radius[i],
                    )
                } else {
                    screen[i]
                };
                let b = if b_fillet {
                    dvec2(
                        vprime[i + 1].x - unit.x * radius[i + 1],
                        vprime[i + 1].y - unit.y * radius[i + 1],
                    )
                } else {
                    screen[i + 1]
                };
                if unit.x.abs() >= unit.y.abs() {
                    let ax = if a_fillet { a.x } else { snap(a.x) };
                    let bx = if b_fillet { b.x } else { snap(b.x) };
                    let (x0, x1) = (ax.min(bx), ax.max(bx));
                    Rect {
                        pos: dvec2(x0, snapped_bar.pos.y),
                        size: dvec2((x1 - x0).max(1.0 / dpi), snapped_bar.size.y),
                    }
                } else {
                    let ay = if a_fillet { a.y } else { snap(a.y) };
                    let by = if b_fillet { b.y } else { snap(b.y) };
                    let (y0, y1) = (ay.min(by), ay.max(by));
                    Rect {
                        pos: dvec2(snapped_bar.pos.x, y0),
                        size: dvec2(snapped_bar.size.x, (y1 - y0).max(1.0 / dpi)),
                    }
                }
            };
            bars[i] = quad;
            draws.edge.draw_abs(cx, quad);
        }

        for i in 1..n.saturating_sub(1) {
            if radius[i] <= 0.0 {
                continue;
            }
            if let Some(fillet) = corner_fillet(
                screen[i - 1],
                screen[i],
                screen[i + 1],
                bars[i - 1],
                bars[i],
                radius[i],
            ) {
                draws
                    .elbow
                    .set_uniform(cx, live_id!(bar_in), &fillet.bar_in);
                draws
                    .elbow
                    .set_uniform(cx, live_id!(bar_out), &fillet.bar_out);
                draws.elbow.set_uniform(cx, live_id!(gate), &fillet.gate);
                draws.elbow.set_uniform(
                    cx,
                    live_id!(center),
                    &[fillet.center.x as f32, fillet.center.y as f32],
                );
                draws
                    .elbow
                    .set_uniform(cx, live_id!(radius), &[fillet.radius as f32]);
                draws
                    .elbow
                    .set_uniform(cx, live_id!(hw), &[fillet.hw as f32]);
                draws.elbow.draw_abs(cx, fillet.quad);
            }
        }

        let points = &edge.points;
        if points.len() >= 2 {
            let ep_to = edge_point_to_screen(&camera, rect.pos, points[points.len() - 1]);
            let previous = edge_point_to_screen(&camera, rect.pos, points[points.len() - 2]);
            let ep_from = edge_point_to_screen(&camera, rect.pos, points[0]);
            let next = edge_point_to_screen(&camera, rect.pos, points[1]);
            let ends = [
                (
                    end_marker(edge.kind, End::To, edge.to_end.navigable),
                    ep_to,
                    dvec2(ep_to.x - previous.x, ep_to.y - previous.y),
                ),
                (
                    end_marker(edge.kind, End::From, edge.from_end.navigable),
                    ep_from,
                    dvec2(ep_from.x - next.x, ep_from.y - next.y),
                ),
            ];
            for (marker, endpoint, direction) in ends {
                if let Some(geometry) = marker_geometry(marker, endpoint, direction, marker_size) {
                    draws.marker.set_uniform(cx, live_id!(v01), &geometry.v01);
                    draws.marker.set_uniform(cx, live_id!(v23), &geometry.v23);
                    draws
                        .marker
                        .set_uniform(cx, live_id!(hollow), &[geometry.hollow]);
                    draws
                        .marker
                        .set_uniform(cx, live_id!(filled), &[geometry.filled]);
                    draws
                        .marker
                        .set_uniform(cx, live_id!(stroke_w), &[(thickness * 0.5) as f32]);
                    draws.marker.draw_abs(cx, geometry.quad);
                }
            }
        }
    }
}
