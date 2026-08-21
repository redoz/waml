use super::super::shell::project_document_header;
use super::*;
use crate::doc_view::DocumentHeaderChrome;
use crate::platform_browser::ExternalUrlAdapter;

#[derive(Default)]
struct FakeBrowser {
    opened: Vec<String>,
    error: Option<String>,
}

impl ExternalUrlAdapter for FakeBrowser {
    fn open(&mut self, _cx: &mut Cx, url: &str) -> Result<(), String> {
        self.opened.push(url.into());
        self.error.clone().map_or(Ok(()), Err)
    }
}

#[test]
fn navigation_opens_an_empty_use_case_by_its_declared_diagram_kind() {
    let (mut cx, mut app) = navigation_app();
    let source = waml::source::SourceBundle::try_from_pairs([(
        "empty-use-case.md",
        "---\ntype: uml.UseCaseDiagram\ntitle: Empty Use Case\n---\n# Empty Use Case\n",
    )])
    .unwrap();
    app.session.replace(source).unwrap();

    let document = crate::documents::open(
        app.session.okf_analysis(),
        app.session.uml_analysis(),
        "empty-use-case",
    )
    .expect("the declared use-case document opens");
    assert_eq!(
        document.view.identity(),
        DocViewIdentity::StructuralDiagram(crate::StructuralVisualKind::UseCase),
        "the view identity comes from the declared diagram kind"
    );

    let mut browser = FakeBrowser::default();
    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Document {
            concept_id: "empty-use-case".into(),
            surface: None,
            fragment: None,
        },
        OpenDisposition::Preview,
        &mut browser,
    ));
    assert_eq!(
        app.documents.active_tab().and_then(|tab| tab.concept_id()),
        Some("empty-use-case")
    );
}

#[test]
fn equal_structural_members_dispatch_to_distinct_view_identities() {
    let (_cx, mut app) = navigation_app();
    let source = waml::source::SourceBundle::try_from_pairs([
        (
            "class-view.md",
            "---\ntype: uml.ClassDiagram\n---\n# Class view\n\n## Members\n- [Buyer](./buyer.md)\n",
        ),
        (
            "use-case-view.md",
            "---\ntype: uml.UseCaseDiagram\n---\n# Use-case view\n\n## Members\n\n### People\n- [Buyer](./buyer.md)\n",
        ),
        ("buyer.md", "---\ntype: uml.Actor\n---\n# Buyer\n"),
    ])
    .unwrap();
    app.session.replace(source).unwrap();

    let class = crate::documents::open(
        app.session.okf_analysis(),
        app.session.uml_analysis(),
        "class-view",
    )
    .unwrap();
    let use_case = crate::documents::open(
        app.session.okf_analysis(),
        app.session.uml_analysis(),
        "use-case-view",
    )
    .unwrap();
    assert_eq!(
        class.view.identity(),
        DocViewIdentity::StructuralDiagram(crate::StructuralVisualKind::Class)
    );
    assert_eq!(
        use_case.view.identity(),
        DocViewIdentity::StructuralDiagram(crate::StructuralVisualKind::UseCase)
    );
}

#[test]
fn invalid_use_case_edit_keeps_use_case_identity_and_last_projection() {
    let (_cx, mut app) = navigation_app();
    let valid = waml::source::SourceBundle::try_from_pairs([(
        "use-cases.md",
        "---\ntype: uml.UseCaseDiagram\n---\n# Use cases\n",
    )])
    .unwrap();
    app.session.replace(valid).unwrap();
    let invalid = waml::source::SourceBundle::try_from_pairs([(
        "use-cases.md",
        "---\ntype: uml.UseCaseDiagram\n---\n# Use cases\n\n## Members\n\n### Empty\n",
    )])
    .unwrap();
    app.session.replace(invalid).unwrap();
    let document = crate::documents::open(
        app.session.okf_analysis(),
        app.session.uml_analysis(),
        "use-cases",
    )
    .unwrap();

    assert_eq!(
        document.view.identity(),
        DocViewIdentity::StructuralDiagram(crate::StructuralVisualKind::UseCase)
    );
}

struct ResettingAnchorView(Rc<RefCell<ViewAnchor>>);

impl DocView for ResettingAnchorView {
    fn identity(&self) -> DocViewIdentity {
        DocViewIdentity::GenericOkf
    }

    fn sync(&mut self, _: &mut Cx, _: &BodyWidgets, _: ViewData<'_>) {
        *self.0.borrow_mut() = ViewAnchor::None;
    }

    fn handle(
        &mut self,
        _: &mut Cx,
        _: &BodyWidgets,
        _: &Actions,
        _: ViewData<'_>,
    ) -> crate::doc_view::ViewOutcome {
        crate::doc_view::ViewOutcome::default()
    }

    fn chrome(&self) -> crate::doc_view::BodyChrome {
        crate::doc_view::BodyChrome::HIDDEN
    }

    fn capture_anchor(&self, _: &BodyWidgets) -> ViewAnchor {
        self.0.borrow().clone()
    }

    fn restore_anchor(
        &mut self,
        _: &mut Cx,
        _: &BodyWidgets,
        _: ViewData<'_>,
        anchor: &ViewAnchor,
    ) -> bool {
        *self.0.borrow_mut() = anchor.clone();
        true
    }
}

fn navigation_app_with_anchor_probe(anchor: ViewAnchor) -> (Cx, App, Rc<RefCell<ViewAnchor>>) {
    let (mut cx, mut app) = navigation_app();
    let state = Rc::new(RefCell::new(anchor.clone()));
    app.documents.transition(
        &mut cx,
        &app.ui,
        &app.session,
        DocumentCommand::Open {
            document: OpenDocument {
                tab_id: LiveId::from_str("anchor-probe"),
                locator: crate::view_history::DocumentLocator::concept(
                    "sales/order",
                    waml::view::surface::SurfaceId::markdown(),
                ),
                title: "Order".into(),
                presentation: DocumentPresentation {
                    icon: Icon::StickyNote,
                    accent: None,
                    category: NavCategory::OkfDocument,
                },
                view: Box::new(ResettingAnchorView(state.clone())),
            },
            persistent: true,
        },
    );
    *state.borrow_mut() = anchor;
    app.view_history
        .reset(app.documents.capture_active_location(&mut cx, &app.ui));
    (cx, app, state)
}

fn widget_action(uid: WidgetUid, action: impl WidgetActionTrait + 'static) -> Action {
    Box::new(WidgetAction {
        data: None,
        action: Box::new(action),
        widget_uid: uid,
        group: None,
    })
}

fn diagram_properties_app() -> (Cx, App) {
    let mut cx = Cx::new(Box::new(|_, _| {}));
    cx.init_cx_os();
    cx.widget_tree_mark_dirty(WidgetUid(0));
    let mut app = cx.with_vm(App::script_new_with_default);
    let source = waml::source::SourceBundle::try_from_pairs([(
        "orders.md",
        "---\ntype: uml.ClassDiagram\ntitle: Orders\nprofile: uml-domain\ndescription: Initial\n---\n# Orders\n",
    )])
    .unwrap();
    app.session.replace(source).unwrap();

    let tool_dock = WidgetRef::new_with_inner(Box::new(
        cx.with_vm(crate::tool_dock::ToolDock::script_new_with_default),
    ));
    let diagram_properties = WidgetRef::new_with_inner(Box::new(
        cx.with_vm(crate::diagram_properties::DiagramProperties::script_new_with_default),
    ));
    let mut diagram_properties_wrap = cx.with_vm(View::script_new_with_default);
    diagram_properties_wrap
        .children
        .push((live_id!(diagram_properties), diagram_properties));
    let diagram_properties_wrap = WidgetRef::new_with_inner(Box::new(diagram_properties_wrap));
    let mut ui = cx.with_vm(View::script_new_with_default);
    ui.children.push((live_id!(tool_dock), tool_dock));
    ui.children
        .push((live_id!(diagram_properties_wrap), diagram_properties_wrap));
    app.ui = WidgetRef::new_with_inner(Box::new(ui));

    let document = crate::documents::open(
        app.session.okf_analysis(),
        app.session.uml_analysis(),
        "orders",
    )
    .unwrap();
    app.documents.transition(
        &mut cx,
        &app.ui,
        &app.session,
        DocumentCommand::Open {
            document,
            persistent: true,
        },
    );
    (cx, app)
}

#[test]
fn retained_property_widget_accepts_sequential_actions_after_session_refresh() {
    let (mut cx, mut app) = diagram_properties_app();
    let initial_revision = app.session.revision();
    let tool_dock_uid = app.ui.widget(&cx, ids!(tool_dock)).widget_uid();
    let diagram_properties_uid = app
        .ui
        .widget(&cx, ids!(diagram_properties_wrap.diagram_properties))
        .widget_uid();

    app.handle_action_batch(
        &mut cx,
        &[widget_action(
            tool_dock_uid,
            crate::tool_dock::ToolDockAction::Triggered(crate::tool_dock::Tool::DiagramProps),
        )],
    );
    app.handle_action_batch(
        &mut cx,
        &[widget_action(
            diagram_properties_uid,
            crate::diagram_properties::DiagramPropertiesAction::DescriptionChanged(Some(
                "First edit".into(),
            )),
        )],
    );
    app.handle_action_batch(
        &mut cx,
        &[widget_action(
            diagram_properties_uid,
            crate::diagram_properties::DiagramPropertiesAction::DescriptionChanged(Some(
                "Second edit".into(),
            )),
        )],
    );

    assert_eq!(app.session.revision(), initial_revision + 2);
    assert_eq!(
        app.session
            .uml_analysis()
            .projection
            .diagrams
            .iter()
            .find(|diagram| diagram.key == "orders")
            .and_then(|diagram| diagram.description.as_deref()),
        Some("Second edit"),
    );
    let text = app.session.source().documents()[0].text();
    assert!(text.contains("description: Second edit"), "{text}");
}

pub(super) fn mount_markdown_surface(cx: &mut Cx, app: &mut App) {
    waml_markdown_editor::live_design(cx);
    let markdown = WidgetRef::new_with_inner(Box::new(
        cx.with_vm(waml_markdown_editor::widget::MarkdownEditor::script_new_with_default),
    ));
    let mut surface = cx.with_vm(View::script_new_with_default);
    surface.children.push((live_id!(editor), markdown));
    let surface = WidgetRef::new_with_inner(Box::new(surface));
    app.ui
        .borrow_mut::<View>()
        .expect("test root view is mounted")
        .children
        .push((live_id!(markdown_surface), surface));
    cx.widget_tree_mark_dirty(app.ui.widget_uid());
}

fn mounted_source_app() -> (Cx, App) {
    let (mut cx, mut app) = navigation_app();
    let source = waml::source::SourceBundle::try_from_pairs([(
        "notes/order.md",
        "---\ntype: Runbook\ntitle: Order\n---\n# Order\nBody\n",
    )])
    .unwrap();
    let change = app.session.replace(source).unwrap();
    app.complete_session_change(&mut cx, change);
    mount_markdown_surface(&mut cx, &mut app);
    app.open_view_source(&mut cx, "notes/order");
    let revision = match app
        .documents
        .capture_active_location(&mut cx, &app.ui)
        .expect("the source tab must be active")
        .anchor
    {
        ViewAnchor::Markdown { revision, .. } => revision,
        _ => panic!("the source tab must own a Markdown anchor"),
    };
    app.ui
        .widget(&cx, ids!(markdown_surface.editor))
        .as_markdown_editor()
        .test_set_layout(Arc::new(LayoutSnapshot::from_parts_for_test(
            revision,
            dvec2(1.0, 1.0),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )));
    (cx, app)
}

#[test]
fn mounted_text_event_promotes_through_the_active_source_view() {
    let (mut cx, mut app) = mounted_source_app();
    let active = app.documents.active_id();
    let before_revision = app.session.revision();
    let editor = app
        .ui
        .widget(&cx, ids!(markdown_surface.editor))
        .as_markdown_editor();
    editor.set_key_focus(&mut cx);

    let actions = cx.capture_actions(|cx| {
        <App as AppMain>::handle_event(
            &mut app,
            cx,
            &Event::TextInput(TextInputEvent {
                input: "X".to_owned(),
                ..Default::default()
            }),
        );
    });
    app.handle_action_batch(&mut cx, &actions);

    assert_eq!(app.documents.active_id(), active);
    assert_eq!(app.session.revision(), before_revision + 1);
    assert_eq!(
        app.session
            .source()
            .document_by_concept_id("notes/order")
            .unwrap()
            .text(),
        "X---\ntype: Runbook\ntitle: Order\n---\n# Order\nBody\n"
    );
    let location = app
        .documents
        .capture_active_location(&mut cx, &app.ui)
        .unwrap();
    let ViewAnchor::Markdown {
        revision,
        selection,
        ..
    } = location.anchor
    else {
        panic!("the retained source view must capture a Markdown anchor");
    };
    assert_eq!(selection.revision(), revision);
    assert_eq!(selection.primary().cursor.offset, TextSize::new(1));
}

#[test]
fn source_range_navigation_activates_source_and_selects_the_current_range() {
    let (mut cx, mut app) = mounted_source_app();
    assert!(app.transition_to_location(
        &mut cx,
        ViewLocation {
            document: crate::navigation::DocumentLocator::concept(
                "notes/order",
                waml::view::surface::SurfaceId::markdown()
            ),
            anchor: ViewAnchor::None,
        },
        TransitionCause::UserNavigation,
    ));
    let snapshot = app.session.snapshot();
    let source = snapshot
        .source
        .document_by_concept_id("notes/order")
        .unwrap();
    let document = snapshot
        .okf_analysis
        .catalog
        .id_for_path(source.path())
        .unwrap();
    let syntax = snapshot.markdown_snapshot(document).unwrap();
    let start = source.text().find("Body").unwrap();
    let range = TextRange::new(
        TextSize::try_from_usize(start).unwrap(),
        TextSize::try_from_usize(start + 4).unwrap(),
    )
    .unwrap();

    assert!(app.handle_navigation_intent(
        &mut cx,
        NavigationIntent::SourceRange {
            document,
            revision: syntax.revision(),
            range,
        },
    ));

    assert_eq!(
        app.documents.active_tab().unwrap().locator.surface,
        waml::view::surface::SurfaceId::source()
    );
    let location = app
        .documents
        .capture_active_location(&mut cx, &app.ui)
        .unwrap();
    let ViewAnchor::Markdown { selection, .. } = location.anchor else {
        panic!("source navigation must install a Markdown selection");
    };
    assert_eq!(selection.primary().range(), range);
}

