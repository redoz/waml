use std::{collections::HashMap, fmt, sync::Arc};

use makepad_widgets::{dvec2, Rect};
use waml_syntax::{MarkdownSyntaxUpdate, SourceText, TextRange, TextSize};

use crate::{document::MarkdownDocumentSnapshot, selection::TextPosition};

use super::{
    Affinity, BlockGeometry, GeometryElementId, GlyphCluster, LayoutBlock, LayoutDocument,
    LayoutElementId, LayoutError, LayoutSnapshot, LayoutSnapshotMetadata, LayoutTextRun,
    TextMetrics, VisualLine,
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
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutViewport {
    pub width: f64,
    pub height: f64,
    pub scroll_y: f64,
    pub overscan: f64,
}

impl LayoutViewport {
    pub fn new(width: f64, height: f64, scroll_y: f64, overscan: f64) -> Self {
        Self {
            width,
            height,
            scroll_y,
            overscan,
        }
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

#[derive(Clone, Debug)]
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

#[derive(Default)]
pub struct LayoutEngine {
    summaries: HashMap<LayoutElementId, BlockSummary>,
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
        let mut summaries = Vec::with_capacity(document.blocks.len());
        let mut y = document.content_insets.top;
        for (index, block) in document.blocks.iter().enumerate() {
            y += block.spec.space_before + block.spec.insets.top;
            let cached = self.summaries.get(&block.id);
            let invalidated = block_is_invalidated(&invalidation, block, index);
            let height = if !invalidated && cached.is_some_and(|old| old.width_key == width_key) {
                cached.map_or(20.0, |old| old.height)
            } else {
                estimated_height(block, document)
            };
            summaries.push(BlockSummary {
                id: block.id,
                source_range: block.source_range,
                parent: block.parent,
                flow_fingerprint: flow_fingerprint(block),
                y,
                height,
                width_key,
                content_fingerprint: content_fingerprint(block),
            });
            y += height + block.spec.insets.bottom + block.spec.space_after;
        }

        let visible_min = (viewport.scroll_y - viewport.overscan).max(0.0);
        let visible_max = viewport.scroll_y + viewport.height + viewport.overscan;
        // A document-local summary index is enough for ordinary off-screen
        // blocks, but an earlier long run can wrap and move the viewport into
        // a block that its one-line estimate placed above it. Measure only
        // those potentially wrapping predecessors before selecting the
        // visible window, then derive every visible range from the corrected
        // summaries.
        for (index, block) in document.blocks.iter().enumerate() {
            if summaries[index].y >= visible_min || !block_may_wrap(block, document, viewport.width) {
                continue;
            }
            let (nested_left, nested_right) = nested_horizontal_insets(document, block);
            let content_x = document.content_insets.left + nested_left;
            let available_width = (viewport.width
                - document.content_insets.left
                - document.content_insets.right
                - nested_left
                - nested_right)
                .max(1.0);
            let output = layout_block(
                block,
                document,
                presentation,
                content_x,
                summaries[index].y,
                available_width,
                shaper,
            )
            .unwrap_or_else(|_| {
                fallback_block(
                    block,
                    document,
                    presentation,
                    content_x,
                    summaries[index].y,
                    available_width,
                )
            });
            summaries[index].height = output.height;
        }
        reflow_summary_positions(document, &mut summaries);
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
        for index in visible_indices.iter().copied() {
            let block = &document.blocks[index];
            let (nested_left, nested_right) = nested_horizontal_insets(document, block);
            let content_x = document.content_insets.left + nested_left;
            let available_width = (viewport.width
                - document.content_insets.left
                - document.content_insets.right
                - nested_left
                - nested_right)
                .max(1.0);
            let output = layout_block(
                block,
                document,
                presentation,
                content_x,
                summaries[index].y,
                available_width,
                shaper,
            );
            match output {
                Ok(output) => {
                    let delta = output.height - summaries[index].height;
                    summaries[index].height = output.height;
                    if delta != 0.0 {
                        for downstream in summaries.iter_mut().skip(index + 1) {
                            downstream.y += delta;
                        }
                    }
                    visual_lines.extend(output.lines);
                    clusters.extend(output.clusters);
                    blocks.push(BlockGeometry::new(
                        block.id,
                        block.source_range,
                        Rect {
                            pos: dvec2(content_x, summaries[index].y),
                            size: dvec2(available_width, output.height),
                        },
                    ));
                }
                Err(_) => {
                    let output = fallback_block(
                        block,
                        document,
                        presentation,
                        content_x,
                        summaries[index].y,
                        available_width,
                    );
                    let delta = output.height - summaries[index].height;
                    summaries[index].height = output.height;
                    if delta != 0.0 {
                        for downstream in summaries.iter_mut().skip(index + 1) {
                            downstream.y += delta;
                        }
                    }
                    visual_lines.extend(output.lines);
                    clusters.extend(output.clusters);
                    blocks.push(BlockGeometry::fallback(
                        block.id,
                        block.source_range,
                        Rect {
                            pos: dvec2(content_x, summaries[index].y),
                            size: dvec2(available_width, output.height),
                        },
                    ));
                }
            }
        }

        let content_y = reflow_summary_positions(document, &mut summaries);
        for summary in &summaries {
            self.summaries.insert(summary.id, summary.clone());
        }

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
            },
            visual_lines.into(),
            blocks.into(),
            clusters.into(),
            summaries.into(),
        ))
    }
}

