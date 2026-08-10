//! Completion suggestions at the positions where the WAML grammar expects an
//! operand. See `docs/superpowers/specs/2026-08-10-completion-suggestions-design.md`.
//!
//! The module has two halves that do not know about each other: a locator
//! answering *where am I*, and candidate providers answering *what goes here*.
//! Neither reads diagnostics; both read the parser's fixed slot kinds.

use std::sync::Arc;

use waml_syntax::{
    AstNode, SyntaxElement, SyntaxNode, SyntaxToken, SyntaxTree, TextRange, TextSize,
};

use crate::action::ActionError;
use crate::analysis::DocumentId;
use crate::uml::{vocabulary, ActionContext};

use super::syntax::{expected_layout_role, LayoutRole, UmlLanguage, UmlSyntaxKind};

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
    if let Some(missing) = tokens
        .iter()
        .find(|token| token.flags().is_missing() && token.range().start().to_usize() == anchor)
    {
        return expectation(missing.clone(), TextRange::new(offset, offset).ok()?);
    }

    // A `## Layout` statement that already parses can still be continued -- a
    // direction clause may follow a complete operand -- so the grammar expects
    // an operand after `- A `, but nothing is missing there and the two rules
    // above find nothing. The statement node itself is the position; the atom
    // it trails is carried as `operand` so the slot has a real token.
    layout_continuation(&tokens, offset, anchor)
}

/// The position just past the last atom of a complete `## Layout` statement,
/// with whitespace between it and the cursor. Requiring the *last* atom keeps
/// this from firing with the cursor parked in front of later words, where the
/// grammar's next expectation is not the one the trailing text will meet.
fn layout_continuation(
    tokens: &[SyntaxToken<UmlLanguage>],
    offset: TextSize,
    anchor: usize,
) -> Option<Expectation> {
    if offset.to_usize() <= anchor {
        return None;
    }
    let is_atom =
        |token: &SyntaxToken<UmlLanguage>| !token.flags().is_missing() && is_layout_atom(token);
    let last = tokens
        .iter()
        .find(|token| is_atom(token) && token.trimmed_range().end().to_usize() == anchor)?;
    let statement = layout_statement(&last.parent()?)?;
    if has_layout_atom_after(&statement, last.trimmed_range().end()) {
        return None;
    }
    Some(Expectation {
        slot: statement.kind(),
        token: UmlSyntaxKind::LayoutWordToken,
        node: statement,
        prefix: TextRange::new(offset, offset).ok()?,
        operand: last.clone(),
    })
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
        candidates.extend(in_document_refs(
            &expectation,
            concept,
            &context,
            version.path(),
        ));
        candidates.extend(type_driven(&expectation, &context, concept, version.path()));
        candidates.extend(derived_names(&expectation, concept));
    }
    candidates.extend(link_targets(&expectation, &context, version.path()));
    // Later slices append further providers here; each is selected on the slot
    // and token kinds alone, so adding a family is a new function and a match
    // arm and the locator never changes.
    // Filter before padding, and against the inserted text as well as the
    // label: a link candidate is *labelled* with the target document's title
    // while the author is typing its href, so a label-only filter answers
    // nothing the moment `(` is followed by a single character.
    let prefix = source
        .get(expectation.prefix.start().to_usize()..expectation.prefix.end().to_usize())
        .unwrap_or("")
        .to_ascii_lowercase();
    candidates.retain(|candidate| {
        candidate.label.to_ascii_lowercase().starts_with(&prefix)
            || candidate.insert.to_ascii_lowercase().starts_with(&prefix)
    });
    // A link target is already delimited by its own parentheses, so padding it
    // would author `[Buyer]( ./a.md )` rather than the canonical form.
    if expectation.token != UmlSyntaxKind::LinkTargetToken {
        for candidate in &mut candidates {
            pad_for_adjacency(candidate, &source);
        }
    }
    candidates.sort_by(|left, right| {
        (left.kind, left.label.as_ref(), left.insert.as_ref()).cmp(&(
            right.kind,
            right.label.as_ref(),
            right.insert.as_ref(),
        ))
    });
    // Dedup on what is authored, not on what is shown: two catalog documents
    // may carry the same title, and collapsing them by label would make one of
    // them unreachable through completion.
    candidates.dedup_by(|left, right| left.kind == right.kind && left.insert == right.insert);
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
        // A direction and a hint are never both admissible: the grammar takes
        // a direction only after a complete operand and a hint only after
        // `with`/`and`/`,`. Offering the other family would corrupt the
        // statement's shape while still satisfying `UnresolvedLayoutRef`.
        UmlSyntaxKind::LayoutWordToken | UmlSyntaxKind::LayoutKeywordToken => {
            match layout_role(expectation) {
                Some(LayoutRole::Direction) => vocabulary::LAYOUT_DIRECTION_PHRASES
                    .iter()
                    .map(|word| keyword(word, replace, "layout direction"))
                    .collect(),
                Some(LayoutRole::Hint) => vocabulary::LAYOUT_HINT_PHRASES
                    .iter()
                    .map(|word| keyword(word, replace, "layout hint"))
                    .collect(),
                Some(LayoutRole::Reference) | None => Vec::new(),
            }
        }
        _ => Vec::new(),
    }
}

