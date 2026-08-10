//! Completion suggestions at the positions where the WAML grammar expects an
//! operand. See `docs/superpowers/specs/2026-08-10-completion-suggestions-design.md`.
//!
//! The module has two halves that do not know about each other: a locator
//! answering *where am I*, and candidate providers answering *what goes here*.
//! Neither reads diagnostics; both read the parser's fixed slot kinds.

use waml_syntax::{SyntaxElement, SyntaxNode, SyntaxToken, SyntaxTree, TextRange, TextSize};

use super::syntax::{UmlLanguage, UmlSyntaxKind};

/// Token kinds that name an operand the author supplies. A token outside this
/// table -- prose, a newline, a bullet, end-of-file -- is never a completion
/// position, which is how silence in prose and after a complete construct
/// falls out with no special case.
const COMPLETION_TOKENS: &[UmlSyntaxKind] = &[
    UmlSyntaxKind::AliasToken,
    UmlSyntaxKind::SourceToken,
    UmlSyntaxKind::VerbToken,
    UmlSyntaxKind::TargetToken,
    UmlSyntaxKind::CallIdToken,
    UmlSyntaxKind::ReturnTargetToken,
    UmlSyntaxKind::ReturnCallToken,
    UmlSyntaxKind::LocalToken,
    UmlSyntaxKind::IdentityToken,
    UmlSyntaxKind::NodeKindToken,
    UmlSyntaxKind::FragmentKindToken,
    UmlSyntaxKind::RelationshipKindToken,
    UmlSyntaxKind::LinkTargetToken,
    UmlSyntaxKind::TypeToken,
    UmlSyntaxKind::LayoutWordToken,
    UmlSyntaxKind::LayoutKeywordToken,
    UmlSyntaxKind::IdentifierToken,
];

/// Where the cursor is, in the parser's own terms.
pub struct Expectation {
    /// The fixed slot node directly above the operand token.
    pub slot: UmlSyntaxKind,
    /// The operand token itself: missing when nothing was authored, present
    /// when the author is part-way through a word.
    pub token: UmlSyntaxKind,
    /// The slot node, for providers that need to look at its siblings.
    pub node: SyntaxNode<UmlLanguage>,
    /// The range a candidate replaces. Empty when nothing was authored.
    pub prefix: TextRange,
}

/// Find the token to the left of `offset`, skipping trivia, and report the slot
/// it belongs to. `None` -- after a complete construct, in prose, in a raw
/// Markdown region, past the end of the document -- is a normal result.
pub fn expectation_at(tree: &SyntaxTree<UmlLanguage>, offset: TextSize) -> Option<Expectation> {
    let source = tree.write_to_string();
    if offset.to_usize() > source.len() {
        return None;
    }
    let mut anchor = offset.to_usize();
    while anchor > 0 && matches!(source.as_bytes()[anchor - 1], b' ' | b'\t') {
        anchor -= 1;
    }
    let mut tokens = Vec::new();
    collect_tokens(&tree.root(), &mut tokens);

    // Mid-word: the author is part-way through an operand. Only a token that
    // is itself a completion candidate counts here -- the cursor merely
    // touching the end of a fixed keyword (e.g. `as|` with no space) is not
    // "typing into" that keyword, so it falls through to the next rule.
    let typed = tokens.iter().find(|token| {
        COMPLETION_TOKENS.contains(&token.kind())
            && !token.flags().is_missing()
            && token.trimmed_range().start().to_usize() < offset.to_usize()
            && offset.to_usize() <= token.trimmed_range().end().to_usize()
    });
    if let Some(token) = typed {
        return expectation(token.clone(), token.trimmed_range());
    }

    // After the keyword: a zero-width missing token marks the empty slot. The
    // parser can lay down several alternative missing tokens at the same
    // position (e.g. an optional trailing `as <call id>` after a complete
    // message); the first one in document order is the primary continuation
    // the grammar expects there, so the search is deliberately unfiltered --
    // `expectation` below is what applies the COMPLETION_TOKENS gate.
    let missing = tokens
        .iter()
        .find(|token| token.flags().is_missing() && token.range().start().to_usize() == anchor)?;
    expectation(missing.clone(), TextRange::new(offset, offset).ok()?)
}

fn expectation(token: SyntaxToken<UmlLanguage>, prefix: TextRange) -> Option<Expectation> {
    if !COMPLETION_TOKENS.contains(&token.kind()) {
        return None;
    }
    let node = token.parent()?;
    Some(Expectation {
        slot: node.kind(),
        token: token.kind(),
        node,
        prefix,
    })
}

fn collect_tokens(node: &SyntaxNode<UmlLanguage>, out: &mut Vec<SyntaxToken<UmlLanguage>>) {
    for element in node.children() {
        match element {
            SyntaxElement::Token(token) => out.push(token),
            SyntaxElement::Node(child) => collect_tokens(&child, out),
        }
    }
}
