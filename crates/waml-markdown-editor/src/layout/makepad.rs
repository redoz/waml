use std::{collections::HashMap, rc::Rc, sync::Arc};

use makepad_widgets::{dvec2, text::layouter::LaidoutText, Align, Cx, DrawText};
use unicode_bidi::BidiInfo;
use waml_syntax::{DocumentRevision, SourceText, TextRange, TextSize};

use super::{
    BaseDirection, FontKey, FontWeight, GeometryElementId, LayoutElementId, LayoutError,
    ParagraphIntrinsic, ParagraphIntrinsicRequest, ParagraphShapeRequest, ShapeSpan, ShapedCluster,
    ShapedFragment, ShapedGlyph, ShapedParagraph, ShapedRow, TextMetrics, TextShaper,
};

pub trait FontResolver {
    fn configure_draw_text(&mut self, key: FontKey, metrics: TextMetrics, draw: &mut DrawText);
}

const FULL_SHAPE_WIDTH: f64 = 1_000_000.0;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct TextLayoutKey {
    revision: DocumentRevision,
    id: LayoutElementId,
    font: FontKey,
    font_size_bits: u32,
    line_spacing_bits: u32,
    weight: FontWeight,
    italic: bool,
    width_bits: u64,
}

#[derive(Default)]
pub(crate) struct MakepadTextLayoutCache {
    entries: HashMap<TextLayoutKey, Rc<LaidoutText>>,
}

impl MakepadTextLayoutCache {
    pub(crate) fn laid_out(
        &self,
        revision: DocumentRevision,
        id: LayoutElementId,
        metrics: TextMetrics,
    ) -> Option<Rc<LaidoutText>> {
        self.entries
            .get(&text_layout_key(revision, id, metrics, FULL_SHAPE_WIDTH).ok()?)
            .cloned()
    }

    pub(crate) fn retain_revision(&mut self, revision: DocumentRevision) {
        self.entries.retain(|key, _| key.revision == revision);
    }
}

pub struct MakepadTextShaper<'a, R> {
    pub cx: &'a mut Cx,
    pub draw_text: &'a mut DrawText,
    pub fonts: &'a mut R,
    pub(crate) revision: DocumentRevision,
    pub(crate) cache: Option<&'a mut MakepadTextLayoutCache>,
}

impl<R: FontResolver> TextShaper for MakepadTextShaper<'_, R> {
    fn shape_paragraph(
        &mut self,
        request: ParagraphShapeRequest<'_>,
    ) -> Result<ShapedParagraph, LayoutError> {
        shape_makepad_paragraph(self, request)
    }

    fn measure_paragraph_intrinsic(
        &mut self,
        request: ParagraphIntrinsicRequest<'_>,
    ) -> Result<ParagraphIntrinsic, LayoutError> {
        measure_makepad_intrinsic(self, request)
    }
}

fn shape_makepad_paragraph<R: FontResolver>(
    shaper: &mut MakepadTextShaper<'_, R>,
    request: ParagraphShapeRequest<'_>,
) -> Result<ShapedParagraph, LayoutError> {
    let paragraph_text = request
        .source
        .slice(request.paragraph_range)
        .map_err(|_| paragraph_error(request.spans, request.paragraph_id))?;
    let base_level = match request.base_direction {
        BaseDirection::Auto => None,
        BaseDirection::LeftToRight => Some(unicode_bidi::Level::ltr()),
        BaseDirection::RightToLeft => Some(unicode_bidi::Level::rtl()),
    };
    let bidi = BidiInfo::new(paragraph_text, base_level);
    let mut clusters = Vec::new();
    // Makepad scales the pitch between baselines by the row's
    // `line_spacing_scale`. Carry it through so our rows advance the same way.
    let mut line_spacing_scale = 1.0_f64;
    for span in request.spans {
        shaper
            .fonts
            .configure_draw_text(span.metrics.font, span.metrics, shaper.draw_text);
        let text = request
            .source
            .slice(span.source_range)
            .map_err(|_| LayoutError::ShapingFailed { run: span.run_id })?;
        let paint_scale = shaper.draw_text.font_scale as f64;
        let layout_scale = shaper.draw_text.font_scale.max(0.0001);
        let max_width = FULL_SHAPE_WIDTH / layout_scale as f64;
        let key = text_layout_key(shaper.revision, span.run_id, span.metrics, FULL_SHAPE_WIDTH)
            .map_err(|_| LayoutError::ShapingFailed { run: span.run_id })?;
        let laid_out = if let Some(cache) = shaper.cache.as_deref_mut() {
            if let Some(cached) = cache.entries.get(&key) {
                cached.clone()
            } else {
                let laid_out = shaper.draw_text.layout(
                    shaper.cx,
                    0.0,
                    0.0,
                    Some(max_width as f32),
                    true,
                    Align::default(),
                    text,
                );
                cache.entries.insert(key, laid_out.clone());
                laid_out
            }
        } else {
            shaper.draw_text.layout(
                shaper.cx,
                0.0,
                0.0,
                Some(max_width as f32),
                true,
                Align::default(),
                text,
            )
        };
        if let Some(row) = laid_out.rows.first() {
            line_spacing_scale = row.line_spacing_scale as f64;
        }
        clusters.extend(extract_clusters(&laid_out, span, paint_scale)?);
    }
    clusters.sort_by_key(|cluster| cluster.source_range.start());
    for (ordinal, cluster) in clusters.iter_mut().enumerate() {
        cluster.id = GeometryElementId {
            layout: request.paragraph_id.layout,
            cluster_ordinal: 0x8000_0000
                | ((request.paragraph_id.cluster_ordinal & 0x0f) << 24)
                | ordinal as u32,
        };
        let relative =
            cluster.source_range.start().to_usize() - request.paragraph_range.start().to_usize();
        cluster.bidi_level = bidi_level_at(&bidi, relative);
    }
    let legal_breaks = legal_breaks(paragraph_text, request.paragraph_range.start().to_usize());
    let rows = break_rows(
        request,
        &clusters,
        &legal_breaks,
        paragraph_text.ends_with('\n'),
        line_spacing_scale,
    );
    let fragments = request
        .spans
        .iter()
        .map(|span| ShapedFragment {
            id: GeometryElementId {
                layout: request.paragraph_id.layout,
                cluster_ordinal: 0x6000_0000
                    | ((request.paragraph_id.cluster_ordinal & 0x0f) << 24)
                    | span.stable_ordinal,
            },
            span_id: span.id,
            stable_ordinal: span.stable_ordinal,
            source_range: span.source_range,
            metrics: span.metrics,
        })
        .collect::<Vec<_>>();
    Ok(ShapedParagraph {
        rows: rows.into(),
        fragments: fragments.into(),
        clusters: clusters.into(),
        bidi_levels: bidi
            .levels
            .iter()
            .map(|level| level.number())
            .collect::<Vec<_>>()
            .into(),
        legal_breaks: legal_breaks.into(),
    })
}

