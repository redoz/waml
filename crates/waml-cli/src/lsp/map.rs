//! Pure Diagnostic→LSP mapping, byte→UTF-16 conversion, and the WAML filter.
//! This is the only place byte offsets become UTF-16 code units.

use std::{fmt, ops::Range};

use tower_lsp::lsp_types as lsp;
use waml::{
    analysis::DocumentVersion,
    diagnostic::{Diagnostic, Severity},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub enum PositionError {
    ByteOffsetOutOfBounds { offset: usize, len: usize },
    ByteOffsetNotCharBoundary { offset: usize },
    ByteOffsetInLineEnding { offset: usize },
    LineOutOfBounds { line: u32 },
    Utf16ColumnOutOfBounds { line: u32, character: u32 },
    Utf16ColumnNotCharBoundary { line: u32, character: u32 },
    ReversedByteRange { start: usize, end: usize },
}

impl fmt::Display for PositionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ByteOffsetOutOfBounds { offset, len } => {
                write!(f, "byte offset {offset} exceeds document length {len}")
            }
            Self::ByteOffsetNotCharBoundary { offset } => {
                write!(f, "byte offset {offset} is not a UTF-8 boundary")
            }
            Self::ByteOffsetInLineEnding { offset } => {
                write!(f, "byte offset {offset} is inside a line ending")
            }
            Self::LineOutOfBounds { line } => write!(f, "line {line} does not exist"),
            Self::Utf16ColumnOutOfBounds { line, character } => {
                write!(f, "UTF-16 column {character} exceeds line {line}")
            }
            Self::Utf16ColumnNotCharBoundary { line, character } => write!(
                f,
                "UTF-16 column {character} on line {line} splits a character"
            ),
            Self::ReversedByteRange { start, end } => {
                write!(f, "byte range {start}..{end} is reversed")
            }
        }
    }
}

impl std::error::Error for PositionError {}

#[allow(dead_code)]
fn line_bounds(text: &str, target_line: u32) -> Option<Range<usize>> {
    let bytes = text.as_bytes();
    let mut line = 0_u32;
    let mut line_start = 0;
    let mut offset = 0;
    while offset < bytes.len() {
        let line_ending_len = match bytes[offset] {
            b'\r' if bytes.get(offset + 1) == Some(&b'\n') => 2,
            b'\n' => 1,
            _ => {
                offset += 1;
                continue;
            }
        };
        if line == target_line {
            return Some(line_start..offset);
        }
        line = line.checked_add(1)?;
        offset += line_ending_len;
        line_start = offset;
    }
    (line == target_line).then_some(line_start..text.len())
}

pub fn to_lsp_position(
    offset: usize,
    document: &DocumentVersion,
) -> Result<lsp::Position, PositionError> {
    let text = document.text().shared().as_str();
    if offset > text.len() {
        return Err(PositionError::ByteOffsetOutOfBounds {
            offset,
            len: text.len(),
        });
    }
    if !text.is_char_boundary(offset) {
        return Err(PositionError::ByteOffsetNotCharBoundary { offset });
    }
    if offset > 0
        && text.as_bytes()[offset - 1] == b'\r'
        && text.as_bytes().get(offset) == Some(&b'\n')
    {
        return Err(PositionError::ByteOffsetInLineEnding { offset });
    }

    let text_offset = offset
        .try_into()
        .map_err(|_| PositionError::ByteOffsetOutOfBounds {
            offset,
            len: text.len(),
        })?;
    let line_column = document
        .line_index()
        .line_col(document.text(), text_offset)
        .map_err(|_| PositionError::ByteOffsetNotCharBoundary { offset })?;
    let character = document
        .line_index()
        .utf16_column(document.text(), text_offset)
        .map_err(|_| PositionError::ByteOffsetNotCharBoundary { offset })?;
    Ok(lsp::Position::new(line_column.line, character))
}

#[allow(dead_code)]
pub fn from_lsp_position(
    position: lsp::Position,
    document: &DocumentVersion,
) -> Result<usize, PositionError> {
    let text = document.text().shared().as_str();
    let bounds = line_bounds(text, position.line).ok_or(PositionError::LineOutOfBounds {
        line: position.line,
    })?;
    let line_text = &text[bounds.clone()];
    let mut utf16_column = 0_u32;
    for (byte_column, character) in line_text.char_indices() {
        if position.character == utf16_column {
            return Ok(bounds.start + byte_column);
        }
        let next_column = utf16_column + character.len_utf16() as u32;
        if position.character < next_column {
            return Err(PositionError::Utf16ColumnNotCharBoundary {
                line: position.line,
                character: position.character,
            });
        }
        utf16_column = next_column;
    }
    if position.character == utf16_column {
        Ok(bounds.end)
    } else {
        Err(PositionError::Utf16ColumnOutOfBounds {
            line: position.line,
            character: position.character,
        })
    }
}

