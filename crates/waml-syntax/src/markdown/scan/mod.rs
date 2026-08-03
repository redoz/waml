//! waml's own Markdown block-scan vocabulary.
//!
//! This module names the events the tree builder and the shell mapper need,
//! independently of any third-party parser. Exactly one implementation exists
//! today ([`pulldown`]), and it is the only file in the tree permitted to
//! reference the underlying markdown crate; `tests/scan_seam.rs` enforces that.
//!
//! Contract notes for any future implementation:
//!
//! * Offsets in [`BlockScan::events`] are byte ranges **relative to the `source`
//!   passed in**. Callers that scan a slice re-base them themselves.
//! * Inline-level constructs are not reported. An implementation that cannot
//!   report a construct must omit its start *and* its end, so the event stream
//!   stays balanced.
//! * [`ScanEvent::End`] names the exact kind that opened, including the
//!   indented/fenced code-block distinction.
//! * [`BlockScan::reference_definitions`] is returned in implementation order,
//!   unsorted. `block.rs` validates each span before sorting, and that order of
//!   operations is load-bearing for its recovery path. Only [`ScanProfile::Tree`]
//!   collects them; under [`ScanProfile::Shell`] the field is always empty,
//!   because the shell mapper never reads it.

mod pulldown;

use std::ops::Range;

pub(crate) use pulldown::{scan_blocks, scan_is_inline_html, scan_text_entities};

/// Which construct set the scan should recognise.
///
/// The two profiles are genuinely different and both are in use: the tree
/// builder opts *in* to the GFM constructs its dialect enables, while the shell
/// mapper starts from everything the parser can see and opts *out* of the same
/// three. The shell mapper protects more than the tree represents on purpose.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScanProfile {
    /// The narrow profile used to build the syntax tree.
    Tree,
    /// The wide profile used to map shell structure.
    Shell,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScanAlignment {
    None,
    Left,
    Center,
    Right,
}

/// A block tag identity with no payload, used for ends and set membership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScanTagKind {
    Paragraph,
    Heading,
    BlockQuote,
    IndentedCodeBlock,
    FencedCodeBlock,
    HtmlBlock,
    List,
    Item,
    Table,
    TableHead,
    TableRow,
    TableCell,
    FootnoteDefinition,
    DefinitionList,
    DefinitionListDefinition,
}

#[cfg(test)]
impl ScanTagKind {
    /// Every variant, for tests that must cover the whole vocabulary.
    ///
    /// When a variant is added, `ordinal` below stops compiling; extend this
    /// list too, and `all_covers_every_kind` checks the two agree.
    pub(crate) const ALL: &'static [ScanTagKind] = &[
        Self::Paragraph,
        Self::Heading,
        Self::BlockQuote,
        Self::IndentedCodeBlock,
        Self::FencedCodeBlock,
        Self::HtmlBlock,
        Self::List,
        Self::Item,
        Self::Table,
        Self::TableHead,
        Self::TableRow,
        Self::TableCell,
        Self::FootnoteDefinition,
        Self::DefinitionList,
        Self::DefinitionListDefinition,
    ];

    /// Declaration index. The exhaustive match is the tripwire for [`Self::ALL`].
    fn ordinal(self) -> usize {
        match self {
            Self::Paragraph => 0,
            Self::Heading => 1,
            Self::BlockQuote => 2,
            Self::IndentedCodeBlock => 3,
            Self::FencedCodeBlock => 4,
            Self::HtmlBlock => 5,
            Self::List => 6,
            Self::Item => 7,
            Self::Table => 8,
            Self::TableHead => 9,
            Self::TableRow => 10,
            Self::TableCell => 11,
            Self::FootnoteDefinition => 12,
            Self::DefinitionList => 13,
            Self::DefinitionListDefinition => 14,
        }
    }
}

/// A block tag opening, with the payload the consumers actually read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ScanTag {
    Paragraph,
    Heading { level: u8 },
    BlockQuote,
    IndentedCodeBlock,
    FencedCodeBlock,
    HtmlBlock,
    List,
    Item,
    Table { alignments: Vec<ScanAlignment> },
    TableHead,
    TableRow,
    TableCell,
    FootnoteDefinition,
    DefinitionList,
    DefinitionListDefinition,
}

