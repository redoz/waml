//! App-level Ctrl+K wiring (Task 11, spec §Palette): the hotkey opens the
//! palette over a real `PopupRoot`, typing re-queries `SearchState` and
//! pushes fresh sections into the live widget, and committing a row opens
//! the hit's document. Needs `mounted_production_shell` (not the lighter
//! `navigation_app` fixture): `open_palette` reads `main_window` for its
//! bounds and mounts a real `popup_root` with the palette as a DSL child.

use super::*;
use crate::popup::root::PopupRoot;

fn ctrl_k() -> Event {
    Event::KeyDown(KeyEvent {
        key_code: KeyCode::KeyK,
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

fn production_app_with_order_concept() -> (Cx, App) {
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
    (cx, app)
}

#[test]
fn ctrl_k_opens_the_palette_over_the_popup_root() {
    let (mut cx, mut app) = production_app_with_order_concept();

    app.handle_event(&mut cx, &ctrl_k());

    let popup_root = app.ui.widget(&cx, ids!(popup_root));
    let popup = popup_root
        .borrow::<PopupRoot>()
        .expect("production shell mounts popup_root");
    assert!(popup.is_open_for(live_id!(palette)));
}

#[test]
fn typing_a_query_updates_the_palettes_own_sections() {
    let (mut cx, mut app) = production_app_with_order_concept();
    app.handle_event(&mut cx, &ctrl_k());

    let event = text_input("order");
    let actions = cx.capture_actions(|cx| {
        <App as AppMain>::handle_event(&mut app, cx, &event);
    });
    app.handle_action_batch(&mut cx, &actions);

    let popup_root = app.ui.widget(&cx, ids!(popup_root));
    let titles = popup_root
        .borrow::<PopupRoot>()
        .expect("production shell mounts popup_root")
        .test_palette_row_titles(&mut cx);
    assert!(
        titles.iter().any(|title| title.contains("Order")),
        "expected a re-queried row naming the Order concept, got {titles:?}"
    );
}

#[test]
fn enter_on_the_armed_concept_row_opens_that_documents_tab() {
    let (mut cx, mut app) = production_app_with_order_concept();
    app.handle_event(&mut cx, &ctrl_k());

    let typed = text_input("order");
    let actions = cx.capture_actions(|cx| {
        <App as AppMain>::handle_event(&mut app, cx, &typed);
    });
    app.handle_action_batch(&mut cx, &actions);

    let enter = Event::KeyDown(KeyEvent {
        key_code: KeyCode::ReturnKey,
        ..Default::default()
    });
    let actions = cx.capture_actions(|cx| {
        <App as AppMain>::handle_event(&mut app, cx, &enter);
    });
    app.handle_action_batch(&mut cx, &actions);

    assert_eq!(
        app.documents.active_tab().and_then(|tab| tab.concept_id()),
        Some("order")
    );
}
