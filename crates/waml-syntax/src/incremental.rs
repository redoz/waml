use std::{collections::HashMap, hash::Hash, sync::Arc};

use crate::{
    parse_okf_markdown, GreenElement, GreenError, GreenFactory, GreenNode, GreenText, GreenTrivia,
    MarkdownDialect, OkfMarkdownLanguage, ParseError, SourceText, SyntaxAnnotation, SyntaxElement,
    SyntaxLanguage, SyntaxNode, SyntaxTree, TextError, TextRange, TextSize,
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

pub struct RebasedGreen<L: SyntaxLanguage> {
    pub element: GreenElement<L>,
    pub shared_source_independent_green: usize,
}

fn rebase_text(
    text: &GreenText,
    new_text: &SourceText,
    map: &ChangeMap,
) -> Result<Option<GreenText>, GreenError> {
    Ok(Some(match text {
        GreenText::Static(value) => GreenText::Static(value),
        GreenText::Owned(value) => GreenText::Owned(value.clone()),
        GreenText::SourceSlice { range, .. } => GreenText::SourceSlice {
            source: new_text.clone(),
            range: match map.translate_unchanged(*range) {
                Some(range) => range,
                None => return Ok(None),
            },
        },
    }))
}

fn rebase_trivia(
    trivia: &GreenTrivia,
    new_text: &SourceText,
    map: &ChangeMap,
) -> Result<Option<GreenTrivia>, GreenError> {
    Ok(
        rebase_text(&trivia.text, new_text, map)?.map(|text| GreenTrivia {
            kind: trivia.kind,
            text,
        }),
    )
}

pub fn rebase_unchanged_green<L: SyntaxLanguage>(
    element: &GreenElement<L>,
    new_text: &SourceText,
    map: &ChangeMap,
) -> Result<Option<RebasedGreen<L>>, GreenError> {
    fn go<L: SyntaxLanguage>(
        element: &GreenElement<L>,
        new_text: &SourceText,
        map: &ChangeMap,
    ) -> Result<Option<(GreenElement<L>, usize)>, GreenError> {
        match element {
            GreenElement::Token(token) if token.is_source_independent() => {
                Ok(Some((element.clone(), 1)))
            }
            GreenElement::Token(token) => {
                let text = match rebase_text(token.text(), new_text, map)? {
                    Some(text) => text,
                    None => return Ok(None),
                };
                let leading = token
                    .leading_trivia()
                    .iter()
                    .map(|trivia| rebase_trivia(trivia, new_text, map))
                    .collect::<Result<Option<Vec<_>>, _>>()?;
                let trailing = token
                    .trailing_trivia()
                    .iter()
                    .map(|trivia| rebase_trivia(trivia, new_text, map))
                    .collect::<Result<Option<Vec<_>>, _>>()?;
                let (Some(leading), Some(trailing)) = (leading, trailing) else {
                    return Ok(None);
                };
                Ok(Some((
                    GreenElement::Token(GreenFactory::new().rebuild_token(
                        token,
                        text,
                        leading,
                        trailing,
                        token.syntax_annotations().into(),
                    )?),
                    0,
                )))
            }
            GreenElement::Node(node) if node.is_source_independent() => {
                Ok(Some((element.clone(), count_greens(element))))
            }
            GreenElement::Node(node) => {
                let mut shared = 0;
                let mut children = Vec::with_capacity(node.children().len());
                for child in node.children() {
                    let Some((child, child_shared)) = go(child, new_text, map)? else {
                        return Ok(None);
                    };
                    shared += child_shared;
                    children.push(child);
                }
                Ok(Some((
                    GreenElement::Node(GreenFactory::new().node_with_annotations(
                        node.kind(),
                        children,
                        node.annotations().into(),
                    )?),
                    shared,
                )))
            }
        }
    }
    fn count_greens<L: SyntaxLanguage>(element: &GreenElement<L>) -> usize {
        match element {
            GreenElement::Token(_) => 1,
            GreenElement::Node(node) => 1 + node.children().iter().map(count_greens).sum::<usize>(),
        }
    }
    Ok(
        go(element, new_text, map)?.map(|(element, shared_source_independent_green)| {
            RebasedGreen {
                element,
                shared_source_independent_green,
            }
        }),
    )
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
struct OccurrenceKey<K> {
    kind: K,
    range: TextRange,
    token: bool,
    ordinal: usize,
}

fn collect_occurrences<L: SyntaxLanguage>(
    node: SyntaxNode<L>,
    map: Option<&ChangeMap>,
    out: &mut HashMap<OccurrenceKey<L::Kind>, Vec<SyntaxAnnotation>>,
    ordinals: &mut HashMap<(L::Kind, TextRange, bool), usize>,
) {
    let add = |kind,
               range,
               token,
               annotations: &[SyntaxAnnotation],
               out: &mut HashMap<OccurrenceKey<L::Kind>, Vec<SyntaxAnnotation>>,
               ordinals: &mut HashMap<(L::Kind, TextRange, bool), usize>| {
        let Some(range) = map.map_or(Some(range), |map| map.translate_unchanged(range)) else {
            return;
        };
        let base = (kind, range, token);
        let ordinal = ordinals.entry(base).or_insert(0);
        let key = OccurrenceKey {
            kind,
            range,
            token,
            ordinal: *ordinal,
        };
        *ordinal += 1;
        if !annotations.is_empty() {
            out.insert(key, annotations.to_vec());
        }
    };
    add(
        node.kind(),
        node.range(),
        false,
        node.syntax_annotations(),
        out,
        ordinals,
    );
    for child in node.children() {
        match child {
            SyntaxElement::Node(child) => collect_occurrences(child, map, out, ordinals),
            SyntaxElement::Token(token) => add(
                token.kind(),
                token.range(),
                true,
                token.syntax_annotations(),
                out,
                ordinals,
            ),
        }
    }
}

pub fn transfer_mapped_annotations<L: SyntaxLanguage>(
    previous: &SyntaxTree<L>,
    candidate: &SyntaxTree<L>,
    map: &ChangeMap,
) -> GreenNode<L> {
    let mut previous_annotations = HashMap::new();
    collect_occurrences(
        previous.root(),
        Some(map),
        &mut previous_annotations,
        &mut HashMap::new(),
    );
    let mut candidate_occurrences = HashMap::new();
    collect_occurrences(
        candidate.root(),
        None,
        &mut candidate_occurrences,
        &mut HashMap::new(),
    );
    fn merge(
        existing: &[SyntaxAnnotation],
        copied: Option<&Vec<SyntaxAnnotation>>,
    ) -> Arc<[SyntaxAnnotation]> {
        let mut annotations = existing.to_vec();
        for annotation in copied.into_iter().flatten() {
            if !annotations
                .iter()
                .any(|present| present.id() == annotation.id())
            {
                annotations.push(annotation.clone());
            }
        }
        annotations.into()
    }
    fn rebuild<L: SyntaxLanguage>(
        node: &GreenNode<L>,
        path: &mut Vec<u32>,
        copied: &HashMap<OccurrenceKey<L::Kind>, Vec<SyntaxAnnotation>>,
        occurrences: &mut HashMap<(L::Kind, TextRange, bool), usize>,
        start: TextSize,
    ) -> GreenNode<L> {
        let range = TextRange::new(start, start.checked_add(node.width()).unwrap()).unwrap();
        let base = (node.kind(), range, false);
        let ordinal = *occurrences.entry(base).or_insert(0);
        *occurrences.get_mut(&base).unwrap() += 1;
        let node_annotations = merge(
            node.annotations(),
            copied.get(&OccurrenceKey {
                kind: node.kind(),
                range,
                token: false,
                ordinal,
            }),
        );
        let mut offset = start;
        let children = node
            .children()
            .iter()
            .enumerate()
            .map(|(index, child)| {
                let result = match child {
                    GreenElement::Node(child) => {
                        path.push(index as u32);
                        let child = rebuild(child, path, copied, occurrences, offset);
                        path.pop();
                        GreenElement::Node(child)
                    }
                    GreenElement::Token(token) => {
                        let range =
                            TextRange::new(offset, offset.checked_add(token.width()).unwrap())
                                .unwrap();
                        let base = (token.kind(), range, true);
                        let ordinal = *occurrences.entry(base).or_insert(0);
                        *occurrences.get_mut(&base).unwrap() += 1;
                        GreenElement::Token(GreenFactory::new().token_with_syntax_annotations(
                            token,
                            merge(
                                token.syntax_annotations(),
                                copied.get(&OccurrenceKey {
                                    kind: token.kind(),
                                    range,
                                    token: true,
                                    ordinal,
                                }),
                            ),
                        ))
                    }
                };
                offset = offset
                    .checked_add(match &result {
                        GreenElement::Node(node) => node.width(),
                        GreenElement::Token(token) => token.width(),
                    })
                    .unwrap();
                result
            })
            .collect::<Vec<_>>();
        GreenFactory::new()
            .node_with_annotations(node.kind(), children, node_annotations)
            .unwrap()
    }
    rebuild(
        candidate.root_green(),
        &mut Vec::new(),
        &previous_annotations,
        &mut HashMap::new(),
        TextSize::try_from_usize(0).unwrap(),
    )
}

impl ChangeMap {
    pub fn checked(old: &SourceText, changes: &[TextChange]) -> Result<Self, FullReparseReason> {
        let zero =
            TextSize::try_from_usize(0).map_err(|_| FullReparseReason::UnsafeSynchronization)?;
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
                start
                    .to_usize()
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
            segments.push(ChangeSegment {
                old: change.old_range,
                new,
            });
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
        Ok(Self {
            old_len: old.len(),
            new_len,
            segments: segments.into(),
        })
    }

    pub fn old_len(&self) -> TextSize {
        self.old_len
    }
    pub fn new_len(&self) -> TextSize {
        self.new_len
    }
    pub fn segments(&self) -> &[ChangeSegment] {
        &self.segments
    }

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
        if old.end() > self.old_len
            || self.segments.iter().any(|segment| {
                if segment.old.start() == segment.old.end() {
                    old.start() < segment.old.start() && segment.old.start() < old.end()
                } else {
                    old.start() < segment.old.end() && segment.old.start() < old.end()
                }
            })
        {
            return None;
        }
        Some(
            TextRange::new(
                self.translate_start_boundary(old.start())?,
                self.translate_end_boundary(old.end())?,
            )
            .ok()?,
        )
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
                || (insertion
                    && (segment.old.start() < old || (start_bias && segment.old.start() == old)));
            if before {
                let new_len = i64::try_from(segment.new.len().to_usize()).ok()?;
                let old_len = i64::try_from(segment.old.len().to_usize()).ok()?;
                let segment_delta = new_len.checked_sub(old_len)?;
                delta = delta.checked_add(segment_delta)?;
            }
        }
        let translated = i64::try_from(old.to_usize()).ok()?.checked_add(delta)?;
        (0..=u32::MAX as i64)
            .contains(&translated)
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
        let prefix = TextRange::new(cursor, change.old_range.start()).map_err(|_| {
            ParseError::InvalidRange {
                range: change.old_range,
            }
        })?;
        rebuilt.push_str(
            old.slice(prefix)
                .map_err(|_| ParseError::InvalidRange { range: prefix })?,
        );
        rebuilt.push_str(&change.replacement);
        cursor = change.old_range.end();
    }
    let tail = TextRange::new(cursor, old.len()).map_err(|_| ParseError::InvalidRange {
        range: TextRange::new(cursor, cursor).unwrap(),
    })?;
    rebuilt.push_str(
        old.slice(tail)
            .map_err(|_| ParseError::InvalidRange { range: tail })?,
    );
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
        return Err(ParseError::StructuralInvariant {
            reason: "incremental changes do not reconstruct candidate source".into(),
        });
    }
    if !changes_reconstruct(&old, &new_text, changes)? {
        return Err(ParseError::StructuralInvariant {
            reason: "incremental changes do not reconstruct candidate source".into(),
        });
    }
    if changes.is_empty() {
        return Ok(ReparseOutcome::Incremental {
            tree: Arc::new(SyntaxTree::new(
                previous.root_green().clone(),
                Arc::from(previous.diagnostics()),
                MarkdownDialect::CommonMarkCurrent,
            )),
            shared_source_independent_green: 0,
            reparsed_range: TextRange::new(TextSize::try_from_usize(0).unwrap(), old.len())
                .unwrap(),
        });
    }
    let parsed = parse_okf_markdown(new_text, MarkdownDialect::CommonMarkCurrent)?;
    Ok(ReparseOutcome::Full {
        tree: parsed.tree,
        reason: FullReparseReason::UnsafeSynchronization,
    })
}
