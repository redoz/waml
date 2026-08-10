//! Seed generators for freshly created documents (the New Package flow's
//! "Diagram" tier). Emits one empty diagram document per UML diagram kind, with
//! the title set but no members, so the user starts from a blank, valid diagram.

use crate::frontmatter::{FmValue, Frontmatter};

/// `(type, profile)` for a diagram kind token. Unknown tokens fall back to the
/// class/domain form. Kept as a small table so adding a kind is one line.
fn kind_frontmatter(kind: &str) -> (&'static str, Option<&'static str>) {
    match kind {
        "class" | "domain" => ("uml.ClassDiagram", Some("uml-domain")),
        "usecase" => ("uml.UseCaseDiagram", None),
        "activity" => ("uml.ActivityDiagram", None),
        "state-machine" => ("uml.StateMachineDiagram", None),
        "sequence" => ("uml.SequenceDiagram", None),
        _ => ("uml.ClassDiagram", Some("uml-domain")),
    }
}

/// Markdown for one empty diagram document of `kind`, titled `name`. No members;
/// canonical frontmatter followed by the document title.
pub fn new_diagram_doc(kind: &str, name: &str) -> String {
    let (ty, profile) = kind_frontmatter(kind);
    let mut entries: Vec<(String, FmValue)> = vec![("type".into(), FmValue::Str(ty.to_string()))];
    if let Some(p) = profile {
        entries.push(("profile".into(), FmValue::Str(p.to_string())));
    }
    entries.push(("title".into(), FmValue::Str(name.to_string())));
    let frontmatter = crate::frontmatter::render_frontmatter(&Frontmatter { entries });
    format!("---\n{frontmatter}\n---\n# {name}\n")
}

#[cfg(test)]
mod tests {
    use super::new_diagram_doc;
    use crate::{analysis::prepare_candidate, source::SourceBundle};

    fn assert_kind_type(kind: &str, expected_type: &str) {
        let md = new_diagram_doc(kind, "Example");
        assert!(md.contains(&format!("type: {expected_type}")), "got: {md}");
    }

    #[test]
    fn class_kind_emits_canonical_class_diagram_type() {
        let md = new_diagram_doc("class", "My Domain");
        assert!(md.contains("type: uml.ClassDiagram"), "got: {md}");
        assert!(md.contains("title: My Domain"), "got: {md}");
        assert!(md.contains("# My Domain"), "H1 present: {md}");
    }

    #[test]
    fn domain_kind_emits_canonical_class_diagram_type() {
        assert_kind_type("domain", "uml.ClassDiagram");
    }

    #[test]
    fn usecase_kind_emits_canonical_use_case_diagram_type() {
        assert_kind_type("usecase", "uml.UseCaseDiagram");
    }

    #[test]
    fn activity_kind_emits_canonical_activity_diagram_type() {
        assert_kind_type("activity", "uml.ActivityDiagram");
    }

    #[test]
    fn state_machine_kind_emits_canonical_state_machine_diagram_type() {
        assert_kind_type("state-machine", "uml.StateMachineDiagram");
    }

    #[test]
    fn sequence_kind_emits_canonical_sequence_diagram_type() {
        assert_kind_type("sequence", "uml.SequenceDiagram");
    }

    #[test]
    fn unknown_kind_falls_back_to_class_form() {
        let md = new_diagram_doc("nonsense", "X");
        assert!(md.contains("type: uml.ClassDiagram"), "got: {md}");
    }

    #[test]
    fn seeded_diagram_is_ingested_by_parser_platform_analysis() {
        // A seeded class diagram at some path must project without panicking and
        // parse as a diagram doc (empty members list is fine).
        let md = new_diagram_doc("class", "My Domain");
        let source = SourceBundle::try_from_pairs([("pkg/my-domain.md", md)]).unwrap();
        let prepared = prepare_candidate(source, None, 0).unwrap();
        let model = &prepared.uml().projection;
        assert!(
            model.diagrams.iter().any(|d| d.title == "My Domain"),
            "diagram present: {:?}",
            model.diagrams
        );
    }
}
