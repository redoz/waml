//! Interaction scene render pass (spec §4.2): lifelines, activation bars,
//! messages, and combined-fragment frames. One pass over the solved scene in
//! world space, through `world_rect_to_screen`/`edge_point_to_screen` (same
//! idiom as the flow renderer).

use super::super::hit::BehaviorTarget;
use super::super::scene::{ActivationGeo, FragmentGeo, LifelineGeo, MessageGeo};
use super::Emphasis;
use crate::accent;
use crate::canvas::primitives::{edge_point_to_screen, fill_rect, world_rect_to_screen};
use crate::canvas::viewport::ViewportSnapshot;
use crate::node_style::AccentBucket;
use makepad_widgets::*;
use waml::model::MessageVerb;

const HEAD_ALPHA: f32 = 0.16;
const ACTIVATION_ALPHA: f32 = 0.55;
const ACTIVATION_UNCLOSED_ALPHA: f32 = 0.3;
const STEM_COLOR: Vec4 = Vec4 {
    x: 0.42,
    y: 0.47,
    z: 0.54,
    w: 1.0,
};
const DASH_LEN: f64 = 5.0;
const DASH_GAP: f64 = 4.0;

pub(in crate::canvas::behavior) struct InteractionDrawResources<'a> {
    pub(super) node_box: &'a mut DrawColor,
    pub(super) fill: &'a mut DrawColor,
    pub(super) triangle: &'a mut DrawColor,
    pub(super) open_head: &'a mut DrawColor,
    pub(super) x_mark: &'a mut DrawColor,
    pub(super) frame_border: &'a mut DrawColor,
    pub(super) pentagon: &'a mut DrawColor,
    pub(super) text: &'a mut DrawText,
    pub(super) text_heading: &'a mut DrawText,
}

fn accent_with_alpha(accent: Vec4, alpha: f32) -> Vec4 {
    vec4(accent.x, accent.y, accent.z, alpha)
}

fn lifeline_accent(default: Vec4, bucket: AccentBucket) -> Vec4 {
    match bucket {
        AccentBucket::None | AccentBucket::Unknown => default,
        b => accent::bucket_color(b),
    }
}

fn lifeline_emphasis(
    hovered: Option<&BehaviorTarget>,
    selected: Option<&BehaviorTarget>,
    id: &str,
) -> Emphasis {
    let is = |t: Option<&BehaviorTarget>| matches!(t, Some(BehaviorTarget::Lifeline(k)) if k == id);
    Emphasis::of(is(hovered), is(selected))
}

fn message_emphasis(
    hovered: Option<&BehaviorTarget>,
    selected: Option<&BehaviorTarget>,
    id: &str,
) -> Emphasis {
    let is = |t: Option<&BehaviorTarget>| matches!(t, Some(BehaviorTarget::Message(k)) if k == id);
    Emphasis::of(is(hovered), is(selected))
}

fn fragment_emphasis(
    hovered: Option<&BehaviorTarget>,
    selected: Option<&BehaviorTarget>,
    id: &str,
) -> Emphasis {
    let is = |t: Option<&BehaviorTarget>| matches!(t, Some(BehaviorTarget::Fragment(k)) if k == id);
    Emphasis::of(is(hovered), is(selected))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    cx: &mut Cx2d,
    viewport: ViewportSnapshot,
    accent: Vec4,
    lifelines: &[LifelineGeo],
    activations: &[ActivationGeo],
    messages: &[MessageGeo],
    fragments: &[FragmentGeo],
    hovered: Option<&BehaviorTarget>,
    selected: Option<&BehaviorTarget>,
    draws: &mut InteractionDrawResources<'_>,
) {
    // Frames first (background layer), then stems, then activation bars,
    // then messages/arrowheads/labels, then lifeline head cards on top.
    for fragment in fragments {
        draw_fragment(
            cx,
            viewport,
            fragment,
            fragment_emphasis(hovered, selected, &fragment.id),
            draws,
        );
    }
    for lifeline in lifelines {
        draw_stem(
            cx,
            viewport,
            lifeline,
            lifeline_emphasis(hovered, selected, &lifeline.id),
            draws,
        );
    }
    for activation in activations {
        draw_activation(cx, viewport, accent, activation, draws);
    }
    for message in messages {
        draw_message(
            cx,
            viewport,
            message,
            message_emphasis(hovered, selected, &message.id),
            draws,
        );
    }
    for lifeline in lifelines {
        draw_head(
            cx,
            viewport,
            accent,
            lifeline,
            lifeline_emphasis(hovered, selected, &lifeline.id),
            draws,
        );
    }
}