/// Which role the `## Layout` grammar expects at `expectation`'s position.
///
/// The recovery shape cannot be read off positionally: `SkippedTokensSyntax`
/// wraps a whole run of atoms in one node, and on an empty bullet it wraps the
/// bullet's own `-`, so counting siblings misreads both. The grammar is asked
/// directly instead -- gather the atom words authored before the cursor,
/// wherever the recovery shape happens to have put them, and let the shape
/// parser say what may follow. Offsets, not node identity, decide what counts
/// as "before", so a clean parse, a recovered parse and a half-typed word all
/// read the same way.
fn layout_role(expectation: &Expectation) -> Option<LayoutRole> {
    let statement = layout_statement(&expectation.node)?;
    let mut words = Vec::new();
    collect_layout_words(&statement, expectation.prefix.start(), &mut words);
    expected_layout_role(&words)
}

/// The `LayoutStatement` at or above `node`.
fn layout_statement(node: &SyntaxNode<UmlLanguage>) -> Option<SyntaxNode<UmlLanguage>> {
    let mut current = Some(node.clone());
    while let Some(candidate) = current {
        if candidate.kind() == UmlSyntaxKind::LayoutStatement {
            return Some(candidate);
        }
        current = candidate.parent();
    }
    None
}

/// Whether `token` is one of the atoms the `## Layout` shape parser consumes.
///
/// Exactly the kinds `layout_statement` (parser.rs) pushes onto its
/// `atom_words`: the punctuation that opens, closes and separates an inline
/// group is an atom of that grammar just as a word is (`reference` matches on
/// the spellings `"("`, `")"` and `","`), so a reconstruction that drops it
/// asks the grammar about a different statement than the author wrote.
fn is_layout_atom(token: &SyntaxToken<UmlLanguage>) -> bool {
    matches!(
        token.kind(),
        UmlSyntaxKind::LayoutWordToken
            | UmlSyntaxKind::LayoutLinkToken
            | UmlSyntaxKind::LayoutQuoteToken
            | UmlSyntaxKind::LayoutOpenParenToken
            | UmlSyntaxKind::LayoutCloseParenToken
            | UmlSyntaxKind::LayoutCommaToken
    )
}

/// Every authored layout atom in `node` that ends at or before `cutoff`,
/// lower-cased and trimmed exactly as the parser's own atom lexer records them.
fn collect_layout_words(node: &SyntaxNode<UmlLanguage>, cutoff: TextSize, out: &mut Vec<String>) {
    for child in node.children() {
        match child {
            SyntaxElement::Token(token) => {
                if is_layout_atom(&token)
                    && !token.flags().is_missing()
                    && token.trimmed_range().end() <= cutoff
                {
                    out.push(token.text().write_to_string().trim().to_ascii_lowercase());
                }
            }
            SyntaxElement::Node(child) => collect_layout_words(&child, cutoff, out),
        }
    }
}

