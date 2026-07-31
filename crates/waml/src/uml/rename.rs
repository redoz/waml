use super::lower::{find_doc, UmlLoweringState};
use crate::edit::EditError;
use crate::source::{BundlePath, SourceBundle};
use waml_syntax::{
    parse_markdown, DocumentRevision, MarkdownDialect, SourceText, SyntaxElement, SyntaxNode,
};

/// Resolve a rename destination from the exact source path. A bare `to`
/// replaces only the basename in the source directory; a slash-containing
/// `to` is an explicit bundle-root-relative concept id.
pub(crate) fn destination_path(source: &BundlePath, to: &str) -> Result<BundlePath, EditError> {
    let to = to.strip_suffix(".md").unwrap_or(to);
    let destination = if to.contains(['/', '\\']) {
        format!("{to}.md")
    } else {
        match source.as_str().rfind('/') {
            Some(index) => format!("{}/{to}.md", &source.as_str()[..index]),
            None => format!("{to}.md"),
        }
    };
    BundlePath::parse(destination).map_err(|error| EditError::at("node.rename", error.to_string()))
}

pub(crate) fn op_node_rename(
    work: &mut SourceBundle,
    state: &UmlLoweringState,
    from: &str,
    to: &str,
) -> Result<(), EditError> {
    let idx = find_doc(work, from, "node.rename")?;
    let source_path = work
        .document_at(idx)
        .expect("resolved document index")
        .path()
        .clone();
    let dest_path = destination_path(&source_path, to)?;
    if work
        .documents()
        .iter()
        .enumerate()
        .any(|(i, document)| i != idx && document.path() == &dest_path)
    {
        return Err(EditError::at(
            "node.rename",
            format!("target slug '{to}' already exists"),
        ));
    }
    let claimed_paths: Vec<_> = state.claimed_paths().cloned().collect();
    for referrer_path in claimed_paths {
        let index = work
            .documents()
            .iter()
            .position(|document| document.path() == &referrer_path)
            .expect("claimed document path");
        let document = work.document_at(index).expect("document index in bounds");
        let source = document.text().to_owned();
        let post_rename_referrer = if referrer_path == source_path {
            &dest_path
        } else {
            &referrer_path
        };
        let changed = rename_typed_references(
            &source,
            &referrer_path,
            post_rename_referrer,
            &source_path,
            &dest_path,
        )?;
        if changed != source {
            *work
                .document_at_mut(index)
                .expect("document index in bounds")
                .text_mut() = changed;
        }
    }
    work.rename_document(idx, dest_path.as_str().to_owned())
        .map_err(|error| EditError::at("node.rename", error.to_string()))?;
    Ok(())
}

fn directory_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(directory, _)| directory)
}

fn relative_href(source: &BundlePath, destination: &BundlePath) -> String {
    let source_directory: Vec<_> = directory_of(source.as_str()).split('/').collect();
    let source_directory = if source_directory == [""] {
        &[][..]
    } else {
        &source_directory
    };
    let destination_segments: Vec<_> = destination.as_str().split('/').collect();
    let shared = source_directory
        .iter()
        .zip(&destination_segments)
        .take_while(|(left, right)| left == right)
        .count();
    let mut href = "../".repeat(source_directory.len() - shared);
    href.push_str(&destination_segments[shared..].join("/"));
    if !href.starts_with("../") {
        href.insert_str(0, "./");
    }
    href
}

fn rename_typed_references(
    source: &str,
    referrer_before: &BundlePath,
    referrer_after: &BundlePath,
    target_before: &BundlePath,
    target_after: &BundlePath,
) -> Result<String, EditError> {
    let text = SourceText::from_shared(std::sync::Arc::new(source.to_owned()))
        .map_err(|error| EditError::at("node.rename", error.to_string()))?;
    let markdown = parse_markdown(
        DocumentRevision::INITIAL,
        text.clone(),
        MarkdownDialect::CommonMarkCurrent,
    )
        .map_err(|error| EditError::at("node.rename", error.to_string()))?;
    let tree = super::syntax::parse_full(text, markdown.structure());
    let mut edits = Vec::new();
    collect_reference_edits(
        &tree.root(),
        source,
        referrer_before,
        referrer_after,
        target_before,
        target_after,
        &mut edits,
    );
    edits.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    let mut output = source.to_owned();
    for (range, replacement) in edits {
        output.replace_range(range, &replacement);
    }
    Ok(output)
}

