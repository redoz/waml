use std::{collections::HashMap, path::PathBuf, sync::Arc};

use makepad_widgets::*;
use makepad_widgets::{animator::Ease, shader::draw_text::FontFamily, text::geom::Point};

use crate::{
    edit::ProposedMarkdownEdit,
    input::{
        ControllerError, EditorInput, EditorKey, MarkdownEditorController, PointerGesture,
        ScrollAnchor, ScrollState, SelectionModifier,
    },
    layout::{
        FontKey, FontResolver, LayoutElementId, LayoutEngine, LayoutError, LayoutInvalidation,
        LayoutSnapshot, LayoutViewport, MakepadTextLayoutCache, MakepadTextShaper, TextMetrics,
    },
    motion::{LayoutChangeCause, MotionConfig, MotionController},
    presentation::style::FONT_MONO,
    presentation::{
        build_draw_commands, ApprovedImageSource, ColorRole, DecorationRole, DrawCommand,
        EmbeddedState, ImageMediaType, InstalledPresentation, PresentationError, PresentationFrame,
    },
    selection::TextPosition,
    session::MarkdownDocumentSession,
};

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.widgets.*

    mod.widgets.MarkdownEditorBase = #(MarkdownEditor::register_widget(vm))

    mod.widgets.MarkdownEditor = set_type_default() do mod.widgets.MarkdownEditorBase {
        width: Fill
        height: Fill
        scroll_bars: ScrollBars {
            show_scroll_x: false
            show_scroll_y: true
        }
        draw_text_sans +: {text_style: theme.font_regular}
        draw_text_sans_italic +: {text_style: theme.font_italic}
        draw_text_sans_semibold +: {text_style: theme.font_bold}
        draw_text_sans_semibold_italic +: {text_style: theme.font_bold_italic}
        draw_text_mono +: {text_style: theme.font_code}
        draw_text_mono_italic +: {text_style: theme.font_code}
        draw_text_mono_semibold +: {text_style: theme.font_code}
        draw_text_mono_semibold_italic +: {text_style: theme.font_code}
        motion_duration: 0.100
        motion_ease: OutCubic
        body_color: #202124
        marker_color: #7a7f87
        marker_active_color: #3f73d8
        link_color: #2869c7
        diagnostic_color: #d64545
        quote_fill: #f5f6f7
        code_fill: #f2f3f5
        table_fill: #f7f8f9
        inline_code_fill: #eceef1
        block_rule_color: #c7cbd1
        selection_color: #598ce647
        caret_color: #202124
    }
}

pub fn live_design(cx: &mut Cx) {
    cx.with_vm(register_script_mod);
}