fn text_layout_key(
    revision: DocumentRevision,
    id: LayoutElementId,
    metrics: TextMetrics,
    width: f64,
) -> Result<TextLayoutKey, ()> {
    if !metrics.font_size.is_finite()
        || metrics.font_size < 0.0
        || !metrics.line_spacing.is_finite()
        || metrics.line_spacing < 0.0
        || !width.is_finite()
        || width < 0.0
    {
        return Err(());
    }
    Ok(TextLayoutKey {
        revision,
        id,
        font: metrics.font,
        font_size_bits: metrics.font_size.to_bits(),
        line_spacing_bits: metrics.line_spacing.to_bits(),
        weight: metrics.weight,
        italic: metrics.italic,
        width_bits: width.to_bits(),
    })
}

fn measure_makepad_intrinsic<R: FontResolver>(
    shaper: &mut MakepadTextShaper<'_, R>,
    request: ParagraphIntrinsicRequest<'_>,
) -> Result<ParagraphIntrinsic, LayoutError> {
    let mut advances = Vec::new();
    for span in request.spans {
        shaper
            .fonts
            .configure_draw_text(span.metrics.font, span.metrics, shaper.draw_text);
        let text = request
            .source
            .slice(span.source_range)
            .map_err(|_| LayoutError::ShapingFailed { run: span.run_id })?;
        let paint_scale = shaper.draw_text.font_scale as f64;
        let layout_scale = shaper.draw_text.font_scale.max(0.0001);
        let laid_out = shaper.draw_text.layout_uncached(
            shaper.cx,
            0.0,
            0.0,
            Some(1_000_000.0 / layout_scale),
            true,
            Align::default(),
            text,
        );
        advances.extend(
            extract_clusters(&laid_out, span, paint_scale)?
                .into_iter()
                .map(|cluster| (cluster.source_range, cluster.advance)),
        );
    }
    advances.sort_by_key(|(source_range, _)| source_range.start());
    Ok(reduce_intrinsic_advances(
        request.source,
        request.paragraph_range,
        &advances,
    ))
}

fn reduce_intrinsic_advances(
    source: &SourceText,
    paragraph_range: TextRange,
    advances: &[(TextRange, f64)],
) -> ParagraphIntrinsic {
    let mut min_content = 0.0_f64;
    let mut max_content = 0.0_f64;
    let mut word_width = 0.0_f64;
    let mut line_width = 0.0_f64;
    let mut cursor = paragraph_range.start();
    for &(source_range, advance) in advances {
        if cursor < source_range.start() {
            if let Ok(gap_range) = TextRange::new(cursor, source_range.start()) {
                if let Ok(gap) = source.slice(gap_range) {
                    include_intrinsic_gap(
                        gap,
                        &mut min_content,
                        &mut max_content,
                        &mut word_width,
                        &mut line_width,
                    );
                }
            }
        }
        let text = source.slice(source_range).unwrap_or("");
        if text.contains('\n') {
            min_content = min_content.max(word_width);
            max_content = max_content.max(line_width);
            word_width = 0.0;
            line_width = 0.0;
        } else {
            line_width += advance;
            if text.chars().all(char::is_whitespace) {
                min_content = min_content.max(word_width);
                word_width = 0.0;
            } else {
                word_width += advance;
            }
        }
        cursor = cursor.max(source_range.end());
    }
    if cursor < paragraph_range.end() {
        if let Ok(gap_range) = TextRange::new(cursor, paragraph_range.end()) {
            if let Ok(gap) = source.slice(gap_range) {
                include_intrinsic_gap(
                    gap,
                    &mut min_content,
                    &mut max_content,
                    &mut word_width,
                    &mut line_width,
                );
            }
        }
    }
    ParagraphIntrinsic {
        min_content: min_content.max(word_width),
        max_content: max_content.max(line_width),
    }
}

