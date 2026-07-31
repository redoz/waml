use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    fmt,
    hash::{Hash, Hasher},
    sync::Arc,
};

use makepad_widgets::{
    dvec2,
    text::{color::Color, font::FontId},
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

    fn min_content_width(
        &mut self,
        source: &SourceText,
        run: &LayoutTextRun,
    ) -> Result<f64, LayoutError> {
        let shaped = self.shape(source, run, 1_000_000.0)?;
        let mut width = 0.0_f64;
        let mut word_width = 0.0_f64;
        for cluster in shaped.clusters.iter() {
            let whitespace = source
                .slice(cluster.source_range)
                .map_or(true, |text| text.chars().all(char::is_whitespace));
            if whitespace {
                word_width = 0.0;
            } else {
                word_width += cluster.advance;
                width = width.max(word_width);
            }
        }
        Ok(width)
    }
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
    /// Row assigned by the shaping authority before block placement.
    pub row_ordinal: u32,
    /// Row top relative to the start of the shaped run, after paint scaling.
    pub row_top: f64,
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
    /// Scale used by Makepad when it converts laid-out logical pixels to
    /// final paint positions.
    pub paint_scale: f64,
    pub font: Option<FontId>,
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
    /// Only the viewport position or height changed.
    Viewport,
    BlockMeasurement(LayoutElementId),
}

