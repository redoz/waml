use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::frontmatter::parse_frontmatter;
use crate::grammar::{
    parse_attribute_line, parse_hint_line, parse_member_line, parse_relationship_line,
    parse_value_line,
};
use crate::syntax::{Document, Section};

#[allow(unused_imports)]
use std::collections::{HashMap, HashSet};

#[allow(unused_imports)]
use crate::model::{
    Attribute, ClassifierType, Diagram, Edge, Member, Model, Node, RenderHints,
};

struct Head {
    title: String,
    heading_start: usize,
    content_start: usize,
}

fn classify(title: &str, content: &str, raw_full: &str) -> Section {
    let lines = |c: &str| c.lines().map(|l| l.to_string()).collect::<Vec<_>>();
    match title.to_lowercase().as_str() {
        "attributes" => {
            Section::Attributes(lines(content).iter().filter_map(|l| parse_attribute_line(l)).collect())
        }
        "values" => {
            Section::Values(lines(content).iter().filter_map(|l| parse_value_line(l)).collect())
        }
        "relationships" => {
            Section::Relationships(lines(content).iter().filter_map(|l| parse_relationship_line(l)).collect())
        }
        "members" => {
            Section::Members(lines(content).iter().filter_map(|l| parse_member_line(l)).collect())
        }
        "render hints" => {
            Section::RenderHints(lines(content).iter().filter_map(|l| parse_hint_line(l)).collect())
        }
        "body" => Section::Body(content.trim().to_string()),
        "notes" => {
            Section::Notes(lines(content).iter().filter_map(|l| parse_value_line(l)).collect())
        }
        _ => Section::Unknown { title: title.to_string(), raw: raw_full.trim_end().to_string() },
    }
}

pub fn parse_document(src: &str) -> Document {
    let (frontmatter, body) = parse_frontmatter(src);
    let parser = Parser::new_ext(&body, Options::empty()).into_offset_iter();

    let mut title = String::new();
    let mut in_h1 = false;
    let mut in_h2 = false;
    let mut cur_title = String::new();
    let mut pending_start = 0usize;
    let mut pending_heading_start = 0usize;
    let mut heads: Vec<Head> = Vec::new();

    for (ev, range) in parser {
        match ev {
            Event::Start(Tag::Heading { level: HeadingLevel::H1, .. }) => in_h1 = true,
            Event::End(TagEnd::Heading(HeadingLevel::H1)) => in_h1 = false,
            Event::Start(Tag::Heading { level: HeadingLevel::H2, .. }) => {
                in_h2 = true;
                cur_title = String::new();
                pending_heading_start = range.start;
                pending_start = range.end;
            }
            Event::End(TagEnd::Heading(HeadingLevel::H2)) => {
                in_h2 = false;
                heads.push(Head {
                    title: cur_title.trim().to_string(),
                    heading_start: pending_heading_start,
                    content_start: pending_start,
                });
            }
            Event::Text(t) | Event::Code(t) => {
                if in_h1 {
                    title.push_str(&t);
                } else if in_h2 {
                    cur_title.push_str(&t);
                }
            }
            _ => {}
        }
    }

    let mut sections = Vec::new();
    for (i, head) in heads.iter().enumerate() {
        let end = heads.get(i + 1).map(|h| h.heading_start).unwrap_or(body.len());
        let content = body[head.content_start..end].trim();
        let raw_full = &body[head.heading_start..end];
        sections.push(classify(&head.title, content, raw_full));
    }

    Document { frontmatter, title: title.trim().to_string(), sections }
}

static MARKER_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?m)^<!--\s*(.+?)\s*-->[ \t]*\n").unwrap());