// Scenario: NATIVE-050
#[test]
fn changed_source_range_navigation_preserves_selection_and_publishes_status() {
    let (mut cx, mut app) = mounted_source_app();
    let before = app.session.snapshot();
    let source = before.source.document_by_concept_id("notes/order").unwrap();
    let document = before
        .okf_analysis
        .catalog
        .id_for_path(source.path())
        .unwrap();
    let revision = before.markdown_snapshot(document).unwrap().revision();
    let editor = app
        .ui
        .widget(&cx, ids!(markdown_surface.editor))
        .as_markdown_editor();
    editor.set_key_focus(&mut cx);
    let actions = cx.capture_actions(|cx| {
        <App as AppMain>::handle_event(
            &mut app,
            cx,
            &Event::TextInput(TextInputEvent {
                input: "X".to_owned(),
                ..Default::default()
            }),
        );
    });
    app.handle_action_batch(&mut cx, &actions);
    let selection_before = match app
        .documents
        .capture_active_location(&mut cx, &app.ui)
        .unwrap()
        .anchor
    {
        ViewAnchor::Markdown { selection, .. } => selection,
        _ => panic!("the source tab must own a Markdown selection"),
    };

    assert!(!app.handle_navigation_intent(
        &mut cx,
        NavigationIntent::SourceRange {
            document,
            revision,
            range: TextRange::new(TextSize::new(0), TextSize::new(1)).unwrap(),
        },
    ));

    let selection_after = match app
        .documents
        .capture_active_location(&mut cx, &app.ui)
        .unwrap()
        .anchor
    {
        ViewAnchor::Markdown { selection, .. } => selection,
        _ => panic!("the source tab must retain its Markdown selection"),
    };
    assert_eq!(selection_after, selection_before);
    let statusbar = app.ui.widget(&cx, ids!(statusbar));
    let statusbar = statusbar
        .borrow::<crate::statusbar::Statusbar>()
        .expect("test statusbar is mounted");
    assert_eq!(
        crate::statusbar::navigation_message(&statusbar),
        Some("Source location is no longer available")
    );
}

#[test]
fn normal_session_completion_retains_the_active_source_view_as_missing() {
    let (mut cx, mut app) = mounted_source_app();
    let active = app.documents.active_id();
    let change = app
        .session
        .replace(waml::source::SourceBundle::default())
        .unwrap();
    app.complete_session_change(&mut cx, change);
    let missing_revision = app.session.revision();
    let editor = app
        .ui
        .widget(&cx, ids!(markdown_surface.editor))
        .as_markdown_editor();
    editor.set_key_focus(&mut cx);

    let actions = cx.capture_actions(|cx| {
        <App as AppMain>::handle_event(
            &mut app,
            cx,
            &Event::TextInput(TextInputEvent {
                input: "X".to_owned(),
                ..Default::default()
            }),
        );
    });
    app.handle_action_batch(&mut cx, &actions);

    assert_eq!(app.documents.active_id(), active);
    assert_eq!(app.session.revision(), missing_revision);
    assert!(app.session.source().documents().is_empty());
    assert!(matches!(
        app.documents
            .capture_active_location(&mut cx, &app.ui)
            .unwrap()
            .anchor,
        ViewAnchor::Markdown { .. }
    ));
}

fn record_markdown_anchors(cx: &mut Cx, app: &App) {
    let Some(ViewAnchor::Markdown { revision, .. }) = app
        .documents
        .capture_active_location(cx, &app.ui)
        .map(|location| location.anchor)
    else {
        return;
    };
    app.ui
        .widget(cx, ids!(markdown_surface.editor))
        .as_markdown_editor()
        .test_set_layout(Arc::new(LayoutSnapshot::from_parts_for_test(
            revision,
            dvec2(1.0, 1.0),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )));
}

/// The flat `key_string` the production tree keys an `index`-owned directory
/// row on -- what `project_tree_folder_is_open` and `id_to_key` compare
/// against since Task 7's `RowId` keys. `address` here is the trimmed OKF
/// address (no leading `/`), matching `RootView::folder_row`'s own trim.
fn tree_key(address: &str) -> String {
    crate::tree::key_string(&waml::view::row::RowId {
        owner: waml::view::row::ViewId::new(waml::view::ROOT_VIEW_OWNER),
        path: waml::view::row::RowPath::parse(address.trim_start_matches('/')).unwrap(),
    })
}

fn project_tree_folder_is_open(cx: &mut Cx, app: &App, key: &str) -> bool {
    app.ui
        .widget(cx, ids!(project_tree))
        .borrow::<crate::tree_panel::ProjectTree>()
        .expect("production shell mounts project_tree")
        .test_folder_is_open(key)
}

fn mounted_project_tree_state(cx: &Cx, app: &App) -> DockState {
    app.ui
        .widget(cx, ids!(project_tree))
        .borrow::<crate::tree_panel::ProjectTree>()
        .expect("production shell mounts project_tree")
        .dock_state()
}

fn project_tree_selected_key(cx: &Cx, app: &App) -> Option<String> {
    app.ui
        .widget(cx, ids!(project_tree))
        .borrow::<crate::tree_panel::ProjectTree>()
        .and_then(|tree| tree.test_selected_key().map(str::to_owned))
}

// Scenario: NATIVE-015
#[test]
fn navigation_external_target_invokes_only_the_browser_adapter_once() {
    let (mut cx, mut app) = navigation_app();
    let mut browser = FakeBrowser::default();

    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::ExternalUrl("https://example.com/docs".into()),
        OpenDisposition::Preview,
        &mut browser,
    ));
    assert_eq!(browser.opened, vec!["https://example.com/docs"]);
    assert!(app.documents.tabs().is_empty());
    assert_eq!(app.nav_state, NavState::default());
}

#[test]
fn navigation_browser_failure_preserves_document_and_directory_state() {
    let (mut cx, mut app) = navigation_app();
    let mut browser = FakeBrowser::default();
    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Document {
            concept_id: "sales/order".into(),
            surface: None,
            fragment: None,
        },
        OpenDisposition::Persistent,
        &mut browser,
    ));
    app.nav_state.scope = "/sales".into();
    let active = app.documents.active_id();
    let nav_state = app.nav_state.clone();
    browser.error = Some("blocked".into());

    assert!(!app.navigate_with(
        &mut cx,
        NavigationTarget::ExternalUrl("https://example.com/blocked".into()),
        OpenDisposition::Preview,
        &mut browser,
    ));
    assert_eq!(browser.opened, vec!["https://example.com/blocked"]);
    assert_eq!(app.documents.active_id(), active);
    assert_eq!(app.nav_state, nav_state);
    let statusbar = app.ui.widget(&cx, ids!(statusbar));
    let statusbar = statusbar
        .borrow::<crate::statusbar::Statusbar>()
        .expect("test statusbar is mounted");
    assert_eq!(
        crate::statusbar::navigation_message(&statusbar),
        Some("Could not open link: blocked")
    );
    drop(statusbar);
    app.ui
        .widget(&cx, ids!(statusbar))
        .borrow_mut::<crate::statusbar::Statusbar>()
        .expect("test statusbar is mounted")
        .set_save_error(&mut cx, Some("disk full"));
    browser.error = None;
    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::ExternalUrl("https://example.com/retry".into()),
        OpenDisposition::Preview,
        &mut browser,
    ));
    let statusbar = app.ui.widget(&cx, ids!(statusbar));
    let statusbar = statusbar
        .borrow::<crate::statusbar::Statusbar>()
        .expect("test statusbar is mounted");
    assert_eq!(crate::statusbar::navigation_message(&statusbar), None);
    assert_eq!(crate::statusbar::save_error(&statusbar), Some("disk full"));
}

/// spec Testing bullet 4: an explicit `surface` on `NavigationTarget::Document`
/// survives the navigation and produces the SAME tab identity that
/// `open_view_source` produces for the same key -- the duplication this plan
/// exists to remove, asserted as identity.
#[test]
fn navigation_document_explicit_surface_survives_and_matches_view_source_identity() {
    let (mut cx, mut app) = navigation_app();
    let mut browser = FakeBrowser::default();

    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Document {
            concept_id: "sales/order".into(),
            surface: Some(waml::view::surface::SurfaceId::source()),
            fragment: None,
        },
        OpenDisposition::Preview,
        &mut browser,
    ));

    let active_tab_id = app.documents.active_id();
    assert_eq!(
        app.documents.active_tab().unwrap().locator,
        crate::navigation::DocumentLocator::source("sales/order")
    );

    app.open_view_source(&mut cx, "sales/order");
    assert_eq!(
        app.documents.active_id(),
        active_tab_id,
        "an explicit-surface navigation and open_view_source must land on the same tab"
    );
}

#[test]
fn navigation_document_preview_persistence_and_repeat_activation_are_stable() {
    let (mut cx, mut app) = navigation_app();
    let mut browser = FakeBrowser::default();
    let order = NavigationTarget::Document {
        concept_id: "sales/order".into(),
        surface: None,
        fragment: None,
    };

    assert!(app.navigate_with(
        &mut cx,
        order.clone(),
        OpenDisposition::Preview,
        &mut browser,
    ));
    assert_eq!(app.documents.tabs().len(), 1);
    assert!(app.documents.tabs()[0].preview);

    assert!(app.navigate_with(
        &mut cx,
        order.clone(),
        OpenDisposition::Persistent,
        &mut browser,
    ));
    assert_eq!(app.documents.tabs().len(), 1);
    assert!(!app.documents.tabs()[0].preview);

    assert!(app.navigate_with(&mut cx, order, OpenDisposition::Persistent, &mut browser,));
    assert_eq!(app.documents.tabs().len(), 1);
    assert!(!app.documents.tabs()[0].preview);
}

// Scenario: NATIVE-014
#[test]
fn navigation_markdown_resolves_only_at_the_app_boundary() {
    let (mut cx, mut app) = navigation_app();

    assert!(app.handle_navigation_intent(
        &mut cx,
        NavigationIntent::MarkdownLink {
            current_concept_id: "sales/order".into(),
            href: "./customer.md".into(),
        },
    ));
    assert_eq!(
        app.documents.active_tab().and_then(|tab| tab.concept_id()),
        Some("sales/customer")
    );
    assert!(app.documents.tabs()[0].preview);
}

fn resolved_target(intent: &NavigationIntent) -> Option<&NavigationTarget> {
    match intent {
        NavigationIntent::Resolved { target, .. } => Some(target),
        NavigationIntent::MarkdownLink { .. } | NavigationIntent::SourceRange { .. } => None,
    }
}

fn navigation_app_with_active_order() -> (Cx, App) {
    let (mut cx, mut app) = navigation_app();
    mount_markdown_surface(&mut cx, &mut app);
    assert!(app.transition_to_location(
        &mut cx,
        ViewLocation {
            document: crate::navigation::DocumentLocator::concept(
                "sales/order",
                waml::view::surface::SurfaceId::markdown()
            ),
            anchor: ViewAnchor::None,
        },
        TransitionCause::UserNavigation,
    ));
    assert_eq!(
        app.documents
            .active_tab()
            .and_then(|tab| tab.concept_id().map(|id| (id, tab.preview))),
        Some(("sales/order", true))
    );
    assert!(app
        .ui
        .widget(&cx, ids!(markdown_surface.editor))
        .borrow::<waml_markdown_editor::widget::MarkdownEditor>()
        .is_some());
    (cx, app)
}

// Scenario: NATIVE-016
#[test]
fn manual_and_preview_transitions_follow_back_and_forward_history() {
    let (mut cx, mut app) = navigation_app_with_active_order();
    assert!(app.transition_to_location(
        &mut cx,
        ViewLocation {
            document: crate::navigation::DocumentLocator::concept(
                "sales/customer",
                waml::view::surface::SurfaceId::markdown()
            ),
            anchor: ViewAnchor::None,
        },
        TransitionCause::UserNavigation,
    ));
    assert_eq!(app.documents.tabs().len(), 1);
    assert!(app.documents.tabs()[0].preview);
    assert_eq!(app.view_history.len(), 2);

    assert!(app.traverse_view_history(&mut cx, HistoryDirection::Back));
    assert_eq!(
        app.documents.active_tab().unwrap().concept_id().unwrap(),
        "sales/order"
    );
    assert_eq!(app.documents.tabs().len(), 1);
    assert!(app.documents.tabs()[0].preview);

    assert!(app.traverse_view_history(&mut cx, HistoryDirection::Forward));
    assert_eq!(
        app.documents.active_tab().unwrap().concept_id().unwrap(),
        "sales/customer"
    );
    assert_eq!(app.view_history.len(), 2);
}

#[test]
fn back_and_forward_stop_on_folder_tabs() {
    let (mut cx, mut app) = navigation_app_with_active_order();
    let mut browser = FakeBrowser::default();
    // Persistent, so it survives the directory hop below.
    assert!(app.transition_document(&mut cx, "sales/order", true));
    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Directory {
            address: "/sales".into(),
        },
        OpenDisposition::Preview,
        &mut browser,
    ));
    let folder_tab_id =
        crate::documents::tab_id_for(&crate::navigation::DocumentLocator::folder("/sales"));
    assert_eq!(app.documents.active_id(), folder_tab_id);
    let folder_tab_locator = crate::navigation::DocumentLocator::folder("/sales");

    assert!(app.transition_to_location(
        &mut cx,
        ViewLocation {
            document: crate::navigation::DocumentLocator::concept(
                "sales/customer",
                waml::view::surface::SurfaceId::markdown()
            ),
            anchor: ViewAnchor::None,
        },
        TransitionCause::UserNavigation,
    ));
    assert_eq!(
        app.documents.active_tab().unwrap().concept_id().unwrap(),
        "sales/customer"
    );

    assert!(app.traverse_view_history(&mut cx, HistoryDirection::Back));
    assert_eq!(
        app.documents.active_id(),
        folder_tab_id,
        "Back must stop on the /sales folder tab, not skip past it"
    );

    assert!(app.traverse_view_history(&mut cx, HistoryDirection::Back));
    assert_eq!(
        app.documents.active_tab().unwrap().concept_id().unwrap(),
        "sales/order"
    );

    assert!(app.traverse_view_history(&mut cx, HistoryDirection::Forward));
    assert_eq!(
        app.documents.active_id(),
        folder_tab_id,
        "Forward must stop on the /sales folder tab, not skip past it"
    );

    assert_eq!(
        app.documents.tab_id_for_locator(&folder_tab_locator),
        Some(folder_tab_id)
    );
}

