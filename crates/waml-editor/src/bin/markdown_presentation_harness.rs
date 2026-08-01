//! Native verification harness for markdown presentation and motion.
//!
//! Run with `--case <name>`. The capture workflow in `tests/README.md`
//! waits for the case-specific ready marker before it captures this window.

use std::{path::PathBuf, sync::Arc};

use makepad_widgets::*;
use waml_markdown_editor::presentation::assets::{
    FAILED_HEIGHT, FAILED_MAX_WIDTH, LOADING_HEIGHT, LOADING_MAX_WIDTH,
};
use waml_markdown_editor::{
    document::MarkdownDocumentSnapshot,
    layout::{LayoutElementId, LayoutSnapshot, MeasuredBlock},
    motion::{LayoutChangeCause, MotionConfig, MotionController},
    presentation::{
        build_draw_commands, build_layout_document, compile_presentation, ApprovedImageSource,
        BlockDecorationRole, CodeHighlightError, CodeHighlightHost, CodeHighlightRequest,
        CodeHighlightResult, CodeHighlightSpan, CodeTokenRole, DrawCommand, EmbeddedAssetFrame,
        EmbeddedMeasurements, EmbeddedState, HighlighterRegistry, ImageMediaType,
        InstalledPresentation, PresentationFrame, PresentationItem, PresentationItemId,
        PresentationPlan, PresentationStyles, PresentedDiagnostic, PresentedDiagnosticSeverity,
    },
    selection::{Affinity, Selection, SelectionSet, TextPosition},
    session::MarkdownDocumentSession,
    syntax::{
        parse_markdown, reparse_markdown, DocumentRevision, MarkdownDialect,
        MarkdownSyntaxSnapshot, SourceText, TextChange, TextRange, TextSize,
    },
    widget::{MarkdownEditorRef, MarkdownEditorWidgetRefExt},
};

#[path = "../fonts.rs"]
mod fonts;
#[path = "../theme_atlas.rs"]
mod theme_atlas;

const PRESENTATION_ALL: &str =
    include_str!("../../../waml-markdown-editor/tests/fixtures/presentation-all.md");
const MALFORMED: &str = include_str!("../../../waml-markdown-editor/tests/fixtures/malformed.md");
const MOTION_BEFORE: &str =
    include_str!("../../../waml-markdown-editor/tests/fixtures/motion-before.md");
const MOTION_AFTER: &str =
    include_str!("../../../waml-markdown-editor/tests/fixtures/motion-after.md");
const CHECKER_SVG: &[u8] =
    include_bytes!("../../../waml-markdown-editor/tests/fixtures/checker.svg");
const CHECKER_LINE: &str = "![checker](checker.svg \"fixture\")";
const MOTION_INSERTION: &str = "An inserted paragraph.\n\n";

app_main!(App);

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*
    use mod.atlas
    use mod.fonts

    mod.widgets.PresentationHarnessSurfaceBase = #(PresentationHarnessSurface::register_widget(vm))
    mod.widgets.PresentationHarnessSurface = set_type_default() do mod.widgets.PresentationHarnessSurfaceBase{
        width: Fill
        height: Fill
        flow: Down
        editor: MarkdownEditor{
            width: Fill
            height: Fill
            draw_text_sans +: {text_style: fonts.text_body}
            draw_text_sans_italic +: {text_style: fonts.text_body}
            draw_text_sans_semibold +: {text_style: fonts.text_heading}
            draw_text_sans_semibold_italic +: {text_style: fonts.text_heading}
            draw_text_mono +: {text_style: fonts.text_mono}
            draw_text_mono_italic +: {text_style: fonts.text_mono}
            draw_text_mono_semibold +: {text_style: fonts.text_mono}
            draw_text_mono_semibold_italic +: {text_style: fonts.text_mono}
            motion_duration: 0.100
            motion_ease: OutCubic
            body_color: atlas.text
            marker_color: atlas.text_dim
            marker_active_color: atlas.accent
            link_color: atlas.accent
            diagnostic_color: atlas.accent
            quote_fill: atlas.surface
            code_fill: atlas.surface
            table_fill: atlas.surface
            inline_code_fill: atlas.surface
            block_rule_color: atlas.text_dim
            selection_color: atlas.selection
            caret_color: atlas.text
        }
    }

    use mod.widgets.PresentationHarnessSurface

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                pass.clear_color: atlas.surface
                window.inner_size: vec2(1280, 900)
                window.title: "WAML markdown presentation verification"
                body +: {
                    surface := PresentationHarnessSurface{}
                }
            }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct PresentationHarnessSurface {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[redraw]
    #[rust]
    area: Area,
    #[rust]
    draw_state: DrawStateWrap<()>,
    #[find]
    #[redraw]
    #[live]
    editor: WidgetRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,
}

impl Widget for PresentationHarnessSurface {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let Some(session) = scope.data.get_mut::<MarkdownDocumentSession>() else {
            return;
        };
        self.editor
            .as_markdown_editor()
            .handle_event_with_session(cx, event, session)
            .unwrap_or_else(|error| panic!("markdown harness event failed: {error:?}"));
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.draw_state.begin(cx, ()) {
            cx.begin_turtle(walk, self.layout);
        }
        if self.draw_state.get().is_some() {
            let Some(session) = scope.data.get_mut::<MarkdownDocumentSession>() else {
                self.draw_state.end();
                cx.end_turtle_with_area(&mut self.area);
                return DrawStep::done();
            };
            let editor = self.editor.as_markdown_editor();
            let editor_walk = self.editor.walk(cx);
            let mut child_scope = Scope::empty();
            editor
                .borrow_mut()
                .expect("markdown harness editor is mounted")
                .draw_walk_with_session(cx, session, &mut child_scope, editor_walk)
                .unwrap_or_else(|error| panic!("markdown harness draw failed: {error:?}"))?;
            cx.end_turtle_with_area(&mut self.area);
            self.draw_state.end();
        }
        DrawStep::done()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HarnessCase {
    Headings,
    Inline,
    Lists,
    Quotes,
    Code,
    Tables,
    Images,
    Invalid,
    Selection,
    MotionStart,
    MotionMid,
    MotionEnd,
}

impl HarnessCase {
    const NAMES: &'static str = "headings|inline|lists|quotes|code|tables|images|invalid|selection|motion-start|motion-mid|motion-end";

