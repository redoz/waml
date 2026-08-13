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

/// A character no destination can carry raw, whichever form it takes.
///
/// `<` is rejected outright inside the angle-bracket form (waml-syntax
/// `inline_destination` scans the RAW slice, so `\<` does not help), `>` ends
/// that form, and a newline ends the inline itself. All three are illegal in a
/// filename on every mainstream filesystem, so a key carrying one cannot
/// round-trip to disk either.
///
/// `#` and `?` are different: they parse fine and reach the resolver intact,
/// and that is the problem. `navigation::resolve_link` rejects any href holding
/// a `?`, and splits the href at the first `#` into path + fragment -- so a raw
/// one truncates the path the destination was built to carry. A key like
/// `foo.md#bar` is the bad case: `/foo.md#bar.md` splits to the path `/foo.md`,
/// which strips to the UNRELATED concept `foo` and navigates there silently.
///
/// Percent-encoding keeps the destination whole in every case: the link stays
/// clickable and the resolver reports an honest miss instead of swallowing the
/// rest of the sentence or opening the wrong document.
fn is_unrepresentable_in_a_destination(ch: char) -> bool {
    matches!(ch, '<' | '>' | '#' | '?') || ch.is_control()
}

/// The bare destination form ends at the first whitespace and needs balanced
/// parentheses; `BundlePath::parse` permits both, so such a key must take the
/// angle-bracket form or the link renders as literal text.
fn needs_angle_brackets(ch: char) -> bool {
    ch.is_whitespace() || ch == '(' || ch == ')'
}

/// The link a classifier's page is written to. Absolute from the bundle root:
/// a node key IS its concept id, so `/{key}.md` resolves the same from any
/// referring directory (`waml-editor`'s `navigation::resolve_link` normalises
/// a leading `/` against the bundle root).
///
/// The returned text is a complete markdown destination, angle-bracketed when
/// the key needs it -- a key is a bundle path, and a bundle path may hold
/// spaces and parentheses.
fn document_href(key: &str) -> String {
    let mut path = String::with_capacity(key.len() + 4);
    path.push('/');
    for ch in key.chars() {
        if is_unrepresentable_in_a_destination(ch) {
            let mut buffer = [0u8; 4];
            for byte in ch.encode_utf8(&mut buffer).as_bytes() {
                path.push_str(&format!("%{byte:02X}"));
            }
        } else {
            path.push(ch);
        }
    }
    path.push_str(".md");
    if path.contains(needs_angle_brackets) {
        format!("<{path}>")
    } else {
        path
    }
}

