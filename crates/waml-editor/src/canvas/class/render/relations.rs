use super::{
    primitives::{node_screen_rect, ClassDrawResources},
    RenderSnapshot,
};
use crate::canvas::{
    class::{selection::ConstraintVisibility, zone_placed},
    geometry::intersect_rect,
};
use makepad_widgets::*;

const VEIL_REACH: f64 = 420.0;

pub(super) fn relations_for_visibility<'a>(
    relations: &'a [crate::scene::SceneRelation],
    mode: ConstraintVisibility,
    selected_key: Option<&str>,
) -> Vec<&'a crate::scene::SceneRelation> {
    match mode {
        ConstraintVisibility::None => Vec::new(),
        ConstraintVisibility::Selected => {
            let Some(key) = selected_key else {
                return Vec::new();
            };
            relations
                .iter()
                .filter(|relation| relation.subject == key || relation.reference == key)
                .collect()
        }
    }
}

fn reframe_to_selected<'a>(
    subject: &'a str,
    reference: &'a str,
    direction: waml::layout::Direction,
    pov: Option<&str>,
) -> (&'a str, &'a str, waml::layout::Direction) {
    if pov == Some(reference) && pov != Some(subject) {
        (reference, subject, direction.opposite())
    } else {
        (subject, reference, direction)
    }
}

fn veil_band(reference: Rect, direction: waml::layout::Direction, reach: f64) -> Rect {
    use waml::layout::Direction::*;
    let (x, width) = match direction {
        LeftOf | AboveLeft | BelowLeft => (reference.pos.x, reach),
        RightOf | AboveRight | BelowRight => (reference.pos.x + reference.size.x - reach, reach),
        Above | Below => (reference.pos.x - reach, reference.size.x + 2.0 * reach),
    };
    let (y, height) = match direction {
        Above | AboveLeft | AboveRight => (reference.pos.y, reach),
        Below | BelowLeft | BelowRight => (reference.pos.y + reference.size.y - reach, reach),
        LeftOf | RightOf => (reference.pos.y - reach, reference.size.y + 2.0 * reach),
    };
    Rect {
        pos: dvec2(x, y),
        size: dvec2(width, height),
    }
}

fn cross_fade_params(
    band: Rect,
    reference: Rect,
    direction: waml::layout::Direction,
    reach: f64,
) -> ([f32; 2], [f32; 2], [f32; 2]) {
    use waml::layout::Direction::*;
    let flat = (0.5f32, 2.0f32, 1.0f32);
    let axis = |origin: f64, span: f64, ref_center: f64, ref_half: f64| {
        if span <= 0.0 {
            return flat;
        }
        (
            ((ref_center - origin) / span) as f32,
            (ref_half / span) as f32,
            (reach / span).max(1e-4) as f32,
        )
    };
    let (x, y) = match direction {
        Above | Below => (
            axis(
                band.pos.x,
                band.size.x,
                reference.pos.x + reference.size.x * 0.5,
                reference.size.x * 0.5,
            ),
            flat,
        ),
        LeftOf | RightOf => (
            flat,
            axis(
                band.pos.y,
                band.size.y,
                reference.pos.y + reference.size.y * 0.5,
                reference.size.y * 0.5,
            ),
        ),
        _ => (flat, flat),
    };
    ([x.0, y.0], [x.1, y.1], [x.2, y.2])
}

fn veil_ramp(direction: waml::layout::Direction) -> ([f32; 2], [f32; 2]) {
    use waml::layout::Direction::*;
    match direction {
        LeftOf => ([1.0, 0.0], [0.0, -9.0]),
        RightOf => ([-1.0, 0.0], [1.0, -9.0]),
        Above => ([0.0, 1.0], [-9.0, 0.0]),
        Below => ([0.0, -1.0], [-9.0, 1.0]),
        AboveLeft => ([1.0, 1.0], [0.0, 0.0]),
        AboveRight => ([-1.0, 1.0], [1.0, 0.0]),
        BelowLeft => ([1.0, -1.0], [0.0, 1.0]),
        BelowRight => ([-1.0, -1.0], [1.0, 1.0]),
    }
}

