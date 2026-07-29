use crate::tree::{build_tree, TreeNode};
use url::Url;
use waml::okf::DirectoryAddress;

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

pub fn breadcrumb_for(
    bundle: &waml::okf::Bundle,
    uml: &waml::uml::Projection,
    concept_id: &str,
) -> Option<Vec<BreadcrumbSegment>> {
    fn find(nodes: &[TreeNode], concept_id: &str, path: &mut Vec<BreadcrumbSegment>) -> bool {
        for node in nodes {
            if node.is_directory {
                path.push(BreadcrumbSegment {
                    title: node.title.clone(),
                    target: NavigationTarget::Directory {
                        address: node.key.clone(),
                    },
                });
                if find(&node.children, concept_id, path) {
                    return true;
                }
                path.pop();
            } else if node.concept_id.as_deref() == Some(concept_id) {
                path.push(BreadcrumbSegment {
                    title: node.title.clone(),
                    target: NavigationTarget::Document {
                        concept_id: concept_id.to_owned(),
                        fragment: None,
                    },
                });
                return true;
            }
        }
        false
    }

    let tree = build_tree(bundle, uml, "Untitled");
    let mut path = Vec::new();
    find(&tree.roots, concept_id, &mut path).then_some(path)
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

    fn fixture() -> (waml::okf::Bundle, waml::uml::Projection) {
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
        let bundle = waml::okf::Bundle::parse(&source).unwrap();
        let uml = waml::uml::project(&bundle);
        (bundle, uml)
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
            NavigationIntent::MarkdownLink { .. } => None,
        }
    }

    #[test]
    fn tree_breadcrumb_and_markdown_entry_targets_are_equivalent() {
        let bundle = resolve_fixture();
        let uml = waml::uml::project(&bundle);
        let breadcrumb = breadcrumb_for(&bundle, &uml, "sales/customer")
            .expect("customer has a canonical breadcrumb");

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
            target: resolve_link(&bundle, "sales/order", "./customer.md")
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
            target: resolve_link(&bundle, "sales/order", "/sales/")
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
        let (bundle, uml) = fixture();
        let segments = breadcrumb_for(&bundle, &uml, "sales/archive/order").unwrap();
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

    #[test]
    fn filtered_nav_state_cannot_change_canonical_breadcrumb() {
        let (bundle, uml) = fixture();
        let before = breadcrumb_for(&bundle, &uml, "sales/archive/order");
        let states = [
            crate::nav::NavState {
                scope: "/sales".into(),
                query: String::new(),
                filter: None,
            },
            crate::nav::NavState {
                scope: "/".into(),
                query: "purchase".into(),
                filter: None,
            },
            crate::nav::NavState {
                scope: "/".into(),
                query: String::new(),
                filter: Some(crate::tree::TreeKind::Class),
            },
        ];
        for state in states {
            let _projected = crate::nav::view(&bundle, &uml, &state);
            assert_eq!(breadcrumb_for(&bundle, &uml, "sales/archive/order"), before);
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
