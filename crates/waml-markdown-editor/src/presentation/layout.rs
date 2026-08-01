//! Maps an immutable presentation plan onto the foundation layout document.
//!
//! Block kinds come from the parsed block list, never from the text. The
//! foundation layout engine stays the only source-to-screen authority; this
//! module only creates neutral constraint values.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use waml_syntax::{TableAlignment, TextRange};

use crate::layout::{
    BlockFlow, BlockLayoutSpec, ColumnAlignment, ColumnConstraint, EdgeInsets, LayoutBlock,
    LayoutDocument, LayoutElementId, LayoutTextRun, MeasuredBlock,
};

use super::{
    style::PresentationStyles, PresentationBlock, PresentationBlockKind, PresentationError,
    PresentationItem, PresentationPlan, TextRole,
};

/// Embedded block sizes measured by the application for one revision.
#[derive(Clone, Debug, Default)]
pub struct EmbeddedMeasurements {
    pub revision: Option<waml_syntax::DocumentRevision>,
    pub blocks: Arc<[MeasuredBlock]>,
}

/// Builds the neutral layout document for one plan revision.
pub fn build_layout_document(
    plan: &PresentationPlan,
    styles: &PresentationStyles,
    measurements: &EmbeddedMeasurements,
) -> Result<LayoutDocument, PresentationError> {
    if let Some(revision) = measurements.revision {
        if revision != plan.revision {
            return Err(PresentationError::RevisionMismatch {
                expected: plan.revision,
                actual: revision,
                component: "embedded measurements",
            });
        }
    }

    let mut blocks = Vec::with_capacity(plan.blocks.len() + 1);
    let mut index_of_owner = HashMap::new();

    // A document-wide root keeps text that belongs to no parsed block, so every
    // source byte still reaches a block.
    let root_id = LayoutElementId {
        owner: plan
            .blocks
            .first()
            .map(|block| block.owner)
            .or_else(|| plan.items.first().map(|item| item.owner()))
            .ok_or(PresentationError::Gap {
                expected: plan.source_len,
                actual: plan.source_len,
            })?,
        fragment_ordinal: u32::MAX,
    };
    blocks.push(LayoutBlock {
        id: root_id,
        source_range: TextRange::new(waml_syntax::TextSize::try_from_usize(0)?, plan.source_len)?,
        parent: None,
        spec: BlockLayoutSpec {
            flow: BlockFlow::Paragraph,
            insets: EdgeInsets::default(),
            space_before: 0.0,
            space_after: 0.0,
            columns: Arc::from([]),
        },
    });

    for block in plan.blocks.iter() {
        let id = LayoutElementId {
            owner: block.owner,
            fragment_ordinal: 0,
        };
        index_of_owner.insert(block.owner, blocks.len());
        let parent = block
            .parent
            .and_then(|parent| plan.blocks.get(parent))
            .and_then(|parent| index_of_owner.get(&parent.owner).copied())
            .map(|parent| blocks[parent].id)
            .or(Some(root_id));
        blocks.push(LayoutBlock {
            id,
            source_range: block.source_range,
            parent,
            spec: block_spec(block, plan, styles),
        });
    }

    let mut text_runs = plan
        .items
        .iter()
        .filter_map(|item| match item {
            PresentationItem::TextRun { range, role, .. } => Some(LayoutTextRun {
                id: owning_block_id(*range, plan, &index_of_owner, &blocks, root_id),
                range: *range,
                metrics: styles.metrics(*role),
            }),
            _ => None,
        })
        .collect::<Vec<_>>();
    attach_contiguous_quote_prefixes(plan, &mut blocks, &mut text_runs);
    fragment_parent_owned_runs(plan, measurements, &mut blocks, &mut text_runs);
    separate_embedded_source_blocks(plan, measurements, &mut blocks, &mut text_runs);

    Ok(LayoutDocument {
        revision: plan.revision,
        content_insets: styles.document_insets(),
        blocks: blocks.into(),
        text_runs: text_runs.into(),
        embedded_blocks: measurements.blocks.clone(),
    })
}

