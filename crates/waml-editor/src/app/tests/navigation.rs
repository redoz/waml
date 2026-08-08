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
        "---\ntype: Diagram\ntitle: Orders\nprofile: uml-domain\ndescription: Initial\n---\n# Orders\n",
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

fn mount_markdown_surface(cx: &mut Cx, app: &mut App) {
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

#[test]
fn navigation_document_preview_persistence_and_repeat_activation_are_stable() {
    let (mut cx, mut app) = navigation_app();
    let mut browser = FakeBrowser::default();
    let order = NavigationTarget::Document {
        concept_id: "sales/order".into(),
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
    let folder_tab_id = crate::folder_view::folder_document_tab_id("/sales");
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
    app.narrow = true;
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
    // here by tab identity, matching the folder's dedicated
    // `folder_document_tab_id` namespace (distinct from every concept tab).
    let (mut cx, mut app) = navigation_app();
    let mut browser = FakeBrowser::default();
    app.narrow = true;
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
    let folder_tab = crate::folder_view::folder_document_tab_id("/");

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
            DocViewIdentity::ClassDiagram
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
        view_toggle: None,
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
            fragment: None,
        },
    };
    let icon_only = DocumentHeaderChrome {
        breadcrumb: false,
        right_dock: Some(Icon::PanelRight),
        view_toggle: None,
    };
    assert_eq!(
        project_document_header(icon_only, Some(vec![segment.clone()])),
        (Vec::new(), Some(Icon::PanelRight), None)
    );

    let breadcrumb = DocumentHeaderChrome {
        breadcrumb: true,
        right_dock: None,
        view_toggle: None,
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
    app.narrow = true;
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
            header.action(std::slice::from_ref(&action)),
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
