use waml::analysis::prepare_candidate;
use waml::source::SourceBundle;

#[test]
fn prepared_referrers_query_uses_the_existing_immutable_snapshot() {
    let source = SourceBundle::try_from_pairs([
        (
            "domain/order.md",
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
        ),
        (
            "domain/customer.md",
            "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n\n## Attributes\n- order: [Order](./order.md)\n",
        ),
        (
            "domain/diagram.md",
            "---\ntype: Diagram\ntitle: Domain\nprofile: uml-domain\n---\n# Domain\n\n## Layout\n- order left of customer\n",
        ),
    ])
    .unwrap();
    let prepared = prepare_candidate(source, None, 0).unwrap();

    assert_eq!(prepared.referrers("domain/order"), ["customer", "diagram"]);
    assert_eq!(prepared.revision(), 0);
}
