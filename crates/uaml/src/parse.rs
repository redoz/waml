use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::frontmatter::parse_frontmatter;
use crate::grammar::{
    parse_attribute_line, parse_hint_line, parse_relationship_line,
    parse_value_line,
};
use crate::syntax::{Document, Section};

use std::collections::{HashMap, HashSet};

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
        "members" => Section::Members(crate::grammar::parse_members_block(content)),
        "render hints" => {
            Section::RenderHints(lines(content).iter().filter_map(|l| parse_hint_line(l)).collect())
        }
        "body" => Section::Body(content.trim().to_string()),
        "notes" => {
            Section::Notes(lines(content).iter().filter_map(|l| parse_value_line(l)).collect())
        }
        "layout" => Section::Layout(
            lines(content).iter().filter_map(|l| crate::layout::parse_layout_line(l)).collect(),
        ),
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
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?m)^<!--\s*(.+?\.md)\s*-->[ \t]*\n").unwrap());

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

use crate::model::{AssocName, RelationshipKind};
use crate::syntax::ParsedName;

fn build_edges(classifiers: &[&ParsedDoc], keyset: &HashSet<&str>) -> Vec<Edge> {
    let mut edges: Vec<Edge> = Vec::new();
    let mut assoc_pair: HashMap<(String, String), usize> = HashMap::new();
    let mut seen_other: HashSet<(String, String, String)> = HashSet::new();

    for p in classifiers {
        let from = &p.slug;
        for s in &p.doc.sections {
            let Section::Relationships(rels) = s else { continue };
            for r in rels {
                let to = &r.target_slug;
                if !keyset.contains(to.as_str()) || to == from {
                    continue;
                }
                let name = match &r.name {
                    None => None,
                    Some(ParsedName::Label(l)) => Some(AssocName::Label(l.clone())),
                    Some(ParsedName::Ref { slug, .. }) => {
                        keyset.contains(slug.as_str()).then(|| AssocName::Assoc(slug.clone()))
                    }
                };

                if r.kind == RelationshipKind::Associates {
                    let mut pair = [from.clone(), to.clone()];
                    pair.sort();
                    let key = (pair[0].clone(), pair[1].clone());
                    if let Some(&idx) = assoc_pair.get(&key) {
                        let e = &mut edges[idx];
                        e.bidirectional = true;
                        e.from_end.navigable = Some(true);
                        e.to_end.navigable = Some(true);
                        if e.name.is_none() && name.is_some() {
                            e.name = name;
                        }
                        continue;
                    }
                    let mut to_end = r.to_end.clone();
                    to_end.navigable = Some(true);
                    edges.push(Edge {
                        source: from.clone(),
                        target: to.clone(),
                        kind: RelationshipKind::Associates,
                        name,
                        from_end: r.from_end.clone(),
                        to_end,
                        bidirectional: false,
                    });
                    assoc_pair.insert(key, edges.len() - 1);
                } else {
                    let dedup = (r.kind.as_str().to_string(), from.clone(), to.clone());
                    if !seen_other.insert(dedup) {
                        continue;
                    }
                    edges.push(Edge {
                        source: from.clone(),
                        target: to.clone(),
                        kind: r.kind,
                        name,
                        from_end: r.from_end.clone(),
                        to_end: r.to_end.clone(),
                        bidirectional: false,
                    });
                }
            }
        }
    }
    edges
}

use crate::syntax::HintLine;