    fn from_args() -> Self {
        let mut args = std::env::args().skip(1);
        let flag = args.next();
        let value = args.next();
        if flag.as_deref() != Some("--case") || args.next().is_some() {
            panic!(
                "usage: markdown_presentation_harness --case <{}>",
                Self::NAMES
            );
        }
        match value.as_deref() {
            Some("headings") => Self::Headings,
            Some("inline") => Self::Inline,
            Some("lists") => Self::Lists,
            Some("quotes") => Self::Quotes,
            Some("code") => Self::Code,
            Some("tables") => Self::Tables,
            Some("images") => Self::Images,
            Some("invalid") => Self::Invalid,
            Some("selection") => Self::Selection,
            Some("motion-start") => Self::MotionStart,
            Some("motion-mid") => Self::MotionMid,
            Some("motion-end") => Self::MotionEnd,
            _ => panic!("unknown --case; expected one of {}", Self::NAMES),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Headings => "headings",
            Self::Inline => "inline",
            Self::Lists => "lists",
            Self::Quotes => "quotes",
            Self::Code => "code",
            Self::Tables => "tables",
            Self::Images => "images",
            Self::Invalid => "invalid",
            Self::Selection => "selection",
            Self::MotionStart => "motion-start",
            Self::MotionMid => "motion-mid",
            Self::MotionEnd => "motion-end",
        }
    }

    fn motion_sample_time(self) -> Option<f64> {
        match self {
            Self::MotionStart => Some(0.0),
            Self::MotionMid => Some(0.05),
            Self::MotionEnd => Some(0.10),
            _ => None,
        }
    }
}

struct PreparedPresentation {
    installed: Arc<InstalledPresentation>,
    session: MarkdownDocumentSession,
    source: Arc<str>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum HarnessStage {
    #[default]
    Uninitialized,
    StaticWarm,
    StaticFinalRedraw,
    MotionBeforeWarm,
    MotionAfterWarm,
    MotionFrozenRedraw,
    ReadyMarker,
    Ready,
}

#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
    #[rust]
    kick: NextFrame,
    #[rust]
    stage: HarnessStage,
    #[rust]
    case: Option<HarnessCase>,
    #[rust]
    session: Option<MarkdownDocumentSession>,
    #[rust]
    pending_after: Option<PreparedPresentation>,
    #[rust]
    before_layout: Option<Arc<LayoutSnapshot>>,
    #[rust]
    motion_target_layout: Option<Arc<LayoutSnapshot>>,
    #[rust]
    before_source: Option<Arc<str>>,
    #[rust]
    ready_path: Option<PathBuf>,
    #[rust]
    final_redraw_count: u8,
    #[rust]
    installed: Option<Arc<InstalledPresentation>>,
    #[rust]
    ready_after_generation: Option<u64>,
    #[rust]
    target_only: bool,
}

impl App {
    fn editor(&self, cx: &Cx) -> MarkdownEditorRef {
        self.ui
            .widget(cx, ids!(surface))
            .borrow::<PresentationHarnessSurface>()
            .expect("presentation harness surface is mounted")
            .editor
            .as_markdown_editor()
    }

