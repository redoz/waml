//! OKF (Open Knowledge Format) — the domain-agnostic substrate beneath the UAML
//! UML profile. A [`Concept`] is the lossless projection of a single markdown
//! document; a [`Bundle`] holds one `Concept` per document in a bundle.
//!
//! Hard rule: this module MUST NOT import any UML type (`ClassifierType`,
//! `RelationshipKind`, `UmlMetaclass`, …). The dependency is one-way — the UAML
//! tier depends on `okf`, never the reverse — so a later `okf-core` crate split
//! stays mechanical.

use std::sync::LazyLock;

use regex::Regex;

use crate::frontmatter::{parse_frontmatter, Frontmatter};

/// Reserved-file role. Every document lands in the bundle regardless of role;
/// `index.md`/`log.md` are flagged so consumers can treat them specially.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum ConceptRole {
    Concept,
    Index,
    Log,
}

impl Default for ConceptRole {
    fn default() -> ConceptRole {
        ConceptRole::Concept
    }
}

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
    /// The free-text `type` frontmatter field (NOT the UML `ClassifierType`).
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
    pub body: String,
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
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "is_concept_role")
    )]
    pub role: ConceptRole,
    /// Producer-specific frontmatter keys with no dedicated field above.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "frontmatter_is_empty")
    )]
    pub extra: Frontmatter,
}

/// Every document projects to exactly one `Concept`; a `Bundle` stays flat.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Bundle {
    pub concepts: Vec<Concept>,
}

impl Bundle {
    pub fn concept(&self, id: &str) -> Option<&Concept> {
        self.concepts.iter().find(|c| c.id == id)
    }
}

#[cfg(feature = "serde")]
fn is_concept_role(r: &ConceptRole) -> bool {
    *r == ConceptRole::Concept
}

#[cfg(feature = "serde")]
fn frontmatter_is_empty(fm: &Frontmatter) -> bool {
    fm.entries.is_empty()
}

/// Frontmatter keys that project onto dedicated `Concept` fields (everything
/// else survives in `extra`).
const KNOWN_KEYS: &[&str] = &["type", "title", "description", "resource", "tags", "timestamp"];

static LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[([^\]]*)\]\(([^)]+)\)").unwrap());

/// The concept ID for a bundle path: full path with a trailing `.md` removed
/// and backslashes normalized to `/` (OKF §2).
pub fn id_of(path: &str) -> String {
    let norm = path.replace('\\', "/");
    norm.strip_suffix(".md").unwrap_or(&norm).to_string()
}

/// The reserved role implied by a path's filename.
fn role_of(path: &str) -> ConceptRole {
    let seg = path.rsplit(['/', '\\']).next().unwrap_or(path);
    match seg {
        "index.md" => ConceptRole::Index,
        "log.md" => ConceptRole::Log,
        _ => ConceptRole::Concept,
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

fn extract_links(text: &str) -> Vec<Link> {
    LINK_RE
        .captures_iter(text)
        .map(|c| Link { text: c[1].to_string(), href: c[2].to_string() })
        .collect()
}

fn extract_citations(text: &str) -> Vec<Citation> {
    LINK_RE
        .captures_iter(text)
        .map(|c| Citation { text: c[1].to_string(), href: c[2].to_string() })
        .collect()
}

/// Project one document (its bundle `path` and raw `src` markdown) into a
/// lossless [`Concept`]. Known frontmatter fields promote to their dedicated
/// slots; unknown keys survive in [`Concept::extra`]; the body is verbatim.
pub fn project(path: &str, src: &str) -> Concept {
    let (fm, body) = parse_frontmatter(src);

    let title = fm.get_str("title").map(String::from);
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

    let (prose, citations_section) = split_citations(&body);
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
        role: role_of(path),
        extra,
    }
}

/// Project an entire bundle. EVERY document lands as a `Concept`, regardless of
/// its `type` (UML or not) or reserved role.
pub fn build_bundle(bundle: &[(String, String)]) -> Bundle {
    Bundle {
        concepts: bundle.iter().map(|(path, src)| project(path, src)).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_strips_md_and_keeps_full_path() {
        assert_eq!(id_of("tables/orders.md"), "tables/orders");
        assert_eq!(id_of("orders.md"), "orders");
        assert_eq!(id_of("a\\b\\c.md"), "a/b/c");
    }

    #[test]
    fn role_reflects_reserved_filenames() {
        assert_eq!(role_of("a/index.md"), ConceptRole::Index);
        assert_eq!(role_of("log.md"), ConceptRole::Log);
        assert_eq!(role_of("tables/orders.md"), ConceptRole::Concept);
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
        assert_eq!(c.role, ConceptRole::Concept);
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
    fn uml_doc_also_projects_to_a_concept() {
        let src = "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n";
        let c = project("shop/order.md", src);
        assert_eq!(c.id, "shop/order");
        assert_eq!(c.ty, "uml.Class");
        assert_eq!(c.title.as_deref(), Some("Order"));
        assert!(c.body.contains("## Attributes"));
    }

    #[test]
    fn build_bundle_lands_every_doc_including_reserved() {
        let bundle = vec![
            ("index.md".to_string(), "# Root Index\n\n* [Order](order.md)\n".to_string()),
            ("order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
            ("log.md".to_string(), "# Update Log\n\n## 2026-05-22\n* Created.\n".to_string()),
        ];
        let b = build_bundle(&bundle);
        assert_eq!(b.concepts.len(), 3);
        assert_eq!(b.concept("index").unwrap().role, ConceptRole::Index);
        assert_eq!(b.concept("log").unwrap().role, ConceptRole::Log);
        assert_eq!(b.concept("order").unwrap().role, ConceptRole::Concept);
        // A reserved index has no frontmatter → empty free-text type.
        assert_eq!(b.concept("index").unwrap().ty, "");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn concept_serializes_with_type_field_and_omits_empty() {
        let c = project("shop/order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n");
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(v["type"], "uml.Class");
        assert_eq!(v["id"], "shop/order");
        assert_eq!(v["title"], "Order");
        // Empty collections/roles are omitted from the wire.
        assert!(v.get("tags").is_none());
        assert!(v.get("links").is_none());
        assert!(v.get("role").is_none());
        assert!(v.get("extra").is_none());
    }
}
