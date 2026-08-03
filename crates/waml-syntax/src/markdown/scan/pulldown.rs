//! The one and only place in this tree that knows pulldown-cmark exists.
//!
//! `tests/scan_seam.rs` fails the build if `pulldown_cmark` appears in any
//! other file under `src/markdown/`.

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag};

use crate::MarkdownDialect;

use super::{BlockScan, ScanAlignment, ScanEvent, ScanProfile, ScanTag, ScanTagKind};

/// The tree profile opts *in* to the GFM constructs the dialect enables.
fn tree_options(dialect: MarkdownDialect) -> Options {
    let mut options = Options::empty();
    if dialect.tables() {
        options.insert(Options::ENABLE_TABLES);
    }
    if dialect.strikethrough() {
        options.insert(Options::ENABLE_STRIKETHROUGH);
    }
    if dialect.task_lists() {
        options.insert(Options::ENABLE_TASKLISTS);
    }
    options
}

/// The shell profile starts from everything the parser can see and opts *out*.
///
/// The shell structure contract protects every construct the parser can
/// identify, which is deliberately wider than what the syntax tree represents.
fn shell_options(dialect: MarkdownDialect) -> Options {
    let mut options = Options::all();
    if !dialect.tables() {
        options.remove(Options::ENABLE_TABLES);
    }
    if !dialect.task_lists() {
        options.remove(Options::ENABLE_TASKLISTS);
    }
    if !dialect.strikethrough() {
        options.remove(Options::ENABLE_STRIKETHROUGH);
    }
    options
}

fn options(dialect: MarkdownDialect, profile: ScanProfile) -> Options {
    match profile {
        ScanProfile::Tree => tree_options(dialect),
        ScanProfile::Shell => shell_options(dialect),
    }
}

fn alignment(value: Alignment) -> ScanAlignment {
    match value {
        Alignment::None => ScanAlignment::None,
        Alignment::Left => ScanAlignment::Left,
        Alignment::Center => ScanAlignment::Center,
        Alignment::Right => ScanAlignment::Right,
    }
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

/// Maps an opening tag, or `None` for constructs the scan vocabulary does not
/// report (every inline tag, metadata blocks, definition-list titles, ...).
fn start_tag(tag: Tag<'_>) -> Option<ScanTag> {
    Some(match tag {
        Tag::Paragraph => ScanTag::Paragraph,
        Tag::Heading { level, .. } => ScanTag::Heading {
            level: heading_level(level),
        },
        Tag::BlockQuote(_) => ScanTag::BlockQuote,
        Tag::CodeBlock(CodeBlockKind::Indented) => ScanTag::IndentedCodeBlock,
        Tag::CodeBlock(CodeBlockKind::Fenced(_)) => ScanTag::FencedCodeBlock,
        Tag::HtmlBlock => ScanTag::HtmlBlock,
        Tag::List(_) => ScanTag::List,
        Tag::Item => ScanTag::Item,
        Tag::Table(alignments) => ScanTag::Table {
            alignments: alignments.into_iter().map(alignment).collect(),
        },
        Tag::TableHead => ScanTag::TableHead,
        Tag::TableRow => ScanTag::TableRow,
        Tag::TableCell => ScanTag::TableCell,
        Tag::FootnoteDefinition(_) => ScanTag::FootnoteDefinition,
        Tag::DefinitionList => ScanTag::DefinitionList,
        Tag::DefinitionListDefinition => ScanTag::DefinitionListDefinition,
        _ => return None,
    })
}

pub(crate) fn scan_blocks(
    source: &str,
    dialect: MarkdownDialect,
    profile: ScanProfile,
) -> BlockScan {
    let parser = Parser::new_ext(source, options(dialect, profile));

    // Only the tree profile reads these, and collecting them clones a range per
    // link reference, so the shell profile skips the work entirely. Must be read
    // before `into_offset_iter` consumes the parser. Order is the parser's own;
    // callers validate before sorting and that order matters.
    let reference_definitions: Vec<std::ops::Range<usize>> = match profile {
        ScanProfile::Tree => parser
            .reference_definitions()
            .iter()
            .map(|(_, definition)| definition.span.clone())
            .collect(),
        ScanProfile::Shell => Vec::new(),
    };

    let mut events = Vec::new();
    // One slot per open tag. `None` marks a construct the vocabulary drops, so
    // its end is dropped too and the stream stays balanced.
    let mut open: Vec<Option<ScanTagKind>> = Vec::new();
    for (event, range) in parser.into_offset_iter() {
        match event {
            Event::Start(tag) => {
                let mapped = start_tag(tag);
                open.push(mapped.as_ref().map(ScanTag::kind));
                if let Some(tag) = mapped {
                    events.push((ScanEvent::Start(tag), range));
                }
            }
            Event::End(_) => {
                if let Some(Some(kind)) = open.pop() {
                    events.push((ScanEvent::End(kind), range));
                }
            }
            Event::Rule => events.push((ScanEvent::Rule, range)),
            _ => {}
        }
    }

    BlockScan {
        events,
        reference_definitions,
    }
}

/// Concatenates the text the parser decodes from `spelling`, entities resolved.
pub(crate) fn scan_text_entities(spelling: &str) -> String {
    let mut text = String::new();
    for event in Parser::new(spelling) {
        if let Event::Text(value) = event {
            text.push_str(&value);
        }
    }
    text
}

/// Whether `candidate` — angle brackets included — is a raw HTML tag.
pub(crate) fn scan_is_inline_html(candidate: &str) -> bool {
    Parser::new(candidate).any(
        |event| matches!(event, Event::InlineHtml(html) | Event::Html(html) if html.as_ref() == candidate),
    )
}
