//! `SourceView` -- the View Source tab body. Renders the subject classifier's
//! raw markdown into the shared `source_view` slot (a scrolling `Markdown`
//! surface fed the verbatim bundle file text; the feed itself is pushed from
//! the shell in `App::sync_active_tab`) and hides the diagram chrome: the
//! canvas is occluded by the opaque slot, the tool dock by
//! `wants_tooldock() == false`, the inspector's element picker explicitly.

use crate::doc_view::{BodyChrome, BodyWidgets, DocView, ViewData, ViewOutcome};
use crate::icons::Icon;
use crate::inspector::Subject;
use makepad_widgets::*;

pub struct SourceView {
    key: String,
}

impl SourceView {
    pub fn new(key: String) -> SourceView {
        SourceView { key }
    }

    fn markdown<'a>(&self, data: ViewData<'a>) -> std::borrow::Cow<'a, str> {
        crate::load::source_for(data.bundle, &self.key)
            .map(std::borrow::Cow::Borrowed)
            .unwrap_or_else(|| std::borrow::Cow::Owned(format!("*No source for `{}`*", self.key)))
    }
}

impl DocView for SourceView {
    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, data: ViewData<'_>) {
        body.show_source(cx);
        let markdown = self.markdown(data);
        body.set_source_markdown(cx, markdown.as_ref());
        let model = data.model;
        if let Some(mut inspector) = body
            .inspector(cx)
            .borrow_mut::<crate::inspector_panel::Inspector>()
        {
            inspector.set_subject(cx, model, Subject::Classifier(self.key.clone()));
            // A source view is not a diagram: no element picker.
            inspector.set_picker_visible(cx, false);
        }
    }

    fn handle(
        &mut self,
        _cx: &mut Cx,
        _body: &BodyWidgets,
        _actions: &Actions,
        _data: ViewData<'_>,
    ) -> ViewOutcome {
        ViewOutcome::default()
    }

    /// Neutral slate, deliberately not the subject's node-kind swatch: a source
    /// tab shows raw text rather than a rendered model view, and the flat grey
    /// says so next to the coloured preview tabs.
    fn chrome(&self) -> BodyChrome {
        BodyChrome {
            tool_dock: false,
            view_bar: false,
            canvas_overlays: false,
            right_dock: Some(Icon::SlidersHorizontal),
        }
    }

    fn tab_accent(&self) -> Option<Vec4> {
        Some(crate::accent::bucket_color(
            crate::node_style::AccentBucket::None,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use waml::model::Model;
    use waml::source::SourceBundle;

    fn data<'a>(model: &'a Model, bundle: &'a SourceBundle) -> ViewData<'a> {
        ViewData {
            model,
            bundle,
            revision: 7,
        }
    }

    #[test]
    fn source_markdown_reads_the_raw_bundle() {
        let model = Model::default();
        let bundle = SourceBundle::try_from_pairs([(
            "shop/order.md".to_string(),
            "# Order\nraw source".to_string(),
        )])
        .unwrap();
        let view = SourceView::new("shop/order".into());

        assert_eq!(view.markdown(data(&model, &bundle)), "# Order\nraw source");
    }

    #[test]
    fn missing_source_keeps_the_existing_italic_fallback() {
        let model = Model::default();
        let bundle = SourceBundle::default();
        let view = SourceView::new("missing".into());

        assert_eq!(
            view.markdown(data(&model, &bundle)),
            "*No source for `missing`*"
        );
    }
}

#[cfg(test)]
mod ownership_contract_tests {
    use super::*;
    use crate::doc_view::DocView;

    #[test]
    fn source_view_is_constructed_with_all_tab_identity() {
        let view = SourceView::new("shop/order".into());

        assert_eq!(
            view.tab_accent(),
            Some(crate::accent::bucket_color(
                crate::node_style::AccentBucket::None,
            ))
        );
    }
}
