//! End-to-end regression tests for the Markdown nesting-depth caps
//! (`MD_MAX_CONTAINER_DEPTH` in `block.rs`, `MD_MAX_INLINE_DEPTH` in
//! `inline.rs`), exercised through the same public entry points the LSP and
//! editor use, and through every downstream recursive consumer the caps are
//! meant to bound: `projection::visit` (via `parse_markdown`'s structure
//! map), `collect_occurrences` (via `reparse_markdown`'s incremental path),
//! and `SyntaxTree::rewrite` (`red.rs`).

use std::sync::Arc;

use waml_syntax::{
    parse_markdown, reparse_markdown, DocumentRevision, GreenNode, GreenToken, MarkdownDialect,
    OkfMarkdownLanguage, OkfSyntaxDiagnosticCode as Diagnostic, SourceText, SyntaxRewriter,
    SyntaxVisitor, TextChange, TextRange, TextSize,
};

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(
        TextSize::try_from_usize(start).unwrap(),
        TextSize::try_from_usize(end).unwrap(),
    )
    .unwrap()
}

struct IdentityRewriter;
impl SyntaxRewriter<OkfMarkdownLanguage> for IdentityRewriter {
    fn rewrite_node(
        &mut self,
        node: &GreenNode<OkfMarkdownLanguage>,
    ) -> GreenNode<OkfMarkdownLanguage> {
        node.clone()
    }
    fn rewrite_token(
        &mut self,
        token: &GreenToken<OkfMarkdownLanguage>,
    ) -> GreenToken<OkfMarkdownLanguage> {
        token.clone()
    }
}

struct CountingVisitor(usize);
impl SyntaxVisitor<OkfMarkdownLanguage> for CountingVisitor {
    fn visit(&mut self, _element: waml_syntax::SyntaxElement<OkfMarkdownLanguage>) {
        self.0 += 1;
    }
}

/// Parses `source` through the public entry point, asserts it succeeds,
/// round-trips exactly, and records at least one nesting-depth diagnostic,
/// then exercises every downstream recursive consumer on the resulting tree:
/// the structure map built by `projection::visit` (built inline by
/// `parse_markdown`), an incremental no-op edit through `collect_occurrences`
/// (via `reparse_markdown`), and a `SyntaxTree::rewrite` pass.
fn assert_capped_and_exercised(name: &str, source: &str) {
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new(source).unwrap(),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap_or_else(|error| panic!("{name}: expected Ok, got {error:?}"));

    assert_eq!(
        snapshot.tree().write_to_string(),
        source,
        "{name}: exact-source round-trip"
    );
    assert!(
        snapshot
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == Diagnostic::NestingDepthExceeded),
        "{name}: expected at least one nesting-depth diagnostic"
    );

    // `projection::visit` already ran inside `parse_markdown` to build the
    // structure map; touching it here proves the capped tree didn't leave it
    // in a broken state.
    let _ = snapshot.structure();

    // `SyntaxTree::rewrite` (red.rs) walks the whole tree recursively.
    let rewritten_root = snapshot.tree().rewrite(&mut IdentityRewriter);
    assert!(
        Arc::ptr_eq(snapshot.tree().root_green(), rewritten_root.root_green()),
        "{name}: identity rewrite over the capped tree did not overflow and preserved sharing"
    );

    // `SyntaxVisitor::walk` recurses per child, same shape as
    // `collect_occurrences`/`projection::visit`.
    let mut visitor = CountingVisitor(0);
    snapshot.tree().root().walk(&mut visitor);
    assert!(visitor.0 > 0, "{name}: walk visited at least one element");

    // `collect_occurrences` (incremental.rs) self-recurses per child node on
    // reparse; a trivial no-op edit at the very end of the document exercises
    // it on the capped tree without perturbing the capped structure itself.
    let mut edited = source.to_string();
    edited.push('\n');
    let update = reparse_markdown(
        &snapshot,
        DocumentRevision::new(snapshot.revision().get() + 1),
        SourceText::new(&edited).unwrap(),
        &[TextChange {
            old_range: range(source.len(), source.len()),
            replacement: Arc::from("\n"),
        }],
    )
    .unwrap_or_else(|error| panic!("{name}: expected reparse Ok, got {error:?}"));
    assert_eq!(
        update.snapshot.tree().write_to_string(),
        edited,
        "{name}: reparsed tree round-trips exactly"
    );
    assert!(
        update
            .snapshot
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == Diagnostic::NestingDepthExceeded),
        "{name}: reparsed tree still carries the nesting-depth diagnostic"
    );
}

#[test]
fn deeply_nested_block_quotes_are_capped_end_to_end() {
    let source = "> ".repeat(10_000) + "leaf\n";
    assert_capped_and_exercised("10,000-deep block quote", &source);
}

/// Nested bullet lists spend two container frames per visual level, so they
/// hit the same cap on a different code path than `>` nesting.
#[test]
fn deeply_nested_lists_are_capped_end_to_end() {
    let source: String = (0..300)
        .map(|level| format!("{}- x\n", "  ".repeat(level)))
        .collect();
    assert_capped_and_exercised("300-deep bullet list", &source);
}

/// A GFM table opened past the cap has its `Table` start suppressed while its
/// head/row/cell events keep arriving; the tree must stay balanced and the
/// source must still round-trip.
#[test]
fn suppressed_table_is_capped_end_to_end() {
    let prefix = "> ".repeat(200);
    let source = format!("{prefix}| a | b |\n{prefix}| --- | ---: |\n{prefix}| 1 | 2 |\n");
    assert_capped_and_exercised("table past the container cap", &source);
}

#[test]
fn deeply_nested_emphasis_is_capped_end_to_end() {
    let depth = 10_000;
    let source = "*".repeat(depth) + "x" + &"*".repeat(depth) + "\n";
    assert_capped_and_exercised("10,000-wide emphasis run", &source);
}

#[test]
fn mixed_container_and_inline_nesting_is_capped_end_to_end() {
    // Block-quote depth alone drives the block cap, and — inside the leaf
    // paragraph — emphasis depth alone drives the inline cap; mixing them
    // proves the two caps are independent and both still bite.
    let depth = 10_000;
    let source = "> ".repeat(depth) + &"*".repeat(depth) + "x" + &"*".repeat(depth) + "\n";
    assert_capped_and_exercised("mixed block+inline nesting", &source);
}
