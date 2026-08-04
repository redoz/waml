//! `MarkdownViewer`: a `ReadingDocument` -> `TextFlow` driver.
//!
//! This widget owns NO layout engine. `TextFlow` already provides block flow,
//! selection, copy and `point_to_index`; the driver's whole job is to walk the
//! reading model and issue the matching `TextFlow` calls, drawing markers as
//! decorations and recording a `SourceMap` as it goes.
//!
//! makepad's own `Markdown` widget was rejected: it calls `pulldown_cmark`
//! itself, which would mean two independent parses of one document, and it
//! cannot see `MarkdownDialect`.

use std::{ops::Range, sync::Arc};

use makepad_widgets::*;
use waml_syntax::{TextRange, TextSize};

use crate::presentation::{FontSizeRole, TextRole};

use super::{
    bullet::{bullet_shape_for_level, DrawReadingBullet},
    ReadingBlock, ReadingBlockKind, ReadingDocument, ReadingPiece,
};

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.widgets.*

    mod.widgets.MarkdownViewerBase = #(MarkdownViewer::register_widget(vm))

    mod.widgets.MarkdownViewer = set_type_default() do mod.widgets.MarkdownViewerBase {
        width: Fill
        height: Fill
        flow: Down
        flow_body := TextFlow{
            width: Fill
            height: Fit
            selectable: true
        }
    }
}

/// One contiguous stretch of `TextFlow`'s selection buffer and the source it
/// came from. `source: None` marks a structural gap TextFlow injected itself
/// (`push_newline` from `end_list_item`, `end_quote`, `end_code`,
/// `new_line_collapsed`), which no source byte backs.
#[derive(Clone, Debug, PartialEq)]
struct MapPiece {
    flow: Range<usize>,
    source: Option<TextRange>,
}

#[derive(Clone, Debug, Default)]
pub struct SourceMap {
    pieces: Vec<MapPiece>,
}

impl SourceMap {
    pub fn clear(&mut self) {
        self.pieces.clear();
    }

    pub fn push(&mut self, flow: Range<usize>, source: Option<TextRange>) {
        if flow.is_empty() {
            return;
        }
        self.pieces.push(MapPiece { flow, source });
    }

    pub fn is_empty(&self) -> bool {
        self.pieces.is_empty()
    }

    /// The source offset a flow index points at. An index inside a structural
    /// gap falls FORWARD to the next real piece; an index past the end lands
    /// on the end of the last real piece.
    pub fn source_offset(&self, flow_index: usize) -> Option<TextSize> {
        let mut last_end = None;
        for piece in &self.pieces {
            if let Some(source) = piece.source {
                if flow_index < piece.flow.end {
                    let within = flow_index.saturating_sub(piece.flow.start);
                    let offset = source.start().to_usize() + within;
                    return TextSize::try_from_usize(offset.min(source.end().to_usize())).ok();
                }
                last_end = Some(source.end());
            } else if flow_index < piece.flow.end {
                // Inside a gap: fall forward to the next real piece.
                return self
                    .pieces
                    .iter()
                    .find(|next| next.flow.start >= piece.flow.end && next.source.is_some())
                    .and_then(|next| next.source.map(|source| source.start()))
                    .or(last_end);
            }
        }
        last_end
    }