impl fmt::Debug for LayoutInvalidation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Document => formatter.write_str("Document"),
            Self::SyntaxUpdate(_) => formatter.write_str("SyntaxUpdate(..)"),
            Self::ViewportWidth => formatter.write_str("ViewportWidth"),
            Self::Viewport => formatter.write_str("Viewport"),
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
    measured: bool,
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

        let invalidated = invalidated_block_range(&invalidation, document);
        let hierarchy = BlockHierarchy::new(document);
        let mut intrinsic_widths = vec![0.0; document.blocks.len()];
        let mut table_intrinsics_ready = vec![false; document.blocks.len()];
        let mut widths = WidthPlan::new(document, &hierarchy, viewport.width, &intrinsic_widths);
        let mut block_data = Vec::with_capacity(document.blocks.len());
        let mut measured = Vec::with_capacity(document.blocks.len());

        for (index, block) in document.blocks.iter().enumerate() {
            let available_width = widths.content[index];
            let width_key = available_width.to_bits();
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
            let force_measure = matches!(
                invalidation,
                LayoutInvalidation::BlockMeasurement(id) if id == block.id
            );
            let (data, is_measured) = if can_reuse {
                let cached = cached.expect("a reusable block has cached data");
                (cached.data.clone(), cached.measured)
            } else if force_measure {
                (
                    measure_block(block, document, presentation, available_width, shaper),
                    true,
                )
            } else {
                (
                    Arc::new(estimated_block_layout_data(
                        block,
                        document,
                        available_width,
                    )),
                    false,
                )
            };
            block_data.push(data);
            measured.push(is_measured);
        }

        let visible_min = (viewport.scroll_y - viewport.overscan).max(0.0);
        let visible_max = viewport.scroll_y + viewport.height + viewport.overscan;
        let measurement_overscan = viewport.overscan.max(LayoutViewport::DEFAULT_OVERSCAN);
        let measurement_min = (viewport.scroll_y - measurement_overscan).max(0.0);
        let measurement_max = viewport.scroll_y + viewport.height + measurement_overscan;
        let (placements, content_y) = loop {
            let (placements, content_y) =
                position_block_tree(document, &hierarchy, &widths, &block_data);
            let measurement_indices =
                visible_indices(&placements, measurement_min, measurement_max);
            let pending_tables = measurement_indices
                .iter()
                .copied()
                .filter(|index| {
                    matches!(document.blocks[*index].spec.flow, super::BlockFlow::Table)
                        && !table_intrinsics_ready[*index]
                })
                .collect::<Vec<_>>();
            if !pending_tables.is_empty() {
                for table in pending_tables {
                    measure_table_min_content(
                        document,
                        &hierarchy,
                        table,
                        presentation.text(),
                        shaper,
                        &mut intrinsic_widths,
                    )?;
                    table_intrinsics_ready[table] = true;
                }
                let next_widths =
                    WidthPlan::new(document, &hierarchy, viewport.width, &intrinsic_widths);
                for index in 0..document.blocks.len() {
                    if widths.content[index].to_bits() != next_widths.content[index].to_bits() {
                        block_data[index] = Arc::new(estimated_block_layout_data(
                            &document.blocks[index],
                            document,
                            next_widths.content[index],
                        ));
                        measured[index] = false;
                    }
                }
                widths = next_widths;
                continue;
            }
            let pending = measurement_indices
                .iter()
                .copied()
                .filter(|index| !measured[*index])
                .collect::<Vec<_>>();
            if pending.is_empty() {
                break (placements, content_y);
            }
            for index in pending {
                block_data[index] = measure_block(
                    &document.blocks[index],
                    document,
                    presentation,
                    widths.content[index],
                    shaper,
                );
                measured[index] = true;
            }
        };
        let visible_indices = visible_indices(&placements, visible_min, visible_max);
        let mut summaries = Vec::with_capacity(document.blocks.len());
        let mut dirty_first = None;
        let mut dirty_end = 0;
        for (index, block) in document.blocks.iter().enumerate() {
            let summary = BlockSummary {
                id: block.id,
                source_range: block.source_range,
                parent: block.parent,
                flow_fingerprint: flow_fingerprint(block),
                y: placements[index].rect.pos.y,
                height: placements[index].rect.size.y,
                width_key: widths.content[index].to_bits(),
                content_fingerprint: content_fingerprint(block, document, presentation),
            };
            let explicitly_invalidated = invalidated.contains(&index);
            let changed = explicitly_invalidated
                || self
                    .blocks
                    .get(&block.id)
                    .map_or(true, |old| old.summary != summary);
            if changed {
                dirty_first.get_or_insert(index);
                dirty_end = index + 1;
            }
            summaries.push(summary);
        }
        let dirty_block_range = dirty_first.map_or(0..0, |first| first..dirty_end);
        let mut visual_lines = Vec::new();
        let mut clusters = Vec::new();
        let mut blocks = Vec::new();
        let mut visible_block_layouts = Vec::new();
        for index in visible_indices.iter().copied() {
            append_positioned_block(
                index,
                &block_data[index],
                placements[index],
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
            .zip(measured)
            .map(|(((block, summary), data), measured)| {
                (
                    block.id,
                    CachedBlock {
                        summary,
                        data,
                        measured,
                    },
                )
            })
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

struct BlockHierarchy {
    roots: Vec<usize>,
    children: Vec<Vec<usize>>,
}

impl BlockHierarchy {
    fn new(document: &LayoutDocument) -> Self {
        let indexes: HashMap<_, _> = document
            .blocks
            .iter()
            .enumerate()
            .map(|(index, block)| (block.id, index))
            .collect();
        let mut roots = Vec::new();
        let mut children = vec![Vec::new(); document.blocks.len()];
        for (index, block) in document.blocks.iter().enumerate() {
            if let Some(parent) = block.parent.and_then(|id| indexes.get(&id).copied()) {
                children[parent].push(index);
            } else {
                roots.push(index);
            }
        }
        Self { roots, children }
    }
}

struct WidthPlan {
    outer: Vec<f64>,
    content: Vec<f64>,
    child_x: Vec<f64>,
    alignment: Vec<super::ColumnAlignment>,
}

impl WidthPlan {
    fn new(
        document: &LayoutDocument,
        hierarchy: &BlockHierarchy,
        viewport_width: f64,
        intrinsic_widths: &[f64],
    ) -> Self {
        let mut plan = Self {
            outer: vec![1.0; document.blocks.len()],
            content: vec![1.0; document.blocks.len()],
            child_x: vec![0.0; document.blocks.len()],
            alignment: vec![super::ColumnAlignment::Start; document.blocks.len()],
        };
        let root_width =
            (viewport_width - document.content_insets.left - document.content_insets.right)
                .max(1.0);
        for &root in &hierarchy.roots {
            assign_block_widths(
                document,
                hierarchy,
                root,
                root_width,
                intrinsic_widths,
                &mut plan,
            );
        }
        plan
    }
}

fn assign_block_widths(
    document: &LayoutDocument,
    hierarchy: &BlockHierarchy,
    index: usize,
    available_width: f64,
    intrinsic_widths: &[f64],
    plan: &mut WidthPlan,
) {
    let block = &document.blocks[index];
    let horizontal_insets = block.spec.insets.left + block.spec.insets.right;
    match block.spec.flow {
        super::BlockFlow::Table => {
            let columns = solve_table_columns(
                document,
                hierarchy,
                index,
                available_width,
                intrinsic_widths,
            );
            let content_width = columns.iter().sum::<f64>().max(1.0);
            plan.content[index] = content_width;
            plan.outer[index] = content_width + horizontal_insets;
            for &child in &hierarchy.children[index] {
                if matches!(document.blocks[child].spec.flow, super::BlockFlow::TableRow) {
                    assign_table_row_widths(
                        document,
                        hierarchy,
                        child,
                        &columns,
                        &document.blocks[index].spec.columns,
                        intrinsic_widths,
                        plan,
                    );
                } else {
                    assign_block_widths(
                        document,
                        hierarchy,
                        child,
                        content_width,
                        intrinsic_widths,
                        plan,
                    );
                }
            }
        }
        _ => {
            plan.outer[index] = available_width.max(1.0);
            plan.content[index] = (available_width - horizontal_insets).max(1.0);
            for &child in &hierarchy.children[index] {
                assign_block_widths(
                    document,
                    hierarchy,
                    child,
                    plan.content[index],
                    intrinsic_widths,
                    plan,
                );
            }
        }
    }
}

fn assign_table_row_widths(
    document: &LayoutDocument,
    hierarchy: &BlockHierarchy,
    index: usize,
    columns: &[f64],
    constraints: &[super::ColumnConstraint],
    intrinsic_widths: &[f64],
    plan: &mut WidthPlan,
) {
    let row_width = columns.iter().sum::<f64>().max(1.0);
    plan.outer[index] = row_width;
    plan.content[index] = row_width;
    for &child in &hierarchy.children[index] {
        let column = match document.blocks[child].spec.flow {
            super::BlockFlow::TableCell { column } => column as usize,
            _ => 0,
        };
        plan.child_x[child] = columns.iter().take(column).sum();
        plan.alignment[child] = constraints
            .get(column)
            .map_or(super::ColumnAlignment::Start, |constraint| {
                constraint.alignment
            });
        assign_block_widths(
            document,
            hierarchy,
            child,
            columns.get(column).copied().unwrap_or(row_width),
            intrinsic_widths,
            plan,
        );
    }
}

fn solve_table_columns(
    document: &LayoutDocument,
    hierarchy: &BlockHierarchy,
    table: usize,
    available_width: f64,
    intrinsic_widths: &[f64],
) -> Vec<f64> {
    let constraints = &document.blocks[table].spec.columns;
    if constraints.is_empty() {
        let count = hierarchy.children[table]
            .iter()
            .flat_map(|row| hierarchy.children[*row].iter())
            .filter_map(|cell| match document.blocks[*cell].spec.flow {
                super::BlockFlow::TableCell { column } => Some(column as usize + 1),
                _ => None,
            })
            .max()
            .unwrap_or(1);
        return vec![(available_width / count as f64).max(1.0); count];
    }
    let mut intrinsic_columns = vec![0.0_f64; constraints.len()];
    for &row in &hierarchy.children[table] {
        for &cell in &hierarchy.children[row] {
            if let super::BlockFlow::TableCell { column } = document.blocks[cell].spec.flow {
                if let Some(width) = intrinsic_columns.get_mut(column as usize) {
                    *width = (*width).max(intrinsic_widths[cell]);
                }
            }
        }
    }
    let mut widths = constraints
        .iter()
        .zip(&intrinsic_columns)
        .map(|(constraint, intrinsic)| {
            constraint
                .max_width
                .map_or(constraint.min_width.max(*intrinsic), |max| {
                    constraint.min_width.max(*intrinsic).min(max)
                })
                .max(1.0)
        })
        .collect::<Vec<_>>();
    let mut remaining = (available_width - widths.iter().sum::<f64>()).max(0.0);
    while remaining > 0.0 {
        let growable = constraints
            .iter()
            .enumerate()
            .filter(|(index, constraint)| {
                constraint
                    .max_width
                    .map_or(true, |max| widths[*index] < max)
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if growable.is_empty() {
            break;
        }
        let total_weight = growable
            .iter()
            .map(|index| intrinsic_columns[*index].max(1.0))
            .sum::<f64>();
        let mut consumed = 0.0;
        for index in growable {
            let share = remaining * intrinsic_columns[index].max(1.0) / total_weight;
            let capacity = constraints[index]
                .max_width
                .map_or(share, |max| (max - widths[index]).max(0.0));
            let growth = share.min(capacity);
            widths[index] += growth;
            consumed += growth;
        }
        if consumed <= f64::EPSILON {
            break;
        }
        remaining -= consumed;
    }
    widths
}

#[derive(Clone, Copy)]
struct BlockPlacement {
    rect: Rect,
    content_origin: DVec2,
}

fn position_block_tree(
    document: &LayoutDocument,
    hierarchy: &BlockHierarchy,
    widths: &WidthPlan,
    data: &[Arc<BlockLayoutData>],
) -> (Vec<BlockPlacement>, f64) {
    let empty = BlockPlacement {
        rect: Rect {
            pos: dvec2(0.0, 0.0),
            size: dvec2(0.0, 0.0),
        },
        content_origin: dvec2(0.0, 0.0),
    };
    let mut placements = vec![empty; document.blocks.len()];
    let mut y = document.content_insets.top;
    for &root in &hierarchy.roots {
        let block = &document.blocks[root];
        y += block.spec.space_before;
        let height = position_block(
            document,
            hierarchy,
            widths,
            data,
            &mut placements,
            root,
            document.content_insets.left,
            y,
        );
        y += height + block.spec.space_after;
    }
    (placements, y + document.content_insets.bottom)
}

#[allow(clippy::too_many_arguments)]
fn position_block(
    document: &LayoutDocument,
    hierarchy: &BlockHierarchy,
    widths: &WidthPlan,
    data: &[Arc<BlockLayoutData>],
    placements: &mut [BlockPlacement],
    index: usize,
    x: f64,
    y: f64,
) -> f64 {
    let block = &document.blocks[index];
    let content_x = x + block.spec.insets.left;
    let content_y = y + block.spec.insets.top;
    let own_height = data[index].block.rect.size.y;
    let body_height = if matches!(block.spec.flow, super::BlockFlow::TableRow) {
        let mut row_height = own_height;
        for &child in &hierarchy.children[index] {
            let child_block = &document.blocks[child];
            let child_y = content_y + child_block.spec.space_before;
            let child_height = position_block(
                document,
                hierarchy,
                widths,
                data,
                placements,
                child,
                content_x + widths.child_x[child],
                child_y,
            );
            row_height = row_height
                .max(child_block.spec.space_before + child_height + child_block.spec.space_after);
        }
        row_height
    } else {
        let mut cursor = content_y + own_height;
        for &child in &hierarchy.children[index] {
            let child_block = &document.blocks[child];
            cursor += child_block.spec.space_before;
            let child_height = position_block(
                document,
                hierarchy,
                widths,
                data,
                placements,
                child,
                content_x + widths.child_x[child],
                cursor,
            );
            cursor += child_height + child_block.spec.space_after;
        }
        (cursor - content_y).max(own_height)
    };
    let height = block.spec.insets.top + body_height + block.spec.insets.bottom;
    let laid_width = data[index]
        .visual_lines
        .iter()
        .map(|line| line.rect.pos.x + line.rect.size.x)
        .fold(0.0, f64::max);
    let free_width = (widths.content[index] - laid_width).max(0.0);
    let alignment_offset = match widths.alignment[index] {
        super::ColumnAlignment::Start => 0.0,
        super::ColumnAlignment::Center => free_width * 0.5,
        super::ColumnAlignment::End => free_width,
    };
    placements[index] = BlockPlacement {
        rect: Rect {
            pos: dvec2(x, y),
            size: dvec2(widths.outer[index], height),
        },
        content_origin: dvec2(content_x + alignment_offset, content_y),
    };
    height
}

struct BlockOutput {
    lines: Vec<VisualLine>,
    clusters: Vec<GlyphCluster>,
    height: f64,
}

fn visible_indices(
    placements: &[BlockPlacement],
    visible_min: f64,
    visible_max: f64,
) -> Vec<usize> {
    placements
        .iter()
        .enumerate()
        .filter_map(|(index, placement)| {
            let rect = placement.rect;
            (rect.pos.y + rect.size.y >= visible_min && rect.pos.y <= visible_max).then_some(index)
        })
        .collect()
}

fn measure_table_min_content<S: TextShaper>(
    document: &LayoutDocument,
    hierarchy: &BlockHierarchy,
    table: usize,
    source: &SourceText,
    shaper: &mut S,
    intrinsic_widths: &mut [f64],
) -> Result<(), LayoutError> {
    for &row in &hierarchy.children[table] {
        for &cell in &hierarchy.children[row] {
            let mut width = 0.0_f64;
            for run in document
                .text_runs
                .iter()
                .filter(|run| run.id == document.blocks[cell].id)
            {
                width = width.max(shaper.min_content_width(source, run)?);
            }
            intrinsic_widths[cell] = width;
        }
    }
    Ok(())
}

fn estimated_block_layout_data(
    block: &LayoutBlock,
    document: &LayoutDocument,
    width: f64,
) -> BlockLayoutData {
    let text_height = document
        .text_runs
        .iter()
        .filter(|run| run.id == block.id)
        .map(|run| run.metrics.font_size as f64)
        .fold(0.0, f64::max);
    let height = document
        .embedded_blocks
        .iter()
        .filter(|embedded| embedded.id == block.id)
        .map(|embedded| embedded.size.y)
        .fold(text_height, f64::max);
    BlockLayoutData {
        block: BlockGeometry::new(
            block.id,
            block.source_range,
            Rect {
                pos: dvec2(0.0, 0.0),
                size: dvec2(width, height),
            },
        ),
        visual_lines: Arc::from([]),
        glyph_clusters: Arc::from([]),
    }
}

fn measure_block<S: TextShaper>(
    block: &LayoutBlock,
    document: &LayoutDocument,
    presentation: &MarkdownDocumentSnapshot,
    width: f64,
    shaper: &mut S,
) -> Arc<BlockLayoutData> {
    let (output, fallback) =
        match layout_block(block, document, presentation, 0.0, 0.0, width, shaper) {
            Ok(output) => (output, false),
            Err(_) => (
                fallback_block(block, document, presentation, 0.0, 0.0, width),
                true,
            ),
        };
    Arc::new(block_layout_data(block, width, output, fallback))
}

fn block_layout_data(
    block: &LayoutBlock,
    width: f64,
    mut output: BlockOutput,
    fallback: bool,
) -> BlockLayoutData {
    for (ordinal, cluster) in output.clusters.iter_mut().enumerate() {
        cluster.id.cluster_ordinal = ordinal as u32;
    }
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
    document_index: usize,
    data: &BlockLayoutData,
    placement: BlockPlacement,
    lines: &mut Vec<VisualLine>,
    clusters: &mut Vec<GlyphCluster>,
    blocks: &mut Vec<BlockGeometry>,
) {
    let rect = placement.rect;
    let x = placement.content_origin.x;
    let y = placement.content_origin.y;
    let mut block = if data.block.is_plain_text_fallback() {
        BlockGeometry::fallback(data.block.id, data.block.source_range, rect)
    } else {
        BlockGeometry::new(data.block.id, data.block.source_range, rect)
    };
    block.set_document_index(document_index);
    blocks.push(block);
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
    if let super::BlockFlow::Hanging {
        marker_range,
        content_indent,
    } = block.spec.flow
    {
        let mut marker_output = BlockOutput {
            lines: Vec::new(),
            clusters: Vec::new(),
            height: 0.0,
        };
        let mut content_output = BlockOutput {
            lines: Vec::new(),
            clusters: Vec::new(),
            height: 0.0,
        };
        let mut pieces = Vec::new();
        for run in runs {
            let marker_start = run.range.start().max(marker_range.start());
            let marker_end = run.range.end().min(marker_range.end());
            if run.range.start() < marker_start {
                pieces.push((
                    false,
                    LayoutTextRun {
                        id: run.id,
                        range: TextRange::new(run.range.start(), marker_start)
                            .expect("a hanging prefix stays ordered"),
                        metrics: run.metrics,
                    },
                ));
            }
            if marker_start < marker_end {
                pieces.push((
                    true,
                    LayoutTextRun {
                        id: run.id,
                        range: TextRange::new(marker_start, marker_end)
                            .expect("a hanging marker stays ordered"),
                        metrics: run.metrics,
                    },
                ));
            }
            let content_start = marker_end.max(run.range.start());
            if content_start < run.range.end() {
                pieces.push((
                    false,
                    LayoutTextRun {
                        id: run.id,
                        range: TextRange::new(content_start, run.range.end())
                            .expect("hanging content stays ordered"),
                        metrics: run.metrics,
                    },
                ));
            }
        }
        let mut shaped_pieces = Vec::with_capacity(pieces.len());
        for (is_marker, run) in pieces {
            let run_width = if is_marker {
                content_indent.max(1.0)
            } else {
                (max_width - content_indent).max(1.0)
            };
            let shaped = shaper.shape(presentation.text(), &run, run_width)?;
            shaped_pieces.push((is_marker, run, shaped, run_width));
        }
        let shared_ascender = shaped_pieces
            .iter()
            .map(|(_, _, shaped, _)| shaped.ascender)
            .fold(0.0, f64::max);
        for (is_marker, run, shaped, run_width) in shaped_pieces {
            let (target, run_x) = if is_marker {
                (&mut marker_output, x)
            } else {
                (&mut content_output, x + content_indent)
            };
            let run_y = y + shared_ascender - shaped.ascender;
            append_run(
                target,
                run.id,
                &run.metrics,
                &shaped,
                run_x,
                run_y,
                run_width,
            );
        }
        return Ok(BlockOutput {
            height: marker_output.height.max(content_output.height),
            lines: marker_output
                .lines
                .into_iter()
                .chain(content_output.lines)
                .collect(),
            clusters: marker_output
                .clusters
                .into_iter()
                .chain(content_output.clusters)
                .collect(),
        });
    }

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
        output.height = document
            .embedded_blocks
            .iter()
            .filter(|embedded| embedded.id == block.id)
            .map(|embedded| embedded.size.y)
            .fold(0.0, f64::max);
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
            row_ordinal: 0,
            row_top: 0.0,
            caret_offsets: Arc::from([text_size(start), text_size(end)]),
            glyphs: Arc::from([ShapedGlyph {
                glyph_id: u16::try_from(character as u32).unwrap_or(0),
                origin: dvec2(0.0, 0.0),
                advance: 8.0,
                paint_scale: 1.0,
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
    let mut row_ordinal = None;
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
        let starts_new_row = row_ordinal.is_some_and(|row| row != shaped.row_ordinal);
        if !line_clusters.is_empty() && (starts_new_row || x + shaped.advance > max_width) {
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
            if starts_new_row {
                y = start_y + shaped.row_top;
            } else {
                y += line_height;
            }
        }
        row_ordinal = Some(shaped.row_ordinal);
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
    output.height = output
        .lines
        .iter()
        .map(|line| line.rect.pos.y + line.height())
        .fold(0.0, f64::max);
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
                glyph.origin = dvec2(x + glyph.origin.x, y + glyph.baseline + glyph.origin.y);
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
        LayoutInvalidation::Document | LayoutInvalidation::ViewportWidth => {
            0..document.blocks.len()
        }
        LayoutInvalidation::Viewport => 0..0,
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
                let last = affected.next_back().unwrap_or(first);
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
