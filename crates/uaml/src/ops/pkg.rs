use super::{find_doc, Bundle, OpError};

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
}