#[test]
fn tab_row_history_actions_traverse_once_and_report_unavailable_targets() {
    let (mut cx, mut app) = navigation_app_with_active_order();
    assert!(app.transition_document(&mut cx, "sales/customer", false));
    assert_eq!(app.test_history_enabled(&mut cx), (true, false));

    let back_button_uid = app.ui.widget(&cx, ids!(history_back_btn)).widget_uid();
    app.handle_action_batch(
        &mut cx,
        &[widget_action(
            back_button_uid,
            crate::icon_button::IconButtonAction::TaggedClicked(live_id!(history_back)),
        )],
    );
    assert_eq!(
        app.documents.active_tab().and_then(|tab| tab.concept_id()),
        Some("sales/order")
    );
    assert_eq!(app.test_history_enabled(&mut cx), (false, true));

    let forward_button_uid = app.ui.widget(&cx, ids!(history_forward_btn)).widget_uid();
    app.handle_action_batch(
        &mut cx,
        &[widget_action(
            forward_button_uid,
            crate::icon_button::IconButtonAction::TaggedClicked(live_id!(history_forward)),
        )],
    );
    assert_eq!(
        app.documents.active_tab().and_then(|tab| tab.concept_id()),
        Some("sales/customer")
    );

    app.handle_action_batch(
        &mut cx,
        &[widget_action(
            back_button_uid,
            crate::icon_button::IconButtonAction::TaggedClicked(live_id!(history_back)),
        )],
    );
    app.handle_action_batch(
        &mut cx,
        &[widget_action(
            back_button_uid,
            crate::icon_button::IconButtonAction::TaggedClicked(live_id!(history_back)),
        )],
    );
    let statusbar = app.ui.widget(&cx, ids!(statusbar));
    let statusbar = statusbar
        .borrow::<crate::statusbar::Statusbar>()
        .expect("test statusbar is mounted");
    assert_eq!(
        crate::statusbar::history_feedback(&statusbar),
        (Some("No previous view"), None)
    );
}

#[test]
fn back_then_manual_navigation_clears_forward() {
    let (mut cx, mut app) = navigation_app_with_active_order();
    for concept_id in ["sales/customer", "sales/next"] {
        assert!(app.transition_to_location(
            &mut cx,
            ViewLocation {
                document: crate::navigation::DocumentLocator::concept(
                    concept_id,
                    waml::view::surface::SurfaceId::markdown()
                ),
                anchor: ViewAnchor::None,
            },
            TransitionCause::UserNavigation,
        ));
    }
    assert!(app.traverse_view_history(&mut cx, HistoryDirection::Back));
    assert_eq!(
        app.documents.active_tab().unwrap().concept_id().unwrap(),
        "sales/customer"
    );

    assert!(app.transition_to_location(
        &mut cx,
        ViewLocation {
            document: crate::navigation::DocumentLocator::concept(
                "sales/order",
                waml::view::surface::SurfaceId::markdown()
            ),
            anchor: ViewAnchor::None,
        },
        TransitionCause::UserNavigation,
    ));

    assert!(!app
        .view_history
        .can_traverse(HistoryDirection::Forward, |_| true));
}

#[test]
fn repeat_current_user_navigation_preserves_the_active_anchor() {
    let anchor = ViewAnchor::Diagram {
        selected_key: Some("sales/customer".into()),
        camera: crate::view_history::DiagramCameraAnchor {
            pan_x: 12.0,
            pan_y: 34.0,
            zoom: 1.5,
        },
    };
    let (mut cx, mut app, state) = navigation_app_with_anchor_probe(anchor.clone());

    assert!(app.transition_to_location(
        &mut cx,
        ViewLocation {
            document: crate::navigation::DocumentLocator::concept(
                "sales/order",
                waml::view::surface::SurfaceId::markdown()
            ),
            anchor: ViewAnchor::None,
        },
        TransitionCause::UserNavigation,
    ));

    assert_eq!(*state.borrow(), anchor);
}

#[test]
fn same_document_undo_reveal_records_the_departing_anchor_for_back() {
    let departing = ViewAnchor::Diagram {
        selected_key: Some("sales/customer".into()),
        camera: crate::view_history::DiagramCameraAnchor {
            pan_x: 12.0,
            pan_y: 34.0,
            zoom: 1.5,
        },
    };
    let (mut cx, mut app, _) = navigation_app_with_anchor_probe(departing.clone());

    assert!(app.transition_to_location(
        &mut cx,
        ViewLocation {
            document: crate::navigation::DocumentLocator::concept(
                "sales/order",
                waml::view::surface::SurfaceId::markdown()
            ),
            anchor: ViewAnchor::Diagram {
                selected_key: None,
                camera: crate::view_history::DiagramCameraAnchor {
                    pan_x: 40.0,
                    pan_y: 50.0,
                    zoom: 2.0,
                },
            },
        },
        TransitionCause::UndoRedoReveal,
    ));

    let back = app
        .view_history
        .target(HistoryDirection::Back, |_| true)
        .expect("Undo reveal must create a Back entry even within one document");
    assert_eq!(back.location.anchor, departing);
}

#[test]
fn active_close_records_fallback_but_promote_and_inactive_close_do_not() {
    let (mut cx, mut app) = navigation_app_with_active_order();
    let order_id = app.documents.active_id();
    app.documents.transition(
        &mut cx,
        &app.ui,
        &app.session,
        DocumentCommand::Promote(order_id),
    );
    let history_after_promote = app.view_history.len();
    assert!(app.transition_to_location(
        &mut cx,
        ViewLocation {
            document: crate::navigation::DocumentLocator::concept(
                "sales/customer",
                waml::view::surface::SurfaceId::markdown()
            ),
            anchor: ViewAnchor::None,
        },
        TransitionCause::UserNavigation,
    ));
    let customer_id = app.documents.active_id();
    assert_eq!(app.documents.tabs().len(), 2);
    let history_before_inactive_close = app.view_history.len();

    assert!(app.close_document(&mut cx, order_id));
    assert_eq!(app.view_history.len(), history_before_inactive_close);
    assert_eq!(app.documents.active_id(), customer_id);

    app.documents.transition(
        &mut cx,
        &app.ui,
        &app.session,
        DocumentCommand::Promote(customer_id),
    );
    assert!(app.transition_to_location(
        &mut cx,
        ViewLocation {
            document: crate::navigation::DocumentLocator::concept(
                "sales/next",
                waml::view::surface::SurfaceId::markdown()
            ),
            anchor: ViewAnchor::None,
        },
        TransitionCause::UserNavigation,
    ));
    let before_active_close = app.view_history.len();
    let next_id = app.documents.active_id();
    assert!(app.close_document(&mut cx, next_id));
    assert_eq!(app.view_history.len(), before_active_close + 1);
    assert_eq!(
        app.documents.active_tab().unwrap().concept_id().unwrap(),
        "sales/customer"
    );
    assert!(history_after_promote > 0);
}

#[test]
fn undo_reveals_the_document_where_the_edit_started_and_records_the_move() {
    let (mut cx, mut app) = navigation_app_with_active_order();
    let order = ViewLocation {
        document: crate::navigation::DocumentLocator::concept(
            "sales/order",
            waml::view::surface::SurfaceId::markdown(),
        ),
        anchor: ViewAnchor::None,
    };
    let customer = ViewLocation {
        document: crate::navigation::DocumentLocator::concept(
            "sales/customer",
            waml::view::surface::SurfaceId::markdown(),
        ),
        anchor: ViewAnchor::None,
    };
    app.session
        .apply_edit(crate::editor_session::EditRequest {
            before_location: order.clone(),
            intent: crate::document::EditIntent {
                edit: waml::edit::PendingEdit::new(waml::okf::Batch(vec![
                    waml::okf::Op::IndexRetitle {
                        directory: waml::okf::DirectoryAddress::parse("/sales").unwrap(),
                        title: "Commerce".into(),
                    },
                ])),
                label: "Rename sales".into(),
                merge_key: None,
                after_location: Some(customer),
            },
        })
        .unwrap();
    assert!(app.transition_to_location(
        &mut cx,
        ViewLocation {
            document: crate::navigation::DocumentLocator::concept(
                "sales/next",
                waml::view::surface::SurfaceId::markdown()
            ),
            anchor: ViewAnchor::None,
        },
        TransitionCause::UserNavigation,
    ));

    assert!(app.perform_undo(&mut cx));

    assert_eq!(
        app.documents.active_tab().unwrap().concept_id().unwrap(),
        "sales/order"
    );
    assert!(app
        .view_history
        .can_traverse(HistoryDirection::Back, |_| true));
    assert!(app.traverse_view_history(&mut cx, HistoryDirection::Back));
    assert_eq!(
        app.documents.active_tab().unwrap().concept_id().unwrap(),
        "sales/next",
        "Back after an Undo reveal returns to the editor that was active"
    );
    let statusbar = app.ui.widget(&cx, ids!(statusbar));
    let statusbar = statusbar
        .borrow::<crate::statusbar::Statusbar>()
        .expect("test statusbar is mounted");
    assert_eq!(
        crate::statusbar::history_feedback(&statusbar),
        (None, Some("Undid: Rename sales"))
    );
}

#[test]
fn global_history_chord_dispatches_before_the_widget_tree_and_consumes_empty_stack() {
    let (mut cx, mut app) = navigation_app_with_active_order();
    app.session
        .apply_edit(crate::editor_session::EditRequest {
            before_location: ViewLocation {
                document: crate::navigation::DocumentLocator::concept(
                    "sales/order",
                    waml::view::surface::SurfaceId::markdown(),
                ),
                anchor: ViewAnchor::None,
            },
            intent: crate::document::EditIntent {
                edit: waml::edit::PendingEdit::new(waml::okf::Batch(vec![
                    waml::okf::Op::IndexRetitle {
                        directory: waml::okf::DirectoryAddress::parse("/sales").unwrap(),
                        title: "Commerce".into(),
                    },
                ])),
                label: "Rename sales".into(),
                merge_key: None,
                after_location: None,
            },
        })
        .unwrap();
    let undo = Event::KeyDown(KeyEvent {
        key_code: KeyCode::KeyZ,
        modifiers: KeyModifiers {
            control: true,
            ..Default::default()
        },
        ..Default::default()
    });

    app.handle_event(&mut cx, &undo);
    {
        let statusbar = app.ui.widget(&cx, ids!(statusbar));
        let statusbar = statusbar
            .borrow::<crate::statusbar::Statusbar>()
            .expect("test statusbar is mounted");
        assert_eq!(
            crate::statusbar::history_feedback(&statusbar),
            (None, Some("Undid: Rename sales"))
        );
    }

    app.handle_event(&mut cx, &undo);
    let statusbar = app.ui.widget(&cx, ids!(statusbar));
    let statusbar = statusbar
        .borrow::<crate::statusbar::Statusbar>()
        .expect("test statusbar is mounted");
    assert_eq!(
        crate::statusbar::history_feedback(&statusbar),
        (Some("Nothing to undo"), None)
    );
}

/// The panel keys rows on `tree::key_string(RowId)`. Pushing the tab's raw
/// `concept_id` at it matches no row, so the active-tab highlight silently
/// stops tracking the active document.
#[test]
fn activating_a_document_highlights_its_row_by_the_panels_own_key() {
    let (cx, app) = navigation_app_with_active_order();
    assert_eq!(
        project_tree_selected_key(&cx, &app).as_deref(),
        Some(tree_key("/sales/order").as_str()),
    );
}

/// Opening a folder bypasses `transition_to_location`, so the shell sync that
/// drives the tree highlight has to be invoked by the `Directory` arm itself.
/// Without it the tree keeps the previously active document's row lit while a
/// folder tab is on screen.
#[test]
fn opening_a_folder_highlights_its_row() {
    let (mut cx, mut app) = navigation_app_with_active_order();
    let mut browser = FakeBrowser::default();

    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Directory {
            address: "/sales".into(),
        },
        OpenDisposition::Preview,
        &mut browser,
    ));

    assert_eq!(
        project_tree_selected_key(&cx, &app).as_deref(),
        Some(tree_key("/sales").as_str())
    );
}

#[test]
fn breadcrumb_reveal_pins_tree_without_navigation() {
    let (mut cx, mut app) = navigation_app_with_active_order();
    app.apply_dock_states(&mut cx, DockState::Flag, DockState::Pinned);
    let active = app.documents.active_id();
    let history_len = app.view_history.len();
    let uid = app.ui.widget(&cx, ids!(document_header)).widget_uid();
    let selected = project_tree_selected_key(&cx, &app);

    app.handle_action_batch(
        &mut cx,
        &[widget_action(
            uid,
            crate::document_header::DocumentHeaderAction::RevealInTree(
                NavigationTarget::Directory {
                    address: "/sales".into(),
                },
            ),
        )],
    );

    assert_eq!(app.documents.active_id(), active);
    assert_eq!(app.view_history.len(), history_len);
    assert_eq!(mounted_project_tree_state(&cx, &app), DockState::Pinned);
    // A reveal pulses and scrolls; the selection keeps tracking the active tab.
    assert_eq!(project_tree_selected_key(&cx, &app), selected);
    assert_eq!(
        app.dock_states(&mut cx),
        (DockState::Pinned, DockState::Pinned)
    );
}

#[test]
fn breadcrumb_reveal_in_narrow_mode_closes_inspector() {
    let (mut cx, mut app) = navigation_app_with_active_order();
    app.dock.force_narrow(true);
    app.apply_dock_states(&mut cx, DockState::Flag, DockState::Pinned);
    let active = app.documents.active_id();
    let history_len = app.view_history.len();
    let uid = app.ui.widget(&cx, ids!(document_header)).widget_uid();

    app.handle_action_batch(
        &mut cx,
        &[widget_action(
            uid,
            crate::document_header::DocumentHeaderAction::RevealInTree(
                NavigationTarget::Directory {
                    address: "/sales".into(),
                },
            ),
        )],
    );

    assert_eq!(app.documents.active_id(), active);
    assert_eq!(app.view_history.len(), history_len);
    assert_eq!(
        app.dock_states(&mut cx),
        (DockState::Pinned, DockState::Flag)
    );
}

