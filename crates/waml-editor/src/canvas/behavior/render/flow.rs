//! Flow scene render pass (spec §4.1): UML activity/state-machine glyphs,
//! routed edges, off-page stubs and labels. One pass over the solved scene
//! in world space, through `world_rect_to_screen`.

use super::super::hit::BehaviorTarget;
use super::super::scene::{FlowEdgeGeo, FlowNodeGeo, FlowOffPageGeo};
use super::Emphasis;
use crate::canvas::primitives::{edge_point_to_screen, fill_rect, world_rect_to_screen};
use crate::canvas::viewport::ViewportSnapshot;
use makepad_widgets::*;
use waml::model::FlowNodeKind;
use waml::solve::SolvedGroup;

/// Behavior accent bucket wash, at low alpha, for a node fill.
const NODE_ALPHA: f32 = 0.16;
const GROUP_ALPHA: f32 = 0.06;
/// Resting route stroke, in lpx at zoom 1.
const ROUTE_THICKNESS: f64 = 2.0;
const ROUTE_COLOR: Vec4 = Vec4 {
    x: 0.42,
    y: 0.47,
    z: 0.54,
    w: 1.0,
};

pub(in crate::canvas::behavior) struct FlowDrawResources<'a> {
    pub(super) node_box: &'a mut DrawColor,
    pub(super) diamond: &'a mut DrawColor,
    pub(super) circle: &'a mut DrawColor,
    pub(super) triangle: &'a mut DrawColor,
    pub(super) fill: &'a mut DrawColor,
    pub(super) text_heading: &'a mut DrawText,
    pub(super) text_body: &'a mut DrawText,
}

fn node_emphasis(
    hovered: Option<&BehaviorTarget>,
    selected: Option<&BehaviorTarget>,
    key: &str,
) -> Emphasis {
    let is =
        |t: Option<&BehaviorTarget>| matches!(t, Some(BehaviorTarget::FlowNode(k)) if k == key);
    Emphasis::of(is(hovered), is(selected))
}

fn edge_emphasis(
    hovered: Option<&BehaviorTarget>,
    selected: Option<&BehaviorTarget>,
    key: &str,
) -> Emphasis {
    let is =
        |t: Option<&BehaviorTarget>| matches!(t, Some(BehaviorTarget::FlowEdge(k)) if k == key);
    Emphasis::of(is(hovered), is(selected))
}

fn accent_with_alpha(accent: Vec4, alpha: f32) -> Vec4 {
    vec4(accent.x, accent.y, accent.z, alpha)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    cx: &mut Cx2d,
    viewport: ViewportSnapshot,
    accent: Vec4,
    nodes: &[FlowNodeGeo],
    edges: &[FlowEdgeGeo],
    off_page: &[FlowOffPageGeo],
    groups: &[SolvedGroup],
    hovered: Option<&BehaviorTarget>,
    selected: Option<&BehaviorTarget>,
    draws: &mut FlowDrawResources<'_>,
) {
    let zoom = viewport.camera.zoom;

    for group in groups {
        let screen = world_rect_to_screen(viewport, group.rect);
        fill_rect(
            cx,
            draws.fill,
            screen,
            accent_with_alpha(accent, GROUP_ALPHA),
        );
        if let Some(title) = &group.title {
            draws.text_body.draw_abs(
                cx,
                dvec2(screen.pos.x + 6.0 * zoom, screen.pos.y + 4.0 * zoom),
                title,
            );
        }
    }

    for edge in edges {
        let emphasis = edge_emphasis(hovered, selected, &edge.key);
        draw_route(cx, viewport, &edge.points, emphasis, draws);
        if let Some(label) = &edge.label {
            if let Some((x, y)) =
                crate::edge_labels::mid_route_label(&edge.points, label.clone()).map(|l| l.anchor)
            {
                let pos = edge_point_to_screen(&viewport.camera, viewport.view_rect.pos, (x, y));
                draws.text_body.draw_abs(cx, pos, label);
            }
        }
    }

    for stub in off_page {
        draw_route(cx, viewport, &stub.points, Emphasis::None, draws);
        if let Some(&(x, y)) = stub.points.last() {
            let pos = edge_point_to_screen(&viewport.camera, viewport.view_rect.pos, (x, y));
            draws
                .text_body
                .draw_abs(cx, dvec2(pos.x + 4.0, pos.y), &stub.target_title);
        }
    }

    for node in nodes {
        draw_node(
            cx,
            viewport,
            accent,
            node,
            node_emphasis(hovered, selected, &node.key),
            draws,
        );
    }
}

