use std::{cmp::Ordering, ops::Range, sync::Arc};

use makepad_widgets::{dvec2, DVec2, Rect};
use waml_syntax::{
    parse_markdown, DocumentRevision, MarkdownDialect, SourceText, SyntaxIdentity, TextRange,
    TextSize,
};

use crate::selection::{Affinity, Selection, TextPosition};

use super::{
    BlockSummary, FontKey, FontWeight, GeometryElementId, LayoutElementId, ShapedGlyph, TextMetrics,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaretStop {
    pub position: TextPosition,
    pub point: DVec2,
}

impl CaretStop {
    pub fn new(position: TextPosition, point: DVec2) -> Self {
        Self { position, point }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaretGeometry {
    pub position: TextPosition,
    pub rect: Rect,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GlyphCluster {
    pub id: GeometryElementId,
    pub source_range: TextRange,
    pub rect: Rect,
    pub caret_stops: Arc<[CaretStop]>,
    /// The exact metric record that produced this placement. Renderers must
    /// paint with it rather than reconstructing a parallel text style.
    pub metrics: TextMetrics,
    /// Exact renderer glyphs and font instances retained by the shaper.
    pub glyphs: Arc<[ShapedGlyph]>,
    /// Index of the owning lane in `LayoutSnapshot::visual_lanes`.
    pub lane_index: usize,
}

impl GlyphCluster {
    pub fn new(
        id: GeometryElementId,
        source_range: TextRange,
        rect: Rect,
        caret_stops: Arc<[CaretStop]>,
    ) -> Self {
        Self::with_metrics(id, source_range, rect, caret_stops, default_text_metrics())
    }

    pub fn with_metrics(
        id: GeometryElementId,
        source_range: TextRange,
        rect: Rect,
        caret_stops: Arc<[CaretStop]>,
        metrics: TextMetrics,
    ) -> Self {
        Self::with_glyphs(id, source_range, rect, caret_stops, metrics, Arc::from([]))
    }

    pub fn with_glyphs(
        id: GeometryElementId,
        source_range: TextRange,
        rect: Rect,
        caret_stops: Arc<[CaretStop]>,
        metrics: TextMetrics,
        glyphs: Arc<[ShapedGlyph]>,
    ) -> Self {
        Self {
            id,
            source_range,
            rect,
            caret_stops,
            metrics,
            glyphs,
            lane_index: 0,
        }
    }

    #[doc(hidden)]
    pub fn for_test(source_range: TextRange, rect: Rect, caret_stops: Vec<CaretStop>) -> Self {
        Self::new(
            GeometryElementId {
                layout: LayoutElementId {
                    owner: fixture_identity(),
                    fragment_ordinal: 0,
                },
                cluster_ordinal: 0,
            },
            source_range,
            rect,
            caret_stops.into(),
        )
    }
}

/// Identity of one visual row. A row groups every lane that shares one
/// horizontal band because the container placed them there, never because their
/// rectangles happen to overlap in Y.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct VisualRowId {
    /// Container that owns the row: the table for table cells, otherwise the
    /// block itself.
    pub owner: LayoutElementId,
    /// Table-row ordinal inside `owner`, or zero outside a table.
    pub row_ordinal: u32,
    /// Line ordinal inside the owning container's row.
    pub line_ordinal: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct VisualLaneId {
    pub row: VisualRowId,
    pub block: LayoutElementId,
    pub stable_order: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VisualLaneKind {
    Paragraph,
    TableCell { column: u32 },
    HangingMarker,
    HangingContent,
}

/// One independently navigable text lane inside a visual row.
#[derive(Clone, Debug, PartialEq)]
pub struct VisualLane {
    pub id: VisualLaneId,
    pub row_index: usize,
    pub kind: VisualLaneKind,
    pub source_range: TextRange,
    pub rect: Rect,
    pub cluster_range: Range<usize>,
    pub stable_order: u32,
}

/// A horizontal band owning one or more independent lanes.
#[derive(Clone, Debug, PartialEq)]
pub struct VisualRow {
    pub id: VisualRowId,
    pub rect: Rect,
    pub lanes: Range<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisualLine {
    pub source_range: TextRange,
    pub rect: Rect,
}

impl VisualLine {
    pub fn new(source_range: TextRange, rect: Rect) -> Self {
        Self { source_range, rect }
    }

    #[doc(hidden)]
    pub fn for_test(source_range: TextRange, y: f64, height: f64) -> Self {
        Self::new(
            source_range,
            Rect {
                pos: dvec2(0.0, y),
                size: dvec2(0.0, height),
            },
        )
    }

    pub fn height(&self) -> f64 {
        self.rect.size.y
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockGeometry {
    pub id: LayoutElementId,
    pub source_range: TextRange,
    pub rect: Rect,
    document_index: Option<usize>,
    plain_text_fallback: bool,
}

/// Block-local lane record. Parallel to `BlockLayoutData::visual_lines`.
#[derive(Clone, Debug, PartialEq)]
pub struct BlockLane {
    pub kind: VisualLaneKind,
    /// Line ordinal inside this lane's own flow, used as the row key.
    pub line_ordinal: u32,
    /// Block-local cluster indexes owned by this lane.
    pub cluster_range: Range<usize>,
}

/// Immutable block-local geometry cached and reused across layout passes.
#[derive(Clone, Debug)]
pub struct BlockLayoutData {
    pub(crate) block: BlockGeometry,
    pub(crate) visual_lines: Arc<[VisualLine]>,
    pub(crate) lanes: Arc<[BlockLane]>,
    pub(crate) glyph_clusters: Arc<[GlyphCluster]>,
}

impl BlockLayoutData {
    pub fn block(&self) -> BlockGeometry {
        self.block
    }

    pub fn visual_lines(&self) -> &[VisualLine] {
        &self.visual_lines
    }

    pub fn lanes(&self) -> &[BlockLane] {
        &self.lanes
    }

    pub fn glyph_clusters(&self) -> &[GlyphCluster] {
        &self.glyph_clusters
    }
}

impl BlockGeometry {
    pub fn new(id: LayoutElementId, source_range: TextRange, rect: Rect) -> Self {
        Self {
            id,
            source_range,
            rect,
            document_index: None,
            plain_text_fallback: false,
        }
    }

    pub(crate) fn fallback(id: LayoutElementId, source_range: TextRange, rect: Rect) -> Self {
        Self {
            id,
            source_range,
            rect,
            document_index: None,
            plain_text_fallback: true,
        }
    }

    pub fn source_range(&self) -> TextRange {
        self.source_range
    }

    /// Exact index of this compact visible entry in `LayoutDocument::blocks`.
    pub fn document_index(&self) -> Option<usize> {
        self.document_index
    }

    pub(crate) fn set_document_index(&mut self, document_index: usize) {
        self.document_index = Some(document_index);
    }

    pub fn is_plain_text_fallback(&self) -> bool {
        self.plain_text_fallback
    }
}

#[derive(Clone, Debug)]
pub struct LayoutSnapshot {
    revision: DocumentRevision,
    viewport_width: f64,
    content_size: DVec2,
    visual_lines: Arc<[VisualLine]>,
    rows: Arc<[VisualRow]>,
    lanes: Arc<[VisualLane]>,
    blocks: Arc<[BlockGeometry]>,
    clusters: Arc<[GlyphCluster]>,
    visible_source_range: TextRange,
    visible_block_range: Range<usize>,
    block_summaries: Arc<[BlockSummary]>,
    block_layouts: Arc<[Arc<BlockLayoutData>]>,
    dirty_block_range: Range<usize>,
}

pub struct LayoutSnapshotMetadata {
    pub revision: DocumentRevision,
    pub viewport_width: f64,
    pub content_size: DVec2,
    pub visible_source_range: TextRange,
    pub visible_block_range: Range<usize>,
    pub dirty_block_range: Range<usize>,
}

/// Positioned geometry arrays of one snapshot.
pub struct LayoutGeometryParts {
    pub visual_lines: Arc<[VisualLine]>,
    pub rows: Arc<[VisualRow]>,
    pub lanes: Arc<[VisualLane]>,
    pub blocks: Arc<[BlockGeometry]>,
    pub clusters: Arc<[GlyphCluster]>,
}

impl LayoutSnapshot {
    pub fn new(
        metadata: LayoutSnapshotMetadata,
        geometry: LayoutGeometryParts,
        block_summaries: Arc<[BlockSummary]>,
        block_layouts: Arc<[Arc<BlockLayoutData>]>,
    ) -> Self {
        Self {
            revision: metadata.revision,
            viewport_width: metadata.viewport_width,
            content_size: metadata.content_size,
            visual_lines: geometry.visual_lines,
            rows: geometry.rows,
            lanes: geometry.lanes,
            blocks: geometry.blocks,
            clusters: geometry.clusters,
            visible_source_range: metadata.visible_source_range,
            visible_block_range: metadata.visible_block_range,
            block_summaries,
            block_layouts,
            dirty_block_range: metadata.dirty_block_range,
        }
    }

    pub fn revision(&self) -> DocumentRevision {
        self.revision
    }

    pub fn viewport_width(&self) -> f64 {
        self.viewport_width
    }

    pub fn content_size(&self) -> DVec2 {
        self.content_size
    }

    pub fn visible_source_range(&self) -> TextRange {
        self.visible_source_range
    }

    /// Deprecated compatibility alias. This range is local to `blocks()`.
    /// Use `visible_block_local_range` and each block's `document_index`.
    pub fn visible_block_range(&self) -> Range<usize> {
        self.visible_block_local_range()
    }

    /// Deprecated compatibility envelope. Visible document indexes can be
    /// sparse; use each visible block's exact `document_index` instead.
    pub fn visible_block_document_range(&self) -> Range<usize> {
        self.visible_block_range.clone()
    }

    /// Indexes valid for `visible_blocks` in this snapshot.
    pub fn visible_block_local_range(&self) -> Range<usize> {
        0..self.blocks.len()
    }

    pub fn visible_blocks(&self) -> &[BlockGeometry] {
        &self.blocks
    }

    pub fn document_block_index(&self, local_index: usize) -> Option<usize> {
        self.blocks
            .get(local_index)
            .and_then(BlockGeometry::document_index)
    }

    /// Deprecated compatibility alias for `visible_blocks`.
    pub fn blocks(&self) -> &[BlockGeometry] {
        &self.blocks
    }

    /// Flattened compatibility view of the lanes. Navigation, hit testing,
    /// selection, and visible-range logic use `visual_rows` and `visual_lanes`.
    pub fn visual_lines(&self) -> &[VisualLine] {
        &self.visual_lines
    }

    pub fn visual_rows(&self) -> &[VisualRow] {
        &self.rows
    }

    pub fn visual_lanes(&self) -> &[VisualLane] {
        &self.lanes
    }

    /// Renderer-ready glyph placements. These are the same clusters used by
    /// source/caret/selection geometry.
    pub fn glyph_clusters(&self) -> &[GlyphCluster] {
        &self.clusters
    }

    pub fn block_summaries(&self) -> &[BlockSummary] {
        &self.block_summaries
    }

    pub fn visible_block_layouts(&self) -> &[Arc<BlockLayoutData>] {
        &self.block_layouts
    }

    pub fn dirty_block_document_range(&self) -> Range<usize> {
        self.dirty_block_range.clone()
    }

    /// Largest scroll offset that still shows content in `viewport_height`.
    pub fn max_scroll_y(&self, viewport_height: f64) -> f64 {
        (self.content_size.y - viewport_height).max(0.0)
    }

    /// Interpolates from `previous` toward `target` by an already-eased scalar.
    ///
    /// The result starts from the complete target snapshot and replaces only
    /// the geometry of elements that survive under a stable identity. New
    /// target elements stay at target geometry and deleted elements are absent,
    /// so nothing is ever invented or resurrected mid-transition.
    pub fn interpolate(previous: &Self, target: &Self, eased: f64) -> Self {
        let eased = eased.clamp(0.0, 1.0);
        let previous_clusters = previous
            .clusters
            .iter()
            .map(|cluster| (cluster.id, cluster))
            .collect::<std::collections::BTreeMap<_, _>>();
        let previous_blocks = previous
            .blocks
            .iter()
            .map(|block| (block.id, block))
            .collect::<std::collections::BTreeMap<_, _>>();

        let clusters = target
            .clusters
            .iter()
            .map(|cluster| {
                let Some(from) = previous_clusters.get(&cluster.id) else {
                    return cluster.clone();
                };
                let mut moved = cluster.clone();
                moved.rect = lerp_rect(from.rect, cluster.rect, eased);
                moved.caret_stops = interpolate_stops(
                    from.source_range,
                    cluster.source_range,
                    &from.caret_stops,
                    &cluster.caret_stops,
                    eased,
                );
                moved.glyphs = interpolate_glyphs(&from.glyphs, &cluster.glyphs, eased);
                moved
            })
            .collect::<Vec<_>>();
        let blocks = target
            .blocks
            .iter()
            .map(|block| {
                let mut moved = *block;
                if let Some(from) = previous_blocks.get(&block.id) {
                    moved.rect = lerp_rect(from.rect, block.rect, eased);
                }
                moved
            })
            .collect::<Vec<_>>();
        let lanes = target
            .lanes
            .iter()
            .cloned()
            .map(|mut lane| {
                if let Some(rect) = clusters
                    .get(lane.cluster_range.clone())
                    .and_then(|slice| slice.first().map(|cluster| cluster.rect))
                {
                    lane.rect.pos.y = rect.pos.y;
                }
                lane
            })
            .collect::<Vec<_>>();
        let rows = target
            .rows
            .iter()
            .cloned()
            .map(|mut row| {
                if let Some(lane) = lanes.get(row.lanes.start) {
                    row.rect.pos.y = lane.rect.pos.y;
                }
                row
            })
            .collect::<Vec<_>>();

        Self {
            revision: target.revision,
            viewport_width: target.viewport_width,
            content_size: target.content_size,
            visual_lines: target.visual_lines.clone(),
            rows: rows.into(),
            lanes: lanes.into(),
            blocks: blocks.into(),
            clusters: clusters.into(),
            visible_source_range: target.visible_source_range,
            visible_block_range: target.visible_block_range.clone(),
            block_summaries: target.block_summaries.clone(),
            block_layouts: target.block_layouts.clone(),
            dirty_block_range: target.dirty_block_range.clone(),
        }
    }

    pub fn source_to_point(&self, position: TextPosition) -> Option<CaretGeometry> {
        let (stop, row) = self.find_owned_stop(position)?;
        // The caret spans its whole row. A stop sits at the top of its own
        // cluster, and on a mixed-size row (a heading marker beside body-sized
        // whitespace) a short cluster starts below the row, so anchoring to the
        // stop would push a row-tall caret past the baseline.
        let (top, height) = row.map_or((stop.point.y, 0.0), |index| {
            let rect = self.rows[index].rect;
            (rect.pos.y, rect.size.y)
        });
        Some(CaretGeometry {
            position,
            rect: Rect {
                pos: dvec2(stop.point.x, top),
                size: dvec2(1.0, height),
            },
        })
    }

    pub fn point_to_source(&self, point: DVec2) -> TextPosition {
        let row = self.row_index_at_y(point.y).or_else(|| {
            self.rows
                .iter()
                .enumerate()
                .min_by(|(_, left), (_, right)| {
                    distance_to_y(left.rect, point.y)
                        .partial_cmp(&distance_to_y(right.rect, point.y))
                        .unwrap_or(Ordering::Equal)
                })
                .map(|(index, _)| index)
        });
        let stops = row
            .and_then(|row| self.lane_at(row, point.x))
            .map_or_else(Vec::new, |lane| self.stops_for_lane(&self.lanes[lane]));
        stops
            .into_iter()
            .min_by(|left, right| {
                (left.point.x - point.x)
                    .abs()
                    .partial_cmp(&(right.point.x - point.x).abs())
                    .unwrap_or(Ordering::Equal)
            })
            .or_else(|| self.all_stops().into_iter().next())
            .map_or(
                TextPosition::new(TextSize::new(0), Affinity::Before),
                |stop| stop.position,
            )
    }

    pub fn selection_rects(&self, selection: Selection) -> Option<Vec<Rect>> {
        let selection_range = selection.range();
        let mut rects = Vec::new();
        for lane in self.lanes.iter() {
            let start = selection_range.start().max(lane.source_range.start());
            let end = selection_range.end().min(lane.source_range.end());
            if start >= end {
                continue;
            }
            let stops = self.stops_for_lane(lane);
            // A lane in range can own no caret stop at all — a blank line
            // between two blocks, or a decoration-only lane. It contributes no
            // rect of its own, and it must not discard the rects the other
            // lanes already produced: a selection dragged into the space
            // between two rows would otherwise vanish entirely.
            let (Some(start_x), Some(end_x)) = (
                boundary_x(&stops, start, true),
                boundary_x(&stops, end, false),
            ) else {
                continue;
            };
            rects.push(Rect {
                pos: dvec2(start_x.min(end_x), lane.rect.pos.y),
                size: dvec2((end_x - start_x).abs(), lane.rect.size.y),
            });
        }
        Some(rects)
    }

    pub fn move_vertical(
        &self,
        position: TextPosition,
        preferred_x: Option<f64>,
        lines: i32,
    ) -> Option<(TextPosition, f64)> {
        let (stop, row) = self.find_owned_stop(position)?;
        let preferred_x = preferred_x.unwrap_or(stop.point.x);
        let mut current = row? as i64;
        let step = if lines < 0 { -1 } else { 1 };
        // A row can hold no caret stop at all — a decoration-only row, or a
        // lane whose clusters were dropped outside the shaped window. Such a
        // row is skipped rather than ending the motion, so one keypress always
        // reaches the next row the caret can actually sit on.
        let mut remaining = lines.unsigned_abs();
        let mut landing = None;
        while remaining > 0 {
            current += step;
            if !(0..self.rows.len() as i64).contains(&current) {
                break;
            }
            let row = &self.rows[current as usize];
            let stop = self.lanes[row.lanes.clone()]
                .iter()
                .flat_map(|lane| self.stops_for_lane(lane))
                .min_by(|left, right| {
                    (left.point.x - preferred_x)
                        .abs()
                        .partial_cmp(&(right.point.x - preferred_x).abs())
                        .unwrap_or(Ordering::Equal)
                });
            if let Some(stop) = stop {
                landing = Some(stop);
                remaining -= 1;
            }
        }
        Some((landing?.position, preferred_x))
    }

    /// Resolves a source position to the stop of the lane that owns it, with
    /// that lane's row. Falls back to the global ordered stops when no lane
    /// claims the position.
    fn find_owned_stop(&self, position: TextPosition) -> Option<(CaretStop, Option<usize>)> {
        let owning = self
            .lanes
            .iter()
            .filter(|lane| {
                lane.source_range.start() <= position.offset
                    && position.offset <= lane.source_range.end()
            })
            .find_map(|lane| {
                self.stops_for_lane(lane)
                    .into_iter()
                    .find(|stop| compare_position(stop.position, position) == Ordering::Equal)
                    .map(|stop| (stop, Some(lane.row_index)))
            });
        owning.or_else(|| {
            let stops = self.all_stops();
            let stop = stops
                .binary_search_by(|candidate| compare_position(candidate.position, position))
                .ok()
                .map(|index| stops[index])?;
            Some((stop, self.row_index_at_y(stop.point.y)))
        })
    }

    fn all_stops(&self) -> Vec<CaretStop> {
        let mut stops: Vec<_> = self
            .clusters
            .iter()
            .flat_map(|cluster| cluster.caret_stops.iter().copied())
            .collect();
        stops.sort_by(|left, right| compare_position(left.position, right.position));
        stops.dedup_by_key(|stop| (stop.position.offset, stop.position.affinity));
        stops
    }

    fn stops_for_lane(&self, lane: &VisualLane) -> Vec<CaretStop> {
        let mut stops: Vec<_> = self
            .clusters
            .get(lane.cluster_range.clone())
            .unwrap_or_default()
            .iter()
            .flat_map(|cluster| cluster.caret_stops.iter().copied())
            .collect();
        stops.sort_by(|left, right| {
            left.point
                .x
                .partial_cmp(&right.point.x)
                .unwrap_or(Ordering::Equal)
        });
        stops
    }

    fn row_index_at_y(&self, y: f64) -> Option<usize> {
        let index = self
            .rows
            .partition_point(|row| row.rect.pos.y + row.rect.size.y <= y);
        self.rows.get(index).and_then(|row| {
            (row.rect.pos.y <= y && y <= row.rect.pos.y + row.rect.size.y).then_some(index)
        })
    }

    /// Selects the lane of `row` that owns `x`. Uses the nearest lane only when
    /// the point falls outside every lane in the row.
    fn lane_at(&self, row: usize, x: f64) -> Option<usize> {
        let lanes = self.rows.get(row)?.lanes.clone();
        let start = lanes.start;
        let slice = self.lanes.get(lanes)?;
        slice
            .iter()
            .position(|lane| lane.rect.pos.x <= x && x <= lane.rect.pos.x + lane.rect.size.x)
            .or_else(|| {
                slice
                    .iter()
                    .enumerate()
                    .min_by(|(_, left), (_, right)| {
                        distance_to_x(left.rect, x)
                            .partial_cmp(&distance_to_x(right.rect, x))
                            .unwrap_or(Ordering::Equal)
                    })
                    .map(|(index, _)| index)
            })
            .map(|index| start + index)
    }

    #[doc(hidden)]
    pub fn from_parts_for_test(
        revision: DocumentRevision,
        content_size: DVec2,
        visual_lines: Vec<VisualLine>,
        mut clusters: Vec<GlyphCluster>,
        blocks: Vec<BlockGeometry>,
    ) -> Self {
        let visible_source_range = visible_range(&visual_lines);
        let visible_block_range = 0..blocks.len();
        let (rows, lanes) = fixture_rows_and_lanes(&visual_lines, &mut clusters);
        Self::new(
            LayoutSnapshotMetadata {
                revision,
                viewport_width: content_size.x,
                content_size,
                visible_source_range,
                visible_block_range,
                dirty_block_range: 0..0,
            },
            LayoutGeometryParts {
                visual_lines: visual_lines.into(),
                rows: rows.into(),
                lanes: lanes.into(),
                blocks: blocks.into(),
                clusters: clusters.into(),
            },
            Arc::from([]),
            Arc::from([]),
        )
    }

    #[doc(hidden)]
    pub fn wrapped_fixture_for_test() -> Self {
        let owner = fixture_identity();
        let layout = LayoutElementId {
            owner,
            fragment_ordinal: 0,
        };
        Self::from_parts_for_test(
            DocumentRevision::INITIAL,
            dvec2(100.0, 48.0),
            vec![
                VisualLine::for_test(text_range(0, 4), 0.0, 18.0),
                VisualLine::for_test(text_range(4, 9), 18.0, 30.0),
            ],
            vec![
                fixture_cluster(
                    layout,
                    0,
                    text_range(0, 4),
                    0.0,
                    18.0,
                    &[0, 1, 4],
                    &[0.0, 10.0, 40.0],
                ),
                fixture_cluster(
                    layout,
                    1,
                    text_range(4, 9),
                    18.0,
                    30.0,
                    &[4, 8, 9],
                    &[0.0, 45.0, 55.0],
                ),
            ],
            Vec::new(),
        )
    }

    /// Two text lines with an empty line between them. The blank line owns a
    /// lane with no clusters, so that lane carries no caret stop at all — the
    /// shape a selection dragged into the space between two rows has to cross.
    #[doc(hidden)]
    pub fn blank_line_fixture_for_test() -> Self {
        let owner = fixture_identity();
        let layout = LayoutElementId {
            owner,
            fragment_ordinal: 0,
        };
        Self::from_parts_for_test(
            DocumentRevision::INITIAL,
            dvec2(100.0, 60.0),
            vec![
                VisualLine::for_test(text_range(0, 4), 0.0, 20.0),
                VisualLine::for_test(text_range(4, 5), 20.0, 20.0),
                VisualLine::for_test(text_range(5, 9), 40.0, 20.0),
            ],
            vec![
                fixture_cluster(
                    layout,
                    0,
                    text_range(0, 4),
                    0.0,
                    20.0,
                    &[0, 1, 4],
                    &[0.0, 10.0, 40.0],
                ),
                fixture_cluster(
                    layout,
                    1,
                    text_range(5, 9),
                    40.0,
                    20.0,
                    &[5, 8, 9],
                    &[0.0, 45.0, 55.0],
                ),
            ],
            Vec::new(),
        )
    }

    #[doc(hidden)]
    pub fn proportional_fixture_for_test() -> Self {
        let owner = fixture_identity();
        let layout = LayoutElementId {
            owner,
            fragment_ordinal: 0,
        };
        Self::from_parts_for_test(
            DocumentRevision::INITIAL,
            dvec2(80.0, 40.0),
            vec![
                VisualLine::for_test(text_range(0, 3), 0.0, 20.0),
                VisualLine::for_test(text_range(3, 6), 20.0, 20.0),
            ],
            vec![
                fixture_cluster(
                    layout,
                    0,
                    text_range(0, 3),
                    0.0,
                    20.0,
                    &[0, 2, 3],
                    &[0.0, 26.0, 40.0],
                ),
                fixture_cluster(
                    layout,
                    1,
                    text_range(3, 6),
                    20.0,
                    20.0,
                    &[3, 5, 6],
                    &[0.0, 26.0, 50.0],
                ),
            ],
            Vec::new(),
        )
    }
}

fn lerp(from: f64, to: f64, eased: f64) -> f64 {
    from + (to - from) * eased
}

fn lerp_rect(from: Rect, to: Rect, eased: f64) -> Rect {
    Rect {
        pos: dvec2(
            lerp(from.pos.x, to.pos.x, eased),
            lerp(from.pos.y, to.pos.y, eased),
        ),
        size: dvec2(
            lerp(from.size.x, to.size.x, eased),
            lerp(from.size.y, to.size.y, eased),
        ),
    }
}

/// Caret stops of one stable cluster interpolate when their relative byte
/// offsets and affinities agree. The target source position stays authoritative
/// because an edit before the cluster can shift its absolute offsets.
fn interpolate_stops(
    from_range: TextRange,
    to_range: TextRange,
    from: &[CaretStop],
    to: &[CaretStop],
    eased: f64,
) -> Arc<[CaretStop]> {
    if from.len() != to.len() {
        return to.into();
    }
    from.iter()
        .zip(to)
        .map(|(from, to)| {
            let relative = |stop: &CaretStop, range: TextRange| {
                let offset = stop.position.offset.to_usize();
                let start = range.start().to_usize();
                let end = range.end().to_usize();
                (start <= offset && offset <= end)
                    .then(|| offset.checked_sub(start))
                    .flatten()
            };
            let from_relative = relative(from, from_range);
            let to_relative = relative(to, to_range);
            if from_relative.is_none()
                || from_relative != to_relative
                || from.position.affinity != to.position.affinity
            {
                return *to;
            }
            CaretStop::new(
                to.position,
                dvec2(
                    lerp(from.point.x, to.point.x, eased),
                    lerp(from.point.y, to.point.y, eased),
                ),
            )
        })
        .collect::<Vec<_>>()
        .into()
}

fn interpolate_glyphs(from: &[ShapedGlyph], to: &[ShapedGlyph], eased: f64) -> Arc<[ShapedGlyph]> {
    if from.len() != to.len() {
        return to.into();
    }
    from.iter()
        .zip(to)
        .map(|(from, to)| {
            let mut glyph = to.clone();
            glyph.origin = dvec2(
                lerp(from.origin.x, to.origin.x, eased),
                lerp(from.origin.y, to.origin.y, eased),
            );
            glyph.baseline = lerp(from.baseline, to.baseline, eased);
            glyph
        })
        .collect::<Vec<_>>()
        .into()
}

fn default_text_metrics() -> TextMetrics {
    TextMetrics {
        font: FontKey(0),
        font_size: 0.0,
        line_spacing: 0.0,
        weight: FontWeight(0),
        italic: false,
    }
}

fn compare_position(left: TextPosition, right: TextPosition) -> Ordering {
    left.offset
        .cmp(&right.offset)
        .then_with(|| left.affinity.cmp(&right.affinity))
}

fn boundary_x(stops: &[CaretStop], offset: TextSize, start: bool) -> Option<f64> {
    let exact = stops
        .iter()
        .filter(|stop| stop.position.offset == offset)
        .min_by_key(|stop| match (start, stop.position.affinity) {
            (true, Affinity::Before) | (false, Affinity::After) => 0,
            _ => 1,
        });
    exact.map(|stop| stop.point.x).or_else(|| {
        let before = stops
            .iter()
            .rev()
            .find(|stop| stop.position.offset < offset);
        let after = stops.iter().find(|stop| stop.position.offset > offset);
        match (before, after) {
            (Some(left), Some(right)) => Some((left.point.x + right.point.x) * 0.5),
            (Some(left), None) => Some(left.point.x),
            (None, Some(right)) => Some(right.point.x),
            (None, None) => None,
        }
    })
}

/// Builds one paragraph lane per fixture line, each in its own visual row.
fn fixture_rows_and_lanes(
    lines: &[VisualLine],
    clusters: &mut [GlyphCluster],
) -> (Vec<VisualRow>, Vec<VisualLane>) {
    let owner = LayoutElementId {
        owner: fixture_identity(),
        fragment_ordinal: 0,
    };
    let mut rows = Vec::with_capacity(lines.len());
    let mut lanes = Vec::with_capacity(lines.len());
    for (index, line) in lines.iter().enumerate() {
        let start = clusters
            .iter()
            .position(|cluster| cluster.rect.pos.y == line.rect.pos.y)
            .unwrap_or(clusters.len());
        let end = start
            + clusters[start..]
                .iter()
                .take_while(|cluster| cluster.rect.pos.y == line.rect.pos.y)
                .count();
        for cluster in &mut clusters[start..end] {
            cluster.lane_index = index;
        }
        let id = VisualRowId {
            owner,
            row_ordinal: 0,
            line_ordinal: index as u32,
        };
        rows.push(VisualRow {
            id,
            rect: line.rect,
            lanes: index..index + 1,
        });
        lanes.push(VisualLane {
            id: VisualLaneId {
                row: id,
                block: owner,
                stable_order: 0,
            },
            row_index: index,
            kind: VisualLaneKind::Paragraph,
            source_range: line.source_range,
            rect: line.rect,
            cluster_range: start..end,
            stable_order: 0,
        });
    }
    (rows, lanes)
}

fn distance_to_x(rect: Rect, x: f64) -> f64 {
    if x < rect.pos.x {
        rect.pos.x - x
    } else if x > rect.pos.x + rect.size.x {
        x - (rect.pos.x + rect.size.x)
    } else {
        0.0
    }
}

fn distance_to_y(rect: Rect, y: f64) -> f64 {
    if y < rect.pos.y {
        rect.pos.y - y
    } else if y > rect.pos.y + rect.size.y {
        y - (rect.pos.y + rect.size.y)
    } else {
        0.0
    }
}

fn visible_range(lines: &[VisualLine]) -> TextRange {
    let start = lines
        .first()
        .map_or(TextSize::new(0), |line| line.source_range.start());
    let end = lines
        .last()
        .map_or(TextSize::new(0), |line| line.source_range.end());
    TextRange::new(start, end).expect("visible lines are source ordered")
}

fn text_range(start: usize, end: usize) -> TextRange {
    TextRange::new(
        TextSize::try_from_usize(start).expect("fixture offset fits"),
        TextSize::try_from_usize(end).expect("fixture offset fits"),
    )
    .expect("fixture range is ordered")
}

fn fixture_cluster(
    layout: LayoutElementId,
    cluster_ordinal: u32,
    source_range: TextRange,
    y: f64,
    height: f64,
    offsets: &[usize],
    xs: &[f64],
) -> GlyphCluster {
    let caret_stops = offsets
        .iter()
        .zip(xs)
        .enumerate()
        .map(|(index, (offset, x))| {
            CaretStop::new(
                TextPosition::new(
                    TextSize::try_from_usize(*offset).expect("fixture offset fits"),
                    if index == 0 || index + 1 == offsets.len() {
                        Affinity::Before
                    } else {
                        Affinity::After
                    },
                ),
                dvec2(*x, y),
            )
        })
        .collect::<Vec<_>>();
    GlyphCluster::new(
        GeometryElementId {
            layout,
            cluster_ordinal,
        },
        source_range,
        Rect {
            pos: dvec2(0.0, y),
            size: dvec2(*xs.last().unwrap_or(&0.0), height),
        },
        caret_stops.into(),
    )
}

fn fixture_identity() -> SyntaxIdentity {
    let source = SourceText::new("# fixture".to_owned()).expect("fixture source is valid");
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        source,
        MarkdownDialect::WAML_DEFAULT,
    )
    .expect("fixture Markdown parses");
    let owner = snapshot
        .queries()
        .headings()
        .next()
        .expect("fixture contains a heading")
        .owner;
    owner
}

#[cfg(test)]
mod interpolation_tests {
    use makepad_widgets::dvec2;
    use waml_syntax::{TextRange, TextSize};

    use super::{interpolate_stops, Affinity, CaretStop, TextPosition};

    fn range(start: usize, end: usize) -> TextRange {
        TextRange::new(
            TextSize::try_from_usize(start).unwrap(),
            TextSize::try_from_usize(end).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn shifted_stable_cluster_caret_stops_follow_each_motion_phase() {
        let from_range = range(10, 20);
        let to_range = range(30, 40);
        let from = [CaretStop::new(
            TextPosition::new(range(12, 12).start(), Affinity::Before),
            dvec2(8.0, 10.0),
        )];
        let to = [CaretStop::new(
            TextPosition::new(range(32, 32).start(), Affinity::Before),
            dvec2(8.0, 40.0),
        )];
        for (eased, expected_y) in [(0.0, 10.0), (0.5, 25.0), (1.0, 40.0)] {
            let stops = interpolate_stops(from_range, to_range, &from, &to, eased);
            assert_eq!(stops[0].position, to[0].position);
            assert_eq!(stops[0].point.y, expected_y);
        }
    }
}
