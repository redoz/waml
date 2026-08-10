use super::{ClassDrawResources, RenderSnapshot};
use crate::canvas::primitives::{font_raster_size, world_rect_to_screen};
use crate::{ActorGeometry, MeasuredNodeGeometry, Point, UseCaseGeometry};
use makepad_widgets::*;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum UseCaseNodeCommand {
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

pub(super) fn commands(title: &str, geometry: &MeasuredNodeGeometry) -> Vec<UseCaseNodeCommand> {
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
    result.extend(
        use_case
            .title_lines
            .iter()
            .zip(&use_case.title_line_bounds)
            .map(|(line, bounds)| UseCaseNodeCommand::Title {
                bounds: *bounds,
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
    interaction: InteractionInk,
) {
    let zoom = snapshot.viewport.camera.zoom;
    let to_screen = |bounds: waml::solve::Rect| world_rect_to_screen(snapshot.viewport, bounds);
    let old_shape = draws.use_case_ellipse.color;
    let old_line = draws.actor_line.color;
    let old_text = draws.text.color;
    let treatment = interaction_treatment(interaction);
    let ink = match treatment.colour {
        InkColour::Muted => draws.group_title_dim.color,
        InkColour::Accent => draws.mono_accent.color,
        InkColour::Normal => old_text,
    };
    draws.use_case_ellipse.color = ink;
    draws.actor_line.color = ink;
    draws.text.color = ink;
    let stroke = treatment.stroke_width as f32;
    draws
        .use_case_ellipse
        .set_uniform(cx, live_id!(stroke_w), &[stroke]);
    draws
        .actor_line
        .set_uniform(cx, live_id!(stroke_w), &[stroke]);
    for command in commands(&node.title, &node.geometry) {
        match command {
            UseCaseNodeCommand::Ellipse { bounds } => {
                draw_ellipse(cx, draws.use_case_ellipse, to_screen(bounds), stroke as f64);
            }
            UseCaseNodeCommand::Head { center, radius } => {
                draw_ellipse(
                    cx,
                    draws.use_case_ellipse,
                    to_screen(waml::solve::Rect {
                        x: center.x - radius,
                        y: center.y - radius,
                        w: radius * 2.0,
                        h: radius * 2.0,
                    }),
                    stroke as f64,
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
    draws.use_case_ellipse.color = old_shape;
    draws.actor_line.color = old_line;
    draws.text.color = old_text;
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct EllipseSurfaceGeometry {
    surface: waml::solve::Rect,
    center: Point,
    radii: Point,
}

fn ellipse_surface_geometry(
    nominal: waml::solve::Rect,
    stroke_width: f64,
) -> EllipseSurfaceGeometry {
    let padding = stroke_width * 0.5 + 1.0;
    let surface = waml::solve::Rect {
        x: nominal.x - padding,
        y: nominal.y - padding,
        w: nominal.w + padding * 2.0,
        h: nominal.h + padding * 2.0,
    };
    EllipseSurfaceGeometry {
        surface,
        center: Point {
            x: nominal.x + nominal.w * 0.5 - surface.x,
            y: nominal.y + nominal.h * 0.5 - surface.y,
        },
        radii: Point {
            x: nominal.w * 0.5,
            y: nominal.h * 0.5,
        },
    }
}

fn draw_ellipse(cx: &mut Cx2d, draw: &mut DrawColor, nominal: Rect, stroke_width: f64) {
    let geometry = ellipse_surface_geometry(
        waml::solve::Rect {
            x: nominal.pos.x,
            y: nominal.pos.y,
            w: nominal.size.x,
            h: nominal.size.y,
        },
        stroke_width,
    );
    draw.set_uniform(
        cx,
        live_id!(center),
        &[geometry.center.x as f32, geometry.center.y as f32],
    );
    draw.set_uniform(
        cx,
        live_id!(radii),
        &[geometry.radii.x as f32, geometry.radii.y as f32],
    );
    draw.draw_abs(
        cx,
        Rect {
            pos: dvec2(geometry.surface.x, geometry.surface.y),
            size: dvec2(geometry.surface.w, geometry.surface.h),
        },
    );
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct InteractionInk {
    pub selected: bool,
    pub related: bool,
    pub muted: bool,
    pub lift: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InkColour {
    Normal,
    Accent,
    Muted,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct InkTreatment {
    colour: InkColour,
    stroke_width: f64,
}

fn interaction_treatment(interaction: InteractionInk) -> InkTreatment {
    InkTreatment {
        colour: if interaction.muted {
            InkColour::Muted
        } else if interaction.selected || interaction.related {
            InkColour::Accent
        } else {
            InkColour::Normal
        },
        stroke_width: 1.4 + interaction.lift * 1.2,
    }
}

#[cfg(test)]
fn ellipse_distance_px(point: Point, radii: Point) -> f64 {
    let normalized_x = point.x / radii.x;
    let normalized_y = point.y / radii.y;
    let radial = normalized_x.hypot(normalized_y);
    let gradient = (normalized_x / radii.x).hypot(normalized_y / radii.y) / radial.max(0.0001);
    (radial - 1.0) / gradient.max(0.0001)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(selected: bool, related: bool, muted: bool, lift: f64) -> InteractionInk {
        InteractionInk {
            selected,
            related,
            muted,
            lift,
        }
    }

    #[test]
    fn interaction_treatment_covers_selection_hover_focus_and_muting() {
        let selected = interaction_treatment(state(true, false, false, 1.0));
        assert_eq!(selected.colour, InkColour::Accent);
        assert!((selected.stroke_width - 2.6).abs() < 1e-9);
        assert_eq!(
            interaction_treatment(state(false, false, false, 0.5)),
            InkTreatment {
                colour: InkColour::Normal,
                stroke_width: 2.0
            }
        );
        assert_eq!(
            interaction_treatment(state(false, true, false, 0.0)).colour,
            InkColour::Accent
        );
        assert_eq!(
            interaction_treatment(state(false, false, true, 0.0)).colour,
            InkColour::Muted
        );
    }

    #[test]
    fn unequal_title_lines_keep_individual_centered_bounds() {
        let geometry = UseCaseGeometry {
            bounds: waml::solve::Rect {
                x: 0.0,
                y: 0.0,
                w: 160.0,
                h: 72.0,
            },
            title_bounds: waml::solve::Rect {
                x: 20.0,
                y: 20.0,
                w: 120.0,
                h: 32.0,
            },
            title_lines: vec!["A much longer line".into(), "short".into()],
            title_line_bounds: vec![
                waml::solve::Rect {
                    x: 20.0,
                    y: 20.0,
                    w: 120.0,
                    h: 16.0,
                },
                waml::solve::Rect {
                    x: 62.0,
                    y: 36.0,
                    w: 36.0,
                    h: 16.0,
                },
            ],
        };
        let titles: Vec<_> = use_case_commands(&geometry)
            .into_iter()
            .filter_map(|command| match command {
                UseCaseNodeCommand::Title { bounds, .. } => Some(bounds),
                _ => None,
            })
            .collect();
        assert_eq!(titles[0].x, 20.0);
        assert_eq!(titles[1].x, 62.0);
    }

    #[test]
    fn ellipse_distance_is_uniform_at_the_tall_and_wide_axes() {
        let radii = Point { x: 80.0, y: 26.0 };
        let one_pixel_inside_side = ellipse_distance_px(
            Point {
                x: radii.x - 1.0,
                y: 0.0,
            },
            radii,
        );
        let one_pixel_inside_top = ellipse_distance_px(
            Point {
                x: 0.0,
                y: radii.y - 1.0,
            },
            radii,
        );

        assert!((one_pixel_inside_side + 1.0).abs() < 0.03);
        assert!((one_pixel_inside_top + 1.0).abs() < 0.03);
        assert!((one_pixel_inside_side - one_pixel_inside_top).abs() < 0.03);
    }

    #[test]
    fn ellipse_surface_keeps_stroke_antialiasing_outside_the_nominal_bounds() {
        let nominal = waml::solve::Rect {
            x: 20.0,
            y: 30.0,
            w: 160.0,
            h: 52.0,
        };
        let geometry = ellipse_surface_geometry(nominal, 1.4);
        let required_margin = 1.4 * 0.5 + 0.5;

        assert!(nominal.x - geometry.surface.x >= required_margin);
        assert!(nominal.y - geometry.surface.y >= required_margin);
        assert!(geometry.surface.x + geometry.surface.w - nominal.x - nominal.w >= required_margin);
        assert!(geometry.surface.y + geometry.surface.h - nominal.y - nominal.h >= required_margin);
        assert_eq!(geometry.radii, Point { x: 80.0, y: 26.0 });
    }
}
