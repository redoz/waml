use super::*;
use crate::app::workspace::{
    browser_save_fragment, bundle_paths_differ, close_after_save, prevent_quit_after_failed_save,
    replace_after_save, restore_markdown_asset_host_after_open, should_flush_save,
    BackingTransitionError, SaveFeedback,
};

#[test]
fn conflict_reload_detects_server_side_creates_and_deletes() {
    let bundle = |pairs: &[(&str, &str)]| {
        waml::source::SourceBundle::try_from_pairs(pairs.iter().copied()).unwrap()
    };
    let current = bundle(&[("a.md", "# A\n"), ("b.md", "# B\n")]);

    // Same paths, different text: representable per-document, not structural.
    assert!(!bundle_paths_differ(
        &current,
        &bundle(&[("a.md", "# A changed\n"), ("b.md", "# B\n")])
    ));
    // A document created on the server.
    assert!(bundle_paths_differ(
        &current,
        &bundle(&[("a.md", "# A\n"), ("b.md", "# B\n"), ("c.md", "# C\n")])
    ));
    // A document deleted on the server.
    assert!(bundle_paths_differ(&current, &bundle(&[("a.md", "# A\n")])));
    // A rename is a create plus a delete.
    assert!(bundle_paths_differ(
        &current,
        &bundle(&[("a.md", "# A\n"), ("renamed.md", "# B\n")])
    ));
}

#[test]
fn failed_open_restores_the_previous_markdown_asset_root() {
    let previous = crate::markdown_hosts::EditorMarkdownAssetHost::shared(
        crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle,
    );
    let candidate = crate::markdown_hosts::EditorMarkdownAssetHost::shared(
        crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle,
    );
    let mut installed = Some(candidate);

    assert!(!restore_markdown_asset_host_after_open(
        &mut installed,
        Some(previous.clone()),
        false,
    ));
    assert!(Rc::ptr_eq(installed.as_ref().unwrap(), &previous));
}

#[test]
fn browser_save_ticket_encodes_invalid_source_bytes_exactly() {
    let original = "# Order\r\n";
    let invalid = "# Order\r\n[unterminated **\u{2028}";
    let path = waml::source::BundlePath::parse("order.md").unwrap();
    let mut session = crate::editor_session::EditorSession::default();
    session
        .replace(waml::source::SourceBundle::try_from_pairs([("order.md", original)]).unwrap())
        .unwrap();
    let before = session.snapshot();
    let document = before.okf_analysis.catalog.id_for_path(&path).unwrap();
    let syntax = before.markdown_snapshot(document).unwrap();
    session
        .apply(waml::edit::ExactSourceEdit {
            document,
            base_revision: syntax.revision(),
            changes: Arc::from([TextChange {
                old_range: TextRange::new(TextSize::new(0), syntax.text().len()).unwrap(),
                replacement: Arc::from(invalid),
            }]),
            expected_text: SourceText::new(invalid.to_string()).unwrap(),
        })
        .unwrap();
    let ticket = session.save_ticket().unwrap();

    let (fragment, completion) = browser_save_fragment(&ticket);
    let decoded = waml::share::decode_source(fragment.trim_start_matches('#')).unwrap();

    assert_eq!(completion.revision, ticket.revision);
    assert_eq!(completion.history_state, ticket.history_state);
    assert_eq!(completion.result, Ok(()));
    assert_eq!(
        decoded.document(&path).unwrap().text().as_bytes(),
        invalid.as_bytes()
    );
}

/// A site booted from `?bundle=` must keep that query after the first edit,
/// and boot from the fragment from then on.
///
/// `FromWasmBrowserUpdateUrl` resolves what `save_backend` hands it against the
/// current href, so a fragment-only string leaves `pathname` and `search`
/// untouched and replaces the fragment whole -- there can never be two. That
/// only holds while the saved string really is fragment-only, which is what is
/// asserted here, together with the precedence flip it causes.
#[test]
fn a_first_edit_replaces_the_fragment_and_keeps_the_bundle_query() {
    let search = "?bundle=bundle.waml";
    assert_eq!(
        crate::browser_boot::select_browser_boot(search, "").unwrap(),
        crate::browser_boot::BrowserBootSource::Bundle("bundle.waml".to_string())
    );

    let original = "# Order\n";
    let edited = "# Order edited\n";
    let path = waml::source::BundlePath::parse("order.md").unwrap();
    let mut session = crate::editor_session::EditorSession::default();
    session
        .replace(waml::source::SourceBundle::try_from_pairs([("order.md", original)]).unwrap())
        .unwrap();
    let before = session.snapshot();
    let document = before.okf_analysis.catalog.id_for_path(&path).unwrap();
    let syntax = before.markdown_snapshot(document).unwrap();
    session
        .apply(waml::edit::ExactSourceEdit {
            document,
            base_revision: syntax.revision(),
            changes: Arc::from([TextChange {
                old_range: TextRange::new(TextSize::new(0), syntax.text().len()).unwrap(),
                replacement: Arc::from(edited),
            }]),
            expected_text: SourceText::new(edited.to_string()).unwrap(),
        })
        .unwrap();
    let ticket = session.save_ticket().unwrap();
    let (fragment, _) = browser_save_fragment(&ticket);

    assert!(fragment.starts_with('#'), "{fragment}");
    assert_eq!(fragment.matches('#').count(), 1, "{fragment}");
    assert!(
        !fragment.contains('?') && !fragment.contains('/'),
        "{fragment}"
    );
    assert_eq!(
        crate::browser_boot::select_browser_boot(search, &fragment).unwrap(),
        crate::browser_boot::BrowserBootSource::Share(fragment)
    );
}

