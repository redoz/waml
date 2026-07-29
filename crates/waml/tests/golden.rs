use waml::source::split_bundle;

const FIXTURE: &str = include_str!("fixtures/orders-domain.md");

const PARSER_PLATFORM_FIXTURES: &[(&str, &str)] = &[
    (
        "generic.md",
        include_str!("fixtures/parser-platform/generic.md"),
    ),
    (
        "unknown-uml.md",
        include_str!("fixtures/parser-platform/unknown-uml.md"),
    ),
    (
        "index.md",
        include_str!("fixtures/parser-platform/index.md"),
    ),
    ("log.md", include_str!("fixtures/parser-platform/log.md")),
    (
        "class.md",
        include_str!("fixtures/parser-platform/class.md"),
    ),
    ("enum.md", include_str!("fixtures/parser-platform/enum.md")),
    (
        "object.md",
        include_str!("fixtures/parser-platform/object.md"),
    ),
    (
        "diagram.md",
        include_str!("fixtures/parser-platform/diagram.md"),
    ),
    (
        "activity.md",
        include_str!("fixtures/parser-platform/activity.md"),
    ),
    (
        "state-machine.md",
        include_str!("fixtures/parser-platform/state-machine.md"),
    ),
    (
        "sequence.md",
        include_str!("fixtures/parser-platform/sequence.md"),
    ),
    (
        "broken-frontmatter.md",
        include_str!("fixtures/parser-platform/broken-frontmatter.md"),
    ),
    (
        "malformed.md",
        include_str!("fixtures/parser-platform/malformed.md"),
    ),
    (
        "malformed-crlf-unicode.md",
        include_str!("fixtures/parser-platform/malformed-crlf-unicode.md"),
    ),
];

#[test]
fn parser_platform_baseline_keeps_every_fixture_serializable() {
    let source = waml::source::SourceBundle::try_from_pairs(
        PARSER_PLATFORM_FIXTURES
            .iter()
            .map(|(path, text)| (*path, *text)),
    )
    .unwrap();
    let prepared = waml::analysis::prepare_candidate(source, None, 0).unwrap();
    for (path, expected) in PARSER_PLATFORM_FIXTURES {
        let id = prepared
            .okf()
            .catalog
            .id_for_path(&waml::source::BundlePath::parse(*path).unwrap())
            .unwrap();
        assert_eq!(
            prepared
                .okf()
                .shell
                .document(id)
                .unwrap()
                .syntax()
                .write_to_string(),
            *expected,
            "{path}: lossless shell write"
        );
    }
}

