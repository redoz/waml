use super::{ClassDrawResources, RenderSnapshot};
use crate::canvas::primitives::{font_raster_size, world_rect_to_screen};
use crate::{ActorGeometry, MeasuredNodeGeometry, Point, UseCaseGeometry};
use makepad_widgets::*;

#[derive(Debug, Clone, PartialEq)]
pub enum UseCaseNodeCommand {
    Head {
        center: Point,
        radius: f64,
    },
    Segment {
        from: Point,
        to: Point,
    },
    Ellipse {
        bounds: waml::solve::Rect,
    },
    Title {
        bounds: waml::solve::Rect,
        text: String,
    },
}

pub fn commands(title: &str, geometry: &MeasuredNodeGeometry) -> Vec<UseCaseNodeCommand> {
    match geometry {
        MeasuredNodeGeometry::Actor(actor) => actor_commands(title, actor),
        MeasuredNodeGeometry::UseCase(use_case) => use_case_commands(use_case),
        _ => Vec::new(),
    }
}

fn actor_commands(title: &str, actor: &ActorGeometry) -> Vec<UseCaseNodeCommand> {
    let mut result = vec![UseCaseNodeCommand::Head {
        center: actor.head_center,
        radius: actor.head_radius,
    }];
    result.extend(
        [actor.body]
            .into_iter()
            .chain(actor.arms)
            .chain(actor.legs)
            .map(|segment| UseCaseNodeCommand::Segment {
                from: segment.from,
                to: segment.to,
            }),
    );
    result.push(UseCaseNodeCommand::Title {
        bounds: actor.title_bounds,
        text: title.to_string(),
    });
    result
}

fn use_case_commands(use_case: &UseCaseGeometry) -> Vec<UseCaseNodeCommand> {
    let mut result = vec![UseCaseNodeCommand::Ellipse {
        bounds: use_case.bounds,
    }];
    let line_height = use_case.title_bounds.h / use_case.title_lines.len().max(1) as f64;
    result.extend(
        use_case
            .title_lines
            .iter()
            .enumerate()
            .map(|(index, line)| UseCaseNodeCommand::Title {
                bounds: waml::solve::Rect {
                    y: use_case.title_bounds.y + line_height * index as f64,
                    h: line_height,
                    ..use_case.title_bounds
                },
                text: line.clone(),
            }),
    );
    result
}

pub(super) fn draw(
    cx: &mut Cx2d,
    snapshot: &RenderSnapshot<'_>,
    draws: &mut ClassDrawResources<'_>,
    node: &crate::scene::SceneNode,
) {
    let zoom = snapshot.viewport.camera.zoom;
    let to_screen = |bounds: waml::solve::Rect| world_rect_to_screen(snapshot.viewport, bounds);
    for command in commands(&node.title, &node.geometry) {
        match command {
            UseCaseNodeCommand::Ellipse { bounds } => {
                draws.use_case_ellipse.draw_abs(cx, to_screen(bounds));
            }
            UseCaseNodeCommand::Head { center, radius } => {
                draws.use_case_ellipse.draw_abs(
                    cx,
                    to_screen(waml::solve::Rect {
                        x: center.x - radius,
                        y: center.y - radius,
                        w: radius * 2.0,
                        h: radius * 2.0,
                    }),
                );
            }
            UseCaseNodeCommand::Segment { from, to } => {
                let pad = 2.0 / zoom;
                let world = waml::solve::Rect {
                    x: from.x.min(to.x) - pad,
                    y: from.y.min(to.y) - pad,
                    w: (from.x - to.x).abs() + pad * 2.0,
                    h: (from.y - to.y).abs() + pad * 2.0,
                };
                let screen = to_screen(world);
                let local = |point: Point| {
                    let absolute = to_screen(waml::solve::Rect {
                        x: point.x,
                        y: point.y,
                        w: 0.0,
                        h: 0.0,
                    })
                    .pos;
                    [
                        (absolute.x - screen.pos.x) as f32,
                        (absolute.y - screen.pos.y) as f32,
                    ]
                };
                draws
                    .actor_line
                    .set_uniform(cx, live_id!(from), &local(from));
                draws.actor_line.set_uniform(cx, live_id!(to), &local(to));
                draws.actor_line.draw_abs(cx, screen);
            }
            UseCaseNodeCommand::Title { bounds, text } => {
                let screen = to_screen(bounds);
                let size = (12.0 * zoom) as f32;
                let font_size = font_raster_size(size);
                draws.text.text_style.font_size = font_size;
                draws.text.font_scale = size / font_size;
                draws.text.draw_abs(cx, screen.pos, &text);
            }
        }
    }
}
