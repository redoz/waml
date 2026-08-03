//! Line-number gutter.
//!
//! Numbers count source lines, not visual rows: a wrapped line is numbered
//! once, at its first row, and its continuation rows stay blank. That is what
//! keyboard motions address, so a relative count here is the same count a
//! motion will take.

use waml_syntax::{LineIndex, SourceText, TextSize};

use crate::layout::{LayoutSnapshot, VisualLane, VisualRow};

/// The band a row's text actually occupies: the union of its lane rects.
///
/// A row rect also spans the owning block's margins, so a heading's row is far
/// taller than its glyphs. Numbering and highlighting both follow the glyphs.
fn text_band(lanes: &[VisualLane], row: &VisualRow) -> Option<(f64, f64)> {
    let lanes = lanes.get(row.lanes.clone())?;
    let top = lanes
        .iter()
        .map(|lane| lane.rect.pos.y)
        .fold(f64::INFINITY, f64::min);
    let bottom = lanes
        .iter()
        .map(|lane| lane.rect.pos.y + lane.rect.size.y)
        .fold(f64::NEG_INFINITY, f64::max);
    (top.is_finite() && bottom.is_finite()).then_some((top, bottom - top))
}

/// Ratio of ascent to font size the shaper assumes for a row of its own.
const ASCENT: f64 = 0.8;

/// Baseline of a row's largest face, in layout coordinates.
///
/// Derived from the row's metrics, not from shaped glyph positions:
/// `ShapedGlyph::baseline` is in paint-scale units, so mixing it with a layout
/// rect drifts a whole row per line.
fn text_baseline(layout: &LayoutSnapshot, row: &VisualRow, top: f64, height: f64) -> f64 {
    let size = layout
        .glyph_clusters()
        .iter()
        .filter(|cluster| row.lanes.contains(&cluster.lane_index))
        .map(|cluster| cluster.metrics.font_size as f64)
        .fold(0.0, f64::max);
    if size > 0.0 {
        top + size * ASCENT
    } else {
        top + height * ASCENT
    }
}

/// Whether a row carries text of its own rather than being an empty placement
/// row. Several rows can start at one source offset — a heading's block emits a
/// zero-width row before the row holding its glyphs — and only the one with
/// text should carry the line's number.
fn row_has_text(lanes: &[VisualLane], row: &VisualRow) -> bool {
    lanes.get(row.lanes.clone()).is_some_and(|lanes| {
        lanes
            .iter()
            .any(|lane| lane.source_range.start() < lane.source_range.end())
    })
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LineNumberMode {
    /// No gutter at all.
    #[default]
    Off,
    /// Every line shows its own number.
    Absolute,
    /// The cursor's line shows its absolute number; every other line shows its
    /// distance from it.
    Relative,
}

/// One numbered row of the gutter, in layout coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct GutterRow {
    /// One-based source line.
    pub line: usize,
    /// The rendered digits for the active mode.
    pub label: String,
    /// Top of the visual row that starts this line.
    pub y: f64,
    pub height: f64,
    /// Baseline of that row's text, so a digit in a smaller face can sit on the
    /// same baseline as the line it numbers.
    pub baseline: f64,
    /// Whether this row holds glyphs rather than being an empty placement row.
    pub has_text: bool,
    /// Whether the cursor sits on this line.
    pub current: bool,
}

/// The numbered rows of one layout, ordered top to bottom.
///
/// Returns nothing when the mode is `Off`, so a caller can reserve no width
/// without a second branch.
pub fn gutter_rows(
    layout: &LayoutSnapshot,
    text: &SourceText,
    lines: &LineIndex,
    cursor: TextSize,
    mode: LineNumberMode,
) -> Vec<GutterRow> {
    if mode == LineNumberMode::Off {
        return Vec::new();
    }
    let line_of = |offset: TextSize| lines.line_col(text, offset).ok().map(|at| at.line as usize);
    let Some(cursor_line) = line_of(cursor) else {
        return Vec::new();
    };
    let lanes = layout.visual_lanes();
    let mut rows = Vec::new();
    let mut seen: Option<usize> = None;
    for row in layout.visual_rows() {
        let Some(start) = lanes
            .get(row.lanes.clone())
            .and_then(|lanes| lanes.iter().map(|lane| lane.source_range.start()).min())
        else {
            continue;
        };
        let Some(line) = line_of(start) else {
            continue;
        };
        let has_text = row_has_text(lanes, row);
        // A wrapped line owns several rows; only its first row is numbered. A
        // line whose first row is an empty placement row is renumbered onto the
        // row that actually holds its glyphs, which is where the reader looks.
        let repeat = seen == Some(line);
        if repeat && (!has_text || rows.last().is_some_and(|last: &GutterRow| last.has_text)) {
            continue;
        }
        seen = Some(line);
        let Some((y, height)) = text_band(lanes, row) else {
            continue;
        };
        if repeat {
            rows.pop();
        }
        let current = line == cursor_line;
        let label = match mode {
            LineNumberMode::Off => unreachable!("returned above"),
            LineNumberMode::Absolute => (line + 1).to_string(),
            LineNumberMode::Relative if current => (line + 1).to_string(),
            LineNumberMode::Relative => line.abs_diff(cursor_line).to_string(),
        };
        rows.push(GutterRow {
            line: line + 1,
            label,
            y,
            height,
            baseline: text_baseline(layout, row, y, height),
            has_text,
            current,
        });
    }
    rows
}

/// Vertical `(y, height)` bands of the visual rows the cursor's source line
/// owns, ordered top to bottom.
///
/// A wrapped line owns several rows, so the highlight covers all of them; a
/// line with no row at all yields nothing.
pub fn current_line_bands(
    layout: &LayoutSnapshot,
    text: &SourceText,
    lines: &LineIndex,
    cursor: TextSize,
) -> Vec<(f64, f64)> {
    let line_of = |offset: TextSize| lines.line_col(text, offset).ok().map(|at| at.line as usize);
    let Some(cursor_line) = line_of(cursor) else {
        return Vec::new();
    };
    let lanes = layout.visual_lanes();
    layout
        .visual_rows()
        .iter()
        .filter(|row| {
            lanes
                .get(row.lanes.clone())
                .and_then(|lanes| lanes.iter().map(|lane| lane.source_range.start()).min())
                .and_then(line_of)
                == Some(cursor_line)
        })
        .filter_map(|row| text_band(lanes, row).map(|band| (row_has_text(lanes, row), band)))
        .collect::<Vec<_>>()
        .into_iter()
        // Empty placement rows only count when the line has nothing else, so a
        // heading is not highlighted a row taller than it looks.
        .fold(
            (false, Vec::new()),
            |(any_text, mut kept), (has_text, band)| {
                if has_text && !any_text {
                    kept.clear();
                }
                if has_text || !any_text {
                    kept.push(band);
                }
                (any_text || has_text, kept)
            },
        )
        .1
}

/// Width reserved for the gutter, including the gap to the text.
///
/// The width follows the widest label the document can produce rather than the
/// widest one on screen, so scrolling never shifts the text sideways.
///
/// Four digits are always reserved so documents up to 9999 lines keep a stable
/// gutter and the text never shifts as the line count grows.
pub fn gutter_width(line_count: usize, digit_width: f64, gap: f64) -> f64 {
    let digits = line_count.max(1).to_string().len().max(4) as f64;
    digits * digit_width + gap
}
