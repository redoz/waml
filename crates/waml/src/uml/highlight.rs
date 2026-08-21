//! UML-specific syntax classification for the "waml" fenced/inline code
//! highlighting subsystem: builds the per-owner [`CodeSyntax`] map that
//! [`crate::uml::Analysis`] reads to answer [`WamlCodeRole`] queries.
//!
//! Everything here is UML vocabulary — the roles are read off `UmlLanguage`
//! tokens — so it belongs to the UML tier, not to the domain-agnostic OKF
//! analysis that merely supplies the markdown the islands sit in.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, OnceLock},
};

use waml_syntax::{
    parse_markdown, MarkdownDialect, MarkdownSemanticRole, MarkdownSyntaxSnapshot, SourceText,
    SyntaxElement, SyntaxIdentity, SyntaxNode, SyntaxToken, SyntaxTree, TextRange, TextSize,
};

use crate::analysis::{DocumentRevision, MarkdownSyntaxSet};
use crate::source::DocumentId;

/// How one token of WAML code should be coloured.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WamlCodeRole {
    Keyword,
    Type,
    Property,
    String,
    Number,
    Comment,
    Punctuation,
    Invalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WamlCodeSpan {
    pub range: TextRange,
    pub role: WamlCodeRole,
}

/// Every WAML island and ```waml``` fence in the bundle, keyed by its markdown
/// owner identity.
pub(crate) type CodeSyntax = BTreeMap<SyntaxIdentity, WamlCodeSyntaxSnapshot>;

pub(crate) struct WamlCodeSyntaxSnapshot {
    pub(crate) document: DocumentId,
    pub(crate) revision: DocumentRevision,
    pub(crate) fenced: bool,
    pub(crate) content_range: TextRange,
    source_range: TextRange,
    syntax: Arc<SyntaxTree<super::syntax::UmlLanguage>>,
    /// Walked/sorted/deduped at most once per snapshot, and only when
    /// `code_spans()` is actually queried (issue 34, Task 4). Building the
    /// snapshot must stay cheap: `build_code_syntax` runs for every island
    /// and fence of every document on each analysis rebuild, while the spans
    /// are asked for only by the document being highlighted.
    spans: OnceLock<Option<Arc<[WamlCodeSpan]>>>,
}

impl WamlCodeSyntaxSnapshot {
    fn new(
        document: DocumentId,
        revision: DocumentRevision,
        fenced: bool,
        source_range: TextRange,
        content_range: TextRange,
        syntax: Arc<SyntaxTree<super::syntax::UmlLanguage>>,
    ) -> Self {
        Self {
            document,
            revision,
            fenced,
            content_range,
            source_range,
            syntax,
            spans: OnceLock::new(),
        }
    }

    pub(crate) fn code_spans(&self) -> Option<Arc<[WamlCodeSpan]>> {
        self.spans
            .get_or_init(|| {
                compute_waml_code_spans(&self.syntax, self.source_range, self.content_range)
            })
            .clone()
    }

    /// Test-only copy recorded under a different revision, used to exercise
    /// the stale-revision rejection branch of [`code_spans`].
    #[cfg(test)]
    pub(crate) fn with_revision(&self, revision: DocumentRevision) -> Self {
        Self::new(
            self.document,
            revision,
            self.fenced,
            self.source_range,
            self.content_range,
            self.syntax.clone(),
        )
    }
}

fn compute_waml_code_spans(
    syntax: &SyntaxTree<super::syntax::UmlLanguage>,
    source_range: TextRange,
    content_range: TextRange,
) -> Option<Arc<[WamlCodeSpan]>> {
    let mut spans = Vec::new();
    collect_waml_code_spans(
        syntax.root(),
        source_range.start(),
        content_range,
        &mut spans,
    )?;
    spans.sort_by_key(|span| (span.range.start(), span.range.end()));
    spans.dedup_by_key(|span| span.range);
    Some(Arc::from(spans))
}