#[test]
fn breadcrumb_reveal_rejects_unknown_target_without_changes() {
    let (mut cx, mut app) = navigation_app_with_active_order();
    app.apply_dock_states(&mut cx, DockState::Flag, DockState::Pinned);
    let active = app.documents.active_id();
    let history_len = app.view_history.len();
    let selected = project_tree_selected_key(&cx, &app);
    let uid = app.ui.widget(&cx, ids!(document_header)).widget_uid();

    app.handle_action_batch(
        &mut cx,
        &[widget_action(
            uid,
            crate::document_header::DocumentHeaderAction::RevealInTree(
                NavigationTarget::Document {
                    concept_id: "/missing".into(),
                    surface: None,
                    fragment: None,
                },
            ),
        )],
    );

    assert_eq!(app.documents.active_id(), active);
    assert_eq!(app.view_history.len(), history_len);
    assert_eq!(project_tree_selected_key(&cx, &app), selected);
    assert_eq!(
        app.dock_states(&mut cx),
        (DockState::Flag, DockState::Pinned)
    );
}

#[test]
#[ignore = "legacy Makepad Markdown ingress replaced by typed MarkdownEditor integration"]
fn navigation_document_ingresses_from_tree_and_markdown_share_preview_command() {
    let target = NavigationTarget::Document {
        concept_id: "sales/customer".into(),
        surface: None,
        fragment: None,
    };
    let tree_intent = NavigationIntent::Resolved {
        target: target.clone(),
        disposition: OpenDisposition::Preview,
    };
    let markdown_resolved_intent = {
        let (_cx, fixture_app) = navigation_app();
        NavigationIntent::Resolved {
            target: crate::navigation::resolve_link(
                fixture_app.session.okf(),
                "sales/order",
                "./customer.md",
            )
            .expect("relative customer link resolves"),
            disposition: OpenDisposition::Preview,
        }
    };

    assert_eq!(
        resolved_target(&tree_intent),
        resolved_target(&markdown_resolved_intent)
    );

    enum Ingress {
        Tree,
        Markdown,
    }
    for ingress in [Ingress::Tree, Ingress::Markdown] {
        let (mut cx, mut app) = navigation_app_with_active_order();
        let order_id = app.documents.active_id();
        let action = match ingress {
            Ingress::Tree => widget_action(
                app.ui.widget(&cx, ids!(project_tree)).widget_uid(),
                crate::tree_panel::ProjectTreeAction::Navigate(tree_intent.clone()),
            ),
            Ingress::Markdown => widget_action(
                app.ui.widget(&cx, ids!(markdown_surface.md)).widget_uid(),
                MarkdownAction::LinkNavigated("./customer.md".into()),
            ),
        };

        app.handle_action_batch(&mut cx, &[action]);
        assert_ne!(app.documents.active_id(), order_id);
        assert_eq!(
            app.documents.active_tab().and_then(|tab| tab.concept_id()),
            Some("sales/customer")
        );
        assert_eq!(app.documents.tabs().len(), 1);
        assert!(
            app.documents.tabs()[0].preview,
            "all ordinary navigation ingresses must use preview disposition"
        );
        assert_eq!(app.ui.widget(&cx, ids!(markdown_surface.md)).text(), "");
        assert!(
            app.ui
                .widget(&cx, ids!(markdown_surface.plain_source))
                .text()
                .contains("# Customer"),
            "each ingress must update the mounted plain source body"
        );
        assert_eq!(app.view_history.len(), 2);
        assert!(app.traverse_view_history(&mut cx, HistoryDirection::Back));
        assert_eq!(
            app.documents.active_tab().unwrap().concept_id().unwrap(),
            "sales/order"
        );
    }

    let (mut cx, mut app) = navigation_app_with_active_order();
    let persistent_tree = NavigationIntent::Resolved {
        target,
        disposition: OpenDisposition::Persistent,
    };
    let project_tree_uid = app.ui.widget(&cx, ids!(project_tree)).widget_uid();
    let persistent_action = || {
        widget_action(
            project_tree_uid,
            crate::tree_panel::ProjectTreeAction::Navigate(persistent_tree.clone()),
        )
    };
    app.handle_action_batch(&mut cx, &[persistent_action()]);
    assert_eq!(app.documents.tabs().len(), 1);
    assert!(!app.documents.tabs()[0].preview);
    let history_after_first = app.view_history.len();
    app.handle_action_batch(&mut cx, &[persistent_action()]);
    assert_eq!(app.documents.tabs().len(), 1);
    assert!(!app.documents.tabs()[0].preview);
    assert_eq!(app.view_history.len(), history_after_first);
}

#[test]
fn navigation_markdown_failures_preserve_document_and_report_exact_status() {
    let (mut cx, mut app) = navigation_app();
    let mut browser = FakeBrowser::default();
    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Document {
            concept_id: "sales/order".into(),
            surface: None,
            fragment: None,
        },
        OpenDisposition::Persistent,
        &mut browser,
    ));
    let active = app.documents.active_id();
    let cases = [
        ("http://", "Invalid link: http://"),
        ("mailto:a@example.com", "Unsupported link scheme: mailto"),
        ("../../../escape.md", "Link leaves this bundle"),
        ("./missing.md", "Document not found: sales/missing"),
    ];

    for (href, expected) in cases {
        assert!(!app.handle_navigation_intent(
            &mut cx,
            NavigationIntent::MarkdownLink {
                current_concept_id: "sales/order".into(),
                href: href.into(),
            },
        ));
        assert_eq!(app.documents.active_id(), active);
        let statusbar = app.ui.widget(&cx, ids!(statusbar));
        let statusbar = statusbar
            .borrow::<crate::statusbar::Statusbar>()
            .expect("test statusbar is mounted");
        assert_eq!(
            crate::statusbar::navigation_message(&statusbar),
            Some(expected),
            "{href}"
        );
    }
}

#[test]
fn navigation_root_opens_the_folder_view_without_resetting_navigation_or_docks() {
    // `Directory` navigation used to toggle the tree row's fold state; that
    // moved to the tree's own chevron hit-test (`tree_panel.rs`), and a
    // `Directory` target now always opens the folder's own view -- verified
    // here by tab identity, matching the folder surface's tab namespace
    // (distinct from every concept tab).
    let (mut cx, mut app) = navigation_app();
    let mut browser = FakeBrowser::default();
    app.dock.force_narrow(true);
    app.nav_state = NavState {
        scope: "/sales".into(),
    };
    app.ui
        .widget(&cx, ids!(project_tree))
        .borrow_mut::<crate::tree_panel::ProjectTree>()
        .expect("test project tree is mounted")
        .close_dock(&mut cx);
    app.ui
        .widget(&cx, ids!(inspector))
        .borrow_mut::<crate::inspector_panel::Inspector>()
        .expect("test inspector is mounted")
        .open_dock(&mut cx);
    let expected_nav = app.nav_state.clone();
    // The folder view has no right-dock content (`FolderView::chrome`), so
    // opening it closes the inspector the same way any other right-dock-less
    // view does (`BodyWidgets::apply_chrome`) -- the tree dock, which this
    // test closed explicitly and which the navigation never touches, stays
    // put.
    let expected_tree_dock = app.dock_states(&mut cx).0;
    let folder_tab = crate::documents::tab_id_for(&crate::navigation::DocumentLocator::folder("/"));

    for _ in 0..2 {
        assert!(app.navigate_with(
            &mut cx,
            NavigationTarget::Directory {
                address: "/".into(),
            },
            OpenDisposition::Preview,
            &mut browser,
        ));
        assert_eq!(app.documents.active_id(), folder_tab);
        assert_eq!(app.nav_state, expected_nav);
        assert_eq!(
            app.dock_states(&mut cx),
            (expected_tree_dock, crate::dock::DockState::Flag)
        );
    }
}

#[test]
#[ignore = "legacy Makepad Markdown ingress replaced by typed MarkdownEditor integration"]
fn navigation_directory_intents_from_tree_and_markdown_share_one_toggle_path() {
    enum Ingress {
        Tree,
        Markdown,
    }

    for ingress in [Ingress::Tree, Ingress::Markdown] {
        let (mut cx, mut app) = navigation_app();
        mount_markdown_surface(&mut cx, &mut app);
        let mut browser = FakeBrowser::default();
        assert!(app.navigate_with(
            &mut cx,
            NavigationTarget::Document {
                concept_id: "sales/order".into(),
                surface: None,
                fragment: None,
            },
            OpenDisposition::Persistent,
            &mut browser,
        ));
        app.nav_state.scope = "/sales".into();
        let active = app.documents.active_id();
        let markdown = app.ui.widget(&cx, ids!(markdown_surface.md));
        assert!(
            markdown.borrow::<Markdown>().is_some(),
            "Markdown ingress must originate from the mounted renderer"
        );
        assert_eq!(markdown.text(), "");
        assert!(
            app.ui
                .widget(&cx, ids!(markdown_surface.plain_source))
                .text()
                .contains("# Order"),
            "the plain source surface must belong to the active document"
        );
        let markdown_uid = markdown.widget_uid();
        assert!(
            project_tree_folder_is_open(&mut cx, &app, &tree_key("/sales")),
            "the fresh Browse tree starts with its top-level folder open"
        );
        let action = match ingress {
            Ingress::Tree => {
                let uid = app.ui.widget(&cx, ids!(project_tree)).widget_uid();
                widget_action(
                    uid,
                    crate::tree_panel::ProjectTreeAction::Navigate(NavigationIntent::Resolved {
                        target: NavigationTarget::Directory {
                            address: "/sales".into(),
                        },
                        disposition: OpenDisposition::Preview,
                    }),
                )
            }
            Ingress::Markdown => {
                widget_action(markdown_uid, MarkdownAction::LinkNavigated("./".into()))
            }
        };
        let actions: ActionsBuf = vec![action];
        app.handle_action_batch(&mut cx, &actions);

        assert!(
            !project_tree_folder_is_open(&mut cx, &app, &tree_key("/sales")),
            "each ingress must close the initially-open folder exactly once"
        );
        assert_eq!(app.documents.active_id(), active);
        assert_eq!(app.nav_state.scope, "/sales");
    }
}

#[test]
#[ignore = "legacy Makepad Markdown renderer assertions replaced by MarkdownEditor integration"]
fn navigation_draw_hook_scrolls_recorded_fragment_after_target_draw() {
    let (mut cx, mut app) = navigation_app();
    mount_markdown_surface(&mut cx, &mut app);
    let mut browser = FakeBrowser::default();

    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Document {
            concept_id: "sales/customer".into(),
            surface: None,
            fragment: Some("history".into()),
        },
        OpenDisposition::Preview,
        &mut browser,
    ));
    assert_eq!(app.ui.widget(&cx, ids!(markdown_surface.md)).text(), "");
    assert!(
        app.ui
            .widget(&cx, ids!(markdown_surface.plain_source))
            .text()
            .contains("## History"),
        "the active document must reach the plain source surface before draw"
    );
    assert_eq!(
        app.pending_fragment,
        Some(PendingFragment {
            concept_id: "sales/customer".into(),
            fragment: "history".into(),
        })
    );
    record_markdown_anchors(&mut cx, &app);

    AppMain::handle_event(
        &mut app,
        &mut cx,
        &Event::Draw(DrawEvent {
            redraw_all: true,
            ..DrawEvent::default()
        }),
    );

    assert!(
        !app.ui
            .widget(&cx, ids!(markdown_surface.md))
            .area()
            .is_empty(),
        "the real renderer draw must record a mounted area"
    );
    assert_eq!(app.pending_fragment, None);
    let statusbar = app.ui.widget(&cx, ids!(statusbar));
    let statusbar = statusbar
        .borrow::<crate::statusbar::Statusbar>()
        .expect("test statusbar is mounted");
    assert_eq!(
        crate::statusbar::navigation_message(&statusbar),
        Some("Section not found: history")
    );
}

#[test]
fn navigation_draw_hook_keeps_mismatch_then_reports_missing_once() {
    let (mut cx, mut app) = navigation_app();
    mount_markdown_surface(&mut cx, &mut app);
    let mut browser = FakeBrowser::default();
    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Document {
            concept_id: "sales/order".into(),
            surface: None,
            fragment: None,
        },
        OpenDisposition::Preview,
        &mut browser,
    ));
    app.pending_fragment = Some(PendingFragment {
        concept_id: "sales/customer".into(),
        fragment: "missing".into(),
    });
    record_markdown_anchors(&mut cx, &app);

    AppMain::handle_event(
        &mut app,
        &mut cx,
        &Event::Draw(DrawEvent {
            redraw_all: true,
            ..DrawEvent::default()
        }),
    );

    assert_eq!(
        app.pending_fragment,
        Some(PendingFragment {
            concept_id: "sales/customer".into(),
            fragment: "missing".into(),
        })
    );

    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Document {
            concept_id: "sales/customer".into(),
            surface: None,
            fragment: Some("missing".into()),
        },
        OpenDisposition::Preview,
        &mut browser,
    ));
    record_markdown_anchors(&mut cx, &app);
    AppMain::handle_event(
        &mut app,
        &mut cx,
        &Event::Draw(DrawEvent {
            redraw_all: true,
            ..DrawEvent::default()
        }),
    );
    assert_eq!(app.pending_fragment, None);
    let statusbar = app.ui.widget(&cx, ids!(statusbar));
    let mut statusbar = statusbar
        .borrow_mut::<crate::statusbar::Statusbar>()
        .expect("test statusbar is mounted");
    assert_eq!(
        crate::statusbar::navigation_message(&statusbar),
        Some("Section not found: missing")
    );
    statusbar.set_navigation_message(&mut cx, None);
    drop(statusbar);

    AppMain::handle_event(
        &mut app,
        &mut cx,
        &Event::Draw(DrawEvent {
            redraw_all: true,
            ..DrawEvent::default()
        }),
    );
    let statusbar = app.ui.widget(&cx, ids!(statusbar));
    let statusbar = statusbar
        .borrow::<crate::statusbar::Statusbar>()
        .expect("test statusbar is mounted");
    assert_eq!(crate::statusbar::navigation_message(&statusbar), None);
}

