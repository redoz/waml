use waml::uml::syntax::UmlSyntaxKind;
use waml::{analysis::analyze_okf, source::SourceBundle, uml};
use waml_syntax::AstNode;

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
fn contains<T: AstNode<waml::uml::syntax::UmlLanguage>>(
    node: waml_syntax::SyntaxNode<waml::uml::syntax::UmlLanguage>,
) -> bool {
    T::cast(node.clone()).is_some()
        || node
            .children()
            .filter_map(waml_syntax::SyntaxElement::into_node)
            .any(contains::<T>)
}

fn descendants(
    node: waml_syntax::SyntaxNode<waml::uml::syntax::UmlLanguage>,
    kind: UmlSyntaxKind,
) -> Vec<waml_syntax::SyntaxNode<waml::uml::syntax::UmlLanguage>> {
    let mut found = Vec::new();
    fn visit(
        node: waml_syntax::SyntaxNode<waml::uml::syntax::UmlLanguage>,
        kind: UmlSyntaxKind,
        found: &mut Vec<waml_syntax::SyntaxNode<waml::uml::syntax::UmlLanguage>>,
    ) {
        if node.kind() == kind {
            found.push(node.clone());
        }
        for child in node
            .children()
            .filter_map(waml_syntax::SyntaxElement::into_node)
        {
            visit(child, kind, found);
        }
    }
    visit(node, kind, &mut found);
    found
}

fn descendant_tokens(
    node: waml_syntax::SyntaxNode<waml::uml::syntax::UmlLanguage>,
) -> Vec<waml_syntax::SyntaxToken<waml::uml::syntax::UmlLanguage>> {
    let mut found = Vec::new();
    fn visit(
        node: waml_syntax::SyntaxNode<waml::uml::syntax::UmlLanguage>,
        found: &mut Vec<waml_syntax::SyntaxToken<waml::uml::syntax::UmlLanguage>>,
    ) {
        for child in node.children() {
            if let Some(token) = child.clone().into_token() {
                found.push(token);
            } else if let Some(node) = child.into_node() {
                visit(node, found);
            }
        }
    }
    visit(node, &mut found);
    found
}

fn diagram_source(layout: &str) -> SourceBundle {
    SourceBundle::try_from_pairs([
        (
            "d.md",
            format!(
                "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Members\n### G\n- [A](./a.md)\n\n## Layout\n{layout}"
            ),
        ),
        ("a.md", "---\ntype: uml.Class\n---\n# A\n".to_string()),
    ])
    .unwrap()
}

#[test]
fn diagram_members_and_layout_are_lossless_and_project_valid_placement() {
    let authored = "---\r\ntype: Diagram\r\ntitle: D\r\n---\r\n# D\r\n\r\n## Members\r\n### G\r\n- [A](./a.md)\r\n\r\n## Layout\r\n- A left of A\r\n";
    let source = SourceBundle::try_from_pairs([
        ("d.md", authored),
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
    ])
    .unwrap();
    let analysis = analyze(&source);
    let diagram = analysis
        .projection
        .diagrams
        .iter()
        .find(|d| d.key == "d")
        .unwrap();
    assert_eq!(diagram.groups[0].members, ["a"]);
    assert!(matches!(
        diagram.layout.as_slice(),
        [waml::syntax::LayoutStatement::Placement { .. }]
    ));
    let id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse("d.md").unwrap())
        .unwrap();
    let root = analysis.syntax.document(id).unwrap().syntax().root();
    assert!(contains::<uml::DiagramMembersSyntax>(root.clone()));
    assert!(contains::<uml::OperandSyntax>(root));
    let root = analysis.syntax.document(id).unwrap().syntax().root();
    let placement = root
        .children()
        .filter_map(waml_syntax::SyntaxElement::into_node)
        .find_map(|node| {
            fn find(
                node: waml_syntax::SyntaxNode<waml::uml::syntax::UmlLanguage>,
            ) -> Option<uml::LayoutPlacementSyntax> {
                if let Some(placement) = uml::LayoutPlacementSyntax::cast(node.clone()) {
                    return Some(placement);
                }
                node.children()
                    .filter_map(waml_syntax::SyntaxElement::into_node)
                    .find_map(find)
            }
            find(node)
        })
        .unwrap();
    assert_eq!(placement.operands().count(), 2);
    assert_eq!(placement.directions().count(), 1);
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

