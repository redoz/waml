//! Whole-page string snapshots of `classifier_page`. The output is reviewable
//! text, so the assertion is the text.

use std::path::{Path, PathBuf};

use waml::classifier_page::classifier_page;
use waml::model::Model;
use waml::source::SourceBundle;

/// The shared fixture bundles live with the editor's tests; `waml-cli`'s own
/// tests reach them the same way (`crates/waml-cli/src/io.rs`).
fn fixture_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../waml-editor/tests/fixtures")
        .join(name)
}

/// Every `*.md` in `dir`, keyed by its bundle-relative path — the shape
/// `SourceBundle` wants, without pulling the editor's ingest in.
fn projection(name: &str) -> Model {
    let dir = fixture_dir(name);
    let mut pairs: Vec<(String, String)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("fixture {name} is readable: {error}"))
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                return None;
            }
            let key = path.file_name()?.to_str()?.to_string();
            Some((key, std::fs::read_to_string(&path).ok()?))
        })
        .collect();
    pairs.sort();
    let source = SourceBundle::try_from_pairs(pairs).expect("fixture bundle parses");
    waml::analysis::prepare_candidate(source, None, 0)
        .expect("fixture analyses")
        .uml()
        .projection
        .clone()
}

#[test]
fn sixkind_car_writes_every_relationship_kind_as_a_sentence() {
    let page = classifier_page(&projection("sixkind"), "car").expect("car has a page");
    assert_eq!(
        page,
        concat!(
            "# Car\n",
            "\n",
            "Class\n",
            "\n",
            "## Properties\n",
            "\n",
            "- `vin` · `String`\n",
            "\n",
            "## Associations\n",
            "\n",
            "- Specializes [Vehicle](/vehicle.md).\n",
            "- Implements [Drivable](/drivable.md).\n",
            "- Depends on [Fuel](/fuel.md).\n",
            "- Associated with one [Driver](/driver.md).\n",
            "- Aggregates zero or more [Wheel](/wheel.md).\n",
            "- Composes one [Engine](/engine.md).\n",
        )
    );
}

#[test]
fn groups_linked_order_states_the_far_end_role_and_count() {
    let page = classifier_page(&projection("groups-linked"), "order").expect("order has a page");
    assert_eq!(
        page,
        concat!(
            "# Order\n",
            "\n",
            "Class\n",
            "\n",
            "## Properties\n",
            "\n",
            "- `id` · `OrderId`\n",
            "- `total` · `Decimal`\n",
            "\n",
            "## Associations\n",
            "\n",
            "- Associated with one customer ([Customer](/customer.md)).\n",
            "- Associated with one invoice ([Invoice](/invoice.md)).\n",
        )
    );
}

/// The section a card could never draw: `Customer` declares no relationship of
/// its own, so everything here is derived from ANOTHER document's edges.
#[test]
fn groups_linked_customer_derives_referenced_by_across_documents() {
    let page =
        classifier_page(&projection("groups-linked"), "customer").expect("customer has a page");
    assert_eq!(
        page,
        concat!(
            "# Customer\n",
            "\n",
            "Class\n",
            "\n",
            "## Properties\n",
            "\n",
            "- `id` · `CustomerId`\n",
            "- `name` · `String`\n",
            "\n",
            "## Referenced by\n",
            "\n",
            "- [Account](/account.md) is associated with Customer.\n",
            "- [Order](/order.md) is associated with Customer.\n",
        )
    );
}

/// Both association sections must be OMITTED, not emitted empty. `mini`'s
/// `customer` IS referenced (by `order.md`'s `associates`), so this exercises
/// the omission of the OTHER section: no `## Associations` of its own.
#[test]
fn mini_customer_has_no_association_sections_at_all() {
    let page = classifier_page(&projection("mini"), "customer").expect("customer has a page");
    assert_eq!(
        page,
        concat!(
            "# Customer\n",
            "\n",
            "Class\n",
            "\n",
            "## Properties\n",
            "\n",
            "- `id` · `CustomerId`\n",
            "- `name` · `String`\n",
            "\n",
            "## Referenced by\n",
            "\n",
            "- [Order](/order.md) is associated with Customer.\n",
        )
    );
}

#[test]
fn a_missing_key_has_no_page() {
    assert_eq!(classifier_page(&projection("mini"), "nope"), None);
}
