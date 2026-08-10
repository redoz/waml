use super::{
    primitives::ClassDrawResources, relations::relations_for_visibility, CardMeasureCache,
    RenderSnapshot,
};
use crate::canvas::pen::{self, Pen};
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
    focus_keys: &HashSet<&str>,
) -> FocusState {
    FocusState {
        selected: selected_key == Some(key),
        related: selected_key != Some(key) && focus_keys.contains(key),
    }
}

/// Whether the search spotlight (spec §DocView::reveal, canvas spotlight)
/// dims `key`: `None` means no spotlight is active (never dims); `Some(lit)`
/// dims every node NOT in `lit`. Composed with -- not replacing -- the
/// existing focus-mute below, at the same node-draw site.
fn node_spotlight_dimmed(key: &str, spotlight: Option<&HashSet<String>>) -> bool {
    spotlight.is_some_and(|lit| !lit.contains(key))
}

fn desaturate(color: Vec4) -> Vec4 {
    let luminance = color.x * 0.299 + color.y * 0.587 + color.z * 0.114;
    vec4(luminance, luminance, luminance, color.w)
}

/// Extra screen pixels a node's draw may spill past its layout rect (the port
/// nubs on the left/right edges and the selection-lift shadow), so viewport
/// culling never clips a node whose body is just off-screen but whose
/// decoration is not.
const CULL_PAD: f64 = 32.0;

/// Whether `screen` (inflated by `CULL_PAD`) intersects the visible `view`
/// rect -- both in screen space, the space the draw loop already works in.
/// Hit-testing (`interaction::node_at`) recomputes from scene rects + camera
/// per event, so culling a draw leaves no stale clickable rect behind.
fn on_screen(screen: Rect, view: Rect) -> bool {
    screen.pos.x + screen.size.x + CULL_PAD >= view.pos.x
        && screen.pos.y + screen.size.y + CULL_PAD >= view.pos.y
        && screen.pos.x - CULL_PAD <= view.pos.x + view.size.x
        && screen.pos.y - CULL_PAD <= view.pos.y + view.size.y
}