/// Walks each document's markdown structure (WAML islands plus fenced
/// ```waml``` code blocks) and parses a UML syntax tree for each, keyed by
/// the markdown owner identity.
pub(crate) fn build_code_syntax(
    markdown: &MarkdownSyntaxSet,
    island_syntax: &super::analysis::UmlIslandSyntaxSet,
) -> CodeSyntax {
    let mut snapshots = BTreeMap::new();
    for (document, markdown) in markdown.documents() {
        for island in markdown.structure().islands.iter() {
            let Some(snapshot) = island_syntax.by_owner(*document, island.owner) else {
                continue;
            };
            if snapshot.content_range() != island.content_range {
                continue;
            }
            snapshots.insert(
                island.owner,
                WamlCodeSyntaxSnapshot::new(
                    *document,
                    markdown.revision(),
                    false,
                    snapshot.source_range(),
                    snapshot.content_range(),
                    snapshot.syntax().clone(),
                ),
            );
        }

        let full_range = match TextRange::new(TextSize::new(0), markdown.text().len()) {
            Ok(range) => range,
            Err(_) => continue,
        };
        let fenced_owners = markdown
            .queries()
            .spans(full_range)
            .filter(|span| span.semantic_role == MarkdownSemanticRole::FencedCode)
            .map(|span| span.owner)
            .collect::<BTreeSet<_>>();
        for owner in fenced_owners {
            let Some(fence) = markdown.queries().fenced_code(owner) else {
                continue;
            };
            if !fence
                .language
                .as_deref()
                .is_some_and(|language| language.eq_ignore_ascii_case("waml"))
            {
                continue;
            }
            let Some(syntax) = parse_fenced_waml_syntax(markdown, fence.content_range) else {
                continue;
            };
            snapshots.insert(
                fence.owner,
                WamlCodeSyntaxSnapshot::new(
                    *document,
                    markdown.revision(),
                    true,
                    fence.content_range,
                    fence.content_range,
                    syntax,
                ),
            );
        }
    }
    snapshots
}

/// The highlight spans of one island or fence, or `None` when the request does
/// not match the analysis that is installed.
///
/// Every rejection below is a staleness check. The caller asks by `(owner,
/// content_range)` for whatever revision it last rendered, and spans computed
/// against a different revision would colour the wrong bytes.
pub(crate) fn code_spans(
    markdown_set: &MarkdownSyntaxSet,
    code_syntax: &CodeSyntax,
    owner: SyntaxIdentity,
    content_range: TextRange,
) -> Option<Arc<[WamlCodeSpan]>> {
    // Resolve the owning snapshot directly instead of scanning every
    // markdown document to find the one that recognizes `owner`
    // (issue 34, Task 4).
    let syntax = code_syntax.get(&owner)?;
    if syntax.content_range != content_range {
        return None;
    }
    let markdown = markdown_set.document(syntax.document)?;
    if syntax.revision != markdown.revision() {
        return None;
    }
    let valid = if syntax.fenced {
        markdown.queries().fenced_code(owner).is_some_and(|fence| {
            fence.content_range == content_range
                && fence
                    .language
                    .as_deref()
                    .is_some_and(|language| language.eq_ignore_ascii_case("waml"))
        })
    } else {
        markdown
            .queries()
            .island(owner)
            .is_some_and(|island| island.content_range == content_range)
    };
    if !valid {
        return None;
    }
    syntax.code_spans()
}

/// Every WAML span in one document, in order and non-overlapping, or `None` if
/// they cannot be made so.
pub(crate) fn document_code_spans(
    markdown_set: &MarkdownSyntaxSet,
    code_syntax: &CodeSyntax,
    document: DocumentId,
) -> Option<Arc<[WamlCodeSpan]>> {
    markdown_set.document(document)?;
    let snapshots = code_syntax
        .values()
        .filter(|snapshot| snapshot.document == document)
        .collect::<Vec<_>>();
    let fenced_ranges = snapshots
        .iter()
        .filter(|snapshot| snapshot.fenced)
        .map(|snapshot| snapshot.content_range)
        .collect::<Vec<_>>();
    let mut spans = Vec::new();
    for snapshot in snapshots {
        let code_spans = snapshot.code_spans()?;
        spans.extend(code_spans.iter().copied().filter(|span| {
            snapshot.fenced
                || !fenced_ranges.iter().any(|range| {
                    span.range.start() < range.end() && range.start() < span.range.end()
                })
        }));
    }
    spans.sort_by_key(|span| (span.range.start(), span.range.end()));
    spans.dedup_by_key(|span| span.range);
    spans
        .windows(2)
        .all(|pair| pair[0].range.end() <= pair[1].range.start())
        .then(|| Arc::from(spans))
}

