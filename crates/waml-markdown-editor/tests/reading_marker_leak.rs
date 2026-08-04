//! Probe: no markdown punctuation or frontmatter may reach the rendered text.

use waml_markdown_editor::presentation::{
    compile_presentation, HighlighterRegistry, PresentationStyles,
};
use waml_markdown_editor::reading::{build_reading_document, ReadingBlock};
use waml_markdown_editor::syntax::{parse_markdown, DocumentRevision, MarkdownDialect, SourceText};

#[test]
fn heading_marks_and_frontmatter_never_reach_rendered_text() {
    let source = "---\ntitle: Evidence\n---\n\n# Alpha\n\nBody text.\n\n## Beta\n\n- item\n";
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
    let doc = build_reading_document(&plan).expect("reading model builds");

    fn walk(blocks: &[ReadingBlock], source: &str, rendered: &mut String) {
        for block in blocks {
            for piece in &block.pieces {
                if piece.emit {
                    let start = piece.range.start().to_usize();
                    let end = piece.range.end().to_usize();
                    rendered.push_str(&source[start..end]);
                }
            }
            walk(&block.children, source, rendered);
        }
    }
    let mut rendered = String::new();
    walk(&doc.roots, source, &mut rendered);

    assert!(!rendered.contains('#'), "heading marker leaked: {rendered:?}");
    assert!(
        !rendered.contains("title:"),
        "frontmatter leaked: {rendered:?}"
    );
    assert!(
        rendered.contains("Alpha") && rendered.contains("Body text."),
        "content missing: {rendered:?}"
    );
}
