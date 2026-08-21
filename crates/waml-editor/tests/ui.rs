use waml_ui_test::{waml_ui_test, DiagramName, DocumentSurface, ViewKind, WamlApp};

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

/// Keeps `scripts/check-use-case-diagram-screenshots.ps1`'s manifest wired to
/// the documents it drives: the three sources still declare the diagram type
/// and title it launches, and its three baselines are still on disk.
///
/// **This proves nothing about pixels**, and it never did -- the audit was
/// right to call the old name misleading. It cannot: those baselines are
/// captures of the real D3D11 renderer taken by hand on a desktop, and
/// nothing in CI can reproduce one. What it does prevent is the manual tool
/// silently rotting when someone renames a use-case view.
///
/// The automated rendering gate is
/// `the_light_cycle_canvas_is_drawn_the_way_its_reference_was`, below.
#[test]
fn use_case_manual_screenshot_tool_still_points_at_the_documents_it_drives() {
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
///
/// The surface check is the other half: the results tab is a document
/// surface like any other, and taking the centre means taking it from the
/// canvas the query was escalated over. A results tab drawn on top of a
/// still-visible canvas would group its hits perfectly well.
#[waml_ui_test(workspace = Mini)]
fn escalating_a_query_groups_results_by_document(mut app: WamlApp) {
    app.expect_workspace_open()
        .open_search_palette()
        .type_search_query("payment")
        .escalate_to_results_tab()
        .expect_active_surface(DocumentSurface::Results)
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

/// A route that crosses a surface boundary three times, in both directions,
/// with the centre checked at every stop (visual sign-off ledger V5).
///
/// The claim is not "the folder view appeared" but "the folder view is the
/// ONLY thing showing", which is the half of surface routing that fails
/// quietly: `DocView`'s `show_*` family is "mine on, my siblings off", and
/// the siblings half has already been wrong in shipped code -- each `show_*`
/// carried a hand-copied surface list, five of those copies never learned
/// about `behavior_canvas_wrap`, so leaving an activity/state-machine/
/// sequence tab left its canvas drawing underneath its replacement. Nothing failed and nothing looked wrong: the
/// stale surface is BEHIND the live one, so it is invisible to a screenshot
/// as well as to a human.
///
/// Two things this deliberately does NOT do, both because it cannot.
///
/// * **It does not press back or forward.** V5's stated forcing case ends in
///   a history traversal, and view history has exactly two triggers: the
///   caption's arrow pair, and the mouse's fourth/fifth buttons
///   (`App::handle_global_shortcuts`). The driver cannot send a thumb
///   button, and the caption band does not lay out at all under the headless
///   backend -- `caption_col`, `title_row`, `doc_tabs`, the burger, the
///   search button and both history arrows all report `visible: true` with a
///   0x0 rect, and a locator will not click a widget with no rect. So the
///   traversal half of V5 stays a human's job, and not for want of trying.
/// * **It does not name the active TAB.** `DocTabs` draws its tabs into its
///   own rects rather than mounting child widgets and exposes no semantic
///   items, so two tabs of the same kind are indistinguishable here. Hence
///   the deliberately cross-KIND route.
#[waml_ui_test(workspace = Mini)]
fn a_route_across_surfaces_leaves_exactly_one_of_them_showing(mut app: WamlApp) {
    app.expect_workspace_open()
        .ensure_diagram_open(DiagramName::ORDERS)
        .expect_active_surface(DocumentSurface::Canvas)
        .open_folder_tab("Mini")
        .expect_active_surface(DocumentSurface::Folder)
        .ensure_diagram_open(DiagramName::ORDERS)
        .expect_active_surface(DocumentSurface::Canvas)
        .switch_active_document_to(ViewKind::Source)
        .expect_active_surface(DocumentSurface::Source)
        .open_folder_tab("Mini")
        .expect_active_surface(DocumentSurface::Folder);
}

/// Committing a hit lands on the hit's own document, on that document's own
/// surface, with its row selected in the tree (visual sign-off ledger V7:
/// "did the selection land where the navigation said it would" is state).
///
/// `"settled"` is chosen because it appears in exactly one document in the
/// `Mini` bundle -- `order.md`'s prose sentence -- so the landing can be
/// named. Nothing in the snapshot reports the active tab's title, so a query
/// with one hit is how a scenario says where it went.
///
/// This settles the NAVIGATION half of `DocView::reveal`: the right
/// document, the right surface for its kind (`Order` is a `uml.Class`, so
/// the classifier preview takes the centre), and the tree agreeing. The
/// SCROLL half -- whether the landed document is scrolled to put the hit in
/// view -- is still not settled here, and not for a subtle reason: `Mini`'s
/// documents all fit on one screen, so there is nothing a reveal could
/// scroll. Covering it means a fixture with a document taller than the
/// viewport, not another assertion.
#[waml_ui_test(workspace = Mini)]
fn committing_a_hit_opens_its_document_and_selects_its_tree_row(mut app: WamlApp) {
    app.expect_workspace_open()
        .open_search_palette()
        .type_search_query("settled")
        .expect_palette_sections(&[("TEXT", 1)])
        .commit_the_armed_palette_row()
        .expect_active_surface(DocumentSurface::Reading)
        .expect_selected_row("Order");
}

/// **The rendering gate, second canvas.** The `Mini` bundle's `Orders` class
/// diagram, held to its own reference.
///
/// The gate's first scenario compares a BEHAVIOR canvas, which draws
/// transition routes and state boxes. This one compares the class canvas,
/// which draws none of those: what it does draw is a class association edge
/// (`Order` associates `Customer`), three class cards with their compartment
/// rules, an abstract title and a stereotype. Diagram pens (ledger V1) moved
/// class edges 3.0 -> 2.0 deliberately, and an ink mask is exactly the
/// instrument for a stroke that quantises to a different number of device
/// pixels.
///
/// It still covers only what is in THIS crop. Lifeline stems and interaction
/// frames -- V1's most likely regression, where 1.2 -> 1.5 doubles at dpi 1
/// -- are on a sequence canvas that no scenario opens, and colour is thrown
/// away by construction. See `waml_ui_test`'s crate docs.
#[waml_ui_test(workspace = Mini)]
fn the_orders_canvas_is_drawn_the_way_its_reference_was(mut app: WamlApp) {
    app.expect_workspace_open()
        .ensure_diagram_open(DiagramName::ORDERS)
        .expect_active_surface(DocumentSurface::Canvas)
        .expect_canvas_matches_reference("orders");
}

/// **The rendering gate.** Open the `Light Cycle` state machine and hold the
/// canvas to the reference recorded for this platform.
///
/// This is one of the suite's two scenarios that look at pixels -- the
/// behavior half; `the_orders_canvas_is_drawn_the_way_its_reference_was`
/// above is the class half. The fixture was chosen for the two connectors
/// `90ffcf0f` moved: `Active`'s
/// self-loop, which went from 16px to 24px of border clearance, and the
/// `Active -> Idle` back edge, which shifted 8px off its midpoint. Both are
/// changes every structural assertion in the router was blind to, and every
/// human was too -- ledger row V14 is still owed on them.
///
/// What this settles is GEOMETRY: where connectors run, how thick a stroke
/// quantises, where glyphs sit. Not colour, not antialias quality. See
/// `waml_ui_test`'s crate docs for the standing line.
#[waml_ui_test(workspace = Behavior)]
fn the_light_cycle_canvas_is_drawn_the_way_its_reference_was(mut app: WamlApp) {
    app.expect_workspace_open()
        .ensure_diagram_open(DiagramName::LIGHT_CYCLE)
        .expect_active_diagram(DiagramName::LIGHT_CYCLE)
        .expect_canvas_matches_reference("light-cycle");
}
