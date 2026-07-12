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
}
