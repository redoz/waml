use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    fmt,
    hash::{Hash, Hasher},
    rc::Rc,
    sync::Arc,
};

use makepad_widgets::{
    dvec2,
    text::{color::Color, font::Font},
    DVec2, Rect,
};
use waml_syntax::{MarkdownSyntaxUpdate, SourceText, TextRange, TextSize};

use crate::{document::MarkdownDocumentSnapshot, selection::TextPosition};

use super::{
    Affinity, BlockGeometry, BlockLayoutData, GeometryElementId, GlyphCluster, LayoutBlock,
    LayoutDocument, LayoutElementId, LayoutError, LayoutSnapshot, LayoutSnapshotMetadata,
    LayoutTextRun, TextMetrics, VisualLine,
};
use crate::layout::geometry::CaretStop;

pub trait TextShaper {
    fn shape(
        &mut self,
        source: &SourceText,
        run: &LayoutTextRun,
        max_width: f64,
    ) -> Result<ShapedRun, LayoutError>;
}

#[derive(Clone, Debug)]
pub struct ShapedRun {
    pub clusters: Arc<[ShapedCluster]>,
    pub ascender: f64,
    pub descender: f64,
    pub line_gap: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShapedCluster {
    pub source_range: TextRange,
    pub advance: f64,
    pub bidi_level: u8,
    pub caret_offsets: Arc<[TextSize]>,
    pub glyphs: Arc<[ShapedGlyph]>,
}

/// Immutable renderer input retained from the shaping authority.
#[derive(Clone, Debug, PartialEq)]
pub struct ShapedGlyph {
    pub glyph_id: u16,
    /// Before layout this is relative to the cluster baseline. In a
    /// `GlyphCluster` it is the exact document-space glyph origin.
    pub origin: DVec2,
    pub advance: f64,
    pub font: Option<Rc<Font>>,
    pub font_key: super::FontKey,
    pub font_size: f32,
    pub ascender: f64,
    pub descender: f64,
    pub line_gap: f64,
    pub baseline: f64,
    pub offset: f64,
    pub color: Option<Color>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutViewport {
    pub width: f64,
    pub height: f64,
    pub scroll_y: f64,
    pub overscan: f64,
}

impl LayoutViewport {
    pub const DEFAULT_OVERSCAN: f64 = 320.0;

    pub fn new(width: f64, height: f64, scroll_y: f64, overscan: f64) -> Self {
        Self {
            width,
            height,
            scroll_y,
            overscan,
        }
    }

