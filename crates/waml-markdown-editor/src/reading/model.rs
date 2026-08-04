//! The viewer-facing block model, derived from a `PresentationPlan`.
//!
//! This is the ONLY place that decides which runs a reading view suppresses.
//! It never drops a run: a suppressed marker is a `ReadingPiece` with
//! `emit == false`, so the model keeps the plan's guarantee that every source
//! byte lies in exactly one piece. Dropping a run instead would make
//! "everything drawn maps back to source" unverifiable.

use std::collections::HashSet;
use std::fmt;

use waml_syntax::{TextRange, TextSize};

use crate::presentation::{
    BlockDecorationKind, PresentationBlockKind, PresentationItem, PresentationPlan, TextRole,
    TextStyle,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadingBlockKind {
    Paragraph,
    Heading(u8),
    BulletItem { level: u8 },
    OrderedItem { level: u8 },
    Quote,
    Code,
    Table { columns: u32 },
    TableRow,
    TableCell { column: u32 },
    Image,
    ThematicBreak,
}

/// One text run of a block, in source order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReadingPiece {
    pub range: TextRange,
    pub role: TextRole,
    pub style: TextStyle,
    /// `false` for markdown punctuation a reading view suppresses. The piece is
    /// KEPT so the source partition stays complete.
    pub emit: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReadingBlock {
    pub kind: ReadingBlockKind,
    pub source_range: TextRange,
    pub pieces: Vec<ReadingPiece>,
    pub children: Vec<ReadingBlock>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReadingDocument {
    pub roots: Vec<ReadingBlock>,
    pub source_len: TextSize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadingError {
    Gap {
        expected: TextSize,
        actual: TextSize,
    },
    Overlap {
        previous_end: TextSize,
        next: TextRange,
    },
    UnknownParent(usize),
}

impl fmt::Display for ReadingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gap { expected, actual } => write!(
                f,
                "reading model gap: expected a piece at {} but the next boundary is {}",
                expected.to_usize(),
                actual.to_usize()
            ),
            Self::Overlap { previous_end, next } => write!(
                f,
                "reading model overlap: {}..{} starts before {}",
                next.start().to_usize(),
                next.end().to_usize(),
                previous_end.to_usize()
            ),
            Self::UnknownParent(index) => {
                write!(
                    f,
                    "reading model block {index} names a parent that follows it"
                )
            }
        }
    }
}

impl std::error::Error for ReadingError {}

impl ReadingDocument {
    /// Every source byte lies in exactly one piece, in order. Mirrors
    /// `PresentationPlan::validate_source_partition`; see the module note for
    /// why the model must not shrink the partition to hide something.
    pub fn validate_source_partition(&self) -> Result<(), ReadingError> {
        let mut expected = TextSize::new(0);
        walk(&self.roots, &mut expected)?;
        if expected != self.source_len {
            return Err(ReadingError::Gap {
                expected,
                actual: self.source_len,
            });
        }
        Ok(())
    }
}

