//! Maps an immutable presentation plan onto the foundation layout document.
//!
//! Block kinds come from the parsed block list, never from the text. The
//! foundation layout engine stays the only source-to-screen authority; this
//! module only creates neutral constraint values.

use std::{collections::HashMap, sync::Arc};

use waml_syntax::{TableAlignment, TextRange};

use crate::layout::{
    BlockFlow, BlockLayoutSpec, ColumnAlignment, ColumnConstraint, EdgeInsets, LayoutBlock,
    LayoutDocument, LayoutElementId, LayoutTextRun, MeasuredBlock,
};

use super::{
    style::PresentationStyles, PresentationBlock, PresentationBlockKind, PresentationError,
    PresentationItem, PresentationPlan,
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

    let text_runs = plan
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

    Ok(LayoutDocument {
        revision: plan.revision,
        content_insets: styles.document_insets(),
        blocks: blocks.into(),
        text_runs: text_runs.into(),
        embedded_blocks: measurements.blocks.clone(),
    })
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
