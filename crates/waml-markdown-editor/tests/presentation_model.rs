use std::{ops::Range, sync::Arc};

use waml_markdown_editor::presentation::{
    BlockDecorationKind, ColorRole, EmbeddedBlockKind, FontRole, FontSizeRole, FontWeightRole,
    PresentationError, PresentationItem, PresentationItemId, PresentationPlan, PresentationRole,
    TextRole, TextStyle,
};
use waml_syntax::{DocumentRevision, SyntaxIdentity, TextRange, TextSize};

fn owner(value: u64) -> SyntaxIdentity {
    SyntaxIdentity::from_raw_for_test(value)
}

fn t(value: usize) -> TextSize {
    TextSize::try_from_usize(value).expect("a test offset fits")
}

fn range(bounds: Range<usize>) -> TextRange {
    TextRange::new(t(bounds.start), t(bounds.end)).expect("a test range is ordered")
}

fn style() -> TextStyle {
    TextStyle {
        font: FontRole::Body,
        size: FontSizeRole::Body,
        weight: FontWeightRole::Regular,
        italic: false,
        color: ColorRole::Text,
        active_color: ColorRole::Text,
        background: None,
        underline: false,
        strikethrough: false,
    }
}

fn run(
    bounds: Range<usize>,
    role: TextRole,
    owner: SyntaxIdentity,
    fragment_ordinal: u32,
) -> PresentationItem {
    PresentationItem::TextRun {
        id: PresentationItemId {
            owner,
            role: PresentationRole::Text(role),
            fragment_ordinal,
        },
        range: range(bounds),
        role,
        style: style(),
        hidden: false,
    }
}

fn plan_for_source(
    source: &str,
    items: impl IntoIterator<Item = PresentationItem>,
) -> PresentationPlan {
    PresentationPlan {
        revision: DocumentRevision::INITIAL,
        source_len: t(source.len()),
        items: items.into_iter().collect::<Vec<_>>().into(),
        links: Arc::from([]),
        blocks: Arc::from([]),
        diagnostics: Arc::from([]),
    }
}

fn plan_with_ranges(
    source: &str,
    ranges: impl IntoIterator<Item = Range<usize>>,
) -> PresentationPlan {
    plan_for_source(
        source,
        ranges
            .into_iter()
            .enumerate()
            .map(|(ordinal, bounds)| run(bounds, TextRole::Body, owner(1), ordinal as u32))
            .collect::<Vec<_>>(),
    )
}

fn single_range_plan(source: &str, bounds: Range<usize>) -> PresentationPlan {
    plan_for_source(source, [run(bounds, TextRole::Body, owner(1), 0)])
}

fn plan_with_duplicate_ids(source: &str) -> PresentationPlan {
    plan_for_source(
        source,
        [
            run(0..2, TextRole::Body, owner(1), 0),
            run(2..source.len(), TextRole::Body, owner(1), 0),
        ],
    )
}

#[test]
fn text_runs_partition_every_source_byte_once() {
    let plan = plan_for_source(
        "**a**\n",
        [
            run(0..2, TextRole::SyntaxMarker, owner(1), 0),
            run(2..3, TextRole::Strong, owner(1), 1),
            run(3..5, TextRole::SyntaxMarker, owner(1), 2),
            run(5..6, TextRole::LineBreak, owner(2), 0),
        ],
    );
    assert_eq!(plan.validate_source_partition(), Ok(()));
}

#[test]
fn partition_rejects_gap_overlap_duplicate_and_out_of_bounds() {
    assert!(matches!(
        plan_with_ranges("abcd", [0..1, 2..4]).validate_source_partition(),
        Err(PresentationError::Gap { .. })
    ));
    assert!(matches!(
        plan_with_ranges("abcd", [0..3, 2..4]).validate_source_partition(),
        Err(PresentationError::Overlap { .. })
    ));
    assert!(matches!(
        single_range_plan("abcd", 0..5).validate_source_partition(),
        Err(PresentationError::OutOfBounds { .. })
    ));
    assert!(matches!(
        plan_with_duplicate_ids("abcd").validate_source_partition(),
        Err(PresentationError::DuplicateId(_))
    ));
}

#[test]
fn a_short_final_run_reports_the_exact_trailing_gap() {
    let plan = single_range_plan("abcd", 0..2);
    assert_eq!(
        plan.validate_source_partition(),
        Err(PresentationError::Gap {
            expected: t(2),
            actual: t(4),
        })
    );
}

#[test]
fn decorations_and_embedded_blocks_stay_out_of_the_text_partition() {
    // Both overlap the owner's source range and neither contributes a byte to
    // the text partition.
    let plan = plan_for_source(
        "**a**\n",
        [
            run(0..6, TextRole::Body, owner(1), 0),
            PresentationItem::BlockDecoration {
                id: PresentationItemId {
                    owner: owner(1),
                    role: PresentationRole::Block(
                        waml_markdown_editor::presentation::BlockDecorationRole::InlineCodeFill,
                    ),
                    fragment_ordinal: 0,
                },
                owner: owner(1),
                source_range: range(0..6),
                kind: BlockDecorationKind::InlineCodeFill,
            },
            PresentationItem::EmbeddedBlock {
                id: PresentationItemId {
                    owner: owner(1),
                    role: PresentationRole::Embedded(
                        waml_markdown_editor::presentation::EmbeddedBlockRole::Image,
                    ),
                    fragment_ordinal: 0,
                },
                owner: owner(1),
                source_range: range(2..3),
                kind: EmbeddedBlockKind::Image {
                    destination: Arc::from("a.png"),
                    alt: Arc::from("a"),
                    title: None,
                },
            },
        ],
    );
    assert_eq!(plan.validate_source_partition(), Ok(()));
}

#[test]
fn active_owners_report_every_owner_touching_the_caret() {
    let plan = plan_for_source(
        "**a**\n",
        [
            run(0..2, TextRole::SyntaxMarker, owner(1), 0),
            run(2..3, TextRole::Strong, owner(7), 0),
            run(3..5, TextRole::SyntaxMarker, owner(1), 1),
            run(5..6, TextRole::LineBreak, owner(2), 0),
        ],
    );
    assert_eq!(plan.active_owners(t(2)).as_ref(), &[owner(1), owner(7)]);
    assert_eq!(plan.active_owners(t(6)).as_ref(), &[owner(2)]);
}
