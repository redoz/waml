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
    use super::bidi_level_at;
    use unicode_bidi::BidiInfo;

    #[test]
    fn adapter_uses_unicode_embedding_levels_for_rtl_text() {
        let text = "a א";
        let bidi = BidiInfo::new(text, None);
        assert_eq!(bidi_level_at(&bidi, 0), 0);
        assert_eq!(bidi_level_at(&bidi, 2), 1);
    }
}