#[test]
fn complete_layout_matrix_matches_legacy_model_and_has_fixed_nested_slots() {
    let lines = [
        "- top of A aligned with center of [A](./a.md)",
        "- column of A, (row of \"two words\", [A](./a.md)) as row with frame, emphasized and small margins right of A",
        "- A above left of A below right of A",
    ];
    let source = diagram_source(&(lines.join("\n") + "\n"));
    let analysis = analyze(&source);
    let diagram = analysis
        .projection
        .diagrams
        .iter()
        .find(|d| d.key == "d")
        .unwrap();
    let expected = lines
        .iter()
        .map(|line| waml::layout::parse_layout_line(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(diagram.layout, expected);

    let id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse("d.md").unwrap())
        .unwrap();
    let root = analysis.syntax.document(id).unwrap().syntax().root();
    assert_eq!(descendants(root.clone(), UmlSyntaxKind::Edge).len(), 2);
    assert_eq!(descendants(root.clone(), UmlSyntaxKind::Hint).len(), 3);
    assert!(descendants(root.clone(), UmlSyntaxKind::Axis).len() >= 3);
    assert!(descendants(root.clone(), UmlSyntaxKind::OperandRef).len() >= 9);
    assert!(descendants(root.clone(), UmlSyntaxKind::NameRef).len() >= 6);
    let chained = descendants(root.clone(), UmlSyntaxKind::LayoutPlacement)
        .into_iter()
        .filter_map(uml::LayoutPlacementSyntax::cast)
        .find(|placement| placement.directions().count() == 2)
        .expect("multi-clause placement has both direct direction slots");
    let operand_ranges = chained
        .operands()
        .map(|operand| operand.syntax().range())
        .collect::<Vec<_>>();
    let direction_ranges = chained
        .directions()
        .map(|direction| direction.syntax().range())
        .collect::<Vec<_>>();
    assert_eq!(operand_ranges.len(), 3);
    assert_eq!(direction_ranges.len(), 2);
    assert!(operand_ranges.windows(2).all(|pair| pair[0] != pair[1]));
    assert!(direction_ranges.windows(2).all(|pair| pair[0] != pair[1]));
    let alignment = descendants(root.clone(), UmlSyntaxKind::LayoutAlignment)
        .into_iter()
        .next()
        .and_then(uml::LayoutAlignmentSyntax::cast)
        .unwrap();
    let left = alignment.left().unwrap();
    assert!(left.edge().is_some());
    assert!(left.operand().is_some());
    let rich_operand = descendants(root.clone(), UmlSyntaxKind::Operand)
        .into_iter()
        .filter_map(uml::OperandSyntax::cast)
        .find(|operand| operand.hints().count() == 3)
        .unwrap();
    assert!(rich_operand.reference().is_some());
    assert!(rich_operand.axis().is_some());
    assert_eq!(rich_operand.hints().count(), 3);
    assert_eq!(
        analysis
            .syntax
            .document(id)
            .unwrap()
            .syntax()
            .write_to_string(),
        source
            .document(&waml::source::BundlePath::parse("d.md").unwrap())
            .unwrap()
            .text()
    );
}

#[test]
fn declared_multi_clause_placement_uses_only_exact_occurrence_ranges() {
    let source = diagram_source("- A above left of A below right of A\n");
    let analysis = analyze(&source);
    let statement = &analysis.declared.concept("d").unwrap().layout[0];
    let uml::DeclaredField::Valid {
        value:
            uml::DeclaredLayoutStatement::Placement {
                operands,
                directions,
            },
        syntax,
    } = statement
    else {
        panic!("expected valid placement");
    };
    assert_eq!(operands.len(), 3);
    assert_eq!(directions.len(), 2);
    for field in operands.iter() {
        let range = match field {
            uml::DeclaredField::Valid { syntax, .. } => syntax.range(),
            _ => panic!("each accepted occurrence is valid"),
        };
        assert_ne!(
            range,
            syntax.range(),
            "must not fall back to statement range"
        );
        assert!(range.start() >= syntax.range().start());
        assert!(range.end() <= syntax.range().end());
        assert!(range.end() > range.start());
    }
    for field in directions.iter() {
        let range = match field {
            uml::DeclaredField::Valid { syntax, .. } => syntax.range(),
            _ => panic!("each accepted occurrence is valid"),
        };
        assert_ne!(
            range,
            syntax.range(),
            "must not fall back to statement range"
        );
        assert!(range.start() >= syntax.range().start());
        assert!(range.end() <= syntax.range().end());
        assert!(range.end() > range.start());
    }
}

#[test]
fn every_malformed_layout_row_has_local_missing_and_non_empty_recovery() {
    let cases = [
        ("- \n", UmlSyntaxKind::LayoutWordToken),
        ("- A left A\n", UmlSyntaxKind::LayoutKeywordToken),
        ("- A above left A\n", UmlSyntaxKind::LayoutKeywordToken),
        ("- A right of\n", UmlSyntaxKind::LayoutWordToken),
        ("- A with frame,\n", UmlSyntaxKind::LayoutWordToken),
        ("- A with unknown-hint\n", UmlSyntaxKind::LayoutWordToken),
        ("- A as diagonal\n", UmlSyntaxKind::LayoutWordToken),
        ("- top of A\n", UmlSyntaxKind::LayoutKeywordToken),
        ("- A trailing unit\n", UmlSyntaxKind::EndOfFileToken),
        ("- column of A,\n", UmlSyntaxKind::LayoutWordToken),
        ("- (A\n", UmlSyntaxKind::LayoutCloseParenToken),
        ("- [A](./a.md\n", UmlSyntaxKind::LayoutLinkToken),
        ("- \"unterminated\n", UmlSyntaxKind::LayoutQuoteToken),
    ];
    for (row, missing_kind) in cases {
        let source = diagram_source(row);
        let analysis = analyze(&source);
        let id = analysis
            .syntax
            .catalog()
            .id_for_path(&waml::source::BundlePath::parse("d.md").unwrap())
            .unwrap();
        let syntax = analysis.syntax.document(id).unwrap().syntax();
        assert_eq!(
            syntax.write_to_string(),
            source
                .document(&waml::source::BundlePath::parse("d.md").unwrap())
                .unwrap()
                .text(),
            "round trip for {row:?}"
        );
        let statement = descendants(syntax.root(), UmlSyntaxKind::LayoutStatement)
            .into_iter()
            .next()
            .unwrap();
        let recovery = descendants(statement.clone(), UmlSyntaxKind::SkippedTokensSyntax);
        assert!(!recovery.is_empty(), "recovery for {row:?}");
        assert!(
            recovery
                .iter()
                .all(|node| node.range().end() > node.range().start()),
            "non-empty recovery for {row:?}"
        );
        assert!(
            recovery
                .iter()
                .all(|node| node.range() != statement.range()),
            "bounded recovery for {row:?}"
        );
        let declared_range = match &analysis.declared.concept("d").unwrap().layout[0] {
            uml::DeclaredField::Incomplete { syntax, .. }
            | uml::DeclaredField::Invalid { syntax, .. } => syntax.range(),
            uml::DeclaredField::Valid { .. } | uml::DeclaredField::Absent => {
                panic!("malformed row must not be valid: {row:?}")
            }
        };
        assert_eq!(
            declared_range,
            recovery[0].range(),
            "declared malformed range is the first invalid unit for {row:?}"
        );
        let missing = descendant_tokens(statement.clone())
            .into_iter()
            .filter(|token| token.flags().is_missing())
            .collect::<Vec<_>>();
        assert!(
            missing.iter().any(|token| token.kind() == missing_kind),
            "expected missing {missing_kind:?} for {row:?}, got {missing:?}"
        );
        assert!(missing.iter().all(|token| {
            token.range().start() >= statement.range().start()
                && token.range().end() <= statement.range().end()
                && token.range().start() == token.range().end()
        }));
        assert!(analysis.projection.diagrams[0].layout.is_empty());
    }
}

#[test]
fn malformed_layout_is_lossless_recovering_and_never_projects_partial_statements() {
    let malformed = [
        "- ",
        "- [A](./a.md",
        "- \"unterminated",
        "- (A",
        "- column of A,",
        "- A left A",
        "- A above left A",
        "- A with frame,",
        "- A as diagonal",
        "- A with unknown-hint",
    ];
    let source = diagram_source(&(malformed.join("\r\n") + "\r\n"));
    let analysis = analyze(&source);
    let diagram = analysis
        .projection
        .diagrams
        .iter()
        .find(|d| d.key == "d")
        .unwrap();
    assert!(diagram.layout.is_empty(), "{:?}", diagram.layout);
    let declared = analysis.declared.concept("d").unwrap();
    assert_eq!(declared.layout.len(), malformed.len());
    assert!(declared
        .layout
        .iter()
        .all(|field| !matches!(field, uml::DeclaredField::Valid { .. })));
    let id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse("d.md").unwrap())
        .unwrap();
    let syntax = analysis.syntax.document(id).unwrap().syntax();
    assert_eq!(
        syntax.write_to_string(),
        source
            .document(&waml::source::BundlePath::parse("d.md").unwrap())
            .unwrap()
            .text()
    );
    assert!(!descendants(syntax.root(), UmlSyntaxKind::SkippedTokensSyntax).is_empty());
    let revision = analysis.syntax.catalog().document(id).unwrap().revision();
    assert!(
        analysis
            .diagnostics
            .iter()
            .filter(|d| d.code == waml::diagnostic::DiagCode::MalformedLayout)
            .count()
            >= malformed.len()
    );
    assert!(analysis.diagnostics.iter().all(|d| {
        d.file == "d.md" && d.document == Some(id) && d.document_revision == Some(revision)
    }));
}

#[test]
fn generic_okf_member_is_declared_and_diagnosed_but_not_projected() {
    let source = SourceBundle::try_from_pairs([
        (
            "d.md",
            "---\ntype: Diagram\n---\n# D\n\n## Members\n### G\n- [Known](./known.md)\n- [Generic](./generic.md)\n",
        ),
        ("known.md", "---\ntype: uml.Class\n---\n# Known\n"),
        ("generic.md", "---\ntype: vendor.Widget\n---\n# Generic\n"),
    ])
    .unwrap();
    let analysis = analyze(&source);
    let declared = analysis.declared.concept("d").unwrap();
    assert_eq!(declared.members.len(), 2);
    let diagram = analysis
        .projection
        .diagrams
        .iter()
        .find(|d| d.key == "d")
        .unwrap();
    assert_eq!(diagram.groups[0].members, ["known"]);
    assert!(analysis.diagnostics.iter().any(|d| {
        d.code == waml::diagnostic::DiagCode::UnresolvedTarget
            && d.file == "d.md"
            && d.document.is_some()
            && d.document_revision.is_some()
            && d.range.is_some()
    }));
}
