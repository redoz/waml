use uaml::ops::{apply, Op};
use uaml::parse::split_bundle;

fn base(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).strip_suffix(".md").unwrap_or(path).to_string()
}

#[test]
fn rename_on_orders_domain_fixture_rewrites_all_referrers() {
    let blob = include_str!("fixtures/orders-domain.md");
    let bundle = split_bundle(blob);
    // Pick a slug the fixture actually defines and references. `order-line` is
    // composed by `order` and appears in the diagram's members. If the fixture
    // used a different slug, this would need to be retargeted.
    assert!(bundle.iter().any(|(p, _)| base(p) == "order-line"), "fixture defines order-line");

    let out = apply(&bundle, &[Op::NodeRename { from: "order-line".into(), to: "line-item".into() }]).unwrap();

    // The renamed doc is re-keyed to its new basename (directory preserved).
    assert!(out.iter().any(|(p, _)| base(p) == "line-item"), "renamed doc re-keyed to new basename");
    assert!(!out.iter().any(|(p, _)| base(p) == "order-line"), "old slug no longer present");

    // order.md's `composes` relationship target is rewritten.
    let order = &out.iter().find(|(p, _)| base(p) == "order").unwrap().1;
    assert!(order.contains("composes [OrderLine](./line-item.md)"), "order.md composes-target rewritten:\n{order}");
    assert!(!order.contains("order-line.md"), "no stale link left in order.md");

    // orders-domain.md's diagram member link is rewritten too.
    let diagram = &out.iter().find(|(p, _)| base(p) == "orders-domain").unwrap().1;
    assert!(diagram.contains("[OrderLine](./line-item.md)"), "diagram member link rewritten:\n{diagram}");
    assert!(!diagram.contains("order-line.md"), "no stale link left in orders-domain.md");
}
