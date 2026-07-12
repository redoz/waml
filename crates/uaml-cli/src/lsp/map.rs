//! Pure Diagnostic→LSP mapping, byte→UTF-16 conversion, and the UAML filter.
//! This is the only place byte offsets become UTF-16 code units.

use tower_lsp::lsp_types as lsp;
use uaml::diagnostic::{Diagnostic, Severity};
use uaml::model::ClassifierType;

/// True iff the document's frontmatter declares a recognized UAML `type:`.
///
/// This scans the leading frontmatter region line by line — mirroring the core
/// parser's `scan_frontmatter_and_preamble` — rather than requiring a cleanly
/// terminated `---`…`---` block. That matters for a buffer mid-edit whose
/// frontmatter is broken/unterminated (the exact `FrontmatterNotClean` case):
/// a strict block parse would classify it as non-UAML and silently suppress
/// its live diagnostics, blinding the LSP to the very error it reports.
pub fn is_uaml(text: &str) -> bool {
    let mut in_fm = false;
    for raw in text.lines() {
        let trimmed = raw.trim_end_matches('\r').trim();
        if !in_fm {
            if trimmed.is_empty() {
                continue;
            }
            if trimmed == "---" {
                in_fm = true;
                continue;
            }
            return false; // first content isn't a frontmatter opener
        }
        if trimmed == "---" || trimmed == "..." {
            break; // frontmatter closed without a recognized type
        }
        if let Some(rest) = trimmed.strip_prefix("type:") {
            let ty = rest.trim().trim_matches('"');
            return ty == "Diagram"
                || !matches!(ClassifierType::parse(ty), ClassifierType::Unknown(_));
        }
    }
    false
}

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
pub fn to_lsp_diagnostic(d: &Diagnostic, line_text: &str) -> lsp::Diagnostic {
    let line = (d.line.saturating_sub(1)) as u32; // LSP is 0-based
    let (start_ch, end_ch) = match d.span {
        Some((s, e)) => (utf16_col(line_text, s), utf16_col(line_text, e)),
        None => (0, utf16_col(line_text, line_text.len())),
    };
    lsp::Diagnostic {
        range: lsp::Range {
            start: lsp::Position { line, character: start_ch },
            end: lsp::Position { line, character: end_ch },
        },
        severity: Some(severity(d.severity)),
        code: Some(lsp::NumberOrString::String(d.code.as_str().to_string())),
        source: Some("uaml".to_string()),
        message: d.message.clone(),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_uaml_detects_recognized_types_only() {
        assert!(is_uaml("---\ntype: uml.Class\n---\n# X\n"));
        assert!(is_uaml("---\ntype: Diagram\n---\n# X\n"));
        assert!(!is_uaml("# just markdown\n"));
        assert!(!is_uaml("---\ntype: bpmn.Task\n---\n# X\n"));
    }

    #[test]
    fn is_uaml_recognizes_broken_frontmatter_with_a_type() {
        // A buffer mid-edit with an unterminated frontmatter block (the exact
        // `FrontmatterNotClean` case) must still be classified as UAML so its
        // live diagnostics are published, not silently suppressed.
        assert!(is_uaml("---\ntype: uml.Class\ntitle: A\n# X\n"));
        // ... closer with no trailing block terminator is still recognized.
        assert!(is_uaml("---\ntype: uml.Class\n"));
        // Broken block without any recognized type stays non-UAML.
        assert!(!is_uaml("---\ntitle: A\n# X\n"));
        assert!(!is_uaml("---\ntype: bpmn.Task\n# X\n"));
    }

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
        assert_eq!(u as usize, line[..byte_start].chars().map(char::len_utf16).sum::<usize>());
    }
}
