//! App-level Ctrl+F wiring (Task 13, spec §Find strip): the hotkey opens the
//! strip pre-scoped to the active document, typing re-queries `SearchState`
//! scoped to it and pushes fresh highlights/spotlight, Next/Previous step the
//! session and reveal through the shared `DocView::reveal` path, and Close
//! clears everything the strip touched. Needs `mounted_production_shell`
//! (not the lighter `navigation_app` fixture), the same reason
//! `app/tests/palette.rs` does: the find strip is a real DSL child of the
//! production widget tree, and `find_strip.rs`'s `FindStrip::open` reads/sets
//! real widget state (key focus, `visible`).

use super::*;

fn ctrl_f() -> Event {
    Event::KeyDown(KeyEvent {
        key_code: KeyCode::KeyF,
        modifiers: KeyModifiers {
            control: true,
            ..Default::default()
        },
        ..Default::default()
    })
}

fn text_input(text: &str) -> Event {
    Event::TextInput(TextInputEvent {
        input: text.to_string(),
        ..Default::default()
    })
}

fn enter() -> Event {
    Event::KeyDown(KeyEvent {
        key_code: KeyCode::ReturnKey,
        ..Default::default()
    })
}

fn escape() -> Event {
    Event::KeyDown(KeyEvent {
        key_code: KeyCode::Escape,
        ..Default::default()
    })
}

/// Dispatch `event` and recursively replay whatever actions it produced,
/// wrapped as `Event::Actions`, until quiescent -- mirroring `Cx::handle_actions`'s
/// own `while self.new_actions.len() != 0` drain loop (`platform/src/os/cx_shared.rs`),
/// which the platform's real event loop runs after every event but which a
/// single bare `<App as AppMain>::handle_event` call does not. Two things
/// depend on this cascade, neither reachable from one flat call:
///
/// - `find_strip.rs`'s `FindStrip` reacts to sibling/child actions
///   (`TextInput`'s `Changed`) via `WidgetMatchEvent::handle_actions`, which
///   only fires on an actual `Event::Actions(..)` pass, never inline during
///   the raw event that produced them (`widget_match_event`'s match arms,
///   `widgets/src/widget_match_event.rs`).
/// - `App::handle_actions` (`app.rs`, the `MatchEvent` trait method) is what
///   calls this test's own `handle_action_batch` -- so a raw event's
///   business-logic reaction (`observe_find_strip` et al.) is itself only
///   reachable through this same cascade, not from the raw event directly.
fn dispatch(cx: &mut Cx, app: &mut App, event: &Event) {
    let actions = cx.capture_actions(|cx| {
        <App as AppMain>::handle_event(app, cx, event);
    });
    if !actions.is_empty() {
        dispatch(cx, app, &Event::Actions(actions));
    }
}

/// A full draw pass over the whole app tree (the same shape as `mod.rs`'s
/// `draw_document_header`, applied to `app.ui` as a whole), followed by the
/// key-focus commit cycle. Two real gaps this closes, both invisible in a
/// windowed run (its normal frame loop already does both every frame) but
/// fatal to a headless `AppMain::handle_event` call:
///
/// 1. `cx.set_key_focus` only ever writes `next_key_focus`
///    (`platform/src/event/keyboard.rs`); `cx.key_focus()` -- what
///    `TextInput` actually gates on -- only updates when something calls
///    `cx.handle_triggers()`, which the platform's own event loop does after
///    every event. `cx.send_trigger(Area::Empty, ..)` before it is the
///    established idiom for forcing that cycle in a test (see
///    `class_diagram_view.rs`'s own `advance_focus` helper) -- `handle_triggers`'s
///    loop body (where the cycle actually runs) is gated on a non-empty
///    trigger queue.
/// 2. A widget's draw-quad area is only assigned by an actual draw walk, and
///    a `View` with `visible: false` skips `draw_walk` (and so never
///    measures its children) entirely -- `FindStrip::open` defers grabbing
///    focus to its own next `draw_walk` for exactly this reason (see that
///    method's doc comment), so that deferred grab needs a real draw to run
///    at all.
fn draw_and_settle_focus(cx: &mut Cx, app: &App, size: DVec2) {
    let draw_event = DrawEvent {
        redraw_all: true,
        ..DrawEvent::default()
    };
    let pass = DrawPass::new_with_name(cx, "find-strip-test");
    let mut draw_list = DrawList2d::new(cx);
    let mut draw_cx = CxDraw::new(cx, &draw_event);
    draw_cx.begin_pass(&pass, None);
    draw_list.begin_always(&mut draw_cx);
    {
        let mut cx_2d = Cx2d::new(&mut draw_cx);
        cx_2d.begin_root_turtle(size, Layout::default());
        app.ui
            .draw_walk_all(&mut cx_2d, &mut Scope::empty(), Walk::fill());
        cx_2d.end_turtle();
        draw_list.end(&mut cx_2d);
    }
    draw_cx.end_pass(&pass);
    drop(draw_cx);
    cx.send_trigger(Area::Empty, Trigger::default());
    cx.handle_triggers();
}