fn draw_dashed_segment(
    cx: &mut Cx2d,
    camera: &crate::canvas::viewport::Camera,
    rect_pos: DVec2,
    fill: &mut DrawColor,
    a: (f64, f64),
    b: (f64, f64),
    thickness: f64,
) {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let len = dx.hypot(dy);
    if len <= f64::EPSILON {
        return;
    }
    let (ux, uy) = (dx / len, dy / len);
    let period = DASH_LEN + DASH_GAP;
    let mut travelled = 0.0;
    while travelled < len {
        let dash_end = (travelled + DASH_LEN).min(len);
        let p0 = (a.0 + ux * travelled, a.1 + uy * travelled);
        let p1 = (a.0 + ux * dash_end, a.1 + uy * dash_end);
        draw_solid_segment(cx, camera, rect_pos, fill, p0, p1, thickness);
        travelled += period;
    }
}

fn draw_solid_segment(
    cx: &mut Cx2d,
    camera: &crate::canvas::viewport::Camera,
    rect_pos: DVec2,
    fill: &mut DrawColor,
    a: (f64, f64),
    b: (f64, f64),
    thickness: f64,
) {
    let sa = edge_point_to_screen(camera, rect_pos, a);
    let sb = edge_point_to_screen(camera, rect_pos, b);
    let quad = if (sa.x - sb.x).abs() >= (sa.y - sb.y).abs() {
        Rect {
            pos: dvec2(sa.x.min(sb.x), (sa.y + sb.y) * 0.5 - thickness * 0.5),
            size: dvec2((sa.x - sb.x).abs().max(1.0), thickness),
        }
    } else {
        Rect {
            pos: dvec2((sa.x + sb.x) * 0.5 - thickness * 0.5, sa.y.min(sb.y)),
            size: dvec2(thickness, (sa.y - sb.y).abs().max(1.0)),
        }
    };
    fill.draw_abs(cx, quad);
}

fn draw_stem(
    cx: &mut Cx2d,
    viewport: ViewportSnapshot,
    lifeline: &LifelineGeo,
    emphasis: Emphasis,
    draws: &mut InteractionDrawResources<'_>,
) {
    let camera = viewport.camera;
    let rect_pos = viewport.view_rect.pos;
    let thickness = emphasis.thickness(1.4) * camera.zoom.max(0.3);
    draws.fill.color = emphasis.stroke(STEM_COLOR);
    let top = (lifeline.stem_x, lifeline.stem_top);
    let bottom = (lifeline.stem_x, lifeline.stem_bottom);
    draw_dashed_segment(cx, &camera, rect_pos, draws.fill, top, bottom, thickness);

    if lifeline.destroyed {
        let size = 10.0 * camera.zoom.max(0.3);
        let center = edge_point_to_screen(&camera, rect_pos, bottom);
        let screen = Rect {
            pos: dvec2(center.x - size * 0.5, center.y - size * 0.5),
            size: dvec2(size, size),
        };
        draws.x_mark.color = STEM_COLOR;
        draws.x_mark.draw_abs(cx, screen);
    }
}