#[test]
fn non_markdown_active_view_rejects_hidden_stale_fragment_once() {
    struct NonMarkdownView;

    impl DocView for NonMarkdownView {
        fn identity(&self) -> DocViewIdentity {
            DocViewIdentity::StructuralDiagram(crate::StructuralVisualKind::Class)
        }

        fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, _data: ViewData<'_>) {
            body.show_canvas(cx);
        }

        fn handle(
            &mut self,
            _cx: &mut Cx,
            _body: &BodyWidgets,
            _actions: &Actions,
            _data: ViewData<'_>,
        ) -> crate::doc_view::ViewOutcome {
            crate::doc_view::ViewOutcome::default()
        }

        fn chrome(&self) -> crate::doc_view::BodyChrome {
            crate::doc_view::BodyChrome::HIDDEN
        }
    }

    let (mut cx, mut app) = navigation_app();
    mount_markdown_surface(&mut cx, &mut app);
    record_markdown_anchors(&mut cx, &app);

    let tab_id = LiveId::from_str("diagram");
    app.documents.transition(
        &mut cx,
        &app.ui,
        &app.session,
        DocumentCommand::Open {
            document: OpenDocument {
                tab_id,
                locator: crate::view_history::DocumentLocator::concept(
                    "diagram",
                    waml::view::surface::SurfaceId::canvas(),
                ),
                title: "Diagram".into(),
                presentation: DocumentPresentation {
                    icon: Icon::Workflow,
                    accent: None,
                    category: NavCategory::Diagram,
                },
                view: Box::new(NonMarkdownView),
            },
            persistent: true,
        },
    );
    let active_before = app.documents.active_id();
    app.pending_fragment = Some(PendingFragment {
        concept_id: "diagram".into(),
        fragment: "details".into(),
    });

    app.apply_pending_fragment(&mut cx);

    assert_eq!(app.pending_fragment, None);
    assert_eq!(app.documents.active_id(), active_before);
    let statusbar = app.ui.widget(&cx, ids!(statusbar));
    let mut statusbar = statusbar
        .borrow_mut::<crate::statusbar::Statusbar>()
        .expect("test statusbar is mounted");
    assert_eq!(
        crate::statusbar::navigation_message(&statusbar),
        Some("Section not found: details")
    );
    statusbar.set_navigation_message(&mut cx, None);
    drop(statusbar);

    app.apply_pending_fragment(&mut cx);

    let statusbar = app.ui.widget(&cx, ids!(statusbar));
    let statusbar = statusbar
        .borrow::<crate::statusbar::Statusbar>()
        .expect("test statusbar is mounted");
    assert_eq!(crate::statusbar::navigation_message(&statusbar), None);
    assert_eq!(app.documents.active_id(), active_before);
}

#[test]
#[ignore = "legacy Makepad Markdown renderer assertions replaced by MarkdownEditor integration"]
fn navigation_source_and_generic_views_activate_and_scroll_real_renderer() {
    #[derive(Clone, Copy)]
    enum ViewKind {
        Source,
        Generic,
    }

    for view_kind in [ViewKind::Source, ViewKind::Generic] {
        for (fragment, expected_status) in [
            ("details", Some("Section not found: details")),
            ("missing", Some("Section not found: missing")),
        ] {
            let (mut cx, mut app) = navigation_app();
            mount_markdown_surface(&mut cx, &mut app);
            let markdown = app.ui.widget(&cx, ids!(markdown_surface.md));
            let markdown_uid = markdown.widget_uid();
            let intent = {
                let body = BodyWidgets::new(&mut cx, &app.ui);
                let mut view: Box<dyn DocView> = match view_kind {
                    ViewKind::Source => {
                        Box::new(crate::source_view::SourceView::new("sales/order".into()))
                    }
                    ViewKind::Generic => Box::new(crate::generic_okf_view::GenericOkfView::new(
                        "sales/order".into(),
                    )),
                };
                let data = ViewData {
                    source: app.session.source(),
                    okf_analysis: app.session.okf_analysis(),
                    uml_analysis: app.session.uml_analysis(),
                    revision: app.session.revision(),
                };
                view.sync(&mut cx, &body, data);
                assert_eq!(markdown.text(), "");
                assert!(
                    app.ui
                        .widget(&cx, ids!(markdown_surface.plain_source))
                        .text()
                        .contains("# Order"),
                    "each view must populate the mounted plain source surface"
                );
                let href = format!("./next.md#{fragment}");
                let actions: ActionsBuf = vec![widget_action(
                    markdown_uid,
                    MarkdownAction::LinkNavigated(href.clone()),
                )];
                let outcome = view.handle(&mut cx, &body, &actions, data);
                assert_eq!(
                    outcome.navigation,
                    Some(NavigationIntent::MarkdownLink {
                        current_concept_id: "sales/order".into(),
                        href,
                    })
                );
                outcome.navigation.expect("view emits navigation")
            };

            assert!(app.handle_navigation_intent(&mut cx, intent));
            assert_eq!(
                app.documents.active_tab().and_then(|tab| tab.concept_id()),
                Some("sales/next")
            );
            assert_eq!(
                app.pending_fragment,
                Some(PendingFragment {
                    concept_id: "sales/next".into(),
                    fragment: fragment.into(),
                })
            );

            record_markdown_anchors(&mut cx, &app);
            AppMain::handle_event(
                &mut app,
                &mut cx,
                &Event::Draw(DrawEvent {
                    redraw_all: true,
                    ..DrawEvent::default()
                }),
            );

            assert_eq!(app.pending_fragment, None);
            assert_eq!(
                app.documents.active_tab().and_then(|tab| tab.concept_id()),
                Some("sales/next"),
                "missing anchors must preserve the newly activated target"
            );
            let statusbar = app.ui.widget(&cx, ids!(statusbar));
            let statusbar = statusbar
                .borrow::<crate::statusbar::Statusbar>()
                .expect("test statusbar is mounted");
            assert_eq!(
                crate::statusbar::navigation_message(&statusbar),
                expected_status,
                "{fragment}"
            );
        }
    }
}

#[test]
fn document_header_projection_keeps_icon_when_breadcrumb_is_missing() {
    let chrome = DocumentHeaderChrome {
        breadcrumb: true,
        right_dock: Some(Icon::PanelRight),
        emphasis_toggle: None,
        view_toggle: None,
        zoom: None,
    };
    let (segments, icon, view_toggle) = project_document_header(chrome, None);

    assert!(segments.is_empty());
    assert_eq!(icon, Some(Icon::PanelRight));
    assert_eq!(view_toggle, None);
}

#[test]
fn document_header_projection_obeys_breadcrumb_flag_and_hidden_chrome() {
    let segment = BreadcrumbSegment {
        title: "Order".into(),
        target: NavigationTarget::Document {
            concept_id: "sales/order".into(),
            surface: None,
            fragment: None,
        },
    };
    let icon_only = DocumentHeaderChrome {
        breadcrumb: false,
        right_dock: Some(Icon::PanelRight),
        emphasis_toggle: None,
        view_toggle: None,
        zoom: None,
    };
    assert_eq!(
        project_document_header(icon_only, Some(vec![segment.clone()])),
        (Vec::new(), Some(Icon::PanelRight), None)
    );

    let breadcrumb = DocumentHeaderChrome {
        breadcrumb: true,
        right_dock: None,
        emphasis_toggle: None,
        view_toggle: None,
        zoom: None,
    };
    assert_eq!(
        project_document_header(breadcrumb, Some(vec![segment.clone()])),
        (vec![segment], None, None)
    );
    assert_eq!(
        project_document_header(DocumentHeaderChrome::default(), None),
        (Vec::new(), None, None)
    );
}

fn assert_mounted_header(
    cx: &Cx,
    app: &App,
    expected_titles: &[&str],
    expected_icon: Option<Icon>,
    expected_height: f64,
) {
    let header = app.ui.widget(cx, ids!(document_header));
    let header = header
        .borrow::<crate::document_header::DocumentHeader>()
        .expect("test document header is mounted");
    assert_eq!(
        header
            .test_segments()
            .iter()
            .map(|segment| segment.title.as_str())
            .collect::<Vec<_>>(),
        expected_titles
    );
    assert_eq!(header.test_right_dock(), expected_icon);
    assert_eq!(header.visible_height(), expected_height);
}

fn mounted_inspector_state(cx: &Cx, app: &App) -> DockState {
    app.ui
        .widget(cx, ids!(inspector))
        .borrow::<crate::inspector_panel::Inspector>()
        .expect("test inspector is mounted")
        .dock_state()
}

#[test]
fn document_header_source_generic_start_source_sequence_has_no_stale_state() {
    let (mut cx, mut app) = navigation_app();
    let source = crate::okf_documents::open_source(app.session.okf_analysis(), "sales/order")
        .expect("source document exists");
    app.documents.transition(
        &mut cx,
        &app.ui,
        &app.session,
        DocumentCommand::Open {
            document: source,
            persistent: false,
        },
    );
    app.sync_document_shell(&mut cx);
    assert_mounted_header(
        &cx,
        &app,
        &["Root", "Sales", "Order"],
        Some(Icon::PanelRight),
        crate::document_header::DOCUMENT_HEADER_H,
    );

    // The minimal harness has no mounted Window bounds, so keep responsive
    // mode explicitly narrow instead of letting a zero-width query perform
    // the initial wide-to-narrow reconciliation during the style check.
    app.dock.force_narrow(true);
    draw_document_header(&mut cx, &app, dvec2(480.0, 30.0));
    let right_button_uid = app
        .ui
        .widget(&cx, ids!(document_header.right_button))
        .widget_uid();
    let action = widget_action(
        right_button_uid,
        crate::icon_button::IconButtonAction::Clicked,
    );
    {
        let header = app.ui.widget(&cx, ids!(document_header));
        let header = header
            .borrow::<crate::document_header::DocumentHeader>()
            .expect("test document header is mounted");
        assert_eq!(
            header.action(&cx, std::slice::from_ref(&action)),
            Some(crate::document_header::DocumentHeaderAction::ToggleRightDock)
        );
    }
    app.handle_action_batch(&mut cx, &[action]);
    assert_eq!(mounted_inspector_state(&cx, &app), DockState::Pinned);
    app.sync_dock_slots(&mut cx);

    let generic = crate::okf_documents::open(app.session.okf_analysis(), "sales/order")
        .expect("generic document exists");
    app.documents.transition(
        &mut cx,
        &app.ui,
        &app.session,
        DocumentCommand::Open {
            document: generic,
            persistent: false,
        },
    );
    app.sync_document_shell(&mut cx);
    assert_mounted_header(
        &cx,
        &app,
        &["Root", "Sales", "Order"],
        None,
        crate::document_header::DOCUMENT_HEADER_H,
    );
    assert_eq!(mounted_inspector_state(&cx, &app), DockState::Flag);
    app.sync_dock_slots(&mut cx);

    app.show_start_screen(&mut cx);
    assert_mounted_header(&cx, &app, &[], None, 0.0);
    assert_eq!(mounted_inspector_state(&cx, &app), DockState::Flag);
    app.sync_dock_slots(&mut cx);

    let source = crate::okf_documents::open_source(app.session.okf_analysis(), "sales/order")
        .expect("source document still exists");
    app.documents.transition(
        &mut cx,
        &app.ui,
        &app.session,
        DocumentCommand::Open {
            document: source,
            persistent: false,
        },
    );
    app.sync_document_shell(&mut cx);
    assert_mounted_header(
        &cx,
        &app,
        &["Root", "Sales", "Order"],
        Some(Icon::PanelRight),
        crate::document_header::DOCUMENT_HEADER_H,
    );
    assert_eq!(mounted_inspector_state(&cx, &app), DockState::Flag);
    app.sync_dock_slots(&mut cx);
}

#[test]
fn zoom_command_with_no_document_is_not_consumed() {
    // Fresh shell, nothing open: the diagram-focused guarantee -- with no
    // zoomable chrome active, `apply_zoom_command` must fall through so the
    // chord reaches whatever the platform default would otherwise be.
    let (mut cx, mut app) = mounted_production_shell();
    assert!(!app.apply_zoom_command(&mut cx, crate::shortcuts::ZoomCommand::In));
}

#[test]
fn zoom_command_steps_the_active_view_and_pushes_the_header() {
    let (mut cx, mut app) = navigation_app();
    let generic = crate::okf_documents::open(app.session.okf_analysis(), "sales/order")
        .expect("generic document exists");
    app.documents.transition(
        &mut cx,
        &app.ui,
        &app.session,
        DocumentCommand::Open {
            document: generic,
            persistent: false,
        },
    );
    app.sync_document_shell(&mut cx);

    // Pin the starting rung so the assertion below is independent of
    // whatever this machine's real `~/.waml/editor.json` currently holds,
    // and restore it afterwards -- `crate::config::reading_zoom`/
    // `set_reading_zoom` are the real (non-test-injectable) disk seam, not
    // the `load_from`/`store_to` temp-dir seam `config::tests` uses.
    let original = crate::config::reading_zoom();
    crate::config::set_reading_zoom(crate::zoom::ZOOM_DEFAULT);

    let consumed = app.apply_zoom_command(&mut cx, crate::shortcuts::ZoomCommand::In);

    let expected = crate::zoom::zoom_in(crate::zoom::ZOOM_DEFAULT);
    let restore = || crate::config::set_reading_zoom(original);
    assert!(consumed, "a Reading-zoom chrome must consume the command");
    if crate::config::reading_zoom() != expected {
        restore();
        panic!(
            "expected reading zoom {expected}, got {}",
            crate::config::reading_zoom()
        );
    }
    let header = app.ui.widget(&cx, ids!(document_header));
    let pushed = header
        .borrow::<crate::document_header::DocumentHeader>()
        .expect("test document header is mounted")
        .test_zoom();
    restore();
    assert_eq!(pushed, Some(expected));
}

