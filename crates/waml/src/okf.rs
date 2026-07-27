//! OKF (Open Knowledge Format) — the domain-agnostic substrate beneath the WAML
//! UML profile. A [`Concept`] is the lossless projection of a single markdown
//! document; a [`Bundle`] separates concepts, indexes, logs, and directories.
//!
//! Hard rule: this module MUST NOT import any UML type (`ElementType`,
//! `RelationshipKind`, `UmlMetaclass`, …). The dependency is one-way — the WAML
//! tier depends on `okf`, never the reverse — so a later `okf-core` crate split
//! stays mechanical.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::LazyLock;

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use regex::Regex;

use crate::frontmatter::{parse_frontmatter_spanned, Frontmatter};
use crate::source::{SourceBundle, SourceDocument, SourceSlice};

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
        parse_bundle(source)
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

/// Frontmatter keys that project onto dedicated `Concept` fields (everything
/// else survives in `extra`).
const KNOWN_KEYS: &[&str] = &[
    "type",
    "title",
    "description",
    "resource",
    "tags",
    "timestamp",
];

static LINK_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([^\]]*)\]\(([^)]+)\)").unwrap());

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

fn reserved_filename(path: &str) -> Option<&str> {
    match path.rsplit('/').next().unwrap_or(path) {
        "index.md" => Some("index.md"),
        "log.md" => Some("log.md"),
        _ => None,
    }
}

/// Split a body into `(prose_without_citations, citations_section_or_empty)` on
/// the first level-1 `# Citations` heading (OKF §8).
fn split_citations(body: &str) -> (&str, &str) {
    let mut offset = 0usize;
    for line in body.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']).trim();
        if let Some(rest) = trimmed.strip_prefix("# ") {
            if rest.trim().eq_ignore_ascii_case("citations") {
                return (&body[..offset], &body[offset..]);
            }
        }
        offset += line.len();
    }
    (body, "")
}