fn draw_head(
    cx: &mut Cx2d,
    viewport: ViewportSnapshot,
    accent: Vec4,
    lifeline: &LifelineGeo,
    emphasis: Emphasis,
    draws: &mut InteractionDrawResources<'_>,
) {
    let screen = world_rect_to_screen(viewport, lifeline.head);
    let zoom = viewport.camera.zoom;
    let wash = accent_with_alpha(
        lifeline_accent(accent, lifeline.bucket),
        HEAD_ALPHA + emphasis.wash_boost(),
    );
    draws.node_box.set_uniform(cx, live_id!(radius), &[0.0]);
    draws.node_box.color = wash;
    draws.node_box.draw_abs(cx, screen);
    draws.text_heading.draw_abs(
        cx,
        dvec2(screen.pos.x + 6.0 * zoom, screen.pos.y + 4.0 * zoom),
        &lifeline.label,
    );
}

fn draw_activation(
    cx: &mut Cx2d,
    viewport: ViewportSnapshot,
    accent: Vec4,
    activation: &ActivationGeo,
    draws: &mut InteractionDrawResources<'_>,
) {
    let screen = world_rect_to_screen(viewport, activation.rect);
    let alpha = if activation.unclosed {
        ACTIVATION_UNCLOSED_ALPHA
    } else {
        ACTIVATION_ALPHA
    };
    fill_rect(cx, draws.fill, screen, accent_with_alpha(accent, alpha));
}

fn draw_message(
    cx: &mut Cx2d,
    viewport: ViewportSnapshot,
    message: &MessageGeo,
    emphasis: Emphasis,
    draws: &mut InteractionDrawResources<'_>,
) {
    let camera = viewport.camera;
    let rect_pos = viewport.view_rect.pos;
    let thickness = emphasis.thickness(2.0) * camera.zoom.max(0.3);
    draws.fill.color = emphasis.stroke(STEM_COLOR);
    let dashed = matches!(message.verb, MessageVerb::Replies | MessageVerb::Creates);

    let (from, to, tip_from) = match message.self_loop {
        Some(rect) => {
            let top_left = (rect.x, rect.y);
            let top_right = (rect.x + rect.w, rect.y);
            let bottom_right = (rect.x + rect.w, rect.y + rect.h);
            if dashed {
                draw_dashed_segment(
                    cx, &camera, rect_pos, draws.fill, top_left, top_right, thickness,
                );
                draw_dashed_segment(
                    cx,
                    &camera,
                    rect_pos,
                    draws.fill,
                    top_right,
                    bottom_right,
                    thickness,
                );
            } else {
                draw_solid_segment(
                    cx, &camera, rect_pos, draws.fill, top_left, top_right, thickness,
                );
                draw_solid_segment(
                    cx,
                    &camera,
                    rect_pos,
                    draws.fill,
                    top_right,
                    bottom_right,
                    thickness,
                );
            }
            (top_right, bottom_right, top_right)
        }
        None => {
            let from = (message.from_x, message.y);
            let to = (message.to_x, message.y);
            if dashed {
                draw_dashed_segment(cx, &camera, rect_pos, draws.fill, from, to, thickness);
            } else {
                draw_solid_segment(cx, &camera, rect_pos, draws.fill, from, to, thickness);
            }
            (from, to, from)
        }
    };
    let _ = tip_from;

    let filled_head = matches!(message.verb, MessageVerb::Calls | MessageVerb::Destroys);
    draw_arrowhead(cx, &camera, rect_pos, from, to, filled_head, draws);

    if let Some(label) = &message.label {
        let anchor = message
            .label_rect
            .map(|r| (r.x, r.y))
            .unwrap_or(((message.from_x + message.to_x) / 2.0, message.y - 14.0));
        let pos = edge_point_to_screen(&camera, rect_pos, anchor);
        draws.text.draw_abs(cx, pos, label);
    }
}

