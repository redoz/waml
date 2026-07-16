//! Seed generators for freshly created documents (the New Package flow's
//! "Diagram" tier). Emits one empty diagram document per UML diagram kind, with
//! the title set but no members, so the user starts from a blank, valid diagram.

use crate::frontmatter::{FmValue, Frontmatter};
use crate::serialize::serialize_document;
use crate::syntax::Document;

/// `(type, profile)` for a diagram kind token. Unknown tokens fall back to the
/// class/domain form. Kept as a small table so adding a kind is one line.
fn kind_frontmatter(kind: &str) -> (&'static str, Option<&'static str>) {
    match kind {
        "activity" => ("uml.Activity", None),
        "sequence" => ("uml.Sequence", None),
        // "class" | "domain" | "usecase" | anything else
        _ => ("Diagram", Some("uml-domain")),
    }
}

/// Markdown for one empty diagram document of `kind`, titled `name`. No members;
/// canonical formatting via `serialize_document`.
pub fn new_diagram_doc(kind: &str, name: &str) -> String {
    let (ty, profile) = kind_frontmatter(kind);
    let mut entries: Vec<(String, FmValue)> = vec![("type".into(), FmValue::Str(ty.to_string()))];
    if let Some(p) = profile {
        entries.push(("profile".into(), FmValue::Str(p.to_string())));
    }
    entries.push(("title".into(), FmValue::Str(name.to_string())));
    let doc = Document {
        frontmatter: Frontmatter { entries },
        title: name.to_string(),
        sections: Vec::new(),
    };
    serialize_document(&doc)
}

#[cfg(test)]
mod tests {
    use super::new_diagram_doc;
    use crate::parse::build_model;

    #[test]
    fn class_kind_emits_diagram_type_and_uml_domain_profile() {
        let md = new_diagram_doc("class", "My Domain");
        assert!(md.contains("type: \"Diagram\""), "got: {md}");
        assert!(md.contains("profile: \"uml-domain\""), "got: {md}");
        assert!(md.contains("title: \"My Domain\""), "got: {md}");
        assert!(md.contains("# My Domain"), "H1 present: {md}");
    }

    #[test]
    fn usecase_kind_emits_diagram_type_and_uml_domain_profile() {
        let md = new_diagram_doc("usecase", "Actors");
        assert!(md.contains("type: \"Diagram\""), "got: {md}");
        assert!(md.contains("profile: \"uml-domain\""), "got: {md}");
        assert!(md.contains("title: \"Actors\""), "got: {md}");
    }

    #[test]
    fn activity_kind_emits_uml_activity_type_and_no_profile() {
        let md = new_diagram_doc("activity", "Checkout");
        assert!(md.contains("type: \"uml.Activity\""), "got: {md}");
        assert!(!md.contains("profile:"), "activity has no profile: {md}");
        assert!(md.contains("title: \"Checkout\""), "got: {md}");
    }

    #[test]
    fn sequence_kind_emits_uml_sequence_type() {
        let md = new_diagram_doc("sequence", "Place Order");
        assert!(md.contains("type: \"uml.Sequence\""), "got: {md}");
        assert!(md.contains("title: \"Place Order\""), "got: {md}");
    }

    #[test]
    fn unknown_kind_falls_back_to_class_form() {
        let md = new_diagram_doc("nonsense", "X");
        assert!(md.contains("type: \"Diagram\""), "got: {md}");
        assert!(md.contains("profile: \"uml-domain\""), "got: {md}");
    }

    #[test]
    fn seeded_diagram_is_a_valid_document_that_build_model_ingests() {
        // A seeded class diagram at some path must project without panicking and
        // parse as a diagram doc (empty members list is fine).
        let md = new_diagram_doc("class", "My Domain");
        let bundle = vec![("pkg/my-domain.md".to_string(), md)];
        let model = build_model(&bundle);
        assert!(model.diagrams.iter().any(|d| d.title == "My Domain"), "diagram present: {:?}", model.diagrams);
    }
}
