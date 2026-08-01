use std::{collections::HashMap, path::PathBuf, sync::Arc};

use makepad_widgets::*;
use makepad_widgets::{animator::Ease, shader::draw_text::FontFamily, text::geom::Point};

use crate::{
    edit::ProposedMarkdownEdit,
    input::{
        ControllerError, EditorInput, EditorKey, MarkdownEditorController, PointerGesture,
        ScrollState, SelectionModifier,
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

#[derive(Default)]
struct WidgetFonts {
    sans: Option<FontFamily>,
    mono: Option<FontFamily>,
}

impl FontResolver for WidgetFonts {
    fn configure_draw_text(&mut self, _key: FontKey, metrics: TextMetrics, draw: &mut DrawText) {
        if let Some(font) = if _key == FONT_MONO {
            self.mono.as_ref()
        } else {
            self.sans.as_ref()
        } {
            draw.text_style.font_family = font.clone();
        }
        draw.text_style.font_size = metrics.font_size;
        draw.text_style.line_spacing = metrics.line_spacing;
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

#[derive(Script, ScriptHook, Widget)]
pub struct MarkdownEditor {
    #[deref]
    view: View,
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
    draw_text_mono: DrawText,
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
}

impl Widget for MarkdownEditor {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {}

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let _ = (&mut self.layout_engine, self.pointer_drag_active);
        if let Some(layout) = &self.frame_layout {
            self.last_draw = DrawRecorder::default();
            draw_visible_layers_for_test(layout, &mut self.last_draw);
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MarkdownEditor {
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
            self.view.redraw(cx);
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
        let input = match event.hits(cx, self.view.area()) {
            Hit::TextInput(event) if !self.read_only => Some(if event.was_paste {
                EditorInput::Paste(Arc::from(event.input.as_str()))
            } else {
                EditorInput::Text(Arc::from(event.input.as_str()))
            }),
            Hit::TextCopy(event) => {
                let layout = match self.frame_layout.as_ref() {
                    Some(layout) => layout.clone(),
                    None => self.install_layout(cx, session)?,
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
                    None => self.install_layout(cx, session)?,
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
                cx.set_key_focus(self.view.area());
                let point = event.abs - self.view.area().rect(cx).pos + dvec2(0.0, self.scroll_y);
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
                point: event.abs - self.view.area().rect(cx).pos + dvec2(0.0, self.scroll_y),
            }),
            Hit::FingerUp(event) if self.pointer_drag_active => {
                self.pointer_drag_active = false;
                if event.was_tap() {
                    let point =
                        event.abs - self.view.area().rect(cx).pos + dvec2(0.0, self.scroll_y);
                    if event.modifiers.is_primary() {
                        if let (Some(installed), Some(layout)) =
                            (self.installed.as_ref(), self.frame_layout.as_ref())
                        {
                            let position = layout.point_to_source(point);
                            if installed.plan.links.iter().any(|link| {
                                link.source_range.start() <= position.offset
                                    && position.offset <= link.source_range.end()
                            }) {
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
        if let Event::Scroll(event) = event {
            if self.view.area().rect(cx).contains(event.abs) && !event.handled_y.get() {
                self.scroll_y = (self.scroll_y + event.scroll.y).max(0.0);
                self.target_layout = None;
                self.pending_cause = Some(LayoutChangeCause::ViewportResize);
                self.view.redraw(cx);
                event.handled_y.set(true);
            }
        }
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
        let layout = self.install_layout(cx, session)?;
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
        self.last_draw = DrawRecorder::default();
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
            for command in commands.iter().filter(|command| command.layer() == layer) {
                self.paint_command(cx, scope, &installed, &layout, command);
                primitive_count += 1;
            }
            self.last_draw.set_last_primitive_count(primitive_count);
        }
        if cx.has_key_focus(self.view.area()) && !self.read_only {
            self.show_ime(cx, session);
        }
        Ok(self.view.draw_walk(cx, scope, walk))
    }

    fn paint_command(
        &mut self,
        cx: &mut Cx2d,
        scope: &mut Scope,
        installed: &InstalledPresentation,
        layout: &LayoutSnapshot,
        command: &DrawCommand,
    ) {
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
            DrawCommand::Text {
                id, range, style, ..
            } => {
                self.paint_text(cx, installed, layout, *id, *range, *style);
            }
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
                    if let Some(image) = self.image_for_source(cx, source) {
                        let _ = image.draw_walk(cx, scope, Walk::abs_rect(*rect));
                    } else {
                        self.draw_embedded.color = self.code_fill;
                        self.draw_embedded.draw_abs(cx, *rect);
                    }
                }
                EmbeddedState::Loading => {
                    self.draw_embedded.color = self.quote_fill;
                    self.draw_embedded.draw_abs(cx, *rect);
                    self.draw_text_sans.color = self.marker_color;
                    self.draw_text_sans
                        .draw_abs(cx, rect.pos + dvec2(8.0, 8.0), "Loading image…");
                }
                EmbeddedState::Failed { message } => {
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

    fn paint_text(
        &mut self,
        cx: &mut Cx2d,
        installed: &InstalledPresentation,
        layout: &LayoutSnapshot,
        id: crate::layout::GeometryElementId,
        range: waml_syntax::TextRange,
        style: crate::presentation::ResolvedTextStyle,
    ) {
        let Some(cluster) = layout
            .glyph_clusters()
            .iter()
            .find(|cluster| cluster.id == id)
        else {
            return;
        };
        let Some((run_id, run_range)) = installed.plan.items.iter().find_map(|item| match item {
            crate::presentation::PresentationItem::TextRun {
                id,
                range: run_range,
                ..
            } if run_range.start() <= range.start() && range.end() <= run_range.end() => Some((
                LayoutElementId {
                    owner: id.owner,
                    fragment_ordinal: id.fragment_ordinal,
                },
                *run_range,
            )),
            _ => None,
        }) else {
            return;
        };
        let Some(laid_out) =
            self.text_layout_cache
                .laid_out(installed.revision, run_id, style.metrics)
        else {
            return;
        };
        self.fonts
            .configure_draw_text(style.metrics.font, style.metrics, &mut self.draw_text_sans);
        let cluster_offset = range
            .start()
            .to_usize()
            .saturating_sub(run_range.start().to_usize());
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
                        x: positioned.origin.x as f32,
                        y: positioned.origin.y as f32,
                    },
                    positioned.font_size,
                    rasterized,
                ))
            })
            .collect::<Vec<_>>();
        self.draw_text_sans.draw_rasterized_glyphs_abs(
            cx,
            &glyphs,
            self.color_for_role(style.color),
        );
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
        let viewport_rect = self.view.area().rect(cx);
        self.fonts.sans = Some(self.draw_text_sans.text_style.font_family.clone());
        self.fonts.mono = Some(self.draw_text_mono.text_style.font_family.clone());
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
                LayoutViewport::new(
                    viewport_rect.size.x.max(1.0),
                    viewport_rect.size.y.max(1.0),
                    self.scroll_y,
                    0.0,
                ),
                LayoutInvalidation::Document,
                &mut shaper,
            )
            .map_err(MarkdownEditorError::Layout)?;
        let layout = Arc::new(layout);
        self.previous_layout = self.target_layout.replace(layout.clone());
        self.motion
            .set_viewport_height(viewport_rect.size.y.max(1.0));
        let cause = self.pending_cause.take().unwrap_or_else(|| {
            if self.frame_layout.is_some() {
                LayoutChangeCause::ViewportResize
            } else {
                LayoutChangeCause::InitialLoad
            }
        });
        let frame = self.motion.commit(
            cx.seconds_since_app_start(),
            self.frame_layout
                .clone()
                .or_else(|| self.previous_layout.clone()),
            layout,
            cause,
            self.reduced_motion,
            None,
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
            None => self.install_layout(cx, session)?,
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
            self.view.redraw(cx);
        }
        if let Some(point) = response.request_ime_at {
            self.last_ime_point = point;
            cx.show_text_ime(self.view.area(), point);
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
        cx.show_text_ime(self.view.area(), point);
    }
}

fn underline_rect(rect: Rect) -> Rect {
    Rect {
        pos: dvec2(rect.pos.x, rect.pos.y + (rect.size.y - 2.0).max(0.0)),
        size: dvec2(rect.size.x, rect.size.y.min(2.0)),
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
            cx.set_key_focus(inner.view.area());
        }
    }

    pub fn redraw(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.view.redraw(cx);
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
            inner.view.redraw(cx);
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
            inner.view.redraw(cx);
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
            inner.view.redraw(cx);
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
