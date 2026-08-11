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

/// Turtle units are logical pixels; font sizes are points.
const LPXS_PER_PT: f64 = 96.0 / 72.0;

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

            // Reading typography (docs/design/markdown-typesetting.md): body
            // prose at 12pt, and ONE line_spacing across all five styles.
            // TextFlow's wrap gap is `ascender * (line_spacing - 1)` of the
            // most spaced run on the line (sticky max per turtle), so any
            // style that deviates -- the theme's font_code carries 1.35 vs
            // font_regular's 1.2 -- makes lines containing that style taller
            // than their neighbours.
            font_size: 12
            text_style_normal: theme.font_regular{
                font_size: 12
                line_spacing: 1.3
            }
            text_style_italic: theme.font_italic{
                font_size: 12
                line_spacing: 1.3
            }
            text_style_bold: theme.font_bold{
                font_size: 12
                line_spacing: 1.3
            }
            text_style_bold_italic: theme.font_bold_italic{
                font_size: 12
                line_spacing: 1.3
            }
            text_style_fixed: theme.font_code{
                font_size: 12
                line_spacing: 1.3
            }

            // Inline code must not change the line's height: TextFlow grows
            // the row by the box's vertical padding + margin, so keep those
            // near zero and let the box hug the glyphs.
            inline_code_padding: Inset{left: 3, right: 3, top: 1, bottom: 1}
            inline_code_margin: Inset{left: 2, right: 2, top: 0, bottom: 0}
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
    /// Per drawn text run: the source range it came from, and the slot range
    /// its per-row rects were tracked into (`TextFlow::areas_tracker.areas`).
    /// `TextFlow`'s own `selection_rects` is behind a private field on the
    /// current makepad pin, so this tracker is the one public route from a
    /// source range to the pixels it occupies -- what
    /// `MarkdownViewer::draw_search_highlights` paints over.
    runs: Vec<(TextRange, Range<usize>)>,
}

impl SourceMap {
    pub fn clear(&mut self) {
        self.pieces.clear();
        self.runs.clear();
    }

    pub fn push(&mut self, flow: Range<usize>, source: Option<TextRange>) {
        if flow.is_empty() {
            return;
        }
        self.pieces.push(MapPiece { flow, source });
    }

    /// `push`, plus the area slots this run's rows were tracked into.
    pub fn push_run(&mut self, flow: Range<usize>, source: TextRange, areas: Range<usize>) {
        self.push(flow, Some(source));
        if !areas.is_empty() {
            self.runs.push((source, areas));
        }
    }

    /// Area slots of every drawn run overlapping `source`.
    pub fn area_slots_for_source(&self, source: TextRange) -> Vec<Range<usize>> {
        self.runs
            .iter()
            .filter(|(run, _)| run.end() > source.start() && run.start() < source.end())
            .map(|(_, areas)| areas.clone())
            .collect()
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
    /// Maximum column width, in ems of the body font size (~70 characters at
    /// the default 38). Leftover width becomes symmetric side margins, so the
    /// column centres instead of stretching the measure across the window.
    #[live(38.0)]
    max_measure_em: f64,
    /// Vertical gap before a sibling block, in ems of the body size.
    /// Space-below is always the NEXT block's own gap: no after-margins, so
    /// there is no CSS-style margin collapsing to reason about.
    #[live(0.75)]
    block_gap_em: f64,
    /// Gap before a heading (headings bind to what follows: the gap above
    /// must be larger than the gap below).
    #[live(1.5)]
    heading_gap_em: f64,
    /// Gap between sibling list items.
    #[live(0.25)]
    list_item_gap_em: f64,
    /// Headings run tighter leading than prose: large sizes exaggerate the
    /// gap, and a wrapped heading must read as one unit.
    #[live(1.1)]
    heading_line_spacing: f32,

    #[rust]
    document: Option<Arc<ReadingDocument>>,
    #[rust]
    source: Option<Arc<str>>,
    #[rust]
    source_map: SourceMap,
    /// Source ranges the active search query hit (spec §DocView::reveal),
    /// installed by `set_search_highlights`. Mapped through `source_map` at
    /// paint time -- stale once the document changes underneath them, same
    /// as every other source-addressed overlay here, so a caller that swaps
    /// documents is responsible for reissuing or clearing them.
    #[rust]
    search_highlights: Arc<[TextRange]>,
    /// Height, in logical pixels, the installed document actually drew at on
    /// the last `draw_walk` -- the `TextFlow`'s turtle-ended rect, recorded
    /// right after `flow.end`. Zero until the first draw with a document
    /// installed (and reset by `install_document`), so callers can tell "not
    /// yet measured" from a real measurement with a `> 0.0` guard.
    /// `BookSurface` reads it to size a prose section to what actually drew.
    #[rust]
    content_height: f64,
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
        // The old document's drawn height says nothing about the new one.
        self.content_height = 0.0;
        self.redraw(cx);
    }

