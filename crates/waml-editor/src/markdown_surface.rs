use makepad_widgets::*;

pub fn surface(ui: &WidgetRef, cx: &mut Cx) -> WidgetRef {
    ui.widget(cx, ids!(markdown_surface))
}

pub fn hide(ui: &WidgetRef, cx: &mut Cx) {
    surface(ui, cx).set_visible(cx, false);
}

pub fn show(ui: &WidgetRef, cx: &mut Cx) {
    surface(ui, cx).set_visible(cx, true);
    ui.widget(cx, ids!(canvas_wrap)).set_visible(cx, false);
}

pub fn set_markdown(ui: &WidgetRef, cx: &mut Cx, markdown: &str) {
    ui.widget(cx, ids!(markdown_surface.md))
        .as_markdown()
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