fn draw_veil_for(
    cx: &mut Cx2d,
    snapshot: &RenderSnapshot<'_>,
    draws: &mut ClassDrawResources<'_>,
    reference_index: usize,
    direction: waml::layout::Direction,
    active: bool,
) {
    let reference = node_screen_rect(
        snapshot.scene,
        snapshot.viewport,
        &snapshot.placement,
        reference_index,
    );
    let band = intersect_rect(
        veil_band(reference, direction, VEIL_REACH),
        snapshot.viewport.view_rect,
    );
    if band.size.x <= 0.5 || band.size.y <= 0.5 {
        return;
    }
    let (ramp, bias) = veil_ramp(direction);
    draws.veil.set_uniform(cx, live_id!(ramp), &ramp);
    draws.veil.set_uniform(cx, live_id!(bias), &bias);
    let (cross_ctr, cross_plateau, cross_soft) =
        cross_fade_params(band, reference, direction, VEIL_REACH);
    draws.veil.set_uniform(cx, live_id!(cross_ctr), &cross_ctr);
    draws
        .veil
        .set_uniform(cx, live_id!(cross_plateau), &cross_plateau);
    draws
        .veil
        .set_uniform(cx, live_id!(cross_soft), &cross_soft);
    if active {
        draws.veil.set_uniform(cx, live_id!(hatch_px), &[6.5]);
        draws.veil.color = vec4(0.16, 0.52, 0.86, 1.0);
    } else {
        draws.veil.set_uniform(cx, live_id!(hatch_px), &[9.0]);
        draws.veil.color = vec4(0.42, 0.47, 0.54, 1.0);
    }
    draws.veil.draw_abs(cx, band);
}

