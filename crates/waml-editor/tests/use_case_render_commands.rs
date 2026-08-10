use waml::model::DiagramGroupRole;
use waml::solve::Rect;
use waml_editor::{
    use_case_group_commands, use_case_node_commands, ActorGeometry, MeasuredNodeGeometry, Point,
    SceneGroup, Segment, UseCaseGeometry, UseCaseGroupCommand, UseCaseNodeCommand,
};

#[test]
fn actor_commands_use_every_stored_primitive() {
    let segment = |x| Segment {
        from: Point { x, y: 10.0 },
        to: Point { x, y: 20.0 },
    };
    let geometry = MeasuredNodeGeometry::Actor(ActorGeometry {
        bounds: Rect {
            x: 0.0,
            y: 0.0,
            w: 60.0,
            h: 100.0,
        },
        head_center: Point { x: 30.0, y: 12.0 },
        head_radius: 10.0,
        body: segment(30.0),
        arms: [segment(20.0), segment(40.0)],
        legs: [segment(15.0), segment(45.0)],
        title_bounds: Rect {
            x: 5.0,
            y: 80.0,
            w: 50.0,
            h: 16.0,
        },
    });
    let commands = use_case_node_commands("Customer", &geometry);
    assert_eq!(
        commands
            .iter()
            .filter(|c| matches!(c, UseCaseNodeCommand::Head { .. }))
            .count(),
        1
    );
    assert_eq!(
        commands
            .iter()
            .filter(|c| matches!(c, UseCaseNodeCommand::Segment { .. }))
            .count(),
        5
    );
    assert!(commands.contains(&UseCaseNodeCommand::Title {
        bounds: Rect {
            x: 5.0,
            y: 80.0,
            w: 50.0,
            h: 16.0
        },
        text: "Customer".into()
    }));
}

#[test]
fn ellipse_commands_preserve_bounds_and_measured_lines() {
    let bounds = Rect {
        x: 10.0,
        y: 20.0,
        w: 160.0,
        h: 72.0,
    };
    let geometry = MeasuredNodeGeometry::UseCase(UseCaseGeometry {
        bounds,
        title_bounds: Rect {
            x: 30.0,
            y: 40.0,
            w: 120.0,
            h: 32.0,
        },
        title_lines: vec!["Place".into(), "order".into()],
    });
    let commands = use_case_node_commands("ignored", &geometry);
    assert_eq!(
        commands.first(),
        Some(&UseCaseNodeCommand::Ellipse { bounds })
    );
    assert_eq!(
        commands
            .iter()
            .filter(|c| matches!(c, UseCaseNodeCommand::Title { .. }))
            .count(),
        2
    );
}

fn group(role: DiagramGroupRole) -> SceneGroup {
    SceneGroup {
        role,
        bounds: Rect {
            x: 1.0,
            y: 2.0,
            w: 3.0,
            h: 4.0,
        },
        heading_bounds: Rect {
            x: 2.0,
            y: 3.0,
            w: 2.0,
            h: 1.0,
        },
        title: Some("System".into()),
        depth: 0,
    }
}

#[test]
fn actor_rails_are_frameless_while_boundaries_and_bands_draw() {
    assert!(use_case_group_commands(&group(DiagramGroupRole::ExternalActors)).is_empty());
    for role in [DiagramGroupRole::SystemBoundary, DiagramGroupRole::Band] {
        let commands = use_case_group_commands(&group(role));
        assert!(matches!(commands[0], UseCaseGroupCommand::Frame { .. }));
        assert!(matches!(commands[1], UseCaseGroupCommand::Heading { .. }));
    }
}
