use std::{collections::HashMap, hash::Hash, sync::Arc};

use crate::{
    GreenElement, GreenError, GreenFactory, GreenNode, GreenText, GreenTrivia, MarkdownDialect,
    MarkdownStructureMap, OkfMarkdownLanguage, OkfMarkdownSyntaxKind, ParseError, SourceText,
    SyntaxAnnotation, SyntaxElement, SyntaxLanguage, SyntaxNode, SyntaxTree, TextError, TextRange,
    TextSize, TreeDiagnostic,
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
        let mut annotations: Vec<SyntaxAnnotation> = Vec::new();
        for annotation in existing.iter().chain(copied.into_iter().flatten()) {
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
    Ok(reparse_okf_markdown_with_structure(previous, new_text, changes)?.0)
}

/// Like [`reparse_okf_markdown`], while retaining the one candidate structure
/// map used for the synchronization proof and parser call.
#[doc(hidden)]
pub fn reparse_okf_markdown_with_structure(
    previous: &SyntaxTree<OkfMarkdownLanguage>,
    new_text: SourceText,
    changes: &[TextChange],
) -> Result<
    (
        ReparseOutcome<OkfMarkdownLanguage>,
        Arc<MarkdownStructureMap>,
    ),
    ParseError,
> {
    let old = SourceText::from_shared(Arc::new(previous.write_to_string()))
        .map_err(|_| ParseError::WidthOverflow)?;
    let new_structure = Arc::new(crate::markdown::map(
        &new_text,
        MarkdownDialect::CommonMarkCurrent,
    )?);
    let map = match ChangeMap::checked(&old, changes) {
        Ok(map) => map,
        Err(reason) => {
            let parsed = crate::shell::parse_okf_markdown_with_structure(
                new_text,
                MarkdownDialect::CommonMarkCurrent,
                new_structure.clone(),
            )?;
            return Ok((
                ReparseOutcome::Full {
                    tree: parsed.tree,
                    reason,
                },
                new_structure,
            ));
        }
    };
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
        let (root, shared_source_independent_green) =
            if green_uses_source(previous.root_green(), &new_text) {
                (
                    previous.root_green().clone(),
                    count_source_independent_greens(&GreenElement::Node(
                        previous.root_green().clone(),
                    )),
                )
            } else {
                let Some(rebased) = rebase_unchanged_green(
                    &GreenElement::Node(previous.root_green().clone()),
                    &new_text,
                    &map,
                )
                .map_err(|_| ParseError::WidthOverflow)?
                else {
                    return Err(ParseError::StructuralInvariant {
                        reason: "empty change map could not rebase unchanged syntax".into(),
                    });
                };
                let GreenElement::Node(root) = rebased.element else {
                    return Err(ParseError::StructuralInvariant {
                        reason: "rebased syntax root was not a node".into(),
                    });
                };
                (root, rebased.shared_source_independent_green)
            };
        return Ok((
            ReparseOutcome::Incremental {
                tree: Arc::new(SyntaxTree::new(
                    root,
                    Arc::from(previous.diagnostics()),
                    MarkdownDialect::CommonMarkCurrent,
                )),
                shared_source_independent_green,
                reparsed_range: TextRange::new(TextSize::try_from_usize(0).unwrap(), old.len())
                    .unwrap(),
            },
            new_structure,
        ));
    }
    let old_structure = Arc::new(crate::markdown::map(
        &old,
        MarkdownDialect::CommonMarkCurrent,
    )?);
    let full = |reason| -> Result<_, ParseError> {
        let parsed = crate::shell::parse_okf_markdown_with_structure(
            new_text.clone(),
            MarkdownDialect::CommonMarkCurrent,
            new_structure.clone(),
        )?;
        Ok((
            ReparseOutcome::Full {
                tree: parsed.tree,
                reason,
            },
            new_structure.clone(),
        ))
    };
    let old_frontmatter = crate::shell::frontmatter_range(&old, &old_structure)?;
    let new_frontmatter = crate::shell::frontmatter_range(&new_text, &new_structure)?;
    if !same_optional_range(old_frontmatter, new_frontmatter, &map)
        || !same_frontmatter_fences(&old, &new_text, old_frontmatter, new_frontmatter, &map)
    {
        return full(FullReparseReason::FrontmatterBoundaryChanged);
    }
    if !same_headings(&old_structure, &new_structure, &map) {
        return full(FullReparseReason::HeadingBoundaryChanged);
    }
    if !same_containers(&old_structure, &new_structure, &map) {
        return full(FullReparseReason::MarkdownContainerBoundaryChanged);
    }

    let windows = shell_windows(previous, &old)?;
    let Some(window) = select_window(&windows, &map) else {
        return full(FullReparseReason::UnsafeSynchronization);
    };
    let Some(new_range) = expanded_window_range(window.range, &map) else {
        return full(FullReparseReason::UnsafeSynchronization);
    };
    let parsed_window = crate::shell::parse_window(
        &new_text,
        &new_structure,
        crate::shell::ShellWindow {
            kind: window.kind,
            range: new_range,
        },
    )?;
    let mut children = Vec::new();
    let mut shared = 0;
    for (index, child) in previous.root_green().children().iter().enumerate() {
        if index == window.first {
            children.extend(parsed_window.elements.iter().cloned());
        }
        if (window.first..=window.last).contains(&index) {
            continue;
        }
        let Some(rebased) = rebase_unchanged_green(child, &new_text, &map)
            .map_err(|_| ParseError::WidthOverflow)?
        else {
            return full(FullReparseReason::UnsafeSynchronization);
        };
        shared += rebased.shared_source_independent_green;
        children.push(rebased.element);
    }
    let root = GreenFactory::new()
        .node(OkfMarkdownSyntaxKind::Root, children)
        .map_err(|_| ParseError::WidthOverflow)?;
    let mut diagnostics: Vec<TreeDiagnostic<_>> = previous
        .diagnostics()
        .iter()
        .filter_map(|diagnostic| {
            ((diagnostic.range.end() <= window.range.start()
                || diagnostic.range.start() >= window.range.end())
            .then(|| map.translate_unchanged(diagnostic.range))?)
            .map(|range| TreeDiagnostic {
                code: diagnostic.code,
                severity: diagnostic.severity,
                message: diagnostic.message.clone(),
                range,
            })
        })
        .collect();
    diagnostics.extend(parsed_window.diagnostics.iter().cloned());
    diagnostics.sort_by_key(|d| (d.range.start(), d.range.end(), d.code as u8));
    let candidate = SyntaxTree::new(root, diagnostics.into(), MarkdownDialect::CommonMarkCurrent);
    let root = if has_syntax_annotations(previous.root_green()) {
        transfer_mapped_annotations(previous, &candidate, &map)
    } else {
        candidate.root_green().clone()
    };
    Ok((
        ReparseOutcome::Incremental {
            tree: Arc::new(SyntaxTree::new(
                root,
                Arc::from(candidate.diagnostics()),
                MarkdownDialect::CommonMarkCurrent,
            )),
            shared_source_independent_green: shared,
            reparsed_range: new_range,
        },
        new_structure,
    ))
}

