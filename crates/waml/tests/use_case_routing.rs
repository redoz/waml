use waml::solve::{
    route::{boundary_port, PortGeometry},
    Rect,
};

#[test]
fn ellipse_port_lies_on_ellipse_boundary() {
    let ellipse = Rect {
        x: 10.0,
        y: 20.0,
        w: 120.0,
        h: 60.0,
    };
    let point = boundary_port(&PortGeometry::Ellipse(ellipse), (300.0, 50.0));
    let nx = (point.0 - 70.0) / 60.0;
    let ny = (point.1 - 50.0) / 30.0;
    assert!((nx * nx + ny * ny - 1.0).abs() < 0.0001);
    assert!(point.0 < 300.0);
}

#[test]
fn actor_port_uses_figure_not_outer_title_bounds() {
    let actor = PortGeometry::Actor {
        bounds: Rect {
            x: 0.0,
            y: 0.0,
            w: 180.0,
            h: 110.0,
        },
        head_center: (90.0, 12.0),
        head_radius: 10.0,
        stroke_radius: 2.0,
        segments: vec![((90.0, 24.0), (90.0, 72.0)), ((66.0, 44.0), (114.0, 44.0))],
    };
    let point = boundary_port(&actor, (300.0, 44.0));
    assert!(
        point.0 <= 116.0,
        "actor title/outer rectangle was used: {point:?}"
    );
    assert!((point.1 - 44.0).abs() < 0.001);
}

#[test]
fn rectangle_port_remains_on_measured_bounds() {
    let rect = Rect {
        x: 20.0,
        y: 30.0,
        w: 80.0,
        h: 40.0,
    };
    assert_eq!(
        boundary_port(&PortGeometry::Rectangle(rect), (200.0, 50.0)),
        (100.0, 50.0)
    );
}
