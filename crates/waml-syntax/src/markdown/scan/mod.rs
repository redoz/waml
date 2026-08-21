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
//! * Inline-level constructs are not reported *by the block scan*. An
//!   implementation that cannot report a construct must omit its start *and*
//!   its end, so the event stream stays balanced. The two inline helpers
//!   ([`scan_text_entities`], [`scan_is_inline_html`]) are the only inline-level
//!   entry points, and each is specified on its own re-export below.
//! * [`ScanEvent::End`] names the exact kind that opened, including the
//!   indented/fenced code-block distinction.
//! * [`BlockScan::reference_definitions`] reports *every* definition, including
//!   one that repeats a label already defined above it: a repeat is a
//!   definition too, and it runs over as many lines, so a caller that could not
//!   see it would have to guess where it ended. Returned in implementation
//!   order, unsorted. `block.rs` validates each span before sorting, and that
//!   order of operations is load-bearing for its recovery path. Only
//!   [`ScanProfile::Tree`]
//!   collects them; under [`ScanProfile::Shell`] the field is `None`, because
//!   the shell mapper never reads it. `None` is distinct from "collected and
//!   there were none", so a consumer that reads it under the wrong profile
//!   sees the omission instead of an empty list.
//! * Range screening is the implementation's job, not the caller's: because the
//!   inline and text events are filtered out here, only the implementation can
//!   still see them. It must check every raw event's range and report
//!   [`BlockScan::malformed_range`] if any of them is not a char-aligned slice
//!   of `source`. Callers re-validate the block ranges they receive, but they
//!   depend on this flag for the events they never see.

mod pulldown;

use std::ops::Range;

// The seam's entry points. `scan_blocks` is specified by the types below; the
// two inline helpers have no types of their own, so their contracts live here.
//
// * `fn scan_text_entities(spelling: &str) -> String`
//
//   Decodes `spelling` as inline Markdown *text* and returns the concatenation
//   of the text it yields, with character references (`&amp;`, `&#38;`, ...)
//   resolved. Everything that is not text — emphasis markers, code spans, raw
//   HTML — contributes nothing, so the result may be shorter than the input and
//   is empty when `spelling` holds no text at all. Input that decodes to itself
//   (no entities, no markup) is returned unchanged. Never fails: an
//   implementation that cannot decode an entity leaves it spelled out.
//
// * `fn scan_is_inline_html(candidate: &str) -> bool`
//
//   Whether `candidate` — angle brackets included — is exactly one raw inline
//   HTML tag and nothing else. `true` only when the implementation recognises
//   HTML whose spelling equals the whole of `candidate`; a tag with leading or
//   trailing text, or text alone, is `false`. Purely a classification: it never
//   allocates a result the caller must interpret and never fails.
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
    const fn ordinal(self) -> usize {
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

    /// The variant count, derived from the last variant's ordinal so it moves
    /// with the exhaustive match in [`Self::ordinal`] instead of sitting as a
    /// second hand-maintained literal for `all_covers_every_kind` to drift
    /// against.
    const COUNT: usize = Self::DefinitionListDefinition.ordinal() + 1;
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
    /// Every link reference definition, repeats of an already-defined label
    /// included, or `None` when the profile does not collect them.
    /// [`ScanProfile::Shell`] always yields `None`; a consumer that reads the
    /// field under that profile has to face the omission rather than mistake it
    /// for a document with no definitions.
    pub reference_definitions: Option<Vec<Range<usize>>>,
    /// Set when *any* raw event the implementation saw — including the inline
    /// and text events this vocabulary drops — carried a range that was not a
    /// char-aligned slice of `source`. Callers treat the whole scan as
    /// untrustworthy and fall back to raw text.
    pub malformed_range: bool,
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
        let expected: Vec<Range<usize>> = vec![Range { start: 0, end: 7 }];
        assert_eq!(scan.reference_definitions, Some(expected));
    }

    #[test]
    fn a_repeated_definition_is_reported_with_its_own_span() {
        // The underlying definition table is keyed by label and holds one entry
        // per label, so a repeat is reported only if the scan looks for it.
        // `[l]: \n/v` is a definition whose destination sits on the line below,
        // and the repeat has to be measured across both lines exactly as the
        // first of its label is.
        let scan = scan_blocks(
            "[l]: \n/u\n\n[l]: \n/v\n",
            MarkdownDialect::WAML_DEFAULT,
            ScanProfile::Tree,
        );
        let mut spans = scan.reference_definitions.expect("tree profile collects");
        spans.sort_by_key(|span| span.start);
        assert_eq!(
            spans,
            vec![Range { start: 0, end: 8 }, Range { start: 10, end: 18 }]
        );
    }

    #[test]
    fn a_repeat_the_parser_reads_as_text_is_not_reported() {
        // The second `[l]: /v` is a lazy continuation of the paragraph above
        // it, so it is inline text, not a definition. A repeat scan that only
        // asked "does this look like a definition on its own" would report it.
        let scan = scan_blocks(
            "[l]: /u\n\ntext\n[l]: /v\n",
            MarkdownDialect::WAML_DEFAULT,
            ScanProfile::Tree,
        );
        assert_eq!(
            scan.reference_definitions,
            Some(vec![Range { start: 0, end: 7 }])
        );
    }

    #[test]
    fn the_shell_profile_does_not_collect_reference_definitions() {
        // `None`, not an empty `Vec`: the source below *has* a definition, so an
        // empty list would be indistinguishable from a wrong answer.
        let scan = scan_blocks(
            "[l]: /u\n\ntext\n",
            MarkdownDialect::WAML_DEFAULT,
            ScanProfile::Shell,
        );
        assert_eq!(scan.reference_definitions, None);
    }

    #[test]
    fn all_covers_every_kind() {
        // Matching ordinals alone would still pass for a *short* `ALL`, which
        // would silently thin the parity coverage `ALL` drives in block.rs. The
        // length assertion is the half that catches an omitted variant, and it
        // is checked against `COUNT` — derived from `ordinal`'s exhaustive
        // match — rather than a second hand-maintained literal, so a variant
        // added to the enum without a matching `ALL` entry fails here instead
        // of staying green.
        assert_eq!(
            ScanTagKind::ALL.len(),
            ScanTagKind::COUNT,
            "a variant is missing from ALL"
        );
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
