use super::{find_doc, Bundle, OpError};
use crate::index_md::{render_index, IndexEntry};
use crate::parse::build_model;

fn join(dir: &str, slug: &str) -> String {
    if dir.is_empty() { format!("{slug}.md") } else { format!("{dir}/{slug}.md") }
}

/// Move a concept/diagram doc to another package directory, keeping its
/// basename (key). Slug-based references are unaffected. Errors if the doc is
/// missing or a same-key doc already lives in `to_dir`.
pub(crate) fn op_pkg_move(work: &mut Bundle, slug: &str, to_dir: &str) -> Result<(), OpError> {
    let idx = find_doc(work, slug, "pkg.move")?;
    let dest = join(to_dir, slug);
    if work.iter().enumerate().any(|(i, (p, _))| i != idx && *p == dest) {
        return Err(OpError::at("pkg.move", format!("'{dest}' already exists")));
    }
    work[idx].0 = dest;
    Ok(())
}

/// Rename a package directory: rewrite the `from/` path prefix of every doc
/// under it to `to/`. Slugs (keys) and slug-based references are unchanged.
/// Errors if `to` already exists as a directory prefix or `from` is empty/absent.
pub(crate) fn op_pkg_rename(work: &mut Bundle, from: &str, to: &str) -> Result<(), OpError> {
    if from.is_empty() { return Err(OpError::at("pkg.rename", "cannot rename the root package")); }
    let from_pfx = format!("{from}/");
    let to_pfx = format!("{to}/");
    if work.iter().any(|(p, _)| p.replace('\\', "/").starts_with(&to_pfx)) {
        return Err(OpError::at("pkg.rename", format!("directory '{to}' already exists")));
    }
    let mut hit = false;
    for (p, _) in work.iter_mut() {
        let norm = p.replace('\\', "/");
        if let Some(rest) = norm.strip_prefix(&from_pfx) {
            *p = format!("{to_pfx}{rest}");
            hit = true;
        }
    }
    if !hit { return Err(OpError::at("pkg.rename", format!("no package '{from}'"))); }
    Ok(())
}

fn parent_of(dir: &str) -> String {
    match dir.rfind('/') { Some(i) => dir[..i].to_string(), None => String::new() }
}