/// Gives a measured embed its own child block below the literal source block.
///
/// Presentation embeds and parsed image source initially share one identity.
/// If that block also owns text, its rectangle cannot represent both the source
/// line and the measured visual. The parsed block keeps the text under a fresh,
/// collision-checked ID. The presentation item keeps its original ID on a
/// measured child, so draw-command identity remains unchanged.
fn separate_embedded_source_blocks(
    plan: &PresentationPlan,
    measurements: &EmbeddedMeasurements,
    blocks: &mut Vec<LayoutBlock>,
    text_runs: &mut [LayoutTextRun],
) {
    let mut used_ids = blocks.iter().map(|block| block.id).collect::<HashSet<_>>();
    used_ids.extend(measurements.blocks.iter().map(|block| block.id));
    used_ids.extend(plan.items.iter().map(|item| {
        let id = item.id();
        LayoutElementId {
            owner: id.owner,
            fragment_ordinal: id.fragment_ordinal,
        }
    }));
    let mut next_ordinal = HashMap::new();

    for measured in measurements.blocks.iter() {
        let Some(block_index) = blocks.iter().position(|block| block.id == measured.id) else {
            continue;
        };
        if !text_runs.iter().any(|run| run.id == measured.id) {
            continue;
        }
        let original = blocks[block_index].clone();
        let source_id = next_gap_fragment_id(measured.id, &mut used_ids, &mut next_ordinal);
        for block in blocks.iter_mut() {
            if block.parent == Some(measured.id) {
                block.parent = Some(source_id);
            }
        }
        for run in text_runs.iter_mut().filter(|run| run.id == measured.id) {
            run.id = source_id;
        }
        blocks[block_index].parent = Some(source_id);
        blocks[block_index].spec = BlockLayoutSpec {
            flow: BlockFlow::Embedded,
            insets: EdgeInsets::default(),
            space_before: 0.0,
            space_after: 0.0,
            columns: Arc::from([]),
        };
        blocks.push(LayoutBlock {
            id: source_id,
            source_range: original.source_range,
            parent: original.parent,
            spec: original.spec,
        });
    }
}

/// Makes contiguous quote prefixes part of the first leaf paragraph.
///
/// Quote markers are parsed on structural ancestor blocks while the text on
/// the same source line belongs to a leaf paragraph. Attaching the contiguous
/// marker chain to that leaf lets one paragraph composer keep `> text` on one
/// visual row. A marker followed by a real source gap stays on its parent and
/// is handled by the ordered gap-fragment path.
fn attach_contiguous_quote_prefixes(
    plan: &PresentationPlan,
    blocks: &mut [LayoutBlock],
    text_runs: &mut [LayoutTextRun],
) {
    let quote_ranges = plan
        .items
        .iter()
        .filter_map(|item| match item {
            PresentationItem::TextRun {
                range,
                role: TextRole::QuoteMarker,
                ..
            } => Some(*range),
            _ => None,
        })
        .collect::<Vec<_>>();
    let parent_ids = blocks
        .iter()
        .filter_map(|block| block.parent)
        .collect::<HashSet<_>>();

    for leaf_index in 0..blocks.len() {
        let leaf_id = blocks[leaf_index].id;
        if parent_ids.contains(&leaf_id) {
            continue;
        }
        let original_start = blocks[leaf_index].source_range.start();
        let mut cursor = original_start;
        while let Some(marker_range) = quote_ranges.iter().copied().find(|range| {
            range.end() == cursor
                && text_runs
                    .iter()
                    .any(|run| run.range == *range && block_is_ancestor(blocks, run.id, leaf_index))
        }) {
            let Some(run) = text_runs.iter_mut().find(|run| run.range == marker_range) else {
                break;
            };
            run.id = leaf_id;
            cursor = marker_range.start();
        }
        if cursor < original_start {
            blocks[leaf_index].source_range =
                TextRange::new(cursor, blocks[leaf_index].source_range.end())
                    .expect("a contiguous quote prefix extends its leaf backward");
        }
    }
}