pub(super) fn draw_nodes(
    cx: &mut Cx2d,
    snapshot: &RenderSnapshot<'_>,
    draws: &mut ClassDrawResources<'_>,
    cards: &mut CardMeasureCache,
) {
    let zoom = snapshot.viewport.camera.zoom;
    draws.node.set_uniform(cx, live_id!(zoom), &[zoom as f32]);
    draws.node.set_uniform(
        cx,
        live_id!(stroke_scale),
        &[snapshot.viewport.camera.stroke_scale()],
    );
    // Always 1.0 on a canvas: drops the zoom-driven stroke-alpha lift and
    // shadow floor that only the non-canvas `AccentFrame` consumers want.
    draws.node.set_uniform(cx, live_id!(screen_space), &[1.0]);

    let focus_keys: HashSet<&str> = relations_for_visibility(
        &snapshot.scene.relations,
        snapshot.selection.constraint_visibility,
        snapshot.selection.selected_key.as_deref(),
    )
    .iter()
    .flat_map(|relation| [relation.subject.as_str(), relation.reference.as_str()])
    .collect();
    let focus_active = !focus_keys.is_empty();
    let selected_key = snapshot.selection.selected_key.as_deref();

    for node in &snapshot.scene.nodes {
        let screen = world_rect_to_screen(snapshot.viewport, node.rect);
        if !on_screen(screen, snapshot.viewport.view_rect) {
            continue;
        }
        let focus = node_focus_state(&node.key, selected_key, &focus_keys);
        let muted = (focus_active && !focus.coloured())
            || node_spotlight_dimmed(&node.key, snapshot.selection.search_spotlight.as_ref());
        if snapshot.scene.visual_kind == crate::StructuralVisualKind::UseCase
            && matches!(
                node.geometry,
                crate::MeasuredNodeGeometry::Actor(_) | crate::MeasuredNodeGeometry::UseCase(_)
            )
        {
            super::use_case_nodes::draw(
                cx,
                snapshot,
                draws,
                node,
                super::use_case_nodes::InteractionInk {
                    selected: focus.selected,
                    related: focus.related,
                    muted,
                    lift: snapshot.selection.lift_for(&node.key),
                },
            );
            continue;
        }
        draws.node.set_uniform(
            cx,
            live_id!(selected),
            &[snapshot.selection.lift_for(&node.key) as f32],
        );
        draws
            .node
            .set_uniform(cx, live_id!(grey), &[if muted { 1.0 } else { 0.0 }]);
        draws.node.draw_surface_abs(cx, screen);
        draw_card(cx, screen, node, zoom, muted, draws, cards);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_card(
    cx: &mut Cx2d,
    screen: Rect,
    node: &crate::scene::SceneNode,
    zoom: f64,
    grey: bool,
    draws: &mut ClassDrawResources<'_>,
    cards: &mut CardMeasureCache,
) {
    use crate::card::{Token, Weight};
    use crate::scene::HeaderStyle;

    let placed = cards.placed(node);
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

    // Dividers are screen-space hairlines, so they must also START on a device
    // pixel: `dy * zoom` is fractional at almost every zoom, and an unsnapped
    // 1-lpx band smears over two rows at half coverage -- reading fatter and
    // blurrier the further you zoom out, which is exactly what CAD linework is
    // supposed to prevent.
    fn rule_rect(cx: &Cx2d, screen: Rect, card_w: f64, dy: f64, zoom: f64, pen: Pen) -> Rect {
        // `pen::fill`, not `pen::outline`: this rect IS the ink, so it must not
        // pick up the `2 * pen` floor a stroke inset needs -- that drew every
        // divider at twice its rung.
        pen::fill(
            cx,
            Rect {
                pos: dvec2(screen.pos.x, screen.pos.y + dy * zoom),
                size: dvec2(card_w, pen.width()),
            },
        )
    }

    if let Some(dy) = placed.header_divider() {
        let rect = rule_rect(cx, screen, card_w, dy, zoom, Pen::HAIRLINE);
        draws.rule.color = vec4(accent.x, accent.y, accent.z, 0.22);
        draws.rule.draw_abs(cx, rect);
    }

    for dy in placed.compartment_dividers() {
        let rect = rule_rect(cx, screen, card_w, dy, zoom, Pen::HAIRLINE);
        draws.rule.color = vec4(dim.x, dim.y, dim.z, 0.5);
        draws.rule.draw_abs(cx, rect);
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
        let nub = super::NUB_SIZE;
        let cy = screen.pos.y + placed.size.1 * 0.5 * zoom - nub * 0.5;
        // Same grid rule as the dividers: a screen-space nub on a fractional
        // edge renders soft and a half pixel wider than its neighbour's.
        let left = pen::fill(
            cx,
            Rect {
                pos: dvec2(screen.pos.x - nub * 0.5, cy),
                size: dvec2(nub, nub),
            },
        );
        let right = pen::fill(
            cx,
            Rect {
                pos: dvec2(screen.pos.x + card_w - nub * 0.5, cy),
                size: dvec2(nub, nub),
            },
        );
        draws.rule.color = accent;
        draws.rule.draw_abs(cx, left);
        draws.rule.draw_abs(cx, right);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_state_splits_selected_neighbour_and_outsider() {
        let focus: HashSet<&str> = ["order", "payment-gateway", "user"].into_iter().collect();
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

    #[test]
    fn spotlight_dims_nodes_outside_the_lit_set_and_clearing_restores() {
        let lit: HashSet<String> = ["order".to_string()].into_iter().collect();

        assert!(!node_spotlight_dimmed("order", Some(&lit)));
        assert!(node_spotlight_dimmed("archive", Some(&lit)));

        // Clearing the spotlight (`None`) dims nothing.
        assert!(!node_spotlight_dimmed("order", None));
        assert!(!node_spotlight_dimmed("archive", None));
    }
}