    /// The source span a flow span covers, or `None` when the span touches no
    /// source-backed piece.
    pub fn source_span(&self, flow: Range<usize>) -> Option<TextRange> {
        let mut start: Option<TextSize> = None;
        let mut end: Option<TextSize> = None;
        for piece in &self.pieces {
            let Some(source) = piece.source else { continue };
            if piece.flow.end <= flow.start || piece.flow.start >= flow.end {
                continue;
            }
            let lead = flow.start.saturating_sub(piece.flow.start);
            let trail = piece.flow.end.saturating_sub(flow.end);
            let piece_start = (source.start().to_usize() + lead).min(source.end().to_usize());
            let piece_end = source
                .end()
                .to_usize()
                .saturating_sub(trail)
                .max(piece_start);
            let piece_start = TextSize::try_from_usize(piece_start).ok()?;
            let piece_end = TextSize::try_from_usize(piece_end).ok()?;
            start = Some(start.map_or(piece_start, |value: TextSize| value.min(piece_start)));
            end = Some(end.map_or(piece_end, |value: TextSize| value.max(piece_end)));
        }
        TextRange::new(start?, end?).ok()
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct MarkdownViewer {
    #[deref]
    view: View,
    #[live]
    draw_bullet: DrawReadingBullet,
    /// Side of the bullet, as a fraction of the body font size.
    #[live(0.30)]
    bullet_scale: f64,
    /// Width of the hanging-marker column, as a fraction of the font size.
    #[live(1.2)]
    bullet_gutter_scale: f64,
    /// `begin_list_item_gutter`'s `pad`, in font-size multiples.
    #[live(1.0)]
    list_indent_scale: f64,

    #[rust]
    document: Option<Arc<ReadingDocument>>,
    #[rust]
    source: Option<Arc<str>>,
    #[rust]
    source_map: SourceMap,
}

impl MarkdownViewer {
    pub fn install_document(
        &mut self,
        cx: &mut Cx,
        document: Arc<ReadingDocument>,
        source: Arc<str>,
    ) {
        self.document = Some(document);
        self.source = Some(source);
        self.source_map.clear();
        self.redraw(cx);
    }

    pub fn source_map(&self) -> &SourceMap {
        &self.source_map
    }

    fn flow(&self, cx: &Cx) -> TextFlowRef {
        self.view.text_flow(cx, ids!(flow_body))
    }

    /// Draws one piece and records its flow span. `TextFlow::draw_text` trims
    /// leading whitespace ONLY when the run is first on a line, and always
    /// trims trailing newlines, before pushing the result into the selection
    /// buffer. The driver mirrors that same decision itself (via
    /// `first_on_line`) and adjusts the recorded range to match; trimming a
    /// leading space unconditionally would swallow the space between two
    /// words on the same line, since consecutive inline runs (e.g. plain text
    /// then `code`) each carry their own leading whitespace.
    fn draw_piece(
        flow: &mut TextFlow,
        map: &mut SourceMap,
        cx: &mut Cx2d,
        source: &str,
        piece: &ReadingPiece,
        first_on_line: &mut bool,
    ) {
        Self::draw_piece_wrapped(flow, map, cx, source, piece, first_on_line, true)
    }

    /// `soft_wrap` turns a source newline into a joining space so reflowed
    /// prose keeps its word boundaries ("from\nit" reads "from it"); code
    /// blocks pass `false` because their newlines are layout.
    #[allow(clippy::too_many_arguments)]
    fn draw_piece_wrapped(
        flow: &mut TextFlow,
        map: &mut SourceMap,
        cx: &mut Cx2d,
        source: &str,
        piece: &ReadingPiece,
        first_on_line: &mut bool,
        soft_wrap: bool,
    ) {
        if !piece.emit {
            return;
        }
        let start = piece.range.start().to_usize();
        let end = piece.range.end().to_usize();
        let Some(raw) = source.get(start..end) else {
            return;
        };
        let start_trimmed = if *first_on_line {
            raw.trim_start()
        } else {
            raw
        };
        let trimmed = start_trimmed.trim_end_matches('\n');
        if trimmed.is_empty() {
            return;
        }
        let lead = raw.len() - start_trimmed.len();
        let range = TextRange::new(
            TextSize::try_from_usize(start + lead).expect("in range"),
            TextSize::try_from_usize(start + lead + trimmed.len()).expect("in range"),
        )
        .expect("ordered");

        // Same byte length as `trimmed`, so the source map stays aligned.
        let joined;
        let drawn = if soft_wrap && trimmed.contains('\n') {
            joined = trimmed.replace('\n', " ");
            joined.as_str()
        } else {
            trimmed
        };

        let before = flow.text_len();
        flow.draw_text(cx, drawn);
        let after = flow.text_len();
        debug_assert_eq!(
            after - before,
            trimmed.len(),
            "TextFlow reshaped the run; the source map would drift"
        );
        map.push(before..after, Some(range));
        *first_on_line = false;
    }

    fn draw_block(&mut self, cx: &mut Cx2d, block: &ReadingBlock, source: &str) {
        let flow_ref = self.flow(cx);
        let Some(mut flow) = flow_ref.borrow_mut() else {
            return;
        };
        match block.kind {
            ReadingBlockKind::Heading(level) => {
                let scale = match level {
                    1 => 1.8,
                    2 => 1.5,
                    3 => 1.3,
                    4 => 1.15,
                    5 => 1.05,
                    _ => 1.0,
                };
                flow.push_size_abs_scale(scale);
                flow.bold.push();
                let mut first_on_line = true;
                for piece in &block.pieces {
                    Self::draw_piece(
                        &mut flow,
                        &mut self.source_map,
                        cx,
                        source,
                        piece,
                        &mut first_on_line,
                    );
                }
                flow.bold.pop();
                flow.font_sizes.pop();
                flow.new_line_collapsed(cx);
                let total = flow.text_len();
                self.source_map.push(total.saturating_sub(1)..total, None);
            }
            ReadingBlockKind::BulletItem { level } | ReadingBlockKind::OrderedItem { level } => {
                let font_size = flow.font_size as f64;
                let gutter = font_size * self.bullet_gutter_scale;
                let rect =
                    flow.begin_list_item_gutter(cx, gutter, self.list_indent_scale * level as f64);
                if matches!(block.kind, ReadingBlockKind::BulletItem { .. }) {
                    let size = font_size * self.bullet_scale;
                    self.draw_bullet.shape = bullet_shape_for_level(level).shader_index();
                    self.draw_bullet.draw_abs(
                        cx,
                        Rect {
                            pos: dvec2(
                                rect.pos.x + (gutter - size) * 0.5,
                                rect.pos.y + (rect.size.y - size) * 0.5,
                            ),
                            size: dvec2(size, size),
                        },
                    );
                }
                let mut first_on_line = true;
                for piece in &block.pieces {
                    Self::draw_piece(
                        &mut flow,
                        &mut self.source_map,
                        cx,
                        source,
                        piece,
                        &mut first_on_line,
                    );
                }
                drop(flow);
                self.draw_children(cx, block, source);
                let Some(mut flow) = flow_ref.borrow_mut() else {
                    return;
                };
                let before = flow.text_len();
                flow.end_list_item(cx);
                let after = flow.text_len();
                self.source_map.push(before..after, None);
                return;
            }
            ReadingBlockKind::Quote => {
                flow.begin_quote(cx);
                let mut first_on_line = true;
                for piece in &block.pieces {
                    Self::draw_piece(
                        &mut flow,
                        &mut self.source_map,
                        cx,
                        source,
                        piece,
                        &mut first_on_line,
                    );
                }
                drop(flow);
                self.draw_children(cx, block, source);
                let Some(mut flow) = flow_ref.borrow_mut() else {
                    return;
                };
                let before = flow.text_len();
                flow.end_quote(cx);
                let after = flow.text_len();
                self.source_map.push(before..after, None);
                return;
            }
            ReadingBlockKind::Code => {
                flow.begin_code(cx);
                flow.fixed.push();
                let mut first_on_line = true;
                for piece in &block.pieces {
                    Self::draw_piece_wrapped(
                        &mut flow,
                        &mut self.source_map,
                        cx,
                        source,
                        piece,
                        &mut first_on_line,
                        false,
                    );
                }
                flow.fixed.pop();
                let before = flow.text_len();
                flow.end_code(cx);
                let after = flow.text_len();
                self.source_map.push(before..after, None);
            }
            ReadingBlockKind::ThematicBreak => {
                flow.sep(cx);
            }
            _ => {
                let mut first_on_line = true;
                for piece in &block.pieces {
                    let style_is_code = matches!(piece.style.size, FontSizeRole::Code)
                        || piece.role == TextRole::InlineCode;
                    let emphasised = piece.style.italic;
                    let strong = matches!(piece.role, TextRole::Strong | TextRole::StrongEmphasis);
                    if style_is_code {
                        flow.inline_code.push();
                        flow.fixed.push();
                    }
                    if emphasised {
                        flow.italic.push();
                    }
                    if strong {
                        flow.bold.push();
                    }
                    Self::draw_piece(
                        &mut flow,
                        &mut self.source_map,
                        cx,
                        source,
                        piece,
                        &mut first_on_line,
                    );
                    if strong {
                        flow.bold.pop();
                    }
                    if emphasised {
                        flow.italic.pop();
                    }
                    if style_is_code {
                        flow.fixed.pop();
                        flow.inline_code.pop();
                    }
                }
                if !block.pieces.is_empty() {
                    let before = flow.text_len();
                    flow.new_line_collapsed(cx);
                    let after = flow.text_len();
                    self.source_map.push(before..after, None);
                }
            }
        }
        drop(flow);
        self.draw_children(cx, block, source);
    }

    fn draw_children(&mut self, cx: &mut Cx2d, block: &ReadingBlock, source: &str) {
        // `children` is cloned so the recursive borrow of `self` is legal. A
        // reading model is per-revision and editor-sized, so the clone is not
        // on any hot path; `install_document` runs once per compile.
        let children = block.children.clone();
        for child in &children {
            self.draw_block(cx, child, source);
        }
    }
}

impl Widget for MarkdownViewer {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let (Some(document), Some(source)) = (self.document.clone(), self.source.clone()) else {
            return self.view.draw_walk(cx, scope, walk);
        };
        self.source_map.clear();
        let flow_ref = self.flow(cx);
        if let Some(mut flow) = flow_ref.borrow_mut() {
            flow.begin(cx, walk);
        }
        for block in document.roots.clone() {
            self.draw_block(cx, &block, &source);
        }
        if let Some(mut flow) = flow_ref.borrow_mut() {
            flow.end(cx);
        }
        DrawStep::done()
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Selection, copy and point_to_index are TextFlow's, not ours.
        self.view.handle_event(cx, event, scope);
    }
}

/// The caret a source handoff should carry into the editor: the start of the
/// reader's selection, or the top of the document when nothing is selected.
/// Free-standing so it is testable without a live widget tree.
pub fn caret_for_span(span: Option<TextRange>) -> TextSize {
    span.map(|span| span.start())
        .unwrap_or_else(|| TextSize::try_from_usize(0).expect("zero is always in range"))
}

impl MarkdownViewerRef {
    pub fn install_document(&self, cx: &mut Cx, document: Arc<ReadingDocument>, source: Arc<str>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.install_document(cx, document, source);
        }
    }

    pub fn selected_source_span(&self, cx: &Cx) -> Option<TextRange> {
        let inner = self.borrow()?;
        let flow = inner.flow(cx);
        let flow = flow.borrow()?;
        let (start, end) = flow.selection_range()?;
        inner.source_map.source_span(start..end)
    }

    /// The caret a source handoff carries into the editor.
    pub fn caret_for_handoff(&self, cx: &Cx) -> TextSize {
        caret_for_span(self.selected_source_span(cx))
    }
}
