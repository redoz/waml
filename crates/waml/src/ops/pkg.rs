use super::{find_doc, Bundle, OpError};
use crate::index_md::{render_index, IndexEntry};
use crate::parse::build_model;

fn join(dir: &str, slug: &str) -> String {
    if dir.is_empty() {
        format!("{slug}.md")
    } else {
        format!("{dir}/{slug}.md")
    }
}

/// Move a concept/diagram doc to another package directory, keeping its
/// basename (key). Slug-based references are unaffected. Errors if the doc is
/// missing or a same-key doc already lives in `to_dir`.
pub(crate) fn op_pkg_move(work: &mut Bundle, slug: &str, to_dir: &str) -> Result<(), OpError> {
    let idx = find_doc(work, slug, "pkg.move")?;
    let dest = join(to_dir, slug);
    if work
        .iter()
        .enumerate()
        .any(|(i, (p, _))| i != idx && *p == dest)
    {
        return Err(OpError::at("pkg.move", format!("'{dest}' already exists")));
    }
    work[idx].0 = dest;
    Ok(())
}

/// Rename a package directory: rewrite the `from/` path prefix of every doc
/// under it to `to/`. Slugs (keys) and slug-based references are unchanged.
/// Errors if `to` already exists as a directory prefix or `from` is empty/absent.
pub(crate) fn op_pkg_rename(work: &mut Bundle, from: &str, to: &str) -> Result<(), OpError> {
    if from.is_empty() {
        return Err(OpError::at("pkg.rename", "cannot rename the root package"));
    }
    let from_pfx = format!("{from}/");
    let to_pfx = format!("{to}/");
    if work
        .iter()
        .any(|(p, _)| p.replace('\\', "/").starts_with(&to_pfx))
    {
        return Err(OpError::at(
            "pkg.rename",
            format!("directory '{to}' already exists"),
        ));
    }
    let mut hit = false;
    for (p, _) in work.iter_mut() {
        let norm = p.replace('\\', "/");
        if let Some(rest) = norm.strip_prefix(&from_pfx) {
            *p = format!("{to_pfx}{rest}");
            hit = true;
        }
    }
    if !hit {
        return Err(OpError::at("pkg.rename", format!("no package '{from}'")));
    }
    Ok(())
}

fn parent_of(dir: &str) -> String {
    match dir.rfind('/') {
        Some(i) => dir[..i].to_string(),
        None => String::new(),
    }
}

/// Delete a package directory. `cascade=true` removes every doc under `path/`
/// (incl. its `index.md`). `cascade=false` = move-to-parent: strip the deleted
/// segment from every child path so children reparent one level up. Root cannot
/// be deleted.
pub(crate) fn op_pkg_delete(work: &mut Bundle, path: &str, cascade: bool) -> Result<(), OpError> {
    if path.is_empty() {
        return Err(OpError::at("pkg.delete", "cannot delete the root package"));
    }
    let pfx = format!("{path}/");
    if cascade {
        let before = work.len();
        work.retain(|(p, _)| !p.replace('\\', "/").starts_with(&pfx));
        if work.len() == before {
            return Err(OpError::at("pkg.delete", format!("no package '{path}'")));
        }
    } else {
        let parent = parent_of(path);
        let parent_pfx = if parent.is_empty() {
            String::new()
        } else {
            format!("{parent}/")
        };
        for (p, _) in work.iter_mut() {
            let norm = p.replace('\\', "/");
            if let Some(rest) = norm.strip_prefix(&pfx) {
                // strip only the deleted segment, keep any deeper nesting
                *p = format!("{parent_pfx}{rest}");
            }
        }
    }
    Ok(())
}

/// `label` is the single source for a node/package's display title (Concept is
/// off the object-model `Node`, spec §2). Look up a member's display title
/// across nodes, diagrams, and sub-packages.
fn member_title(model: &crate::model::Model, k: &str) -> String {
    model
        .nodes
        .iter()
        .find(|n| n.key == k)
        .map(|n| n.label.clone())
        .or_else(|| {
            model
                .diagrams
                .iter()
                .find(|d| d.key == k)
                .map(|d| d.title.clone())
        })
        .or_else(|| {
            model
                .packages
                .iter()
                .find(|p| p.key == k)
                .map(|p| p.label.clone())
        })
        .unwrap_or_else(|| k.to_string())
}

