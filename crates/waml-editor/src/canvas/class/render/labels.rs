use super::{primitives::ClassDrawResources, RenderSnapshot};
use crate::canvas::primitives::{fill_rect, font_raster_size, world_rect_to_screen};
use makepad_widgets::*;

const LABEL_PAD: f64 = 3.0;

/// Below this drawn font size the text is unreadable, so labels are skipped
/// entirely rather than painted as illegible smears that still cost fill rate.
const MIN_LEGIBLE_PX: f64 = 5.0;

pub(super) fn draw_edge_labels(
    cx: &mut Cx2d,
    snapshot: &RenderSnapshot<'_>,
    draws: &mut ClassDrawResources<'_>,
) {
    let viewport = snapshot.viewport;
    // Edge text is annotation, not content: it reads well below the card type
    // scale, and at 11 the multiplicity/role chips out-shouted the cards.
    let target_size = (8.0 * viewport.camera.zoom).max(4.0) as f32;
    if (target_size as f64) < MIN_LEGIBLE_PX {
        return;
    }
    let font_size = font_raster_size(target_size);
    draws.edge_label.text_style.font_size = font_size;
    draws.edge_label.font_scale = target_size / font_size;

    for label in &snapshot.scene.labels {
        let screen = world_rect_to_screen(viewport, label.rect);
        fill_rect(
            cx,
            draws.edge_label_bg,
            Rect {
                pos: screen.pos - dvec2(LABEL_PAD, LABEL_PAD),
                size: screen.size + dvec2(LABEL_PAD * 2.0, LABEL_PAD * 2.0),
            },
            draws.edge_label_bg.color,
        );
        draws.edge_label.draw_abs(cx, screen.pos, &label.text);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn min_legible_cutoff_skips_illegible_zoom() {
        // 4px target (below MIN_LEGIBLE_PX=5) at a low zoom is exactly the case
        // that used to paint illegible smears; the guard clause returns before
        // any draw call, so there is nothing to assert on the draw resources --
        // this just documents the threshold so a regression edit trips a diff.
        let target_size = (8.0_f64 * 0.4_f64).max(4.0);
        assert!(target_size < super::MIN_LEGIBLE_PX);
    }
}
