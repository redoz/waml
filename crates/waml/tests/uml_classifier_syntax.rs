use waml::{analysis::analyze_okf, source::SourceBundle, uml};
use waml_syntax::AstNode;

fn contains<T: AstNode<waml::uml::syntax::UmlLanguage>>(
    node: waml_syntax::SyntaxNode<waml::uml::syntax::UmlLanguage>,
) -> bool {
    T::cast(node.clone()).is_some()
        || node
            .children()
            .filter_map(waml_syntax::SyntaxElement::into_node)
            .any(contains::<T>)
}

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
fn classifier_sections_are_lossless_and_expose_fixed_typed_slots() {
    let authored = "---\r\ntype: uml.Class\r\n---\r\n# Café\r\n\r\n## Values\r\n- OPEN\r\n\r\n## Slots\r\n- status: \"OPEN\"\r\n\r\n## Relationships\r\n- depends [Customer](./customer.md)\r\n\r\n## Members\r\n### People\r\n- [Customer](./customer.md)\r\n- instance of [Customer](./customer.md) as primary with status set to OPEN\r\n\r\n## Operations\r\n- must remain Markdown\r\n";
    let source = SourceBundle::try_from_pairs([
        ("cafe.md", authored),
        ("customer.md", "---\ntype: uml.Class\n---\n# Customer\n"),
    ])
    .unwrap();
    let analysis = analyze(&source);
    let id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse("cafe.md").unwrap())
        .unwrap();
    let root = analysis.syntax.document(id).unwrap().syntax().root();
    assert!(contains::<uml::ValueSyntax>(root.clone()));
    assert!(contains::<uml::SlotSyntax>(root.clone()));
    assert!(contains::<uml::RelationshipSyntax>(root.clone()));
    assert!(contains::<uml::MemberSyntax>(root.clone()));
    assert!(contains::<uml::InlineInstanceSyntax>(root));
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
fn classifier_items_do_not_hide_authored_grammar_in_raw_markdown_tokens() {
    let source = SourceBundle::try_from_pairs([
        ("class.md", "---\ntype: uml.Class\n---\n# Class\n\n## Values\n- READY\n\n## Slots\n- state: \"ready\"\n\n## Relationships\n- depends [Other](./other.md)\n\n## Members\n- [Other](./other.md)\n"),
        ("other.md", "---\ntype: uml.Class\n---\n# Other\n"),
    ]).unwrap();
    let analysis = analyze(&source);
    let id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse("class.md").unwrap())
        .unwrap();
    let root = analysis.syntax.document(id).unwrap().syntax().root();
    fn typed_nodes_have_no_raw(node: waml_syntax::SyntaxNode<waml::uml::syntax::UmlLanguage>) {
        let typed = matches!(
            node.kind(),
            waml::uml::syntax::UmlSyntaxKind::Value
                | waml::uml::syntax::UmlSyntaxKind::Slot
                | waml::uml::syntax::UmlSyntaxKind::Relationship
                | waml::uml::syntax::UmlSyntaxKind::Member
                | waml::uml::syntax::UmlSyntaxKind::InlineInstance
        );
        if typed {
            assert!(!node
                .children()
                .any(|e| e.kind() == waml::uml::syntax::UmlSyntaxKind::RawMarkdownToken));
        }
        for child in node
            .children()
            .filter_map(waml_syntax::SyntaxElement::into_node)
        {
            typed_nodes_have_no_raw(child);
        }
    }
    typed_nodes_have_no_raw(root);
}

#[test]
fn classifier_accessors_read_only_direct_fixed_slots() {
    let source = SourceBundle::try_from_pairs([
        ("class.md", "---\ntype: uml.Class\n---\n# Class\n\n## Values\n- READY\n\n## Slots\n- state: \"ready\"\n\n## Relationships\n- depends [Other](./other.md)\n\n## Members\n- [Other](./other.md)\n"),
        ("other.md", "---\ntype: uml.Class\n---\n# Other\n"),
    ]).unwrap();
    let analysis = analyze(&source);
    let id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse("class.md").unwrap())
        .unwrap();
    let root = analysis.syntax.document(id).unwrap().syntax().root();
    fn visit(node: waml_syntax::SyntaxNode<waml::uml::syntax::UmlLanguage>) {
        use waml::uml::{MemberSyntax, RelationshipSyntax, SlotSyntax, ValueSyntax};
        if let Some(value) = ValueSyntax::cast(node.clone()) {
            assert_eq!(
                value.value_token().unwrap().text().write_to_string(),
                "READY"
            );
        }
        if let Some(slot) = SlotSyntax::cast(node.clone()) {
            assert_eq!(slot.name_token().unwrap().text().write_to_string(), "state");
            assert!(!slot.colon_token().unwrap().flags().is_missing());
        }
        if let Some(rel) = RelationshipSyntax::cast(node.clone()) {
            assert_eq!(
                rel.kind_token().unwrap().text().write_to_string(),
                "depends"
            );
            assert_eq!(rel.link().unwrap().children().count(), 6);
        }
        if let Some(member) = MemberSyntax::cast(node.clone()) {
            assert_eq!(
                member.target_token().unwrap().text().write_to_string(),
                "./other.md"
            );
        }
        for child in node
            .children()
            .filter_map(waml_syntax::SyntaxElement::into_node)
        {
            visit(child);
        }
    }
    visit(root);
}

