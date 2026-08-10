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
    /// The exact token identified above, missing or typed. `token`/`slot`
    /// alone cannot tell one atom from a sibling of the same kind -- e.g. the
    /// alternating operand/direction words a malformed `## Layout` statement
    /// recovers as flat siblings all share `slot == LayoutStatement` -- so a
    /// provider that needs to find its own position among `node`'s children
    /// compares against this by identity.
    pub operand: SyntaxToken<UmlLanguage>,
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
    //
    // A token can also be present-but-empty rather than missing: `relationship_link`
    // always lays down a real (non-missing) `LinkTargetToken` once it sees a
    // balanced `[...](...)`, even when nothing sits between the parens yet --
    // unlike most other slots, which stay `missing_token` until something is
    // typed. For that zero-width case there is no "left of the token" to be
    // strictly past, so the position is a match exactly at the token's own
    // (single) offset.
    let typed = tokens.iter().find(|token| {
        if !COMPLETION_TOKENS.contains(&token.kind()) || token.flags().is_missing() {
            return false;
        }
        let range = token.trimmed_range();
        if range.start() == range.end() {
            range.start().to_usize() == offset.to_usize()
        } else {
            range.start().to_usize() < offset.to_usize()
                && offset.to_usize() <= range.end().to_usize()
        }
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
        operand: token,
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
    candidates.extend(link_targets(&expectation, &context, version.path()));
    // Later slices append further providers here; each is selected on the slot
    // and token kinds alone, so adding a family is a new function and a match
    // arm and the locator never changes.
    for candidate in &mut candidates {
        pad_for_adjacency(candidate, &source);
    }
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

/// Guard against a candidate fusing with whatever sits outside `replace`. The
/// parser does not always position a slot's boundary on the far side of its
/// mandatory separator -- `### Check` places the missing `NodeKindToken`
/// right after the heading markers, before the space that separates them
/// from the identity, so a naive insertion there glues onto `###`; a cursor
/// resting after that same space instead glues the candidate onto `Check`.
/// Padding on whichever side is not already whitespace (or document start /
/// end) keeps every insertion syntactically separated, which is what the
/// accept-set property test in `uml_completion_accept_set.rs` pins.
fn pad_for_adjacency(candidate: &mut Completion, source: &str) {
    let bytes = source.as_bytes();
    let start = candidate.replace.start().to_usize();
    let end = candidate.replace.end().to_usize();
    let is_boundary = |byte: Option<&u8>| matches!(byte, None | Some(b' ' | b'\t' | b'\n' | b'\r'));
    let needs_lead = start
        .checked_sub(1)
        .is_some_and(|before| !is_boundary(bytes.get(before)));
    let needs_trail = !is_boundary(bytes.get(end));
    if !needs_lead && !needs_trail {
        return;
    }
    let mut padded = String::with_capacity(candidate.insert.len() + 2);
    if needs_lead {
        padded.push(' ');
    }
    padded.push_str(&candidate.insert);
    if needs_trail {
        padded.push(' ');
    }
    candidate.insert = Arc::from(padded.as_str());
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
            if layout_reference_position(expectation) == Some(true) {
                // This atom occupies a reference/operand position (see
                // `layout_reference_position`); a direction or hint word
                // there would parse as the operand, not silence -- offering
                // it would let a candidate satisfy `UnresolvedLayoutRef`
                // while still corrupting the statement's shape.
                return Vec::new();
            }
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

/// Whether the atom `expectation` points at occupies a reference (operand)
/// position, for the flat error-recovery shape only. A cleanly parsed layout
/// statement wraps each role in its own node (`NameRef` for a reference,
/// `DirectionClause` for a direction, `HintClause` for a hint), so
/// `expectation.slot` already tells them apart there. When the shape parser
/// recovers from a malformed statement, `append_layout_recovery` instead lays
/// every remaining atom down as a flat sibling of `LayoutStatement`,
/// alternating operand, direction, operand, ... in document order (the
/// grammar's own shape, mirrored by `LayoutShapeCursor` in parser.rs) -- so
/// this walks that sibling list by hand and returns `Some(true)` on an even
/// atom index (a reference), `Some(false)` on an odd one (a direction or
/// hint), and `None` when `expectation` did not come from this flat shape at
/// all.
fn layout_reference_position(expectation: &Expectation) -> Option<bool> {
    if expectation.slot != UmlSyntaxKind::LayoutStatement {
        return None;
    }
    let mut atom_index = 0usize;
    for child in expectation.node.children() {
        let is_target =
            matches!(&child, SyntaxElement::Token(token) if *token == expectation.operand);
        if is_target {
            return Some(atom_index % 2 == 0);
        }
        let is_atom = match &child {
            SyntaxElement::Token(token) => matches!(
                token.kind(),
                UmlSyntaxKind::LayoutWordToken
                    | UmlSyntaxKind::LayoutLinkToken
                    | UmlSyntaxKind::LayoutQuoteToken
            ),
            SyntaxElement::Node(node) => node.kind() == UmlSyntaxKind::SkippedTokensSyntax,
        };
        if is_atom {
            atom_index += 1;
        }
    }
    None
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

/// A diagram member's authored link text (e.g. the `A` in `[A](./a.md)`).
/// `member.target` holds the href, which is what the diagnostic resolves
/// through -- but `## Layout` operands are typed as bare names, matched by
/// basename against the resolved target (see `solve::resolve::resolve_ref`),
/// so the candidate this provider offers is the link text, not the href.
fn member_display_name(member: &crate::uml::DeclaredMember) -> Option<String> {
    let text_token = member
        .syntax
        .link()?
        .children()
        .find(|element| element.kind() == UmlSyntaxKind::LinkTextToken)
        .and_then(SyntaxElement::into_token)?;
    if text_token.flags().is_missing() {
        return None;
    }
    let text = text_token.text().write_to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
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
            if expectation.slot == UmlSyntaxKind::FlowTarget {
                return concept
                    .flow_nodes
                    .iter()
                    .filter_map(|node| declared_name(&node.identity))
                    .map(|identity| reference(identity, replace, "flow node"))
                    .collect();
            }
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
        // `for` references an id the author already declared with `as`. The
        // parser gives the two positions different slot kinds, so no keyword
        // test is needed here.
        UmlSyntaxKind::ReturnCallToken => concept
            .messages
            .iter()
            .filter_map(|message| declared_name(&message.call_id))
            .map(|id| reference(id, replace, "declared call id"))
            .collect(),
        UmlSyntaxKind::IdentityToken => concept
            .flow_nodes
            .iter()
            .filter_map(|node| declared_name(&node.identity))
            .map(|identity| reference(identity, replace, "flow node"))
            .collect(),
        UmlSyntaxKind::LocalToken => concept
            .lifelines
            .iter()
            .filter_map(|lifeline| declared_name(&lifeline.alias))
            .map(|alias| reference(alias, replace, "lifeline handle"))
            .collect(),
        UmlSyntaxKind::LayoutWordToken => {
            if layout_reference_position(expectation) == Some(false) {
                // A direction or hint position (see `layout_reference_position`):
                // the shape parser's fallback accepts any plain word as a
                // reference, so a member name inserted here parses cleanly as
                // a *second* operand rather than the direction the statement
                // still needs -- wrong shape, no diagnostic to catch it.
                return Vec::new();
            }
            let mut out = Vec::new();
            let mut push_member = |target: &str| {
                out.push(reference(target, replace, "diagram member"));
            };
            for member in concept.members.iter() {
                if let Some(name) = member_display_name(member) {
                    push_member(&name);
                }
            }
            for group in concept.member_groups.iter() {
                if let Some(name) = declared_name(&group.name) {
                    push_member(name);
                }
                for member in group.members.iter() {
                    if let Some(name) = member_display_name(member) {
                        push_member(&name);
                    }
                }
            }
            out
        }
        _ => Vec::new(),
    }
}

/// Documents from the OKF catalog, filtered by what the enclosing section
/// accepts. Insert the bundle-relative path, label with the document title,
/// reusing the resolution `UnresolvedTarget` already performs so a chosen path
/// round-trips.
fn link_targets(
    expectation: &Expectation,
    context: &ActionContext<'_>,
    from: &crate::source::BundlePath,
) -> Vec<Completion> {
    if expectation.token != UmlSyntaxKind::LinkTargetToken {
        return Vec::new();
    }
    let wants_classifier = matches!(
        section_of(&expectation.node),
        Some(UmlSyntaxKind::LifelinesSection) | Some(UmlSyntaxKind::MembersSection)
    );
    let mut out = Vec::new();
    for document in context.okf().catalog.documents().values() {
        let path = document.path();
        if path == from {
            continue;
        }
        let Some(concept) = context
            .okf()
            .bundle
            .concept(&crate::okf::id_of(path.as_str()))
        else {
            continue;
        };
        let element_type = crate::model::ElementType::parse(&concept.ty);
        if wants_classifier && !element_type.is_classifier() {
            continue;
        }
        let href = relative_href(from, path);
        // Round-trip guard: only offer a path that resolves back to this
        // document, so a candidate can never produce UnresolvedTarget.
        if crate::okf::resolve_href(from.as_str(), &href) != crate::okf::id_of(path.as_str()) {
            continue;
        }
        let label = concept
            .title
            .as_deref()
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| path.as_str());
        out.push(Completion {
            label: Arc::from(label),
            insert: Arc::from(href.as_str()),
            kind: CompletionKind::Link,
            detail: Some(Arc::from(path.as_str())),
            replace: expectation.prefix,
        });
    }
    out
}

/// The WAML section a node sits in, or `None` for a node outside one.
fn section_of(node: &SyntaxNode<UmlLanguage>) -> Option<UmlSyntaxKind> {
    let mut current = Some(node.clone());
    while let Some(candidate) = current {
        if matches!(
            candidate.kind(),
            UmlSyntaxKind::AttributesSection
                | UmlSyntaxKind::ValuesSection
                | UmlSyntaxKind::SlotsSection
                | UmlSyntaxKind::RelationshipsSection
                | UmlSyntaxKind::MembersSection
                | UmlSyntaxKind::LayoutSection
                | UmlSyntaxKind::FlowSection
                | UmlSyntaxKind::LifelinesSection
                | UmlSyntaxKind::MessagesSection
                | UmlSyntaxKind::GatesSection
        ) {
            return Some(candidate.kind());
        }
        current = candidate.parent();
    }
    None
}

/// A bundle-relative href from `from` to `to`, in the `./name.md` form the
/// corpus already uses.
fn relative_href(from: &crate::source::BundlePath, to: &crate::source::BundlePath) -> String {
    let depth = from.as_str().matches('/').count();
    let mut href = String::new();
    if depth == 0 {
        href.push_str("./");
    } else {
        for _ in 0..depth {
            href.push_str("../");
        }
    }
    href.push_str(to.as_str());
    href
}
