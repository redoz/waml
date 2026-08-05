//! OKF (Open Knowledge Format) — the domain-agnostic substrate beneath the WAML
//! UML profile. A [`Concept`] is the lossless projection of a single markdown
//! document; a [`Bundle`] separates concepts, indexes, logs, and directories.
//!
//! Hard rule: this module MUST NOT import any UML type (`ElementType`,
//! `RelationshipKind`, `UmlMetaclass`, …). The dependency is one-way — the WAML
//! tier depends on `okf`, never the reverse — so a later `okf-core` crate split
//! stays mechanical.

use std::fmt;

use crate::frontmatter::Frontmatter;
use crate::source::{SourceBundle, SourceSlice};

pub(crate) mod lower;
pub mod ops;
pub(crate) mod shell;
pub use ops::{Batch, Op};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct DirectoryAddress(String);

impl DirectoryAddress {
    pub fn parse(address: impl Into<String>) -> Result<Self, BundleError> {
        let original = address.into();
        let normalized = original.replace('\\', "/");
        let invalid = !normalized.starts_with('/')
            || (normalized != "/" && normalized.ends_with('/'))
            || (normalized != "/"
                && (normalized
                    .split('/')
                    .skip(1)
                    .any(|segment| segment.is_empty() || segment == "." || segment == "..")
                    || normalized
                        .rsplit('/')
                        .next()
                        .is_some_and(|segment| segment.ends_with(".md"))));
        if invalid {
            return Err(BundleError::InvalidDirectoryAddress(original));
        }
        Ok(DirectoryAddress(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn parent(&self) -> Option<DirectoryAddress> {
        if self.0 == "/" {
            return None;
        }
        let parent = self
            .0
            .rsplit_once('/')
            .map(|(parent, _)| parent)
            .unwrap_or("");
        Some(DirectoryAddress(if parent.is_empty() {
            "/".into()
        } else {
            parent.into()
        }))
    }

    pub fn join_directory(&self, child: &str) -> Result<DirectoryAddress, BundleError> {
        let child = child.replace('\\', "/");
        let joined = if self.0 == "/" {
            format!("/{child}")
        } else {
            format!("{}/{child}", self.0)
        };
        DirectoryAddress::parse(joined)
    }

    pub fn concept_parent(id: &str) -> Result<DirectoryAddress, BundleError> {
        let id = id.replace('\\', "/");
        if id.is_empty()
            || id.starts_with('/')
            || id.ends_with('/')
            || id
                .split('/')
                .any(|segment| segment.is_empty() || segment == "." || segment == "..")
        {
            return Err(BundleError::InvalidConceptId(id));
        }
        match id.rsplit_once('/') {
            Some((directory, _)) => DirectoryAddress::parse(format!("/{directory}")),
            None => DirectoryAddress::parse("/"),
        }
    }

    pub fn index_path(&self) -> String {
        if self.0 == "/" {
            "index.md".into()
        } else {
            format!("{}/index.md", self.0.trim_start_matches('/'))
        }
    }

    fn from_source_path(path: &str) -> DirectoryAddress {
        match path.rsplit_once('/') {
            Some((directory, _)) => DirectoryAddress(format!("/{directory}")),
            None => DirectoryAddress("/".into()),
        }
    }
}

impl fmt::Display for DirectoryAddress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for DirectoryAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let address = <String as serde::Deserialize>::deserialize(deserializer)?;
        DirectoryAddress::parse(address).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BundleError {
    InvalidDirectoryAddress(String),
    InvalidConceptId(String),
    DuplicateConceptId {
        id: String,
        first_path: String,
        second_path: String,
    },
    Analysis(String),
}

impl fmt::Display for BundleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BundleError::InvalidDirectoryAddress(address) => {
                write!(formatter, "invalid directory address: {address}")
            }
            BundleError::InvalidConceptId(id) => write!(formatter, "invalid concept id: {id}"),
            BundleError::DuplicateConceptId {
                id,
                first_path,
                second_path,
            } => write!(
                formatter,
                "duplicate concept id '{id}' from '{first_path}' and '{second_path}'"
            ),
            BundleError::Analysis(reason) => formatter.write_str(reason),
        }
    }
}

impl std::error::Error for BundleError {}

/// An untyped OKF link (`[text](href)`) drawn from a concept's body (OKF §5.3).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Link {
    pub text: String,
    pub href: String,
}

/// An actor reference (`kind:id`, e.g. `human:ahormati`) naming who or what
/// performed an action such as generating or verifying a concept (OKF §7).
/// The `kind:` prefix is a convention, not a validation gate — parsing never
/// fails; a bare id (no `:`) simply carries no `kind`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Actor {
    pub kind: Option<String>,
    pub id: String,
}

/// A usage window: the span of time a source was consulted or is valid for
/// (OKF §5.1). Either bound may be absent.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UsageWindow {
    pub from: Option<String>,
    pub to: Option<String>,
}

/// A source: a link to external material backing a claim, with optional
/// credibility signals (OKF §5.1). Replaces the v0.1 `Citation`, whose
/// legacy `# Citations` body heading still projects here (see
/// `extract_legacy_sources`) with every credibility signal absent.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Source {
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub id: Option<String>,
    pub resource: String,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub title: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub author: Option<Actor>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub usage_count: Option<f64>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub last_modified: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub usage_window: Option<UsageWindow>,
}