#[test]
fn declared_classifier_fields_are_lowered_from_fixed_syntax_slots() {
    let source = SourceBundle::try_from_pairs([
        ("class.md", "---\ntype: uml.Class\n---\n# Class\n\n## Values\n- READY\n\n## Slots\n- state: \"ready\"\n\n## Relationships\n- depends [Other](./other.md)\n\n## Members\n- [Other](./other.md)\n"),
        ("other.md", "---\ntype: uml.Class\n---\n# Other\n"),
    ]).unwrap();
    let analysis = analyze(&source);
    let concept = analysis.declared.concept("class").unwrap();
    assert!(
        matches!(concept.values[0].value, uml::DeclaredField::Valid { ref value, .. } if value == "READY")
    );
    assert!(
        matches!(concept.slots[0].name, uml::DeclaredField::Valid { ref value, .. } if value == "state")
    );
    assert!(
        matches!(concept.slots[0].value, uml::DeclaredField::Valid { ref value, .. } if value == "\"ready\"")
    );
    assert!(matches!(
        concept.relationships[0].kind,
        uml::DeclaredField::Valid {
            value: waml::model::RelationshipKind::Depends,
            ..
        }
    ));
    assert!(
        matches!(concept.relationships[0].target, uml::DeclaredField::Valid { ref value, .. } if value == "./other.md")
    );
    assert!(
        matches!(concept.members[0].target, uml::DeclaredField::Valid { ref value, .. } if value == "./other.md")
    );
}

#[test]
fn slot_value_variants_and_missing_colon_are_distinguished() {
    let source = SourceBundle::try_from_pairs([
        ("class.md", "---\ntype: uml.Class\n---\n# Class\n\n## Slots\n- bare: OPEN\n- quoted: \"OPEN\"\n- linked: [State](./state.md)\n- missing OPEN\n"),
        ("state.md", "---\ntype: uml.Class\n---\n# State\n"),
    ]).unwrap();
    let analysis = analyze(&source);
    let id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse("class.md").unwrap())
        .unwrap();
    let root = analysis.syntax.document(id).unwrap().syntax().root();
    let mut variants = Vec::new();
    fn visit(
        node: waml_syntax::SyntaxNode<waml::uml::syntax::UmlLanguage>,
        out: &mut Vec<waml::uml::SlotSyntax>,
    ) {
        if let Some(slot) = waml::uml::SlotSyntax::cast(node.clone()) {
            out.push(slot);
        }
        for child in node
            .children()
            .filter_map(waml_syntax::SyntaxElement::into_node)
        {
            visit(child, out);
        }
    }
    visit(root, &mut variants);
    assert_eq!(variants.len(), 4);
    assert_eq!(variants[0].value_kind(), waml::uml::SlotValueKind::Bare);
    assert_eq!(variants[1].value_kind(), waml::uml::SlotValueKind::Quoted);
    assert_eq!(variants[2].value_kind(), waml::uml::SlotValueKind::Link);
    assert!(variants[3].colon_token().unwrap().flags().is_missing());
}