    fn advance(&mut self, cx: &mut Cx) {
        let editor = self.editor(cx);
        match self.stage {
            HarnessStage::StaticWarm => {
                let Some(target) = editor
                    .target_layout()
                    .filter(|layout| !layout.glyph_clusters().is_empty())
                else {
                    editor.redraw(cx);
                    self.kick = cx.new_next_frame();
                    return;
                };
                if self.target_only {
                    self.before_layout = Some(target.clone());
                    self.motion_target_layout = Some(target);
                }
                editor.redraw(cx);
                self.kick = cx.new_next_frame();
                self.stage = HarnessStage::StaticFinalRedraw;
            }
            HarnessStage::StaticFinalRedraw | HarnessStage::MotionFrozenRedraw => {
                if self.final_redraw_count < 3 {
                    self.final_redraw_count += 1;
                    editor.redraw(cx);
                    self.kick = cx.new_next_frame();
                    return;
                }
                self.ready_after_generation = Some(editor.test_paint_evidence_generation());
                editor.redraw(cx);
                self.kick = cx.new_next_frame();
                self.stage = HarnessStage::ReadyMarker;
            }
            HarnessStage::ReadyMarker => {
                let previous_generation = self
                    .ready_after_generation
                    .expect("ready marker waits for a requested native draw");
                if editor.test_paint_evidence_generation() <= previous_generation {
                    self.kick = cx.new_next_frame();
                    return;
                }
                let layout = editor
                    .frame_layout()
                    .expect("final frame layout was presented");
                assert!(
                    !layout.glyph_clusters().is_empty(),
                    "ready marker requires drawable text geometry"
                );
                self.assert_ready_evidence(layout, &editor);
                self.write_ready_marker();
                self.kick = NextFrame::default();
                self.stage = HarnessStage::Ready;
            }
            HarnessStage::MotionBeforeWarm => {
                let Some(before) = editor
                    .target_layout()
                    .filter(|layout| !layout.glyph_clusters().is_empty())
                else {
                    editor.redraw(cx);
                    self.kick = cx.new_next_frame();
                    return;
                };
                self.before_layout = Some(before);
                let after = self
                    .pending_after
                    .take()
                    .expect("motion-after presentation is prepared");
                self.session = Some(after.session);
                self.installed = Some(after.installed.clone());
                editor.install_presentation(
                    cx,
                    after.installed,
                    LayoutChangeCause::ExternalReplacement,
                );
                self.kick = cx.new_next_frame();
                self.stage = HarnessStage::MotionAfterWarm;
            }
            HarnessStage::MotionAfterWarm => {
                let Some(after) = editor
                    .target_layout()
                    .filter(|layout| !layout.glyph_clusters().is_empty())
                else {
                    editor.redraw(cx);
                    self.kick = cx.new_next_frame();
                    return;
                };
                let before = self
                    .before_layout
                    .clone()
                    .expect("motion-before layout is retained for interpolation");
                let before_source = self
                    .before_source
                    .take()
                    .expect("motion-before source is retained");
                let after_source: Arc<str> = self
                    .session
                    .as_ref()
                    .expect("motion-after session is active")
                    .snapshot()
                    .text()
                    .shared()
                    .as_str()
                    .into();
                let change = motion_change(&before_source, &after_source);
                let before_offset = unique_range(&before_source, "One paragraph that stays.").start;
                let after_offset = unique_range(&after_source, "One paragraph that stays.").start;
                let before_cluster = before
                    .glyph_clusters()
                    .iter()
                    .find(|cluster| {
                        cluster.source_range.start().to_usize() <= before_offset
                            && before_offset < cluster.source_range.end().to_usize()
                    })
                    .expect("motion-before stable text has glyph geometry");
                let after_cluster = after
                    .glyph_clusters()
                    .iter()
                    .find(|cluster| {
                        cluster.source_range.start().to_usize() <= after_offset
                            && after_offset < cluster.source_range.end().to_usize()
                    })
                    .expect("motion-after stable text has glyph geometry");
                assert_eq!(
                    before_cluster.id, after_cluster.id,
                    "incremental motion fixtures preserve the stable paragraph identity"
                );
                let stable_id = before_cluster.id;
                let before_y = before_cluster.rect.pos.y;
                let after_y = after_cluster.rect.pos.y;
                self.motion_target_layout = Some(after.clone());
                let mut motion = MotionController::new(900.0);
                motion.commit(
                    0.0,
                    Some(before),
                    after,
                    LayoutChangeCause::LocalEdit {
                        changes: Arc::from([change]),
                    },
                    false,
                    None,
                    MotionConfig {
                        duration_seconds: 0.100,
                        ..MotionConfig::default()
                    },
                );
                let sampled = motion.sample(
                    self.case
                        .and_then(HarnessCase::motion_sample_time)
                        .expect("motion case has a fixed sample time"),
                );
                let sample_time = self
                    .case
                    .and_then(HarnessCase::motion_sample_time)
                    .expect("motion case has a fixed sample time");
                let progress = (sample_time / 0.100).clamp(0.0, 1.0);
                let eased = 1.0 - (1.0 - progress).powi(3);
                let expected_y = before_y + (after_y - before_y) * eased;
                let sampled_y = sampled
                    .layout
                    .glyph_clusters()
                    .iter()
                    .find(|cluster| cluster.id == stable_id)
                    .expect("sample keeps the stable paragraph identity")
                    .rect
                    .pos
                    .y;
                assert!(
                    (sampled_y - expected_y).abs() < 1.0e-6,
                    "sampled stable paragraph y={sampled_y}, expected {expected_y}"
                );
                editor.test_set_layout(sampled.layout);
                editor.redraw(cx);
                self.kick = cx.new_next_frame();
                self.stage = HarnessStage::MotionFrozenRedraw;
            }
            HarnessStage::Uninitialized | HarnessStage::Ready => {}
        }
    }

    fn write_ready_marker(&self) {
        let path = self
            .ready_path
            .as_ref()
            .expect("ready marker path was initialized");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .unwrap_or_else(|error| panic!("create ready directory failed: {error}"));
        }
        std::fs::write(path, b"ready\n")
            .unwrap_or_else(|error| panic!("write ready marker failed: {error}"));
    }

    fn assert_ready_evidence(&self, layout: Arc<LayoutSnapshot>, editor: &MarkdownEditorRef) {
        let installed = self
            .installed
            .as_ref()
            .expect("ready evidence retains the installed presentation");
        let session = self
            .session
            .as_ref()
            .expect("ready evidence retains the document session");
        let frame = PresentationFrame {
            revision: installed.revision,
            layout: layout.clone(),
            active_owners: Arc::from([]),
            diagnostics: installed.diagnostics.clone(),
            assets: installed.assets.clone(),
        };
        let commands = build_draw_commands(
            &frame,
            &installed.plan,
            &installed.styles,
            session.selections(),
            session.ime(),
        )
        .expect("ready evidence builds the exact native draw plan");
        let source_text = session.snapshot().text().shared();
        let source = source_text.as_str();
        let case = self.case.expect("ready evidence has a case");
        if case == HarnessCase::Images || case.motion_sample_time().is_some() {
            assert_ready_embeds_below_literal_source(&commands, &installed.plan, &layout);
        }
        match case {
            HarnessCase::Code => {
                let start = source.rfind("```").expect("code case has a closing fence");
                assert_text_bytes_are_painted(&commands, start..start + 3);
                assert_source_ranges_cover_bytes(
                    &editor.test_painted_text_ranges(),
                    start..start + 3,
                    "closing fence has three rasterized native glyphs",
                );
            }
            HarnessCase::Tables => {
                let rule = unique_line_range(source, "---");
                assert_text_bytes_are_painted(&commands, rule);
                assert!(
                    commands.iter().any(|command| matches!(
                        command,
                        DrawCommand::BlockBackground {
                            rect,
                            role: BlockDecorationRole::ThematicRule,
                            ..
                        } if rect.size.y == installed.styles.spacing().quote_rule
                    )),
                    "the thematic decoration is a narrow horizontal rule"
                );
            }
            HarnessCase::MotionStart | HarnessCase::MotionMid | HarnessCase::MotionEnd => {
                let ready = commands
                    .iter()
                    .filter_map(|command| match command {
                        DrawCommand::EmbeddedBlock {
                            rect,
                            state: EmbeddedState::Ready { .. },
                            ..
                        } => Some(*rect),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    ready.len(),
                    1,
                    "each motion phase paints one checker primitive"
                );
                assert_eq!(ready[0].pos.x, 24.0, "checker uses the left content inset");
                assert_eq!(ready[0].size, dvec2(96.0, 48.0));
                assert_motion_paint_and_overlay_evidence(MotionEvidence {
                    case,
                    source,
                    session,
                    commands: &commands,
                    layout: &layout,
                    painted_ranges: &editor.test_painted_text_ranges(),
                    before: self
                        .before_layout
                        .as_deref()
                        .expect("motion evidence retains the before layout"),
                    target: self
                        .motion_target_layout
                        .as_deref()
                        .expect("motion evidence retains the exact target layout"),
                });
            }
            _ => {}
        }
    }
}

impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        let case = HarnessCase::from_args();
        let ready_path = PathBuf::from(format!(
            r"C:\tmp\markdown-presentation-verification\{}.ready",
            case.name()
        ));
        let _ = std::fs::remove_file(&ready_path);