#[test]
fn parser_platform_corpus_full_and_retained_results_match() {
    use waml::{
        analysis::{prepare_candidate, PreviousAnalyses},
        source::{BundlePath, SourceBundle},
        uml::{syntax::UmlLanguage, DeclaredBundle, DeclaredField, DeclaredLayoutStatement},
    };

    fn diagnostic_fingerprint(
        analysis: &waml::uml::Analysis,
    ) -> Vec<(
        String,
        String,
        String,
        String,
        usize,
        Option<(usize, usize)>,
        Option<(usize, usize)>,
    )> {
        let mut diagnostics = analysis
            .diagnostics
            .iter()
            .map(|diagnostic| {
                (
                    diagnostic.code.as_str().to_owned(),
                    format!("{:?}", diagnostic.severity),
                    diagnostic.message.clone(),
                    diagnostic.file.clone(),
                    diagnostic.line,
                    diagnostic.span,
                    diagnostic
                        .range
                        .map(|range| (range.start().to_usize(), range.end().to_usize())),
                )
            })
            .collect::<Vec<_>>();
        diagnostics.sort();
        diagnostics
    }

    fn declared_fingerprint(declared: &DeclaredBundle) -> Vec<String> {
        fn field<T>(label: &str, field: &DeclaredField<UmlLanguage, T>, out: &mut Vec<String>) {
            let range = |syntax: &waml_syntax::SyntaxNode<UmlLanguage>| {
                let range = syntax.range();
                format!("{}..{}", range.start().to_usize(), range.end().to_usize())
            };
            match field {
                DeclaredField::Absent => out.push(format!("{label}:absent")),
                DeclaredField::Valid { syntax, .. } => {
                    out.push(format!("{label}:valid@{}", range(syntax)))
                }
                DeclaredField::Incomplete { syntax, expected } => out.push(format!(
                    "{label}:incomplete({expected:?})@{}",
                    range(syntax)
                )),
                DeclaredField::Invalid {
                    syntax,
                    diagnostics,
                } => out.push(format!(
                    "{label}:invalid({:?})@{}",
                    diagnostics,
                    range(syntax)
                )),
            }
        }
        fn slots(slots: &[waml::uml::DeclaredSlot], label: &str, out: &mut Vec<String>) {
            for (index, slot) in slots.iter().enumerate() {
                field(&format!("{label}:{index}:name"), &slot.name, out);
                field(&format!("{label}:{index}:value"), &slot.value, out);
            }
        }
        fn inline_instances(
            instances: &[waml::uml::DeclaredInlineInstance],
            label: &str,
            out: &mut Vec<String>,
        ) {
            for (index, instance) in instances.iter().enumerate() {
                let label = format!("{label}:{index}");
                field(&format!("{label}:classifier"), &instance.classifier, out);
                field(&format!("{label}:name"), &instance.name, out);
                slots(&instance.slots, &format!("{label}:slot"), out);
            }
        }
        fn member_groups(
            groups: &[waml::uml::DeclaredMemberGroup],
            label: &str,
            out: &mut Vec<String>,
        ) {
            for (index, group) in groups.iter().enumerate() {
                let label = format!("{label}:{index}");
                field(&format!("{label}:name"), &group.name, out);
                for (member_index, member) in group.members.iter().enumerate() {
                    field(
                        &format!("{label}:member:{member_index}:target"),
                        &member.target,
                        out,
                    );
                }
                inline_instances(&group.inline_instances, &format!("{label}:inline"), out);
                member_groups(&group.children, &format!("{label}:child"), out);
            }
        }

        let mut out = Vec::new();
        for concept in declared.concepts() {
            out.push(format!("concept:{}", concept.concept_id));
            for (index, attribute) in concept.attributes.iter().enumerate() {
                field(
                    &format!("attribute:{index}:visibility"),
                    &attribute.visibility,
                    &mut out,
                );
                field(
                    &format!("attribute:{index}:name"),
                    &attribute.name,
                    &mut out,
                );
                field(&format!("attribute:{index}:type"), &attribute.ty, &mut out);
                field(
                    &format!("attribute:{index}:multiplicity"),
                    &attribute.multiplicity,
                    &mut out,
                );
            }
            for (index, value) in concept.values.iter().enumerate() {
                field(&format!("value:{index}:value"), &value.value, &mut out);
            }
            slots(&concept.slots, "slot", &mut out);
            for (index, relationship) in concept.relationships.iter().enumerate() {
                field(
                    &format!("relationship:{index}:kind"),
                    &relationship.kind,
                    &mut out,
                );
                field(
                    &format!("relationship:{index}:target"),
                    &relationship.target,
                    &mut out,
                );
                field(
                    &format!("relationship:{index}:name"),
                    &relationship.name,
                    &mut out,
                );
                field(
                    &format!("relationship:{index}:from_end"),
                    &relationship.from_end,
                    &mut out,
                );
                field(
                    &format!("relationship:{index}:to_end"),
                    &relationship.to_end,
                    &mut out,
                );
            }
            for (index, member) in concept.members.iter().enumerate() {
                field(&format!("member:{index}:target"), &member.target, &mut out);
            }
            member_groups(&concept.member_groups, "member_group", &mut out);
            inline_instances(&concept.inline_instances, "inline", &mut out);
            for (index, layout) in concept.layout.iter().enumerate() {
                field(&format!("layout:{index}"), layout, &mut out);
                if let DeclaredField::Valid { value, .. } = layout {
                    match value {
                        DeclaredLayoutStatement::Placement {
                            operands,
                            directions,
                        } => {
                            for (field_index, operand) in operands.iter().enumerate() {
                                field(
                                    &format!("layout:{index}:operand:{field_index}"),
                                    operand,
                                    &mut out,
                                );
                            }
                            for (field_index, direction) in directions.iter().enumerate() {
                                field(
                                    &format!("layout:{index}:direction:{field_index}"),
                                    direction,
                                    &mut out,
                                );
                            }
                        }
                        DeclaredLayoutStatement::Alignment { left, right } => {
                            field(&format!("layout:{index}:left"), left, &mut out);
                            field(&format!("layout:{index}:right"), right, &mut out);
                        }
                        DeclaredLayoutStatement::Standalone(operand) => {
                            field(&format!("layout:{index}:standalone"), operand, &mut out);
                        }
                    }
                }
            }
            for (index, node) in concept.flow_nodes.iter().enumerate() {
                field(&format!("flow_node:{index}:kind"), &node.kind, &mut out);
                field(
                    &format!("flow_node:{index}:identity"),
                    &node.identity,
                    &mut out,
                );
                field(
                    &format!("flow_node:{index}:object_ref"),
                    &node.object_ref,
                    &mut out,
                );
                field(&format!("flow_node:{index}:entry"), &node.entry, &mut out);
                field(&format!("flow_node:{index}:do"), &node.do_, &mut out);
                field(&format!("flow_node:{index}:exit"), &node.exit, &mut out);
                field(
                    &format!("flow_node:{index}:refines"),
                    &node.refines,
                    &mut out,
                );
                field(
                    &format!("flow_node:{index}:partition"),
                    &node.partition,
                    &mut out,
                );
                for (note_index, note) in node.notes.iter().enumerate() {
                    field(
                        &format!("flow_node:{index}:note:{note_index}"),
                        note,
                        &mut out,
                    );
                }
                for (transition_index, transition) in node.transitions.iter().enumerate() {
                    field(
                        &format!("flow_node:{index}:transition:{transition_index}:trigger"),
                        &transition.trigger,
                        &mut out,
                    );
                    field(
                        &format!("flow_node:{index}:transition:{transition_index}:guard"),
                        &transition.guard,
                        &mut out,
                    );
                    field(
                        &format!("flow_node:{index}:transition:{transition_index}:target"),
                        &transition.target,
                        &mut out,
                    );
                    field(
                        &format!("flow_node:{index}:transition:{transition_index}:carries"),
                        &transition.carries,
                        &mut out,
                    );
                    field(
                        &format!("flow_node:{index}:transition:{transition_index}:effect"),
                        &transition.effect,
                        &mut out,
                    );
                }
            }
            for (index, lifeline) in concept.lifelines.iter().enumerate() {
                field(
                    &format!("lifeline:{index}:target"),
                    &lifeline.target,
                    &mut out,
                );
                field(
                    &format!("lifeline:{index}:title"),
                    &lifeline.title,
                    &mut out,
                );
                field(
                    &format!("lifeline:{index}:alias"),
                    &lifeline.alias,
                    &mut out,
                );
            }
            for (index, message) in concept.messages.iter().enumerate() {
                field(&format!("message:{index}:from"), &message.from, &mut out);
                field(&format!("message:{index}:verb"), &message.verb, &mut out);
                field(&format!("message:{index}:to"), &message.to, &mut out);
                field(
                    &format!("message:{index}:signature"),
                    &message.signature,
                    &mut out,
                );
            }
            for (index, operand) in concept.sequence_operands.iter().enumerate() {
                field(
                    &format!("sequence_operand:{index}:fragment"),
                    &operand.fragment,
                    &mut out,
                );
                field(
                    &format!("sequence_operand:{index}:guard"),
                    &operand.guard,
                    &mut out,
                );
            }
        }
        out
    }

    let baseline_source = SourceBundle::try_from_pairs(
        PARSER_PLATFORM_FIXTURES
            .iter()
            .map(|(path, text)| (*path, *text)),
    )
    .unwrap();
    let baseline = prepare_candidate(baseline_source, None, 1).unwrap();
    let retained_source = SourceBundle::try_from_pairs(
        PARSER_PLATFORM_FIXTURES
            .iter()
            .map(|(path, text)| (*path, *text)),
    )
    .unwrap();
    let retained = prepare_candidate(
        retained_source,
        Some(PreviousAnalyses {
            okf: baseline.okf(),
            uml: baseline.uml(),
        }),
        2,
    )
    .unwrap();

    for (path, expected) in PARSER_PLATFORM_FIXTURES {
        let path = BundlePath::parse(*path).unwrap();
        assert_eq!(
            baseline.source().document(&path).unwrap().text(),
            *expected,
            "{path}: full source"
        );
        assert_eq!(
            retained.source().document(&path).unwrap().text(),
            *expected,
            "{path}: retained source"
        );
        for (label, prepared) in [("full", &baseline), ("retained", &retained)] {
            let id = prepared.okf().catalog.id_for_path(&path).unwrap();
            assert_eq!(
                prepared
                    .okf()
                    .shell
                    .document(id)
                    .unwrap()
                    .syntax()
                    .write_to_string(),
                *expected,
                "{path}: {label} shell writer"
            );
        }
    }
    assert_eq!(baseline.okf().bundle, retained.okf().bundle, "OKF bundle");
    let claims = |prepared: &waml::analysis::PreparedCandidate| {
        let mut claims = prepared
            .uml()
            .claims
            .iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        claims.sort_unstable();
        claims
    };
    assert_eq!(claims(&baseline), claims(&retained), "sorted UML claims");
    assert_eq!(
        baseline.uml().projection,
        retained.uml().projection,
        "UML projection"
    );
    assert_eq!(
        diagnostic_fingerprint(baseline.uml()),
        diagnostic_fingerprint(retained.uml()),
        "normalized diagnostics"
    );
    assert_eq!(
        declared_fingerprint(&baseline.uml().declared),
        declared_fingerprint(&retained.uml().declared),
        "declared occurrence states"
    );

    for (label, prepared) in [("full", &baseline), ("retained", &retained)] {
        for path in ["index.md", "log.md", "generic.md", "unknown-uml.md"] {
            let id = prepared
                .okf()
                .catalog
                .id_for_path(&BundlePath::parse(path).unwrap())
                .unwrap();
            assert!(
                prepared.uml().syntax.document(id).is_none(),
                "{label}: {path} absent from UML syntax"
            );
        }
    }
}

