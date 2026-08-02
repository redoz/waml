use std::sync::Arc;

use crate::{
    MarkdownDialect, OkfMarkdownLanguage, OkfMarkdownSyntaxKind as Kind, ParseError, SyntaxElement,
    SyntaxIdentity, SyntaxNode, SyntaxTree, TextRange, TextSize,
};

#[derive(Clone, Debug)]
pub struct ConfirmedHeading {
    pub level: u8,
    pub range: TextRange,
    pub text_range: TextRange,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum WamlSectionKind {
    Attributes,
    Values,
    Slots,
    Relationships,
    Members,
    Layout,
    Nodes,
    Lifelines,
    Gates,
    Messages,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WamlLanguageIsland {
    pub owner: SyntaxIdentity,
    pub kind: WamlSectionKind,
    pub heading_range: TextRange,
    pub content_range: TextRange,
}

#[derive(Clone, Debug)]
pub struct MarkdownStructureMap {
    pub headings: Arc<[ConfirmedHeading]>,
    pub nested_headings: Arc<[ConfirmedHeading]>,
    pub protected_ranges: Arc<[TextRange]>,
    pub list_item_lines: Arc<[TextRange]>,
    pub tab_indented_item_lines: Arc<[TextRange]>,
    pub opaque_ranges: Arc<[TextRange]>,
    pub dialect: MarkdownDialect,
    pub islands: Arc<[WamlLanguageIsland]>,
}

impl MarkdownStructureMap {
    pub fn local_for_island(
        &self,
        owner: SyntaxIdentity,
        content_range: TextRange,
    ) -> Option<(TextRange, Self)> {
        let island = self
            .islands
            .iter()
            .find(|island| island.owner == owner && island.content_range == content_range)?;
        let source_range =
            TextRange::new(island.heading_range.start(), island.content_range.end()).ok()?;
        let ranges = |source: &[TextRange]| {
            source
                .iter()
                .filter_map(|range| local_range(*range, source_range))
                .collect::<Arc<[_]>>()
        };
        let heading_range = local_range(island.heading_range, source_range)?;
        let content_range = local_range(island.content_range, source_range)?;
        let local = Self {
            headings: self
                .headings
                .iter()
                .filter_map(|heading| local_heading(heading, source_range))
                .collect(),
            nested_headings: self
                .nested_headings
                .iter()
                .filter_map(|heading| local_heading(heading, source_range))
                .collect(),
            protected_ranges: ranges(&self.protected_ranges),
            list_item_lines: ranges(&self.list_item_lines),
            tab_indented_item_lines: ranges(&self.tab_indented_item_lines),
            opaque_ranges: ranges(&self.opaque_ranges),
            dialect: self.dialect,
            islands: Arc::from([WamlLanguageIsland {
                owner: island.owner,
                kind: island.kind,
                heading_range,
                content_range,
            }]),
        };
        Some((source_range, local))
    }
}

fn local_range(range: TextRange, source_range: TextRange) -> Option<TextRange> {
    if range.start() < source_range.start() || range.end() > source_range.end() {
        return None;
    }
    let offset = source_range.start().to_usize();
    TextRange::new(
        TextSize::try_from_usize(range.start().to_usize().checked_sub(offset)?).ok()?,
        TextSize::try_from_usize(range.end().to_usize().checked_sub(offset)?).ok()?,
    )
    .ok()
}

fn local_heading(heading: &ConfirmedHeading, source_range: TextRange) -> Option<ConfirmedHeading> {
    Some(ConfirmedHeading {
        level: heading.level,
        range: local_range(heading.range, source_range)?,
        text_range: local_range(heading.text_range, source_range)?,
    })
}

pub(crate) fn from_tree(
    tree: &SyntaxTree<OkfMarkdownLanguage>,
    source: &str,
) -> Result<MarkdownStructureMap, ParseError> {
    let mut result = Collector {
        collect_headings: tree.dialect().waml_sections(),
        ..Collector::default()
    };
    visit(&tree.root(), 0, source, &mut result)?;
    result.headings.sort_by_key(|heading| heading.range.start());
    result
        .nested_headings
        .sort_by_key(|heading| heading.range.start());
    result
        .list_item_lines
        .sort_by_key(|range| (range.start(), range.end()));
    result.list_item_lines.dedup();
    result
        .tab_indented_item_lines
        .sort_by_key(|range| (range.start(), range.end()));
    result.tab_indented_item_lines.dedup();
    result
        .islands
        .sort_by_key(|island| island.heading_range.start());
    Ok(MarkdownStructureMap {
        headings: result.headings.into(),
        nested_headings: result.nested_headings.into(),
        protected_ranges: normalize(result.protected).into(),
        list_item_lines: result.list_item_lines.into(),
        tab_indented_item_lines: result.tab_indented_item_lines.into(),
        opaque_ranges: normalize(result.opaque).into(),
        dialect: tree.dialect(),
        islands: result.islands.into(),
    })
}

#[derive(Default)]
struct Collector {
    collect_headings: bool,
    headings: Vec<ConfirmedHeading>,
    nested_headings: Vec<ConfirmedHeading>,
    protected: Vec<TextRange>,
    list_item_lines: Vec<TextRange>,
    tab_indented_item_lines: Vec<TextRange>,
    opaque: Vec<TextRange>,
    islands: Vec<WamlLanguageIsland>,
}

fn visit(
    node: &SyntaxNode<OkfMarkdownLanguage>,
    container_depth: usize,
    source: &str,
    out: &mut Collector,
) -> Result<(), ParseError> {
    let kind = node.kind();
    let protected = protects(kind) || protected_paragraph(node, source);
    if protected {
        out.protected.push(node.range());
        if !matches!(kind, Kind::List | Kind::ListItem) {
            out.opaque.push(node.range());
        }
    }
    if kind == Kind::ListItem && container_depth == 1 {
        let start = line_start(source, node.range().start().to_usize());
        out.list_item_lines
            .push(range(start, line_end(source, start))?);
    }
    if kind == Kind::IndentedCodeBlock && container_depth == 0 {
        collect_tab_indented_items(source, node.range(), &mut out.tab_indented_item_lines)?;
    }
    if out.collect_headings
        && container_depth == 0
        && matches!(kind, Kind::AtxHeading | Kind::SetextHeading)
    {
        let heading = heading(source, node.range())?;
        if heading.level <= 2 {
            out.headings.push(heading);
        } else {
            out.nested_headings.push(heading);
        }
    }
    if kind == Kind::WamlSection {
        let heading = node
            .children()
            .find_map(|child| match child {
                SyntaxElement::Node(child)
                    if matches!(child.kind(), Kind::AtxHeading | Kind::SetextHeading) =>
                {
                    Some(heading(source, child.range()))
                }
                _ => None,
            })
            .transpose()?
            .ok_or_else(|| ParseError::StructuralInvariant {
                reason: "WAML section has no heading child".into(),
            })?;
        let owner = identity(node)?;
        let section_end = node.range().end();
        out.islands.push(WamlLanguageIsland {
            owner,
            kind: waml_kind(source, heading.text_range).ok_or_else(|| {
                ParseError::StructuralInvariant {
                    reason: "WAML section has an unrecognized heading".into(),
                }
            })?,
            heading_range: heading.range,
            content_range: TextRange::new(heading.range.end(), section_end).map_err(|_| {
                ParseError::StructuralInvariant {
                    reason: "WAML section content range is reversed".into(),
                }
            })?,
        });
    }
    let child_container_depth = container_depth + usize::from(protected);
    for child in node.children() {
        if let SyntaxElement::Node(child) = child {
            visit(&child, child_container_depth, source, out)?;
        }
    }
    Ok(())
}

fn heading(source: &str, heading_range: TextRange) -> Result<ConfirmedHeading, ParseError> {
    let start = heading_range.start().to_usize();
    let end = heading_range.end().to_usize();
    let line_end = line_end(source, start).min(end);
    let line = &source[start..line_end];
    let trimmed = line.trim_start();
    let hashes = trimmed.bytes().take_while(|byte| *byte == b'#').count();
    let level = if hashes > 0 { hashes as u8 } else { 2 };
    let text_start = if hashes > 0 {
        start
            + (line.len() - trimmed.len())
            + hashes
            + usize::from(trimmed.as_bytes().get(hashes) == Some(&b' '))
    } else {
        start
    };
    Ok(ConfirmedHeading {
        level,
        range: heading_range,
        text_range: range(text_start, line_end)?,
    })
}

pub(crate) fn waml_kind(source: &str, text_range: TextRange) -> Option<WamlSectionKind> {
    let value = source[text_range.start().to_usize()..text_range.end().to_usize()]
        .trim_end_matches(['\r', '\n'])
        .trim_end_matches('#')
        .trim();
    match value {
        "Attributes" => Some(WamlSectionKind::Attributes),
        "Values" => Some(WamlSectionKind::Values),
        "Slots" => Some(WamlSectionKind::Slots),
        "Relationships" => Some(WamlSectionKind::Relationships),
        "Members" => Some(WamlSectionKind::Members),
        "Layout" => Some(WamlSectionKind::Layout),
        "Nodes" => Some(WamlSectionKind::Nodes),
        "Lifelines" => Some(WamlSectionKind::Lifelines),
        "Gates" => Some(WamlSectionKind::Gates),
        "Messages" => Some(WamlSectionKind::Messages),
        _ => None,
    }
}

fn identity(node: &SyntaxNode<OkfMarkdownLanguage>) -> Result<SyntaxIdentity, ParseError> {
    node.syntax_annotations()
        .iter()
        .find(|annotation| annotation.kind() == "waml.markdown.identity")
        .and_then(|annotation| annotation.data())
        .and_then(SyntaxIdentity::from_annotation_data)
        .ok_or_else(|| ParseError::StructuralInvariant {
            reason: "WAML section has no syntax identity".into(),
        })
}

fn protects(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::BlockQuote
            | Kind::List
            | Kind::ListItem
            | Kind::IndentedCodeBlock
            | Kind::FencedCodeBlock
            | Kind::HtmlBlock
            | Kind::Table
    )
}

fn protected_paragraph(node: &SyntaxNode<OkfMarkdownLanguage>, source: &str) -> bool {
    let value = &source[node.range().start().to_usize()..node.range().end().to_usize()];
    ((node.kind() == Kind::LinkReferenceDefinition || node.kind() == Kind::Paragraph)
        && value.starts_with("[^"))
        || (node.kind() == Kind::Paragraph
            && value
                .lines()
                .nth(1)
                .is_some_and(|line| line.starts_with(':')))
}

fn collect_tab_indented_items(
    source: &str,
    code_range: TextRange,
    out: &mut Vec<TextRange>,
) -> Result<(), ParseError> {
    let mut at = line_start(source, code_range.start().to_usize());
    let end = code_range.end().to_usize();
    while at < end {
        let next = line_end(source, at).min(end);
        if source[at..next]
            .strip_prefix('\t')
            .is_some_and(|line| line.starts_with('-'))
        {
            out.push(range(at, next)?);
        }
        at = next;
    }
    Ok(())
}

fn line_start(source: &str, offset: usize) -> usize {
    source[..offset.min(source.len())]
        .rfind('\n')
        .map_or(0, |at| at + 1)
}

fn line_end(source: &str, start: usize) -> usize {
    source[start..]
        .find('\n')
        .map_or(source.len(), |at| start + at + 1)
}

fn range(start: usize, end: usize) -> Result<TextRange, ParseError> {
    TextRange::new(
        TextSize::try_from_usize(start).map_err(|_| ParseError::SourceTooLarge { bytes: start })?,
        TextSize::try_from_usize(end).map_err(|_| ParseError::SourceTooLarge { bytes: end })?,
    )
    .map_err(|_| ParseError::StructuralInvariant {
        reason: "projection produced a reversed range".into(),
    })
}

fn normalize(mut ranges: Vec<TextRange>) -> Vec<TextRange> {
    ranges.sort_by_key(|range| (range.start(), range.end()));
    let mut normalized: Vec<TextRange> = Vec::new();
    for current in ranges {
        if let Some(previous) = normalized.last_mut() {
            if current.start() <= previous.end() {
                *previous = previous.cover(current);
                continue;
            }
        }
        normalized.push(current);
    }
    normalized
}