fn build_diagrams(parsed: &[ParsedDoc], keyset: &HashSet<&str>) -> Vec<Diagram> {
    let mut out = Vec::new();
    for p in parsed.iter().filter(|p| p.ty == ClassifierType::Diagram) {
        let fm = &p.doc.frontmatter;
        let title = fm.get_str("title").map(String::from).unwrap_or_else(|| "Untitled diagram".to_string());
        let profile = fm
            .get_str("profile")
            .filter(|s| !s.is_empty())
            .unwrap_or("uml-domain")
            .to_string();

        let mut members = Vec::new();
        let mut hints = RenderHints::default();
        for s in &p.doc.sections {
            match s {
                Section::Members(block) => {
                    fn collect(g: &crate::syntax::MemberGroup, keyset: &HashSet<&str>, out: &mut Vec<Member>) {
                        for m in &g.members {
                            if keyset.contains(m.slug.as_str()) {
                                out.push(Member { key: m.slug.clone() });
                            }
                        }
                        for c in &g.children {
                            collect(c, keyset, out);
                        }
                    }
                    for g in &block.groups {
                        collect(g, keyset, &mut members);
                    }
                }
                Section::RenderHints(hs) => {
                    for h in hs {
                        match h {
                            HintLine::Emphasize(list) => hints.emphasize = list.clone(),
                            HintLine::Collapse { slug, .. } => {
                                if keyset.contains(slug.as_str()) {
                                    hints.collapse.push(slug.clone());
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        out.push(Diagram { key: p.slug.clone(), title, profile, members, hints });
    }
    out
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

    #[test]
    fn stray_html_comment_is_not_a_bundle_marker() {
        // A lone HTML comment that is NOT a `.md` path marker (e.g. an
        // author's review note) must not be treated as a bundle split
        // point: it belongs to the surrounding document, not a new one.
        let blob = "# Order\n\nSome intro text.\n\n<!-- reviewed: TODO -->\n\nMore text after the comment.\n";
        let parts = split_bundle(blob);
        assert_eq!(parts.len(), 1, "a stray non-.md comment must not split the blob");
        assert_eq!(parts[0].0, "pasted/doc.md");
        assert!(parts[0].1.contains("Some intro text."), "content before the comment must be kept");
        assert!(parts[0].1.contains("More text after the comment."), "content after the comment must be kept");

        // A genuine `.md` marker must still split the blob.
        let real = "<!-- shop/x.md -->\n# X\n";
        let real_parts = split_bundle(real);
        assert_eq!(real_parts.len(), 1);
        assert_eq!(real_parts[0].0, "shop/x.md");
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

    fn rel_bundle() -> Vec<(String, String)> {
        vec![
            ("a/order.md".into(),
             "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- composes [OrderLine](./order-line.md): 1 to 1..* lines\n- associates [Customer](./customer.md): 1 to 1\n".into()),
            ("a/order-line.md".into(),
             "---\ntype: uml.Class\ntitle: OrderLine\n---\n# OrderLine\n".into()),
            ("a/customer.md".into(),
             "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n\n## Relationships\n- associates [Order](./order.md): 1 to 1\n".into()),
        ]
    }

    #[test]
    fn builds_composition_edge() {
        let m = build_model(&rel_bundle());
        let comp = m.edges.iter().find(|e| e.kind == crate::model::RelationshipKind::Composes).unwrap();
        assert_eq!(comp.source, "order");
        assert_eq!(comp.target, "order-line");
        assert_eq!(comp.to_end.role.as_deref(), Some("lines"));
        assert!(!comp.bidirectional);
    }

    #[test]
    fn reciprocal_associates_collapse_to_one_bidirectional_edge() {
        let m = build_model(&rel_bundle());
        let assocs: Vec<_> = m.edges.iter().filter(|e| e.kind == crate::model::RelationshipKind::Associates).collect();
        assert_eq!(assocs.len(), 1, "reciprocal associates must collapse to one edge");
        assert!(assocs[0].bidirectional);
        assert_eq!(assocs[0].from_end.navigable, Some(true));
        assert_eq!(assocs[0].to_end.navigable, Some(true));
    }

    #[test]
    fn skips_unresolved_targets() {
        let b = vec![("x/a.md".into(),
            "---\ntype: uml.Class\ntitle: A\n---\n# A\n\n## Relationships\n- depends [Ghost](./ghost.md)\n".into())];
        let m = build_model(&b);
        assert!(m.edges.is_empty());
    }

    fn diagram_bundle() -> Vec<(String, String)> {
        vec![
            ("d/order.md".into(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".into()),
            ("d/pricing.md".into(), "---\ntype: uml.Class\ntitle: Pricing\n---\n# Pricing\n".into()),
            ("d/orders-domain.md".into(),
             "---\ntype: Diagram\ntitle: Orders Domain\nprofile: uml-domain\n---\n# Orders Domain\n\n## Members\n- [Order](./order.md)\n- [Pricing](./pricing.md)\n- [Ghost](./ghost.md)\n\n## Render hints\n- emphasize: order\n- collapse [Pricing](./pricing.md)\n".into()),
        ]
    }

    #[test]
    fn builds_diagram_with_members_and_hints() {
        let m = build_model(&diagram_bundle());
        assert_eq!(m.nodes.len(), 2, "diagram doc is not a node");
        assert_eq!(m.diagrams.len(), 1);
        let d = &m.diagrams[0];
        assert_eq!(d.title, "Orders Domain");
        assert_eq!(d.profile, "uml-domain");
        // resolvable members only; ghost is dropped
        assert_eq!(d.members.iter().map(|x| x.key.as_str()).collect::<Vec<_>>(), vec!["order", "pricing"]);
        assert_eq!(d.hints.emphasize, vec!["order"]);
        assert_eq!(d.hints.collapse, vec!["pricing"]);
    }
}
