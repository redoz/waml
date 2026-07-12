use crate::frontmatter::render_frontmatter;
use crate::grammar::{
    render_attribute_line, render_members_block, render_relationship_line,
};
use crate::model::Attribute;
use crate::syntax::{Document, LayoutItem, Line, ParsedRel, Section};

fn render_line_attr(l: &Line<Attribute>) -> String {
    match l {
        Line::Parsed(a) => render_attribute_line(a),
        Line::Error(e) => e.raw.clone(),
    }
}

fn render_line_rel(l: &Line<ParsedRel>) -> String {
    match l {
        Line::Parsed(r) => render_relationship_line(r),
        Line::Error(e) => e.raw.clone(),
    }
}

fn render_line_layout(l: &Line<LayoutItem>) -> String {
    match l {
        Line::Parsed(it) => crate::layout::render_layout_line(&it.stmt),
        Line::Error(e) => e.raw.clone(),
    }
}

fn section_order(s: &Section) -> u8 {
    match s {
        Section::Body(_) => 0,
        Section::Attributes(_) => 1,
        Section::Values(_) => 2,
        Section::Relationships(_) => 3,
        Section::Notes(_) => 4,
        Section::Members(_) => 5,
        Section::Layout(_) => 6,
        Section::Unknown { .. } => 7,
    }
}

fn render_section(s: &Section) -> String {
    match s {
        Section::Body(body) => format!("## Body\n{body}"),
        Section::Attributes(attrs) => {
            let body = attrs.iter().map(render_line_attr).collect::<Vec<_>>().join("\n");
            format!("## Attributes\n{body}")
        }
        Section::Values(values) => {
            let body = values.iter().map(|v| format!("- {v}")).collect::<Vec<_>>().join("\n");
            format!("## Values\n{body}")
        }
        Section::Relationships(rels) => {
            let body = rels.iter().map(render_line_rel).collect::<Vec<_>>().join("\n");
            format!("## Relationships\n{body}")
        }
        Section::Notes(notes) => {
            let body = notes.iter().map(|n| format!("- {n}")).collect::<Vec<_>>().join("\n");
            format!("## Notes\n{body}")
        }
        Section::Members(block) => render_members_block(block),
        Section::Layout(items) => {
            let body = items
                .iter()
                .map(render_line_layout)
                .collect::<Vec<_>>()
                .join("\n");
            if body.is_empty() { "## Layout".to_string() } else { format!("## Layout\n{body}") }
        }
        Section::Unknown { raw, .. } => raw.trim_end().to_string(),
    }
}

pub fn serialize_document(doc: &Document) -> String {
    let mut ordered: Vec<&Section> = doc.sections.iter().collect();
    // Stable sort keeps Unknown sections in their original relative order.
    ordered.sort_by_key(|s| section_order(s));

    let fm = render_frontmatter(&doc.frontmatter);
    let mut out = format!("---\n{fm}\n---\n\n# {}\n", doc.title);
    for s in ordered {
        out.push('\n');
        out.push_str(&render_section(s));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_document;

    const ORDER: &str = "---\ntype: uml.Class\nstereotype: [aggregateRoot, entity]\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n- status: [OrderStatus](./order-status.md) {0..1}\n\n## Relationships\n- composes [OrderLine](./order-line.md): 1 to 1..* lines\n\n## Provenance\nHand-authored. Keep me.\n";

    #[test]
    fn serialize_is_a_semantic_fixpoint() {
        let once = serialize_document(&parse_document(ORDER));
        let twice = serialize_document(&parse_document(&once));
        assert_eq!(once, twice);
    }

    #[test]
    fn serialize_preserves_unknown_section() {
        let out = serialize_document(&parse_document(ORDER));
        assert!(out.contains("## Provenance\nHand-authored. Keep me."));
    }

    #[test]
    fn serialize_omits_default_multiplicity() {
        let out = serialize_document(&parse_document(ORDER));
        assert!(out.contains("- id: OrderId\n"));
        assert!(out.contains("- status: [OrderStatus](./order-status.md) {0..1}"));
    }

    #[test]
    fn serialize_is_a_semantic_fixpoint_with_nested_bracket_frontmatter() {
        // Regression for the fmt panic: a nested-bracket frontmatter value
        // (e.g. `stereotype: [a, [b]]`) must serialize without panicking, and
        // the whole document must still be a semantic fixpoint.
        let text = "---\ntype: uml.Class\ntitle: Order\nstereotype: [aggregateRoot, [nested]]\n---\n# Order\n\n## Attributes\n- id: OrderId\n";
        let once = serialize_document(&parse_document(text));
        let twice = serialize_document(&parse_document(&once));
        assert_eq!(once, twice);
    }

    #[test]
    fn render_hints_section_degrades_to_preserved_unknown() {
        let src = "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Render hints\n- emphasize: order\n";
        let out = serialize_document(&parse_document(src));
        // Preserved verbatim as an Unknown section, not silently dropped.
        assert!(out.contains("## Render hints\n- emphasize: order"));
    }

    #[test]
    fn serialize_round_trips_layout_section() {
        let src = "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Layout\n- Users left of Orders\n- top of Users aligned with top of Orders\n";
        let once = serialize_document(&parse_document(src));
        let twice = serialize_document(&parse_document(&once));
        assert_eq!(once, twice);
        assert!(once.contains("## Layout\n- Users left of Orders"));
    }
}