        let editor = self.editor(cx);
        editor.set_read_only(cx, true);
        editor.set_paint_evidence_enabled(true);
        self.target_only = case == HarnessCase::MotionEnd
            && matches!(
                std::env::var("WAML_MARKDOWN_HARNESS_TARGET_ONLY").as_deref(),
                Ok("1")
            );
        if let Some(sample_time) = case.motion_sample_time() {
            let _ = sample_time;
            let before_source = motion_source(MOTION_BEFORE);
            let after_source = motion_source(MOTION_AFTER);
            let before_syntax = parse_fixture(&before_source, DocumentRevision::INITIAL);
            let change = motion_change(&before_source, &after_source);
            let after_syntax = reparse_markdown(
                &before_syntax,
                DocumentRevision::new(2),
                SourceText::new(after_source.clone()).expect("motion-after source is valid UTF-8"),
                std::slice::from_ref(&change),
            )
            .expect("motion-after fixture reparses incrementally")
            .snapshot;
            let before = prepare_syntax(case, Arc::from(before_source), before_syntax);
            let after = prepare_syntax(case, Arc::from(after_source), after_syntax);
            if self.target_only {
                self.session = Some(after.session);
                self.installed = Some(after.installed.clone());
                editor.install_presentation(cx, after.installed, LayoutChangeCause::InitialLoad);
                self.stage = HarnessStage::StaticWarm;
            } else {
                self.before_source = Some(before.source.clone());
                self.session = Some(before.session);
                self.installed = Some(before.installed.clone());
                self.pending_after = Some(after);
                editor.install_presentation(cx, before.installed, LayoutChangeCause::InitialLoad);
                self.stage = HarnessStage::MotionBeforeWarm;
            }
        } else {
            let source = source_for(case);
            let prepared = prepare(case, &source, DocumentRevision::INITIAL);
            self.session = Some(prepared.session);
            self.installed = Some(prepared.installed.clone());
            editor.install_presentation(cx, prepared.installed, LayoutChangeCause::InitialLoad);
            if case == HarnessCase::Selection {
                editor.set_key_focus(cx);
            }
            self.stage = HarnessStage::StaticWarm;
        }
        self.case = Some(case);
        self.ready_path = Some(ready_path);
        self.kick = cx.new_next_frame();
    }
}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        makepad_widgets::script_mod(vm);
        crate::theme_atlas::script_mod(vm);
        crate::fonts::script_mod(vm);
        waml_markdown_editor::script_mod(vm);
        self::script_mod(vm)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        if self.kick.is_event(event).is_some() {
            self.advance(cx);
        }
        self.match_event(cx, event);
        if let Some(session) = self.session.as_mut() {
            self.ui
                .handle_event(cx, event, &mut Scope::with_data(session));
        }
    }
}

struct WamlHighlighter;

impl CodeHighlightHost for WamlHighlighter {
    fn highlight(
        &self,
        request: &CodeHighlightRequest,
    ) -> Result<CodeHighlightResult, CodeHighlightError> {
        let start = request.content_range.start().to_usize();
        let spans = [
            (0, 4, CodeTokenRole::Keyword),
            (6, 9, CodeTokenRole::Type),
            (9, 10, CodeTokenRole::Punctuation),
            (10, 15, CodeTokenRole::Type),
        ]
        .into_iter()
        .map(|(from, to, role)| CodeHighlightSpan {
            range: text_range(start + from, start + to),
            role,
        })
        .collect::<Vec<_>>();
        Ok(CodeHighlightResult {
            revision: request.revision,
            owner: request.owner,
            spans: spans.into(),
        })
    }
}