fn green_uses_source(node: &GreenNode<OkfMarkdownLanguage>, source: &SourceText) -> bool {
    node.children().iter().all(|child| match child {
        GreenElement::Node(child) => green_uses_source(child, source),
        GreenElement::Token(token) => std::iter::once(token.text())
            .chain(token.leading_trivia().iter().map(|trivia| &trivia.text))
            .chain(token.trailing_trivia().iter().map(|trivia| &trivia.text))
            .all(|text| match text {
                GreenText::SourceSlice { source: actual, .. } => {
                    Arc::ptr_eq(actual.shared(), source.shared())
                }
                GreenText::Static(_) | GreenText::Owned(_) => true,
            }),
    })
}

fn count_source_independent_greens(element: &GreenElement<OkfMarkdownLanguage>) -> usize {
    match element {
        GreenElement::Token(token) => usize::from(token.is_source_independent()),
        GreenElement::Node(node) if node.is_source_independent() => {
            1 + node
                .children()
                .iter()
                .map(count_source_independent_greens)
                .sum::<usize>()
        }
        GreenElement::Node(node) => node
            .children()
            .iter()
            .map(count_source_independent_greens)
            .sum(),
    }
}

#[derive(Clone, Copy)]
struct Window {
    kind: crate::shell::ShellWindowKind,
    range: TextRange,
    first: usize,
    last: usize,
}

