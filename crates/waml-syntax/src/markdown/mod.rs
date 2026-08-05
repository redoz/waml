use std::sync::Arc;

pub(crate) mod block;
pub(crate) mod gfm;
pub(crate) mod inline;
mod kind;
pub(crate) mod parser;
#[cfg(test)]
mod parser_tests;
mod projection;
pub(crate) mod reference;
pub(crate) mod reparse;
pub mod scalar;
pub(crate) mod scan;
mod snapshot;

pub use gfm::{HtmlTagFilter, TableAlignment, TaskListState};
pub use kind::{
    OkfMarkdownLanguage, OkfMarkdownSyntaxKind, OkfSyntaxDiagnosticCode, SyntaxIdentity,
};
pub(crate) use projection::{from_tree, waml_kind};
pub use projection::{ConfirmedHeading, MarkdownStructureMap, WamlLanguageIsland, WamlSectionKind};
pub use snapshot::{
    parse_markdown, reparse_markdown, FencedCodeInfo, MarkdownEntity, MarkdownHeading,
    MarkdownImage, MarkdownLink, MarkdownLinkKind, MarkdownList, MarkdownListKind, MarkdownRawHtml,
    MarkdownReparseOutcome, MarkdownSemanticRole, MarkdownSourceRole, MarkdownSyntaxQueries,
    MarkdownSyntaxSnapshot, MarkdownSyntaxSpan, MarkdownSyntaxUpdate, MarkdownTableCell,
};

use scan::{scan_blocks, ScanEvent, ScanProfile, ScanTag, ScanTagKind};

/// Maximum container nesting depth (block quotes, lists, list items, tables)
/// the block builder will materialize as real tree structure. Only container
/// frames count — leaf blocks (paragraphs, headings, code blocks) cannot
/// recurse — and note a nested bullet level costs two (`List` + `ListItem`),
/// so this is 64 block-quote levels but ~32 list levels. Beyond this,
/// `block.rs::parse_strict` stops pushing frames so every downstream
/// recursive consumer (`rebuild`, `projection::visit`, `collect_occurrences`,
/// `red::rewrite`) is bounded — sized well below the smallest stack this
/// parser runs on (wasm's default ~1 MB, with large recursive frames).
pub(crate) const MD_MAX_CONTAINER_DEPTH: usize = 64;

/// Maximum frontmatter block-structure nesting depth the parser will
/// materialize (see `parser.rs`), published so every consumer that walks the
/// resulting tree caps at the SAME depth. A consumer capping lower would read
/// nesting the parser accepted — and diagnosed as clean — back as `Null`, then
/// write that truncation into the user's document on the next save.
pub const FRONTMATTER_MAX_NESTING_DEPTH: usize = MD_MAX_CONTAINER_DEPTH;

/// Maximum inline nesting depth (emphasis, strikethrough, link labels, ...)
/// `inline.rs::parse_inlines` will recurse into. Beyond this, the
/// strikethrough/emphasis/link arms are skipped so the delimiters flush as
/// plain text instead of recursing again — independent of
/// `MD_MAX_CONTAINER_DEPTH` since inline nesting is unbounded even inside a
/// single paragraph.
pub(crate) const MD_MAX_INLINE_DEPTH: usize = 32;

use crate::{MarkdownDialect, SourceText, TextRange, TextSize};

use crate::shell::ParseError;

/// Maps CommonMark block structure without making any OKF-specific claims.
/// Container depth is deliberately tracked independently of the scan's event
/// ranges: a heading inside a quote/list/code/HTML container must never become
/// a shell boundary.
/// Internal data used only to synchronize shell parsing before the syntax tree
/// exists. The public `MarkdownStructureMap` is derived from that finished tree
/// in `projection`.
pub(crate) struct ShellStructure {
    pub headings: Arc<[ConfirmedHeading]>,
    pub nested_headings: Arc<[ConfirmedHeading]>,
    pub protected_ranges: Arc<[TextRange]>,
    pub list_item_lines: Arc<[TextRange]>,
    pub tab_indented_item_lines: Arc<[TextRange]>,
    pub opaque_ranges: Arc<[TextRange]>,
    pub dialect: MarkdownDialect,
}

