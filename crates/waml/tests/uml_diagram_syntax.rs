use waml::model::{BehaviorKind, DiagramKind, ElementType};
use waml::uml::syntax::UmlSyntaxKind;
use waml::{analysis::analyze_okf, source::SourceBundle, uml};
use waml_syntax::AstNode;

#[test]
fn canonical_diagram_kinds_round_trip() {
    let cases = [
        ("uml.ClassDiagram", DiagramKind::Class),
        ("uml.UseCaseDiagram", DiagramKind::UseCase),
        ("uml.ActivityDiagram", DiagramKind::Activity),
        ("uml.StateMachineDiagram", DiagramKind::StateMachine),
        ("uml.SequenceDiagram", DiagramKind::Sequence),
    ];
    for (name, kind) in cases {
        assert_eq!(DiagramKind::parse(name), Some(kind));
        assert_eq!(kind.as_str(), name);
    }
}

#[test]
fn canonical_diagrams_are_views_and_retired_names_are_not_aliases() {
    let canonical = [
        ("uml.ClassDiagram", DiagramKind::Class),
        ("uml.UseCaseDiagram", DiagramKind::UseCase),
        ("uml.ActivityDiagram", DiagramKind::Activity),
        ("uml.StateMachineDiagram", DiagramKind::StateMachine),
        ("uml.SequenceDiagram", DiagramKind::Sequence),
    ];
    for (name, kind) in canonical {
        let ty = ElementType::parse(name);
        assert_eq!(ty, ElementType::Diagram(kind));
        assert!(ty.is_view());
        assert!(!ty.is_classifier());
    }

    let behavior = ElementType::Behavior(BehaviorKind::Activity);
    assert!(behavior.is_classifier());
    assert!(!behavior.is_view());

    for retired in [
        "Diagram",
        "uml.Activity",
        "uml.StateMachine",
        "uml.Sequence",
    ] {
        assert_eq!(DiagramKind::parse(retired), None, "{retired}");
        assert!(!ElementType::parse(retired).is_view(), "{retired}");
    }
}

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
fn contains<T: AstNode<uml::syntax::UmlLanguage>>(
    node: waml_syntax::SyntaxNode<uml::syntax::UmlLanguage>,
) -> bool {
    T::cast(node.clone()).is_some()
        || node
            .children()
            .filter_map(waml_syntax::SyntaxElement::into_node)
            .any(contains::<T>)
}

fn descendants(
    node: waml_syntax::SyntaxNode<uml::syntax::UmlLanguage>,
    kind: UmlSyntaxKind,
) -> Vec<waml_syntax::SyntaxNode<uml::syntax::UmlLanguage>> {
    let mut found = Vec::new();
    fn visit(
        node: waml_syntax::SyntaxNode<uml::syntax::UmlLanguage>,
        kind: UmlSyntaxKind,
        found: &mut Vec<waml_syntax::SyntaxNode<uml::syntax::UmlLanguage>>,
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
    node: waml_syntax::SyntaxNode<uml::syntax::UmlLanguage>,
) -> Vec<waml_syntax::SyntaxToken<uml::syntax::UmlLanguage>> {
    let mut found = Vec::new();
    fn visit(
        node: waml_syntax::SyntaxNode<uml::syntax::UmlLanguage>,
        found: &mut Vec<waml_syntax::SyntaxToken<uml::syntax::UmlLanguage>>,
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
                "---\ntype: uml.ClassDiagram\ntitle: D\n---\n# D\n\n## Members\n### G\n- [A](./a.md)\n\n## Layout\n{layout}"
            ),
        ),
        ("a.md", "---\ntype: uml.Class\n---\n# A\n".to_string()),
    ])
    .unwrap()
}