/// Split a concatenated bundle blob into `(path, content)` pairs on
/// `<!-- path/slug.md -->` markers. An unmarked blob is a single document.
pub fn split_bundle(text: &str) -> Vec<(String, String)> {
    let mut marks: Vec<(usize, usize, String)> = Vec::new(); // (marker_start, content_start, path)
    for m in MARKER_RE.captures_iter(text) {
        let whole = m.get(0).unwrap();
        let path = m[1].to_string();
        marks.push((whole.start(), whole.end(), path));
    }
    if marks.is_empty() {
        return vec![("pasted/doc.md".to_string(), text.to_string())];
    }
    let mut out = Vec::new();
    for (i, (_, content_start, path)) in marks.iter().enumerate() {
        let end = marks.get(i + 1).map(|m| m.0).unwrap_or(text.len());
        out.push((path.clone(), text[*content_start..end].to_string()));
    }
    out
}

/// A classifier's filename slug (the node key): last path segment, `.md` stripped.
fn doc_slug(path: &str) -> String {
    let seg = path.rsplit(['/', '\\']).next().unwrap_or(path);
    seg.strip_suffix(".md").unwrap_or(seg).to_string()
}

struct ParsedDoc {
    slug: String,
    ty: ClassifierType,
    doc: Document,
}

fn parse_bundle(bundle: &[(String, String)]) -> Vec<ParsedDoc> {
    bundle
        .iter()
        .map(|(path, text)| {
            let doc = parse_document(text);
            let ty = ClassifierType::parse(doc.frontmatter.get_str("type").unwrap_or("uml.Class"));
            ParsedDoc { slug: doc_slug(path), ty, doc }
        })
        .collect()
}

fn resolve_attr(attr: &Attribute, keyset: &HashSet<&str>) -> Attribute {
    let mut a = attr.clone();
    if let Some(slug) = &a.ty.ref_ {
        if !keyset.contains(slug.as_str()) {
            a.ty.ref_ = None; // degrade to a bare token
        }
    }
    a
}

fn build_node(p: &ParsedDoc, keyset: &HashSet<&str>) -> Node {
    let fm = &p.doc.frontmatter;
    let title = fm.get_str("title").map(String::from).unwrap_or_else(|| {
        if p.doc.title.is_empty() { "Untitled".to_string() } else { p.doc.title.clone() }
    });
    let mut attributes = Vec::new();
    let mut values = Vec::new();
    let mut body = None;
    for s in &p.doc.sections {
        match s {
            Section::Attributes(a) => attributes = a.iter().map(|x| resolve_attr(x, keyset)).collect(),
            Section::Values(v) => values = v.clone(),
            Section::Body(b) => body = Some(b.clone()),
            _ => {}
        }
    }
    Node {
        key: p.slug.clone(),
        ty: p.ty.clone(),
        title,
        stereotypes: fm.get_string_list("stereotype"),
        abstract_: fm.get_bool("abstract") == Some(true),
        description: fm.get_str("description").map(String::from),
        attributes,
        values,
        body,
        annotates: Vec::new(), // deferred: uml.Note anchors
    }
}

pub fn build_model(bundle: &[(String, String)]) -> Model {
    let parsed = parse_bundle(bundle);
    let classifiers: Vec<&ParsedDoc> =
        parsed.iter().filter(|p| p.ty != ClassifierType::Diagram).collect();
    let keyset: HashSet<&str> = classifiers.iter().map(|p| p.slug.as_str()).collect();

    let nodes = classifiers.iter().map(|p| build_node(p, &keyset)).collect();
    let edges: Vec<Edge> = build_edges(&classifiers, &keyset);
    let diagrams: Vec<Diagram> = build_diagrams(&parsed, &keyset);

    Model { nodes, edges, diagrams }
}