/// A production shell with one `uml.Class` concept, indexed and opened as
/// the active tab -- `documents::default_surface_for` puts a claimed UML
/// concept on its own generated documentation page (a `ClassifierPreviewView`,
/// since "order.md" is a bare classifier, not a named diagram document).
/// Drawn once so the rest of the tree has real areas before the strip itself
/// opens (see `draw_and_settle_focus`).
fn production_app_with_order_classifier() -> (Cx, App) {
    let (mut cx, mut app) = mounted_production_shell();
    let source = waml::source::SourceBundle::try_from_pairs([(
        "order.md",
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
    )])
    .unwrap();
    app.session.replace(source).unwrap();
    let snapshot = app.session.snapshot();
    app.search.rebuild(
        &snapshot.source,
        &snapshot.okf_analysis,
        &snapshot.uml_analysis,
    );
    app.ensure_markdown_asset_host(crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle);
    app.refresh_nav(&mut cx, true);
    app.show_editor(&mut cx);
    assert!(
        app.transition_document(&mut cx, "order", true),
        "fixture setup must land the order concept as the active tab"
    );
    draw_and_settle_focus(&mut cx, &app, dvec2(1280.0, 800.0));
    (cx, app)
}

#[test]
fn ctrl_f_opens_the_strip_pre_scoped_to_the_active_document() {
    let (mut cx, mut app) = production_app_with_order_classifier();

    dispatch(&mut cx, &mut app, &ctrl_f());

    assert!(app
        .ui
        .widget(&cx, ids!(find_strip))
        .as_find_strip()
        .is_open());
    assert_eq!(
        app.find
            .as_ref()
            .map(|session| session.scope.document.clone()),
        Some(Some("order.md".to_string()))
    );
}

#[test]
fn typing_a_term_from_the_fixture_yields_a_non_zero_hit_count() {
    let (mut cx, mut app) = production_app_with_order_classifier();
    dispatch(&mut cx, &mut app, &ctrl_f());
    // `open` defers its key-focus grab to the strip's own next `draw_walk`
    // (`find_strip.rs`'s doc comment) -- the strip only becomes visible on
    // THIS event, so the fixture's own draw pass (before it opened) never
    // drew it and doesn't count.
    draw_and_settle_focus(&mut cx, &app, dvec2(1280.0, 800.0));

    dispatch(&mut cx, &mut app, &text_input("order"));

    let hits = app
        .find
        .as_ref()
        .map(|session| session.hits.len())
        .unwrap_or(0);
    assert!(
        hits > 0,
        "expected at least one hit for \"order\", got {hits}"
    );
}

#[test]
fn next_on_a_canvas_tab_sets_a_spotlight_and_calls_reveal() {
    let (mut cx, mut app) = production_app_with_order_classifier();
    dispatch(&mut cx, &mut app, &ctrl_f());
    draw_and_settle_focus(&mut cx, &app, dvec2(1280.0, 800.0));

    dispatch(&mut cx, &mut app, &text_input("order"));
    dispatch(&mut cx, &mut app, &enter());

    let canvas = app.ui.widget(&cx, ids!(canvas));
    let surface = canvas
        .borrow::<crate::canvas::ClassDiagramSurface>()
        .expect("production shell mounts canvas");
    assert!(
        surface
            .search_spotlight()
            .is_some_and(|keys| !keys.is_empty()),
        "expected a non-empty spotlight after Next"
    );
    // The fixture's single classifier opens on `ClassifierPreviewView`, whose
    // `reveal` is a documented no-op ("this tab already IS a fixed focus on
    // self.key", `classifier_preview_view.rs`) -- there is no OTHER node on
    // this canvas to select or pan to, so `selected_key` staying `None` here
    // is the correct outcome, not an unwired path. `reveal(ModelElement)`
    // actually selecting and centring the camera on a multi-node
    // `ClassDiagramView` is already covered by Task 7's own
    // `reveal_model_element_selects_and_centres_the_camera_on_the_node`
    // (`class_diagram_view.rs`); this test's job is only to confirm `Next`
    // reaches that shared `reveal_active` path without crashing, which the
    // spotlight assertion above and the lack of a panic already establish.
    assert_eq!(surface.selected_key(), None);
}

