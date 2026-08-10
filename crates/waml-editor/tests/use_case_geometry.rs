use waml::source::SourceBundle;
use waml_editor::{
    measure_node, MeasuredNodeGeometry, MonoTextMeasurer, Point, StructuralVisualKind,
    StructuralVisualPolicy,
};

fn node(ty: &str, title: &str) -> waml::model::Node {
    let source =
        SourceBundle::try_from_pairs([("node.md", format!("---\ntype: {ty}\n---\n# {title}\n"))])
            .unwrap();
    waml::analysis::prepare_candidate(source, None, 0)
        .unwrap()
        .uml()
        .projection
        .nodes[0]
        .clone()
}

fn policy() -> StructuralVisualPolicy {
    StructuralVisualPolicy {
        kind: StructuralVisualKind::UseCase,
    }
}

fn contains(rect: waml::solve::Rect, point: Point) -> bool {
    point.x >= rect.x
        && point.x <= rect.x + rect.w
        && point.y >= rect.y
        && point.y <= rect.y + rect.h
}

#[test]
fn actor_bounds_include_figure_and_title_and_grow_for_long_names() {
    let short = measure_node(policy(), &node("uml.Actor", "Buyer"), &MonoTextMeasurer);
    let long = measure_node(
        policy(),
        &node("uml.Actor", "International purchasing administrator"),
        &MonoTextMeasurer,
    );
    let MeasuredNodeGeometry::Actor(short) = short else {
        panic!("actor geometry")
    };
    let MeasuredNodeGeometry::Actor(long) = long else {
        panic!("actor geometry")
    };
    assert!(long.bounds.w > short.bounds.w);
    for point in [
        short.head_center,
        short.body.from,
        short.body.to,
        short.arms[0].to,
        short.arms[1].to,
        short.legs[0].to,
        short.legs[1].to,
    ] {
        assert!(contains(short.bounds, point));
    }
    assert!(short.title_bounds.y >= short.head_center.y + short.head_radius);
    assert!(contains(
        short.bounds,
        Point {
            x: short.title_bounds.x + short.title_bounds.w,
            y: short.title_bounds.y + short.title_bounds.h,
        }
    ));
}

#[test]
fn use_case_wraps_centers_and_grows_the_ellipse() {
    let short = measure_node(policy(), &node("uml.UseCase", "Pay"), &MonoTextMeasurer);
    let wrapped = measure_node(
        policy(),
        &node(
            "uml.UseCase",
            "Review and approve an international purchase request",
        ),
        &MonoTextMeasurer,
    );
    let MeasuredNodeGeometry::UseCase(short) = short else {
        panic!("use-case geometry")
    };
    let MeasuredNodeGeometry::UseCase(wrapped) = wrapped else {
        panic!("use-case geometry")
    };
    assert_eq!(short.title_lines, ["Pay"]);
    assert!(wrapped.title_lines.len() > 1);
    assert!(wrapped.bounds.h >= short.bounds.h);
    assert!(
        (wrapped.title_bounds.x + wrapped.title_bounds.w / 2.0
            - (wrapped.bounds.x + wrapped.bounds.w / 2.0))
            .abs()
            < 0.001
    );
}

#[test]
fn final_visible_line_is_ellipsized_inside_bounded_geometry() {
    let title = (0..40).map(|_| "requirement").collect::<Vec<_>>().join(" ");
    let measured = measure_node(policy(), &node("uml.UseCase", &title), &MonoTextMeasurer);
    let MeasuredNodeGeometry::UseCase(measured) = measured else {
        panic!("use-case geometry")
    };
    assert_eq!(measured.title_lines.len(), 3);
    assert!(measured.title_lines.last().unwrap().ends_with('…'));
    assert!(contains(
        measured.bounds,
        Point {
            x: measured.title_bounds.x + measured.title_bounds.w,
            y: measured.title_bounds.y + measured.title_bounds.h,
        }
    ));
}

#[test]
fn measurement_and_translated_hit_bounds_are_deterministic() {
    let node = node("uml.UseCase", "Submit purchase request");
    let first = measure_node(policy(), &node, &MonoTextMeasurer);
    let second = measure_node(policy(), &node, &MonoTextMeasurer);
    assert_eq!(first, second);
    let translated = first.translated(40.0, 80.0);
    let bounds = translated.bounds().unwrap();
    assert_eq!((bounds.x, bounds.y), (40.0, 80.0));
}