fn shell_windows(
    previous: &SyntaxTree<OkfMarkdownLanguage>,
    old: &SourceText,
) -> Result<Vec<Window>, ParseError> {
    let children = previous.root_green().children();
    let mut start = TextSize::try_from_usize(0).unwrap();
    let mut result = Vec::new();
    for (index, child) in children.iter().enumerate() {
        let width = match child {
            GreenElement::Node(node) => node.width(),
            GreenElement::Token(token) => token.width(),
        };
        let end = start
            .checked_add(width)
            .map_err(|_| ParseError::WidthOverflow)?;
        let range = TextRange::new(start, end).map_err(|_| ParseError::WidthOverflow)?;
        let kind = match child {
            GreenElement::Node(node) => match node.kind() {
                OkfMarkdownSyntaxKind::Frontmatter => {
                    Some(crate::shell::ShellWindowKind::Frontmatter)
                }
                OkfMarkdownSyntaxKind::Heading => Some(crate::shell::ShellWindowKind::Heading),
                OkfMarkdownSyntaxKind::MarkdownRegion => {
                    Some(crate::shell::ShellWindowKind::MarkdownRegion)
                }
                _ => None,
            },
            _ => None,
        };
        if let Some(kind) = kind {
            result.push(Window {
                kind,
                range,
                first: index,
                last: index,
            });
        }
        start = end;
    }
    let eof = children
        .len()
        .checked_sub(1)
        .ok_or(ParseError::WidthOverflow)?;
    let tail_first = if eof > 0
        && matches!(&children[eof - 1], GreenElement::Node(node) if node.kind() == OkfMarkdownSyntaxKind::MarkdownRegion)
    {
        eof - 1
    } else {
        eof
    };
    let tail_start = children[..tail_first].iter().try_fold(
        TextSize::try_from_usize(0).unwrap(),
        |at, child| {
            at.checked_add(match child {
                GreenElement::Node(node) => node.width(),
                GreenElement::Token(token) => token.width(),
            })
            .map_err(|_| ParseError::WidthOverflow)
        },
    )?;
    result.retain(|window| window.first != tail_first);
    result.push(Window {
        kind: crate::shell::ShellWindowKind::Tail,
        range: TextRange::new(tail_start, old.len()).map_err(|_| ParseError::WidthOverflow)?,
        first: tail_first,
        last: eof,
    });
    Ok(result)
}

fn select_window(windows: &[Window], map: &ChangeMap) -> Option<Window> {
    let hull = map.changed_old_range()?;
    let zero = hull.start() == hull.end();
    let mut candidates: Vec<_> = windows
        .iter()
        .copied()
        .filter(|window| {
            if zero {
                window.range.start() <= hull.start() && hull.start() <= window.range.end()
            } else {
                window.range.start() <= hull.start() && hull.end() <= window.range.end()
            }
        })
        .collect();
    if zero {
        if hull.start() == map.old_len() {
            return candidates
                .into_iter()
                .find(|window| window.kind == crate::shell::ShellWindowKind::Tail);
        }
        if let Some(raw) = candidates.iter().find(|window| {
            window.kind == crate::shell::ShellWindowKind::MarkdownRegion
                && window.range.start() == hull.start()
        }) {
            return Some(*raw);
        }
        if let Some(tail) = candidates.iter().find(|window| {
            window.kind == crate::shell::ShellWindowKind::Tail
                && window.range.start() == hull.start()
        }) {
            return Some(*tail);
        }
        (candidates.len() == 1).then(|| candidates.remove(0))
    } else {
        (candidates.len() == 1).then(|| candidates.remove(0))
    }
}

fn expanded_window_range(old: TextRange, map: &ChangeMap) -> Option<TextRange> {
    let mut start = map.translate_start_boundary(old.start())?;
    let mut end = map.translate_end_boundary(old.end())?;
    for segment in map.segments() {
        if segment.old.start() == segment.old.end()
            && old.start() <= segment.old.start()
            && segment.old.start() <= old.end()
        {
            start = start.min(segment.new.start());
            end = end.max(segment.new.end());
        }
    }
    TextRange::new(start, end).ok()
}

fn map_range(range: TextRange, map: &ChangeMap) -> Option<TextRange> {
    TextRange::new(
        map.translate_start_boundary(range.start())?,
        map.translate_end_boundary(range.end())?,
    )
    .ok()
}
fn same_optional_range(old: Option<TextRange>, new: Option<TextRange>, map: &ChangeMap) -> bool {
    old.and_then(|range| map_range(range, map)) == new
}