pub(super) fn draw_relations(
    cx: &mut Cx2d,
    snapshot: &RenderSnapshot<'_>,
    draws: &mut ClassDrawResources<'_>,
) {
    let selected_key = snapshot.selection.selected_key.as_deref();
    let mut chosen: Vec<(usize, waml::layout::Direction, bool)> = relations_for_visibility(
        &snapshot.scene.relations,
        snapshot.selection.constraint_visibility,
        selected_key,
    )
    .into_iter()
    .filter_map(|relation| {
        let (subject, reference, direction) = reframe_to_selected(
            &relation.subject,
            &relation.reference,
            relation.dir,
            selected_key,
        );
        snapshot
            .scene
            .nodes
            .iter()
            .position(|node| node.key == subject)?;
        let reference_index = snapshot
            .scene
            .nodes
            .iter()
            .position(|node| node.key == reference)?;
        Some((reference_index, direction, false))
    })
    .collect();

    if let (Some(zone), Some(reference_key), Some(_)) = (
        snapshot.placement.compass_zone,
        snapshot.placement.armed_target_key.as_deref(),
        snapshot.placement.preview_ghost.as_ref(),
    ) {
        if let (Some(direction), Some(reference_index)) = (
            zone_placed(zone).dir,
            snapshot
                .scene
                .nodes
                .iter()
                .position(|node| node.key == reference_key),
        ) {
            chosen.push((reference_index, direction, true));
        }
    }

    for (reference_index, direction, active) in chosen {
        draw_veil_for(cx, snapshot, draws, reference_index, direction, active);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn veil_band_anchors_and_clamps_per_direction() {
        use waml::layout::Direction::*;
        let reference = Rect {
            pos: dvec2(200.0, 100.0),
            size: dvec2(180.0, 80.0),
        };
        let reach = 300.0;

        let band = veil_band(reference, LeftOf, reach);
        assert_eq!((band.pos.x, band.size.x), (200.0, 300.0));
        assert_eq!((band.pos.y, band.size.y), (-200.0, 680.0));

        let band = veil_band(reference, RightOf, reach);
        assert_eq!(band.pos.x + band.size.x, 380.0);
        assert_eq!(
            (band.size.x, band.pos.y, band.size.y),
            (300.0, -200.0, 680.0)
        );

        let band = veil_band(reference, Above, reach);
        assert_eq!((band.pos.y, band.size.y), (100.0, 300.0));
        assert_eq!((band.pos.x, band.size.x), (-100.0, 780.0));

        let band = veil_band(reference, AboveLeft, reach);
        assert_eq!(
            (band.pos.x, band.pos.y, band.size.x, band.size.y),
            (200.0, 100.0, 300.0, 300.0)
        );
    }

    #[test]
    fn cross_fade_centres_on_the_reference_on_the_unlocked_axis() {
        use waml::layout::Direction::*;
        let reference = Rect {
            pos: dvec2(200.0, 100.0),
            size: dvec2(180.0, 80.0),
        };
        let reach = 300.0;
        let close = |a: f32, b: f32| (a - b).abs() < 1e-4;

        let band = veil_band(reference, LeftOf, reach);
        let (center, plateau, soft) = cross_fade_params(band, reference, LeftOf, reach);
        assert_eq!((center[0], plateau[0], soft[0]), (0.5, 2.0, 1.0));
        assert!(close(center[1], 0.5));
        assert!(close(plateau[1], 40.0 / 680.0));
        assert!(close(soft[1], 300.0 / 680.0));

        let band = veil_band(reference, AboveLeft, reach);
        let (center, plateau, soft) = cross_fade_params(band, reference, AboveLeft, reach);
        assert_eq!((center[0], plateau[0], soft[0]), (0.5, 2.0, 1.0));
        assert_eq!((center[1], plateau[1], soft[1]), (0.5, 2.0, 1.0));
    }

    #[test]
    fn visibility_gates_which_relations_draw() {
        use crate::scene::SceneRelation;
        use waml::layout::Direction;
        let relations = vec![
            SceneRelation {
                subject: "order".into(),
                reference: "customer".into(),
                dir: Direction::LeftOf,
            },
            SceneRelation {
                subject: "payment-gateway".into(),
                reference: "order".into(),
                dir: Direction::Below,
            },
            SceneRelation {
                subject: "a".into(),
                reference: "b".into(),
                dir: Direction::LeftOf,
            },
        ];
        assert!(
            relations_for_visibility(&relations, ConstraintVisibility::None, Some("order"))
                .is_empty()
        );
        assert!(
            relations_for_visibility(&relations, ConstraintVisibility::Selected, None).is_empty()
        );
        let selected =
            relations_for_visibility(&relations, ConstraintVisibility::Selected, Some("order"));
        assert_eq!(selected.len(), 2);
        assert!(selected
            .iter()
            .all(|relation| relation.subject == "order" || relation.reference == "order"));
        assert_eq!(
            ConstraintVisibility::default(),
            ConstraintVisibility::Selected
        );
    }

    #[test]
    fn reframe_puts_the_selected_node_in_the_clear() {
        use waml::layout::Direction;
        assert_eq!(
            reframe_to_selected("a", "b", Direction::LeftOf, Some("a")),
            ("a", "b", Direction::LeftOf)
        );
        assert_eq!(
            reframe_to_selected("a", "b", Direction::LeftOf, Some("b")),
            ("b", "a", Direction::RightOf)
        );
        assert_eq!(
            reframe_to_selected("a", "b", Direction::AboveLeft, Some("b")),
            ("b", "a", Direction::BelowRight)
        );
        assert_eq!(
            reframe_to_selected("a", "b", Direction::Below, None),
            ("a", "b", Direction::Below)
        );
        assert_eq!(
            reframe_to_selected("a", "b", Direction::Below, Some("c")),
            ("a", "b", Direction::Below)
        );
    }
}
