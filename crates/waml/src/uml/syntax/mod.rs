use std::sync::Arc;

use waml_syntax::{
    rebase_unchanged_green, transfer_mapped_annotations, ChangeMap, GreenElement, GreenFactory,
    MarkdownStructureMap, SourceText, SyntaxTree, TextChange, TextRange, TextSize, TreeDiagnostic,
};

mod ast;
mod kind;
mod parser;

pub(in crate::uml) fn parse_full(
    text: SourceText,
    structure: &MarkdownStructureMap,
) -> Arc<SyntaxTree<UmlLanguage>> {
    parser::parse(text, structure)
}

pub(in crate::uml) fn reparse_island(
    previous: &SyntaxTree<UmlLanguage>,
    previous_structure: &MarkdownStructureMap,
    text: SourceText,
    structure: &MarkdownStructureMap,
    changes: &[TextChange],
) -> Option<Arc<SyntaxTree<UmlLanguage>>> {
    let old = SourceText::from_shared(Arc::new(previous.write_to_string())).ok()?;
    let map = ChangeMap::checked(&old, changes).ok()?;
    if map.new_len() != text.len()
        || changes.is_empty()
        || !same_structure(previous_structure, structure, &map)
    {
        return None;
    }
    let old_islands = parser::islands(old.shared(), previous_structure)?;
    let new_islands = parser::islands(text.shared(), structure)?;
    let changed = map.changed_old_range()?;
    let selected = select_owner(&old_islands, changed, map.old_len())?;
    let selected_old = old_islands[selected];
    let selected_new_range = expanded_range(selected_old.range, &map)?;
    let selected_new = new_islands.iter().position(|island| {
        island.kind == selected_old.kind && island.range == selected_new_range
    })?;
    if new_islands
        .iter()
        .filter(|island| island.kind == selected_old.kind && island.range == selected_new_range)
        .count()
        != 1
    {
        return None;
    }
    if old_islands.len() != new_islands.len() {
        return None;
    }
    for (index, old_island) in old_islands.iter().enumerate() {
        if index == selected {
            continue;
        }
        let range = mapped_range(old_island.range, &map)?;
        let Some(new_island) = new_islands.get(index) else {
            return None;
        };
        if new_island.kind != old_island.kind || new_island.range != range {
            return None;
        }
    }
    let factory = GreenFactory::<UmlLanguage>::new();
    let source = text.shared();
    let mut regenerated = Vec::new();
    let parsed = parser::parse_island_element(
        &factory,
        &text,
        source,
        structure,
        new_islands[selected_new],
        &mut regenerated,
    );
    let old_children = previous.root_green().children();
    if old_children.len() != old_islands.len() + 1 {
        return None;
    }
    let mut children = Vec::with_capacity(old_children.len());
    for (index, child) in old_children.iter().enumerate() {
        if index == selected {
            children.push(parsed.clone());
            continue;
        }
        children.push(rebase_unchanged_green(child, &text, &map).ok()??.element);
    }
    let root = factory.node(UmlSyntaxKind::Root, children).ok()?;
    let mut diagnostics: Vec<TreeDiagnostic<UmlSyntaxDiagnosticCode>> = previous
        .diagnostics()
        .iter()
        .filter_map(|diagnostic| {
            (diagnostic.range.end() <= selected_old.range.start()
                || diagnostic.range.start() >= selected_old.range.end())
            .then(|| map.translate_unchanged(diagnostic.range))?
            .map(|range| TreeDiagnostic {
                code: diagnostic.code,
                severity: diagnostic.severity,
                message: diagnostic.message.clone(),
                range,
            })
        })
        .collect();
    diagnostics.append(&mut regenerated);
    diagnostics.sort_by_key(|d| (d.range.start(), d.range.end(), d.code as u8));
    let candidate = SyntaxTree::new(root, diagnostics.into(), structure.dialect);
    let root = if has_annotations(previous.root_green()) {
        transfer_mapped_annotations(previous, &candidate, &map)
    } else {
        candidate.root_green().clone()
    };
    Some(Arc::new(SyntaxTree::new(
        root,
        Arc::from(candidate.diagnostics()),
        structure.dialect,
    )))
}

