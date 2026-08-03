//! The viewer block model derived from a `PresentationPlan`.
//!
//! The model keeps every source byte: a suppressed marker is a piece with
//! `emit == false`, never a missing piece. That is the same invariant
//! `PresentationPlan::validate_source_partition` enforces, and it is what
//! makes "everything drawn maps back to source" checkable.

use waml_markdown_editor::presentation::{
    compile_presentation, HighlighterRegistry, PresentationStyles, TextRole,
};
use waml_markdown_editor::reading::{build_reading_document, ReadingBlockKind, ReadingDocument};
use waml_markdown_editor::syntax::{parse_markdown, DocumentRevision, MarkdownDialect, SourceText};

fn document(source: &str) -> ReadingDocument {
    let text = SourceText::new(source).expect("valid source");
    let syntax = parse_markdown(
        DocumentRevision::INITIAL,
        text,
        MarkdownDialect::WAML_DEFAULT,
    )
    .expect("markdown parses");
    let styles = PresentationStyles::balanced();
    let plan = compile_presentation(&syntax, &styles, &HighlighterRegistry::default())
        .expect("presentation compiles");
    build_reading_document(&plan).expect("reading model builds")
}

fn kinds(doc: &ReadingDocument) -> Vec<ReadingBlockKind> {
    fn walk(
        blocks: &[waml_markdown_editor::reading::ReadingBlock],
        out: &mut Vec<ReadingBlockKind>,
    ) {
        for block in blocks {
            out.push(block.kind);
            walk(&block.children, out);
        }
    }
    let mut out = Vec::new();
    walk(&doc.roots, &mut out);
    out
}

#[test]
fn every_source_byte_lands_in_exactly_one_piece() {
    let doc = document("# Title\n\nBody *emphasis* and `code`.\n\n- one\n- two\n");
    doc.validate_source_partition()
        .expect("the reading model must cover the source exactly once");
}

#[test]
fn a_suppressed_marker_is_kept_as_a_non_emitting_piece() {
    let doc = document("# Title\n");
    let heading = doc
        .roots
        .iter()
        .find(|block| matches!(block.kind, ReadingBlockKind::Heading(1)))
        .expect("an h1 block");
    let marker = heading
        .pieces
        .iter()
        .find(|piece| matches!(piece.role, TextRole::HeadingMarker(1)))
        .expect("the `#` run survives as a piece");
    assert!(
        !marker.emit,
        "a reading view suppresses the `#` but must not drop its source range"
    );
    assert!(
        heading.pieces.iter().any(|piece| piece.emit),
        "the heading text itself still emits"
    );
}

#[test]
fn frontmatter_is_suppressed_but_still_covered() {
    let doc = document("---\ntitle: Notes\n---\n\n# Notes\n");
    doc.validate_source_partition()
        .expect("frontmatter bytes stay in the partition");
    fn any_emitting_frontmatter(blocks: &[waml_markdown_editor::reading::ReadingBlock]) -> bool {
        blocks.iter().any(|block| {
            block
                .pieces
                .iter()
                .any(|piece| piece.role == TextRole::Frontmatter && piece.emit)
                || any_emitting_frontmatter(&block.children)
        })
    }
    assert!(
        !any_emitting_frontmatter(&doc.roots),
        "frontmatter is document metadata, not prose"
    );
}

#[test]
fn an_unordered_item_becomes_a_bullet_item_and_its_marker_does_not_emit() {
    let doc = document("- one\n- two\n");
    let items: Vec<_> = kinds(&doc)
        .into_iter()
        .filter(|kind| matches!(kind, ReadingBlockKind::BulletItem { .. }))
        .collect();
    assert_eq!(items.len(), 2, "two bullet items");
    assert_eq!(items[0], ReadingBlockKind::BulletItem { level: 0 });
    fn markers_emit(blocks: &[waml_markdown_editor::reading::ReadingBlock]) -> bool {
        blocks.iter().any(|block| {
            block
                .pieces
                .iter()
                .any(|piece| piece.role == TextRole::ListMarker && piece.emit)
                || markers_emit(&block.children)
        })
    }
    assert!(
        !markers_emit(&doc.roots),
        "a bullet character is punctuation"
    );
}

#[test]
fn an_ordered_number_is_content_and_still_emits() {
    let doc = document("1. one\n2. two\n");
    let items: Vec<_> = kinds(&doc)
        .into_iter()
        .filter(|kind| matches!(kind, ReadingBlockKind::OrderedItem { .. }))
        .collect();
    assert_eq!(items.len(), 2, "two ordered items");
    fn emitting_markers(blocks: &[waml_markdown_editor::reading::ReadingBlock]) -> usize {
        blocks
            .iter()
            .map(|block| {
                block
                    .pieces
                    .iter()
                    .filter(|piece| piece.role == TextRole::ListMarker && piece.emit)
                    .count()
                    + emitting_markers(&block.children)
            })
            .sum()
    }
    assert_eq!(
        emitting_markers(&doc.roots),
        2,
        "an ordered number is content a reader needs"
    );
}

#[test]
fn nested_items_report_their_nesting_level() {
    let doc = document("- outer\n  - inner\n");
    let levels: Vec<u8> = kinds(&doc)
        .into_iter()
        .filter_map(|kind| match kind {
            ReadingBlockKind::BulletItem { level } => Some(level),
            _ => None,
        })
        .collect();
    assert_eq!(levels, vec![0, 1], "nesting depth drives the bullet shape");
}

#[test]
fn a_quote_nests_its_paragraph_and_suppresses_the_angle_bracket() {
    let doc = document("> quoted\n");
    let quote = doc
        .roots
        .iter()
        .find(|block| block.kind == ReadingBlockKind::Quote)
        .expect("a quote block");
    assert!(
        !quote.children.is_empty(),
        "a quote owns the paragraph it wraps"
    );
    fn quote_markers_emit(blocks: &[waml_markdown_editor::reading::ReadingBlock]) -> bool {
        blocks.iter().any(|block| {
            block
                .pieces
                .iter()
                .any(|piece| piece.role == TextRole::QuoteMarker && piece.emit)
                || quote_markers_emit(&block.children)
        })
    }
    assert!(!quote_markers_emit(&doc.roots), "`>` is punctuation");
}

#[test]
fn fenced_code_keeps_its_content_and_suppresses_its_fences() {
    let doc = document("```rust\nlet x = 1;\n```\n");
    assert!(
        kinds(&doc).contains(&ReadingBlockKind::Code),
        "a fenced block becomes a code block"
    );
    fn roles(blocks: &[waml_markdown_editor::reading::ReadingBlock]) -> Vec<(TextRole, bool)> {
        let mut out = Vec::new();
        for block in blocks {
            out.extend(block.pieces.iter().map(|piece| (piece.role, piece.emit)));
            out.extend(roles(&block.children));
        }
        out
    }
    let roles = roles(&doc.roots);
    assert!(
        roles
            .iter()
            .any(|(role, emit)| *role == TextRole::CodeFence && !*emit),
        "the ``` fences are suppressed"
    );
    assert!(
        roles.iter().any(|(role, emit)| matches!(
            role,
            TextRole::CodeContent | TextRole::CodeToken(_)
        ) && *emit),
        "the code itself is drawn"
    );
}

#[test]
fn an_empty_document_is_a_valid_empty_model() {
    let doc = document("");
    assert!(doc.roots.is_empty(), "no blocks");
    doc.validate_source_partition()
        .expect("a zero-length source is trivially covered");
}