#[test]
fn a_second_rapid_back_traversal_does_not_corrupt_the_intermediate_history_entry() {
    let (mut cx, mut app) = navigation_app_with_active_order();
    for concept_id in ["sales/customer", "sales/next"] {
        assert!(app.transition_to_location(
            &mut cx,
            ViewLocation {
                document: crate::navigation::DocumentLocator::concept(
                    concept_id,
                    waml::view::surface::SurfaceId::markdown()
                ),
                anchor: ViewAnchor::None,
            },
            TransitionCause::UserNavigation,
        ));
    }
    assert_eq!(app.view_history.len(), 3);

    // First Back: sales/next -> sales/customer. This schedules a deferred
    // anchor restore for sales/customer and leaves entry[1] (sales/customer)
    // as-is.
    assert!(app.traverse_view_history(&mut cx, HistoryDirection::Back));
    assert_eq!(
        app.documents.active_tab().unwrap().concept_id().unwrap(),
        "sales/customer"
    );
    let pending_after_first = app
        .pending_anchor_restore
        .clone()
        .expect("HistoryTraversal with a non-None anchor schedules a deferred restore");
    assert_eq!(
        pending_after_first.document.concept_id(),
        Some("sales/customer")
    );
    let entry_after_first = app
        .view_history
        .entry_at(1)
        .cloned()
        .expect("sales/customer entry exists at index 1");

    // Second Back, with no intervening Draw to apply the first restore:
    // sales/customer -> sales/order. Departing sales/customer must NOT be
    // refreshed into history from its pre-restore-stale capture, because a
    // restore for sales/customer is still pending.
    assert!(app.traverse_view_history(&mut cx, HistoryDirection::Back));
    assert_eq!(
        app.documents.active_tab().unwrap().concept_id().unwrap(),
        "sales/order"
    );
    assert_eq!(
        app.view_history.entry_at(1),
        Some(&entry_after_first),
        "the intermediate sales/customer entry must be untouched by the second traversal"
    );

    // The second traversal supersedes the first pending restore with a new
    // one (for sales/order) at a newer generation.
    let pending_after_second = app
        .pending_anchor_restore
        .clone()
        .expect("second HistoryTraversal schedules its own deferred restore");
    assert_eq!(
        pending_after_second.document.concept_id(),
        Some("sales/order")
    );
    assert!(pending_after_second.generation > pending_after_first.generation);
    assert_eq!(
        pending_after_second.generation,
        app.anchor_restore_generation
    );
}

#[test]
fn pumping_a_draw_applies_the_latest_pending_restore_and_refreshes_its_entry() {
    let (mut cx, mut app) = navigation_app_with_active_order();
    for concept_id in ["sales/customer", "sales/next"] {
        assert!(app.transition_to_location(
            &mut cx,
            ViewLocation {
                document: crate::navigation::DocumentLocator::concept(
                    concept_id,
                    waml::view::surface::SurfaceId::markdown()
                ),
                anchor: ViewAnchor::None,
            },
            TransitionCause::UserNavigation,
        ));
    }

    assert!(app.traverse_view_history(&mut cx, HistoryDirection::Back));
    assert!(app.traverse_view_history(&mut cx, HistoryDirection::Back));
    assert_eq!(
        app.documents.active_tab().unwrap().concept_id().unwrap(),
        "sales/order"
    );
    let scheduled_generation = app
        .pending_anchor_restore
        .as_ref()
        .expect("a restore is pending after two rapid traversals")
        .generation;
    assert_eq!(scheduled_generation, app.anchor_restore_generation);

    // Simulate the deferred Draw that `app/event.rs` drives.
    app.apply_pending_anchor_restore(&mut cx);

    assert!(app.pending_anchor_restore.is_none());
    let refreshed = app
        .view_history
        .entry_at(0)
        .cloned()
        .expect("sales/order entry exists at index 0");
    assert_eq!(refreshed.document.concept_id(), Some("sales/order"));
}

/// A flip re-runs every open folder tab IN PLACE -- same tab, view
/// swapped -- and leaves concept tabs alone. Opening a second tab for the
/// same folder, or leaving the old view behind, are both the defect the
/// old "View raw" had.
#[test]
fn a_mode_flip_re_runs_open_folder_tabs_in_place() {
    let (mut cx, mut app) = navigation_app();
    let mut browser = FakeBrowser::default();

    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Directory {
            address: "/sales".into(),
        },
        OpenDisposition::Persistent,
        &mut browser,
    ));
    let tabs_before = app.documents.tabs().len();
    let tab_id = app.documents.active_id();

    let registry = crate::folder_projection::core_registry();
    let full_mask = waml::view::mask::ProjectionMask::from_names(
        crate::folder_projection::maskable_names(&registry)
            .into_iter()
            .flat_map(|(_owner, names)| names)
            .map(|name| name.to_string())
            .collect::<Vec<_>>(),
    );

    app.set_projection_mask(&mut cx, full_mask);

    assert!(!app.projection_mask.is_empty());
    assert_eq!(app.documents.tabs().len(), tabs_before, "no second tab");
    assert_eq!(
        app.documents.active_id(),
        tab_id,
        "the tab keeps its identity"
    );

    app.set_projection_mask(&mut cx, waml::view::mask::ProjectionMask::default());
    assert!(app.projection_mask.is_empty());
    assert_eq!(app.documents.tabs().len(), tabs_before);
}

/// The mask is a session fact, not a preference. Nothing writes it anywhere.
#[test]
fn the_mask_starts_empty_and_is_never_persisted() {
    let (mut cx, mut app) = navigation_app();
    assert!(app.projection_mask.is_empty());
    let registry = crate::folder_projection::core_registry();
    let full_mask = waml::view::mask::ProjectionMask::from_names(
        crate::folder_projection::maskable_names(&registry)
            .into_iter()
            .flat_map(|(_owner, names)| names)
            .map(|name| name.to_string())
            .collect::<Vec<_>>(),
    );
    app.set_projection_mask(&mut cx, full_mask);
    // The settings type has no field for it, by construction. This
    // assertion is the fence: adding one must break a test, not pass
    // silently.
    let settings = crate::project_settings::ProjectSettings::default();
    let json = serde_json::to_string(&settings).unwrap();
    // Assert on the names that EXIST today. `view_mode` is gone, so a fence
    // spelled that way could never fire again.
    assert!(
        !json.contains("projection_mask"),
        "the projection mask must not reach settings: {json}"
    );
    assert!(
        !json.contains("mask"),
        "no mask-shaped field at all: {json}"
    );
}

/// Task 8's fixture: `/shop` has an `index.md` (the forcing case resolves),
/// `/loose` links a concept but has no `index.md` on disk (the negative
/// case must not resolve).
fn navigation_app_with_folders() -> (Cx, App) {
    let mut cx = Cx::new(Box::new(|_, _| {}));
    cx.init_cx_os();
    cx.widget_tree_mark_dirty(WidgetUid(0));
    let mut app = cx.with_vm(App::script_new_with_default);
    let source = waml::source::SourceBundle::try_from_pairs([
        ("index.md", "# Root\n\n* [Shop](shop/)\n* [Loose](loose/)\n"),
        ("shop/index.md", "# Shop\n\n* [Thing](thing.md)\n"),
        (
            "shop/thing.md",
            "---\ntype: Runbook\ntitle: Thing\n---\n# Thing\n",
        ),
        (
            "loose/thing.md",
            "---\ntype: Runbook\ntitle: Loose Thing\n---\n# Loose Thing\n",
        ),
    ])
    .unwrap();
    app.session.replace(source).unwrap();
    let mut project_tree = cx.with_vm(crate::tree_panel::ProjectTree::script_new_with_default);
    project_tree.set_view(
        &mut cx,
        crate::nav::view(
            app.session.okf_analysis(),
            app.session.uml_analysis(),
            &NavState::default(),
            &app.projection_mask,
            app.chain_limits,
        ),
    );
    let project_tree = WidgetRef::new_with_inner(Box::new(project_tree));
    let statusbar = WidgetRef::new_with_inner(Box::new(
        cx.with_vm(crate::statusbar::Statusbar::script_new_with_default),
    ));
    let document_header = WidgetRef::new_with_inner(Box::new(
        cx.with_vm(crate::document_header::DocumentHeader::script_new_with_default),
    ));
    let inspector = WidgetRef::new_with_inner(Box::new(
        cx.with_vm(crate::inspector_panel::Inspector::script_new_with_default),
    ));
    let mut ui = cx.with_vm(View::script_new_with_default);
    ui.children.push((live_id!(project_tree), project_tree));
    ui.children.push((live_id!(statusbar), statusbar));
    ui.children
        .push((live_id!(document_header), document_header));
    ui.children.push((live_id!(inspector), inspector));
    for id in [live_id!(history_back_btn), live_id!(history_forward_btn)] {
        let button = WidgetRef::new_with_inner(Box::new(
            cx.with_vm(crate::icon_button::IconButton::script_new_with_default),
        ));
        ui.children.push((id, button));
    }
    app.ui = WidgetRef::new_with_inner(Box::new(ui));
    (cx, app)
}

/// The forcing case (T8 S1): open a folder tab, then request its source
/// surface via `open_source_for`. The active tab's locator becomes the
/// folder's source locator, the folder tab is still open, Back returns to
/// the folder listing, Forward returns to the source tab, and a second
/// `open_source_for` reuses the same tab rather than growing tab count.
#[test]
fn open_source_for_a_folder_forces_the_source_tab_and_round_trips_through_history() {
    let (mut cx, mut app) = navigation_app_with_folders();
    let mut browser = FakeBrowser::default();

    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Directory {
            address: "/shop".into(),
        },
        OpenDisposition::Persistent,
        &mut browser,
    ));
    let tabs_after_folder = app.documents.tabs().len();
    let folder_locator = crate::navigation::DocumentLocator::folder("/shop");
    assert_eq!(
        app.documents.active_tab().unwrap().locator(),
        folder_locator
    );

    app.open_source_for(&mut cx, waml::view::row::RowTarget::Folder("/shop".into()));

    let source_locator = crate::navigation::DocumentLocator::new(
        waml::view::row::RowTarget::Folder("/shop".into()),
        waml::view::surface::SurfaceId::source(),
    );
    assert_eq!(
        app.documents.active_tab().unwrap().locator(),
        source_locator,
        "the active tab must be the folder's source tab"
    );
    // The folder tab is either still open alongside the source tab, or was
    // replaced in the shared preview slot -- assert whichever
    // `transition_to_location`'s preview semantics actually produced.
    let tab_count_after_source = app.documents.tabs().len();
    assert!(
        tab_count_after_source == tabs_after_folder
            || tab_count_after_source == tabs_after_folder + 1,
        "unexpected tab count after opening the source: {tab_count_after_source}"
    );

    assert!(app.traverse_view_history(&mut cx, HistoryDirection::Back));
    assert_eq!(
        app.documents.active_tab().unwrap().locator(),
        folder_locator,
        "Back must return to the folder tab"
    );

    assert!(app.traverse_view_history(&mut cx, HistoryDirection::Forward));
    assert_eq!(
        app.documents.active_tab().unwrap().locator(),
        source_locator,
        "Forward must return to the source tab"
    );

    // A second `open_source_for` on the same folder reuses the same tab.
    let tabs_before_second_open = app.documents.tabs().len();
    app.open_source_for(&mut cx, waml::view::row::RowTarget::Folder("/shop".into()));
    assert_eq!(
        app.documents.tabs().len(),
        tabs_before_second_open,
        "a repeat open_source_for must not grow the tab count"
    );
    assert_eq!(
        app.documents.active_tab().unwrap().locator(),
        source_locator
    );
}

/// The root works too (T8 S2) -- `RowTarget::Folder("/")` resolves through
/// the "index" key edge the spike confirmed.
#[test]
fn open_source_for_the_root_folder_round_trips_through_history() {
    let (mut cx, mut app) = navigation_app_with_folders();
    let mut browser = FakeBrowser::default();

    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Directory {
            address: "/".into(),
        },
        OpenDisposition::Persistent,
        &mut browser,
    ));
    let folder_locator = crate::navigation::DocumentLocator::folder("/");
    assert_eq!(
        app.documents.active_tab().unwrap().locator(),
        folder_locator
    );

    app.open_source_for(&mut cx, waml::view::row::RowTarget::Folder("/".into()));

    let source_locator = crate::navigation::DocumentLocator::new(
        waml::view::row::RowTarget::Folder("/".into()),
        waml::view::surface::SurfaceId::source(),
    );
    assert_eq!(
        app.documents.active_tab().unwrap().locator(),
        source_locator
    );

    assert!(app.traverse_view_history(&mut cx, HistoryDirection::Back));
    assert_eq!(
        app.documents.active_tab().unwrap().locator(),
        folder_locator
    );

    assert!(app.traverse_view_history(&mut cx, HistoryDirection::Forward));
    assert_eq!(
        app.documents.active_tab().unwrap().locator(),
        source_locator
    );
}

/// The negative case (T8 S3): a folder linked but with no `index.md` on
/// disk offers nothing -- `open_source_for` leaves the active tab and tab
/// count unchanged, opens no blank tab, and records no history entry. This
/// is the app-level face of Task 3's gate test.
#[test]
fn open_source_for_a_folder_without_an_index_md_changes_nothing() {
    let (mut cx, mut app) = navigation_app_with_folders();
    let mut browser = FakeBrowser::default();

    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Directory {
            address: "/loose".into(),
        },
        OpenDisposition::Persistent,
        &mut browser,
    ));
    let folder_locator = crate::navigation::DocumentLocator::folder("/loose");
    let tabs_before = app.documents.tabs().len();
    let active_before = app.documents.active_tab().unwrap().locator();
    assert_eq!(active_before, folder_locator);
    let history_len_before = app.view_history.len();

    app.open_source_for(&mut cx, waml::view::row::RowTarget::Folder("/loose".into()));

    assert_eq!(
        app.documents.tabs().len(),
        tabs_before,
        "no blank tab must be opened"
    );
    assert_eq!(
        app.documents.active_tab().unwrap().locator(),
        active_before,
        "the active tab must be unchanged"
    );
    assert_eq!(
        app.view_history.len(),
        history_len_before,
        "no history entry must be recorded"
    );
}

