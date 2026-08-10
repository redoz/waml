use waml::solve::Rect;
use waml_editor::{MeasuredNodeGeometry, UseCaseGeometry};

#[test]
fn structural_hit_bounds_use_the_complete_measured_ellipse() {
    let measured = Rect {
        x: 20.0,
        y: 30.0,
        w: 160.0,
        h: 72.0,
    };
    let geometry = MeasuredNodeGeometry::UseCase(UseCaseGeometry {
        bounds: measured,
        title_bounds: Rect {
            x: 50.0,
            y: 58.0,
            w: 100.0,
            h: 16.0,
        },
        title_lines: vec!["Place order".into()],
        title_line_bounds: vec![Rect {
            x: 50.0,
            y: 58.0,
            w: 100.0,
            h: 16.0,
        }],
    });
    assert_eq!(
        geometry.hit_bounds(Rect {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0
        }),
        measured
    );
}

#[test]
fn class_cards_keep_the_existing_scene_rectangle() {
    let fallback = Rect {
        x: 4.0,
        y: 5.0,
        w: 80.0,
        h: 60.0,
    };
    assert_eq!(
        MeasuredNodeGeometry::ClassCard.hit_bounds(fallback),
        fallback
    );
}
