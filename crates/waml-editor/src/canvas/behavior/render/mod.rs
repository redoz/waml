//! `BehaviorSurface` render passes: `Empty` draws the Atlas background and a
//! centered message; `Flow` draws the solved flow scene (Task 7).
//! `Interaction` passes land in Task 8.

mod flow;

use super::hit::BehaviorTarget;
use super::scene::BehaviorScene;
use crate::canvas::viewport::ViewportSnapshot;
use makepad_widgets::*;

pub(super) use flow::FlowDrawResources;

pub(super) struct BehaviorDrawResources<'a> {
    pub(super) bg: &'a mut DrawColor,
    pub(super) text: &'a mut DrawText,
    pub(super) node_box: &'a mut DrawColor,
    pub(super) diamond: &'a mut DrawColor,
    pub(super) circle: &'a mut DrawColor,
    pub(super) triangle: &'a mut DrawColor,
    pub(super) fill: &'a mut DrawColor,
    pub(super) text_heading: &'a mut DrawText,
    pub(super) accent: Vec4,
}

pub(super) fn draw(
    cx: &mut Cx2d,
    viewport: ViewportSnapshot,
    scene: &BehaviorScene,
    hovered: Option<&BehaviorTarget>,
    draws: &mut BehaviorDrawResources<'_>,
) {
    let rect = viewport.view_rect;
    draws.bg.draw_abs(cx, rect);
    match scene {
        BehaviorScene::Empty { message } => draw_message(cx, viewport, message, &mut *draws.text),
        BehaviorScene::Flow {
            nodes,
            edges,
            off_page,
            groups,
        } => {
            let mut flow_draws = FlowDrawResources {
                node_box: &mut *draws.node_box,
                diamond: &mut *draws.diamond,
                circle: &mut *draws.circle,
                triangle: &mut *draws.triangle,
                fill: &mut *draws.fill,
                text_heading: &mut *draws.text_heading,
                text_body: &mut *draws.text,
            };
            flow::draw(
                cx,
                viewport,
                draws.accent,
                nodes,
                edges,
                off_page,
                groups,
                hovered,
                &mut flow_draws,
            )
        }
    }
}

fn draw_message(cx: &mut Cx2d, viewport: ViewportSnapshot, message: &str, text: &mut DrawText) {
    let rect = viewport.view_rect;
    if message.is_empty() {
        return;
    }
    let size = text
        .layout(cx, 0.0, 0.0, None, false, Align::default(), message)
        .size_in_lpxs;
    let pos = dvec2(
        rect.pos.x + (rect.size.x - size.width as f64) * 0.5,
        rect.pos.y + (rect.size.y - size.height as f64) * 0.5,
    );
    text.draw_abs(cx, pos, message);
}