/// Link-label text with the punctuation that would corrupt the label escaped.
/// A stray `]` closes the label early and a stray `[` opens a span the label
/// never closes; either way the link degrades to literal text. The escape
/// backslash is a syntax marker, so the reading view never draws it.
fn escape_label(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if matches!(ch, '[' | ']' | '\\') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// The far end's noun phrase. The classifier name is the link text either way;
/// a declared role leads, with the classifier in parentheses behind it. A role
/// spelled exactly like the classifier adds nothing, so it collapses.
///
/// Class names are never inflected: a plural count beside a singular name
/// ("one or more Wheel") is deliberate. The name is an identifier and must
/// match the model exactly; an author who wants a plural noun declares a role,
/// which is their own text.
fn far_end_phrase(role: Option<&str>, classifier: &str, key: &str) -> String {
    let link = format!("[{}]({})", escape_label(classifier), document_href(key));
    match role {
        Some(role) if !role.is_empty() && role != classifier => {
            format!("{} ({link})", escape_label(role))
        }
        _ => link,
    }
}

/// The verb under `## Associations`, where this classifier is the elided
/// subject. `Associates` is the one kind that shifts register: it is not a
/// transitive verb in ordinary English ("Associates one Customer" reads as a
/// typo), so its elided form is the participial "Associated with".
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
    if let Some(associations) = association_section(model, node) {
        sections.push(associations);
    }
    if let Some(referenced_by) = referenced_by_section(model, node) {
        sections.push(referenced_by);
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

/// Both ends navigable — declared reciprocally, or flagged during resolution.
/// Such an edge renders ONCE, under `## Associations`, whichever side of it
/// this classifier sits on.
fn is_bidirectional(edge: &crate::model::Edge) -> bool {
    edge.bidirectional
        || (edge.from_end.navigable == Some(true) && edge.to_end.navigable == Some(true))
}

/// The classifier's title for a key, or the key when it names no node.
fn node_title(model: &Model, key: &str) -> String {
    model
        .node(key)
        .and_then(|node| node.concept.title.clone())
        .unwrap_or_else(|| key.to_string())
}

/// Outgoing relationships, plus every bidirectional edge that touches this
/// classifier from either side. The subject is this classifier and is elided.
fn association_section(model: &Model, node: &Node) -> Option<String> {
    let mut bullets: Vec<String> = Vec::new();
    for edge in &model.edges {
        if edge.kind == RelationshipKind::Annotates {
            continue;
        }
        let outgoing = edge.source == node.key;
        let incoming = edge.target == node.key;
        if !outgoing && !(incoming && is_bidirectional(edge)) {
            continue;
        }
        let (far_end, far_key) = if outgoing {
            (&edge.to_end, &edge.target)
        } else {
            (&edge.from_end, &edge.source)
        };
        let phrase = far_end_phrase(
            far_end.role.as_deref(),
            &node_title(model, far_key),
            far_key,
        );
        let count = far_end
            .multiplicity
            .as_ref()
            .and_then(|multiplicity| spell_multiplicity(multiplicity.as_str()));
        let subject = match count {
            Some(count) => format!("{count} {phrase}"),
            None => phrase,
        };
        let tail = if is_bidirectional(edge) {
            " (both ways)"
        } else {
            ""
        };
        bullets.push(format!("- {} {subject}{tail}.", outgoing_verb(edge.kind)));
    }
    (!bullets.is_empty()).then(|| format!("## Associations\n\n{}", bullets.join("\n")))
}

/// Incoming relationships, with the FAR classifier as the named subject and
/// this one as the object. A bidirectional edge already rendered under
/// `## Associations` and must not repeat here.
fn referenced_by_section(model: &Model, node: &Node) -> Option<String> {
    let title = node
        .concept
        .title
        .clone()
        .unwrap_or_else(|| node.key.clone());
    let mut bullets: Vec<String> = Vec::new();
    for edge in &model.edges {
        if edge.kind == RelationshipKind::Annotates || is_bidirectional(edge) {
            continue;
        }
        if edge.target != node.key || edge.source == node.key {
            continue;
        }
        let subject = far_end_phrase(None, &node_title(model, &edge.source), &edge.source);
        bullets.push(format!("- {subject} {} {title}.", incoming_verb(edge.kind)));
    }
    (!bullets.is_empty()).then(|| format!("## Referenced by\n\n{}", bullets.join("\n")))
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

    /// `BundlePath::parse` permits spaces and parentheses, and the bare
    /// destination form ends at the first whitespace -- so a key that carries
    /// either must go out angle-bracketed or the whole link renders as
    /// un-clickable literal text.
    #[test]
    fn a_key_the_bare_destination_form_cannot_carry_is_angle_bracketed() {
        assert_eq!(
            far_end_phrase(None, "Order", "my order"),
            "[Order](</my order.md>)"
        );
        assert_eq!(
            far_end_phrase(None, "Order", "orders (draft)"),
            "[Order](</orders (draft).md>)"
        );
        // Nothing special in the key: the plain form stays plain.
        assert_eq!(far_end_phrase(None, "Order", "order"), "[Order](/order.md)");
    }

    /// `<` and `>` cannot appear in ANY destination form, so they are
    /// percent-encoded rather than emitted raw (which would truncate the
    /// destination and swallow the rest of the sentence).
    #[test]
    fn a_key_no_destination_form_can_carry_is_percent_encoded() {
        assert_eq!(
            far_end_phrase(None, "Order", "a<b>c"),
            "[Order](/a%3Cb%3Ec.md)"
        );
    }

    /// `#` and `?` reach the resolver intact, and that is the problem: it
    /// rejects any href holding a `?`, and splits the href at the first `#`
    /// into path + fragment. A raw one therefore truncates the path -- and a
    /// key like `foo.md#bar` truncates it to a DIFFERENT document that exists.
    #[test]
    fn a_key_the_resolver_would_read_structurally_is_percent_encoded() {
        assert_eq!(
            far_end_phrase(None, "Note", "notes#1"),
            "[Note](/notes%231.md)"
        );
        assert_eq!(far_end_phrase(None, "Query", "q?x"), "[Query](/q%3Fx.md)");
        // The silent mis-navigation: `/foo.md#bar.md` splits to the path
        // `/foo.md`, which strips to the unrelated concept `foo`.
        assert_eq!(
            far_end_phrase(None, "Foo", "foo.md#bar"),
            "[Foo](/foo.md%23bar.md)"
        );
    }

    /// The one that matters: the emitted page, put back through the markdown
    /// parser, must still hold a LINK whose destination is the far
    /// classifier's document. The bare form breaks at the space and then
    /// demands a `)`, so before angle-bracketing this page parsed with no
    /// links at all.
    #[test]
    fn a_link_to_a_spaced_key_survives_a_round_trip_through_the_parser() {
        use waml_syntax::{parse_markdown, DocumentRevision, MarkdownDialect, SourceText};

        use crate::model::{Edge, RelEnd};

        let mut model = projection(&[("car.md", "---\ntype: uml.Class\ntitle: Car\n---\n# Car\n")]);
        // Built by hand: a relationship AUTHORED against a spaced path hits
        // the same destination-form problem in the source document, which is
        // the author's own escape to make, not this generator's.
        model.edges.push(Edge {
            source: "car".into(),
            target: "front wheel".into(),
            kind: RelationshipKind::Aggregates,
            name: None,
            from_end: RelEnd::default(),
            to_end: RelEnd::default(),
            bidirectional: false,
        });
        let page = classifier_page(&model, "car").expect("the node has a page");
        let text = SourceText::new(page.clone()).expect("the page is valid source text");
        let snapshot = parse_markdown(
            DocumentRevision::INITIAL,
            text,
            MarkdownDialect::WAML_DEFAULT,
        )
        .expect("the generated page parses");
        let destinations: Vec<String> = snapshot
            .queries()
            .links()
            .map(|link| link.destination.to_string())
            .collect();
        assert_eq!(destinations, vec!["/front wheel.md".to_string()], "{page}");
    }

    #[test]
    fn brackets_in_a_title_or_role_are_escaped_so_the_label_survives() {
        assert_eq!(
            far_end_phrase(None, "Order [draft]", "order"),
            "[Order \\[draft\\]](/order.md)"
        );
        assert_eq!(
            far_end_phrase(Some("lines ]"), "OrderLine", "order-line"),
            "lines \\] ([OrderLine](/order-line.md))"
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

    #[test]
    fn outgoing_relationships_elide_the_subject_and_spell_the_count() {
        let model = projection(&[
            (
                "car.md",
                "---\ntype: uml.Class\ntitle: Car\n---\n# Car\n\n## Relationships\n- specializes [Vehicle](./vehicle.md)\n- depends [Fuel](./fuel.md)\n- aggregates [Wheel](./wheel.md): 1 to *\n- composes [Engine](./engine.md): 1 to 1 engine\n",
            ),
            ("vehicle.md", "---\ntype: uml.Class\ntitle: Vehicle\n---\n"),
            ("fuel.md", "---\ntype: uml.Class\ntitle: Fuel\n---\n"),
            ("wheel.md", "---\ntype: uml.Class\ntitle: Wheel\n---\n"),
            ("engine.md", "---\ntype: uml.Class\ntitle: Engine\n---\n"),
        ]);
        let page = classifier_page(&model, "car").expect("the node has a page");
        assert!(page.contains("## Associations\n\n"), "page was:\n{page}");
        for expected in [
            "- Specializes [Vehicle](/vehicle.md).\n",
            "- Depends on [Fuel](/fuel.md).\n",
            "- Aggregates zero or more [Wheel](/wheel.md).\n",
            "- Composes one engine ([Engine](/engine.md)).\n",
        ] {
            assert!(page.contains(expected), "missing {expected:?} in:\n{page}");
        }
    }

    #[test]
    fn incoming_relationships_name_the_far_classifier_as_the_subject() {
        let model = projection(&[
            ("car.md", "---\ntype: uml.Class\ntitle: Car\n---\n# Car\n"),
            (
                "special-order.md",
                "---\ntype: uml.Class\ntitle: SpecialOrder\n---\n# SpecialOrder\n\n## Relationships\n- specializes [Car](./car.md)\n",
            ),
            (
                "shipping-label.md",
                "---\ntype: uml.Class\ntitle: ShippingLabel\n---\n# ShippingLabel\n\n## Relationships\n- depends [Car](./car.md)\n",
            ),
        ]);
        let page = classifier_page(&model, "car").expect("the node has a page");
        assert!(page.contains("## Referenced by\n\n"), "page was:\n{page}");
        for expected in [
            "- [SpecialOrder](/special-order.md) specializes Car.\n",
            "- [ShippingLabel](/shipping-label.md) depends on Car.\n",
        ] {
            assert!(page.contains(expected), "missing {expected:?} in:\n{page}");
        }
        assert!(
            !page.contains("## Associations"),
            "car declares no relationships of its own:\n{page}"
        );
    }

    #[test]
    fn a_note_anchor_is_not_a_relationship() {
        let model = projection(&[
            (
                "order.md",
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- annotates [Aside](./aside.md)\n",
            ),
            ("aside.md", "---\ntype: uml.Note\ntitle: Aside\n---\n"),
        ]);
        let page = classifier_page(&model, "order").expect("the node has a page");
        assert!(
            !page.contains("## Associations") && !page.contains("## Referenced by"),
            "an Annotates anchor must produce no association section:\n{page}"
        );
    }

    #[test]
    fn a_bidirectional_edge_renders_once_under_associations() {
        use crate::model::{Edge, RelEnd};
        let mut model = projection(&[
            (
                "order.md",
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
            ),
            (
                "customer.md",
                "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n",
            ),
        ]);
        // Lowering never sets `bidirectional` today, so build the edge.
        model.edges.push(Edge {
            source: "customer".into(),
            target: "order".into(),
            kind: RelationshipKind::Associates,
            name: None,
            from_end: RelEnd {
                multiplicity: Multiplicity::parse("1"),
                role: None,
                navigable: Some(true),
            },
            to_end: RelEnd {
                multiplicity: Multiplicity::parse("1..*"),
                role: None,
                navigable: Some(true),
            },
            bidirectional: true,
        });
        let page = classifier_page(&model, "order").expect("the node has a page");
        assert!(
            page.contains("- Associated with one [Customer](/customer.md) (both ways).\n"),
            "page was:\n{page}"
        );
        assert!(
            !page.contains("## Referenced by"),
            "a bidirectional edge must not also appear as incoming:\n{page}"
        );
    }
}