fn parse_fenced_waml_syntax(
    markdown: &MarkdownSyntaxSnapshot,
    content_range: TextRange,
) -> Option<Arc<SyntaxTree<super::syntax::UmlLanguage>>> {
    let source = markdown
        .text()
        .shared()
        .get(content_range.start().to_usize()..content_range.end().to_usize())?;
    let source = SourceText::new(source.to_owned()).ok()?;
    let parsed = parse_markdown(markdown.revision(), source, MarkdownDialect::WAML_DEFAULT).ok()?;
    Some(super::syntax::parse_full(
        parsed.text().clone(),
        parsed.structure(),
    ))
}

fn collect_waml_code_spans(
    node: SyntaxNode<super::syntax::UmlLanguage>,
    absolute_start: TextSize,
    content_range: TextRange,
    spans: &mut Vec<WamlCodeSpan>,
) -> Option<()> {
    for element in node.children() {
        match element {
            SyntaxElement::Node(child) => {
                collect_waml_code_spans(child, absolute_start, content_range, spans)?
            }
            SyntaxElement::Token(token) => {
                let Some(role) = waml_code_role(&token) else {
                    continue;
                };
                let local = token_content_range(&token)?;
                let start = absolute_start.checked_add(local.start()).ok()?;
                let end = absolute_start.checked_add(local.end()).ok()?;
                let start = start.max(content_range.start());
                let end = end.min(content_range.end());
                if start < end {
                    spans.push(WamlCodeSpan {
                        range: TextRange::new(start, end).ok()?,
                        role,
                    });
                }
            }
        }
    }
    Some(())
}

fn token_content_range(token: &SyntaxToken<super::syntax::UmlLanguage>) -> Option<TextRange> {
    let leading_bytes = token
        .leading_trivia()
        .iter()
        .map(|trivia| trivia.text.write_to_string().len())
        .sum::<usize>();
    let leading = TextSize::try_from_usize(leading_bytes).ok()?;
    let authored = token.text().write_to_string();
    let trimmed = authored.trim_matches(char::is_whitespace);
    let prefix =
        TextSize::try_from_usize(authored.len().checked_sub(authored.trim_start().len())?).ok()?;
    let content = TextSize::try_from_usize(trimmed.len()).ok()?;
    let start = token
        .range()
        .start()
        .checked_add(leading)
        .and_then(|start| start.checked_add(prefix))
        .ok()?;
    TextRange::new(start, start.checked_add(content).ok()?).ok()
}

