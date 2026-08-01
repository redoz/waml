use std::sync::Arc;

use makepad_widgets::*;
use waml::analysis::DocumentId;
use waml_markdown_editor::{
    document::MarkdownDocumentSnapshot,
    motion::LayoutChangeCause,
    presentation::{
        build_layout_document, compile_presentation, EmbeddedAssets, EmbeddedMeasurements,
        HighlighterRegistry, InstalledPresentation, PresentationPlan, PresentationStyles,
        PresentedDiagnostic, PresentedDiagnosticSeverity,
    },
    session::{HostSnapshotCause, MarkdownDocumentSession},
    syntax::{parse_markdown, DocumentRevision, MarkdownDialect, SourceText},
    widget::MarkdownEditorRef,
};

use crate::doc_view::{
    BodyChrome, BodyWidgets, DocView, DocViewIdentity, DocumentHeaderChrome, ViewData, ViewOutcome,
    ViewReconcilePolicy,
};
use crate::editor_session::{EditorSessionSnapshot, ProposedSourceEdit, SessionChange};
use crate::icons::Icon;
use crate::inspector::Subject;
use crate::navigation::NavigationIntent;
use crate::view_history::ViewAnchor;

enum SourceViewState {
    Uninitialized,
    Ready(Box<ReadySourceView>),
    Missing(Box<MissingSourceView>),
}

struct MissingSourceView {
    message: Arc<str>,
    session: MarkdownDocumentSession,
}

struct ReadySourceView {
    document: DocumentId,
    session: MarkdownDocumentSession,
    plan: Arc<PresentationPlan>,
    styles: Arc<PresentationStyles>,
    assets: EmbeddedAssets,
    diagnostics: Arc<[PresentedDiagnostic]>,
    pending_changes: Option<Arc<[waml_syntax::TextChange]>>,
}

type CompiledPresentation = (
    Arc<PresentationPlan>,
    Arc<PresentationStyles>,
    EmbeddedAssets,
    Arc<InstalledPresentation>,
);

fn presented_diagnostics_for(
    document: DocumentId,
    syntax: &waml_syntax::MarkdownSyntaxSnapshot,
    semantic: &[waml::analysis::RevisionedDiagnostic],
) -> Arc<[PresentedDiagnostic]> {
    let revision = syntax.revision();
    let mut diagnostics = syntax
        .diagnostics()
        .iter()
        .map(|diagnostic| PresentedDiagnostic {
            revision,
            range: diagnostic.range,
            severity: match diagnostic.severity {
                waml_syntax::SyntaxSeverity::Error => PresentedDiagnosticSeverity::Error,
                waml_syntax::SyntaxSeverity::Warning => PresentedDiagnosticSeverity::Warning,
                waml_syntax::SyntaxSeverity::Info => PresentedDiagnosticSeverity::Information,
            },
            message: diagnostic.message.clone(),
        })
        .collect::<Vec<_>>();
    diagnostics.extend(
        semantic
            .iter()
            .filter(|diagnostic| diagnostic.document == document && diagnostic.revision == revision)
            .map(|diagnostic| PresentedDiagnostic {
                revision,
                range: diagnostic.range,
                severity: match diagnostic.severity {
                    waml::diagnostic::Severity::Error => PresentedDiagnosticSeverity::Error,
                    waml::diagnostic::Severity::Warning => PresentedDiagnosticSeverity::Warning,
                },
                message: diagnostic.message.clone(),
            }),
    );
    diagnostics.into()
}

pub struct SourceView {
    key: String,
    read_only: bool,
    fragment: Option<String>,
    state: SourceViewState,
}

impl SourceView {
    pub fn new(key: String) -> SourceView {
        SourceView {
            key,
            read_only: false,
            fragment: None,
            state: SourceViewState::Uninitialized,
        }
    }

    pub(crate) fn new_read_only(key: String) -> SourceView {
        SourceView {
            key,
            read_only: true,
            fragment: None,
            state: SourceViewState::Uninitialized,
        }
    }

