use std::sync::Arc;

use crate::{
    parse_okf_markdown, MarkdownDialect, OkfMarkdownLanguage, ParseError, SourceText,
    SyntaxLanguage, SyntaxTree, TextError, TextRange, TextSize,
};

/// One replacement expressed in checked, half-open byte offsets of the old text.
#[derive(Clone, Debug)]
pub struct TextChange {
    pub old_range: TextRange,
    pub replacement: Arc<str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChangeSegment {
    pub old: TextRange,
    pub new: TextRange,
}

#[derive(Clone, Debug)]
pub struct ChangeMap {
    old_len: TextSize,
    new_len: TextSize,
    segments: Arc<[ChangeSegment]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FullReparseReason {
    NoPreviousSnapshot,
    OverlappingChanges,
    InvalidUtf8Boundary,
    FrontmatterBoundaryChanged,
    MarkdownContainerBoundaryChanged,
    HeadingBoundaryChanged,
    IslandBoundaryChanged,
    UnsafeSynchronization,
}

pub enum ReparseOutcome<L: SyntaxLanguage> {
    Incremental {
        tree: Arc<SyntaxTree<L>>,
        shared_source_independent_green: usize,
        reparsed_range: TextRange,
    },
    Full {
        tree: Arc<SyntaxTree<L>>,
        reason: FullReparseReason,
    },
}

impl ChangeMap {
    pub fn checked(
        old: &SourceText,
        changes: &[TextChange],
    ) -> Result<Self, FullReparseReason> {
        let zero = TextSize::try_from_usize(0).map_err(|_| FullReparseReason::UnsafeSynchronization)?;
        let mut previous_start = zero;
        let mut previous_end = zero;
        let mut previous_was_insertion = false;
        let mut new_at = zero;
        let mut segments = Vec::with_capacity(changes.len());
        for change in changes {
            let start = change.old_range.start();
            if start < previous_start
                || start < previous_end
                || (previous_was_insertion && start == previous_start)
            {
                return Err(FullReparseReason::OverlappingChanges);
            }
            old.slice(change.old_range).map_err(|error| match error {
                TextError::NonUtf8Boundary { .. } => FullReparseReason::InvalidUtf8Boundary,
                _ => FullReparseReason::UnsafeSynchronization,
            })?;
            let unchanged = TextSize::try_from_usize(
                start.to_usize()
                    .checked_sub(previous_end.to_usize())
                    .ok_or(FullReparseReason::UnsafeSynchronization)?,
            )
            .map_err(|_| FullReparseReason::UnsafeSynchronization)?;
            new_at = new_at
                .checked_add(unchanged)
                .map_err(|_| FullReparseReason::UnsafeSynchronization)?;
            let replacement = TextSize::try_from_usize(change.replacement.len())
                .map_err(|_| FullReparseReason::UnsafeSynchronization)?;
            let new_end = new_at
                .checked_add(replacement)
                .map_err(|_| FullReparseReason::UnsafeSynchronization)?;
            let new = TextRange::new(new_at, new_end)
                .map_err(|_| FullReparseReason::UnsafeSynchronization)?;
            segments.push(ChangeSegment { old: change.old_range, new });
            new_at = new_end;
            previous_start = start;
            previous_end = change.old_range.end();
            previous_was_insertion = start == previous_end;
        }
        let tail = TextSize::try_from_usize(
            old.len()
                .to_usize()
                .checked_sub(previous_end.to_usize())
                .ok_or(FullReparseReason::UnsafeSynchronization)?,
        )
        .map_err(|_| FullReparseReason::UnsafeSynchronization)?;
        let new_len = new_at
            .checked_add(tail)
            .map_err(|_| FullReparseReason::UnsafeSynchronization)?;
        Ok(Self { old_len: old.len(), new_len, segments: segments.into() })
    }

    pub fn old_len(&self) -> TextSize { self.old_len }
    pub fn new_len(&self) -> TextSize { self.new_len }
    pub fn segments(&self) -> &[ChangeSegment] { &self.segments }

    pub fn changed_old_range(&self) -> Option<TextRange> {
        let first = self.segments.first()?;
        let last = self.segments.last()?;
        TextRange::new(first.old.start(), last.old.end()).ok()
    }

    pub fn changed_new_range(&self) -> Option<TextRange> {
        let first = self.segments.first()?;
        let last = self.segments.last()?;
        TextRange::new(first.new.start(), last.new.end()).ok()
    }

