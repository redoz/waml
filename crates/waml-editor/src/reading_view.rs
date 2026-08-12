//! The `waml-editor` side of the markdown reading view.
//!
//! Mirrors `SourceView`, but installs a `ReadingDocument` on the
//! `MarkdownViewer` instead of an `InstalledPresentation` on the editor. The
//! two surfaces share the parse, the compile and the styles; they share no
//! layout engine.

use std::sync::Arc;

use makepad_widgets::{event::TouchState, *};
use waml_markdown_editor::presentation::{
    compile_presentation, PresentationPlan, PresentationStyles,
};
use waml_markdown_editor::reading::{
    build_reading_document, caret_for_span, BlockExtensionAppearance, BlockExtensionEventOutcome,
    BlockExtensionStates, MarkdownBlockExtensionHost, ReadingDocument,
};
use waml_markdown_editor::syntax::{TextRange, TextSize};

use crate::doc_view::BodyWidgets;
use crate::editor_session::EditorSessionSnapshot;
use crate::markdown_extensions::{
    EditorMarkdownExtensionHost, MarkdownExtensionLease, SharedMarkdownExtensionHost,
};
use crate::markdown_hosts::{SharedMarkdownAssetHost, WamlCodeHighlightHost};
use crate::source_view::SourceView;

#[cfg(any(target_arch = "wasm32", test))]
struct DeferredFrameSlot<T> {
    token: Option<T>,
}

