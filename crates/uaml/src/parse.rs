use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::frontmatter::parse_frontmatter;
use crate::grammar::{
    parse_attribute_line, parse_hint_line, parse_member_line, parse_relationship_line,
    parse_value_line,
};
use crate::syntax::{Document, Section};

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
}
