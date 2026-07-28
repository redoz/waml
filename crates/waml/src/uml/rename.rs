use super::lower::{find_doc, slug_of};
use crate::edit::EditError;
use crate::okf;
use crate::source::SourceBundle;
use waml_syntax::{parse_okf_markdown, MarkdownDialect, SourceText, SyntaxElement, SyntaxNode};

/// Swap the basename of `path` to `to.md`, preserving any directory prefix.
fn replace_basename(path: &str, to: &str) -> String {
    match path.rfind(['/', '\\']) {
        Some(i) => format!("{}/{}.md", &path[..i], to),
        None => format!("{to}.md"),
    }
}

pub(crate) fn op_node_rename(
    work: &mut SourceBundle,
    from: &str,
    to: &str,
) -> Result<(), EditError> {
    // `from` may be a full bundle-path id (the parse/graph layer's node key)
    // or a bare basename; `to` is always a bare local name in the renamed
    // doc's own directory. Repointing compares against stored hrefs, which
    // are bare same-directory-relative slugs — resolve `from` down to that
    // form before rewriting referrers.
    let idx = find_doc(work, from, "node.rename")?;
    let source_path = work
        .document_at(idx)
        .expect("resolved document index")
        .path()
        .as_str();
    let from_basename = slug_of(source_path);
    let dest_path = replace_basename(source_path, to);
    let dest_id = okf::id_of(&dest_path);
    if work
        .documents()
        .iter()
        .enumerate()
        .any(|(i, document)| i != idx && okf::id_of(document.path().as_str()) == dest_id)
    {
        return Err(EditError::at(
            "node.rename",
            format!("target slug '{to}' already exists"),
        ));
    }
    for index in 0..work.len() {
        let source = work
            .document_at(index)
            .expect("document index in bounds")
            .text()
            .to_owned();
        let changed = rename_typed_references(&source, &from_basename, to)?;
        if changed != source {
            *work
                .document_at_mut(index)
                .expect("document index in bounds")
                .text_mut() = changed;
        }
    }
    work.rename_document(idx, dest_path)
        .map_err(|error| EditError::at("node.rename", error.to_string()))?;
    Ok(())
}

fn rename_typed_references(source: &str, from: &str, to: &str) -> Result<String, EditError> {
    let text = SourceText::from_shared(std::sync::Arc::new(source.to_owned()))
        .map_err(|error| EditError::at("node.rename", error.to_string()))?;
    let shell = parse_okf_markdown(text.clone(), MarkdownDialect::CommonMarkCurrent)
        .map_err(|error| EditError::at("node.rename", error.to_string()))?;
    let tree = super::syntax::parser::parse(text, &shell.structure);
    let mut edits = Vec::new();
    collect_reference_edits(&tree.root(), source, from, to, &mut edits);
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
    from: &str,
    to: &str,
    edits: &mut Vec<(std::ops::Range<usize>, String)>,
) {
    use super::syntax::UmlSyntaxKind;
    if node.kind() == UmlSyntaxKind::AttributesSection {
        let range = node.range().start().to_usize()..node.range().end().to_usize();
        let authored = &source[range.clone()];
        let needle = format!("](./{from}.md)");
        let replacement = authored.replace(&needle, &format!("](./{to}.md)"));
        if replacement != authored {
            edits.push((range, replacement));
        }
        return;
    }
    if matches!(
        node.kind(),
        UmlSyntaxKind::Link | UmlSyntaxKind::TypeReference
    ) {
        let range = node.range().start().to_usize()..node.range().end().to_usize();
        let authored = &source[range.clone()];
        let needle = format!("./{from}.md");
        let replacement = authored.replace(&needle, &format!("./{to}.md"));
        if replacement != authored {
            edits.push((range, replacement));
        }
        return;
    }
    if node.kind() == UmlSyntaxKind::LayoutStatement {
        let range = node.range().start().to_usize()..node.range().end().to_usize();
        let authored = &source[range.clone()];
        let needle = format!("./{from}.md");
        let mut replacement = authored.replace(&needle, &format!("./{to}.md"));
        let bare = regex::Regex::new(&format!(r"\b{}\b", regex::escape(from)))
            .expect("escaped slug is valid regex");
        replacement = bare.replace_all(&replacement, to).into_owned();
        if replacement != authored {
            edits.push((range, replacement));
        }
        return;
    }
    for child in node.children() {
        match child {
            SyntaxElement::Node(child) => {
                collect_reference_edits(&child, source, from, to, edits);
            }
            SyntaxElement::Token(token)
                if matches!(
                    token.kind(),
                    UmlSyntaxKind::LinkTargetToken | UmlSyntaxKind::LayoutLinkToken
                ) =>
            {
                let range = token.range().start().to_usize()..token.range().end().to_usize();
                let authored = &source[range.clone()];
                let needle = format!("./{from}.md");
                if authored.contains(&needle) {
                    edits.push((range, authored.replace(&needle, &format!("./{to}.md"))));
                }
            }
            SyntaxElement::Token(token) if token.kind() == UmlSyntaxKind::LayoutWordToken => {
                let range = token.range().start().to_usize()..token.range().end().to_usize();
                if source[range.clone()] == *from {
                    edits.push((range, to.to_owned()));
                }
            }
            SyntaxElement::Token(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::slug_of;
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