fn block_is_ancestor(blocks: &[LayoutBlock], ancestor: LayoutElementId, mut child: usize) -> bool {
    while let Some(parent) = blocks[child].parent {
        if parent == ancestor {
            return true;
        }
        let Some(parent_index) = blocks.iter().position(|block| block.id == parent) else {
            return false;
        };
        child = parent_index;
    }
    false
}

/// Moves a parent's own source runs into ordered leaf fragments.
///
/// The layout engine paints a block's own runs before its child blocks. A
/// structural parent can own separator or marker runs on both sides of those
/// children, so keeping those runs on the parent changes source order. Each
/// maximal contiguous run sequence becomes a zero-margin paragraph child. The
/// existing hierarchy sort then interleaves it with parsed children by source.
fn fragment_parent_owned_runs(
    plan: &PresentationPlan,
    measurements: &EmbeddedMeasurements,
    blocks: &mut Vec<LayoutBlock>,
    text_runs: &mut [LayoutTextRun],
) {
    let parents = blocks
        .iter()
        .filter_map(|block| block.parent)
        .collect::<HashSet<_>>();
    let parent_flows = blocks
        .iter()
        .map(|block| (block.id, block.spec.flow.clone()))
        .collect::<HashMap<_, _>>();
    let mut used_ids = blocks.iter().map(|block| block.id).collect::<HashSet<_>>();
    used_ids.extend(measurements.blocks.iter().map(|block| block.id));
    used_ids.extend(plan.items.iter().map(|item| {
        let id = item.id();
        LayoutElementId {
            owner: id.owner,
            fragment_ordinal: id.fragment_ordinal,
        }
    }));
    let mut next_ordinal = HashMap::new();
    let mut active: Option<(LayoutElementId, waml_syntax::TextSize, usize)> = None;

    for run in text_runs {
        let parent_id = run.id;
        if !parents.contains(&parent_id) {
            active = None;
            continue;
        }
        if let Some((active_parent, active_end, fragment_index)) = active {
            if active_parent == parent_id && active_end == run.range.start() {
                let fragment = &mut blocks[fragment_index];
                fragment.source_range =
                    TextRange::new(fragment.source_range.start(), run.range.end())
                        .expect("contiguous gap runs stay ordered");
                fragment.spec.flow =
                    gap_fragment_flow(parent_flows.get(&parent_id), fragment.source_range);
                run.id = fragment.id;
                active = Some((parent_id, run.range.end(), fragment_index));
                continue;
            }
        }

        let fragment_id = next_gap_fragment_id(parent_id, &mut used_ids, &mut next_ordinal);
        let fragment_index = blocks.len();
        blocks.push(LayoutBlock {
            id: fragment_id,
            source_range: run.range,
            parent: Some(parent_id),
            spec: BlockLayoutSpec {
                flow: gap_fragment_flow(parent_flows.get(&parent_id), run.range),
                insets: EdgeInsets::default(),
                space_before: 0.0,
                space_after: 0.0,
                columns: Arc::from([]),
            },
        });
        run.id = fragment_id;
        active = Some((parent_id, run.range.end(), fragment_index));
    }
}

fn gap_fragment_flow(parent_flow: Option<&BlockFlow>, range: TextRange) -> BlockFlow {
    match parent_flow {
        Some(flow @ BlockFlow::Hanging { marker_range, .. })
            if range.start() <= marker_range.start() && marker_range.end() <= range.end() =>
        {
            flow.clone()
        }
        _ => BlockFlow::Paragraph,
    }
}

fn next_gap_fragment_id(
    parent_id: LayoutElementId,
    used_ids: &mut HashSet<LayoutElementId>,
    next_ordinal: &mut HashMap<waml_syntax::SyntaxIdentity, u32>,
) -> LayoutElementId {
    let ordinal = next_ordinal.entry(parent_id.owner).or_insert(u32::MAX - 1);
    loop {
        let candidate = LayoutElementId {
            owner: parent_id.owner,
            fragment_ordinal: *ordinal,
        };
        *ordinal = ordinal
            .checked_sub(1)
            .expect("a document cannot exhaust layout fragment ordinals");
        if used_ids.insert(candidate) {
            return candidate;
        }
    }
}