fn projection(bundle: &[(String, String)]) -> waml::uml::Projection {
    let source = waml::source::SourceBundle::try_from_pairs(bundle.iter().cloned()).unwrap();
    waml::analysis::prepare_candidate(source, None, 0)
        .unwrap()
        .uml()
        .projection
        .clone()
}

#[test]
fn parser_platform_baseline_keeps_okf_membership_and_selective_uml_claims() {
    use waml::source::SourceBundle;

    let source = SourceBundle::try_from_pairs(
        PARSER_PLATFORM_FIXTURES
            .iter()
            .map(|(path, text)| (*path, *text)),
    )
    .unwrap();
    let okf = waml::okf::Bundle::parse(&source).unwrap();
    let projection = waml::analysis::prepare_candidate(source, None, 0)
        .unwrap()
        .uml()
        .projection
        .clone();

    assert_eq!(
        okf.concepts()
            .iter()
            .map(|concept| concept.id.as_str())
            .collect::<Vec<_>>(),
        [
            "activity",
            "broken-frontmatter",
            "class",
            "diagram",
            "enum",
            "generic",
            "malformed",
            "malformed-crlf-unicode",
            "object",
            "sequence",
            "state-machine",
            "unknown-uml",
        ],
        "parser-platform OKF concept membership"
    );
    assert!(okf.index("/").unwrap().authored);
    assert!(okf.log("/").is_some());
    assert!(okf.concept("generic").is_some());
    assert!(okf.concept("unknown-uml").is_some());
    assert!(projection.contains_concept("class"));
    assert!(projection.contains_concept("enum"));
    assert!(projection.contains_concept("object"));
    assert!(!projection.contains_concept("generic"));
    assert!(!projection.contains_concept("unknown-uml"));
    assert!(projection.contains_concept("malformed-crlf-unicode"));
    assert_eq!(
        projection
            .nodes
            .iter()
            .map(|node| node.key.as_str())
            .collect::<Vec<_>>(),
        [
            "class",
            "enum",
            "malformed",
            "malformed-crlf-unicode",
            "object",
        ],
        "classifier fixture selective UML claims"
    );

    let class = projection.node("class").unwrap();
    assert_eq!(
        class
            .attributes
            .iter()
            .map(|attribute| (attribute.name.as_str(), attribute.ty.name.as_str()))
            .collect::<Vec<_>>(),
        [("id", "OrderId"), ("label", "String")],
        "class.md projected attributes"
    );
    assert_eq!(
        projection.node("enum").unwrap().values,
        ["Draft", "Submitted"],
        "enum.md projected values"
    );
    assert_eq!(
        projection
            .edges
            .iter()
            .map(|edge| (
                edge.source.as_str(),
                edge.kind.as_str(),
                edge.target.as_str()
            ))
            .collect::<Vec<_>>(),
        [
            ("class", "depends", "enum"),
            ("object", "instance of", "class"),
        ],
        "class.md/object.md projected relationships"
    );
    assert_eq!(projection.diagrams.len(), 1, "diagram.md projection count");
    assert_eq!(
        projection.diagrams[0].groups[0].members,
        ["class", "enum"],
        "diagram.md projected members"
    );
    assert_eq!(
        projection.diagrams[0].layout.len(),
        1,
        "diagram.md projected layout"
    );
    let waml::layout::LayoutStatement::Placement {
        operands,
        directions,
    } = &projection.diagrams[0].layout[0]
    else {
        panic!("diagram.md layout statement kind");
    };
    assert_eq!(
        directions,
        &[waml::layout::Direction::RightOf],
        "diagram.md layout direction"
    );
    assert_eq!(operands.len(), 2, "diagram.md layout operand count");
    for (operand, expected) in operands.iter().zip(["class", "enum"]) {
        assert!(
            matches!(
                &operand.ref_,
                waml::layout::OperandRef::Name(waml::layout::NameRef::Bare(name))
                    if name == expected
            ),
            "diagram.md layout operand {expected}"
        );
    }
    assert_eq!(
        projection
            .flows
            .iter()
            .map(|flow| (flow.key.as_str(), flow.nodes.len(), flow.edges.len()))
            .collect::<Vec<_>>(),
        [("activity", 3, 2), ("state-machine", 2, 1)],
        "activity.md/state-machine.md projected flows"
    );
    assert_eq!(
        projection
            .activity_nodes
            .iter()
            .map(|node| (node.key.as_str(), node.id.as_str(), node.kind))
            .collect::<Vec<_>>(),
        [
            ("activity#Start", "Start", waml::model::FlowNodeKind::Plain),
            ("activity#Pack", "Pack", waml::model::FlowNodeKind::Plain),
            ("activity#Done", "Done", waml::model::FlowNodeKind::Plain),
            (
                "state-machine#Draft",
                "Draft",
                waml::model::FlowNodeKind::Plain,
            ),
            (
                "state-machine#Submitted",
                "Submitted",
                waml::model::FlowNodeKind::Plain,
            ),
        ],
        "activity.md/state-machine.md projected node identities and kinds"
    );
    assert_eq!(
        projection
            .flow_edges
            .iter()
            .map(|edge| {
                (
                    edge.behavior.as_str(),
                    edge.from.as_str(),
                    edge.to.as_str(),
                    edge.trigger.as_deref(),
                )
            })
            .collect::<Vec<_>>(),
        [
            ("activity", "activity#Start", "activity#Pack", None),
            ("activity", "activity#Pack", "activity#Done", None),
            (
                "state-machine",
                "state-machine#Draft",
                "state-machine#Submitted",
                Some("submit"),
            ),
        ],
        "activity.md/state-machine.md projected transition values"
    );
    assert_eq!(
        projection
            .interactions
            .iter()
            .map(|interaction| {
                (
                    interaction.key.as_str(),
                    interaction.nodes.len(),
                    interaction.edges.len(),
                )
            })
            .collect::<Vec<_>>(),
        [("sequence", 2, 1)],
        "sequence.md projected interaction"
    );
    let interaction = &projection.interactions[0];
    assert!(
        matches!(
            &interaction.nodes[0],
            waml::model::SeqNode::Lifeline { id, title, ref_, alias }
                if id == "Buyer"
                    && title == "Buyer"
                    && ref_.as_deref() == Some("object")
                    && alias.is_none()
        ),
        "sequence.md Buyer lifeline"
    );
    assert!(
        matches!(
            &interaction.nodes[1],
            waml::model::SeqNode::Lifeline { id, title, ref_, alias }
                if id == "Order"
                    && title == "Order"
                    && ref_.as_deref() == Some("class")
                    && alias.is_none()
        ),
        "sequence.md Order lifeline"
    );
    assert_eq!(
        (
            interaction.edges[0].id.as_str(),
            interaction.edges[0].from.as_str(),
            interaction.edges[0].verb,
            interaction.edges[0].to.as_str(),
            interaction.edges[0].signature.as_deref(),
        ),
        (
            "m0",
            "Buyer",
            waml::model::MessageVerb::Calls,
            "Order",
            Some("submit()"),
        ),
        "sequence.md message values"
    );
}