    /// See the `content_height` field: the installed document's drawn height
    /// from the last `draw_walk`, zero before any such draw.
    pub fn content_height(&self) -> f64 {
        self.content_height
    }

    pub fn source_map(&self) -> &SourceMap {
        &self.source_map
    }

    /// Install the active search query's hit ranges, replacing whatever set
    /// was installed before. An empty `Vec` is the same as
    /// `clear_search_highlights`.
    pub fn set_search_highlights(&mut self, cx: &mut Cx, ranges: Vec<TextRange>) {
        self.search_highlights = ranges.into();
        self.redraw(cx);
    }

    /// Drop the installed search highlights. A no-op (no redraw) when
    /// nothing is installed.
    pub fn clear_search_highlights(&mut self, cx: &mut Cx) {
        if self.search_highlights.is_empty() {
            return;
        }
        self.search_highlights = Arc::from([]);
        self.redraw(cx);
    }

    fn flow(&self, cx: &Cx) -> TextFlowRef {
        self.view.text_flow(cx, ids!(flow_body))
    }

    /// Paints the installed search highlights as a translucent fill over the
    /// matched text, reusing `TextFlow`'s own selection color and draw shape
    /// -- the same channel the raw editor's `SearchMatch` decoration uses --
    /// rather than inventing a new theming channel.
    ///
    /// The rects come from the per-run area slots `draw_piece_wrapped`
    /// recorded (`SourceMap::area_slots_for_source`): `TextFlow`'s own
    /// `SelectionTracker::selection_rects` sits behind a private field on
    /// this makepad pin (7bb5e50a), and the only public selection mutator
    /// (`set_selection`) drives the reader's own click-drag selection, which
    /// this must not fight. Granularity is therefore one rect per drawn ROW
    /// of a matched run -- which is exactly the granularity of a search hit,
    /// whose target span IS a text run.
    fn draw_search_highlights(&mut self, cx: &mut Cx2d) {
        if self.search_highlights.is_empty() {
            return;
        }
        let rects = self.highlight_rects(cx);
        if rects.is_empty() {
            return;
        }
        let flow_ref = self.flow(cx);
        let Some(mut flow) = flow_ref.borrow_mut() else {
            return;
        };
        for rect in rects {
            flow.draw_selection.draw_abs(cx, rect);
        }
    }

    /// Window-space rects of every installed highlight, from the LAST draw's
    /// recorded run areas. Empty before the first draw of a document.
    fn highlight_rects(&self, cx: &Cx) -> Vec<Rect> {
        let flow_ref = self.flow(cx);
        let Some(flow) = flow_ref.borrow() else {
            return Vec::new();
        };
        let mut rects = Vec::new();
        for highlight in self.search_highlights.iter() {
            for slots in self.source_map.area_slots_for_source(*highlight) {
                for slot in slots {
                    let Some(area) = flow.areas_tracker.areas.get(slot) else {
                        continue;
                    };
                    let rect = area.rect(cx);
                    if rect.size.x > 0.0 && rect.size.y > 0.0 {
                        rects.push(rect);
                    }
                }
            }
        }
        rects
    }

    /// Top of the drawn document, from the last draw's tracked run rects.
    fn content_top(&self, cx: &Cx) -> Option<f64> {
        let flow_ref = self.flow(cx);
        let flow = flow_ref.borrow()?;
        flow.areas_tracker
            .areas
            .iter()
            .map(|area| area.rect(cx))
            .filter(|rect| rect.size.y > 0.0)
            .fold(None, |acc: Option<f64>, rect| {
                Some(acc.map_or(rect.pos.y, |top| top.min(rect.pos.y)))
            })
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
        // Bracket the run so `TextFlow` records one area per drawn ROW into
        // its rect tracker: that is the only public handle on where this
        // source range actually landed in pixels (search-highlight painting).
        flow.areas_tracker.push_tracker();
        flow.draw_text(cx, drawn);
        let (area_start, area_end) = flow.areas_tracker.pop_tracker();
        let after = flow.text_len();
        debug_assert_eq!(
            after - before,
            trimmed.len(),
            "TextFlow reshaped the run; the source map would drift"
        );
        map.push_run(before..after, range, area_start..area_end);
        *first_on_line = false;
    }

