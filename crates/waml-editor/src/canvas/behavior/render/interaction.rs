//! Interaction scene render pass (spec §4.2): lifelines, activation bars,
//! messages, and combined-fragment frames. One pass over the solved scene in
//! world space, through `world_rect_to_screen`/`edge_point_to_screen` (same
//! idiom as the flow renderer).

use super::super::hit::BehaviorTarget;
use super::super::scene::{ActivationGeo, FragmentGeo, LifelineGeo, MessageGeo};
use super::{BehaviorPalette, Emphasis, ARROW_HEAD};
use crate::accent;
use crate::canvas::linework::BehaviorLineworkMetrics;
use crate::canvas::primitives::{
    edge_point_to_screen, fill_rect, snap_rect, snap_stroke_width, stroke_quad,
    world_rect_to_screen,
};
use crate::canvas::viewport::ViewportSnapshot;
use crate::node_style::AccentBucket;
use makepad_widgets::*;
use waml::model::MessageVerb;

/// Head fill: a hint of the participant's accent, not a field of it -- the
/// gradient border carries the colour, so the wash only has to keep the box
/// from reading as canvas ground.
const HEAD_ALPHA: f32 = 0.07;
/// The two ends of the head frame's gradient stroke: near-opaque accent at the
/// gradient's start, faded at its end -- the same falloff the Atlas frame draws
/// with its neutral stops.
const HEAD_BORDER_HI_ALPHA: f32 = 1.0;
const HEAD_BORDER_LO_ALPHA: f32 = 0.45;

/// Extra weight on the head frame over the app-wide 1.5px hairline. With the
/// wash this faint the border is what draws the box, so it carries a heavier
/// pen than a class card's -- fed through `stroke_scale`, which keeps the
/// result screen-space in CAD mode.
const HEAD_FRAME_WEIGHT: f32 = 1.5;

/// Fragment-tab padding around the kind keyword, in lpx at zoom 1, and the
/// bevel's share of the plate height (matches `InteractionTab`'s `notch`).
const TAB_PAD_X: f64 = 6.0;
const TAB_PAD_Y: f64 = 3.0;
const TAB_BEVEL_FRACTION: f64 = 0.45;
const ACTIVATION_ALPHA: f32 = 0.55;
const ACTIVATION_UNCLOSED_ALPHA: f32 = 0.3;
const DASH_LEN: f64 = 5.0;
const DASH_GAP: f64 = 4.0;

pub(in crate::canvas::behavior) struct InteractionDrawResources<'a> {
    pub(super) fill: &'a mut DrawColor,
    pub(super) triangle: &'a mut DrawColor,
    pub(super) open_head: &'a mut DrawColor,
    pub(super) x_mark: &'a mut DrawColor,
    pub(super) frame_border: &'a mut DrawColor,
    pub(super) pentagon: &'a mut DrawColor,
    pub(super) head: &'a mut DrawColor,
    pub(super) text: &'a mut DrawText,
    pub(super) text_heading: &'a mut DrawText,
    pub(super) palette: BehaviorPalette,
    pub(super) linework: BehaviorLineworkMetrics,
}