#[test]
fn shutdown_and_quit_request_are_final_save_events() {
    assert!(should_flush_save(&Event::Shutdown));
    assert!(should_flush_save(&Event::QuitRequested(
        QuitRequestedEvent::new(QuitReason::Menu)
    )));
    assert!(!should_flush_save(&Event::Startup));
}

#[test]
fn failed_final_save_retains_dirty_and_prevents_quit() {
    let result = Err("disk full".to_string());

    let quit = Event::QuitRequested(QuitRequestedEvent::new(QuitReason::Menu));
    assert!(prevent_quit_after_failed_save(&quit, &result));
    let Event::QuitRequested(quit) = quit else {
        unreachable!()
    };
    assert!(quit.handled.get());
}

// Scenario: NATIVE-031
#[test]
fn successful_bundle_open_clears_the_visible_save_error() {
    let mut state = SaveFeedback::default();
    state.finish_save(&Err("disk full".into()));
    assert_eq!(state.save_error(), Some("disk full"));

    state.opened_replacement_bundle();

    assert_eq!(state.save_error(), None);
}

#[test]
fn replacement_saves_old_document_before_loading_new_document() {
    let calls = RefCell::new(Vec::new());

    let loaded = replace_after_save(
        || {
            calls.borrow_mut().push("save");
            Ok(())
        },
        || {
            calls.borrow_mut().push("load");
            Ok("new document")
        },
    )
    .unwrap();

    assert_eq!(calls.into_inner(), vec!["save", "load"]);
    assert_eq!(loaded, "new document");
}

#[test]
fn failed_save_blocks_replacement_load() {
    let error = replace_after_save(
        || Err("external edit conflict".into()),
        || -> Result<(), String> { panic!("replacement must not load after a failed save") },
    )
    .unwrap_err();

    assert_eq!(
        error,
        BackingTransitionError::Save("external edit conflict".into())
    );
}

// Scenario: NATIVE-056
#[test]
fn failed_save_blocks_close_and_keeps_document_state() {
    let mut state = vec!["edited"];
    let before = state.clone();

    assert_eq!(
        close_after_save(
            &mut state,
            |_| Err("disk full".into()),
            |current| current.clear(),
        ),
        Err("disk full".into())
    );
    assert_eq!(state, before);
}

/// Closing a model and opening another one must show the NEW model's tree.
///
/// The nav projection is memoized on the session revision, so a close that
/// rewinds the revision counter (back to a value the previous model already
/// used) makes the next open collide with the cached tree of the model that
/// was just closed.
#[test]
fn reopening_after_a_close_shows_the_new_model_in_the_tree() {
    let (mut cx, mut app) = navigation_app();
    app.refresh_nav(&mut cx, true);
    assert!(
        tree_row_titles(&cx, &app).iter().any(|t| t == "Sales"),
        "the first model's tree is the baseline: {:?}",
        tree_row_titles(&cx, &app)
    );

    assert!(app.close_model(&mut cx));

    let next = waml::source::SourceBundle::try_from_pairs([
        ("index.md", "# Root\n\n* [Ops](ops/)\n"),
        ("ops/index.md", "# Ops\n\n* [Incident](incident.md)\n"),
        (
            "ops/incident.md",
            "---\ntype: Runbook\ntitle: Incident\n---\n# Incident\n",
        ),
    ])
    .unwrap();
    assert!(app.open_bundle(&mut cx, next, "ops".to_string(), None));

    let titles = tree_row_titles(&cx, &app);
    assert!(titles.iter().any(|t| t == "Ops"), "{titles:?}");
    assert!(!titles.iter().any(|t| t == "Sales"), "{titles:?}");
}

/// `open_bundle` must rebuild the text index, not just the tree/tabs -- a
/// term from the freshly-opened bundle should be findable immediately, with
/// no separate "build the index" step a caller could forget.
#[test]
fn opening_a_bundle_rebuilds_the_search_index() {
    let (mut cx, mut app) = navigation_app();

    let next = waml::source::SourceBundle::try_from_pairs([
        ("index.md", "# Root\n\n* [Ops](ops/)\n"),
        ("ops/index.md", "# Ops\n\n* [Incident](incident.md)\n"),
        (
            "ops/incident.md",
            "---\ntype: Runbook\ntitle: Incident\n---\n# Incident\n\nA fire drill happened.\n",
        ),
    ])
    .unwrap();
    assert!(app.open_bundle(&mut cx, next, "ops".to_string(), None));

    let hits = app
        .search
        .query("fire", &waml::search::QueryScope::default());
    assert!(
        !hits.is_empty(),
        "expected a hit for a term in the freshly-opened bundle"
    );
}

fn tree_row_titles(cx: &Cx, app: &App) -> Vec<String> {
    app.ui
        .widget(cx, ids!(project_tree))
        .borrow::<crate::tree_panel::ProjectTree>()
        .expect("the test shell mounts project_tree")
        .test_row_titles()
}

// Scenario: NATIVE-057
#[test]
fn successful_save_allows_close_and_clears_document_state() {
    let mut state = vec!["edited"];
    let mut saved = false;

    close_after_save(
        &mut state,
        |current| {
            assert_eq!(current, &vec!["edited"]);
            saved = true;
            Ok(())
        },
        |current| current.clear(),
    )
    .unwrap();

    assert!(saved);
    assert!(state.is_empty());
}