fn source_for(case: HarnessCase) -> String {
    match case {
        HarnessCase::Headings => fixture_section(
            PRESENTATION_ALL,
            "# Heading *em* and **strong**",
            "###### Heading 6",
        ),
        HarnessCase::Inline => fixture_section(
            PRESENTATION_ALL,
            "Body with ~~strike~~, [label](./other.md#part), `inline`, and <kbd>raw</kbd>.",
            "Body with ~~strike~~, [label](./other.md#part), `inline`, and <kbd>raw</kbd>.",
        ),
        HarnessCase::Lists => fixture_section(PRESENTATION_ALL, "- bullet", "- [ ] open"),
        HarnessCase::Quotes => {
            fixture_section(PRESENTATION_ALL, "> quoted **text**", "> quoted **text**")
        }
        HarnessCase::Code => fixture_section(PRESENTATION_ALL, "```waml", "```"),
        HarnessCase::Tables => {
            fixture_section(PRESENTATION_ALL, "| left | center | right |", "---")
        }
        HarnessCase::Images => {
            let line = unique_line(PRESENTATION_ALL, CHECKER_LINE);
            format!("{line}\n{line}\n{line}\n")
        }
        HarnessCase::Invalid => MALFORMED.to_owned(),
        HarnessCase::Selection => fixture_section(
            PRESENTATION_ALL,
            "### Heading 3 with ***strong emphasis***",
            "### Heading 3 with ***strong emphasis***",
        ),
        HarnessCase::MotionStart | HarnessCase::MotionMid | HarnessCase::MotionEnd => {
            unreachable!("motion sources are prepared separately")
        }
    }
}

fn fixture_section(source: &str, first_line: &str, last_line: &str) -> String {
    let first = unique_line_range(source, first_line);
    let last = unique_line_range(source, last_line);
    assert!(
        first.start <= last.start,
        "fixture section lines must stay in source order"
    );
    let mut selected = source[first.start..last.end].to_owned();
    selected.push('\n');
    selected
}

fn motion_source(fixture: &str) -> String {
    let mut source = fixture.to_owned();
    if !source.ends_with('\n') {
        source.push('\n');
    }
    source.push_str(CHECKER_LINE);
    source.push('\n');
    source
}

fn motion_change(before: &str, after: &str) -> TextChange {
    let insertion = unique_range(after, MOTION_INSERTION);
    let mut without_insertion = after.to_owned();
    without_insertion.replace_range(insertion.clone(), "");
    assert_eq!(
        without_insertion, before,
        "motion fixtures differ only by the checked insertion"
    );
    TextChange {
        old_range: text_range(insertion.start, insertion.start),
        replacement: Arc::from(MOTION_INSERTION),
    }
}

fn parse_fixture(source: &str, revision: DocumentRevision) -> Arc<MarkdownSyntaxSnapshot> {
    parse_markdown(
        revision,
        SourceText::new(source.to_owned()).expect("fixture source is valid UTF-8"),
        MarkdownDialect::WAML_DEFAULT,
    )
    .expect("fixture parses with the WAML markdown dialect")
}

fn prepare(case: HarnessCase, source: &str, revision: DocumentRevision) -> PreparedPresentation {
    let source: Arc<str> = Arc::from(source);
    let syntax = parse_fixture(&source, revision);
    prepare_syntax(case, source, syntax)
}

fn prepare_syntax(
    case: HarnessCase,
    source: Arc<str>,
    syntax: Arc<MarkdownSyntaxSnapshot>,
) -> PreparedPresentation {
    let revision = syntax.revision();
    let styles = Arc::new(PresentationStyles::balanced());
    let mut highlighters = HighlighterRegistry::default();
    if case == HarnessCase::Code {
        highlighters.register("waml", Arc::new(WamlHighlighter));
    }
    let plan = compile_presentation(&syntax, &styles, &highlighters)
        .expect("fixture compiles into a presentation plan");
    let (assets, measurements) = assets_for(case, &plan);
    let layout_document = Arc::new(
        build_layout_document(&plan, &styles, &measurements)
            .expect("presentation plan builds a layout document"),
    );
    let insets = layout_document.content_insets;
    assert_eq!(
        [insets.top, insets.right, insets.bottom, insets.left],
        [24.0; 4],
        "native presentation keeps a 24px inset on every side"
    );
    for item in plan.items.iter() {
        let PresentationItem::TextRun {
            range, role, style, ..
        } = item
        else {
            continue;
        };
        if style.active_color == style.color {
            continue;
        }
        let run = layout_document
            .text_runs
            .iter()
            .find(|run| run.range == *range)
            .expect("an active marker keeps its layout run");
        assert_eq!(
            run.metrics,
            styles.metrics(*role),
            "active marker color cannot change text metrics"
        );
    }
    let diagnostics = diagnostics_for(case, source.as_ref(), revision);
    let installed =
        InstalledPresentation::new(plan, styles, layout_document, diagnostics.clone(), assets)
            .expect("the installed presentation is revision-consistent");
    let document = Arc::new(MarkdownDocumentSnapshot::new(syntax));
    let selections = selections_for(case, &document, source.as_ref());
    let mut session = MarkdownDocumentSession::with_selections(document, selections)
        .expect("case selections are valid for the fixture");
    if case == HarnessCase::Selection || case.motion_sample_time().is_some() {
        session.begin_ime().expect("overlay case starts IME");
        session
            .update_ime("候補", 0..2)
            .expect("overlay case installs a valid UTF-16 preedit range");
        let diagnostic = diagnostics
            .first()
            .expect("overlay case has a diagnostic range");
        let ime = session.ime().expect("overlay case retains IME state");
        assert!(
            diagnostic.range.end() <= ime.replace_range().start()
                || ime.replace_range().end() <= diagnostic.range.start(),
            "diagnostic and IME attachments use separate source subranges"
        );
    }
    PreparedPresentation {
        installed,
        session,
        source,
    }
}