    pub(crate) fn resolve_document(
        snapshot: &EditorSessionSnapshot,
        key: &str,
    ) -> Option<(DocumentId, Arc<waml_syntax::MarkdownSyntaxSnapshot>)> {
        let _ = crate::load::source_for(&snapshot.source, key)?;
        let source = snapshot.source.document_by_concept_id(key)?;
        let document = snapshot.okf_analysis.catalog.id_for_path(source.path())?;
        Some((document, snapshot.markdown_snapshot(document)?.clone()))
    }

    fn compile(
        syntax: &waml_syntax::MarkdownSyntaxSnapshot,
        diagnostics: Arc<[PresentedDiagnostic]>,
    ) -> Result<CompiledPresentation, String> {
        let styles = Arc::new(PresentationStyles::balanced());
        let plan = compile_presentation(syntax, &styles, &HighlighterRegistry::default())
            .map_err(|error| format!("presentation compile failed: {error:?}"))?;
        let assets = EmbeddedAssets::default();
        let layout = Arc::new(
            build_layout_document(&plan, &styles, &EmbeddedMeasurements::default())
                .map_err(|error| format!("presentation layout failed: {error:?}"))?,
        );
        let installed = InstalledPresentation::new(
            plan.clone(),
            styles.clone(),
            layout,
            diagnostics,
            assets.frame(&plan),
        )
        .map_err(|error| format!("presentation install failed: {error:?}"))?;
        Ok((plan, styles, assets, installed))
    }

    fn set_missing(&mut self, cx: &mut Cx, editor: &MarkdownEditorRef) {
        let message: Arc<str> = format!("No source for '{}'", self.key).into();
        let text = SourceText::new(format!("# Source unavailable\n\n{message}\n"))
            .expect("the static missing-source message is valid UTF-8 source text");
        let syntax = parse_markdown(
            DocumentRevision::INITIAL,
            text,
            MarkdownDialect::WAML_DEFAULT,
        )
        .expect("the static missing-source message is valid markdown");
        let snapshot = Arc::new(MarkdownDocumentSnapshot::new(syntax.clone()));
        let mut session = MarkdownDocumentSession::new(snapshot);
        session.set_read_only(true);
        match Self::compile(&syntax, Arc::from([])) {
            Ok((_, _, _, installed)) => {
                editor.install_presentation(cx, installed, LayoutChangeCause::ExternalReplacement)
            }
            Err(error) => {
                log!("missing-source presentation failed: {error}");
                editor.clear_presentation(cx);
            }
        }
        editor.set_read_only(cx, true);
        self.state = SourceViewState::Missing(Box::new(MissingSourceView { message, session }));
    }

