use std::{collections::HashMap, sync::Arc};

use waml_syntax::{
    rebase_unchanged_green, ChangeMap, GreenElement, GreenFactory, MarkdownStructureMap,
    SourceText, SyntaxIdentity, SyntaxTree, TextChange, TextRange, TextSize, TreeDiagnostic,
};
// Only `reparse_island` (test-gated below) needs the annotation transfer.
#[cfg(test)]
use waml_syntax::transfer_mapped_annotations;

mod ast;
mod kind;
mod parser;

pub(in crate::uml) use parser::{expected_layout_role, LayoutRole};

pub(crate) fn parse_full(
    text: SourceText,
    structure: &MarkdownStructureMap,
) -> Arc<SyntaxTree<UmlLanguage>> {
    parser::parse(text, structure)
}

pub(in crate::uml) fn parse_authoritative_island(
    text: SourceText,
    structure: &MarkdownStructureMap,
    owner: SyntaxIdentity,
    content_range: TextRange,
) -> Option<Arc<SyntaxTree<UmlLanguage>>> {
    let (source_range, local_structure) = structure.local_for_island(owner, content_range)?;
    let start = source_range.start().to_usize();
    let end = source_range.end().to_usize();
    let local_text = SourceText::new(text.shared()[start..end].to_owned()).ok()?;
    Some(parser::parse(local_text, &local_structure))
}

pub(in crate::uml) fn compose_full_from_islands(
    text: SourceText,
    structure: &MarkdownStructureMap,
    islands: &HashMap<(SyntaxIdentity, TextRange), Arc<SyntaxTree<UmlLanguage>>>,
) -> Option<Arc<SyntaxTree<UmlLanguage>>> {
    let factory = GreenFactory::<UmlLanguage>::new();
    let mut children = Vec::new();
    let mut diagnostics = Vec::new();
    for descriptor in parser::islands(text.len(), structure)? {
        let Some(owner) = descriptor.owner else {
            children.push(parser::parse_island_element(
                &factory,
                &text,
                text.shared(),
                structure,
                descriptor,
                &mut diagnostics,
            ));
            continue;
        };
        let tree = islands.get(&(owner, descriptor.content_range))?;
        let element = tree.root_green().children().first()?;
        let local_source = recover_exact_source(tree.root_green())?;
        let start = descriptor.range.start().to_usize();
        let end = descriptor.range.end().to_usize();
        if local_source.shared().as_str() != &text.shared()[start..end] {
            return None;
        }
        let zero = TextSize::try_from_usize(0).ok()?;
        let local_end = local_source.len();
        let changes = [
            TextChange {
                old_range: TextRange::new(zero, zero).ok()?,
                replacement: Arc::from(&text.shared()[..start]),
            },
            TextChange {
                old_range: TextRange::new(local_end, local_end).ok()?,
                replacement: Arc::from(&text.shared()[end..]),
            },
        ];
        let map = ChangeMap::checked(&local_source, &changes).ok()?;
        let rebased = rebase_unchanged_green(element, &text, &map).ok()??;
        children.push(rebased.element);
        let offset = descriptor.range.start().to_usize();
        diagnostics.extend(tree.diagnostics().iter().map(|diagnostic| {
            TreeDiagnostic {
                code: diagnostic.code,
                severity: diagnostic.severity,
                message: diagnostic.message.clone(),
                range: document_range(diagnostic.range, offset)
                    .expect("island diagnostics fit the source document"),
            }
        }));
    }
    children.push(GreenElement::Token(
        factory.missing_token(UmlSyntaxKind::EndOfFileToken),
    ));
    let root = factory.node(UmlSyntaxKind::Root, children).ok()?;
    Some(Arc::new(SyntaxTree::new(
        root,
        diagnostics.into(),
        structure.dialect,
    )))
}

