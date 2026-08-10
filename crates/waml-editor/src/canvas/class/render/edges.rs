use super::{primitives::ClassDrawResources, RenderSnapshot};
use crate::canvas::geometry::{corner_fillet, elbow_radius, marker_geometry, ELBOW_MIN_DEVICE_PX};
use crate::canvas::pen::{self, Pen};
use crate::canvas::primitives::edge_point_to_screen;
use crate::{EdgeLineStyle, StructuralVisualPolicy};
use makepad_widgets::*;

const DASH_DEVICE_PX: f64 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq)]
struct DashSegmentGeometry {
    quad: Rect,
    from: [f32; 2],
    to: [f32; 2],
    phase_device_px: f64,
    next_phase_device_px: f64,
}

fn dash_segment_geometry(
    from: DVec2,
    to: DVec2,
    stroke_width: f64,
    dpi: f64,
    phase_device_px: f64,
) -> DashSegmentGeometry {
    let pad = stroke_width * 0.5 + 1.0 / dpi;
    let quad = Rect {
        pos: dvec2(from.x.min(to.x) - pad, from.y.min(to.y) - pad),
        size: dvec2(
            (from.x - to.x).abs() + pad * 2.0,
            (from.y - to.y).abs() + pad * 2.0,
        ),
    };
    let local = |point: DVec2| [(point.x - quad.pos.x) as f32, (point.y - quad.pos.y) as f32];
    DashSegmentGeometry {
        quad,
        from: local(from),
        to: local(to),
        phase_device_px,
        next_phase_device_px: (phase_device_px + (to - from).length() * dpi) % DASH_DEVICE_PX,
    }
}

pub(super) fn draw_edges(
    cx: &mut Cx2d,
    snapshot: &RenderSnapshot<'_>,
    draws: &mut ClassDrawResources<'_>,
) {
    let camera = snapshot.viewport.camera;
    let rect = snapshot.viewport.view_rect;
    let pen = Pen::REGULAR;
    // `edge_thickness` (deleted with `LineworkMetrics`) was always exactly
    // `Pen::REGULAR.width()`, so `pen.width()` is the identical value here.
    let thickness = if snapshot.scene.visual_kind == crate::StructuralVisualKind::UseCase {
        0.75
    } else {
        pen.width()
    };
    let marker_size = super::MARKER_SIZE;
    let r_base = pen.width() * 2.0;
    let dpi = cx.current_dpi_factor();
    let elbow_min = ELBOW_MIN_DEVICE_PX / dpi;
    let old_edge_color = draws.edge.color;
    let old_dashed_color = draws.edge_dashed.color;
    let old_actor_line_color = draws.actor_line.color;

    for edge in &snapshot.scene.edges {
        let notation = StructuralVisualPolicy {
            kind: snapshot.scene.visual_kind,
        }
        .edge_notation(edge.kind, edge.from_end.navigable, edge.to_end.navigable);
        let n = edge.points.len();
        let screen: Vec<DVec2> = edge
            .points
            .iter()
            .map(|point| edge_point_to_screen(&camera, rect.pos, *point))
            .collect();
        if snapshot.scene.visual_kind == crate::StructuralVisualKind::UseCase
            && edge.kind == waml::model::RelationshipKind::Associates
            && screen.len() >= 2
        {
            draws.actor_line.color = draws.edge.color;
            let geometry =
                dash_segment_geometry(screen[0], screen[screen.len() - 1], thickness, dpi, 0.0);
            draws
                .actor_line
                .set_uniform(cx, live_id!(from), &geometry.from);
            draws.actor_line.set_uniform(cx, live_id!(to), &geometry.to);
            draws
                .actor_line
                .set_uniform(cx, live_id!(stroke_w), &[thickness as f32]);
            draws.actor_line.draw_abs(cx, geometry.quad);
            continue;
        }
        let mut radius = vec![0.0f64; n];
        for i in 1..n.saturating_sub(1) {
            let r = elbow_radius(screen[i - 1], screen[i], screen[i + 1], r_base);
            radius[i] = if r >= elbow_min { r } else { 0.0 };
        }
        if notation.line == EdgeLineStyle::Dashed {
            radius.fill(0.0);
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
            bars.push(pen::band(cx, a, b, pen));
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
        let mut dash_phase_device_px = 0.0;
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
            match notation.line {
                EdgeLineStyle::Solid => {
                    draws
                        .edge
                        .set_uniform(cx, live_id!(pen_w), &[pen.width() as f32]);
                    // `set_dyn_instance`, not `set_uniform`: `thin_y` rides
                    // `EdgeLine`'s instance buffer so flipping it per segment
                    // cannot split the batch (see `EdgeLine` in
                    // `class/widget.rs`).
                    let horizontal = pen::band_is_horizontal(screen[i], screen[i + 1]);
                    draws.edge.set_dyn_instance(
                        cx,
                        live_id!(thin_y),
                        &[if horizontal { 1.0 } else { 0.0 }],
                    );
                    draws.edge.draw_abs(cx, quad);
                }
                EdgeLineStyle::Dashed => {
                    let geometry = dash_segment_geometry(
                        screen[i],
                        screen[i + 1],
                        thickness,
                        dpi,
                        dash_phase_device_px,
                    );
                    draws
                        .edge_dashed
                        .set_uniform(cx, live_id!(from), &geometry.from);
                    draws
                        .edge_dashed
                        .set_uniform(cx, live_id!(to), &geometry.to);
                    draws.edge_dashed.set_uniform(
                        cx,
                        live_id!(phase_device_px),
                        &[geometry.phase_device_px as f32],
                    );
                    draws
                        .edge_dashed
                        .set_uniform(cx, live_id!(dpi), &[dpi as f32]);
                    draws
                        .edge_dashed
                        .set_uniform(cx, live_id!(stroke_w), &[thickness as f32]);
                    draws.edge_dashed.set_uniform(
                        cx,
                        live_id!(dash_device_px),
                        &[DASH_DEVICE_PX as f32],
                    );
                    draws.edge_dashed.draw_abs(cx, geometry.quad);
                    dash_phase_device_px = geometry.next_phase_device_px;
                }
            }
        }

        for i in 1..n.saturating_sub(1) {
            if radius[i] <= 0.0 || notation.line == EdgeLineStyle::Dashed {
                continue;
            }
            // `bars` now hold the pen's CANVAS, two device pixels wider than the
            // ink. The fillet band has to match the ink, so shrink both bars
            // back to the quantised width before handing them over.
            let ink = pen::ink_px(pen, dpi) / dpi;
            let deflate = |bar: Rect, horizontal: bool| -> Rect {
                if horizontal {
                    Rect {
                        pos: dvec2(bar.pos.x, bar.pos.y + (bar.size.y - ink) * 0.5),
                        size: dvec2(bar.size.x, ink),
                    }
                } else {
                    Rect {
                        pos: dvec2(bar.pos.x + (bar.size.x - ink) * 0.5, bar.pos.y),
                        size: dvec2(ink, bar.size.y),
                    }
                }
            };
            let in_bar = deflate(
                bars[i - 1],
                pen::band_is_horizontal(screen[i - 1], screen[i]),
            );
            let out_bar = deflate(bars[i], pen::band_is_horizontal(screen[i], screen[i + 1]));
            if let Some(fillet) = corner_fillet(
                screen[i - 1],
                screen[i],
                screen[i + 1],
                in_bar,
                out_bar,
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
                    .set_uniform(cx, live_id!(pen_w), &[pen.width() as f32]);
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
                    notation.to_marker,
                    ep_to,
                    dvec2(ep_to.x - previous.x, ep_to.y - previous.y),
                ),
                (
                    notation.from_marker,
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
                        .set_uniform(cx, live_id!(pen_w), &[pen.width() as f32]);
                    draws.marker.draw_abs(cx, geometry.quad);
                }
            }
        }
    }
    draws.edge.color = old_edge_color;
    draws.edge_dashed.color = old_dashed_color;
    draws.actor_line.color = old_actor_line_color;
}

