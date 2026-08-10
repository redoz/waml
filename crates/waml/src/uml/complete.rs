//! Completion suggestions at the positions where the WAML grammar expects an
//! operand. See `docs/superpowers/specs/2026-08-10-completion-suggestions-design.md`.
//!
//! The module has two halves that do not know about each other: a locator
//! answering *where am I*, and candidate providers answering *what goes here*.
//! Neither reads diagnostics; both read the parser's fixed slot kinds.

use std::sync::Arc;

use waml_syntax::{SyntaxElement, SyntaxNode, SyntaxToken, SyntaxTree, TextRange, TextSize};

use crate::action::ActionError;
use crate::analysis::DocumentId;
use crate::uml::{vocabulary, ActionContext};

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

/// What family a candidate came from. Consumers map this onto their own icon
/// vocabulary; `waml` must not gain an LSP dependency, so nothing here names
/// an LSP type.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CompletionKind {
    /// A closed grammar word.
    Keyword,
    /// A handle or id declared elsewhere in this document.
    Reference,
    /// A slot or attribute name from a classifier.
    Field,
    /// An enum member or other value.
    Value,
    /// A document from the catalog.
    Link,
    /// A name the author is inventing.
    Name,
}

/// One candidate. `insert` is what goes into the document; `label` is what the
/// client shows; `replace` is the range the insertion covers, so a client
/// replaces a half-typed word rather than appending to it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Completion {
    pub label: Arc<str>,
    pub insert: Arc<str>,
    pub kind: CompletionKind,
    pub detail: Option<Arc<str>>,
    pub replace: TextRange,
}

/// Candidates for `offset` in `document`.
///
/// Mirrors `repair_actions`: an unknown document is an error, a catalog or
/// revision mismatch is an error (`ActionContext::new` already made that
/// check), and everything else -- an offset past the end, an incomplete tree,
/// a position in prose -- is an empty list. An unfinished document is the
/// expected input, not a failure.
pub fn completions(
    context: ActionContext<'_>,
    document: DocumentId,
    offset: TextSize,
) -> Result<Vec<Completion>, ActionError> {
    let version = context
        .okf()
        .catalog
        .document(document)
        .ok_or(ActionError::UnknownDocument { document })?;
    let Some(snapshot) = context.uml().syntax.document(document) else {
        return Ok(Vec::new());
    };
    if !Arc::ptr_eq(version, snapshot.document()) {
        return Err(ActionError::MismatchedCatalog);
    }
    let source = snapshot.syntax().write_to_string();
    let Some(expectation) = expectation_at(snapshot.syntax(), offset) else {
        return Ok(Vec::new());
    };
    let mut candidates = fixed_vocabulary(&expectation);
    let concept_id = crate::okf::id_of(version.path().as_str());
    if let Some(concept) = context.uml().declared.concept(&concept_id) {
        candidates.extend(in_document_refs(&expectation, concept));
    }
    // Later slices append further providers here; each is selected on the slot
    // and token kinds alone, so adding a family is a new function and a match
    // arm and the locator never changes.
    let prefix = source
        .get(expectation.prefix.start().to_usize()..expectation.prefix.end().to_usize())
        .unwrap_or("")
        .to_ascii_lowercase();
    candidates.retain(|candidate| candidate.label.to_ascii_lowercase().starts_with(&prefix));
    candidates.sort_by(|left, right| {
        (left.kind, left.label.as_ref()).cmp(&(right.kind, right.label.as_ref()))
    });
    candidates.dedup_by(|left, right| left.kind == right.kind && left.label == right.label);
    Ok(candidates)
}

fn keyword(word: &str, replace: TextRange, detail: &str) -> Completion {
    Completion {
        label: Arc::from(word),
        insert: Arc::from(word),
        kind: CompletionKind::Keyword,
        detail: Some(Arc::from(detail)),
        replace,
    }
}

/// Closed grammar words: message verbs, relationship kinds, flow node kinds,
/// fragment kinds, layout directions and hints. Requires no analysis. Every
/// word comes from `uml::vocabulary`; none is retyped here.
fn fixed_vocabulary(expectation: &Expectation) -> Vec<Completion> {
    let replace = expectation.prefix;
    match expectation.token {
        UmlSyntaxKind::VerbToken => vocabulary::MESSAGE_VERBS
            .iter()
            .map(|word| keyword(word, replace, "message verb"))
            .collect(),
        UmlSyntaxKind::RelationshipKindToken => vocabulary::relationship_keywords()
            .map(|word| keyword(word, replace, "relationship kind"))
            .collect(),
        UmlSyntaxKind::NodeKindToken => vocabulary::flow_node_keywords()
            .map(|word| keyword(word, replace, "flow node kind"))
            .collect(),
        UmlSyntaxKind::FragmentKindToken => vocabulary::fragment_keywords()
            .map(|word| keyword(word, replace, "combined fragment"))
            .collect(),
        UmlSyntaxKind::LayoutWordToken | UmlSyntaxKind::LayoutKeywordToken => {
            vocabulary::LAYOUT_DIRECTION_PHRASES
                .iter()
                .map(|word| keyword(word, replace, "layout direction"))
                .chain(
                    vocabulary::LAYOUT_HINT_PHRASES
                        .iter()
                        .map(|word| keyword(word, replace, "layout hint")),
                )
                .collect()
        }
        _ => Vec::new(),
    }
}

use crate::uml::{DeclaredConcept, DeclaredField};

fn reference(value: &str, replace: TextRange, detail: &str) -> Completion {
    Completion {
        label: Arc::from(value),
        insert: Arc::from(value),
        kind: CompletionKind::Reference,
        detail: Some(Arc::from(detail)),
        replace,
    }
}

fn declared_name(field: &DeclaredField<UmlLanguage, String>) -> Option<&str> {
    match field {
        DeclaredField::Valid { value, .. } => Some(value.as_str()),
        _ => None,
    }
}

/// Names declared elsewhere in this document. Every candidate is a value the
/// diagnostic at this position accepts -- `UnknownLifelineHandle` for an
/// endpoint, and so on -- which Task 10 pins as a property test.
fn in_document_refs(expectation: &Expectation, concept: &DeclaredConcept) -> Vec<Completion> {
    let replace = expectation.prefix;
    match expectation.token {
        UmlSyntaxKind::SourceToken
        | UmlSyntaxKind::TargetToken
        | UmlSyntaxKind::ReturnTargetToken => {
            let mut out = Vec::new();
            for lifeline in concept.lifelines.iter() {
                if let Some(alias) = declared_name(&lifeline.alias) {
                    out.push(reference(alias, replace, "lifeline handle"));
                }
            }
            for gate in concept.gates.iter() {
                if let Some(name) = declared_name(&gate.name) {
                    out.push(reference(&format!("@{name}"), replace, "local gate"));
                }
            }
            for use_ in concept.interaction_uses.iter() {
                if let Some(alias) = declared_name(&use_.alias) {
                    out.push(reference(
                        &format!("{alias}@"),
                        replace,
                        "gate on an interaction use",
                    ));
                }
            }
            out.push(reference("outside", replace, "outside the frame"));
            out
        }
        _ => Vec::new(),
    }
}