    pub fn default_overscan(width: f64, height: f64, scroll_y: f64) -> Self {
        Self::new(width, height, scroll_y, Self::DEFAULT_OVERSCAN)
    }
}

#[derive(Clone)]
pub enum LayoutInvalidation {
    Document,
    SyntaxUpdate(MarkdownSyntaxUpdate),
    ViewportWidth,
    BlockMeasurement(LayoutElementId),
}

impl fmt::Debug for LayoutInvalidation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Document => formatter.write_str("Document"),
            Self::SyntaxUpdate(_) => formatter.write_str("SyntaxUpdate(..)"),
            Self::ViewportWidth => formatter.write_str("ViewportWidth"),
            Self::BlockMeasurement(id) => {
                formatter.debug_tuple("BlockMeasurement").field(id).finish()
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockSummary {
    pub id: LayoutElementId,
    pub source_range: TextRange,
    pub parent: Option<LayoutElementId>,
    pub flow_fingerprint: u64,
    pub y: f64,
    pub height: f64,
    pub width_key: u64,
    pub content_fingerprint: u64,
}

#[derive(Clone)]
struct CachedBlock {
    summary: BlockSummary,
    data: Arc<BlockLayoutData>,
}

#[derive(Default)]
pub struct LayoutEngine {
    blocks: HashMap<LayoutElementId, CachedBlock>,
}

impl LayoutEngine {
    pub fn layout<S: TextShaper>(
        &mut self,
        document: &LayoutDocument,
        presentation: &MarkdownDocumentSnapshot,
        viewport: LayoutViewport,
        invalidation: LayoutInvalidation,
        shaper: &mut S,
    ) -> Result<LayoutSnapshot, LayoutError> {
        if document.revision != presentation.revision() {
            return Err(LayoutError::RevisionMismatch {
                document: presentation.revision(),
                layout: document.revision,
            });
        }
        if let LayoutInvalidation::SyntaxUpdate(update) = &invalidation {
            if update.snapshot.revision() != presentation.revision() {
                return Err(LayoutError::RevisionMismatch {
                    document: presentation.revision(),
                    layout: update.snapshot.revision(),
                });
            }
        }

        let width_key = viewport.width.to_bits();
        let invalidated = invalidated_block_range(&invalidation, document);
        let mut summaries = Vec::with_capacity(document.blocks.len());
        let mut block_data = Vec::with_capacity(document.blocks.len());
        let mut dirty_first = None;
        let mut dirty_end = 0;
        let mut y = document.content_insets.top;

        for (index, block) in document.blocks.iter().enumerate() {
            y += block.spec.space_before + block.spec.insets.top;
            let (nested_left, nested_right) = nested_horizontal_insets(document, block);
            let available_width = (viewport.width
                - document.content_insets.left
                - document.content_insets.right
                - nested_left
                - nested_right)
                .max(1.0);
            let flow_fingerprint = flow_fingerprint(block);
            let content_fingerprint = content_fingerprint(block, document, presentation);
            let cached = self.blocks.get(&block.id);
            let explicitly_invalidated = invalidated.contains(&index);
            let can_reuse = !explicitly_invalidated
                && cached.is_some_and(|old| {
                    old.summary.id == block.id
                        && old.summary.source_range == block.source_range
                        && old.summary.parent == block.parent
                        && old.summary.flow_fingerprint == flow_fingerprint
                        && old.summary.width_key == width_key
                        && old.summary.content_fingerprint == content_fingerprint
                });
            let data = if can_reuse {
                cached.expect("a reusable block has cached data").data.clone()
            } else {
                let (output, fallback) = match layout_block(
                    block,
                    document,
                    presentation,
                    0.0,
                    0.0,
                    available_width,
                    shaper,
                ) {
                    Ok(output) => (output, false),
                    Err(_) => (
                        fallback_block(
                            block,
                            document,
                            presentation,
                            0.0,
                            0.0,
                            available_width,
                        ),
                        true,
                    ),
                };
                Arc::new(block_layout_data(block, available_width, output, fallback))
            };
            let summary = BlockSummary {
                id: block.id,
                source_range: block.source_range,
                parent: block.parent,
                flow_fingerprint,
                y,
                height: data.block.rect.size.y,
                width_key,
                content_fingerprint,
            };
            let changed = explicitly_invalidated
                || cached.is_none_or(|old| old.summary != summary);
            if changed {
                dirty_first.get_or_insert(index);
                dirty_end = index + 1;
            }
            y += summary.height + block.spec.insets.bottom + block.spec.space_after;
            summaries.push(summary);
            block_data.push(data);
        }

        let dirty_block_range = dirty_first.map_or(0..0, |first| first..dirty_end);
        let content_y = y + document.content_insets.bottom;
        let visible_min = (viewport.scroll_y - viewport.overscan).max(0.0);
        let visible_max = viewport.scroll_y + viewport.height + viewport.overscan;
        let visible_indices: Vec<_> = summaries
            .iter()
            .enumerate()
            .filter_map(|(index, summary)| {
                (summary.y + summary.height >= visible_min && summary.y <= visible_max)
                    .then_some(index)
            })
            .collect();

        let mut visual_lines = Vec::new();
        let mut clusters = Vec::new();
        let mut blocks = Vec::new();
        let mut visible_block_layouts = Vec::new();
        for index in visible_indices.iter().copied() {
            let block = &document.blocks[index];
            let (nested_left, _) = nested_horizontal_insets(document, block);
            let content_x = document.content_insets.left + nested_left;
            append_positioned_block(
                &block_data[index],
                content_x,
                summaries[index].y,
                &mut visual_lines,
                &mut clusters,
                &mut blocks,
            );
            visible_block_layouts.push(block_data[index].clone());
        }

        self.blocks = document
            .blocks
            .iter()
            .zip(summaries.iter().cloned())
            .zip(block_data.iter().cloned())
            .map(|((block, summary), data)| (block.id, CachedBlock { summary, data }))
            .collect();

        let visible_source_range = visual_lines
            .first()
            .zip(visual_lines.last())
            .and_then(|(first, last)| {
                TextRange::new(first.source_range.start(), last.source_range.end()).ok()
            })
            .unwrap_or_else(empty_range);
        let visible_block_range = visible_indices
            .first()
            .zip(visible_indices.last())
            .map_or(0..0, |(first, last)| *first..last + 1);

        Ok(LayoutSnapshot::new(
            LayoutSnapshotMetadata {
                revision: document.revision,
                viewport_width: viewport.width,
                content_size: dvec2(viewport.width, content_y),
                visible_source_range,
                visible_block_range,
                dirty_block_range,
            },
            visual_lines.into(),
            blocks.into(),
            clusters.into(),
            summaries.into(),
            visible_block_layouts.into(),
        ))
    }
}

struct BlockOutput {
    lines: Vec<VisualLine>,
    clusters: Vec<GlyphCluster>,
    height: f64,
}

fn block_layout_data(
    block: &LayoutBlock,
    width: f64,
    output: BlockOutput,
    fallback: bool,
) -> BlockLayoutData {
    let rect = Rect {
        pos: dvec2(0.0, 0.0),
        size: dvec2(width, output.height),
    };
    let geometry = if fallback {
        BlockGeometry::fallback(block.id, block.source_range, rect)
    } else {
        BlockGeometry::new(block.id, block.source_range, rect)
    };
    BlockLayoutData {
        block: geometry,
        visual_lines: output.lines.into(),
        glyph_clusters: output.clusters.into(),
    }
}

fn append_positioned_block(
    data: &BlockLayoutData,
    x: f64,
    y: f64,
    lines: &mut Vec<VisualLine>,
    clusters: &mut Vec<GlyphCluster>,
    blocks: &mut Vec<BlockGeometry>,
) {
    let rect = Rect {
        pos: dvec2(x, y),
        size: data.block.rect.size,
    };
    blocks.push(if data.block.is_plain_text_fallback() {
        BlockGeometry::fallback(data.block.id, data.block.source_range, rect)
    } else {
        BlockGeometry::new(data.block.id, data.block.source_range, rect)
    });
    lines.extend(data.visual_lines.iter().map(|line| {
        let mut line = *line;
        line.rect.pos.x += x;
        line.rect.pos.y += y;
        line
    }));
    clusters.extend(data.glyph_clusters.iter().cloned().map(|mut cluster| {
        cluster.rect.pos.x += x;
        cluster.rect.pos.y += y;
        let mut stops = cluster.caret_stops.to_vec();
        for stop in &mut stops {
            stop.point.x += x;
            stop.point.y += y;
        }
        cluster.caret_stops = stops.into();
        let mut glyphs = cluster.glyphs.to_vec();
        for glyph in &mut glyphs {
            glyph.origin.x += x;
            glyph.origin.y += y;
            glyph.baseline += y;
        }
        cluster.glyphs = glyphs.into();
        cluster
    }));
}

fn layout_block<S: TextShaper>(
    block: &LayoutBlock,
    document: &LayoutDocument,
    presentation: &MarkdownDocumentSnapshot,
    x: f64,
    y: f64,
    max_width: f64,
    shaper: &mut S,
) -> Result<BlockOutput, LayoutError> {
    let runs: Vec<_> = document
        .text_runs
        .iter()
        .filter(|run| run.id == block.id)
        .collect();
    let mut output = BlockOutput {
        lines: Vec::new(),
        clusters: Vec::new(),
        height: 0.0,
    };
    for run in runs {
        let shaped = shaper.shape(presentation.text(), run, max_width)?;
        let run_y = y + output.height;
        append_run(
            &mut output,
            run.id,
            &run.metrics,
            &shaped,
            x,
            run_y,
            max_width,
        );
    }
    if output.lines.is_empty() {
        output.height = estimated_height(block, document);
    }
    Ok(output)
}

fn fallback_block(
    block: &LayoutBlock,
    document: &LayoutDocument,
    presentation: &MarkdownDocumentSnapshot,
    x: f64,
    y: f64,
    max_width: f64,
) -> BlockOutput {
    let text = presentation.text().slice(block.source_range).unwrap_or("");
    let mut shaped = Vec::new();
    for (relative, character) in text.char_indices() {
        let start = block.source_range.start().to_usize() + relative;
        let end = start + character.len_utf8();
        shaped.push(ShapedCluster {
            source_range: text_range(start, end),
            advance: 8.0,
            bidi_level: 0,
            caret_offsets: Arc::from([text_size(start), text_size(end)]),
            glyphs: Arc::from([ShapedGlyph {
                glyph_id: u16::try_from(character as u32).unwrap_or(0),
                origin: dvec2(0.0, 0.0),
                advance: 8.0,
                font: None,
                font_key: document_metrics(block, document).font,
                font_size: 16.0,
                ascender: 12.8,
                descender: -3.2,
                line_gap: 0.0,
                baseline: 12.8,
                offset: 0.0,
                color: None,
            }]),
        });
    }
    let run = ShapedRun {
        clusters: shaped.into(),
        ascender: 12.8,
        descender: 3.2,
        line_gap: 0.0,
    };
    let mut output = BlockOutput {
        lines: Vec::new(),
        clusters: Vec::new(),
        height: 0.0,
    };
    let metrics = document_metrics(block, document);
    append_run(&mut output, block.id, &metrics, &run, x, y, max_width);
    output
}

fn append_run(
    output: &mut BlockOutput,
    layout_id: LayoutElementId,
    metrics: &TextMetrics,
    run: &ShapedRun,
    start_x: f64,
    start_y: f64,
    max_width: f64,
) {
    let line_height = (run.ascender + run.descender.abs() + run.line_gap).max(1.0);
    let mut line_clusters: Vec<(usize, &ShapedCluster, f64)> = Vec::new();
    let mut x = 0.0;
    let mut y = start_y;
    let mut source_order: Vec<_> = run
        .clusters
        .iter()
        .map(|cluster| cluster.source_range)
        .collect();
    source_order.sort_by_key(|range| range.start());
    source_order.dedup();
    for shaped in run.clusters.iter() {
        let ordinal = source_order
            .binary_search_by_key(&shaped.source_range.start(), |range| range.start())
            .expect("a shaped cluster appears in source order");
        if x > 0.0 && x + shaped.advance > max_width {
            flush_line(
                output,
                layout_id,
                metrics,
                &line_clusters,
                start_x,
                y,
                line_height,
            );
            line_clusters.clear();
            x = 0.0;
            y += line_height;
        }
        line_clusters.push((ordinal, shaped, x));
        x += shaped.advance;
    }
    if !line_clusters.is_empty() {
        flush_line(
            output,
            layout_id,
            metrics,
            &line_clusters,
            start_x,
            y,
            line_height,
        );
    }
    output.height = output.lines.iter().map(VisualLine::height).sum();
}

fn flush_line(
    output: &mut BlockOutput,
    layout_id: LayoutElementId,
    metrics: &TextMetrics,
    line_clusters: &[(usize, &ShapedCluster, f64)],
    start_x: f64,
    y: f64,
    height: f64,
) {
    let Some((_, _, _)) = line_clusters.first() else {
        return;
    };
    let source_start = line_clusters
        .iter()
        .map(|(_, cluster, _)| cluster.source_range.start())
        .min()
        .expect("a line has a first source boundary");
    let source_end = line_clusters
        .iter()
        .map(|(_, cluster, _)| cluster.source_range.end())
        .max()
        .expect("a line has a last source boundary");
    let width = line_clusters
        .iter()
        .map(|(_, cluster, _)| cluster.advance)
        .sum();
    output.lines.push(VisualLine::new(
        TextRange::new(source_start, source_end).expect("shaped clusters remain source ordered"),
        Rect {
            pos: dvec2(start_x, y),
            size: dvec2(width, height),
        },
    ));
    let mut visual_clusters = line_clusters.to_vec();
    reorder_by_bidi_level(&mut visual_clusters);
    let mut x = start_x;
    for (ordinal, shaped, _) in visual_clusters {
        let stops = shaped
            .caret_offsets
            .iter()
            .enumerate()
            .map(|(index, offset)| {
                let mut fraction = if shaped.caret_offsets.len() <= 1 {
                    0.0
                } else {
                    index as f64 / (shaped.caret_offsets.len() - 1) as f64
                };
                if shaped.bidi_level % 2 == 1 {
                    fraction = 1.0 - fraction;
                }
                CaretStop::new(
                    TextPosition::new(
                        *offset,
                        if index == 0 {
                            Affinity::Before
                        } else {
                            Affinity::After
                        },
                    ),
                    dvec2(x + shaped.advance * fraction, y),
                )
            })
            .collect::<Vec<_>>();
        let glyphs = shaped
            .glyphs
            .iter()
            .cloned()
            .map(|mut glyph| {
                glyph.origin = dvec2(
                    x + glyph.origin.x,
                    y + glyph.baseline + glyph.origin.y,
                );
                glyph.baseline += y;
                glyph
            })
            .collect::<Vec<_>>();
        output.clusters.push(GlyphCluster::with_glyphs(
            GeometryElementId {
                layout: layout_id,
                cluster_ordinal: ordinal as u32,
            },
            shaped.source_range,
            Rect {
                pos: dvec2(x, y),
                size: dvec2(shaped.advance, height),
            },
            stops.into(),
            *metrics,
            glyphs.into(),
        ));
        x += shaped.advance;
    }
}

fn reorder_by_bidi_level(clusters: &mut [(usize, &ShapedCluster, f64)]) {
    let Some(max_level) = clusters
        .iter()
        .map(|(_, cluster, _)| cluster.bidi_level)
        .max()
    else {
        return;
    };
    let Some(min_odd_level) = clusters
        .iter()
        .map(|(_, cluster, _)| cluster.bidi_level)
        .filter(|level| level % 2 == 1)
        .min()
    else {
        return;
    };
    for level in (min_odd_level..=max_level).rev() {
        let mut start = 0;
        while start < clusters.len() {
            while start < clusters.len() && clusters[start].1.bidi_level < level {
                start += 1;
            }
            let mut end = start;
            while end < clusters.len() && clusters[end].1.bidi_level >= level {
                end += 1;
            }
            clusters[start..end].reverse();
            start = end;
        }
    }
}

fn nested_horizontal_insets(document: &LayoutDocument, block: &LayoutBlock) -> (f64, f64) {
    let mut left = block.spec.insets.left;
    let mut right = block.spec.insets.right;
    let mut parent = block.parent;
    while let Some(parent_id) = parent {
        let Some(parent_block) = document
            .blocks
            .iter()
            .find(|candidate| candidate.id == parent_id)
        else {
            break;
        };
        left += parent_block.spec.insets.left;
        right += parent_block.spec.insets.right;
        parent = parent_block.parent;
    }
    (left, right)
}

fn estimated_height(block: &LayoutBlock, document: &LayoutDocument) -> f64 {
    let text_height = document
        .text_runs
        .iter()
        .filter(|run| run.id == block.id)
        .map(|run| run.metrics.font_size as f64)
        .fold(20.0, f64::max);
    document
        .embedded_blocks
        .iter()
        .filter(|embedded| embedded.id == block.id)
        .map(|embedded| embedded.size.y)
        .fold(text_height, f64::max)
}

fn document_metrics(block: &LayoutBlock, document: &LayoutDocument) -> TextMetrics {
    document
        .text_runs
        .iter()
        .find(|run| run.id == block.id)
        .map(|run| run.metrics)
        .unwrap_or(TextMetrics {
            font: super::FontKey(0),
            font_size: 16.0,
            line_spacing: 1.0,
            weight: super::FontWeight(400),
            italic: false,
        })
}

fn invalidated_block_range(
    invalidation: &LayoutInvalidation,
    document: &LayoutDocument,
) -> std::ops::Range<usize> {
    match invalidation {
        LayoutInvalidation::Document | LayoutInvalidation::ViewportWidth => 0..document.blocks.len(),
        LayoutInvalidation::BlockMeasurement(id) => document
            .blocks
            .iter()
            .position(|block| block.id == *id)
            .map_or(0..0, |index| index..index + 1),
        LayoutInvalidation::SyntaxUpdate(update) => {
            let mut affected = document
                .blocks
                .iter()
                .enumerate()
                .filter_map(|(index, block)| {
                    update
                        .affected_ranges
                        .iter()
                        .any(|range| ranges_intersect(*range, block.source_range))
                        .then_some(index)
                });
            affected.next().map_or(0..0, |first| {
                let last = affected.last().unwrap_or(first);
                first..last + 1
            })
        }
    }
}

fn ranges_intersect(left: TextRange, right: TextRange) -> bool {
    left.start() < right.end() && right.start() < left.end()
}

fn flow_fingerprint(block: &LayoutBlock) -> u64 {
    let mut hasher = DefaultHasher::new();
    match &block.spec.flow {
        super::BlockFlow::Paragraph => 0_u8.hash(&mut hasher),
        super::BlockFlow::Hanging {
            marker_range,
            content_indent,
        } => {
            1_u8.hash(&mut hasher);
            marker_range.hash(&mut hasher);
            content_indent.to_bits().hash(&mut hasher);
        }
        super::BlockFlow::Quote => 2_u8.hash(&mut hasher),
        super::BlockFlow::Code => 3_u8.hash(&mut hasher),
        super::BlockFlow::Table => 4_u8.hash(&mut hasher),
        super::BlockFlow::TableRow => 5_u8.hash(&mut hasher),
        super::BlockFlow::TableCell { column } => {
            6_u8.hash(&mut hasher);
            column.hash(&mut hasher);
        }
        super::BlockFlow::Embedded => 7_u8.hash(&mut hasher),
    }
    for value in [
        block.spec.insets.top,
        block.spec.insets.right,
        block.spec.insets.bottom,
        block.spec.insets.left,
        block.spec.space_before,
        block.spec.space_after,
    ] {
        value.to_bits().hash(&mut hasher);
    }
    for column in block.spec.columns.iter() {
        column.min_width.to_bits().hash(&mut hasher);
        column.max_width.map(f64::to_bits).hash(&mut hasher);
        match column.alignment {
            super::ColumnAlignment::Start => 0_u8,
            super::ColumnAlignment::Center => 1_u8,
            super::ColumnAlignment::End => 2_u8,
        }
        .hash(&mut hasher);
    }
    hasher.finish()
}

fn content_fingerprint(
    block: &LayoutBlock,
    document: &LayoutDocument,
    presentation: &MarkdownDocumentSnapshot,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    block.source_range.hash(&mut hasher);
    presentation
        .text()
        .slice(block.source_range)
        .unwrap_or("")
        .as_bytes()
        .hash(&mut hasher);
    for run in document.text_runs.iter().filter(|run| run.id == block.id) {
        run.range.hash(&mut hasher);
        run.metrics.font.hash(&mut hasher);
        run.metrics.font_size.to_bits().hash(&mut hasher);
        run.metrics.line_spacing.to_bits().hash(&mut hasher);
        run.metrics.weight.hash(&mut hasher);
        run.metrics.italic.hash(&mut hasher);
    }
    for embedded in document
        .embedded_blocks
        .iter()
        .filter(|embedded| embedded.id == block.id)
    {
        embedded.source_range.hash(&mut hasher);
        embedded.size.x.to_bits().hash(&mut hasher);
        embedded.size.y.to_bits().hash(&mut hasher);
        embedded.baseline.map(f64::to_bits).hash(&mut hasher);
    }
    hasher.finish()
}

fn empty_range() -> TextRange {
    TextRange::new(TextSize::new(0), TextSize::new(0)).expect("zero range is ordered")
}

fn text_size(value: usize) -> TextSize {
    TextSize::try_from_usize(value).expect("source offsets fit TextSize")
}

fn text_range(start: usize, end: usize) -> TextRange {
    TextRange::new(text_size(start), text_size(end)).expect("source range is ordered")
}
