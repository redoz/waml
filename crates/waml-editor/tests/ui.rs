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
