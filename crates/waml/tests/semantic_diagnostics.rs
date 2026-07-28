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

fn exact<'a>(
    found: &'a [waml::diagnostic::Diagnostic],
    code: DiagCode,
    message: &str,
) -> &'a waml::diagnostic::Diagnostic {
    found
        .iter()
        .find(|diagnostic| diagnostic.code == code && diagnostic.message == message)
        .unwrap_or_else(|| panic!("missing {code:?} {message:?}; got {found:#?}"))
}

fn assert_source_contract(
    diagnostic: &waml::diagnostic::Diagnostic,
    source: &str,
    file: &str,
    severity: Severity,
    line: usize,
    span: (usize, usize),
    ranged: &str,
) {
    assert_eq!(diagnostic.file, file);
    assert_eq!(diagnostic.severity, severity);
    assert_eq!(diagnostic.line, line);
    assert_eq!(diagnostic.span, Some(span));
    assert!(diagnostic.document.is_some());
    assert!(diagnostic.document_revision.is_some());
    let range = diagnostic.range.expect("semantic diagnostic range");
    assert_eq!(
        &source[range.start().to_usize()..range.end().to_usize()],
        ranged
    );
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
fn unknown_slot_warns_even_when_classifier_declares_zero_attributes() {
    let instance = "---\ntype: uml.InstanceSpecification\n---\n# order-42\n\n## Relationships\n- instance of [Order](./order.md)\n\n## Slots\n- bogus: 3\n";
    let found = diagnostics([
        ("m/order.md", "---\ntype: uml.Class\n---\n# Order\n"),
        ("m/order-42.md", instance),
    ]);
    let warning = exact(
        &found,
        DiagCode::SlotUnknownAttribute,
        "slot 'bogus' names no classifier attribute",
    );
    assert_source_contract(
        warning,
        instance,
        "m/order-42.md",
        Severity::Warning,
        10,
        (2, 7),
        "bogus",
    );
}

#[test]
fn unresolved_diagram_member_is_a_precise_warning() {
    let diagram = "---\ntype: Diagram\n---\n# D\n\n## Members\n- [Ghost](../missing/ghost.md)\n";
    let found = diagnostics([("views/d.md", diagram)]);
    let warning = exact(
        &found,
        DiagCode::UnresolvedTarget,
        "unresolved UML member '../missing/ghost.md'",
    );
    assert_source_contract(
        warning,
        diagram,
        "views/d.md",
        Severity::Warning,
        7,
        (10, 29),
        "../missing/ghost.md",
    );
}

#[test]
fn endless_associates_requires_ends_only_between_classifiers() {
    let class = "---\ntype: uml.Class\n---\n# Order\n\n## Relationships\n- associates [Customer](./customer.md)\n";
    let flagged = diagnostics([
        ("c/order.md", class),
        ("c/customer.md", "---\ntype: uml.Class\n---\n# Customer\n"),
    ]);
    let error = exact(
        &flagged,
        DiagCode::MalformedRelationship,
        "'associates' between classifiers requires ': <near> to <far>' multiplicity ends (ends are optional only on an actor↔use-case communication link)",
    );
    assert_source_contract(
        error,
        class,
        "c/order.md",
        Severity::Error,
        7,
        (24, 37),
        "./customer.md",
    );

    let clean = diagnostics([
        (
            "u/place-order.md",
            "---\ntype: uml.UseCase\n---\n# Place Order\n\n## Relationships\n- associates [Customer](./customer.md)\n",
        ),
        (
            "u/customer.md",
            "---\ntype: uml.Actor\n---\n# Customer\n",
        ),
    ]);
    assert!(
        clean
            .iter()
            .all(|diagnostic| diagnostic.code != DiagCode::MalformedRelationship),
        "{clean:#?}"
    );
}

#[test]
fn unresolved_instance_links_is_warn_only_and_precise() {
    let instance = "---\ntype: uml.InstanceSpecification\n---\n# order-42\n\n## Relationships\n- links [line-42](../missing/line-42.md)\n";
    let found = diagnostics([("objects/order-42.md", instance)]);
    let warning = exact(
        &found,
        DiagCode::UnresolvedTarget,
        "unresolved UML target '../missing/line-42.md'",
    );
    assert_source_contract(
        warning,
        instance,
        "objects/order-42.md",
        Severity::Warning,
        7,
        (18, 39),
        "../missing/line-42.md",
    );
    assert!(found.iter().all(|diagnostic| {
        diagnostic.code != DiagCode::UnresolvedTarget || diagnostic.severity == Severity::Warning
    }));
}

#[test]
fn inline_instances_apply_classifier_and_empty_classifier_slot_conformance() {
    let diagram = "---\ntype: Diagram\n---\n# Objects\n\n## Members\n- instance of [Order](../domain/order.md) as order-42 with bogus set to 3\n- instance of [Gone](../domain/gone.md) as gone\n- instance of [Other](../objects/other.md) as other-copy\n";
    let found = diagnostics([
        ("domain/order.md", "---\ntype: uml.Class\n---\n# Order\n"),
        (
            "objects/other.md",
            "---\ntype: uml.InstanceSpecification\n---\n# Other\n",
        ),
        ("views/objects.md", diagram),
    ]);
    let slot = exact(
        &found,
        DiagCode::SlotUnknownAttribute,
        "slot 'bogus' names no classifier attribute",
    );
    assert_source_contract(
        slot,
        diagram,
        "views/objects.md",
        Severity::Warning,
        7,
        (59, 64),
        "bogus",
    );
    let unresolved = exact(
        &found,
        DiagCode::InstanceOfUnresolved,
        "'instance of' target '../domain/gone.md' resolves to no document",
    );
    assert_source_contract(
        unresolved,
        diagram,
        "views/objects.md",
        Severity::Warning,
        8,
        (21, 38),
        "../domain/gone.md",
    );
    let non_classifier = exact(
        &found,
        DiagCode::InstanceOfNonClassifier,
        "'instance of' target 'objects/other' is not a classifier",
    );
    assert_source_contract(
        non_classifier,
        diagram,
        "views/objects.md",
        Severity::Warning,
        9,
        (22, 41),
        "../objects/other.md",
    );
    assert!(found.iter().all(|diagnostic| {
        diagnostic.code != DiagCode::UnresolvedTarget || diagnostic.severity == Severity::Warning
    }));
}

#[test]
fn conformant_inline_instance_has_no_instance_diagnostics() {
    let found = diagnostics([
        (
            "domain/order.md",
            "---\ntype: uml.Class\n---\n# Order\n\n## Attributes\n- id: OrderId\n",
        ),
        (
            "views/objects.md",
            "---\ntype: Diagram\n---\n# Objects\n\n## Members\n- instance of [Order](../domain/order.md) as order-42 with id set to \"ORD-42\"\n",
        ),
    ]);
    assert!(
        found.iter().all(|diagnostic| !matches!(
            diagnostic.code,
            DiagCode::SlotUnknownAttribute
                | DiagCode::InstanceOfNonClassifier
                | DiagCode::InstanceOfUnresolved
                | DiagCode::UnresolvedTarget
        )),
        "{found:#?}"
    );
}

#[test]
fn layout_resolution_matrix_covers_relative_unique_ambiguous_and_unknown_refs() {
    let found = diagnostics([
        (
            "views/d.md",
            "---\ntype: Diagram\n---\n# D\n\n## Layout\n- [Order](../domain/order.md)\n- Customer\n- Missing\n- Order\n",
        ),
        (
            "domain/order.md",
            "---\ntype: uml.Class\n---\n# Order\n",
        ),
        (
            "domain/customer.md",
            "---\ntype: uml.Class\n---\n# Customer\n",
        ),
        (
            "archive/order.md",
            "---\ntype: uml.Class\n---\n# Archived Order\n",
        ),
    ]);
    let warnings = found
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagCode::UnresolvedLayoutRef)
        .collect::<Vec<_>>();
    assert_eq!(warnings.len(), 2, "{found:#?}");
    assert!(warnings.iter().any(|diagnostic| {
        diagnostic.message == "layout operand 'Missing' resolves no member group"
            && diagnostic.severity == Severity::Warning
    }));
    assert!(warnings.iter().any(|diagnostic| {
        diagnostic.message == "layout operand 'Order' resolves no member group"
            && diagnostic.severity == Severity::Warning
    }));
}

#[test]
fn layout_cycle_anchors_first_placement_not_prior_standalone_or_alignment() {
    let diagram = "---\ntype: Diagram\n---\n# D\n\n## Layout\n- A\n- top of A aligned with bottom of B\n- A left of B\n- B left of A\n";
    let found = diagnostics([
        ("views/d.md", diagram),
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
    ]);
    let cycle = exact(
        &found,
        DiagCode::LayoutCycle,
        "layout placement constraints form a cycle (contradictory ordering)",
    );
    assert_source_contract(
        cycle,
        diagram,
        "views/d.md",
        Severity::Error,
        9,
        (1, 13),
        " A left of B",
    );

    let clean = diagnostics([
        (
            "d.md",
            "---\ntype: Diagram\n---\n# D\n\n## Layout\n- A left of B left of C\n- A above D\n",
        ),
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
        ("c.md", "---\ntype: uml.Class\n---\n# C\n"),
        ("d-node.md", "---\ntype: uml.Class\n---\n# D\n"),
    ]);
    assert!(clean
        .iter()
        .all(|diagnostic| diagnostic.code != DiagCode::LayoutCycle));
}
