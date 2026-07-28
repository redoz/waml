use std::sync::Arc;

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::{MarkdownDialect, SourceText, TextRange, TextSize};

use crate::shell::ParseError;

#[derive(Clone, Debug)]
pub struct ConfirmedHeading {
    pub level: u8,
    pub range: TextRange,
    pub text_range: TextRange,
}

#[derive(Clone, Debug)]
pub struct MarkdownStructureMap {
    pub headings: Arc<[ConfirmedHeading]>,
    pub protected_ranges: Arc<[TextRange]>,
    /// Exact source lines whose bullets CommonMark confirmed as top-level list
    /// items.
    /// Kept separately from normalized protected ranges so nested opaque blocks
    /// cannot inherit list-item status from an overlapping outer list.
    pub list_item_lines: Arc<[TextRange]>,
    /// Task-language-compatible tab-indented bullet lines. CommonMark treats
    /// these as indented code, so they remain explicit rather than being
    /// conflated with confirmed list items.
    pub tab_indented_item_lines: Arc<[TextRange]>,
    /// Protected leaf/container ranges that must remain opaque to embedded
    /// language parsers. Outer List/Item ownership is deliberately omitted;
    /// nested code and other protected content remains represented here.
    pub opaque_ranges: Arc<[TextRange]>,
    pub dialect: MarkdownDialect,
}

