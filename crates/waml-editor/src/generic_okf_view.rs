use std::borrow::Cow;

use crate::doc_view::{BodyChrome, BodyWidgets, DocView, ViewData, ViewOutcome};
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

    fn idle_outcome() -> ViewOutcome {
        ViewOutcome::default()
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
        _body: &BodyWidgets,
        _actions: &Actions,
        _data: ViewData<'_>,
    ) -> ViewOutcome {
        Self::idle_outcome()
    }

    fn chrome(&self) -> BodyChrome {
        BodyChrome::HIDDEN
    }

    fn tab_accent(&self) -> Option<Vec4> {
        crate::okf_documents::generic_okf_accent()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use waml::source::SourceBundle;

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
        assert_eq!(view.chrome(), BodyChrome::HIDDEN);
        assert_eq!(
            view.tab_accent(),
            crate::okf_documents::generic_okf_accent()
        );
    }

    #[test]
    fn generic_document_emits_no_edits_or_shell_actions() {
        let outcome = GenericOkfView::idle_outcome();
        assert!(outcome.edit.is_none());
        assert!(outcome.popup.is_none());
        assert!(outcome.promote_subject.is_none());
        assert!(!outcome.close_active);
    }
}
