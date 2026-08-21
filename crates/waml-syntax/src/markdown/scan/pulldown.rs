//! The one and only place in this tree that knows pulldown-cmark exists.
//!
//! `tests/scan_seam.rs` fails the build if `pulldown_cmark` appears in any
//! other file under `src/markdown/`.

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag};

use crate::{markdown::reference::repeats_a_definition_label, MarkdownDialect};

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
    let options = options(dialect, profile);
    let parser = Parser::new_ext(source, options);

    // Only the tree profile reads these, and collecting them clones a range per
    // link reference, so the shell profile skips the work entirely. Must be read
    // before `into_offset_iter` consumes the parser. Order is the parser's own;
    // callers validate before sorting and that order matters.
    let first_of_each_label: Option<Vec<std::ops::Range<usize>>> = match profile {
        ScanProfile::Tree => Some(
            parser
                .reference_definitions()
                .iter()
                .map(|(_, definition)| definition.span.clone())
                .collect(),
        ),
        ScanProfile::Shell => None,
    };

    let mut events = Vec::new();
    let mut malformed_range = false;
    // Ranges of the content events this vocabulary drops. A link reference
    // definition produces none of them, so anything they cover is settled
    // text, and the repeat pass below refuses to read a definition back out of
    // it however the blanking rearranged the blocks around it.
    let mut content = Vec::new();
    // One slot per open tag. `None` marks a construct the vocabulary drops, so
    // its end is dropped too and the stream stays balanced.
    let mut open: Vec<Option<ScanTagKind>> = Vec::new();
    for (event, range) in parser.into_offset_iter() {
        // Screen *every* raw event, including the inline and text events the
        // vocabulary drops: a malformed range anywhere means the underlying
        // parser disagrees with us about the source, and callers fall back to
        // raw text rather than trust any of the stream.
        if !range_is_well_formed(source, &range) {
            malformed_range = true;
        }
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
            Event::Rule => {
                content.push(range.clone());
                events.push((ScanEvent::Rule, range));
            }
            _ => content.push(range),
        }
    }

    let reference_definitions = first_of_each_label
        .map(|spans| all_reference_definitions(source, options, spans, &content));

    BlockScan {
        events,
        reference_definitions,
        malformed_range,
    }
}

/// Grows `spans` — the first definition of each label — into the span of
/// *every* link reference definition in `source`.
///
/// `Parser::reference_definitions` is keyed by normalised label, so it reports
/// one definition per label and drops every repeat. A repeat is still a
/// definition: CommonMark parses it exactly like the first and only declines
/// to let it win the label, and it may run over as many lines. Leaving it out
/// left the tree builder to rebuild the repeat from a line-anchored fallback,
/// which stopped at the end of the line the label sat on — so `[id]: \nx` was
/// a definition with destination `x` when its label was fresh and a
/// destination-less definition trailed by a stray `x` when it was not.
///
/// Rescanning with the definitions found so far blanked out makes the next
/// repeat of each label the first of its own, so the same parser decides the
/// extent of every definition. Blanking keeps line breaks and every byte
/// offset, so each pass reads the same document with some blank lines in it.
///
/// A pass that finds nothing new ends the loop, and the blanking is what
/// guarantees that: found bytes can never be read as a definition again, so
/// each pass strictly grows the blanked region. The whole loop is skipped
/// unless the source repeats a definition label by the loosest reading, which
/// costs one byte pass rather than one parse.
fn all_reference_definitions(
    source: &str,
    options: Options,
    spans: Vec<std::ops::Range<usize>>,
    content: &[std::ops::Range<usize>],
) -> Vec<std::ops::Range<usize>> {
    if spans.is_empty() || !repeats_a_definition_label(source) {
        return spans;
    }
    let mut masked = source.as_bytes().to_vec();
    let mut definitions = spans.clone();
    // Every span blanked so far, whether or not it was kept. A pass reads
    // definitions out of a document some of whose blocks the blanking has
    // rearranged, so it can name a span the real parse spends on paragraph
    // text; blanking that one anyway is what lets the next pass see past it,
    // since the table is keyed by label and one span per label is all it holds.
    let mut blanked = spans;
    let mut pending: Vec<std::ops::Range<usize>> = blanked.clone();
    loop {
        for span in pending.drain(..) {
            blank(&mut masked, &span);
        }
        let Ok(text) = std::str::from_utf8(&masked) else {
            // Blanking replaces whole spans with ASCII, so this cannot happen
            // for char-aligned spans; bail rather than assert on a range the
            // caller has not screened yet.
            return definitions;
        };
        for span in Parser::new_ext(text, options)
            .reference_definitions()
            .iter()
            .map(|(_, definition)| definition.span.clone())
        {
            if !range_is_well_formed(text, &span)
                || span.start >= span.end
                || blanked.iter().any(|seen| overlaps(seen, &span))
            {
                continue;
            }
            if !content.iter().any(|settled| overlaps(settled, &span)) {
                definitions.push(span.clone());
            }
            blanked.push(span.clone());
            pending.push(span);
        }
        if pending.is_empty() {
            return definitions;
        }
    }
}

/// Replaces `span` with spaces, leaving its line breaks where they were.
///
/// Keeping the breaks keeps every following line on the line it was on, so a
/// rescan of the blanked source reads the same blocks around it.
fn blank(masked: &mut [u8], span: &std::ops::Range<usize>) {
    let Some(bytes) = masked.get_mut(span.clone()) else {
        return;
    };
    for byte in bytes {
        if !matches!(byte, b'\n' | b'\r') {
            *byte = b' ';
        }
    }
}

fn overlaps(left: &std::ops::Range<usize>, right: &std::ops::Range<usize>) -> bool {
    left.start < right.end && right.start < left.end
}

/// Whether an event range is a usable, char-aligned slice of `source`.
fn range_is_well_formed(source: &str, range: &std::ops::Range<usize>) -> bool {
    range.start <= range.end
        && range.end <= source.len()
        && source.is_char_boundary(range.start)
        && source.is_char_boundary(range.end)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn well_formed_ranges_are_accepted() {
        assert!(range_is_well_formed("héllo", &(0..6)));
        assert!(range_is_well_formed("héllo", &(3..3)));
    }

    #[test]
    fn malformed_ranges_are_rejected() {
        // Inverted, past the end, and split across a multi-byte char.
        let inverted = std::ops::Range { start: 3, end: 1 };
        assert!(!range_is_well_formed("héllo", &inverted));
        assert!(!range_is_well_formed("héllo", &(0..7)));
        assert!(!range_is_well_formed("héllo", &(2..3)));
    }
}