/// Records who generated a concept and when (OKF §7). `by` is REQUIRED, so
/// unlike `Concept::timestamp` this can never be synthesized for a v0.1
/// document — see `Concept::generated_at`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Generated {
    pub by: Actor,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub at: Option<String>,
}

/// One verification record: an actor attesting to a concept at a point in
/// time (OKF §7). `Concept::verified` may hold several.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Verification {
    pub by: Actor,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub at: Option<String>,
}

/// The domain-agnostic projection of one markdown document. Round-trips every
/// OKF field losslessly — nothing a producer wrote is dropped: known fields are
/// promoted, the raw markdown body is retained verbatim, and any remaining
/// frontmatter survives in [`Concept::extra`].
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Concept {
    /// Concept ID = full path minus the `.md` suffix (OKF §2).
    pub id: String,
    /// The free-text `type` frontmatter field (NOT the UML `ElementType`).
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    pub ty: String,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub title: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub description: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub resource: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub tags: Vec<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub timestamp: Option<String>,
    /// The full markdown body (everything after the frontmatter), verbatim.
    pub body: SourceSlice,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub links: Vec<Link>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub sources: Vec<Source>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub generated: Option<Generated>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub verified: Vec<Verification>,
    /// Producer-specific frontmatter keys with no dedicated field above.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "frontmatter_is_empty")
    )]
    pub extra: Frontmatter,
}

