use super::{ClassDrawResources, RenderSnapshot};
use crate::canvas::primitives::{font_raster_size, snap_rect, world_rect_to_screen};
use crate::{scene::SceneGroup, GroupVisualKind, StructuralVisualKind, StructuralVisualPolicy};
use makepad_widgets::*;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum UseCaseGroupCommand {
    Frame {
        bounds: waml::solve::Rect,
    },
    Heading {
        bounds: waml::solve::Rect,
        text: String,
    },
}

pub(super) fn commands(group: &SceneGroup) -> Vec<UseCaseGroupCommand> {
    let kind = StructuralVisualPolicy {
        kind: StructuralVisualKind::UseCase,
    }
    .group_kind(group.role);
    if matches!(kind, GroupVisualKind::ActorRail | GroupVisualKind::Generic) {
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
    // Solved groups are postorder (children before parent). Paint in reverse so
    // an opaque parent boundary cannot cover its nested band frames/headings.
    for group in snapshot.scene.use_case_groups.iter().rev() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use waml::model::DiagramGroupRole;

    fn group(role: DiagramGroupRole) -> SceneGroup {
        SceneGroup {
            role,
            bounds: waml::solve::Rect {
                x: 0.0,
                y: 0.0,
                w: 10.0,
                h: 10.0,
            },
            heading_bounds: waml::solve::Rect {
                x: 1.0,
                y: 1.0,
                w: 8.0,
                h: 2.0,
            },
            title: Some("Group".into()),
            depth: 0,
        }
    }

    #[test]
    fn policy_frames_only_boundaries_and_bands() {
        assert!(commands(&group(DiagramGroupRole::ExternalActors)).is_empty());
        assert!(commands(&group(DiagramGroupRole::Generic)).is_empty());
        assert!(!commands(&group(DiagramGroupRole::SystemBoundary)).is_empty());
        assert!(!commands(&group(DiagramGroupRole::Band)).is_empty());
    }
}
