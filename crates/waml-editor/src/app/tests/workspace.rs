use super::*;

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