fn waml_code_role(token: &SyntaxToken<super::syntax::UmlLanguage>) -> Option<WamlCodeRole> {
    use super::syntax::UmlSyntaxKind as Kind;

    let kind = token.kind();
    if token.flags().is_bad() || token.flags().is_missing() || kind == Kind::BadToken {
        return Some(WamlCodeRole::Invalid);
    }
    if matches!(
        kind,
        Kind::BulletToken
            | Kind::ColonToken
            | Kind::OpenBracketToken
            | Kind::CloseBracketToken
            | Kind::CommaToken
            | Kind::ArrowToken
            | Kind::EqualsToken
            | Kind::LayoutOpenParenToken
            | Kind::LayoutCloseParenToken
            | Kind::LayoutCommaToken
            | Kind::HeadingMarkerToken
    ) {
        return Some(WamlCodeRole::Punctuation);
    }
    let ancestors = std::iter::successors(token.parent(), |node| node.parent())
        .map(|node| node.kind())
        .collect::<Vec<_>>();
    let text = token.text().write_to_string();
    if text.chars().any(|character| character.is_ascii_digit())
        && ancestors
            .iter()
            .any(|ancestor| matches!(ancestor, Kind::Multiplicity | Kind::Margin))
    {
        return Some(WamlCodeRole::Number);
    }
    if kind == Kind::TypeToken || ancestors.contains(&Kind::TypeReference) {
        return Some(WamlCodeRole::Type);
    }
    if kind == Kind::IdentifierToken
        && ancestors.iter().any(|ancestor| {
            matches!(
                ancestor,
                Kind::Attribute | Kind::Slot | Kind::InlineSlot | Kind::FlowNodeKindSlot
            )
        })
    {
        return Some(WamlCodeRole::Property);
    }
    if kind == Kind::LayoutQuoteToken {
        return Some(WamlCodeRole::String);
    }
    if matches!(
        kind,
        Kind::VisibilityToken
            | Kind::RelationshipKindToken
            | Kind::AsToken
            | Kind::WithToken
            | Kind::SetToToken
            | Kind::ToToken
            | Kind::LayoutKeywordToken
            | Kind::FlowKeywordToken
            | Kind::MessageKeywordToken
            | Kind::OperandKeywordToken
            | Kind::InternalKeywordToken
            | Kind::ElseToken
    ) {
        return Some(WamlCodeRole::Keyword);
    }
    if kind == Kind::LayoutWordToken
        && ancestors.iter().any(|ancestor| {
            matches!(
                ancestor,
                Kind::LayoutPlacement
                    | Kind::LayoutAlignment
                    | Kind::LayoutStandalone
                    | Kind::Anchored
                    | Kind::DirectionClause
                    | Kind::HintClause
                    | Kind::Axis
                    | Kind::Hint
                    | Kind::Shape
                    | Kind::Margin
                    | Kind::Flag
            )
        })
    {
        return Some(WamlCodeRole::Keyword);
    }
    (kind == Kind::RawMarkdownToken).then_some(WamlCodeRole::Comment)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::prepare_candidate;
    use crate::source::SourceBundle;

    /// `code_spans` resolves the owning snapshot directly and then validates
    /// it against the owning document: cover the island branch, the fenced
    /// branch, the `content_range` mismatch that precedes both, and the
    /// stale-revision rejection.
    #[test]
    fn code_spans_validate_both_owner_branches_and_reject_a_stale_revision() {
        let authored = "---\ntype: uml.Class\n---\n# Example\n\n## Attributes\n- unknown: Number {0..42}\n\n```waml\n## Attributes\n- unknown: Number {0..42}\n```\n";
        let candidate = prepare_candidate(
            SourceBundle::try_from_pairs([("example.md", authored)]).unwrap(),
            None,
            7,
        )
        .unwrap();
        let analysis = candidate.uml();
        let document = DocumentId::new(0);
        let markdown = analysis.markdown.document(document).unwrap();
        let full_range = TextRange::new(TextSize::new(0), markdown.text().len()).unwrap();
        let fence_owner = markdown
            .queries()
            .spans(full_range)
            .find(|span| span.semantic_role == MarkdownSemanticRole::FencedCode)
            .expect("the fenced code must have a semantic owner")
            .owner;
        let fence = markdown.queries().fenced_code(fence_owner).unwrap();
        let island = markdown
            .structure()
            .islands
            .iter()
            .find(|island| island.kind == waml_syntax::WamlSectionKind::Attributes)
            .expect("the document must retain its attributes island");

        assert!(analysis
            .code_spans(island.owner, island.content_range)
            .is_some());
        assert!(analysis
            .code_spans(fence.owner, fence.content_range)
            .is_some());
        // Each owner is rejected under the other's content range, before
        // either owner branch is consulted.
        assert!(analysis
            .code_spans(island.owner, fence.content_range)
            .is_none());
        assert!(analysis
            .code_spans(fence.owner, island.content_range)
            .is_none());

        // Rebuilding the map from the analysis' own inputs reproduces what
        // `Analysis::code_spans` serves, which is what makes the stale copy
        // below a fair stand-in for the installed one.
        let rebuilt = build_code_syntax(&analysis.markdown, &analysis.island_syntax);
        assert!(code_spans(
            &analysis.markdown,
            &rebuilt,
            island.owner,
            island.content_range
        )
        .is_some());

        let bumped = markdown.revision().checked_next().unwrap();
        let stale = rebuilt
            .iter()
            .map(|(owner, snapshot)| (*owner, snapshot.with_revision(bumped)))
            .collect::<CodeSyntax>();
        assert!(code_spans(
            &analysis.markdown,
            &stale,
            island.owner,
            island.content_range
        )
        .is_none());
        assert!(code_spans(&analysis.markdown, &stale, fence.owner, fence.content_range).is_none());
    }
}