/// A mode flip must rebuild folder LISTING tabs only. A folder's `source`
/// tab shares the folder target but not the surface; counting it would
/// rebuild the listing twice and still never refresh the source tab.
#[test]
fn a_mode_flip_collects_folder_listing_tabs_only_not_their_source_tabs() {
    let (mut cx, mut app) = navigation_app_with_folders();
    let mut browser = FakeBrowser::default();

    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Directory {
            address: "/shop".into(),
        },
        OpenDisposition::Persistent,
        &mut browser,
    ));
    app.open_source_for(&mut cx, waml::view::row::RowTarget::Folder("/shop".into()));
    assert!(
        app.documents
            .tabs()
            .iter()
            .any(|tab| tab.locator.surface == waml::view::surface::SurfaceId::source()),
        "the folder's source tab must be open for this test to mean anything"
    );

    assert_eq!(
        app.open_directory_tab_addresses(),
        vec![(
            "/shop".to_string(),
            waml::view::surface::SurfaceId::folder()
        )],
        "the source tab must not be collected as a folder listing"
    );

    let registry = crate::folder_projection::core_registry();
    let full_mask = waml::view::mask::ProjectionMask::from_names(
        crate::folder_projection::maskable_names(&registry)
            .into_iter()
            .flat_map(|(_owner, names)| names)
            .map(|name| name.to_string())
            .collect::<Vec<_>>(),
    );
    app.set_projection_mask(&mut cx, full_mask);
    assert!(!app.projection_mask.is_empty());
}

/// A folder navigation that fails must not pin the unrelated preview tab
/// that happened to be active. The promote is a consequence of arriving.
#[test]
fn a_failed_folder_navigation_does_not_promote_the_active_preview_tab() {
    let (mut cx, mut app) = navigation_app_with_folders();
    let mut browser = FakeBrowser::default();

    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Directory {
            address: "/shop".into(),
        },
        OpenDisposition::Preview,
        &mut browser,
    ));
    let active = app.documents.active_id();
    assert!(
        app.documents
            .tabs()
            .iter()
            .find(|tab| tab.id == active)
            .expect("active tab")
            .preview,
        "the preview folder open must land in the preview slot"
    );

    assert!(!app.navigate_with(
        &mut cx,
        NavigationTarget::Directory {
            address: "/does-not-exist".into(),
        },
        OpenDisposition::Persistent,
        &mut browser,
    ));

    assert!(
        app.documents
            .tabs()
            .iter()
            .find(|tab| tab.id == active)
            .expect("active tab survives")
            .preview,
        "a failed folder navigation must not promote the preview tab"
    );
}

/// The two extra mouse buttons every pointing device with a thumb rest has
/// (XBUTTON1/2 on Windows, `BTN_SIDE`/`BTN_EXTRA` on Linux, X11 buttons 8/9)
/// drive the same view history as the chrome's back/forward pair.
fn mouse_button_press(button: MouseButton) -> Event {
    Event::MouseDown(MouseDownEvent {
        abs: Vec2d::default(),
        button,
        window_id: WindowId(0, 0),
        modifiers: KeyModifiers::default(),
        handled: Cell::new(Area::default()),
        time: 0.0,
    })
}

#[test]
fn mouse_back_and_forward_buttons_traverse_view_history() {
    let (mut cx, mut app) = navigation_app_with_active_order();
    assert!(app.transition_document(&mut cx, "sales/customer", false));
    assert_eq!(app.test_history_enabled(&mut cx), (true, false));

    app.handle_event(&mut cx, &mouse_button_press(MouseButton::BACK));
    assert_eq!(
        app.documents.active_tab().and_then(|tab| tab.concept_id()),
        Some("sales/order")
    );
    assert_eq!(app.test_history_enabled(&mut cx), (false, true));

    app.handle_event(&mut cx, &mouse_button_press(MouseButton::FORWARD));
    assert_eq!(
        app.documents.active_tab().and_then(|tab| tab.concept_id()),
        Some("sales/customer")
    );
}

#[test]
fn exhausted_mouse_back_button_reports_the_same_problem_as_the_chrome_button() {
    let (mut cx, mut app) = navigation_app_with_active_order();
    assert!(app.transition_document(&mut cx, "sales/customer", false));

    app.handle_event(&mut cx, &mouse_button_press(MouseButton::BACK));
    app.handle_event(&mut cx, &mouse_button_press(MouseButton::BACK));

    let statusbar = app.ui.widget(&cx, ids!(statusbar));
    let statusbar = statusbar
        .borrow::<crate::statusbar::Statusbar>()
        .expect("test statusbar is mounted");
    assert_eq!(
        crate::statusbar::history_feedback(&statusbar),
        (Some("No previous view"), None)
    );
}

fn rebuilt_search_app() -> (Cx, App) {
    let (cx, mut app) = navigation_app();
    let snapshot = app.session.snapshot();
    app.search.rebuild(
        &snapshot.source,
        &snapshot.okf_analysis,
        &snapshot.uml_analysis,
    );
    (cx, app)
}

#[test]
fn open_search_results_opens_a_titled_tab_and_reactivates_on_a_rerun() {
    let (mut cx, mut app) = rebuilt_search_app();

    app.open_search_results(&mut cx, "order");

    let tab_id = app.documents.active_id();
    assert_eq!(app.documents.active_tab().unwrap().title, "Search: order");

    // Switch away, then re-run the SAME query: it must re-activate the
    // existing tab, never open a second one (decision 7).
    assert!(app.transition_document(&mut cx, "sales/customer", false));
    assert_ne!(app.documents.active_id(), tab_id);

    app.open_search_results(&mut cx, "order");

    assert_eq!(app.documents.active_id(), tab_id);
    assert_eq!(
        app.documents
            .tabs()
            .iter()
            .filter(|tab| tab.title == "Search: order")
            .count(),
        1
    );
}

#[test]
fn open_search_results_for_two_queries_opens_two_distinct_tabs() {
    let (mut cx, mut app) = rebuilt_search_app();

    app.open_search_results(&mut cx, "order");
    let order_tab = app.documents.active_id();
    app.open_search_results(&mut cx, "customer");
    let customer_tab = app.documents.active_id();

    assert_ne!(order_tab, customer_tab);
    assert_eq!(app.documents.tabs().len(), 2);
}

#[test]
fn activating_a_search_result_row_navigates_to_the_hit_document_and_stashes_a_pending_reveal() {
    let (mut cx, mut app) = rebuilt_search_app();
    app.open_search_results(&mut cx, "order");

    // What `SearchResultsView::handle` returns for a row click on a hit in
    // "sales/order" -- exercised through `apply_view_outcome` the same way
    // the shell's action-observer applies it.
    let outcome = crate::doc_view::ViewOutcome {
        navigation: Some(crate::navigation::NavigationIntent::Resolved {
            target: crate::navigation::NavigationTarget::Document {
                concept_id: "sales/order".to_string(),
                surface: Some(waml::view::surface::SurfaceId::markdown()),
                fragment: None,
            },
            disposition: crate::navigation::OpenDisposition::Preview,
        }),
        reveal: Some((
            "sales/order".to_string(),
            crate::doc_view::RevealTarget::TextSpan { start: 0, end: 4 },
        )),
        ..Default::default()
    };

    app.apply_view_outcome(&mut cx, outcome);

    assert_eq!(
        app.documents.active_tab().and_then(|tab| tab.concept_id()),
        Some("sales/order")
    );
    assert_eq!(
        app.pending_reveal,
        Some(PendingReveal {
            concept_id: "sales/order".to_string(),
            target: crate::doc_view::RevealTarget::TextSpan { start: 0, end: 4 },
        })
    );
}

/// "details" hits both `sales/customer.md` (a body word under "## History")
/// and `sales/next.md` (its own "## Details" heading) -- a guaranteed
/// two-document result set, the fixture Task 14's cross-document F3 test
/// needs.
#[test]
fn open_search_results_starts_a_bundle_wide_session_over_two_documents() {
    let (mut cx, mut app) = rebuilt_search_app();
    app.open_search_results(&mut cx, "details");

    let session = app
        .session_search
        .as_ref()
        .expect("open_search_results starts the bundle-wide session");
    assert_eq!(session.query, "details");
    assert_eq!(session.hits.len(), 2);
    assert!(session.cursor.is_none());
    let concepts: std::collections::HashSet<String> = session
        .hits
        .iter()
        .map(crate::search_results_view::concept_id_for_hit)
        .collect();
    assert_eq!(
        concepts.len(),
        2,
        "the two hits land in different documents"
    );
}

#[test]
fn f3_advances_the_live_session_across_document_boundaries_and_wraps() {
    let (mut cx, mut app) = rebuilt_search_app();
    app.open_search_results(&mut cx, "details");

    let first_hit = app.session_search.as_ref().unwrap().hits[0].clone();
    let first_concept = crate::search_results_view::concept_id_for_hit(&first_hit);
    let second_concept = crate::search_results_view::concept_id_for_hit(
        &app.session_search.as_ref().unwrap().hits[1],
    );

    // Land on the first hit, the way a results-tab row click does.
    let (navigation, reveal) = crate::search_results_view::navigation_for_hit(&first_hit);
    app.apply_view_outcome(
        &mut cx,
        crate::doc_view::ViewOutcome {
            navigation: Some(navigation),
            reveal,
            ..Default::default()
        },
    );
    assert_eq!(
        app.documents.active_tab().and_then(|tab| tab.concept_id()),
        Some(first_concept.as_str())
    );
    assert_eq!(app.session_search.as_ref().unwrap().cursor, Some(0));

    // F3 crosses into the OTHER document.
    app.step_session(&mut cx, true);
    assert_eq!(
        app.documents.active_tab().and_then(|tab| tab.concept_id()),
        Some(second_concept.as_str())
    );
    assert!(app.pending_reveal.is_some());

    // F3 again wraps back to the first.
    app.step_session(&mut cx, true);
    assert_eq!(
        app.documents.active_tab().and_then(|tab| tab.concept_id()),
        Some(first_concept.as_str())
    );

    // Shift+F3 walks backward, wrapping the other way, straight to the
    // second document.
    app.step_session(&mut cx, false);
    assert_eq!(
        app.documents.active_tab().and_then(|tab| tab.concept_id()),
        Some(second_concept.as_str())
    );
}

/// Every Names/Model/Structure entry of one concept carries the same
/// `HitTarget::ModelElement`, so two hits routinely resolve to the SAME
/// `(concept_id, RevealTarget)`. The landing must keep the index the step
/// chose rather than re-deriving it from the target, or F3 oscillates
/// between the first two and never reaches the rest.
#[test]
fn f3_advances_past_hits_that_share_one_reveal_target() {
    let (mut cx, mut app) = rebuilt_search_app();
    let hit = |entry: u32| waml::search::Hit {
        document: "sales/customer.md".to_string(),
        concept_id: Some("sales/customer".to_string()),
        group: waml::search::FieldGroup::Model,
        target: waml::search::HitTarget::ModelElement {
            key: "sales/customer".to_string(),
        },
        entry,
        score: 1.0,
    };
    app.session_search = Some(crate::search_session::SearchSession::new(
        "customer".to_string(),
        vec![hit(0), hit(1), hit(2)],
        waml::search::QueryScope::default(),
    ));

    app.step_session(&mut cx, true);
    assert_eq!(app.session_search.as_ref().unwrap().cursor, Some(0));

    app.step_session(&mut cx, true);
    assert_eq!(
        app.session_search.as_ref().unwrap().cursor,
        Some(1),
        "F3 must not fall back to the first hit sharing the target"
    );

    app.step_session(&mut cx, true);
    assert_eq!(app.session_search.as_ref().unwrap().cursor, Some(2));

    // Shift+F3 walks back down the same list.
    app.step_session(&mut cx, false);
    assert_eq!(app.session_search.as_ref().unwrap().cursor, Some(1));
}

/// `mark_session_landing` lights every match the session found in the
/// landing document, but the pending reveal that always follows it installs
/// the landed range as the surface's ONLY highlight -- so the documented
/// "every other match in the open document is highlighted" only survives to
/// a frame if the reveal is followed by a re-light.
#[test]
fn a_landed_reveal_keeps_the_sessions_other_matches_in_that_document_lit() {
    let (mut cx, mut app) = rebuilt_search_app();
    mount_markdown_surface(&mut cx, &mut app);
    let hit = |start: u32, end: u32| waml::search::Hit {
        document: "sales/customer.md".to_string(),
        concept_id: Some("sales/customer".to_string()),
        group: waml::search::FieldGroup::Prose,
        target: waml::search::HitTarget::TextSpan {
            start,
            end,
            line: 0,
        },
        entry: 0,
        score: 1.0,
    };
    app.session_search = Some(crate::search_session::SearchSession::new(
        "details".to_string(),
        vec![hit(4, 8), hit(20, 24)],
        waml::search::QueryScope::default(),
    ));

    // Land on the first hit, the way a results-tab row click does.
    let first = app.session_search.as_ref().unwrap().hits[0].clone();
    let (navigation, reveal) = crate::search_results_view::navigation_for_hit(&first);
    app.apply_view_outcome(
        &mut cx,
        crate::doc_view::ViewOutcome {
            navigation: Some(navigation),
            reveal,
            ..Default::default()
        },
    );
    assert_eq!(
        app.documents.active_tab().and_then(|tab| tab.concept_id()),
        Some("sales/customer")
    );

    app.apply_pending_reveal(&mut cx);

    let highlights = app
        .ui
        .widget(&cx, ids!(markdown_surface.editor))
        .as_markdown_editor()
        .search_highlights();
    assert_eq!(
        highlights.len(),
        2,
        "the reveal must not narrow the session's highlights down to the landed range, got {highlights:?}"
    );
}

#[test]
fn esc_ends_the_live_session_and_further_f3_is_a_no_op() {
    let (mut cx, mut app) = rebuilt_search_app();
    app.open_search_results(&mut cx, "details");
    app.step_session(&mut cx, true);
    let landed = app
        .documents
        .active_tab()
        .and_then(|tab| tab.concept_id())
        .map(str::to_string);
    assert!(landed.is_some());

    app.handle_escape_event(
        &mut cx,
        &Event::KeyDown(KeyEvent {
            key_code: KeyCode::Escape,
            ..Default::default()
        }),
    );

    assert!(app.session_search.is_none());

    // A further F3 does nothing: no session left to walk.
    app.step_session(&mut cx, true);
    assert_eq!(
        app.documents
            .active_tab()
            .and_then(|tab| tab.concept_id())
            .map(str::to_string),
        landed
    );
}