pub fn to_lsp_range(
    range: Range<usize>,
    document: &DocumentVersion,
) -> Result<lsp::Range, PositionError> {
    if range.start > range.end {
        return Err(PositionError::ReversedByteRange {
            start: range.start,
            end: range.end,
        });
    }
    Ok(lsp::Range::new(
        to_lsp_position(range.start, document)?,
        to_lsp_position(range.end, document)?,
    ))
}

#[allow(dead_code)]
pub fn from_lsp_range(
    range: lsp::Range,
    document: &DocumentVersion,
) -> Result<Range<usize>, PositionError> {
    let start = from_lsp_position(range.start, document)?;
    let end = from_lsp_position(range.end, document)?;
    if start > end {
        return Err(PositionError::ReversedByteRange { start, end });
    }
    Ok(start..end)
}

/// True iff the document's frontmatter declares a recognized WAML `type:`.
///
/// This scans the leading frontmatter region line by line — mirroring the core
/// parser's `scan_frontmatter_and_preamble` — rather than requiring a cleanly
/// terminated `---`…`---` block. That matters for a buffer mid-edit whose
/// frontmatter is broken/unterminated (the exact `FrontmatterNotClean` case):
/// a strict block parse would classify it as non-WAML and silently suppress
/// its live diagnostics, blinding the LSP to the very error it reports.
/// UTF-16 code-unit offset of byte offset `byte_col` within `line_text`.
pub fn utf16_col(line_text: &str, byte_col: usize) -> Result<u32, PositionError> {
    if byte_col > line_text.len() {
        return Err(PositionError::ByteOffsetOutOfBounds {
            offset: byte_col,
            len: line_text.len(),
        });
    }
    let prefix = line_text
        .get(..byte_col)
        .ok_or(PositionError::ByteOffsetNotCharBoundary { offset: byte_col })?;
    Ok(prefix.chars().map(|c| c.len_utf16() as u32).sum())
}

fn severity(s: Severity) -> lsp::DiagnosticSeverity {
    match s {
        Severity::Error => lsp::DiagnosticSeverity::ERROR,
        Severity::Warning => lsp::DiagnosticSeverity::WARNING,
    }
}