#[test]
fn orders_domain_builds_the_expected_model() {
    let bundle = split_bundle(FIXTURE);
    let m = projection(&bundle);

    // Five classifiers, one diagram.
    assert_eq!(m.nodes.len(), 5);
    assert_eq!(m.diagrams.len(), 1);
    assert_eq!(
        m.diagrams[0]
            .groups
            .iter()
            .map(|g| g.members.len())
            .sum::<usize>(),
        5
    );

    // Two edges: composes + associates.
    assert_eq!(m.edges.len(), 2);
    let kinds: Vec<_> = m.edges.iter().map(|e| e.kind.as_str()).collect();
    assert!(kinds.contains(&"composes"));
    assert!(kinds.contains(&"associates"));

    // The composition target resolves and carries the far role.
    let comp = m
        .edges
        .iter()
        .find(|e| e.kind.as_str() == "composes")
        .unwrap();
    assert_eq!(comp.source, "shop/order");
    assert_eq!(comp.target, "shop/order-line");
    assert_eq!(comp.to_end.role.as_deref(), Some("lines"));

    // The associates edge (declared on order.md as "1 order to 1 customer")
    // resolves order -> customer, near role "order" and far role "customer".
    let assoc = m
        .edges
        .iter()
        .find(|e| e.kind.as_str() == "associates")
        .unwrap();
    assert_eq!(assoc.source, "shop/order");
    assert_eq!(assoc.target, "shop/customer");
    assert_eq!(assoc.from_end.role.as_deref(), Some("order"));
    assert_eq!(assoc.to_end.role.as_deref(), Some("customer"));

    // The Money value-object's own attribute types are bare tokens (no matching docs).
    let money = m.node("shop/money").unwrap();
    assert_eq!(money.attributes[0].ty.name, "Decimal");
    assert_eq!(money.attributes[0].ty.ref_, None);

    // Order has 3 attributes (id, status, total); total resolves to Money.
    let order = m.node("shop/order").unwrap();
    // Title now lives ONLY on the concept (single authoritative source).
    assert_eq!(order.concept.title.as_deref(), Some("Order"));
    assert_eq!(order.attributes.len(), 3);
    let total = order.attributes.iter().find(|a| a.name == "total").unwrap();
    assert_eq!(total.ty.name, "Money");
    assert_eq!(total.ty.ref_.as_deref(), Some("shop/money"));
}