pub(crate) fn register_script_mod(vm: &mut ScriptVm) -> ScriptValue {
    script_mod(vm)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EmbeddedBlockEvent {
    Activated,
    RetryRequested,
    MeasurementChanged { size: DVec2 },
}

#[derive(Clone, Debug, Default)]
pub enum MarkdownEditorAction {
    ProposedEdit(ProposedMarkdownEdit),
    SelectionChanged,
    NavigationRequested {
        position: TextPosition,
    },
    EmbeddedBlockEvent {
        id: LayoutElementId,
        event: EmbeddedBlockEvent,
    },
    Error(MarkdownEditorError),
    #[default]
    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkdownEditorError {
    Layout(LayoutError),
    Presentation(PresentationError),
    ControllerLayout(LayoutError),
    ControllerEdit,
    MissingLayoutDocument,
    StalePresentation {
        installed: waml_syntax::DocumentRevision,
        session: waml_syntax::DocumentRevision,
    },
}

impl From<ControllerError> for MarkdownEditorError {
    fn from(error: ControllerError) -> Self {
        match error {
            ControllerError::Layout(error) => Self::ControllerLayout(error),
            ControllerError::Edit(_) => Self::ControllerEdit,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextFace {
    SansRegular,
    SansRegularItalic,
    SansSemibold,
    SansSemiboldItalic,
    MonoRegular,
    MonoRegularItalic,
    MonoSemibold,
    MonoSemiboldItalic,
}

fn text_face(metrics: TextMetrics) -> TextFace {
    match (
        metrics.font == FONT_MONO,
        metrics.weight.0 >= 600,
        metrics.italic,
    ) {
        (false, false, false) => TextFace::SansRegular,
        (false, false, true) => TextFace::SansRegularItalic,
        (false, true, false) => TextFace::SansSemibold,
        (false, true, true) => TextFace::SansSemiboldItalic,
        (true, false, false) => TextFace::MonoRegular,
        (true, false, true) => TextFace::MonoRegularItalic,
        (true, true, false) => TextFace::MonoSemibold,
        (true, true, true) => TextFace::MonoSemiboldItalic,
    }
}

#[derive(Default)]
struct WidgetFonts {
    sans_regular: Option<FontFamily>,
    sans_regular_italic: Option<FontFamily>,
    sans_semibold: Option<FontFamily>,
    sans_semibold_italic: Option<FontFamily>,
    mono_regular: Option<FontFamily>,
    mono_regular_italic: Option<FontFamily>,
    mono_semibold: Option<FontFamily>,
    mono_semibold_italic: Option<FontFamily>,
}

impl WidgetFonts {
    fn configure_face(&self, face: TextFace, metrics: TextMetrics, draw: &mut DrawText) {
        let font = match face {
            TextFace::SansRegular => self.sans_regular.as_ref(),
            TextFace::SansRegularItalic => self.sans_regular_italic.as_ref(),
            TextFace::SansSemibold => self.sans_semibold.as_ref(),
            TextFace::SansSemiboldItalic => self.sans_semibold_italic.as_ref(),
            TextFace::MonoRegular => self.mono_regular.as_ref(),
            TextFace::MonoRegularItalic => self.mono_regular_italic.as_ref(),
            TextFace::MonoSemibold => self.mono_semibold.as_ref(),
            TextFace::MonoSemiboldItalic => self.mono_semibold_italic.as_ref(),
        };
        if let Some(font) = font {
            draw.text_style.font_family = font.clone();
        }
        draw.text_style.font_size = metrics.font_size;
        draw.text_style.line_spacing = metrics.line_spacing;
    }
}

impl FontResolver for WidgetFonts {
    fn configure_draw_text(&mut self, _key: FontKey, metrics: TextMetrics, draw: &mut DrawText) {
        self.configure_face(text_face(metrics), metrics, draw);
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum AssetIdentity {
    Bytes { cache_key: Arc<str>, media_type: u8 },
    CanonicalFile(Arc<PathBuf>),
}

#[derive(Default)]
struct DecodedImageCache {
    images: HashMap<AssetIdentity, Option<ImageRef>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DrawLayer {
    BlockBackground,
    Selection,
    Text,
    Decoration,
    EmbeddedBlock,
    CaretAndIme,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextPaintOperation {
    Background {
        rect: Rect,
        color: ColorRole,
    },
    Glyphs {
        face: TextFace,
        metrics: TextMetrics,
        color: ColorRole,
    },
    Underline {
        rect: Rect,
        color: ColorRole,
    },
    Strikethrough {
        rect: Rect,
        color: ColorRole,
    },
}

impl TextPaintOperation {
    pub fn layer(&self) -> DrawLayer {
        match self {
            Self::Background { .. } => DrawLayer::BlockBackground,
            Self::Glyphs { .. } => DrawLayer::Text,
            Self::Underline { .. } | Self::Strikethrough { .. } => DrawLayer::Decoration,
        }
    }
}

pub fn build_text_paint_operations(
    rect: Rect,
    style: crate::presentation::ResolvedTextStyle,
) -> Vec<TextPaintOperation> {
    let mut operations = Vec::with_capacity(4);
    if let Some(color) = style.background {
        operations.push(TextPaintOperation::Background { rect, color });
    }
    operations.push(TextPaintOperation::Glyphs {
        face: text_face(style.metrics),
        metrics: style.metrics,
        color: style.color,
    });
    if style.underline {
        operations.push(TextPaintOperation::Underline {
            rect: underline_rect(rect),
            color: style.color,
        });
    }
    if style.strikethrough {
        operations.push(TextPaintOperation::Strikethrough {
            rect: strikethrough_rect(rect),
            color: style.color,
        });
    }
    operations
}

#[derive(Default)]
pub struct DrawRecorder {
    layers: Vec<DrawLayer>,
    snapshot_ptrs: Vec<*const LayoutSnapshot>,
    primitive_counts: Vec<usize>,
}

impl DrawRecorder {
    pub fn layers(&self) -> &[DrawLayer] {
        &self.layers
    }

    pub fn snapshot_ptrs(&self) -> &[*const LayoutSnapshot] {
        &self.snapshot_ptrs
    }

    pub fn primitive_counts(&self) -> &[usize] {
        &self.primitive_counts
    }

    fn record(&mut self, layer: DrawLayer, layout: &Arc<LayoutSnapshot>) {
        self.layers.push(layer);
        self.snapshot_ptrs.push(Arc::as_ptr(layout));
        self.primitive_counts.push(0);
    }

    fn set_last_primitive_count(&mut self, count: usize) {
        if let Some(last) = self.primitive_counts.last_mut() {
            *last = count;
        }
    }
}

pub fn draw_visible_layers_for_test(layout: &Arc<LayoutSnapshot>, recorder: &mut DrawRecorder) {
    for layer in [
        DrawLayer::BlockBackground,
        DrawLayer::Selection,
        DrawLayer::Text,
        DrawLayer::Decoration,
        DrawLayer::EmbeddedBlock,
        DrawLayer::CaretAndIme,
    ] {
        recorder.record(layer, layout);
    }
}

pub fn derive_motion_scroll_anchor(
    on_screen: &LayoutSnapshot,
    target: &LayoutSnapshot,
    position: TextPosition,
    scroll_y: f64,
) -> Option<ScrollAnchor> {
    let on_screen_caret = on_screen.source_to_point(position)?;
    target.source_to_point(position)?;
    Some(ScrollAnchor {
        position,
        viewport_y: on_screen_caret.rect.pos.y - scroll_y,
    })
}

pub fn navigation_position(
    plan: &crate::presentation::PresentationPlan,
    layout: &LayoutSnapshot,
    point: DVec2,
) -> Option<TextPosition> {
    let position = layout.point_to_source(point);
    plan.links
        .iter()
        .any(|link| {
            link.source_range.start() <= position.offset
                && position.offset < link.source_range.end()
        })
        .then_some(position)
}

struct TextGlyphPaint {
    id: crate::layout::GeometryElementId,
    range: waml_syntax::TextRange,
    rect: Rect,
    face: TextFace,
    metrics: TextMetrics,
    color: ColorRole,
}

#[derive(Clone, Copy)]
struct TextPaintTarget {
    id: crate::layout::GeometryElementId,
    range: waml_syntax::TextRange,
    rect: Rect,
}

#[derive(Default)]
struct PaintEvidence {
    enabled: bool,
    generation: u64,
    ranges: Vec<waml_syntax::TextRange>,
    commands: Vec<DrawCommand>,
    glyph_origins: Vec<DVec2>,
    embedded_states: Vec<EmbeddedState>,
}

impl PaintEvidence {
    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.ranges = Vec::new();
            self.commands = Vec::new();
            self.glyph_origins = Vec::new();
            self.embedded_states = Vec::new();
        }
    }

    #[cfg(test)]
    fn enabled(&self) -> bool {
        self.enabled
    }

    fn begin_frame(&mut self) {
        if self.enabled {
            self.generation = self.generation.saturating_add(1);
            self.ranges.clear();
            self.commands.clear();
            self.glyph_origins.clear();
            self.embedded_states.clear();
        }
    }

    fn generation(&self) -> u64 {
        self.generation
    }

    fn record(&mut self, range: waml_syntax::TextRange) {
        if self.enabled {
            self.ranges.push(range);
        }
    }

    fn ranges(&self) -> &[waml_syntax::TextRange] {
        &self.ranges
    }

    fn record_command(&mut self, command: &DrawCommand) {
        if self.enabled {
            self.commands.push(command.clone());
        }
    }

    fn commands(&self) -> &[DrawCommand] {
        &self.commands
    }

    fn record_glyph_origin(&mut self, origin: DVec2) {
        if self.enabled {
            self.glyph_origins.push(origin);
        }
    }

    fn glyph_origins(&self) -> &[DVec2] {
        &self.glyph_origins
    }

    fn record_embedded_state(&mut self, state: EmbeddedState) {
        if self.enabled {
            self.embedded_states.push(state);
        }
    }

    fn embedded_states(&self) -> &[EmbeddedState] {
        &self.embedded_states
    }

    #[cfg(test)]
    fn capacity(&self) -> usize {
        self.ranges.capacity()
            + self.commands.capacity()
            + self.glyph_origins.capacity()
            + self.embedded_states.capacity()
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct MarkdownEditor {
    #[deref]
    view: View,
    #[live]
    scroll_bars: ScrollBars,
    #[rust]
    controller: MarkdownEditorController,
    #[rust]
    layout_engine: LayoutEngine,
    #[rust]
    installed: Option<Arc<InstalledPresentation>>,
    #[rust]
    target_layout: Option<Arc<LayoutSnapshot>>,
    #[rust]
    previous_layout: Option<Arc<LayoutSnapshot>>,
    #[rust]
    frame_layout: Option<Arc<LayoutSnapshot>>,
    #[rust]
    motion: MotionController,
    #[rust]
    pending_cause: Option<LayoutChangeCause>,
    #[rust]
    next_frame: NextFrame,
    #[rust]
    pointer_drag_active: bool,
    #[rust]
    read_only: bool,
    #[rust]
    reduced_motion: bool,
    #[rust]
    last_ime_point: DVec2,
    #[rust]
    scroll_y: f64,
    #[rust]
    has_focus: bool,
    #[live]
    draw_text_sans: DrawText,
    #[live]
    draw_text_sans_italic: DrawText,
    #[live]
    draw_text_sans_semibold: DrawText,
    #[live]
    draw_text_sans_semibold_italic: DrawText,
    #[live]
    draw_text_mono: DrawText,
    #[live]
    draw_text_mono_italic: DrawText,
    #[live]
    draw_text_mono_semibold: DrawText,
    #[live]
    draw_text_mono_semibold_italic: DrawText,
    #[live]
    draw_background: DrawColor,
    #[live]
    draw_selection: DrawColor,
    #[live]
    draw_decoration: DrawColor,
    #[live]
    draw_embedded: DrawColor,
    #[live]
    draw_caret: DrawColor,
    #[live]
    motion_duration: f64,
    #[live]
    motion_ease: Ease,
    #[live]
    body_color: Vec4,
    #[live]
    marker_color: Vec4,
    #[live]
    marker_active_color: Vec4,
    #[live]
    link_color: Vec4,
    #[live]
    diagnostic_color: Vec4,
    #[live]
    quote_fill: Vec4,
    #[live]
    code_fill: Vec4,
    #[live]
    table_fill: Vec4,
    #[live]
    inline_code_fill: Vec4,
    #[live]
    block_rule_color: Vec4,
    #[live]
    selection_color: Vec4,
    #[live]
    caret_color: Vec4,
    #[rust]
    fonts: WidgetFonts,
    #[rust]
    text_layout_cache: MakepadTextLayoutCache,
    #[rust]
    image_cache: DecodedImageCache,
    #[rust]
    last_draw: DrawRecorder,
    #[rust]
    paint_evidence: PaintEvidence,
}

impl Widget for MarkdownEditor {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let Some(session) = scope.data.get_mut::<MarkdownDocumentSession>() else {
            self.view.handle_event(cx, event, scope);
            return;
        };
        match self.handle_event_with_session(cx, event, session) {
            Ok(actions) => cx.extend_actions(actions),
            Err(error) => log!("markdown editor event failed: {error:?}"),
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let Some(session) = scope.data.get_mut::<MarkdownDocumentSession>() else {
            return self.view.draw_walk(cx, scope, walk);
        };
        let mut child_scope = Scope::empty();
        match self.draw_walk_with_session(cx, session, &mut child_scope, walk) {
            Ok(step) => step,
            Err(error) => {
                log!("markdown editor draw failed: {error:?}");
                self.view.draw_walk(cx, &mut child_scope, walk)
            }
        }
    }
}

impl MarkdownEditor {
    fn redraw(&mut self, cx: &mut Cx) {
        self.scroll_bars.redraw(cx);
        self.view.redraw(cx);
    }

    pub fn handle_event_with_session(
        &mut self,
        cx: &mut Cx,
        event: &Event,
        session: &mut MarkdownDocumentSession,
    ) -> Result<Vec<Action>, MarkdownEditorError> {
        if let Some(frame_event) = self.next_frame.is_event(event) {
            let frame = self.motion.sample(frame_event.time);
            self.frame_layout = Some(frame.layout.clone());
            self.scroll_y = frame.scroll_y;
            session.set_scroll(ScrollState {
                x: session.scroll().x,
                y: frame.scroll_y,
            });
            self.scroll_bars
                .set_scroll_pos_no_clip(cx, dvec2(session.scroll().x, frame.scroll_y));
            self.redraw(cx);
            if frame.active {
                self.next_frame = cx.new_next_frame();
            }
        }
        if self.has_focus {
            if let Event::TextInput(event) = event {
                let input = if event.was_paste {
                    EditorInput::Paste(Arc::from(event.input.as_str()))
                } else {
                    EditorInput::Text(Arc::from(event.input.as_str()))
                };
                return self.handle_input_with_session(cx, session, input);
            }
        }
        let scroll_actions = self
            .scroll_bars
            .handle_event(cx, event, &mut Scope::empty());
        if !scroll_actions.is_empty() {
            let scroll = self.scroll_bars.get_scroll_pos();
            self.scroll_y = scroll.y;
            session.set_scroll(ScrollState {
                x: scroll.x,
                y: scroll.y,
            });
            self.target_layout = None;
            self.pending_cause = Some(LayoutChangeCause::ViewportResize);
            self.redraw(cx);
        }
        let area = self.scroll_bars.area();
        let input = match event.hits(cx, area) {
            Hit::TextInput(event) if !self.read_only => Some(if event.was_paste {
                EditorInput::Paste(Arc::from(event.input.as_str()))
            } else {
                EditorInput::Text(Arc::from(event.input.as_str()))
            }),
            Hit::TextCopy(event) => {
                let layout = match self.frame_layout.as_ref() {
                    Some(layout) => layout.clone(),
                    None => self.install_layout(cx, session, None)?,
                };
                let response = self
                    .controller
                    .handle(session, &layout, EditorInput::Copy)
                    .map_err(MarkdownEditorError::from)?;
                *event.response.borrow_mut() = response.clipboard;
                return Ok(Vec::new());
            }
            Hit::TextCut(event) => {
                let layout = match self.frame_layout.as_ref() {
                    Some(layout) => layout.clone(),
                    None => self.install_layout(cx, session, None)?,
                };
                let copied = self
                    .controller
                    .handle(session, &layout, EditorInput::Copy)
                    .map_err(MarkdownEditorError::from)?;
                *event.response.borrow_mut() = copied.clipboard;
                return self.handle_input_with_session(cx, session, EditorInput::Cut);
            }
            Hit::KeyDown(event) => key_input(event),
            Hit::FingerDown(event) if event.is_primary_hit() => {
                cx.set_key_focus(area);
                let point = event.abs - area.rect(cx).pos + dvec2(0.0, self.scroll_y);
                self.pointer_drag_active = true;
                Some(EditorInput::PointerDown(PointerGesture {
                    point,
                    clicks: event.tap_count as u8,
                    modifier: if event.modifiers.is_primary() {
                        SelectionModifier::Add
                    } else if event.modifiers.shift {
                        SelectionModifier::Extend
                    } else {
                        SelectionModifier::Replace
                    },
                }))
            }
            Hit::FingerMove(event) if self.pointer_drag_active => Some(EditorInput::PointerMove {
                point: event.abs - area.rect(cx).pos + dvec2(0.0, self.scroll_y),
            }),
            Hit::FingerUp(event) if self.pointer_drag_active => {
                self.pointer_drag_active = false;
                if event.was_tap() {
                    let point = event.abs - area.rect(cx).pos + dvec2(0.0, self.scroll_y);
                    if event.modifiers.is_primary() {
                        if let (Some(installed), Some(layout)) =
                            (self.installed.as_ref(), self.frame_layout.as_ref())
                        {
                            if let Some(position) =
                                navigation_position(&installed.plan, layout, point)
                            {
                                return Ok(vec![self.make_action(
                                    MarkdownEditorAction::NavigationRequested { position },
                                )]);
                            }
                        }
                    }
                    if let Some((id, _)) = self.embedded_at(point) {
                        return Ok(vec![self.make_action(
                            MarkdownEditorAction::EmbeddedBlockEvent {
                                id,
                                event: EmbeddedBlockEvent::Activated,
                            },
                        )]);
                    }
                }
                Some(EditorInput::PointerUp)
            }
            Hit::KeyFocusLost(_) => {
                self.has_focus = false;
                cx.hide_text_ime();
                None
            }
            _ => None,
        };
        input.map_or(Ok(Vec::new()), |input| {
            self.handle_input_with_session(cx, session, input)
        })
    }

    pub fn draw_walk_with_session(
        &mut self,
        cx: &mut Cx2d,
        session: &mut MarkdownDocumentSession,
        scope: &mut Scope,
        walk: Walk,
    ) -> Result<DrawStep, MarkdownEditorError> {
        let requested_scroll = dvec2(session.scroll().x, session.scroll().y);
        if self.scroll_bars.get_scroll_pos() != requested_scroll {
            self.scroll_bars
                .set_scroll_pos_no_clip(cx, requested_scroll);
            self.scroll_y = requested_scroll.y;
        }
        let viewport = cx.peek_walk_turtle(walk);
        let viewport_size = viewport.size;
        let layout = self.install_layout(cx, session, Some(viewport_size))?;
        let installed = self
            .installed
            .as_ref()
            .ok_or(MarkdownEditorError::MissingLayoutDocument)?
            .clone();
        let frame = PresentationFrame {
            revision: installed.revision,
            layout: layout.clone(),
            active_owners: installed
                .plan
                .active_owners(session.selections().primary().cursor.offset),
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
        .map_err(MarkdownEditorError::Presentation)?;
        let content_origin = viewport.pos - self.scroll_bars.get_scroll_pos();
        let commands = commands
            .iter()
            .map(|command| command.translated(content_origin))
            .collect::<Arc<[_]>>();
        self.scroll_bars.begin(cx, walk, Layout::default());
        self.last_draw = DrawRecorder::default();
        self.paint_evidence.begin_frame();
        for layer in [
            DrawLayer::BlockBackground,
            DrawLayer::Selection,
            DrawLayer::Text,
            DrawLayer::Decoration,
            DrawLayer::EmbeddedBlock,
            DrawLayer::CaretAndIme,
        ] {
            self.last_draw.record(layer, &layout);
            let mut primitive_count = 0;
            for command in commands.iter() {
                if command.layer() == layer {
                    self.paint_evidence.record_command(command);
                }
                if let DrawCommand::Text {
                    id,
                    range,
                    rect,
                    style,
                } = command
                {
                    for operation in build_text_paint_operations(*rect, *style)
                        .into_iter()
                        .filter(|operation| operation.layer() == layer)
                    {
                        self.paint_text_operation(
                            cx,
                            &installed,
                            &layout,
                            TextPaintTarget {
                                id: *id,
                                range: *range,
                                rect: *rect,
                            },
                            operation,
                        );
                        primitive_count += 1;
                    }
                } else if command.layer() == layer {
                    self.paint_command(cx, scope, command);
                    primitive_count += 1;
                }
            }
            self.last_draw.set_last_primitive_count(primitive_count);
        }
        cx.turtle_mut()
            .set_used(layout.content_size().x, layout.content_size().y);
        self.scroll_bars.end(cx);
        let scroll = self.scroll_bars.get_scroll_pos();
        self.scroll_y = scroll.y;
        session.set_scroll(ScrollState {
            x: scroll.x,
            y: scroll.y,
        });
        if cx.has_key_focus(self.scroll_bars.area()) && !self.read_only {
            self.show_ime(cx, session);
        }
        Ok(DrawStep::done())
    }

    fn paint_command(&mut self, cx: &mut Cx2d, scope: &mut Scope, command: &DrawCommand) {
        match command {
            DrawCommand::BlockBackground { rect, role, .. } => {
                self.draw_background.color = match role {
                    crate::presentation::BlockDecorationRole::QuoteRule => self.block_rule_color,
                    crate::presentation::BlockDecorationRole::InlineCodeFill => {
                        self.inline_code_fill
                    }
                    crate::presentation::BlockDecorationRole::FencedCodeSurface => self.code_fill,
                    crate::presentation::BlockDecorationRole::TableGrid => self.block_rule_color,
                    crate::presentation::BlockDecorationRole::TableHeaderFill => self.table_fill,
                    crate::presentation::BlockDecorationRole::TaskCheckbox => self.quote_fill,
                    crate::presentation::BlockDecorationRole::ThematicRule => self.block_rule_color,
                };
                self.draw_background.draw_abs(cx, *rect);
            }
            DrawCommand::Selection { rect } => {
                self.draw_selection.color = self.selection_color;
                self.draw_selection.draw_abs(cx, *rect);
            }
            DrawCommand::Text { .. } => {}
            DrawCommand::Decoration { rects, role, .. } => {
                self.draw_decoration.color = match role {
                    DecorationRole::LinkUnderline => self.link_color,
                    DecorationRole::DiagnosticUnderline(_) => self.diagnostic_color,
                };
                for rect in rects.iter() {
                    self.draw_decoration.draw_abs(cx, underline_rect(*rect));
                }
            }
            DrawCommand::EmbeddedBlock { rect, state, .. } => match state {
                EmbeddedState::Ready { source } => {
                    self.paint_evidence
                        .record_embedded_state(EmbeddedState::Ready {
                            source: source.clone(),
                        });
                    if let Some(image) = self.image_for_source(cx, source) {
                        let walk = Walk::abs_rect(*rect);
                        debug_assert_eq!(
                            cx.peek_walk_turtle(walk),
                            *rect,
                            "ready image walk must keep draw-command coordinates"
                        );
                        let _ = image.draw_walk(cx, scope, walk);
                    } else {
                        self.draw_embedded.color = self.code_fill;
                        self.draw_embedded.draw_abs(cx, *rect);
                    }
                }
                EmbeddedState::Loading => {
                    self.paint_evidence
                        .record_embedded_state(EmbeddedState::Loading);
                    self.draw_embedded.color = self.quote_fill;
                    self.draw_embedded.draw_abs(cx, *rect);
                    self.draw_text_sans.color = self.marker_color;
                    self.draw_text_sans
                        .draw_abs(cx, rect.pos + dvec2(8.0, 8.0), "Loading image…");
                }
                EmbeddedState::Failed { message } => {
                    self.paint_evidence
                        .record_embedded_state(EmbeddedState::Failed {
                            message: message.clone(),
                        });
                    self.draw_embedded.color = self.diagnostic_color;
                    self.draw_embedded.draw_abs(cx, *rect);
                    self.draw_text_sans.color = self.body_color;
                    self.draw_text_sans
                        .draw_abs(cx, rect.pos + dvec2(8.0, 8.0), message);
                }
            },
            DrawCommand::CaretAndIme { caret, composition } => {
                self.draw_caret.color = self.caret_color;
                self.draw_caret.draw_abs(cx, *caret);
                for rect in composition.iter() {
                    self.draw_caret.draw_abs(cx, underline_rect(*rect));
                }
            }
        }
    }

    fn paint_text_operation(
        &mut self,
        cx: &mut Cx2d,
        installed: &InstalledPresentation,
        layout: &LayoutSnapshot,
        target: TextPaintTarget,
        operation: TextPaintOperation,
    ) {
        match operation {
            TextPaintOperation::Background { rect, color } => {
                self.draw_background.color = self.color_for_role(color);
                self.draw_background.draw_abs(cx, rect);
            }
            TextPaintOperation::Glyphs {
                face,
                metrics,
                color,
            } => self.paint_text(
                cx,
                installed,
                layout,
                TextGlyphPaint {
                    id: target.id,
                    range: target.range,
                    rect: target.rect,
                    face,
                    metrics,
                    color,
                },
            ),
            TextPaintOperation::Underline { rect, color }
            | TextPaintOperation::Strikethrough { rect, color } => {
                self.draw_decoration.color = self.color_for_role(color);
                self.draw_decoration.draw_abs(cx, rect);
            }
        }
    }

    fn paint_text(
        &mut self,
        cx: &mut Cx2d,
        installed: &InstalledPresentation,
        layout: &LayoutSnapshot,
        paint: TextGlyphPaint,
    ) {
        let Some(cluster) = layout
            .glyph_clusters()
            .iter()
            .find(|cluster| cluster.id == paint.id)
        else {
            return;
        };
        let origin = paint.rect.pos - cluster.rect.pos;
        let Some((shaped_range, laid_out)) = self.text_layout_cache.laid_out(
            installed.revision,
            paint.id.layout,
            paint.range,
            paint.metrics,
        ) else {
            return;
        };
        self.fonts
            .configure_face(paint.face, paint.metrics, &mut self.draw_text_sans);
        let cluster_offset = paint
            .range
            .start()
            .to_usize()
            .saturating_sub(shaped_range.start().to_usize());
        let laid_glyphs = laid_out
            .rows
            .iter()
            .flat_map(|row| {
                let row_offset = row.text.start_in_parent();
                row.glyphs
                    .iter()
                    .filter(move |glyph| row_offset + glyph.cluster == cluster_offset)
            })
            .collect::<Vec<_>>();
        let dpi = cx.current_dpi_factor() as f32;
        let glyphs = laid_glyphs
            .into_iter()
            .zip(cluster.glyphs.iter())
            .filter_map(|(laid, positioned)| {
                let rasterized = laid.rasterize(laid.font_size_in_lpxs * dpi)?;
                Some((
                    Point {
                        x: (positioned.origin.x + origin.x) as f32,
                        y: (positioned.origin.y + origin.y) as f32,
                    },
                    positioned.font_size,
                    rasterized,
                ))
            })
            .collect::<Vec<_>>();
        self.draw_text_sans.draw_rasterized_glyphs_abs(
            cx,
            &glyphs,
            self.color_for_role(paint.color),
        );
        for (origin, _, _) in &glyphs {
            self.paint_evidence
                .record_glyph_origin(dvec2(origin.x as f64, origin.y as f64));
        }
        if !glyphs.is_empty() {
            self.paint_evidence.record(paint.range);
        }
    }

    fn color_for_role(&self, role: ColorRole) -> Vec4 {
        match role {
            ColorRole::Text | ColorRole::Code => self.body_color,
            ColorRole::Marker | ColorRole::Muted | ColorRole::TableRule => self.marker_color,
            ColorRole::ActiveMarker => self.marker_active_color,
            ColorRole::Link => self.link_color,
            ColorRole::Recovery => self.diagnostic_color,
            ColorRole::CodeSurface => self.code_fill,
            ColorRole::Quote => self.quote_fill,
        }
    }

    fn image_for_source(&mut self, cx: &mut Cx, source: &ApprovedImageSource) -> Option<ImageRef> {
        let identity = match source {
            ApprovedImageSource::Bytes {
                cache_key,
                media_type,
                ..
            } => AssetIdentity::Bytes {
                cache_key: cache_key.clone(),
                media_type: match media_type {
                    ImageMediaType::Svg => 0,
                    ImageMediaType::Png => 1,
                    ImageMediaType::Jpeg => 2,
                },
            },
            ApprovedImageSource::CanonicalFile { path, .. } => {
                AssetIdentity::CanonicalFile(path.clone())
            }
        };
        if let Some(image) = self.image_cache.images.get(&identity) {
            return image.clone();
        }
        let widget =
            WidgetRef::new_with_inner(Box::new(cx.with_vm(Image::script_new_with_default)));
        let image = widget.as_image();
        let decoded = match source {
            ApprovedImageSource::Bytes {
                media_type, data, ..
            } => match media_type {
                ImageMediaType::Svg => image.load_svg_from_shared_data(cx, data.clone()),
                ImageMediaType::Png => image.load_png_from_data(cx, data),
                ImageMediaType::Jpeg => image.load_jpg_from_data(cx, data),
            },
            ApprovedImageSource::CanonicalFile { path, .. } => {
                image.load_image_file_by_path(cx, path.as_path())
            }
        };
        if decoded.is_err() {
            self.image_cache.images.insert(identity, None);
            return None;
        }
        self.image_cache
            .images
            .insert(identity, Some(image.clone()));
        Some(image)
    }

    fn embedded_at(&self, point: DVec2) -> Option<(LayoutElementId, Rect)> {
        let layout = self.frame_layout.as_ref()?;
        let document = &self.installed.as_ref()?.layout_document;
        layout.blocks()[layout.visible_block_range()]
            .iter()
            .find(|block| {
                block.rect.contains(point)
                    && document
                        .embedded_blocks
                        .iter()
                        .any(|item| item.id == block.id)
            })
            .map(|block| (block.id, block.rect))
    }

    fn install_layout(
        &mut self,
        cx: &mut Cx,
        session: &MarkdownDocumentSession,
        requested_viewport: Option<DVec2>,
    ) -> Result<Arc<LayoutSnapshot>, MarkdownEditorError> {
        let installed = self
            .installed
            .as_ref()
            .ok_or(MarkdownEditorError::MissingLayoutDocument)?
            .clone();
        if installed.revision != session.local_revision() {
            return Err(MarkdownEditorError::StalePresentation {
                installed: installed.revision,
                session: session.local_revision(),
            });
        }
        if self.pending_cause.is_none() {
            if let Some(layout) = self.frame_layout.as_ref() {
                return Ok(layout.clone());
            }
        }
        let viewport_size =
            resolved_layout_viewport(self.scroll_bars.area().rect(cx).size, requested_viewport);
        self.fonts.sans_regular = Some(self.draw_text_sans.text_style.font_family.clone());
        self.fonts.sans_regular_italic =
            Some(self.draw_text_sans_italic.text_style.font_family.clone());
        self.fonts.sans_semibold =
            Some(self.draw_text_sans_semibold.text_style.font_family.clone());
        self.fonts.sans_semibold_italic = Some(
            self.draw_text_sans_semibold_italic
                .text_style
                .font_family
                .clone(),
        );
        self.fonts.mono_regular = Some(self.draw_text_mono.text_style.font_family.clone());
        self.fonts.mono_regular_italic =
            Some(self.draw_text_mono_italic.text_style.font_family.clone());
        self.fonts.mono_semibold =
            Some(self.draw_text_mono_semibold.text_style.font_family.clone());
        self.fonts.mono_semibold_italic = Some(
            self.draw_text_mono_semibold_italic
                .text_style
                .font_family
                .clone(),
        );
        self.text_layout_cache.retain_revision(installed.revision);
        let mut shaper = MakepadTextShaper {
            cx,
            draw_text: &mut self.draw_text_sans,
            fonts: &mut self.fonts,
            revision: installed.revision,
            cache: Some(&mut self.text_layout_cache),
        };
        let layout = self
            .layout_engine
            .layout(
                &installed.layout_document,
                session.snapshot(),
                LayoutViewport::default_overscan(
                    viewport_size.x.max(1.0),
                    viewport_size.y.max(1.0),
                    self.scroll_y,
                ),
                LayoutInvalidation::Document,
                &mut shaper,
            )
            .map_err(MarkdownEditorError::Layout)?;
        let layout = Arc::new(layout);
        self.previous_layout = self.target_layout.replace(layout.clone());
        self.motion.set_viewport_height(viewport_size.y.max(1.0));
        let cause = self.pending_cause.take().unwrap_or_else(|| {
            if self.frame_layout.is_some() {
                LayoutChangeCause::ViewportResize
            } else {
                LayoutChangeCause::InitialLoad
            }
        });
        let anchor = self.frame_layout.as_deref().and_then(|on_screen| {
            derive_motion_scroll_anchor(
                on_screen,
                &layout,
                session.selections().primary().cursor,
                self.scroll_y,
            )
        });
        let frame = self.motion.commit(
            cx.seconds_since_app_start(),
            self.frame_layout
                .clone()
                .or_else(|| self.previous_layout.clone()),
            layout,
            cause,
            self.reduced_motion,
            anchor,
            MotionConfig {
                duration_seconds: self.motion_duration,
                ease: self.motion_ease,
                ..MotionConfig::default()
            },
        );
        self.frame_layout = Some(frame.layout.clone());
        if frame.active {
            self.next_frame = cx.new_next_frame();
        }
        Ok(frame.layout)
    }

    pub fn handle_input_with_session(
        &mut self,
        cx: &mut Cx,
        session: &mut MarkdownDocumentSession,
        input: EditorInput,
    ) -> Result<Vec<Action>, MarkdownEditorError> {
        if self.read_only
            && matches!(
                input,
                EditorInput::Text(_) | EditorInput::Paste(_) | EditorInput::Cut
            )
        {
            return Ok(Vec::new());
        }
        let layout = match self.frame_layout.as_ref() {
            Some(layout) => layout.clone(),
            None => self.install_layout(cx, session, None)?,
        };
        let old_selection = session.selections().clone();
        let response = self
            .controller
            .handle(session, &layout, input)
            .map_err(MarkdownEditorError::from)?;
        let mut actions: Vec<Action> = response
            .proposals
            .into_iter()
            .map(|proposal| {
                Box::new(WidgetAction {
                    data: None,
                    action: Box::new(MarkdownEditorAction::ProposedEdit(proposal)),
                    widget_uid: self.widget_uid(),
                    group: None,
                }) as Action
            })
            .collect();
        if session.selections() != &old_selection {
            actions.push(self.make_action(MarkdownEditorAction::SelectionChanged));
        }
        if response.request_redraw {
            self.redraw(cx);
        }
        if let Some(point) = response.request_ime_at {
            self.last_ime_point = point;
            cx.show_text_ime(self.scroll_bars.area(), point - dvec2(0.0, self.scroll_y));
        }
        Ok(actions)
    }

    fn make_action(&self, action: MarkdownEditorAction) -> Action {
        Box::new(WidgetAction {
            data: None,
            action: Box::new(action),
            widget_uid: self.widget_uid(),
            group: None,
        })
    }

    fn show_ime(&mut self, cx: &mut Cx, session: &MarkdownDocumentSession) {
        let Some(point) = self.frame_layout.as_ref().and_then(|layout| {
            layout
                .source_to_point(session.selections().primary().cursor)
                .map(|caret| caret.rect.pos)
        }) else {
            return;
        };
        self.last_ime_point = point;
        cx.show_text_ime(self.scroll_bars.area(), point - dvec2(0.0, self.scroll_y));
    }
}

fn underline_rect(rect: Rect) -> Rect {
    Rect {
        pos: dvec2(rect.pos.x, rect.pos.y + (rect.size.y - 2.0).max(0.0)),
        size: dvec2(rect.size.x, rect.size.y.min(2.0)),
    }
}

fn strikethrough_rect(rect: Rect) -> Rect {
    Rect {
        pos: dvec2(
            rect.pos.x,
            rect.pos.y + ((rect.size.y - 2.0) * 0.5).max(0.0),
        ),
        size: dvec2(rect.size.x, rect.size.y.min(2.0)),
    }
}

fn resolved_layout_viewport(area_size: DVec2, requested: Option<DVec2>) -> DVec2 {
    requested
        .filter(|size| size.x > 0.0 && size.y > 0.0)
        .unwrap_or(area_size)
}

fn key_input(event: KeyEvent) -> Option<EditorInput> {
    let extend = event.modifiers.shift;
    let key = match event.key_code {
        KeyCode::ReturnKey | KeyCode::NumpadEnter => EditorKey::Enter,
        KeyCode::Tab if extend => EditorKey::BackTab,
        KeyCode::Tab => EditorKey::Tab,
        KeyCode::Delete => EditorKey::Delete,
        KeyCode::Backspace => EditorKey::Backspace,
        KeyCode::ArrowLeft => EditorKey::Left { extend },
        KeyCode::ArrowRight => EditorKey::Right { extend },
        KeyCode::ArrowUp => EditorKey::Up { extend },
        KeyCode::ArrowDown => EditorKey::Down { extend },
        KeyCode::KeyA if event.modifiers.is_primary() => EditorKey::SelectAll,
        KeyCode::KeyZ if event.modifiers.is_primary() && extend => EditorKey::Redo,
        KeyCode::KeyZ if event.modifiers.is_primary() => EditorKey::Undo,
        _ => return None,
    };
    Some(EditorInput::Key(key))
}

impl MarkdownEditorRef {
    pub fn handle_event_with_session(
        &self,
        cx: &mut Cx,
        event: &Event,
        session: &mut MarkdownDocumentSession,
    ) -> Result<Vec<Action>, MarkdownEditorError> {
        self.borrow_mut()
            .ok_or(MarkdownEditorError::MissingLayoutDocument)?
            .handle_event_with_session(cx, event, session)
    }
    pub fn set_key_focus(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.has_focus = true;
            cx.set_key_focus(inner.scroll_bars.area());
        }
    }

    pub fn redraw(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.redraw(cx);
        }
    }

    pub fn set_read_only(&self, cx: &mut Cx, read_only: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.read_only = read_only;
            if read_only {
                cx.hide_text_ime();
            }
        }
    }

    pub fn set_reduced_motion(&self, cx: &mut Cx, reduced_motion: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.reduced_motion = reduced_motion;
            if reduced_motion {
                if let Some(target) = inner.target_layout.clone() {
                    let previous = inner.frame_layout.clone();
                    let frame = inner.motion.commit(
                        cx.seconds_since_app_start(),
                        previous,
                        target,
                        LayoutChangeCause::ViewportResize,
                        true,
                        None,
                        MotionConfig::default(),
                    );
                    inner.frame_layout = Some(frame.layout);
                    inner.next_frame = NextFrame::default();
                }
            }
            inner.redraw(cx);
        }
    }

    pub fn install_presentation(
        &self,
        cx: &mut Cx,
        presentation: Arc<InstalledPresentation>,
        cause: LayoutChangeCause,
    ) {
        if presentation.validate().is_err() {
            return;
        }
        if let Some(mut inner) = self.borrow_mut() {
            inner.installed = Some(presentation);
            inner.pending_cause = Some(cause);
            inner.target_layout = None;
            inner.redraw(cx);
        }
    }

    pub fn clear_presentation(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.installed = None;
            inner.target_layout = None;
            inner.previous_layout = None;
            inner.frame_layout = None;
            inner.pending_cause = None;
            inner.next_frame = NextFrame::default();
            inner.redraw(cx);
        }
    }

    pub fn target_layout(&self) -> Option<Arc<LayoutSnapshot>> {
        self.borrow().and_then(|inner| inner.target_layout.clone())
    }

    pub fn frame_layout(&self) -> Option<Arc<LayoutSnapshot>> {
        self.borrow().and_then(|inner| inner.frame_layout.clone())
    }

    pub fn proposed_edit(actions: &Actions) -> Option<ProposedMarkdownEdit> {
        actions.iter().find_map(|action| {
            let widget_action = action.downcast_ref::<WidgetAction>()?;
            match widget_action
                .action
                .downcast_ref::<MarkdownEditorAction>()?
            {
                MarkdownEditorAction::ProposedEdit(proposal) => Some(proposal.clone()),
                _ => None,
            }
        })
    }

    pub fn selection_changed(actions: &Actions) -> bool {
        has_action(actions, |action| {
            matches!(action, MarkdownEditorAction::SelectionChanged)
        })
    }

    pub fn navigation_requested(actions: &Actions) -> Option<TextPosition> {
        find_action(actions, |action| match action {
            MarkdownEditorAction::NavigationRequested { position } => Some(*position),
            _ => None,
        })
    }

    pub fn embedded_block_event(
        actions: &Actions,
    ) -> Option<(LayoutElementId, EmbeddedBlockEvent)> {
        find_action(actions, |action| match action {
            MarkdownEditorAction::EmbeddedBlockEvent { id, event } => Some((*id, *event)),
            _ => None,
        })
    }

    pub fn handle_input_with_session(
        &self,
        cx: &mut Cx,
        session: &mut MarkdownDocumentSession,
        input: EditorInput,
    ) -> Result<Vec<Action>, MarkdownEditorError> {
        self.borrow_mut()
            .ok_or(MarkdownEditorError::MissingLayoutDocument)?
            .handle_input_with_session(cx, session, input)
    }

    #[doc(hidden)]
    pub fn test_set_layout(&self, layout: Arc<LayoutSnapshot>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.target_layout = Some(layout.clone());
            inner.frame_layout = Some(layout);
        }
    }

    #[doc(hidden)]
    pub fn test_show_ime(&self, cx: &mut Cx, session: &mut MarkdownDocumentSession) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_ime(cx, session);
        }
    }

    #[doc(hidden)]
    pub fn test_last_ime_point(&self) -> DVec2 {
        self.borrow()
            .map_or(DVec2::default(), |inner| inner.last_ime_point)
    }

    #[doc(hidden)]
    pub fn set_paint_evidence_enabled(&self, enabled: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.paint_evidence.set_enabled(enabled);
        }
    }

    #[doc(hidden)]
    pub fn test_painted_text_ranges(&self) -> Vec<waml_syntax::TextRange> {
        self.borrow()
            .map_or_else(Vec::new, |inner| inner.paint_evidence.ranges().to_vec())
    }

    #[doc(hidden)]
    pub fn test_painted_commands(&self) -> Vec<DrawCommand> {
        self.borrow()
            .map_or_else(Vec::new, |inner| inner.paint_evidence.commands().to_vec())
    }

    #[doc(hidden)]
    pub fn test_painted_glyph_origins(&self) -> Vec<DVec2> {
        self.borrow().map_or_else(Vec::new, |inner| {
            inner.paint_evidence.glyph_origins().to_vec()
        })
    }

    #[doc(hidden)]
    pub fn test_painted_embedded_states(&self) -> Vec<EmbeddedState> {
        self.borrow().map_or_else(Vec::new, |inner| {
            inner.paint_evidence.embedded_states().to_vec()
        })
    }

    #[doc(hidden)]
    pub fn test_paint_evidence_generation(&self) -> u64 {
        self.borrow()
            .map_or(0, |inner| inner.paint_evidence.generation())
    }
}

fn find_action<T>(
    actions: &Actions,
    project: impl Fn(&MarkdownEditorAction) -> Option<T>,
) -> Option<T> {
    actions.iter().find_map(|action| {
        let widget_action = action.downcast_ref::<WidgetAction>()?;
        project(
            widget_action
                .action
                .downcast_ref::<MarkdownEditorAction>()?,
        )
    })
}

fn has_action(actions: &Actions, predicate: impl Fn(&MarkdownEditorAction) -> bool) -> bool {
    find_action(actions, |action| predicate(action).then_some(())).is_some()
}

#[cfg(test)]
mod viewport_tests {
    use super::*;

    #[test]
    fn paint_evidence_is_disabled_without_allocation_by_default() {
        let mut evidence = PaintEvidence::default();

        evidence.begin_frame();
        evidence.record(
            waml_syntax::TextRange::new(
                waml_syntax::TextSize::new(1),
                waml_syntax::TextSize::new(2),
            )
            .unwrap(),
        );

        assert!(!evidence.enabled());
        assert!(evidence.ranges().is_empty());
        assert_eq!(evidence.capacity(), 0);
    }

    #[test]
    fn paint_evidence_records_and_clears_only_after_opt_in() {
        let mut evidence = PaintEvidence::default();
        let range = waml_syntax::TextRange::new(
            waml_syntax::TextSize::new(3),
            waml_syntax::TextSize::new(5),
        )
        .unwrap();

        evidence.set_enabled(true);
        evidence.record(range);
        assert_eq!(evidence.ranges(), &[range]);

        evidence.begin_frame();
        assert!(evidence.enabled());
        assert!(evidence.ranges().is_empty());

        evidence.set_enabled(false);
        assert_eq!(evidence.capacity(), 0);
    }

    #[test]
    fn paint_evidence_generation_advances_only_for_enabled_draws() {
        let mut evidence = PaintEvidence::default();

        evidence.begin_frame();
        assert_eq!(evidence.generation(), 0);

        evidence.set_enabled(true);
        evidence.begin_frame();
        assert_eq!(evidence.generation(), 1);

        evidence.set_enabled(false);
        evidence.begin_frame();
        assert_eq!(evidence.generation(), 1);
    }

    #[test]
    fn positive_pre_draw_viewport_overrides_a_stale_redraw_area() {
        assert_eq!(
            resolved_layout_viewport(dvec2(0.0, 0.0), Some(dvec2(1280.0, 871.0))),
            dvec2(1280.0, 871.0)
        );
    }

    #[test]
    fn invalid_pre_draw_viewport_keeps_the_event_path_area_fallback() {
        assert_eq!(
            resolved_layout_viewport(dvec2(640.0, 480.0), Some(dvec2(0.0, 480.0))),
            dvec2(640.0, 480.0)
        );
    }

    #[test]
    fn image_walk_stays_absolute_with_a_nonzero_turtle_origin() {
        let current_turtle_origin = dvec2(568.0, 29.0);
        let rect = Rect {
            pos: dvec2(24.0, 200.0),
            size: dvec2(96.0, 48.0),
        };
        let walk = Walk::abs_rect(rect);
        assert_eq!(walk.abs_pos, Some(dvec2(24.0, 200.0)));
        assert_ne!(walk.abs_pos, Some(rect.pos - current_turtle_origin));
        assert!(matches!(walk.width, Size::Fixed(value) if value == 96.0));
        assert!(matches!(walk.height, Size::Fixed(value) if value == 48.0));
    }
}