    pub fn translate_unchanged(&self, old: TextRange) -> Option<TextRange> {
        if old.end() > self.old_len || self.segments.iter().any(|segment| {
            if segment.old.start() == segment.old.end() {
                old.start() < segment.old.start() && segment.old.start() < old.end()
            } else {
                old.start() < segment.old.end() && segment.old.start() < old.end()
            }
        }) {
            return None;
        }
        Some(TextRange::new(
            self.translate_start_boundary(old.start())?,
            self.translate_end_boundary(old.end())?,
        ).ok()?)
    }

    pub fn translate_start_boundary(&self, old: TextSize) -> Option<TextSize> {
        self.translate_boundary(old, true)
    }

    pub fn translate_end_boundary(&self, old: TextSize) -> Option<TextSize> {
        self.translate_boundary(old, false)
    }

    fn translate_boundary(&self, old: TextSize, start_bias: bool) -> Option<TextSize> {
        if old > self.old_len {
            return None;
        }
        let mut delta = 0_i64;
        for segment in self.segments.iter() {
            let insertion = segment.old.start() == segment.old.end();
            if !insertion && segment.old.start() < old && old < segment.old.end() {
                return None;
            }
            let before = segment.old.end() < old
                || (!insertion && segment.old.end() == old)
                || (insertion && (segment.old.start() < old || (start_bias && segment.old.start() == old)));
            if before {
                delta += segment.new.len().to_usize() as i64 - segment.old.len().to_usize() as i64;
            }
        }
        let translated = old.to_usize() as i64 + delta;
        (0..=u32::MAX as i64).contains(&translated)
            .then(|| TextSize::try_from_usize(translated as usize).ok())?
    }
}

fn changes_reconstruct(
    old: &SourceText,
    new: &SourceText,
    changes: &[TextChange],
) -> Result<bool, ParseError> {
    let mut rebuilt = String::with_capacity(new.len().to_usize());
    let mut cursor = TextSize::try_from_usize(0).map_err(|_| ParseError::WidthOverflow)?;
    for change in changes {
        let prefix = TextRange::new(cursor, change.old_range.start())
            .map_err(|_| ParseError::InvalidRange { range: change.old_range })?;
        rebuilt.push_str(old.slice(prefix).map_err(|_| ParseError::InvalidRange { range: prefix })?);
        rebuilt.push_str(&change.replacement);
        cursor = change.old_range.end();
    }
    let tail = TextRange::new(cursor, old.len())
        .map_err(|_| ParseError::InvalidRange { range: TextRange::new(cursor, cursor).unwrap() })?;
    rebuilt.push_str(old.slice(tail).map_err(|_| ParseError::InvalidRange { range: tail })?);
    Ok(rebuilt == new.shared().as_str())
}

/// Safely advance an OKF shell tree. A full parse is deliberately retained as
/// the oracle whenever a local synchronization proof is unavailable.
pub fn reparse_okf_markdown(
    previous: &SyntaxTree<OkfMarkdownLanguage>,
    new_text: SourceText,
    changes: &[TextChange],
) -> Result<ReparseOutcome<OkfMarkdownLanguage>, ParseError> {
    let old = SourceText::from_shared(Arc::new(previous.write_to_string()))
        .map_err(|_| ParseError::WidthOverflow)?;
    let map = ChangeMap::checked(&old, changes).map_err(|_| ParseError::StructuralInvariant {
        reason: "invalid incremental change map".into(),
    })?;
    if map.new_len() != new_text.len() {
        return Err(ParseError::StructuralInvariant { reason: "incremental changes do not reconstruct candidate source".into() });
    }
    if !changes_reconstruct(&old, &new_text, changes)? {
        return Err(ParseError::StructuralInvariant { reason: "incremental changes do not reconstruct candidate source".into() });
    }
    if changes.is_empty() {
        return Ok(ReparseOutcome::Incremental {
            tree: Arc::new(SyntaxTree::new(previous.root_green().clone(), Arc::from(previous.diagnostics()), MarkdownDialect::CommonMarkCurrent)),
            shared_source_independent_green: 0,
            reparsed_range: TextRange::new(TextSize::try_from_usize(0).unwrap(), old.len()).unwrap(),
        });
    }
    let parsed = parse_okf_markdown(new_text, MarkdownDialect::CommonMarkCurrent)?;
    Ok(ReparseOutcome::Full { tree: parsed.tree, reason: FullReparseReason::UnsafeSynchronization })
}
