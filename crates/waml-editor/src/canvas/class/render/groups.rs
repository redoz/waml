use super::{primitives::ClassDrawResources, RenderSnapshot};
use crate::canvas::primitives::{font_raster_size, snap_rect, world_rect_to_screen};
use makepad_widgets::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum GroupDraw {
    Chrome,
    Dashed,
    Skip,
}

fn group_draw_mode(shape: waml::layout::Shape, show_hidden: bool) -> GroupDraw {
    match (shape, show_hidden) {
        (waml::layout::Shape::Frame, _) => GroupDraw::Chrome,
        (_, true) => GroupDraw::Dashed,
        (_, false) => GroupDraw::Skip,
    }
}

fn untitled_label(n: usize) -> String {
    format!("Untitled {n}")
}

fn group_label(
    title: &Option<String>,
    untitled_n: Option<usize>,
    mode: GroupDraw,
) -> Option<String> {
    match (title, untitled_n) {
        (Some(title), _) => Some(title.clone()),
        (None, Some(n)) if mode == GroupDraw::Dashed => Some(untitled_label(n)),
        _ => None,
    }
}

fn group_plan(
    groups: &[waml::solve::SolvedGroup],
    show_hidden: bool,
) -> Vec<(GroupDraw, Option<String>)> {
    let mut untitled_seen = 0usize;
    groups
        .iter()
        .map(|group| {
            let mode = group_draw_mode(group.shape, show_hidden);
            let untitled = if group.title.is_none() {
                untitled_seen += 1;
                Some(untitled_seen)
            } else {
                None
            };
            (mode, group_label(&group.title, untitled, mode))
        })
        .collect()
}

pub(super) fn draw_groups(
    cx: &mut Cx2d,
    snapshot: &RenderSnapshot<'_>,
    draws: &mut ClassDrawResources<'_>,
) {
    let zoom = snapshot.viewport.camera.zoom;
    let plan = group_plan(
        &snapshot.scene.groups,
        snapshot.selection.show_hidden_borders,
    );
    let group_draws: Vec<(Rect, Option<String>, GroupDraw)> = snapshot
        .scene
        .groups
        .iter()
        .zip(plan)
        .filter_map(|(group, (mode, label))| {
            if mode == GroupDraw::Skip {
                return None;
            }
            Some((
                world_rect_to_screen(snapshot.viewport, group.rect),
                label,
                mode,
            ))
        })
        .collect();

    let title_ink = draws.text.color;
    let dim_ink = draws.group_title_dim.color;
    for (screen, label, mode) in group_draws {
        // The frame edge is an SDF stroke at a constant screen-space width, so
        // its rect has to sit on the device grid or the hairline splits over two
        // rows -- the "fatter and blurrier when zoomed out" tell.
        let framed = snap_rect(cx, screen);
        match mode {
            GroupDraw::Chrome => {
                draws.group.draw_abs(cx, framed);
                // Fill alone is nearly invisible against the canvas ground, so
                // every frame gets a hairline edge over its fill.
                draws.group_border.set_uniform(
                    cx,
                    live_id!(stroke_w),
                    &[snapshot.linework.group_stroke_width],
                );
                draws.group_border.draw_abs(cx, framed);
            }
            GroupDraw::Dashed => {
                draws.group_dashed.set_uniform(
                    cx,
                    live_id!(dash_px),
                    &[snapshot.linework.group_dash_period],
                );
                draws.group_dashed.set_uniform(
                    cx,
                    live_id!(stroke_w),
                    &[snapshot.linework.group_stroke_width],
                );
                draws.group_dashed.draw_abs(cx, framed);
            }
            GroupDraw::Skip => {}
        }
        if let Some(label) = label {
            draws.text.color = if mode == GroupDraw::Dashed {
                dim_ink
            } else {
                title_ink
            };
            let size = (12.0 * zoom) as f32;
            let font_size = font_raster_size(size);
            draws.text.text_style.font_size = font_size;
            draws.text.font_scale = size / font_size;
            draws.text.draw_abs(
                cx,
                dvec2(screen.pos.x + 6.0 * zoom, screen.pos.y + 4.0 * zoom),
                &label,
            );
        }
    }
    draws.text.color = title_ink;
}

#[cfg(test)]
mod tests {
    use super::*;
    use waml::solve::Rect as WorldRect;

    fn group(title: Option<&str>, shape: waml::layout::Shape) -> waml::solve::SolvedGroup {
        waml::solve::SolvedGroup {
            rect: WorldRect {
                x: 0.0,
                y: 0.0,
                w: 10.0,
                h: 10.0,
            },
            shape,
            title: title.map(str::to_string),
            depth: 0,
        }
    }

    #[test]
    fn only_frame_groups_draw_chrome() {
        use waml::layout::Shape;
        assert_eq!(group_draw_mode(Shape::Frame, false), GroupDraw::Chrome);
        assert_eq!(group_draw_mode(Shape::Frame, true), GroupDraw::Chrome);
        assert_eq!(group_draw_mode(Shape::Box, false), GroupDraw::Skip);
        assert_eq!(group_draw_mode(Shape::Shrink, false), GroupDraw::Skip);
        assert_eq!(group_draw_mode(Shape::Box, true), GroupDraw::Dashed);
        assert_eq!(group_draw_mode(Shape::Shrink, true), GroupDraw::Dashed);
    }

    #[test]
    fn untitled_groups_get_a_one_based_label() {
        assert_eq!(untitled_label(1), "Untitled 1");
        assert_eq!(untitled_label(2), "Untitled 2");
        assert_eq!(untitled_label(12), "Untitled 12");
    }

    #[test]
    fn a_named_group_always_draws_its_own_name() {
        for mode in [GroupDraw::Chrome, GroupDraw::Dashed] {
            assert_eq!(
                group_label(&Some("Users".to_string()), None, mode),
                Some("Users".to_string())
            );
        }
        assert_eq!(group_label(&None, Some(1), GroupDraw::Chrome), None);
        assert_eq!(
            group_label(&None, Some(3), GroupDraw::Dashed),
            Some("Untitled 3".to_string())
        );
        assert_eq!(group_label(&None, None, GroupDraw::Dashed), None);
    }

    #[test]
    fn the_plan_covers_every_group_in_scene_order() {
        use waml::layout::Shape;
        let groups = [
            group(Some("Users"), Shape::Frame),
            group(None, Shape::Box),
            group(Some("Billing"), Shape::Shrink),
        ];
        let plan = group_plan(&groups, false);
        assert_eq!(plan.len(), groups.len());
        assert_eq!(plan[0], (GroupDraw::Chrome, Some("Users".to_string())));
        assert_eq!(plan[1].0, GroupDraw::Skip);
        assert_eq!(plan[2].0, GroupDraw::Skip);
    }

    #[test]
    fn the_untitled_counter_advances_over_groups_that_draw_no_title() {
        use waml::layout::Shape;
        let groups = [
            group(None, Shape::Frame),
            group(None, Shape::Box),
            group(Some("Billing"), Shape::Box),
            group(None, Shape::Box),
        ];
        let off = group_plan(&groups, false);
        assert_eq!(off[0], (GroupDraw::Chrome, None));
        assert_eq!(off[1].0, GroupDraw::Skip);

        let on = group_plan(&groups, true);
        assert_eq!(on[0], (GroupDraw::Chrome, None));
        assert_eq!(on[1], (GroupDraw::Dashed, Some("Untitled 2".to_string())));
        assert_eq!(on[2], (GroupDraw::Dashed, Some("Billing".to_string())));
        assert_eq!(on[3], (GroupDraw::Dashed, Some("Untitled 3".to_string())));
    }
}