/// How a rewritten index.md orders its members. `Sort` = A–Z by title; `Explicit`
/// = a caller-supplied order (unknown keys ignored, missing keys appended).
enum MemberOrder<'a> {
    Explicit(&'a [String]),
    Sort,
    /// Keep the package's current (reconciled) member order — used by retitle,
    /// which must not reshuffle the listing.
    Keep,
}

/// Write/replace `<path>/index.md` (root → `index.md`) with a listing in the
/// requested order, preserving intro prose + blurbs. The H1 title comes from
/// `title_override` when set, else the package's current title (root →
/// `model.path`, else `concept.title`), else the dir basename.
fn write_package_index(
    work: &mut Bundle,
    path: &str,
    order: MemberOrder<'_>,
    title_override: Option<&str>,
) -> Result<(), OpError> {
    let model = build_model(work);
    let pkg = model
        .packages
        .iter()
        .find(|p| p.key == path)
        .ok_or_else(|| OpError::at("pkg.index", format!("no package '{path}'")))?;
    // desired order
    let mut keys: Vec<String> = match order {
        MemberOrder::Explicit(o) => {
            let mut v: Vec<String> = o
                .iter()
                .filter(|k| pkg.members().contains(k))
                .cloned()
                .collect();
            for m in pkg.members() {
                if !v.contains(m) {
                    v.push(m.clone());
                }
            }
            v
        }
        MemberOrder::Sort => {
            let mut v = pkg.members().to_vec();
            v.sort_by_key(|k| member_title(&model, k).to_lowercase());
            v
        }
        MemberOrder::Keep => pkg.members().to_vec(),
    };
    let entries: Vec<IndexEntry> = keys
        .drain(..)
        .map(|k| {
            let (title, is_pkg, blurb) = model
                .nodes
                .iter()
                .find(|n| n.key == k)
                .map(|n| {
                    (
                        n.label.clone(),
                        false,
                        model
                            .concept(&k)
                            .and_then(|c| c.description.as_ref())
                            .map(|d| d.lines().next().unwrap_or("").to_string()),
                    )
                })
                .or_else(|| {
                    model
                        .diagrams
                        .iter()
                        .find(|d| d.key == k)
                        .map(|d| (d.title.clone(), false, None))
                })
                .or_else(|| {
                    model
                        .packages
                        .iter()
                        .find(|p| p.key == k)
                        .map(|p| (p.label.clone(), true, None))
                })
                .unwrap_or((k.clone(), false, None));
            IndexEntry {
                key: k,
                title,
                is_package: is_pkg,
                blurb,
            }
        })
        .collect();
    // Current title: root's name lives on model.path (the root index.md H1);
    // other packages carry it on `label`. An explicit override wins.
    let current_title = if path.is_empty() {
        (!model.path.is_empty()).then(|| model.path.clone())
    } else {
        Some(pkg.label.clone())
    };
    let title_for_index = title_override.map(str::to_string).or(current_title);
    let description = model.concept(path).and_then(|c| c.description.as_deref());
    let text = render_index(path, title_for_index.as_deref(), description, &entries);
    // Root special-case is ONLY the index-file path arithmetic.
    let idx_path = if path.is_empty() {
        "index.md".to_string()
    } else {
        format!("{path}/index.md")
    };
    match work.iter_mut().find(|(p, _)| *p == idx_path) {
        Some(slot) => slot.1 = text,
        None => work.push((idx_path, text)),
    }
    Ok(())
}

pub(crate) fn op_pkg_reorder(
    work: &mut Bundle,
    path: &str,
    order: &[String],
) -> Result<(), OpError> {
    write_package_index(work, path, MemberOrder::Explicit(order), None)
}
pub(crate) fn op_pkg_sort(work: &mut Bundle, path: &str) -> Result<(), OpError> {
    write_package_index(work, path, MemberOrder::Sort, None)
}