fn book_navigation_app() -> (Cx, App) {
    let (cx, mut app) = navigation_app();
    let source = waml::source::SourceBundle::try_from_pairs([
        (
            "index.md",
            "# Root\n\n* [Guide](guide/)\n* [Plain](plain/)\n",
        ),
        (
            "guide/index.md",
            "---\nview: book\n---\n# Guide\n\n* [Intro](intro.md)\n",
        ),
        ("guide/intro.md", "# Intro\n\nSome prose.\n"),
        ("plain/index.md", "# Plain\n\n* [Note](note.md)\n"),
        ("plain/note.md", "# Note\n"),
    ])
    .unwrap();
    app.session.replace(source).unwrap();
    (cx, app)
}

#[test]
fn a_directory_declaring_view_book_opens_on_the_book_surface() {
    let (mut cx, mut app) = book_navigation_app();
    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Directory {
            address: "/guide".to_string()
        },
        crate::navigation::OpenDisposition::Preview,
        &mut FakeBrowser::default(),
    ));
    let tab = app.documents.active_tab().unwrap();
    assert_eq!(
        tab.locator().surface,
        waml::view::surface::SurfaceId::book()
    );
    assert!(matches!(
        &tab.locator().target,
        waml::view::row::RowTarget::Folder(address) if address == "/guide"
    ));
}

#[test]
fn a_plain_directory_still_opens_the_folder_listing() {
    let (mut cx, mut app) = book_navigation_app();
    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Directory {
            address: "/plain".to_string()
        },
        crate::navigation::OpenDisposition::Preview,
        &mut FakeBrowser::default(),
    ));
    assert_eq!(
        app.documents.active_tab().unwrap().locator(),
        crate::view_history::DocumentLocator::folder("/plain")
    );
}

#[test]
fn the_book_header_toggle_drops_to_the_folder_listing_of_the_same_directory() {
    let (mut cx, mut app) = book_navigation_app();
    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Directory {
            address: "/guide".to_string()
        },
        crate::navigation::OpenDisposition::Preview,
        &mut FakeBrowser::default(),
    ));
    let outcome = crate::doc_view::ViewOutcome {
        open_folder_listing: Some("/guide".to_string()),
        ..Default::default()
    };
    app.apply_view_outcome(&mut cx, outcome);
    assert_eq!(
        app.documents.active_tab().unwrap().locator(),
        crate::view_history::DocumentLocator::folder("/guide")
    );
}

#[test]
fn refreshing_folder_tabs_rebuilds_an_open_book_tab_in_place() {
    let (mut cx, mut app) = book_navigation_app();
    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Directory {
            address: "/guide".to_string()
        },
        crate::navigation::OpenDisposition::Preview,
        &mut FakeBrowser::default(),
    ));
    let before = app.documents.active_id();
    app.refresh_folder_tabs(&mut cx);
    assert_eq!(
        app.documents.active_id(),
        before,
        "same tab identity after reopen-in-place"
    );
    assert_eq!(
        app.documents.active_tab().unwrap().locator().surface,
        waml::view::surface::SurfaceId::book()
    );
}

#[test]
fn a_tree_click_on_a_section_of_the_active_book_reveals_instead_of_opening() {
    let (mut cx, mut app) = book_navigation_app();
    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Directory {
            address: "/guide".to_string()
        },
        crate::navigation::OpenDisposition::Preview,
        &mut FakeBrowser::default(),
    ));
    let tabs_before = app.documents.tabs().len();
    let handled = app.try_reveal_in_active_book(
        &mut cx,
        &NavigationTarget::Document {
            concept_id: "guide/intro".to_string(),
            surface: None,
            fragment: None,
        },
    );
    assert!(handled, "a section concept must resolve to a reveal");
    assert_eq!(app.documents.tabs().len(), tabs_before, "no new tab");
    assert_eq!(
        app.documents.active_tab().unwrap().locator().surface,
        waml::view::surface::SurfaceId::book(),
        "the book stays active"
    );
}

#[test]
fn a_tree_click_outside_the_active_book_still_opens_a_tab() {
    let (mut cx, mut app) = book_navigation_app();
    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Directory {
            address: "/guide".to_string()
        },
        crate::navigation::OpenDisposition::Preview,
        &mut FakeBrowser::default(),
    ));
    let handled = app.try_reveal_in_active_book(
        &mut cx,
        &NavigationTarget::Document {
            concept_id: "plain/note".to_string(),
            surface: None,
            fragment: None,
        },
    );
    assert!(
        !handled,
        "a non-section target falls through to the open path"
    );
}

#[test]
fn a_tree_click_reveals_nothing_when_a_folder_listing_is_active() {
    let (mut cx, mut app) = book_navigation_app();
    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Directory {
            address: "/plain".to_string()
        },
        crate::navigation::OpenDisposition::Preview,
        &mut FakeBrowser::default(),
    ));
    assert!(!app.try_reveal_in_active_book(
        &mut cx,
        &NavigationTarget::Document {
            concept_id: "plain/note".to_string(),
            surface: None,
            fragment: None,
        },
    ));
}

#[test]
fn a_tree_mark_outcome_scrolls_and_pulses_the_tree_row() {
    let (mut cx, mut app) = book_navigation_app();
    // `navigation_app` mounted a real `ProjectTree`, but its view was built
    // from the session BEFORE `book_navigation_app` swapped the source;
    // rebuild it so the panel's roots list the book's rows.
    app.refresh_nav(&mut cx, false);

    let outcome = crate::doc_view::ViewOutcome {
        tree_mark: Some(NavigationTarget::Document {
            concept_id: "guide/intro".to_string(),
            surface: None,
            fragment: None,
        }),
        ..Default::default()
    };
    let flow = app.apply_view_outcome(&mut cx, outcome);
    assert_eq!(
        flow,
        super::super::actions::ActionFlow::Continue,
        "marking is a mirror, not a claim on the event"
    );

    let panel = app.ui.widget(&cx, ids!(project_tree));
    let reveal = panel
        .borrow::<crate::tree_panel::ProjectTree>()
        .and_then(|p| p.test_reveal_key().map(str::to_string));
    assert!(reveal.is_some(), "the tree's reveal pulse path is armed");
}

// ---------------------------------------------------------------------------
// Read as scroll (spec 2026-08-11-read-as-scroll-design): the folder context
// menu's entry is a navigation to the folder's book locator, not a mode.
// ---------------------------------------------------------------------------

#[test]
fn read_as_scroll_opens_the_folders_book_tab_distinct_from_its_listing() {
    let (mut cx, mut app) = book_navigation_app();
    // "/plain" declares nothing, so a click opens its listing...
    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Directory {
            address: "/plain".to_string()
        },
        crate::navigation::OpenDisposition::Preview,
        &mut FakeBrowser::default(),
    ));
    let listing_id = app.documents.active_id();
    // ...while the menu entry opens the same folder's BOOK tab.
    assert!(app.open_folder_as_scroll(&mut cx, "/plain"));
    let tab = app.documents.active_tab().unwrap();
    assert_eq!(
        tab.locator(),
        crate::view_history::DocumentLocator::new(
            waml::view::row::RowTarget::Folder("/plain".to_string()),
            waml::view::surface::SurfaceId::book(),
        )
    );
    assert_ne!(
        app.documents.active_id(),
        listing_id,
        "the book tab and the listing tab are two tabs"
    );
}

#[test]
fn the_menu_path_and_the_declared_path_share_one_book_tab() {
    let (mut cx, mut app) = book_navigation_app();
    // "/guide" declares `view: book`, so a plain click already opens the book.
    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Directory {
            address: "/guide".to_string()
        },
        crate::navigation::OpenDisposition::Preview,
        &mut FakeBrowser::default(),
    ));
    let declared_id = app.documents.active_id();
    let declared_locator = app.documents.active_tab().unwrap().locator();
    let tabs_before = app.documents.tabs().len();
    // The menu entry lands on the SAME tab: one path, no second book surface.
    assert!(app.open_folder_as_scroll(&mut cx, "/guide"));
    assert_eq!(app.documents.active_id(), declared_id);
    assert_eq!(
        app.documents.active_tab().unwrap().locator(),
        declared_locator
    );
    assert_eq!(app.documents.tabs().len(), tabs_before, "no duplicate tab");
}

#[test]
fn a_folder_with_no_index_opens_as_a_scroll_titled_by_its_name() {
    let (mut cx, mut app) = navigation_app();
    let source = waml::source::SourceBundle::try_from_pairs([
        ("index.md", "# Root\n\n* [First](notes/first.md)\n"),
        ("notes/first.md", "# First\n\nSome prose.\n"),
    ])
    .unwrap();
    app.session.replace(source).unwrap();

    assert!(
        app.open_folder_as_scroll(&mut cx, "/notes"),
        "an index-less folder is exactly what the menu entry is for"
    );
    let tab = app.documents.active_tab().unwrap();
    assert_eq!(
        tab.locator().surface,
        waml::view::surface::SurfaceId::book()
    );
    assert_eq!(tab.title, "notes", "title falls back to the folder name");
}

#[test]
fn a_missing_folder_reports_instead_of_opening() {
    let (mut cx, mut app) = book_navigation_app();
    assert!(!app.open_folder_as_scroll(&mut cx, "/absent"));
    assert!(app.documents.active_tab().is_none());
}

#[test]
fn right_clicking_a_folder_row_opens_no_tab() {
    let (mut cx, mut app) = mounted_production_shell();
    let uid = app.ui.widget(&cx, ids!(project_tree)).widget_uid();
    let actions: ActionsBuf = vec![Box::new(WidgetAction {
        data: None,
        action: Box::new(crate::tree_panel::ProjectTreeAction::ContextMenu {
            target: NavigationTarget::Directory {
                address: "/guide".to_string(),
            },
            anchor: dvec2(10.0, 10.0),
        }),
        widget_uid: uid,
        group: None,
    })];

    let flow = app.handle_tree_context_menu(&mut cx, &actions);

    assert_eq!(flow, super::super::actions::ActionFlow::Consumed);
    assert!(
        app.documents.tabs().is_empty(),
        "a menu the user may dismiss must not open a tab as a side effect"
    );
    assert_eq!(
        app.folder_menu_address.as_deref(),
        Some("/guide"),
        "the commit handler's subject is armed"
    );
}

#[test]
fn committing_read_as_scroll_opens_the_armed_folders_book() {
    let (mut cx, mut app) = mounted_production_shell();
    let source = waml::source::SourceBundle::try_from_pairs([
        ("index.md", "# Root\n\n* [Plain](plain/)\n"),
        ("plain/index.md", "# Plain\n\n* [Note](note.md)\n"),
        ("plain/note.md", "# Note\n"),
    ])
    .unwrap();
    app.session.replace(source).unwrap();
    app.ensure_markdown_asset_host(crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle);
    // As armed by `handle_tree_context_menu`'s `Directory` arm.
    app.folder_menu_address = Some("/plain".to_string());
    let popup_uid = app.ui.widget(&cx, ids!(popup_root)).widget_uid();
    let actions: ActionsBuf = vec![Box::new(WidgetAction {
        data: None,
        action: Box::new(crate::popup::root::PopupRootAction::Closed {
            tag: live_id!(folder_menu),
            result: crate::popup::base::PopupResult::Invoked(live_id!(read_as_scroll)),
        }),
        widget_uid: popup_uid,
        group: None,
    })];

    app.observe_popup_results(&mut cx, &actions);

    let tab = app.documents.active_tab().expect("the commit opened a tab");
    assert_eq!(
        tab.locator(),
        crate::view_history::DocumentLocator::new(
            waml::view::row::RowTarget::Folder("/plain".to_string()),
            waml::view::surface::SurfaceId::book(),
        )
    );
}

/// The book tab's surface must be a LIVE `BookSurface`, not a dead node.
/// `book := BookSurface{ .. }` only resolves if App's own DSL imports the
/// widget (`use mod.widgets.BookSurface`); registering `script_mod` is not
/// enough. Without the import the node silently becomes dead and invisible --
/// every `set_model` no-ops and the book tab renders blank -- while every
/// model-level book test above stays green. Asserted through
/// `mounted_production_shell`, the only fixture that evaluates the real App
/// DSL.
#[test]
fn the_mounted_book_surface_is_a_live_widget() {
    let (cx, app) = mounted_production_shell();
    let book = app.ui.widget(&cx, ids!(book_surface.book));
    assert!(
        book.borrow::<crate::book_surface::BookSurface>().is_some(),
        "book_surface.book must resolve to a real BookSurface -- \
         a missing `use mod.widgets.BookSurface` leaves a dead, invisible node"
    );
}

/// `BookSurface` builds its prose children from App's `ReadingProse` alias by
/// NAME, so nothing in the compiler stops the alias from being renamed or
/// dropped -- the book would silently fall back to the unthemed default and
/// draw prose in makepad's dark-theme text colour on our light surface.
/// Asserted against the real App DSL, the only place the alias is declared.
#[test]
fn the_reading_prose_alias_is_declared() {
    let (mut cx, _app) = mounted_production_shell();
    let declared = cx.with_vm(|vm| !crate::book_surface::reading_prose_value(vm).is_nil());
    assert!(
        declared,
        "mod.widgets.ReadingProse must exist -- BookSurface resolves it by name"
    );
}

#[test]
fn right_clicking_a_concept_row_still_opens_it_before_the_menu() {
    let (mut cx, mut app) = mounted_production_shell();
    let source = waml::source::SourceBundle::try_from_pairs([(
        "order.md",
        "---\ntype: Runbook\ntitle: Order\n---\n# Order\n",
    )])
    .unwrap();
    app.session.replace(source).unwrap();
    app.ensure_markdown_asset_host(crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle);
    let uid = app.ui.widget(&cx, ids!(project_tree)).widget_uid();
    let actions: ActionsBuf = vec![Box::new(WidgetAction {
        data: None,
        action: Box::new(crate::tree_panel::ProjectTreeAction::ContextMenu {
            target: NavigationTarget::Document {
                concept_id: "order".to_string(),
                surface: None,
                fragment: None,
            },
            anchor: dvec2(10.0, 10.0),
        }),
        widget_uid: uid,
        group: None,
    })];

    let flow = app.handle_tree_context_menu(&mut cx, &actions);

    assert_eq!(flow, super::super::actions::ActionFlow::Consumed);
    assert_eq!(
        app.documents.active_tab().and_then(|tab| tab.concept_id()),
        Some("order"),
        "the concept path keeps its established open-then-menu behavior"
    );
    assert_eq!(app.node_menu_key.as_deref(), Some("order"));
}
