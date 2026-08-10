use super::{
    primitives::{node_screen_rect, ClassDrawResources},
    RenderSnapshot,
};
use crate::canvas::pen::Pen;
use crate::canvas::primitives::{fill_rect, world_rect_to_screen};
use makepad_widgets::*;

pub(super) fn draw_conflict_focus(
    cx: &mut Cx2d,
    snapshot: &RenderSnapshot<'_>,
    draws: &mut ClassDrawResources<'_>,
) {
    let Some(keep) = &snapshot.selection.conflict_focus_keys else {
        return;
    };
    for (index, node) in snapshot.scene.nodes.iter().enumerate() {
        if !keep.contains(&node.key) {
            fill_rect(
                cx,
                draws.rule,
                node_screen_rect(
                    snapshot.scene,
                    snapshot.viewport,
                    &snapshot.placement,
                    index,
                ),
                vec4(0.62, 0.65, 0.70, 0.55),
            );
        }
    }
}

pub(super) fn draw_placement(
    cx: &mut Cx2d,
    snapshot: &RenderSnapshot<'_>,
    draws: &mut ClassDrawResources<'_>,
) {
    if !snapshot.placement.drag_moved {
        return;
    }
    let (Some(dragged_key), Some(ghost)) = (
        snapshot.placement.dragged_key.as_deref(),
        snapshot.placement.ghost,
    ) else {
        return;
    };
    let Some(node) = snapshot
        .scene
        .nodes
        .iter()
        .find(|node| node.key == dragged_key)
    else {
        return;
    };

    let ghost_screen = world_rect_to_screen(snapshot.viewport, ghost);
    let origin_screen = world_rect_to_screen(snapshot.viewport, node.rect);
    if snapshot.placement.preview_ghost.is_none() {
        fill_rect(cx, draws.rule, origin_screen, vec4(0.52, 0.57, 0.64, 0.40));
        let grey = vec4(0.62, 0.67, 0.74, 0.85);
        let thickness = Pen::LIGHT.width();
        fill_rect(
            cx,
            draws.rule,
            Rect {
                pos: origin_screen.pos,
                size: dvec2(origin_screen.size.x, thickness),
            },
            grey,
        );
        fill_rect(
            cx,
            draws.rule,
            Rect {
                pos: dvec2(
                    origin_screen.pos.x,
                    origin_screen.pos.y + origin_screen.size.y - thickness,
                ),
                size: dvec2(origin_screen.size.x, thickness),
            },
            grey,
        );
        fill_rect(
            cx,
            draws.rule,
            Rect {
                pos: origin_screen.pos,
                size: dvec2(thickness, origin_screen.size.y),
            },
            grey,
        );
        fill_rect(
            cx,
            draws.rule,
            Rect {
                pos: dvec2(
                    origin_screen.pos.x + origin_screen.size.x - thickness,
                    origin_screen.pos.y,
                ),
                size: dvec2(thickness, origin_screen.size.y),
            },
            grey,
        );
    }

    if snapshot.placement.preview_ghost.is_some() {
        let accent = vec4(0.37, 0.63, 1.0, 0.9);
        let thickness = Pen::REGULAR.width();
        fill_rect(
            cx,
            draws.rule,
            Rect {
                pos: ghost_screen.pos,
                size: dvec2(ghost_screen.size.x, thickness),
            },
            accent,
        );
        fill_rect(
            cx,
            draws.rule,
            Rect {
                pos: dvec2(
                    ghost_screen.pos.x,
                    ghost_screen.pos.y + ghost_screen.size.y - thickness,
                ),
                size: dvec2(ghost_screen.size.x, thickness),
            },
            accent,
        );
        fill_rect(
            cx,
            draws.rule,
            Rect {
                pos: ghost_screen.pos,
                size: dvec2(thickness, ghost_screen.size.y),
            },
            accent,
        );
        fill_rect(
            cx,
            draws.rule,
            Rect {
                pos: dvec2(
                    ghost_screen.pos.x + ghost_screen.size.x - thickness,
                    ghost_screen.pos.y,
                ),
                size: dvec2(thickness, ghost_screen.size.y),
            },
            accent,
        );
    } else {
        fill_rect(cx, draws.rule, ghost_screen, vec4(0.37, 0.63, 1.0, 0.22));
    }

    draws.mono_bold.text_style.font_size = 12.0;
    draws.mono_bold.font_scale = 1.0;
    draws.mono_bold.draw_abs(
        cx,
        dvec2(ghost_screen.pos.x + 6.0, ghost_screen.pos.y + 6.0),
        &node.key,
    );

    if let Some(reference_key) = snapshot.placement.armed_target_key.as_deref() {
        draws.mono_dim.text_style.font_size = 12.0;
        draws.mono_dim.font_scale = 1.0;
        if let Some(direction) = snapshot.placement.placed.dir {
            let line = format!("{} {} {reference_key}", node.key, dir_word(direction));
            draws.mono_dim.draw_abs(
                cx,
                dvec2(
                    snapshot.viewport.view_rect.pos.x + 12.0,
                    snapshot.viewport.view_rect.pos.y + 10.0,
                ),
                &line,
            );
        }
    }
}

fn dir_word(direction: waml::layout::Direction) -> &'static str {
    use waml::layout::Direction::*;
    match direction {
        LeftOf => "left of",
        RightOf => "right of",
        Above => "above",
        Below => "below",
        AboveLeft => "above left of",
        AboveRight => "above right of",
        BelowLeft => "below left of",
        BelowRight => "below right of",
    }
}