fn selections_for(
    case: HarnessCase,
    document: &MarkdownDocumentSnapshot,
    source: &str,
) -> SelectionSet {
    if case == HarnessCase::Selection || case.motion_sample_time().is_some() {
        let needle = if case == HarnessCase::Selection {
            "***strong emphasis***"
        } else {
            "One paragraph that stays."
        };
        let selected = unique_range(source, needle);
        let caret_in_selected = source[selected.clone()]
            .find(if case == HarnessCase::Selection {
                "emphasis"
            } else {
                "stays"
            })
            .expect("selected overlay text contains the caret target");
        let caret_target = selected.start + caret_in_selected;
        let ime_target = Selection::new(
            position(caret_target, Affinity::Before),
            position(caret_target + 2, Affinity::After),
        );
        return SelectionSet::from_selections(document, vec![ime_target], 0)
            .expect("nonempty overlay selection is valid");
    }
    let offset = match case {
        HarnessCase::Headings => inside(source, "Heading 2", 2),
        HarnessCase::Inline => inside(source, "`inline`", 4),
        HarnessCase::Lists => inside(source, "ordered", 2),
        HarnessCase::Quotes => inside(source, "quoted", 2),
        HarnessCase::Code => inside(source, "uml.class", 5),
        HarnessCase::Tables => inside(source, "| a | b | c |", 6),
        HarnessCase::MotionStart | HarnessCase::MotionMid | HarnessCase::MotionEnd => {
            unreachable!("motion cases install their selection and caret with the overlay range")
        }
        HarnessCase::Images | HarnessCase::Invalid | HarnessCase::Selection => 0,
    };
    SelectionSet::caret(document, text_size(offset)).expect("case caret is valid")
}

fn diagnostics_for(
    case: HarnessCase,
    source: &str,
    revision: DocumentRevision,
) -> Arc<[PresentedDiagnostic]> {
    let diagnostic = match case {
        HarnessCase::Invalid => Some((
            unique_range(source, "Unmatched **strong opener stays literal."),
            PresentedDiagnosticSeverity::Error,
            "unmatched strong opener",
        )),
        HarnessCase::Selection => Some((
            unique_range(source, "strong"),
            PresentedDiagnosticSeverity::Warning,
            "selected nested inline syntax",
        )),
        HarnessCase::MotionStart | HarnessCase::MotionMid | HarnessCase::MotionEnd => Some((
            unique_range(source, "Before"),
            PresentedDiagnosticSeverity::Warning,
            "stable motion overlay attachment",
        )),
        _ => None,
    };
    diagnostic.map_or_else(
        || Arc::from([]),
        |(range, severity, message)| {
            Arc::from([PresentedDiagnostic {
                revision,
                range: text_range(range.start, range.end),
                severity,
                message: Arc::from(message),
            }])
        },
    )
}

fn assets_for(
    case: HarnessCase,
    plan: &PresentationPlan,
) -> (Arc<EmbeddedAssetFrame>, EmbeddedMeasurements) {
    let images = plan
        .items
        .iter()
        .filter_map(|item| match item {
            PresentationItem::EmbeddedBlock {
                id, source_range, ..
            } => Some((*id, *source_range)),
            _ => None,
        })
        .collect::<Vec<_>>();
    if case == HarnessCase::Images {
        assert_eq!(
            images.len(),
            3,
            "images case has exactly three parsed images"
        );
    }
    let ready = ApprovedImageSource::Bytes {
        cache_key: Arc::from("checker.svg"),
        media_type: ImageMediaType::Svg,
        data: Arc::from(CHECKER_SVG),
        pixel_size: (96, 48),
    };
    let states = images
        .iter()
        .enumerate()
        .map(|(index, (id, _))| {
            let state = if case == HarnessCase::Images {
                match index {
                    0 => EmbeddedState::Loading,
                    1 => EmbeddedState::Failed {
                        message: Arc::from("fixture failed; retry"),
                    },
                    2 => EmbeddedState::Ready {
                        source: ready.clone(),
                    },
                    _ => unreachable!("images count was checked"),
                }
            } else if case.motion_sample_time().is_some() {
                EmbeddedState::Ready {
                    source: ready.clone(),
                }
            } else {
                EmbeddedState::Loading
            };
            (*id, state)
        })
        .collect::<Vec<_>>();
    let blocks = images
        .iter()
        .zip(states.iter())
        .map(|((id, source_range), (_, state))| MeasuredBlock {
            id: layout_id(*id),
            source_range: *source_range,
            size: match state {
                EmbeddedState::Loading => dvec2(LOADING_MAX_WIDTH, LOADING_HEIGHT),
                EmbeddedState::Failed { .. } => dvec2(FAILED_MAX_WIDTH, FAILED_HEIGHT),
                EmbeddedState::Ready { source } => {
                    let (width, height) = source.pixel_size();
                    dvec2(f64::from(width), f64::from(height))
                }
            },
            baseline: None,
        })
        .collect::<Vec<_>>();
    (
        Arc::new(EmbeddedAssetFrame {
            revision: plan.revision,
            items: states.into(),
        }),
        EmbeddedMeasurements {
            revision: Some(plan.revision),
            blocks: blocks.into(),
        },
    )
}

fn layout_id(item: PresentationItemId) -> LayoutElementId {
    LayoutElementId {
        owner: item.owner,
        fragment_ordinal: item.fragment_ordinal,
    }
}

fn unique_line<'a>(source: &'a str, line: &str) -> &'a str {
    let range = unique_line_range(source, line);
    &source[range]
}

fn unique_line_range(source: &str, line: &str) -> std::ops::Range<usize> {
    let mut offset = 0;
    let mut matches = Vec::new();
    for segment in source.split_inclusive('\n') {
        let candidate = segment.strip_suffix('\n').unwrap_or(segment);
        let candidate = candidate.strip_suffix('\r').unwrap_or(candidate);
        if candidate == line {
            matches.push(offset..offset + candidate.len());
        }
        offset += segment.len();
    }
    assert_eq!(matches.len(), 1, "expected one exact line {line:?}");
    matches.pop().expect("one exact line was checked")
}

fn unique_range(source: &str, needle: &str) -> std::ops::Range<usize> {
    let matches = source.match_indices(needle).collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "expected one exact marker {needle:?}");
    let start = matches[0].0;
    start..start + needle.len()
}

