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
use crate::source::{SourceBundle, SourceDocument, SourceSlice};

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

/// A citation: a link to an external source backing a claim, listed under a
/// `# Citations` heading (OKF §8).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Citation {
    pub text: String,
    pub href: String,
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
    pub citations: Vec<Citation>,
    /// Producer-specific frontmatter keys with no dedicated field above.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "frontmatter_is_empty")
    )]
    pub extra: Frontmatter,
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Bundle {
    concepts: Vec<Concept>,
    indexes: Vec<Index>,
    logs: Vec<Log>,
    directories: Vec<Directory>,
}

impl Bundle {
    pub fn parse(source: &SourceBundle) -> Result<Self, BundleError> {
        crate::analysis::analyze_okf(source, None, 0)
            .map(|analysis| analysis.bundle)
            .map_err(|error| match error {
                crate::analysis::AnalysisError::Okf(source) => source,
                other => BundleError::Analysis(other.to_string()),
            })
    }

    pub fn concept(&self, id: &str) -> Option<&Concept> {
        self.concepts.iter().find(|c| c.id == id)
    }

    pub fn index(&self, address: &str) -> Option<&Index> {
        self.indexes
            .iter()
            .find(|index| index.directory.as_str() == address)
    }

    pub fn log(&self, address: &str) -> Option<&Log> {
        self.logs
            .iter()
            .find(|log| log.directory.as_str() == address)
    }

    pub fn directory(&self, address: &str) -> Option<&Directory> {
        self.directories
            .iter()
            .find(|directory| directory.address.as_str() == address)
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
/// id (same shape as [`id_of`]). Strips a leading `./`, joins against
/// `referring_path`'s parent directory, normalizes `..` segments, normalizes
/// `\` to `/`, and strips a trailing `.md`.
pub fn resolve_href(referring_path: &str, href: &str) -> String {
    let referring_norm = referring_path.replace('\\', "/");
    let href_norm = href.replace('\\', "/");
    let href_trimmed = href_norm.strip_prefix("./").unwrap_or(&href_norm);

    let mut segments: Vec<&str> = referring_norm
        .rsplit_once('/')
        .map(|(dir, _)| dir.split('/').collect())
        .unwrap_or_default();

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

pub fn project_document(document: &SourceDocument) -> Option<Concept> {
    let filename = document
        .path()
        .as_str()
        .rsplit('/')
        .next()
        .unwrap_or_default();
    if matches!(filename, "index.md" | "log.md") {
        return None;
    }
    let source = SourceBundle::try_from_pairs([(
        document.path().as_str().to_owned(),
        document.text().to_owned(),
    )])
    .expect("existing source document has a valid path");
    Bundle::parse(&source)
        .expect("one source document has no duplicate concept ID")
        .concepts
        .into_iter()
        .next()
}

pub fn project(path: &str, src: &str) -> Concept {
    let bundle_path = if path.ends_with(".md") {
        path.to_owned()
    } else {
        format!("{path}.md")
    };
    let source = SourceBundle::try_from_pairs([(bundle_path, src.to_owned())])
        .expect("OKF projection path must be bundle-relative");
    let mut concept = Bundle::parse(&source)
        .expect("one source document has no duplicate concept ID")
        .concepts
        .into_iter()
        .next()
        .expect("non-reserved projection produces one concept");
    concept.id = id_of(path);
    concept
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
        let c = project("playbooks/dataplex.md", src);

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
        assert_eq!(c.citations.len(), 1);
        assert_eq!(c.citations[0].href, "https://cloud.google.com/blog/x");
        // Unknown frontmatter survives in `extra`; known keys do not leak in.
        assert_eq!(c.extra.get_str("owner"), Some("data-team"));
        assert!(c.extra.get("type").is_none());
        assert!(c.extra.get("tags").is_none());
        assert!(c.extra.get("title").is_none());
    }

    #[test]
    fn title_falls_back_to_first_h1_when_frontmatter_title_absent() {
        // No `title:` frontmatter → concept.title resolves to the H1 text.
        let c = project(
            "shop/order.md",
            "---\ntype: uml.Class\n---\n# Order Heading\n\n## Attributes\n- id: X\n",
        );
        assert_eq!(c.title.as_deref(), Some("Order Heading"));
    }

    #[test]
    fn title_prefers_frontmatter_over_h1() {
        let c = project("x.md", "---\ntitle: FM Title\n---\n# H1 Title\n");
        assert_eq!(c.title.as_deref(), Some("FM Title"));
    }

    #[test]
    fn title_is_none_when_neither_frontmatter_nor_h1_present() {
        let c = project(
            "x.md",
            "---\ntype: uml.Class\n---\n\nprose with no heading\n",
        );
        assert_eq!(c.title, None);
    }

    #[test]
    fn uml_doc_also_projects_to_a_concept() {
        let src =
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n";
        let c = project("shop/order.md", src);
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

    #[cfg(feature = "serde")]
    #[test]
    fn concept_serializes_with_type_field_and_omits_empty() {
        let c = project(
            "shop/order.md",
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
        );
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