    fn draw_block(&mut self, cx: &mut Cx2d, block: &ReadingBlock, source: &str) {
        let flow_ref = self.flow(cx);
        let Some(mut flow) = flow_ref.borrow_mut() else {
            return;
        };
        match block.kind {
            ReadingBlockKind::Heading(level) => {
                // GitHub's markdown ladder; h5/h6 drop below body size.
                let scale = match level {
                    1 => 2.0,
                    2 => 1.5,
                    3 => 1.25,
                    4 => 1.0,
                    5 => 0.875,
                    _ => 0.85,
                };
                flow.push_size_abs_scale(scale);
                let saved_bold = flow.text_style_bold.line_spacing;
                let saved_bold_italic = flow.text_style_bold_italic.line_spacing;
                flow.text_style_bold.line_spacing = self.heading_line_spacing;
                flow.text_style_bold_italic.line_spacing = self.heading_line_spacing;
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
                flow.text_style_bold.line_spacing = saved_bold;
                flow.text_style_bold_italic.line_spacing = saved_bold_italic;
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
                    // The gutter rect is zero-height (a `walk_margin` box), so
                    // centring on it parks the bullet above the line. Aim at
                    // the optical middle of the first text line instead:
                    // ~0.55 em below the line top sits between the x-height
                    // centre and the cap centre.
                    let line_middle = font_size * LPXS_PER_PT * 0.55;
                    self.draw_bullet.shape = bullet_shape_for_level(level).shader_index();
                    self.draw_bullet.draw_abs(
                        cx,
                        Rect {
                            pos: dvec2(
                                rect.pos.x + (gutter - size) * 0.5,
                                rect.pos.y + line_middle - size * 0.5,
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
                // A nested list must start on its own line inside the item's
                // turtle; without this the first child rides the tail of the
                // item's text line.
                if !block.children.is_empty() {
                    let before = flow.text_len();
                    flow.new_line_collapsed(cx);
                    let after = flow.text_len();
                    self.source_map.push(before..after, None);
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
                // Same as list items: child blocks must not ride the tail of
                // the quote's own text line.
                if !block.pieces.is_empty() && !block.children.is_empty() {
                    let before = flow.text_len();
                    flow.new_line_collapsed(cx);
                    let after = flow.text_len();
                    self.source_map.push(before..after, None);
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
                // Mono runs visually larger at equal nominal size; code
                // blocks drop to the fixed scale like inline code does.
                let fixed_scale = flow.fixed_font_size_scale;
                flow.push_size_rel_scale(fixed_scale);
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
                flow.font_sizes.pop();
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
                        let fixed_scale = flow.fixed_font_size_scale;
                        flow.push_size_rel_scale(fixed_scale);
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
                        flow.font_sizes.pop();
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
        // `block` borrows the Arc'd document held by `draw_walk`, not `self`,
        // so recursing with `&mut self` needs no clone.
        self.draw_siblings(cx, &block.children, source);
    }

    /// Draws a run of sibling blocks with the vertical gap each block asks
    /// for BEFORE it (never before the first sibling — the `:first-child`
    /// no-top-margin rule falls out for free, and nested containers work
    /// because the gap lands inside whatever turtle the container opened).
    fn draw_siblings(&mut self, cx: &mut Cx2d, blocks: &[ReadingBlock], source: &str) {
        for (index, block) in blocks.iter().enumerate() {
            if index > 0 {
                self.block_gap(cx, self.gap_before_em(block.kind));
            }
            self.draw_block(cx, block, source);
        }
    }

    /// The gap a block wants above itself, in ems of the body size.
    fn gap_before_em(&self, kind: ReadingBlockKind) -> f64 {
        match kind {
            ReadingBlockKind::Heading(_) => self.heading_gap_em,
            ReadingBlockKind::BulletItem { .. } | ReadingBlockKind::OrderedItem { .. } => {
                self.list_item_gap_em
            }
            // Rows flatten to paragraph flow today; keep them tight so a
            // flattened table still reads as one unit.
            ReadingBlockKind::TableRow | ReadingBlockKind::TableCell { .. } => 0.0,
            _ => self.block_gap_em,
        }
    }

    /// Emits `em_scale` ems of vertical space (a structural gap: recorded in
    /// the source map exactly like the newline bookkeeping, backed by no
    /// source byte).
    fn block_gap(&mut self, cx: &mut Cx2d, em_scale: f64) {
        if em_scale <= 0.0 {
            return;
        }
        let flow_ref = self.flow(cx);
        let Some(mut flow) = flow_ref.borrow_mut() else {
            return;
        };
        let gap = flow.font_size as f64 * LPXS_PER_PT * em_scale;
        let before = flow.text_len();
        flow.new_line_collapsed_with_spacing(cx, gap);
        let after = flow.text_len();
        self.source_map.push(before..after, None);
    }
}

impl Widget for MarkdownViewer {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let (Some(document), Some(source)) = (self.document.clone(), self.source.clone()) else {
            return self.view.draw_walk(cx, scope, walk);
        };
        self.source_map.clear();
        let flow_ref = self.flow(cx);
        // Clamp the column to a readable measure and centre it: leftover
        // width becomes symmetric side margins on the flow's walk.
        let walk = {
            let measure = flow_ref
                .borrow()
                .map(|flow| flow.font_size as f64 * LPXS_PER_PT * self.max_measure_em);
            match measure {
                Some(measure) => {
                    // An unresolved parent width peeks as NaN; NaN margins
                    // would blank the whole surface, so clamp only when the
                    // available width is a real number.
                    let avail = cx.peek_walk_turtle(walk).size.x;
                    let side = if avail.is_finite() {
                        ((avail - measure) * 0.5).max(0.0)
                    } else {
                        0.0
                    };
                    Walk {
                        margin: Inset {
                            left: walk.margin.left + side,
                            right: walk.margin.right + side,
                            ..walk.margin
                        },
                        ..walk
                    }
                }
                None => walk,
            }
        };
        if let Some(mut flow) = flow_ref.borrow_mut() {
            flow.begin(cx, walk);
        }
        self.draw_siblings(cx, &document.roots, &source);
        if let Some(mut flow) = flow_ref.borrow_mut() {
            flow.end(cx);
            // `TextFlow::end` closes with `end_turtle_with_area`, so the
            // flow's area rect now spans exactly what this pass drew. Its
            // height is the document's natural drawn height at the walked
            // width -- the one honest measurement (this widget's own `view`
            // is never drawn on this path, so `self.area()` would be stale).
            self.content_height = flow.area().rect(cx).size.y;
        }
        self.draw_search_highlights(cx);
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

    /// See [`MarkdownViewer::content_height`]. Zero for an empty ref, which
    /// deliberately reads the same as "not yet drawn".
    pub fn content_height(&self) -> f64 {
        self.borrow().map_or(0.0, |inner| inner.content_height())
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

    pub fn set_search_highlights(&self, cx: &mut Cx, ranges: Vec<TextRange>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_search_highlights(cx, ranges);
        }
    }

    pub fn clear_search_highlights(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.clear_search_highlights(cx);
        }
    }

    /// Distance, in logical pixels, from the top of the drawn document to the
    /// first installed search highlight -- what a reveal scrolls the reading
    /// surface's own scroller to. Measured against the drawn content rather
    /// than the viewport, so it is independent of the current scroll offset.
    /// `None` before the document has drawn, or when no highlight is
    /// installed.
    pub fn search_highlight_offset(&self, cx: &Cx) -> Option<f64> {
        let inner = self.borrow()?;
        let top = inner
            .highlight_rects(cx)
            .into_iter()
            .map(|rect| rect.pos.y)
            .fold(None, |acc: Option<f64>, y| {
                Some(acc.map_or(y, |a| a.min(y)))
            })?;
        let content_top = inner.content_top(cx)?;
        Some(top - content_top)
    }

    /// The window-space rects `draw_search_highlights` painted on the last
    /// draw. Empty before the document has drawn.
    #[doc(hidden)]
    pub fn test_highlight_rects(&self, cx: &Cx) -> Vec<Rect> {
        self.borrow()
            .map_or_else(Vec::new, |inner| inner.highlight_rects(cx))
    }

    #[doc(hidden)]
    pub fn test_search_highlights(&self) -> Vec<TextRange> {
        self.borrow()
            .map_or_else(Vec::new, |inner| inner.search_highlights.to_vec())
    }
}
