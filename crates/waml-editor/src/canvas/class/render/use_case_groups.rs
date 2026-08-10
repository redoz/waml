use super::{ClassDrawResources, RenderSnapshot};
use crate::canvas::pen::{self, Pen};
use crate::canvas::primitives::{font_raster_size, world_rect_to_screen};
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

fn paint_commands(groups: &[SceneGroup]) -> Vec<UseCaseGroupCommand> {
    let frames = groups
        .iter()
        .rev()
        .flat_map(commands)
        .filter(|command| matches!(command, UseCaseGroupCommand::Frame { .. }));
    let headings = groups
        .iter()
        .flat_map(commands)
        .filter(|command| matches!(command, UseCaseGroupCommand::Heading { .. }));
    frames.chain(headings).collect()
}

pub(super) fn draw_frames(
    cx: &mut Cx2d,
    snapshot: &RenderSnapshot<'_>,
    draws: &mut ClassDrawResources<'_>,
) {
    // Solved groups are postorder (children before parent). Paint every opaque
    // frame before any heading. This keeps child fills from erasing a parent
    // heading while still putting child frames above their parent fill.
    for command in paint_commands(&snapshot.scene.use_case_groups) {
        if let UseCaseGroupCommand::Frame { bounds } = command {
            let screen = pen::outline(
                cx,
                world_rect_to_screen(snapshot.viewport, bounds),
                Pen::HAIRLINE,
            );
            draws.group.draw_abs(cx, screen);
            draws
                .group_border
                .set_uniform(cx, live_id!(pen_w), &[Pen::HAIRLINE.width() as f32]);
            draws.group_border.draw_abs(cx, screen);
        }
    }
}

pub(super) fn draw_headings(
    cx: &mut Cx2d,
    snapshot: &RenderSnapshot<'_>,
    draws: &mut ClassDrawResources<'_>,
) {
    let zoom = snapshot.viewport.camera.zoom;
    for command in paint_commands(&snapshot.scene.use_case_groups) {
        if let UseCaseGroupCommand::Heading { bounds, text } = command {
            let screen = world_rect_to_screen(snapshot.viewport, bounds);
            draws.group.draw_abs(cx, screen);
            let size = (12.0 * zoom) as f32;
            let font_size = font_raster_size(size);
            draws.text.text_style.font_size = font_size;
            draws.text.font_scale = size / font_size;
            draws.text.draw_abs(cx, screen.pos, &text);
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

    #[test]
    fn nested_frames_are_complete_before_parent_and_child_headings() {
        let child = group(DiagramGroupRole::Band);
        let parent = group(DiagramGroupRole::SystemBoundary);
        let commands = paint_commands(&[child, parent]);
        assert!(matches!(commands[0], UseCaseGroupCommand::Frame { .. }));
        assert!(matches!(commands[1], UseCaseGroupCommand::Frame { .. }));
        assert!(matches!(commands[2], UseCaseGroupCommand::Heading { .. }));
        assert!(matches!(commands[3], UseCaseGroupCommand::Heading { .. }));
    }
}