/// One step of a block's own content in source order: either one of its
/// pieces, or a whole child subtree. A block's `pieces` do NOT all precede
/// its `children` in source order -- a table's delimiter row is a piece of
/// the `Table` block itself, but it sits BETWEEN the header row and the data
/// rows, which are `TableRow` children. Walking pieces-then-children would
/// see the header row's bytes skipped over and misreport a gap.
enum Step<'a> {
    Piece(&'a ReadingPiece),
    Child(&'a ReadingBlock),
}

fn walk(blocks: &[ReadingBlock], expected: &mut TextSize) -> Result<(), ReadingError> {
    for block in blocks {
        let mut steps: Vec<Step<'_>> = block
            .pieces
            .iter()
            .map(Step::Piece)
            .chain(block.children.iter().map(Step::Child))
            .collect();
        steps.sort_by_key(|step| match step {
            Step::Piece(piece) => piece.range.start(),
            Step::Child(child) => child.source_range.start(),
        });
        for step in steps {
            match step {
                Step::Piece(piece) => {
                    if piece.range.start() < *expected {
                        return Err(ReadingError::Overlap {
                            previous_end: *expected,
                            next: piece.range,
                        });
                    }
                    if piece.range.start() > *expected {
                        return Err(ReadingError::Gap {
                            expected: *expected,
                            actual: piece.range.start(),
                        });
                    }
                    *expected = piece.range.end();
                }
                Step::Child(child) => {
                    walk(std::slice::from_ref(child), expected)?;
                }
            }
        }
    }
    Ok(())
}

/// Builds the reading model. Pieces are emitted in source order, and each is
/// attached to the deepest block whose source range contains it; a piece
/// inside no block becomes its own synthetic `Paragraph` so the partition
/// stays complete (blank lines and inter-block whitespace take this path).
pub fn build_reading_document(plan: &PresentationPlan) -> Result<ReadingDocument, ReadingError> {
    // 0. One pass over the items collects every ListBullet range, so the
    //    per-run and per-block "is this a bullet?" checks are set lookups
    //    rather than rescans of the whole item list.
    let bullet_ranges = collect_bullet_ranges(plan);

    // 1. Turn the plan's flat block list into reading kinds, keeping the
    //    parent indices. Levels come from ancestor list-item depth.
    let mut kinds: Vec<ReadingBlockKind> = Vec::with_capacity(plan.blocks.len());
    for (index, block) in plan.blocks.iter().enumerate() {
        if let Some(parent) = block.parent {
            if parent >= index {
                return Err(ReadingError::UnknownParent(index));
            }
        }
        let level = list_depth(plan, index);
        kinds.push(match block.kind {
            PresentationBlockKind::Paragraph => ReadingBlockKind::Paragraph,
            PresentationBlockKind::Heading(level) => ReadingBlockKind::Heading(level),
            PresentationBlockKind::ListItem { marker_range } => {
                let is_bullet = bullet_ranges.contains(&range_key(marker_range));
                if is_bullet {
                    ReadingBlockKind::BulletItem { level }
                } else {
                    ReadingBlockKind::OrderedItem { level }
                }
            }
            PresentationBlockKind::Quote => ReadingBlockKind::Quote,
            PresentationBlockKind::Code => ReadingBlockKind::Code,
            PresentationBlockKind::Table { columns } => ReadingBlockKind::Table { columns },
            PresentationBlockKind::TableRow => ReadingBlockKind::TableRow,
            PresentationBlockKind::TableCell { column, .. } => {
                ReadingBlockKind::TableCell { column }
            }
            PresentationBlockKind::Image => ReadingBlockKind::Image,
        });
    }

    // 2. Bucket every text run into the deepest containing block. The plan's
    //    runs partition the source in order (`validate_source_partition` ran
    //    at compile time) and block ranges nest, so one sweep with an
    //    open-block stack replaces a per-run scan of every block.
    let mut buckets: Vec<Vec<ReadingPiece>> = vec![Vec::new(); plan.blocks.len()];
    let mut orphans: Vec<ReadingPiece> = Vec::new();
    let mut sweep = BlockSweep::new(plan);
    for item in plan.items.iter() {
        let PresentationItem::TextRun {
            range, role, style, ..
        } = item
        else {
            continue;
        };
        let piece = ReadingPiece {
            range: *range,
            role: *role,
            style: *style,
            emit: emits(&bullet_ranges, *role, *range),
        };
        match sweep.deepest_block(plan, *range) {
            Some(index) => buckets[index].push(piece),
            None => orphans.push(piece),
        }
    }

    // 3. Assemble the tree, interleaving orphan pieces in source order so the
    //    partition walk in `validate_source_partition` stays monotone.
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); plan.blocks.len()];
    let mut roots: Vec<usize> = Vec::new();
    for (index, block) in plan.blocks.iter().enumerate() {
        match block.parent {
            Some(parent) => children[parent].push(index),
            None => roots.push(index),
        }
    }

    fn assemble(
        index: usize,
        kinds: &[ReadingBlockKind],
        buckets: &mut [Vec<ReadingPiece>],
        children: &[Vec<usize>],
        ranges: &[TextRange],
    ) -> ReadingBlock {
        let kids = children[index]
            .iter()
            .map(|child| assemble(*child, kinds, buckets, children, ranges))
            .collect::<Vec<_>>();
        ReadingBlock {
            kind: kinds[index],
            source_range: ranges[index],
            pieces: std::mem::take(&mut buckets[index]),
            children: kids,
        }
    }

    let ranges: Vec<TextRange> = plan.blocks.iter().map(|block| block.source_range).collect();
    let mut assembled: Vec<ReadingBlock> = Vec::new();
    let mut orphan_iter = orphans.into_iter().peekable();
    for root in roots {
        while let Some(piece) = orphan_iter.peek() {
            if piece.range.start() >= ranges[root].start() {
                break;
            }
            let piece = orphan_iter.next().expect("peeked");
            assembled.push(gap_block(piece));
        }
        assembled.push(assemble(root, &kinds, &mut buckets, &children, &ranges));
    }
    for piece in orphan_iter {
        assembled.push(gap_block(piece));
    }

    let document = ReadingDocument {
        roots: assembled,
        source_len: plan.source_len,
    };
    document.validate_source_partition()?;
    Ok(document)
}