#[test]
fn relationship_has_fixed_name_and_end_slots_with_bounded_recovery() {
    let authored = "---\r\ntype: uml.Class\r\n---\r\n# Café\r\n\r\n## Relationships\r\n- associates [Customer](./customer.md) as \"owns\": 1 order to 0..* café\r\n- composes [Line](./line.md): 1 to 1..* lines\r\n- associates [Broken](./broken.md): nope to 1 trailing\r\n";
    let source = SourceBundle::try_from_pairs([
        ("class.md", authored),
        ("customer.md", "---\ntype: uml.Class\n---\n# Customer\n"),
        ("line.md", "---\ntype: uml.Class\n---\n# Line\n"),
    ])
    .unwrap();
    let analysis = analyze(&source);
    let concept = analysis.declared.concept("class").unwrap();
    let first = &concept.relationships[0];
    assert_eq!(
        first
            .syntax
            .name_label_token()
            .unwrap()
            .text()
            .write_to_string(),
        "\"owns\""
    );
    assert_eq!(
        first
            .syntax
            .from_end()
            .unwrap()
            .multiplicity_token()
            .text()
            .write_to_string(),
        "1"
    );
    assert_eq!(
        first
            .syntax
            .to_end()
            .unwrap()
            .role_token()
            .unwrap()
            .text()
            .write_to_string(),
        "café"
    );
    assert!(matches!(first.name, uml::DeclaredField::Valid { .. }));
    assert!(matches!(first.from_end, uml::DeclaredField::Valid { .. }));
    assert!(matches!(
        concept.relationships[2].from_end,
        uml::DeclaredField::Invalid { .. }
    ));
    assert_eq!(
        analysis
            .syntax
            .document(
                analysis
                    .syntax
                    .catalog()
                    .id_for_path(&waml::source::BundlePath::parse("class.md").unwrap())
                    .unwrap()
            )
            .unwrap()
            .syntax()
            .write_to_string(),
        authored
    );
}

#[test]
fn relationship_recovery_keeps_required_slots_and_progresses_to_next_item() {
    let authored = "---\ntype: uml.Class\n---\n# C\n\n## Relationships\n- composes [Line](./line.md)\n- includes [Use](./use.md): 1 to 1\n- associates [Bad](./bad.md): 1 to\n- depends [Good](./good.md)\n";
    let source = SourceBundle::try_from_pairs([
        ("c.md", authored),
        ("line.md", "---\ntype: uml.Class\n---\n# Line\n"),
        ("use.md", "---\ntype: uml.UseCase\n---\n# Use\n"),
        ("good.md", "---\ntype: uml.Class\n---\n# Good\n"),
    ])
    .unwrap();
    let analysis = analyze(&source);
    let relationships = &analysis.declared.concept("c").unwrap().relationships;
    assert_eq!(
        relationships.len(),
        4,
        "recovery must not consume the next list item"
    );
    assert!(matches!(
        relationships[0].from_end,
        uml::DeclaredField::Incomplete { .. }
    ));
    assert!(matches!(
        relationships[1].from_end,
        uml::DeclaredField::Invalid { .. }
    ));
    assert!(matches!(
        relationships[2].to_end,
        uml::DeclaredField::Invalid { .. } | uml::DeclaredField::Incomplete { .. }
    ));
    assert!(matches!(
        relationships[3].target,
        uml::DeclaredField::Valid { .. }
    ));
    assert!(!analysis.diagnostics.is_empty());
}