fn collect_reference_edits(
    node: &SyntaxNode<super::syntax::UmlLanguage>,
    source: &str,
    referrer_before: &BundlePath,
    referrer_after: &BundlePath,
    target_before: &BundlePath,
    target_after: &BundlePath,
    edits: &mut Vec<(std::ops::Range<usize>, String)>,
) {
    use super::syntax::UmlSyntaxKind;
    if node.kind() == UmlSyntaxKind::Attribute {
        let range = node.range().start().to_usize()..node.range().end().to_usize();
        let authored = &source[range.clone()];
        if let Some(href_range) = markdown_href_range(authored) {
            if let Some(replacement) = rewritten_href(
                &authored[href_range.clone()],
                referrer_before,
                referrer_after,
                target_before,
                target_after,
            ) {
                let start = range.start + href_range.start;
                edits.push((start..range.start + href_range.end, replacement));
            }
        }
        return;
    }
    for child in node.children() {
        match child {
            SyntaxElement::Node(child) => {
                collect_reference_edits(
                    &child,
                    source,
                    referrer_before,
                    referrer_after,
                    target_before,
                    target_after,
                    edits,
                );
            }
            SyntaxElement::Token(token)
                if matches!(
                    token.kind(),
                    UmlSyntaxKind::LinkTargetToken
                        | UmlSyntaxKind::TypeToken
                        | UmlSyntaxKind::LayoutLinkToken
                ) =>
            {
                let range = token.range().start().to_usize()..token.range().end().to_usize();
                let authored = &source[range.clone()];
                let href_range = if token.kind() == UmlSyntaxKind::LinkTargetToken {
                    0..authored.len()
                } else if let Some(range) = markdown_href_range(authored) {
                    range
                } else {
                    continue;
                };
                if let Some(replacement) = rewritten_href(
                    &authored[href_range.clone()],
                    referrer_before,
                    referrer_after,
                    target_before,
                    target_after,
                ) {
                    let start = range.start + href_range.start;
                    edits.push((start..range.start + href_range.end, replacement));
                }
            }
            SyntaxElement::Token(token) if token.kind() == UmlSyntaxKind::LayoutWordToken => {
                let range = token.range().start().to_usize()..token.range().end().to_usize();
                let authored = &source[range.clone()];
                let leading = authored.len() - authored.trim_start().len();
                let trailing = authored.trim_end().len();
                let word = &authored[leading..trailing];
                let target =
                    crate::okf::resolve_href(referrer_before.as_str(), &format!("./{word}.md"));
                if target_before.concept_id() == Some(target.as_str()) {
                    let destination = relative_href(referrer_after, target_after);
                    let replacement = destination
                        .strip_prefix("./")
                        .and_then(|href| href.strip_suffix(".md"))
                        .filter(|slug| !slug.contains('/'))
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("[{word}]({destination})"));
                    edits.push((
                        range,
                        format!(
                            "{}{replacement}{}",
                            &authored[..leading],
                            &authored[trailing..]
                        ),
                    ));
                }
            }
            SyntaxElement::Token(_) => {}
        }
    }
}

fn markdown_href_range(authored: &str) -> Option<std::ops::Range<usize>> {
    let start = authored.find("](")? + 2;
    let end = authored[start..].rfind(')')? + start;
    (start <= end).then_some(start..end)
}

