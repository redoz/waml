use std::{collections::HashMap, sync::Arc};

use crate::{
    ChangeMap, DocumentRevision, FullReparseReason, GreenElement, GreenText, MarkdownDialect,
    MarkdownStructureMap, OkfMarkdownLanguage, OkfSyntaxDiagnosticCode, ParseError, ReparseOutcome,
    SourceText, SyntaxElement, SyntaxIdentity, SyntaxTree, TextChange, TextRange, TreeDiagnostic,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkdownLinkKind {
    Inline,
    Reference,
    Autolink,
    ExtendedAutolink,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkdownSourceRole {
    Content,
    SyntaxMarker,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkdownSemanticRole {
    Document,
    Frontmatter,
    BlockQuote,
    List,
    ListItem,
    Paragraph,
    Heading,
    ThematicBreak,
    IndentedCode,
    FencedCode,
    HtmlBlock,
    LinkDefinition,
    Table,
    TableHead,
    TableBody,
    TableRow,
    TableCell,
    Text,
    Escape,
    Entity,
    CodeSpan,
    Emphasis,
    Strong,
    Strikethrough,
    Link,
    Image,
    Autolink,
    RawHtml,
    SoftBreak,
    HardBreak,
    TaskMarker,
    Whitespace,
    Recovery,
    WamlSection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarkdownSyntaxSpan {
    pub owner: SyntaxIdentity,
    pub range: TextRange,
    pub source_role: MarkdownSourceRole,
    pub semantic_role: MarkdownSemanticRole,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarkdownHeading {
    pub owner: SyntaxIdentity,
    pub range: TextRange,
    pub content_range: TextRange,
    pub level: u8,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkdownListKind {
    Bullet,
    Ordered { start: u64 },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarkdownList {
    pub owner: SyntaxIdentity,
    pub range: TextRange,
    pub kind: MarkdownListKind,
    pub is_item: bool,
    pub task: Option<super::TaskListState>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarkdownTableCell {
    pub owner: SyntaxIdentity,
    pub range: TextRange,
    pub alignment: super::TableAlignment,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarkdownRawHtml {
    pub owner: SyntaxIdentity,
    pub range: TextRange,
    pub filter: super::HtmlTagFilter,
    pub filtered_ranges: Arc<[TextRange]>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarkdownImage {
    pub owner: SyntaxIdentity,
    pub source_range: TextRange,
    pub alt_range: TextRange,
    pub source: Arc<str>,
    pub source_definition_range: Option<TextRange>,
    pub title: Option<Arc<str>>,
    pub kind: MarkdownLinkKind,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FencedCodeInfo {
    pub owner: SyntaxIdentity,
    pub source_range: TextRange,
    pub fence_range: TextRange,
    pub info_range: Option<TextRange>,
    pub content_range: TextRange,
    pub info: Arc<str>,
    pub language: Option<Arc<str>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarkdownLink {
    pub destination: Arc<str>,
    pub destination_range: Option<TextRange>,
    pub title: Option<Arc<str>>,
    pub kind: MarkdownLinkKind,
    pub identity: super::SyntaxIdentity,
    pub owner: super::SyntaxIdentity,
    pub source_range: TextRange,
    pub content_range: TextRange,
}

#[derive(Clone, Debug)]
pub struct MarkdownEntity {
    pub value: Arc<str>,
    pub source_range: TextRange,
    pub identity: super::SyntaxIdentity,
}

#[derive(Default)]
pub struct MarkdownSyntaxQueries {
    links: Arc<[MarkdownLink]>,
    images: Arc<[MarkdownImage]>,
    spans: Arc<[MarkdownSyntaxSpan]>,
    headings: Arc<[MarkdownHeading]>,
    lists: Arc<[MarkdownList]>,
    cells: Arc<[MarkdownTableCell]>,
    html: Arc<[MarkdownRawHtml]>,
    fenced: Arc<[FencedCodeInfo]>,
    islands: Arc<[super::WamlLanguageIsland]>,
    diagnostics: Arc<[TreeDiagnostic<OkfSyntaxDiagnosticCode>]>,
    heading_by_owner: HashMap<SyntaxIdentity, usize>,
    list_by_owner: HashMap<SyntaxIdentity, usize>,
    cell_by_owner: HashMap<SyntaxIdentity, usize>,
    link_by_owner: HashMap<SyntaxIdentity, usize>,
    image_by_owner: HashMap<SyntaxIdentity, usize>,
    html_by_owner: HashMap<SyntaxIdentity, usize>,
    fenced_by_owner: HashMap<SyntaxIdentity, usize>,
    island_by_owner: HashMap<SyntaxIdentity, usize>,
    entities: Arc<[MarkdownEntity]>,
    backlinks: Arc<HashMap<Arc<str>, Arc<[super::SyntaxIdentity]>>>,
}
impl MarkdownSyntaxQueries {
    pub fn links(&self) -> impl Iterator<Item = &MarkdownLink> {
        self.links.iter()
    }
    pub fn headings(&self) -> impl Iterator<Item = &MarkdownHeading> {
        self.headings.iter()
    }
    pub fn lists(&self) -> impl Iterator<Item = &MarkdownList> {
        self.lists.iter()
    }
    pub fn entities(&self) -> impl Iterator<Item = &MarkdownEntity> {
        self.entities.iter()
    }
    pub fn spans(&self, range: TextRange) -> impl Iterator<Item = &MarkdownSyntaxSpan> + '_ {
        self.spans.iter().filter(move |span| {
            span.range.start() < range.end() && range.start() < span.range.end()
        })
    }
    pub fn images(&self) -> impl Iterator<Item = &MarkdownImage> + '_ {
        self.images.iter()
    }
    pub fn heading(&self, owner: SyntaxIdentity) -> Option<&MarkdownHeading> {
        self.heading_by_owner
            .get(&owner)
            .and_then(|&i| self.headings.get(i))
    }
    pub fn list(&self, owner: SyntaxIdentity) -> Option<&MarkdownList> {
        self.list_by_owner
            .get(&owner)
            .and_then(|&i| self.lists.get(i))
    }
    pub fn table_cell(&self, owner: SyntaxIdentity) -> Option<&MarkdownTableCell> {
        self.cell_by_owner
            .get(&owner)
            .and_then(|&i| self.cells.get(i))
    }
    pub fn link(&self, owner: SyntaxIdentity) -> Option<&MarkdownLink> {
        self.link_by_owner
            .get(&owner)
            .and_then(|&i| self.links.get(i))
    }
    pub fn image(&self, owner: SyntaxIdentity) -> Option<&MarkdownImage> {
        self.image_by_owner
            .get(&owner)
            .and_then(|&i| self.images.get(i))
    }
    pub fn raw_html(&self, owner: SyntaxIdentity) -> Option<&MarkdownRawHtml> {
        self.html_by_owner
            .get(&owner)
            .and_then(|&i| self.html.get(i))
    }
    pub fn fenced_code(&self, owner: SyntaxIdentity) -> Option<&FencedCodeInfo> {
        self.fenced_by_owner
            .get(&owner)
            .and_then(|&i| self.fenced.get(i))
    }
    pub fn island(&self, owner: SyntaxIdentity) -> Option<&super::WamlLanguageIsland> {
        self.island_by_owner
            .get(&owner)
            .and_then(|&i| self.islands.get(i))
    }
    pub fn diagnostics(
        &self,
        range: TextRange,
    ) -> impl Iterator<Item = &TreeDiagnostic<OkfSyntaxDiagnosticCode>> + '_ {
        self.diagnostics.iter().filter(move |diagnostic| {
            diagnostic.range.start() <= range.end() && range.start() <= diagnostic.range.end()
        })
    }
    pub fn has_recovery(&self, range: TextRange) -> bool {
        self.diagnostics(range).next().is_some()
    }
    pub fn reference_backlinks(&self, label: &str) -> Arc<[super::SyntaxIdentity]> {
        super::reference::normalize_label(label)
            .and_then(|label| self.backlinks.get(&label).cloned())
            .unwrap_or_else(|| Arc::from([]))
    }
}

pub struct MarkdownSyntaxSnapshot {
    revision: DocumentRevision,
    text: SourceText,
    tree: Arc<SyntaxTree<OkfMarkdownLanguage>>,
    structure: Arc<MarkdownStructureMap>,
    diagnostics: Arc<[TreeDiagnostic<OkfSyntaxDiagnosticCode>]>,
    queries: Arc<MarkdownSyntaxQueries>,
}

impl MarkdownSyntaxSnapshot {
    fn from_tree(
        revision: DocumentRevision,
        text: SourceText,
        tree: Arc<SyntaxTree<OkfMarkdownLanguage>>,
        structure: Arc<MarkdownStructureMap>,
    ) -> Result<Arc<Self>, ParseError> {
        if tree.write_to_string() != text.shared().as_str()
            || !source_backed_green_uses(&GreenElement::Node(tree.root_green().clone()), &text)
        {
            return Err(ParseError::StructuralInvariant {
                reason: "Markdown snapshot tree does not own the published source".into(),
            });
        }
        let diagnostics = Arc::from(tree.diagnostics());
        let queries = Arc::new(queries(&tree, structure.islands.clone())?);
        Ok(Arc::new(Self {
            revision,
            text,
            tree,
            structure,
            diagnostics,
            queries,
        }))
    }

    pub fn revision(&self) -> DocumentRevision {
        self.revision
    }
    pub fn text(&self) -> &SourceText {
        &self.text
    }
    pub fn tree(&self) -> &Arc<SyntaxTree<OkfMarkdownLanguage>> {
        &self.tree
    }
    pub fn structure(&self) -> &Arc<MarkdownStructureMap> {
        &self.structure
    }
    pub fn diagnostics(&self) -> &Arc<[TreeDiagnostic<OkfSyntaxDiagnosticCode>]> {
        &self.diagnostics
    }
    pub fn queries(&self) -> &MarkdownSyntaxQueries {
        &self.queries
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MarkdownReparseOutcome {
    Incremental {
        shared_source_independent_green: usize,
        reparsed_range: Option<TextRange>,
    },
    Full {
        reason: FullReparseReason,
    },
}

#[derive(Clone)]
pub struct MarkdownSyntaxUpdate {
    pub snapshot: Arc<MarkdownSyntaxSnapshot>,
    pub affected_ranges: Arc<[TextRange]>,
    pub outcome: MarkdownReparseOutcome,
}

pub fn parse_markdown(
    revision: DocumentRevision,
    text: SourceText,
    dialect: MarkdownDialect,
) -> Result<Arc<MarkdownSyntaxSnapshot>, ParseError> {
    let parsed = super::parser::parse(text.clone(), dialect)?;
    MarkdownSyntaxSnapshot::from_tree(revision, text, parsed.tree, parsed.structure)
}

pub fn reparse_markdown(
    previous: &MarkdownSyntaxSnapshot,
    revision: DocumentRevision,
    new_text: SourceText,
    changes: &[TextChange],
) -> Result<MarkdownSyntaxUpdate, ParseError> {
    if revision <= previous.revision {
        return Err(ParseError::NonMonotonicRevision {
            previous: previous.revision,
            requested: revision,
        });
    }
    let (outcome, _structure) = crate::incremental::reparse_okf_markdown_with_structure(
        previous.tree.as_ref(),
        new_text.clone(),
        changes,
        Some(previous.text()),
        Some(previous.structure()),
    )?;
    let (mut tree, shared_source_independent_green, reparsed_range) = match outcome {
        ReparseOutcome::Incremental {
            tree,
            shared_source_independent_green,
            reparsed_range,
        } => {
            if reparsed_range.end() > new_text.len() {
                return Err(ParseError::StructuralInvariant {
                    reason: "incremental reparse range exceeds the new Markdown snapshot".into(),
                });
            }
            (tree, shared_source_independent_green, reparsed_range)
        }
        ReparseOutcome::Full { tree, reason } => {
            let tree = if let Ok(map) = ChangeMap::checked(previous.text(), changes) {
                let root = super::reparse::preserve_unchanged_island_identities(
                    previous.tree(),
                    &tree,
                    &map,
                )?;
                Arc::new(SyntaxTree::new(
                    root,
                    Arc::from(tree.diagnostics()),
                    tree.dialect(),
                ))
            } else {
                tree
            };
            let structure = Arc::new(super::from_tree(&tree, new_text.shared())?);
            let affected_ranges: Arc<[TextRange]> =
                Arc::from([super::reparse::full_range(&new_text)]);
            return Ok(MarkdownSyntaxUpdate {
                snapshot: MarkdownSyntaxSnapshot::from_tree(revision, new_text, tree, structure)?,
                affected_ranges,
                outcome: MarkdownReparseOutcome::Full { reason },
            });
        }
    };
    let map = ChangeMap::checked(previous.text(), changes).map_err(|_| {
        ParseError::StructuralInvariant {
            reason: "incremental bridge returned a tree for an invalid change map".into(),
        }
    })?;
    let mut affected_ranges = vec![reparsed_range];
    let mut dependent_ranges = Vec::new();
    if super::reparse::change_touches_reference_definition(
        previous.text(),
        &new_text,
        changes,
        &map,
    ) {
        let parsed = super::parser::parse(new_text.clone(), previous.tree().dialect())?;
        let labels = super::reparse::changed_reference_labels(
            previous.text(),
            previous.tree().root_green(),
            &new_text,
            parsed.tree.root_green(),
        )?;
        if !labels.is_empty() {
            let oracle_queries = queries(&parsed.tree, parsed.structure.islands.clone())?;
            for label in &labels {
                dependent_ranges.extend(
                    backlink_ranges(previous.queries(), label)
                        .into_iter()
                        .filter_map(|range| map.translate_unchanged(range)),
                );
                dependent_ranges.extend(backlink_ranges(&oracle_queries, label));
            }
            dependent_ranges = super::reparse::normalize_affected_ranges(dependent_ranges);
            affected_ranges = super::reparse::changed_definition_ranges(
                &new_text,
                parsed.tree.root_green(),
                &labels,
            )?;
            if affected_ranges.is_empty() {
                affected_ranges.extend(map.segments().iter().map(|segment| segment.new));
            }
            if !dependent_ranges.is_empty() {
                let root = super::reparse::splice_reference_dependents(
                    tree.root_green(),
                    parsed.tree.root_green(),
                    &dependent_ranges,
                )?;
                tree = Arc::new(SyntaxTree::new(
                    root,
                    Arc::from(parsed.tree.diagnostics()),
                    previous.tree().dialect(),
                ));
                affected_ranges.extend(dependent_ranges.iter().copied());
            }
        }
    }
    let restored_root = super::reparse::restore_unchanged_subtrees(
        previous.tree().root_green(),
        tree.root_green(),
        &new_text,
        &map,
        &dependent_ranges,
    )?;
    let restored_tree = SyntaxTree::new(
        restored_root,
        Arc::from(tree.diagnostics()),
        previous.tree().dialect(),
    );
    let restored_root = super::reparse::preserve_unchanged_island_identities(
        previous.tree(),
        &restored_tree,
        &map,
    )?;
    tree = Arc::new(SyntaxTree::new(
        restored_root,
        Arc::from(restored_tree.diagnostics()),
        previous.tree().dialect(),
    ));
    let structure = Arc::new(super::from_tree(&tree, new_text.shared())?);
    let snapshot = MarkdownSyntaxSnapshot::from_tree(revision, new_text, tree, structure)?;
    let affected_ranges: Arc<[TextRange]> =
        super::reparse::normalize_affected_ranges(affected_ranges).into();
    let outcome = MarkdownReparseOutcome::Incremental {
        shared_source_independent_green,
        reparsed_range: (affected_ranges.len() == 1).then(|| affected_ranges[0]),
    };
    Ok(MarkdownSyntaxUpdate {
        snapshot,
        affected_ranges,
        outcome,
    })
}

fn backlink_ranges(queries: &MarkdownSyntaxQueries, label: &str) -> Vec<TextRange> {
    // A backlink owner identity belongs to the inline run's owning block, so a
    // single owner can cover several reference links to the same label. Collect
    // every matching link, not just the first, or later links keep a stale
    // destination when their shared definition changes.
    let owners = queries.reference_backlinks(label);
    let mut ranges: Vec<TextRange> = queries
        .links()
        .filter(|link| owners.contains(&link.identity))
        .map(|link| link.source_range)
        .collect();
    for owner in owners.iter() {
        if let Some(range) = queries.image(*owner).map(|image| image.source_range) {
            if !ranges.contains(&range) {
                ranges.push(range);
            }
        }
    }
    ranges
}

fn source_backed_green_uses(
    element: &GreenElement<OkfMarkdownLanguage>,
    source: &SourceText,
) -> bool {
    match element {
        GreenElement::Node(node) => node
            .children()
            .iter()
            .all(|child| source_backed_green_uses(child, source)),
        GreenElement::Token(token) => std::iter::once(token.text())
            .chain(
                token
                    .leading_trivia()
                    .iter()
                    .chain(token.trailing_trivia())
                    .map(|trivia| &trivia.text),
            )
            .all(|text| match text {
                GreenText::SourceSlice { source: found, .. } => {
                    Arc::ptr_eq(source.shared(), found.shared())
                }
                GreenText::Static(_) | GreenText::Owned(_) => true,
            }),
    }
}

fn queries(
    tree: &SyntaxTree<crate::OkfMarkdownLanguage>,
    islands: Arc<[super::WamlLanguageIsland]>,
) -> Result<MarkdownSyntaxQueries, ParseError> {
    let mut collected = Collected::default();
    let mut spans = Vec::new();
    collect_queries(&tree.root(), &mut collected, tree.dialect().tag_filter())?;
    collect_spans(&tree.root(), None, &mut spans)?;
    collected
        .links
        .sort_by_key(|link| (link.source_range.start(), link.source_range.end()));
    collected
        .images
        .sort_by_key(|image| (image.source_range.start(), image.source_range.end()));
    spans.sort_by_key(|span| (span.range.start(), span.range.end()));
    let heading_by_owner = identity_map(&collected.headings, |value| value.owner);
    let list_by_owner = identity_map(&collected.lists, |value| value.owner);
    let cell_by_owner = identity_map(&collected.cells, |value| value.owner);
    let link_by_owner = identity_map(&collected.links, |value| value.owner);
    let image_by_owner = identity_map(&collected.images, |value| value.owner);
    let html_by_owner = identity_map(&collected.html, |value| value.owner);
    let fenced_by_owner = identity_map(&collected.fenced, |value| value.owner);
    let island_by_owner = islands
        .iter()
        .enumerate()
        .map(|(i, value)| (value.owner, i))
        .collect();
    Ok(MarkdownSyntaxQueries {
        links: collected.links.into(),
        images: collected.images.into(),
        spans: spans.into(),
        headings: collected.headings.into(),
        lists: collected.lists.into(),
        cells: collected.cells.into(),
        html: collected.html.into(),
        fenced: collected.fenced.into(),
        islands,
        diagnostics: Arc::from(tree.diagnostics()),
        heading_by_owner,
        list_by_owner,
        cell_by_owner,
        link_by_owner,
        image_by_owner,
        html_by_owner,
        fenced_by_owner,
        island_by_owner,
        entities: collected.entities.into(),
        backlinks: collected
            .backlinks
            .into_iter()
            .map(|(label, owners)| (label, owners.into()))
            .collect::<HashMap<_, _>>()
            .into(),
    })
}

#[derive(Default)]
struct Collected {
    links: Vec<MarkdownLink>,
    images: Vec<MarkdownImage>,
    headings: Vec<MarkdownHeading>,
    lists: Vec<MarkdownList>,
    cells: Vec<MarkdownTableCell>,
    html: Vec<MarkdownRawHtml>,
    fenced: Vec<FencedCodeInfo>,
    entities: Vec<MarkdownEntity>,
    backlinks: HashMap<Arc<str>, Vec<SyntaxIdentity>>,
}

fn identity_map<T>(
    values: &[T],
    owner: impl Fn(&T) -> SyntaxIdentity,
) -> HashMap<SyntaxIdentity, usize> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| (owner(value), index))
        .collect()
}

fn collect_queries(
    node: &crate::SyntaxNode<crate::OkfMarkdownLanguage>,
    out: &mut Collected,
    tag_filter: bool,
) -> Result<(), ParseError> {
    use crate::OkfMarkdownSyntaxKind as Kind;
    if node.kind() != Kind::Root && is_semantic(node.kind()) && identity(node).is_none() {
        return Err(ParseError::StructuralInvariant {
            reason: format!(
                "semantic Markdown {:?} node has no unique valid identity",
                node.kind()
            )
            .into(),
        });
    }
    let owner = identity(node);
    match node.kind() {
        Kind::AtxHeading | Kind::SetextHeading => {
            let owner = owner.unwrap();
            out.headings.push(MarkdownHeading {
                owner,
                range: node.range(),
                content_range: token_cover(node, |kind| kind == Kind::TextToken)
                    .unwrap_or(node.range()),
                level: heading_level(node)?,
            });
        }
        Kind::List | Kind::ListItem => {
            let owner = owner.unwrap();
            let marker = first_token(node, Kind::ListMarkerToken).or_else(|| first_any_token(node));
            let Some(marker) = marker else {
                for child in node.children() {
                    if let crate::SyntaxElement::Node(child) = child {
                        collect_queries(&child, out, tag_filter)?;
                    }
                }
                return Ok(());
            };
            let marker_text = marker.text().write_to_string();
            let kind = list_kind(&marker_text);
            out.lists.push(MarkdownList {
                owner,
                range: node.range(),
                kind,
                is_item: node.kind() == Kind::ListItem,
                task: descendant_annotation(node, super::gfm::TASK_STATE)
                    .as_deref()
                    .and_then(parse_task_state),
            });
        }
        Kind::TableCell => {
            let owner = owner.unwrap();
            let alignment = annotation_data(node, super::gfm::TABLE_ALIGNMENT)
                .and_then(parse_alignment)
                .ok_or_else(|| invariant("Markdown table cell has invalid alignment"))?;
            out.cells.push(MarkdownTableCell {
                owner,
                range: node.range(),
                alignment,
            });
        }
        Kind::HtmlBlock | Kind::RawHtml => {
            let owner = owner.unwrap();
            let filter = annotation_data(node, super::gfm::HTML_TAG_FILTER)
                .and_then(parse_html_filter)
                .unwrap_or(super::HtmlTagFilter::Allowed);
            let mut source = String::new();
            crate::write_green_to(node.green(), &mut source)
                .map_err(|_| invariant("failed to reconstruct raw HTML source"))?;
            let source_start = node.range().start().to_usize();
            let filtered_ranges = if tag_filter {
                super::gfm::disallowed_html_tag_ranges(&source)
            } else {
                Vec::new()
            }
            .into_iter()
            .map(|(start, end)| {
                TextRange::new(
                    crate::TextSize::try_from_usize(source_start + start).map_err(|_| {
                        ParseError::SourceTooLarge {
                            bytes: source_start + start,
                        }
                    })?,
                    crate::TextSize::try_from_usize(source_start + end).map_err(|_| {
                        ParseError::SourceTooLarge {
                            bytes: source_start + end,
                        }
                    })?,
                )
                .map_err(|_| invariant("filtered HTML tag has a reversed range"))
            })
            .collect::<Result<Arc<[_]>, _>>()?;
            out.html.push(MarkdownRawHtml {
                owner,
                range: node.range(),
                filter,
                filtered_ranges,
            });
        }
        Kind::FencedCodeBlock => {
            let owner = owner.unwrap();
            let fence = first_token(node, Kind::CodeFenceToken)
                .ok_or_else(|| invariant("fenced code has no opening fence"))?;
            let info_token = first_token(node, Kind::InfoStringToken);
            let info = info_token
                .as_ref()
                .map(|token| {
                    super::reference::decode_destination(token.text().write_to_string().trim())
                })
                .unwrap_or_default();
            let content_range = token_cover(node, |kind| kind == Kind::CodeTextToken)
                .unwrap_or(TextRange::new(fence.range().end(), fence.range().end()).unwrap());
            out.fenced.push(FencedCodeInfo {
                owner,
                source_range: node.range(),
                fence_range: fence.range(),
                info_range: info_token.as_ref().map(|token| token.range()),
                content_range,
                language: info
                    .split_ascii_whitespace()
                    .next()
                    .filter(|value| !value.is_empty())
                    .map(Arc::from),
                info: Arc::from(info),
            });
        }
        _ => {}
    }
    if matches!(node.kind(), Kind::Link | Kind::Image | Kind::Autolink) {
        let annotations = node.syntax_annotations();
        let destination = required_annotation(
            annotations,
            super::inline::destination_annotation(),
            "link destination",
        )?;
        let destination_range = required_annotation(
            annotations,
            super::inline::destination_range_annotation(),
            "link destination range",
        )
        .and_then(parse_annotation_range)?;
        let annotated_kind = match required_annotation(
            annotations,
            super::inline::link_kind_annotation(),
            "link kind",
        )?
        .as_ref()
        {
            "inline" => MarkdownLinkKind::Inline,
            "reference" => MarkdownLinkKind::Reference,
            _ => {
                return Err(ParseError::StructuralInvariant {
                    reason: "unknown semantic Markdown link kind".into(),
                })
            }
        };
        let owner = identity(node).ok_or_else(|| ParseError::StructuralInvariant {
            reason: "semantic Markdown link has no identity".into(),
        })?;
        let identity =
            required_annotation(annotations, super::inline::owner_annotation(), "link owner")?
                .parse::<u64>()
                .ok()
                .and_then(|value| super::SyntaxIdentity::from_annotation_data(&value.to_string()))
                .ok_or_else(|| ParseError::StructuralInvariant {
                    reason: "semantic Markdown link has invalid owner identity".into(),
                })?;
        let title = super::inline::link_annotation(annotations, super::inline::title_annotation())
            .map(Arc::from);
        if let Some(label) =
            super::inline::link_annotation(annotations, super::inline::reference_label_annotation())
                .map(Arc::from)
        {
            let owners = out.backlinks.entry(label).or_default();
            if !owners.contains(&identity) {
                owners.push(identity);
            }
        }
        let kind = if node.kind() == Kind::Autolink {
            if first_token(node, Kind::AutolinkOpenToken).is_some() {
                MarkdownLinkKind::Autolink
            } else {
                MarkdownLinkKind::ExtendedAutolink
            }
        } else {
            annotated_kind
        };
        let content_range = delimited_content(node).unwrap_or(node.range());
        if node.kind() == Kind::Image {
            out.images.push(MarkdownImage {
                owner,
                source_range: node.range(),
                alt_range: content_range,
                source: destination.clone(),
                source_definition_range: Some(destination_range),
                title: title.clone(),
                kind,
            });
        }
        out.links.push(MarkdownLink {
            destination,
            destination_range: Some(destination_range),
            title,
            kind,
            identity,
            owner,
            source_range: node.range(),
            content_range,
        });
    } else if node.kind() == Kind::Entity {
        let value = required_annotation(
            node.syntax_annotations(),
            super::inline::entity_value_annotation(),
            "entity value",
        )?;
        let identity = identity(node).ok_or_else(|| ParseError::StructuralInvariant {
            reason: "semantic Markdown entity has no identity".into(),
        })?;
        out.entities.push(MarkdownEntity {
            value,
            source_range: node.range(),
            identity,
        });
    }
    for child in node.children() {
        if let SyntaxElement::Node(child) = child {
            collect_queries(&child, out, tag_filter)?;
        }
    }
    Ok(())
}

fn identity(node: &crate::SyntaxNode<crate::OkfMarkdownLanguage>) -> Option<super::SyntaxIdentity> {
    crate::syntax_identity(node)
}

fn invariant(reason: &'static str) -> ParseError {
    ParseError::StructuralInvariant {
        reason: reason.into(),
    }
}

fn is_semantic(kind: crate::OkfMarkdownSyntaxKind) -> bool {
    use crate::OkfMarkdownSyntaxKind as Kind;
    matches!(
        kind,
        Kind::Frontmatter
            | Kind::FrontmatterEntry
            | Kind::FrontmatterMapping
            | Kind::FrontmatterSequence
            | Kind::FrontmatterSequenceItem
            | Kind::BlockQuote
            | Kind::List
            | Kind::ListItem
            | Kind::Paragraph
            | Kind::AtxHeading
            | Kind::SetextHeading
            | Kind::ThematicBreak
            | Kind::IndentedCodeBlock
            | Kind::FencedCodeBlock
            | Kind::HtmlBlock
            | Kind::LinkReferenceDefinition
            | Kind::Table
            | Kind::TableHead
            | Kind::TableBody
            | Kind::TableRow
            | Kind::TableCell
            | Kind::Text
            | Kind::Escape
            | Kind::Entity
            | Kind::CodeSpan
            | Kind::Emphasis
            | Kind::StrongEmphasis
            | Kind::Strikethrough
            | Kind::Link
            | Kind::Image
            | Kind::Autolink
            | Kind::RawHtml
            | Kind::SoftLineBreak
            | Kind::HardLineBreak
            | Kind::WamlSection
            | Kind::SkippedTokensSyntax
    )
}

fn annotation_data<'a>(
    node: &'a crate::SyntaxNode<crate::OkfMarkdownLanguage>,
    kind: &str,
) -> Option<&'a str> {
    node.syntax_annotations()
        .iter()
        .find(|annotation| annotation.kind() == kind)
        .and_then(crate::SyntaxAnnotation::data)
}

fn descendant_annotation(
    node: &crate::SyntaxNode<crate::OkfMarkdownLanguage>,
    kind: &str,
) -> Option<Arc<str>> {
    if let Some(value) = annotation_data(node, kind) {
        return Some(Arc::from(value));
    }
    for child in node.children() {
        if let SyntaxElement::Node(child) = child {
            if let Some(value) = descendant_annotation(&child, kind) {
                return Some(value);
            }
        }
    }
    None
}

fn parse_alignment(value: &str) -> Option<super::TableAlignment> {
    match value {
        "none" => Some(super::TableAlignment::None),
        "left" => Some(super::TableAlignment::Left),
        "center" => Some(super::TableAlignment::Center),
        "right" => Some(super::TableAlignment::Right),
        _ => None,
    }
}

fn parse_task_state(value: &str) -> Option<super::TaskListState> {
    match value {
        "unchecked" => Some(super::TaskListState::Unchecked),
        "checked" => Some(super::TaskListState::Checked),
        _ => None,
    }
}

fn parse_html_filter(value: &str) -> Option<super::HtmlTagFilter> {
    match value {
        "allowed" => Some(super::HtmlTagFilter::Allowed),
        "disallowed" => Some(super::HtmlTagFilter::Disallowed),
        _ => None,
    }
}

fn first_token(
    node: &crate::SyntaxNode<crate::OkfMarkdownLanguage>,
    expected: crate::OkfMarkdownSyntaxKind,
) -> Option<crate::SyntaxToken<crate::OkfMarkdownLanguage>> {
    for child in node.children() {
        match child {
            SyntaxElement::Token(token) if token.kind() == expected => return Some(token),
            SyntaxElement::Node(child) => {
                if let Some(token) = first_token(&child, expected) {
                    return Some(token);
                }
            }
            _ => {}
        }
    }
    None
}

fn first_any_token(
    node: &crate::SyntaxNode<crate::OkfMarkdownLanguage>,
) -> Option<crate::SyntaxToken<crate::OkfMarkdownLanguage>> {
    for child in node.children() {
        match child {
            SyntaxElement::Token(token) => return Some(token),
            SyntaxElement::Node(child) => {
                if let Some(token) = first_any_token(&child) {
                    return Some(token);
                }
            }
        }
    }
    None
}

fn list_kind(marker: &str) -> MarkdownListKind {
    let marker = marker.trim_start();
    let digits = marker.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0
        && marker
            .as_bytes()
            .get(digits)
            .is_some_and(|byte| matches!(byte, b'.' | b')'))
    {
        marker[..digits]
            .parse::<u64>()
            .map(|start| MarkdownListKind::Ordered { start })
            .unwrap_or(MarkdownListKind::Bullet)
    } else {
        MarkdownListKind::Bullet
    }
}

fn token_cover(
    node: &crate::SyntaxNode<crate::OkfMarkdownLanguage>,
    predicate: impl Copy + Fn(crate::OkfMarkdownSyntaxKind) -> bool,
) -> Option<TextRange> {
    let mut result: Option<TextRange> = None;
    for child in node.children() {
        match child {
            SyntaxElement::Token(token) if predicate(token.kind()) => {
                result = Some(result.map_or(token.range(), |range| range.cover(token.range())));
            }
            SyntaxElement::Node(child) => {
                if let Some(range) = token_cover(&child, predicate) {
                    result = Some(result.map_or(range, |present| present.cover(range)));
                }
            }
            _ => {}
        }
    }
    result
}

fn delimited_content(node: &crate::SyntaxNode<crate::OkfMarkdownLanguage>) -> Option<TextRange> {
    use crate::OkfMarkdownSyntaxKind as Kind;
    let open = node.children().find_map(|child| match child {
        SyntaxElement::Token(token) if token.kind() == Kind::LinkLabelOpenToken => Some(token),
        _ => None,
    })?;
    let close = node.children().find_map(|child| match child {
        SyntaxElement::Token(token) if token.kind() == Kind::LinkLabelCloseToken => Some(token),
        _ => None,
    })?;
    TextRange::new(open.range().end(), close.range().start()).ok()
}

fn heading_level(node: &crate::SyntaxNode<crate::OkfMarkdownLanguage>) -> Result<u8, ParseError> {
    use crate::OkfMarkdownSyntaxKind as Kind;
    let marker = if node.kind() == Kind::AtxHeading {
        first_token(node, Kind::HeadingMarkerToken)
    } else {
        first_token(node, Kind::SetextUnderlineToken)
    }
    .ok_or_else(|| invariant("Markdown heading has no marker"))?;
    let value = marker.text().write_to_string();
    if node.kind() == Kind::AtxHeading {
        u8::try_from(value.bytes().take_while(|byte| *byte == b'#').count())
            .map_err(|_| invariant("Markdown heading level is too large"))
    } else if value.trim_start().starts_with('=') {
        Ok(1)
    } else {
        Ok(2)
    }
}

fn collect_spans(
    node: &crate::SyntaxNode<crate::OkfMarkdownLanguage>,
    inherited: Option<SyntaxIdentity>,
    out: &mut Vec<MarkdownSyntaxSpan>,
) -> Result<(), ParseError> {
    if node.kind() == crate::OkfMarkdownSyntaxKind::Root {
        let children: Vec<_> = node.children().collect();
        let mut previous_owner = None;
        for (index, child) in children.iter().enumerate() {
            match child {
                SyntaxElement::Node(child) => {
                    collect_spans(child, None, out)?;
                    previous_owner = first_semantic_identity(child);
                }
                SyntaxElement::Token(token) if token.range().start() < token.range().end() => {
                    let owner = previous_owner.or_else(|| {
                        children[index + 1..].iter().find_map(|child| match child {
                            SyntaxElement::Node(child) => first_semantic_identity(child),
                            SyntaxElement::Token(_) => None,
                        })
                    });
                    if let Some(owner) = owner {
                        out.push(MarkdownSyntaxSpan {
                            owner,
                            range: token.range(),
                            source_role: source_role(token.kind()),
                            semantic_role: token_semantic_role(
                                token.kind(),
                                MarkdownSemanticRole::Document,
                            ),
                        });
                    }
                }
                SyntaxElement::Token(_) => {}
            }
        }
        return Ok(());
    }
    let own = identity(node);
    let Some(owner) = own.or(inherited) else {
        for child in node.children() {
            if let SyntaxElement::Node(child) = child {
                collect_spans(&child, None, out)?;
            }
        }
        return Ok(());
    };
    let role = semantic_role(node.kind());
    for child in node.children() {
        match child {
            SyntaxElement::Node(child) => collect_spans(&child, Some(owner), out)?,
            SyntaxElement::Token(token) if token.range().start() < token.range().end() => {
                out.push(MarkdownSyntaxSpan {
                    owner,
                    range: token.range(),
                    source_role: source_role(token.kind()),
                    semantic_role: token_semantic_role(token.kind(), role),
                })
            }
            SyntaxElement::Token(_) => {}
        }
    }
    Ok(())
}

fn first_semantic_identity(
    node: &crate::SyntaxNode<crate::OkfMarkdownLanguage>,
) -> Option<SyntaxIdentity> {
    identity(node).or_else(|| {
        node.children().find_map(|child| match child {
            SyntaxElement::Node(child) => first_semantic_identity(&child),
            SyntaxElement::Token(_) => None,
        })
    })
}

fn source_role(kind: crate::OkfMarkdownSyntaxKind) -> MarkdownSourceRole {
    use crate::OkfMarkdownSyntaxKind::*;
    match kind {
        TextToken
        | CodeTextToken
        | FrontmatterValueToken
        | FrontmatterKeyToken
        | FrontmatterQuotedValueToken
        | HtmlToken
        | EntityToken
        | InfoStringToken => MarkdownSourceRole::Content,
        _ => MarkdownSourceRole::SyntaxMarker,
    }
}

fn token_semantic_role(
    kind: crate::OkfMarkdownSyntaxKind,
    parent: MarkdownSemanticRole,
) -> MarkdownSemanticRole {
    use crate::OkfMarkdownSyntaxKind as Kind;
    match kind {
        Kind::TextToken => MarkdownSemanticRole::Text,
        Kind::TaskListMarkerToken => MarkdownSemanticRole::TaskMarker,
        Kind::WhitespaceToken | Kind::NewlineToken => MarkdownSemanticRole::Whitespace,
        Kind::BadToken => MarkdownSemanticRole::Recovery,
        _ => parent,
    }
}

fn semantic_role(kind: crate::OkfMarkdownSyntaxKind) -> MarkdownSemanticRole {
    use crate::OkfMarkdownSyntaxKind as K;
    match kind {
        K::Root => MarkdownSemanticRole::Document,
        K::Frontmatter
        | K::FrontmatterEntry
        | K::FrontmatterMapping
        | K::FrontmatterSequence
        | K::FrontmatterSequenceItem => MarkdownSemanticRole::Frontmatter,
        K::BlockQuote => MarkdownSemanticRole::BlockQuote,
        K::List => MarkdownSemanticRole::List,
        K::ListItem => MarkdownSemanticRole::ListItem,
        K::Paragraph => MarkdownSemanticRole::Paragraph,
        K::AtxHeading | K::SetextHeading => MarkdownSemanticRole::Heading,
        K::ThematicBreak => MarkdownSemanticRole::ThematicBreak,
        K::IndentedCodeBlock => MarkdownSemanticRole::IndentedCode,
        K::FencedCodeBlock => MarkdownSemanticRole::FencedCode,
        K::HtmlBlock => MarkdownSemanticRole::HtmlBlock,
        K::LinkReferenceDefinition => MarkdownSemanticRole::LinkDefinition,
        K::Table => MarkdownSemanticRole::Table,
        K::TableHead => MarkdownSemanticRole::TableHead,
        K::TableBody => MarkdownSemanticRole::TableBody,
        K::TableRow => MarkdownSemanticRole::TableRow,
        K::TableCell => MarkdownSemanticRole::TableCell,
        K::Text => MarkdownSemanticRole::Text,
        K::Escape => MarkdownSemanticRole::Escape,
        K::Entity => MarkdownSemanticRole::Entity,
        K::CodeSpan => MarkdownSemanticRole::CodeSpan,
        K::Emphasis => MarkdownSemanticRole::Emphasis,
        K::StrongEmphasis => MarkdownSemanticRole::Strong,
        K::Strikethrough => MarkdownSemanticRole::Strikethrough,
        K::Link => MarkdownSemanticRole::Link,
        K::Image => MarkdownSemanticRole::Image,
        K::Autolink => MarkdownSemanticRole::Autolink,
        K::RawHtml => MarkdownSemanticRole::RawHtml,
        K::SoftLineBreak => MarkdownSemanticRole::SoftBreak,
        K::HardLineBreak => MarkdownSemanticRole::HardBreak,
        K::SkippedTokensSyntax => MarkdownSemanticRole::Recovery,
        K::WamlSection => MarkdownSemanticRole::WamlSection,
        _ => MarkdownSemanticRole::Whitespace,
    }
}

fn required_annotation(
    annotations: &[crate::SyntaxAnnotation],
    kind: &str,
    description: &'static str,
) -> Result<Arc<str>, ParseError> {
    super::inline::link_annotation(annotations, kind)
        .map(Arc::from)
        .ok_or_else(|| ParseError::StructuralInvariant {
            reason: format!("semantic Markdown node has no {description}").into(),
        })
}

fn parse_annotation_range(value: Arc<str>) -> Result<TextRange, ParseError> {
    let Some((start, end)) = value.split_once(':') else {
        return Err(ParseError::StructuralInvariant {
            reason: "semantic Markdown link has invalid destination range".into(),
        });
    };
    let start = start
        .parse::<usize>()
        .map_err(|_| ParseError::StructuralInvariant {
            reason: "semantic Markdown link has invalid destination start".into(),
        })?;
    let end = end
        .parse::<usize>()
        .map_err(|_| ParseError::StructuralInvariant {
            reason: "semantic Markdown link has invalid destination end".into(),
        })?;
    TextRange::new(
        crate::TextSize::try_from_usize(start)
            .map_err(|_| ParseError::SourceTooLarge { bytes: start })?,
        crate::TextSize::try_from_usize(end)
            .map_err(|_| ParseError::SourceTooLarge { bytes: end })?,
    )
    .map_err(|_| ParseError::StructuralInvariant {
        reason: "semantic Markdown link has reversed destination range".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GreenElement, GreenFactory, OkfMarkdownSyntaxKind as Kind};

    #[test]
    fn query_index_rejects_semantic_node_without_identity() {
        let factory = GreenFactory::<OkfMarkdownLanguage>::new();
        let semantic = factory.node(Kind::Text, []).unwrap();
        let root = factory
            .node(Kind::Root, [GreenElement::Node(semantic)])
            .unwrap();
        let tree = SyntaxTree::new(root, Arc::from([]), MarkdownDialect::WAML_DEFAULT);

        assert!(matches!(
            queries(&tree, Arc::from([])),
            Err(ParseError::StructuralInvariant { .. })
        ));
    }
}