#[test]
fn member_groups_and_inline_instances_have_fixed_indented_slots() {
    let authored = "---\r\ntype: uml.Class\r\n---\r\n# Café\r\n\r\n## Members\r\n### People\r\n- [Customer](./customer.md)\r\n    - instance of [Customer](./customer.md) as primary with state set to \"open\" and owner set to [Owner](./owner.md)\r\n#### VIP\r\n- [Owner](./owner.md)\r\n- [Broken](./broken.md) stray\r\n- [Last](./last.md)\r\n";
    let source = SourceBundle::try_from_pairs([
        ("cafe.md", authored),
        ("customer.md", "---\ntype: uml.Class\n---\n# Customer\n"),
        ("owner.md", "---\ntype: uml.Class\n---\n# Owner\n"),
        ("last.md", "---\ntype: uml.Class\n---\n# Last\n"),
    ])
    .unwrap();
    let analysis = analyze(&source);
    let concept = analysis.declared.concept("cafe").unwrap();
    assert_eq!(concept.member_groups.len(), 1);
    assert_eq!(concept.member_groups[0].children.len(), 1);
    assert_eq!(concept.member_groups[0].members.len(), 1);
    assert_eq!(concept.member_groups[0].inline_instances.len(), 1);
    assert_eq!(concept.member_groups[0].children[0].members.len(), 3);
    assert_eq!(
        concept.members.len(),
        4,
        "bad member recovery cannot swallow Last"
    );
    assert_eq!(concept.inline_instances.len(), 1);
    let inline = &concept.inline_instances[0];
    assert_eq!(
        inline.syntax.name_token().unwrap().text().write_to_string(),
        "primary"
    );
    assert_eq!(inline.slots.len(), 2);
    assert!(
        matches!(inline.slots[0].value, uml::DeclaredField::Valid { ref value, .. } if value == "\"open\"")
    );
    let id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse("cafe.md").unwrap())
        .unwrap();
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
fn only_h2_classifier_sections_parse_and_nested_values_remain_owned_or_opaque() {
    let authored = "---\ntype: uml.Class\n---\n# Values\n- title body\n\n## Members\n### Values\n- [Good](./good.md)\n\n## Operations\n### Values\n- operation body\n";
    let source = SourceBundle::try_from_pairs([
        ("c.md", authored),
        ("good.md", "---\ntype: uml.Class\n---\n# Good\n"),
    ])
    .unwrap();
    let analysis = analyze(&source);
    let concept = analysis.declared.concept("c").unwrap();
    assert!(concept.values.is_empty());
    assert_eq!(concept.member_groups.len(), 1);
    assert!(
        matches!(concept.member_groups[0].name, uml::DeclaredField::Valid { ref value, .. } if value == "Values")
    );
    assert_eq!(concept.member_groups[0].members.len(), 1);
    let id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse("c.md").unwrap())
        .unwrap();
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
fn invalid_group_inline_instance_never_creates_a_dangling_member() {
    let source = SourceBundle::try_from_pairs([
        (
            "diagram.md",
            "---\ntype: Diagram\n---\n# Diagram\n\n## Members\n### Invalid\n- instance of [Good](./good.md) as bad with state OPEN\n",
        ),
        ("good.md", "---\ntype: uml.Class\n---\n# Good\n"),
    ])
    .unwrap();
    let analysis = analyze(&source);
    let concept = analysis.declared.concept("diagram").unwrap();
    assert!(matches!(
        concept.member_groups[0].inline_instances[0].slots[0].value,
        uml::DeclaredField::Incomplete { .. }
    ));
    assert!(analysis.projection.node("diagram#bad").is_none());
    assert!(analysis.projection.diagrams[0].groups[0].members.is_empty());
}

#[test]
fn multi_word_values_are_one_field_and_malformed_items_do_not_project() {
    let source = SourceBundle::try_from_pairs([("c.md", "---\ntype: uml.Class\n---\n# C\n\n## Values\n- Ready for use\n\n## Slots\n- missing value\n\n## Members\n- [Good](./good.md) stray\n"), ("good.md", "---\ntype: uml.Class\n---\n# Good\n")]).unwrap();
    let analysis = analyze(&source);
    let declared = analysis.declared.concept("c").unwrap();
    assert!(
        matches!(declared.values[0].value, uml::DeclaredField::Valid { ref value, .. } if value=="Ready for use")
    );
    assert!(matches!(
        declared.slots[0].value,
        uml::DeclaredField::Incomplete { .. }
    ));
    assert!(matches!(
        declared.members[0].target,
        uml::DeclaredField::Invalid { .. }
    ));
    assert_eq!(
        analysis.projection.node("c").unwrap().values,
        ["Ready for use"]
    );
    assert!(analysis.projection.node("c").unwrap().slots.is_empty());
}