fn document_range(range: TextRange, offset: usize) -> Option<TextRange> {
    TextRange::new(
        TextSize::try_from_usize(range.start().to_usize().checked_add(offset)?).ok()?,
        TextSize::try_from_usize(range.end().to_usize().checked_add(offset)?).ok()?,
    )
    .ok()
}

// Incremental island reparse. Complete and tested, but NOT wired up: every
// production path still goes through `parse_full` (see `uml::lower`), so the
// shipping build must not carry it. `cfg(test)` keeps the proof suite green and
// lets whoever lands the fast path delete one attribute to switch it on.
#[cfg(test)]
pub(in crate::uml) fn reparse_island(
    previous: &SyntaxTree<UmlLanguage>,
    previous_structure: &MarkdownStructureMap,
    text: SourceText,
    structure: &MarkdownStructureMap,
    changes: &[TextChange],
) -> Option<Arc<SyntaxTree<UmlLanguage>>> {
    let old = recover_exact_source(previous.root_green())?;
    let map = ChangeMap::checked(&old, changes).ok()?;
    if map.new_len() != text.len() || changes.is_empty() {
        return None;
    }
    let old_islands = parser::islands(old.len(), previous_structure)?;
    let new_islands = parser::islands(text.len(), structure)?;
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
    let selected_new_content_range = expanded_range(selected_old.content_range, &map)?;
    let selected_new = new_islands.iter().position(|island| {
        island.kind == selected_old.kind
            && island.range == selected_new_range
            && island.content_range == selected_new_content_range
    })?;
    if new_islands
        .iter()
        .filter(|island| {
            island.kind == selected_old.kind
                && island.range == selected_new_range
                && island.content_range == selected_new_content_range
        })
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
        let new_island = new_islands.get(index)?;
        let content_range = mapped_range(old_island.content_range, &map)?;
        if new_island.kind != old_island.kind
            || new_island.owner != old_island.owner
            || new_island.range != range
            || new_island.content_range != content_range
        {
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
        transfer_mapped_annotations(previous, &candidate, &map).ok()?
    } else {
        candidate.root_green().clone()
    };
    Some(Arc::new(SyntaxTree::new(
        root,
        Arc::from(candidate.diagnostics()),
        structure.dialect,
    )))
}