/// A run that lies in no parsed block (blank lines, inter-block whitespace).
/// It becomes its own paragraph so the model still covers the source.
fn gap_block(piece: ReadingPiece) -> ReadingBlock {
    ReadingBlock {
        kind: ReadingBlockKind::Paragraph,
        source_range: piece.range,
        pieces: vec![piece],
        children: Vec::new(),
    }
}

/// Whether a run is drawn. Markdown punctuation is suppressed; an ordered list
/// number is content, an unordered bullet character is not.
fn emits(bullet_ranges: &HashSet<(usize, usize)>, role: TextRole, range: TextRange) -> bool {
    if role.is_syntax_marker() {
        return false;
    }
    if role == TextRole::ListMarker {
        return !bullet_ranges.contains(&range_key(range));
    }
    true
}

fn range_key(range: TextRange) -> (usize, usize) {
    (range.start().to_usize(), range.end().to_usize())
}

/// The compiler already knows which markers are bullets: an unordered item
/// carries a `ListBullet` decoration over the marker's own range. Reading that
/// back keeps the "is this a bullet?" answer in one place.
fn collect_bullet_ranges(plan: &PresentationPlan) -> HashSet<(usize, usize)> {
    plan.items
        .iter()
        .filter_map(|item| match item {
            PresentationItem::BlockDecoration {
                source_range,
                kind: BlockDecorationKind::ListBullet { .. },
                ..
            } => Some(range_key(*source_range)),
            _ => None,
        })
        .collect()
}

/// Deepest-containing-block queries for runs arriving in source order.
///
/// Blocks are visited in pre-order (start ascending, wider range first). The
/// stack holds the ancestor chain of the last pushed block; because block
/// ranges nest and queried runs never move backwards, a block popped for one
/// run can never contain a later one.
struct BlockSweep {
    order: Vec<usize>,
    next: usize,
    open: Vec<usize>,
}

impl BlockSweep {
    fn new(plan: &PresentationPlan) -> Self {
        let mut order: Vec<usize> = (0..plan.blocks.len()).collect();
        order.sort_by_key(|&index| {
            let range = plan.blocks[index].source_range;
            (range.start(), std::cmp::Reverse(range.end()))
        });
        Self {
            order,
            next: 0,
            open: Vec::new(),
        }
    }

    fn deepest_block(&mut self, plan: &PresentationPlan, range: TextRange) -> Option<usize> {
        while self.next < self.order.len() {
            let index = self.order[self.next];
            if plan.blocks[index].source_range.start() > range.start() {
                break;
            }
            while let Some(&top) = self.open.last() {
                if plan.blocks[top].source_range.end() >= plan.blocks[index].source_range.end() {
                    break;
                }
                self.open.pop();
            }
            self.open.push(index);
            self.next += 1;
        }
        while let Some(&top) = self.open.last() {
            if plan.blocks[top].source_range.end() >= range.end() {
                break;
            }
            self.open.pop();
        }
        self.open.last().copied()
    }
}

/// Nesting depth of block `index`, counting only `ListItem` ancestors.
fn list_depth(plan: &PresentationPlan, index: usize) -> u8 {
    let mut depth: u8 = 0;
    let mut cursor = plan.blocks[index].parent;
    while let Some(parent) = cursor {
        if matches!(
            plan.blocks[parent].kind,
            PresentationBlockKind::ListItem { .. }
        ) {
            depth = depth.saturating_add(1);
        }
        cursor = plan.blocks[parent].parent;
    }
    depth
}
