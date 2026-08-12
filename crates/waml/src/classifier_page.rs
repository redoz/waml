//! A pure markdown page for one classifier: prose identity, a definition list
//! of properties, and every relationship written as a directional sentence.
//!
//! Model in, markdown out. No editor dependency — a CLI subcommand can emit
//! the identical page.

use crate::model::{
    Attribute, ElementType, Model, Node, RelationshipKind, UmlMetaclass, Visibility,
};
use crate::multiplicity::Multiplicity;

/// Cardinal numbers spelled out through ten; above ten prose reads worse than
/// digits, so digits win.
// Wired into the association sentences in Task 4, via `spell_multiplicity`.
#[allow(dead_code)]
fn number_word(n: u64) -> String {
    const WORDS: [&str; 11] = [
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
    ];
    match WORDS.get(n as usize) {
        Some(word) => (*word).to_string(),
        None => n.to_string(),
    }
}

/// A UML multiplicity as English. `None` when `raw` is not a multiplicity this
/// crate can parse — the caller then omits the count entirely rather than
/// printing notation into a sentence.
// Wired into the association sentences in Task 4.
#[allow(dead_code)]
fn spell_multiplicity(raw: &str) -> Option<String> {
    let parsed = Multiplicity::parse(raw)?;
    let raw = parsed.as_str();
    if raw == "*" {
        return Some("zero or more".to_string());
    }
    let Some((lo, hi)) = raw.split_once("..") else {
        // An exact count. `1` is the ordinary case and reads as a plain
        // article; anything else is worth calling out.
        let n: u64 = raw.parse().ok()?;
        return Some(if n == 1 {
            "one".to_string()
        } else {
            format!("exactly {}", number_word(n))
        });
    };
    let lo: u64 = lo.parse().ok()?;
    if hi == "*" {
        return Some(match lo {
            0 => "zero or more".to_string(),
            1 => "one or more".to_string(),
            lo => format!("{} or more", number_word(lo)),
        });
    }
    let hi: u64 = hi.parse().ok()?;
    if lo == 0 && hi == 1 {
        return Some("zero or one".to_string());
    }
    Some(format!("{} to {}", number_word(lo), number_word(hi)))
}

/// The link a classifier's page is written to. Absolute from the bundle root:
/// a node key IS its concept id, so `/{key}.md` resolves the same from any
/// referring directory (`waml-editor`'s `navigation::resolve_link` normalises
/// a leading `/` against the bundle root).
// Wired in via `far_end_phrase`, which is itself wired into the association
// sections in Task 4.
#[allow(dead_code)]
fn document_href(key: &str) -> String {
    format!("/{key}.md")
}

/// The far end's noun phrase. The classifier name is the link text either way;
/// a declared role leads, with the classifier in parentheses behind it. A role
/// spelled exactly like the classifier adds nothing, so it collapses.
///
/// Class names are never inflected: a plural count beside a singular name
/// ("one or more Wheel") is deliberate. The name is an identifier and must
/// match the model exactly; an author who wants a plural noun declares a role,
/// which is their own text.
// Wired into the association sections in Task 4.
#[allow(dead_code)]
fn far_end_phrase(role: Option<&str>, classifier: &str, key: &str) -> String {
    let link = format!("[{classifier}]({})", document_href(key));
    match role {
        Some(role) if !role.is_empty() && role != classifier => format!("{role} ({link})"),
        _ => link,
    }
}

/// The verb under `## Associations`, where this classifier is the elided
/// subject. `Associates` is the one kind that shifts register: it is not a
/// transitive verb in ordinary English ("Associates one Customer" reads as a
/// typo), so its elided form is the participial "Associated with".
// Wired into the association sections in Task 4.
#[allow(dead_code)]
fn outgoing_verb(kind: RelationshipKind) -> &'static str {
    match kind {
        RelationshipKind::Associates => "Associated with",
        RelationshipKind::Aggregates => "Aggregates",
        RelationshipKind::Composes => "Composes",
        RelationshipKind::Specializes => "Specializes",
        RelationshipKind::Implements => "Implements",
        RelationshipKind::Depends => "Depends on",
        RelationshipKind::Includes => "Includes",
        RelationshipKind::Extends => "Extends",
        RelationshipKind::InstanceOf => "Instance of",
        RelationshipKind::Links => "Links to",
        RelationshipKind::Annotates => {
            unreachable!("Annotates anchors a uml.Note and is skipped before any verb lookup")
        }
    }
}

