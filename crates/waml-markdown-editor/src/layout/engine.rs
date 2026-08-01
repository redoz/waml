use std::{
    cmp::Ordering,
    collections::{hash_map::DefaultHasher, HashMap},
    fmt,
    hash::{Hash, Hasher},
    ops::Range,
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
    Affinity, BlockGeometry, BlockLane, BlockLayoutData, GeometryElementId, GlyphCluster,
    LayoutBlock, LayoutDocument, LayoutElementId, LayoutError, LayoutGeometryParts, LayoutSnapshot,
    LayoutSnapshotMetadata, LayoutTextRun, TextMetrics, VisualLane, VisualLaneId, VisualLaneKind,
    VisualLine, VisualRow, VisualRowId,
};
use crate::layout::geometry::CaretStop;

pub trait TextShaper {
    fn shape_paragraph(
        &mut self,
        request: ParagraphShapeRequest<'_>,
    ) -> Result<ShapedParagraph, LayoutError>;

    fn measure_paragraph_intrinsic(
        &mut self,
        request: ParagraphIntrinsicRequest<'_>,
    ) -> Result<ParagraphIntrinsic, LayoutError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BaseDirection {
    Auto,
    LeftToRight,
    RightToLeft,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShapeSpan {
    pub id: GeometryElementId,
    pub run_id: LayoutElementId,
    pub stable_ordinal: u32,
    pub source_range: TextRange,
    pub metrics: TextMetrics,
}

#[derive(Clone, Copy, Debug)]
pub struct ParagraphShapeRequest<'a> {
    pub source: &'a SourceText,
    pub paragraph_id: GeometryElementId,
    pub paragraph_range: TextRange,
    pub spans: &'a [ShapeSpan],
    pub full_width: f64,
    pub first_row_width: f64,
    pub base_direction: BaseDirection,
}

#[derive(Clone, Copy, Debug)]
pub struct ParagraphIntrinsicRequest<'a> {
    pub source: &'a SourceText,
    pub paragraph_id: GeometryElementId,
    pub paragraph_range: TextRange,
    pub spans: &'a [ShapeSpan],
    pub base_direction: BaseDirection,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ParagraphIntrinsic {
    pub min_content: f64,
    pub max_content: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShapedParagraph {
    pub rows: Arc<[ShapedRow]>,
    pub fragments: Arc<[ShapedFragment]>,
    pub clusters: Arc<[ShapedCluster]>,
    pub bidi_levels: Arc<[u8]>,
    pub legal_breaks: Arc<[TextSize]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShapedRow {
    pub id: GeometryElementId,
    pub source_range: TextRange,
    pub cluster_range: Range<usize>,
    pub caret_offsets: Arc<[TextSize]>,
    pub ascender: f64,
    pub descender: f64,
    pub line_gap: f64,
    /// Makepad's `line_spacing_scale`: the multiplier it applies to the pitch
    /// between consecutive baselines. It does NOT affect the first baseline.
    pub line_spacing_scale: f64,
    pub row_top: f64,
}

impl ShapedRow {
    /// Distance from this row's baseline to the next row's baseline, matching
    /// Makepad's `LaidoutRow::line_spacing_in_lpxs`.
    pub fn line_advance(&self) -> f64 {
        (self.ascender + self.descender.abs() + self.line_gap) * self.line_spacing_scale
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShapedFragment {
    pub id: GeometryElementId,
    pub span_id: GeometryElementId,
    pub stable_ordinal: u32,
    pub source_range: TextRange,
    pub metrics: TextMetrics,
}

#[derive(Clone, Debug)]
pub struct ShapedRun {
    pub clusters: Arc<[ShapedCluster]>,
    pub ascender: f64,
    pub descender: f64,
    pub line_gap: f64,
}

#[derive(Clone, Debug)]
pub struct IntrinsicCluster {
    pub source_range: TextRange,
    pub advance: f64,
}

#[derive(Clone, Debug)]
pub struct IntrinsicRun {
    pub clusters: Arc<[IntrinsicCluster]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShapedCluster {
    pub id: GeometryElementId,
    pub span_id: GeometryElementId,
    pub source_range: TextRange,
    pub metrics: TextMetrics,
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
    data: Option<Arc<BlockLayoutData>>,
    measurement: BlockMeasurement,
    measured: bool,
}

#[derive(Clone, Copy)]
struct BlockMeasurement {
    height: f64,
}

impl BlockMeasurement {
    fn from_data(data: &BlockLayoutData) -> Self {
        Self {
            height: data.block.rect.size.y,
        }
    }
}

struct DocumentLayoutIndex {
    block_indices: HashMap<LayoutElementId, usize>,
    run_indices: Vec<Vec<usize>>,
    embedded_indices: Vec<Vec<usize>>,
    content_fingerprints: Vec<u64>,
    subtree_fingerprints: Vec<u64>,
    estimated_heights: Vec<f64>,
    build_stats: IndexBuildStats,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct IndexBuildStats {
    pub source_bytes: usize,
    pub run_visits: usize,
    pub embedded_visits: usize,
    pub block_visits: usize,
    pub hierarchy_node_visits: usize,
}

impl DocumentLayoutIndex {
    fn new(
        document: &LayoutDocument,
        presentation: &MarkdownDocumentSnapshot,
        hierarchy: &BlockHierarchy,
    ) -> Result<Self, LayoutError> {
        let mut block_indices = HashMap::with_capacity(document.blocks.len());
        let mut run_indices = vec![Vec::new(); document.blocks.len()];
        let mut embedded_indices = vec![Vec::new(); document.blocks.len()];
        let mut estimated_heights = vec![0.0_f64; document.blocks.len()];
        let mut block_hashers = (0..document.blocks.len())
            .map(|_| DefaultHasher::new())
            .collect::<Vec<_>>();
        let mut build_stats = IndexBuildStats {
            hierarchy_node_visits: hierarchy.node_visits,
            ..IndexBuildStats::default()
        };
        for (index, block) in document.blocks.iter().enumerate() {
            block_indices.insert(block.id, index);
            block.source_range.hash(&mut block_hashers[index]);
            build_stats.block_visits += 1;
        }

        let mut ordered_runs = document
            .text_runs
            .iter()
            .enumerate()
            .map(|(index, run)| (run.range.start(), run.range.end(), index))
            .collect::<Vec<_>>();
        ordered_runs.sort_unstable_by_key(|(start, end, index)| (*start, *end, *index));
        for pair in ordered_runs.windows(2) {
            let (_, first_end, first_index) = pair[0];
            let (second_start, _, second_index) = pair[1];
            if first_end > second_start {
                return Err(LayoutError::OverlappingTextRuns {
                    first: document.text_runs[first_index].range,
                    second: document.text_runs[second_index].range,
                });
            }
        }
        let mut run_content_hashers = (0..document.text_runs.len())
            .map(|_| DefaultHasher::new())
            .collect::<Vec<_>>();
        let mut active_run = 0_usize;
        for (offset, byte) in presentation.text().shared().as_bytes().iter().enumerate() {
            while ordered_runs
                .get(active_run)
                .is_some_and(|(_, end, _)| offset >= end.to_usize())
            {
                active_run += 1;
            }
            if let Some((start, end, run_index)) = ordered_runs.get(active_run).copied() {
                if start.to_usize() <= offset && offset < end.to_usize() {
                    byte.hash(&mut run_content_hashers[run_index]);
                }
            }
            build_stats.source_bytes += 1;
        }
        let run_content_fingerprints = run_content_hashers
            .into_iter()
            .map(|hasher| hasher.finish())
            .collect::<Vec<_>>();

        for (run_index, run) in document.text_runs.iter().enumerate() {
            build_stats.run_visits += 1;
            let Some(&block_index) = block_indices.get(&run.id) else {
                continue;
            };
            run_indices[block_index].push(run_index);
            estimated_heights[block_index] =
                estimated_heights[block_index].max(run.metrics.font_size as f64);
            run.range.hash(&mut block_hashers[block_index]);
            run_content_fingerprints[run_index].hash(&mut block_hashers[block_index]);
            run.metrics.font.hash(&mut block_hashers[block_index]);
            run.metrics
                .font_size
                .to_bits()
                .hash(&mut block_hashers[block_index]);
            run.metrics
                .line_spacing
                .to_bits()
                .hash(&mut block_hashers[block_index]);
            run.metrics.weight.hash(&mut block_hashers[block_index]);
            run.metrics.italic.hash(&mut block_hashers[block_index]);
        }
        for indexes in &mut run_indices {
            indexes.sort_unstable_by_key(|index| {
                let run = &document.text_runs[*index];
                (run.range.start(), run.range.end(), *index)
            });
        }
        for (embedded_index, embedded) in document.embedded_blocks.iter().enumerate() {
            build_stats.embedded_visits += 1;
            let Some(&block_index) = block_indices.get(&embedded.id) else {
                continue;
            };
            embedded_indices[block_index].push(embedded_index);
            estimated_heights[block_index] = estimated_heights[block_index].max(embedded.size.y);
            embedded.source_range.hash(&mut block_hashers[block_index]);
            embedded
                .size
                .x
                .to_bits()
                .hash(&mut block_hashers[block_index]);
            embedded
                .size
                .y
                .to_bits()
                .hash(&mut block_hashers[block_index]);
            embedded
                .baseline
                .map(f64::to_bits)
                .hash(&mut block_hashers[block_index]);
        }
        for indexes in &mut embedded_indices {
            indexes.sort_unstable_by_key(|index| {
                let embedded = &document.embedded_blocks[*index];
                (
                    embedded.source_range.start(),
                    embedded.source_range.end(),
                    *index,
                )
            });
        }
        let content_fingerprints = block_hashers
            .into_iter()
            .map(|hasher| hasher.finish())
            .collect::<Vec<_>>();
        let mut subtree_fingerprints = vec![0; document.blocks.len()];
        for &block_index in &hierarchy.postorder {
            let mut hasher = DefaultHasher::new();
            content_fingerprints[block_index].hash(&mut hasher);
            flow_fingerprint(&document.blocks[block_index]).hash(&mut hasher);
            for child in &hierarchy.children[block_index] {
                subtree_fingerprints[*child].hash(&mut hasher);
            }
            subtree_fingerprints[block_index] = hasher.finish();
        }
        Ok(Self {
            block_indices,
            run_indices,
            embedded_indices,
            content_fingerprints,
            subtree_fingerprints,
            estimated_heights,
            build_stats,
        })
    }
}

struct CachedTableIntrinsics {
    fingerprint: u64,
    cells: Vec<(LayoutElementId, f64)>,
}

struct TableIntrinsicState<'a> {
    widths: &'a mut [f64],
    cache: &'a mut HashMap<LayoutElementId, CachedTableIntrinsics>,
}

#[derive(Default)]
pub struct LayoutEngine {
    blocks: HashMap<LayoutElementId, CachedBlock>,
    table_intrinsics: HashMap<LayoutElementId, CachedTableIntrinsics>,
    last_index_build_stats: IndexBuildStats,
    last_lane_offset_stats: LaneOffsetStats,
    last_subtree_fingerprints: HashMap<LayoutElementId, u64>,
}

impl LayoutEngine {
    #[doc(hidden)]
    pub fn build_index_stats_for_test(
        document: &LayoutDocument,
        presentation: &MarkdownDocumentSnapshot,
    ) -> Result<IndexBuildStats, LayoutError> {
        let hierarchy = BlockHierarchy::try_new(document)?;
        Ok(DocumentLayoutIndex::new(document, presentation, &hierarchy)?.build_stats)
    }

    #[doc(hidden)]
    pub fn cached_summary_count_for_test(&self) -> usize {
        self.blocks.len()
    }

    #[doc(hidden)]
    pub fn retained_layout_payload_count_for_test(&self) -> usize {
        self.blocks
            .values()
            .filter(|block| block.data.is_some())
            .count()
    }

    #[doc(hidden)]
    pub fn last_index_build_stats_for_test(&self) -> IndexBuildStats {
        self.last_index_build_stats
    }

    #[doc(hidden)]
    pub fn last_lane_offset_stats_for_test(&self) -> LaneOffsetStats {
        self.last_lane_offset_stats
    }

    #[doc(hidden)]
    pub fn subtree_fingerprint_for_test(&self, id: LayoutElementId) -> Option<u64> {
        self.last_subtree_fingerprints.get(&id).copied()
    }

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
        let hierarchy = BlockHierarchy::try_new(document)?;
        let layout_index = DocumentLayoutIndex::new(document, presentation, &hierarchy)?;
        self.last_index_build_stats = layout_index.build_stats;
        self.last_subtree_fingerprints = document
            .blocks
            .iter()
            .zip(layout_index.subtree_fingerprints.iter().copied())
            .map(|(block, fingerprint)| (block.id, fingerprint))
            .collect();
        let mut intrinsic_widths = vec![0.0; document.blocks.len()];
        let mut table_intrinsics_ready = vec![false; document.blocks.len()];
        let mut widths = WidthPlan::new(document, &hierarchy, viewport.width, &intrinsic_widths);
        let mut block_data = Vec::with_capacity(document.blocks.len());
        let mut measurements = Vec::with_capacity(document.blocks.len());
        let mut measured = Vec::with_capacity(document.blocks.len());

        for (index, block) in document.blocks.iter().enumerate() {
            let available_width = widths.content[index];
            let width_key = available_width.to_bits();
            let flow_fingerprint = flow_fingerprint(block);
            let content_fingerprint = layout_index.content_fingerprints[index];
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
            let (data, measurement, is_measured) = if can_reuse {
                let cached = cached.expect("a reusable block has cached data");
                (cached.data.clone(), cached.measurement, cached.measured)
            } else if force_measure {
                let data = measure_block(
                    index,
                    document,
                    &layout_index,
                    presentation,
                    available_width,
                    shaper,
                );
                let measurement = BlockMeasurement::from_data(&data);
                (Some(data), measurement, true)
            } else {
                (
                    None,
                    estimated_block_measurement(&layout_index, index),
                    false,
                )
            };
            block_data.push(data);
            measurements.push(measurement);
            measured.push(is_measured);
        }

        let visible_min = (viewport.scroll_y - viewport.overscan).max(0.0);
        let visible_max = viewport.scroll_y + viewport.height + viewport.overscan;
        let measurement_overscan = viewport.overscan.max(LayoutViewport::DEFAULT_OVERSCAN);
        let measurement_min = (viewport.scroll_y - measurement_overscan).max(0.0);
        let measurement_max = viewport.scroll_y + viewport.height + measurement_overscan;
        let (placements, content_y) = loop {
            let (placements, content_y) =
                position_block_tree(document, &hierarchy, &widths, &measurements);
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
                        &layout_index,
                        &hierarchy,
                        table,
                        presentation.text(),
                        shaper,
                        TableIntrinsicState {
                            widths: &mut intrinsic_widths,
                            cache: &mut self.table_intrinsics,
                        },
                    )?;
                    table_intrinsics_ready[table] = true;
                }
                let next_widths =
                    WidthPlan::new(document, &hierarchy, viewport.width, &intrinsic_widths);
                for index in 0..document.blocks.len() {
                    if widths.content[index].to_bits() != next_widths.content[index].to_bits() {
                        block_data[index] = None;
                        measurements[index] = estimated_block_measurement(&layout_index, index);
                        measured[index] = false;
                    }
                }
                widths = next_widths;
                continue;
            }
            let pending = measurement_indices
                .iter()
                .copied()
                .filter(|index| !measured[*index] || block_data[*index].is_none())
                .collect::<Vec<_>>();
            if pending.is_empty() {
                break (placements, content_y);
            }
            for index in pending {
                let data = measure_block(
                    index,
                    document,
                    &layout_index,
                    presentation,
                    widths.content[index],
                    shaper,
                );
                measurements[index] = BlockMeasurement::from_data(&data);
                block_data[index] = Some(data);
                measured[index] = true;
            }
        };
        let measurement_indices = visible_indices(&placements, measurement_min, measurement_max);
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
                content_fingerprint: layout_index.content_fingerprints[index],
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
        // Row ownership comes from the container, never from a Y comparison:
        // every cell of one table row shares that row's key.
        let mut row_owners = document
            .blocks
            .iter()
            .map(|block| (block.id, 0u32))
            .collect::<Vec<_>>();
        for (index, block) in document.blocks.iter().enumerate() {
            if !matches!(block.spec.flow, super::BlockFlow::Table) {
                continue;
            }
            for (ordinal, row) in hierarchy.children[index].iter().enumerate() {
                for cell in &hierarchy.children[*row] {
                    row_owners[*cell] = (block.id, ordinal as u32);
                }
            }
        }
        let mut visual_lines = Vec::new();
        let mut lane_drafts = Vec::new();
        let mut lane_offset_stats = LaneOffsetStats::default();
        let mut clusters = Vec::new();
        let mut blocks = Vec::new();
        let mut visible_block_layouts = Vec::new();
        let mut retain_payload = vec![false; document.blocks.len()];
        for index in measurement_indices {
            retain_payload[index] = true;
        }
        for index in visible_indices.iter().copied() {
            let data = block_data[index]
                .as_ref()
                .expect("each visible block is rehydrated in the measurement window");
            append_positioned_block(
                index,
                data,
                placements[index],
                row_owners[index],
                PositionedBlockSink {
                    lines: &mut visual_lines,
                    lanes: &mut lane_drafts,
                    clusters: &mut clusters,
                    blocks: &mut blocks,
                    stats: &mut lane_offset_stats,
                },
            );
            visible_block_layouts.push(data.clone());
        }

        self.blocks = document
            .blocks
            .iter()
            .zip(summaries.iter().cloned())
            .enumerate()
            .map(|(index, (block, summary))| {
                (
                    block.id,
                    CachedBlock {
                        summary,
                        data: retain_payload[index]
                            .then(|| block_data[index].clone())
                            .flatten(),
                        measurement: measurements[index],
                        measured: measured[index],
                    },
                )
            })
            .collect();

        self.last_lane_offset_stats = lane_offset_stats;
        let (rows, lanes) = assemble_rows(lane_drafts, &mut clusters);
        // Array order is not part of the visible range: it folds every lane.
        let visible_source_range = lanes
            .iter()
            .map(|lane| lane.source_range.start())
            .min()
            .zip(lanes.iter().map(|lane| lane.source_range.end()).max())
            .and_then(|(start, end)| TextRange::new(start, end).ok())
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
            LayoutGeometryParts {
                visual_lines: visual_lines.into(),
                rows: rows.into(),
                lanes: lanes.into(),
                blocks: blocks.into(),
                clusters: clusters.into(),
            },
            summaries.into(),
            visible_block_layouts.into(),
        ))
    }
}

