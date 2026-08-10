use waml::{
    analysis::prepare_candidate,
    diagnostic::DiagCode,
    model::DiagramKind,
    source::SourceBundle,
    upgrade::{
        inspect_legacy_diagram_types, LegacyDiagramType, LegacyDiagramTypeUse,
        UpgradeInspectionError,
    },
};

fn bundle(documents: impl IntoIterator<Item = (&'static str, &'static str)>) -> SourceBundle {
    SourceBundle::try_from_pairs(documents).expect("test bundle must be valid")
}

#[test]
fn direct_legacy_behavior_types_map_to_canonical_diagram_kinds() {
    let source = bundle([
        ("activity.md", "---\ntype: uml.Activity\n---\n# Activity\n"),
        ("sequence.md", "---\ntype: uml.Sequence\n---\n# Sequence\n"),
        ("state.md", "---\ntype: uml.StateMachine\n---\n# State\n"),
    ]);

    assert_eq!(
        inspect_legacy_diagram_types(&source).unwrap(),
        vec![
            LegacyDiagramTypeUse {
                path: "activity.md".into(),
                legacy: LegacyDiagramType::Activity,
                replacement: DiagramKind::Activity,
            },
            LegacyDiagramTypeUse {
                path: "sequence.md".into(),
                legacy: LegacyDiagramType::Sequence,
                replacement: DiagramKind::Sequence,
            },
            LegacyDiagramTypeUse {
                path: "state.md".into(),
                legacy: LegacyDiagramType::StateMachine,
                replacement: DiagramKind::StateMachine,
            },
        ]
    );
}

#[test]
fn legacy_diagram_with_only_use_cases_and_neutral_members_becomes_use_case() {
    let source = bundle([
        (
            "views/context.md",
            "---\ntype: Diagram\n---\n# Context\n\n## Members\n- [Checkout](../checkout.md)\n- [Customer](../customer.md)\n- [Boundary](../boundary.md)\n- [Note](../note.md)\n",
        ),
        (
            "checkout.md",
            "---\ntype: uml.UseCase\n---\n# Checkout\n",
        ),
        (
            "customer.md",
            "---\ntype: uml.Actor\n---\n# Customer\n",
        ),
        (
            "boundary.md",
            "---\ntype: uml.Package\n---\n# Boundary\n",
        ),
        ("note.md", "---\ntype: uml.Note\n---\n# Note\n"),
    ]);

    assert_eq!(
        inspect_legacy_diagram_types(&source).unwrap(),
        vec![LegacyDiagramTypeUse {
            path: "views/context.md".into(),
            legacy: LegacyDiagramType::Diagram,
            replacement: DiagramKind::UseCase,
        }]
    );
}

#[test]
fn legacy_er_profile_diagram_becomes_class() {
    let source = bundle([
        (
            "er.md",
            "---\ntype: Diagram\nprofile: er\n---\n# Data\n\n## Members\n- [Order](./order.md)\n",
        ),
        ("order.md", "---\ntype: uml.Class\n---\n# Order\n"),
    ]);

    assert_eq!(
        inspect_legacy_diagram_types(&source).unwrap()[0].replacement,
        DiagramKind::Class
    );
}

#[test]
fn empty_legacy_diagram_becomes_class() {
    let source = bundle([(
        "empty.md",
        "---\ntype: Diagram\n---\n# Empty\n\n## Members\n### Empty group\n",
    )]);

    assert_eq!(
        inspect_legacy_diagram_types(&source).unwrap()[0].replacement,
        DiagramKind::Class
    );
}

#[test]
fn mixed_use_case_and_classifier_members_are_ambiguous_and_sorted() {
    let source = bundle([
        (
            "mixed.md",
            "---\ntype: Diagram\n---\n# Mixed\n\n## Members\n- [Use case](./use-case.md)\n- [Zeta](./zeta.md)\n- [Alpha](./alpha.md)\n",
        ),
        (
            "use-case.md",
            "---\ntype: uml.UseCase\n---\n# Use case\n",
        ),
        (
            "zeta.md",
            "---\ntype: uml.Interface\n---\n# Zeta\n",
        ),
        (
            "alpha.md",
            "---\ntype: uml.DataType\n---\n# Alpha\n",
        ),
    ]);

    assert_eq!(
        inspect_legacy_diagram_types(&source),
        Err(UpgradeInspectionError::AmbiguousLegacyDiagram {
            path: "mixed.md".into(),
            incompatible_members: vec!["alpha".into(), "zeta".into()],
        })
    );
}

#[test]
fn normal_preparation_still_rejects_legacy_diagram_types() {
    let source = bundle([("legacy.md", "---\ntype: Diagram\n---\n# Legacy\n")]);
    let prepared = prepare_candidate(source, None, 0).unwrap();

    assert!(prepared
        .uml()
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == DiagCode::ObsoleteDiagramType));
}