fn inside(source: &str, needle: &str, relative: usize) -> usize {
    let range = unique_range(source, needle);
    assert!(relative < range.len(), "caret must be inside {needle:?}");
    range.start + relative
}

fn text_size(value: usize) -> TextSize {
    TextSize::try_from_usize(value).expect("fixture offset fits TextSize")
}

fn text_range(start: usize, end: usize) -> TextRange {
    TextRange::new(text_size(start), text_size(end)).expect("fixture range is ordered")
}

fn assert_ready_embeds_below_literal_source(
    commands: &[DrawCommand],
    plan: &waml_markdown_editor::presentation::PresentationPlan,
    layout: &LayoutSnapshot,
) {
    for (id, rect) in commands.iter().filter_map(|command| match command {
        DrawCommand::EmbeddedBlock {
            id,
            rect,
            state: EmbeddedState::Ready { .. },
        } => Some((*id, *rect)),
        _ => None,
    }) {
        let source_range = plan
            .items
            .iter()
            .find_map(|item| match item {
                PresentationItem::EmbeddedBlock {
                    id: item_id,
                    source_range,
                    ..
                } if layout_id(*item_id) == id => Some(*source_range),
                _ => None,
            })
            .expect("ready embedded command retains its source range");
        let literal_bottom = layout
            .glyph_clusters()
            .iter()
            .filter(|cluster| {
                source_range.start() <= cluster.source_range.start()
                    && cluster.source_range.end() <= source_range.end()
            })
            .map(|cluster| cluster.rect.pos.y + cluster.rect.size.y)
            .reduce(f64::max)
            .expect("ready embed retains literal source glyph geometry");
        assert!(
            rect.pos.y >= literal_bottom,
            "ready embed overlaps literal source: id={id:?}, source={source_range:?}, literal_bottom={literal_bottom}, rect={rect:?}"
        );
    }
}

fn position(offset: usize, affinity: Affinity) -> TextPosition {
    TextPosition::new(text_size(offset), affinity)
}

fn assert_text_bytes_are_painted(commands: &[DrawCommand], expected: std::ops::Range<usize>) {
    let ranges = commands
        .iter()
        .filter_map(|command| match command {
            DrawCommand::Text { range, .. }
                if text_size(expected.start) < range.end()
                    && range.start() < text_size(expected.end) =>
            {
                Some(*range)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_source_ranges_cover_bytes(&ranges, expected, "exact text geometry and paint");
}

fn assert_source_ranges_cover_bytes(
    ranges: &[TextRange],
    expected: std::ops::Range<usize>,
    message: &str,
) {
    let painted = ranges
        .iter()
        .flat_map(|range| range.start().to_usize()..range.end().to_usize())
        .filter(|offset| expected.contains(offset))
        .collect::<Vec<_>>();
    assert_eq!(
        painted,
        expected.collect::<Vec<_>>(),
        "every expected source byte has {message}"
    );
}

struct MotionEvidence<'a> {
    case: HarnessCase,
    source: &'a str,
    session: &'a MarkdownDocumentSession,
    commands: &'a [DrawCommand],
    layout: &'a LayoutSnapshot,
    painted_ranges: &'a [TextRange],
    before: &'a LayoutSnapshot,
    target: &'a LayoutSnapshot,
}

fn assert_motion_paint_and_overlay_evidence(evidence: MotionEvidence<'_>) {
    let MotionEvidence {
        case,
        source,
        session,
        commands,
        layout,
        painted_ranges,
        before,
        target,
    } = evidence;
    let heading = unique_range(source, "# Before");
    let insertion = unique_range(source, "An inserted paragraph.");
    let stable = unique_range(source, "One paragraph that stays.");
    let image_source = unique_line_range(source, CHECKER_LINE);
    for (label, range) in [
        ("heading", heading.clone()),
        ("insertion", insertion.clone()),
        ("stable paragraph", stable.clone()),
        ("image source", image_source),
    ] {
        assert_non_whitespace_source_bytes_are_painted(
            painted_ranges,
            source,
            range,
            &format!("{label} bytes reached native glyph rasterization"),
        );
    }

    let diagnostic = commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Decoration {
                range,
                rects,
                role: waml_markdown_editor::presentation::DecorationRole::DiagnosticUnderline(_),
                ..
            } => Some((*range, rects.clone())),
            _ => None,
        })
        .expect("motion phase paints one diagnostic underline");
    let diagnostic_range = unique_range(source, "Before");
    assert_eq!(
        diagnostic.0,
        text_range(diagnostic_range.start, diagnostic_range.end)
    );
    assert_eq!(
        diagnostic.1.as_ref(),
        range_rects(layout, diagnostic.0).as_slice(),
        "diagnostic underline follows exact phase geometry"
    );
    assert_source_ranges_cover_bytes(
        painted_ranges,
        diagnostic_range,
        "diagnostic source bytes reached native glyph rasterization",
    );

    let selection_rects = commands
        .iter()
        .filter_map(|command| match command {
            DrawCommand::Selection { rect } => Some(*rect),
            _ => None,
        })
        .collect::<Vec<_>>();
    let expected_selection = session
        .selections()
        .as_slice()
        .iter()
        .filter(|selection| !selection.is_empty())
        .flat_map(|selection| layout.selection_rects(*selection).unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(
        selection_rects, expected_selection,
        "selection follows exact phase geometry"
    );

    let (caret, composition) = commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::CaretAndIme { caret, composition } => Some((*caret, composition.clone())),
            _ => None,
        })
        .expect("motion phase paints the caret and IME");
    let cursor = session.selections().primary().cursor;
    assert_eq!(
        caret,
        layout
            .source_to_point(cursor)
            .expect("motion caret has phase geometry")
            .rect,
        "caret follows exact phase geometry"
    );
    let cursor_offset = cursor.offset.to_usize();
    assert_source_ranges_cover_bytes(
        painted_ranges,
        cursor_offset..cursor_offset + 1,
        "caret source byte reached native glyph rasterization",
    );
    let ime = session.ime().expect("motion phase retains IME");
    assert_eq!(
        composition.as_ref(),
        range_rects(layout, ime.replace_range()).as_slice(),
        "IME underline follows exact phase geometry"
    );
    assert_source_ranges_cover_bytes(
        painted_ranges,
        ime.replace_range().start().to_usize()..ime.replace_range().end().to_usize(),
        "IME source bytes reached native glyph rasterization",
    );

    assert_target_only_clusters_use_target_geometry(layout, target, &insertion);
    if case == HarnessCase::MotionStart {
        assert_start_survivors_use_before_geometry(layout, before, target, &stable);
        let insertion_rect = cluster_bounds(layout, &insertion);
        let stable_rect = cluster_bounds(layout, &stable);
        assert!(
            insertion_rect.pos.y < stable_rect.pos.y + stable_rect.size.y
                && stable_rect.pos.y < insertion_rect.pos.y + insertion_rect.size.y,
            "motion-start intentionally overlaps target-only insertion geometry with survivors at their before geometry"
        );
    }
    if case == HarnessCase::MotionEnd {
        assert_exact_target_layout(layout, target);
    }
}