struct BlockHierarchy {
    roots: Vec<usize>,
    children: Vec<Vec<usize>>,
    postorder: Vec<usize>,
    node_visits: usize,
}

impl BlockHierarchy {
    fn try_new(document: &LayoutDocument) -> Result<Self, LayoutError> {
        let mut indexes = HashMap::with_capacity(document.blocks.len());
        for (index, block) in document.blocks.iter().enumerate() {
            if indexes.insert(block.id, index).is_some() {
                return Err(LayoutError::DuplicateBlockId { id: block.id });
            }
        }
        let mut roots = Vec::new();
        let mut children = vec![Vec::new(); document.blocks.len()];
        for (index, block) in document.blocks.iter().enumerate() {
            if let Some(parent_id) = block.parent {
                let Some(&parent) = indexes.get(&parent_id) else {
                    return Err(LayoutError::MissingParent {
                        block: block.id,
                        parent: parent_id,
                    });
                };
                children[parent].push(index);
            } else {
                roots.push(index);
            }
        }
        let stable_key = |index: &usize| {
            let block = &document.blocks[*index];
            (
                block.source_range.start(),
                block.source_range.end(),
                block.id.owner.get(),
                block.id.fragment_ordinal,
            )
        };
        roots.sort_by_key(stable_key);
        for child_indexes in &mut children {
            child_indexes.sort_by_key(stable_key);
        }

        let mut traversal_order = (0..document.blocks.len()).collect::<Vec<_>>();
        traversal_order.sort_by_key(stable_key);
        let mut colors = vec![0_u8; document.blocks.len()];
        let mut postorder = Vec::with_capacity(document.blocks.len());
        let mut node_visits = 0_usize;
        for start in traversal_order {
            if colors[start] != 0 {
                continue;
            }
            colors[start] = 1;
            let mut stack = vec![(start, 0_usize)];
            while let Some((node, next_child)) = stack.last_mut() {
                if let Some(&child) = children[*node].get(*next_child) {
                    *next_child += 1;
                    match colors[child] {
                        0 => {
                            colors[child] = 1;
                            stack.push((child, 0));
                        }
                        1 => {
                            return Err(LayoutError::HierarchyCycle {
                                block: document.blocks[child].id,
                            });
                        }
                        _ => {}
                    }
                } else {
                    colors[*node] = 2;
                    postorder.push(*node);
                    node_visits += 1;
                    stack.pop();
                }
            }
        }
        Ok(Self {
            roots,
            children,
            postorder,
            node_visits,
        })
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
    content_width: f64,
    alignment: super::ColumnAlignment,
}

fn position_block_tree(
    document: &LayoutDocument,
    hierarchy: &BlockHierarchy,
    widths: &WidthPlan,
    measurements: &[BlockMeasurement],
) -> (Vec<BlockPlacement>, f64) {
    let empty = BlockPlacement {
        rect: Rect {
            pos: dvec2(0.0, 0.0),
            size: dvec2(0.0, 0.0),
        },
        content_origin: dvec2(0.0, 0.0),
        content_width: 0.0,
        alignment: super::ColumnAlignment::Start,
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
            measurements,
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
    measurements: &[BlockMeasurement],
    placements: &mut [BlockPlacement],
    index: usize,
    x: f64,
    y: f64,
) -> f64 {
    let block = &document.blocks[index];
    let content_x = x + block.spec.insets.left;
    let content_y = y + block.spec.insets.top;
    let own_height = measurements[index].height;
    let body_height = if matches!(block.spec.flow, super::BlockFlow::TableRow) {
        let mut row_height = own_height;
        for &child in &hierarchy.children[index] {
            let child_block = &document.blocks[child];
            let child_y = content_y + child_block.spec.space_before;
            let child_height = position_block(
                document,
                hierarchy,
                widths,
                measurements,
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
                measurements,
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
    placements[index] = BlockPlacement {
        rect: Rect {
            pos: dvec2(x, y),
            size: dvec2(widths.outer[index], height),
        },
        content_origin: dvec2(content_x, content_y),
        content_width: widths.content[index],
        alignment: widths.alignment[index],
    };
    height
}

struct BlockOutput {
    lines: Vec<VisualLine>,
    /// Parallel to `lines`. One lane record per produced line.
    lanes: Vec<BlockLane>,
    clusters: Vec<GlyphCluster>,
    height: f64,
}

#[derive(Clone)]
struct InlinePiece {
    is_marker: bool,
    run: LayoutTextRun,
}

#[derive(Clone)]
struct PendingCluster {
    shaped: ShapedCluster,
    ascender: f64,
    descender: f64,
    line_gap: f64,
}

struct InlineComposer {
    kind: VisualLaneKind,
    start_x: f64,
    start_y: f64,
    max_width: f64,
    y: f64,
    line_width: f64,
    line: Vec<PendingCluster>,
    output: BlockOutput,
    first_baseline: Option<f64>,
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
    layout_index: &DocumentLayoutIndex,
    hierarchy: &BlockHierarchy,
    table: usize,
    source: &SourceText,
    shaper: &mut S,
    state: TableIntrinsicState<'_>,
) -> Result<(), LayoutError> {
    let table_id = document.blocks[table].id;
    let fingerprint = layout_index.subtree_fingerprints[table];
    if let Some(cached) = state
        .cache
        .get(&table_id)
        .filter(|cached| cached.fingerprint == fingerprint)
    {
        for (cell_id, width) in &cached.cells {
            if let Some(index) = layout_index.block_indices.get(cell_id) {
                state.widths[*index] = *width;
            }
        }
        return Ok(());
    }
    let mut cells = Vec::new();
    for &row in &hierarchy.children[table] {
        for &cell in &hierarchy.children[row] {
            let width =
                measure_block_intrinsic(document, layout_index, hierarchy, cell, source, shaper)?;
            state.widths[cell] = width;
            cells.push((document.blocks[cell].id, width));
        }
    }
    state
        .cache
        .insert(table_id, CachedTableIntrinsics { fingerprint, cells });
    Ok(())
}

fn measure_block_intrinsic<S: TextShaper>(
    document: &LayoutDocument,
    layout_index: &DocumentLayoutIndex,
    hierarchy: &BlockHierarchy,
    block_index: usize,
    source: &SourceText,
    shaper: &mut S,
) -> Result<f64, LayoutError> {
    let mut width = layout_index.embedded_indices[block_index]
        .iter()
        .map(|index| document.embedded_blocks[*index].size.x)
        .fold(0.0, f64::max);
    let runs = layout_index.run_indices[block_index]
        .iter()
        .map(|index| document.text_runs[*index].clone())
        .collect::<Vec<_>>();
    if !runs.is_empty() {
        let spans = shape_spans(&runs);
        let paragraph_range = span_range(&spans, document.blocks[block_index].source_range);
        let intrinsic = shaper.measure_paragraph_intrinsic(ParagraphIntrinsicRequest {
            source,
            paragraph_id: paragraph_geometry_id(document.blocks[block_index].id, 0),
            paragraph_range,
            spans: &spans,
            base_direction: BaseDirection::Auto,
        })?;
        width = width.max(intrinsic.min_content);
    }
    for child in &hierarchy.children[block_index] {
        width = width.max(measure_block_intrinsic(
            document,
            layout_index,
            hierarchy,
            *child,
            source,
            shaper,
        )?);
    }
    Ok(width)
}

fn estimated_block_measurement(
    index: &DocumentLayoutIndex,
    block_index: usize,
) -> BlockMeasurement {
    BlockMeasurement {
        height: index.estimated_heights[block_index],
    }
}

fn measure_block<S: TextShaper>(
    block_index: usize,
    document: &LayoutDocument,
    layout_index: &DocumentLayoutIndex,
    presentation: &MarkdownDocumentSnapshot,
    width: f64,
    shaper: &mut S,
) -> Arc<BlockLayoutData> {
    let block = &document.blocks[block_index];
    let (output, fallback) = match layout_block(
        block_index,
        document,
        layout_index,
        presentation,
        width,
        shaper,
    ) {
        Ok(output) => (output, false),
        Err(_) => (
            fallback_block(block_index, document, layout_index, presentation, width),
            true,
        ),
    };
    Arc::new(block_layout_data(block, width, output, fallback))
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
        lanes: output.lanes.into(),
        glyph_clusters: output.clusters.into(),
    }
}

/// One lane produced by a positioned block, before rows are assembled.
struct LaneDraft {
    row: VisualRowId,
    block: LayoutElementId,
    kind: VisualLaneKind,
    source_range: TextRange,
    rect: Rect,
    cluster_range: Range<usize>,
    stable_order: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LaneOffsetStats {
    /// Cluster placements that read their lane offset by direct array index.
    pub direct_lane_offset_lookups: usize,
    /// Cluster placements that had to scan lanes to find their offset.
    pub linear_lane_offset_scans: usize,
}

struct PositionedBlockSink<'a> {
    lines: &'a mut Vec<VisualLine>,
    lanes: &'a mut Vec<LaneDraft>,
    clusters: &'a mut Vec<GlyphCluster>,
    blocks: &'a mut Vec<BlockGeometry>,
    stats: &'a mut LaneOffsetStats,
}

fn append_positioned_block(
    document_index: usize,
    data: &BlockLayoutData,
    placement: BlockPlacement,
    row_owner: (LayoutElementId, u32),
    sink: PositionedBlockSink<'_>,
) {
    let PositionedBlockSink {
        lines,
        lanes,
        clusters,
        blocks,
        stats,
    } = sink;
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
    // Each lane owns a contiguous block-local cluster range, so every cluster
    // reads its alignment offset by direct index instead of scanning lines.
    let lane_offsets = data
        .visual_lines
        .iter()
        .map(|line| {
            alignment_offset(
                placement.alignment,
                placement.content_width,
                line.rect.size.x,
            )
        })
        .collect::<Vec<_>>();
    let mut cluster_lane = vec![usize::MAX; data.glyph_clusters.len()];
    for (index, lane) in data.lanes.iter().enumerate() {
        for slot in cluster_lane
            .get_mut(lane.cluster_range.clone())
            .unwrap_or_default()
        {
            *slot = index;
        }
    }
    let cluster_base = clusters.len();
    for (index, (line, lane)) in data.visual_lines.iter().zip(data.lanes.iter()).enumerate() {
        let offset = lane_offsets[index];
        let mut line = *line;
        line.rect.pos.x += x + offset;
        line.rect.pos.y += y;
        lines.push(line);
        lanes.push(LaneDraft {
            row: VisualRowId {
                owner: row_owner.0,
                row_ordinal: row_owner.1,
                line_ordinal: lane.line_ordinal,
            },
            block: data.block.id,
            kind: lane.kind,
            source_range: line.source_range,
            rect: line.rect,
            cluster_range: cluster_base + lane.cluster_range.start
                ..cluster_base + lane.cluster_range.end,
            stable_order: index as u32,
        });
    }
    let lane_base = lanes.len() - data.lanes.len();
    clusters.extend(
        data.glyph_clusters
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, mut cluster)| {
                let lane = cluster_lane[index];
                let line_offset = lane_offsets.get(lane).copied().unwrap_or(0.0);
                if lane == usize::MAX {
                    stats.linear_lane_offset_scans += 1;
                } else {
                    stats.direct_lane_offset_lookups += 1;
                    cluster.lane_index = lane_base + lane;
                }
                cluster.rect.pos.x += x + line_offset;
                cluster.rect.pos.y += y;
                let mut stops = cluster.caret_stops.to_vec();
                for stop in &mut stops {
                    stop.point.x += x + line_offset;
                    stop.point.y += y;
                }
                cluster.caret_stops = stops.into();
                let mut glyphs = cluster.glyphs.to_vec();
                for glyph in &mut glyphs {
                    glyph.origin.x += x + line_offset;
                    glyph.origin.y += y;
                    glyph.baseline += y;
                }
                cluster.glyphs = glyphs.into();
                cluster
            }),
    );
}

/// Groups lane drafts into visual rows. Rows are ordered by Y, then by X; lanes
/// inside a row are ordered by X. Cluster lane indexes are remapped to the final
/// lane order.
fn assemble_rows(
    drafts: Vec<LaneDraft>,
    clusters: &mut [GlyphCluster],
) -> (Vec<VisualRow>, Vec<VisualLane>) {
    let mut order: HashMap<VisualRowId, usize> = HashMap::new();
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (index, draft) in drafts.iter().enumerate() {
        let group = *order.entry(draft.row).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[group].push(index);
    }
    for group in &mut groups {
        group.sort_by(|left, right| {
            drafts[*left]
                .rect
                .pos
                .x
                .partial_cmp(&drafts[*right].rect.pos.x)
                .unwrap_or(Ordering::Equal)
                .then(left.cmp(right))
        });
    }
    groups.sort_by(|left, right| {
        let left = &drafts[left[0]];
        let right = &drafts[right[0]];
        left.rect
            .pos
            .y
            .partial_cmp(&right.rect.pos.y)
            .unwrap_or(Ordering::Equal)
            .then_with(|| {
                left.rect
                    .pos
                    .x
                    .partial_cmp(&right.rect.pos.x)
                    .unwrap_or(Ordering::Equal)
            })
    });

    let mut rows = Vec::with_capacity(groups.len());
    let mut lanes: Vec<VisualLane> = Vec::with_capacity(drafts.len());
    let mut remap = vec![0usize; drafts.len()];
    for (row_index, group) in groups.iter().enumerate() {
        let start = lanes.len();
        let mut rect = drafts[group[0]].rect;
        for draft_index in group {
            let draft = &drafts[*draft_index];
            rect = union_rect(rect, draft.rect);
            remap[*draft_index] = lanes.len();
            lanes.push(VisualLane {
                id: VisualLaneId {
                    row: draft.row,
                    block: draft.block,
                    stable_order: draft.stable_order,
                },
                row_index,
                kind: draft.kind,
                source_range: draft.source_range,
                rect: draft.rect,
                cluster_range: draft.cluster_range.clone(),
                stable_order: draft.stable_order,
            });
        }
        rows.push(VisualRow {
            id: drafts[group[0]].row,
            rect,
            lanes: start..lanes.len(),
        });
    }
    for cluster in clusters.iter_mut() {
        cluster.lane_index = remap.get(cluster.lane_index).copied().unwrap_or(0);
    }
    (rows, lanes)
}

fn union_rect(left: Rect, right: Rect) -> Rect {
    let x = left.pos.x.min(right.pos.x);
    let y = left.pos.y.min(right.pos.y);
    let right_edge = (left.pos.x + left.size.x).max(right.pos.x + right.size.x);
    let bottom = (left.pos.y + left.size.y).max(right.pos.y + right.size.y);
    Rect {
        pos: dvec2(x, y),
        size: dvec2(right_edge - x, bottom - y),
    }
}

fn alignment_offset(alignment: super::ColumnAlignment, content_width: f64, line_width: f64) -> f64 {
    let free_width = (content_width - line_width).max(0.0);
    match alignment {
        super::ColumnAlignment::Start => 0.0,
        super::ColumnAlignment::Center => free_width * 0.5,
        super::ColumnAlignment::End => free_width,
    }
}

fn layout_block<S: TextShaper>(
    block_index: usize,
    document: &LayoutDocument,
    layout_index: &DocumentLayoutIndex,
    presentation: &MarkdownDocumentSnapshot,
    max_width: f64,
    shaper: &mut S,
) -> Result<BlockOutput, LayoutError> {
    let x = 0.0;
    let y = 0.0;
    let block = &document.blocks[block_index];
    let runs = layout_index.run_indices[block_index]
        .iter()
        .map(|index| document.text_runs[*index].clone())
        .collect::<Vec<_>>();
    if let super::BlockFlow::Hanging {
        marker_range,
        content_indent,
    } = block.spec.flow
    {
        let mut pieces = Vec::new();
        for run in runs {
            let before_end = run.range.end().min(marker_range.start());
            if run.range.start() < before_end {
                push_inline_piece(
                    &mut pieces,
                    false,
                    LayoutTextRun {
                        id: run.id,
                        range: TextRange::new(run.range.start(), before_end)
                            .expect("a hanging prefix stays inside its run"),
                        metrics: run.metrics,
                    },
                );
            }
            let marker_start = run.range.start().max(marker_range.start());
            let marker_end = run.range.end().min(marker_range.end());
            if marker_start < marker_end {
                push_inline_piece(
                    &mut pieces,
                    true,
                    LayoutTextRun {
                        id: run.id,
                        range: TextRange::new(marker_start, marker_end)
                            .expect("a hanging marker stays ordered"),
                        metrics: run.metrics,
                    },
                );
            }
            let content_start = run.range.start().max(marker_range.end());
            if content_start < run.range.end() {
                push_inline_piece(
                    &mut pieces,
                    false,
                    LayoutTextRun {
                        id: run.id,
                        range: TextRange::new(content_start, run.range.end())
                            .expect("hanging content stays inside its run"),
                        metrics: run.metrics,
                    },
                );
            }
        }

        let marker_width = content_indent.max(1.0);
        let content_width = (max_width - content_indent).max(1.0);
        let mut marker = InlineComposer::new(VisualLaneKind::HangingMarker, x, y, marker_width);
        let mut content = InlineComposer::new(
            VisualLaneKind::HangingContent,
            x + content_indent,
            y,
            content_width,
        );
        let marker_runs = pieces
            .iter()
            .filter(|piece| piece.is_marker)
            .map(|piece| piece.run.clone())
            .collect::<Vec<_>>();
        let content_runs = pieces
            .iter()
            .filter(|piece| !piece.is_marker)
            .map(|piece| piece.run.clone())
            .collect::<Vec<_>>();
        if !marker_runs.is_empty() {
            shape_into_composer(
                shaper,
                presentation.text(),
                &marker_runs,
                block.source_range,
                paragraph_geometry_id(block.id, 1),
                &mut marker,
            )?;
        }
        if !content_runs.is_empty() {
            shape_into_composer(
                shaper,
                presentation.text(),
                &content_runs,
                block.source_range,
                paragraph_geometry_id(block.id, 2),
                &mut content,
            )?;
        }
        let (mut marker_output, marker_baseline) = marker.finish();
        let (mut content_output, content_baseline) = content.finish();
        if let (Some(marker_baseline), Some(content_baseline)) = (marker_baseline, content_baseline)
        {
            let shared_baseline = marker_baseline.max(content_baseline);
            shift_output_y(&mut marker_output, shared_baseline - marker_baseline);
            shift_output_y(&mut content_output, shared_baseline - content_baseline);
        }
        let marker_clusters = marker_output.clusters.len();
        return Ok(BlockOutput {
            height: marker_output.height.max(content_output.height),
            lines: marker_output
                .lines
                .into_iter()
                .chain(content_output.lines)
                .collect(),
            lanes: marker_output
                .lanes
                .into_iter()
                .chain(content_output.lanes.into_iter().map(|mut lane| {
                    lane.cluster_range = lane.cluster_range.start + marker_clusters
                        ..lane.cluster_range.end + marker_clusters;
                    lane
                }))
                .collect(),
            clusters: marker_output
                .clusters
                .into_iter()
                .chain(content_output.clusters)
                .collect(),
        });
    }

    let mut coalesced = Vec::new();
    for run in runs {
        push_coalesced_run(&mut coalesced, run);
    }
    let mut composer = InlineComposer::new(lane_kind(&block.spec.flow), x, y, max_width);
    if !coalesced.is_empty() {
        shape_into_composer(
            shaper,
            presentation.text(),
            &coalesced,
            block.source_range,
            paragraph_geometry_id(block.id, 0),
            &mut composer,
        )?;
    }
    let (mut output, _) = composer.finish();
    if output.lines.is_empty() {
        output.height = layout_index.embedded_indices[block_index]
            .iter()
            .map(|index| document.embedded_blocks[*index].size.y)
            .fold(0.0, f64::max);
    }
    Ok(output)
}

fn lane_kind(flow: &super::BlockFlow) -> VisualLaneKind {
    match flow {
        super::BlockFlow::TableCell { column } => VisualLaneKind::TableCell { column: *column },
        _ => VisualLaneKind::Paragraph,
    }
}

fn paragraph_geometry_id(layout: LayoutElementId, lane_ordinal: u32) -> GeometryElementId {
    GeometryElementId {
        layout,
        cluster_ordinal: lane_ordinal,
    }
}

fn shape_spans(runs: &[LayoutTextRun]) -> Vec<ShapeSpan> {
    runs.iter()
        .enumerate()
        .map(|(ordinal, run)| ShapeSpan {
            id: GeometryElementId {
                layout: run.id,
                cluster_ordinal: 0x4000_0000 | ordinal as u32,
            },
            run_id: run.id,
            stable_ordinal: ordinal as u32,
            source_range: run.range,
            metrics: run.metrics,
        })
        .collect()
}

fn span_range(spans: &[ShapeSpan], fallback: TextRange) -> TextRange {
    let Some(first) = spans.first() else {
        return fallback;
    };
    let start = spans
        .iter()
        .map(|span| span.source_range.start())
        .min()
        .unwrap_or(first.source_range.start());
    let end = spans
        .iter()
        .map(|span| span.source_range.end())
        .max()
        .unwrap_or(first.source_range.end());
    TextRange::new(start, end).expect("paragraph spans stay ordered")
}

fn shape_into_composer<S: TextShaper>(
    shaper: &mut S,
    source: &SourceText,
    runs: &[LayoutTextRun],
    fallback_range: TextRange,
    paragraph_id: GeometryElementId,
    composer: &mut InlineComposer,
) -> Result<(), LayoutError> {
    let spans = shape_spans(runs);
    let shaped = shaper.shape_paragraph(ParagraphShapeRequest {
        source,
        paragraph_id,
        paragraph_range: span_range(&spans, fallback_range),
        spans: &spans,
        full_width: composer.max_width,
        first_row_width: composer.remaining_width(),
        base_direction: BaseDirection::Auto,
    })?;
    validate_shaped_paragraph(&spans, &shaped)?;
    composer.push_paragraph(shaped);
    Ok(())
}

fn validate_shaped_paragraph(
    spans: &[ShapeSpan],
    shaped: &ShapedParagraph,
) -> Result<(), LayoutError> {
    let mut ids = std::collections::HashSet::new();
    for id in shaped
        .rows
        .iter()
        .map(|row| row.id)
        .chain(shaped.fragments.iter().map(|fragment| fragment.id))
        .chain(shaped.clusters.iter().map(|cluster| cluster.id))
    {
        if !ids.insert(id) {
            return Err(LayoutError::DuplicateShapedId { id });
        }
    }
    let input = spans
        .iter()
        .map(|span| span.id)
        .collect::<std::collections::HashSet<_>>();
    for span_id in shaped
        .fragments
        .iter()
        .map(|fragment| fragment.span_id)
        .chain(shaped.clusters.iter().map(|cluster| cluster.span_id))
    {
        if !input.contains(&span_id) {
            return Err(LayoutError::MissingShapeSpan { id: span_id });
        }
    }
    let mapped = shaped
        .fragments
        .iter()
        .map(|fragment| fragment.span_id)
        .collect::<std::collections::HashSet<_>>();
    for span in spans {
        if !mapped.contains(&span.id) {
            return Err(LayoutError::MissingShapeSpan { id: span.id });
        }
    }
    Ok(())
}

fn push_coalesced_run(runs: &mut Vec<LayoutTextRun>, run: LayoutTextRun) {
    if let Some(previous) = runs.last_mut() {
        if previous.id == run.id
            && previous.metrics == run.metrics
            && previous.range.end() == run.range.start()
        {
            previous.range = TextRange::new(previous.range.start(), run.range.end())
                .expect("coalesced inline runs stay ordered");
            return;
        }
    }
    runs.push(run);
}

fn push_inline_piece(pieces: &mut Vec<InlinePiece>, is_marker: bool, run: LayoutTextRun) {
    if let Some(previous) = pieces.last_mut() {
        if previous.is_marker == is_marker
            && previous.run.id == run.id
            && previous.run.metrics == run.metrics
            && previous.run.range.end() == run.range.start()
        {
            previous.run.range = TextRange::new(previous.run.range.start(), run.range.end())
                .expect("coalesced hanging runs stay ordered");
            return;
        }
    }
    pieces.push(InlinePiece { is_marker, run });
}

fn shift_output_y(output: &mut BlockOutput, delta: f64) {
    if delta <= 0.0 {
        return;
    }
    for line in &mut output.lines {
        line.rect.pos.y += delta;
    }
    for cluster in &mut output.clusters {
        cluster.rect.pos.y += delta;
        cluster.caret_stops = cluster
            .caret_stops
            .iter()
            .cloned()
            .map(|mut stop| {
                stop.point.y += delta;
                stop
            })
            .collect::<Vec<_>>()
            .into();
        cluster.glyphs = cluster
            .glyphs
            .iter()
            .cloned()
            .map(|mut glyph| {
                glyph.origin.y += delta;
                glyph.baseline += delta;
                glyph
            })
            .collect::<Vec<_>>()
            .into();
    }
    output.height += delta;
}

fn fallback_block(
    block_index: usize,
    document: &LayoutDocument,
    layout_index: &DocumentLayoutIndex,
    presentation: &MarkdownDocumentSnapshot,
    max_width: f64,
) -> BlockOutput {
    let x = 0.0;
    let y = 0.0;
    let block = &document.blocks[block_index];
    let metrics = document_metrics(block_index, document, layout_index);
    let text = presentation.text().slice(block.source_range).unwrap_or("");
    let mut shaped = Vec::new();
    for (relative, character) in text.char_indices() {
        let start = block.source_range.start().to_usize() + relative;
        let end = start + character.len_utf8();
        shaped.push(ShapedCluster {
            id: GeometryElementId {
                layout: block.id,
                cluster_ordinal: 0,
            },
            span_id: GeometryElementId {
                layout: block.id,
                cluster_ordinal: 0x4000_0000,
            },
            source_range: text_range(start, end),
            metrics,
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
                font_key: metrics.font,
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
    let paragraph_id = paragraph_geometry_id(block.id, 0);
    for (ordinal, cluster) in shaped.iter_mut().enumerate() {
        cluster.id = GeometryElementId {
            layout: paragraph_id.layout,
            cluster_ordinal: 0x8000_0000 | ordinal as u32,
        };
        cluster.span_id = GeometryElementId {
            layout: block.id,
            cluster_ordinal: 0x4000_0000,
        };
        cluster.metrics = metrics;
    }
    let row = ShapedRow {
        id: GeometryElementId {
            layout: paragraph_id.layout,
            cluster_ordinal: 0xc000_0000,
        },
        source_range: block.source_range,
        cluster_range: 0..shaped.len(),
        caret_offsets: Arc::from([block.source_range.start(), block.source_range.end()]),
        ascender: 12.8,
        descender: 3.2,
        line_gap: 0.0,
        line_spacing_scale: 1.0,
        row_top: 0.0,
    };
    let paragraph = ShapedParagraph {
        rows: Arc::from([row]),
        fragments: Arc::from([ShapedFragment {
            id: GeometryElementId {
                layout: paragraph_id.layout,
                cluster_ordinal: 0x6000_0000,
            },
            span_id: GeometryElementId {
                layout: block.id,
                cluster_ordinal: 0x4000_0000,
            },
            stable_ordinal: 0,
            source_range: block.source_range,
            metrics,
        }]),
        clusters: shaped.into(),
        bidi_levels: Arc::from([]),
        legal_breaks: Arc::from([block.source_range.end()]),
    };
    let mut composer = InlineComposer::new(lane_kind(&block.spec.flow), x, y, max_width);
    composer.push_paragraph(paragraph);
    composer.finish().0
}

impl InlineComposer {
    fn new(kind: VisualLaneKind, start_x: f64, start_y: f64, max_width: f64) -> Self {
        Self {
            kind,
            start_x,
            start_y,
            max_width: max_width.max(1.0),
            y: start_y,
            line_width: 0.0,
            line: Vec::new(),
            output: BlockOutput {
                lines: Vec::new(),
                lanes: Vec::new(),
                clusters: Vec::new(),
                height: 0.0,
            },
            first_baseline: None,
        }
    }

    fn remaining_width(&self) -> f64 {
        (self.max_width - self.line_width).max(1.0)
    }

    fn push_paragraph(&mut self, paragraph: ShapedParagraph) {
        let paragraph_start_y = self.y;
        for row in paragraph.rows.iter() {
            self.flush_line();
            self.y = self.y.max(paragraph_start_y + row.row_top);
            for shaped in paragraph.clusters[row.cluster_range.clone()].iter() {
                let ascender = shaped
                    .glyphs
                    .iter()
                    .map(|glyph| glyph.ascender)
                    .fold(shaped.metrics.font_size as f64 * 0.8, f64::max);
                let descender = shaped
                    .glyphs
                    .iter()
                    .map(|glyph| glyph.descender.abs())
                    .fold(shaped.metrics.font_size as f64 * 0.2, f64::max);
                let line_gap = shaped
                    .glyphs
                    .iter()
                    .map(|glyph| glyph.line_gap)
                    .fold(0.0, f64::max);
                self.line.push(PendingCluster {
                    shaped: shaped.clone(),
                    ascender,
                    descender,
                    line_gap,
                });
                self.line_width += shaped.advance;
            }
            if self.line.is_empty() {
                let height = (row.ascender + row.descender.abs() + row.line_gap).max(1.0);
                let clusters = self.output.clusters.len();
                self.output.lanes.push(BlockLane {
                    kind: self.kind,
                    line_ordinal: self.output.lines.len() as u32,
                    cluster_range: clusters..clusters,
                });
                self.output.lines.push(VisualLine::new(
                    row.source_range,
                    Rect {
                        pos: dvec2(self.start_x, self.y),
                        size: dvec2(0.0, height),
                    },
                ));
                self.first_baseline.get_or_insert(self.y + row.ascender);
                self.y += height;
            } else {
                self.flush_line();
            }
        }
    }

    fn flush_line(&mut self) {
        let Some(ascender) = flush_inline_line(
            &mut self.output,
            self.kind,
            &self.line,
            self.start_x,
            self.y,
        ) else {
            return;
        };
        self.first_baseline.get_or_insert(self.y + ascender);
        let height = self.output.lines.last().map_or(0.0, VisualLine::height);
        self.y += height;
        self.line.clear();
        self.line_width = 0.0;
    }

    fn finish(mut self) -> (BlockOutput, Option<f64>) {
        self.flush_line();
        self.output.height = (self.y - self.start_y).max(0.0);
        (self.output, self.first_baseline)
    }
}

fn flush_inline_line(
    output: &mut BlockOutput,
    kind: VisualLaneKind,
    line_clusters: &[PendingCluster],
    start_x: f64,
    y: f64,
) -> Option<f64> {
    line_clusters.first()?;
    let source_start = line_clusters
        .iter()
        .map(|cluster| cluster.shaped.source_range.start())
        .min()
        .expect("a line has a first source boundary");
    let source_end = line_clusters
        .iter()
        .map(|cluster| cluster.shaped.source_range.end())
        .max()
        .expect("a line has a last source boundary");
    let width = line_clusters
        .iter()
        .map(|cluster| cluster.shaped.advance)
        .sum();
    let ascender = line_clusters
        .iter()
        .map(|cluster| cluster.ascender)
        .fold(0.0, f64::max);
    let descender = line_clusters
        .iter()
        .map(|cluster| cluster.descender.abs())
        .fold(0.0, f64::max);
    let line_gap = line_clusters
        .iter()
        .map(|cluster| cluster.line_gap)
        .fold(0.0, f64::max);
    let height = (ascender + descender + line_gap).max(1.0);
    let cluster_start = output.clusters.len();
    output.lanes.push(BlockLane {
        kind,
        line_ordinal: output.lines.len() as u32,
        cluster_range: cluster_start..cluster_start + line_clusters.len(),
    });
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
    for cluster in visual_clusters {
        let shaped = &cluster.shaped;
        let metric_y = y + ascender - cluster.ascender;
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
                    dvec2(x + shaped.advance * fraction, metric_y),
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
                    metric_y + glyph.baseline + glyph.origin.y,
                );
                glyph.baseline += metric_y;
                glyph
            })
            .collect::<Vec<_>>();
        output.clusters.push(GlyphCluster::with_glyphs(
            shaped.id,
            shaped.source_range,
            Rect {
                pos: dvec2(x, y),
                size: dvec2(shaped.advance, height),
            },
            stops.into(),
            shaped.metrics,
            glyphs.into(),
        ));
        x += shaped.advance;
    }
    Some(ascender)
}

fn reorder_by_bidi_level(clusters: &mut [PendingCluster]) {
    let Some(max_level) = clusters
        .iter()
        .map(|cluster| cluster.shaped.bidi_level)
        .max()
    else {
        return;
    };
    let Some(min_odd_level) = clusters
        .iter()
        .map(|cluster| cluster.shaped.bidi_level)
        .filter(|level| level % 2 == 1)
        .min()
    else {
        return;
    };
    for level in (min_odd_level..=max_level).rev() {
        let mut start = 0;
        while start < clusters.len() {
            while start < clusters.len() && clusters[start].shaped.bidi_level < level {
                start += 1;
            }
            let mut end = start;
            while end < clusters.len() && clusters[end].shaped.bidi_level >= level {
                end += 1;
            }
            clusters[start..end].reverse();
            start = end;
        }
    }
}

fn document_metrics(
    block_index: usize,
    document: &LayoutDocument,
    layout_index: &DocumentLayoutIndex,
) -> TextMetrics {
    layout_index.run_indices[block_index]
        .first()
        .map(|index| document.text_runs[*index].metrics)
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

fn empty_range() -> TextRange {
    TextRange::new(TextSize::new(0), TextSize::new(0)).expect("zero range is ordered")
}

fn text_size(value: usize) -> TextSize {
    TextSize::try_from_usize(value).expect("source offsets fit TextSize")
}

fn text_range(start: usize, end: usize) -> TextRange {
    TextRange::new(text_size(start), text_size(end)).expect("source range is ordered")
}
