use crate::frontmatter::render_frontmatter;
use crate::grammar::{
    render_attribute_line, render_members_block, render_relationship_line, render_slot_line,
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

fn render_line_value(l: &Line<String>) -> String {
    match l {
        Line::Parsed(v) => format!("- {v}"),
        Line::Error(e) => e.raw.clone(),
    }
}

fn section_order(s: &Section) -> u8 {
    match s {
        Section::Body(_) => 0,
        Section::Attributes(_) => 1,
        Section::Slots(_) => 2,
        Section::Values(_) => 3,
        Section::Relationships(_) => 4,
        Section::Notes(_) => 5,
        Section::Nodes(_) => 6,
        Section::Lifelines(_) => 7,
        Section::Messages(_) => 8,
        Section::Members(_) => 9,
        Section::Layout(_) => 10,
        Section::Unknown { .. } => 11,
    }
}

fn render_section(s: &Section) -> String {
    match s {
        Section::Body(body) => format!("## Body\n{body}"),
        Section::Attributes(attrs) => {
            let body = attrs
                .iter()
                .map(render_line_attr)
                .collect::<Vec<_>>()
                .join("\n");
            format!("## Attributes\n{body}")
        }
        Section::Slots(slots) => {
            let body = slots
                .iter()
                .map(|l| match l {
                    Line::Parsed(s) => render_slot_line(s),
                    Line::Error(e) => e.raw.clone(),
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!("## Slots\n{body}")
        }
        Section::Values(values) => {
            let body = values
                .iter()
                .map(render_line_value)
                .collect::<Vec<_>>()
                .join("\n");
            format!("## Values\n{body}")
        }
        Section::Relationships(rels) => {
            let body = rels
                .iter()
                .map(render_line_rel)
                .collect::<Vec<_>>()
                .join("\n");
            format!("## Relationships\n{body}")
        }
        Section::Notes(notes) => {
            let body = notes
                .iter()
                .map(render_line_value)
                .collect::<Vec<_>>()
                .join("\n");
            format!("## Notes\n{body}")
        }
        Section::Nodes(block) => crate::grammar::render_flow_block(block),
        Section::Lifelines(lines) => {
            let body = lines
                .iter()
                .map(|l| match l {
                    Line::Parsed(x) => crate::grammar::render_lifeline_line(x),
                    Line::Error(e) => e.raw.clone(),
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!("## Lifelines\n{body}")
        }
        Section::Messages(block) => crate::grammar::render_messages_block(block),
        Section::Members(block) => render_members_block(block),
        Section::Layout(items) => {
            let body = items
                .iter()
                .map(render_line_layout)
                .collect::<Vec<_>>()
                .join("\n");
            if body.is_empty() {
                "## Layout".to_string()
            } else {
                format!("## Layout\n{body}")
            }
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
    fn serialize_round_trips_slots_section() {
        let text = "---\ntype: uml.InstanceSpecification\ntitle: order42\n---\n\n# order42\n\n## Slots\n- id: \"ORD-42\"\n- status: PLACED\n- owner: [Ann](./ann.md)\n";
        let (doc, _) = crate::parse::parse(text);
        let once = serialize_document(&doc);
        assert_eq!(once, text, "## Slots must round-trip byte-identically");
    }

    #[test]
    fn serialize_round_trips_inline_instance_member() {
        let text = "---\ntype: Diagram\ntitle: Objects\nprofile: uml-domain\n---\n\n# Objects\n\n## Members\n- [Order](./order.md)\n- instance of [Order](./order.md) as order42 with id set to \"ORD-42\" and status set to PLACED\n";
        let (doc, _) = crate::parse::parse(text);
        assert_eq!(
            serialize_document(&doc),
            text,
            "inline instance member must round-trip byte-identically"
        );
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
    fn serialize_is_a_semantic_fixpoint_with_diagram_display_frontmatter() {
        // Regression guard: diagram docs round-trip through the generic
        // serialize_document/render_frontmatter path — no diagram-specific
        // serializer exists, so every authored display key must survive.
        let text = "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\ndescription: \"Notes\"\nshowAttributes: false\nshowType: false\nshowAttributeVisibility: false\nshowAttributeMultiplicity: false\nmaxAttributes: 6\nshowRoles: false\nshowCardinality: false\nshowLabels: true\nshowStereotype: false\nstereotypeFilter: [entity, valueObject]\nstereotypeColors: [\"entity:#ffedd5\"]\n---\n# D\n";
        let once = serialize_document(&parse_document(text));
        let twice = serialize_document(&parse_document(&once));
        assert_eq!(once, twice);
        assert!(once.contains("description: Notes"));
        assert!(once.contains("showAttributes: false"));
        assert!(once.contains("maxAttributes: 6"));
        assert!(once.contains("stereotypeFilter: [entity, valueObject]"));
        assert!(once.contains("stereotypeColors: [entity:#ffedd5]"));
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

    #[test]
    fn flow_document_serialize_is_a_semantic_fixpoint() {
        let src = "---\ntype: uml.StateMachine\ntitle: Order Lifecycle\ndescribes: [Order](./order.md)\n---\n# Order Lifecycle\n\n## Nodes\n\n### initial\n- transitions to Draft\n\n### Draft\n- on `place` when `items > 0` transitions to Placed\n\n#### Notes\n- Auto-expires after 24h.\n\n### decision Ready to ship?\n- when `paid and inStock` transitions to Ship\n- else transitions to Hold\n\n### object [Order](./order.md)\n\n### Ship\n- transitions to Deliver carries [Order](./order.md)\n\n### final\n";
        let once = serialize_document(&parse_document(src));
        let twice = serialize_document(&parse_document(&once));
        assert_eq!(once, twice);
        assert!(once.contains("### decision Ready to ship?"));
        assert!(once.contains("- else transitions to Hold"));
        assert!(once.contains("### object [Order](./order.md)"));
        assert!(once.contains("#### Notes\n- Auto-expires after 24h."));
    }

    #[test]
    fn sequence_document_serialize_is_a_semantic_fixpoint() {
        let src = "---\ntype: uml.Sequence\ntitle: Place Order\ndescribes: [Place Order](./place-order.md)\n---\n# Place Order\n\n## Lifelines\n- [Customer](./customer.md)\n- [Order](./order.md) as order\n- [Warehouse](./warehouse.md) as wh\n\n## Messages\n- Customer calls order: `place(items)`\n- alt\n  - when `paid`\n    - order calls wh: `ship()`\n  - else\n    - order sends Customer: `paymentFailed()`\n- order replies Customer: `confirmation`\n";
        let once = serialize_document(&parse_document(src));
        let twice = serialize_document(&parse_document(&once));
        assert_eq!(once, twice);
        assert!(once
            .contains("## Lifelines\n- [Customer](./customer.md)\n- [Order](./order.md) as order"));
        assert!(once.contains("- alt\n  - when `paid`\n    - order calls wh: `ship()`"));
    }
}
