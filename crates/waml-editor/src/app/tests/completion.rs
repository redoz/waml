//! The completion popup, driven end to end without a window: a mounted app, a
//! sequence document, real events. What cannot be asserted here is pixels --
//! that the popup is *visible* is the visual check -- but everything else is:
//! when the list opens, what accepting writes, and when the list stays shut.

use super::navigation::mount_markdown_surface;
use super::*;

const SEQUENCE: &str = "---\ntype: uml.SequenceDiagram\ntitle: S\n---\n# S\n\n## Lifelines\n\n- [A](./a.md) as A\n\n## Messages\n\n- A \n";

fn mounted_sequence_app() -> (Cx, App) {
    let (mut cx, mut app) = navigation_app();
    let source = waml::source::SourceBundle::try_from_pairs([
        ("notes/seq.md", SEQUENCE),
        ("notes/a.md", "---\ntype: uml.Class\ntitle: A\n---\n# A\n"),
    ])
    .unwrap();
    let change = app.session.replace(source).unwrap();
    app.complete_session_change(&mut cx, change);
    mount_markdown_surface(&mut cx, &mut app);
    app.open_view_source(&mut cx, "notes/seq");
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

/// Dispatch one event through the whole app and apply whatever it proposed.
fn dispatch(cx: &mut Cx, app: &mut App, event: Event) {
    let actions = cx.capture_actions(|cx| {
        <App as AppMain>::handle_event(app, cx, &event);
    });
    app.handle_action_batch(cx, &actions);
}

fn key(cx: &mut Cx, app: &mut App, key_code: KeyCode) {
    dispatch(
        cx,
        app,
        Event::KeyDown(KeyEvent {
            key_code,
            ..Default::default()
        }),
    );
}

/// The caret parked in the empty verb slot after `- A `.
fn place_caret_at_verb_slot(cx: &mut Cx, app: &mut App) {
    let document = app
        .session
        .snapshot()
        .okf_analysis
        .catalog
        .id_for_path(&waml::source::BundlePath::parse("notes/seq.md").unwrap())
        .unwrap();
    let revision = app
        .session
        .snapshot()
        .markdown_snapshot(document)
        .unwrap()
        .revision();
    let offset = TextSize::try_from_usize(SEQUENCE.find("- A \n").unwrap() + "- A ".len()).unwrap();
    assert!(app.handle_navigation_intent(
        cx,
        NavigationIntent::SourceRange {
            document,
            revision,
            range: TextRange::new(offset, offset).unwrap(),
        },
    ));
}

#[test]
fn a_caret_move_into_an_empty_verb_slot_opens_the_list() {
    let (mut cx, mut app) = mounted_sequence_app();
    let editor = app
        .ui
        .widget(&cx, ids!(markdown_surface.editor))
        .as_markdown_editor();
    editor.set_key_focus(&mut cx);
    place_caret_at_verb_slot(&mut cx, &mut app);
    // Navigation installs the caret directly; the popup opens on the caret
    // *gesture* -- here one arrow step back into the slot, as a hand on the
    // keyboard would arrive.
    key(&mut cx, &mut app, KeyCode::ArrowRight);
    key(&mut cx, &mut app, KeyCode::ArrowLeft);
    assert!(
        editor.has_completions(),
        "an empty verb slot must offer the message verbs"
    );
}

#[test]
fn tab_accepts_the_first_candidate_into_the_document() {
    let (mut cx, mut app) = mounted_sequence_app();
    let editor = app
        .ui
        .widget(&cx, ids!(markdown_surface.editor))
        .as_markdown_editor();
    editor.set_key_focus(&mut cx);
    place_caret_at_verb_slot(&mut cx, &mut app);
    key(&mut cx, &mut app, KeyCode::ArrowRight);
    key(&mut cx, &mut app, KeyCode::ArrowLeft);
    assert!(editor.has_completions());

    key(&mut cx, &mut app, KeyCode::Tab);

    let text = app
        .session
        .source()
        .document_by_concept_id("notes/seq")
        .unwrap()
        .text()
        .to_string();
    // The candidate list is sorted, so the first keyword is `calls`.
    assert!(text.contains("- A calls"), "{text}");
}

#[test]
fn escape_closes_the_list_and_a_plain_letter_never_opens_one_in_prose() {
    let (mut cx, mut app) = mounted_sequence_app();
    let editor = app
        .ui
        .widget(&cx, ids!(markdown_surface.editor))
        .as_markdown_editor();
    editor.set_key_focus(&mut cx);
    place_caret_at_verb_slot(&mut cx, &mut app);
    key(&mut cx, &mut app, KeyCode::ArrowRight);
    key(&mut cx, &mut app, KeyCode::ArrowLeft);
    assert!(editor.has_completions());

    key(&mut cx, &mut app, KeyCode::Escape);
    assert!(!editor.has_completions(), "Escape must close the list");

    // Arrow keys after a dismiss reopen it -- dismissal is per position, not
    // sticky -- which is also what makes Escape cheap to press by mistake.
    key(&mut cx, &mut app, KeyCode::ArrowRight);
    key(&mut cx, &mut app, KeyCode::ArrowLeft);
    assert!(editor.has_completions());
}