/// The verb under `## Referenced by`, where the FAR classifier is the named
/// subject and this one is the object.
// Wired into the association sections in Task 4.
#[allow(dead_code)]
fn incoming_verb(kind: RelationshipKind) -> &'static str {
    match kind {
        RelationshipKind::Associates => "is associated with",
        RelationshipKind::Aggregates => "aggregates",
        RelationshipKind::Composes => "composes",
        RelationshipKind::Specializes => "specializes",
        RelationshipKind::Implements => "implements",
        RelationshipKind::Depends => "depends on",
        RelationshipKind::Includes => "includes",
        RelationshipKind::Extends => "extends",
        RelationshipKind::InstanceOf => "is an instance of",
        RelationshipKind::Links => "links to",
        RelationshipKind::Annotates => {
            unreachable!("Annotates anchors a uml.Note and is skipped before any verb lookup")
        }
    }
}

/// The classifier's own page, as markdown. `None` when `key` names no node.
///
/// Sections emit in a fixed order and each is omitted when it would be empty:
/// title, dek, description, properties (or values), associations, then
/// referenced by.
pub fn classifier_page(model: &Model, key: &str) -> Option<String> {
    let node = model.node(key)?;
    let mut sections: Vec<String> = Vec::new();

    let title = node
        .concept
        .title
        .clone()
        .unwrap_or_else(|| key.to_string());
    sections.push(format!("# {title}"));

    if let Some(dek) = dek_line(node) {
        sections.push(dek);
    }
    if let Some(description) = node
        .concept
        .description
        .as_deref()
        .map(str::trim)
        .filter(|description| !description.is_empty())
    {
        sections.push(description.to_string());
    }
    if let Some(members) = member_section(node) {
        sections.push(members);
    }
    // `concept.body` is deliberately not emitted: it is the whole source
    // document, so echoing it would repeat every section above.

    Some(format!("{}\n", sections.join("\n\n")))
}

