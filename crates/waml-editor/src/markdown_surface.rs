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

#[cfg(test)]
mod tests {
    #[test]
    fn app_uses_markdown_neutral_surface_id() {
        let app = include_str!("app.rs");
        assert!(app.contains("markdown_surface := View"));
        assert!(!app.contains("source_view := View"));
    }
}
