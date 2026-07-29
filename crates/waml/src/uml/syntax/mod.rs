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
        || !same_structure(&old, &text, previous_structure, structure, &map)
    {
        return None;
    }
    let old_islands = parser::islands(old.shared(), previous_structure)?;
    let new_islands = parser::islands(text.shared(), structure)?;
    let changed = map.changed_old_range()?;
    let selected = select_owner(&old_islands, changed, map.old_len())?;
    let selected_old = old_islands[selected];
    if previous.diagnostics().iter().any(|diagnostic| {
        diagnostic.range.start() == diagnostic.range.end()
            && ((selected > 0 && diagnostic.range.start() == selected_old.range.start())
                || (selected + 1 < old_islands.len()
                    && diagnostic.range.start() == selected_old.range.end()))
    }) {
        return None;
    }
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
            let boundary_point = diagnostic.range.start() == diagnostic.range.end()
                && (diagnostic.range.start() == selected_old.range.start()
                    || diagnostic.range.start() == selected_old.range.end());
            (!boundary_point
                && (diagnostic.range.end() <= selected_old.range.start()
                    || diagnostic.range.start() >= selected_old.range.end()))
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
fn same_structure(
    old_text: &SourceText,
    new_text: &SourceText,
    old: &MarkdownStructureMap,
    new: &MarkdownStructureMap,
    map: &ChangeMap,
) -> bool {
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
                && (left.level != 2
                    || old_text.slice(left.range).ok() == new_text.slice(right.range).ok())
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

    use waml_syntax::{
        parse_okf_markdown, GreenElement, GreenText, MarkdownDialect, SourceText, SyntaxAnnotation,
        SyntaxElement, SyntaxNode, SyntaxToken, SyntaxTree, TextSize,
    };

    use super::{parse_full, reparse_island, UmlLanguage, UmlSyntaxKind};

    fn annotations(annotations: &[SyntaxAnnotation]) -> Vec<(u64, &str, Option<&str>)> {
        annotations
            .iter()
            .map(|annotation| (annotation.id().get(), annotation.kind(), annotation.data()))
            .collect()
    }

    #[derive(Debug, Eq, PartialEq)]
    enum TextFingerprint {
        Static(String),
        Owned(String),
        SourceSlice {
            range: waml_syntax::TextRange,
            spelling: String,
        },
    }

    fn text_fingerprint(text: &GreenText) -> TextFingerprint {
        match text {
            GreenText::Static(value) => TextFingerprint::Static((*value).to_owned()),
            GreenText::Owned(value) => TextFingerprint::Owned(value.to_string()),
            GreenText::SourceSlice { range, .. } => TextFingerprint::SourceSlice {
                range: *range,
                spelling: text.write_to_string(),
            },
        }
    }

    fn structural_fingerprint(tree: &SyntaxTree<UmlLanguage>) -> Vec<String> {
        fn visit(
            element: &GreenElement<UmlLanguage>,
            at: TextSize,
            out: &mut Vec<String>,
        ) -> TextSize {
            match element {
                GreenElement::Node(node) => {
                    let end = at.checked_add(node.width()).unwrap();
                    out.push(format!(
                        "node:{:?}:{at:?}..{end:?}:{:?}",
                        node.kind(),
                        annotations(node.annotations())
                    ));
                    node.children()
                        .iter()
                        .fold(at, |offset, child| visit(child, offset, out))
                }
                GreenElement::Token(token) => {
                    let end = at.checked_add(token.width()).unwrap();
                    let leading: Vec<_> = token
                        .leading_trivia()
                        .iter()
                        .map(|trivia| (trivia.kind, text_fingerprint(&trivia.text)))
                        .collect();
                    let trailing: Vec<_> = token
                        .trailing_trivia()
                        .iter()
                        .map(|trivia| (trivia.kind, text_fingerprint(&trivia.text)))
                        .collect();
                    out.push(format!(
                        "token:{:?}:{at:?}..{end:?}:{:?}:{leading:?}:{trailing:?}:missing={}:bad={}:codes={:?}:{:?}",
                        token.kind(),
                        text_fingerprint(token.text()),
                        token.flags().is_missing(),
                        token.flags().is_bad(),
                        token.annotations(),
                        annotations(token.syntax_annotations())
                    ));
                    end
                }
            }
        }
        let mut out = Vec::new();
        visit(
            &GreenElement::Node(tree.root_green().clone()),
            TextSize::try_from_usize(0).unwrap(),
            &mut out,
        );
        out
    }

    fn diagnostic_fingerprint(tree: &SyntaxTree<UmlLanguage>) -> Vec<String> {
        tree.diagnostics()
            .iter()
            .map(|diagnostic| {
                format!(
                    "{:?}:{:?}:{:?}:{}",
                    diagnostic.code, diagnostic.severity, diagnostic.range, diagnostic.message
                )
            })
            .collect()
    }

    fn first_node(tree: &SyntaxTree<UmlLanguage>, kind: UmlSyntaxKind) -> SyntaxNode<UmlLanguage> {
        fn find(
            node: SyntaxNode<UmlLanguage>,
            kind: UmlSyntaxKind,
        ) -> Option<SyntaxNode<UmlLanguage>> {
            if node.kind() == kind {
                return Some(node);
            }
            node.children()
                .find_map(|child| child.into_node().and_then(|node| find(node, kind)))
        }
        find(tree.root(), kind).expect("expected node")
    }

    fn first_missing_token(node: SyntaxNode<UmlLanguage>) -> SyntaxToken<UmlLanguage> {
        fn find(node: SyntaxNode<UmlLanguage>) -> Option<SyntaxToken<UmlLanguage>> {
            node.children().find_map(|child| match child {
                SyntaxElement::Token(token) if token.flags().is_missing() => Some(token),
                SyntaxElement::Node(node) => find(node),
                _ => None,
            })
        }
        find(node).expect("expected missing token")
    }

    #[test]
    fn reparse_island_matches_full_and_reuses_only_source_independent_greens() {
        let old_text = SourceText::from_shared(Arc::new(
            "## Attributes\n- old: String\n\n## Layout\n- left of\n".to_owned(),
        ))
        .unwrap();
        let new_text = SourceText::from_shared(Arc::new(
            "## Attributes\n- new: String\n\n## Layout\n- left of\n".to_owned(),
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

        assert_eq!(
            structural_fingerprint(&reparsed),
            structural_fingerprint(&full)
        );
        assert_eq!(
            diagnostic_fingerprint(&reparsed),
            diagnostic_fingerprint(&full)
        );
        let previous_layout = first_node(&previous, UmlSyntaxKind::LayoutSection);
        let reparsed_layout = first_node(&reparsed, UmlSyntaxKind::LayoutSection);
        assert!(first_missing_token(previous_layout.clone())
            .same_green(&first_missing_token(reparsed_layout.clone())));
        assert!(!first_node(&previous, UmlSyntaxKind::AttributesSection)
            .same_green(&first_node(&reparsed, UmlSyntaxKind::AttributesSection)));
        assert!(!previous_layout.same_green(&reparsed_layout));
        assert!(!previous.root().same_green(&reparsed.root()));
    }

    #[test]
    fn reparse_island_does_not_duplicate_final_boundary_diagnostics() {
        let old_text = SourceText::from_shared(Arc::new(
            "## Nodes\n### state Node\ntrigger: before\n###".to_owned(),
        ))
        .unwrap();
        let new_text = SourceText::from_shared(Arc::new(
            "## Nodes\n### state Node\ntrigger: changed\n###".to_owned(),
        ))
        .unwrap();
        let old_shell =
            parse_okf_markdown(old_text.clone(), MarkdownDialect::CommonMarkCurrent).unwrap();
        let new_shell =
            parse_okf_markdown(new_text.clone(), MarkdownDialect::CommonMarkCurrent).unwrap();
        let previous = parse_full(old_text.clone(), &old_shell.structure);
        let full = parse_full(new_text.clone(), &new_shell.structure);
        assert_eq!(previous.write_to_string(), old_text.shared().as_str());
        let reparsed = reparse_island(
            &previous,
            &old_shell.structure,
            new_text.clone(),
            &new_shell.structure,
            &crate::analysis::single_text_change(&old_text, &new_text),
        )
        .expect("earlier edit in one final island is unambiguous");

        assert_eq!(
            structural_fingerprint(&reparsed),
            structural_fingerprint(&full)
        );
        assert_eq!(
            diagnostic_fingerprint(&reparsed),
            diagnostic_fingerprint(&full)
        );
    }

    #[test]
    fn reparse_island_rejects_recognized_heading_rename() {
        let old_text =
            SourceText::from_shared(Arc::new("## Attributes\n- old: String\n".to_owned())).unwrap();
        let new_text =
            SourceText::from_shared(Arc::new("## attributes\n- old: String\n".to_owned())).unwrap();
        let old_shell =
            parse_okf_markdown(old_text.clone(), MarkdownDialect::CommonMarkCurrent).unwrap();
        let new_shell =
            parse_okf_markdown(new_text.clone(), MarkdownDialect::CommonMarkCurrent).unwrap();
        let previous = parse_full(old_text.clone(), &old_shell.structure);

        assert!(reparse_island(
            &previous,
            &old_shell.structure,
            new_text.clone(),
            &new_shell.structure,
            &crate::analysis::single_text_change(&old_text, &new_text),
        )
        .is_none());
    }
}