    pub(crate) fn install_snapshot(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        workspace: &EditorSessionSnapshot,
        host_cause: HostSnapshotCause,
    ) {
        body.show_markdown_editor(cx);
        let editor = body.markdown_editor();
        let Some((document, syntax)) = Self::resolve_document(workspace, &self.key) else {
            self.set_missing(cx, &editor);
            return;
        };
        let incoming = Arc::new(MarkdownDocumentSnapshot::new(syntax.clone()));
        let diagnostics = presented_diagnostics_for(
            document,
            &syntax,
            workspace.uml_analysis.revisioned_diagnostics(),
        );

        let mut layout_cause = match host_cause {
            HostSnapshotCause::InitialLoad => LayoutChangeCause::InitialLoad,
            HostSnapshotCause::ExternalReplacement => LayoutChangeCause::ExternalReplacement,
            HostSnapshotCause::AcknowledgedLocalEdit | HostSnapshotCause::ApplicationHistory => {
                LayoutChangeCause::ExternalReplacement
            }
        };
        if let SourceViewState::Ready(ready) = &mut self.state {
            if ready.document == document {
                let changes = ready.pending_changes.take();
                let cause = if ready.session.local_revision() == incoming.revision() {
                    HostSnapshotCause::AcknowledgedLocalEdit
                } else {
                    host_cause
                };
                if let Err(error) =
                    ready
                        .session
                        .synchronize_from_host(incoming.clone(), changes.as_deref(), cause)
                {
                    log!("source host synchronization failed: {error:?}");
                    return;
                }
                if let Some(changes) = changes {
                    layout_cause = LayoutChangeCause::LocalEdit { changes };
                }
            }
        }

        let needs_session = !matches!(
            self.state,
            SourceViewState::Ready(ref ready) if ready.document == document
        );
        if needs_session {
            self.state = SourceViewState::Ready(Box::new(ReadySourceView {
                document,
                session: {
                    let mut session = MarkdownDocumentSession::new(incoming);
                    session.set_read_only(self.read_only);
                    session
                },
                plan: Arc::new(PresentationPlan {
                    revision: syntax.revision(),
                    source_len: syntax.text().len(),
                    items: Arc::from([]),
                    links: Arc::from([]),
                    blocks: Arc::from([]),
                    diagnostics: Arc::from([]),
                }),
                styles: Arc::new(PresentationStyles::balanced()),
                assets: EmbeddedAssets::default(),
                diagnostics: Arc::from([]),
                pending_changes: None,
            }));
            layout_cause = LayoutChangeCause::InitialLoad;
        }

        let should_compile = matches!(
            &self.state,
            SourceViewState::Ready(ready) if ready.plan.revision != syntax.revision()
                || ready.plan.items.is_empty()
                || ready.diagnostics != diagnostics
        );
        if should_compile {
            let Ok((plan, styles, assets, installed)) = Self::compile(&syntax, diagnostics.clone())
            else {
                self.set_missing(cx, &editor);
                return;
            };
            if let SourceViewState::Ready(ready) = &mut self.state {
                ready.session.set_read_only(self.read_only);
                ready.plan = plan;
                ready.styles = styles;
                ready.assets = assets;
                ready.diagnostics = diagnostics;
            }
            editor.set_read_only(cx, self.read_only);
            editor.install_presentation(cx, installed, layout_cause);
        }
    }

    pub(crate) fn route_editor_event(&mut self, cx: &mut Cx, ui: &WidgetRef, event: &Event) {
        match &mut self.state {
            SourceViewState::Ready(ready) => {
                ui.handle_event(cx, event, &mut Scope::with_data(&mut ready.session));
            }
            SourceViewState::Missing(missing) => {
                ui.handle_event(cx, event, &mut Scope::with_data(&mut missing.session));
            }
            SourceViewState::Uninitialized => {
                ui.handle_event(cx, event, &mut Scope::empty());
            }
        }
    }
}

impl DocView for SourceView {
    fn identity(&self) -> DocViewIdentity {
        DocViewIdentity::Source
    }

