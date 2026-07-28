use waml::diagnostic::{DiagCode, Severity};
use waml::source::SourceBundle;

fn diagnostics(
    documents: impl IntoIterator<Item = (&'static str, &'static str)>,
) -> Vec<waml::diagnostic::Diagnostic> {
    let source = SourceBundle::try_from_pairs(documents).unwrap();
    waml::analysis::prepare_candidate(source, None, 0)
        .unwrap()
        .uml()
        .diagnostics
        .to_vec()
}

#[test]
fn instance_of_uses_specific_warn_only_diagnostics() {
    let unresolved = diagnostics([(
        "m/order-42.md",
        "---\ntype: uml.InstanceSpecification\n---\n# order-42\n\n## Relationships\n- instance of [Gone](./gone.md)\n",
    )]);
    assert!(unresolved.iter().any(|diagnostic| {
        diagnostic.code == DiagCode::InstanceOfUnresolved
            && diagnostic.severity == Severity::Warning
    }));
    assert!(unresolved
        .iter()
        .all(|diagnostic| diagnostic.code != DiagCode::UnresolvedTarget));

    let non_classifier = diagnostics([
        (
            "m/order-42.md",
            "---\ntype: uml.InstanceSpecification\n---\n# order-42\n\n## Relationships\n- instance of [line-42](./line-42.md)\n",
        ),
        (
            "m/line-42.md",
            "---\ntype: uml.InstanceSpecification\n---\n# line-42\n",
        ),
    ]);
    assert!(non_classifier.iter().any(|diagnostic| {
        diagnostic.code == DiagCode::InstanceOfNonClassifier
            && diagnostic.severity == Severity::Warning
    }));
}

#[test]
fn instance_slots_warn_only_for_unknown_classifier_attributes() {
    let found = diagnostics([
        (
            "m/order.md",
            "---\ntype: uml.Class\n---\n# Order\n\n## Attributes\n- id: OrderId\n",
        ),
        (
            "m/order-42.md",
            "---\ntype: uml.InstanceSpecification\n---\n# order-42\n\n## Relationships\n- instance of [Order](./order.md)\n\n## Slots\n- id: \"ORD-42\"\n- bogus: 3\n",
        ),
    ]);
    let warnings = found
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagCode::SlotUnknownAttribute)
        .collect::<Vec<_>>();
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].severity, Severity::Warning);
    assert!(warnings[0].message.contains("bogus"));
}

#[test]
fn layout_reports_unresolved_refs_and_directed_cycles() {
    let found = diagnostics([
        (
            "d.md",
            "---\ntype: Diagram\nprofile: uml-domain\n---\n# D\n\n## Members\n- [A](./a.md)\n- [B](./b.md)\n\n## Layout\n- A left of Ghost\n- A above B\n- B above A\n",
        ),
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
    ]);
    assert!(found
        .iter()
        .any(|diagnostic| diagnostic.code == DiagCode::UnresolvedLayoutRef));
    assert!(found
        .iter()
        .any(|diagnostic| diagnostic.code == DiagCode::LayoutCycle));
}
