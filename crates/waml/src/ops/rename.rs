use super::{find_doc, slug_of, Bundle, OpError};
use crate::okf;
use crate::parse::parse_document;
use crate::serialize::serialize_document;
use crate::syntax::{Document, Line, NameRef, Operand, OperandRef, ParsedName, Section};

/// Swap the basename of `path` to `to.md`, preserving any directory prefix.
fn replace_basename(path: &str, to: &str) -> String {
    match path.rfind(['/', '\\']) {
        Some(i) => format!("{}/{}.md", &path[..i], to),
        None => format!("{to}.md"),
    }
}

/// Repoint every `from`-slug reference inside one document to `to`. Titles are
/// left untouched. Returns whether anything changed.
fn rename_in_doc(doc: &mut Document, from: &str, to: &str) -> bool {
    let mut changed = false;
    for sec in &mut doc.sections {
        match sec {
            Section::Attributes(attrs) => {
                for a in attrs.iter_mut().filter_map(Line::parsed_mut) {
                    if a.ty.ref_.as_deref() == Some(from) {
                        a.ty.ref_ = Some(to.to_string());
                        changed = true;
                    }
                }
            }
            Section::Relationships(rels) => {
                for r in rels.iter_mut().filter_map(Line::parsed_mut) {
                    if r.target_slug == from {
                        r.target_slug = to.to_string();
                        changed = true;
                    }
                    if let Some(ParsedName::Ref { slug, .. }) = &mut r.name {
                        if slug == from {
                            *slug = to.to_string();
                            changed = true;
                        }
                    }
                }
            }
            Section::Members(block) => {
                fn rename_in_group(
                    g: &mut crate::syntax::MemberGroup,
                    from: &str,
                    to: &str,
                    changed: &mut bool,
                ) {
                    for m in g.members.iter_mut().filter_map(Line::parsed_mut) {
                        match m {
                            crate::syntax::MemberItem::Member(ml) => {
                                if ml.slug == from {
                                    ml.slug = to.to_string();
                                    *changed = true;
                                }
                            }
                            crate::syntax::MemberItem::Instance(inst) => {
                                if inst.classifier.slug == from {
                                    inst.classifier.slug = to.to_string();
                                    *changed = true;
                                }
                            }
                        }
                    }
                    for c in &mut g.children {
                        rename_in_group(c, from, to, changed);
                    }
                }
                for g in &mut block.groups {
                    rename_in_group(g, from, to, &mut changed);
                }
            }
            Section::Layout(stmts) => {
                for it in stmts.iter_mut().filter_map(Line::parsed_mut) {
                    match &mut it.stmt {
                        crate::syntax::LayoutStatement::Standalone(op) => {
                            changed |= rename_in_operand(op, from, to);
                        }
                        crate::syntax::LayoutStatement::Placement { operands, .. } => {
                            for op in operands {
                                changed |= rename_in_operand(op, from, to);
                            }
                        }
                        crate::syntax::LayoutStatement::Alignment { left, right } => {
                            changed |= rename_in_operand(&mut left.operand, from, to);
                            changed |= rename_in_operand(&mut right.operand, from, to);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    changed
}

/// Repoint a `from`-slug reference inside one layout operand to `to`, recursing
/// through inline groups and parens. Returns whether anything changed.
fn rename_in_operand(op: &mut Operand, from: &str, to: &str) -> bool {
    let mut changed = false;
    match &mut op.ref_ {
        OperandRef::Name(NameRef::Link { slug, .. }) => {
            if slug == from {
                *slug = to.to_string();
                changed = true;
            }
        }
        OperandRef::Name(NameRef::Bare(s)) => {
            if s == from {
                *s = to.to_string();
                changed = true;
            }
        }
        OperandRef::InlineGroup { items, .. } => {
            for item in items {
                changed |= rename_in_operand(item, from, to);
            }
        }
        OperandRef::Paren(inner) => {
            changed |= rename_in_operand(inner, from, to);
        }
    }
    changed
}

pub(crate) fn op_node_rename(work: &mut Bundle, from: &str, to: &str) -> Result<(), OpError> {
    // `from` may be a full bundle-path id (the parse/graph layer's node key)
    // or a bare basename; `to` is always a bare local name in the renamed
    // doc's own directory. Repointing compares against stored hrefs, which
    // are bare same-directory-relative slugs — resolve `from` down to that
    // form before rewriting referrers.
    let idx = find_doc(work, from, "node.rename")?;
    let from_basename = slug_of(&work[idx].0);
    let dest_path = replace_basename(&work[idx].0, to);
    let dest_id = okf::id_of(&dest_path);
    if work
        .iter()
        .enumerate()
        .any(|(i, (p, _))| i != idx && okf::id_of(p) == dest_id)
    {
        return Err(OpError::at(
            "node.rename",
            format!("target slug '{to}' already exists"),
        ));
    }
    for (_, text) in work.iter_mut() {
        let mut doc = parse_document(text);
        if rename_in_doc(&mut doc, &from_basename, to) {
            *text = serialize_document(&doc);
        }
    }
    work[idx].0 = dest_path;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::ops::{apply, slug_of, Op};

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
