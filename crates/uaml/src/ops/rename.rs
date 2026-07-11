use super::{find_doc, slug_of, Bundle, OpError};
use crate::parse::parse_document;
use crate::serialize::serialize_document;
use crate::syntax::{Document, HintLine, ParsedName, Section};

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
                for a in attrs {
                    if a.ty.ref_.as_deref() == Some(from) {
                        a.ty.ref_ = Some(to.to_string());
                        changed = true;
                    }
                }
            }
            Section::Relationships(rels) => {
                for r in rels {
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
            Section::Members(ms) => {
                for m in ms {
                    if m.slug == from {
                        m.slug = to.to_string();
                        changed = true;
                    }
                }
            }
            Section::RenderHints(hs) => {
                for h in hs {
                    match h {
                        HintLine::Emphasize(list) => {
                            for x in list.iter_mut() {
                                if x == from {
                                    *x = to.to_string();
                                    changed = true;
                                }
                            }
                        }
                        HintLine::Collapse { slug, .. } => {
                            if slug == from {
                                *slug = to.to_string();
                                changed = true;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    changed
}

pub(crate) fn op_node_rename(work: &mut Bundle, from: &str, to: &str) -> Result<(), OpError> {
    let idx = find_doc(work, from, "node.rename")?;
    if work.iter().any(|(p, _)| slug_of(p) == to) {
        return Err(OpError::at("node.rename", format!("target slug '{to}' already exists")));
    }
    for (p, text) in work.iter_mut() {
        if slug_of(p) == from {
            continue; // the renamed doc's own body doesn't reference itself
        }
        let mut doc = parse_document(text);
        if rename_in_doc(&mut doc, from, to) {
            *text = serialize_document(&doc);
        }
    }
    work[idx].0 = replace_basename(&work[idx].0, to);
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
            // a diagram referrer: member + emphasize (bare slug) + collapse (link)
            ("shop/diagram.md".to_string(),
             "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Members\n- [OrderLine](./order-line.md) at 10,20\n\n## Render hints\n- emphasize: order-line, order\n- collapse [OrderLine](./order-line.md)\n".to_string()),
        ]
    }

    #[test]
    fn rename_rewrites_every_referrer_and_rekeys_the_file() {
        let out = apply(&bundle(), &[Op::NodeRename { from: "order-line".into(), to: "line-item".into() }]).unwrap();

        // file re-keyed, directory preserved
        assert!(out.iter().any(|(p, _)| p == "shop/line-item.md"));
        assert!(out.iter().all(|(p, _)| slug_of(p) != "order-line"));

        let order = &out.iter().find(|(p, _)| p == "shop/order.md").unwrap().1;
        assert!(order.contains("(./line-item.md)"), "links repointed");
        assert!(!order.contains("(./order-line.md)"), "no stale link left");
        assert!(order.contains("[OrderLine]"), "titles preserved");

        let diagram = &out.iter().find(|(p, _)| p == "shop/diagram.md").unwrap().1;
        assert!(diagram.contains("(./line-item.md)"), "member + collapse repointed");
        assert!(diagram.contains("emphasize: line-item, order"), "bare-slug emphasize repointed");
    }

    #[test]
    fn rename_refuses_a_slug_collision() {
        let mut b = bundle();
        b.push(("shop/line-item.md".to_string(), "---\ntype: uml.Class\ntitle: LineItem\n---\n# LineItem\n".to_string()));
        let err = apply(&b, &[Op::NodeRename { from: "order-line".into(), to: "line-item".into() }]).unwrap_err();
        assert!(err.reason.contains("already exists"));
    }
}
