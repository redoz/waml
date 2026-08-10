//! Field extraction: projects one bundle (`SourceBundle` + `OkfAnalysis` +
//! `uml::Analysis`) into four field groups per document — the flat
//! [`FieldEntry`] list the in-memory index consumes. Per document: title +
//! headings become `Names` targeting a `TextSpan`; the UML classifier name (if
//! any) becomes `Names` targeting its `ModelElement`; kind, relationship
//! endpoint pairs, and tags become `Model` targeting `ModelElement`s; markdown
//! body text runs become `Prose` targeting `TextSpan`s (markup and code-fence
//! markers are skipped, and spans are deduped per document so an overlapping
//! raw/projected pass never double-counts one span); frontmatter keys, the
//! `id:` value, and link targets become `Structure`.

use std::collections::HashSet;

use waml_syntax::{MarkdownSemanticRole, MarkdownSourceRole, TextRange, TextSize};

use super::{FieldGroup, HitTarget};
use crate::{
    analysis::{DocumentVersion, OkfAnalysis},
    frontmatter::FmValue,
    source::{SourceBundle, SourceDocument},
};

#[derive(Clone, Debug, PartialEq)]
pub struct FieldEntry {
    pub group: FieldGroup,
    /// Raw text to index; the tokenizer runs over this.
    pub text: String,
    /// Where a match on this entry lands.
    pub target: HitTarget,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DocumentFields {
    pub path: String,
    pub concept_id: Option<String>,
    pub title: String,
    /// UML kind of the primary element, lowercased ("class", "actor", …),
    /// for the `kind:` filter. `None` for plain markdown.
    pub kind: Option<String>,
    pub entries: Vec<FieldEntry>,
}

/// Extracts every document in `source` into its four field groups. Documents
/// with no admitted concept (reserved filenames such as `index.md`/`log.md`,
/// or a quarantined shell) contribute nothing.
pub fn extract_bundle(
    source: &SourceBundle,
    okf: &OkfAnalysis,
    uml: &crate::uml::Analysis,
) -> Vec<DocumentFields> {
    source
        .documents()
        .iter()
        .filter_map(|document| extract_document(document, okf, uml))
        .collect()
}

fn line_of(version: &DocumentVersion, offset: TextSize) -> u32 {
    version
        .line_index()
        .line_col(version.text(), offset)
        .map(|column| column.line + 1)
        .unwrap_or(1)
}

fn extract_document(
    document: &SourceDocument,
    okf: &OkfAnalysis,
    uml: &crate::uml::Analysis,
) -> Option<DocumentFields> {
    let path = document.path().as_str().to_string();
    let concept_id = document.path().concept_id()?.to_string();
    let concept = okf.bundle.concept(&concept_id)?;
    let node = uml.projection.node(&concept_id);

    let kind = node.and_then(|node| node.ty.as_str().strip_prefix("uml.").map(str::to_lowercase));

    let document_id = okf.catalog.id_for_path(document.path())?;
    let version = okf.catalog.document(document_id)?;
    let snapshot = okf.markdown_snapshot(document_id)?;
    let full = TextRange::new(TextSize::new(0), snapshot.text().len()).ok()?;
    let headings: Vec<_> = snapshot.queries().headings().cloned().collect();

    let mut fields = DocumentFields {
        path,
        concept_id: Some(concept_id.clone()),
        title: concept.title.clone().unwrap_or_default(),
        kind,
        entries: Vec::new(),
    };

    // Names: every heading targets its own TextSpan; the classifier name (when
    // `uml` claims the concept) additionally targets the ModelElement.
    for heading in &headings {
        let text = snapshot.text().slice(heading.content_range).unwrap_or("");
        if text.trim().is_empty() {
            continue;
        }
        fields.entries.push(FieldEntry {
            group: FieldGroup::Names,
            text: text.to_string(),
            target: HitTarget::TextSpan {
                start: heading.content_range.start().to_usize() as u32,
                end: heading.content_range.end().to_usize() as u32,
                line: line_of(version, heading.content_range.start()),
            },
        });
    }
    if let Some(node) = node {
        let name = node
            .concept
            .title
            .clone()
            .unwrap_or_else(|| concept_id.clone());
        fields.entries.push(FieldEntry {
            group: FieldGroup::Names,
            text: name,
            target: HitTarget::ModelElement {
                key: concept_id.clone(),
            },
        });
    }

    // Model: kind, stereotypes, tags, and relationship endpoint pairs.
    if let Some(node) = node {
        if let Some(kind) = &fields.kind {
            fields.entries.push(FieldEntry {
                group: FieldGroup::Model,
                text: kind.clone(),
                target: HitTarget::ModelElement {
                    key: concept_id.clone(),
                },
            });
        }
        for stereotype in &node.stereotypes {
            fields.entries.push(FieldEntry {
                group: FieldGroup::Model,
                text: stereotype.clone(),
                target: HitTarget::ModelElement {
                    key: concept_id.clone(),
                },
            });
        }
    }
    for tag in &concept.tags {
        fields.entries.push(FieldEntry {
            group: FieldGroup::Model,
            text: tag.clone(),
            target: HitTarget::ModelElement {
                key: concept_id.clone(),
            },
        });
    }
    for edge in &uml.projection.edges {
        let other = if edge.source == concept_id {
            &edge.target
        } else if edge.target == concept_id {
            &edge.source
        } else {
            continue;
        };
        let other_name = uml
            .projection
            .node(other)
            .and_then(|node| node.concept.title.clone())
            .unwrap_or_else(|| other.clone());
        fields.entries.push(FieldEntry {
            group: FieldGroup::Model,
            text: format!("{} {}", edge.kind.as_str(), other_name),
            // The entry is indexed under THIS document, so its hit must
            // reveal THIS document's element -- the other endpoint only
            // names the relationship in the indexed text.
            target: HitTarget::ModelElement {
                key: concept_id.clone(),
            },
        });
    }

    // Prose: body text runs. Markup/marker spans and heading content (already
    // covered by Names) are skipped; spans are deduped per document so an
    // overlapping raw/projected extraction pass never yields a duplicate span.
    let mut seen_prose: HashSet<(u32, u32)> = HashSet::new();
    for span in snapshot.queries().spans(full) {
        if span.source_role != MarkdownSourceRole::Content {
            continue;
        }
        if !matches!(
            span.semantic_role,
            MarkdownSemanticRole::Text
                | MarkdownSemanticRole::Escape
                | MarkdownSemanticRole::Entity
        ) {
            continue;
        }
        if headings
            .iter()
            .any(|heading| contains(heading.content_range, span.range))
        {
            continue;
        }
        let text = snapshot.text().slice(span.range).unwrap_or("");
        if text.trim().is_empty() {
            continue;
        }
        let start = span.range.start().to_usize() as u32;
        let end = span.range.end().to_usize() as u32;
        if !seen_prose.insert((start, end)) {
            continue;
        }
        fields.entries.push(FieldEntry {
            group: FieldGroup::Prose,
            text: text.to_string(),
            target: HitTarget::TextSpan {
                start,
                end,
                line: line_of(version, span.range.start()),
            },
        });
    }

    // Structure: frontmatter keys, the `id:` value, and link targets.
    for span in snapshot.queries().spans(full) {
        if span.semantic_role != MarkdownSemanticRole::FrontmatterKey {
            continue;
        }
        let text = snapshot.text().slice(span.range).unwrap_or("");
        if text.is_empty() {
            continue;
        }
        fields.entries.push(FieldEntry {
            group: FieldGroup::Structure,
            text: text.to_string(),
            target: HitTarget::TextSpan {
                start: span.range.start().to_usize() as u32,
                end: span.range.end().to_usize() as u32,
                line: line_of(version, span.range.start()),
            },
        });
    }
    if let Some(FmValue::Str(id_value)) = concept.extra.get("id") {
        fields.entries.push(FieldEntry {
            group: FieldGroup::Structure,
            text: id_value.clone(),
            target: HitTarget::ModelElement {
                key: concept_id.clone(),
            },
        });
    }
    for link in snapshot.queries().links() {
        if link.destination.is_empty() {
            continue;
        }
        fields.entries.push(FieldEntry {
            group: FieldGroup::Structure,
            text: link.destination.to_string(),
            target: HitTarget::TextSpan {
                start: link.source_range.start().to_usize() as u32,
                end: link.source_range.end().to_usize() as u32,
                line: line_of(version, link.source_range.start()),
            },
        });
    }

    Some(fields)
}

fn contains(outer: TextRange, inner: TextRange) -> bool {
    outer.start() <= inner.start() && inner.end() <= outer.end()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::prepare_candidate;

    fn extract(pairs: &[(&str, &str)]) -> Vec<DocumentFields> {
        let source = SourceBundle::try_from_pairs(pairs.iter().copied()).unwrap();
        let candidate = prepare_candidate(source.clone(), None, 0).unwrap();
        extract_bundle(&source, candidate.okf(), candidate.uml())
    }

    fn fields_for<'a>(all: &'a [DocumentFields], path: &str) -> &'a DocumentFields {
        all.iter().find(|f| f.path == path).unwrap()
    }

    fn texts(fields: &DocumentFields, group: FieldGroup) -> Vec<&str> {
        fields
            .entries
            .iter()
            .filter(|entry| entry.group == group)
            .map(|entry| entry.text.as_str())
            .collect()
    }

    #[test]
    fn a_class_document_contributes_all_four_groups() {
        let all = extract(&[
            (
                "order.md",
                "---\ntype: uml.Class\ntags: [checkout]\nid: order-concept\n---\n# Order\n\nAn order captures a checkout.\n\n## Relationships\n- depends [Customer](./customer.md)\n",
            ),
            ("customer.md", "---\ntype: uml.Class\n---\n# Customer\n"),
        ]);
        let order = fields_for(&all, "order.md");

        assert_eq!(order.kind.as_deref(), Some("class"));
        let names = texts(order, FieldGroup::Names);
        assert!(names.contains(&"Order"));
        let model = texts(order, FieldGroup::Model);
        assert!(model.contains(&"class"));
        assert!(model.iter().any(|t| t.contains("depends")));
        let prose = texts(order, FieldGroup::Prose);
        assert!(prose.iter().any(|t| t.contains("checkout")));
        let structure = texts(order, FieldGroup::Structure);
        assert!(structure.contains(&"type"));
        assert!(structure.contains(&"./customer.md"));
    }

    #[test]
    fn names_entries_target_the_model_element_and_prose_entries_target_text_spans() {
        let all = extract(&[(
            "order.md",
            "---\ntype: uml.Class\n---\n# Order\n\nBody text.\n",
        )]);
        let order = fields_for(&all, "order.md");

        let classifier_name = order
            .entries
            .iter()
            .find(|entry| entry.group == FieldGroup::Names && entry.text == "Order")
            .unwrap();
        // Two Names entries share the text "Order" here (the heading and the
        // classifier name); at least one must target the ModelElement.
        let has_model_element_name = order.entries.iter().any(|entry| {
            entry.group == FieldGroup::Names
                && matches!(entry.target, HitTarget::ModelElement { .. })
        });
        assert!(has_model_element_name);
        let _ = classifier_name;

        for entry in order
            .entries
            .iter()
            .filter(|e| e.group == FieldGroup::Prose)
        {
            assert!(matches!(entry.target, HitTarget::TextSpan { .. }));
        }
    }

    #[test]
    fn overlapping_raw_and_projected_prose_dedupe_to_one_entry_per_span() {
        let all = extract(&[("order.md", "# Order\n\nOne sentence body.\n")]);
        let order = fields_for(&all, "order.md");

        let mut seen = HashSet::new();
        for entry in order
            .entries
            .iter()
            .filter(|e| e.group == FieldGroup::Prose)
        {
            if let HitTarget::TextSpan { start, end, .. } = entry.target {
                assert!(seen.insert((start, end)), "duplicate span for {:?}", entry);
            }
        }
    }

    #[test]
    fn a_relationship_entry_reveals_its_own_document_not_the_other_endpoint() {
        let all = extract(&[
            (
                "order.md",
                "---\ntype: uml.Class\n---\n# Order\n\n## Relationships\n- depends [Customer](./customer.md)\n",
            ),
            ("customer.md", "---\ntype: uml.Class\n---\n# Customer\n"),
        ]);

        for (path, own_key) in [("order.md", "order"), ("customer.md", "customer")] {
            let fields = fields_for(&all, path);
            let relationship = fields
                .entries
                .iter()
                .find(|entry| entry.group == FieldGroup::Model && entry.text.starts_with("depends"))
                .unwrap_or_else(|| panic!("{path} must index its relationship"));
            assert_eq!(
                relationship.target,
                HitTarget::ModelElement {
                    key: own_key.to_string()
                },
                "{path}'s relationship entry must reveal {path}'s own element"
            );
        }
    }

    #[test]
    fn a_plain_markdown_document_contributes_names_prose_structure_but_no_model() {
        let all = extract(&[(
            "notes.md",
            "---\nsummary: quick\n---\n# Notes\n\nSome prose here. See [ref](./order.md).\n",
        )]);
        let notes = fields_for(&all, "notes.md");

        assert!(notes.kind.is_none());
        assert!(!texts(notes, FieldGroup::Names).is_empty());
        assert!(!texts(notes, FieldGroup::Prose).is_empty());
        assert!(texts(notes, FieldGroup::Structure).contains(&"summary"));
        assert!(texts(notes, FieldGroup::Model).is_empty());
    }
}
