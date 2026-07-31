use makepad_widgets::*;

pub fn surface(ui: &WidgetRef, cx: &mut Cx) -> WidgetRef {
    ui.widget(cx, ids!(markdown_surface))
}

pub fn plain_text_child(ui: &WidgetRef, cx: &mut Cx) -> WidgetRef {
    let surface = ui.widget(cx, ids!(markdown_surface));
    let existing = surface.widget(cx, ids!(plain_source));
    if existing.borrow::<TextInput>().is_some() {
        return existing;
    }
    let text = WidgetRef::new_with_inner(Box::new(cx.with_vm(TextInput::script_new_with_default)));
    if let Some(mut view) = surface.borrow_mut::<View>() {
        view.children.push((live_id!(plain_source), text.clone()));
    }
    surface.widget(cx, ids!(md)).set_visible(cx, false);
    text
}

pub fn hide(ui: &WidgetRef, cx: &mut Cx) {
    surface(ui, cx).set_visible(cx, false);
}

pub fn show(ui: &WidgetRef, cx: &mut Cx) {
    surface(ui, cx).set_visible(cx, true);
    ui.widget(cx, ids!(canvas_wrap)).set_visible(cx, false);
}

pub fn set_markdown(ui: &WidgetRef, cx: &mut Cx, markdown: &str) {
    let surface = surface(ui, cx);
    let compatibility = surface.widget(cx, ids!(md));
    compatibility.set_visible(cx, false);
    compatibility.set_text(cx, markdown);
    plain_text_child(ui, cx).set_visible(cx, true);
    plain_text_child(ui, cx)
        .as_text_input()
        .set_text(cx, markdown);
}

pub fn link_navigated(actions: &Actions) -> Option<String> {
    for action in actions {
        let Some(widget_action) = action.downcast_ref::<WidgetAction>() else {
            continue;
        };
        let Some(MarkdownAction::LinkNavigated(href)) =
            widget_action.action.downcast_ref::<MarkdownAction>()
        else {
            continue;
        };
        return Some(href.clone());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_surface_preserves_markdown_and_updates_plain_text() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let markdown =
            WidgetRef::new_with_inner(Box::new(cx.with_vm(Markdown::script_new_with_default)));
        let mut surface = cx.with_vm(View::script_new_with_default);
        surface.children.push((live_id!(md), markdown));
        let mut root = cx.with_vm(View::script_new_with_default);
        root.children.push((
            live_id!(markdown_surface),
            WidgetRef::new_with_inner(Box::new(surface)),
        ));
        let ui = WidgetRef::new_with_inner(Box::new(root));

        super::set_markdown(&ui, &mut cx, "first source");
        let source = super::surface(&ui, &mut cx);

        assert!(source.widget(&cx, ids!(md)).borrow::<Markdown>().is_some());
        assert_eq!(source.widget(&cx, ids!(md)).text(), "first source");
        super::set_markdown(&ui, &mut cx, "second source");
        assert_eq!(
            source
                .widget(&cx, ids!(plain_source))
                .as_text_input()
                .text(),
            "second source"
        );
    }

    fn widget_action(action: impl WidgetActionTrait + 'static) -> Action {
        Box::new(WidgetAction {
            data: None,
            action: Box::new(action),
            widget_uid: WidgetUid(1),
            group: None,
        })
    }

    #[test]
    fn link_navigation_preserves_the_raw_href() {
        let actions: ActionsBuf = vec![widget_action(MarkdownAction::LinkNavigated(
            "../customer.md#history".into(),
        ))];

        assert_eq!(
            link_navigated(&actions),
            Some("../customer.md#history".into())
        );
    }

    #[test]
    fn unrelated_widget_actions_are_ignored() {
        let actions: ActionsBuf = vec![widget_action(ButtonAction::None)];

        assert_eq!(link_navigated(&actions), None);
    }

    #[test]
    fn app_uses_markdown_neutral_surface_id() {
        let app = include_str!("app.rs");
        assert!(app.contains("markdown_surface := View"));
        assert!(!app.contains("source_view := View"));
    }
}
