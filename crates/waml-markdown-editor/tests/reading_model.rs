//! The viewer block model derived from a `PresentationPlan`.
//!
//! The model keeps every source byte: a suppressed marker is a piece with
//! `emit == false`, never a missing piece. That is the same invariant
//! `PresentationPlan::validate_source_partition` enforces, and it is what
//! makes "everything drawn maps back to source" checkable.

use std::sync::Arc;
use waml_markdown_editor::presentation::{
    compile_presentation, HighlighterRegistry, PresentationBlockKind, PresentationPlan,
    PresentationStyles, TextRole,
};

use waml_markdown_editor::presentation::{EmbeddedBlockRole, PresentationRole};
use waml_markdown_editor::reading::{
    build_reading_document, ReadingBlockKind, ReadingDocument, RegisteredBlockExtensions,
};
use waml_markdown_editor::syntax::{parse_markdown, DocumentRevision, MarkdownDialect, SourceText};

fn document(source: &str) -> ReadingDocument {
    document_with_extensions(source, RegisteredBlockExtensions::default())
}

fn document_with_extensions(
    source: &str,
    extensions: RegisteredBlockExtensions,
) -> ReadingDocument {
    let plan = plan(source);
    build_reading_document(&plan, &extensions).expect("reading model builds")
}

fn plan(source: &str) -> Arc<PresentationPlan> {
    let text = SourceText::new(source).expect("valid source");
    let syntax = parse_markdown(
        DocumentRevision::INITIAL,
        text,
        MarkdownDialect::WAML_DEFAULT,
    )
    .expect("markdown parses");
    let styles = PresentationStyles::balanced();
    compile_presentation(&syntax, &styles, &HighlighterRegistry::default())
        .expect("presentation compiles")
}

fn kinds(doc: &ReadingDocument) -> Vec<ReadingBlockKind> {
    fn walk(
        blocks: &[waml_markdown_editor::reading::ReadingBlock],
        out: &mut Vec<ReadingBlockKind>,
    ) {
        for block in blocks {
            out.push(block.kind.clone());
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
            block.pieces.iter().any(|piece| {
                matches!(
                    piece.role,
                    TextRole::Frontmatter | TextRole::FrontmatterToken(_)
                ) && piece.emit
            }) || any_emitting_frontmatter(&block.children)
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

#[test]
fn a_table_delimiter_row_between_child_rows_does_not_break_the_partition() {
    // The `Table` block's own delimiter-row piece sits BETWEEN its header-row
    // and data-row children in source order. A walk that assumes a block's
    // pieces all precede its children misses the header row's bytes and
    // reports a false gap.
    let doc = document("| Column | Meaning |\n| ------ | ------- |\n| one    | first   |\n");
    doc.validate_source_partition()
        .expect("the delimiter row must not break source coverage");
    assert!(
        kinds(&doc).contains(&ReadingBlockKind::Table { columns: 2 }),
        "a table block must still be produced"
    );
}

#[test]
fn a_deep_nesting_of_quotes_does_not_overflow_the_stack() {
    // The model build and the partition walk recurse over block nesting.
    // `parse_markdown` itself overflows around 10k nested quotes (a
    // pre-existing parser bound, not the model's), so this pins the model at
    // a depth the parser survives; if the parser gains a depth cap, raise
    // this to match it.
    let mut source = String::new();
    for _ in 0..500 {
        source.push('>');
    }
    source.push_str(" deep\n");
    let doc = document(&source);
    doc.validate_source_partition()
        .expect("a deep nesting still covers its source");
}

#[test]
fn a_thematic_break_becomes_a_thematic_break_block() {
    let doc = document("before\n\n---\n\nafter\n");
    let all = kinds(&doc);
    assert!(
        all.contains(&ReadingBlockKind::ThematicBreak),
        "expected a ThematicBreak block, got {all:?}"
    );
}

#[test]
fn the_reading_model_carries_the_plans_links() {
    let doc = document("See [Customer](./customer.md) for more.\n");
    assert_eq!(doc.links.len(), 1, "one inline link");
    assert_eq!(&*doc.links[0].destination, "./customer.md");
}

#[test]
fn a_link_is_found_by_any_offset_inside_its_source_range() {
    use waml_markdown_editor::syntax::TextSize;

    let doc = document("See [Customer](./customer.md) for more.\n");
    let link = doc.links[0].clone();
    let start = link.source_range.start().to_usize();
    let end = link.source_range.end().to_usize();

    for offset in [start, start + 1, end - 1] {
        let found = doc
            .link_at(TextSize::try_from_usize(offset).unwrap())
            .unwrap_or_else(|| panic!("offset {offset} is inside the link"));
        assert_eq!(found.destination, link.destination);
    }
    // The end boundary is exclusive, and the leading "See " is outside.
    assert!(doc
        .link_at(TextSize::try_from_usize(end).unwrap())
        .is_none());
    assert!(doc.link_at(TextSize::try_from_usize(0).unwrap()).is_none());
}

#[test]
fn a_document_without_links_carries_none() {
    let doc = document("# Title\n\nJust prose.\n");
    assert!(doc.links.is_empty());
}

#[test]
fn registered_fence_languages_become_fenced_extensions_case_insensitively() {
    let extensions = RegisteredBlockExtensions::from_languages([Arc::from("mermaid")]);
    for language in ["mermaid", "MERMAID", "MeRmAiD"] {
        let source = format!("```{language}\ngraph TD; A-->B\n```\n");
        let plan = plan(&source);
        let code = plan
            .blocks
            .iter()
            .find(|block| matches!(block.kind, PresentationBlockKind::Code { .. }))
            .expect("a fenced code block");
        let doc = build_reading_document(&plan, &extensions).expect("reading model builds");
        let block = doc
            .roots
            .iter()
            .find(|block| matches!(block.kind, ReadingBlockKind::FencedExtension(_)))
            .expect("a registered fence becomes an extension block");
        let ReadingBlockKind::FencedExtension(extension) = &block.kind else {
            panic!("registered fence becomes an extension")
        };
        assert_eq!(extension.source_range, code.source_range);
        assert_eq!(
            &source[extension.content_range.start().to_usize()
                ..extension.content_range.end().to_usize()],
            "graph TD; A-->B\n"
        );
        assert_eq!(
            extension.id,
            waml_markdown_editor::presentation::PresentationItemId {
                owner: code.owner,
                role: PresentationRole::Embedded(EmbeddedBlockRole::FencedExtension),
                fragment_ordinal: 0,
            }
        );
        doc.validate_source_partition()
            .expect("the source partition stays exact");
    }
}

#[test]
fn unregistered_or_non_fenced_code_stays_code() {
    let registered = RegisteredBlockExtensions::from_languages([Arc::from("mermaid")]);
    for source in [
        "```rust\nlet x = 1;\n```\n",
        "```mermaid-js\ngraph TD; A-->B\n```\n",
        "```\ngraph TD; A-->B\n```\n",
        "    graph TD; A-->B\n",
    ] {
        let doc = document_with_extensions(source, registered.clone());
        assert!(kinds(&doc).contains(&ReadingBlockKind::Code));
    }
    let doc = document_with_extensions(
        "```mermaid\ngraph TD; A-->B\n```\n",
        RegisteredBlockExtensions::default(),
    );
    assert!(kinds(&doc).contains(&ReadingBlockKind::Code));
}