// Filled in by Tasks 2 and 3; stubs keep the crate compiling now.
fn build_edges(_classifiers: &[&ParsedDoc], _keyset: &HashSet<&str>) -> Vec<Edge> {
    Vec::new()
}
fn build_diagrams(_parsed: &[ParsedDoc], _keyset: &HashSet<&str>) -> Vec<Diagram> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::RelationshipKind;

    const ORDER: &str = "---\ntype: uml.Class\nstereotype: [aggregateRoot, entity]\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n- status: [OrderStatus](./order-status.md) [0..1]\n\n## Relationships\n- composes [OrderLine](./order-line.md): 1 to 1..* lines\n\n## Provenance\nHand-authored. Keep me.\n";

    #[test]
    fn parses_frontmatter_title_and_known_sections() {
        let doc = parse_document(ORDER);
        assert_eq!(doc.frontmatter.get_str("title"), Some("Order"));
        assert_eq!(doc.title, "Order");
        let attrs = doc.sections.iter().find_map(|s| match s {
            Section::Attributes(a) => Some(a),
            _ => None,
        }).unwrap();
        assert_eq!(attrs.len(), 2);
        assert_eq!(attrs[1].ty.ref_.as_deref(), Some("order-status"));
        let rels = doc.sections.iter().find_map(|s| match s {
            Section::Relationships(r) => Some(r),
            _ => None,
        }).unwrap();
        assert_eq!(rels[0].kind, RelationshipKind::Composes);
    }

    #[test]
    fn preserves_unknown_section_verbatim() {
        let doc = parse_document(ORDER);
        let unknown = doc.sections.iter().find_map(|s| match s {
            Section::Unknown { title, raw } => Some((title.clone(), raw.clone())),
            _ => None,
        }).unwrap();
        assert_eq!(unknown.0, "Provenance");
        assert!(unknown.1.contains("Hand-authored. Keep me."));
        assert!(unknown.1.starts_with("## Provenance"));
    }

    #[test]
    fn ignores_headings_inside_code_fences() {
        let src = "# Doc\n\n## Body\n```\n## Not a section\n```\n";
        let doc = parse_document(src);
        // The fenced `## Not a section` must not open a section.
        assert_eq!(doc.sections.len(), 1);
        assert!(matches!(doc.sections[0], Section::Body(_)));
    }

    #[test]
    fn splits_blob_on_markers() {
        let blob = "<!-- shop/order.md -->\n# Order\n\n<!-- shop/customer.md -->\n# Customer\n";
        let parts = split_bundle(blob);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].0, "shop/order.md");
        assert!(parts[0].1.contains("# Order"));
        assert_eq!(parts[1].0, "shop/customer.md");
    }

    #[test]
    fn unmarked_blob_is_a_single_doc() {
        let parts = split_bundle("# Just one doc\n");
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].0, "pasted/doc.md");
    }
}

#[cfg(test)]
mod model_tests {
    use super::*;
    use crate::model::{ClassifierType, UmlMetaclass};

    fn bundle() -> Vec<(String, String)> {
        vec![
            ("shop/order.md".into(),
             "---\ntype: uml.Class\nstereotype: [aggregateRoot, entity]\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n- status: [OrderStatus](./order-status.md) [0..1]\n- ghost: [Missing](./missing.md)\n".into()),
            ("shop/order-status.md".into(),
             "---\ntype: uml.Enum\ntitle: OrderStatus\n---\n# OrderStatus\n\n## Values\n- DRAFT\n- PLACED\n".into()),
        ]
    }

    #[test]
    fn builds_classifier_nodes() {
        let m = build_model(&bundle());
        assert_eq!(m.nodes.len(), 2);
        let order = m.node("order").unwrap();
        assert_eq!(order.title, "Order");
        assert_eq!(order.ty, ClassifierType::Uml(UmlMetaclass::Class));
        assert_eq!(order.stereotypes, vec!["aggregateRoot", "entity"]);
        assert_eq!(order.attributes.len(), 3);
    }

    #[test]
    fn resolves_and_degrades_attribute_refs() {
        let m = build_model(&bundle());
        let order = m.node("order").unwrap();
        // resolvable link keeps its ref
        assert_eq!(order.attributes[1].ty.ref_.as_deref(), Some("order-status"));
        // unresolvable link degrades to a bare token (ref dropped), name preserved
        assert_eq!(order.attributes[2].ty.name, "Missing");
        assert_eq!(order.attributes[2].ty.ref_, None);
    }

    #[test]
    fn collects_enum_values() {
        let m = build_model(&bundle());
        assert_eq!(m.node("order-status").unwrap().values, vec!["DRAFT", "PLACED"]);
    }
}
