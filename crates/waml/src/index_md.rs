use crate::frontmatter::{FmValue, Frontmatter};
use crate::okf::decl::ViewDecl;
use crate::source::{BundlePath, SourceBundle};

/// True if `path`'s final `/`-separated segment is `index.md`,
/// case-insensitively. The one predicate every index-basename check in this
/// crate and `waml-cli` shares.
pub fn is_index_basename(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .unwrap_or(path)
        .eq_ignore_ascii_case("index.md")
}

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

pub(crate) fn render_members(dir: &str, members: &[IndexEntry], newline: &str) -> String {
    let mut out = String::new();
    for entry in members {
        let url = member_url(dir, entry);
        match &entry.blurb {
            Some(blurb) if !blurb.trim().is_empty() => {
                out.push_str(&format!("* [{}]({url}) - {}", entry.title, blurb.trim()));
            }
            _ => out.push_str(&format!("* [{}]({url})", entry.title)),
        }
        out.push_str(newline);
    }
    out
}

/// Declarations rendered into a non-root index's frontmatter block. Every
/// field is `None`/absent by default — a caller with nothing to declare
/// (`IndexFrontmatter::default()`) renders no frontmatter block at all, so
/// today's plain-listing bytes are unchanged.
#[derive(Default)]
pub struct IndexFrontmatter<'a> {
    pub profile: Option<&'a str>,
    pub view: Option<&'a ViewDecl>,
    pub extra: Option<&'a Frontmatter>,
}

/// A one-entry `ViewDecl` renders as a scalar (`view: outline`); a
/// multi-entry chain renders as a flow sequence
/// (`view: [hide-refs, group-by-tag]`), first entry outermost.
fn view_decl_to_fm_value(decl: &ViewDecl) -> FmValue {
    match decl.entries.as_slice() {
        [one] => FmValue::Str(one.raw.clone()),
        entries => FmValue::List(
            entries
                .iter()
                .map(|entry| FmValue::Str(entry.raw.clone()))
                .collect(),
        ),
    }
}

fn render_index_frontmatter(title: Option<&str>, frontmatter: &IndexFrontmatter<'_>) -> String {
    let mut entries: Vec<(String, FmValue)> = Vec::new();
    // `title` enters frontmatter only when some other key is present —
    // a title-only index still renders as a plain H1, matching today's bytes.
    let has_other = frontmatter.profile.is_some()
        || frontmatter.view.is_some()
        || frontmatter
            .extra
            .is_some_and(|extra| !extra.entries.is_empty());
    if !has_other {
        return String::new();
    }
    if let Some(title) = title.map(str::trim).filter(|t| !t.is_empty()) {
        entries.push(("title".into(), FmValue::Str(title.to_owned())));
    }
    if let Some(profile) = frontmatter.profile {
        entries.push(("profile".into(), FmValue::Str(profile.to_owned())));
    }
    if let Some(view) = frontmatter.view {
        entries.push(("view".into(), view_decl_to_fm_value(view)));
    }
    if let Some(extra) = frontmatter.extra {
        entries.extend(extra.entries.iter().cloned());
    }
    let rendered = crate::frontmatter::render_frontmatter(&Frontmatter { entries });
    format!("---\n{rendered}\n---\n")
}

pub fn render_index(
    dir: &str,
    title: Option<&str>,
    description: Option<&str>,
    members: &[IndexEntry],
    frontmatter: &IndexFrontmatter<'_>,
) -> String {
    let fallback = if dir.is_empty() {
        "index"
    } else {
        dir.rsplit('/').next().unwrap_or(dir)
    };
    let fm_block = render_index_frontmatter(title, frontmatter);
    // A custom title (parsed from the existing H1, or set by pkg.retitle) is
    // emitted verbatim; only an absent/blank title falls back to the basename.
    // When the title already moved into frontmatter, the H1 still carries it
    // verbatim too (frontmatter is the parse-time source of truth; the H1
    // stays human-readable and in sync).
    let heading = title
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .unwrap_or(fallback);
    let mut out = fm_block;
    out.push_str(&format!("# {heading}\n"));
    if let Some(d) = description.filter(|d| !d.trim().is_empty()) {
        out.push('\n');
        out.push_str(d.trim());
        out.push('\n');
    }
    if !members.is_empty() {
        out.push('\n');
    }
    out.push_str(&render_members(dir, members, "\n"));
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
                &IndexFrontmatter {
                    profile: index.profile.as_deref(),
                    view: index.view.as_ref(),
                    extra: Some(&index.extra),
                },
            ),
        );
    }
    out.retain_documents(|document| {
        let path = document.path().as_str();
        !is_index_basename(path) || retained_indexes.contains(path)
    });
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexChange {
    Upsert { path: String, rendered: String },
    Remove { path: String },
}