fn rewritten_href(
    authored: &str,
    referrer_before: &BundlePath,
    referrer_after: &BundlePath,
    target_before: &BundlePath,
    target_after: &BundlePath,
) -> Option<String> {
    let trimmed_start = authored.len() - authored.trim_start().len();
    let trimmed_end = authored.trim_end().len();
    let trimmed = &authored[trimmed_start..trimmed_end];
    let (opening, href, closing) = trimmed
        .strip_prefix('<')
        .and_then(|href| href.strip_suffix('>').map(|href| ("<", href, ">")))
        .unwrap_or(("", trimmed, ""));
    let suffix_start = href.find(['?', '#']).unwrap_or(href.len());
    let (path, suffix) = href.split_at(suffix_start);
    if path.is_empty()
        || crate::okf::resolve_href(referrer_before.as_str(), path)
            != target_before.concept_id().expect("classifier path")
    {
        return None;
    }

    let mut destination = relative_href(referrer_after, target_after);
    if path.contains('\\') && !path.contains('/') {
        destination = destination.replace('/', "\\");
    }
    if !path.starts_with("./")
        && !path.starts_with(".\\")
        && !path.starts_with("../")
        && !path.starts_with("..\\")
    {
        destination = destination
            .strip_prefix("./")
            .unwrap_or(&destination)
            .to_owned();
    }
    Some(format!(
        "{}{opening}{destination}{suffix}{closing}{}",
        &authored[..trimmed_start],
        &authored[trimmed_end..]
    ))
}

#[cfg(test)]
mod tests {
    use super::super::lower::slug_of;
    use crate::ops::{apply, Op};

    fn bundle() -> Vec<(String, String)> {
        vec![
            // the doc being renamed
            ("shop/order-line.md".to_string(),
             "---\ntype: uml.Class\ntitle: OrderLine\n---\n# OrderLine\n".to_string()),
            // a referrer: rel target + attribute type-ref + as-ref name link
            ("shop/order.md".to_string(),
             "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- first: [OrderLine](./order-line.md)\n\n## Relationships\n- composes [OrderLine](./order-line.md) as [OrderLine](./order-line.md): 1 to 1..* lines\n".to_string()),
            // a diagram referrer: member link
            ("shop/diagram.md".to_string(),
             "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Members\n- [OrderLine](./order-line.md)\n".to_string()),
        ]
    }

    #[test]
    fn rename_rewrites_every_referrer_and_rekeys_the_file() {
        let out = apply(
            &bundle(),
            &[Op::NodeRename {
                from: "order-line".into(),
                to: "line-item".into(),
            }],
        )
        .unwrap();

        // file re-keyed, directory preserved
        assert!(out.iter().any(|(p, _)| p == "shop/line-item.md"));
        assert!(out.iter().all(|(p, _)| slug_of(p) != "order-line"));

        let order = &out.iter().find(|(p, _)| p == "shop/order.md").unwrap().1;
        assert!(order.contains("(./line-item.md)"), "links repointed");
        assert!(!order.contains("(./order-line.md)"), "no stale link left");
        assert!(order.contains("[OrderLine]"), "titles preserved");

        let diagram = &out.iter().find(|(p, _)| p == "shop/diagram.md").unwrap().1;
        assert!(diagram.contains("(./line-item.md)"), "member repointed");
    }

    #[test]
    fn rename_rewrites_self_references_in_the_renamed_doc_itself() {
        let b = vec![
            // self-referencing doc: attribute type-ref, rel target + name
            ("shop/tree-node.md".to_string(),
             "---\ntype: uml.Class\ntitle: TreeNode\n---\n# TreeNode\n\n## Attributes\n- parent: [TreeNode](./tree-node.md)\n\n## Relationships\n- composes [TreeNode](./tree-node.md) as [TreeNode](./tree-node.md): 1 to 0..* children\n".to_string()),
        ];
        let out = apply(
            &b,
            &[Op::NodeRename {
                from: "tree-node".into(),
                to: "node".into(),
            }],
        )
        .unwrap();

        let doc = &out.iter().find(|(p, _)| p == "shop/node.md").unwrap().1;
        assert!(
            doc.contains("(./node.md)"),
            "self-reference repointed to new slug"
        );
        assert!(
            !doc.contains("(./tree-node.md)"),
            "no stale self-reference left"
        );
        assert!(doc.contains("[TreeNode]"), "title preserved");
    }

