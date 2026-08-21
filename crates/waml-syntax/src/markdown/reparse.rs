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
///
/// This is a cheap pre-filter over the edited lines only, so it uses the
/// use-only label scan: a line that reads as a reference definition spends its
/// leading label defining, not using. That reading can be wrong — see
/// [`reference_labels`] — but this guard is allowed to be, because every path
/// that survives it still passes through
/// [`window_reparse_may_lose_reference_resolution`], which scans the whole
/// window with no such assumption.
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
            .flat_map(reference_use_labels)
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
///
/// Uses the conservative [`reference_labels`] scan, not
/// [`reference_use_labels`]: a window reparse decides for itself which of its
/// lines are definitions, and it decides from in-window bytes alone, so no
/// line in the window may be assumed to be one.
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

/// [`reference_labels`], minus every line that reads as a reference
/// definition.
///
/// Only for guards that are allowed to under-report — see
/// [`change_may_affect_reference_use`]. Assuming a definition-shaped line
/// holds no reference uses is not sound: the shape test is loose, and even a
/// real definition can carry a use in its tail. [`reference_labels`] is the
/// scan that assumes nothing.
fn reference_use_labels(line: &str) -> Vec<Arc<str>> {
    if line_is_definition(line) {
        return Vec::new();
    }
    reference_labels(line)
}

