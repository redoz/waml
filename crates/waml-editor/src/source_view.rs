//! `SourceView` -- the View Source tab body. Renders the subject classifier's
//! raw markdown into the shared `markdown_surface` slot (a scrolling `Markdown`
//! surface fed the verbatim bundle file text; the feed itself is pushed from
//! the shell in `App::sync_active_tab`) and hides the diagram chrome: the
//! canvas is occluded by the opaque slot, while its `BodyChrome` declaration
//! hides the diagram tool dock and the inspector's element picker explicitly.

use crate::doc_view::{
    BodyChrome, BodyWidgets, DocView, DocumentHeaderChrome, ViewData, ViewOutcome,
};
use crate::icons::Icon;
use crate::inspector::Subject;
use crate::navigation::NavigationIntent;
use crate::view_history::ViewAnchor;
use makepad_widgets::*;

pub struct SourceView {
    key: String,
    fragment: Option<String>,
}

impl SourceView {
    pub fn new(key: String) -> SourceView {
        SourceView {
            key,
            fragment: None,
        }
    }

    fn markdown<'a>(&self, data: ViewData<'a>) -> std::borrow::Cow<'a, str> {
        crate::load::source_for(data.source, &self.key)
            .map(std::borrow::Cow::Borrowed)
            .unwrap_or_else(|| std::borrow::Cow::Owned(format!("*No source for `{}`*", self.key)))
    }
}

impl DocView for SourceView {
    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, data: ViewData<'_>) {
        body.show_markdown(cx);
        let markdown = self.markdown(data);
        body.set_markdown(cx, markdown.as_ref());
        if let Some(mut inspector) = body
            .inspector(cx)
            .borrow_mut::<crate::inspector_panel::Inspector>()
        {
            inspector.set_subject_analysis(
                cx,
                data.uml_analysis,
                Subject::Classifier(self.key.clone()),
            );
            // A source view is not a diagram: no element picker.
            inspector.set_picker_visible(cx, false);
        }
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
                current_concept_id: self.key.clone(),
                href,
            }),
            ..ViewOutcome::default()
        }
    }

    /// Neutral slate, deliberately not the subject's node-kind swatch: a source
    /// tab shows raw text rather than a rendered model view, and the flat grey
    /// says so next to the coloured preview tabs.
    fn chrome(&self) -> BodyChrome {
        BodyChrome {
            tool_dock: false,
            view_bar: false,
            canvas_overlays: false,
            document_header: DocumentHeaderChrome {
                breadcrumb: true,
                right_dock: Some(Icon::SlidersHorizontal),
            },
        }
    }

    fn tab_accent(&self) -> Option<Vec4> {
        Some(crate::accent::bucket_color(
            crate::node_style::AccentBucket::None,
        ))
    }

    fn capture_anchor(&self, body: &BodyWidgets) -> ViewAnchor {
        ViewAnchor::Markdown {
            fragment: self.fragment.clone(),
            scroll_y: body.markdown_scroll_y(),
        }
    }

    fn restore_anchor(&mut self, cx: &mut Cx, body: &BodyWidgets, anchor: &ViewAnchor) -> bool {
        let ViewAnchor::Markdown { fragment, scroll_y } = anchor else {
            return false;
        };
        self.fragment = fragment
            .as_deref()
            .filter(|fragment| body.scroll_markdown_to_fragment(cx, fragment))
            .map(str::to_owned);
        body.set_markdown_scroll_y(cx, *scroll_y);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::navigation::NavigationIntent;
    use waml::source::SourceBundle;

    fn markdown_link_action(widget_uid: WidgetUid, href: &str) -> Action {
        Box::new(WidgetAction {
            data: None,
            action: Box::new(MarkdownAction::LinkNavigated(href.into())),
            widget_uid,
            group: None,
        })
    }

    fn mounted_body(cx: &mut Cx) -> (WidgetRef, BodyWidgets, WidgetUid) {
        cx.widget_tree_mark_dirty(WidgetUid(0));
        let markdown =
            WidgetRef::new_with_inner(Box::new(cx.with_vm(Markdown::script_new_with_default)));
        let markdown_uid = markdown.widget_uid();
        let mut surface = cx.with_vm(View::script_new_with_default);
        surface.children.push((live_id!(md), markdown));
        let mut root = cx.with_vm(View::script_new_with_default);
        root.children.push((
            live_id!(markdown_surface),
            WidgetRef::new_with_inner(Box::new(surface)),
        ));
        let ui = WidgetRef::new_with_inner(Box::new(root));
        let body = BodyWidgets::new(cx, &ui);
        (ui, body, markdown_uid)
    }

    fn data<'a>(
        source: &'a SourceBundle,
        okf_analysis: &'a waml::analysis::OkfAnalysis,
        uml_analysis: &'a waml::uml::Analysis,
    ) -> ViewData<'a> {
        ViewData {
            source,
            okf_analysis,
            uml_analysis,
            revision: 7,
        }
    }

    #[test]
    fn source_markdown_reads_the_raw_bundle() {
        let source = SourceBundle::try_from_pairs([(
            "shop/order.md".to_string(),
            "# Order\nraw source".to_string(),
        )])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source.clone(), None, 7).unwrap();
        let view = SourceView::new("shop/order".into());

        assert_eq!(
            view.markdown(data(&source, prepared.okf(), prepared.uml())),
            "# Order\nraw source"
        );
    }

    #[test]
    fn missing_source_keeps_the_existing_italic_fallback() {
        let source = SourceBundle::default();
        let prepared = waml::analysis::prepare_candidate(source.clone(), None, 7).unwrap();
        let view = SourceView::new("missing".into());

        assert_eq!(
            view.markdown(data(&source, prepared.okf(), prepared.uml())),
            "*No source for `missing`*"
        );
    }

    #[test]
    fn mounted_markdown_link_emits_raw_navigation_intent_from_source_subject() {
        let source = SourceBundle::try_from_pairs([
            (
                "shop/order.md",
                "---\ntype: vendor.Runbook\n---\n# Order\n\n[Next](./next.md#details)\n",
            ),
            (
                "shop/next.md",
                "---\ntype: vendor.Runbook\n---\n# Next\n\n## Details\n",
            ),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source.clone(), None, 1).unwrap();
        let (_, okf_analysis, uml_analysis, _) = prepared.into_parts();
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let (ui, body, markdown_uid) = mounted_body(&mut cx);
        let mut view = SourceView::new("shop/order".into());
        view.sync(&mut cx, &body, data(&source, &okf_analysis, &uml_analysis));
        assert!(ui
            .widget(&cx, ids!(markdown_surface.md))
            .text()
            .contains("[Next](./next.md#details)"));
        let actions: ActionsBuf = vec![markdown_link_action(markdown_uid, "./next.md#details")];

        let outcome = view.handle(
            &mut cx,
            &body,
            &actions,
            data(&source, &okf_analysis, &uml_analysis),
        );

        assert_eq!(
            outcome.navigation,
            Some(NavigationIntent::MarkdownLink {
                current_concept_id: "shop/order".into(),
                href: "./next.md#details".into(),
            })
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