/// Delete a package directory. `cascade=true` removes every doc under `path/`
/// (incl. its `index.md`). `cascade=false` = move-to-parent: strip the deleted
/// segment from every child path so children reparent one level up. Root cannot
/// be deleted.
pub(crate) fn op_pkg_delete(work: &mut Bundle, path: &str, cascade: bool) -> Result<(), OpError> {
    if path.is_empty() { return Err(OpError::at("pkg.delete", "cannot delete the root package")); }
    let pfx = format!("{path}/");
    if cascade {
        let before = work.len();
        work.retain(|(p, _)| !p.replace('\\', "/").starts_with(&pfx));
        if work.len() == before { return Err(OpError::at("pkg.delete", format!("no package '{path}'"))); }
    } else {
        let parent = parent_of(path);
        let parent_pfx = if parent.is_empty() { String::new() } else { format!("{parent}/") };
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

/// Title/description now live on `concept` (single source). Look up a member's
/// display title across nodes, diagrams, and sub-packages.
fn member_title(model: &crate::model::Model, k: &str) -> String {
    model.nodes.iter().find(|n| n.key == k).and_then(|n| n.concept.title.clone())
        .or_else(|| model.diagrams.iter().find(|d| d.key == k).map(|d| d.title.clone()))
        .or_else(|| model.packages.iter().find(|p| p.key == k).and_then(|p| p.concept.title.clone()))
        .unwrap_or_else(|| k.to_string())
}

/// Write/replace `<path>/index.md` with a listing in the requested (or A–Z)
/// order, preserving intro prose + blurbs. `order` keys not in the package are
/// ignored; members missing from `order` are appended in existing order.
fn write_package_index(work: &mut Bundle, path: &str, order: Option<&[String]>) -> Result<(), OpError> {
    let model = build_model(work);
    let pkg = model.packages.iter().find(|p| p.key == path)
        .ok_or_else(|| OpError::at("pkg.order", format!("no package '{path}'")))?;
    // desired order
    let mut keys: Vec<String> = match order {
        Some(o) => {
            let mut v: Vec<String> = o.iter().filter(|k| pkg.members.contains(k)).cloned().collect();
            for m in &pkg.members { if !v.contains(m) { v.push(m.clone()); } }
            v
        }
        None => {
            let mut v = pkg.members.clone();
            v.sort_by_key(|k| member_title(&model, k).to_lowercase());
            v
        }
    };
    let entries: Vec<IndexEntry> = keys.drain(..).map(|k| {
        let (title, is_pkg, blurb) = model.nodes.iter().find(|n| n.key == k)
            .map(|n| (
                n.concept.title.clone().unwrap_or_else(|| k.clone()),
                false,
                n.concept.description.as_ref().map(|d| d.lines().next().unwrap_or("").to_string()),
            ))
            .or_else(|| model.diagrams.iter().find(|d| d.key == k).map(|d| (d.title.clone(), false, None)))
            .or_else(|| model.packages.iter().find(|p| p.key == k)
                .map(|p| (p.concept.title.clone().unwrap_or_else(|| k.clone()), true, None)))
            .unwrap_or((k.clone(), false, None));
        IndexEntry { key: k, title, is_package: is_pkg, blurb }
    }).collect();
    let text = render_index(path, pkg.concept.description.as_deref(), &entries);
    let idx_path = format!("{path}/index.md");
    match work.iter_mut().find(|(p, _)| *p == idx_path) {
        Some(slot) => slot.1 = text,
        None => work.push((idx_path, text)),
    }
    Ok(())
}

pub(crate) fn op_pkg_reorder(work: &mut Bundle, path: &str, order: &[String]) -> Result<(), OpError> {
    write_package_index(work, path, Some(order))
}
pub(crate) fn op_pkg_sort(work: &mut Bundle, path: &str) -> Result<(), OpError> {
    write_package_index(work, path, None)
}

#[cfg(test)]
mod tests {
    use crate::ops::{apply, Op};
    #[test]
    fn move_changes_directory_keeps_basename() {
        let b = vec![("sales/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string())];
        let out = apply(&b, &[Op::PkgMove { slug: "order".into(), to_dir: "billing".into() }]).unwrap();
        assert!(out.iter().any(|(p, _)| p == "billing/order.md"));
        assert!(out.iter().all(|(p, _)| p != "sales/order.md"));
    }
    #[test]
    fn move_to_root_uses_bare_filename() {
        let b = vec![("sales/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string())];
        let out = apply(&b, &[Op::PkgMove { slug: "order".into(), to_dir: "".into() }]).unwrap();
        assert!(out.iter().any(|(p, _)| p == "order.md"));
    }

    #[test]
    fn rename_package_rewrites_child_paths_only() {
        let b = vec![
            ("sales/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- depends [Customer](./customer.md)\n".to_string()),
            ("sales/customer.md".to_string(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string()),
        ];
        let out = apply(&b, &[Op::PkgRename { from: "sales".into(), to: "commerce".into() }]).unwrap();
        assert!(out.iter().any(|(p, _)| p == "commerce/order.md"));
        assert!(out.iter().any(|(p, _)| p == "commerce/customer.md"));
        assert!(out.iter().all(|(p, _)| !p.starts_with("sales/")));
        // slug-based references untouched
        let order = &out.iter().find(|(p, _)| p == "commerce/order.md").unwrap().1;
        assert!(order.contains("(./customer.md)"));
    }

    #[test]
    fn delete_package_cascade_removes_subtree() {
        let b = vec![
            ("sales/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
            ("sales/orders/line.md".to_string(), "---\ntype: uml.Class\ntitle: Line\n---\n# Line\n".to_string()),
            ("billing/invoice.md".to_string(), "---\ntype: uml.Class\ntitle: Invoice\n---\n# Invoice\n".to_string()),
        ];
        let out = apply(&b, &[Op::PkgDelete { path: "sales".into(), cascade: true }]).unwrap();
        assert!(out.iter().all(|(p, _)| !p.starts_with("sales")));
        assert!(out.iter().any(|(p, _)| p == "billing/invoice.md"));
    }
    #[test]
    fn delete_package_reparent_moves_children_up() {
        let b = vec![
            ("sales/orders/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
        ];
        let out = apply(&b, &[Op::PkgDelete { path: "sales/orders".into(), cascade: false }]).unwrap();
        assert!(out.iter().any(|(p, _)| p == "sales/order.md"));
        assert!(out.iter().all(|(p, _)| !p.contains("orders")));
    }

    #[test]
    fn reorder_writes_index_md_in_requested_order() {
        let b = vec![
            ("sales/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
            ("sales/customer.md".to_string(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string()),
        ];
        let out = apply(
            &b,
            &[Op::PkgReorder { path: "sales".into(), order: vec!["sales/order".into(), "sales/customer".into()] }],
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
            ("sales/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
            ("sales/customer.md".to_string(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string()),
        ];
        let out = apply(&b, &[Op::PkgSort { path: "sales".into() }]).unwrap();
        let idx = &out.iter().find(|(p, _)| p == "sales/index.md").unwrap().1;
        assert!(idx.find("customer.md").unwrap() < idx.find("order.md").unwrap());
    }
}
