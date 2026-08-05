//! UML-specific syntax classification for the "waml" fenced/inline code
//! highlighting subsystem: builds a per-owner [`WamlCodeSyntaxSnapshot`] map
//! that `waml::analysis::OkfAnalysis` reads to answer `WamlCodeRole` queries.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use waml_syntax::{
    parse_markdown, MarkdownDialect, MarkdownSemanticRole, MarkdownSyntaxSnapshot, SourceText,
    SyntaxElement, SyntaxIdentity, SyntaxNode, SyntaxToken, SyntaxTree, TextRange, TextSize,
};

use crate::analysis::{
    DocumentId, DocumentRevision, MarkdownSyntaxSet, WamlCodeRole, WamlCodeSpan,
};

pub(crate) struct WamlCodeSyntaxSnapshot {
    pub(crate) document: DocumentId,
    pub(crate) revision: DocumentRevision,
    pub(crate) fenced: bool,
    pub(crate) content_range: TextRange,
    /// Walked/sorted/deduped once at construction so `code_spans()` is a
    /// clone rather than a tree walk on every call (issue 34, Task 4). The
    /// source `syntax` tree and `source_range` used to compute this are not
    /// retained beyond construction — nothing else in this type reads them.
    spans: Option<Arc<[WamlCodeSpan]>>,
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
        let spans = compute_waml_code_spans(&syntax, source_range, content_range);
        Self {
            document,
            revision,
            fenced,
            content_range,
            spans,
        }
    }

    pub(crate) fn code_spans(&self) -> Option<Arc<[WamlCodeSpan]>> {
        self.spans.clone()
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
    uml: &super::Analysis,
) -> BTreeMap<SyntaxIdentity, WamlCodeSyntaxSnapshot> {
    let mut snapshots = BTreeMap::new();
    for (document, markdown) in markdown.documents() {
        for island in markdown.structure().islands.iter() {
            let Some(snapshot) = uml.island_syntax.by_owner(*document, island.owner) else {
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
