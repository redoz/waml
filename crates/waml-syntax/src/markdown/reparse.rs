use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::{
    rebase_unchanged_green, ChangeMap, GreenElement, GreenFactory, GreenNode, OkfMarkdownLanguage,
    OkfMarkdownSyntaxKind, ParseError, SourceText, SyntaxElement, SyntaxNode, SyntaxTree,
    TextChange, TextRange, TextSize,
};

use super::reference::{normalize_label, MarkdownReferenceDefinition, MarkdownReferenceMap};

/// Returns true when a change touches any reference-definition line.
///
/// The check is deliberately conservative. A false positive only expands the
/// incremental work set; a false negative could leave a reference annotation
/// with an obsolete destination.
pub(crate) fn change_touches_reference_definition(
    old: &SourceText,
    new: &SourceText,
    changes: &[TextChange],
    map: &ChangeMap,
) -> bool {
    changes.iter().zip(map.segments()).any(|(change, segment)| {
        intersecting_lines(old.shared(), change.old_range).any(line_is_definition)
            || intersecting_lines(new.shared(), segment.new).any(line_is_definition)
            || trailing_paragraph_lines(old.shared(), change.old_range).any(line_is_definition)
            || trailing_paragraph_lines(new.shared(), segment.new).any(line_is_definition)
    })
}

/// Lines following the edited lines up to the first blank line.
///
/// An edit can promote or demote a definition-shaped line it never touches:
/// deleting the only text of a paragraph's opening line turns the next line
/// from a paragraph continuation into a real reference definition. Scan the
/// rest of that paragraph run so such promotions still count as touching a
/// definition.
fn trailing_paragraph_lines(source: &str, range: TextRange) -> impl Iterator<Item = &str> {
    let end = range.end().to_usize().min(source.len());
    let line_end = source[end..].find('\n').map_or(source.len(), |at| end + at);
    source[line_end..]
        .split_terminator('\n')
        .skip(1)
        .take_while(|line| !line.trim().is_empty())
}

/// Returns true when an edited line can add, remove, or join a reference use.
/// A local shell window cannot resolve such a use against definitions outside
/// that window, so the incremental bridge must use a named full fallback.
pub(crate) fn change_may_affect_reference_use(
    old: &SourceText,
    old_root: &GreenNode<OkfMarkdownLanguage>,
    new: &SourceText,
    changes: &[TextChange],
    map: &ChangeMap,
) -> Result<bool, ParseError> {
    let references = MarkdownReferenceMap::from_tree(old.shared(), old_root, 0)?;
    Ok(changes.iter().zip(map.segments()).any(|(change, segment)| {
        intersecting_lines(old.shared(), change.old_range)
            .chain(intersecting_lines(new.shared(), segment.new))
            .flat_map(reference_labels)
            .any(|label| references.definitions.contains_key(&label))
    }))
}

/// Returns true when reparsing a shell window could resolve a reference use
/// differently from a full parse.
///
/// A window reparse resolves reference links only against definitions inside
/// the window. Any line in the window (old or new) that uses a label whose
/// definition lies outside the window would therefore lose (or change) its
/// resolution, so the caller must fall back to a full parse. The edited-lines
/// guard alone is not enough: the selected window regularly spans lines the
/// edit never touched.
pub(crate) fn window_reparse_may_lose_reference_resolution(
    old: &SourceText,
    old_root: &GreenNode<OkfMarkdownLanguage>,
    old_window: TextRange,
    new: &SourceText,
    new_window: TextRange,
) -> Result<bool, ParseError> {
    let references = MarkdownReferenceMap::from_tree(old.shared(), old_root, 0)?;
    if references.definitions.is_empty() {
        return Ok(false);
    }
    let defined_outside = |label: &Arc<str>| {
        references.definitions.get(label).is_some_and(|definition| {
            definition.source_range.start() < old_window.start()
                || definition.source_range.end() > old_window.end()
        })
    };
    Ok(intersecting_lines(old.shared(), old_window)
        .chain(intersecting_lines(new.shared(), new_window))
        .flat_map(reference_labels)
        .any(|label| defined_outside(&label)))
}