impl ScanTag {
    pub(crate) fn kind(&self) -> ScanTagKind {
        match self {
            Self::Paragraph => ScanTagKind::Paragraph,
            Self::Heading { .. } => ScanTagKind::Heading,
            Self::BlockQuote => ScanTagKind::BlockQuote,
            Self::IndentedCodeBlock => ScanTagKind::IndentedCodeBlock,
            Self::FencedCodeBlock => ScanTagKind::FencedCodeBlock,
            Self::HtmlBlock => ScanTagKind::HtmlBlock,
            Self::List => ScanTagKind::List,
            Self::Item => ScanTagKind::Item,
            Self::Table { .. } => ScanTagKind::Table,
            Self::TableHead => ScanTagKind::TableHead,
            Self::TableRow => ScanTagKind::TableRow,
            Self::TableCell => ScanTagKind::TableCell,
            Self::FootnoteDefinition => ScanTagKind::FootnoteDefinition,
            Self::DefinitionList => ScanTagKind::DefinitionList,
            Self::DefinitionListDefinition => ScanTagKind::DefinitionListDefinition,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ScanEvent {
    Start(ScanTag),
    End(ScanTagKind),
    Rule,
}

/// One block scan of one source slice.
///
/// Eager rather than streaming: the whole event stream is materialised. That
/// costs one allocation proportional to the event count and buys a seam with no
/// lifetime plumbing, which matters because the reference definitions must be
/// read before the underlying parser is consumed. Documents here are
/// editor-sized; revisit only if `benches/markdown_parse.rs` says otherwise.
#[derive(Debug, Default)]
pub(crate) struct BlockScan {
    pub events: Vec<(ScanEvent, Range<usize>)>,
    pub reference_definitions: Vec<Range<usize>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MarkdownDialect;

    fn kinds(source: &str, profile: ScanProfile) -> Vec<ScanEvent> {
        scan_blocks(source, MarkdownDialect::WAML_DEFAULT, profile)
            .events
            .into_iter()
            .map(|(event, _)| event)
            .collect()
    }

    #[test]
    fn reports_a_paragraph_with_its_byte_range() {
        let scan = scan_blocks("hello\n", MarkdownDialect::WAML_DEFAULT, ScanProfile::Tree);
        assert_eq!(
            scan.events,
            vec![
                (ScanEvent::Start(ScanTag::Paragraph), 0..6),
                (ScanEvent::End(ScanTagKind::Paragraph), 0..6),
            ]
        );
    }

    #[test]
    fn inline_constructs_are_dropped_symmetrically() {
        // Emphasis opens and closes inside the paragraph. Neither is reported,
        // and the paragraph's own pair still balances.
        assert_eq!(
            kinds("a *b* c\n", ScanProfile::Tree),
            vec![
                ScanEvent::Start(ScanTag::Paragraph),
                ScanEvent::End(ScanTagKind::Paragraph),
            ]
        );
    }

    #[test]
    fn code_block_ends_distinguish_indented_from_fenced() {
        assert_eq!(
            kinds("    code\n", ScanProfile::Tree),
            vec![
                ScanEvent::Start(ScanTag::IndentedCodeBlock),
                ScanEvent::End(ScanTagKind::IndentedCodeBlock),
            ]
        );
        assert_eq!(
            kinds("```\ncode\n```\n", ScanProfile::Tree),
            vec![
                ScanEvent::Start(ScanTag::FencedCodeBlock),
                ScanEvent::End(ScanTagKind::FencedCodeBlock),
            ]
        );
    }

    #[test]
    fn heading_carries_its_level() {
        assert_eq!(
            kinds("### t\n", ScanProfile::Tree),
            vec![
                ScanEvent::Start(ScanTag::Heading { level: 3 }),
                ScanEvent::End(ScanTagKind::Heading),
            ]
        );
    }

    #[test]
    fn thematic_break_is_a_rule() {
        assert_eq!(kinds("---\n\n", ScanProfile::Tree), vec![ScanEvent::Rule]);
    }

    #[test]
    fn table_alignments_survive_the_seam() {
        let events = kinds("|a|b|c|\n|:-|:-:|-:|\n|1|2|3|\n", ScanProfile::Tree);
        assert_eq!(
            events.first(),
            Some(&ScanEvent::Start(ScanTag::Table {
                alignments: vec![
                    ScanAlignment::Left,
                    ScanAlignment::Center,
                    ScanAlignment::Right,
                ],
            }))
        );
    }

    #[test]
    fn the_shell_profile_sees_footnotes_the_tree_profile_does_not() {
        let source = "[^a]: note\n";
        assert!(kinds(source, ScanProfile::Shell)
            .contains(&ScanEvent::Start(ScanTag::FootnoteDefinition)));
        assert!(!kinds(source, ScanProfile::Tree)
            .contains(&ScanEvent::Start(ScanTag::FootnoteDefinition)));
    }

    #[test]
    fn every_start_has_a_matching_end() {
        let source = "> - a\n> - b\n\n# h\n\n```r\nx\n```\n\npara [l]: x\n\n[l]: /u\n";
        for profile in [ScanProfile::Tree, ScanProfile::Shell] {
            let mut stack = Vec::new();
            for event in kinds(source, profile) {
                match event {
                    ScanEvent::Start(tag) => stack.push(tag.kind()),
                    ScanEvent::End(kind) => {
                        assert_eq!(stack.pop(), Some(kind), "unbalanced end in {profile:?}");
                    }
                    ScanEvent::Rule => {}
                }
            }
            assert!(stack.is_empty(), "unclosed starts in {profile:?}");
        }
    }

    #[test]
    fn reference_definitions_are_reported() {
        let scan = scan_blocks(
            "[l]: /u\n\ntext\n",
            MarkdownDialect::WAML_DEFAULT,
            ScanProfile::Tree,
        );
        assert_eq!(scan.reference_definitions, vec![0..7]);
    }

    #[test]
    fn the_shell_profile_does_not_collect_reference_definitions() {
        let scan = scan_blocks(
            "[l]: /u\n\ntext\n",
            MarkdownDialect::WAML_DEFAULT,
            ScanProfile::Shell,
        );
        assert!(scan.reference_definitions.is_empty());
    }

    #[test]
    fn all_covers_every_kind() {
        for (index, kind) in ScanTagKind::ALL.iter().enumerate() {
            assert_eq!(kind.ordinal(), index, "ALL is out of step at {index}");
        }
    }

    #[test]
    fn text_entities_are_decoded() {
        assert_eq!(scan_text_entities("&amp;"), "&");
        assert_eq!(scan_text_entities("plain"), "plain");
    }

    #[test]
    fn inline_html_is_recognised_with_its_brackets() {
        assert!(scan_is_inline_html("<span>"));
        assert!(!scan_is_inline_html("not a tag"));
    }
}