#[test]
fn diagram_members_and_layout_are_lossless_and_project_valid_placement() {
    let authored = "---\r\ntype: uml.ClassDiagram\r\ntitle: D\r\n---\r\n# D\r\n\r\n## Members\r\n### G\r\n- [A](./a.md)\r\n\r\n## Layout\r\n- A left of A\r\n";
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
        [waml::layout::LayoutStatement::Placement { .. }]
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
                node: waml_syntax::SyntaxNode<uml::syntax::UmlLanguage>,
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

fn linked_diagram_source() -> SourceBundle {
    SourceBundle::try_from_pairs([
        (
            "orders-diagram.md",
            "---\ntype: uml.ClassDiagram\ntitle: Orders\nprofile: uml-domain\n---\n# Orders\n\n## Members\n- [Order](./order.md)\n- [Customer](./customer.md)\n\n## Layout\n- [Order](./order.md) left of [Customer](./customer.md)\n",
        ),
        ("order.md", "---\ntype: uml.Class\n---\n# Order\n"),
        (
            "customer.md",
            "---\ntype: uml.Class\n---\n# Customer\n",
        ),
    ])
    .unwrap()
}

#[test]
fn diagram_projection_preserves_profile() {
    let source = linked_diagram_source();
    let analysis = analyze(&source);
    let diagram = analysis
        .projection
        .diagrams
        .iter()
        .find(|diagram| diagram.key == "orders-diagram")
        .unwrap();

    assert_eq!(diagram.profile, "uml-domain");
}

#[test]
fn diagram_projection_preserves_complete_two_link_placement() {
    let source = linked_diagram_source();
    let analysis = analyze(&source);
    let diagram = analysis
        .projection
        .diagrams
        .iter()
        .find(|diagram| diagram.key == "orders-diagram")
        .unwrap();

    let [waml::layout::LayoutStatement::Placement {
        operands,
        directions,
    }] = diagram.layout.as_slice()
    else {
        let declared = &analysis.declared.concept("orders-diagram").unwrap().layout[0];
        let state = match declared {
            uml::DeclaredField::Absent => "absent",
            uml::DeclaredField::Valid { .. } => "valid",
            uml::DeclaredField::Incomplete { .. } => "incomplete",
            uml::DeclaredField::Invalid { .. } => "invalid",
        };
        panic!("linked placement must remain a validated placement; declared={state}");
    };
    assert_eq!(directions, &[waml::layout::Direction::LeftOf]);
    assert_eq!(operands.len(), 2);
    assert!(matches!(
        &operands[0].ref_,
        waml::layout::OperandRef::Name(waml::layout::NameRef::Link { slug, .. })
            if slug == "./order.md"
    ));
    assert!(matches!(
        &operands[1].ref_,
        waml::layout::OperandRef::Name(waml::layout::NameRef::Link { slug, .. })
            if slug == "./customer.md"
    ));
}

#[test]
fn contradictory_linked_placements_reach_the_shared_solver_dropped_report() {
    let source = SourceBundle::try_from_pairs([
        (
            "diagram.md",
            "---\ntype: uml.ClassDiagram\nprofile: uml-domain\n---\n# Diagram\n\n## Members\n- [A](./a.md)\n- [B](./b.md)\n- [C](./c.md)\n\n## Layout\n- [A](./a.md) left of [B](./b.md)\n- [B](./b.md) left of [C](./c.md)\n- [C](./c.md) left of [A](./a.md)\n",
        ),
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
        ("c.md", "---\ntype: uml.Class\n---\n# C\n"),
    ])
    .unwrap();
    let analysis = analyze(&source);
    let diagram = analysis.projection.diagrams.first().unwrap();
    let sizes: waml::solve::SizeMap = ["a", "b", "c"]
        .into_iter()
        .map(|key| (key.to_string(), waml::solve::Size { w: 100.0, h: 60.0 }))
        .collect();

    // The unified path every frontend takes: resolve -> constrain::compile ->
    // stress::layout_constrained. The link-resolved placements must survive
    // compilation intact, and the solver must drop the cycle-closing sep and
    // trace it back — via the compiled provenance — to the authored relation
    // (that mapping is what the editor's conflict list is built from).
    let (scene, _resolve_diagnostics) = waml::solve::resolve::resolve(diagram);
    let compiled = waml::solve::constrain::compile(
        &scene,
        &sizes,
        &std::collections::BTreeMap::new(),
        &std::collections::BTreeSet::new(),
        &waml::solve::SolveConfig::default(),
    );
    assert!(
        compiled.dropped.is_empty(),
        "all three placements name known members; none may drop at compile time: {:?}",
        compiled.dropped
    );

    let ids: Vec<waml::solve::BoxId> = compiled
        .keys
        .iter()
        .cloned()
        .map(waml::solve::BoxId::Node)
        .collect();
    let dims: Vec<waml::solve::Size> = compiled.keys.iter().map(|key| sizes[key]).collect();
    let (_rects, _hulls, (dropped_x, dropped_y)) = waml::solve::stress::layout_constrained(
        &ids,
        &dims,
        &[],
        &[],
        &compiled.seps,
        &waml::solve::stress::StressConfig::default(),
    );

    let dropped: Vec<&waml::solve::Constraint> = dropped_x
        .iter()
        .filter_map(|&i| compiled.provenance_x.get(i).and_then(Option::as_ref))
        .chain(
            dropped_y
                .iter()
                .filter_map(|&i| compiled.provenance_y.get(i).and_then(Option::as_ref)),
        )
        .collect();
    assert!(
        !dropped.is_empty(),
        "the contradictory cycle must surface in the solver's dropped report"
    );
    assert!(
        dropped.iter().all(|relation| matches!(
            relation,
            waml::solve::Constraint::Place {
                dir: waml::layout::Direction::LeftOf,
                ..
            }
        )),
        "every dropped sep must trace to an authored left-of placement: {dropped:?}"
    );
}

#[test]
fn complete_layout_matrix_matches_legacy_model_and_has_fixed_nested_slots() {
    use waml::layout::{
        Anchored, Axis, Direction, Edge, Flag, Hint, LayoutStatement, Margin, NameRef, Operand,
        OperandRef, Shape,
    };

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
    let bare = |name: &str| Operand {
        ref_: OperandRef::Name(NameRef::Bare(name.into())),
        axis: None,
        hints: vec![],
    };
    let link = || Operand {
        ref_: OperandRef::Name(NameRef::Link {
            title: "A".into(),
            slug: "./a.md".into(),
        }),
        axis: None,
        hints: vec![],
    };
    let expected = vec![
        LayoutStatement::Alignment {
            left: Anchored {
                edge: Some(Edge::Top),
                operand: bare("A"),
            },
            right: Anchored {
                edge: Some(Edge::Center),
                operand: link(),
            },
        },
        LayoutStatement::Placement {
            operands: vec![
                Operand {
                    ref_: OperandRef::InlineGroup {
                        axis: Axis::Column,
                        items: vec![
                            bare("A"),
                            Operand {
                                ref_: OperandRef::Paren(Box::new(Operand {
                                    ref_: OperandRef::InlineGroup {
                                        axis: Axis::Row,
                                        items: vec![bare("two words"), link()],
                                    },
                                    axis: None,
                                    hints: vec![],
                                })),
                                axis: Some(Axis::Row),
                                hints: vec![
                                    Hint::Shape(Shape::Frame),
                                    Hint::Flag(Flag::Emphasized),
                                    Hint::Margin(Margin::Small),
                                ],
                            },
                        ],
                    },
                    axis: None,
                    hints: vec![],
                },
                bare("A"),
            ],
            directions: vec![Direction::RightOf],
        },
        LayoutStatement::Placement {
            operands: vec![bare("A"), bare("A"), bare("A")],
            directions: vec![Direction::AboveLeft, Direction::BelowRight],
        },
    ];
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
    let LayoutStatement::Placement {
        operands: expected_operands,
        ..
    } = &expected[1]
    else {
        unreachable!("second expected statement is a placement");
    };
    let OperandRef::InlineGroup {
        items: expected_group_items,
        ..
    } = &expected_operands[0].ref_
    else {
        unreachable!("first placement operand is an inline group");
    };
    assert_eq!(rich_operand.value(), Some(expected_group_items[1].clone()));
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
fn fixed_layout_slots_preserve_crlf_utf8_recovery_ranges_and_declared_state() {
    let authored = "---\r\ntype: uml.ClassDiagram\r\n---\r\n# D\r\n\r\n## Layout\r\n- Café left Café\r\n- Café above left of \"Étage\"\r\n";
    let source = SourceBundle::try_from_pairs([("d.md", authored)]).unwrap();
    let analysis = analyze(&source);
    let id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse("d.md").unwrap())
        .unwrap();
    let syntax = analysis.syntax.document(id).unwrap().syntax();
    assert_eq!(syntax.write_to_string(), authored);

    let statements = descendants(syntax.root(), UmlSyntaxKind::LayoutStatement);
    assert_eq!(statements.len(), 2);
    let first_recovery = descendants(statements[0].clone(), UmlSyntaxKind::SkippedTokensSyntax);
    assert_eq!(first_recovery.len(), 2);
    let recovered = first_recovery[0].range();
    assert_eq!(
        &authored[recovered.start().to_usize()..recovered.end().to_usize()],
        " left"
    );
    let trailing = first_recovery[1].range();
    assert_eq!(
        &authored[trailing.start().to_usize()..trailing.end().to_usize()],
        " Café"
    );
    assert!(descendant_tokens(statements[0].clone())
        .iter()
        .any(|token| token.flags().is_missing()
            && token.kind() == UmlSyntaxKind::LayoutKeywordToken
            && token.range().start() == token.range().end()));

    let declared = &analysis.declared.concept("d").unwrap().layout;
    assert!(matches!(
        &declared[0],
        uml::DeclaredField::Incomplete { syntax, .. } if syntax.range() == recovered
    ));
    let uml::DeclaredField::Valid {
        value:
            uml::DeclaredLayoutStatement::Placement {
                operands,
                directions,
            },
        ..
    } = &declared[1]
    else {
        panic!("valid UTF-8 row must recover independently after malformed row");
    };
    assert_eq!(operands.len(), 2);
    assert_eq!(directions.len(), 1);
    assert!(matches!(
        &directions[0],
        uml::DeclaredField::Valid {
            value: waml::layout::Direction::AboveLeft,
            ..
        }
    ));
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
            "---\ntype: uml.ClassDiagram\n---\n# D\n\n## Members\n### G\n- [Known](./known.md)\n- [Generic](./generic.md)\n",
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

/// The malformed run of a layout bullet, rendered as the authored line with a
/// caret row beneath it, so a span regression shows up as a visible shift.
fn layout_diagnostic_marks(line: &str) -> Vec<(String, String)> {
    let source = diagram_source(&format!("{line}\n"));
    analyze(&source)
        .diagnostics
        .iter()
        .filter(|d| d.code == waml::diagnostic::DiagCode::MalformedLayout)
        .map(|d| {
            let (start, end) = d.span.expect("layout diagnostic span");
            let (start, end) = (start.min(line.len()), end.min(line.len()));
            (
                format!(
                    "{}{}",
                    " ".repeat(start),
                    "~".repeat(end.saturating_sub(start))
                ),
                d.message.clone(),
            )
        })
        .collect()
}

#[test]
fn layout_recovery_names_the_missing_word_and_marks_only_the_malformed_run() {
    let line = "- [Order](./order.md) left f [Customer](./customer.md)";
    let marks = layout_diagnostic_marks(line);
    assert_eq!(
        marks.len(),
        1,
        "one diagnostic per malformed bullet: {marks:?}"
    );
    let (caret, message) = &marks[0];
    // Starts on `left`, not on the space in front of it, and stops after the
    // word that should have been `of` -- both members are left unmarked.
    assert_eq!(caret, "                      ~~~~~~");
    assert_eq!(message, "expected \"of\" after \"left\"");
}

#[test]
fn every_layout_shape_failure_names_the_word_the_grammar_wanted() {
    let cases = [
        (
            "- [Order](./order.md) left f [Customer](./customer.md)",
            "expected \"of\" after \"left\"",
        ),
        (
            "- [Order](./order.md) above left f [Customer](./customer.md)",
            "expected \"of\" after \"above left\"",
        ),
        ("- row [Order](./order.md)", "expected \"of\" after \"row\""),
        (
            "- [Order](./order.md) aligned [Customer](./customer.md)",
            "expected \"with\" after \"aligned\"",
        ),
        (
            "- [Order](./order.md) as sideways",
            "expected \"row\" or \"column\" after \"as\"",
        ),
        (
            "- [Order](./order.md) with banana",
            "\"banana\" is not a layout hint",
        ),
        (
            "- [Order](./order.md) with no",
            "expected \"margin\" after \"no\"",
        ),
        (
            "- ( [Order](./order.md) left of [Customer](./customer.md)",
            "expected \")\" to close the group",
        ),
        (
            "- [Order](./order.md) left of [Customer](./customer.md) junk here",
            "unexpected extra words after the layout statement",
        ),
        (
            "- top of [Order](./order.md)",
            "an edge anchor like \"top of\" needs \"aligned with\"",
        ),
    ];
    for (line, expected) in cases {
        let marks = layout_diagnostic_marks(line);
        assert_eq!(marks.len(), 1, "one diagnostic for {line:?}: {marks:?}");
        assert_eq!(marks[0].1, expected, "message for {line:?}");
    }
}

#[test]
fn layout_diagnostic_spans_never_start_on_whitespace() {
    for line in [
        "- [Order](./order.md) left f [Customer](./customer.md)",
        "- [Order](./order.md) as sideways",
        "- [Order](./order.md) with banana",
        "- row [Order](./order.md)",
        "- ",
        "- [A](./a.md",
    ] {
        for (caret, message) in layout_diagnostic_marks(line) {
            let Some(start) = caret.find('~') else {
                continue;
            };
            assert!(
                !line.as_bytes()[start].is_ascii_whitespace(),
                "{line:?} marks whitespace at {start} ({message})\n{line}\n{caret}"
            );
        }
    }
}
