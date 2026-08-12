//! A pure markdown page for one classifier: prose identity, a definition list
//! of properties, and every relationship written as a directional sentence.
//!
//! Model in, markdown out. No editor dependency — a CLI subcommand can emit
//! the identical page.

use crate::model::RelationshipKind;
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
}