pub fn plan_indexes(files: &[(String, String)]) -> Result<Vec<IndexChange>, String> {
    let mut original_index_paths = std::collections::BTreeMap::new();
    let mut planning_files = Vec::with_capacity(files.len());
    for (path, text) in files {
        let (directory, basename) = path.rsplit_once('/').unwrap_or(("", path));
        let planning_path = if is_index_basename(basename) {
            if directory.is_empty() {
                "index.md".to_string()
            } else {
                format!("{directory}/index.md")
            }
        } else {
            path.clone()
        };
        if is_index_basename(basename) {
            if let Some(existing) = original_index_paths.insert(planning_path.clone(), path.clone())
            {
                if existing != *path {
                    return Err(format!("index basename collision: {existing} and {path}"));
                }
            }
        }
        planning_files.push((planning_path, text.clone()));
    }

    let prepared = crate::validate::prepare(&planning_files)?;
    let before: std::collections::BTreeMap<String, String> =
        prepared.source().to_pairs().into_iter().collect();
    let mut after: std::collections::BTreeMap<String, String> = reindex_source(prepared.source())
        .to_pairs()
        .into_iter()
        .collect();
    let mut desired_index_directories = std::collections::BTreeSet::from([String::new()]);
    for path in before.keys() {
        if is_index_basename(path) {
            continue;
        }
        let mut parent = path.rsplit_once('/').map(|(parent, _)| parent);
        while let Some(directory) = parent {
            desired_index_directories.insert(directory.to_string());
            parent = directory.rsplit_once('/').map(|(parent, _)| parent);
        }
    }
    after.retain(|path, _| {
        let directory = path
            .rsplit_once('/')
            .map(|(directory, _)| directory)
            .unwrap_or("");
        !is_index_basename(path) || desired_index_directories.contains(directory)
    });
    let paths: std::collections::BTreeSet<_> = before.keys().chain(after.keys()).collect();
    let mut changes = Vec::new();

    for path in paths {
        if !is_index_basename(path) {
            continue;
        }
        if before.get(path) == after.get(path) {
            continue;
        }
        match after.get(path) {
            Some(rendered) => changes.push(IndexChange::Upsert {
                path: original_index_paths
                    .get(path)
                    .cloned()
                    .unwrap_or_else(|| path.clone()),
                rendered: rendered.clone(),
            }),
            None => changes.push(IndexChange::Remove {
                path: original_index_paths
                    .get(path)
                    .cloned()
                    .unwrap_or_else(|| path.clone()),
            }),
        }
    }

    Ok(changes)
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
            &IndexFrontmatter::default(),
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
        let out = render_index(
            "sales",
            None,
            Some("Sales bounded context."),
            &members,
            &IndexFrontmatter::default(),
        );
        assert!(out.starts_with("# sales\n"));
        assert!(out.contains("Sales bounded context."));
        assert!(out.contains("* [orders](orders/)"));
        assert!(out.contains("* [Customer](./customer.md) - a buyer"));
        assert!(!out.contains("---")); // frontmatter-less
    }

    #[test]
    fn render_index_emits_a_custom_title_verbatim() {
        let out = render_index(
            "sales",
            Some("Sales Domain"),
            None,
            &[],
            &IndexFrontmatter::default(),
        );
        assert!(
            out.starts_with("# Sales Domain\n"),
            "custom title must be the H1: {out}"
        );
    }

    #[test]
    fn render_index_root_uses_title_over_index_fallback() {
        // Root ("" dir): a Some title wins; None falls back to "index".
        assert!(render_index(
            "",
            Some("My Domain"),
            None,
            &[],
            &IndexFrontmatter::default()
        )
        .starts_with("# My Domain\n"));
        assert!(
            render_index("", None, None, &[], &IndexFrontmatter::default())
                .starts_with("# index\n")
        );
    }

    #[test]
    fn render_index_emits_declared_frontmatter_before_the_heading() {
        let out = render_index(
            "sales",
            Some("Sales"),
            None,
            &[],
            &IndexFrontmatter {
                profile: Some("uml-domain"),
                view: None,
                extra: None,
            },
        );
        assert!(
            out.starts_with("---\ntitle: Sales\nprofile: uml-domain\n---\n# Sales\n"),
            "got: {out}"
        );
    }

    #[test]
    fn render_index_emits_a_chain_as_a_flow_sequence() {
        let view = ViewDecl {
            entries: vec![
                crate::okf::decl::ViewEntry {
                    raw: "hide-refs".into(),
                    line: 0,
                },
                crate::okf::decl::ViewEntry {
                    raw: "group-by-tag".into(),
                    line: 0,
                },
            ],
        };
        let out = render_index(
            "sales",
            None,
            None,
            &[],
            &IndexFrontmatter {
                profile: None,
                view: Some(&view),
                extra: None,
            },
        );
        assert!(
            out.contains("view: [hide-refs, group-by-tag]"),
            "got: {out}"
        );

        let reparsed = crate::okf::Bundle::parse(
            &SourceBundle::try_from_pairs([("sales/index.md", out.as_str())]).unwrap(),
        )
        .unwrap();
        let index = reparsed.index("/sales").unwrap();
        let entries: Vec<&str> = index
            .view
            .as_ref()
            .unwrap()
            .entries
            .iter()
            .map(|e| e.raw.as_str())
            .collect();
        assert_eq!(entries, ["hide-refs", "group-by-tag"]);
    }

    #[test]
    fn render_index_without_declarations_is_byte_identical_to_today() {
        let members = vec![IndexEntry {
            key: "customer".into(),
            title: "Customer".into(),
            blurb: None,
            is_package: false,
        }];
        let with_default = render_index(
            "sales",
            Some("Sales"),
            None,
            &members,
            &IndexFrontmatter::default(),
        );
        let legacy = "# Sales\n\n* [Customer](./customer.md)\n";
        assert_eq!(with_default, legacy);
    }

    #[test]
    fn index_frontmatter_survives_parse_render_reparse() {
        let source = SourceBundle::try_from_pairs([(
            "sales/index.md",
            "---\ntitle: Sales\nprofile: uml-domain\nview: [hide-refs, group-by-tag]\ngenerator: acme\n---\n# Sales\n\n* [Customer](./customer.md)\n",
        ),
        (
            "sales/customer.md",
            "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n",
        )])
        .unwrap();

        let once = reindex_source(&source);
        let twice = reindex_source(&once);

        let parsed_once = crate::okf::Bundle::parse(&once).unwrap();
        let parsed_twice = crate::okf::Bundle::parse(&twice).unwrap();
        let index_once = parsed_once.index("/sales").unwrap();
        let index_twice = parsed_twice.index("/sales").unwrap();
        assert_eq!(index_once.title, index_twice.title);
        assert_eq!(index_once.profile, index_twice.profile);
        assert_eq!(index_once.view, index_twice.view);
        assert_eq!(index_once.extra, index_twice.extra);
        assert_eq!(index_once.members, index_twice.members);
        assert!(
            once.shares_text_with(&twice, "sales/index.md"),
            "second pass must be byte-stable"
        );
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
    fn plan_indexes_keeps_deep_ancestors_and_removes_an_orphan_index() {
        let files = vec![
            (
                "alpha/beta/order.md".to_string(),
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
            ),
            ("orphan/Index.md".to_string(), "# Orphan\n".to_string()),
        ];

        let changes = plan_indexes(&files).unwrap();
        let changes = changes
            .iter()
            .map(|change| match change {
                IndexChange::Upsert { path, .. } => ("upsert", path.as_str()),
                IndexChange::Remove { path } => ("remove", path.as_str()),
            })
            .collect::<Vec<_>>();

        assert_eq!(
            changes,
            [
                ("upsert", "alpha/beta/index.md"),
                ("upsert", "alpha/index.md"),
                ("upsert", "index.md"),
                ("remove", "orphan/Index.md"),
            ]
        );
    }

    #[test]
    fn plan_indexes_rejects_case_colliding_index_paths() {
        let files = vec![
            ("nested/index.md".to_string(), "# Lower\n".to_string()),
            ("nested/Index.md".to_string(), "# Mixed\n".to_string()),
        ];

        let error = plan_indexes(&files).unwrap_err();

        assert!(error.contains("nested/index.md"), "{error}");
        assert!(error.contains("nested/Index.md"), "{error}");
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