#[test]
fn delimiter_recovery_distinguishes_incomplete_from_invalid_and_never_projects() {
    let source=SourceBundle::try_from_pairs([("c.md", "---\ntype: uml.Class\n---\n# C\n\n## Slots\n- missing value\n- quote: \"unterminated\n- link: [Broken](./broken.md\n- trailing: OPEN extra\n- lone-quote: \"\n- empty-link: [Broken]()\n\n## Relationships\n- composes [Good](./good.md)\n- associates [Good](./good.md): 1 1\n- depends [Good](./good.md) trailing\n- depends [Broken](./broken.md\n- associates [Good](./good.md) as \"broken: 1 to 1\n- associates [Good](./good.md) as [Broken](): 1 to 1\n- depends\n\n## Members\n- [Good](./good.md) trailing\n- [Empty]()\n- instance of [Good](./good.md) primary\n- instance of [Good](./good.md) as x with state OPEN\n- instance of [Good](./good.md) as q with state set to \"unterminated\n- instance of [Good](./good.md) as r with state set to \"\n"), ("good.md", "---\ntype: uml.Class\n---\n# Good\n")]).unwrap();
    let analysis = analyze(&source);
    let d = analysis.declared.concept("c").unwrap();
    assert!(matches!(
        d.slots[0].value,
        uml::DeclaredField::Incomplete { .. }
    ));
    assert!(matches!(
        d.slots[1].value,
        uml::DeclaredField::Invalid { .. }
    ));
    assert!(matches!(
        d.slots[2].value,
        uml::DeclaredField::Invalid { .. }
    ));
    assert!(matches!(
        d.slots[3].value,
        uml::DeclaredField::Invalid { .. }
    ));
    assert!(matches!(
        d.slots[4].value,
        uml::DeclaredField::Invalid { .. }
    ));
    assert!(matches!(
        d.slots[5].value,
        uml::DeclaredField::Invalid { .. }
    ));
    assert!(matches!(
        d.relationships[0].from_end,
        uml::DeclaredField::Incomplete { .. }
    ));
    assert!(matches!(
        d.relationships[1].to_end,
        uml::DeclaredField::Incomplete { .. } | uml::DeclaredField::Invalid { .. }
    ));
    assert!(matches!(
        d.relationships[2].kind,
        uml::DeclaredField::Invalid { .. }
    ));
    assert!(matches!(
        d.relationships[3].target,
        uml::DeclaredField::Invalid { .. }
    ));
    assert!(matches!(
        d.relationships[4].name,
        uml::DeclaredField::Invalid { .. }
    ));
    assert!(matches!(
        d.relationships[5].name,
        uml::DeclaredField::Invalid { .. }
    ));
    assert!(matches!(
        d.relationships[6].target,
        uml::DeclaredField::Incomplete { .. }
    ));
    assert!(matches!(
        d.members[0].target,
        uml::DeclaredField::Invalid { .. }
    ));
    assert!(matches!(
        d.members[1].target,
        uml::DeclaredField::Invalid { .. }
    ));
    assert!(matches!(
        d.inline_instances[0].name,
        uml::DeclaredField::Incomplete { .. }
    ));
    assert!(matches!(
        d.inline_instances[1].slots[0].value,
        uml::DeclaredField::Incomplete { .. }
    ));
    assert!(matches!(
        d.inline_instances[2].slots[0].value,
        uml::DeclaredField::Invalid { .. }
    ));
    assert!(matches!(
        d.inline_instances[3].slots[0].value,
        uml::DeclaredField::Invalid { .. }
    ));
    let node = analysis.projection.node("c").unwrap();
    assert!(node.slots.is_empty());
    assert!(analysis.projection.edges.is_empty());
    assert!(analysis.projection.node("c#x").is_none());
    assert!(analysis.projection.node("c#q").is_none());
    assert!(analysis.projection.node("c#r").is_none());
}

#[test]
fn declared_projection_resolves_only_claimed_targets_with_located_diagnostic() {
    let source = SourceBundle::try_from_pairs([
        ("order.md", "---\ntype: uml.Class\nabstract: true\nstereotype: [entity, aggregate]\n---\n# Order\n\n## Values\n- OPEN\n\n## Slots\n- state: \"open\"\n\n## Relationships\n- associates [Customer](./customer.md): 1 to 1\n- composes [Customer](./customer.md)\n- depends [Generic](./generic.md)\n"),
        ("customer.md", "---\ntype: uml.Class\n---\n# Customer\n"),
        ("generic.md", "---\ntype: vendor.Custom\n---\n# Generic\n"),
    ]).unwrap();
    let analysis = analyze(&source);
    let order = analysis.projection.node("order").unwrap();
    assert_eq!(order.values, ["OPEN"]);
    assert_eq!(order.slots[0].value, "\"open\"");
    assert!(order.abstract_);
    assert_eq!(order.stereotypes, ["entity", "aggregate"]);
    assert_eq!(analysis.projection.edges.len(), 1);
    let unresolved = analysis
        .diagnostics
        .iter()
        .find(|d| d.code == waml::diagnostic::DiagCode::UnresolvedTarget)
        .unwrap();
    assert_eq!(unresolved.file, "order.md");
    let range = unresolved.range.unwrap();
    let text = source
        .document(&waml::source::BundlePath::parse("order.md").unwrap())
        .unwrap()
        .text();
    assert_eq!(
        &text[range.start().to_usize()..range.end().to_usize()],
        "./generic.md"
    );
    assert!(
        unresolved.document.is_some()
            && unresolved.document_revision.is_some()
            && unresolved.range.is_some()
            && unresolved.span.is_some()
    );
}