fn include_intrinsic_gap(
    gap: &str,
    min_content: &mut f64,
    max_content: &mut f64,
    word_width: &mut f64,
    line_width: &mut f64,
) {
    for character in gap.chars() {
        if character == '\n' {
            *min_content = (*min_content).max(*word_width);
            *max_content = (*max_content).max(*line_width);
            *word_width = 0.0;
            *line_width = 0.0;
        } else if character.is_whitespace() {
            *min_content = (*min_content).max(*word_width);
            *word_width = 0.0;
        }
    }
}

fn extract_clusters(
    laid_out: &LaidoutText,
    span: &ShapeSpan,
    paint_scale: f64,
) -> Result<Vec<ShapedCluster>, LayoutError> {
    let mut clusters = Vec::new();
    for row in &laid_out.rows {
        let mut logical_clusters = row
            .glyphs
            .iter()
            .map(|glyph| glyph.cluster)
            .collect::<Vec<_>>();
        logical_clusters.push(row.text.len());
        logical_clusters.sort_unstable();
        logical_clusters.dedup();
        let mut index = 0;
        while index < row.glyphs.len() {
            let cluster = row.glyphs[index].cluster;
            let mut next = index + 1;
            while next < row.glyphs.len() && row.glyphs[next].cluster == cluster {
                next += 1;
            }
            let next_cluster = logical_clusters
                .iter()
                .copied()
                .find(|candidate| *candidate > cluster)
                .unwrap_or(row.text.len());
            let row_start = row.text.start_in_parent();
            let start = span.source_range.start().to_usize() + row_start + cluster;
            let end = span.source_range.start().to_usize() + row_start + next_cluster;
            let advance = row.glyphs.get(next).map_or_else(
                || row.width_in_lpxs - row.glyphs[index].origin_in_lpxs.x,
                |glyph| glyph.origin_in_lpxs.x - row.glyphs[index].origin_in_lpxs.x,
            ) as f64
                * paint_scale;
            let cluster_origin = row.glyphs[index].origin_in_lpxs.x;
            let glyphs = row.glyphs[index..next]
                .iter()
                .map(|glyph| ShapedGlyph {
                    glyph_id: glyph.id,
                    origin: dvec2(
                        (glyph.origin_in_lpxs.x - cluster_origin + glyph.offset_in_lpxs()) as f64
                            * paint_scale,
                        glyph.origin_in_lpxs.y as f64 * paint_scale,
                    ),
                    advance: glyph.advance_in_lpxs() as f64 * paint_scale,
                    paint_scale,
                    font: Some(glyph.font.id()),
                    font_key: span.metrics.font,
                    font_size: glyph.font_size_in_lpxs,
                    ascender: glyph.ascender_in_lpxs() as f64 * paint_scale,
                    descender: glyph.descender_in_lpxs() as f64 * paint_scale,
                    line_gap: glyph.line_gap_in_lpxs() as f64 * paint_scale,
                    baseline: row.ascender_in_lpxs as f64 * paint_scale,
                    offset: glyph.offset_in_lpxs() as f64,
                    color: glyph.color,
                })
                .collect::<Vec<_>>();
            clusters.push(ShapedCluster {
                id: GeometryElementId {
                    layout: span.id.layout,
                    cluster_ordinal: 0,
                },
                span_id: span.id,
                source_range: TextRange::new(text_size(start), text_size(end))
                    .map_err(|_| LayoutError::ShapingFailed { run: span.run_id })?,
                metrics: span.metrics,
                advance,
                bidi_level: 0,
                row_ordinal: 0,
                row_top: 0.0,
                caret_offsets: Arc::from([text_size(start), text_size(end)]),
                glyphs: glyphs.into(),
            });
            index = next;
        }
    }
    Ok(clusters)
}

fn legal_breaks(text: &str, absolute_start: usize) -> Vec<TextSize> {
    let mut breaks = text
        .char_indices()
        .filter(|(_, character)| character.is_whitespace())
        .map(|(relative, character)| text_size(absolute_start + relative + character.len_utf8()))
        .collect::<Vec<_>>();
    breaks.push(text_size(absolute_start + text.len()));
    breaks.sort_unstable();
    breaks.dedup();
    breaks
}

