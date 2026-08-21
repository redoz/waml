use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    sync::Arc,
};

use crate::{
    GreenElement, GreenError, GreenFactory, GreenNode, GreenText, GreenTrivia,
    MarkdownStructureMap, OkfMarkdownLanguage, OkfMarkdownSyntaxKind, OkfSyntaxDiagnosticCode,
    ParseError, SourceText, SyntaxAnnotation, SyntaxElement, SyntaxLanguage, SyntaxNode,
    SyntaxTree, TextError, TextRange, TextSize, TreeDiagnostic,
};

/// One replacement expressed in checked, half-open byte offsets of the old text.
#[derive(Clone, Debug)]
pub struct TextChange {
    pub old_range: TextRange,
    pub replacement: Arc<str>,
}

#[cfg(test)]
mod low_level_tests;
#[cfg(test)]
mod properties;

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

/// The outcome of successfully planning a windowed (non-full) reparse: the
/// spliced tree plus the bookkeeping [`ReparseOutcome::Incremental`] needs.
/// Carries exactly the fields [`reparse_okf_markdown_with_structure`]'s tail
/// reads back out of a successful [`plan_window_reparse`] call.
struct WindowPlan {
    tree: Arc<SyntaxTree<OkfMarkdownLanguage>>,
    shared_source_independent_green: usize,
    reparsed_range: TextRange,
    public_structure: Arc<MarkdownStructureMap>,
}

