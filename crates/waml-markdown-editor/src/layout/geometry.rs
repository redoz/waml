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
    plain_text_fallback: bool,
}

impl BlockGeometry {
    pub fn new(id: LayoutElementId, source_range: TextRange, rect: Rect) -> Self {
        Self {
            id,
            source_range,
            rect,
            plain_text_fallback: false,
        }
    }

    pub(crate) fn fallback(id: LayoutElementId, source_range: TextRange, rect: Rect) -> Self {
        Self {
            id,
            source_range,
            rect,
            plain_text_fallback: true,
        }
    }

    pub fn source_range(&self) -> TextRange {
        self.source_range
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
    blocks: Arc<[BlockGeometry]>,
    clusters: Arc<[GlyphCluster]>,
    visible_source_range: TextRange,
    visible_block_range: Range<usize>,
    block_summaries: Arc<[BlockSummary]>,
}

pub struct LayoutSnapshotMetadata {
    pub revision: DocumentRevision,
    pub viewport_width: f64,
    pub content_size: DVec2,
    pub visible_source_range: TextRange,
    pub visible_block_range: Range<usize>,
}

impl LayoutSnapshot {
    pub fn new(
        metadata: LayoutSnapshotMetadata,
        visual_lines: Arc<[VisualLine]>,
        blocks: Arc<[BlockGeometry]>,
        clusters: Arc<[GlyphCluster]>,
        block_summaries: Arc<[BlockSummary]>,
    ) -> Self {
        Self {
            revision: metadata.revision,
            viewport_width: metadata.viewport_width,
            content_size: metadata.content_size,
            visual_lines,
            blocks,
            clusters,
            visible_source_range: metadata.visible_source_range,
            visible_block_range: metadata.visible_block_range,
            block_summaries,
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

    pub fn visible_block_range(&self) -> Range<usize> {
        self.visible_block_range.clone()
    }

    /// Document indexes covered by the compact visible geometry arrays.
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
        (local_index < self.blocks.len()).then(|| self.visible_block_range.start + local_index)
    }

    pub fn blocks(&self) -> &[BlockGeometry] {
        &self.blocks
    }

    pub fn visual_lines(&self) -> &[VisualLine] {
        &self.visual_lines
    }

    /// Renderer-ready glyph placements. These are the same clusters used by
    /// source/caret/selection geometry.
    pub fn glyph_clusters(&self) -> &[GlyphCluster] {
        &self.clusters
    }

    pub fn block_summaries(&self) -> &[BlockSummary] {
        &self.block_summaries
    }

    pub fn source_to_point(&self, position: TextPosition) -> Option<CaretGeometry> {
        let stop = self.find_stop(position)?;
        let height = self
            .line_at_y(stop.point.y)
            .map_or(0.0, |line| line.rect.size.y);
        Some(CaretGeometry {
            position,
            rect: Rect {
                pos: stop.point,
                size: dvec2(1.0, height),
            },
        })
    }

    pub fn point_to_source(&self, point: DVec2) -> TextPosition {
        let line = self.line_at_y(point.y).or_else(|| {
            self.visual_lines.iter().min_by(|left, right| {
                distance_to_y(left.rect, point.y)
                    .partial_cmp(&distance_to_y(right.rect, point.y))
                    .unwrap_or(Ordering::Equal)
            })
        });
        let stops = line.map_or_else(Vec::new, |line| self.stops_for_line(line));
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
        for line in self.visual_lines.iter() {
            let start = selection_range.start().max(line.source_range.start());
            let end = selection_range.end().min(line.source_range.end());
            if start >= end {
                continue;
            }
            let stops = self.stops_for_line(line);
            let start_x = boundary_x(&stops, start, true)?;
            let end_x = boundary_x(&stops, end, false)?;
            rects.push(Rect {
                pos: dvec2(start_x.min(end_x), line.rect.pos.y),
                size: dvec2((end_x - start_x).abs(), line.rect.size.y),
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
        let caret = self.source_to_point(position)?;
        let preferred_x = preferred_x.unwrap_or(caret.rect.pos.x);
        let current = self.line_index_at_y(caret.rect.pos.y)? as i64;
        let target = current.checked_add(lines as i64)?;
        if !(0..self.visual_lines.len() as i64).contains(&target) {
            return None;
        }
        let stops = self.stops_for_line(&self.visual_lines[target as usize]);
        let stop = stops.into_iter().min_by(|left, right| {
            (left.point.x - preferred_x)
                .abs()
                .partial_cmp(&(right.point.x - preferred_x).abs())
                .unwrap_or(Ordering::Equal)
        })?;
        Some((stop.position, preferred_x))
    }

    fn find_stop(&self, position: TextPosition) -> Option<CaretStop> {
        let stops = self.all_stops();
        stops
            .binary_search_by(|candidate| compare_position(candidate.position, position))
            .ok()
            .map(|index| stops[index])
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

    fn stops_for_line(&self, line: &VisualLine) -> Vec<CaretStop> {
        let mut stops: Vec<_> = self
            .clusters
            .iter()
            .filter(|cluster| {
                cluster.rect.pos.y < line.rect.pos.y + line.rect.size.y
                    && cluster.rect.pos.y + cluster.rect.size.y > line.rect.pos.y
            })
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

    fn line_at_y(&self, y: f64) -> Option<&VisualLine> {
        self.line_index_at_y(y)
            .map(|index| &self.visual_lines[index])
    }

    fn line_index_at_y(&self, y: f64) -> Option<usize> {
        let index = self
            .visual_lines
            .partition_point(|line| line.rect.pos.y + line.rect.size.y <= y);
        self.visual_lines.get(index).and_then(|line| {
            (line.rect.pos.y <= y && y <= line.rect.pos.y + line.rect.size.y).then_some(index)
        })
    }

    #[doc(hidden)]
    pub fn from_parts_for_test(
        revision: DocumentRevision,
        content_size: DVec2,
        visual_lines: Vec<VisualLine>,
        clusters: Vec<GlyphCluster>,
        blocks: Vec<BlockGeometry>,
    ) -> Self {
        let visible_source_range = visible_range(&visual_lines);
        let visible_block_range = 0..blocks.len();
        Self::new(
            LayoutSnapshotMetadata {
                revision,
                viewport_width: content_size.x,
                content_size,
                visible_source_range,
                visible_block_range,
            },
            visual_lines.into(),
            blocks.into(),
            clusters.into(),
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