/// Map a core `Diagnostic` to an LSP diagnostic, given the text of its line.
pub fn to_lsp_diagnostic(d: &Diagnostic, document: &DocumentVersion) -> lsp::Diagnostic {
    let range = d.range.and_then(|range| {
        to_lsp_range(range.start().to_usize()..range.end().to_usize(), document).ok()
    });
    let range = range.unwrap_or_else(|| {
        let requested_line = d.line.saturating_sub(1);
        let (line, line_text) = document
            .text()
            .shared()
            .lines()
            .enumerate()
            .nth(requested_line)
            .map(|(line, text)| (line as u32, text))
            .unwrap_or((0, ""));
        let default_range = (0, utf16_col(line_text, line_text.len()).unwrap_or(0));
        let (start_ch, end_ch) = d
            .span
            .and_then(|(start, end)| {
                Some((
                    utf16_col(line_text, start).ok()?,
                    utf16_col(line_text, end).ok()?,
                ))
            })
            .unwrap_or(default_range);
        lsp::Range {
            start: lsp::Position {
                line,
                character: start_ch,
            },
            end: lsp::Position {
                line,
                character: end_ch,
            },
        }
    });
    lsp::Diagnostic {
        range,
        severity: Some(severity(d.severity)),
        code: Some(lsp::NumberOrString::String(d.code.as_str().to_string())),
        source: Some("waml".to_string()),
        message: d.message.clone(),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::lsp::bundle::LspAnalysisState;

    use super::*;

    fn state_with_text(text: &str) -> LspAnalysisState {
        LspAnalysisState::from_documents(
            None,
            [(PathBuf::from("position-test.md"), text.to_owned())],
        )
        .unwrap()
    }

    fn document(state: &LspAnalysisState) -> &DocumentVersion {
        let source = &state.source.documents()[0];
        let id = state.okf.catalog.id_for_path(source.path()).unwrap();
        state.okf.catalog.document(id).unwrap()
    }

    #[test]
    fn utf16_col_counts_code_units_not_bytes() {
        // "héllo": 'é' is 2 bytes but 1 UTF-16 unit.
        let line = "héllo";
        assert_eq!(utf16_col(line, 0), Ok(0));
        assert_eq!(utf16_col(line, 3), Ok(2)); // after "hé" (1 + 2 bytes) -> 2 units
        assert_eq!(
            utf16_col(line, 2),
            Err(PositionError::ByteOffsetNotCharBoundary { offset: 2 })
        );
        assert_eq!(
            utf16_col(line, 7),
            Err(PositionError::ByteOffsetOutOfBounds { offset: 7, len: 6 })
        );
    }

    #[test]
    fn non_ascii_link_span_maps_to_correct_utf16_range() {
        // A `[Café](./cafe.md)` link: the byte span must convert to UTF-16 units.
        let line = "- depends [Café](./cafe.md)";
        let byte_start = line.find("[Café]").unwrap();
        let u = utf16_col(line, byte_start).unwrap();
        assert_eq!(
            u as usize,
            line[..byte_start]
                .chars()
                .map(char::len_utf16)
                .sum::<usize>()
        );
    }

    #[test]
    fn diagnostic_fallback_uses_full_line_when_either_span_boundary_is_invalid() {
        use waml::diagnostic::DiagCode;

        let state = state_with_text("# 😀\nnext");
        let document = document(&state);
        let default_range = lsp::Range::new(lsp::Position::new(0, 0), lsp::Position::new(0, 4));

        for span in [(3, 6), (2, 3)] {
            let diagnostic = Diagnostic::new(
                DiagCode::MalformedAttribute,
                "invalid byte boundary",
                "position-test.md",
                1,
            )
            .with_span(span);

            assert_eq!(
                to_lsp_diagnostic(&diagnostic, document).range,
                default_range
            );
        }
    }

    #[test]
    fn ascii_positions_cover_line_and_document_boundaries() {
        let state = state_with_text("abc\ndef");
        let document = document(&state);

        let cases = [
            (0, lsp::Position::new(0, 0)),
            (3, lsp::Position::new(0, 3)),
            (4, lsp::Position::new(1, 0)),
            (7, lsp::Position::new(1, 3)),
        ];
        for (offset, position) in cases {
            assert_eq!(to_lsp_position(offset, document), Ok(position));
            assert_eq!(from_lsp_position(position, document), Ok(offset));
        }
    }

    #[test]
    fn crlf_positions_reject_the_unrepresentable_interior_offset() {
        let state = state_with_text("a\r\nb");
        let document = document(&state);

        assert_eq!(to_lsp_position(1, document), Ok(lsp::Position::new(0, 1)));
        assert_eq!(
            to_lsp_position(2, document),
            Err(PositionError::ByteOffsetInLineEnding { offset: 2 })
        );
        assert_eq!(to_lsp_position(3, document), Ok(lsp::Position::new(1, 0)));
    }

    #[test]
    fn astral_range_round_trips_through_utf16_positions() {
        let state = state_with_text("# 😀 目标\nnext");
        let document = document(&state);
        let byte_range = 7..13;
        let lsp_range = lsp::Range::new(lsp::Position::new(0, 5), lsp::Position::new(0, 7));

        assert_eq!(to_lsp_range(byte_range.clone(), document), Ok(lsp_range));
        assert_eq!(from_lsp_range(lsp_range, document), Ok(byte_range));
    }

    #[test]
    fn checked_positions_reject_invalid_utf8_and_utf16_boundaries() {
        let state = state_with_text("# 😀\nnext");
        let document = document(&state);

        assert_eq!(
            to_lsp_position(3, document),
            Err(PositionError::ByteOffsetNotCharBoundary { offset: 3 })
        );
        assert_eq!(
            to_lsp_position(12, document),
            Err(PositionError::ByteOffsetOutOfBounds {
                offset: 12,
                len: 11
            })
        );
        assert_eq!(
            from_lsp_position(lsp::Position::new(0, 3), document),
            Err(PositionError::Utf16ColumnNotCharBoundary {
                line: 0,
                character: 3,
            })
        );
        assert_eq!(
            from_lsp_position(lsp::Position::new(0, 5), document),
            Err(PositionError::Utf16ColumnOutOfBounds {
                line: 0,
                character: 5,
            })
        );
        assert_eq!(
            from_lsp_position(lsp::Position::new(2, 0), document),
            Err(PositionError::LineOutOfBounds { line: 2 })
        );
    }

    #[test]
    fn checked_ranges_reject_reversed_boundaries() {
        let state = state_with_text("one\ntwo");
        let document = document(&state);

        assert_eq!(
            to_lsp_range(Range { start: 5, end: 4 }, document),
            Err(PositionError::ReversedByteRange { start: 5, end: 4 })
        );
        assert_eq!(
            from_lsp_range(
                lsp::Range::new(lsp::Position::new(1, 0), lsp::Position::new(0, 3)),
                document,
            ),
            Err(PositionError::ReversedByteRange { start: 4, end: 3 })
        );
    }
}