/// Set a package's display title by writing its index.md H1, creating the file
/// (root → `index.md`, else `<path>/index.md`) when absent. Preserves the intro
/// prose and member listing. Empty/whitespace titles are rejected. Generic over
/// any package key; root ("") is just one instance.
pub(crate) fn op_pkg_retitle(work: &mut Bundle, path: &str, title: &str) -> Result<(), OpError> {
    if title.trim().is_empty() {
        return Err(OpError::at("pkg.retitle", "title cannot be empty"));
    }
    write_package_index(work, path, MemberOrder::Keep, Some(title))
}

/// Insert a package: re-root every doc in `docs` under `<parent_path>/<name>/`
/// (or `<name>/` at root) and append. The incoming top-level folder segment is
/// stripped so a template's baked folder is replaced by the target prefix;
/// `./`-relative links stay valid untouched. Identity is the full path, so
/// distinct same-basename docs across packages coexist. Errors if the target
/// package path already exists or `name` is empty.
pub(crate) fn op_pkg_insert(
    work: &mut Bundle,
    parent_path: &str,
    name: &str,
    docs: &[(String, String)],
) -> Result<(), OpError> {
    if name.is_empty() {
        return Err(OpError::at("pkg.insert", "package name is required"));
    }
    let prefix = if parent_path.is_empty() {
        format!("{name}/")
    } else {
        format!("{parent_path}/{name}/")
    };
    if work
        .iter()
        .any(|(p, _)| p.replace('\\', "/").starts_with(&prefix))
    {
        return Err(OpError::at(
            "pkg.insert",
            format!("package '{}' already exists", prefix.trim_end_matches('/')),
        ));
    }
    for (path, text) in docs {
        let norm = path.replace('\\', "/");
        // strip the incoming top-level folder segment (if any)
        let rest = match norm.split_once('/') {
            Some((_, r)) => r,
            None => norm.as_str(),
        };
        work.push((format!("{prefix}{rest}"), text.clone()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::ops::{apply, Op};
    #[test]
    fn move_changes_directory_keeps_basename() {
        let b = vec![(
            "sales/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
        )];
        let out = apply(
            &b,
            &[Op::PkgMove {
                slug: "order".into(),
                to_dir: "billing".into(),
            }],
        )
        .unwrap();
        assert!(out.iter().any(|(p, _)| p == "billing/order.md"));
        assert!(out.iter().all(|(p, _)| p != "sales/order.md"));
    }
    #[test]
    fn move_to_root_uses_bare_filename() {
        let b = vec![(
            "sales/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
        )];
        let out = apply(
            &b,
            &[Op::PkgMove {
                slug: "order".into(),
                to_dir: "".into(),
            }],
        )
        .unwrap();
        assert!(out.iter().any(|(p, _)| p == "order.md"));
    }

    #[test]
    fn rename_package_rewrites_child_paths_only() {
        let b = vec![
            ("sales/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- depends [Customer](./customer.md)\n".to_string()),
            ("sales/customer.md".to_string(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string()),
        ];
        let out = apply(
            &b,
            &[Op::PkgRename {
                from: "sales".into(),
                to: "commerce".into(),
            }],
        )
        .unwrap();
        assert!(out.iter().any(|(p, _)| p == "commerce/order.md"));
        assert!(out.iter().any(|(p, _)| p == "commerce/customer.md"));
        assert!(out.iter().all(|(p, _)| !p.starts_with("sales/")));
        // slug-based references untouched
        let order = &out
            .iter()
            .find(|(p, _)| p == "commerce/order.md")
            .unwrap()
            .1;
        assert!(order.contains("(./customer.md)"));
    }

    #[test]
    fn delete_package_cascade_removes_subtree() {
        let b = vec![
            (
                "sales/order.md".to_string(),
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
            ),
            (
                "sales/orders/line.md".to_string(),
                "---\ntype: uml.Class\ntitle: Line\n---\n# Line\n".to_string(),
            ),
            (
                "billing/invoice.md".to_string(),
                "---\ntype: uml.Class\ntitle: Invoice\n---\n# Invoice\n".to_string(),
            ),
        ];
        let out = apply(
            &b,
            &[Op::PkgDelete {
                path: "sales".into(),
                cascade: true,
            }],
        )
        .unwrap();
        assert!(out.iter().all(|(p, _)| !p.starts_with("sales")));
        assert!(out.iter().any(|(p, _)| p == "billing/invoice.md"));
    }
    #[test]
    fn delete_package_reparent_moves_children_up() {
        let b = vec![(
            "sales/orders/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
        )];
        let out = apply(
            &b,
            &[Op::PkgDelete {
                path: "sales/orders".into(),
                cascade: false,
            }],
        )
        .unwrap();
        assert!(out.iter().any(|(p, _)| p == "sales/order.md"));
        assert!(out.iter().all(|(p, _)| !p.contains("orders")));
    }

    #[test]
    fn reorder_writes_index_md_in_requested_order() {
        let b = vec![
            (
                "sales/order.md".to_string(),
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
            ),
            (
                "sales/customer.md".to_string(),
                "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string(),
            ),
        ];
        let out = apply(
            &b,
            &[Op::PkgReorder {
                path: "sales".into(),
                order: vec!["sales/order".into(), "sales/customer".into()],
            }],
        )
        .unwrap();
        let idx = &out.iter().find(|(p, _)| p == "sales/index.md").unwrap().1;
        let oi = idx.find("order.md").unwrap();
        let ci = idx.find("customer.md").unwrap();
        assert!(oi < ci, "order must precede customer in index.md");
    }
    #[test]
    fn sort_writes_index_md_alphabetically() {
        let b = vec![
            (
                "sales/order.md".to_string(),
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
            ),
            (
                "sales/customer.md".to_string(),
                "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string(),
            ),
        ];
        let out = apply(
            &b,
            &[Op::PkgSort {
                path: "sales".into(),
            }],
        )
        .unwrap();
        let idx = &out.iter().find(|(p, _)| p == "sales/index.md").unwrap().1;
        assert!(idx.find("customer.md").unwrap() < idx.find("order.md").unwrap());
    }

    #[test]
    fn retitle_creates_root_index_when_absent() {
        let b = vec![(
            "order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
        )];
        let out = apply(
            &b,
            &[Op::PkgRetitle {
                path: "".into(),
                title: "Acme".into(),
            }],
        )
        .unwrap();
        let idx = &out
            .iter()
            .find(|(p, _)| p == "index.md")
            .expect("root index.md created")
            .1;
        assert!(idx.starts_with("# Acme\n"), "root H1: {idx}");
        assert!(
            idx.contains("./order.md"),
            "member listing preserved: {idx}"
        );
    }

    #[test]
    fn retitle_preserves_intro_and_members_for_a_nested_package() {
        let b = vec![
            (
                "sales/index.md".to_string(),
                "# Old\n\nIntro prose.\n\n* [order](./order.md)\n".to_string(),
            ),
            (
                "sales/order.md".to_string(),
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
            ),
            (
                "sales/customer.md".to_string(),
                "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string(),
            ),
        ];
        let out = apply(
            &b,
            &[Op::PkgRetitle {
                path: "sales".into(),
                title: "Sales Domain".into(),
            }],
        )
        .unwrap();
        let idx = &out.iter().find(|(p, _)| p == "sales/index.md").unwrap().1;
        assert!(idx.starts_with("# Sales Domain\n"), "new H1: {idx}");
        assert!(idx.contains("Intro prose."), "intro preserved: {idx}");
        assert!(
            idx.contains("./order.md") && idx.contains("./customer.md"),
            "members preserved: {idx}"
        );
    }

    #[test]
    fn retitle_rejects_an_empty_title() {
        let b = vec![(
            "order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
        )];
        let err = apply(
            &b,
            &[Op::PkgRetitle {
                path: "".into(),
                title: "   ".into(),
            }],
        )
        .unwrap_err();
        assert_eq!(err.op, "pkg.retitle");
        assert!(err.reason.contains("empty"), "reason: {}", err.reason);
    }

    #[test]
    fn insert_reroots_docs_under_parent_and_name() {
        let b: crate::ops::Bundle = vec![];
        let docs = vec![
            (
                "orders-domain-uml/order.md".to_string(),
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
            ),
            (
                "orders-domain-uml/customer.md".to_string(),
                "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string(),
            ),
        ];
        let out = apply(
            &b,
            &[Op::PkgInsert {
                parent_path: "sales".into(),
                name: "orders".into(),
                docs,
            }],
        )
        .unwrap();
        assert!(
            out.iter().any(|(p, _)| p == "sales/orders/order.md"),
            "{out:?}"
        );
        assert!(
            out.iter().any(|(p, _)| p == "sales/orders/customer.md"),
            "{out:?}"
        );
        assert!(
            out.iter()
                .all(|(p, _)| !p.starts_with("orders-domain-uml/")),
            "top folder stripped: {out:?}"
        );
    }

    #[test]
    fn insert_at_root_uses_name_as_top_segment() {
        let b: crate::ops::Bundle = vec![];
        let docs = vec![(
            "tmpl/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
        )];
        let out = apply(
            &b,
            &[Op::PkgInsert {
                parent_path: "".into(),
                name: "orders".into(),
                docs,
            }],
        )
        .unwrap();
        assert!(out.iter().any(|(p, _)| p == "orders/order.md"), "{out:?}");
    }

    #[test]
    fn insert_preserves_same_directory_relative_links() {
        let b: crate::ops::Bundle = vec![];
        let docs = vec![
            ("t/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- depends [Customer](./customer.md)\n".to_string()),
            ("t/customer.md".to_string(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string()),
        ];
        let out = apply(
            &b,
            &[Op::PkgInsert {
                parent_path: "".into(),
                name: "orders".into(),
                docs,
            }],
        )
        .unwrap();
        let order = &out.iter().find(|(p, _)| p == "orders/order.md").unwrap().1;
        assert!(
            order.contains("(./customer.md)"),
            "relative link untouched: {order}"
        );
    }

    #[test]
    fn insert_keeps_distinct_same_basename_docs_across_packages() {
        // The old TS mergeBundles bug: a same-basename doc in a different package
        // must NOT be dropped. Full-path identity keeps both.
        let b: crate::ops::Bundle = vec![(
            "billing/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Invoice Order\n---\n# Invoice Order\n".to_string(),
        )];
        let docs = vec![(
            "t/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Sales Order\n---\n# Sales Order\n".to_string(),
        )];
        let out = apply(
            &b,
            &[Op::PkgInsert {
                parent_path: "".into(),
                name: "sales".into(),
                docs,
            }],
        )
        .unwrap();
        assert!(
            out.iter().any(|(p, _)| p == "billing/order.md"),
            "existing kept: {out:?}"
        );
        assert!(
            out.iter().any(|(p, _)| p == "sales/order.md"),
            "inserted kept: {out:?}"
        );
        assert_eq!(out.len(), 2, "neither dropped: {out:?}");
    }

    #[test]
    fn insert_errors_when_target_package_already_exists() {
        let b: crate::ops::Bundle = vec![(
            "sales/orders/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
        )];
        let docs = vec![(
            "t/thing.md".to_string(),
            "---\ntype: uml.Class\ntitle: Thing\n---\n# Thing\n".to_string(),
        )];
        let err = apply(
            &b,
            &[Op::PkgInsert {
                parent_path: "sales".into(),
                name: "orders".into(),
                docs,
            }],
        )
        .unwrap_err();
        assert_eq!(err.op, "pkg.insert");
        assert!(err.reason.contains("already exists"), "got: {}", err.reason);
    }

    #[test]
    fn insert_errors_on_empty_name() {
        let b: crate::ops::Bundle = vec![];
        let docs = vec![(
            "t/x.md".to_string(),
            "---\ntype: uml.Class\ntitle: X\n---\n# X\n".to_string(),
        )];
        let err = apply(
            &b,
            &[Op::PkgInsert {
                parent_path: "".into(),
                name: "".into(),
                docs,
            }],
        )
        .unwrap_err();
        assert!(err.reason.contains("name"), "got: {}", err.reason);
    }
}