fn range_rects(layout: &LayoutSnapshot, range: TextRange) -> Vec<Rect> {
    layout
        .selection_rects(Selection::new(
            position(range.start().to_usize(), Affinity::Before),
            position(range.end().to_usize(), Affinity::After),
        ))
        .unwrap_or_default()
}

fn assert_non_whitespace_source_bytes_are_painted(
    ranges: &[TextRange],
    source: &str,
    expected: std::ops::Range<usize>,
    message: &str,
) {
    let expected = expected
        .filter(|offset| !source.as_bytes()[*offset].is_ascii_whitespace())
        .collect::<Vec<_>>();
    let painted = ranges
        .iter()
        .flat_map(|range| range.start().to_usize()..range.end().to_usize())
        .filter(|offset| expected.contains(offset))
        .collect::<Vec<_>>();
    assert_eq!(
        painted, expected,
        "every visible expected source byte has {message}"
    );
}

fn assert_target_only_clusters_use_target_geometry(
    actual: &LayoutSnapshot,
    target: &LayoutSnapshot,
    range: &std::ops::Range<usize>,
) {
    let expected = clusters_in_range(target, range);
    assert!(!expected.is_empty(), "target insertion has glyph geometry");
    assert_eq!(
        clusters_in_range(actual, range),
        expected,
        "target-only elements appear immediately at target geometry"
    );
}

fn assert_start_survivors_use_before_geometry(
    actual: &LayoutSnapshot,
    before: &LayoutSnapshot,
    target: &LayoutSnapshot,
    target_range: &std::ops::Range<usize>,
) {
    for target_cluster in clusters_in_range(target, target_range) {
        let actual_cluster = actual
            .glyph_clusters()
            .iter()
            .find(|cluster| cluster.id == target_cluster.id)
            .expect("motion-start retains each stable target identity");
        let before_cluster = before
            .glyph_clusters()
            .iter()
            .find(|cluster| cluster.id == target_cluster.id)
            .expect("motion-start stable identity exists in before geometry");
        assert_eq!(actual_cluster.source_range, target_cluster.source_range);
        assert_eq!(
            actual_cluster.rect, before_cluster.rect,
            "motion-start survivors begin at before geometry"
        );
    }
}

fn clusters_in_range<'a>(
    layout: &'a LayoutSnapshot,
    range: &std::ops::Range<usize>,
) -> Vec<&'a waml_markdown_editor::layout::GlyphCluster> {
    layout
        .glyph_clusters()
        .iter()
        .filter(|cluster| {
            text_size(range.start) <= cluster.source_range.start()
                && cluster.source_range.end() <= text_size(range.end)
        })
        .collect()
}

fn cluster_bounds(layout: &LayoutSnapshot, range: &std::ops::Range<usize>) -> Rect {
    let clusters = clusters_in_range(layout, range);
    assert!(
        !clusters.is_empty(),
        "checked source range has glyph geometry"
    );
    let min_x = clusters
        .iter()
        .map(|cluster| cluster.rect.pos.x)
        .reduce(f64::min)
        .unwrap();
    let min_y = clusters
        .iter()
        .map(|cluster| cluster.rect.pos.y)
        .reduce(f64::min)
        .unwrap();
    let max_x = clusters
        .iter()
        .map(|cluster| cluster.rect.pos.x + cluster.rect.size.x)
        .reduce(f64::max)
        .unwrap();
    let max_y = clusters
        .iter()
        .map(|cluster| cluster.rect.pos.y + cluster.rect.size.y)
        .reduce(f64::max)
        .unwrap();
    Rect {
        pos: dvec2(min_x, min_y),
        size: dvec2(max_x - min_x, max_y - min_y),
    }
}

fn assert_exact_target_layout(actual: &LayoutSnapshot, target: &LayoutSnapshot) {
    assert_eq!(actual.revision(), target.revision());
    assert_eq!(actual.viewport_width(), target.viewport_width());
    assert_eq!(actual.content_size(), target.content_size());
    assert_eq!(actual.visible_source_range(), target.visible_source_range());
    assert_eq!(actual.visible_block_range(), target.visible_block_range());
    assert_eq!(actual.blocks(), target.blocks());
    assert_eq!(actual.visual_lines(), target.visual_lines());
    assert_eq!(actual.visual_rows(), target.visual_rows());
    assert_eq!(actual.visual_lanes(), target.visual_lanes());
    assert_eq!(actual.glyph_clusters(), target.glyph_clusters());
    assert_eq!(actual.block_summaries(), target.block_summaries());
}
