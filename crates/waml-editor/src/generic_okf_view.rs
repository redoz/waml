use std::borrow::Cow;

use crate::doc_view::{
    BodyChrome, BodyWidgets, DocView, DocumentHeaderChrome, ViewData, ViewOutcome,
};
use crate::navigation::NavigationIntent;
use makepad_widgets::*;

pub struct GenericOkfView {
    concept_id: String,
}

impl GenericOkfView {
    pub fn new(concept_id: String) -> Self {
        Self { concept_id }
    }

    fn markdown<'a>(&self, data: ViewData<'a>) -> Cow<'a, str> {
        data.okf
            .concept(&self.concept_id)
            .map(|concept| Cow::Borrowed(concept.body.as_str()))
            .unwrap_or_else(|| Cow::Owned(format!("*No source for `{}`*", self.concept_id)))
    }
}

impl DocView for GenericOkfView {
    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, data: ViewData<'_>) {
        let markdown = self.markdown(data);
        body.show_markdown(cx);
        body.set_markdown(cx, markdown.as_ref());
    }

    fn handle(
        &mut self,
        _cx: &mut Cx,
        body: &BodyWidgets,
        actions: &Actions,
        _data: ViewData<'_>,
    ) -> ViewOutcome {
        let Some(href) = body.markdown_link(actions) else {
            return ViewOutcome::default();
        };
        ViewOutcome {
            navigation: Some(NavigationIntent::MarkdownLink {
                current_concept_id: self.concept_id.clone(),
                href,
            }),
            ..ViewOutcome::default()
        }
    }

    fn chrome(&self) -> BodyChrome {
        BodyChrome {
            tool_dock: false,
            view_bar: false,
            canvas_overlays: false,
            document_header: DocumentHeaderChrome {
                breadcrumb: true,
                right_dock: None,
            },
        }
    }

    fn tab_accent(&self) -> Option<Vec4> {
        crate::okf_documents::generic_okf_accent()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::navigation::NavigationIntent;
    use waml::source::SourceBundle;

    fn markdown_link_action(href: &str) -> Action {
        Box::new(WidgetAction {
            data: None,
            action: Box::new(MarkdownAction::LinkNavigated(href.into())),
            widget_uid: WidgetUid(1),
            group: None,
        })
    }

    fn fixture(
        pairs: impl IntoIterator<Item = (&'static str, &'static str)>,
    ) -> (SourceBundle, waml::okf::Bundle, waml::uml::Projection) {
        let source = SourceBundle::try_from_pairs(pairs).unwrap();
        let okf = waml::okf::Bundle::parse(&source).unwrap();
        let uml = waml::uml::project(&okf);
        (source, okf, uml)
    }

    fn data<'a>(
        source: &'a SourceBundle,
        okf: &'a waml::okf::Bundle,
        uml: &'a waml::uml::Projection,
    ) -> ViewData<'a> {
        ViewData {
            source,
            okf,
            uml,
            revision: 3,
        }
    }

    #[test]
    fn markdown_renders_the_semantic_concept_body() {
        let (source, okf, uml) = fixture([(
            "runbook.md",
            "---\ntype: vendor.Runbook\ntitle: Recovery\n---\n# Recovery\n\nRestart it.\n",
        )]);
        let view = GenericOkfView::new("runbook".into());
        assert_eq!(
            view.markdown(data(&source, &okf, &uml)),
            "# Recovery\n\nRestart it.\n"
        );
    }

    #[test]
    fn missing_concept_has_an_italic_fallback() {
        let (source, okf, uml) = fixture([]);
        let view = GenericOkfView::new("missing".into());
        assert_eq!(
            view.markdown(data(&source, &okf, &uml)),
            "*No source for `missing`*"
        );
    }

    #[test]
    fn generic_document_hides_all_diagram_chrome_and_has_stable_accent() {
        let view = GenericOkfView::new("runbook".into());
        assert_eq!(
            view.chrome(),
            BodyChrome {
                tool_dock: false,
                view_bar: false,
                canvas_overlays: false,
                document_header: DocumentHeaderChrome {
                    breadcrumb: true,
                    right_dock: None,
                },
            }
        );
        assert_eq!(
            view.tab_accent(),
            crate::okf_documents::generic_okf_accent()
        );
    }

    #[test]
    fn markdown_link_emits_raw_navigation_intent_from_generic_concept() {
        let (source, okf, uml) = fixture([]);
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let body = BodyWidgets::new(&mut cx, &WidgetRef::empty());
        let mut view = GenericOkfView::new("runbook".into());
        let actions: ActionsBuf = vec![markdown_link_action("../customer.md#history")];

        let outcome = view.handle(&mut cx, &body, &actions, data(&source, &okf, &uml));

        assert_eq!(
            outcome.navigation,
            Some(NavigationIntent::MarkdownLink {
                current_concept_id: "runbook".into(),
                href: "../customer.md#history".into(),
            })
        );
    }
}
