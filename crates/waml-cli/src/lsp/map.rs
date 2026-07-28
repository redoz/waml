//! Pure Diagnostic→LSP mapping, byte→UTF-16 conversion, and the WAML filter.
//! This is the only place byte offsets become UTF-16 code units.

use tower_lsp::lsp_types as lsp;
use waml::{
    analysis::DocumentVersion,
    diagnostic::{Diagnostic, Severity},
};

/// True iff the document's frontmatter declares a recognized WAML `type:`.
///
/// This scans the leading frontmatter region line by line — mirroring the core
/// parser's `scan_frontmatter_and_preamble` — rather than requiring a cleanly
/// terminated `---`…`---` block. That matters for a buffer mid-edit whose
/// frontmatter is broken/unterminated (the exact `FrontmatterNotClean` case):
/// a strict block parse would classify it as non-WAML and silently suppress
/// its live diagnostics, blinding the LSP to the very error it reports.
/// UTF-16 code-unit offset of byte offset `byte_col` within `line_text`.
pub fn utf16_col(line_text: &str, byte_col: usize) -> u32 {
    line_text[..byte_col.min(line_text.len())]
        .chars()
        .map(|c| c.len_utf16() as u32)
        .sum()
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
        let start = document
            .line_index()
            .line_col(document.text(), range.start())
            .ok()?;
        let end = document
            .line_index()
            .line_col(document.text(), range.end())
            .ok()?;
        Some(lsp::Range {
            start: lsp::Position {
                line: start.line,
                character: document
                    .line_index()
                    .utf16_column(document.text(), range.start())
                    .ok()?,
            },
            end: lsp::Position {
                line: end.line,
                character: document
                    .line_index()
                    .utf16_column(document.text(), range.end())
                    .ok()?,
            },
        })
    });
    let range = range.unwrap_or_else(|| {
        let line = (d.line.saturating_sub(1)) as u32;
        let line_text = document
            .text()
            .shared()
            .lines()
            .nth(line as usize)
            .unwrap_or("");
        let (start_ch, end_ch) = match d.span {
            Some((start, end)) => (utf16_col(line_text, start), utf16_col(line_text, end)),
            None => (0, utf16_col(line_text, line_text.len())),
        };
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
    use super::*;

    #[test]
    fn utf16_col_counts_code_units_not_bytes() {
        // "héllo": 'é' is 2 bytes but 1 UTF-16 unit.
        let line = "héllo";
        assert_eq!(utf16_col(line, 0), 0);
        assert_eq!(utf16_col(line, 3), 2); // after "hé" (1 + 2 bytes) -> 2 units
    }

    #[test]
    fn non_ascii_link_span_maps_to_correct_utf16_range() {
        // A `[Café](./cafe.md)` link: the byte span must convert to UTF-16 units.
        let line = "- depends [Café](./cafe.md)";
        let byte_start = line.find("[Café]").unwrap();
        let u = utf16_col(line, byte_start);
        assert_eq!(
            u as usize,
            line[..byte_start]
                .chars()
                .map(char::len_utf16)
                .sum::<usize>()
        );
    }
}