/// Authored linework, in lpx at zoom 1: lifeline stem, message stroke, fragment
/// frame border and operand divider, plus the destroyed-lifeline X.
const STEM_THICKNESS: f64 = 1.4;
const MESSAGE_THICKNESS: f64 = 2.0;
const FRAME_THICKNESS: f64 = 1.2;
const DIVIDER_THICKNESS: f64 = 1.4;
const X_MARK_SIZE: f64 = 10.0;

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
    // then messages/arrowheads/labels, then the fragment tabs and the lifeline
    // head cards on top. The tab is a plate with a keyword on it: a lifeline
    // running under it would strike straight through the word, so it draws
    // above the stems even though its frame draws below them.
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
    // Signatures draw a rung smaller than the rest of the scene text (see
    // `MESSAGE_TEXT_PT`), and so does a named head's type line -- the same rung
    // the solver measured both with. The pen goes back to the base size once
    // both passes are done, so the next frame's guards start from it.
    super::apply_text_zoom_pt(draws.text, viewport.camera.zoom, super::MESSAGE_TEXT_PT);
    for message in messages {
        draw_message(
            cx,
            viewport,
            message,
            message_emphasis(hovered, selected, &message.id),
            draws,
        );
    }
    for fragment in fragments {
        draw_fragment_tab(
            cx,
            viewport,
            fragment,
            fragment_emphasis(hovered, selected, &fragment.id),
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
    super::apply_text_zoom(draws.text, viewport.camera.zoom);
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
    // CAD hatching: the dash pattern is stepped in SCREEN space, so its period
    // stays put at any zoom -- exactly as `BehaviorLineworkMetrics` holds the
    // stroke width. Stepping in world space instead stretched the dashes with
    // the camera and turned the lifelines solid when zoomed in.
    let sa = edge_point_to_screen(camera, rect_pos, a);
    let sb = edge_point_to_screen(camera, rect_pos, b);
    let dx = sb.x - sa.x;
    let dy = sb.y - sa.y;
    let len = dx.hypot(dy);
    if len <= f64::EPSILON {
        return;
    }
    let (ux, uy) = (dx / len, dy / len);
    let period = DASH_LEN + DASH_GAP;
    let mut travelled = 0.0;
    while travelled < len {
        let dash_end = (travelled + DASH_LEN).min(len);
        let p0 = dvec2(sa.x + ux * travelled, sa.y + uy * travelled);
        let p1 = dvec2(sa.x + ux * dash_end, sa.y + uy * dash_end);
        fill.draw_abs(cx, stroke_quad(cx, p0, p1, thickness));
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
    fill.draw_abs(cx, stroke_quad(cx, sa, sb, thickness));
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
    let thickness = draws.linework.thickness(emphasis.thickness(STEM_THICKNESS));
    draws.fill.color = emphasis.stroke(draws.palette.line, draws.palette);
    let top = (lifeline.stem_x, lifeline.stem_top);
    let bottom = (lifeline.stem_x, lifeline.stem_bottom);
    draw_dashed_segment(cx, &camera, rect_pos, draws.fill, top, bottom, thickness);

    if lifeline.destroyed {
        let size = draws.linework.glyph(X_MARK_SIZE);
        let center = edge_point_to_screen(&camera, rect_pos, bottom);
        let screen = Rect {
            pos: dvec2(center.x - size * 0.5, center.y - size * 0.5),
            size: dvec2(size, size),
        };
        draws.x_mark.color = draws.palette.line;
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
    let head_accent = lifeline_accent(accent, lifeline.bucket);
    let wash = accent_with_alpha(head_accent, HEAD_ALPHA + emphasis.wash_boost());
    // The head is drawn with the app-wide `AccentFrame`: same 150deg gradient
    // border, same geometry, re-tinted to this participant's accent instead of
    // the theme's neutral frame stops (redoz@). `border_hi`/`border_lo` are the
    // two ends of that gradient, so passing the accent at two alphas keeps the
    // angle and the fade and only changes the hue.
    let zoom = viewport.camera.zoom;
    draws.head.set_uniform(cx, live_id!(zoom), &[zoom as f32]);
    draws.head.set_uniform(
        cx,
        live_id!(stroke_scale),
        &[draws.linework.frame_stroke_scale * HEAD_FRAME_WEIGHT],
    );
    draws.head.set_uniform(
        cx,
        live_id!(screen_space),
        &[draws.linework.frame_screen_space],
    );
    let hi = accent_with_alpha(head_accent, HEAD_BORDER_HI_ALPHA);
    let lo = accent_with_alpha(head_accent, HEAD_BORDER_LO_ALPHA);
    draws
        .head
        .set_uniform(cx, live_id!(border_hi), &[hi.x, hi.y, hi.z, hi.w]);
    draws
        .head
        .set_uniform(cx, live_id!(border_lo), &[lo.x, lo.y, lo.z, lo.w]);
    draws.head.color = wash;
    draws.head.draw_abs(cx, screen);

    // Anonymous head: the classifier alone, centred. Named head: the instance
    // name over the classifier -- what UML writes `checkout : Order`, said with
    // weight and size instead of a colon (see `measure_interaction`, which
    // sizes the box for exactly these two lines).
    //
    // Both lines are measured rather than offset by a constant: the solver
    // sizes the box as `text + head_pad * 2`, so a fixed offset only happens to
    // look right for one label width and drifts for every other.
    let title_size = measure_text(cx, draws.text_heading, &lifeline.label);
    match &lifeline.instance {
        None => {
            let pos = dvec2(
                screen.pos.x + (screen.size.x - title_size.x) * 0.5,
                screen.pos.y + (screen.size.y - title_size.y) * 0.5,
            );
            draws.text_heading.draw_abs(cx, pos, &lifeline.label);
        }
        Some(name) => {
            let name_size = measure_text(cx, draws.text_heading, name);
            // `text` is the message pen at this point in the pass, one rung
            // below `text_heading` -- the same size relation the solver
            // measured the type line with.
            let type_size = measure_text(cx, draws.text, &lifeline.label);
            let stack_h = name_size.y + type_size.y;
            let top = screen.pos.y + (screen.size.y - stack_h) * 0.5;
            draws.text_heading.draw_abs(
                cx,
                dvec2(screen.pos.x + (screen.size.x - name_size.x) * 0.5, top),
                name,
            );
            draws.text.draw_abs(
                cx,
                dvec2(
                    screen.pos.x + (screen.size.x - type_size.x) * 0.5,
                    top + name_size.y,
                ),
                &lifeline.label,
            );
        }
    }
}

/// Drawn size of `text` through `pen`, in screen px at the pen's current zoom.
fn measure_text(cx: &mut Cx2d, pen: &mut DrawText, text: &str) -> DVec2 {
    let measured = pen
        .layout(cx, 0.0, 0.0, None, false, Align::default(), text)
        .size_in_lpxs;
    let scale = pen.font_scale as f64;
    dvec2(
        measured.width as f64 * scale,
        measured.height as f64 * scale,
    )
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

type Segment = ((f64, f64), (f64, f64));

/// A self-message's loop: the THREE outer sides of its `self_loop` rect (out
/// along the top, down the far side, back along the bottom) plus the arrow
/// segment whose head returns to the stem (spec §4.2). The stem is the rect's
/// LEFT edge, so the return side ends on it and the head points back inward,
/// never down at the far corner.
fn self_loop_geometry(rect: waml::solve::Rect) -> ([Segment; 3], Segment) {
    let top_left = (rect.x, rect.y);
    let top_right = (rect.x + rect.w, rect.y);
    let bottom_right = (rect.x + rect.w, rect.y + rect.h);
    let bottom_left = (rect.x, rect.y + rect.h);
    (
        [
            (top_left, top_right),
            (top_right, bottom_right),
            (bottom_right, bottom_left),
        ],
        (bottom_right, bottom_left),
    )
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
    let thickness = draws
        .linework
        .thickness(emphasis.thickness(MESSAGE_THICKNESS));
    draws.fill.color = emphasis.stroke(draws.palette.line, draws.palette);
    let dashed = matches!(message.verb, MessageVerb::Replies | MessageVerb::Creates);

    let (from, to) = match message.self_loop {
        Some(rect) => {
            let (sides, arrow) = self_loop_geometry(rect);
            for (a, b) in sides {
                if dashed {
                    draw_dashed_segment(cx, &camera, rect_pos, draws.fill, a, b, thickness);
                } else {
                    draw_solid_segment(cx, &camera, rect_pos, draws.fill, a, b, thickness);
                }
            }
            arrow
        }
        None => {
            let from = (message.from_x, message.y);
            let to = (message.to_x, message.y);
            if dashed {
                draw_dashed_segment(cx, &camera, rect_pos, draws.fill, from, to, thickness);
            } else {
                draw_solid_segment(cx, &camera, rect_pos, draws.fill, from, to, thickness);
            }
            (from, to)
        }
    };

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
    let head = draws.linework.glyph(ARROW_HEAD);
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
    // The frame is stroked by an SDF pen, so both its rect and its width have to
    // be snapped here -- `stroke_quad` only covers the quad-drawn linework.
    let screen = snap_rect(cx, world_rect_to_screen(viewport, fragment.rect));
    let border = snap_stroke_width(
        cx,
        draws
            .linework
            .thickness(emphasis.thickness(FRAME_THICKNESS)),
    ) as f32;
    let divider = draws.linework.thickness(DIVIDER_THICKNESS);
    draws.frame_border.color = emphasis.stroke(draws.palette.line, draws.palette);
    draws
        .frame_border
        .set_uniform(cx, live_id!(stroke_w), &[border]);
    draws.frame_border.draw_abs(cx, screen);

    let camera = viewport.camera;
    let rect_pos = viewport.view_rect.pos;
    draws.fill.color = draws.palette.line;
    for operand in &fragment.operands {
        if let Some(y) = operand.divider_y {
            let left = (fragment.rect.x, y);
            let right = (fragment.rect.x + fragment.rect.w, y);
            draw_dashed_segment(cx, &camera, rect_pos, draws.fill, left, right, divider);
        }
        let guard_screen = world_rect_to_screen(viewport, operand.guard_rect);
        draws.text.draw_abs(
            cx,
            dvec2(guard_screen.pos.x, guard_screen.pos.y),
            &format!("[{}]", operand.guard_text),
        );
    }
}

/// The fragment's kind plate, drawn in its own pass on top of the stems (see
/// `draw`). The plate is sized from the keyword it carries, not from a
/// constant: at a fixed 36x18 the word overflowed the plate at every zoom and
/// the bevel cut through it.
fn draw_fragment_tab(
    cx: &mut Cx2d,
    viewport: ViewportSnapshot,
    fragment: &FragmentGeo,
    emphasis: Emphasis,
    draws: &mut InteractionDrawResources<'_>,
) {
    let screen = snap_rect(cx, world_rect_to_screen(viewport, fragment.rect));
    let zoom = viewport.camera.zoom;
    let label = fragment.kind.as_str();
    let text = measure_text(cx, draws.text_heading, label);
    let pad_x = TAB_PAD_X * zoom;
    let pad_y = TAB_PAD_Y * zoom;
    // The bevel eats the bottom-right corner, so the plate carries an extra
    // half-bevel of width past the text to keep the last glyph clear of it.
    let bevel = (text.y + pad_y * 2.0) * TAB_BEVEL_FRACTION;
    let tab = Rect {
        pos: screen.pos,
        size: dvec2(text.x + pad_x * 2.0 + bevel * 0.5, text.y + pad_y * 2.0),
    };
    // The plate strokes its own outline in the frame's colour. Its top and left
    // edges sit ON the frame's border, so an unstroked plate erased that stretch
    // of border and left a white notch in the corner of the frame.
    let border = snap_stroke_width(
        cx,
        draws
            .linework
            .thickness(emphasis.thickness(FRAME_THICKNESS)),
    ) as f32;
    let stroke = emphasis.stroke(draws.palette.line, draws.palette);
    draws.pentagon.set_uniform(cx, live_id!(stroke_w), &[border]);
    draws.pentagon.set_uniform(
        cx,
        live_id!(border_col),
        &[stroke.x, stroke.y, stroke.z, stroke.w],
    );
    draws.pentagon.draw_abs(cx, tab);
    draws
        .text_heading
        .draw_abs(cx, dvec2(tab.pos.x + pad_x, tab.pos.y + pad_y), label);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A self-message draws all THREE outer sides of its loop rect and its head
    /// returns to the stem (the rect's left edge), not down at the far corner.
    #[test]
    fn a_self_loop_walks_three_sides_and_returns_to_the_stem() {
        let rect = waml::solve::Rect {
            x: 100.0,
            y: 40.0,
            w: 30.0,
            h: 24.0,
        };
        let (sides, (from, to)) = self_loop_geometry(rect);
        assert_eq!(
            sides,
            [
                ((100.0, 40.0), (130.0, 40.0)),
                ((130.0, 40.0), (130.0, 64.0)),
                ((130.0, 64.0), (100.0, 64.0)),
            ]
        );
        // The arrow runs back along the bottom side, leftward into the stem.
        assert_eq!(from, (130.0, 64.0));
        assert_eq!(to, (100.0, 64.0));
        assert!(to.0 < from.0, "the head must point back at the lifeline");
        assert_eq!(to.1, from.1, "the head must not point downward");
    }
}
