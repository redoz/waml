use waml::diagnostic::DiagCode;
use waml::layout::{NameRef, OperandRef};
use waml::source::SourceBundle;

#[test]
fn authored_href_spelling_is_preserved_while_resolution_is_canonical() {
    let source = SourceBundle::try_from_pairs([
        (
            "shop/order.md",
            "---\ntype: uml.Class\n---\n# Order\n\n## Attributes\n- money: [Money](money.md)\n- currency: [Currency](../types/currency.md?compact#value)\n- selfRef: [Order](#attributes)\n\n## Relationships\n- depends [Currency](../types/currency.md#value)\n",
        ),
        (
            "shop/money.md",
            "---\ntype: uml.DataType\n---\n# Money\n",
        ),
        (
            "types/currency.md",
            "---\ntype: uml.DataType\n---\n# Currency\n",
        ),
        (
            "shop/d.md",
            "---\ntype: uml.ClassDiagram\nprofile: uml-domain\n---\n# D\n\n## Members\n- [Money](money.md)\n\n## Layout\n- [Money](money.md?compact#value)\n",
        ),
    ])
    .unwrap();
    let prepared = waml::analysis::prepare_candidate(source, None, 0).unwrap();
    let projection = &prepared.uml().projection;
    let order = projection.node("shop/order").unwrap();

    assert_eq!(order.attributes[0].ty.ref_.as_deref(), Some("shop/money"));
    assert_eq!(
        order.attributes[1].ty.ref_.as_deref(),
        Some("types/currency")
    );
    assert_eq!(order.attributes[2].ty.ref_.as_deref(), Some("shop/order"));
    assert_eq!(projection.edges[0].target, "types/currency");

    let diagram = projection
        .diagrams
        .iter()
        .find(|diagram| diagram.key == "shop/d")
        .unwrap();
    let waml::layout::LayoutStatement::Standalone(operand) = &diagram.layout[0] else {
        panic!("expected standalone layout operand");
    };
    assert!(matches!(
        &operand.ref_,
        OperandRef::Name(NameRef::Link { slug, .. })
            if slug == "money.md?compact#value"
    ));
    assert!(prepared
        .uml()
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code != DiagCode::UnresolvedLayoutRef));
}