/// Every bracketed label on the line, as a *potential* reference use.
///
/// Definition-shaped lines are scanned like any other. `line_is_definition`
/// recognises a definition by shape alone, and shape is not the whole rule:
/// `[a]: /url` is a definition but `[a]: /url x` is not, because `x` is not a
/// valid title — that line is a paragraph whose `[a]` is a shortcut reference
/// use resolved against a definition somewhere else in the document. An edit
/// flips a line between those two readings by touching only its tail, so a
/// guard that skipped definition-shaped lines would miss exactly the case
/// where the window's shape depends on bytes outside it.
///
/// Naming a real definition line's own label costs nothing where it matters:
/// the winning definition for that label is that same line, so
/// [`window_reparse_may_lose_reference_resolution`] sees it inside the window
/// and stays incremental. It only forces a fallback when a *duplicate*
/// definition of the label lives outside the window — and there the window's
/// reading really does depend on the outside text.
fn reference_labels(line: &str) -> Vec<Arc<str>> {
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

/// Re-stamps every link's owner annotation with the identity of the inline
/// root that actually contains it in `root`.
///
/// A link's owner annotation names its containing block, and reference
/// backlinks are keyed by that identity. Incremental reuse keeps a link's green
/// node verbatim — deliberately, so link identities stay stable — and the owner
/// annotation rides along. When an edit merges two blocks (deleting the last
/// line of a paragraph so the blank line behind it becomes that paragraph's
/// terminator), the surviving inline root has one identity but the reused links
/// still name the two dead ones, so `reference_backlinks` reports two owners
/// where a full parse reports one.
pub(crate) fn restamp_inline_owners(
    root: &GreenNode<OkfMarkdownLanguage>,
) -> Result<GreenNode<OkfMarkdownLanguage>, ParseError> {
    restamp_node(root, None)
}

/// The blocks `inline::rebuild` hands to the inline phase as an owner.
fn is_inline_root(kind: OkfMarkdownSyntaxKind) -> bool {
    matches!(
        kind,
        OkfMarkdownSyntaxKind::Paragraph
            | OkfMarkdownSyntaxKind::ListItem
            | OkfMarkdownSyntaxKind::AtxHeading
            | OkfMarkdownSyntaxKind::SetextHeading
            | OkfMarkdownSyntaxKind::TableCell
    )
}

fn green_identity_data(node: &GreenNode<OkfMarkdownLanguage>) -> Option<Arc<str>> {
    let mut annotations = node
        .annotations()
        .iter()
        .filter(|annotation| annotation.kind() == "waml.markdown.identity");
    let annotation = annotations.next()?;
    if annotations.next().is_some() {
        return None;
    }
    annotation.data().map(Arc::from)
}

fn restamp_node(
    node: &GreenNode<OkfMarkdownLanguage>,
    owner: Option<&Arc<str>>,
) -> Result<GreenNode<OkfMarkdownLanguage>, ParseError> {
    let own_identity = is_inline_root(node.kind())
        .then(|| green_identity_data(node))
        .flatten();
    let owner = own_identity.as_ref().or(owner);
    let mut annotations = None;
    if matches!(
        node.kind(),
        OkfMarkdownSyntaxKind::Link
            | OkfMarkdownSyntaxKind::Image
            | OkfMarkdownSyntaxKind::Autolink
    ) {
        if let Some(owner) = owner {
            let stale = node.annotations().iter().any(|annotation| {
                annotation.kind() == super::inline::owner_annotation()
                    && annotation.data() != Some(owner.as_ref())
            });
            if stale {
                annotations = Some(
                    node.annotations()
                        .iter()
                        .map(|annotation| {
                            if annotation.kind() == super::inline::owner_annotation() {
                                crate::SyntaxAnnotation::new(
                                    annotation.id(),
                                    annotation.kind().to_owned(),
                                    Some(owner.clone()),
                                )
                            } else {
                                annotation.clone()
                            }
                        })
                        .collect::<Vec<_>>(),
                );
            }
        }
    }
    let mut changed = annotations.is_some();
    let mut children = Vec::with_capacity(node.children().len());
    for child in node.children() {
        match child {
            GreenElement::Node(child_node) => {
                let restamped = restamp_node(child_node, owner)?;
                changed |= !Arc::ptr_eq(child_node, &restamped);
                children.push(GreenElement::Node(restamped));
            }
            GreenElement::Token(_) => children.push(child.clone()),
        }
    }
    if !changed {
        return Ok(node.clone());
    }
    GreenFactory::new()
        .node_with_annotations(
            node.kind(),
            children,
            annotations
                .map(Arc::from)
                .unwrap_or_else(|| node.annotations().into()),
        )
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

/// Whether a pre-edit subtree may stand in for a freshly parsed one.
///
/// The reuse key is kind plus span, which spanning byte-identical text does
/// not make sufficient: a block's shape can depend on context outside its own
/// span. Collapsing the blank line between two list items makes the list
/// tight, which drops the `Paragraph` wrapper inside every item without
/// touching any item's own text — so the second item still keys as unchanged
/// while a full parse now gives it a different shape. Restoring identity onto
/// a subtree the parser reshaped would hand callers a tree no full parse
/// produces, so compare shapes and let a mismatch fall through to the
/// child-by-child walk.
fn same_shape(
    previous: &GreenElement<OkfMarkdownLanguage>,
    candidate: &GreenElement<OkfMarkdownLanguage>,
) -> bool {
    match (previous, candidate) {
        (GreenElement::Node(previous), GreenElement::Node(candidate)) => {
            same_shape_node(previous, candidate)
        }
        (GreenElement::Token(previous), GreenElement::Token(candidate)) => {
            same_shape_token(previous, candidate)
        }
        _ => false,
    }
}

fn same_shape_node(
    previous: &GreenNode<OkfMarkdownLanguage>,
    candidate: &GreenNode<OkfMarkdownLanguage>,
) -> bool {
    previous.kind() == candidate.kind()
        && previous.width() == candidate.width()
        && previous.children().len() == candidate.children().len()
        && previous
            .children()
            .iter()
            .zip(candidate.children())
            .all(|(previous, candidate)| same_shape(previous, candidate))
}

fn same_shape_token(
    previous: &crate::GreenToken<OkfMarkdownLanguage>,
    candidate: &crate::GreenToken<OkfMarkdownLanguage>,
) -> bool {
    previous.kind() == candidate.kind()
        && previous.flags() == candidate.flags()
        && same_shape_text(previous.text(), candidate.text())
        && same_shape_trivia(previous.leading_trivia(), candidate.leading_trivia())
        && same_shape_trivia(previous.trailing_trivia(), candidate.trailing_trivia())
}

fn same_shape_trivia(previous: &[crate::GreenTrivia], candidate: &[crate::GreenTrivia]) -> bool {
    previous.len() == candidate.len()
        && previous.iter().zip(candidate).all(|(previous, candidate)| {
            previous.kind == candidate.kind && same_shape_text(&previous.text, &candidate.text)
        })
}

fn same_shape_text(previous: &crate::GreenText, candidate: &crate::GreenText) -> bool {
    match (previous, candidate) {
        // Both sides slice the same unchanged span of source, so equal widths
        // already mean equal bytes; the old slice still addresses the pre-edit
        // text, so the ranges themselves cannot be compared.
        (
            crate::GreenText::SourceSlice {
                range: previous, ..
            },
            crate::GreenText::SourceSlice {
                range: candidate, ..
            },
        ) => previous.len() == candidate.len(),
        _ => previous.write_to_string() == candidate.write_to_string(),
    }
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
    if let Some(previous) = reusable_nodes
        .get(&(candidate.kind(), range))
        .filter(|previous| same_shape_node(previous, candidate))
    {
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
                .filter(|old| same_shape_token(old, token))
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
