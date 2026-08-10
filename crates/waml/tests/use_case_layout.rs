use waml::layout::{Axis, Direction, Hint, LayoutStatement, NameRef, Operand, OperandRef, Shape};
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
        "\n## Members\n\n### Actors\n- [Primary](./primary.md)\n- [Secondary](./secondary.md)\n\n### Checkout\n\n#### Work\n- [First](./first.md)\n- [Second](./second.md)\n- [Third](./third.md)\n",
        &[
            ("primary.md", "uml.Actor"),
            ("secondary.md", "uml.Actor"),
            ("first.md", "uml.UseCase"),
            ("second.md", "uml.UseCase"),
            ("third.md", "uml.UseCase"),
        ],
    );
    let relationships = vec![
        (BoxId::Node("third".into()), BoxId::Node("primary".into())),
        (BoxId::Node("first".into()), BoxId::Node("secondary".into())),
        (
            BoxId::Node("second".into()),
            BoxId::Node("secondary".into()),
        ),
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

#[test]
fn large_band_uses_a_balanced_stable_grid() {
    let diagram = diagram(
        "\n## Members\n\n### Boundary\n\n#### Work\n- [A](./a.md)\n- [B](./b.md)\n- [C](./c.md)\n- [D](./d.md)\n- [E](./e.md)\n- [F](./f.md)\n",
        &[("a.md", "uml.UseCase"), ("b.md", "uml.UseCase"), ("c.md", "uml.UseCase"), ("d.md", "uml.UseCase"), ("e.md", "uml.UseCase"), ("f.md", "uml.UseCase")],
    );
    let scene = resolve_use_case(&diagram, &[]).0;
    assert!(scene.constraints.contains(&Constraint::Place {
        a: BoxId::Node("a".into()),
        b: BoxId::Node("d".into()),
        dir: Direction::Above,
    }));
    assert!(scene.constraints.contains(&Constraint::Place {
        a: BoxId::Node("d".into()),
        b: BoxId::Node("e".into()),
        dir: Direction::LeftOf,
    }));
    assert!(!scene.constraints.contains(&Constraint::Place {
        a: BoxId::Node("c".into()),
        b: BoxId::Node("d".into()),
        dir: Direction::LeftOf,
    }));
}

#[test]
fn all_actor_rails_stay_left_and_keep_authored_member_order() {
    let diagram = diagram(
        "\n## Members\n\n### Customers\n- [Buyer](./buyer.md)\n- [Guest](./guest.md)\n\n### Staff\n- [Admin](./admin.md)\n\n### Boundary\n- [Pay](./pay.md)\n",
        &[("buyer.md", "uml.Actor"), ("guest.md", "uml.Actor"), ("admin.md", "uml.Actor"), ("pay.md", "uml.UseCase")],
    );
    let relationships = [(BoxId::Node("guest".into()), BoxId::Node("pay".into()))];
    let scene = resolve_use_case(&diagram, &relationships).0;
    for actor_group in [0, 1] {
        assert!(scene.constraints.contains(&Constraint::Place {
            a: BoxId::Group(actor_group),
            b: BoxId::Group(2),
            dir: Direction::LeftOf,
        }));
    }
    assert!(scene.constraints.contains(&Constraint::Place {
        a: BoxId::Node("buyer".into()),
        b: BoxId::Node("guest".into()),
        dir: Direction::Above,
    }));
}

#[test]
fn authored_shrink_shape_is_not_replaced_by_boundary_default() {
    let mut diagram = diagram(
        "\n## Members\n\n### Boundary\n- [Pay](./pay.md)\n",
        &[("pay.md", "uml.UseCase")],
    );
    diagram.layout.push(LayoutStatement::Standalone(Operand {
        ref_: OperandRef::Name(NameRef::Bare("Boundary".into())),
        axis: None,
        hints: vec![Hint::Shape(Shape::Shrink)],
    }));
    let scene = resolve_use_case(&diagram, &[]).0;
    assert_eq!(
        scene
            .boxes
            .iter()
            .find(|item| item.id == BoxId::Group(0))
            .unwrap()
            .shape,
        Shape::Shrink
    );
}