/// Maps CommonMark block structure without making any OKF-specific claims.
/// Container depth is deliberately tracked independently of pulldown's event
/// ranges: a heading inside a quote/list/code/HTML container must never become
/// a shell boundary.
pub(crate) fn map(
    text: &SourceText,
    dialect: MarkdownDialect,
) -> Result<MarkdownStructureMap, ParseError> {
    let source = text.shared();
    let len = source.len();
    let _ = size(len)?;
    let frontmatter_end = initial_frontmatter_end(source);
    let mut headings = Vec::new();
    let mut protected = Vec::new();
    let mut list_item_lines = Vec::new();
    let mut tab_indented_item_lines = Vec::new();
    let mut containers: Vec<usize> = Vec::new();
    let mut opaque_starts: Vec<Option<usize>> = Vec::new();
    let mut opaque = Vec::new();
    let mut pending: Option<(u8, usize, usize)> = None;

    for (event, offsets) in Parser::new_ext(source, Options::all()).into_offset_iter() {
        let start = offsets.start;
        let end = offsets.end;
        if start < frontmatter_end {
            continue;
        }
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    let level = heading_level(level);
                    if containers.is_empty() {
                        pending = Some((level, start, end));
                    }
                }
                Tag::Item => {
                    if containers.len() == 1 {
                        let line_start = line_start(source, start);
                        let line_end = line_end(source, line_start).unwrap_or(len);
                        list_item_lines.push(range(line_start, line_end)?);
                    }
                    containers.push(start);
                    opaque_starts.push(None);
                }
                tag if protects(&tag) => {
                    if matches!(&tag, Tag::CodeBlock(CodeBlockKind::Indented))
                        && containers.is_empty()
                    {
                        collect_tab_indented_items(
                            source,
                            start,
                            end,
                            &mut tab_indented_item_lines,
                        )?;
                    }
                    let is_opaque = opaque_container(&tag);
                    containers.push(start);
                    opaque_starts.push(is_opaque.then_some(start));
                }
                _ => {}
            },
            Event::End(end_tag) => match end_tag {
                TagEnd::Heading(level) => {
                    if let Some((expected, heading_start, heading_end)) = pending.take() {
                        if expected == heading_level(level) && containers.is_empty() {
                            let heading_end = heading_end.max(end).min(len);
                            let text_start = heading_text_start(source, heading_start, heading_end);
                            headings.push(ConfirmedHeading {
                                level: expected,
                                range: range(heading_start, heading_end)?,
                                text_range: range(text_start, heading_end)?,
                            });
                        }
                    }
                }
                end_tag if protects_end(end_tag) => {
                    if let Some(container_start) = containers.pop() {
                        protected.push(range(container_start, end.max(container_start).min(len))?);
                    }
                    if let Some(Some(opaque_start)) = opaque_starts.pop() {
                        opaque.push(range(opaque_start, end.max(opaque_start).min(len))?);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
    // Parser offsets are ordered, but sorting/merging makes this an explicit API
    // invariant for callers and for malformed nested Markdown.
    let normalized = normalize(protected);
    let opaque = normalize(opaque);
    headings.sort_by_key(|h| h.range.start());
    list_item_lines.sort_by_key(|range| (range.start(), range.end()));
    list_item_lines.dedup();
    tab_indented_item_lines.sort_by_key(|range| (range.start(), range.end()));
    tab_indented_item_lines.dedup();
    Ok(MarkdownStructureMap {
        headings: headings.into(),
        protected_ranges: normalized.into(),
        list_item_lines: list_item_lines.into(),
        tab_indented_item_lines: tab_indented_item_lines.into(),
        opaque_ranges: opaque.into(),
        dialect,
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

fn initial_frontmatter_end(source: &str) -> usize {
    let bom = usize::from(source.starts_with('\u{feff}')) * 3;
    let Some(first_end) = line_end(source, bom) else {
        return 0;
    };
    if source[bom..first_end].trim_end_matches(['\r', '\n']) != "---" {
        return 0;
    }
    let mut at = first_end;
    while let Some(end) = line_end(source, at) {
        if matches!(
            source[at..end].trim_end_matches(['\r', '\n']),
            "---" | "..."
        ) {
            return end;
        }
        at = end;
    }
    0
}
fn line_end(source: &str, start: usize) -> Option<usize> {
    (start < source.len()).then(|| {
        source[start..]
            .find('\n')
            .map(|n| start + n + 1)
            .unwrap_or(source.len())
    })
}
fn line_start(source: &str, offset: usize) -> usize {
    source[..offset.min(source.len())]
        .rfind('\n')
        .map(|newline| newline + 1)
        .unwrap_or(0)
}
fn collect_tab_indented_items(
    source: &str,
    start: usize,
    end: usize,
    items: &mut Vec<TextRange>,
) -> Result<(), ParseError> {
    let mut at = line_start(source, start);
    while at < end {
        let next = line_end(source, at).unwrap_or(source.len()).min(end);
        if source[at..next]
            .strip_prefix('\t')
            .is_some_and(|line| line.starts_with('-'))
        {
            items.push(range(at, next)?);
        }
        at = next;
    }
    Ok(())
}

fn protects(tag: &Tag<'_>) -> bool {
    matches!(
        tag,
        Tag::BlockQuote(_)
            | Tag::CodeBlock(_)
            | Tag::HtmlBlock
            | Tag::List(_)
            | Tag::Item
            | Tag::FootnoteDefinition(_)
            | Tag::Table(_)
            | Tag::DefinitionList
            | Tag::DefinitionListDefinition
    )
}

fn opaque_container(tag: &Tag<'_>) -> bool {
    !matches!(tag, Tag::List(_) | Tag::Item)
}
fn protects_end(tag: TagEnd) -> bool {
    matches!(
        tag,
        TagEnd::BlockQuote(_)
            | TagEnd::CodeBlock
            | TagEnd::HtmlBlock
            | TagEnd::List(_)
            | TagEnd::Item
            | TagEnd::FootnoteDefinition
            | TagEnd::Table
            | TagEnd::DefinitionList
            | TagEnd::DefinitionListDefinition
    )
}
fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}
fn heading_text_start(source: &str, start: usize, end: usize) -> usize {
    let bytes = source.as_bytes();
    let mut at = start;
    while at < end && bytes[at] == b' ' {
        at += 1;
    }
    while at < end && bytes[at] == b'#' {
        at += 1;
    }
    if at < end && bytes[at] == b' ' {
        at += 1;
    }
    at
}
fn size(value: usize) -> Result<TextSize, ParseError> {
    TextSize::try_from_usize(value).map_err(|_| ParseError::SourceTooLarge { bytes: value })
}
fn range(start: usize, end: usize) -> Result<TextRange, ParseError> {
    TextRange::new(size(start)?, size(end)?).map_err(|_| ParseError::StructuralInvariant {
        reason: "mapper produced a reversed range".into(),
    })
}