fn draw_route(
    cx: &mut Cx2d,
    viewport: ViewportSnapshot,
    points: &[(f64, f64)],
    emphasis: Emphasis,
    draws: &mut FlowDrawResources<'_>,
) {
    if points.len() < 2 {
        return;
    }
    let camera = viewport.camera;
    let rect_pos = viewport.view_rect.pos;
    let screen: Vec<DVec2> = points
        .iter()
        .map(|p| edge_point_to_screen(&camera, rect_pos, *p))
        .collect();
    let thickness = (emphasis.thickness(ROUTE_THICKNESS) * camera.zoom).max(1.4);
    draws.fill.color = emphasis.stroke(ROUTE_COLOR);
    for pair in screen.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let quad = if (a.x - b.x).abs() >= (a.y - b.y).abs() {
            Rect {
                pos: dvec2(a.x.min(b.x), a.y - thickness * 0.5),
                size: dvec2((a.x - b.x).abs().max(1.0), thickness),
            }
        } else {
            Rect {
                pos: dvec2(a.x - thickness * 0.5, a.y.min(b.y)),
                size: dvec2(thickness, (a.y - b.y).abs().max(1.0)),
            }
        };
        draws.fill.draw_abs(cx, quad);
    }

    let tip = *screen.last().unwrap();
    let prev = screen[screen.len() - 2];
    let dir = dvec2(tip.x - prev.x, tip.y - prev.y);
    let len = (dir.x * dir.x + dir.y * dir.y).sqrt();
    if len <= f64::EPSILON {
        return;
    }
    let unit = dvec2(dir.x / len, dir.y / len);
    let perp = dvec2(-unit.y, unit.x);
    let head = 9.0 * camera.zoom.max(0.3);
    let back = dvec2(tip.x - unit.x * head, tip.y - unit.y * head);
    let left = dvec2(back.x + perp.x * head * 0.5, back.y + perp.y * head * 0.5);
    let right = dvec2(back.x - perp.x * head * 0.5, back.y - perp.y * head * 0.5);
    let color = draws.fill.color;
    draw_triangle(cx, tip, left, right, color, draws);
}

fn draw_triangle(
    cx: &mut Cx2d,
    a: DVec2,
    b: DVec2,
    c: DVec2,
    color: Vec4,
    draws: &mut FlowDrawResources<'_>,
) {
    let min_x = a.x.min(b.x).min(c.x);
    let min_y = a.y.min(b.y).min(c.y);
    let max_x = a.x.max(b.x).max(c.x);
    let max_y = a.y.max(b.y).max(c.y);
    let quad = Rect {
        pos: dvec2(min_x, min_y),
        size: dvec2((max_x - min_x).max(1.0), (max_y - min_y).max(1.0)),
    };
    draws.triangle.set_uniform(
        cx,
        live_id!(v0),
        &[(a.x - min_x) as f32, (a.y - min_y) as f32],
    );
    draws.triangle.set_uniform(
        cx,
        live_id!(v1),
        &[(b.x - min_x) as f32, (b.y - min_y) as f32],
    );
    draws.triangle.set_uniform(
        cx,
        live_id!(v2),
        &[(c.x - min_x) as f32, (c.y - min_y) as f32],
    );
    draws.triangle.color = color;
    draws.triangle.draw_abs(cx, quad);
}

