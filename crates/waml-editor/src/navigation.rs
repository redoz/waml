pub use crate::view_history::{DocumentKind, DocumentLocator};
use url::Url;
use waml::analysis::DocumentId;
use waml::okf::DirectoryAddress;
use waml_markdown_editor::syntax::{DocumentRevision, TextRange};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NavigationTarget {
    Document {
        concept_id: String,
        fragment: Option<String>,
    },
    Directory {
        address: String,
    },
    ExternalUrl(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenDisposition {
    Preview,
    Persistent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BreadcrumbSegment {
    pub title: String,
    pub target: NavigationTarget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NavigationIntent {
    Resolved {
        target: NavigationTarget,
        disposition: OpenDisposition,
    },
    MarkdownLink {
        current_concept_id: String,
        href: String,
    },
    #[allow(dead_code)]
    // Diagnostic list actions construct this intent in the integration follow-up.
    SourceRange {
        document: DocumentId,
        revision: DocumentRevision,
        range: TextRange,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NavigationError {
    MalformedTarget(String),
    UnsupportedScheme(String),
    EscapesBundle,
    MissingDocument(String),
    MissingDirectory(String),
}

impl NavigationError {
    pub fn status_message(&self) -> String {
        match self {
            NavigationError::MalformedTarget(value) => format!("Invalid link: {value}"),
            NavigationError::UnsupportedScheme(value) => {
                format!("Unsupported link scheme: {value}")
            }
            NavigationError::EscapesBundle => "Link leaves this bundle".into(),
            NavigationError::MissingDocument(value) => {
                format!("Document not found: {value}")
            }
            NavigationError::MissingDirectory(value) => {
                format!("Directory not found: {value}")
            }
        }
    }
}

/// The last `/`-separated segment, or the whole string when there is none.
fn last_segment(value: &str) -> &str {
    value
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(value)
}

/// The authored path from the bundle root down to the directory that declares
/// `concept_id`, then the document itself.
///
/// Read straight off the bundle's own directory tree, NOT off `build_tree`:
/// a tree build now runs the folder-view middleware chain for every directory
/// in the bundle, recursively, and this runs on every document activation.
/// A breadcrumb is the authored location of a file -- the same list in
/// Projected as in Raw, and under any `ChainLimits` -- so there is nothing
/// here for a projection to decide, and a document a chain declines to list
/// still has a place it lives.
pub fn breadcrumb_for(
    okf: &waml::analysis::OkfAnalysis,
    concept_id: &str,
) -> Option<Vec<BreadcrumbSegment>> {
    let bundle = &okf.bundle;
    let concept = bundle.concept(concept_id)?;
    let owner = bundle
        .directories()
        .iter()
        .find(|directory| directory.concepts.iter().any(|id| id == concept_id))?;

    let mut path = directory_chain(bundle, owner.address.as_str())?;
    path.push(BreadcrumbSegment {
        title: concept
            .title
            .clone()
            .unwrap_or_else(|| last_segment(concept_id).to_string()),
        target: NavigationTarget::Document {
            concept_id: concept_id.to_owned(),
            fragment: None,
        },
    });
    Some(path)
}

/// The breadcrumb for a FOLDER tab, whose subject is a directory address
/// rather than a concept id.
///
/// A folder tab is opened by `NavigationTarget::Directory`, and its address is
/// not a concept the bundle can resolve -- so `breadcrumb_for` returns `None`
/// for one and the header came up empty. A folder has an authored location
/// exactly as a document does; the trail simply ENDS at the folder instead of
/// passing through it, which is why the last segment is not dropped here.
pub fn breadcrumb_for_directory(
    okf: &waml::analysis::OkfAnalysis,
    address: &str,
) -> Option<Vec<BreadcrumbSegment>> {
    directory_chain(&okf.bundle, address)
}

/// Root-first list of segments for `address` and every directory above it.
/// `None` if the bundle has no such directory.
///
/// Root-last, then reversed: `Directory::parent` walks upward, and the
/// bundle's parent links are acyclic by construction (lowering builds them
/// from the address hierarchy).
fn directory_chain(bundle: &waml::okf::Bundle, address: &str) -> Option<Vec<BreadcrumbSegment>> {
    let mut upward = vec![bundle.directory(address)?];
    while let Some(parent) = upward
        .last()
        .and_then(|directory| directory.parent.as_ref())
        .and_then(|parent| bundle.directory(parent.as_str()))
    {
        upward.push(parent);
    }
    Some(
        upward
            .iter()
            .rev()
            .map(|directory| {
                let address = directory.address.as_str();
                BreadcrumbSegment {
                    title: bundle
                        .index(address)
                        .and_then(|index| index.title.clone())
                        .unwrap_or_else(|| {
                            if address == "/" {
                                "Untitled".to_string()
                            } else {
                                last_segment(address).to_string()
                            }
                        }),
                    target: NavigationTarget::Directory {
                        address: address.to_string(),
                    },
                }
            })
            .collect(),
    )
}

pub fn resolve_link(
    bundle: &waml::okf::Bundle,
    current_concept_id: &str,
    href: &str,
) -> Result<NavigationTarget, NavigationError> {
    let syntactic_scheme = href.split_once(':').and_then(|(scheme, _)| {
        let mut chars = scheme.chars();
        let starts_with_letter = chars.next().is_some_and(|ch| ch.is_ascii_alphabetic());
        (starts_with_letter
            && chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.')))
        .then_some(scheme)
    });
    if let Some(scheme) = syntactic_scheme {
        let scheme = scheme.to_ascii_lowercase();
        if !matches!(scheme.as_str(), "http" | "https") {
            return Err(NavigationError::UnsupportedScheme(scheme));
        }
        let url =
            Url::parse(href).map_err(|_| NavigationError::MalformedTarget(href.to_owned()))?;
        if url.host_str().is_none() {
            return Err(NavigationError::MalformedTarget(href.to_owned()));
        }
        return Ok(NavigationTarget::ExternalUrl(href.to_owned()));
    }

    if href.is_empty() || href.contains('?') {
        return Err(NavigationError::MalformedTarget(href.to_owned()));
    }
    let (logical_path, fragment) = match href.split_once('#') {
        Some((_path, "")) => {
            return Err(NavigationError::MalformedTarget(href.to_owned()));
        }
        Some((path, fragment)) => (path, Some(fragment.to_owned())),
        None => (href, None),
    };

    let parent = DirectoryAddress::concept_parent(current_concept_id)
        .map_err(|_| NavigationError::MalformedTarget(href.to_owned()))?;
    if logical_path.is_empty() {
        if bundle.concept(current_concept_id).is_none() {
            return Err(NavigationError::MissingDocument(
                current_concept_id.to_owned(),
            ));
        }
        return Ok(NavigationTarget::Document {
            concept_id: current_concept_id.to_owned(),
            fragment,
        });
    }

    let is_directory = logical_path.ends_with('/');
    if is_directory && fragment.is_some() {
        return Err(NavigationError::MalformedTarget(href.to_owned()));
    }
    let normalized_path = if is_directory {
        logical_path.trim_end_matches('/')
    } else {
        logical_path
            .strip_suffix(".md")
            .ok_or_else(|| NavigationError::MalformedTarget(href.to_owned()))?
    };
    if normalized_path.is_empty() && logical_path != "/" {
        return Err(NavigationError::MalformedTarget(href.to_owned()));
    }

    let absolute = logical_path.starts_with('/');
    let mut segments = if absolute || parent.as_str() == "/" {
        Vec::new()
    } else {
        parent
            .as_str()
            .trim_start_matches('/')
            .split('/')
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    let logical_segments = normalized_path.trim_start_matches('/').split('/');
    for segment in logical_segments {
        match segment {
            "" if normalized_path.is_empty() => {}
            "" => return Err(NavigationError::MalformedTarget(href.to_owned())),
            "." => {}
            ".." => {
                if segments.pop().is_none() {
                    return Err(NavigationError::EscapesBundle);
                }
            }
            segment => segments.push(segment.to_owned()),
        }
    }

    if is_directory {
        let address = if segments.is_empty() {
            "/".to_owned()
        } else {
            format!("/{}", segments.join("/"))
        };
        if bundle.directory(&address).is_none() {
            return Err(NavigationError::MissingDirectory(address));
        }
        Ok(NavigationTarget::Directory { address })
    } else {
        let concept_id = segments.join("/");
        if concept_id.is_empty() {
            return Err(NavigationError::MalformedTarget(href.to_owned()));
        }
        if bundle.concept(&concept_id).is_none() {
            return Err(NavigationError::MissingDocument(concept_id));
        }
        Ok(NavigationTarget::Document {
            concept_id,
            fragment,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crumb(title: &str, target: NavigationTarget) -> BreadcrumbSegment {
        BreadcrumbSegment {
            title: title.into(),
            target,
        }
    }

    fn fixture() -> (waml::analysis::OkfAnalysis, waml::uml::Analysis) {
        let source = waml::source::SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Sales](sales/)\n"),
            ("sales/index.md", "# Sales\n\n* [Archive](archive/)\n"),
            (
                "sales/archive/index.md",
                "# Archive\n\n* [Order](order.md)\n",
            ),
            (
                "sales/archive/order.md",
                "---\ntype: uml.Class\ntitle: Purchase Order\n---\n# Order\n",
            ),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let (_, okf, uml, _) = prepared.into_parts();
        (okf, uml)
    }

    fn resolve_fixture() -> waml::okf::Bundle {
        let source = waml::source::SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Sales](sales/)\n"),
            (
                "sales/index.md",
                "# Sales\n\n* [Archive](archive/)\n* [Order](order.md)\n* [Customer](customer.md)\n",
            ),
            ("sales/order.md", "# Order\n"),
            ("sales/customer.md", "# Customer\n"),
            ("sales/archive/index.md", "# Archive\n"),
            ("shared.md", "# Shared\n"),
        ])
        .unwrap();
        waml::okf::Bundle::parse(&source).unwrap()
    }

    fn doc(concept_id: &str, fragment: Option<&str>) -> NavigationTarget {
        NavigationTarget::Document {
            concept_id: concept_id.into(),
            fragment: fragment.map(str::to_owned),
        }
    }

    fn dir(address: &str) -> NavigationTarget {
        NavigationTarget::Directory {
            address: address.into(),
        }
    }

    fn resolved_target(intent: &NavigationIntent) -> Option<&NavigationTarget> {
        match intent {
            NavigationIntent::Resolved { target, .. } => Some(target),
            NavigationIntent::MarkdownLink { .. } | NavigationIntent::SourceRange { .. } => None,
        }
    }

    #[test]
    fn tree_breadcrumb_and_markdown_entry_targets_are_equivalent() {
        let source = waml::source::SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Sales](sales/)\n"),
            (
                "sales/index.md",
                "# Sales\n\n* [Archive](archive/)\n* [Order](order.md)\n* [Customer](customer.md)\n",
            ),
            ("sales/order.md", "# Order\n"),
            ("sales/customer.md", "# Customer\n"),
            ("sales/archive/index.md", "# Archive\n"),
            ("shared.md", "# Shared\n"),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let (_, okf, _uml, _) = prepared.into_parts();
        let breadcrumb =
            breadcrumb_for(&okf, "sales/customer").expect("customer has a canonical breadcrumb");

        let tree_document = NavigationIntent::Resolved {
            target: doc("sales/customer", None),
            disposition: OpenDisposition::Preview,
        };
        let breadcrumb_document = NavigationIntent::Resolved {
            target: breadcrumb
                .last()
                .expect("breadcrumb ends at the document")
                .target
                .clone(),
            disposition: OpenDisposition::Preview,
        };
        let markdown_document = NavigationIntent::Resolved {
            target: resolve_link(&okf.bundle, "sales/order", "./customer.md")
                .expect("relative document resolves"),
            disposition: OpenDisposition::Preview,
        };
        assert_eq!(
            resolved_target(&tree_document),
            resolved_target(&breadcrumb_document)
        );
        assert_eq!(
            resolved_target(&tree_document),
            resolved_target(&markdown_document)
        );

        let tree_directory = NavigationIntent::Resolved {
            target: dir("/sales"),
            disposition: OpenDisposition::Preview,
        };
        let breadcrumb_directory = NavigationIntent::Resolved {
            target: breadcrumb
                .iter()
                .find(|segment| segment.title == "Sales")
                .expect("breadcrumb contains Sales")
                .target
                .clone(),
            disposition: OpenDisposition::Preview,
        };
        let markdown_directory = NavigationIntent::Resolved {
            target: resolve_link(&okf.bundle, "sales/order", "/sales/")
                .expect("logical directory resolves"),
            disposition: OpenDisposition::Preview,
        };
        assert_eq!(
            resolved_target(&tree_directory),
            resolved_target(&breadcrumb_directory)
        );
        assert_eq!(
            resolved_target(&tree_directory),
            resolved_target(&markdown_directory)
        );

        let persistent_tree_document = NavigationIntent::Resolved {
            target: doc("sales/customer", None),
            disposition: OpenDisposition::Persistent,
        };
        assert_eq!(
            resolved_target(&tree_document),
            resolved_target(&persistent_tree_document)
        );
        assert_ne!(tree_document, persistent_tree_document);
    }

    #[test]
    fn breadcrumb_uses_authored_titles_and_full_tree_hierarchy() {
        let (okf, _uml) = fixture();
        let segments = breadcrumb_for(&okf, "sales/archive/order").unwrap();
        assert_eq!(
            segments,
            vec![
                crumb(
                    "Root",
                    NavigationTarget::Directory {
                        address: "/".into(),
                    },
                ),
                crumb(
                    "Sales",
                    NavigationTarget::Directory {
                        address: "/sales".into(),
                    },
                ),
                crumb(
                    "Archive",
                    NavigationTarget::Directory {
                        address: "/sales/archive".into(),
                    },
                ),
                crumb(
                    "Purchase Order",
                    NavigationTarget::Document {
                        concept_id: "sales/archive/order".into(),
                        fragment: None,
                    },
                ),
            ]
        );
    }

    /// A breadcrumb is the authored location of a file, not a listing: a
    /// document the folder's declared chain hides is still openable by link,
    /// and it still has a place it lives. Reading the chain here also made
    /// every document activation pay a whole-bundle projection run.
    #[test]
    fn a_document_a_chain_declines_to_list_still_has_a_breadcrumb() {
        let source = waml::source::SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Sales](sales/)\n"),
            (
                "sales/index.md",
                "---\nview: hide\nhide: [\"**\"]\n---\n# Sales\n\n* [Order](./order.md)\n",
            ),
            ("sales/order.md", "# Order\n"),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let segments =
            breadcrumb_for(prepared.okf(), "sales/order").expect("a hidden row still has a home");
        assert_eq!(
            segments
                .iter()
                .map(|segment| segment.title.as_str())
                .collect::<Vec<_>>(),
            vec!["Root", "Sales", "Order"],
        );
    }

    /// Regression: a folder tab's subject is a directory ADDRESS, which
    /// `breadcrumb_for` cannot resolve as a concept -- so every index page
    /// opened with an empty header.
    #[test]
    fn a_folder_tab_gets_the_trail_down_to_the_folder() {
        let source = waml::source::SourceBundle::try_from_pairs([
            (
                "index.md",
                "# Root

* [Sales](sales/)
",
            ),
            (
                "sales/index.md",
                "# Sales

* [Archive](archive/)
",
            ),
            (
                "sales/archive/index.md",
                "# Archive
",
            ),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();

        let segments = breadcrumb_for_directory(prepared.okf(), "/sales/archive")
            .expect("a folder has a place it lives");
        assert_eq!(
            segments
                .iter()
                .map(|segment| segment.title.as_str())
                .collect::<Vec<_>>(),
            vec!["Root", "Sales", "Archive"],
            "the trail ENDS at the folder rather than passing through it"
        );
        // Every segment is a directory, including the last.
        assert!(segments
            .iter()
            .all(|segment| matches!(segment.target, NavigationTarget::Directory { .. })));

        // The root is its own single-segment trail, not an empty one.
        let root = breadcrumb_for_directory(prepared.okf(), "/").expect("the root is a folder too");
        assert_eq!(root.len(), 1);

        assert_eq!(breadcrumb_for_directory(prepared.okf(), "/nope"), None);
    }

    #[test]
    fn scoped_nav_state_cannot_change_canonical_breadcrumb() {
        let (okf, uml) = fixture();
        let before = breadcrumb_for(&okf, "sales/archive/order");
        let states = [
            crate::nav::NavState {
                scope: "/sales".into(),
            },
            crate::nav::NavState { scope: "/".into() },
        ];
        for state in states {
            let _projected = crate::nav::view(
                &okf,
                &uml,
                &state,
                &waml::view::mask::ProjectionMask::default(),
                waml::view::chain::ChainLimits::default(),
            );
            assert_eq!(breadcrumb_for(&okf, "sales/archive/order"), before);
        }
    }

    #[test]
    fn resolve_link_normalizes_the_complete_logical_target_matrix() {
        let bundle = resolve_fixture();
        let cases = [
            ("./customer.md", doc("sales/customer", None)),
            ("../shared.md", doc("shared", None)),
            ("/sales/customer.md", doc("sales/customer", None)),
            ("#orders", doc("sales/order", Some("orders"))),
            (
                "./customer.md#history",
                doc("sales/customer", Some("history")),
            ),
            ("./archive/", dir("/sales/archive")),
            ("/", dir("/")),
            (
                "https://example.com/a?q=1#b",
                NavigationTarget::ExternalUrl("https://example.com/a?q=1#b".into()),
            ),
        ];
        for (href, expected) in cases {
            assert_eq!(
                resolve_link(&bundle, "sales/order", href),
                Ok(expected),
                "{href}"
            );
        }
    }

    #[test]
    fn resolve_link_returns_typed_errors_for_invalid_or_missing_targets() {
        let bundle = resolve_fixture();
        let cases = [
            ("../../../escape.md", NavigationError::EscapesBundle),
            (
                "mailto:a@example.com",
                NavigationError::UnsupportedScheme("mailto".into()),
            ),
            (
                "http://",
                NavigationError::MalformedTarget("http://".into()),
            ),
            (
                "./missing.md",
                NavigationError::MissingDocument("sales/missing".into()),
            ),
            (
                "./missing/",
                NavigationError::MissingDirectory("/sales/missing".into()),
            ),
            (
                "./customer.md?mode=1",
                NavigationError::MalformedTarget("./customer.md?mode=1".into()),
            ),
            ("", NavigationError::MalformedTarget(String::new())),
            ("#", NavigationError::MalformedTarget("#".into())),
        ];
        for (href, expected) in cases {
            assert_eq!(
                resolve_link(&bundle, "sales/order", href),
                Err(expected),
                "{href}"
            );
        }
    }

    #[test]
    fn resolve_errors_have_exact_status_messages() {
        let cases = [
            (
                NavigationError::MalformedTarget("bad".into()),
                "Invalid link: bad",
            ),
            (
                NavigationError::UnsupportedScheme("mailto".into()),
                "Unsupported link scheme: mailto",
            ),
            (NavigationError::EscapesBundle, "Link leaves this bundle"),
            (
                NavigationError::MissingDocument("sales/missing".into()),
                "Document not found: sales/missing",
            ),
            (
                NavigationError::MissingDirectory("/sales/missing".into()),
                "Directory not found: /sales/missing",
            ),
        ];
        for (error, expected) in cases {
            assert_eq!(error.status_message(), expected);
        }
    }
}
