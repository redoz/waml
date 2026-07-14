use crate::parse::build_model;

pub struct IndexEntry {
    pub key: String,
    pub title: String,
    pub blurb: Option<String>,
    pub is_package: bool,
}

/// Relative URL for a member, from its containing dir. Sub-packages -> `seg/`,
/// concept docs -> `./slug.md` (dir-relative — `e.key` is a full bundle id,
/// so strip the referring `dir` prefix before writing the href).
fn member_url(dir: &str, e: &IndexEntry) -> String {
    if e.is_package {
        let seg = e.key.rsplit('/').next().unwrap_or(&e.key);
        format!("{seg}/")
    } else {
        let rel = e.key.strip_prefix(dir).and_then(|s| s.strip_prefix('/')).unwrap_or(&e.key);
        format!("./{rel}.md")
    }
}

pub fn render_index(dir: &str, description: Option<&str>, members: &[IndexEntry]) -> String {
    let title = if dir.is_empty() { "index" } else { dir.rsplit('/').next().unwrap_or(dir) };
    let mut out = format!("# {title}\n");
    if let Some(d) = description.filter(|d| !d.trim().is_empty()) {
        out.push('\n');
        out.push_str(d.trim());
        out.push('\n');
    }
    if !members.is_empty() {
        out.push('\n');
    }
    for e in members {
        let url = member_url(dir, e);
        match &e.blurb {
            Some(b) if !b.trim().is_empty() => {
                out.push_str(&format!("* [{}]({url}) - {}\n", e.title, b.trim()))
            }
            _ => out.push_str(&format!("* [{}]({url})\n", e.title)),
        }
    }
    out
}

/// Rebuild every directory's index.md from the current model's package forest.
/// Title/description now live on `concept` (single source); read them there.
pub fn reindex_bundle(bundle: &[(String, String)]) -> Vec<(String, String)> {
    let model = build_model(bundle);
    // key -> (title, is_package, blurb-source description)
    let mut meta = std::collections::HashMap::new();
    for n in &model.nodes {
        meta.insert(
            n.key.clone(),
            (n.concept.title.clone().unwrap_or_else(|| n.key.clone()), false, n.concept.description.clone()),
        );
    }
    for d in &model.diagrams {
        meta.insert(d.key.clone(), (d.title.clone(), false, None));
    }
    for p in &model.packages {
        let title = p.concept.title.clone().unwrap_or_else(|| p.key.clone());
        meta.insert(p.key.clone(), (title, true, None));
    }
    // start from concept/diagram docs (drop existing index.md), then append fresh indexes
    let mut out: Vec<(String, String)> = bundle
        .iter()
        .filter(|(p, _)| !p.rsplit(['/', '\\']).next().unwrap_or(p).eq_ignore_ascii_case("index.md"))
        .cloned()
        .collect();
    for pkg in &model.packages {
        let entries: Vec<IndexEntry> = pkg
            .members
            .iter()
            .filter_map(|k| {
                meta.get(k).map(|(title, is_pkg, desc)| IndexEntry {
                    key: k.clone(),
                    title: title.clone(),
                    is_package: *is_pkg,
                    blurb: desc.as_ref().map(|d| d.lines().next().unwrap_or("").to_string()),
                })
            })
            .collect();
        let path = if pkg.key.is_empty() {
            "index.md".to_string()
        } else {
            format!("{}/index.md", pkg.key)
        };
        out.push((path, render_index(&pkg.key, pkg.concept.description.as_deref(), &entries)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_index_emits_intro_and_listing() {
        let members = vec![
            IndexEntry { key: "sales/orders".into(), title: "orders".into(), blurb: None, is_package: true },
            IndexEntry { key: "customer".into(), title: "Customer".into(), blurb: Some("a buyer".into()), is_package: false },
        ];
        let out = render_index("sales", Some("Sales bounded context."), &members);
        assert!(out.starts_with("# sales\n"));
        assert!(out.contains("Sales bounded context."));
        assert!(out.contains("* [orders](orders/)"));
        assert!(out.contains("* [Customer](./customer.md) - a buyer"));
        assert!(!out.contains("---")); // frontmatter-less
    }

    #[test]
    fn reindex_bundle_creates_index_for_each_directory() {
        let b = vec![
            ("sales/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
            ("sales/orders/line.md".to_string(), "---\ntype: uml.Class\ntitle: Line\n---\n# Line\n".to_string()),
        ];
        let out = reindex_bundle(&b);
        assert!(out.iter().any(|(p, _)| p == "index.md"));
        assert!(out.iter().any(|(p, _)| p == "sales/index.md"));
        assert!(out.iter().any(|(p, _)| p == "sales/orders/index.md"));
        // concept docs untouched
        assert_eq!(out.iter().find(|(p, _)| p == "sales/order.md").unwrap().1,
                   b.iter().find(|(p, _)| p == "sales/order.md").unwrap().1);
    }
}