fn recover_exact_source(root: &waml_syntax::GreenNode<UmlLanguage>) -> Option<SourceText> {
    fn walk(
        root: &waml_syntax::GreenNode<UmlLanguage>,
        f: &mut impl FnMut(&waml_syntax::GreenText) -> Option<()>,
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
    let mut source: Option<SourceText> = None;
    walk(root, &mut |text| {
        if let waml_syntax::GreenText::SourceSlice { source: found, .. } = text {
            match &source {
                Some(expected) if !Arc::ptr_eq(expected.shared(), found.shared()) => return None,
                Some(_) => {}
                None => source = Some(found.clone()),
            }
        }
        Some(())
    })?;
    let source = source?;
    let mut offset = TextSize::try_from_usize(0).ok()?;
    walk(root, &mut |text| {
        match text {
            waml_syntax::GreenText::SourceSlice {
                source: found,
                range,
            } => {
                Arc::ptr_eq(source.shared(), found.shared()).then_some(())?;
                (range.start() == offset).then_some(())?;
                offset = range.end();
            }
            waml_syntax::GreenText::Static(value) => {
                let end = offset
                    .checked_add(TextSize::try_from_usize(value.len()).ok()?)
                    .ok()?;
                (source.slice(TextRange::new(offset, end).ok()?).ok()? == *value).then_some(())?;
                offset = end;
            }
            waml_syntax::GreenText::Owned(value) => {
                let end = offset
                    .checked_add(TextSize::try_from_usize(value.len()).ok()?)
                    .ok()?;
                (source.slice(TextRange::new(offset, end).ok()?).ok()? == value.as_ref())
                    .then_some(())?;
                offset = end;
            }
        }
        Some(())
    })?;
    (offset == source.len() && root.width() == source.len()).then_some(source)
}

// Helpers below serve `reparse_island` only, so they share its gate.
#[cfg(test)]
fn mapped_range(range: TextRange, map: &ChangeMap) -> Option<TextRange> {
    TextRange::new(
        map.translate_start_boundary(range.start())?,
        map.translate_end_boundary(range.end())?,
    )
    .ok()
}
#[cfg(test)]
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
#[cfg(test)]
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
#[cfg(test)]
fn has_annotations(node: &waml_syntax::GreenNode<UmlLanguage>) -> bool {
    !node.annotations().is_empty()
        || node.children().iter().any(|child| match child {
            GreenElement::Node(child) => has_annotations(child),
            GreenElement::Token(token) => !token.syntax_annotations().is_empty(),
        })
}

pub use ast::{
    AnchoredSyntax, AttributeSyntax, AxisSyntax, BindingSyntax, DiagramMembersSyntax,
    DirectionClauseSyntax, EdgeSyntax, FlagSyntax, FlowBlockSyntax, FlowInternalSyntax,
    FlowNodeSyntax, FlowTraceSyntax, FlowTracesSyntax, FlowTransitionSyntax, GateSyntax,
    HintClauseSyntax, HintSyntax, InlineInstanceSyntax, InteractionUseSyntax,
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
        parse_markdown, reparse_markdown, DocumentRevision, GreenElement, GreenFactory, GreenText,
        MarkdownDialect, ParseError, ShellParse, SourceText, SyntaxAnnotation, SyntaxElement,
        SyntaxNode, SyntaxToken, SyntaxTree, TextChange, TextRange, TextSize,
    };

    use super::{parse_full, recover_exact_source, reparse_island, UmlLanguage, UmlSyntaxKind};

    fn parse_okf_markdown(
        text: SourceText,
        dialect: MarkdownDialect,
    ) -> Result<ShellParse, ParseError> {
        let snapshot = parse_markdown(DocumentRevision::INITIAL, text, dialect)?;
        Ok(ShellParse {
            tree: snapshot.tree().clone(),
            structure: snapshot.structure().clone(),
        })
    }

    fn markdown_reparse_pair(
        old_text: &SourceText,
        new_text: SourceText,
    ) -> (ShellParse, ShellParse, Vec<TextChange>) {
        let changes = crate::analysis::single_text_change(old_text, &new_text);
        let old_snapshot = parse_markdown(
            DocumentRevision::INITIAL,
            old_text.clone(),
            MarkdownDialect::WAML_DEFAULT,
        )
        .unwrap();
        let update =
            reparse_markdown(&old_snapshot, DocumentRevision::new(2), new_text, &changes).unwrap();
        (
            ShellParse {
                tree: old_snapshot.tree().clone(),
                structure: old_snapshot.structure().clone(),
            },
            ShellParse {
                tree: update.snapshot.tree().clone(),
                structure: update.snapshot.structure().clone(),
            },
            changes,
        )
    }

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
        SourceSlice { range: TextRange, spelling: String },
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
    fn malformed_lifeline_preserves_space_before_recovery() {
        let authored = "## Lifelines\nm ha";
        let text = SourceText::from_shared(Arc::new(authored.to_owned())).unwrap();
        let shell = parse_okf_markdown(text.clone(), MarkdownDialect::WAML_DEFAULT).unwrap();
        let tree = parse_full(text, &shell.structure);

        assert_eq!(tree.write_to_string(), authored);
    }

    #[test]
    fn malformed_message_preserves_space_before_recovery() {
        let authored = "## Messages\nas `s";
        let text = SourceText::from_shared(Arc::new(authored.to_owned())).unwrap();
        let shell = parse_okf_markdown(text.clone(), MarkdownDialect::WAML_DEFAULT).unwrap();
        let tree = parse_full(text, &shell.structure);

        assert_eq!(tree.write_to_string(), authored);
    }

    #[test]
    fn malformed_message_preserves_trailing_space() {
        let authored = "## Messages\nD, D ";
        let text = SourceText::from_shared(Arc::new(authored.to_owned())).unwrap();
        let shell = parse_okf_markdown(text.clone(), MarkdownDialect::WAML_DEFAULT).unwrap();
        let tree = parse_full(text, &shell.structure);

        assert_eq!(tree.write_to_string(), authored);
    }

    #[test]
    fn malformed_message_preserves_space_before_value_recovery() {
        let authored = "## Messages\n- A calls B `s";
        let text = SourceText::from_shared(Arc::new(authored.to_owned())).unwrap();
        let shell = parse_okf_markdown(text.clone(), MarkdownDialect::WAML_DEFAULT).unwrap();
        let tree = parse_full(text, &shell.structure);

        assert_eq!(tree.write_to_string(), authored);
    }

    #[test]
    fn whitespace_only_value_line_is_lossless() {
        let authored = "## Values\n ";
        let text = SourceText::from_shared(Arc::new(authored.to_owned())).unwrap();
        let shell = parse_okf_markdown(text.clone(), MarkdownDialect::WAML_DEFAULT).unwrap();
        let tree = parse_full(text, &shell.structure);

        assert_eq!(tree.write_to_string(), authored);
    }

    #[test]
    fn malformed_relationship_preserves_space_before_link_recovery() {
        let authored = "## Relationships\n- status: Draft\n";
        let text = SourceText::from_shared(Arc::new(authored.to_owned())).unwrap();
        let shell = parse_okf_markdown(text.clone(), MarkdownDialect::WAML_DEFAULT).unwrap();
        let tree = parse_full(text, &shell.structure);

        assert_eq!(tree.write_to_string(), authored);
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
        let (old_shell, new_shell, changes) = markdown_reparse_pair(&old_text, new_text.clone());
        let previous = parse_full(old_text.clone(), &old_shell.structure);
        let full = parse_full(new_text.clone(), &new_shell.structure);

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
        let (old_shell, new_shell, changes) = markdown_reparse_pair(&old_text, new_text.clone());
        let previous = parse_full(old_text.clone(), &old_shell.structure);
        let full = parse_full(new_text.clone(), &new_shell.structure);
        assert_eq!(previous.write_to_string(), old_text.shared().as_str());
        let reparsed = reparse_island(
            &previous,
            &old_shell.structure,
            new_text.clone(),
            &new_shell.structure,
            &changes,
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
        let (old_shell, new_shell, changes) = markdown_reparse_pair(&old_text, new_text.clone());
        let previous = parse_full(old_text.clone(), &old_shell.structure);

        assert!(reparse_island(
            &previous,
            &old_shell.structure,
            new_text.clone(),
            &new_shell.structure,
            &changes,
        )
        .is_none());
    }

    #[test]
    fn source_recovery_handles_deep_and_wide_uml_trees_iteratively() {
        fn token(source: &SourceText, start: usize, end: usize) -> GreenElement<UmlLanguage> {
            GreenElement::Token(
                GreenFactory::new()
                    .token(
                        UmlSyntaxKind::IdentifierToken,
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
        let deep_source = SourceText::from_shared(Arc::new("x".to_owned())).unwrap();
        let mut deep = GreenFactory::new()
            .node(UmlSyntaxKind::MarkdownRegion, [token(&deep_source, 0, 1)])
            .unwrap();
        for _ in 0..2_048 {
            deep = GreenFactory::new()
                .node(UmlSyntaxKind::MarkdownRegion, [GreenElement::Node(deep)])
                .unwrap();
        }
        let deep_root = GreenFactory::new()
            .node(UmlSyntaxKind::Root, [GreenElement::Node(deep)])
            .unwrap();
        assert!(Arc::ptr_eq(
            recover_exact_source(&deep_root).unwrap().shared(),
            deep_source.shared()
        ));

        let wide_source = SourceText::from_shared(Arc::new("x".repeat(20_000))).unwrap();
        let wide_root = GreenFactory::new()
            .node(
                UmlSyntaxKind::Root,
                (0..20_000).map(|i| token(&wide_source, i, i + 1)),
            )
            .unwrap();
        assert!(Arc::ptr_eq(
            recover_exact_source(&wide_root).unwrap().shared(),
            wide_source.shared()
        ));
        std::mem::forget(deep_root);
    }

    #[test]
    fn source_recovery_rejects_hostile_streams() {
        fn token(source: &SourceText, start: usize, end: usize) -> GreenElement<UmlLanguage> {
            GreenElement::Token(
                GreenFactory::new()
                    .token(
                        UmlSyntaxKind::IdentifierToken,
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
        let empty_source = SourceText::from_shared(Arc::new(String::new())).unwrap();
        let source = SourceText::from_shared(Arc::new("ab".to_owned())).unwrap();
        let factory = GreenFactory::new();
        let parser_source = SourceText::from_shared(Arc::new(
            "---\ntype: uml.Class\n---\n# Ordinary\n## Attributes\n- name: String\n".to_owned(),
        ))
        .unwrap();
        let shell =
            parse_okf_markdown(parser_source.clone(), MarkdownDialect::WAML_DEFAULT).unwrap();
        let ordinary = parse_full(parser_source.clone(), &shell.structure);
        assert!(Arc::ptr_eq(
            recover_exact_source(ordinary.root_green())
                .unwrap()
                .shared(),
            parser_source.shared()
        ));
        let empty = factory
            .node(UmlSyntaxKind::Root, [token(&empty_source, 0, 0)])
            .unwrap();
        assert!(Arc::ptr_eq(
            recover_exact_source(&empty).unwrap().shared(),
            empty_source.shared()
        ));
        let trivia = factory
            .trivia(
                waml_syntax::TriviaKind::Whitespace,
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
                UmlSyntaxKind::Root,
                [GreenElement::Token(
                    factory
                        .token(
                            UmlSyntaxKind::IdentifierToken,
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
                UmlSyntaxKind::Root,
                [token(&source, 1, 2), token(&source, 0, 1)],
            )
            .unwrap();
        assert!(recover_exact_source(&reordered).is_none());
        let duplicated = factory
            .node(
                UmlSyntaxKind::Root,
                [token(&source, 0, 1), token(&source, 0, 1)],
            )
            .unwrap();
        assert!(recover_exact_source(&duplicated).is_none());
        let overlap = factory
            .node(
                UmlSyntaxKind::Root,
                [token(&source, 0, 1), token(&source, 0, 2)],
            )
            .unwrap();
        assert!(recover_exact_source(&overlap).is_none());
        let gap = factory
            .node(UmlSyntaxKind::Root, [token(&source, 1, 2)])
            .unwrap();
        assert!(recover_exact_source(&gap).is_none());
        let other = SourceText::from_shared(Arc::new("ab".to_owned())).unwrap();
        let mixed = factory
            .node(
                UmlSyntaxKind::Root,
                [token(&source, 0, 1), token(&other, 1, 2)],
            )
            .unwrap();
        assert!(recover_exact_source(&mixed).is_none());
        let owned = factory
            .node(
                UmlSyntaxKind::Root,
                [
                    token(&source, 0, 1),
                    GreenElement::Token(
                        factory
                            .token(
                                UmlSyntaxKind::IdentifierToken,
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
        let static_mismatch = factory
            .node(
                UmlSyntaxKind::Root,
                [
                    token(&source, 0, 1),
                    GreenElement::Token(
                        factory
                            .token(
                                UmlSyntaxKind::IdentifierToken,
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
                UmlSyntaxKind::Root,
                [GreenElement::Token(
                    factory
                        .token(
                            UmlSyntaxKind::IdentifierToken,
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
}
