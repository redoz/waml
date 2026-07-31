use std::sync::Arc;

use makepad_widgets::{Align, Cx, DrawText};
use unicode_bidi::BidiInfo;
use waml_syntax::{SourceText, TextRange, TextSize};

use super::{
    FontKey, LayoutError, LayoutTextRun, ShapedCluster, ShapedRun, TextMetrics, TextShaper,
};

pub trait FontResolver {
    fn configure_draw_text(&mut self, key: FontKey, metrics: TextMetrics, draw: &mut DrawText);
}

pub struct MakepadTextShaper<'a, R> {
    pub cx: &'a mut Cx,
    pub draw_text: &'a mut DrawText,
    pub fonts: &'a mut R,
}

impl<R: FontResolver> TextShaper for MakepadTextShaper<'_, R> {
    fn shape(
        &mut self,
        source: &SourceText,
        run: &LayoutTextRun,
        max_width: f64,
    ) -> Result<ShapedRun, LayoutError> {
        self.fonts
            .configure_draw_text(run.metrics.font, run.metrics, self.draw_text);
        let text = source
            .slice(run.range)
            .map_err(|_| LayoutError::ShapingFailed { run: run.id })?;
        let bidi = BidiInfo::new(text, None);
        let laid_out = self.draw_text.layout(
            self.cx,
            0.0,
            0.0,
            Some(max_width as f32),
            true,
            Align::default(),
            text,
        );
        let mut clusters = Vec::new();
        let mut ascender = 0.0_f64;
        let mut descender = 0.0_f64;
        let mut line_gap = 0.0_f64;
        for row in &laid_out.rows {
            ascender = ascender.max(row.ascender_in_lpxs as f64);
            descender = descender.min(row.descender_in_lpxs as f64);
            line_gap = line_gap.max(row.line_gap_in_lpxs as f64);
            let mut logical_clusters: Vec<_> =
                row.glyphs.iter().map(|glyph| glyph.cluster).collect();
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
                let start = run.range.start().to_usize() + row_start + cluster;
                let end = run.range.start().to_usize() + row_start + next_cluster;
                let advance = row.glyphs.get(next).map_or_else(
                    || row.width_in_lpxs - row.glyphs[index].origin_in_lpxs.x,
                    |glyph| glyph.origin_in_lpxs.x - row.glyphs[index].origin_in_lpxs.x,
                );
                clusters.push(ShapedCluster {
                    source_range: TextRange::new(text_size(start), text_size(end))
                        .map_err(|_| LayoutError::ShapingFailed { run: run.id })?,
                    advance: advance as f64,
                    bidi_level: bidi_level_at(&bidi, row_start + cluster),
                    caret_offsets: Arc::from([text_size(start), text_size(end)]),
                });
                index = next;
            }
        }
        clusters.sort_by_key(|cluster| cluster.source_range.start());
        Ok(ShapedRun {
            clusters: clusters.into(),
            ascender,
            descender,
            line_gap,
        })
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

    use super::{bidi_level_at, FontResolver, MakepadTextShaper};
    use crate::{
        document::MarkdownDocumentSnapshot,
        layout::{
            Affinity, BlockFlow, BlockLayoutSpec, EdgeInsets, FontKey, FontWeight, LayoutBlock,
            LayoutDocument, LayoutElementId, LayoutEngine, LayoutInvalidation, LayoutTextRun,
            LayoutViewport, TextMetrics,
        },
        selection::TextPosition,
    };

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