    fn reconcile_policy(&self) -> ViewReconcilePolicy {
        ViewReconcilePolicy::RetainLiveState
    }

    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, data: ViewData<'_>) {
        body.show_markdown_editor(cx);
        if let Some(mut inspector) = body
            .inspector(cx)
            .borrow_mut::<crate::inspector_panel::Inspector>()
        {
            inspector.set_subject_analysis(
                cx,
                data.uml_analysis,
                Subject::Classifier(self.key.clone()),
            );
            inspector.set_picker_visible(cx, false);
        }
    }

    fn sync_from_session(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        snapshot: &EditorSessionSnapshot,
    ) {
        self.sync(cx, body, snapshot.borrowed().into());
        self.install_snapshot(cx, body, snapshot, HostSnapshotCause::InitialLoad);
    }

    fn after_session_snapshot(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        snapshot: &EditorSessionSnapshot,
        change: SessionChange,
    ) {
        let cause = if change.source_changed {
            HostSnapshotCause::ApplicationHistory
        } else {
            HostSnapshotCause::AcknowledgedLocalEdit
        };
        self.install_snapshot(cx, body, snapshot, cause);
    }

    fn route_ui_event(&mut self, cx: &mut Cx, ui: &WidgetRef, event: &Event) {
        self.route_editor_event(cx, ui, event);
    }

    fn handle(
        &mut self,
        _cx: &mut Cx,
        _body: &BodyWidgets,
        actions: &Actions,
        _data: ViewData<'_>,
    ) -> ViewOutcome {
        let mut outcome = ViewOutcome::default();
        if let SourceViewState::Ready(ready) = &mut self.state {
            if let Some(local) = MarkdownEditorRef::proposed_edit(actions) {
                ready.pending_changes = Some(Arc::from(local.edit.changes.clone()));
                outcome.source_edit = Some(ProposedSourceEdit::from_local(ready.document, local));
            }
            if let Some(position) = MarkdownEditorRef::navigation_requested(actions) {
                if let Some(link) = ready.plan.links.iter().find(|link| {
                    link.source_range.start() <= position.offset
                        && position.offset <= link.source_range.end()
                }) {
                    outcome.navigation = Some(NavigationIntent::MarkdownLink {
                        current_concept_id: self.key.clone(),
                        href: link.destination.to_string(),
                    });
                }
            }
        }
        outcome
    }

    fn chrome(&self) -> BodyChrome {
        BodyChrome {
            tool_dock: false,
            view_bar: false,
            canvas_overlays: false,
            document_header: DocumentHeaderChrome {
                breadcrumb: true,
                right_dock: Some(Icon::PanelRight),
            },
        }
    }

    fn tab_accent(&self) -> Option<Vec4> {
        Some(crate::accent::bucket_color(
            crate::node_style::AccentBucket::None,
        ))
    }

    fn capture_anchor(&self, _body: &BodyWidgets) -> ViewAnchor {
        let session = match &self.state {
            SourceViewState::Ready(ready) => &ready.session,
            SourceViewState::Missing(missing) => {
                let _ = missing.message.len();
                &missing.session
            }
            SourceViewState::Uninitialized => return ViewAnchor::None,
        };
        ViewAnchor::Markdown {
            fragment: self.fragment.clone(),
            revision: session.local_revision(),
            selection: session.selections().clone(),
            scroll: *session.scroll_state(),
        }
    }

    fn restore_anchor(
        &mut self,
        _cx: &mut Cx,
        _body: &BodyWidgets,
        _data: ViewData<'_>,
        anchor: &ViewAnchor,
    ) -> bool {
        let ViewAnchor::Markdown {
            fragment,
            revision,
            selection,
            scroll,
        } = anchor
        else {
            return false;
        };
        if selection.revision() != *revision {
            return false;
        }
        if let Some(fragment) = fragment {
            let SourceViewState::Ready(ready) = &self.state else {
                return false;
            };
            let syntax = ready.session.snapshot().syntax();
            let found = syntax.queries().headings().any(|heading| {
                syntax
                    .text()
                    .slice(heading.content_range)
                    .is_ok_and(|text| text.trim().eq_ignore_ascii_case(fragment))
            });
            if !found {
                return false;
            }
        }
        self.fragment = fragment.clone();
        let session = match &mut self.state {
            SourceViewState::Ready(ready) => &mut ready.session,
            SourceViewState::Missing(missing) => &mut missing.session,
            SourceViewState::Uninitialized => return false,
        };
        if session.local_revision() == *revision {
            if session.set_selections(selection.clone()).is_err() {
                return false;
            }
            session.set_scroll_state(*scroll);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use waml::analysis::{DiagnosticSource, RevisionedDiagnostic};
    use waml::diagnostic::Severity;
    use waml::source::SourceBundle;
    use waml_markdown_editor::{
        input::{EditorInput, ScrollState},
        selection::{Affinity, Selection, SelectionSet, TextPosition},
    };
    use waml_syntax::TextSize;

    fn mounted_body(cx: &mut Cx) -> WidgetRef {
        waml_markdown_editor::live_design(cx);
        let markdown = WidgetRef::new_with_inner(Box::new(
            cx.with_vm(waml_markdown_editor::widget::MarkdownEditor::script_new_with_default),
        ));
        let mut surface = cx.with_vm(View::script_new_with_default);
        surface.children.push((live_id!(editor), markdown));
        let surface = WidgetRef::new_with_inner(Box::new(surface));
        let mut root = cx.with_vm(View::script_new_with_default);
        root.children.push((live_id!(markdown_surface), surface));
        WidgetRef::new_with_inner(Box::new(root))
    }

    fn draw_markdown_widget(cx: &mut Cx, ui: &WidgetRef, session: &mut MarkdownDocumentSession) {
        let draw_event = DrawEvent {
            redraw_all: true,
            ..DrawEvent::default()
        };
        let pass = DrawPass::new_with_name(cx, "source-view-draw-test");
        let mut draw_list = DrawList2d::new(cx);
        let mut draw_cx = CxDraw::new(cx, &draw_event);
        draw_cx.begin_pass(&pass, None);
        draw_list.begin_always(&mut draw_cx);
        {
            let mut cx_2d = Cx2d::new(&mut draw_cx);
            cx_2d.begin_root_turtle(dvec2(640.0, 480.0), Layout::default());
            ui.widget(&cx_2d, ids!(markdown_surface.editor))
                .draw_walk_all(&mut cx_2d, &mut Scope::with_data(session), Walk::fill());
            cx_2d.end_turtle();
            draw_list.end(&mut cx_2d);
        }
        draw_cx.end_pass(&pass);
    }

    fn source_session() -> crate::editor_session::EditorSession {
        let mut session = crate::editor_session::EditorSession::default();
        session
            .replace(
                SourceBundle::try_from_pairs([(
                    "shop/order.md",
                    "---\ntype: Runbook\ntitle: Order\n---\n# Order\nBody\n",
                )])
                .unwrap(),
            )
            .unwrap();
        session
    }

    #[test]
    fn presented_diagnostics_include_current_syntax_and_only_current_document_semantics() {
        let mut session = crate::editor_session::EditorSession::default();
        session
            .replace(
                SourceBundle::try_from_pairs([
                    ("shop/order.md", "# Order\n"),
                    ("shop/customer.md", "# Customer\n"),
                ])
                .unwrap(),
            )
            .unwrap();
        let workspace = session.snapshot();
        let order = workspace
            .okf_analysis
            .catalog
            .id_for_path(
                workspace
                    .source
                    .document_by_concept_id("shop/order")
                    .unwrap()
                    .path(),
            )
            .unwrap();
        let customer = workspace
            .okf_analysis
            .catalog
            .id_for_path(
                workspace
                    .source
                    .document_by_concept_id("shop/customer")
                    .unwrap()
                    .path(),
            )
            .unwrap();
        let revision = DocumentRevision::new(7);
        let syntax = parse_markdown(
            revision,
            SourceText::new("```waml\n".to_owned()).unwrap(),
            MarkdownDialect::WAML_DEFAULT,
        )
        .unwrap();
        assert!(
            !syntax.diagnostics().is_empty(),
            "the fixture must carry one immediate syntax diagnostic"
        );
        let semantic = [
            RevisionedDiagnostic {
                document: order,
                revision,
                range: syntax.diagnostics()[0].range,
                source: DiagnosticSource::Semantic,
                severity: Severity::Warning,
                code: Arc::from("current"),
                message: Arc::from("current semantic"),
            },
            RevisionedDiagnostic {
                document: order,
                revision: DocumentRevision::new(6),
                range: syntax.diagnostics()[0].range,
                source: DiagnosticSource::Semantic,
                severity: Severity::Error,
                code: Arc::from("stale"),
                message: Arc::from("stale semantic"),
            },
            RevisionedDiagnostic {
                document: customer,
                revision,
                range: syntax.diagnostics()[0].range,
                source: DiagnosticSource::Semantic,
                severity: Severity::Error,
                code: Arc::from("other"),
                message: Arc::from("other document"),
            },
        ];

        let presented = presented_diagnostics_for(order, &syntax, &semantic);

        assert_eq!(presented.len(), syntax.diagnostics().len() + 1);
        assert!(presented
            .iter()
            .any(|diagnostic| diagnostic.message.as_ref() == "current semantic"));
        assert!(!presented.iter().any(|diagnostic| {
            matches!(
                diagnostic.message.as_ref(),
                "stale semantic" | "other document"
            )
        }));
        assert!(presented
            .iter()
            .all(|diagnostic| diagnostic.revision == revision));
    }

    #[test]
    fn nested_source_resolves_to_the_immutable_application_snapshot() {
        let mut session = crate::editor_session::EditorSession::default();
        session
            .replace(
                SourceBundle::try_from_pairs([(
                    "shop/order.md",
                    "---\ntype: uml.Class\n---\n# Order\n",
                )])
                .unwrap(),
            )
            .unwrap();
        let snapshot = session.snapshot();

        let (document, syntax) = SourceView::resolve_document(&snapshot, "shop/order").unwrap();

        assert!(Arc::ptr_eq(
            snapshot.markdown_snapshot(document).unwrap(),
            &syntax
        ));
        assert_eq!(
            syntax.text().shared().as_str(),
            "---\ntype: uml.Class\n---\n# Order\n"
        );
    }

    #[test]
    fn source_views_declare_live_state_retention() {
        let view = SourceView::new("shop/order".into());
        assert_eq!(
            view.reconcile_policy(),
            ViewReconcilePolicy::RetainLiveState
        );
    }

    #[test]
    fn mounted_ready_view_replaces_old_content_with_a_read_only_missing_presentation() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let ui = mounted_body(&mut cx);
        let body = BodyWidgets::new(&mut cx, &ui);
        let mut view = SourceView::new("shop/order".into());
        let ready = source_session().snapshot();
        let (_, syntax) = SourceView::resolve_document(&ready, "shop/order")
            .expect("the ready fixture must resolve");
        SourceView::compile(&syntax, Arc::from([])).expect("the ready fixture must compile");
        view.install_snapshot(&mut cx, &body, &ready, HostSnapshotCause::InitialLoad);
        assert!(matches!(view.state, SourceViewState::Ready(_)));

        let missing = crate::editor_session::EditorSession::default().snapshot();
        view.install_snapshot(
            &mut cx,
            &body,
            &missing,
            HostSnapshotCause::ExternalReplacement,
        );

        let SourceViewState::Missing(missing) = &mut view.state else {
            panic!("the mounted source view must enter its explicit missing state");
        };
        assert_eq!(missing.message.as_ref(), "No source for 'shop/order'");
        assert!(missing.session.is_read_only());
        let text = missing.session.snapshot().text().shared().as_str();
        assert!(text.contains("Source unavailable"));
        assert!(text.contains("No source for 'shop/order'"));
        assert!(!text.contains("# Order"));
        let actions = body
            .markdown_editor()
            .handle_input_with_session(
                &mut cx,
                &mut missing.session,
                EditorInput::Text(Arc::from("X")),
            )
            .unwrap();
        assert!(MarkdownEditorRef::proposed_edit(&actions).is_none());
    }

    #[test]
    fn standard_widget_draw_paints_ready_and_missing_presentations() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let ui = mounted_body(&mut cx);
        let body = BodyWidgets::new(&mut cx, &ui);
        let editor = body.markdown_editor();
        editor.set_paint_evidence_enabled(true);
        let mut view = SourceView::new("shop/order".into());
        let ready = source_session().snapshot();
        view.install_snapshot(&mut cx, &body, &ready, HostSnapshotCause::InitialLoad);

        let SourceViewState::Ready(ready) = &mut view.state else {
            panic!("the source view must retain its ready session");
        };
        draw_markdown_widget(&mut cx, &ui, &mut ready.session);

        let ready_generation = editor.test_paint_evidence_generation();
        assert!(ready_generation > 0);
        assert!(!editor.test_painted_text_ranges().is_empty());

        let missing = crate::editor_session::EditorSession::default().snapshot();
        view.install_snapshot(
            &mut cx,
            &body,
            &missing,
            HostSnapshotCause::ExternalReplacement,
        );
        let SourceViewState::Missing(missing) = &mut view.state else {
            panic!("the source view must retain its explicit missing presentation");
        };
        draw_markdown_widget(&mut cx, &ui, &mut missing.session);

        assert!(editor.test_paint_evidence_generation() > ready_generation);
        assert!(!editor.test_painted_text_ranges().is_empty());
        let SourceViewState::Missing(missing) = &view.state else {
            panic!("the source view must retain its explicit missing presentation");
        };
        let text = missing.session.snapshot().text().shared().as_str();
        assert!(text.contains("No source for 'shop/order'"));
        assert!(!text.contains("# Order"));
    }

    #[test]
    fn exact_revision_anchor_restores_full_selection_and_scroll() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let ui = mounted_body(&mut cx);
        let body = BodyWidgets::new(&mut cx, &ui);
        let mut view = SourceView::new("shop/order".into());
        let workspace = source_session().snapshot();
        view.install_snapshot(&mut cx, &body, &workspace, HostSnapshotCause::InitialLoad);
        let SourceViewState::Ready(ready) = &mut view.state else {
            panic!("the source view must be ready");
        };
        let position = |offset, affinity| TextPosition::new(TextSize::new(offset), affinity);
        let selections = SelectionSet::from_selections(
            ready.session.snapshot(),
            vec![
                Selection::new(position(2, Affinity::Before), position(6, Affinity::After)),
                Selection::caret(position(10, Affinity::Before)),
            ],
            1,
        )
        .unwrap();
        ready.session.set_selections(selections.clone()).unwrap();
        let scroll = ScrollState { x: 12.0, y: 84.0 };
        ready.session.set_scroll_state(scroll);
        let anchor = view.capture_anchor(&body);

        let SourceViewState::Ready(ready) = &mut view.state else {
            unreachable!();
        };
        ready.session.set_primary_offset(TextSize::new(0)).unwrap();
        ready.session.set_scroll_state(ScrollState::default());
        assert!(view.restore_anchor(&mut cx, &body, workspace.borrowed().into(), &anchor,));

        let SourceViewState::Ready(ready) = &view.state else {
            unreachable!();
        };
        assert_eq!(ready.session.selections(), &selections);
        assert_eq!(*ready.session.scroll_state(), scroll);
    }

    #[test]
    fn different_revision_anchor_keeps_the_retained_live_selection_and_scroll() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let ui = mounted_body(&mut cx);
        let body = BodyWidgets::new(&mut cx, &ui);
        let mut view = SourceView::new("shop/order".into());
        let workspace = source_session().snapshot();
        view.install_snapshot(&mut cx, &body, &workspace, HostSnapshotCause::InitialLoad);
        let SourceViewState::Ready(ready) = &mut view.state else {
            panic!("the source view must be ready");
        };
        ready.session.set_primary_offset(TextSize::new(7)).unwrap();
        let retained_selection = ready.session.selections().clone();
        let retained_scroll = ScrollState { x: 3.0, y: 45.0 };
        ready.session.set_scroll_state(retained_scroll);
        let anchor = ViewAnchor::markdown_start(
            ready.session.local_revision().checked_next().unwrap(),
            None,
            ScrollState::default(),
        );

        assert!(view.restore_anchor(&mut cx, &body, workspace.borrowed().into(), &anchor,));

        let SourceViewState::Ready(ready) = &view.state else {
            unreachable!();
        };
        assert_eq!(ready.session.selections(), &retained_selection);
        assert_eq!(*ready.session.scroll_state(), retained_scroll);
    }
}