#[cfg(any(target_arch = "wasm32", test))]
impl<T> Default for DeferredFrameSlot<T> {
    fn default() -> Self {
        Self { token: None }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
impl<T> DeferredFrameSlot<T> {
    fn arm(&mut self, has_work: bool, make_token: impl FnOnce() -> T) {
        if !has_work {
            self.token = None;
        } else if self.token.is_none() {
            self.token = Some(make_token());
        }
    }

    fn take_if(&mut self, matches: impl FnOnce(&T) -> bool) -> bool {
        if self.token.as_ref().is_some_and(matches) {
            self.token = None;
            true
        } else {
            false
        }
    }

    fn invalidate(&mut self) {
        self.token = None;
    }

    fn reactivate(&mut self, has_work: bool, make_token: impl FnOnce() -> T) {
        self.invalidate();
        self.arm(has_work, make_token);
    }

    #[cfg(test)]
    fn token(&self) -> Option<&T> {
        self.token.as_ref()
    }
}

pub struct ReadingView {
    key: String,
    /// `true` once the reader has asked to see the markdown source. The
    /// editor side stays read-only: this toggles RENDERING, not writability.
    showing_source: bool,
    plan: Option<Arc<PresentationPlan>>,
    source: Option<Arc<str>>,
    document: Option<Arc<ReadingDocument>>,
    revision: Option<waml_markdown_editor::syntax::DocumentRevision>,
    handoff_source: Option<TextRange>,
    appearance: BlockExtensionAppearance,
    states: BlockExtensionStates,
    extension_host: SharedMarkdownExtensionHost,
    lease: MarkdownExtensionLease,
    #[cfg(target_arch = "wasm32")]
    next_frame: DeferredFrameSlot<NextFrame>,
    #[cfg(all(target_arch = "wasm32", feature = "browser-test-trace"))]
    browser_trace_generation: u64,
    #[cfg(all(target_arch = "wasm32", feature = "browser-test-trace"))]
    last_browser_trace: Option<(u64, usize, usize, usize)>,
}

impl ReadingView {
    /// `assets` is accepted for symmetry with `SourceView::new_with_asset_host`
    /// and because a future task wires embedded-image assets into the reading
    /// view; this task's viewer does not resolve embedded images yet, so no
    /// lease is opened against it.
    pub fn new_with_extension_host(
        key: String,
        _assets: SharedMarkdownAssetHost,
        extension_host: SharedMarkdownExtensionHost,
    ) -> ReadingView {
        let lease = EditorMarkdownExtensionHost::open_lease(&extension_host);
        ReadingView {
            key,
            showing_source: false,
            plan: None,
            source: None,
            document: None,
            revision: None,
            handoff_source: None,
            appearance: configured_appearance(),
            states: BlockExtensionStates::default(),
            extension_host,
            lease,
            #[cfg(target_arch = "wasm32")]
            next_frame: DeferredFrameSlot::default(),
            #[cfg(all(target_arch = "wasm32", feature = "browser-test-trace"))]
            browser_trace_generation: 0,
            #[cfg(all(target_arch = "wasm32", feature = "browser-test-trace"))]
            last_browser_trace: None,
        }
    }

    pub fn showing_source(&self) -> bool {
        self.showing_source
    }

    pub fn set_showing_source(&mut self, showing: bool) {
        self.showing_source = showing;
    }

    pub fn install_snapshot(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        snapshot: &EditorSessionSnapshot,
    ) {
        let Some((_document, syntax)) = SourceView::resolve_document(snapshot, &self.key) else {
            return;
        };
        let revision = syntax.revision();
        if self.revision == Some(revision) {
            self.reconcile_appearance(cx, body, configured_appearance());
            return;
        }
        let styles = Arc::new(PresentationStyles::balanced());
        let highlighters = WamlCodeHighlightHost::registry(Arc::new(snapshot.clone()));
        // On failure the viewer keeps showing the previous revision; say so,
        // or the stale surface is indistinguishable from a current one.
        let plan = match compile_presentation(&syntax, &styles, &highlighters) {
            Ok(plan) => plan,
            Err(error) => {
                log!(
                    "reading view {}: presentation compile failed, keeping the previous revision: {error:?}",
                    self.key
                );
                return;
            }
        };
        let document = match build_reading_document(&plan, &self.lease.registered_languages()) {
            Ok(document) => document,
            Err(error) => {
                log!(
                    "reading view {}: reading model build failed, keeping the previous revision: {error:?}",
                    self.key
                );
                return;
            }
        };
        let source: Arc<str> = Arc::from(syntax.text().shared().as_str());
        self.plan = Some(plan);
        self.source = Some(source);
        self.revision = Some(revision);
        self.document = Some(Arc::new(document));
        self.appearance = configured_appearance();
        self.reconcile_extensions(cx, body, false);
    }

    pub fn handle_event(&mut self, cx: &mut Cx, body: &BodyWidgets, event: &Event) {
        self.capture_visual_handoff(body, event);
        if matches!(event, Event::LiveEdit) {
            self.reconcile_appearance(cx, body, configured_appearance());
        }

        let changed = self.drain_extension_events();

        #[cfg(target_arch = "wasm32")]
        let changed = {
            let mut changed = changed;
            if self
                .next_frame
                .take_if(|next_frame| next_frame.is_event(event).is_some())
            {
                let _ = self.lease.run_one_deferred();
                changed |= self.drain_extension_events();
            }
            changed
        };

        if changed {
            self.install_extension_frame(cx, body, true);
        }

        #[cfg(target_arch = "wasm32")]
        self.arm_deferred_frame(cx);
    }

    fn reconcile_appearance(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        appearance: BlockExtensionAppearance,
    ) {
        if self.appearance == appearance || self.document.is_none() {
            return;
        }

        let next_lease = EditorMarkdownExtensionHost::open_lease(&self.extension_host);
        let old_lease = std::mem::replace(&mut self.lease, next_lease);
        drop(old_lease);
        self.states = BlockExtensionStates::default();
        self.appearance = appearance;
        self.reconcile_extensions(cx, body, true);
    }

    fn reconcile_extensions(&mut self, cx: &mut Cx, body: &BodyWidgets, preserve_handoff: bool) {
        let (Some(document), Some(source), Some(revision)) =
            (self.document.clone(), self.source.clone(), self.revision)
        else {
            return;
        };
        self.states.reconcile(
            &mut self.lease,
            revision,
            &document,
            source,
            self.appearance,
        );
        #[cfg(all(target_arch = "wasm32", feature = "browser-test-trace"))]
        {
            self.browser_trace_generation = self.browser_trace_generation.wrapping_add(1);
        }
        self.install_extension_frame(cx, body, preserve_handoff);

        // A cache hit is admitted synchronously and does not need a platform
        // wake-up. Install it now so the view cannot stay in Loading until an
        // unrelated input event arrives.
        if self.drain_extension_events() {
            self.install_extension_frame(cx, body, true);
        }

        #[cfg(target_arch = "wasm32")]
        self.arm_deferred_frame(cx);
    }

    fn drain_extension_events(&mut self) -> bool {
        let mut changed = false;
        for event in self.lease.drain_events() {
            changed |= matches!(
                self.states.apply_event(event),
                BlockExtensionEventOutcome::Applied
            );
        }
        changed
    }

    fn capture_visual_handoff(&mut self, body: &BodyWidgets, event: &Event) {
        let point = match event {
            Event::MouseDown(event) if event.button.is_primary() => Some(event.abs),
            Event::TouchUpdate(event) => event
                .touches
                .iter()
                .find(|touch| matches!(touch.state, TouchState::Start))
                .map(|touch| touch.abs),
            _ => None,
        };
        if let Some(point) = point {
            self.handoff_source = body
                .markdown_viewer()
                .borrow()
                .and_then(|viewer| viewer.source_map().visual_source_at(point));
        }
    }

    fn install_extension_frame(&mut self, cx: &mut Cx, body: &BodyWidgets, preserve_handoff: bool) {
        let (Some(plan), Some(document), Some(source), Some(revision)) = (
            self.plan.as_ref(),
            self.document.clone(),
            self.source.clone(),
            self.revision,
        ) else {
            return;
        };
        debug_assert_eq!(plan.revision, revision);
        self.handoff_source = if preserve_handoff {
            body.markdown_viewer()
                .selected_source_span(cx)
                .or(self.handoff_source)
        } else {
            None
        };
        let frame = self.states.frame(revision);
        body.markdown_viewer()
            .install_document(cx, document, source, frame);
        self.trace_browser_pending();
    }

    #[cfg(target_arch = "wasm32")]
    fn arm_deferred_frame(&mut self, cx: &mut Cx) {
        self.next_frame
            .arm(self.lease.has_deferred_work(), || cx.new_next_frame());
    }

    pub(crate) fn caret_for_handoff(&self, cx: &Cx, body: &BodyWidgets) -> TextSize {
        caret_for_span(
            body.markdown_viewer()
                .selected_source_span(cx)
                .or(self.handoff_source),
        )
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn on_activate(&mut self, cx: &mut Cx) {
        self.next_frame
            .reactivate(self.lease.has_deferred_work(), || cx.new_next_frame());
    }

    #[cfg(test)]
    pub(crate) fn extensions_pending_for_test(&self) -> usize {
        self.states.pending_count()
    }

    fn trace_browser_pending(&mut self) {
        #[cfg(all(target_arch = "wasm32", feature = "browser-test-trace"))]
        {
            let Some(revision) = self.revision else {
                return;
            };
            let (ready, failed, loading) = extension_state_totals(&self.states, revision);
            let trace = (self.browser_trace_generation, ready, failed, loading);
            if self.last_browser_trace == Some(trace) {
                return;
            }
            self.last_browser_trace = Some(trace);
            log!(
                "WAML_TEST_EXTENSION_PENDING generation={} count={} ready={} failed={} loading={}",
                trace.0,
                trace.3,
                trace.1,
                trace.2,
                trace.3
            );
        }
    }
}

#[cfg(any(test, all(target_arch = "wasm32", feature = "browser-test-trace")))]
fn extension_state_totals(
    states: &BlockExtensionStates,
    revision: waml_markdown_editor::syntax::DocumentRevision,
) -> (usize, usize, usize) {
    let mut ready = 0;
    let mut failed = 0;
    let mut loading = 0;
    for (_, state) in states.frame(revision).items.iter() {
        match state {
            waml_markdown_editor::reading::BlockExtensionState::Ready(_) => ready += 1,
            waml_markdown_editor::reading::BlockExtensionState::Failed(_) => failed += 1,
            waml_markdown_editor::reading::BlockExtensionState::Loading => loading += 1,
        }
    }
    (ready, failed, loading)
}

fn configured_appearance() -> BlockExtensionAppearance {
    match crate::config::theme() {
        crate::config::ThemeMode::Light => BlockExtensionAppearance::Light,
        crate::config::ThemeMode::Dark => BlockExtensionAppearance::Dark,
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::time::Duration;

    use super::*;
    use waml::source::SourceBundle;
    use waml_markdown_editor::reading::{BlockExtensionState, ReadingBlock};

    fn mounted_body(cx: &mut Cx) -> (WidgetRef, BodyWidgets) {
        waml_markdown_editor::live_design(cx);
        let viewer = WidgetRef::new_with_inner(Box::new(
            cx.with_vm(waml_markdown_editor::reading::MarkdownViewer::script_new_with_default),
        ));
        let mut viewer_body = cx.with_vm(View::script_new_with_default);
        viewer_body.children.push((live_id!(viewer), viewer));
        let viewer_body = WidgetRef::new_with_inner(Box::new(viewer_body));
        let mut viewer_surface = cx.with_vm(View::script_new_with_default);
        viewer_surface
            .children
            .push((live_id!(viewer_body), viewer_body));
        let viewer_surface = WidgetRef::new_with_inner(Box::new(viewer_surface));
        let mut root = cx.with_vm(View::script_new_with_default);
        root.children
            .push((live_id!(markdown_viewer_surface), viewer_surface));
        let ui = WidgetRef::new_with_inner(Box::new(root));
        let body = BodyWidgets::new(cx, &ui);
        (ui, body)
    }

    fn assets() -> SharedMarkdownAssetHost {
        crate::markdown_hosts::EditorMarkdownAssetHost::shared(
            crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle,
        )
    }

    fn snapshot(source: &str) -> Arc<EditorSessionSnapshot> {
        let mut session = crate::editor_session::EditorSession::default();
        session
            .replace(
                SourceBundle::try_from_pairs([("runbook.md", source)])
                    .expect("the test source is valid"),
            )
            .expect("the test bundle is analyzable");
        session.snapshot()
    }

    fn has_visible_text(blocks: &[ReadingBlock], source: &str, expected: &str) -> bool {
        blocks.iter().any(|block| {
            block.pieces.iter().any(|piece| {
                piece.emit
                    && source
                        .get(piece.range.start().to_usize()..piece.range.end().to_usize())
                        .is_some_and(|text| text.contains(expected))
            }) || has_visible_text(&block.children, source, expected)
        })
    }

    fn ready_svg(view: &ReadingView) -> Arc<[u8]> {
        let frame = view.states.frame(view.revision.unwrap());
        match frame.items.as_ref() {
            [(_, BlockExtensionState::Ready(svg))] => svg.data.clone(),
            states => panic!("expected one Ready Mermaid frame, got {states:?}"),
        }
    }

    fn settle_extensions(view: &mut ReadingView, cx: &mut Cx, body: &BodyWidgets) {
        for _ in 0..400 {
            view.handle_event(cx, body, &Event::Signal);
            if view.states.pending_count() == 0 {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!(
            "Mermaid rendering did not settle; {} block(s) remain pending",
            view.states.pending_count()
        );
    }

    #[test]
    fn deferred_frame_slot_keeps_its_token_until_the_matching_event() {
        let mut slot = DeferredFrameSlot::default();
        slot.arm(true, || 1_u64);
        assert_eq!(slot.token(), Some(&1));

        assert!(!slot.take_if(|token| *token == 99));
        slot.arm(true, || 2);
        assert_eq!(
            slot.token(),
            Some(&1),
            "an unrelated event must not replace the queued frame token"
        );

        assert!(slot.take_if(|token| *token == 1));
        slot.arm(true, || 2);
        assert_eq!(slot.token(), Some(&2));
    }

    #[test]
    fn returning_after_a_lost_frame_rearms_and_completes_deferred_work() {
        let mut slot = DeferredFrameSlot::default();
        let mut pending = 1;
        slot.arm(pending > 0, || 1_u64);
        assert_eq!(slot.token(), Some(&1));

        // The view becomes inactive. DocumentHost does not route token 1's
        // event to it, so the slot cannot consume the token.
        slot.reactivate(pending > 0, || 2);
        assert_eq!(
            slot.token(),
            Some(&2),
            "reactivation must replace the token whose event was lost"
        );

        assert!(slot.take_if(|token| *token == 2));
        pending -= 1;
        slot.arm(pending > 0, || 3);
        assert_eq!(pending, 0);
        assert_eq!(slot.token(), None);
    }

    #[test]
    fn mermaid_blocks_open_loading_without_changing_source_or_sibling_prose() {
        let source = "---\ntype: Runbook\n---\n# Runbook\n\nBefore.\n\n```mermaid\nflowchart LR\n    A --> B\n```\n\nAfter.\n";
        let extensions = crate::markdown_extensions::EditorMarkdownExtensionHost::shared();
        let baseline_refs = Rc::strong_count(&extensions);
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let (_ui, body) = mounted_body(&mut cx);
        let mut view =
            ReadingView::new_with_extension_host("runbook".into(), assets(), extensions.clone());

        view.install_snapshot(&mut cx, &body, &snapshot(source));

        assert_eq!(view.source.as_deref(), Some(source));
        assert_eq!(view.states.pending_count(), 1);
        assert!(matches!(
            view.states.frame(view.revision.unwrap()).items.as_ref(),
            [(_, BlockExtensionState::Loading)]
        ));
        let document = view
            .document
            .as_ref()
            .expect("the reading document is installed");
        assert!(
            has_visible_text(&document.roots, source, "Before."),
            "prose before the diagram stays visible"
        );
        assert!(
            has_visible_text(&document.roots, source, "After."),
            "prose after the diagram stays visible"
        );

        drop(view);
        assert_eq!(
            Rc::strong_count(&extensions),
            baseline_refs,
            "dropping the reading view drops and closes its extension lease"
        );
    }

    #[test]
    fn a_non_mermaid_fence_never_requests_the_extension_host() {
        let source = "---\ntype: Runbook\n---\n# Runbook\n\n```rust\nfn main() {}\n```\n";
        let extensions = crate::markdown_extensions::EditorMarkdownExtensionHost::shared();
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let (_ui, body) = mounted_body(&mut cx);
        let mut view = ReadingView::new_with_extension_host("runbook".into(), assets(), extensions);

        view.install_snapshot(&mut cx, &body, &snapshot(source));

        assert_eq!(view.states.pending_count(), 0);
        assert!(view.states.frame(view.revision.unwrap()).items.is_empty());
    }

    #[test]
    fn mermaid_completions_install_ready_and_local_failed_frames() {
        let source = "---\ntype: Runbook\n---\n# Runbook\n\nBefore.\n\n```mermaid\nflowchart LR\n    A --> B\n```\n\nBetween.\n\n```mermaid\nflowchart LR\n    A -->\n```\n\nAfter.\n";
        let extensions = crate::markdown_extensions::EditorMarkdownExtensionHost::shared();
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let (_ui, body) = mounted_body(&mut cx);
        let mut view = ReadingView::new_with_extension_host("runbook".into(), assets(), extensions);

        view.install_snapshot(&mut cx, &body, &snapshot(source));
        assert_eq!(view.states.pending_count(), 2);
        assert_eq!(
            extension_state_totals(&view.states, view.revision.unwrap()),
            (0, 0, 2),
            "the installed loading frame reports all state totals"
        );
        assert!(view
            .states
            .frame(view.revision.unwrap())
            .items
            .iter()
            .all(|(_, state)| matches!(state, BlockExtensionState::Loading)));

        settle_extensions(&mut view, &mut cx, &body);

        let frame = view.states.frame(view.revision.unwrap());
        assert_eq!(
            extension_state_totals(&view.states, view.revision.unwrap()),
            (1, 1, 0),
            "the installed settled frame reports Ready, Failed, and Loading totals"
        );
        assert_eq!(
            frame
                .items
                .iter()
                .filter(|(_, state)| matches!(state, BlockExtensionState::Ready(_)))
                .count(),
            1
        );
        assert_eq!(
            frame
                .items
                .iter()
                .filter(|(_, state)| matches!(state, BlockExtensionState::Failed(_)))
                .count(),
            1
        );
        assert_eq!(view.source.as_deref(), Some(source));
        let document = view
            .document
            .as_ref()
            .expect("the reading document stays installed");
        for prose in ["Before.", "Between.", "After."] {
            assert!(
                has_visible_text(&document.roots, source, prose),
                "a failed Mermaid block must not hide sibling prose `{prose}`"
            );
        }
    }

    #[test]
    fn a_theme_change_replaces_the_lease_and_installs_only_the_new_appearance() {
        let source =
            "---\ntype: Runbook\n---\n# Runbook\n\n```mermaid\nflowchart LR\n    A --> B\n```\n";
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let (_ui, body) = mounted_body(&mut cx);

        let mut baseline = ReadingView::new_with_extension_host(
            "runbook".into(),
            assets(),
            crate::markdown_extensions::EditorMarkdownExtensionHost::shared(),
        );
        baseline.install_snapshot(&mut cx, &body, &snapshot(source));
        let old_appearance = baseline.appearance;
        settle_extensions(&mut baseline, &mut cx, &body);
        let old_svg = ready_svg(&baseline);

        let extensions = crate::markdown_extensions::EditorMarkdownExtensionHost::shared();
        let mut view = ReadingView::new_with_extension_host("runbook".into(), assets(), extensions);
        view.install_snapshot(&mut cx, &body, &snapshot(source));
        let new_appearance = match old_appearance {
            BlockExtensionAppearance::Light => BlockExtensionAppearance::Dark,
            BlockExtensionAppearance::Dark => BlockExtensionAppearance::Light,
        };
        view.reconcile_appearance(&mut cx, &body, new_appearance);

        assert_eq!(view.appearance, new_appearance);
        assert!(matches!(
            view.states.frame(view.revision.unwrap()).items.as_ref(),
            [(_, BlockExtensionState::Loading)]
        ));
        settle_extensions(&mut view, &mut cx, &body);
        let new_svg = ready_svg(&view);
        assert_ne!(
            new_svg, old_svg,
            "the replacement lease must render the requested appearance"
        );

        for _ in 0..20 {
            view.handle_event(&mut cx, &body, &Event::Signal);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            ready_svg(&view),
            new_svg,
            "an old-theme completion must not replace the admitted frame"
        );
    }

    #[test]
    fn two_reading_views_share_the_renderer_cache_but_not_their_leases() {
        let source = "---\ntype: Runbook\n---\n# Runbook\n\n```mermaid\nflowchart LR\n    Cache --> Hit\n```\n";
        let extensions = crate::markdown_extensions::EditorMarkdownExtensionHost::shared();
        let document = snapshot(source);
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let (_ui, body) = mounted_body(&mut cx);
        let mut first =
            ReadingView::new_with_extension_host("runbook".into(), assets(), extensions.clone());
        first.install_snapshot(&mut cx, &body, &document);
        settle_extensions(&mut first, &mut cx, &body);
        let expected = ready_svg(&first);

        let mut second =
            ReadingView::new_with_extension_host("runbook".into(), assets(), extensions);
        second.install_snapshot(&mut cx, &body, &document);

        assert_eq!(
            second.states.pending_count(),
            0,
            "a shared renderer cache hit is admitted during reconciliation"
        );
        assert_eq!(ready_svg(&second), expected);
    }

    #[test]
    fn a_new_mermaid_revision_rejects_the_old_completion() {
        let old_source = "---\ntype: Runbook\n---\n# Runbook\n\n```mermaid\nflowchart LR\n    Old --> Ready\n```\n";
        let new_source =
            "---\ntype: Runbook\n---\n# Runbook\n\n```mermaid\nflowchart LR\n    New -->\n```\n";
        let extensions = crate::markdown_extensions::EditorMarkdownExtensionHost::shared();
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let (_ui, body) = mounted_body(&mut cx);
        let mut view = ReadingView::new_with_extension_host("runbook".into(), assets(), extensions);
        let mut session = crate::editor_session::EditorSession::default();

        session
            .replace(SourceBundle::try_from_pairs([("runbook.md", old_source)]).unwrap())
            .unwrap();
        view.install_snapshot(&mut cx, &body, &session.snapshot());
        let old_revision = view.revision.unwrap();
        let document = *session
            .snapshot()
            .markdown_snapshots
            .keys()
            .next()
            .expect("the runbook Markdown snapshot exists");
        session
            .replace_external(document, old_revision, new_source.to_string())
            .unwrap();
        view.install_snapshot(&mut cx, &body, &session.snapshot());
        let new_revision = view.revision.unwrap();
        assert_ne!(new_revision, old_revision);
        assert_eq!(view.states.pending_count(), 1);

        settle_extensions(&mut view, &mut cx, &body);

        let frame = view.states.frame(new_revision);
        assert_eq!(frame.revision, new_revision);
        assert!(matches!(
            frame.items.as_ref(),
            [(_, BlockExtensionState::Failed(_))]
        ));
        assert_eq!(view.source.as_deref(), Some(new_source));
    }
}