#[cfg(test)]
mod dash_tests {
    use super::*;

    #[test]
    fn dash_geometry_follows_horizontal_vertical_and_diagonal_segments() {
        for (from, to) in [
            (dvec2(2.0, 3.0), dvec2(12.0, 3.0)),
            (dvec2(2.0, 3.0), dvec2(2.0, 13.0)),
            (dvec2(2.0, 3.0), dvec2(8.0, 11.0)),
        ] {
            let geometry = dash_segment_geometry(from, to, 2.0, 1.0, 0.0);
            assert_eq!(geometry.from, [2.0, 2.0]);
            assert_eq!(
                geometry.to,
                [
                    (to.x - geometry.quad.pos.x) as f32,
                    (to.y - geometry.quad.pos.y) as f32
                ]
            );
            assert!(geometry.quad.contains(from));
            assert!(geometry.quad.contains(to));
        }
    }

    #[test]
    fn dash_phase_is_continuous_across_bends() {
        let first = dash_segment_geometry(dvec2(0.0, 0.0), dvec2(10.0, 0.0), 2.0, 1.0, 0.0);
        let second = dash_segment_geometry(
            dvec2(10.0, 0.0),
            dvec2(10.0, 7.0),
            2.0,
            1.0,
            first.next_phase_device_px,
        );
        assert_eq!(first.next_phase_device_px, 2.0);
        assert_eq!(second.phase_device_px, first.next_phase_device_px);
        assert_eq!(second.next_phase_device_px, 1.0);
    }

    #[test]
    fn dash_period_is_defined_in_device_pixels_at_one_and_two_x_dpi() {
        let one = dash_segment_geometry(dvec2(0.0, 0.0), dvec2(5.0, 0.0), 2.0, 1.0, 0.0);
        let two = dash_segment_geometry(dvec2(0.0, 0.0), dvec2(5.0, 0.0), 2.0, 2.0, 0.0);
        assert_eq!(one.next_phase_device_px, 5.0);
        assert_eq!(two.next_phase_device_px, 2.0);
    }
}