/// Plans a windowed reparse against `old`/`new_text`, or reports the reason a
/// full reparse is required. Guard checks that only rule out incremental
/// reuse (island/frontmatter/heading/container boundaries, unsafe
/// synchronization) come back as `Ok(Err(reason))` — the caller's job is to
/// fall back to a full parse for those. Hard structural errors (arithmetic
/// overflow, an unrecoverable tree shape) come back as the outer `Err`,
/// exactly as they did before this function existed; they are not
/// recoverable by falling back to a full parse of `new_text`.
#[allow(clippy::too_many_arguments)]
fn plan_window_reparse(
    previous: &SyntaxTree<OkfMarkdownLanguage>,
    old: &SourceText,
    new_text: &SourceText,
    dialect: crate::MarkdownDialect,
    changes: &[TextChange],
    map: &ChangeMap,
    new_structure: &Arc<crate::markdown::ShellStructure>,
    old_projection: &Arc<MarkdownStructureMap>,
) -> Result<Result<WindowPlan, FullReparseReason>, ParseError> {
    let old_structure = Arc::new(crate::markdown::shell_map(old, dialect)?);
    if old_projection.islands.iter().any(|island| {
        let island_range = island.heading_range.cover(island.content_range);
        map.segments().iter().any(|segment| {
            if segment.old.start() == segment.old.end() {
                island_range.start() <= segment.old.start()
                    && segment.old.start() <= island_range.end()
            } else {
                segment.old.start() < island_range.end() && island_range.start() < segment.old.end()
            }
        })
    }) {
        return Ok(Err(FullReparseReason::IslandBoundaryChanged));
    }
    if dialect.waml_sections()
        && new_structure
            .headings
            .iter()
            .chain(new_structure.nested_headings.iter())
            .filter(|heading| {
                crate::markdown::waml_kind(new_text.shared(), heading.text_range).is_some()
            })
            .any(|heading| {
                map.segments().iter().any(|segment| {
                    segment.new.start() < heading.range.end()
                        && heading.range.start() < segment.new.end()
                })
            })
    {
        return Ok(Err(FullReparseReason::IslandBoundaryChanged));
    }
    let old_frontmatter = crate::shell::frontmatter_range(old, &old_structure)?;
    let new_frontmatter = crate::shell::frontmatter_range(new_text, new_structure)?;
    if !same_optional_range(old_frontmatter, new_frontmatter, map)
        || !same_frontmatter_fences(old, new_text, old_frontmatter, new_frontmatter, map)
        || edit_touches_frontmatter_leading_whitespace(
            old,
            new_text,
            old_frontmatter,
            new_frontmatter,
            map,
        )
    {
        return Ok(Err(FullReparseReason::FrontmatterBoundaryChanged));
    }
    if !same_headings(&old_structure, new_structure, map) {
        return Ok(Err(FullReparseReason::HeadingBoundaryChanged));
    }
    if !same_containers(&old_structure, new_structure, map) {
        return Ok(Err(FullReparseReason::MarkdownContainerBoundaryChanged));
    }
    if width_change_may_shift_reference_definition(previous, map) {
        return Ok(Err(FullReparseReason::UnsafeSynchronization));
    }
    if crate::markdown::reparse::change_may_affect_reference_use(
        old,
        previous.root_green(),
        new_text,
        changes,
        map,
    )? {
        return Ok(Err(FullReparseReason::UnsafeSynchronization));
    }

    let windows = shell_windows(previous, old)?;
    let Some(mut window) = select_window(&windows, map) else {
        return Ok(Err(FullReparseReason::UnsafeSynchronization));
    };
    if window.kind == crate::shell::ShellWindowKind::Heading
        && window.range.end() == old.len()
        && matches!(new_text.shared().as_bytes().last(), Some(b' ' | b'\t'))
    {
        window.kind = crate::shell::ShellWindowKind::Tail;
        window.last = previous
            .root_green()
            .children()
            .len()
            .checked_sub(1)
            .ok_or(ParseError::WidthOverflow)?;
    }
    // The splice below swaps out root children `first..=last` and reparses
    // `window.range` in their place, so the two must describe the same span.
    // The tail window is anchored at the document end, which is past the start
    // of an end-of-file token that carries trailing spaces or tabs as leading
    // trivia — splicing that window in would drop the whitespace from the
    // tree. Fall back rather than publish a tree that no longer writes back to
    // its own source.
    if !window_spans_its_children(previous, &window)? {
        return Ok(Err(FullReparseReason::UnsafeSynchronization));
    }
    if previous.diagnostics().iter().any(|diagnostic| {
        diagnostic.range.start() == diagnostic.range.end()
            && (diagnostic.range.start() == window.range.start()
                || diagnostic.range.start() == window.range.end())
    }) {
        return Ok(Err(FullReparseReason::UnsafeSynchronization));
    }
    let Some(new_range) = expanded_window_range(window.range, map) else {
        return Ok(Err(FullReparseReason::UnsafeSynchronization));
    };
    if crate::markdown::reparse::window_reparse_may_lose_reference_resolution(
        old,
        previous.root_green(),
        window.range,
        new_text,
        new_range,
    )? {
        return Ok(Err(FullReparseReason::UnsafeSynchronization));
    }
    let parsed_window = match crate::shell::parse_window(
        new_text,
        new_structure,
        crate::shell::ShellWindow {
            kind: window.kind,
            range: new_range,
        },
    ) {
        Ok(parsed_window) => parsed_window,
        Err(ParseError::StructuralInvariant { reason })
            if reason.as_ref() == "shell window parser did not consume the selected range" =>
        {
            return Ok(Err(FullReparseReason::UnsafeSynchronization));
        }
        Err(error) => return Err(error),
    };
    if parsed_window
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == OkfSyntaxDiagnosticCode::MalformedBlock)
    {
        return Ok(Err(FullReparseReason::UnsafeSynchronization));
    }
    let mut children = Vec::new();
    for (index, child) in previous.root_green().children().iter().enumerate() {
        if index == window.first {
            children.extend(parsed_window.elements.iter().cloned());
        }
        if (window.first..=window.last).contains(&index) {
            continue;
        }
        let Some(rebased) =
            rebase_unchanged_green(child, new_text, map).map_err(|_| ParseError::WidthOverflow)?
        else {
            return Ok(Err(FullReparseReason::UnsafeSynchronization));
        };
        children.push(rebased.element);
    }
    let root = GreenFactory::new()
        .node(OkfMarkdownSyntaxKind::Root, children)
        .map_err(|_| ParseError::WidthOverflow)?;
    // Carry the untouched diagnostics across in their existing order and drop
    // the window's own in the gap they came from. Sorting the merged list by
    // position instead would reorder diagnostics a full parse emits together —
    // a block reports its interior problems before the recovery verdict that
    // spans the whole block — and the two would no longer be interchangeable.
    let carry = |diagnostic: &TreeDiagnostic<OkfSyntaxDiagnosticCode>| {
        map.translate_unchanged(diagnostic.range)
            .map(|range| TreeDiagnostic {
                code: diagnostic.code,
                severity: diagnostic.severity,
                message: diagnostic.message.clone(),
                range,
            })
    };
    let mut diagnostics: Vec<TreeDiagnostic<_>> = previous
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.range.end() <= window.range.start())
        .filter_map(carry)
        .collect();
    diagnostics.extend(parsed_window.diagnostics.iter().cloned());
    diagnostics.extend(
        previous
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.range.start() >= window.range.end())
            .filter_map(carry),
    );
    // A malformed block anywhere in the merged diagnostics (the window's own
    // malformed blocks already fell back above, so this catches carried-over
    // ones outside the window) means the shell is not trustworthy enough to
    // synchronize against; hand the document back to the full parser.
    if dialect.waml_sections()
        && diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == OkfSyntaxDiagnosticCode::MalformedBlock)
    {
        return Ok(Err(FullReparseReason::UnsafeSynchronization));
    }
    let candidate = SyntaxTree::new(root, diagnostics.into(), dialect);
    let root = if has_syntax_annotations(previous.root_green()) {
        // Width arithmetic that does not fit means the spliced tree is not a
        // sound base to carry annotations onto; degrade to a full parse rather
        // than ending the document's reparse in an error.
        match transfer_mapped_annotations(previous, &candidate, map) {
            Ok(root) => root,
            Err(_) => return Ok(Err(FullReparseReason::UnsafeSynchronization)),
        }
    } else {
        candidate.root_green().clone()
    };
    let shared_source_independent_green =
        count_shared_source_independent_greens(previous.root_green(), &root);
    let tree = Arc::new(SyntaxTree::new(
        root,
        Arc::from(candidate.diagnostics()),
        dialect,
    ));
    let public_structure = Arc::new(crate::markdown::from_tree(&tree, new_text.shared())?);
    // The island-boundary guards above refuse any change that touches an old
    // island or a new WAML heading, so a successful window reparse must keep
    // the island count. A mismatch means the splice drifted; fall back.
    if dialect.waml_sections() && public_structure.islands.len() != old_projection.islands.len() {
        return Ok(Err(FullReparseReason::IslandBoundaryChanged));
    }
    // Debug-only oracle: cross-check the spliced result against a full parse.
    // This used to run unconditionally in production, making every
    // "incremental" reparse strictly more expensive than a full one; the
    // property suites (built with debug assertions) keep exercising it.
    #[cfg(debug_assertions)]
    if dialect.waml_sections() {
        let oracle = crate::markdown::parser::parse_with_structure(
            new_text.clone(),
            dialect,
            new_structure.clone(),
        )?;
        debug_assert!(
            !oracle
                .tree
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code == OkfSyntaxDiagnosticCode::MalformedBlock),
            "incremental reparse accepted a document the full parser marks malformed"
        );
        debug_assert_eq!(
            public_structure.islands.len(),
            oracle.structure.islands.len(),
            "incremental reparse island count diverged from the full parse"
        );
    }
    Ok(Ok(WindowPlan {
        tree,
        shared_source_independent_green,
        reparsed_range: new_range,
        public_structure,
    }))
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
) -> Result<GreenNode<L>, ParseError> {
    // Only the previous tree's annotations are indexed. `rebuild` below walks
    // the candidate green tree itself and re-derives each occurrence ordinal as
    // it goes, so a second `collect_occurrences` pass over the candidate red
    // tree -- which this used to do, into a map nothing ever read -- is a whole
    // extra traversal of the document for no result. It was the single largest
    // item in an incremental reparse.
    let mut previous_annotations = HashMap::new();
    collect_occurrences(
        previous.root(),
        Some(map),
        &mut previous_annotations,
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
        copied: &HashMap<OccurrenceKey<L::Kind>, Vec<SyntaxAnnotation>>,
        occurrences: &mut HashMap<(L::Kind, TextRange, bool), usize>,
        start: TextSize,
    ) -> Result<GreenNode<L>, ParseError> {
        let range = TextRange::new(
            start,
            start
                .checked_add(node.width())
                .map_err(|_| ParseError::WidthOverflow)?,
        )
        .map_err(|_| ParseError::WidthOverflow)?;
        let base = (node.kind(), range, false);
        let slot = occurrences.entry(base).or_insert(0);
        let ordinal = *slot;
        *slot += 1;
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
        let mut children_changed = false;
        let mut children = Vec::with_capacity(node.children().len());
        for child in node.children() {
            let result = match child {
                GreenElement::Node(child) => {
                    let rebuilt = rebuild(child, copied, occurrences, offset)?;
                    children_changed |= !Arc::ptr_eq(child, &rebuilt);
                    GreenElement::Node(rebuilt)
                }
                GreenElement::Token(token) => {
                    let range = TextRange::new(
                        offset,
                        offset
                            .checked_add(token.width())
                            .map_err(|_| ParseError::WidthOverflow)?,
                    )
                    .map_err(|_| ParseError::WidthOverflow)?;
                    let base = (token.kind(), range, true);
                    let slot = occurrences.entry(base).or_insert(0);
                    let ordinal = *slot;
                    *slot += 1;
                    let annotations = merge(
                        token.syntax_annotations(),
                        copied.get(&OccurrenceKey {
                            kind: token.kind(),
                            range,
                            token: true,
                            ordinal,
                        }),
                    );
                    let rebuilt = if annotations.as_ref() == token.syntax_annotations() {
                        token.clone()
                    } else {
                        GreenFactory::new().token_with_syntax_annotations(token, annotations)
                    };
                    children_changed |= !Arc::ptr_eq(token, &rebuilt);
                    GreenElement::Token(rebuilt)
                }
            };
            offset = offset
                .checked_add(match &result {
                    GreenElement::Node(node) => node.width(),
                    GreenElement::Token(token) => token.width(),
                })
                .map_err(|_| ParseError::WidthOverflow)?;
            children.push(result);
        }
        if !children_changed && node_annotations.as_ref() == node.annotations() {
            Ok(node.clone())
        } else {
            GreenFactory::new()
                .node_with_annotations(node.kind(), children, node_annotations)
                .map_err(|_| ParseError::WidthOverflow)
        }
    }
    rebuild(
        candidate.root_green(),
        &previous_annotations,
        &mut HashMap::new(),
        TextSize::try_from_usize(0).map_err(|_| ParseError::WidthOverflow)?,
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
        TextRange::new(
            self.translate_start_boundary(old.start())?,
            self.translate_end_boundary(old.end())?,
        )
        .ok()
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
    let mut cursor = TextSize::try_from_usize(0).map_err(|_| ParseError::WidthOverflow)?;
    let mut new_cursor = cursor;
    for change in changes {
        let prefix = TextRange::new(cursor, change.old_range.start()).map_err(|_| {
            ParseError::InvalidRange {
                range: change.old_range,
            }
        })?;
        let new_prefix = TextRange::new(
            new_cursor,
            new_cursor
                .checked_add(prefix.len())
                .map_err(|_| ParseError::WidthOverflow)?,
        )
        .map_err(|_| ParseError::WidthOverflow)?;
        if old
            .slice(prefix)
            .map_err(|_| ParseError::InvalidRange { range: prefix })?
            != new
                .slice(new_prefix)
                .map_err(|_| ParseError::InvalidRange { range: new_prefix })?
        {
            return Ok(false);
        }
        new_cursor = new_prefix.end();
        let replacement = TextSize::try_from_usize(change.replacement.len())
            .map_err(|_| ParseError::WidthOverflow)?;
        let new_replacement = TextRange::new(
            new_cursor,
            new_cursor
                .checked_add(replacement)
                .map_err(|_| ParseError::WidthOverflow)?,
        )
        .map_err(|_| ParseError::WidthOverflow)?;
        if new
            .slice(new_replacement)
            .map_err(|_| ParseError::InvalidRange {
                range: new_replacement,
            })?
            != change.replacement.as_ref()
        {
            return Ok(false);
        }
        new_cursor = new_replacement.end();
        cursor = change.old_range.end();
    }
    let tail = TextRange::new(cursor, old.len()).map_err(|_| ParseError::InvalidRange {
        range: TextRange::new(cursor, cursor).unwrap(),
    })?;
    let new_tail = TextRange::new(new_cursor, new.len()).map_err(|_| ParseError::WidthOverflow)?;
    Ok(old
        .slice(tail)
        .map_err(|_| ParseError::InvalidRange { range: tail })?
        == new
            .slice(new_tail)
            .map_err(|_| ParseError::InvalidRange { range: new_tail })?)
}

fn walk_green_texts<L: SyntaxLanguage>(
    root: &GreenNode<L>,
    f: &mut impl FnMut(&GreenText) -> Option<()>,
) -> Option<()> {
    let mut frames = vec![root.children().iter()];
    while let Some(frame) = frames.last_mut() {
        let Some(element) = frame.next() else {
            frames.pop();
            continue;
        };
        match element {
            GreenElement::Node(node) => frames.push(node.children().iter()),
            GreenElement::Token(token) => {
                for text in token
                    .leading_trivia()
                    .iter()
                    .map(|x| &x.text)
                    .chain(std::iter::once(token.text()))
                    .chain(token.trailing_trivia().iter().map(|x| &x.text))
                {
                    f(text)?;
                }
            }
        }
    }
    Some(())
}

// Two depth-first passes: discover one backing allocation, then prove the
// emitted leaf stream covers it exactly. Any uncertainty chooses full parsing.
fn recover_exact_source<L: SyntaxLanguage>(root: &GreenNode<L>) -> Option<SourceText> {
    let mut source: Option<SourceText> = None;
    walk_green_texts(root, &mut |text| {
        if let GreenText::SourceSlice { source: found, .. } = text {
            match &source {
                Some(expected) if !Arc::ptr_eq(expected.shared(), found.shared()) => return None,
                Some(_) => {}
                None => source = Some(found.clone()),
            }
        }
        Some(())
    })?;
    let source = source?;
    verify_exact_source(root, &source).then_some(source)
}

// The proving pass of [`recover_exact_source`], reusable when a candidate
// backing source is already known (e.g. the previous snapshot's text).
fn verify_exact_source<L: SyntaxLanguage>(root: &GreenNode<L>, source: &SourceText) -> bool {
    let mut offset = match TextSize::try_from_usize(0) {
        Ok(offset) => offset,
        Err(_) => return false,
    };
    let verified = walk_green_texts(root, &mut |text| {
        match text {
            GreenText::SourceSlice {
                source: found,
                range,
            } => {
                Arc::ptr_eq(source.shared(), found.shared()).then_some(())?;
                (range.start() == offset).then_some(())?;
                offset = range.end();
            }
            GreenText::Static(value) => {
                let end = offset
                    .checked_add(TextSize::try_from_usize(value.len()).ok()?)
                    .ok()?;
                (source.slice(TextRange::new(offset, end).ok()?).ok()? == *value).then_some(())?;
                offset = end;
            }
            GreenText::Owned(value) => {
                let end = offset
                    .checked_add(TextSize::try_from_usize(value.len()).ok()?)
                    .ok()?;
                (source.slice(TextRange::new(offset, end).ok()?).ok()? == value.as_ref())
                    .then_some(())?;
                offset = end;
            }
        }
        Some(())
    });
    verified.is_some() && offset == source.len() && root.width() == source.len()
}

/// Safely advance an OKF shell tree. A full parse is deliberately retained as
/// the oracle whenever a local synchronization proof is unavailable.
/// Test seam: production always wants the structure map too and calls
/// [`reparse_okf_markdown_with_structure`] directly.
#[cfg(test)]
pub(crate) fn reparse_okf_markdown(
    previous: &SyntaxTree<OkfMarkdownLanguage>,
    new_text: SourceText,
    changes: &[TextChange],
) -> Result<ReparseOutcome<OkfMarkdownLanguage>, ParseError> {
    Ok(reparse_okf_markdown_with_structure(previous, new_text, changes, None, None)?.0)
}

/// Like [`reparse_okf_markdown`], while returning the public structure derived
/// from the completed tree. A separate internal map drives synchronization.
///
/// `previous_text` and `previous_projection` are optional reuse hints from the
/// caller's snapshot of `previous`: when they verifiably match the tree they
/// replace a full source reconstruction and a full old-tree projection.
#[doc(hidden)]
pub(crate) fn reparse_okf_markdown_with_structure(
    previous: &SyntaxTree<OkfMarkdownLanguage>,
    new_text: SourceText,
    changes: &[TextChange],
    previous_text: Option<&SourceText>,
    previous_projection: Option<&Arc<MarkdownStructureMap>>,
) -> Result<
    (
        ReparseOutcome<OkfMarkdownLanguage>,
        Arc<MarkdownStructureMap>,
    ),
    ParseError,
> {
    let dialect = previous.dialect();
    let new_structure = Arc::new(crate::markdown::shell_map(&new_text, dialect)?);
    let full = |reason| -> Result<_, ParseError> {
        let parsed = crate::markdown::parser::parse_with_structure(
            new_text.clone(),
            dialect,
            new_structure.clone(),
        )?;
        let public_structure = parsed.structure.clone();
        Ok((
            ReparseOutcome::Full {
                tree: parsed.tree,
                reason,
            },
            public_structure,
        ))
    };
    let recovered = match previous_text {
        Some(text) if verify_exact_source(previous.root_green(), text) => Some(text.clone()),
        _ => recover_exact_source(previous.root_green()),
    };
    let Some(old) = recovered else {
        return full(FullReparseReason::UnsafeSynchronization);
    };
    let map = match ChangeMap::checked(&old, changes) {
        Ok(map) => map,
        Err(reason) => {
            return full(reason);
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
        let tree = Arc::new(SyntaxTree::new(
            root,
            Arc::from(previous.diagnostics()),
            dialect,
        ));
        let public_structure = Arc::new(crate::markdown::from_tree(&tree, new_text.shared())?);
        return Ok((
            ReparseOutcome::Incremental {
                tree,
                shared_source_independent_green,
                reparsed_range: TextRange::new(TextSize::try_from_usize(0).unwrap(), old.len())
                    .unwrap(),
            },
            public_structure,
        ));
    }
    let old_projection: Arc<MarkdownStructureMap> = match previous_projection {
        Some(projection)
            if previous_text.is_some_and(|text| Arc::ptr_eq(text.shared(), old.shared())) =>
        {
            projection.clone()
        }
        _ => Arc::new(crate::markdown::from_tree(previous, old.shared())?),
    };
    match plan_window_reparse(
        previous,
        &old,
        &new_text,
        dialect,
        changes,
        &map,
        &new_structure,
        &old_projection,
    )? {
        Ok(plan) => Ok((
            ReparseOutcome::Incremental {
                tree: plan.tree,
                shared_source_independent_green: plan.shared_source_independent_green,
                reparsed_range: plan.reparsed_range,
            },
            plan.public_structure,
        )),
        Err(reason) => full(reason),
    }
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

fn count_shared_source_independent_greens<L: SyntaxLanguage>(
    previous: &GreenNode<L>,
    current: &GreenNode<L>,
) -> usize {
    fn collect<L: SyntaxLanguage>(
        element: &GreenElement<L>,
        nodes: &mut HashSet<usize>,
        tokens: &mut HashSet<usize>,
    ) {
        match element {
            GreenElement::Node(node) => {
                if node.is_source_independent() {
                    nodes.insert(Arc::as_ptr(node) as usize);
                }
                for child in node.children() {
                    collect(child, nodes, tokens);
                }
            }
            GreenElement::Token(token) => {
                if token.is_source_independent() {
                    tokens.insert(Arc::as_ptr(token) as usize);
                }
            }
        }
    }
    fn count<L: SyntaxLanguage>(
        element: &GreenElement<L>,
        nodes: &HashSet<usize>,
        tokens: &HashSet<usize>,
    ) -> usize {
        match element {
            GreenElement::Node(node) => {
                usize::from(
                    node.is_source_independent() && nodes.contains(&(Arc::as_ptr(node) as usize)),
                ) + node
                    .children()
                    .iter()
                    .map(|child| count(child, nodes, tokens))
                    .sum::<usize>()
            }
            GreenElement::Token(token) => usize::from(
                token.is_source_independent() && tokens.contains(&(Arc::as_ptr(token) as usize)),
            ),
        }
    }

    let mut nodes = HashSet::new();
    let mut tokens = HashSet::new();
    collect(
        &GreenElement::Node(previous.clone()),
        &mut nodes,
        &mut tokens,
    );
    count(&GreenElement::Node(current.clone()), &nodes, &tokens)
}

#[derive(Clone, Copy)]
struct Window {
    kind: crate::shell::ShellWindowKind,
    range: TextRange,
    first: usize,
    last: usize,
}

/// Whether `window`'s range covers exactly the root children it replaces.
fn window_spans_its_children(
    previous: &SyntaxTree<OkfMarkdownLanguage>,
    window: &Window,
) -> Result<bool, ParseError> {
    let children = previous.root_green().children();
    let Some(covered) = children.get(window.first..=window.last) else {
        return Ok(false);
    };
    let width = covered
        .iter()
        .try_fold(TextSize::try_from_usize(0).unwrap(), |sum, child| {
            sum.checked_add(match child {
                GreenElement::Node(node) => node.width(),
                GreenElement::Token(token) => token.width(),
            })
        })
        .map_err(|_| ParseError::WidthOverflow)?;
    Ok(width == window.range.len())
}

fn shell_windows(
    previous: &SyntaxTree<OkfMarkdownLanguage>,
    old: &SourceText,
) -> Result<Vec<Window>, ParseError> {
    let children = previous.root_green().children();
    let eof = children
        .len()
        .checked_sub(1)
        .ok_or(ParseError::WidthOverflow)?;
    let width = |child: &GreenElement<OkfMarkdownLanguage>| match child {
        GreenElement::Node(node) => node.width(),
        GreenElement::Token(token) => token.width(),
    };
    let mut start = TextSize::try_from_usize(0).unwrap();
    let mut result = Vec::new();
    let mut index = 0;
    while index < eof {
        let kind = match &children[index] {
            GreenElement::Node(node) => match node.kind() {
                OkfMarkdownSyntaxKind::Frontmatter => {
                    Some(crate::shell::ShellWindowKind::Frontmatter)
                }
                OkfMarkdownSyntaxKind::Heading
                | OkfMarkdownSyntaxKind::AtxHeading
                | OkfMarkdownSyntaxKind::SetextHeading => {
                    Some(crate::shell::ShellWindowKind::Heading)
                }
                OkfMarkdownSyntaxKind::MarkdownRegion => {
                    Some(crate::shell::ShellWindowKind::MarkdownRegion)
                }
                _ => None,
            },
            _ => None,
        };
        let (kind, last) = if let Some(kind) = kind {
            (kind, index)
        } else {
            let mut last = index;
            while last + 1 < eof {
                let boundary = matches!(
                    &children[last + 1],
                    GreenElement::Node(node)
                        if matches!(
                            node.kind(),
                            OkfMarkdownSyntaxKind::Frontmatter
                                | OkfMarkdownSyntaxKind::Heading
                                | OkfMarkdownSyntaxKind::AtxHeading
                                | OkfMarkdownSyntaxKind::SetextHeading
                        )
                );
                if boundary {
                    break;
                }
                last += 1;
            }
            (crate::shell::ShellWindowKind::MarkdownRegion, last)
        };
        let end = children[index..=last].iter().try_fold(start, |at, child| {
            at.checked_add(width(child))
                .map_err(|_| ParseError::WidthOverflow)
        })?;
        result.push(Window {
            kind,
            range: TextRange::new(start, end).map_err(|_| ParseError::WidthOverflow)?,
            first: index,
            last,
        });
        start = end;
        index = last + 1;
    }
    let (tail_first, tail_start) = if let Some(last) = result
        .last()
        .filter(|window| window.kind == crate::shell::ShellWindowKind::MarkdownRegion)
        .copied()
    {
        result.pop();
        (last.first, last.range.start())
    } else {
        (eof, old.len())
    };
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

/// Any change segment overlapping the frontmatter window that could have
/// restructured its nested tree without moving the region or its fences
/// forces a full reparse: a segment inside (or adjacent to) a line's
/// leading whitespace can widen or narrow indentation, and a segment that
/// removes or inserts a newline can merge or split lines — both change
/// which lines nest under which without the range-and-fence check noticing.
/// Frontmatter is small; precision buys nothing.
fn edit_touches_frontmatter_leading_whitespace(
    old: &SourceText,
    new: &SourceText,
    old_frontmatter: Option<TextRange>,
    new_frontmatter: Option<TextRange>,
    map: &ChangeMap,
) -> bool {
    let Some(new_frontmatter) = new_frontmatter else {
        return false;
    };
    map.segments().iter().any(|segment| {
        let range = segment.new;
        if range.end() <= new_frontmatter.start() || new_frontmatter.end() <= range.start() {
            return false;
        }
        // Conservative: if the segment starts at or before the first
        // non-whitespace column of its line in the NEW text, indentation
        // may have changed.
        let source = new.shared();
        let start = range.start().to_usize().min(source.len());
        let line_start = source[..start].rfind('\n').map_or(0, |at| at + 1);
        let content_start = line_start
            + source[line_start..]
                .bytes()
                .take_while(|b| *b == b' ' || *b == b'\t')
                .count();
        if start <= content_start {
            return true;
        }
        // A same-length (or any) replacement can still restructure the mapping
        // WITHOUT starting at a line's leading whitespace, and the column check
        // above cannot see it:
        //
        // - A newline merges or splits two lines.
        // - A colon decides whether a line is a mapping key at all. Removing
        //   the one in `meta:` demotes it to a plain scalar, which reclassifies
        //   every more-indented line beneath it — those entries are no longer
        //   nested under anything. Reusing them keeps a stale subtree.
        //
        // Check both the bytes being replaced (OLD side) and the bytes
        // replacing them (NEW side), since either direction restructures.
        const RESTRUCTURING: [char; 2] = ['\n', ':'];
        if old_frontmatter.is_some_and(|old_frontmatter| {
            let old_source = old.shared();
            let old_range = segment.old;
            let clamped_start = old_range.start().to_usize().min(old_source.len());
            let clamped_end = old_range.end().to_usize().min(old_source.len());
            clamped_start < clamped_end
                && old_range.start() < old_frontmatter.end()
                && old_frontmatter.start() < old_range.end()
                && old_source[clamped_start..clamped_end].contains(RESTRUCTURING)
        }) {
            return true;
        }
        let clamped_end = range.end().to_usize().min(source.len());
        start < clamped_end && source[start..clamped_end].contains(RESTRUCTURING)
    })
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
    // The same scan the classifier uses to find the close fence: a `---`
    // more indented than an open block scalar's parent is content, never a
    // fence, so this side must not compare against a mismatched line.
    let close =
        crate::shell::frontmatter_close_fence_line(source, open_end, end).and_then(|close_line| {
            TextRange::new(
                TextSize::try_from_usize(close_line.start).ok()?,
                frontmatter.end(),
            )
            .ok()
        });
    Some((open, close))
}
fn same_ranges(old: &[TextRange], new: &[TextRange], map: &ChangeMap) -> bool {
    let Some(mut old): Option<Vec<_>> = old
        .iter()
        .copied()
        .map(|range| map_range(range, map))
        .collect()
    else {
        return false;
    };
    let mut new = new.to_vec();
    old.sort_by_key(|range| (range.start(), range.end()));
    new.sort_by_key(|range| (range.start(), range.end()));
    old == new
}
fn same_headings(
    old: &crate::markdown::ShellStructure,
    new: &crate::markdown::ShellStructure,
    map: &ChangeMap,
) -> bool {
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
    old: &crate::markdown::ShellStructure,
    new: &crate::markdown::ShellStructure,
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

fn width_change_may_shift_reference_definition(
    previous: &SyntaxTree<OkfMarkdownLanguage>,
    map: &ChangeMap,
) -> bool {
    let mut definition_ends = Vec::new();
    let mut stack = vec![previous.root()];
    while let Some(node) = stack.pop() {
        if node.kind() == OkfMarkdownSyntaxKind::LinkReferenceDefinition {
            definition_ends.push(node.range().end());
        }
        for child in node.children() {
            if let SyntaxElement::Node(child) = child {
                stack.push(child);
            }
        }
    }

    map.segments().iter().any(|segment| {
        let old_len = segment.old.end().to_usize() - segment.old.start().to_usize();
        let new_len = segment.new.end().to_usize() - segment.new.start().to_usize();
        old_len != new_len && definition_ends.iter().any(|end| segment.old.start() < *end)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_token(
        source: &SourceText,
        start: usize,
        end: usize,
    ) -> GreenElement<OkfMarkdownLanguage> {
        GreenElement::Token(
            GreenFactory::new()
                .token(
                    OkfMarkdownSyntaxKind::RawTextToken,
                    GreenText::SourceSlice {
                        source: source.clone(),
                        range: TextRange::new(
                            TextSize::try_from_usize(start).unwrap(),
                            TextSize::try_from_usize(end).unwrap(),
                        )
                        .unwrap(),
                    },
                    [],
                    [],
                )
                .unwrap(),
        )
    }

    #[test]
    fn exact_source_recovery_handles_deep_and_wide_trees_iteratively() {
        let deep_source = SourceText::from_shared(Arc::new("x".to_owned())).unwrap();
        let mut deep = GreenFactory::new()
            .node(
                OkfMarkdownSyntaxKind::MarkdownRegion,
                [source_token(&deep_source, 0, 1)],
            )
            .unwrap();
        for _ in 0..2_048 {
            deep = GreenFactory::new()
                .node(
                    OkfMarkdownSyntaxKind::MarkdownRegion,
                    [GreenElement::Node(deep)],
                )
                .unwrap();
        }
        let deep_root = GreenFactory::new()
            .node(OkfMarkdownSyntaxKind::Root, [GreenElement::Node(deep)])
            .unwrap();
        let recovered = recover_exact_source(&deep_root).unwrap();
        assert!(Arc::ptr_eq(recovered.shared(), deep_source.shared()));

        let wide_source = SourceText::from_shared(Arc::new("x".repeat(20_000))).unwrap();
        let wide_root = GreenFactory::new()
            .node(
                OkfMarkdownSyntaxKind::Root,
                (0..20_000).map(|index| source_token(&wide_source, index, index + 1)),
            )
            .unwrap();
        let recovered = recover_exact_source(&wide_root).unwrap();
        assert!(Arc::ptr_eq(recovered.shared(), wide_source.shared()));
        std::mem::forget(deep_root);
    }

    #[test]
    fn exact_source_recovery_rejects_mixed_gap_and_owned_mismatch_streams() {
        let left = SourceText::from_shared(Arc::new("ab".to_owned())).unwrap();
        let right = SourceText::from_shared(Arc::new("ab".to_owned())).unwrap();
        let factory = GreenFactory::new();
        let mixed = factory
            .node(
                OkfMarkdownSyntaxKind::Root,
                [source_token(&left, 0, 1), source_token(&right, 1, 2)],
            )
            .unwrap();
        assert!(recover_exact_source(&mixed).is_none());
        let gap = factory
            .node(OkfMarkdownSyntaxKind::Root, [source_token(&left, 1, 2)])
            .unwrap();
        assert!(recover_exact_source(&gap).is_none());
        let owned = factory
            .node(
                OkfMarkdownSyntaxKind::Root,
                [
                    source_token(&left, 0, 1),
                    GreenElement::Token(
                        factory
                            .token(
                                OkfMarkdownSyntaxKind::RawTextToken,
                                GreenText::Owned(Arc::from("x")),
                                [],
                                [],
                            )
                            .unwrap(),
                    ),
                ],
            )
            .unwrap();
        assert!(recover_exact_source(&owned).is_none());
    }

    #[test]
    fn source_recovery_rejects_hostile_streams() {
        let empty_source = SourceText::from_shared(Arc::new(String::new())).unwrap();
        let source = SourceText::from_shared(Arc::new("ab".to_owned())).unwrap();
        let factory = GreenFactory::new();
        let parser_source =
            SourceText::from_shared(Arc::new("# Ordinary\nbody\n".to_owned())).unwrap();
        let parser_tree = crate::markdown::parser::parse(
            parser_source.clone(),
            crate::MarkdownDialect::WAML_DEFAULT,
        )
        .unwrap()
        .tree;
        assert!(Arc::ptr_eq(
            recover_exact_source(parser_tree.root_green())
                .unwrap()
                .shared(),
            parser_source.shared()
        ));
        let empty = factory
            .node(
                OkfMarkdownSyntaxKind::Root,
                [source_token(&empty_source, 0, 0)],
            )
            .unwrap();
        assert!(Arc::ptr_eq(
            recover_exact_source(&empty).unwrap().shared(),
            empty_source.shared()
        ));
        let trivia = factory
            .trivia(
                crate::TriviaKind::Whitespace,
                GreenText::SourceSlice {
                    source: source.clone(),
                    range: TextRange::new(
                        TextSize::try_from_usize(0).unwrap(),
                        TextSize::try_from_usize(2).unwrap(),
                    )
                    .unwrap(),
                },
            )
            .unwrap();
        let trivia_only = factory
            .node(
                OkfMarkdownSyntaxKind::Root,
                [GreenElement::Token(
                    factory
                        .token(
                            OkfMarkdownSyntaxKind::RawTextToken,
                            GreenText::Static(""),
                            [trivia],
                            [],
                        )
                        .unwrap(),
                )],
            )
            .unwrap();
        assert!(Arc::ptr_eq(
            recover_exact_source(&trivia_only).unwrap().shared(),
            source.shared()
        ));
        let reordered = factory
            .node(
                OkfMarkdownSyntaxKind::Root,
                [source_token(&source, 1, 2), source_token(&source, 0, 1)],
            )
            .unwrap();
        assert!(recover_exact_source(&reordered).is_none());
        let duplicated = factory
            .node(
                OkfMarkdownSyntaxKind::Root,
                [source_token(&source, 0, 1), source_token(&source, 0, 1)],
            )
            .unwrap();
        assert!(recover_exact_source(&duplicated).is_none());
        let overlap = factory
            .node(
                OkfMarkdownSyntaxKind::Root,
                [source_token(&source, 0, 1), source_token(&source, 0, 2)],
            )
            .unwrap();
        assert!(recover_exact_source(&overlap).is_none());
        let static_mismatch = factory
            .node(
                OkfMarkdownSyntaxKind::Root,
                [
                    source_token(&source, 0, 1),
                    GreenElement::Token(
                        factory
                            .token(
                                OkfMarkdownSyntaxKind::RawTextToken,
                                GreenText::Static("x"),
                                [],
                                [],
                            )
                            .unwrap(),
                    ),
                ],
            )
            .unwrap();
        assert!(recover_exact_source(&static_mismatch).is_none());
        let source_independent = factory
            .node(
                OkfMarkdownSyntaxKind::Root,
                [GreenElement::Token(
                    factory
                        .token(
                            OkfMarkdownSyntaxKind::RawTextToken,
                            GreenText::Owned(Arc::from("ab")),
                            [],
                            [],
                        )
                        .unwrap(),
                )],
            )
            .unwrap();
        assert!(recover_exact_source(&source_independent).is_none());
    }

    /// Plans a window reparse from `previous_src` to `next_src` directly
    /// (bypassing the outer full-parse fallback wrapper) and asserts it comes
    /// back as `Ok(Err(expected))` — a windowed reparse is possible in
    /// principle, but this specific boundary class rules it out.
    fn assert_plan_reason(
        previous_src: &str,
        next_src: &str,
        changes: &[TextChange],
        expected: FullReparseReason,
    ) {
        let dialect = crate::MarkdownDialect::WAML_DEFAULT;
        let previous = crate::markdown::parser::parse(
            SourceText::from_shared(Arc::new(previous_src.to_owned())).unwrap(),
            dialect,
        )
        .unwrap()
        .tree;
        let old = recover_exact_source(previous.root_green()).unwrap();
        let new_text = SourceText::from_shared(Arc::new(next_src.to_owned())).unwrap();
        let map = ChangeMap::checked(&old, changes).unwrap();
        let new_structure = Arc::new(crate::markdown::shell_map(&new_text, dialect).unwrap());
        let old_projection = Arc::new(crate::markdown::from_tree(&previous, old.shared()).unwrap());
        let plan = plan_window_reparse(
            &previous,
            &old,
            &new_text,
            dialect,
            changes,
            &map,
            &new_structure,
            &old_projection,
        )
        .unwrap();
        match plan {
            Err(reason) => assert_eq!(reason, expected),
            Ok(_) => panic!("expected {expected:?}, got a successful window plan"),
        }
    }

    #[test]
    fn plan_window_reparse_reports_island_boundary_changed() {
        // The edit lands inside the WAML heading/content span of an existing
        // island, which the island-boundary guard refuses outright.
        let previous_src = "# Document\n## Attributes\none\n";
        let start = previous_src.find("one").unwrap();
        assert_plan_reason(
            previous_src,
            "# Document\n## Attributes\ntwo\n",
            &[TextChange {
                old_range: range(start, start + "one".len()),
                replacement: Arc::from("two"),
            }],
            FullReparseReason::IslandBoundaryChanged,
        );
    }

    #[test]
    fn plan_window_reparse_reports_frontmatter_boundary_changed() {
        assert_plan_reason(
            "body\n",
            "---\ntype: x\n---\nbody\n",
            &[TextChange {
                old_range: range(0, 0),
                replacement: Arc::from("---\ntype: x\n---\n"),
            }],
            FullReparseReason::FrontmatterBoundaryChanged,
        );
    }

    #[test]
    fn plan_window_reparse_reports_heading_boundary_changed() {
        assert_plan_reason(
            "# H\nbody\n",
            "## H\nbody\n",
            &[TextChange {
                old_range: range(0, 1),
                replacement: Arc::from("##"),
            }],
            FullReparseReason::HeadingBoundaryChanged,
        );
    }

    #[test]
    fn plan_window_reparse_reports_markdown_container_boundary_changed() {
        assert_plan_reason(
            "body\n",
            "- body\n",
            &[TextChange {
                old_range: range(0, 0),
                replacement: Arc::from("- "),
            }],
            FullReparseReason::MarkdownContainerBoundaryChanged,
        );
    }

    fn range(start: usize, end: usize) -> TextRange {
        TextRange::new(
            TextSize::try_from_usize(start).unwrap(),
            TextSize::try_from_usize(end).unwrap(),
        )
        .unwrap()
    }
}
