use crate::source::{BundlePath, SourceBundle};

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
pub fn reindex_source(bundle: &SourceBundle) -> SourceBundle {
    let parsed = crate::okf::Bundle::parse(bundle).expect("validated source bundle parses as OKF");
    // Keep existing index allocations until their replacement text is known:
    // `upsert` preserves the Arc when the rendered index is byte-identical.
    let mut out = bundle.clone();
    let mut retained_indexes = std::collections::BTreeSet::new();
    for index in parsed.indexes() {
        let directory = index.directory.as_str().trim_start_matches('/');
        let entries: Vec<IndexEntry> =
            index
                .members
                .iter()
                .filter_map(|member| {
                    if member.starts_with('/') {
                        let child = parsed.index(member)?;
                        Some(IndexEntry {
                            key: member.clone(),
                            title: child.title.clone().unwrap_or_else(|| {
                                member.rsplit('/').next().unwrap_or(member).into()
                            }),
                            is_package: true,
                            blurb: None,
                        })
                    } else {
                        let concept = parsed.concept(member)?;
                        Some(IndexEntry {
                            key: member.clone(),
                            title: concept.title.clone().unwrap_or_else(|| {
                                member.rsplit('/').next().unwrap_or(member).into()
                            }),
                            is_package: false,
                            blurb: concept.description.as_ref().map(|description| {
                                description.lines().next().unwrap_or("").to_string()
                            }),
                        })
                    }
                })
                .collect();
        let path = index.directory.index_path();
        retained_indexes.insert(path.clone());
        out.upsert(
            BundlePath::parse(path).expect("generated index path is valid"),
            render_index(
                directory,
                index.title.as_deref(),
                index.description.as_deref(),
                &entries,
            ),
        );
    }
    out.retain_documents(|document| {
        let path = document.path().as_str();
        let is_index = path
            .rsplit('/')
            .next()
            .unwrap_or(path)
            .eq_ignore_ascii_case("index.md");
        !is_index || retained_indexes.contains(path)
    });
    out
}

#[deprecated(note = "use SourceBundle with reindex_source")]
pub fn reindex_bundle(bundle: &[(String, String)]) -> Vec<(String, String)> {
    let source =
        SourceBundle::try_from_pairs(bundle.iter().cloned()).expect("bundle paths must be valid");
    reindex_source(&source).to_pairs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_reconciled_indexes_keep_their_source_allocation() {
        let index = render_index(
            "",
            Some("Root"),
            None,
            &[IndexEntry {
                key: "order".into(),
                title: "Order".into(),
                is_package: false,
                blurb: None,
            }],
        );
        let source = SourceBundle::try_from_pairs([
            ("index.md", index.as_str()),
            (
                "order.md",
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
            ),
            ("log.md", "# Log\n"),
        ])
        .unwrap();

        let reconciled = reindex_source(&source);

        assert!(source.shares_text_with(&reconciled, "index.md"));
        assert!(source.shares_text_with(&reconciled, "order.md"));
        assert!(source.shares_text_with(&reconciled, "log.md"));
    }

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
        let out = reindex_source(&SourceBundle::try_from_pairs(b).unwrap()).to_pairs();
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
        let out = reindex_source(&SourceBundle::try_from_pairs(b.clone()).unwrap()).to_pairs();
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