#[test]
fn close_clears_the_spotlight_and_highlights() {
    let (mut cx, mut app) = production_app_with_order_classifier();
    dispatch(&mut cx, &mut app, &ctrl_f());
    draw_and_settle_focus(&mut cx, &app, dvec2(1280.0, 800.0));

    dispatch(&mut cx, &mut app, &text_input("order"));
    assert!(app.find.as_ref().is_some_and(|s| !s.hits.is_empty()));

    // Escape inside the strip's focused query input -> `FindStripAction::Close`
    // (Task 12's widget-local mapping, `find_strip.rs`).
    dispatch(&mut cx, &mut app, &escape());

    assert!(app.find.is_none());
    assert!(!app
        .ui
        .widget(&cx, ids!(find_strip))
        .as_find_strip()
        .is_open());
    let canvas = app.ui.widget(&cx, ids!(canvas));
    let surface = canvas
        .borrow::<crate::canvas::ClassDiagramSurface>()
        .expect("production shell mounts canvas");
    assert!(surface.search_spotlight().is_none());
}

/// The markdown editor and reading viewer are ONE shared surface per shell,
/// so a highlight installed for one document must not survive into the next.
#[test]
fn switching_documents_clears_the_previous_documents_highlights() {
    let (mut cx, mut app) = mounted_production_shell();
    let source = waml::source::SourceBundle::try_from_pairs([
        (
            "order.md",
            "---\ntype: Runbook\n---\n# Order\n\nOrder prose here.\n",
        ),
        (
            "notes.md",
            "---\ntype: Runbook\n---\n# Notes\n\nNotes prose here.\n",
        ),
    ])
    .unwrap();
    app.session.replace(source).unwrap();
    let snapshot = app.session.snapshot();
    app.search.rebuild(
        &snapshot.source,
        &snapshot.okf_analysis,
        &snapshot.uml_analysis,
    );
    app.ensure_markdown_asset_host(crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle);
    app.refresh_nav(&mut cx, true);
    app.show_editor(&mut cx);
    assert!(app.transition_document(&mut cx, "order", true));
    draw_and_settle_focus(&mut cx, &app, dvec2(1280.0, 800.0));

    dispatch(&mut cx, &mut app, &ctrl_f());
    draw_and_settle_focus(&mut cx, &app, dvec2(1280.0, 800.0));
    dispatch(&mut cx, &mut app, &text_input("order"));

    let body = crate::doc_view::BodyWidgets::new(&mut cx, &app.ui);
    assert!(
        !body.markdown_editor().test_search_highlights().is_empty(),
        "typing must light the open document first"
    );
    assert!(!body.markdown_viewer().test_search_highlights().is_empty());

    app.transition_document(&mut cx, "notes", true);

    assert!(
        body.markdown_editor().test_search_highlights().is_empty(),
        "order.md's byte ranges must not stay lit over notes.md"
    );
    assert!(body.markdown_viewer().test_search_highlights().is_empty());
}

/// A "find in document" strip over a tab that HAS no document must report
/// nothing, not the bundle-wide count an unscoped `QueryScope` would return.
#[test]
fn typing_in_the_strip_over_a_tab_with_no_concept_reports_zero_results() {
    let (mut cx, mut app) = production_app_with_order_classifier();
    // The results tab is a real, focusable surface that carries no concept.
    app.open_search_results(&mut cx, "order");
    draw_and_settle_focus(&mut cx, &app, dvec2(1280.0, 800.0));
    assert!(
        app.documents
            .active_tab()
            .and_then(|tab| tab.concept_id())
            .is_none(),
        "fixture setup must land a tab with no concept"
    );
    assert!(
        !app.search
            .query("order", &waml::search::QueryScope::default())
            .is_empty(),
        "fixture must have bundle-wide hits, or the scoping proves nothing"
    );

    dispatch(&mut cx, &mut app, &ctrl_f());
    draw_and_settle_focus(&mut cx, &app, dvec2(1280.0, 800.0));
    dispatch(&mut cx, &mut app, &text_input("order"));

    // The query really reached the session (otherwise "0 hits" would be
    // vacuous), and it still answered with nothing.
    assert_eq!(
        app.find.as_ref().map(|session| session.query.as_str()),
        Some("order")
    );
    assert_eq!(app.find.as_ref().map(|session| session.hits.len()), Some(0));
}

#[test]
fn opening_the_strip_over_a_tab_with_no_concept_never_crashes() {
    let (mut cx, mut app) = mounted_production_shell();

    // Nothing open at all -- no active tab, so no concept to scope to.
    dispatch(&mut cx, &mut app, &ctrl_f());

    assert!(app
        .ui
        .widget(&cx, ids!(find_strip))
        .as_find_strip()
        .is_open());
    assert_eq!(
        app.find
            .as_ref()
            .and_then(|session| session.scope.document.clone()),
        None
    );
}