struct BlockOutput {
    lines: Vec<VisualLine>,
    clusters: Vec<GlyphCluster>,
    height: f64,
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
        output.clusters.push(GlyphCluster::with_metrics(
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

fn block_may_wrap(block: &LayoutBlock, document: &LayoutDocument, viewport_width: f64) -> bool {
    let (left, right) = nested_horizontal_insets(document, block);
    let available_width = (viewport_width
        - document.content_insets.left
        - document.content_insets.right
        - left
        - right)
        .max(1.0);
    document
        .text_runs
        .iter()
        .filter(|run| run.id == block.id)
        .any(|run| {
            let byte_len = run.range.end().to_usize() - run.range.start().to_usize();
            byte_len as f64 * (run.metrics.font_size as f64 * 0.5) > available_width
        })
}

fn reflow_summary_positions(document: &LayoutDocument, summaries: &mut [BlockSummary]) -> f64 {
    let mut content_y = document.content_insets.top;
    for (block, summary) in document.blocks.iter().zip(summaries.iter_mut()) {
        content_y += block.spec.space_before + block.spec.insets.top;
        summary.y = content_y;
        content_y += summary.height + block.spec.insets.bottom + block.spec.space_after;
    }
    content_y + document.content_insets.bottom
}

fn block_is_invalidated(
    invalidation: &LayoutInvalidation,
    block: &LayoutBlock,
    _index: usize,
) -> bool {
    match invalidation {
        LayoutInvalidation::Document | LayoutInvalidation::ViewportWidth => true,
        LayoutInvalidation::BlockMeasurement(id) => *id == block.id,
        LayoutInvalidation::SyntaxUpdate(update) => update
            .affected_ranges
            .iter()
            .any(|range| ranges_intersect(*range, block.source_range)),
    }
}

fn ranges_intersect(left: TextRange, right: TextRange) -> bool {
    left.start() < right.end() && right.start() < left.end()
}

fn flow_fingerprint(block: &LayoutBlock) -> u64 {
    block.spec.space_before.to_bits()
        ^ block.spec.space_after.to_bits().rotate_left(7)
        ^ block.spec.insets.left.to_bits().rotate_left(13)
        ^ block.spec.insets.right.to_bits().rotate_left(19)
}

fn content_fingerprint(block: &LayoutBlock) -> u64 {
    ((block.source_range.start().to_usize() as u64) << 32)
        ^ block.source_range.end().to_usize() as u64
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
