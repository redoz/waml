use std::sync::Arc;

use proptest::prelude::*;
use waml_syntax::{
    parse_markdown, reparse_markdown, DocumentRevision, MarkdownDialect, SourceText, SyntaxElement,
    SyntaxNode, TextChange, TextRange, TextSize,
};

const BASE: &str = "---\ntitle: test\n---\n\n# Model\n\n[id]: /one\n\n- item\n\n| a | b |\n| - | - |\n| 1 | 2 |\n\n<div>html</div>\n\nuse [x][id]\n\n## Attributes\nname: String\n";

fn source(value: &str) -> SourceText {
    SourceText::new(value).unwrap()
}

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(TextSize::try_from_usize(start).unwrap(), TextSize::try_from_usize(end).unwrap())
        .unwrap()
}

fn fingerprint(node: SyntaxNode<waml_syntax::OkfMarkdownLanguage>, out: &mut Vec<String>) {
    out.push(format!("node:{:?}:{:?}", node.kind(), node.range()));
    for child in node.children() {
        match child {
            SyntaxElement::Node(child) => fingerprint(child, out),
            SyntaxElement::Token(token) => out.push(format!(
                "token:{:?}:{:?}:{}",
                token.kind(),
                token.range(),
                token.text().write_to_string()
            )),
        }
    }
}

fn structural_fingerprint(snapshot: &waml_syntax::MarkdownSyntaxSnapshot) -> Vec<String> {
    let mut output = Vec::new();
    fingerprint(snapshot.tree().root(), &mut output);
    output
}

fn diagnostic_fingerprint(snapshot: &waml_syntax::MarkdownSyntaxSnapshot) -> Vec<String> {
    let mut diagnostics: Vec<_> = snapshot
        .diagnostics()
        .iter()
        .map(|diagnostic| format!("{:?}:{:?}:{}", diagnostic.code, diagnostic.range, diagnostic.message))
        .collect();
    diagnostics.sort_unstable();
    diagnostics
}

fn boundaries(value: &str) -> Vec<usize> {
    value
        .char_indices()
        .map(|(offset, _)| offset)
        .chain(std::iter::once(value.len()))
        .collect()
}

fn assert_full_oracle(snapshot: &waml_syntax::MarkdownSyntaxSnapshot, candidate: &str) {
    let full = parse_markdown(
        DocumentRevision::new(snapshot.revision().get() + 1),
        source(candidate),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    assert_eq!(snapshot.text().shared().as_str(), candidate);
    assert_eq!(snapshot.tree().write_to_string(), candidate);
    assert_eq!(structural_fingerprint(snapshot), structural_fingerprint(&full));
    assert_eq!(diagnostic_fingerprint(snapshot), diagnostic_fingerprint(&full));
    assert_eq!(
        format!(
            "{:?}:{:?}:{:?}:{:?}:{:?}:{:?}",
            snapshot.structure().headings,
            snapshot.structure().nested_headings,
            snapshot.structure().protected_ranges,
            snapshot.structure().list_item_lines,
            snapshot.structure().tab_indented_item_lines,
            snapshot.structure().opaque_ranges,
        ),
        format!(
            "{:?}:{:?}:{:?}:{:?}:{:?}:{:?}",
            full.structure().headings,
            full.structure().nested_headings,
            full.structure().protected_ranges,
            full.structure().list_item_lines,
            full.structure().tab_indented_item_lines,
            full.structure().opaque_ranges,
        ),
        "structure metadata agrees without identity IDs"
    );
    assert_eq!(
        snapshot.queries().links().count(),
        full.queries().links().count(),
        "reference resolution and query roles agree"
    );
    assert_eq!(snapshot.structure().islands.len(), full.structure().islands.len());
}

proptest! {
    #[test]
    fn randomized_full_and_incremental_snapshots_agree(edits in prop::collection::vec((any::<u8>(), any::<u8>(), any::<u8>()), 1..=8)) {
        let mut candidate = BASE.to_owned();
        let mut snapshot = parse_markdown(DocumentRevision::INITIAL, source(&candidate), MarkdownDialect::WAML_DEFAULT).unwrap();
        let mut revision = DocumentRevision::INITIAL;
        for (first, second, replacement_kind) in edits {
            let points = boundaries(&candidate);
            let left = usize::from(first) % points.len();
            let right = usize::from(second) % points.len();
            let (start, end) = if left <= right { (points[left], points[right]) } else { (points[right], points[left]) };
            let replacement: Arc<str> = match replacement_kind % 4 {
                0 => Arc::from(""),
                1 => Arc::from("x"),
                2 => Arc::from("é"),
                _ => Arc::from("[n][id]"),
            };
            candidate.replace_range(start..end, &replacement);
            revision = revision.checked_next().unwrap();
            let update = reparse_markdown(
                &snapshot,
                revision,
                source(&candidate),
                &[TextChange { old_range: range(start, end), replacement }],
            ).unwrap();
            assert_full_oracle(&update.snapshot, &candidate);
            if let waml_syntax::MarkdownReparseOutcome::Full { reason } = update.outcome {
                prop_assert!(format!("{reason:?}").len() > 0, "every full outcome has a named reason");
            }
            snapshot = update.snapshot;
        }
    }
}