/// The document's first level-1 heading (H1) text, trimmed, or `None` when the
/// body has no non-empty H1. Extraction mirrors `parse::parse`'s title logic
/// (pulldown over the frontmatter-stripped body) byte-for-byte, so the enriched
/// `concept.title` H1 fallback stays identical to the flat-field title fallback.
fn first_h1(body: &str) -> Option<String> {
    let mut title = String::new();
    let mut in_h1 = false;
    for ev in Parser::new_ext(body, Options::empty()) {
        match ev {
            Event::Start(Tag::Heading {
                level: HeadingLevel::H1,
                ..
            }) => in_h1 = true,
            Event::End(TagEnd::Heading(HeadingLevel::H1)) => in_h1 = false,
            Event::Text(t) | Event::Code(t) if in_h1 => {
                title.push_str(&t);
            }
            _ => {}
        }
    }
    let trimmed = title.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn extract_links(text: &str) -> Vec<Link> {
    LINK_RE
        .captures_iter(text)
        .map(|c| Link {
            text: c[1].to_string(),
            href: c[2].to_string(),
        })
        .collect()
}

fn extract_citations(text: &str) -> Vec<Citation> {
    LINK_RE
        .captures_iter(text)
        .map(|c| Citation {
            text: c[1].to_string(),
            href: c[2].to_string(),
        })
        .collect()
}

/// Project one document (its bundle `path` and raw `src` markdown) into a
/// lossless [`Concept`]. Known frontmatter fields promote to their dedicated
/// slots; unknown keys survive in [`Concept::extra`]; the body is verbatim.
fn project_source_document(document: &SourceDocument) -> Concept {
    let parsed = parse_frontmatter_spanned(document.text());
    let fm = parsed.frontmatter;
    let body = document
        .slice(parsed.body_range)
        .expect("frontmatter parser returns source boundaries");

    let title = fm
        .get_str("title")
        .map(String::from)
        .or_else(|| first_h1(body.as_str()));
    let description = fm.get_str("description").map(String::from);
    let resource = fm.get_str("resource").map(String::from);
    let timestamp = fm.get_str("timestamp").map(String::from);
    let tags = fm.get_string_list("tags");
    let ty = fm.get_str("type").unwrap_or("").to_string();

    let extra = Frontmatter {
        entries: fm
            .entries
            .iter()
            .filter(|(k, _)| !KNOWN_KEYS.contains(&k.as_str()))
            .cloned()
            .collect(),
    };

    let (prose, citations_section) = split_citations(body.as_str());
    let links = extract_links(prose);
    let citations = extract_citations(citations_section);

    Concept {
        id: document
            .path()
            .concept_id()
            .expect("validated source paths end in .md")
            .to_owned(),
        ty,
        title,
        description,
        resource,
        tags,
        timestamp,
        body,
        links,
        citations,
        extra,
    }
}

pub fn project_document(document: &SourceDocument) -> Option<Concept> {
    reserved_filename(document.path().as_str())
        .is_none()
        .then(|| project_source_document(document))
}

pub fn project(path: &str, src: &str) -> Concept {
    let (fm, body) = crate::frontmatter::parse_frontmatter(src);
    let body: SourceSlice = body.into();
    let title = fm
        .get_str("title")
        .map(String::from)
        .or_else(|| first_h1(body.as_str()));
    let description = fm.get_str("description").map(String::from);
    let resource = fm.get_str("resource").map(String::from);
    let timestamp = fm.get_str("timestamp").map(String::from);
    let tags = fm.get_string_list("tags");
    let ty = fm.get_str("type").unwrap_or("").to_string();
    let extra = Frontmatter {
        entries: fm
            .entries
            .iter()
            .filter(|(key, _)| !KNOWN_KEYS.contains(&key.as_str()))
            .cloned()
            .collect(),
    };
    let (prose, citations_section) = split_citations(body.as_str());
    let links = extract_links(prose);
    let citations = extract_citations(citations_section);

    Concept {
        id: id_of(path),
        ty,
        title,
        description,
        resource,
        tags,
        timestamp,
        body,
        links,
        citations,
        extra,
    }
}

struct AuthoredIndex {
    index: Index,
    authored_order: Vec<String>,
}

fn parse_authored_index(document: &SourceDocument, directory: DirectoryAddress) -> AuthoredIndex {
    let parsed = parse_frontmatter_spanned(document.text());
    let title_from_frontmatter = parsed
        .frontmatter
        .get_str("title")
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(str::to_owned);
    let body = document
        .slice(parsed.body_range)
        .expect("frontmatter parser returns source boundaries");
    let mut title = String::new();
    let mut intro_lines = Vec::new();
    let mut authored_order = Vec::new();
    let mut seen_bullet = false;
    let bullet = Regex::new(r"^\s*[*-]\s*\[[^\]]*\]\(([^)]+)\)(?:\s*-\s*(.*))?$").unwrap();
    for line in body.lines() {
        if let Some(captures) = bullet.captures(line) {
            seen_bullet = true;
            let url = captures.get(1).expect("captured index URL").as_str();
            let member = if let Some(subdirectory) = url.strip_suffix('/') {
                let name = subdirectory.trim_start_matches("./").trim_end_matches('/');
                directory
                    .join_directory(name)
                    .map(|address| address.to_string())
                    .unwrap_or_default()
            } else {
                resolve_href(&document.path().to_string(), url)
            };
            if !member.is_empty() {
                authored_order.push(member);
            }
        } else if !seen_bullet {
            let trimmed = line.trim();
            if let Some(heading) = trimmed.strip_prefix("# ") {
                if title.is_empty() {
                    title = heading.trim().to_owned();
                }
            } else if !trimmed.is_empty() {
                intro_lines.push(trimmed);
            }
        }
    }

    AuthoredIndex {
        index: Index {
            directory,
            title: title_from_frontmatter.or((!title.is_empty()).then_some(title)),
            description: (!intro_lines.is_empty()).then(|| intro_lines.join(" ")),
            members: Vec::new(),
            body: Some(body),
            authored: true,
        },
        authored_order,
    }
}

fn default_member_order(directory: &Directory, concepts: &[Concept]) -> Vec<String> {
    let mut members = Vec::new();
    for child in &directory.child_directories {
        let label = child.as_str().rsplit('/').next().unwrap_or(child.as_str());
        members.push((label.to_lowercase(), child.to_string()));
    }
    for id in &directory.concepts {
        let concept = concepts
            .iter()
            .find(|concept| concept.id == *id)
            .expect("directory concept exists");
        let label = concept
            .title
            .as_deref()
            .unwrap_or_else(|| id.rsplit('/').next().unwrap_or(id));
        members.push((label.to_lowercase(), id.clone()));
    }
    members.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
    members.into_iter().map(|(_, id)| id).collect()
}

fn parse_bundle(source: &SourceBundle) -> Result<Bundle, BundleError> {
    let mut addresses = BTreeSet::from([DirectoryAddress("/".into())]);
    for document in source.documents() {
        let mut address = DirectoryAddress::from_source_path(document.path().as_str());
        loop {
            addresses.insert(address.clone());
            let Some(parent) = address.parent() else {
                break;
            };
            address = parent;
        }
    }

    let mut concepts = Vec::new();
    let mut concept_paths = BTreeMap::new();
    let mut authored_indexes = BTreeMap::new();
    let mut logs = Vec::new();
    for document in source.documents() {
        let path = document.path().as_str();
        let directory = DirectoryAddress::from_source_path(path);
        match reserved_filename(path) {
            Some("index.md") => {
                authored_indexes
                    .insert(directory.clone(), parse_authored_index(document, directory));
            }
            Some("log.md") => {
                let parsed = parse_frontmatter_spanned(document.text());
                logs.push(Log {
                    directory,
                    body: document
                        .slice(parsed.body_range)
                        .expect("frontmatter parser returns source boundaries"),
                });
            }
            _ => {
                let concept = project_source_document(document);
                if let Some(first_path) = concept_paths.insert(concept.id.clone(), path.to_owned())
                {
                    return Err(BundleError::DuplicateConceptId {
                        id: concept.id,
                        first_path,
                        second_path: path.to_owned(),
                    });
                }
                concepts.push(concept);
            }
        }
    }
    concepts.sort_by(|left, right| left.id.cmp(&right.id));
    logs.sort_by(|left, right| left.directory.cmp(&right.directory));

    let mut directories: Vec<Directory> = addresses
        .iter()
        .map(|address| {
            let mut child_directories: Vec<_> = addresses
                .iter()
                .filter(|candidate| candidate.parent().as_ref() == Some(address))
                .cloned()
                .collect();
            child_directories.sort();
            let mut direct_concepts: Vec<_> = concepts
                .iter()
                .filter(|concept| {
                    DirectoryAddress::concept_parent(&concept.id).as_ref() == Ok(address)
                })
                .map(|concept| concept.id.clone())
                .collect();
            direct_concepts.sort();
            Directory {
                address: address.clone(),
                parent: address.parent(),
                child_directories,
                concepts: direct_concepts,
            }
        })
        .collect();
    directories.sort_by(|left, right| left.address.cmp(&right.address));

    let mut indexes = Vec::new();
    for directory in &directories {
        let default_order = default_member_order(directory, &concepts);
        match authored_indexes.remove(&directory.address) {
            Some(mut authored) => {
                let mut members = Vec::new();
                for member in authored.authored_order {
                    if default_order.contains(&member) && !members.contains(&member) {
                        members.push(member);
                    }
                }
                for member in default_order {
                    if !members.contains(&member) {
                        members.push(member);
                    }
                }
                authored.index.members = members;
                indexes.push(authored.index);
            }
            None => indexes.push(Index {
                directory: directory.address.clone(),
                title: None,
                description: None,
                members: default_order,
                body: None,
                authored: false,
            }),
        }
    }

    Ok(Bundle {
        concepts,
        indexes,
        logs,
        directories,
    })
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
