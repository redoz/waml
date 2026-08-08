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
        close_after_save(&mut state, |_| Err("disk full".into())),
        Err("disk full".into())
    );
    assert_eq!(state, before);
}

// Scenario: NATIVE-057
#[test]
fn successful_save_allows_close_and_clears_document_state() {
    let mut state = vec!["edited"];
    let mut saved = false;

    close_after_save(&mut state, |current| {
        assert_eq!(current, &vec!["edited"]);
        saved = true;
        Ok(())
    })
    .unwrap();

    assert!(saved);
    assert!(state.is_empty());
}
