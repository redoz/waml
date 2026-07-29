use super::{primitives::ClassDrawResources, relations::relations_for_visibility, RenderSnapshot};
use crate::canvas::primitives::{font_raster_size, world_rect_to_screen};
use crate::frame::SurfaceExt;
use makepad_widgets::*;
use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
struct FocusState {
    selected: bool,
    related: bool,
}

impl FocusState {
    fn coloured(self) -> bool {
        self.selected || self.related
    }
}

fn node_focus_state(
    key: &str,
    selected_key: Option<&str>,
    focus_keys: &HashSet<String>,
) -> FocusState {
    FocusState {
        selected: selected_key == Some(key),
        related: selected_key != Some(key) && focus_keys.contains(key),
    }
}

fn desaturate(color: Vec4) -> Vec4 {
    let luminance = color.x * 0.299 + color.y * 0.587 + color.z * 0.114;
    vec4(luminance, luminance, luminance, color.w)
}

pub(super) fn draw_nodes(
    cx: &mut Cx2d,
    snapshot: &RenderSnapshot<'_>,
    draws: &mut ClassDrawResources<'_>,
) {
    let zoom = snapshot.viewport.camera.zoom;
    draws.node.set_uniform(cx, live_id!(zoom), &[zoom as f32]);

    let focus_keys: HashSet<String> = relations_for_visibility(
        &snapshot.scene.relations,
        snapshot.selection.constraint_visibility,
        snapshot.selection.selected_key.as_deref(),
    )
    .iter()
    .flat_map(|relation| [relation.subject.clone(), relation.reference.clone()])
    .collect();
    let focus_active = !focus_keys.is_empty();
    let selected_key = snapshot.selection.selected_key.as_deref();

    for (index, node) in snapshot.scene.nodes.iter().enumerate() {
        let screen = world_rect_to_screen(snapshot.viewport, node.rect);
        draws.node.set_uniform(
            cx,
            live_id!(selected),
            &[if snapshot.selection.selected_index == Some(index) {
                1.0
            } else {
                0.0
            }],
        );
        let focus = node_focus_state(&node.key, selected_key, &focus_keys);
        let muted = focus_active && !focus.coloured();
        draws
            .node
            .set_uniform(cx, live_id!(grey), &[if muted { 1.0 } else { 0.0 }]);
        draws.node.draw_surface_abs(cx, screen);
        draw_card(cx, screen, node, zoom, muted, draws);
    }
}

fn draw_card(
    cx: &mut Cx2d,
    screen: Rect,
    node: &crate::scene::SceneNode,
    zoom: f64,
    grey: bool,
    draws: &mut ClassDrawResources<'_>,
) {
    use crate::card::{self, Token, Weight};
    use crate::scene::HeaderStyle;

    let placed = card::measure(&card::class_shape(node, &card::mono_sheet()));
    let accent_full = draws.mono_accent.color;
    let amber_full = draws.mono_amber.color;
    let dim = draws.mono_dim.color;
    let accent = if grey {
        desaturate(accent_full)
    } else {
        accent_full
    };
    let amber = if grey {
        desaturate(amber_full)
    } else {
        amber_full
    };
    let card_w = placed.size.0 * zoom;

    if node.header == HeaderStyle::Fill {
        if let Some(bottom) = placed.header_band_bottom() {
            draws.rule.color = vec4(accent.x, accent.y, accent.z, 0.12);
            draws.rule.draw_abs(
                cx,
                Rect {
                    pos: screen.pos,
                    size: dvec2(card_w, bottom * zoom),
                },
            );
        }
    }

    if let Some(dy) = placed.header_divider() {
        draws.rule.color = vec4(accent.x, accent.y, accent.z, 0.22);
        draws.rule.draw_abs(
            cx,
            Rect {
                pos: dvec2(screen.pos.x, screen.pos.y + dy * zoom),
                size: dvec2(card_w, (1.0 * zoom).max(1.0)),
            },
        );
    }

    for dy in placed.compartment_dividers() {
        draws.rule.color = vec4(dim.x, dim.y, dim.z, 0.5);
        draws.rule.draw_abs(
            cx,
            Rect {
                pos: dvec2(screen.pos.x, screen.pos.y + dy * zoom),
                size: dvec2(card_w, (1.0 * zoom).max(1.0)),
            },
        );
    }

    if grey {
        draws.mono_accent.color = accent;
        draws.mono_amber.color = amber;
    }
    for text in &placed.texts {
        let pos = dvec2(screen.pos.x + text.x * zoom, screen.pos.y + text.y * zoom);
        let size = (text.style.size_pt * zoom) as f32;
        let font_size = font_raster_size(size);
        let font_scale = size / font_size;
        match (text.style.weight, text.style.color) {
            (Weight::Bold, _) => {
                draws.mono_bold.text_style.font_size = font_size;
                draws.mono_bold.font_scale = font_scale;
                draws.mono_bold.draw_abs(cx, pos, &text.text);
            }
            (Weight::Regular, Token::Accent) => {
                draws.mono_accent.text_style.font_size = font_size;
                draws.mono_accent.font_scale = font_scale;
                draws.mono_accent.draw_abs(cx, pos, &text.text);
            }
            (Weight::Regular, Token::Amber) => {
                draws.mono_amber.text_style.font_size = font_size;
                draws.mono_amber.font_scale = font_scale;
                draws.mono_amber.draw_abs(cx, pos, &text.text);
            }
            (Weight::Regular, _) => {
                draws.mono_dim.text_style.font_size = font_size;
                draws.mono_dim.font_scale = font_scale;
                draws.mono_dim.draw_abs(cx, pos, &text.text);
            }
        }
    }
    if grey {
        draws.mono_accent.color = accent_full;
        draws.mono_amber.color = amber_full;
    }

    if node.ports {
        let nub = 6.0 * zoom;
        let cy = screen.pos.y + placed.size.1 * 0.5 * zoom - nub * 0.5;
        draws.rule.color = accent;
        draws.rule.draw_abs(
            cx,
            Rect {
                pos: dvec2(screen.pos.x - nub * 0.5, cy),
                size: dvec2(nub, nub),
            },
        );
        draws.rule.draw_abs(
            cx,
            Rect {
                pos: dvec2(screen.pos.x + card_w - nub * 0.5, cy),
                size: dvec2(nub, nub),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_state_splits_selected_neighbour_and_outsider() {
        let focus: HashSet<String> = ["order", "payment-gateway", "user"]
            .into_iter()
            .map(String::from)
            .collect();
        let selected = node_focus_state("order", Some("order"), &focus);
        assert_eq!(
            selected,
            FocusState {
                selected: true,
                related: false
            }
        );
        assert!(selected.coloured());

        let neighbour = node_focus_state("payment-gateway", Some("order"), &focus);
        assert_eq!(
            neighbour,
            FocusState {
                selected: false,
                related: true
            }
        );
        assert!(neighbour.coloured());

        let outsider = node_focus_state("archive", Some("order"), &focus);
        assert_eq!(outsider, FocusState::default());
        assert!(!outsider.coloured());
    }
}