fn break_rows(
    request: ParagraphShapeRequest<'_>,
    clusters: &[ShapedCluster],
    legal_breaks: &[TextSize],
    trailing_newline: bool,
    line_spacing_scale: f64,
) -> Vec<ShapedRow> {
    if clusters.is_empty() {
        let text = request.source.slice(request.paragraph_range).unwrap_or("");
        let mut rows = Vec::new();
        let mut row_top = 0.0;
        for (ordinal, boundary) in text
            .char_indices()
            .filter(|(_, character)| *character == '\n')
            .map(|(relative, character)| {
                text_size(
                    request.paragraph_range.start().to_usize() + relative + character.len_utf8(),
                )
            })
            .chain(std::iter::once(request.paragraph_range.end()))
            .enumerate()
        {
            let row = empty_row(
                request,
                boundary,
                0,
                row_top,
                ordinal as u32,
                line_spacing_scale,
            );
            row_top += row.line_advance();
            rows.push(row);
        }
        return rows;
    }
    let mut rows = Vec::new();
    let mut row_start = 0;
    let mut segment_start = 0;
    let mut row_width = 0.0;
    let mut row_top = 0.0;
    for index in 0..clusters.len() {
        if index > row_start {
            let gap_start = clusters[index - 1].source_range.end();
            let gap_end = clusters[index].source_range.start();
            let newline_boundaries = TextRange::new(gap_start, gap_end)
                .ok()
                .and_then(|range| request.source.slice(range).ok())
                .map(|gap| {
                    gap.char_indices()
                        .filter(|(_, character)| *character == '\n')
                        .map(|(relative, character)| {
                            text_size(gap_start.to_usize() + relative + character.len_utf8())
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if !newline_boundaries.is_empty() {
                let row = shaped_row(
                    request.paragraph_id,
                    clusters,
                    row_start,
                    index,
                    row_top,
                    line_spacing_scale,
                );
                row_top += row.line_advance();
                rows.push(row);
                for (ordinal, boundary) in newline_boundaries.iter().skip(1).enumerate() {
                    let row = empty_row(
                        request,
                        *boundary,
                        index,
                        row_top,
                        ordinal as u32,
                        line_spacing_scale,
                    );
                    row_top += row.line_advance();
                    rows.push(row);
                }
                row_start = index;
                segment_start = index;
                row_width = 0.0;
            }
        }
        let boundary = clusters[index].source_range.end();
        if !legal_breaks.contains(&boundary) && index + 1 < clusters.len() {
            continue;
        }
        let segment_end = index + 1;
        let segment_width = clusters[segment_start..segment_end]
            .iter()
            .map(|cluster| cluster.advance)
            .sum::<f64>();
        let available = if rows.is_empty() {
            request.first_row_width
        } else {
            request.full_width
        }
        .max(1.0);
        if segment_start > row_start && row_width + segment_width > available {
            let row = shaped_row(
                request.paragraph_id,
                clusters,
                row_start,
                segment_start,
                row_top,
                line_spacing_scale,
            );
            row_top += row.line_advance();
            rows.push(row);
            row_start = segment_start;
            row_width = 0.0;
        }
        row_width += segment_width;
        let is_hard_break = request
            .source
            .slice(TextRange::new(clusters[segment_start].source_range.start(), boundary).unwrap())
            .is_ok_and(|text| text.contains('\n'));
        segment_start = segment_end;
        if is_hard_break {
            let row = shaped_row(
                request.paragraph_id,
                clusters,
                row_start,
                segment_end,
                row_top,
                line_spacing_scale,
            );
            row_top += row.line_advance();
            rows.push(row);
            row_start = segment_end;
            row_width = 0.0;
        }
    }
    if row_start < clusters.len() || rows.is_empty() {
        let row = shaped_row(
            request.paragraph_id,
            clusters,
            row_start,
            clusters.len(),
            row_top,
            line_spacing_scale,
        );
        row_top += row.line_advance();
        rows.push(row);
    }
    if trailing_newline {
        let boundary = request.paragraph_range.end();
        rows.push(empty_row(
            request,
            boundary,
            clusters.len(),
            row_top,
            1,
            line_spacing_scale,
        ));
    }
    rows
}

fn empty_row(
    request: ParagraphShapeRequest<'_>,
    boundary: TextSize,
    cluster_index: usize,
    row_top: f64,
    empty_ordinal: u32,
    line_spacing_scale: f64,
) -> ShapedRow {
    let metrics = request
        .spans
        .iter()
        .find(|span| span.source_range.start() <= boundary && boundary <= span.source_range.end())
        .or_else(|| request.spans.last())
        .map(|span| span.metrics);
    let ascender = metrics.map_or(12.8, |value| value.font_size as f64 * 0.8);
    let descender = metrics.map_or(3.2, |value| value.font_size as f64 * 0.2);
    ShapedRow {
        id: stable_row_id(request.paragraph_id, boundary, boundary, empty_ordinal),
        source_range: TextRange::new(boundary, boundary).expect("empty row range is valid"),
        cluster_range: cluster_index..cluster_index,
        caret_offsets: Arc::from([boundary]),
        ascender,
        descender,
        line_gap: 0.0,
        line_spacing_scale,
        row_top,
    }
}

fn shaped_row(
    paragraph_id: GeometryElementId,
    clusters: &[ShapedCluster],
    start: usize,
    end: usize,
    row_top: f64,
    line_spacing_scale: f64,
) -> ShapedRow {
    let source_start = clusters
        .get(start)
        .map_or_else(|| text_size(0), |cluster| cluster.source_range.start());
    let source_end = clusters
        .get(end.saturating_sub(1))
        .map_or(source_start, |cluster| cluster.source_range.end());
    let ascender = clusters[start..end]
        .iter()
        .flat_map(|cluster| cluster.glyphs.iter().map(|glyph| glyph.ascender))
        .fold(0.0, f64::max);
    let descender = clusters[start..end]
        .iter()
        .flat_map(|cluster| cluster.glyphs.iter().map(|glyph| glyph.descender.abs()))
        .fold(0.0, f64::max);
    let line_gap = clusters[start..end]
        .iter()
        .flat_map(|cluster| cluster.glyphs.iter().map(|glyph| glyph.line_gap))
        .fold(0.0, f64::max);
    ShapedRow {
        id: stable_row_id(paragraph_id, source_start, source_end, 0),
        source_range: TextRange::new(source_start, source_end).expect("row range is ordered"),
        cluster_range: start..end,
        caret_offsets: Arc::from([source_start, source_end]),
        ascender: ascender.max(1.0),
        descender,
        line_gap,
        line_spacing_scale,
        row_top,
    }
}

fn stable_row_id(
    paragraph_id: GeometryElementId,
    start: TextSize,
    end: TextSize,
    empty_ordinal: u32,
) -> GeometryElementId {
    let mut value = start.to_usize() as u64;
    value = value
        .wrapping_mul(0x9e37_79b9)
        .wrapping_add(paragraph_id.cluster_ordinal as u64);
    value = value
        .wrapping_mul(0x9e37_79b9)
        .wrapping_add(end.to_usize() as u64);
    value = value
        .wrapping_mul(0x9e37_79b9)
        .wrapping_add(empty_ordinal as u64);
    GeometryElementId {
        layout: paragraph_id.layout,
        cluster_ordinal: 0xc000_0000 | (value as u32 & 0x0fff_ffff),
    }
}

fn paragraph_error(spans: &[ShapeSpan], paragraph_id: GeometryElementId) -> LayoutError {
    LayoutError::ShapingFailed {
        run: spans
            .first()
            .map_or(paragraph_id.layout, |span| span.run_id),
    }
}

fn text_size(value: usize) -> TextSize {
    TextSize::try_from_usize(value).expect("Makepad cluster offsets fit the source")
}

fn bidi_level_at(bidi: &BidiInfo<'_>, byte_offset: usize) -> u8 {
    bidi.levels
        .get(byte_offset)
        .map_or(0, unicode_bidi::Level::number)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use makepad_widgets::*;
    use unicode_bidi::BidiInfo;
    use waml_syntax::{parse_markdown, DocumentRevision, MarkdownDialect, SourceText};

    use super::{
        bidi_level_at, reduce_intrinsic_advances, text_size, FontResolver, MakepadTextShaper,
        TextRange,
    };
    use crate::{
        document::MarkdownDocumentSnapshot,
        layout::{
            Affinity, BaseDirection, BlockFlow, BlockLayoutSpec, EdgeInsets, FontKey, FontWeight,
            GeometryElementId, LayoutBlock, LayoutDocument, LayoutElementId, LayoutEngine,
            LayoutError, LayoutInvalidation, LayoutTextRun, LayoutViewport, ParagraphIntrinsic,
            ParagraphIntrinsicRequest, ParagraphShapeRequest, ShapeSpan, ShapedParagraph,
            TextMetrics, TextShaper,
        },
        selection::TextPosition,
    };

    fn shape_run<R: FontResolver>(
        shaper: &mut MakepadTextShaper<'_, R>,
        source: &SourceText,
        run: &LayoutTextRun,
        full_width: f64,
        first_row_width: f64,
    ) -> ShapedParagraph {
        let paragraph_id = GeometryElementId {
            layout: run.id,
            cluster_ordinal: 0,
        };
        let spans = [ShapeSpan {
            id: GeometryElementId {
                layout: run.id,
                cluster_ordinal: 0x4000_0000,
            },
            run_id: run.id,
            stable_ordinal: 0,
            source_range: run.range,
            metrics: run.metrics,
        }];
        shaper
            .shape_paragraph(ParagraphShapeRequest {
                source,
                paragraph_id,
                paragraph_range: run.range,
                spans: &spans,
                full_width,
                first_row_width,
                base_direction: BaseDirection::Auto,
            })
            .unwrap()
    }

    #[test]
    fn adapter_uses_unicode_embedding_levels_for_rtl_text() {
        let text = "a א";
        let bidi = BidiInfo::new(text, None);
        assert_eq!(bidi_level_at(&bidi, 0), 0);
        assert_eq!(bidi_level_at(&bidi, 2), 1);
    }

    #[test]
    fn makepad_shaper_and_engine_do_not_reorder_rtl_twice() {
        let source = SourceText::new("# אב".to_owned()).unwrap();
        let syntax = parse_markdown(
            DocumentRevision::new(8),
            source,
            MarkdownDialect::WAML_DEFAULT,
        )
        .unwrap();
        let heading = syntax.queries().headings().next().unwrap().clone();
        let id = LayoutElementId {
            owner: heading.owner,
            fragment_ordinal: 0,
        };
        let presentation = MarkdownDocumentSnapshot::new(syntax);
        let document = LayoutDocument {
            revision: presentation.revision(),
            content_insets: EdgeInsets::default(),
            blocks: Arc::from([LayoutBlock {
                id,
                source_range: heading.range,
                parent: None,
                spec: BlockLayoutSpec {
                    flow: BlockFlow::Paragraph,
                    insets: EdgeInsets::default(),
                    space_before: 0.0,
                    space_after: 0.0,
                    columns: Arc::from([]),
                },
            }]),
            text_runs: Arc::from([LayoutTextRun {
                id,
                range: heading.content_range,
                metrics: TextMetrics {
                    font: FontKey(0),
                    font_size: 16.0,
                    line_spacing: 1.0,
                    weight: FontWeight(400),
                    italic: false,
                },
            }]),
            embedded_blocks: Arc::from([]),
        };
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let mut geometry = None;
        cx.with_vm(|vm| {
            makepad_widgets::makepad_draw::script_mod(vm);
            makepad_widgets::script_mod(vm);
            let mut draw_text = Label::script_new_with_default(vm).draw_text;
            vm.with_cx_mut(|cx| {
                let mut fonts = NoopFonts;
                let mut shaper = MakepadTextShaper {
                    cx,
                    draw_text: &mut draw_text,
                    fonts: &mut fonts,
                    revision: DocumentRevision::INITIAL,
                    cache: None,
                };
                geometry = Some(
                    LayoutEngine::default()
                        .layout(
                            &document,
                            &presentation,
                            LayoutViewport::new(400.0, 100.0, 0.0, 0.0),
                            LayoutInvalidation::Document,
                            &mut shaper,
                        )
                        .unwrap(),
                );
            });
        });
        let geometry = geometry.unwrap();
        let start = geometry
            .source_to_point(TextPosition::new(
                heading.content_range.start(),
                Affinity::Before,
            ))
            .unwrap()
            .rect
            .pos
            .x;
        let end = geometry
            .source_to_point(TextPosition::new(
                heading.content_range.end(),
                Affinity::After,
            ))
            .unwrap()
            .rect
            .pos
            .x;
        assert!(start > end, "RTL source start must be right of source end");
    }

    #[test]
    fn makepad_shaper_retains_exact_ligature_combining_and_arabic_glyphs() {
        for sample in ["ffi", "e\u{301}", "سلام"] {
            let source = SourceText::new(format!("# {sample}")).unwrap();
            let syntax = parse_markdown(
                DocumentRevision::new(11),
                source,
                MarkdownDialect::WAML_DEFAULT,
            )
            .unwrap();
            let heading = syntax.queries().headings().next().unwrap().clone();
            let id = LayoutElementId {
                owner: heading.owner,
                fragment_ordinal: 0,
            };
            let run = LayoutTextRun {
                id,
                range: heading.content_range,
                metrics: TextMetrics {
                    font: FontKey(91),
                    font_size: 19.0,
                    line_spacing: 1.0,
                    weight: FontWeight(400),
                    italic: false,
                },
            };
            let text = syntax.text().slice(run.range).unwrap().to_owned();
            let mut cx = Cx::new(Box::new(|_, _| {}));
            cx.with_vm(|vm| {
                makepad_widgets::makepad_draw::script_mod(vm);
                makepad_widgets::script_mod(vm);
                let mut draw_text = Label::script_new_with_default(vm).draw_text;
                vm.with_cx_mut(|cx| {
                    let raw =
                        draw_text.layout(cx, 0.0, 0.0, Some(400.0), true, Align::default(), &text);
                    let mut fonts = NoopFonts;
                    let mut shaper = MakepadTextShaper {
                        cx,
                        draw_text: &mut draw_text,
                        fonts: &mut fonts,
                        revision: DocumentRevision::INITIAL,
                        cache: None,
                    };
                    let shaped = shape_run(&mut shaper, syntax.text(), &run, 400.0, 400.0);
                    let shaped_glyphs = shaped
                        .clusters
                        .iter()
                        .flat_map(|cluster| cluster.glyphs.iter())
                        .collect::<Vec<_>>();
                    let mut expected = Vec::new();
                    for row in &raw.rows {
                        let mut index = 0;
                        while index < row.glyphs.len() {
                            let cluster = row.glyphs[index].cluster;
                            let cluster_origin = row.glyphs[index].origin_in_lpxs.x;
                            let mut next = index + 1;
                            while next < row.glyphs.len() && row.glyphs[next].cluster == cluster {
                                next += 1;
                            }
                            for (ordinal, glyph) in row.glyphs[index..next].iter().enumerate() {
                                expected.push((
                                    row.text.start_in_parent() + cluster,
                                    ordinal,
                                    glyph,
                                    dvec2(
                                        (glyph.origin_in_lpxs.x - cluster_origin
                                            + glyph.offset_in_lpxs())
                                            as f64
                                            * draw_text.font_scale as f64,
                                        glyph.origin_in_lpxs.y as f64 * draw_text.font_scale as f64,
                                    ),
                                    row.ascender_in_lpxs as f64 * draw_text.font_scale as f64,
                                ));
                            }
                            index = next;
                        }
                    }
                    expected.sort_by_key(|(cluster, ordinal, _, _, _)| (*cluster, *ordinal));
                    assert_eq!(shaped_glyphs.len(), expected.len(), "sample {sample:?}");
                    assert!(!shaped_glyphs.is_empty(), "sample {sample:?}");
                    for (glyph, (_, _, raw, origin, baseline)) in
                        shaped_glyphs.into_iter().zip(expected)
                    {
                        assert!(glyph.font.is_some(), "sample {sample:?}");
                        assert_eq!(glyph.font, Some(raw.font.id()));
                        assert_eq!(glyph.font_key, FontKey(91));
                        assert_eq!(glyph.glyph_id, raw.id);
                        assert_eq!(glyph.origin, origin);
                        assert_eq!(glyph.advance, raw.advance_in_lpxs() as f64);
                        assert_eq!(glyph.paint_scale, draw_text.font_scale as f64);
                        assert_eq!(glyph.font_size, raw.font_size_in_lpxs);
                        assert_eq!(glyph.ascender, raw.ascender_in_lpxs() as f64);
                        assert_eq!(glyph.descender, raw.descender_in_lpxs() as f64);
                        assert_eq!(glyph.line_gap, raw.line_gap_in_lpxs() as f64);
                        assert_eq!(glyph.baseline, baseline);
                        assert_eq!(glyph.offset, raw.offset_in_lpxs() as f64);
                        assert_eq!(glyph.color, raw.color);
                    }
                });
            });
        }
    }

    #[test]
    fn paragraph_shape_contract_honors_first_row_width() {
        let source = SourceText::new("# alpha beta gamma delta epsilon".to_owned()).unwrap();
        let syntax = parse_markdown(
            DocumentRevision::new(12),
            source,
            MarkdownDialect::WAML_DEFAULT,
        )
        .unwrap();
        let heading = syntax.queries().headings().next().unwrap().clone();
        let run = LayoutTextRun {
            id: LayoutElementId {
                owner: heading.owner,
                fragment_ordinal: 0,
            },
            range: heading.content_range,
            metrics: TextMetrics {
                font: FontKey(92),
                font_size: 19.0,
                line_spacing: 1.0,
                weight: FontWeight(400),
                italic: false,
            },
        };
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.with_vm(|vm| {
            makepad_widgets::makepad_draw::script_mod(vm);
            makepad_widgets::script_mod(vm);
            let mut draw_text = Label::script_new_with_default(vm).draw_text;
            vm.with_cx_mut(|cx| {
                let mut fonts = NoopFonts;
                let mut shaper = MakepadTextShaper {
                    cx,
                    draw_text: &mut draw_text,
                    fonts: &mut fonts,
                    revision: DocumentRevision::INITIAL,
                    cache: None,
                };
                let full = shape_run(&mut shaper, syntax.text(), &run, 180.0, 180.0);
                let inline = shape_run(&mut shaper, syntax.text(), &run, 180.0, 40.0);
                let first_row_width = |shaped: &ShapedParagraph| {
                    let row = &shaped.rows[0];
                    shaped.clusters[row.cluster_range.clone()]
                        .iter()
                        .map(|cluster| cluster.advance)
                        .sum::<f64>()
                };
                assert!(first_row_width(&inline) < first_row_width(&full));
                assert!(inline.rows.len() > 1);
            });
        });
    }

    #[test]
    fn intrinsic_gap_newline_splits_max_content() {
        let source = SourceText::new("wide\nnarrow").unwrap();
        let paragraph_range = TextRange::new(text_size(0), source.len()).unwrap();
        let advances = [
            (TextRange::new(text_size(0), text_size(4)).unwrap(), 40.0),
            (TextRange::new(text_size(5), source.len()).unwrap(), 25.0),
        ];

        let intrinsic = reduce_intrinsic_advances(&source, paragraph_range, &advances);

        assert_eq!(intrinsic.min_content, 40.0);
        assert_eq!(intrinsic.max_content, 40.0);
    }

    #[test]
    fn makepad_ten_thousand_intrinsics_do_not_retain_laidout_text() {
        let source = SourceText::new("# x".to_owned()).unwrap();
        let syntax = parse_markdown(
            DocumentRevision::new(13),
            source,
            MarkdownDialect::WAML_DEFAULT,
        )
        .unwrap();
        let heading = syntax.queries().headings().next().unwrap().clone();
        let run = LayoutTextRun {
            id: LayoutElementId {
                owner: heading.owner,
                fragment_ordinal: 0,
            },
            range: heading.content_range,
            metrics: TextMetrics {
                font: FontKey(93),
                font_size: 16.0,
                line_spacing: 1.0,
                weight: FontWeight(400),
                italic: false,
            },
        };
        let paragraph_id = GeometryElementId {
            layout: run.id,
            cluster_ordinal: 0,
        };
        let spans = [ShapeSpan {
            id: GeometryElementId {
                layout: run.id,
                cluster_ordinal: 0x4000_0000,
            },
            run_id: run.id,
            stable_ordinal: 0,
            source_range: run.range,
            metrics: run.metrics,
        }];
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.with_vm(|vm| {
            makepad_widgets::makepad_draw::script_mod(vm);
            makepad_widgets::script_mod(vm);
            let mut draw_text = Label::script_new_with_default(vm).draw_text;
            vm.with_cx_mut(|cx| {
                let before = draw_text.layout_cache_stats(cx);
                let mut fonts = NoopFonts;
                {
                    let mut shaper = MakepadTextShaper {
                        cx,
                        draw_text: &mut draw_text,
                        fonts: &mut fonts,
                        revision: DocumentRevision::INITIAL,
                        cache: None,
                    };
                    for _ in 0..10_000 {
                        let intrinsic = shaper
                            .measure_paragraph_intrinsic(ParagraphIntrinsicRequest {
                                source: syntax.text(),
                                paragraph_id,
                                paragraph_range: run.range,
                                spans: &spans,
                                base_direction: BaseDirection::Auto,
                            })
                            .unwrap();
                        assert!(intrinsic.max_content > 0.0);
                    }
                }
                let after = draw_text.layout_cache_stats(cx);
                assert_eq!(after, before);
            });
        });
    }

    #[test]
    fn makepad_shaper_matches_scaled_multi_row_paint_origins() {
        let source = SourceText::new("# alpha beta gamma delta epsilon".to_owned()).unwrap();
        let syntax = parse_markdown(
            DocumentRevision::new(12),
            source,
            MarkdownDialect::WAML_DEFAULT,
        )
        .unwrap();
        let heading = syntax.queries().headings().next().unwrap().clone();
        let id = LayoutElementId {
            owner: heading.owner,
            fragment_ordinal: 0,
        };
        let run = LayoutTextRun {
            id,
            range: heading.content_range,
            metrics: TextMetrics {
                font: FontKey(92),
                font_size: 19.0,
                line_spacing: 1.0,
                weight: FontWeight(400),
                italic: false,
            },
        };
        let text = syntax.text().slice(run.range).unwrap().to_owned();
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.with_vm(|vm| {
            makepad_widgets::makepad_draw::script_mod(vm);
            makepad_widgets::script_mod(vm);
            let mut draw_text = Label::script_new_with_default(vm).draw_text;
            draw_text.font_scale = 1.5;
            vm.with_cx_mut(|cx| {
                let raw = draw_text.layout(cx, 0.0, 0.0, Some(60.0), true, Align::default(), &text);
                assert!(raw.rows.len() > 1);
                let expected = raw
                    .rows
                    .iter()
                    .flat_map(|row| {
                        row.glyphs.iter().map(|glyph| {
                            dvec2(
                                ((row.origin_in_lpxs.x
                                    + glyph.origin_in_lpxs.x
                                    + glyph.offset_in_lpxs())
                                    * draw_text.font_scale) as f64,
                                ((row.origin_in_lpxs.y + glyph.origin_in_lpxs.y)
                                    * draw_text.font_scale) as f64,
                            )
                        })
                    })
                    .collect::<Vec<_>>();
                let mut fonts = NoopFonts;
                let mut shaper = MakepadTextShaper {
                    cx,
                    draw_text: &mut draw_text,
                    fonts: &mut fonts,
                    revision: DocumentRevision::INITIAL,
                    cache: None,
                };
                let shaped = shape_run(&mut shaper, syntax.text(), &run, 90.0, 90.0);
                assert!(shaped
                    .clusters
                    .iter()
                    .flat_map(|cluster| cluster.glyphs.iter())
                    .all(|glyph| glyph.paint_scale == 1.5));

                struct ReplayShaper(ShapedParagraph);
                impl TextShaper for ReplayShaper {
                    fn shape_paragraph(
                        &mut self,
                        _request: ParagraphShapeRequest<'_>,
                    ) -> Result<ShapedParagraph, LayoutError> {
                        Ok(self.0.clone())
                    }

                    fn measure_paragraph_intrinsic(
                        &mut self,
                        _request: ParagraphIntrinsicRequest<'_>,
                    ) -> Result<ParagraphIntrinsic, LayoutError> {
                        Ok(ParagraphIntrinsic::default())
                    }
                }

                let document = LayoutDocument {
                    revision: syntax.revision(),
                    content_insets: EdgeInsets::default(),
                    blocks: Arc::from([LayoutBlock {
                        id,
                        source_range: heading.range,
                        parent: None,
                        spec: BlockLayoutSpec {
                            flow: BlockFlow::Paragraph,
                            insets: EdgeInsets::default(),
                            space_before: 0.0,
                            space_after: 0.0,
                            columns: Arc::from([]),
                        },
                    }]),
                    text_runs: Arc::from([run]),
                    embedded_blocks: Arc::from([]),
                };
                let presentation = MarkdownDocumentSnapshot::new(syntax.clone());
                let mut replay = ReplayShaper(shaped);
                let layout = LayoutEngine::default()
                    .layout(
                        &document,
                        &presentation,
                        LayoutViewport::new(90.0, 300.0, 0.0, 0.0),
                        LayoutInvalidation::Document,
                        &mut replay,
                    )
                    .unwrap();
                let actual = layout
                    .glyph_clusters()
                    .iter()
                    .flat_map(|cluster| cluster.glyphs.iter().map(|glyph| glyph.origin))
                    .collect::<Vec<_>>();
                assert_eq!(actual.len(), expected.len());
                for (actual, expected) in actual.into_iter().zip(expected) {
                    assert!(
                        (actual.x - expected.x).abs() < 0.001,
                        "x: {actual:?} != {expected:?}"
                    );
                    assert!(
                        (actual.y - expected.y).abs() < 0.001,
                        "y: {actual:?} != {expected:?}"
                    );
                }
            });
        });
    }

    struct NoopFonts;

    impl FontResolver for NoopFonts {
        fn configure_draw_text(
            &mut self,
            _key: FontKey,
            _metrics: TextMetrics,
            _draw: &mut DrawText,
        ) {
        }
    }
}
