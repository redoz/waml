use waml::{analysis::analyze_okf, source::SourceBundle, uml};

fn analyze(source: &SourceBundle) -> uml::Analysis {
    let okf = analyze_okf(source, None, 1).unwrap();
    uml::analyze(
        waml::analysis::DomainAnalysisContext {
            source,
            catalog: &okf.catalog,
            shell: &okf.shell,
            structures: &okf.structures,
            okf: &okf.bundle,
            session_revision: 1,
        },
        None,
    )
    .unwrap()
}

#[test]
fn claimed_class_gets_a_uml_analysis_snapshot() {
    let source = SourceBundle::try_from_pairs([(
        "order.md",
        "---\ntype: uml.Class\n---\n# Order\n\n## Attributes\n- + id: OrderId [1]\n",
    )])
    .unwrap();
    let analysis = analyze(&source);
    assert!(analysis.claims.contains("order"));
    assert_eq!(analysis.syntax.len(), 1);
}

#[test]
fn attributes_are_lossless_and_expose_declared_partial_fields() {
    let authored = "---\r\ntype: uml.Class\r\n---\r\n# Café\r\n\r\n## Attributes\r\n\t- + id: OrderId [1]\r\n  - broken Type [x]\r\n  - missing:\r\n";
    let source = SourceBundle::try_from_pairs([("cafe.md", authored)]).unwrap();
    let analysis = analyze(&source);
    let concept = analysis.declared.concept("cafe").unwrap();
    assert_eq!(concept.attributes.len(), 3);
    assert!(matches!(
        concept.attributes[0].name,
        uml::DeclaredField::Valid { .. }
    ));
    assert!(matches!(
        concept.attributes[0].ty,
        uml::DeclaredField::Valid { .. }
    ));
    assert!(matches!(
        concept.attributes[1].ty,
        uml::DeclaredField::Valid { .. }
    ));
    assert!(matches!(
        concept.attributes[2].ty,
        uml::DeclaredField::Incomplete { .. }
    ));
    assert!(analysis
        .diagnostics
        .iter()
        .any(|d| d.code == waml::diagnostic::DiagCode::MalformedAttribute));
    let snapshot = analysis
        .syntax
        .document(
            analysis
                .syntax
                .catalog()
                .id_for_path(&waml::source::BundlePath::parse("cafe.md").unwrap())
                .unwrap(),
        )
        .unwrap();
    assert_eq!(snapshot.syntax().write_to_string(), authored);
}

#[test]
fn unknown_concept_remains_generic_without_uml_snapshot() {
    let source = SourceBundle::try_from_pairs([(
        "vendor.md",
        "---\ntype: vendor.Widget\n---\n# Vendor\n\n## Attributes\n- id: X\n",
    )])
    .unwrap();
    let analysis = analyze(&source);
    assert!(!analysis.claims.contains("vendor"));
    assert_eq!(analysis.syntax.len(), 0);
}

#[test]
fn catalog_claims_each_supported_uml_type_once_and_leaves_generic_types_unclaimed() {
    let accepted = [
        ("uml.Class", "classifier"),
        ("uml.Interface", "classifier"),
        ("uml.Enum", "classifier"),
        ("uml.DataType", "classifier"),
        ("uml.Package", "concept"),
        ("uml.Note", "concept"),
        ("uml.Association", "classifier"),
        ("uml.Actor", "classifier"),
        ("uml.UseCase", "classifier"),
        ("uml.InstanceSpecification", "concept"),
        ("uml.Activity", "behavior"),
        ("uml.StateMachine", "behavior"),
        ("uml.Sequence", "behavior"),
        ("Diagram", "diagram"),
    ];
    let source =
        SourceBundle::try_from_pairs(accepted.iter().enumerate().map(|(index, (ty, _))| {
            (
                format!("case{index}.md"),
                format!("---\ntype: {ty}\n---\n# Case {index}\n"),
            )
        }))
        .unwrap();
    let analysis = analyze(&source);
    assert_eq!(analysis.claims.iter().count(), accepted.len());
    for (index, (_, category)) in accepted.iter().enumerate() {
        let id = format!("case{index}");
        assert!(analysis.claims.contains(&id));
        let document = analysis
            .syntax
            .catalog()
            .id_for_path(&waml::source::BundlePath::parse(format!("{id}.md")).unwrap())
            .unwrap();
        assert!(analysis.syntax.document(document).is_some());
        match *category {
            "classifier" | "concept" => assert!(analysis.projection.node(&id).is_some(), "{id}"),
            "behavior" => assert!(analysis.projection.contains_concept(&id), "{id}"),
            "diagram" => assert!(analysis
                .projection
                .diagrams
                .iter()
                .any(|diagram| diagram.key == id)),
            _ => unreachable!(),
        }
    }
    for ty in ["", "vendor.Widget", "uml.FutureThing", "diagram"] {
        let source = SourceBundle::try_from_pairs([(
            "generic.md",
            format!("---\ntype: {ty}\n---\n# Generic\n"),
        )])
        .unwrap();
        let analysis = analyze(&source);
        assert_eq!(analysis.claims.iter().count(), 0, "{ty}");
        assert_eq!(analysis.syntax.len(), 0, "{ty}");
    }
}

#[test]
fn validated_projection_keeps_only_syntax_valid_attributes() {
    let source = SourceBundle::try_from_pairs([(
        "order.md",
        "---\ntype: uml.Class\n---\n# Order\n\n## Attributes\n- + id: OrderId [1]\n- missing: \n- broken Type [x]\n",
    )])
    .unwrap();
    let analysis = analyze(&source);
    let order = analysis.projection.node("order").unwrap();
    assert_eq!(order.attributes.len(), 1);
    assert_eq!(order.attributes[0].name, "id");
    assert_eq!(order.attributes[0].ty.name, "OrderId");
}

#[test]
fn diagnostics_are_line_relative_not_document_absolute() {
    let source = SourceBundle::try_from_pairs([(
        "order.md",
        "---\ntype: uml.Class\n---\n# Order\n\n## Attributes\n- id Type\n- count: Number [x]\n",
    )])
    .unwrap();
    let analysis = analyze(&source);
    let missing_colon = analysis
        .diagnostics
        .iter()
        .find(|d| d.message == "missing ':' in attribute")
        .unwrap();
    assert_eq!(missing_colon.line, 7);
    assert_eq!(missing_colon.span, Some((0, 9)));
    let invalid_multiplicity = analysis
        .diagnostics
        .iter()
        .find(|d| d.message == "invalid multiplicity")
        .unwrap();
    assert_eq!(invalid_multiplicity.line, 8);
    assert_eq!(invalid_multiplicity.span, Some((16, 19)));
}

#[test]
fn unterminated_multiplicity_is_declared_invalid_not_absent() {
    let source = SourceBundle::try_from_pairs([(
        "order.md",
        "---\ntype: uml.Class\n---\n# Order\n\n## Attributes\n- id: OrderId [1\n",
    )])
    .unwrap();
    let analysis = analyze(&source);
    let attribute = &analysis.declared.concept("order").unwrap().attributes[0];
    assert!(matches!(
        attribute.multiplicity,
        uml::DeclaredField::Invalid { .. }
    ));
    assert!(analysis
        .projection
        .node("order")
        .unwrap()
        .attributes
        .is_empty());
}
