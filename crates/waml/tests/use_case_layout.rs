use waml::layout::{Axis, Direction, Shape};
use waml::solve::{use_case::resolve_use_case, BoxId, Constraint};
use waml::source::SourceBundle;

fn diagram(body: &str, nodes: &[(&str, &str)]) -> waml::model::Diagram {
    let mut documents = vec![(
        "diagram.md".to_string(),
        format!("---\ntype: uml.UseCaseDiagram\n---\n# Use cases\n{body}"),
    )];
    documents.extend(nodes.iter().map(|(path, ty)| {
        (
            (*path).to_string(),
            format!("---\ntype: {ty}\n---\n# {path}\n"),
        )
    }));
    waml::analysis::prepare_candidate(SourceBundle::try_from_pairs(documents).unwrap(), None, 0)
        .unwrap()
        .uml()
        .projection
        .diagrams[0]
        .clone()
}

#[test]
fn actors_are_left_of_boundaries_and_stack_in_authored_order() {
    let diagram = diagram(
        "\n## Members\n\n### People\n- [Buyer](./buyer.md)\n- [Admin](./admin.md)\n\n### Checkout\n- [Pay](./pay.md)\n",
        &[
            ("buyer.md", "uml.Actor"),
            ("admin.md", "uml.Actor"),
            ("pay.md", "uml.UseCase"),
        ],
    );
    let (scene, diagnostics) = resolve_use_case(&diagram, &[]);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    assert!(scene.constraints.contains(&Constraint::Place {
        a: BoxId::Group(0),
        b: BoxId::Group(1),
        dir: Direction::LeftOf,
    }));
    assert!(scene.constraints.contains(&Constraint::Place {
        a: BoxId::Node("buyer".into()),
        b: BoxId::Node("admin".into()),
        dir: Direction::Above,
    }));
    assert_eq!(
        scene
            .boxes
            .iter()
            .find(|b| b.id == BoxId::Group(0))
            .unwrap()
            .axis,
        Some(Axis::Column)
    );
    assert_eq!(
        scene
            .boxes
            .iter()
            .find(|b| b.id == BoxId::Group(1))
            .unwrap()
            .shape,
        Shape::Frame
    );
}

#[test]
fn bands_keep_authored_order_and_members_use_stable_rows() {
    let diagram = diagram(
        "\n## Members\n\n### Checkout\n\n#### Purchase\n- [Browse](./browse.md)\n- [Pay](./pay.md)\n\n#### Support\n- [Refund](./refund.md)\n",
        &[
            ("browse.md", "uml.UseCase"),
            ("pay.md", "uml.UseCase"),
            ("refund.md", "uml.UseCase"),
        ],
    );
    let (scene, _) = resolve_use_case(&diagram, &[]);
    assert!(scene.constraints.contains(&Constraint::Place {
        a: BoxId::Group(1),
        b: BoxId::Group(2),
        dir: Direction::Above,
    }));
    assert!(scene.constraints.contains(&Constraint::Place {
        a: BoxId::Node("browse".into()),
        b: BoxId::Node("pay".into()),
        dir: Direction::LeftOf,
    }));
    assert_eq!(
        scene
            .boxes
            .iter()
            .find(|b| b.id == BoxId::Group(1))
            .unwrap()
            .shape,
        Shape::Box
    );
}

#[test]
fn relationship_adjacency_reorders_only_members_and_is_deterministic() {
    let diagram = diagram(
        "\n## Members\n\n### Checkout\n\n#### Work\n- [First](./first.md)\n- [Second](./second.md)\n- [Third](./third.md)\n",
        &[
            ("first.md", "uml.UseCase"),
            ("second.md", "uml.UseCase"),
            ("third.md", "uml.UseCase"),
        ],
    );
    let relationships = vec![
        (BoxId::Node("third".into()), BoxId::Node("first".into())),
        (BoxId::Node("third".into()), BoxId::Node("second".into())),
    ];
    let first = resolve_use_case(&diagram, &relationships).0;
    for _ in 0..10 {
        assert_eq!(resolve_use_case(&diagram, &relationships).0, first);
    }
    assert!(first.constraints.iter().any(|constraint| matches!(
        constraint,
        Constraint::Place { a: BoxId::Node(key), .. } if key == "third"
    )));
}
