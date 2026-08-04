use std::{collections::HashMap, path::PathBuf, sync::Arc};

use makepad_widgets::*;
use makepad_widgets::{animator::Ease, shader::draw_text::FontFamily, text::geom::Point};

use crate::{
    edit::ProposedMarkdownEdit,
    gutter::{current_line_bands, gutter_rows, gutter_width, LineNumberMode},
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
        current_line_fill: #00000009
        caret_color: #202124
    }
}

pub fn live_design(cx: &mut Cx) {
    cx.with_vm(register_script_mod);
}

pub(crate) fn register_script_mod(vm: &mut ScriptVm) -> ScriptValue {
    // A child widget is dead and invisible unless its script_mod registers
    // BEFORE its consumer's, so the bullet and the viewer go first.
    crate::reading::script_mod(vm);
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

impl TextFace {
    const COUNT: usize = 8;

    /// Index into `WidgetFonts::faces` / the `#[live] DrawText` fields. Kept as
    /// an explicit match (rather than a `#[repr(usize)]` cast) so a new variant
    /// fails to compile here instead of silently reading the wrong slot.
    fn index(self) -> usize {
        match self {
            TextFace::SansRegular => 0,
            TextFace::SansRegularItalic => 1,
            TextFace::SansSemibold => 2,
            TextFace::SansSemiboldItalic => 3,
            TextFace::MonoRegular => 4,
            TextFace::MonoRegularItalic => 5,
            TextFace::MonoSemibold => 6,
            TextFace::MonoSemiboldItalic => 7,
        }
    }

    const ALL: [TextFace; TextFace::COUNT] = [
        TextFace::SansRegular,
        TextFace::SansRegularItalic,
        TextFace::SansSemibold,
        TextFace::SansSemiboldItalic,
        TextFace::MonoRegular,
        TextFace::MonoRegularItalic,
        TextFace::MonoSemibold,
        TextFace::MonoSemiboldItalic,
    ];
}

/// Digit advance and ascent of the mono face at `GUTTER_FONT_SIZE`, measured
/// through the shaper so a theme or font swap keeps the gutter aligned
/// instead of silently drifting against a hand-picked constant (issue 33).
#[derive(Clone, Copy, Debug, PartialEq)]
struct GutterMetrics {
    digit_width: f64,
    ascent: f64,
}

impl GutterMetrics {
    /// Baked from the shipped mono face at `GUTTER_FONT_SIZE` and at
    /// `font_scale == 1.0`. Used only when shaping is unavailable — e.g. a
    /// headless test with no font backend — so the gutter still renders
    /// something reasonable. Scale it with [`GutterMetrics::scaled`] before
    /// use: the measured path bakes the painter's `font_scale` in, so an
    /// unscaled fallback would be off by exactly that factor.
    const FALLBACK: GutterMetrics = GutterMetrics {
        digit_width: GUTTER_DIGIT_WIDTH,
        ascent: GUTTER_FONT_SIZE as f64 * GUTTER_ASCENT,
    };

    /// These metrics as the painter sees them at `font_scale`.
    fn scaled(self, font_scale: f64) -> GutterMetrics {
        GutterMetrics {
            digit_width: self.digit_width * font_scale,
            ascent: self.ascent * font_scale,
        }
    }
}

#[derive(Default)]
struct WidgetFonts {
    faces: [Option<FontFamily>; TextFace::COUNT],
    /// Measured metrics plus the `font_scale` bits they were measured at
    /// (Task 3, issue 33). Cleared by `install_face`, so a family swap can
    /// never leave a stale digit metric behind, and re-measured when the mono
    /// `DrawText` scale changes, since the measurement bakes that scale in.
    gutter_metrics: Option<(u32, GutterMetrics)>,
}

impl WidgetFonts {
    /// True when the cached face already matches `family`, so the caller can
    /// skip cloning the `FontFamily` on the common no-change install.
    fn face_matches(&self, face: TextFace, family: &FontFamily) -> bool {
        self.faces[face.index()].as_ref() == Some(family)
    }

    /// Installs a refreshed face and drops the measured gutter metrics: a
    /// live-apply or theme rehydrate that swaps the mono family must not keep
    /// painting the gutter with the previous face's digit advance.
    fn install_face(&mut self, face: TextFace, family: FontFamily) {
        self.faces[face.index()] = Some(family);
        self.gutter_metrics = None;
    }

    fn configure_face(&self, face: TextFace, metrics: TextMetrics, draw: &mut DrawText) {
        let font = self.faces[face.index()].as_ref();
        if let Some(font) = font {
            draw.text_style.font_family = font.clone();
        }
        // Makepad reads `font_size` as points and scales it by 96/72 on the way
        // to logical pixels. Our metrics are already logical pixels, so undo
        // that factor here — the shaper and the painter share this seam, so
        // geometry and glyphs stay in step.
        draw.text_style.font_size = metrics.font_size * 0.75;
        draw.text_style.line_spacing = metrics.line_spacing;
    }

    /// Digit width/ascent for the gutter, measured lazily through the mono
    /// `DrawText` and cached until the face or its `font_scale` changes. Falls
    /// back to the documented constants, scaled to the painter's `font_scale`,
    /// when shaping the probe glyph produces nothing usable (e.g. no font
    /// backend in a headless test, or an atlas that is not loaded yet).
    ///
    /// Only a *successful* measurement is cached. A failure is transient —
    /// fonts that have not finished loading, a reset atlas — and caching it
    /// would pin the fallback for the rest of the session, since the only
    /// other invalidation (`install_face`) stops firing once the faces settle.
    fn gutter_metrics(&mut self, cx: &mut Cx, mono: &mut DrawText) -> GutterMetrics {
        let scale_key = mono.font_scale.to_bits();
        if let Some((cached_scale, metrics)) = self.gutter_metrics {
            if cached_scale == scale_key {
                return metrics;
            }
        }
        match measure_gutter_metrics(cx, mono) {
            Some(measured) => {
                self.gutter_metrics = Some((scale_key, measured));
                measured
            }
            None => GutterMetrics::FALLBACK.scaled(mono.font_scale as f64),
        }
    }
}

/// Shapes a single "0" glyph in `mono` at `GUTTER_FONT_SIZE` (matching the
/// size `paint_gutter` renders at) and reads its advance and ascent back from
/// the laid-out row. `None` when the shaper produces a degenerate row (no
/// font backend, zero-size glyph).
fn measure_gutter_metrics(cx: &mut Cx, mono: &mut DrawText) -> Option<GutterMetrics> {
    let saved_font_size = mono.text_style.font_size;
    mono.text_style.font_size = GUTTER_FONT_SIZE * 0.75;
    let laid_out = mono.layout_uncached(
        cx,
        0.0,
        0.0,
        Some(GUTTER_FONT_SIZE * 100.0),
        true,
        Align::default(),
        "0",
    );
    mono.text_style.font_size = saved_font_size;
    let row = laid_out.rows.first()?;
    let paint_scale = mono.font_scale as f64;
    let digit_width = row.width_in_lpxs as f64 * paint_scale;
    let ascent = row.ascender_in_lpxs as f64 * paint_scale;
    (digit_width.is_finite() && digit_width > 0.0 && ascent.is_finite() && ascent > 0.0).then_some(
        GutterMetrics {
            digit_width,
            ascent,
        },
    )
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

/// Gutter type is one size for every line, independent of the styles a line
/// carries, so the numbers form a straight column.
const GUTTER_FONT_SIZE: f32 = 11.0;
/// Advance of one digit in the mono face at `GUTTER_FONT_SIZE`.
const GUTTER_DIGIT_WIDTH: f64 = 6.6;
const GUTTER_GAP: f64 = 10.0;
/// Ascent of the mono face as a fraction of its size, matching the nominal
/// ratio the shaper uses for an empty row.
const GUTTER_ASCENT: f64 = 0.8;

/// The layout/motion pipeline: everything a document swap or a full
/// invalidation must reset together. Reset by whole-struct replacement
/// (`LayoutPipeline::default()`) rather than by field enumeration, so a new
/// field can never be left out of the reset the way `draw_commands_cache`,
/// `scroll_y`, and `motion` were before this struct existed (see issue 33).
#[derive(Default)]
struct LayoutPipeline {
    installed: Option<Arc<InstalledPresentation>>,
    target_layout: Option<Arc<LayoutSnapshot>>,
    previous_layout: Option<Arc<LayoutSnapshot>>,
    frame_layout: Option<Arc<LayoutSnapshot>>,
    motion: MotionController,
    pending_cause: Option<LayoutChangeCause>,
    /// What actually changed since the last layout. A scroll must not claim the
    /// document changed: `Document` invalidates every block, so the measurement
    /// cache would miss on every wheel tick and off-window blocks would fall
    /// back to their one-line estimate — making content height a function of
    /// scroll position.
    pending_invalidation: Option<LayoutInvalidation>,
    /// Content width the installed layout was built at, so a real width change
    /// is still told apart from a scroll.
    last_layout_width: Option<f64>,
    next_frame: NextFrame,
    /// Cached base draw-command list + per-layer visit plan (P-6); see
    /// `cached_draw_commands`. Invalidated by exactly the same events as the
    /// rest of the pipeline, so it lives here rather than as a sibling field.
    draw_commands_cache: Option<DrawCommandsCache>,
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
    pipeline: LayoutPipeline,
    #[rust]
    pointer_drag_active: bool,
    #[rust]
    read_only: bool,
    #[rust]
    line_numbers: LineNumberMode,
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
    current_line_fill: Vec4,
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

/// The draw layers in paint order -- the single sequence `draw_walk` runs.
const DRAW_LAYERS: [DrawLayer; 6] = [
    DrawLayer::BlockBackground,
    DrawLayer::Selection,
    DrawLayer::Text,
    DrawLayer::Decoration,
    DrawLayer::EmbeddedBlock,
    DrawLayer::CaretAndIme,
];

fn layer_slot(layer: DrawLayer) -> usize {
    match layer {
        DrawLayer::BlockBackground => 0,
        DrawLayer::Selection => 1,
        DrawLayer::Text => 2,
        DrawLayer::Decoration => 3,
        DrawLayer::EmbeddedBlock => 4,
        DrawLayer::CaretAndIme => 5,
    }
}

/// Which commands each layer must visit. A `Text` command fans out into text
/// paint operations whose layers depend only on its style (background ->
/// BlockBackground, glyphs -> Text, underline/strikethrough -> Decoration);
/// every other command paints solely on its own layer. Translation moves rects
/// but never layers, so the plan is computed once per cached command list and
/// reused across scroll frames instead of scanning every command per layer.
fn layer_plan(commands: &[DrawCommand]) -> [Vec<usize>; DRAW_LAYERS.len()] {
    let mut plan: [Vec<usize>; DRAW_LAYERS.len()] = Default::default();
    for (index, command) in commands.iter().enumerate() {
        if let DrawCommand::Text { style, .. } = command {
            if style.background.is_some() {
                plan[layer_slot(DrawLayer::BlockBackground)].push(index);
            }
            plan[layer_slot(DrawLayer::Text)].push(index);
            if style.underline || style.strikethrough {
                plan[layer_slot(DrawLayer::Decoration)].push(index);
            }
        } else {
            plan[layer_slot(command.layer())].push(index);
        }
    }
    plan
}

/// P-6: `build_draw_commands` output cached across frames. The build's inputs
/// are the installed presentation (plan/styles/diagnostics/assets/revision --
/// covered by `Arc` identity), the layout snapshot (`Arc` identity; a pure
/// scroll reuses the same snapshot), and the selection set (compared by value;
/// the active-owner set derives from the primary cursor, which lives in it).
/// An in-flight IME composition bypasses the cache entirely -- it is
/// per-keystroke anyway and carries no cheap equality.
struct DrawCommandsCache {
    installed: Arc<InstalledPresentation>,
    layout: Arc<LayoutSnapshot>,
    selections: crate::selection::SelectionSet,
    commands: Arc<[DrawCommand]>,
    plan: Arc<[Vec<usize>; DRAW_LAYERS.len()]>,
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
        if let Some(frame_event) = self.pipeline.next_frame.is_event(event) {
            let frame = self.pipeline.motion.sample(frame_event.time);
            self.pipeline.frame_layout = Some(frame.layout.clone());
            self.scroll_y = frame.scroll_y;
            session.set_scroll(ScrollState {
                x: session.scroll().x,
                y: frame.scroll_y,
            });
            self.scroll_bars
                .set_scroll_pos_no_clip(cx, dvec2(session.scroll().x, frame.scroll_y));
            self.redraw(cx);
            if frame.active {
                self.pipeline.next_frame = cx.new_next_frame();
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
            self.pipeline.target_layout = None;
            self.pipeline.pending_cause = Some(LayoutChangeCause::ViewportResize);
            self.pipeline.pending_invalidation = Some(LayoutInvalidation::Viewport);
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
                let layout = match self.pipeline.frame_layout.as_ref() {
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
                let layout = match self.pipeline.frame_layout.as_ref() {
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
            // Makepad holds whatever cursor a widget last set until another one
            // speaks, so the text surface has to claim `Text` on entry AND hand
            // it back on exit -- otherwise the caret follows the pointer out
            // over the chrome. Claimed read-only too: the selection/copy
            // gestures work there.
            Hit::FingerHoverIn(_) => {
                cx.set_cursor(MouseCursor::Text);
                None
            }
            Hit::FingerHoverOut(_) => {
                cx.set_cursor(MouseCursor::Default);
                None
            }
            Hit::FingerDown(event) if event.is_primary_hit() => {
                cx.set_key_focus(area);
                let gutter = self.gutter_width(cx, session);
                let point =
                    abs_to_layout_point(event.abs, area.rect(cx).pos, gutter, self.scroll_y);
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
            Hit::FingerMove(event) if self.pointer_drag_active => {
                let gutter = self.gutter_width(cx, session);
                Some(EditorInput::PointerMove {
                    point: abs_to_layout_point(event.abs, area.rect(cx).pos, gutter, self.scroll_y),
                })
            }
            Hit::FingerUp(event) if self.pointer_drag_active => {
                self.pointer_drag_active = false;
                if event.was_tap() {
                    let gutter = self.gutter_width(cx, session);
                    let point =
                        abs_to_layout_point(event.abs, area.rect(cx).pos, gutter, self.scroll_y);
                    if event.modifiers.is_primary() {
                        if let (Some(installed), Some(layout)) = (
                            self.pipeline.installed.as_ref(),
                            self.pipeline.frame_layout.as_ref(),
                        ) {
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
        let gutter = self.gutter_width(cx, session);
        // The gutter eats viewport width before wrapping is decided, so text
        // never reflows when the mode is switched mid-session.
        let viewport_size = dvec2((viewport.size.x - gutter).max(1.0), viewport.size.y);
        let layout = self.install_layout(cx, session, Some(viewport_size))?;
        let installed = self
            .pipeline
            .installed
            .as_ref()
            .ok_or(MarkdownEditorError::MissingLayoutDocument)?
            .clone();
        let (base_commands, plan) = self.cached_draw_commands(session, &installed, &layout)?;
        let content_origin = viewport.pos + dvec2(gutter, 0.0) - self.scroll_bars.get_scroll_pos();
        let commands = base_commands
            .iter()
            .map(|command| command.translated(content_origin))
            .collect::<Arc<[_]>>();
        self.scroll_bars.begin(cx, walk, Layout::default());
        self.last_draw = DrawRecorder::default();
        self.paint_evidence.begin_frame();
        self.paint_current_line(cx, session, &layout, viewport, gutter);
        for layer in DRAW_LAYERS {
            self.last_draw.record(layer, &layout);
            let mut primitive_count = 0;
            // Visit only the commands the plan says contribute to this layer
            // (the loop used to rescan the whole list once per layer).
            for &index in plan[layer_slot(layer)].iter() {
                let command = &commands[index];
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
                } else {
                    // Non-text commands are planned only at their own layer.
                    self.paint_command(cx, scope, command);
                    primitive_count += 1;
                }
            }
            self.last_draw.set_last_primitive_count(primitive_count);
        }
        self.paint_gutter(cx, session, &layout, viewport.pos, gutter);
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

    /// The base (untranslated) draw-command list + per-layer visit plan for
    /// this frame, rebuilt only when an input actually changed (P-6). A pure
    /// scroll keeps the installed presentation, layout snapshot, and selection
    /// set identical, so it reuses the cached list instead of re-deriving
    /// every command. See `DrawCommandsCache` for the key's coverage argument.
    #[allow(clippy::type_complexity)]
    fn cached_draw_commands(
        &mut self,
        session: &MarkdownDocumentSession,
        installed: &Arc<InstalledPresentation>,
        layout: &Arc<LayoutSnapshot>,
    ) -> Result<(Arc<[DrawCommand]>, Arc<[Vec<usize>; DRAW_LAYERS.len()]>), MarkdownEditorError>
    {
        let reusable = session.ime().is_none()
            && self
                .pipeline
                .draw_commands_cache
                .as_ref()
                .is_some_and(|cache| {
                    Arc::ptr_eq(&cache.installed, installed)
                        && Arc::ptr_eq(&cache.layout, layout)
                        && cache.selections == *session.selections()
                });
        if !reusable {
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
            let plan = Arc::new(layer_plan(&commands));
            self.pipeline.draw_commands_cache = Some(DrawCommandsCache {
                installed: installed.clone(),
                layout: layout.clone(),
                selections: session.selections().clone(),
                commands,
                plan,
            });
        }
        let cache = self
            .pipeline
            .draw_commands_cache
            .as_ref()
            .expect("cache installed just above");
        Ok((cache.commands.clone(), cache.plan.clone()))
    }

    /// Digit width/ascent for the mono face at `GUTTER_FONT_SIZE`, measured
    /// through the shaper and cached until the next font refresh (Task 3,
    /// issue 33). `fonts` and `draw_text_mono` are disjoint fields, so this
    /// disjoint two-field borrow is fine even though both are `&mut self`.
    fn gutter_metrics(&mut self, cx: &mut Cx) -> GutterMetrics {
        self.fonts.gutter_metrics(cx, &mut self.draw_text_mono)
    }

    /// Logical pixels reserved on the left for line numbers, gap included.
    fn gutter_width(&mut self, cx: &mut Cx, session: &MarkdownDocumentSession) -> f64 {
        if self.line_numbers == LineNumberMode::Off {
            return 0.0;
        }
        let snapshot = session.snapshot();
        let text = snapshot.text();
        let last_line = snapshot
            .line_index()
            .line_col(text, text.len())
            .map_or(0, |at| at.line as usize);
        let digit_width = self.gutter_metrics(cx).digit_width;
        gutter_width(last_line + 1, digit_width, GUTTER_GAP)
    }

    /// Muted band behind every visual row of the cursor's source line. Drawn
    /// before the layer loop so text, selection, and caret all sit on top, and
    /// spanning gutter plus content so the number reads as part of the line.
    fn paint_current_line(
        &mut self,
        cx: &mut Cx2d,
        session: &MarkdownDocumentSession,
        layout: &LayoutSnapshot,
        viewport: Rect,
        gutter: f64,
    ) {
        if self.read_only {
            return;
        }
        let snapshot = session.snapshot();
        let bands = current_line_bands(
            layout,
            snapshot.text(),
            snapshot.line_index(),
            session.selections().primary().cursor.offset,
        );
        let origin_y = viewport.pos.y - self.scroll_bars.get_scroll_pos().y;
        self.draw_background.color = self.current_line_fill;
        for (y, height) in bands {
            self.draw_background.draw_abs(
                cx,
                Rect {
                    pos: dvec2(viewport.pos.x, origin_y + y),
                    size: dvec2(viewport.size.x.max(gutter), height),
                },
            );
        }
    }

    fn paint_gutter(
        &mut self,
        cx: &mut Cx2d,
        session: &MarkdownDocumentSession,
        layout: &LayoutSnapshot,
        viewport_origin: DVec2,
        gutter: f64,
    ) {
        if gutter <= 0.0 {
            return;
        }
        let snapshot = session.snapshot();
        let rows = gutter_rows(
            layout,
            snapshot.text(),
            snapshot.line_index(),
            session.selections().primary().cursor.offset,
            self.line_numbers,
        );
        let origin_y = viewport_origin.y - self.scroll_bars.get_scroll_pos().y;
        let right = viewport_origin.x + gutter - GUTTER_GAP;
        let metrics = self.gutter_metrics(cx);
        self.draw_text_mono.text_style.font_size = GUTTER_FONT_SIZE * 0.75;
        for row in rows {
            self.draw_text_mono.color = if row.current {
                self.body_color
            } else {
                self.marker_color
            };
            // Right-aligned on the measured digit advance of the mono face.
            let x = right - row.label.chars().count() as f64 * metrics.digit_width;
            // `draw_abs` takes the top of the text box, so back the digit's own
            // measured ascent off the line's baseline: the two sit on one
            // baseline even where the line is a heading in a much larger face.
            let top = row.baseline - metrics.ascent;
            self.draw_text_mono
                .draw_abs(cx, dvec2(x, origin_y + top), &row.label);
        }
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
                    crate::presentation::BlockDecorationRole::ListBullet => self.marker_color,
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
        let layout = self.pipeline.frame_layout.as_ref()?;
        let document = &self.pipeline.installed.as_ref()?.layout_document;
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

    /// Single source of truth for the `TextFace` -> `#[live] DrawText` field
    /// mapping; used by both the font-population loop and (indirectly, via
    /// `self.fonts`) `configure_face` at draw time.
    fn draw_text_for(&self, face: TextFace) -> &DrawText {
        match face {
            TextFace::SansRegular => &self.draw_text_sans,
            TextFace::SansRegularItalic => &self.draw_text_sans_italic,
            TextFace::SansSemibold => &self.draw_text_sans_semibold,
            TextFace::SansSemiboldItalic => &self.draw_text_sans_semibold_italic,
            TextFace::MonoRegular => &self.draw_text_mono,
            TextFace::MonoRegularItalic => &self.draw_text_mono_italic,
            TextFace::MonoSemibold => &self.draw_text_mono_semibold,
            TextFace::MonoSemiboldItalic => &self.draw_text_mono_semibold_italic,
        }
    }

    fn install_layout(
        &mut self,
        cx: &mut Cx,
        session: &MarkdownDocumentSession,
        requested_viewport: Option<DVec2>,
    ) -> Result<Arc<LayoutSnapshot>, MarkdownEditorError> {
        let installed = self
            .pipeline
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
        if self.pipeline.pending_cause.is_none() {
            if let Some(layout) = self.pipeline.frame_layout.as_ref() {
                return Ok(layout.clone());
            }
        }
        let viewport_size =
            resolved_layout_viewport(self.scroll_bars.area().rect(cx).size, requested_viewport);
        // Compare before cloning: the steady state costs eight `FontFamily`
        // comparisons instead of eight clones, but a live-apply or theme
        // rehydrate that swaps a `#[live] DrawText` family is still picked up
        // on the next install (and drops the measured gutter metrics with it),
        // so the cache self-heals rather than shaping with a stale face.
        for face in TextFace::ALL {
            let stale = !self
                .fonts
                .face_matches(face, &self.draw_text_for(face).text_style.font_family);
            if stale {
                let family = self.draw_text_for(face).text_style.font_family.clone();
                self.fonts.install_face(face, family);
            }
        }
        // A width change reflows every block whatever the caller claimed, and a
        // caller that claimed nothing has no cache worth trusting.
        let width_changed = self
            .pipeline
            .last_layout_width
            .map_or(true, |width| width.to_bits() != viewport_size.x.to_bits());
        self.pipeline.last_layout_width = Some(viewport_size.x);
        let invalidation = match self.pipeline.pending_invalidation.take() {
            _ if width_changed => LayoutInvalidation::ViewportWidth,
            Some(invalidation) => invalidation,
            None => LayoutInvalidation::Document,
        };
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
                invalidation,
                &mut shaper,
            )
            .map_err(MarkdownEditorError::Layout)?;
        let layout = Arc::new(layout);
        self.pipeline.previous_layout = self.pipeline.target_layout.replace(layout.clone());
        self.pipeline
            .motion
            .set_viewport_height(viewport_size.y.max(1.0));
        let cause = self.pipeline.pending_cause.take().unwrap_or_else(|| {
            if self.pipeline.frame_layout.is_some() {
                LayoutChangeCause::ViewportResize
            } else {
                LayoutChangeCause::InitialLoad
            }
        });
        let anchor = self.pipeline.frame_layout.as_deref().and_then(|on_screen| {
            derive_motion_scroll_anchor(
                on_screen,
                &layout,
                session.selections().primary().cursor,
                self.scroll_y,
            )
        });
        let frame = self.pipeline.motion.commit(
            cx.seconds_since_app_start(),
            self.pipeline
                .frame_layout
                .clone()
                .or_else(|| self.pipeline.previous_layout.clone()),
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
        self.pipeline.frame_layout = Some(frame.layout.clone());
        if frame.active {
            self.pipeline.next_frame = cx.new_next_frame();
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
        let layout = match self.pipeline.frame_layout.as_ref() {
            Some(layout) => layout.clone(),
            None => self.install_layout(cx, session, None)?,
        };
        let old_selection = session.selections().clone();
        let response = match self.controller.handle(session, &layout, input.clone()) {
            Ok(response) => response,
            // The frame layout trails the session by one frame right after an
            // edit, and geometry-driven input (vertical motion, pointer hits)
            // rejects it. Install the current layout and run the input once
            // more rather than dropping the keystroke. When no fresh layout can
            // be built — the presentation itself is stale — the original
            // mismatch is still what the caller needs to see.
            Err(ControllerError::Layout(stale @ LayoutError::RevisionMismatch { .. })) => {
                let retried = self
                    .install_layout(cx, session, None)
                    .and_then(|layout| {
                        self.controller
                            .handle(session, &layout, input)
                            .map_err(MarkdownEditorError::from)
                    })
                    .map_err(|_| MarkdownEditorError::ControllerLayout(stale));
                retried?
            }
            Err(error) => return Err(MarkdownEditorError::from(error)),
        };
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
        let Some(point) = self.pipeline.frame_layout.as_ref().and_then(|layout| {
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

/// Translate a window-absolute pointer position into layout space: remove the
/// widget origin and the line-number gutter, then add back vertical scroll.
///
/// This is the event-side counterpart of the draw-side `content_origin`
/// computation in `draw_walk_with_session`
/// (`viewport.pos + dvec2(gutter, 0.0) - self.scroll_bars.get_scroll_pos()`).
/// Both must agree on the same `gutter` value (`self.gutter_width(session)`)
/// or clicks land offset from what was drawn. Only vertical scroll is added
/// back (matching the widget's current y-only scroll usage) -- widening to a
/// full 2-D scroll is out of scope here.
fn abs_to_layout_point(abs: DVec2, area_origin: DVec2, gutter: f64, scroll_y: f64) -> DVec2 {
    abs - area_origin - dvec2(gutter, 0.0) + dvec2(0.0, scroll_y)
}

#[cfg(test)]
mod abs_to_layout_point_tests {
    use super::*;
    use crate::gutter::gutter_width;

    #[test]
    fn gutter_off_matches_old_translation() {
        let abs = dvec2(120.0, 340.0);
        let origin = dvec2(20.0, 40.0);
        let scroll_y = 15.0;
        let old = abs - origin + dvec2(0.0, scroll_y);
        let new = abs_to_layout_point(abs, origin, 0.0, scroll_y);
        assert_eq!(old, new);
    }

    #[test]
    fn gutter_on_shifts_x_left() {
        let abs = dvec2(120.0, 340.0);
        let origin = dvec2(20.0, 40.0);
        let scroll_y = 15.0;
        let gutter = 36.0;
        let old = abs - origin + dvec2(0.0, scroll_y);
        let new = abs_to_layout_point(abs, origin, gutter, scroll_y);
        assert_eq!(new.x, old.x - gutter);
        assert_eq!(new.y, old.y);
    }

    #[test]
    fn realistic_gutter_value() {
        let abs = dvec2(200.0, 500.0);
        let origin = dvec2(10.0, 10.0);
        let scroll_y = 0.0;
        let gutter = gutter_width(100, GUTTER_DIGIT_WIDTH, GUTTER_GAP);
        let new = abs_to_layout_point(abs, origin, gutter, scroll_y);
        assert_eq!(new, abs - origin - dvec2(gutter, 0.0));
    }
}

#[cfg(test)]
mod text_face_index_tests {
    use super::*;
    use crate::layout::FontWeight;
    use crate::presentation::style::FONT_SANS;

    #[test]
    fn all_metric_combinations_round_trip_through_index() {
        let mut seen = [false; TextFace::COUNT];
        for is_mono in [false, true] {
            for is_bold in [false, true] {
                for is_italic in [false, true] {
                    let metrics = TextMetrics {
                        font: if is_mono { FONT_MONO } else { FONT_SANS },
                        font_size: 16.0,
                        weight: FontWeight(if is_bold { 700 } else { 400 }),
                        italic: is_italic,
                        line_spacing: 1.0,
                    };
                    let face = text_face(metrics);
                    let index = face.index();
                    assert!(index < TextFace::COUNT, "index {index} out of range");
                    assert!(!seen[index], "face {face:?} collided at index {index}");
                    seen[index] = true;
                }
            }
        }
        assert!(seen.iter().all(|&hit| hit), "not every index was reached");
    }
}

#[cfg(test)]
mod gutter_metrics_tests {
    use super::*;

    /// A `Cx` plus the editor's own mono `DrawText`, taken from the widget's
    /// script default so it carries a *resolved* `theme.font_code` family.
    /// A bare `Label` `DrawText` has no resolved family: shaping through it
    /// only logs `type mismatch for property res` and returns nothing, which
    /// silently turned every assertion below into a no-op early return.
    fn mono_probe() -> (Cx, DrawText) {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        crate::live_design(&mut cx);
        let draw_text = cx
            .with_vm(MarkdownEditor::script_new_with_default)
            .draw_text_mono;
        (cx, draw_text)
    }

    #[test]
    fn the_mono_probe_can_actually_shape() {
        let (mut cx, mut draw_text) = mono_probe();
        assert!(
            measure_gutter_metrics(&mut cx, &mut draw_text).is_some(),
            "the editor's mono face must shape in the gate, or every gutter \
             metric test silently degrades to the fallback and asserts nothing",
        );
    }

    #[test]
    fn measured_digit_width_is_within_5_percent_of_the_shipped_fallback() {
        let (mut cx, mut draw_text) = mono_probe();
        let measured =
            measure_gutter_metrics(&mut cx, &mut draw_text).expect("the mono face must shape");
        let fallback = GutterMetrics::FALLBACK.scaled(draw_text.font_scale as f64);
        let tolerance = fallback.digit_width * 0.05;
        assert!(
            (measured.digit_width - fallback.digit_width).abs() <= tolerance,
            "measured {:?} drifted from the fallback {:?} by more than 5% -- \
             the shipped mono face changed, update GUTTER_DIGIT_WIDTH",
            measured,
            fallback,
        );
    }

    #[test]
    fn cache_is_populated_once_and_reused() {
        let (mut cx, mut draw_text) = mono_probe();
        let mut fonts = WidgetFonts::default();
        assert!(fonts.gutter_metrics.is_none());
        let first = fonts.gutter_metrics(&mut cx, &mut draw_text);
        assert!(fonts.gutter_metrics.is_some());
        let second = fonts.gutter_metrics(&mut cx, &mut draw_text);
        assert_eq!(first, second);
    }

    #[test]
    fn a_failed_measurement_is_not_cached() {
        let (mut cx, mut draw_text) = mono_probe();
        let mut fonts = WidgetFonts::default();
        // A zero font_scale makes the probe row degenerate, standing in for a
        // font that has not loaded yet.
        draw_text.font_scale = 0.0;
        let degraded = fonts.gutter_metrics(&mut cx, &mut draw_text);
        assert_eq!(degraded, GutterMetrics::FALLBACK.scaled(0.0));
        assert!(
            fonts.gutter_metrics.is_none(),
            "a failed measurement must not be cached, or the fallback is \
             pinned for the rest of the session",
        );
        draw_text.font_scale = 1.0;
        let healed = fonts.gutter_metrics(&mut cx, &mut draw_text);
        assert!(
            fonts.gutter_metrics.is_some(),
            "the next probe must self-heal once shaping works again",
        );
        assert!(healed.digit_width > 0.0);
    }

    #[test]
    fn the_fallback_follows_the_font_scale() {
        let doubled = GutterMetrics::FALLBACK.scaled(2.0);
        assert_eq!(
            doubled.digit_width,
            GutterMetrics::FALLBACK.digit_width * 2.0
        );
        assert_eq!(doubled.ascent, GutterMetrics::FALLBACK.ascent * 2.0);
    }

    #[test]
    fn cache_is_rekeyed_when_font_scale_changes() {
        let (mut cx, mut draw_text) = mono_probe();
        draw_text.font_scale = 1.0;
        let mut fonts = WidgetFonts::default();
        let at_one = fonts.gutter_metrics(&mut cx, &mut draw_text);
        draw_text.font_scale = 2.0;
        let at_two = fonts.gutter_metrics(&mut cx, &mut draw_text);
        assert!(
            (at_two.digit_width - at_one.digit_width * 2.0).abs() < 0.5,
            "digit width {} did not follow the doubled font_scale from {}",
            at_two.digit_width,
            at_one.digit_width,
        );
    }

    #[test]
    fn installing_a_face_drops_the_measured_metrics() {
        let (mut cx, mut draw_text) = mono_probe();
        let family = draw_text.text_style.font_family.clone();
        let mut fonts = WidgetFonts::default();
        fonts.gutter_metrics(&mut cx, &mut draw_text);
        assert!(fonts.gutter_metrics.is_some());
        assert!(!fonts.face_matches(TextFace::MonoRegular, &family));
        fonts.install_face(TextFace::MonoRegular, family.clone());
        assert!(
            fonts.gutter_metrics.is_none(),
            "a face swap must invalidate the measured gutter metrics",
        );
        assert!(fonts.face_matches(TextFace::MonoRegular, &family));
    }
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

    /// Off, absolute, or cursor-relative line numbers. Changing the mode
    /// changes the reserved width, so the layout is rebuilt on the next draw.
    pub fn set_line_numbers(&self, cx: &mut Cx, mode: LineNumberMode) {
        if let Some(mut inner) = self.borrow_mut() {
            if inner.line_numbers == mode {
                return;
            }
            inner.line_numbers = mode;
            inner.pipeline.target_layout = None;
            inner.pipeline.pending_cause = Some(LayoutChangeCause::ViewportResize);
            inner.pipeline.pending_invalidation = Some(LayoutInvalidation::ViewportWidth);
            inner.redraw(cx);
        }
    }

    pub fn line_numbers(&self) -> LineNumberMode {
        self.borrow()
            .map_or(LineNumberMode::Off, |inner| inner.line_numbers)
    }

    pub fn set_reduced_motion(&self, cx: &mut Cx, reduced_motion: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.reduced_motion = reduced_motion;
            if reduced_motion {
                if let Some(target) = inner.pipeline.target_layout.clone() {
                    let previous = inner.pipeline.frame_layout.clone();
                    let frame = inner.pipeline.motion.commit(
                        cx.seconds_since_app_start(),
                        previous,
                        target,
                        LayoutChangeCause::ViewportResize,
                        true,
                        None,
                        MotionConfig::default(),
                    );
                    inner.pipeline.frame_layout = Some(frame.layout);
                    inner.pipeline.next_frame = NextFrame::default();
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
            inner.pipeline.installed = Some(presentation);
            inner.pipeline.pending_cause = Some(cause);
            inner.pipeline.pending_invalidation = Some(LayoutInvalidation::Document);
            inner.pipeline.target_layout = None;
            inner.redraw(cx);
        }
    }

    pub fn clear_presentation(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            // Whole-struct replacement, not field enumeration: every pipeline
            // field -- including `draw_commands_cache`, which drifted out of
            // the old hand-written reset (issue 33) -- is guaranteed cleared.
            inner.pipeline = LayoutPipeline::default();
            // `scroll_y` (hit-testing) and the scrollbar position (painting)
            // are two halves of the same scroll: zero them together, or the
            // next frame hit-tests against an origin it never drew at --
            // `draw_walk_with_session` only resyncs when the *session* scroll
            // disagrees with the scrollbar, so the split would not self-heal.
            inner.scroll_y = 0.0;
            inner
                .scroll_bars
                .set_scroll_pos_no_clip(cx, DVec2::default());
            inner.redraw(cx);
        }
    }

    pub fn target_layout(&self) -> Option<Arc<LayoutSnapshot>> {
        self.borrow()
            .and_then(|inner| inner.pipeline.target_layout.clone())
    }

    pub fn frame_layout(&self) -> Option<Arc<LayoutSnapshot>> {
        self.borrow()
            .and_then(|inner| inner.pipeline.frame_layout.clone())
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
            inner.pipeline.target_layout = Some(layout.clone());
            inner.pipeline.frame_layout = Some(layout);
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
    pub fn test_gutter_width(&self, cx: &mut Cx, session: &MarkdownDocumentSession) -> f64 {
        self.borrow_mut()
            .map_or(0.0, |mut inner| inner.gutter_width(cx, session))
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

    /// Scrolls both halves of the scroll state -- the hit-test `scroll_y` and
    /// the painted scrollbar position -- so a test can assert they are reset
    /// together.
    #[doc(hidden)]
    pub fn test_set_scroll_y(&self, cx: &mut Cx, scroll_y: f64) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.scroll_y = scroll_y;
            inner
                .scroll_bars
                .set_scroll_pos_no_clip(cx, dvec2(0.0, scroll_y));
        }
    }

    /// `(hit-test scroll_y, painted scrollbar y)`.
    #[doc(hidden)]
    pub fn test_scroll_state(&self) -> (f64, f64) {
        self.borrow().map_or((0.0, 0.0), |inner| {
            (inner.scroll_y, inner.scroll_bars.get_scroll_pos().y)
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

#[cfg(test)]
mod layout_pipeline_tests {
    use super::*;
    use crate::{
        layout::LayoutDocument,
        presentation::{EmbeddedAssetFrame, PresentationPlan, PresentationStyles},
        selection::SelectionSet,
    };
    use waml_syntax::{DocumentRevision, TextSize};

    fn empty_layout_document(revision: DocumentRevision) -> Arc<LayoutDocument> {
        Arc::new(LayoutDocument {
            revision,
            content_insets: Default::default(),
            blocks: Arc::from([]),
            text_runs: Arc::from([]),
            embedded_blocks: Arc::from([]),
        })
    }

    fn installed_presentation() -> Arc<InstalledPresentation> {
        let revision = DocumentRevision::INITIAL;
        InstalledPresentation::new(
            Arc::new(PresentationPlan {
                revision,
                source_len: TextSize::new(0),
                items: Arc::from([]),
                links: Arc::from([]),
                blocks: Arc::from([]),
                diagnostics: Arc::from([]),
            }),
            Arc::new(PresentationStyles::balanced()),
            empty_layout_document(revision),
            Arc::from([]),
            Arc::new(EmbeddedAssetFrame {
                revision,
                items: Arc::from([]),
            }),
        )
        .expect("minimal presentation validates")
    }

    fn layout_snapshot() -> Arc<LayoutSnapshot> {
        Arc::new(LayoutSnapshot::from_parts_for_test(
            DocumentRevision::INITIAL,
            dvec2(0.0, 0.0),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ))
    }

    /// Every pipeline field -- including `draw_commands_cache`, the field
    /// that drifted out of the old hand-enumerated `clear_presentation`
    /// (issue 33) -- must come back to its default value after a whole-struct
    /// reset. This is the mechanism `clear_presentation` now relies on: a
    /// field added to `LayoutPipeline` in the future is reset by construction,
    /// never by remembering to touch a fifth or sixth call site.
    #[test]
    fn default_reset_clears_every_pipeline_field() {
        let installed = installed_presentation();
        let layout = layout_snapshot();

        let mut pipeline = LayoutPipeline {
            installed: Some(installed.clone()),
            target_layout: Some(layout.clone()),
            previous_layout: Some(layout.clone()),
            frame_layout: Some(layout.clone()),
            motion: MotionController::default(),
            pending_cause: Some(LayoutChangeCause::ViewportResize),
            pending_invalidation: Some(LayoutInvalidation::Document),
            last_layout_width: Some(640.0),
            next_frame: NextFrame::default(),
            draw_commands_cache: Some(DrawCommandsCache {
                installed,
                layout: layout.clone(),
                selections: SelectionSet::caret_in_text(
                    DocumentRevision::INITIAL,
                    &waml_syntax::SourceText::new(String::new()).unwrap(),
                    TextSize::new(0),
                )
                .expect("empty caret selection"),
                commands: Arc::from([]),
                plan: Arc::new(Default::default()),
            }),
        };

        assert!(pipeline.installed.is_some());
        assert!(pipeline.target_layout.is_some());
        assert!(pipeline.previous_layout.is_some());
        assert!(pipeline.frame_layout.is_some());
        assert!(pipeline.pending_cause.is_some());
        assert!(pipeline.pending_invalidation.is_some());
        assert!(pipeline.last_layout_width.is_some());
        assert!(pipeline.draw_commands_cache.is_some());

        pipeline = LayoutPipeline::default();

        assert!(pipeline.installed.is_none());
        assert!(pipeline.target_layout.is_none());
        assert!(pipeline.previous_layout.is_none());
        assert!(pipeline.frame_layout.is_none());
        assert!(pipeline.pending_cause.is_none());
        assert!(pipeline.pending_invalidation.is_none());
        assert!(pipeline.last_layout_width.is_none());
        assert!(pipeline.draw_commands_cache.is_none());
    }
}