#[test]
fn every_doc_is_lossless_through_the_authoritative_shell() {
    let bundle = split_bundle(FIXTURE);
    let source = waml::source::SourceBundle::try_from_pairs(bundle.clone()).unwrap();
    let prepared = waml::analysis::prepare_candidate(source, None, 0).unwrap();
    for (path, text) in bundle {
        let id = prepared
            .okf()
            .catalog
            .id_for_path(&waml::source::BundlePath::parse(path).unwrap())
            .unwrap();
        assert_eq!(
            prepared
                .okf()
                .shell
                .document(id)
                .unwrap()
                .syntax()
                .write_to_string(),
            text
        );
    }
}

#[test]
fn nested_packages_round_trip_through_reindex() {
    use waml::index_md::reindex_source;
    use waml::source::SourceBundle;
    let b = vec![
        (
            "sales/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
        ),
        (
            "sales/customer.md".to_string(),
            "---\ntype: uml.Class\ntitle: Customer\ndescription: A buyer.\n---\n# Customer\n"
                .to_string(),
        ),
        (
            "sales/orders/line.md".to_string(),
            "---\ntype: uml.Class\ntitle: Line\n---\n# Line\n".to_string(),
        ),
    ];
    let m1 = projection(&b);
    let bundle2 = reindex_source(&SourceBundle::try_from_pairs(b.clone()).unwrap()).to_pairs();
    let m2 = projection(&bundle2);
    // packages + members stable across the round-trip
    let names = |m: &waml::model::Model| {
        let mut v: Vec<_> = m
            .packages
            .iter()
            .map(|p| (p.key.clone(), p.members.clone()))
            .collect();
        v.sort();
        v
    };
    assert_eq!(names(&m1), names(&m2));
    // blurb from description survived into sales/index.md
    let idx = bundle2.iter().find(|(p, _)| p == "sales/index.md").unwrap();
    assert!(idx.1.contains("[Customer](./customer.md) - A buyer."));
    // second reindex is a fixpoint
    let bundle3 =
        reindex_source(&SourceBundle::try_from_pairs(bundle2.clone()).unwrap()).to_pairs();
    assert_eq!(
        bundle2
            .iter()
            .find(|(p, _)| p == "sales/index.md")
            .unwrap()
            .1,
        bundle3
            .iter()
            .find(|(p, _)| p == "sales/index.md")
            .unwrap()
            .1
    );
}

#[test]
fn orders_domain_has_no_diagnostics() {
    let bundle = waml::source::split_bundle(FIXTURE);
    let diags = waml::validate::validate(&bundle);
    assert!(diags.is_empty(), "expected clean fixture, got: {diags:?}");
}