    #[test]
    fn rename_refuses_a_slug_collision() {
        let mut b = bundle();
        b.push((
            "shop/line-item.md".to_string(),
            "---\ntype: uml.Class\ntitle: LineItem\n---\n# LineItem\n".to_string(),
        ));
        let err = apply(
            &b,
            &[Op::NodeRename {
                from: "order-line".into(),
                to: "line-item".into(),
            }],
        )
        .unwrap_err();
        assert!(err.reason.contains("already exists"));
    }

    #[test]
    fn rename_rewrites_layout_operand_links() {
        let b = vec![
            ("shop/order.md".to_string(),
             "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
            ("shop/diagram.md".to_string(),
             "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Members\n- [Order](./order.md)\n\n## Layout\n- [Order](./order.md) with collapsed\n".to_string()),
        ];
        let out = apply(
            &b,
            &[Op::NodeRename {
                from: "order".into(),
                to: "invoice".into(),
            }],
        )
        .unwrap();

        let diagram = &out.iter().find(|(p, _)| p == "shop/diagram.md").unwrap().1;
        assert!(
            diagram.contains("## Layout\n- [Order](./invoice.md) with collapsed"),
            "layout link repointed: {diagram}"
        );
        assert!(
            !diagram.contains("(./order.md)"),
            "no stale layout link left: {diagram}"
        );

        let diags = crate::validate::validate(&out);
        assert!(
            diags
                .iter()
                .all(|d| d.code != crate::diagnostic::DiagCode::UnresolvedLayoutRef),
            "renamed bundle must validate cleanly: {diags:?}"
        );
    }

    #[test]
    fn rename_rewrites_bare_layout_operand() {
        let b = vec![
            ("shop/order.md".to_string(),
             "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
            ("shop/customer.md".to_string(),
             "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string()),
            ("shop/diagram.md".to_string(),
             "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Members\n- [Order](./order.md)\n- [Customer](./customer.md)\n\n## Layout\n- order left of customer\n".to_string()),
        ];
        let out = apply(
            &b,
            &[Op::NodeRename {
                from: "order".into(),
                to: "invoice".into(),
            }],
        )
        .unwrap();

        let diagram = &out.iter().find(|(p, _)| p == "shop/diagram.md").unwrap().1;
        assert!(
            diagram.contains("invoice left of customer"),
            "bare layout operand repointed: {diagram}"
        );
        assert!(
            !diagram.contains("order left of"),
            "no stale bare layout operand left: {diagram}"
        );
    }

    #[test]
    fn rename_resolves_from_by_full_path_id_and_still_rewrites_referrers() {
        // `from` addressed as the parse/graph layer's full bundle-path id
        // (`shop/order-line`), not the bare basename `order-line`.
        let out = apply(
            &bundle(),
            &[Op::NodeRename {
                from: "shop/order-line".into(),
                to: "line-item".into(),
            }],
        )
        .unwrap();

        assert!(out.iter().any(|(p, _)| p == "shop/line-item.md"));
        let order = &out.iter().find(|(p, _)| p == "shop/order.md").unwrap().1;
        assert!(
            order.contains("(./line-item.md)"),
            "links repointed when `from` is a full-path id"
        );
        assert!(!order.contains("(./order-line.md)"), "no stale link left");
    }

    #[test]
    fn rename_collision_check_is_scoped_to_the_destination_directory() {
        // A same-basename doc exists in a *different* directory — must not
        // block the rename (full-path keying allows same-basename docs to
        // coexist across directories).
        let mut b = bundle();
        b.push((
            "billing/line-item.md".to_string(),
            "---\ntype: uml.Class\ntitle: LineItem\n---\n# LineItem\n".to_string(),
        ));
        let out = apply(
            &b,
            &[Op::NodeRename {
                from: "order-line".into(),
                to: "line-item".into(),
            }],
        )
        .unwrap();
        assert!(out.iter().any(|(p, _)| p == "shop/line-item.md"));
        assert!(
            out.iter().any(|(p, _)| p == "billing/line-item.md"),
            "unrelated same-basename doc untouched"
        );
    }
}
