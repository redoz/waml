use std::collections::BTreeMap;
use waml::model::{Diagram, DiagramDisplay, DiagramGroup};
use waml::solve::{pretty, solve_diagram, Size, SizeMap, SolveConfig};
use waml::syntax::*;

fn bare(name: &str) -> Operand {
    Operand { ref_: OperandRef::Name(NameRef::Bare(name.into())), axis: None, hints: vec![] }
}

#[test]
fn orders_domain_diagram_solves_to_expected_layout() {
    // Users (as column, framed) left of Orders. All nodes 200x90.
    let users_treated = Operand {
        ref_: OperandRef::Name(NameRef::Bare("Users".into())),
        axis: Some(Axis::Column),
        hints: vec![Hint::Shape(Shape::Frame)],
    };
    let diagram = Diagram {
        key: "orders".into(),
        title: "Orders".into(),
        profile: "uml-domain".into(),
        description: None,
        groups: vec![
            DiagramGroup { name: "Users".into(), members: vec!["customer".into(), "account".into()], children: vec![] },
            DiagramGroup { name: "Orders".into(), members: vec!["order".into()], children: vec![] },
        ],
        layout: vec![
            LayoutStatement::Standalone(users_treated),
            LayoutStatement::Placement { operands: vec![bare("Users"), bare("Orders")], directions: vec![Direction::LeftOf] },
        ],
        display: DiagramDisplay::default(),
    };

    let mut sizes: SizeMap = BTreeMap::new();
    for k in ["customer", "account", "order"] {
        sizes.insert(k.into(), Size { w: 200.0, h: 90.0 });
    }

    let (solved, diags) = solve_diagram(&diagram, &sizes, &SolveConfig::default());
    assert!(diags.is_empty(), "no diagnostics: {diags:?}");
    assert_eq!(
        pretty(&solved),
        "node account @ 16,122 200x90\n\
         node customer @ 16,16 200x90\n\
         node order @ 264,69 200x90\n\
         group Frame \"Users\" d0 @ 0,0 232x228\n\
         group Shrink \"Orders\" d0 @ 248,53 232x122\n"
    );
}
