use std::sync::Arc;

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

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
    let mut containers: Vec<usize> = Vec::new();
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
                    if containers.is_empty() && level <= 2 {
                        pending = Some((level, start, end));
                    }
                }
                tag if protects(&tag) => containers.push(start),
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
                }
                _ => {}
            },
            _ => {}
        }
    }
    // Parser offsets are ordered, but sorting/merging makes this an explicit API
    // invariant for callers and for malformed nested Markdown.
    protected.sort_by_key(|r| (r.start(), r.end()));
    let mut normalized: Vec<TextRange> = Vec::new();
    for current in protected {
        if let Some(previous) = normalized.last_mut() {
            if current.start() <= previous.end() {
                *previous = previous.cover(current);
                continue;
            }
        }
        normalized.push(current);
    }
    headings.sort_by_key(|h| h.range.start());
    Ok(MarkdownStructureMap {
        headings: headings.into(),
        protected_ranges: normalized.into(),
        dialect,
    })
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