/// Kind label, stereotypes as guillemet names, then `abstract`. `None` when a
/// node somehow carries none of the three.
fn dek_line(node: &Node) -> Option<String> {
    let mut parts = vec![kind_label(&node.ty)];
    parts.extend(
        node.stereotypes
            .iter()
            .map(|stereotype| format!("«{stereotype}»")),
    );
    if node.abstract_ {
        parts.push("abstract".to_string());
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// The metaclass' own name (`Class`, `Interface`, `Enum`, `DataType`), without
/// the `uml.` family prefix. Mirrors the inspector's `kind_label`.
fn kind_label(ty: &ElementType) -> String {
    match ty {
        ElementType::Uml(metaclass) => metaclass.name().to_string(),
        other => {
            let text = other.as_str();
            text.strip_prefix("uml.").unwrap_or(&text).to_string()
        }
    }
}

/// `## Values` for an enum, `## Properties` for every other classifier.
/// `None` when the node declares neither.
fn member_section(node: &Node) -> Option<String> {
    if node.ty == ElementType::Uml(UmlMetaclass::Enum) {
        if node.values.is_empty() {
            return None;
        }
        let bullets: Vec<String> = node
            .values
            .iter()
            .map(|value| format!("- `{value}`"))
            .collect();
        return Some(format!("## Values\n\n{}", bullets.join("\n")));
    }
    if node.attributes.is_empty() {
        return None;
    }
    let bullets: Vec<String> = node.attributes.iter().map(property_bullet).collect();
    Some(format!("## Properties\n\n{}", bullets.join("\n")))
}

/// One property. Name and type always; multiplicity only when it is not a bare
/// `1` (a definition list is scanned down a column, where a repeated `1` on
/// every row is noise); visibility only when declared; a description as an
/// indented continuation line under the bullet.
fn property_bullet(attribute: &Attribute) -> String {
    let mut line = format!("- `{}` · `{}`", attribute.name, attribute.ty.name);
    if let Some(multiplicity) = attribute
        .multiplicity
        .as_ref()
        .map(|multiplicity| multiplicity.as_str())
        .filter(|multiplicity| *multiplicity != "1")
    {
        line.push_str(&format!(" `{multiplicity}`"));
    }
    if let Some(visibility) = attribute.visibility {
        line.push_str(&format!(" — {}", visibility_word(visibility)));
    }
    if let Some(description) = attribute
        .description
        .as_deref()
        .map(str::trim)
        .filter(|description| !description.is_empty())
    {
        line.push_str(&format!("\n  {description}"));
    }
    line
}

fn visibility_word(visibility: Visibility) -> &'static str {
    match visibility {
        Visibility::Public => "public",
        Visibility::Private => "private",
        Visibility::Protected => "protected",
        Visibility::Package => "package",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_of_the_prose_table_spells_out() {
        let cases = [
            ("1", "one"),
            ("0..1", "zero or one"),
            ("1..*", "one or more"),
            ("0..*", "zero or more"),
            ("*", "zero or more"),
            ("3", "exactly three"),
            ("2..5", "two to five"),
            ("2..*", "two or more"),
        ];
        for (raw, expected) in cases {
            assert_eq!(
                spell_multiplicity(raw).as_deref(),
                Some(expected),
                "multiplicity {raw}"
            );
        }
    }

    #[test]
    fn numbers_spell_out_through_ten_and_show_digits_above_it() {
        assert_eq!(number_word(0), "zero");
        assert_eq!(number_word(10), "ten");
        assert_eq!(number_word(11), "11");
        // Both boundaries, as read through the speller itself.
        assert_eq!(spell_multiplicity("10").as_deref(), Some("exactly ten"));
        assert_eq!(spell_multiplicity("11").as_deref(), Some("exactly 11"));
    }

    #[test]
    fn an_unparseable_multiplicity_spells_nothing() {
        // `Multiplicity::parse` rejects each of these, so the sentence must
        // omit the count rather than invent one.
        for raw in ["", "0", "many", "1..", "5..2", "-1"] {
            assert_eq!(spell_multiplicity(raw), None, "multiplicity {raw:?}");
        }
    }

    #[test]
    fn a_far_end_without_a_role_is_just_the_linked_classifier() {
        assert_eq!(far_end_phrase(None, "Wheel", "wheel"), "[Wheel](/wheel.md)");
    }

    #[test]
    fn a_declared_role_leads_and_the_classifier_follows_in_parentheses() {
        assert_eq!(
            far_end_phrase(Some("lines"), "OrderLine", "sales/order-line"),
            "lines ([OrderLine](/sales/order-line.md))"
        );
    }

    #[test]
    fn a_role_identical_to_the_classifier_is_not_repeated() {
        // "Customer (Customer)" says nothing twice.
        assert_eq!(
            far_end_phrase(Some("Customer"), "Customer", "customer"),
            "[Customer](/customer.md)"
        );
    }

    #[test]
    fn every_kind_has_both_a_subject_elided_and_a_named_subject_verb() {
        use crate::model::RelationshipKind as RK;
        let cases = [
            (RK::Associates, "Associated with", "is associated with"),
            (RK::Aggregates, "Aggregates", "aggregates"),
            (RK::Composes, "Composes", "composes"),
            (RK::Specializes, "Specializes", "specializes"),
            (RK::Implements, "Implements", "implements"),
            (RK::Depends, "Depends on", "depends on"),
            (RK::Includes, "Includes", "includes"),
            (RK::Extends, "Extends", "extends"),
            (RK::InstanceOf, "Instance of", "is an instance of"),
            (RK::Links, "Links to", "links to"),
        ];
        for (kind, out, incoming) in cases {
            assert_eq!(outgoing_verb(kind), out, "{kind:?} outgoing");
            assert_eq!(incoming_verb(kind), incoming, "{kind:?} incoming");
        }
    }

    use crate::model::{Attribute, Model, Node, TypeRef};
    use crate::multiplicity::Multiplicity;
    use crate::source::SourceBundle;

    /// The projection of a small in-test bundle — the same path the editor
    /// installs (`prepare_candidate` -> `uml().projection`).
    fn projection(pairs: &[(&str, &str)]) -> Model {
        let source = SourceBundle::try_from_pairs(
            pairs
                .iter()
                .map(|(path, text)| ((*path).to_string(), (*text).to_string())),
        )
        .expect("fixture bundle parses");
        crate::analysis::prepare_candidate(source, None, 0)
            .expect("fixture analyses")
            .uml()
            .projection
            .clone()
    }

    #[test]
    fn a_missing_key_has_no_page() {
        let model = projection(&[(
            "order.md",
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
        )]);
        assert_eq!(classifier_page(&model, "nope"), None);
    }

    #[test]
    fn the_head_carries_title_kind_stereotypes_abstract_and_description() {
        let model = projection(&[(
            "order.md",
            "---\ntype: uml.Class\ntitle: Purchase Order\ndescription: One customer order.\nstereotype: [aggregateRoot, entity]\nabstract: true\n---\n# Order\n",
        )]);
        let page = classifier_page(&model, "order").expect("the node has a page");
        assert_eq!(
            page,
            "# Purchase Order\n\nClass · «aggregateRoot» · «entity» · abstract\n\nOne customer order.\n"
        );
    }

    #[test]
    fn a_title_less_node_falls_back_to_its_key() {
        let model = projection(&[("order.md", "---\ntype: uml.Class\n---\n")]);
        let page = classifier_page(&model, "order").expect("the node has a page");
        assert!(page.starts_with("# order\n"), "page was:\n{page}");
    }

    #[test]
    fn properties_show_type_always_and_multiplicity_only_when_it_is_not_one() {
        let model = projection(&[(
            "order.md",
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId {1}\n- -total: Decimal\n- lines: OrderLine {1..*}\n",
        )]);
        let page = classifier_page(&model, "order").expect("the node has a page");
        assert!(page.contains("## Properties\n\n"), "page was:\n{page}");
        assert!(page.contains("- `id` · `OrderId`\n"), "page was:\n{page}");
        assert!(
            page.contains("- `total` · `Decimal` — private\n"),
            "page was:\n{page}"
        );
        assert!(
            page.contains("- `lines` · `OrderLine` `1..*`\n"),
            "page was:\n{page}"
        );
    }

    #[test]
    fn an_attribute_description_is_an_indented_continuation_line() {
        // Lowering never sets `Attribute::description` today, so build one.
        let mut model = projection(&[(
            "order.md",
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
        )]);
        let node: &mut Node = model
            .nodes
            .iter_mut()
            .find(|node| node.key == "order")
            .expect("the fixture node");
        node.attributes.push(Attribute {
            name: "lines".into(),
            ty: TypeRef {
                name: "OrderLine".into(),
                ref_: None,
            },
            multiplicity: Multiplicity::parse("1..*"),
            visibility: None,
            description: Some("The line items on the order.".into()),
        });
        let page = classifier_page(&model, "order").expect("the node has a page");
        assert!(
            page.contains("- `lines` · `OrderLine` `1..*`\n  The line items on the order.\n"),
            "page was:\n{page}"
        );
    }

    #[test]
    fn every_visibility_marker_has_a_word() {
        let model = projection(&[(
            "order.md",
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- +a: A\n- -b: B\n- #c: C\n- ~d: D\n",
        )]);
        let page = classifier_page(&model, "order").expect("the node has a page");
        for expected in [
            "- `a` · `A` — public\n",
            "- `b` · `B` — private\n",
            "- `c` · `C` — protected\n",
            "- `d` · `D` — package\n",
        ] {
            assert!(page.contains(expected), "missing {expected:?} in:\n{page}");
        }
    }

    #[test]
    fn an_enum_renders_values_in_place_of_properties() {
        let model = projection(&[(
            "state.md",
            "---\ntype: uml.Enum\ntitle: State\n---\n# State\n\n## Values\n- OPEN\n- Ready for use\n",
        )]);
        let page = classifier_page(&model, "state").expect("the node has a page");
        assert!(page.contains("## Values\n\n"), "page was:\n{page}");
        assert!(page.contains("- `OPEN`\n"), "page was:\n{page}");
        assert!(page.contains("- `Ready for use`\n"), "page was:\n{page}");
        assert!(
            !page.contains("## Properties"),
            "an enum must not also emit Properties:\n{page}"
        );
    }

    /// `concept.body` is the WHOLE markdown body, so echoing it would
    /// duplicate everything the page just rendered in prose. This guards that
    /// omission — it is deliberate, not an oversight.
    #[test]
    fn the_page_does_not_echo_the_source_document_back() {
        let model = projection(&[(
            "order.md",
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId {1}\n",
        )]);
        let page = classifier_page(&model, "order").expect("the node has a page");
        assert!(
            !page.contains("## Attributes"),
            "the authored UML section must not be pasted back in:\n{page}"
        );
    }
}