fn intersecting_lines(source: &str, range: TextRange) -> impl Iterator<Item = &str> {
    let start = range.start().to_usize().min(source.len());
    let end = range.end().to_usize().min(source.len());
    let line_start = source[..start].rfind('\n').map_or(0, |at| at + 1);
    let line_end = source[end..].find('\n').map_or(source.len(), |at| end + at);
    source[line_start..line_end].split_terminator('\n')
}

fn line_is_definition(line: &str) -> bool {
    let line = line.trim();
    line.starts_with('[') && line.contains("]:")
}

fn reference_labels(line: &str) -> Vec<Arc<str>> {
    if line_is_definition(line) {
        return Vec::new();
    }
    let mut labels = Vec::new();
    let mut rest = line;
    while let Some(open) = rest.find('[') {
        let after_open = &rest[open + 1..];
        let Some(close) = after_open.find(']') else {
            break;
        };
        let text = &after_open[..close];
        let after = &after_open[close + 1..];
        let (label, consumed) = if let Some(after_label) = after.strip_prefix('[') {
            let Some(label_end) = after_label.find(']') else {
                break;
            };
            let label = &after_label[..label_end];
            (if label.is_empty() { text } else { label }, label_end + 2)
        } else if after.starts_with('(') {
            // Inline link destination: `[text](dest)` carries no reference
            // label. Consume only the parenthesized tail so a later
            // reference use on the same line (e.g. `[a](x) then [b][id]`)
            // is still scanned. Track paren depth to handle balanced nested
            // parens in the destination; if the line ends unbalanced, only
            // consume the opening `(` (consumed = 1) so scanning resumes
            // right after it — over-scanning is safe (extra fallback),
            // under-scanning is the bug this guards against.
            let mut depth = 0usize;
            let mut close_at = None;
            for (idx, ch) in after.char_indices() {
                match ch {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            close_at = Some(idx);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            ("", close_at.map_or(1, |idx| idx + 1))
        } else {
            (text, 0)
        };
        if let Some(label) = normalize_label(label) {
            labels.push(label);
        }
        rest = &after[consumed..];
    }
    labels
}

pub(crate) fn changed_reference_labels(
    old_source: &SourceText,
    old_root: &GreenNode<OkfMarkdownLanguage>,
    new_source: &SourceText,
    new_root: &GreenNode<OkfMarkdownLanguage>,
) -> Result<HashSet<Arc<str>>, ParseError> {
    let old = MarkdownReferenceMap::from_tree(old_source.shared(), old_root, 0)?;
    let new = MarkdownReferenceMap::from_tree(new_source.shared(), new_root, 0)?;
    let labels: HashSet<_> = old
        .definitions
        .keys()
        .chain(new.definitions.keys())
        .cloned()
        .collect();
    Ok(labels
        .into_iter()
        .filter(|label| !same_definition(old.definitions.get(label), new.definitions.get(label)))
        .collect())
}

pub(crate) fn changed_definition_ranges(
    source: &SourceText,
    root: &GreenNode<OkfMarkdownLanguage>,
    labels: &HashSet<Arc<str>>,
) -> Result<Vec<TextRange>, ParseError> {
    let map = MarkdownReferenceMap::from_tree(source.shared(), root, 0)?;
    Ok(labels
        .iter()
        .filter_map(|label| {
            map.definitions
                .get(label)
                .map(|definition| definition.source_range)
        })
        .collect())
}

fn same_definition(
    old: Option<&MarkdownReferenceDefinition>,
    new: Option<&MarkdownReferenceDefinition>,
) -> bool {
    match (old, new) {
        (Some(old), Some(new)) => {
            old.label == new.label && old.destination == new.destination && old.title == new.title
        }
        (None, None) => true,
        (Some(_), None) | (None, Some(_)) => false,
    }
}

pub(crate) fn splice_reference_dependents(
    base: &GreenNode<OkfMarkdownLanguage>,
    oracle: &GreenNode<OkfMarkdownLanguage>,
    affected: &[TextRange],
) -> Result<GreenNode<OkfMarkdownLanguage>, ParseError> {
    splice_node(base, oracle, affected, TextSize::new(0))
}

pub(crate) fn restore_unchanged_subtrees(
    previous: &GreenNode<OkfMarkdownLanguage>,
    candidate: &GreenNode<OkfMarkdownLanguage>,
    new_text: &SourceText,
    map: &ChangeMap,
    excluded: &[TextRange],
) -> Result<GreenNode<OkfMarkdownLanguage>, ParseError> {
    let mut reusable_nodes = HashMap::new();
    let mut reusable_tokens = HashMap::new();
    collect_reusable(
        previous,
        TextSize::new(0),
        map,
        excluded,
        &mut reusable_nodes,
        &mut reusable_tokens,
    )?;
    restore_candidate_node(
        candidate,
        TextSize::new(0),
        new_text,
        map,
        &reusable_nodes,
        &reusable_tokens,
    )
}

pub(crate) fn preserve_unchanged_island_identities(
    previous: &SyntaxTree<OkfMarkdownLanguage>,
    candidate: &SyntaxTree<OkfMarkdownLanguage>,
    map: &ChangeMap,
) -> Result<GreenNode<OkfMarkdownLanguage>, ParseError> {
    let mut identities = HashMap::new();
    collect_island_identities(previous.root(), map, &mut identities);
    let mut replacements = Vec::new();
    collect_island_replacements(candidate.root(), &identities, &mut replacements);
    let mut root = candidate.root_green().clone();
    for (path, identity) in replacements {
        root = replace_identity_at_path(&root, &path, &identity)?;
    }
    Ok(root)
}

fn collect_island_identities(
    node: SyntaxNode<OkfMarkdownLanguage>,
    map: &ChangeMap,
    identities: &mut HashMap<(OkfMarkdownSyntaxKind, TextRange), crate::SyntaxAnnotation>,
) {
    if node.kind() == OkfMarkdownSyntaxKind::WamlSection {
        let unchanged = !map
            .segments()
            .iter()
            .any(|segment| intersects(node.range(), segment.old));
        let translated = unchanged
            .then(|| {
                TextRange::new(
                    map.translate_start_boundary(node.range().start())?,
                    map.translate_end_boundary(node.range().end())?,
                )
                .ok()
            })
            .flatten();
        if let (Some(new_range), Some(identity)) = (
            translated,
            node.syntax_annotations()
                .iter()
                .find(|annotation| annotation.kind() == "waml.markdown.identity"),
        ) {
            identities.insert((node.kind(), new_range), identity.clone());
        }
    }
    for child in node.children() {
        if let SyntaxElement::Node(child) = child {
            collect_island_identities(child, map, identities);
        }
    }
}

fn collect_island_replacements(
    node: SyntaxNode<OkfMarkdownLanguage>,
    identities: &HashMap<(OkfMarkdownSyntaxKind, TextRange), crate::SyntaxAnnotation>,
    replacements: &mut Vec<(Vec<u32>, crate::SyntaxAnnotation)>,
) {
    if node.kind() == OkfMarkdownSyntaxKind::WamlSection {
        if let Some(identity) = identities.get(&(node.kind(), node.range())) {
            replacements.push((node.locator().path().as_slice().to_vec(), identity.clone()));
        }
    }
    for child in node.children() {
        if let SyntaxElement::Node(child) = child {
            collect_island_replacements(child, identities, replacements);
        }
    }
}

fn replace_identity_at_path(
    node: &GreenNode<OkfMarkdownLanguage>,
    path: &[u32],
    identity: &crate::SyntaxAnnotation,
) -> Result<GreenNode<OkfMarkdownLanguage>, ParseError> {
    if path.is_empty() {
        let mut annotations = node
            .annotations()
            .iter()
            .filter(|annotation| annotation.kind() != "waml.markdown.identity")
            .cloned()
            .collect::<Vec<_>>();
        annotations.push(identity.clone());
        return GreenFactory::new()
            .node_with_annotations(
                node.kind(),
                node.children().iter().cloned(),
                annotations.into(),
            )
            .map_err(|_| ParseError::WidthOverflow);
    }
    let index = path[0] as usize;
    let mut children = node.children().to_vec();
    let Some(GreenElement::Node(child)) = children.get(index) else {
        return Err(ParseError::StructuralInvariant {
            reason: "WAML island identity path did not resolve to a node".into(),
        });
    };
    children[index] = GreenElement::Node(replace_identity_at_path(child, &path[1..], identity)?);
    GreenFactory::new()
        .node_with_annotations(node.kind(), children, node.annotations().into())
        .map_err(|_| ParseError::WidthOverflow)
}

fn collect_reusable(
    node: &GreenNode<OkfMarkdownLanguage>,
    start: TextSize,
    map: &ChangeMap,
    excluded: &[TextRange],
    reusable_nodes: &mut HashMap<
        (OkfMarkdownSyntaxKind, TextRange),
        GreenNode<OkfMarkdownLanguage>,
    >,
    reusable_tokens: &mut HashMap<
        (OkfMarkdownSyntaxKind, TextRange),
        crate::GreenToken<OkfMarkdownLanguage>,
    >,
) -> Result<(), ParseError> {
    let range = TextRange::new(
        start,
        start
            .checked_add(node.width())
            .map_err(|_| ParseError::WidthOverflow)?,
    )
    .map_err(|_| ParseError::WidthOverflow)?;
    if let Some(new_range) = map.translate_unchanged(range) {
        if !excluded
            .iter()
            .any(|excluded| intersects(new_range, *excluded))
        {
            reusable_nodes
                .entry((node.kind(), new_range))
                .or_insert_with(|| node.clone());
        }
    }
    let mut at = start;
    for child in node.children() {
        match child {
            GreenElement::Node(child) => {
                collect_reusable(child, at, map, excluded, reusable_nodes, reusable_tokens)?;
            }
            GreenElement::Token(token) => {
                let old_range = TextRange::new(
                    at,
                    at.checked_add(token.width())
                        .map_err(|_| ParseError::WidthOverflow)?,
                )
                .map_err(|_| ParseError::WidthOverflow)?;
                if let Some(new_range) = map.translate_unchanged(old_range) {
                    if !excluded
                        .iter()
                        .any(|excluded| intersects(new_range, *excluded))
                    {
                        reusable_tokens
                            .entry((token.kind(), new_range))
                            .or_insert_with(|| token.clone());
                    }
                }
            }
        }
        at = at
            .checked_add(element_width(child))
            .map_err(|_| ParseError::WidthOverflow)?;
    }
    Ok(())
}

fn restore_candidate_node(
    candidate: &GreenNode<OkfMarkdownLanguage>,
    start: TextSize,
    new_text: &SourceText,
    map: &ChangeMap,
    reusable_nodes: &HashMap<(OkfMarkdownSyntaxKind, TextRange), GreenNode<OkfMarkdownLanguage>>,
    reusable_tokens: &HashMap<
        (OkfMarkdownSyntaxKind, TextRange),
        crate::GreenToken<OkfMarkdownLanguage>,
    >,
) -> Result<GreenNode<OkfMarkdownLanguage>, ParseError> {
    let range = TextRange::new(
        start,
        start
            .checked_add(candidate.width())
            .map_err(|_| ParseError::WidthOverflow)?,
    )
    .map_err(|_| ParseError::WidthOverflow)?;
    if let Some(previous) = reusable_nodes.get(&(candidate.kind(), range)) {
        if let Some(rebased) =
            rebase_unchanged_green(&GreenElement::Node(previous.clone()), new_text, map)
                .map_err(|_| ParseError::WidthOverflow)?
        {
            let GreenElement::Node(node) = rebased.element else {
                return Err(ParseError::StructuralInvariant {
                    reason: "rebased unchanged Markdown node became a token".into(),
                });
            };
            return Ok(node);
        }
    }
    let mut at = start;
    let mut changed = false;
    let mut children = Vec::with_capacity(candidate.children().len());
    for child in candidate.children() {
        let child_range = TextRange::new(
            at,
            at.checked_add(element_width(child))
                .map_err(|_| ParseError::WidthOverflow)?,
        )
        .map_err(|_| ParseError::WidthOverflow)?;
        let restored = match child {
            GreenElement::Node(node) => {
                let restored = restore_candidate_node(
                    node,
                    at,
                    new_text,
                    map,
                    reusable_nodes,
                    reusable_tokens,
                )?;
                changed |= !Arc::ptr_eq(node, &restored);
                GreenElement::Node(restored)
            }
            GreenElement::Token(token) => reusable_tokens
                .get(&(token.kind(), child_range))
                .filter(|old| old.is_source_independent())
                .map(|old| {
                    changed |= !Arc::ptr_eq(token, old);
                    GreenElement::Token(old.clone())
                })
                .unwrap_or_else(|| child.clone()),
        };
        at = child_range.end();
        children.push(restored);
    }
    if !changed {
        Ok(candidate.clone())
    } else {
        GreenFactory::new()
            .node_with_annotations(candidate.kind(), children, candidate.annotations().into())
            .map_err(|_| ParseError::WidthOverflow)
    }
}

fn splice_node(
    base: &GreenNode<OkfMarkdownLanguage>,
    oracle: &GreenNode<OkfMarkdownLanguage>,
    affected: &[TextRange],
    start: TextSize,
) -> Result<GreenNode<OkfMarkdownLanguage>, ParseError> {
    let node_range = TextRange::new(
        start,
        start
            .checked_add(base.width())
            .map_err(|_| ParseError::WidthOverflow)?,
    )
    .map_err(|_| ParseError::WidthOverflow)?;
    if !affected.iter().any(|range| intersects(node_range, *range)) {
        return Ok(base.clone());
    }
    if base.kind() != oracle.kind()
        || base.width() != oracle.width()
        || base.children().len() != oracle.children().len()
    {
        return Ok(oracle.clone());
    }
    if matches!(
        base.kind(),
        OkfMarkdownSyntaxKind::Link
            | OkfMarkdownSyntaxKind::Image
            | OkfMarkdownSyntaxKind::Autolink
    ) {
        let mut annotations = oracle
            .annotations()
            .iter()
            .filter(|annotation| {
                annotation.kind() != "waml.markdown.identity"
                    && annotation.kind() != super::inline::owner_annotation()
            })
            .cloned()
            .collect::<Vec<_>>();
        annotations.extend(
            base.annotations()
                .iter()
                .filter(|annotation| {
                    annotation.kind() == "waml.markdown.identity"
                        || annotation.kind() == super::inline::owner_annotation()
                })
                .cloned(),
        );
        return GreenFactory::new()
            .node_with_annotations(
                oracle.kind(),
                oracle.children().iter().cloned(),
                annotations.into(),
            )
            .map_err(|_| ParseError::WidthOverflow);
    }
    let mut at = start;
    let mut changed = false;
    let mut children = Vec::with_capacity(base.children().len());
    for (base_child, oracle_child) in base.children().iter().zip(oracle.children()) {
        let child_range = TextRange::new(
            at,
            at.checked_add(element_width(base_child))
                .map_err(|_| ParseError::WidthOverflow)?,
        )
        .map_err(|_| ParseError::WidthOverflow)?;
        let child = if !affected.iter().any(|range| intersects(child_range, *range)) {
            base_child.clone()
        } else {
            match (base_child, oracle_child) {
                (GreenElement::Node(base_node), GreenElement::Node(oracle_node)) => {
                    let node = splice_node(base_node, oracle_node, affected, at)?;
                    changed |= !Arc::ptr_eq(base_node, &node);
                    GreenElement::Node(node)
                }
                (GreenElement::Token(base_token), GreenElement::Token(oracle_token))
                    if base_token.kind() == oracle_token.kind()
                        && base_token.width() == oracle_token.width() =>
                {
                    changed |= !Arc::ptr_eq(base_token, oracle_token);
                    GreenElement::Token(oracle_token.clone())
                }
                _ => {
                    changed = true;
                    oracle_child.clone()
                }
            }
        };
        at = child_range.end();
        children.push(child);
    }
    if !changed {
        return Ok(base.clone());
    }
    GreenFactory::new()
        .node_with_annotations(base.kind(), children, base.annotations().into())
        .map_err(|_| ParseError::WidthOverflow)
}

fn element_width(element: &GreenElement<OkfMarkdownLanguage>) -> TextSize {
    match element {
        GreenElement::Node(node) => node.width(),
        GreenElement::Token(token) => token.width(),
    }
}

fn intersects(left: TextRange, right: TextRange) -> bool {
    left.start() < right.end() && right.start() < left.end()
}

/// Sort, deduplicate, and merge overlapping or touching non-empty ranges.
pub(crate) fn normalize_affected_ranges(mut ranges: Vec<TextRange>) -> Vec<TextRange> {
    ranges.retain(|range| range.start() < range.end());
    ranges.sort_by_key(|range| (range.start(), range.end()));
    let mut normalized: Vec<TextRange> = Vec::new();
    for range in ranges {
        if let Some(previous) = normalized.last_mut() {
            if range.start() <= previous.end() {
                *previous = previous.cover(range);
                continue;
            }
        }
        normalized.push(range);
    }
    normalized
}

pub(crate) fn full_range(text: &SourceText) -> TextRange {
    TextRange::new(TextSize::try_from_usize(0).unwrap(), text.len()).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(start: usize, end: usize) -> TextRange {
        TextRange::new(
            TextSize::try_from_usize(start).unwrap(),
            TextSize::try_from_usize(end).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn normalization_merges_touching_ranges() {
        assert_eq!(
            normalize_affected_ranges(vec![range(8, 9), range(1, 4), range(4, 8)]),
            vec![range(1, 9)]
        );
    }

    #[test]
    fn intersecting_line_scan_checks_interior_lines() {
        let source = SourceText::new("before\n[id]: /one\nafter\n").unwrap();
        let next = SourceText::new("BEFORE\n[id]: /two\nAFTER\n").unwrap();
        let changes = [TextChange {
            old_range: range(0, source.len().to_usize()),
            replacement: Arc::from("BEFORE\n[id]: /two\nAFTER\n"),
        }];
        let map = ChangeMap::checked(&source, &changes).unwrap();

        assert!(change_touches_reference_definition(
            &source, &next, &changes, &map
        ));

        let local_changes = [TextChange {
            old_range: range(13, 17),
            replacement: Arc::from("/two"),
        }];
        let local_map = ChangeMap::checked(&source, &local_changes).unwrap();
        assert!(change_touches_reference_definition(
            &source,
            &SourceText::new("before\n[id]: /two\nafter\n").unwrap(),
            &local_changes,
            &local_map,
        ));
    }

    #[test]
    fn semantic_definition_diff_reports_changed_label() {
        let old = SourceText::new("[id]: /one\n\nuse [x][id]\n").unwrap();
        let new = SourceText::new("[id]: /two\n\nuse [x][id]\n").unwrap();
        let old_parse =
            crate::markdown::parser::parse(old.clone(), crate::MarkdownDialect::WAML_DEFAULT)
                .unwrap();
        let new_parse =
            crate::markdown::parser::parse(new.clone(), crate::MarkdownDialect::WAML_DEFAULT)
                .unwrap();

        assert_eq!(
            changed_reference_labels(
                &old,
                old_parse.tree.root_green(),
                &new,
                new_parse.tree.root_green(),
            )
            .unwrap()
            .into_iter()
            .collect::<Vec<_>>(),
            vec![Arc::from("id")]
        );
    }
}