fn mapped_range(range: TextRange, map: &ChangeMap) -> Option<TextRange> {
    TextRange::new(
        map.translate_start_boundary(range.start())?,
        map.translate_end_boundary(range.end())?,
    )
    .ok()
}
fn expanded_range(range: TextRange, map: &ChangeMap) -> Option<TextRange> {
    let mut mapped = mapped_range(range, map)?;
    for segment in map
        .segments()
        .iter()
        .filter(|segment| segment.old.start() == segment.old.end())
    {
        if range.start() <= segment.old.start() && segment.old.start() <= range.end() {
            mapped = TextRange::new(
                mapped.start().min(segment.new.start()),
                mapped.end().max(segment.new.end()),
            )
            .ok()?;
        }
    }
    Some(mapped)
}
fn select_owner(
    islands: &[parser::Island],
    changed: TextRange,
    old_len: TextSize,
) -> Option<usize> {
    let zero = changed.start() == changed.end();
    let owners: Vec<_> = islands
        .iter()
        .enumerate()
        .filter(|(_, island)| {
            if zero {
                island.range.start() <= changed.start() && changed.start() <= island.range.end()
            } else {
                island.range.start() <= changed.start() && changed.end() <= island.range.end()
            }
        })
        .map(|(index, _)| index)
        .collect();
    if zero && changed.start() == old_len {
        return owners.last().copied();
    }
    (owners.len() == 1).then(|| owners[0])
}
fn same_structure(old: &MarkdownStructureMap, new: &MarkdownStructureMap, map: &ChangeMap) -> bool {
    let same = |old: &[TextRange], new: &[TextRange]| {
        old.iter()
            .map(|range| mapped_range(*range, map))
            .collect::<Option<Vec<_>>>()
            .as_deref()
            == Some(new)
    };
    old.headings
        .iter()
        .zip(new.headings.iter())
        .all(|(left, right)| {
            left.level == right.level
                && mapped_range(left.range, map) == Some(right.range)
                && mapped_range(left.text_range, map) == Some(right.text_range)
        })
        && old.headings.len() == new.headings.len()
        && old
            .nested_headings
            .iter()
            .zip(new.nested_headings.iter())
            .all(|(left, right)| {
                left.level == right.level
                    && mapped_range(left.range, map) == Some(right.range)
                    && mapped_range(left.text_range, map) == Some(right.text_range)
            })
        && old.nested_headings.len() == new.nested_headings.len()
        && same(&old.protected_ranges, &new.protected_ranges)
        && same(&old.opaque_ranges, &new.opaque_ranges)
        && same(&old.list_item_lines, &new.list_item_lines)
        && same(&old.tab_indented_item_lines, &new.tab_indented_item_lines)
}
fn has_annotations(node: &waml_syntax::GreenNode<UmlLanguage>) -> bool {
    !node.annotations().is_empty()
        || node.children().iter().any(|child| match child {
            GreenElement::Node(child) => has_annotations(child),
            GreenElement::Token(token) => !token.syntax_annotations().is_empty(),
        })
}

pub use ast::{
    AnchoredSyntax, AttributeSyntax, AxisSyntax, DiagramMembersSyntax, DirectionClauseSyntax,
    EdgeSyntax, FlagSyntax, FlowBlockSyntax, FlowInternalSyntax, FlowNodeSyntax,
    FlowTransitionSyntax, HintClauseSyntax, HintSyntax, InlineInstanceSyntax,
    LayoutAlignmentSyntax, LayoutAtomSyntax, LayoutPlacementSyntax, LayoutSectionSyntax,
    LayoutStandaloneSyntax, LayoutStatementSyntax, LifelineSyntax, MarginSyntax, MemberGroupSyntax,
    MemberLineSyntax, MemberSyntax, MessageSyntax, MessagesBlockSyntax, MultiplicitySyntax,
    NameRefSyntax, OperandRefSyntax, OperandSyntax, RelationshipEndSyntax, RelationshipSyntax,
    SequenceFragmentSyntax, SequenceOperandSyntax, ShapeSyntax, SlotSyntax, SlotValueKind,
    TypeReferenceSyntax, ValueSyntax,
};
pub use kind::{UmlSyntaxDiagnosticCode, UmlSyntaxKind};
#[derive(Debug)]
pub struct UmlLanguage;
impl waml_syntax::SyntaxLanguage for UmlLanguage {
    type Kind = UmlSyntaxKind;
    type DiagnosticCode = UmlSyntaxDiagnosticCode;
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use waml_syntax::{parse_okf_markdown, MarkdownDialect, SourceText, SyntaxElement, SyntaxTree};

    use super::{parse_full, reparse_island, UmlLanguage, UmlSyntaxKind};

    fn tree_fingerprint(tree: &SyntaxTree<UmlLanguage>) -> (String, Vec<(u32, u32, u8)>) {
        (
            tree.write_to_string(),
            tree.diagnostics()
                .iter()
                .map(|diagnostic| {
                    (
                        diagnostic.range.start().to_usize() as u32,
                        diagnostic.range.end().to_usize() as u32,
                        diagnostic.code as u8,
                    )
                })
                .collect(),
        )
    }

    fn first_token(
        tree: &SyntaxTree<UmlLanguage>,
        kind: UmlSyntaxKind,
    ) -> waml_syntax::SyntaxToken<UmlLanguage> {
        tree.root()
            .children()
            .find_map(|element| match element {
                SyntaxElement::Token(token) if token.kind() == kind => Some(token),
                _ => None,
            })
            .expect("expected token")
    }

    #[test]
    fn reparse_island_matches_full_and_reuses_only_source_independent_greens() {
        let old_text = SourceText::from_shared(Arc::new(
            "## Attributes\n- old: String\n\n## Layout\n- item\n".to_owned(),
        ))
        .unwrap();
        let new_text = SourceText::from_shared(Arc::new(
            "## Attributes\n- new: String\n\n## Layout\n- item\n".to_owned(),
        ))
        .unwrap();
        let old_shell =
            parse_okf_markdown(old_text.clone(), MarkdownDialect::CommonMarkCurrent).unwrap();
        let new_shell =
            parse_okf_markdown(new_text.clone(), MarkdownDialect::CommonMarkCurrent).unwrap();
        let previous = parse_full(old_text.clone(), &old_shell.structure);
        let full = parse_full(new_text.clone(), &new_shell.structure);
        let changes = crate::analysis::single_text_change(&old_text, &new_text);

        let reparsed = reparse_island(
            &previous,
            &old_shell.structure,
            new_text,
            &new_shell.structure,
            &changes,
        )
        .expect("attribute edit has one unambiguous island");

        assert_eq!(tree_fingerprint(&reparsed), tree_fingerprint(&full));
        assert!(first_token(&previous, UmlSyntaxKind::EndOfFileToken)
            .same_green(&first_token(&reparsed, UmlSyntaxKind::EndOfFileToken)));
        assert!(!previous.root().same_green(&reparsed.root()));
    }
}