pub(crate) fn shell_map(
    text: &SourceText,
    dialect: MarkdownDialect,
) -> Result<ShellStructure, ParseError> {
    let source = text.shared();
    let len = source.len();
    let _ = size(len)?;
    let frontmatter_end = if dialect.waml_frontmatter() {
        initial_frontmatter_end(source)
    } else {
        0
    };
    let mut headings = Vec::new();
    let mut nested_headings = Vec::new();
    let mut protected = Vec::new();
    let mut list_item_lines = Vec::new();
    let mut tab_indented_item_lines = Vec::new();
    let mut containers: Vec<usize> = Vec::new();
    let mut opaque_starts: Vec<Option<usize>> = Vec::new();
    let mut opaque = Vec::new();
    let mut pending: Option<(u8, usize, usize)> = None;

    for (event, offsets) in scan_blocks(source, dialect, ScanProfile::Shell).events {
        let start = offsets.start;
        let end = offsets.end;
        if start < frontmatter_end {
            continue;
        }
        match event {
            ScanEvent::Start(tag) => match tag {
                ScanTag::Heading { level } => {
                    if dialect.waml_sections() && containers.is_empty() {
                        pending = Some((level, start, end));
                    }
                }
                ScanTag::Item => {
                    if containers.len() == 1 {
                        let line_start = line_start(source, start);
                        let line_end = line_end(source, line_start).unwrap_or(len);
                        list_item_lines.push(range(line_start, line_end)?);
                    }
                    containers.push(start);
                    opaque_starts.push(None);
                }
                tag if protects(tag.kind()) => {
                    if tag.kind() == ScanTagKind::IndentedCodeBlock && containers.is_empty() {
                        collect_tab_indented_items(
                            source,
                            start,
                            end,
                            &mut tab_indented_item_lines,
                        )?;
                    }
                    let is_opaque = opaque_container(tag.kind());
                    containers.push(start);
                    opaque_starts.push(is_opaque.then_some(start));
                }
                _ => {}
            },
            ScanEvent::End(kind) => match kind {
                // The scan's open-tag stack guarantees this end closes the
                // heading that opened, so the level needs no re-check.
                ScanTagKind::Heading => {
                    if let Some((expected, heading_start, heading_end)) = pending.take() {
                        if containers.is_empty() {
                            let heading_end = heading_end.max(end).min(len);
                            let text_start = heading_text_start(source, heading_start, heading_end);
                            let heading = ConfirmedHeading {
                                level: expected,
                                range: range(heading_start, heading_end)?,
                                text_range: range(text_start, heading_end)?,
                            };
                            if expected <= 2 {
                                headings.push(heading);
                            } else {
                                nested_headings.push(heading);
                            }
                        }
                    }
                }
                kind if protects(kind) => {
                    if let Some(container_start) = containers.pop() {
                        protected.push(range(container_start, end.max(container_start).min(len))?);
                    }
                    if let Some(Some(opaque_start)) = opaque_starts.pop() {
                        opaque.push(range(opaque_start, end.max(opaque_start).min(len))?);
                    }
                }
                _ => {}
            },
            ScanEvent::Rule => {}
        }
    }
    // Parser offsets are ordered, but sorting/merging makes this an explicit API
    // invariant for callers and for malformed nested Markdown.
    let normalized = normalize(protected);
    let opaque = normalize(opaque);
    headings.sort_by_key(|h| h.range.start());
    nested_headings.sort_by_key(|h| h.range.start());
    list_item_lines.sort_by_key(|range| (range.start(), range.end()));
    list_item_lines.dedup();
    tab_indented_item_lines.sort_by_key(|range| (range.start(), range.end()));
    tab_indented_item_lines.dedup();
    Ok(ShellStructure {
        headings: headings.into(),
        nested_headings: nested_headings.into(),
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

/// Whether `source` opens with the frontmatter fence line the full parser
/// requires — the classifier's own open rule: the first line, minus trailing
/// horizontal whitespace and the line break, is exactly `---` at column 0
/// (an optional BOM aside). When this is `false` the full parser produces no
/// frontmatter at all, so a caller that only wants the frontmatter can stop
/// without parsing anything.
pub fn has_leading_frontmatter_fence(source: &str) -> bool {
    let bom = usize::from(source.starts_with('\u{feff}')) * 3;
    let Some(first_end) = line_end(source, bom) else {
        return false;
    };
    source[bom..first_end]
        .trim_end_matches(['\r', '\n'])
        .trim_end_matches([' ', '\t'])
        == "---"
}

/// Returns the leading frontmatter fence slice of `source` — from the opening
/// `---` through the matching closing `---`/`...` line, inclusive — but ONLY
/// when the block is unambiguous, meaning parsing this slice alone yields the
/// same frontmatter the full parser (`parser::classify_frontmatter`) would.
/// Cheap line scan, no tree built; callers that only need the frontmatter
/// (e.g. classifying a document's claimed `type` without a full parse) can
/// hand this small slice to [`crate::parse_markdown`].
///
/// `None` means "cannot prove it — parse the whole document", NEVER "there is
/// no frontmatter" (use [`has_leading_frontmatter_fence`] for that question).
/// This scan is deliberately stricter than the classifier, so every shape the
/// two could resolve differently bails out here instead of silently
/// disagreeing: a fence line carrying trailing whitespace or indentation
/// (the classifier trims both, this does not), a fence that the classifier
/// would skip because it sits inside a YAML block scalar, and an unclosed
/// block the classifier recovers via `plausible_unclosed_frontmatter`.
pub fn leading_frontmatter_slice(source: &str) -> Option<&str> {
    let bom = usize::from(source.starts_with('\u{feff}')) * 3;
    let first_end = line_end(source, bom)?;
    if source[bom..first_end].trim_end_matches(['\r', '\n']) != "---" {
        return None;
    }
    let mut at = first_end;
    while let Some(end) = line_end(source, at) {
        let content = source[at..end].trim_end_matches(['\r', '\n']);
        if matches!(content, "---" | "...") {
            return Some(&source[..end]);
        }
        // A fence the classifier would accept but this scan would not, and a
        // block scalar that could hide a fence from the classifier, both make
        // the two rules diverge — bail out and let the caller parse in full.
        // `|`/`>` anywhere is a deliberate over-approximation of a block
        // scalar header: cheap, and its only cost is a full parse.
        if matches!(content.trim(), "---" | "...") || content.contains('|') || content.contains('>')
        {
            return None;
        }
        at = end;
    }
    None
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

/// Containers whose interior must never yield a shell boundary.
///
/// Serves both directions: the scan reports a precise end kind, so the same
/// set decides which starts push a container and which ends pop one.
fn protects(kind: ScanTagKind) -> bool {
    matches!(
        kind,
        ScanTagKind::BlockQuote
            | ScanTagKind::IndentedCodeBlock
            | ScanTagKind::FencedCodeBlock
            | ScanTagKind::HtmlBlock
            | ScanTagKind::List
            | ScanTagKind::Item
            | ScanTagKind::FootnoteDefinition
            | ScanTagKind::Table
            | ScanTagKind::DefinitionList
            | ScanTagKind::DefinitionListDefinition
    )
}

fn opaque_container(kind: ScanTagKind) -> bool {
    !matches!(kind, ScanTagKind::List | ScanTagKind::Item)
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
