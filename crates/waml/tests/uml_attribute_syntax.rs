use std::sync::Arc;
use waml::{analysis::analyze_okf, source::SourceBundle, uml};
use waml_syntax::SyntaxElement;

fn analyze(source: &SourceBundle) -> uml::Analysis {
    let okf = analyze_okf(source, None, 1).unwrap();
    uml::analyze(
        waml::analysis::DomainAnalysisContext {
            source,
            catalog: &okf.catalog,
            markdown: &okf.markdown,
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
        ("uml.ActivityDiagram", "behavior"),
        ("uml.StateMachineDiagram", "behavior"),
        ("uml.SequenceDiagram", "behavior"),
        ("uml.ClassDiagram", "diagram"),
        ("uml.UseCaseDiagram", "diagram"),
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

#[test]
fn unresolvable_href_type_ref_has_no_ref_but_keeps_the_label() {
    let source = SourceBundle::try_from_pairs([(
        "order.md",
        "---\ntype: uml.Class\n---\n# Order\n\n## Attributes\n- id: [OrderId](./missing.md) [1]\n",
    )])
    .unwrap();
    let analysis = analyze(&source);
    let attribute = &analysis.declared.concept("order").unwrap().attributes[0];
    match &attribute.ty {
        uml::DeclaredField::Valid { value, .. } => {
            assert_eq!(value.name, "OrderId");
            assert!(value.ref_.is_none());
        }
        _ => panic!("expected a valid type ref"),
    }
}

#[test]
fn only_shell_confirmed_top_level_h2_opens_the_attribute_island() {
    let authored = "---\ntype: uml.Class\n---\n# Order\n\n```md\n## Attributes\n- fenced: Bad [1]\n```\n\n> ## Attributes\n> - quoted: Bad [1]\n\n- ## Attributes\n  - listed: Bad [1]\n\n<div>\n## Attributes\n- html: Bad [1]\n</div>\n\nordinary ## Attributes\n- prose: Bad [1]\n\n## Attributes\n- real: Good [1]\n";
    let source = SourceBundle::try_from_pairs([("order.md", authored)]).unwrap();
    let analysis = analyze(&source);
    let concept = analysis.declared.concept("order").unwrap();
    assert_eq!(
        concept
            .attributes
            .iter()
            .map(|attribute| attribute.syntax.name_token().text().write_to_string())
            .collect::<Vec<_>>(),
        ["real"]
    );
    assert_eq!(
        analysis.projection.node("order").unwrap().attributes[0].name,
        "real"
    );
    let snapshot = analysis
        .syntax
        .document(
            analysis
                .syntax
                .catalog()
                .id_for_path(&waml::source::BundlePath::parse("order.md").unwrap())
                .unwrap(),
        )
        .unwrap();
    assert_eq!(snapshot.syntax().write_to_string(), authored);
}

#[test]
fn every_invalid_present_multiplicity_has_one_exact_located_diagnostic() {
    let authored = "---\ntype: uml.Class\n---\n# Order\n\n## Attributes\n- zero: T [0]\n- reversed: T [5..2]\n- empty: T []\n- open_hi: T [1..]\n- open_lo: T [..5]\n- negative: T [-1]\n- repeated: T [1..2..3]\n- alpha: T [a]\n- unclosed: T [1\n";
    let source = SourceBundle::try_from_pairs([("order.md", authored)]).unwrap();
    let analysis = analyze(&source);
    let expected = [
        (7, (10, 13), "invalid multiplicity"),
        (8, (14, 20), "invalid multiplicity"),
        (9, (11, 13), "invalid multiplicity"),
        (10, (13, 18), "invalid multiplicity"),
        (11, (13, 18), "invalid multiplicity"),
        (12, (14, 18), "invalid multiplicity"),
        (13, (14, 23), "invalid multiplicity"),
        (14, (11, 14), "invalid multiplicity"),
        (15, (14, 16), "unterminated multiplicity"),
    ];
    assert_eq!(analysis.diagnostics.len(), expected.len());
    for (diagnostic, (line, span, message)) in analysis.diagnostics.iter().zip(expected) {
        assert_eq!(
            diagnostic.code,
            waml::diagnostic::DiagCode::MalformedAttribute
        );
        assert_eq!(diagnostic.severity, waml::diagnostic::Severity::Error);
        assert_eq!(diagnostic.line, line);
        assert_eq!(diagnostic.span, Some(span));
        assert_eq!(diagnostic.message, message);
    }
    let concept = analysis.declared.concept("order").unwrap();
    assert!(concept
        .attributes
        .iter()
        .all(|attribute| matches!(attribute.multiplicity, uml::DeclaredField::Invalid { .. })));
    assert!(analysis
        .projection
        .node("order")
        .unwrap()
        .attributes
        .is_empty());
}

#[test]
fn typed_accessors_preserve_fixed_missing_slots_and_recovery() {
    let source = SourceBundle::try_from_pairs([(
        "order.md",
        "---\ntype: uml.Class\n---\n# Order\n\n## Attributes\n- : T\n- name T\n- good: T [1] trailing\n",
    )]).unwrap();
    let analysis = analyze(&source);
    let attributes = &analysis.declared.concept("order").unwrap().attributes;
    assert!(attributes[0].syntax.name_token().flags().is_missing());
    assert!(!attributes[0].syntax.colon_token().flags().is_missing());
    assert!(attributes[1].syntax.colon_token().flags().is_missing());
    let ty = attributes[2].syntax.type_syntax().unwrap();
    assert_eq!(ty.type_token().text().write_to_string(), "T");
    let multiplicity = attributes[2].syntax.multiplicity().unwrap();
    assert_eq!(multiplicity.open_token().text().write_to_string(), "[");
    assert_eq!(multiplicity.value_token().text().write_to_string(), "1");
    assert_eq!(multiplicity.close_token().text().write_to_string(), "]");
    let recovery: Vec<_> = attributes[2].syntax.recovery().collect();
    assert_eq!(recovery.len(), 1);
    let SyntaxElement::Node(recovery) = &recovery[0] else {
        panic!("recovery slot must contain skipped syntax")
    };
    let recovered = recovery.children().next().unwrap().into_token().unwrap();
    assert!(recovered.flags().is_bad());
    assert_eq!(recovered.text().write_to_string(), " trailing");
}

#[test]
fn snapshots_and_diagnostics_expose_catalog_revision_provenance() {
    let source = SourceBundle::try_from_pairs([(
        "order.md",
        "---\ntype: uml.Class\n---\n# Order\n\n## Attributes\n- count: T [0]\n",
    )])
    .unwrap();
    let okf = analyze_okf(&source, None, 41).unwrap();
    assert!(matches!(
        uml::analyze(
            waml::analysis::DomainAnalysisContext {
                source: &source,
                catalog: &okf.catalog,
                markdown: &okf.markdown,
                okf: &okf.bundle,
                session_revision: 42,
            },
            None,
        ),
        Err(waml::analysis::AnalysisError::Specialization { name: "uml", .. })
    ));
    let analysis = uml::analyze(
        waml::analysis::DomainAnalysisContext {
            source: &source,
            catalog: &okf.catalog,
            markdown: &okf.markdown,
            okf: &okf.bundle,
            session_revision: 41,
        },
        None,
    )
    .unwrap();
    assert_eq!(analysis.session_revision(), 41);
    assert!(Arc::ptr_eq(analysis.syntax.catalog(), &okf.catalog));
    assert!(Arc::ptr_eq(analysis.markdown.catalog(), &okf.catalog));
    let id = okf
        .catalog
        .id_for_path(&waml::source::BundlePath::parse("order.md").unwrap())
        .unwrap();
    let _markdown = okf.markdown.document(id).unwrap();
    let uml = analysis.syntax.document(id).unwrap();
    assert!(Arc::ptr_eq(
        okf.catalog.document(id).unwrap(),
        uml.document()
    ));
    assert!(Arc::ptr_eq(
        okf.catalog.document(id).unwrap().text().shared(),
        uml.document().text().shared()
    ));
    let diagnostic = &analysis.diagnostics[0];
    assert_eq!(diagnostic.file, "order.md");
    assert_eq!(diagnostic.document, Some(id));
    assert_eq!(
        diagnostic.document_revision,
        Some(uml.document().revision())
    );
    assert_eq!(
        diagnostic.range,
        Some(
            waml_syntax::TextRange::new(
                waml_syntax::TextSize::try_from_usize(58).unwrap(),
                waml_syntax::TextSize::try_from_usize(61).unwrap(),
            )
            .unwrap()
        )
    );
}

#[test]
fn nested_fenced_bullet_inside_real_attribute_list_stays_markdown() {
    let authored = "---\ntype: uml.Class\n---\n# Order\n\n## Attributes\n- real: Good [1]\n\n  ```text\n  - fenced: Bad [1]\n  ```\n\n      - indented: Bad [1]\n\n  <div>\n  - html: Bad [1]\n  </div>\n";
    let source = SourceBundle::try_from_pairs([("order.md", authored)]).unwrap();
    let analysis = analyze(&source);
    let id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse("order.md").unwrap())
        .unwrap();
    let structure = analysis.markdown.document(id).unwrap().structure();
    let indented = authored.find("- indented").unwrap();
    let indented_line = authored[..indented].rfind('\n').map_or(0, |at| at + 1);
    let opaque: Vec<_> = structure
        .opaque_ranges
        .iter()
        .map(|range| &authored[range.start().to_usize()..range.end().to_usize()])
        .collect();
    assert!(
        structure.opaque_ranges.iter().any(|range| {
            range.start().to_usize() <= indented && indented < range.end().to_usize()
        }),
        "nested indented code must be structurally opaque: {opaque:?}"
    );
    let items: Vec<_> = structure
        .list_item_lines
        .iter()
        .map(|range| &authored[range.start().to_usize()..range.end().to_usize()])
        .collect();
    assert!(
        !structure
            .list_item_lines
            .iter()
            .any(|range| range.start().to_usize() == indented_line),
        "nested indented line must not be a top-level item: {items:?}"
    );
    let tab_items: Vec<_> = structure
        .tab_indented_item_lines
        .iter()
        .map(|range| &authored[range.start().to_usize()..range.end().to_usize()])
        .collect();
    assert!(
        !structure
            .tab_indented_item_lines
            .iter()
            .any(|range| range.start().to_usize() == indented_line),
        "space-indented code must not be a tab item: {tab_items:?}"
    );
    let concept = analysis.declared.concept("order").unwrap();
    assert_eq!(
        concept
            .attributes
            .iter()
            .map(|attribute| attribute.syntax.name_token().text().write_to_string())
            .collect::<Vec<_>>(),
        ["real"]
    );
    assert_eq!(
        analysis.projection.node("order").unwrap().attributes[0].name,
        "real"
    );
    assert!(analysis.diagnostics.is_empty());
    assert_eq!(
        analysis
            .syntax
            .document(id)
            .unwrap()
            .syntax()
            .write_to_string(),
        authored
    );
}

// Regression: a whitespace-only line inside a body section trims away to
// nothing, so the attribute parser's content end landed *before* the
// indentation it had already skipped, and slicing that reversed range paniced
// ("byte range starts at N but ends at N-1"). Found by the uml_islands fuzz
// target.
#[test]
fn whitespace_only_body_line_does_not_panic() {
    let value = "---\ntype: uml.Class\n---\n# Order\n\n## Attributes\n\n- id: OrderId\n   \n";
    let source = SourceBundle::try_from_pairs([("fuzz.md", value.to_string())]).unwrap();
    let analysis = analyze(&source);
    assert_eq!(analysis.syntax.len(), 1);
}
