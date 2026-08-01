use std::sync::Arc;

use makepad_widgets::*;
use waml::analysis::DocumentId;
use waml_markdown_editor::{
    document::MarkdownDocumentSnapshot,
    layout::LayoutInvalidation,
    motion::LayoutChangeCause,
    presentation::{
        build_layout_document, compile_presentation, AssetEventOutcome, EmbeddedAssets,
        HighlighterRegistry, InstalledPresentation, MarkdownAssetHost, PresentationPlan,
        PresentationStyles, PresentedDiagnostic, PresentedDiagnosticSeverity,
    },
    session::{HostSnapshotCause, MarkdownDocumentSession},
    syntax::{parse_markdown, DocumentRevision, MarkdownDialect, SourceText},
    widget::MarkdownEditorRef,
};

use crate::doc_view::{
    BodyChrome, BodyWidgets, DocView, DocViewIdentity, DocumentHeaderChrome, ViewData, ViewOutcome,
    ViewReconcilePolicy,
};
use crate::editor_session::{
    exact_replacement_change, EditorSessionSnapshot, ProposedSourceEdit, SessionChange,
};
use crate::icons::Icon;
use crate::inspector::Subject;
use crate::markdown_hosts::{
    EditorMarkdownAssetHost, MarkdownAssetLease, SharedMarkdownAssetHost, WamlCodeHighlightHost,
};
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
    path: waml::source::BundlePath,
    session: MarkdownDocumentSession,
    plan: Arc<PresentationPlan>,
    styles: Arc<PresentationStyles>,
    assets: EmbeddedAssets,
    diagnostics: Arc<[PresentedDiagnostic]>,
    pending_changes: Option<Arc<[waml_markdown_editor::syntax::TextChange]>>,
}

type CompiledPresentation = (
    Arc<PresentationPlan>,
    Arc<PresentationStyles>,
    EmbeddedAssets,
    Arc<InstalledPresentation>,
);

fn presented_diagnostics_for(
    document: DocumentId,
    syntax: &waml_markdown_editor::syntax::MarkdownSyntaxSnapshot,
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
                waml_markdown_editor::syntax::SyntaxSeverity::Error => {
                    PresentedDiagnosticSeverity::Error
                }
                waml_markdown_editor::syntax::SyntaxSeverity::Warning => {
                    PresentedDiagnosticSeverity::Warning
                }
                waml_markdown_editor::syntax::SyntaxSeverity::Info => {
                    PresentedDiagnosticSeverity::Information
                }
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
    asset_lease: Option<MarkdownAssetLease>,
}

impl SourceView {
    #[cfg(test)]
    pub fn new(key: String) -> SourceView {
        Self::new_with_asset_host(
            key,
            EditorMarkdownAssetHost::shared(
                crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle,
            ),
        )
    }

    pub fn new_with_asset_host(key: String, assets: SharedMarkdownAssetHost) -> SourceView {
        SourceView {
            key,
            read_only: false,
            fragment: None,
            state: SourceViewState::Uninitialized,
            asset_lease: Some(EditorMarkdownAssetHost::open_lease(&assets)),
        }
    }

    pub(crate) fn new_read_only(key: String, assets: SharedMarkdownAssetHost) -> SourceView {
        let mut view = Self::new_with_asset_host(key, assets);
        view.read_only = true;
        view
    }

    pub(crate) fn resolve_document(
        snapshot: &EditorSessionSnapshot,
        key: &str,
    ) -> Option<(
        DocumentId,
        Arc<waml_markdown_editor::syntax::MarkdownSyntaxSnapshot>,
    )> {
        let _ = crate::load::source_for(&snapshot.source, key)?;
        let source = snapshot.source.document_by_concept_id(key)?;
        let document = snapshot.okf_analysis.catalog.id_for_path(source.path())?;
        Some((document, snapshot.markdown_snapshot(document)?.clone()))
    }