fn draw_node(
    cx: &mut Cx2d,
    viewport: ViewportSnapshot,
    accent: Vec4,
    node: &FlowNodeGeo,
    emphasis: Emphasis,
    draws: &mut FlowDrawResources<'_>,
) {
    let screen = world_rect_to_screen(viewport, node.rect);
    let zoom = viewport.camera.zoom;
    let wash = accent_with_alpha(accent, NODE_ALPHA + emphasis.wash_boost());

    match node.kind {
        FlowNodeKind::Initial => {
            draws.circle.set_uniform(cx, live_id!(bullseye), &[0.0]);
            draws.circle.color = accent;
            draws.circle.draw_abs(cx, screen);
        }
        FlowNodeKind::Final => {
            draws.circle.set_uniform(cx, live_id!(bullseye), &[1.0]);
            draws.circle.color = accent;
            draws.circle.draw_abs(cx, screen);
        }
        FlowNodeKind::Decision | FlowNodeKind::Merge => {
            draws.diamond.color = wash;
            draws.diamond.draw_abs(cx, screen);
            draw_title(cx, screen, zoom, &node.title, draws);
        }
        FlowNodeKind::Fork | FlowNodeKind::Join => {
            draws.fill.color = accent;
            draws.fill.draw_abs(cx, screen);
        }
        FlowNodeKind::Object => {
            draws.node_box.set_uniform(cx, live_id!(radius), &[0.0]);
            draws.node_box.color = wash;
            draws.node_box.draw_abs(cx, screen);
            draw_title(cx, screen, zoom, &node.title, draws);
            if let Some(ty) = &node.type_name {
                draws.text_body.draw_abs(
                    cx,
                    dvec2(screen.pos.x + 6.0 * zoom, screen.pos.y + 20.0 * zoom),
                    &format!(":{ty}"),
                );
            }
        }
        FlowNodeKind::Plain => {
            draws
                .node_box
                .set_uniform(cx, live_id!(radius), &[(6.0 * zoom) as f32]);
            draws.node_box.color = wash;
            draws.node_box.draw_abs(cx, screen);
            draw_title(cx, screen, zoom, &node.title, draws);
            let mut y = 24.0 * zoom;
            for line in &node.lines {
                draws.text_body.draw_abs(
                    cx,
                    dvec2(screen.pos.x + 6.0 * zoom, screen.pos.y + y),
                    line,
                );
                y += 16.0 * zoom;
            }
            if node.refines {
                let glyph = Rect {
                    pos: dvec2(
                        screen.pos.x + screen.size.x - 12.0 * zoom,
                        screen.pos.y + screen.size.y - 10.0 * zoom,
                    ),
                    size: dvec2(6.0 * zoom, 6.0 * zoom),
                };
                draws.fill.color = accent;
                draws.fill.draw_abs(cx, glyph);
            }
        }
    }
}

fn draw_title(
    cx: &mut Cx2d,
    screen: Rect,
    zoom: f64,
    title: &str,
    draws: &mut FlowDrawResources<'_>,
) {
    draws.text_heading.draw_abs(
        cx,
        dvec2(screen.pos.x + 6.0 * zoom, screen.pos.y + 4.0 * zoom),
        title,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A selected element outranks a hovered one, and the two are visibly
    /// distinct: selection is the state a click leaves behind (spec §5.2).
    #[test]
    fn selection_outranks_hover_and_reads_stronger() {
        let key = "flow#Ship";
        let node = BehaviorTarget::FlowNode(key.to_string());
        let other = BehaviorTarget::FlowNode("flow#Pack".to_string());

        assert_eq!(node_emphasis(None, None, key), Emphasis::None);
        assert_eq!(node_emphasis(Some(&node), None, key), Emphasis::Hovered);
        assert_eq!(node_emphasis(None, Some(&node), key), Emphasis::Selected);
        assert_eq!(
            node_emphasis(Some(&node), Some(&node), key),
            Emphasis::Selected
        );
        assert_eq!(
            node_emphasis(Some(&other), Some(&other), key),
            Emphasis::None
        );

        let edge = BehaviorTarget::FlowEdge(key.to_string());
        assert_eq!(edge_emphasis(None, Some(&edge), key), Emphasis::Selected);
        assert_eq!(edge_emphasis(None, Some(&node), key), Emphasis::None);

        assert!(Emphasis::Selected.wash_boost() > Emphasis::Hovered.wash_boost());
        assert!(
            Emphasis::Selected.thickness(ROUTE_THICKNESS)
                > Emphasis::Hovered.thickness(ROUTE_THICKNESS)
        );
        assert_ne!(
            Emphasis::Selected.stroke(ROUTE_COLOR),
            Emphasis::Hovered.stroke(ROUTE_COLOR)
        );
    }
}
