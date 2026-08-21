use waml_ui_test::{waml_ui_test, DiagramName, ViewKind, WamlApp};

const USE_CASE_SCREENSHOTS: [(&str, &str, &str); 3] = [
    (
        "docs/waml/use-cases/views/editor-workflows.md",
        "Editor Workflows",
        "screenshots/use-case/editor-workflows.png",
    ),
    (
        "docs/waml/use-cases/views/browser-and-publishing-workflows.md",
        "Browser and Publishing Workflows",
        "screenshots/use-case/browser-and-publishing-workflows.png",
    ),
    (
        "docs/waml/use-cases/views/tooling-workflows.md",
        "Tooling Workflows",
        "screenshots/use-case/tooling-workflows.png",
    ),
];

#[test]
fn use_case_screenshot_manifest() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (source, title, baseline) in USE_CASE_SCREENSHOTS {
        let source_text = std::fs::read_to_string(workspace.join(source)).unwrap();
        assert!(source_text.contains("type: uml.UseCaseDiagram"));
        assert!(source_text.contains(&format!("title: {title}")));
        assert!(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join(baseline)
            .is_file());
    }
}

#[waml_ui_test(workspace = Mini)]
fn open_and_switch_document_views(mut app: WamlApp) {
    app.expect_workspace_open()
        .ensure_diagram_open(DiagramName::ORDERS)
        .expect_active_diagram(DiagramName::ORDERS)
        .switch_active_document_to(ViewKind::Source)
        .expect_active_view(ViewKind::Source)
        .switch_active_document_to(ViewKind::Diagram)
        .expect_active_view(ViewKind::Diagram);
}

/// The rows the project tree draws for `Mini`, top to bottom. A projection
/// that silently drops a row leaves a view that still looks like a working
/// tree, only with content missing (visual sign-off ledger V4: "the failure
/// mode is invisible by construction") -- so the row list, not the pixels,
/// is the thing worth asserting. The same operation holds the tree to its
/// layout invariant, which is the other half nobody can see: a row can stay
/// in the model, keep reporting itself visible, and still collapse to a
/// zero-height rect.
#[waml_ui_test(workspace = Mini)]
fn project_tree_lists_every_row_of_the_bundle(mut app: WamlApp) {
    app.expect_workspace_open().expect_project_tree_rows(&[
        "Mini",
        "Customer",
        "Order",
        "Orders",
        "PaymentGateway",
    ]);
}

/// Opening a diagram selects its row and leaves that row in view. This is
/// the tree half of the reveal contract: a selection that scrolled out of
/// the viewport is not a landing, it is a selection the user cannot see.
#[waml_ui_test(workspace = Mini)]
fn opening_a_diagram_selects_its_row_in_view(mut app: WamlApp) {
    app.expect_workspace_open()
        .ensure_diagram_open(DiagramName::ORDERS)
        .expect_selected_row("Orders");
}

/// F3 walks the find strip's hits and wraps at both ends (spec §Find strip).
/// The counter reads `"1 of 4"` before the first step as well as after it --
/// `FindModel::counter_text` renders a `None` cursor as the first match --
/// so the traversal only becomes visible from the second step onward. That
/// is exactly the shape an off-by-one hides in, and nothing else in the
/// suite walks a cursor.
#[waml_ui_test(workspace = Mini)]
fn f3_walks_the_find_hits_and_wraps_at_both_ends(mut app: WamlApp) {
    app.expect_workspace_open()
        .ensure_diagram_open(DiagramName::ORDERS)
        .open_find_strip()
        .type_search_query("payment")
        .expect_find_counter("1 of 4")
        .advance_to_next_hit()
        .expect_find_counter("1 of 4")
        .advance_to_next_hit()
        .expect_find_counter("2 of 4")
        .advance_to_next_hit()
        .advance_to_next_hit()
        .expect_find_counter("4 of 4")
        .advance_to_next_hit()
        .expect_find_counter("1 of 4")
        .advance_to_previous_hit()
        .expect_find_counter("4 of 4");
}

/// The Ctrl+K palette blends a query's hits into titled sections (spec
/// §Palette). `Mini`'s `order.md` carries a prose sentence mentioning
/// "payment" alongside the `PaymentGateway` interface, so the query hits
/// every group but `RECENT`: `PaymentGateway`'s own title (CONCEPTS,
/// DOCUMENTS), the prose mention plus the diagram's two `PaymentGateway`
/// links (TEXT, STRUCTURE).
#[waml_ui_test(workspace = Mini)]
fn palette_blends_a_query_into_titled_sections(mut app: WamlApp) {
    app.expect_workspace_open()
        .open_search_palette()
        .type_search_query("payment")
        .expect_palette_sections(&[
            ("CONCEPTS", 1),
            ("DOCUMENTS", 1),
            ("TEXT", 4),
            ("STRUCTURE", 2),
        ]);
}

/// Escalating the same query to the full results tab groups every hit by
/// its document (spec §Results tab), in the order each document's first hit
/// appears in the ranked list.
#[waml_ui_test(workspace = Mini)]
fn escalating_a_query_groups_results_by_document(mut app: WamlApp) {
    app.expect_workspace_open()
        .open_search_palette()
        .type_search_query("payment")
        .escalate_to_results_tab()
        .expect_results_grouped_by_document(&[
            ("payment-gateway.md", 2),
            ("order.md", 2),
            ("orders-diagram.md", 4),
        ]);
}

/// Ctrl+F scopes its query to the active tab's own document (spec §Find
/// strip). With the Orders diagram active, "payment" matches only within
/// `orders-diagram.md` (its two `PaymentGateway` links, each a prose hit
/// plus a structure hit) -- a different total than the bundle-wide palette
/// query above, proving the scope actually narrows the search.
#[waml_ui_test(workspace = Mini)]
fn find_strip_counts_hits_scoped_to_the_active_document(mut app: WamlApp) {
    app.expect_workspace_open()
        .ensure_diagram_open(DiagramName::ORDERS)
        .open_find_strip()
        .type_search_query("payment")
        .expect_find_counter("1 of 4");
}