fn same_frontmatter_fences(
    old: &SourceText,
    new: &SourceText,
    old_frontmatter: Option<TextRange>,
    new_frontmatter: Option<TextRange>,
    map: &ChangeMap,
) -> bool {
    let (Some(old_frontmatter), Some(new_frontmatter)) = (old_frontmatter, new_frontmatter) else {
        return old_frontmatter.is_none() && new_frontmatter.is_none();
    };
    let Some((old_open, old_close)) = frontmatter_fences(old, old_frontmatter) else {
        return false;
    };
    let Some((new_open, new_close)) = frontmatter_fences(new, new_frontmatter) else {
        return false;
    };
    let same = |old_range: TextRange, new_range: TextRange| {
        map_range(old_range, map) == Some(new_range)
            && old.slice(old_range).ok() == new.slice(new_range).ok()
    };
    same(old_open, new_open)
        && match (old_close, new_close) {
            (Some(old_close), Some(new_close)) => same(old_close, new_close),
            (None, None) => true,
            _ => false,
        }
}

fn frontmatter_fences(
    text: &SourceText,
    frontmatter: TextRange,
) -> Option<(TextRange, Option<TextRange>)> {
    let source = text.shared();
    let start = frontmatter.start().to_usize();
    let end = frontmatter.end().to_usize();
    let open_end = source[start..end]
        .find('\n')
        .map_or(end, |at| start + at + 1);
    let open = TextRange::new(
        frontmatter.start(),
        TextSize::try_from_usize(open_end).ok()?,
    )
    .ok()?;
    if source[start..open_end].trim() != "---" {
        return None;
    }
    let significant_end = source[..end].trim_end_matches(['\r', '\n']).len();
    let close_start = source[..significant_end]
        .rfind('\n')
        .map_or(start, |at| at + 1);
    let close = matches!(source[close_start..end].trim(), "---" | "...")
        .then(|| {
            TextRange::new(
                TextSize::try_from_usize(close_start).ok()?,
                frontmatter.end(),
            )
            .ok()
        })
        .flatten();
    Some((open, close))
}
fn same_ranges(old: &[TextRange], new: &[TextRange], map: &ChangeMap) -> bool {
    let mut old: Option<Vec<_>> = old
        .iter()
        .copied()
        .map(|range| map_range(range, map))
        .collect();
    let mut new = new.to_vec();
    old.as_mut()
        .unwrap()
        .sort_by_key(|range| (range.start(), range.end()));
    new.sort_by_key(|range| (range.start(), range.end()));
    old.unwrap() == new
}
fn same_headings(old: &MarkdownStructureMap, new: &MarkdownStructureMap, map: &ChangeMap) -> bool {
    let old = old
        .headings
        .iter()
        .chain(old.nested_headings.iter())
        .map(|heading| {
            Some((
                heading.level,
                translate_heading_boundaries(heading.range, map)?,
                translate_heading_boundaries(heading.text_range, map)?,
            ))
        })
        .collect::<Option<Vec<_>>>();
    let new = new
        .headings
        .iter()
        .chain(new.nested_headings.iter())
        .map(|heading| (heading.level, heading.range, heading.text_range))
        .collect::<Vec<_>>();
    old == Some(new)
}

fn translate_heading_boundaries(range: TextRange, map: &ChangeMap) -> Option<TextRange> {
    TextRange::new(
        map.translate_start_boundary(range.start())?,
        map.translate_end_boundary(range.end())?,
    )
    .ok()
}

fn has_syntax_annotations(node: &GreenNode<OkfMarkdownLanguage>) -> bool {
    !node.annotations().is_empty()
        || node.children().iter().any(|child| match child {
            GreenElement::Node(child) => has_syntax_annotations(child),
            GreenElement::Token(token) => !token.syntax_annotations().is_empty(),
        })
}
fn same_containers(
    old: &MarkdownStructureMap,
    new: &MarkdownStructureMap,
    map: &ChangeMap,
) -> bool {
    same_ranges(&old.protected_ranges, &new.protected_ranges, map)
        && same_ranges(&old.opaque_ranges, &new.opaque_ranges, map)
        && same_ranges(&old.list_item_lines, &new.list_item_lines, map)
        && same_ranges(
            &old.tab_indented_item_lines,
            &new.tab_indented_item_lines,
            map,
        )
}