/// The id of the innermost parsed block that contains `range`.
fn owning_block_id(
    range: TextRange,
    plan: &PresentationPlan,
    index_of_owner: &HashMap<waml_syntax::SyntaxIdentity, usize>,
    blocks: &[LayoutBlock],
    root_id: LayoutElementId,
) -> LayoutElementId {
    plan.blocks
        .iter()
        // Blocks are emitted outermost first, so the last container wins.
        .rfind(|block| {
            block.source_range.start() <= range.start() && range.end() <= block.source_range.end()
        })
        .and_then(|block| index_of_owner.get(&block.owner).copied())
        .map_or(root_id, |index| blocks[index].id)
}

fn block_spec(
    block: &PresentationBlock,
    plan: &PresentationPlan,
    styles: &PresentationStyles,
) -> BlockLayoutSpec {
    let spacing = styles.spacing();
    let mut spec = BlockLayoutSpec {
        flow: BlockFlow::Paragraph,
        insets: EdgeInsets::default(),
        space_before: 0.0,
        space_after: spacing.paragraph_after,
        columns: Arc::from([]),
    };
    match block.kind {
        PresentationBlockKind::Paragraph | PresentationBlockKind::Image => {}
        PresentationBlockKind::Heading(level) => {
            let (before, after) = spacing.heading_margins(level);
            spec.space_before = before;
            spec.space_after = after;
        }
        PresentationBlockKind::ListItem { marker_range } => {
            spec.flow = BlockFlow::Hanging {
                marker_range,
                content_indent: styles.marker_indent(marker_range),
            };
            spec.space_after = 0.0;
        }
        PresentationBlockKind::Quote => {
            spec.flow = BlockFlow::Quote;
            spec.insets = EdgeInsets {
                left: spacing.quote_inset,
                ..EdgeInsets::default()
            };
        }
        PresentationBlockKind::Code => {
            spec.flow = BlockFlow::Code;
            spec.insets = EdgeInsets {
                top: spacing.code_padding,
                right: spacing.code_padding,
                bottom: spacing.code_padding,
                left: spacing.code_padding,
            };
        }
        PresentationBlockKind::Table { columns } => {
            spec.flow = BlockFlow::Table;
            spec.columns = column_constraints(block, plan, columns);
        }
        PresentationBlockKind::TableRow => {
            spec.flow = BlockFlow::TableRow;
            spec.space_after = 0.0;
        }
        PresentationBlockKind::TableCell { column, .. } => {
            spec.flow = BlockFlow::TableCell { column };
            spec.insets = EdgeInsets {
                top: spacing.cell_padding_y,
                right: spacing.cell_padding_x,
                bottom: spacing.cell_padding_y,
                left: spacing.cell_padding_x,
            };
            spec.space_after = 0.0;
        }
    }
    spec
}

/// Column constraints carry parsed alignment; widths stay intrinsic so the
/// layout engine keeps solving them.
fn column_constraints(
    table: &PresentationBlock,
    plan: &PresentationPlan,
    columns: u32,
) -> Arc<[ColumnConstraint]> {
    let mut alignments = vec![TableAlignment::None; columns as usize];
    for block in plan.blocks.iter() {
        let PresentationBlockKind::TableCell { column, alignment } = block.kind else {
            continue;
        };
        let inside = table.source_range.start() <= block.source_range.start()
            && block.source_range.end() <= table.source_range.end();
        if !inside {
            continue;
        }
        if let Some(slot) = alignments.get_mut(column as usize) {
            if *slot == TableAlignment::None {
                *slot = alignment;
            }
        }
    }
    alignments
        .into_iter()
        .map(|alignment| ColumnConstraint {
            min_width: 0.0,
            max_width: None,
            alignment: match alignment {
                TableAlignment::Center => ColumnAlignment::Center,
                TableAlignment::Right => ColumnAlignment::End,
                TableAlignment::None | TableAlignment::Left => ColumnAlignment::Start,
            },
        })
        .collect::<Vec<_>>()
        .into()
}
