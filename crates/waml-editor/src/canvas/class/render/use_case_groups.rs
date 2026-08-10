use super::{ClassDrawResources, RenderSnapshot};
use crate::canvas::primitives::{font_raster_size, snap_rect, world_rect_to_screen};
use crate::scene::SceneGroup;
use makepad_widgets::*;
use waml::model::DiagramGroupRole;

#[derive(Debug, Clone, PartialEq)]
pub enum UseCaseGroupCommand {
    Frame {
        bounds: waml::solve::Rect,
    },
    Heading {
        bounds: waml::solve::Rect,
        text: String,
    },
}

pub fn commands(group: &SceneGroup) -> Vec<UseCaseGroupCommand> {
    if group.role == DiagramGroupRole::ExternalActors {
        return Vec::new();
    }
    let mut result = vec![UseCaseGroupCommand::Frame {
        bounds: group.bounds,
    }];
    if let Some(title) = &group.title {
        result.push(UseCaseGroupCommand::Heading {
            bounds: group.heading_bounds,
            text: title.clone(),
        });
    }
    result
}

pub(super) fn draw(
    cx: &mut Cx2d,
    snapshot: &RenderSnapshot<'_>,
    draws: &mut ClassDrawResources<'_>,
) {
    let zoom = snapshot.viewport.camera.zoom;
    for group in &snapshot.scene.use_case_groups {
        for command in commands(group) {
            match command {
                UseCaseGroupCommand::Frame { bounds } => {
                    let screen = snap_rect(cx, world_rect_to_screen(snapshot.viewport, bounds));
                    draws.group.draw_abs(cx, screen);
                    draws.group_border.set_uniform(
                        cx,
                        live_id!(stroke_w),
                        &[snapshot.linework.group_stroke_width],
                    );
                    draws.group_border.draw_abs(cx, screen);
                }
                UseCaseGroupCommand::Heading { bounds, text } => {
                    let screen = world_rect_to_screen(snapshot.viewport, bounds);
                    let size = (12.0 * zoom) as f32;
                    let font_size = font_raster_size(size);
                    draws.text.text_style.font_size = font_size;
                    draws.text.font_scale = size / font_size;
                    draws.text.draw_abs(cx, screen.pos, &text);
                }
            }
        }
    }
}