impl Concept {
    /// The best-known generation timestamp: `generated.at` when a v0.2
    /// `generated` block promoted, else the v0.1 `timestamp` field. A pure
    /// v0.1 document names no actor, so `Concept::generated` itself stays
    /// `None` rather than being fabricated — this accessor is the fallback
    /// (OKF §13.1).
    pub fn generated_at(&self) -> Option<&str> {
        self.generated
            .as_ref()
            .and_then(|generated| generated.at.as_deref())
            .or(self.timestamp.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Index {
    pub directory: DirectoryAddress,
    pub title: Option<String>,
    pub description: Option<String>,
    pub members: Vec<String>,
    pub body: Option<SourceSlice>,
    pub authored: bool,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Log {
    pub directory: DirectoryAddress,
    pub body: SourceSlice,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Directory {
    pub address: DirectoryAddress,
    pub parent: Option<DirectoryAddress>,
    pub child_directories: Vec<DirectoryAddress>,
    pub concepts: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Bundle {
    concepts: Vec<Concept>,
    indexes: Vec<Index>,
    logs: Vec<Log>,
    directories: Vec<Directory>,
}

/// Deserialization accepts arbitrary input, so it cannot assume the wire order matches
/// the sortedness the accessors binary-search on; [`Bundle::from_parts`] re-establishes it.
/// It is otherwise equivalent to the derived impl: every field remains required, so a
/// truncated bundle still fails to decode rather than yielding an empty vector.
#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Bundle {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Every field stays required, exactly as the derived impl had it: a truncated or
        // corrupt bundle must still fail with `missing field`, not decode as empty.
        #[derive(serde::Deserialize)]
        struct Wire {
            concepts: Vec<Concept>,
            indexes: Vec<Index>,
            logs: Vec<Log>,
            directories: Vec<Directory>,
        }

        let wire = Wire::deserialize(deserializer)?;
        Ok(Bundle::from_parts(
            wire.concepts,
            wire.indexes,
            wire.logs,
            wire.directories,
        ))
    }
}

impl Bundle {
    /// The single constructor for a populated `Bundle`. It establishes the ordering the
    /// accessors below binary-search on: concepts by `id`, indexes and logs by
    /// `directory`, directories by `address`. Callers that already build in order (see
    /// `shell::project`) pay only the sortedness check of a sorted merge sort.
    pub(crate) fn from_parts(
        mut concepts: Vec<Concept>,
        mut indexes: Vec<Index>,
        mut logs: Vec<Log>,
        mut directories: Vec<Directory>,
    ) -> Self {
        concepts.sort_by(|left, right| left.id.cmp(&right.id));
        indexes.sort_by(|left, right| left.directory.cmp(&right.directory));
        logs.sort_by(|left, right| left.directory.cmp(&right.directory));
        directories.sort_by(|left, right| left.address.cmp(&right.address));
        Self {
            concepts,
            indexes,
            logs,
            directories,
        }
    }

    pub fn parse(source: &SourceBundle) -> Result<Self, BundleError> {
        crate::analysis::analyze_okf(source, None, 0)
            .map(|analysis| analysis.bundle)
            .map_err(|error| match error {
                crate::analysis::AnalysisError::Okf(source) => source,
                other => BundleError::Analysis(other.to_string()),
            })
    }

    /// Requires `self.concepts` to be sorted by `id` (established by
    /// [`Bundle::from_parts`], the only constructor of a populated bundle).
    pub fn concept(&self, id: &str) -> Option<&Concept> {
        self.concepts
            .binary_search_by(|c| c.id.as_str().cmp(id))
            .ok()
            .map(|i| &self.concepts[i])
    }

    /// Requires `self.indexes` to be sorted by `directory` (established by
    /// [`Bundle::from_parts`]).
    pub fn index(&self, address: &str) -> Option<&Index> {
        self.indexes
            .binary_search_by(|index| index.directory.as_str().cmp(address))
            .ok()
            .map(|i| &self.indexes[i])
    }

    /// Requires `self.logs` to be sorted by `directory` (established by
    /// [`Bundle::from_parts`]).
    pub fn log(&self, address: &str) -> Option<&Log> {
        self.logs
            .binary_search_by(|log| log.directory.as_str().cmp(address))
            .ok()
            .map(|i| &self.logs[i])
    }

    /// Requires `self.directories` to be sorted by `address` (established by
    /// [`Bundle::from_parts`]).
    pub fn directory(&self, address: &str) -> Option<&Directory> {
        self.directories
            .binary_search_by(|directory| directory.address.as_str().cmp(address))
            .ok()
            .map(|i| &self.directories[i])
    }

    pub fn concepts(&self) -> &[Concept] {
        &self.concepts
    }

    pub fn indexes(&self) -> &[Index] {
        &self.indexes
    }

    pub fn logs(&self) -> &[Log] {
        &self.logs
    }

    pub fn directories(&self) -> &[Directory] {
        &self.directories
    }
}

#[cfg(feature = "serde")]
fn frontmatter_is_empty(fm: &Frontmatter) -> bool {
    fm.entries.is_empty()
}

/// The concept ID for a bundle path: full path with a trailing `.md` removed
/// and backslashes normalized to `/` (OKF §2).
pub fn id_of(path: &str) -> String {
    let norm = path.replace('\\', "/");
    norm.strip_suffix(".md").unwrap_or(&norm).to_string()
}

/// Resolve a written href (e.g. `./orders.md`, `../shop/order.md`) against the
/// *referring* document's own bundle-relative path, producing the target's full
/// id (same shape as [`id_of`]). Bare, `./`, `../`, and root-relative paths are
/// accepted; query/fragment suffixes are ignored for identity. The authored
/// href itself is never normalized or rewritten.
pub fn resolve_href(referring_path: &str, href: &str) -> String {
    let referring_norm = referring_path.replace('\\', "/");
    let href_norm = href
        .split_once(['?', '#'])
        .map_or(href, |(resource, _)| resource)
        .replace('\\', "/");
    if href_norm.is_empty() {
        return id_of(&referring_norm);
    }
    let rooted = href_norm.starts_with('/');
    let href_trimmed = href_norm.strip_prefix("./").unwrap_or(&href_norm);

    let mut segments: Vec<&str> = if rooted {
        Vec::new()
    } else {
        referring_norm
            .rsplit_once('/')
            .map(|(dir, _)| dir.split('/').collect())
            .unwrap_or_default()
    };

    for seg in href_trimmed.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            other => segments.push(other),
        }
    }

    let joined = segments.join("/");
    id_of(&joined)
}

/// Author a stable relative Markdown href from one bundle document to another.
pub fn relative_href(referring_path: &str, target_path: &str) -> String {
    let referring = referring_path.replace('\\', "/");
    let target = target_path.replace('\\', "/");
    let from = referring
        .rsplit_once('/')
        .map(|(directory, _)| directory.split('/').collect::<Vec<_>>())
        .unwrap_or_default();
    let to = target.split('/').collect::<Vec<_>>();
    let common = from
        .iter()
        .zip(&to)
        .take_while(|(left, right)| left == right)
        .count();
    let mut authored = "../".repeat(from.len().saturating_sub(common));
    authored.push_str(&to[common..].join("/"));
    if !authored.starts_with('.') {
        authored.insert_str(0, "./");
    }
    authored
}

/// Project a single Markdown source into its OKF [`Concept`], or `None` when
/// the projection yields zero concepts — a reserved filename (`index.md` /
/// `log.md`) or a source the OKF shell quarantines instead of erroring
/// (e.g. an oversized document). Caller-error conditions (a non-bundle-
/// relative path, an impossible duplicate concept ID for a single document)
/// fold into the same `None` rather than panicking.
pub fn project(path: &str, src: &str) -> Option<Concept> {
    let bundle_path = if path.ends_with(".md") {
        path.to_owned()
    } else {
        format!("{path}.md")
    };
    let source = SourceBundle::try_from_pairs([(bundle_path, src.to_owned())]).ok()?;
    let mut concept = Bundle::parse(&source).ok()?.concepts.into_iter().next()?;
    concept.id = id_of(path);
    Some(concept)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn directory_addresses_are_rooted_normalized_and_navigable() {
        let root = DirectoryAddress::parse("/").unwrap();
        let orders = DirectoryAddress::parse(r"\sales\orders").unwrap();

        assert_eq!(orders.as_str(), "/sales/orders");
        assert_eq!(orders.parent().unwrap().as_str(), "/sales");
        assert_eq!(root.parent(), None);
        assert_eq!(
            DirectoryAddress::parse("/sales")
                .unwrap()
                .join_directory("orders")
                .unwrap(),
            orders
        );
        assert_eq!(
            DirectoryAddress::concept_parent("sales/orders/order")
                .unwrap()
                .as_str(),
            "/sales/orders"
        );
        assert_eq!(orders.index_path(), "sales/orders/index.md");

        for invalid in ["sales", "/sales/../orders", "/sales/order.md", "/sales/"] {
            assert!(DirectoryAddress::parse(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn first_class_bundle_separates_reserved_documents_and_builds_hierarchy() {
        let source = SourceBundle::try_from_pairs([
            (
                "index.md",
                "# Root\n\n* [Sales](sales/)\n* [Root](./root.md)\n",
            ),
            ("root.md", "---\ntype: Note\n---\n# Root\n"),
            (
                "sales/index.md",
                "# Sales\n\nSales docs.\n\n* [Order](./order.md)\n",
            ),
            ("sales/order.md", "---\ntype: uml.Class\n---\n# Order\n"),
            ("sales/log.md", "# Log\n\n2026-07-27: started\n"),
            ("sales/archive/old.md", "---\ntype: Archive\n---\n# Old\n"),
        ])
        .unwrap();

        let bundle = Bundle::parse(&source).unwrap();

        assert_eq!(
            bundle
                .concepts()
                .iter()
                .map(|concept| concept.id.as_str())
                .collect::<Vec<_>>(),
            ["root", "sales/archive/old", "sales/order"]
        );
        assert!(bundle.concept("index").is_none());
        assert!(bundle.concept("sales/index").is_none());
        assert!(bundle.concept("sales/log").is_none());

        let root = bundle.index("/").unwrap();
        assert!(root.authored);
        assert_eq!(root.members, ["/sales", "root"]);
        let sales = bundle.index("/sales").unwrap();
        assert!(sales.authored);
        assert_eq!(sales.description.as_deref(), Some("Sales docs."));
        assert_eq!(sales.members, ["sales/order", "/sales/archive"]);
        let archive = bundle.index("/sales/archive").unwrap();
        assert!(!archive.authored);
        assert_eq!(archive.members, ["sales/archive/old"]);

        assert!(bundle.log("/sales").is_some());
        assert_eq!(
            bundle
                .directories()
                .iter()
                .map(|directory| directory.address.as_str())
                .collect::<Vec<_>>(),
            ["/", "/sales", "/sales/archive"]
        );
        let sales_dir = bundle.directory("/sales").unwrap();
        assert_eq!(sales_dir.parent.as_ref().unwrap().as_str(), "/");
        assert_eq!(
            sales_dir.child_directories,
            [DirectoryAddress::parse("/sales/archive").unwrap()]
        );
        assert_eq!(sales_dir.concepts, ["sales/order"]);
    }

    #[test]
    fn bundle_accessors_binary_search_first_middle_last_and_absent() {
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Root\n"),
            ("alpha.md", "---\ntype: Note\n---\n# Alpha\n"),
            ("beta.md", "---\ntype: Note\n---\n# Beta\n"),
            ("gamma.md", "---\ntype: Note\n---\n# Gamma\n"),
            ("east/index.md", "# East\n"),
            ("east/log.md", "# Log\n"),
            ("west/index.md", "# West\n"),
            ("west/log.md", "# Log\n"),
        ])
        .unwrap();

        let bundle = Bundle::parse(&source).unwrap();

        // concepts: alpha, beta, gamma sort first/middle/last.
        assert_eq!(bundle.concept("alpha").unwrap().id, "alpha");
        assert_eq!(bundle.concept("beta").unwrap().id, "beta");
        assert_eq!(bundle.concept("gamma").unwrap().id, "gamma");
        assert!(bundle.concept("missing").is_none());

        // directories: "/", "/east", "/west" sort first/middle/last.
        assert!(bundle.directory("/").is_some());
        assert!(bundle.directory("/east").is_some());
        assert!(bundle.directory("/west").is_some());
        assert!(bundle.directory("/missing").is_none());

        assert!(bundle.index("/").is_some());
        assert!(bundle.index("/east").is_some());
        assert!(bundle.index("/west").is_some());
        assert!(bundle.index("/missing").is_none());

        assert!(bundle.log("/east").is_some());
        assert!(bundle.log("/west").is_some());
        assert!(bundle.log("/missing").is_none());
    }

    #[test]
    fn nested_directories_group_children_and_concepts_in_address_order() {
        // Every directory below holds MORE THAN ONE child directory and more than one
        // concept, and the documents are handed over out of order, so a grouping or
        // ordering regression in the single-pass build shows up here.
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Root\n"),
            ("zeta.md", "---\ntype: Note\n---\n# Zeta\n"),
            ("alpha.md", "---\ntype: Note\n---\n# Alpha\n"),
            ("sales/west/deal.md", "---\ntype: Note\n---\n# Deal\n"),
            ("sales/order.md", "---\ntype: Note\n---\n# Order\n"),
            ("sales/east/lead.md", "---\ntype: Note\n---\n# Lead\n"),
            ("sales/east/bid.md", "---\ntype: Note\n---\n# Bid\n"),
            ("sales/customer.md", "---\ntype: Note\n---\n# Customer\n"),
            ("ops/runbook.md", "---\ntype: Note\n---\n# Runbook\n"),
        ])
        .unwrap();

        let bundle = Bundle::parse(&source).unwrap();

        let grouped = |address: &str| {
            let directory = bundle.directory(address).unwrap();
            (
                directory
                    .child_directories
                    .iter()
                    .map(|child| child.as_str().to_owned())
                    .collect::<Vec<_>>(),
                directory.concepts.clone(),
            )
        };

        let (root_children, root_concepts) = grouped("/");
        assert_eq!(root_children, ["/ops", "/sales"]);
        assert_eq!(root_concepts, ["alpha", "zeta"]);

        let (sales_children, sales_concepts) = grouped("/sales");
        assert_eq!(sales_children, ["/sales/east", "/sales/west"]);
        assert_eq!(sales_concepts, ["sales/customer", "sales/order"]);

        let (east_children, east_concepts) = grouped("/sales/east");
        assert!(east_children.is_empty());
        assert_eq!(east_concepts, ["sales/east/bid", "sales/east/lead"]);

        let (west_children, west_concepts) = grouped("/sales/west");
        assert!(west_children.is_empty());
        assert_eq!(west_concepts, ["sales/west/deal"]);

        let (ops_children, ops_concepts) = grouped("/ops");
        assert!(ops_children.is_empty());
        assert_eq!(ops_concepts, ["ops/runbook"]);

        // A directory only reaches its own children — no leakage across siblings.
        assert_eq!(
            bundle
                .directories()
                .iter()
                .map(|directory| directory.address.as_str())
                .collect::<Vec<_>>(),
            ["/", "/ops", "/sales", "/sales/east", "/sales/west"]
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn deserialized_bundle_rejects_a_truncated_wire_form() {
        for missing in ["concepts", "indexes", "logs", "directories"] {
            let mut wire = serde_json::to_value(Bundle::default()).unwrap();
            wire.as_object_mut().unwrap().remove(missing);
            let error = serde_json::from_value::<Bundle>(wire)
                .expect_err("a bundle missing a field must not decode");
            assert!(
                error.to_string().contains(missing),
                "expected a missing-field error naming `{missing}`, got: {error}"
            );
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn deserialized_bundle_reestablishes_accessor_ordering() {
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Root\n"),
            ("alpha.md", "---\ntype: Note\n---\n# Alpha\n"),
            ("beta.md", "---\ntype: Note\n---\n# Beta\n"),
            ("east/index.md", "# East\n"),
            ("east/log.md", "# Log\n"),
            ("west/index.md", "# West\n"),
            ("west/log.md", "# Log\n"),
        ])
        .unwrap();
        let bundle = Bundle::parse(&source).unwrap();

        // A wire bundle is untrusted input: reverse every vector so no accessor's
        // binary search would find its record without re-sorting on the way in.
        let mut wire = serde_json::to_value(&bundle).unwrap();
        for key in ["concepts", "indexes", "logs", "directories"] {
            wire[key].as_array_mut().unwrap().reverse();
        }
        let decoded: Bundle = serde_json::from_value(wire).unwrap();

        assert_eq!(decoded, bundle);
        assert_eq!(decoded.concept("alpha").unwrap().id, "alpha");
        assert_eq!(decoded.concept("beta").unwrap().id, "beta");
        assert!(decoded.concept("missing").is_none());
        assert!(decoded.index("/").is_some());
        assert!(decoded.index("/east").is_some());
        assert!(decoded.index("/west").is_some());
        assert!(decoded.log("/east").is_some());
        assert!(decoded.log("/west").is_some());
        assert!(decoded.directory("/").is_some());
        assert!(decoded.directory("/east").is_some());
        assert!(decoded.directory("/west").is_some());
    }

    #[test]
    fn parsed_records_share_their_source_documents() {
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Root\n"),
            ("note.md", "---\ntype: Note\n---\n# Note\n"),
            ("log.md", "# Log\n"),
        ])
        .unwrap();
        let bundle = Bundle::parse(&source).unwrap();

        for (path, body) in [
            (
                "index.md",
                bundle.index("/").unwrap().body.as_ref().unwrap(),
            ),
            ("note.md", &bundle.concept("note").unwrap().body),
            ("log.md", &bundle.log("/").unwrap().body),
        ] {
            let document = source
                .document(&crate::source::BundlePath::parse(path).unwrap())
                .unwrap();
            assert!(
                Arc::ptr_eq(document.text_arc(), body.source_arc()),
                "{path}"
            );
        }
    }

    #[test]
    fn id_strips_md_and_keeps_full_path() {
        assert_eq!(id_of("tables/orders.md"), "tables/orders");
        assert_eq!(id_of("orders.md"), "orders");
        assert_eq!(id_of("a\\b\\c.md"), "a/b/c");
    }

    #[test]
    fn resolve_href_same_dir() {
        assert_eq!(
            resolve_href("tables/index.md", "./orders.md"),
            "tables/orders"
        );
    }

    #[test]
    fn resolve_href_root_referring_doc() {
        assert_eq!(resolve_href("readme.md", "./x.md"), "x");
    }

    #[test]
    fn resolve_href_nested_multi_segment() {
        assert_eq!(
            resolve_href("tables/index.md", "./sub/x.md"),
            "tables/sub/x"
        );
    }

    #[test]
    fn resolve_href_parent_dir_escape() {
        assert_eq!(
            resolve_href("tables/orders.md", "../shop/order.md"),
            "shop/order"
        );
    }

    #[test]
    fn href_resolution_accepts_authored_spellings_and_ignores_suffixes() {
        for href in [
            "money.md",
            "./money.md",
            "./money.md?view=compact",
            "./money.md#amount",
            "./money.md?view=compact#amount",
        ] {
            assert_eq!(resolve_href("shop/order.md", href), "shop/money", "{href}");
        }
        assert_eq!(
            resolve_href("shop/orders/order.md", "../money.md#amount"),
            "shop/money"
        );
        assert_eq!(resolve_href("shop/order.md", "#attributes"), "shop/order");
        assert_eq!(
            resolve_href("shop/order.md", "/types/money.md"),
            "types/money"
        );
        assert_eq!(
            relative_href("shop/order.md", "shop/money.md"),
            "./money.md"
        );
        assert_eq!(
            relative_href("shop/orders/order.md", "types/money.md"),
            "../../types/money.md"
        );
    }

    #[test]
    fn non_uml_doc_round_trips_every_field_onto_concept() {
        // Headline test: an OKF `Playbook` (a non-`uaml.*` type) with tags,
        // resource, timestamp, links, and citations — every field survives on
        // the projected Concept. Impossible before the two-tier split.
        let src = "---\n\
            type: Playbook\n\
            title: Dataplex Playbook\n\
            description: How to onboard Dataplex.\n\
            resource: /playbooks/dataplex\n\
            tags: [data, governance]\n\
            timestamp: 2026-05-22\n\
            owner: data-team\n\
            ---\n\
            # Dataplex Playbook\n\n\
            See the [customers table](/tables/customers.md) for the join key.\n\n\
            # Citations\n\n\
            [1] [BigQuery announcement](https://cloud.google.com/blog/x)\n";
        let c = project("playbooks/dataplex.md", src).unwrap();

        assert_eq!(c.id, "playbooks/dataplex");
        assert_eq!(c.ty, "Playbook");
        assert_eq!(c.title.as_deref(), Some("Dataplex Playbook"));
        assert_eq!(c.description.as_deref(), Some("How to onboard Dataplex."));
        assert_eq!(c.resource.as_deref(), Some("/playbooks/dataplex"));
        assert_eq!(c.tags, vec!["data", "governance"]);
        assert_eq!(c.timestamp.as_deref(), Some("2026-05-22"));
        // Body is verbatim (round-trip truth).
        assert!(c.body.contains("# Dataplex Playbook"));
        assert!(c.body.contains("# Citations"));
        // A body link is captured; the citation link is separated out.
        assert_eq!(c.links.len(), 1);
        assert_eq!(c.links[0].href, "/tables/customers.md");
        assert_eq!(c.sources.len(), 1);
        assert_eq!(c.sources[0].resource, "https://cloud.google.com/blog/x");
        // Unknown frontmatter survives in `extra`; known keys do not leak in.
        assert_eq!(c.extra.get_str("owner"), Some("data-team"));
        assert!(c.extra.get("type").is_none());
        assert!(c.extra.get("tags").is_none());
        assert!(c.extra.get("title").is_none());
    }

    #[test]
    fn concept_links_and_sources_follow_authoritative_markdown_queries() {
        let concept = project(
            "links.md",
            "---\ntype: Note\n---\n# Links\n\n[real **nested**](./real.md)\n\n![image](./image.md)\n\n\\[escaped](./escaped.md)\n\n[reference][ref]\n\n```md\n[fenced](./fenced.md)\n```\n\n# Citations\n\n[citation](https://example.test/citation)\n\n![citation image](./citation.png)\n\n[ref]: ./reference.md\n",
        )
        .unwrap();

        assert_eq!(
            concept
                .links
                .iter()
                .map(|link| (link.text.as_str(), link.href.as_str()))
                .collect::<Vec<_>>(),
            [
                ("real **nested**", "./real.md"),
                ("reference", "./reference.md"),
            ]
        );
        assert_eq!(
            concept
                .sources
                .iter()
                .map(|source| (source.title.as_deref(), source.resource.as_str()))
                .collect::<Vec<_>>(),
            [(Some("citation"), "https://example.test/citation")]
        );
    }

    #[test]
    fn legacy_citation_maps_to_source_with_absent_signals() {
        let src = "---\ntype: Note\n---\n# Note\n\n# Citations\n\n[1] [BigQuery announcement](https://cloud.google.com/blog/x)\n";
        let c = project("note.md", src).unwrap();

        assert_eq!(c.sources.len(), 1);
        let source = &c.sources[0];
        assert_eq!(source.resource, "https://cloud.google.com/blog/x");
        assert_eq!(source.title.as_deref(), Some("BigQuery announcement"));
        assert_eq!(source.id, None);
        assert_eq!(source.author, None);
        assert_eq!(source.usage_count, None);
        assert_eq!(source.last_modified, None);
        assert_eq!(source.usage_window, None);
    }

    #[test]
    fn frontmatter_sources_promote_with_all_signals() {
        let src = "---\ntype: Note\nsources:\n  - id: src-1\n    resource: https://example.test/a\n    title: A\n    usage_count: 3\n    last_modified: 2026-01-01\n    usage_window:\n      from: 2025-01-01\n      to: 2025-06-01\n---\n# Note\n";
        let c = project("note.md", src).unwrap();

        assert_eq!(c.sources.len(), 1);
        let source = &c.sources[0];
        assert_eq!(source.id.as_deref(), Some("src-1"));
        assert_eq!(source.resource, "https://example.test/a");
        assert_eq!(source.title.as_deref(), Some("A"));
        assert_eq!(source.usage_count, Some(3.0));
        assert_eq!(source.last_modified.as_deref(), Some("2026-01-01"));
        assert_eq!(
            source.usage_window,
            Some(UsageWindow {
                from: Some("2025-01-01".to_owned()),
                to: Some("2025-06-01".to_owned()),
            })
        );
        assert!(c.extra.get("sources").is_none());
        assert!(c.extra.get("usage_window").is_none());
    }

    #[test]
    fn frontmatter_sources_promote_author_through_actor_parser() {
        let src = "---\ntype: Note\nsources:\n  - resource: https://example.test/a\n    author: human:ahormati\n---\n# Note\n";
        let c = project("note.md", src).unwrap();

        assert_eq!(
            c.sources[0].author,
            Some(Actor {
                kind: Some("human".to_owned()),
                id: "ahormati".to_owned(),
            })
        );
    }

    #[test]
    fn generated_promotes_by_and_at() {
        let src = "---\ntype: Note\ngenerated:\n  by: process:finance-nightly\n  at: 2026-05-22\n---\n# Note\n";
        let c = project("note.md", src).unwrap();

        let generated = c.generated.as_ref().unwrap();
        assert_eq!(generated.by.kind.as_deref(), Some("process"));
        assert_eq!(generated.by.id, "finance-nightly");
        assert_eq!(generated.at.as_deref(), Some("2026-05-22"));
        assert!(c.extra.get("generated").is_none());
    }

    #[test]
    fn generated_without_by_stays_none_and_survives_in_extra() {
        let src = "---\ntype: Note\ngenerated:\n  at: 2026-05-22\n---\n# Note\n";
        let c = project("note.md", src).unwrap();

        assert_eq!(c.generated, None);
        assert!(c.extra.get("generated").is_some());
    }

    #[test]
    fn generated_at_falls_back_to_timestamp_for_v01() {
        let v01 = project(
            "note.md",
            "---\ntype: Note\ntimestamp: 2026-01-01\n---\n# Note\n",
        )
        .unwrap();
        assert_eq!(v01.generated, None);
        assert_eq!(v01.timestamp.as_deref(), Some("2026-01-01"));
        assert_eq!(v01.generated_at(), Some("2026-01-01"));

        let v02 = project(
            "note.md",
            "---\ntype: Note\ntimestamp: 2026-01-01\ngenerated:\n  by: human:a\n  at: 2026-05-22\n---\n# Note\n",
        )
        .unwrap();
        assert_eq!(v02.generated_at(), Some("2026-05-22"));
    }

    #[test]
    fn bare_verified_mapping_normalizes_to_one_element() {
        let src = "---\ntype: Note\nverified:\n  by: human:reviewer\n---\n# Note\n";
        let c = project("note.md", src).unwrap();

        assert_eq!(c.verified.len(), 1);
        assert_eq!(c.verified[0].by.id, "reviewer");
        assert!(c.extra.get("verified").is_none());
    }

    #[test]
    fn verified_entry_without_by_fails_the_whole_key() {
        let src =
            "---\ntype: Note\nverified:\n  - by: human:reviewer\n  - at: 2026-05-22\n---\n# Note\n";
        let c = project("note.md", src).unwrap();

        assert!(c.verified.is_empty());
        assert!(c.extra.get("verified").is_some());
    }

    #[test]
    fn bare_actor_id_has_no_kind() {
        let src = "---\ntype: Note\nverified:\n  by: finance-nightly\n---\n# Note\n";
        let c = project("note.md", src).unwrap();

        assert_eq!(
            c.verified[0].by,
            Actor {
                kind: None,
                id: "finance-nightly".to_owned(),
            }
        );
    }

    #[test]
    fn frontmatter_sources_take_precedence_over_citations_heading() {
        let src = "---\ntype: Note\nsources:\n  - resource: https://example.test/fm\n---\n# Note\n\n# Citations\n\n[legacy](https://example.test/legacy)\n";
        let c = project("note.md", src).unwrap();

        assert_eq!(c.sources.len(), 1);
        assert_eq!(c.sources[0].resource, "https://example.test/fm");
        assert!(c.body.contains("# Citations"));
    }

    #[test]
    fn sibling_usage_window_fills_entries_and_entry_window_overrides() {
        let src = "---\ntype: Note\nsources:\n  - resource: https://example.test/a\n  - resource: https://example.test/b\n    usage_window:\n      from: 2020-01-01\n      to: 2020-02-01\nusage_window:\n  from: 2025-01-01\n  to: 2025-06-01\n---\n# Note\n";
        let c = project("note.md", src).unwrap();

        assert_eq!(c.sources.len(), 2);
        assert_eq!(
            c.sources[0].usage_window,
            Some(UsageWindow {
                from: Some("2025-01-01".to_owned()),
                to: Some("2025-06-01".to_owned()),
            })
        );
        assert_eq!(
            c.sources[1].usage_window,
            Some(UsageWindow {
                from: Some("2020-01-01".to_owned()),
                to: Some("2020-02-01".to_owned()),
            })
        );
    }

    #[test]
    fn sources_entry_without_resource_fails_the_whole_key() {
        let src = "---\ntype: Note\nsources:\n  - resource: https://example.test/a\n  - title: no resource here\n---\n# Note\n";
        let c = project("note.md", src).unwrap();

        assert!(c.sources.is_empty());
        assert!(c.extra.get("sources").is_some());
    }

    #[test]
    fn sources_that_is_not_a_list_promotes_nothing_and_survives_in_extra() {
        let src = "---\ntype: Note\nsources: oops\n---\n# Note\n";
        let c = project("note.md", src).unwrap();

        assert!(c.sources.is_empty());
        assert_eq!(c.extra.get_str("sources"), Some("oops"));
    }

    #[test]
    fn usage_count_that_is_not_a_number_stays_none_and_key_still_promotes() {
        let src = "---\ntype: Note\nsources:\n  - resource: https://example.test/a\n    usage_count: many\n---\n# Note\n";
        let c = project("note.md", src).unwrap();

        assert_eq!(c.sources.len(), 1);
        assert_eq!(c.sources[0].usage_count, None);
        assert!(c.extra.get("sources").is_none());
    }

    #[test]
    fn authored_index_order_uses_only_real_list_links() {
        let source = SourceBundle::try_from_pairs([
            (
                "index.md",
                "# Root\n\nIntro.\n\n* [Real](./real.md)\n* ![Image](./image.md)\n* \\[Escaped](./escaped.md)\n* [Reference][ref]\n\n```md\n* [Fenced](./fenced.md)\n```\n\n[ref]: ./reference.md\n",
            ),
            ("real.md", "# Real\n"),
            ("reference.md", "# Reference\n"),
            ("image.md", "# Image\n"),
            ("escaped.md", "# Escaped\n"),
            ("fenced.md", "# Fenced\n"),
        ])
        .unwrap();
        let bundle = Bundle::parse(&source).unwrap();
        let index = bundle.index("/").unwrap();

        assert_eq!(index.title.as_deref(), Some("Root"));
        assert_eq!(index.description.as_deref(), Some("Intro."));
        assert_eq!(
            index.members,
            ["real", "reference", "escaped", "fenced", "image"]
        );
    }

    #[test]
    fn authored_index_dedupes_duplicates_and_drops_stale_members() {
        let source = SourceBundle::try_from_pairs([
            (
                "index.md",
                "# Root\n\n* [Real](./real.md)\n* [Real again](./real.md)\n* [Ghost](./ghost.md)\n",
            ),
            ("real.md", "# Real\n"),
        ])
        .unwrap();
        let bundle = Bundle::parse(&source).unwrap();
        let index = bundle.index("/").unwrap();

        // "real" listed twice authored-side collapses to one entry; "ghost" has no
        // backing concept (not in default_order) so it is dropped, not fabricated.
        assert_eq!(index.members, ["real"]);
    }

    #[test]
    fn title_falls_back_to_first_h1_when_frontmatter_title_absent() {
        // No `title:` frontmatter → concept.title resolves to the H1 text.
        let c = project(
            "shop/order.md",
            "---\ntype: uml.Class\n---\n# Order Heading\n\n## Attributes\n- id: X\n",
        )
        .unwrap();
        assert_eq!(c.title.as_deref(), Some("Order Heading"));
    }

    #[test]
    fn title_prefers_frontmatter_over_h1() {
        let c = project("x.md", "---\ntitle: FM Title\n---\n# H1 Title\n").unwrap();
        assert_eq!(c.title.as_deref(), Some("FM Title"));
    }

    #[test]
    fn title_is_none_when_neither_frontmatter_nor_h1_present() {
        let c = project(
            "x.md",
            "---\ntype: uml.Class\n---\n\nprose with no heading\n",
        )
        .unwrap();
        assert_eq!(c.title, None);
    }

    #[test]
    fn uml_doc_also_projects_to_a_concept() {
        let src =
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n";
        let c = project("shop/order.md", src).unwrap();
        assert_eq!(c.id, "shop/order");
        assert_eq!(c.ty, "uml.Class");
        assert_eq!(c.title.as_deref(), Some("Order"));
        assert!(c.body.contains("## Attributes"));
    }

    #[test]
    fn bundle_separates_reserved_documents_from_concepts() {
        let bundle = vec![
            (
                "index.md".to_string(),
                "# Root Index\n\n* [Order](order.md)\n".to_string(),
            ),
            (
                "order.md".to_string(),
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
            ),
            (
                "log.md".to_string(),
                "# Update Log\n\n## 2026-05-22\n* Created.\n".to_string(),
            ),
        ];
        let source = SourceBundle::try_from_pairs(bundle).unwrap();
        let b = Bundle::parse(&source).unwrap();
        assert_eq!(b.concepts().len(), 1);
        assert!(b.concept("index").is_none());
        assert!(b.concept("log").is_none());
        assert!(b.concept("order").is_some());
        assert!(b.index("/").unwrap().authored);
        assert!(b.log("/").is_some());
    }

    #[test]
    fn project_returns_none_for_reserved_filename() {
        // `index.md` and `log.md` are routed away from the concepts vec
        // (see `okf::shell::project`), so a bare projection of either yields
        // zero concepts rather than panicking.
        assert!(project("index.md", "# Root\n").is_none());
        assert!(project("log.md", "# Log\n").is_none());
        assert!(project("docs/index.md", "# Root\n").is_none());
    }

    #[test]
    fn project_returns_none_for_shell_rejected_path() {
        // A path that cannot round-trip through `BundlePath::parse` (here,
        // rooted at `/`) is a caller-error condition folded into `None`
        // instead of an `expect` panic — see design decision 3.
        assert!(project("/etc/passwd", "# X\n").is_none());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn concept_serializes_with_type_field_and_omits_empty() {
        let c = project(
            "shop/order.md",
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
        )
        .unwrap();
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(v["type"], "uml.Class");
        assert_eq!(v["id"], "shop/order");
        assert_eq!(v["title"], "Order");
        // Empty collections are omitted from the wire.
        assert!(v.get("tags").is_none());
        assert!(v.get("links").is_none());
        assert!(v.get("extra").is_none());
    }
}
