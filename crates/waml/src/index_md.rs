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
        let rel = e
            .key
            .strip_prefix(dir)
            .and_then(|s| s.strip_prefix('/'))
            .unwrap_or(&e.key);
        format!("./{rel}.md")
    }
}

pub fn render_index(
    dir: &str,
    title: Option<&str>,
    description: Option<&str>,
    members: &[IndexEntry],
) -> String {
    let fallback = if dir.is_empty() {
        "index"
    } else {
        dir.rsplit('/').next().unwrap_or(dir)
    };
    // A custom title (parsed from the existing H1, or set by pkg.retitle) is
    // emitted verbatim; only an absent/blank title falls back to the basename.
    let heading = title
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .unwrap_or(fallback);
    let mut out = format!("# {heading}\n");
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
        let desc = model.concept(&n.key).and_then(|c| c.description.clone());
        meta.insert(n.key.clone(), (n.label.clone(), false, desc));
    }
    for d in &model.diagrams {
        meta.insert(d.key.clone(), (d.title.clone(), false, None));
    }
    for p in &model.packages {
        meta.insert(p.key.clone(), (p.label.clone(), true, None));
    }
    // start from concept/diagram docs (drop existing index.md), then append fresh indexes
    let mut out: Vec<(String, String)> = bundle
        .iter()
        .filter(|(p, _)| {
            !p.rsplit(['/', '\\'])
                .next()
                .unwrap_or(p)
                .eq_ignore_ascii_case("index.md")
        })
        .cloned()
        .collect();
    for pkg in &model.packages {
        let entries: Vec<IndexEntry> = pkg
            .members()
            .iter()
            .filter_map(|k| {
                meta.get(k).map(|(title, is_pkg, desc)| IndexEntry {
                    key: k.clone(),
                    title: title.clone(),
                    is_package: *is_pkg,
                    blurb: desc
                        .as_ref()
                        .map(|d| d.lines().next().unwrap_or("").to_string()),
                })
            })
            .collect();
        let path = if pkg.key.is_empty() {
            "index.md".to_string()
        } else {
            format!("{}/index.md", pkg.key)
        };
        // Root's name is the model path (root index.md H1); nested packages carry
        // it on `label`. Preserve either verbatim instead of resetting to the dir
        // basename.
        let title: Option<&str> = if pkg.key.is_empty() {
            (!model.path.is_empty()).then_some(model.path.as_str())
        } else {
            Some(pkg.label.as_str())
        };
        let description = model
            .concept(&pkg.key)
            .and_then(|c| c.description.as_deref());
        out.push((path, render_index(&pkg.key, title, description, &entries)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_index_emits_intro_and_listing() {
        let members = vec![
            IndexEntry {
                key: "sales/orders".into(),
                title: "orders".into(),
                blurb: None,
                is_package: true,
            },
            IndexEntry {
                key: "customer".into(),
                title: "Customer".into(),
                blurb: Some("a buyer".into()),
                is_package: false,
            },
        ];
        // title None => fall back to the dir basename.
        let out = render_index("sales", None, Some("Sales bounded context."), &members);
        assert!(out.starts_with("# sales\n"));
        assert!(out.contains("Sales bounded context."));
        assert!(out.contains("* [orders](orders/)"));
        assert!(out.contains("* [Customer](./customer.md) - a buyer"));
        assert!(!out.contains("---")); // frontmatter-less
    }

    #[test]
    fn render_index_emits_a_custom_title_verbatim() {
        let out = render_index("sales", Some("Sales Domain"), None, &[]);
        assert!(
            out.starts_with("# Sales Domain\n"),
            "custom title must be the H1: {out}"
        );
    }

    #[test]
    fn render_index_root_uses_title_over_index_fallback() {
        // Root ("" dir): a Some title wins; None falls back to "index".
        assert!(render_index("", Some("My Domain"), None, &[]).starts_with("# My Domain\n"));
        assert!(render_index("", None, None, &[]).starts_with("# index\n"));
    }

    #[test]
    fn reindex_preserves_a_custom_root_index_title() {
        let b = vec![
            (
                "index.md".to_string(),
                "# My Domain\n\n* [Order](./order.md)\n".to_string(),
            ),
            (
                "order.md".to_string(),
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
            ),
        ];
        let out = reindex_bundle(&b);
        let root = &out.iter().find(|(p, _)| p == "index.md").unwrap().1;
        assert!(
            root.starts_with("# My Domain\n"),
            "root H1 must survive reindex, got: {root}"
        );
    }

    #[test]
    fn reindex_bundle_creates_index_for_each_directory() {
        let b = vec![
            (
                "sales/order.md".to_string(),
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
            ),
            (
                "sales/orders/line.md".to_string(),
                "---\ntype: uml.Class\ntitle: Line\n---\n# Line\n".to_string(),
            ),
        ];
        let out = reindex_bundle(&b);
        assert!(out.iter().any(|(p, _)| p == "index.md"));
        assert!(out.iter().any(|(p, _)| p == "sales/index.md"));
        assert!(out.iter().any(|(p, _)| p == "sales/orders/index.md"));
        // concept docs untouched
        assert_eq!(
            out.iter().find(|(p, _)| p == "sales/order.md").unwrap().1,
            b.iter().find(|(p, _)| p == "sales/order.md").unwrap().1
        );
    }
}