/// Whether `node` still holds an authored layout atom past `offset`.
fn has_layout_atom_after(node: &SyntaxNode<UmlLanguage>, offset: TextSize) -> bool {
    node.children().any(|child| match child {
        SyntaxElement::Token(token) => {
            is_layout_atom(&token)
                && !token.flags().is_missing()
                && token.trimmed_range().end() > offset
        }
        SyntaxElement::Node(child) => has_layout_atom_after(&child, offset),
    })
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

/// The bare name a `## Layout` operand may use to reach `member`.
///
/// Not the link text: a bare operand is matched by `slugify(name)` against the
/// member's *resolved id* -- exactly, or by unique basename -- in both
/// `collect_unresolved_layout_refs` (analysis.rs) and
/// `solve::resolve::resolve_ref`, so `- [Status](./enum.md)` is reached as
/// `enum` and offering `Status` there is an outright `UnresolvedLayoutRef`.
/// The two coincide only when the author happened to title the link after the
/// file. A basename that is not already in slug form cannot be named by a bare
/// operand at all (no operand slugifies back to it), so it is not offered.
fn member_layout_name(
    member: &crate::uml::DeclaredMember,
    from: &crate::source::BundlePath,
) -> Option<String> {
    let DeclaredField::Valid { value: href, .. } = &member.target else {
        return None;
    };
    let id = crate::okf::resolve_href(from.as_str(), href);
    let basename = id.rsplit('/').next().unwrap_or(id.as_str()).to_owned();
    (!basename.is_empty() && crate::slug::slugify(&basename, "") == basename).then_some(basename)
}

/// `name` spelled as a single `## Layout` atom.
///
/// The statement grammar is whitespace-separated and also breaks an atom on
/// `(`, `)`, `,` and `[`, so a name carrying any of those -- a group heading
/// like `### Core People` -- parses as one operand only inside quotes. A name
/// containing a quote itself has no spelling at all and is not offered.
fn layout_atom(name: &str) -> Option<String> {
    if name.contains('"') {
        return None;
    }
    let breaks = name
        .chars()
        .any(|character| character.is_whitespace() || matches!(character, '(' | ')' | ',' | '['));
    Some(if breaks {
        format!("\"{name}\"")
    } else {
        name.to_owned()
    })
}

/// The lifeline handles of the interaction the binding under the cursor belongs
/// to. `validate_use_bindings` (sequence.rs) checks a binding's *target* half
/// against the referenced interaction's lifelines, not this document's, so this
/// is the accept set at a `BindingTarget`; an unresolvable use offers nothing.
fn used_interaction_handles(
    expectation: &Expectation,
    context: &ActionContext<'_>,
    concept: &DeclaredConcept,
    from: &crate::source::BundlePath,
) -> Vec<String> {
    let at = expectation.prefix.start();
    let Some(use_) = concept.interaction_uses.iter().find(|use_| {
        let range = use_.syntax.syntax().range();
        range.start() <= at && at <= range.end()
    }) else {
        return Vec::new();
    };
    let Some(href) = declared_name(&use_.link) else {
        return Vec::new();
    };
    let target = crate::okf::resolve_href(from.as_str(), href);
    let Some(target_concept) = context.uml().declared.concept(&target) else {
        return Vec::new();
    };
    target_concept
        .lifelines
        .iter()
        .filter_map(|lifeline| declared_name(&lifeline.alias))
        .map(str::to_owned)
        .collect()
}

/// Names declared elsewhere in this document. Every candidate is a value the
/// diagnostic at this position accepts -- `UnknownLifelineHandle` for an
/// endpoint, and so on -- which Task 10 pins as a property test.
fn in_document_refs(
    expectation: &Expectation,
    concept: &DeclaredConcept,
    context: &ActionContext<'_>,
    from: &crate::source::BundlePath,
) -> Vec<Completion> {
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
            // A binding's target half is an endpoint of the *used* interaction,
            // not a message endpoint of this document: no handle of this
            // document, no gate and no `outside` belongs here.
            if expectation.slot == UmlSyntaxKind::BindingTarget {
                return used_interaction_handles(expectation, context, concept, from)
                    .iter()
                    .map(|handle| reference(handle, replace, "lifeline of the used interaction"))
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
        // `IdentityToken` only ever occurs at a flow node's own heading
        // (`FlowIdentity`, parser.rs), which *declares* an identity -- a
        // reference goes through `FlowTarget`/`TargetToken` above. Every
        // already-declared identity is a `DuplicateFlowNode` here, so a
        // declaration site offers nothing, exactly as `as <call id>` does.
        UmlSyntaxKind::LocalToken => concept
            .lifelines
            .iter()
            .filter_map(|lifeline| declared_name(&lifeline.alias))
            .map(|alias| reference(alias, replace, "lifeline handle"))
            .collect(),
        UmlSyntaxKind::LayoutWordToken => {
            if layout_role(expectation) != Some(LayoutRole::Reference) {
                // A direction or hint position: the shape parser's fallback
                // accepts any plain word as a reference, so a member name
                // inserted here parses as a *second* operand rather than the
                // direction the statement still needs -- wrong shape, and at a
                // hint position an outright `MalformedLayout`.
                return Vec::new();
            }
            let mut out = Vec::new();
            let mut push = |name: &str, detail: &str| {
                if let Some(atom) = layout_atom(name) {
                    out.push(reference(&atom, replace, detail));
                }
            };
            for member in concept.members.iter() {
                if let Some(name) = member_layout_name(member, from) {
                    push(&name, "diagram member");
                }
            }
            for group in concept.member_groups.iter() {
                if let Some(name) = declared_name(&group.name) {
                    push(name, "member group");
                }
                for member in group.members.iter() {
                    if let Some(name) = member_layout_name(member, from) {
                        push(&name, "diagram member");
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
        let href = crate::okf::relative_href(from.as_str(), path.as_str());
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

/// Whether `expectation`'s operand sits in a `Slot`'s name half rather than
/// its value half. `simple_item`/`classifier_tokens` (parser.rs) lex a
/// slot's name and value as flat `IdentifierToken` siblings of the same
/// `Slot` node -- neither gets its own wrapping node -- so the two are told
/// apart only by position relative to the slot's `ColonToken` sibling.
/// Absent entirely (an empty bullet has no colon at all, so the parser lays
/// a single placeholder `IdentifierToken` directly) means there is nothing
/// to be but the name.
fn is_slot_name_position(expectation: &Expectation) -> bool {
    if expectation.slot != UmlSyntaxKind::Slot {
        return false;
    }
    let mut past_colon = false;
    for child in expectation.node.children() {
        if let SyntaxElement::Token(token) = &child {
            if *token == expectation.operand {
                return !past_colon;
            }
            if token.kind() == UmlSyntaxKind::ColonToken {
                past_colon = true;
            }
        }
    }
    true
}

/// The classifier a concept's own `## Slots` describe: the target of an
/// `instance of` relationship declared in its `## Relationships`, resolved
/// the same way `SlotUnknownAttribute`'s own check resolves it (see the
/// first loop over `concept.slots` in `analysis.rs`). `None` when there is
/// no such relationship, or it does not resolve, which is the ordinary case
/// in a plain classifier document -- a slot position offers nothing there.
fn instance_of_classifier(
    concept: &DeclaredConcept,
    from: &crate::source::BundlePath,
) -> Option<String> {
    concept.relationships.iter().find_map(|relationship| {
        let DeclaredField::Valid { value: kind, .. } = &relationship.kind else {
            return None;
        };
        if *kind != crate::model::RelationshipKind::InstanceOf {
            return None;
        }
        let DeclaredField::Valid { value: href, .. } = &relationship.target else {
            return None;
        };
        Some(crate::okf::resolve_href(from.as_str(), href))
    })
}

/// Slot names from the concept's own classifier's attributes -- the accept
/// set `SlotUnknownAttribute` already computes for `concept.slots` against an
/// `instance of` relationship. The value half is not offered: an empty slot
/// value ("- status: |") never gets a token at all, missing or otherwise, so
/// there is no position to hang a value provider on -- see
/// `a_slot_value_is_not_yet_a_completion_position` in
/// `uml_completion_locator.rs`.
fn type_driven(
    expectation: &Expectation,
    context: &ActionContext<'_>,
    concept: &DeclaredConcept,
    from: &crate::source::BundlePath,
) -> Vec<Completion> {
    if expectation.token != UmlSyntaxKind::IdentifierToken || !is_slot_name_position(expectation) {
        return Vec::new();
    }
    let Some(classifier) = instance_of_classifier(concept, from) else {
        return Vec::new();
    };
    let Some(target) = context.uml().projection.node(&classifier) else {
        return Vec::new();
    };
    let replace = expectation.prefix;
    target
        .attributes
        .iter()
        .map(|attribute| Completion {
            label: Arc::from(attribute.name.as_str()),
            insert: Arc::from(attribute.name.as_str()),
            kind: CompletionKind::Field,
            detail: Some(Arc::from(attribute.ty.name.as_str())),
            replace,
        })
        .collect()
}

/// The three name shapes a link title suggests: the kebab form, the lower-cased
/// first word, and the initials. Order is the order they are offered in.
fn name_shapes(title: &str) -> Vec<String> {
    let words = title
        .split_whitespace()
        .map(|word| {
            word.chars()
                .filter(|character| character.is_alphanumeric())
                .collect::<String>()
                .to_lowercase()
        })
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    if words.is_empty() {
        return Vec::new();
    }
    let mut shapes = vec![words.join("-")];
    shapes.push(words[0].clone());
    if words.len() > 1 {
        shapes.push(
            words
                .iter()
                .filter_map(|word| word.chars().next())
                .collect::<String>(),
        );
    }
    shapes.dedup();
    shapes
}

/// Positions where the author invents a name rather than referencing one.
fn derived_names(expectation: &Expectation, concept: &DeclaredConcept) -> Vec<Completion> {
    let detail = match expectation.slot {
        UmlSyntaxKind::LifelineAlias => "lifeline handle",
        UmlSyntaxKind::InteractionUseAlias => "interaction use alias",
        UmlSyntaxKind::MessageCallId => "call id",
        UmlSyntaxKind::InlineInstance => "instance name",
        _ => return Vec::new(),
    };
    let Some(title) = sibling_link_title(&expectation.node) else {
        // Nothing to derive from: offer nothing rather than guess.
        return Vec::new();
    };
    let taken = concept
        .lifelines
        .iter()
        .filter_map(|lifeline| declared_name(&lifeline.alias))
        .chain(
            concept
                .interaction_uses
                .iter()
                .filter_map(|use_| declared_name(&use_.alias)),
        )
        .chain(
            concept
                .messages
                .iter()
                .filter_map(|message| declared_name(&message.call_id)),
        )
        .chain(
            concept
                .inline_instances
                .iter()
                .filter_map(|instance| declared_name(&instance.name)),
        )
        .map(str::to_owned)
        .collect::<Vec<_>>();
    name_shapes(&title)
        .into_iter()
        .filter(|shape| !taken.contains(shape))
        .map(|shape| Completion {
            label: Arc::from(shape.as_str()),
            insert: Arc::from(shape.as_str()),
            kind: CompletionKind::Name,
            detail: Some(Arc::from(detail)),
            replace: expectation.prefix,
        })
        .collect()
}

/// The title text of the link on the same bullet as `node`: walk up to the
/// bullet, find its `Link` child, and read the `LinkTextToken`. `None` when
/// the token is missing or empty -- with nothing to derive from, the caller
/// offers nothing rather than a guess.
///
/// The walk starts at `node` itself, not `node.parent()`: for
/// `LifelineAlias`/`InteractionUseAlias`/`MessageCallId` the operand token
/// sits in its own wrapping slot node, so the bullet is one level up -- but
/// for `InlineInstance` there is no such wrapping slot (the instance name's
/// `IdentifierToken` is a direct child of `InlineInstance` itself, unlike the
/// other three; see `inline_instance` in parser.rs), so `expectation.node` at
/// that position already *is* the bullet. Starting one level too high would
/// walk straight past it and never match.
fn sibling_link_title(node: &SyntaxNode<UmlLanguage>) -> Option<String> {
    let mut current = Some(node.clone());
    while let Some(bullet) = current {
        if matches!(
            bullet.kind(),
            UmlSyntaxKind::Lifeline
                | UmlSyntaxKind::InteractionUse
                | UmlSyntaxKind::Message
                | UmlSyntaxKind::InlineInstance
        ) {
            let link = bullet
                .children()
                .filter_map(SyntaxElement::into_node)
                .find(|child| child.kind() == UmlSyntaxKind::Link)?;
            let text = link
                .children()
                .filter_map(SyntaxElement::into_token)
                .find(|token| token.kind() == UmlSyntaxKind::LinkTextToken)?;
            if text.flags().is_missing() {
                return None;
            }
            let title = text.text().write_to_string();
            return (!title.trim().is_empty()).then_some(title);
        }
        current = bullet.parent();
    }
    None
}
