use crate::frontmatter::render_frontmatter;
use crate::grammar::{
    render_attribute_line, render_hint_line, render_member_line, render_relationship_line,
};
use crate::syntax::{Document, Section};

fn section_order(s: &Section) -> u8 {
    match s {
        Section::Body(_) => 0,
        Section::Attributes(_) => 1,
        Section::Values(_) => 2,
        Section::Relationships(_) => 3,
        Section::Notes(_) => 4,
        Section::Members(_) => 5,
        Section::RenderHints(_) => 6,
        Section::Unknown { .. } => 7,
    }
}

fn render_section(s: &Section) -> String {
    match s {
        Section::Body(body) => format!("## Body\n{body}"),
        Section::Attributes(attrs) => {
            let body = attrs.iter().map(render_attribute_line).collect::<Vec<_>>().join("\n");
            format!("## Attributes\n{body}")
        }
        Section::Values(values) => {
            let body = values.iter().map(|v| format!("- {v}")).collect::<Vec<_>>().join("\n");
            format!("## Values\n{body}")
        }
        Section::Relationships(rels) => {
            let body = rels.iter().map(render_relationship_line).collect::<Vec<_>>().join("\n");
            format!("## Relationships\n{body}")
        }
        Section::Notes(notes) => {
            let body = notes.iter().map(|n| format!("- {n}")).collect::<Vec<_>>().join("\n");
            format!("## Notes\n{body}")
        }
        Section::Members(members) => {
            let body = members.iter().map(render_member_line).collect::<Vec<_>>().join("\n");
            format!("## Members\n{body}")
        }
        Section::RenderHints(hints) => {
            let body = hints.iter().map(render_hint_line).collect::<Vec<_>>().join("\n");
            format!("## Render hints\n{body}")
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

    const ORDER: &str = "---\ntype: uml.Class\nstereotype: [aggregateRoot, entity]\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n- status: [OrderStatus](./order-status.md) [0..1]\n\n## Relationships\n- composes [OrderLine](./order-line.md): 1 to 1..* lines\n\n## Provenance\nHand-authored. Keep me.\n";

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
        assert!(out.contains("- status: [OrderStatus](./order-status.md) [0..1]"));
    }
}