fn draw_arrowhead(
    cx: &mut Cx2d,
    camera: &crate::canvas::viewport::Camera,
    rect_pos: DVec2,
    from: (f64, f64),
    to: (f64, f64),
    filled: bool,
    draws: &mut InteractionDrawResources<'_>,
) {
    let tip = edge_point_to_screen(camera, rect_pos, to);
    let start = edge_point_to_screen(camera, rect_pos, from);
    let dir = dvec2(tip.x - start.x, tip.y - start.y);
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

    let min_x = tip.x.min(left.x).min(right.x);
    let min_y = tip.y.min(left.y).min(right.y);
    let max_x = tip.x.max(left.x).max(right.x);
    let max_y = tip.y.max(left.y).max(right.y);
    let quad = Rect {
        pos: dvec2(min_x, min_y),
        size: dvec2((max_x - min_x).max(1.0), (max_y - min_y).max(1.0)),
    };
    let color = draws.fill.color;
    if filled {
        draws.triangle.set_uniform(
            cx,
            live_id!(v0),
            &[(tip.x - min_x) as f32, (tip.y - min_y) as f32],
        );
        draws.triangle.set_uniform(
            cx,
            live_id!(v1),
            &[(left.x - min_x) as f32, (left.y - min_y) as f32],
        );
        draws.triangle.set_uniform(
            cx,
            live_id!(v2),
            &[(right.x - min_x) as f32, (right.y - min_y) as f32],
        );
        draws.triangle.color = color;
        draws.triangle.draw_abs(cx, quad);
    } else {
        draws.open_head.set_uniform(
            cx,
            live_id!(v0),
            &[(tip.x - min_x) as f32, (tip.y - min_y) as f32],
        );
        draws.open_head.set_uniform(
            cx,
            live_id!(v1),
            &[(left.x - min_x) as f32, (left.y - min_y) as f32],
        );
        draws.open_head.set_uniform(
            cx,
            live_id!(v2),
            &[(right.x - min_x) as f32, (right.y - min_y) as f32],
        );
        draws.open_head.color = color;
        draws.open_head.draw_abs(cx, quad);
    }
}

fn draw_fragment(
    cx: &mut Cx2d,
    viewport: ViewportSnapshot,
    fragment: &FragmentGeo,
    emphasis: Emphasis,
    draws: &mut InteractionDrawResources<'_>,
) {
    let screen = world_rect_to_screen(viewport, fragment.rect);
    draws.frame_border.color = emphasis.stroke(STEM_COLOR);
    draws
        .frame_border
        .set_uniform(cx, live_id!(stroke_w), &[emphasis.thickness(1.2) as f32]);
    draws.frame_border.draw_abs(cx, screen);

    let zoom = viewport.camera.zoom;
    let tab_w = (36.0 * zoom).max(20.0);
    let tab_h = (18.0 * zoom).max(12.0);
    let tab = Rect {
        pos: screen.pos,
        size: dvec2(tab_w, tab_h),
    };
    draws.pentagon.draw_abs(cx, tab);
    draws.text_heading.draw_abs(
        cx,
        dvec2(tab.pos.x + 4.0 * zoom, tab.pos.y + 3.0 * zoom),
        fragment.kind.as_str(),
    );

    let camera = viewport.camera;
    let rect_pos = viewport.view_rect.pos;
    draws.fill.color = STEM_COLOR;
    for operand in &fragment.operands {
        if let Some(y) = operand.divider_y {
            let left = (fragment.rect.x, y);
            let right = (fragment.rect.x + fragment.rect.w, y);
            draw_dashed_segment(
                cx,
                &camera,
                rect_pos,
                draws.fill,
                left,
                right,
                (1.4 * zoom).max(1.0),
            );
        }
        let guard_screen = world_rect_to_screen(viewport, operand.guard_rect);
        draws.text.draw_abs(
            cx,
            dvec2(guard_screen.pos.x, guard_screen.pos.y),
            &format!("[{}]", operand.guard_text),
        );
    }
}