    fn compile(
        syntax: &waml_markdown_editor::syntax::MarkdownSyntaxSnapshot,
        diagnostics: Arc<[PresentedDiagnostic]>,
        highlighters: &HighlighterRegistry,
        mut assets: EmbeddedAssets,
        asset_host: Option<(&mut MarkdownAssetLease, &waml::source::BundlePath)>,
    ) -> Result<CompiledPresentation, String> {
        let styles = Arc::new(PresentationStyles::balanced());
        let plan = compile_presentation(syntax, &styles, highlighters)
            .map_err(|error| format!("presentation compile failed: {error:?}"))?;
        if let Some((host, path)) = asset_host {
            host.reconcile_presentation(&plan, path.clone());
            assets.reconcile(host, &plan);
            for event in host.drain_events() {
                let _ = assets.apply_event(event);
            }
        }
        let layout = Arc::new(
            build_layout_document(&plan, &styles, &assets.measurements(f64::INFINITY))
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
        if let SourceViewState::Ready(ready) = &self.state {
            if let Some(lease) = self.asset_lease.as_mut() {
                lease.unbind_document(&ready.path);
            }
        }
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
        match Self::compile(
            &syntax,
            Arc::from([]),
            &HighlighterRegistry::default(),
            EmbeddedAssets::default(),
            None,
        ) {
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
        let Some(path) = workspace
            .source
            .document_by_concept_id(&self.key)
            .map(|source| source.path().clone())
        else {
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
                if cause != HostSnapshotCause::ExternalReplacement {
                    if let Some(changes) = changes {
                        layout_cause = LayoutChangeCause::LocalEdit { changes };
                    }
                }
            }
        }

        let needs_session = !matches!(
            self.state,
            SourceViewState::Ready(ref ready) if ready.document == document
        );
        if needs_session {
            if let SourceViewState::Ready(ready) = &self.state {
                self.asset_lease
                    .as_mut()
                    .expect("SourceView owns its Markdown asset lease")
                    .unbind_document(&ready.path);
            }
            self.state = SourceViewState::Ready(Box::new(ReadySourceView {
                document,
                path: path.clone(),
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
            let highlighters = WamlCodeHighlightHost::registry(Arc::new(workspace.clone()));
            let retained_assets = match &mut self.state {
                SourceViewState::Ready(ready) => std::mem::take(&mut ready.assets),
                _ => EmbeddedAssets::default(),
            };
            let asset_lease = self
                .asset_lease
                .as_mut()
                .expect("SourceView owns its Markdown asset lease");
            let Ok((plan, styles, assets, installed)) = Self::compile(
                &syntax,
                diagnostics.clone(),
                &highlighters,
                retained_assets,
                Some((asset_lease, &path)),
            ) else {
                self.set_missing(cx, &editor);
                return;
            };
            if let SourceViewState::Ready(ready) = &mut self.state {
                ready.session.set_read_only(self.read_only);
                ready.path = path;
                ready.plan = plan;
                ready.styles = styles;
                ready.assets = assets;
                ready.diagnostics = diagnostics;
            }
            editor.set_read_only(cx, self.read_only);
            editor.install_presentation(cx, installed, layout_cause);
        }
    }

    fn apply_asset_events(&mut self, cx: &mut Cx, editor: &MarkdownEditorRef) {
        let events = self
            .asset_lease
            .as_mut()
            .expect("SourceView owns its Markdown asset lease")
            .drain_events();
        for event in events {
            let SourceViewState::Ready(ready) = &mut self.state else {
                continue;
            };
            let AssetEventOutcome::Applied {
                invalidation: Some(LayoutInvalidation::BlockMeasurement(id)),
            } = ready.assets.apply_event(event)
            else {
                continue;
            };
            let layout = match build_layout_document(
                &ready.plan,
                &ready.styles,
                &ready.assets.measurements(f64::INFINITY),
            ) {
                Ok(layout) => Arc::new(layout),
                Err(error) => {
                    log!("image measurement layout failed: {error:?}");
                    continue;
                }
            };
            let installed = match InstalledPresentation::new(
                ready.plan.clone(),
                ready.styles.clone(),
                layout,
                ready.diagnostics.clone(),
                ready.assets.frame(&ready.plan),
            ) {
                Ok(installed) => installed,
                Err(error) => {
                    log!("image measurement install failed: {error:?}");
                    continue;
                }
            };
            editor.install_presentation(cx, installed, LayoutChangeCause::ImageMeasurement(id));
        }
    }

    pub(crate) fn route_editor_event(&mut self, cx: &mut Cx, ui: &WidgetRef, event: &Event) {
        let body = BodyWidgets::new(cx, ui);
        self.apply_asset_events(cx, &body.markdown_editor());
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

    fn sync_external_replacement(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        snapshot: &EditorSessionSnapshot,
    ) {
        self.sync(cx, body, snapshot.borrowed().into());
        if let Some((document, incoming)) = Self::resolve_document(snapshot, &self.key) {
            if let SourceViewState::Ready(ready) = &mut self.state {
                if ready.document == document {
                    ready.pending_changes = Some(Arc::from([exact_replacement_change(
                        ready.session.snapshot().text().shared().as_str(),
                        incoming.text().shared().as_str(),
                    )]));
                }
            }
        }
        self.install_snapshot(cx, body, snapshot, HostSnapshotCause::ExternalReplacement);
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
    use crate::markdown_hosts::{EditorMarkdownAssetHost, MarkdownAssetPolicy};
    use waml::analysis::{DiagnosticSource, RevisionedDiagnostic};
    use waml::diagnostic::Severity;
    use waml::source::SourceBundle;
    use waml_markdown_editor::syntax::TextSize;
    use waml_markdown_editor::{
        input::{EditorInput, ScrollState},
        selection::{Affinity, Selection, SelectionSet, TextPosition},
    };

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

    fn source_view(key: &str) -> SourceView {
        SourceView::new_with_asset_host(
            key.to_owned(),
            EditorMarkdownAssetHost::shared(MarkdownAssetPolicy::BrowserBundle),
        )
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
    fn source_view_compiles_fenced_waml_with_the_snapshot_highlighter() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let ui = mounted_body(&mut cx);
        let body = BodyWidgets::new(&mut cx, &ui);
        let assets = EditorMarkdownAssetHost::shared(MarkdownAssetPolicy::BrowserBundle);
        let mut session = crate::editor_session::EditorSession::default();
        session
            .replace(
                SourceBundle::try_from_pairs([(
                    "runbook.md",
                    "---\ntype: Runbook\n---\n# Runbook\n\n```waml\n## Attributes\n- unknown: Number {0..42}\n```\n",
                )])
                .unwrap(),
            )
            .unwrap();
        let snapshot = session.snapshot();
        let mut view = SourceView::new_with_asset_host("runbook".into(), assets);

        view.install_snapshot(&mut cx, &body, &snapshot, HostSnapshotCause::InitialLoad);

        let SourceViewState::Ready(ready) = &view.state else {
            panic!("the source view must retain its ready presentation");
        };
        assert!(ready.plan.items.iter().any(|item| matches!(
            item,
            waml_markdown_editor::presentation::PresentationItem::TextRun {
                id: waml_markdown_editor::presentation::PresentationItemId {
                    role: waml_markdown_editor::presentation::PresentationRole::Text(
                        waml_markdown_editor::presentation::TextRole::CodeToken(
                            waml_markdown_editor::presentation::CodeTokenRole::Property
                        )
                    ),
                    ..
                },
                ..
            }
        )));
    }

    #[test]
    fn source_view_applies_browser_asset_failure_before_installing_the_plan() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let ui = mounted_body(&mut cx);
        let body = BodyWidgets::new(&mut cx, &ui);
        let assets = EditorMarkdownAssetHost::shared(MarkdownAssetPolicy::BrowserBundle);
        let mut session = crate::editor_session::EditorSession::default();
        session
            .replace(
                SourceBundle::try_from_pairs([(
                    "runbook.md",
                    "---\ntype: Runbook\n---\n# Runbook\n\n![diagram](tiny.svg)\n",
                )])
                .unwrap(),
            )
            .unwrap();
        let snapshot = session.snapshot();
        let mut view = SourceView::new_with_asset_host("runbook".into(), assets);

        view.install_snapshot(&mut cx, &body, &snapshot, HostSnapshotCause::InitialLoad);

        let SourceViewState::Ready(ready) = &view.state else {
            panic!("the source view must retain its ready presentation");
        };
        let image = ready
            .plan
            .items
            .iter()
            .find_map(|item| match item {
                waml_markdown_editor::presentation::PresentationItem::EmbeddedBlock {
                    id, ..
                } => Some(*id),
                _ => None,
            })
            .expect("the image must produce an embedded presentation item");
        assert!(matches!(
            ready.assets.state(image),
            Some(waml_markdown_editor::presentation::EmbeddedState::Failed { .. })
        ));
    }

    #[test]
    fn source_view_applies_async_image_completion_without_mutating_source() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let ui = mounted_body(&mut cx);
        let body = BodyWidgets::new(&mut cx, &ui);
        let fixture_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/markdown-assets");
        let assets = EditorMarkdownAssetHost::shared(
            MarkdownAssetPolicy::native(fixture_root).expect("fixture root must exist"),
        );
        let authored = "---\ntype: Runbook\n---\n# Runbook\n\n![diagram](tiny.svg)\n";
        let mut session = crate::editor_session::EditorSession::default();
        session
            .replace(SourceBundle::try_from_pairs([("runbook.md", authored)]).unwrap())
            .unwrap();
        let snapshot = session.snapshot();
        let mut view = SourceView::new_with_asset_host("runbook".into(), assets);
        view.install_snapshot(&mut cx, &body, &snapshot, HostSnapshotCause::InitialLoad);

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            view.apply_asset_events(&mut cx, &body.markdown_editor());
            let SourceViewState::Ready(ready) = &view.state else {
                panic!("the source view must remain ready");
            };
            if ready
                .assets
                .frame(&ready.plan)
                .items
                .iter()
                .any(|(_, state)| {
                    matches!(
                        state,
                        waml_markdown_editor::presentation::EmbeddedState::Ready { .. }
                    )
                })
            {
                assert_eq!(ready.session.snapshot().text().shared().as_str(), authored);
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "image completion timed out"
            );
            std::thread::yield_now();
        }
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
        let view = source_view("shop/order");
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
        let mut view = source_view("shop/order");
        let ready = source_session().snapshot();
        let (_, syntax) = SourceView::resolve_document(&ready, "shop/order")
            .expect("the ready fixture must resolve");
        SourceView::compile(
            &syntax,
            Arc::from([]),
            &HighlighterRegistry::default(),
            EmbeddedAssets::default(),
            None,
        )
        .expect("the ready fixture must compile");
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
        let mut view = source_view("shop/order");
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
        let mut view = source_view("shop/order");
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
        let mut view = source_view("shop/order");
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
