use super::{primitives::ClassDrawResources, RenderSnapshot};
use crate::canvas::primitives::{edge_point_to_screen, fill_rect, font_raster_size};
use crate::edge_labels::{aligned_text_pos, edge_end_labels, EdgeLabel};
use makepad_widgets::*;

const LABEL_PAD: f64 = 3.0;

pub(super) fn draw_edge_labels(
    cx: &mut Cx2d,
    snapshot: &RenderSnapshot<'_>,
    draws: &mut ClassDrawResources<'_>,
) {
    let viewport = snapshot.viewport;
    // Edge text is annotation, not content: it reads well below the card type
    // scale, and at 11 the multiplicity/role chips out-shouted the cards.
    let target_size = (8.0 * viewport.camera.zoom).max(4.0) as f32;
    let font_size = font_raster_size(target_size);
    draws.edge_label.text_style.font_size = font_size;
    draws.edge_label.font_scale = target_size / font_size;

    for edge in &snapshot.scene.edges {
        for label in edge_end_labels(edge, &snapshot.scene.display) {
            draw_label(cx, viewport, draws, label);
        }
    }
}

fn draw_label(
    cx: &mut Cx2d,
    viewport: crate::canvas::viewport::ViewportSnapshot,
    draws: &mut ClassDrawResources<'_>,
    label: EdgeLabel,
) {
    let anchor = edge_point_to_screen(&viewport.camera, viewport.view_rect.pos, label.anchor);
    let measured = draws
        .edge_label
        .layout(cx, 0.0, 0.0, None, false, Align::default(), &label.text)
        .size_in_lpxs;
    let text_size = scaled_text_size(
        dvec2(measured.width as f64, measured.height as f64),
        draws.edge_label.font_scale as f64,
    );
    let text_pos = aligned_text_pos(anchor, text_size, label.align);
    fill_rect(
        cx,
        draws.edge_label_bg,
        Rect {
            pos: text_pos - dvec2(LABEL_PAD, LABEL_PAD),
            size: text_size + dvec2(LABEL_PAD * 2.0, LABEL_PAD * 2.0),
        },
        draws.edge_label_bg.color,
    );
    draws.edge_label.draw_abs(cx, text_pos, &label.text);
}

fn scaled_text_size(measured: DVec2, font_scale: f64) -> DVec2 {
    measured * font_scale
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edge_labels::LabelAlign;

    #[test]
    fn measured_text_size_scales_before_alignment() {
        let anchor = dvec2(100.0, 100.0);
        let measured = dvec2(40.0, 20.0);
        let scaled = scaled_text_size(measured, 0.5);
        assert_eq!(scaled, dvec2(20.0, 10.0));
        assert_eq!(
            aligned_text_pos(anchor, scaled, LabelAlign::Left),
            dvec2(80.0, 100.0)
        );
    }
}
