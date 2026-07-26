use super::{
    primitives::{edge_point_to_screen, fill_rect, font_raster_size, ClassDrawResources},
    RenderSnapshot,
};
use crate::edge_labels::{edge_end_labels, EdgeLabel, LabelAlign};
use makepad_widgets::*;

const LABEL_PAD: f64 = 3.0;

pub(super) fn draw_edge_labels(
    cx: &mut Cx2d,
    snapshot: &RenderSnapshot<'_>,
    draws: &mut ClassDrawResources<'_>,
) {
    let viewport = snapshot.viewport;
    let target_size = (11.0 * viewport.camera.zoom).max(4.0) as f32;
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
    let text_size = dvec2(measured.width as f64, measured.height as f64);
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

fn aligned_text_pos(anchor: DVec2, size: DVec2, align: LabelAlign) -> DVec2 {
    match align {
        LabelAlign::Left => dvec2(anchor.x - size.x, anchor.y),
        LabelAlign::Right => anchor,
        LabelAlign::Above => dvec2(anchor.x - size.x * 0.5, anchor.y - size.y),
        LabelAlign::Below => dvec2(anchor.x - size.x * 0.5, anchor.y),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_alignment_keeps_each_label_in_its_declared_open_direction() {
        let anchor = dvec2(100.0, 100.0);
        let size = dvec2(24.0, 10.0);
        assert_eq!(
            aligned_text_pos(anchor, size, LabelAlign::Left),
            dvec2(76.0, 100.0)
        );
        assert_eq!(aligned_text_pos(anchor, size, LabelAlign::Right), anchor);
        assert_eq!(
            aligned_text_pos(anchor, size, LabelAlign::Above),
            dvec2(88.0, 90.0)
        );
        assert_eq!(
            aligned_text_pos(anchor, size, LabelAlign::Below),
            dvec2(88.0, 100.0)
        );
    }
}
