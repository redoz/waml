use std::collections::{HashMap, HashSet};

use pulldown_cmark::{Event, Options, Parser, Tag};

use crate::diagnostic::{DiagCode, Diagnostic};
use crate::frontmatter::parse_frontmatter;
use crate::grammar::{parse_attribute_line, parse_member_line, parse_relationship_line};
use crate::model::ClassifierType;

fn has_metadata_block(text: &str) -> bool {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);
    Parser::new_ext(text, opts).any(|e| matches!(e, Event::Start(Tag::MetadataBlock(_))))
}

fn slug_of(path: &str) -> String {
    let seg = path.rsplit(['/', '\\']).next().unwrap_or(path);
    seg.strip_suffix(".md").unwrap_or(seg).to_string()
}

fn doc_type(text: &str) -> String {
    parse_frontmatter(text).0.get_str("type").unwrap_or("uml.Class").to_string()
}

fn rel_error_message(line: &str) -> String {
    const ENDED: [&str; 3] = ["associates", "aggregates", "composes"];
    const OTHER: [&str; 3] = ["specializes", "implements", "depends"];
    let verb = line.trim_start_matches("- ").split_whitespace().next().unwrap_or("");
    let has_ends = line.contains(':');
    if ENDED.contains(&verb) && !has_ends {
        format!("'{verb}' requires ': <near> to <far>' multiplicity ends")
    } else if OTHER.contains(&verb) && has_ends {
        format!("'{verb}' does not take multiplicity ends")
    } else if verb == "annotates" {
        "note anchors ('annotates') are not supported yet".to_string()
    } else if !ENDED.contains(&verb) && !OTHER.contains(&verb) {
        format!("unknown relationship verb '{verb}'")
    } else {
        "malformed relationship line".to_string()
    }
}

fn validate_doc(path: &str, text: &str, keyset: &HashSet<String>, diags: &mut Vec<Diagnostic>) {
    if text.trim_start().starts_with("---") && !has_metadata_block(text) {
        diags.push(Diagnostic::new(
            DiagCode::FrontmatterNotClean,
            "frontmatter is not a clean CommonMark metadata block (would render as a thematic break + heading)",
            path,
            1,
        ));
    }

    let mut in_fm = false;
    let mut fm_done = false;
    let mut in_fence = false;
    let mut section = String::new();

    for (i, raw) in text.lines().enumerate() {
        let n = i + 1;
        let trimmed = raw.trim_end_matches('\r').trim();

        if !fm_done && trimmed == "---" {
            if in_fm {
                fm_done = true;
            }
            in_fm = !in_fm;
            continue;
        }
        if in_fm {
            if let Some(rest) = trimmed.strip_prefix("type:") {
                let ty = rest.trim().trim_matches('"');
                if ty != "Diagram" && matches!(ClassifierType::parse(ty), ClassifierType::Unknown(_)) {
                    diags.push(Diagnostic::warn(
                        DiagCode::UnknownType,
                        format!("unknown type '{ty}' — rendered as a generic box"),
                        path,
                        n,
                    ));
                }
            }
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if let Some(h) = trimmed.strip_prefix("## ") {
            section = h.trim().to_lowercase();
            continue;
        }
        if !trimmed.starts_with("- ") {
            continue;
        }

        match section.as_str() {
            "attributes" => {
                if parse_attribute_line(trimmed).is_none() {
                    diags.push(Diagnostic::new(DiagCode::MalformedAttribute, "malformed attribute line", path, n));
                }
            }
            "relationships" => match parse_relationship_line(trimmed) {
                None => diags.push(Diagnostic::new(DiagCode::MalformedRelationship, rel_error_message(trimmed), path, n)),
                Some(r) => {
                    if !keyset.contains(&r.target_slug) {
                        diags.push(Diagnostic::new(
                            DiagCode::UnresolvedTarget,
                            format!("relationship target './{}.md' resolves to no document", r.target_slug),
                            path,
                            n,
                        ));
                    }
                }
            },
            "members" => {
                if let Some(m) = parse_member_line(trimmed) {
                    if !keyset.contains(&m.slug) {
                        diags.push(Diagnostic::warn(
                            DiagCode::UnresolvedTarget,
                            format!("diagram member './{}.md' resolves to no document", m.slug),
                            path,
                            n,
                        ));
                    }
                }
            }
            _ => {}
        }
    }
}

pub fn validate(bundle: &[(String, String)]) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let mut keyset: HashSet<String> = HashSet::new();
    let mut slug_count: HashMap<String, usize> = HashMap::new();

    for (path, text) in bundle {
        let slug = slug_of(path);
        *slug_count.entry(slug.clone()).or_insert(0) += 1;
        if doc_type(text) != "Diagram" {
            keyset.insert(slug);
        }
    }

    for (path, text) in bundle {
        let slug = slug_of(path);
        if slug_count[&slug] > 1 {
            diags.push(Diagnostic::new(
                DiagCode::DuplicateSlug,
                format!("duplicate document slug '{slug}'"),
                path,
                1,
            ));
        }
        validate_doc(path, text, &keyset, &mut diags);
    }
    diags
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::Severity;

    #[test]
    fn flags_unresolved_relationship_target() {
        let b = vec![("a/order.md".into(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- depends [Ghost](./ghost.md)\n".into())];
        let d = validate(&b);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].code, DiagCode::UnresolvedTarget);
        assert_eq!(d[0].line, 8);
    }

    #[test]
    fn flags_missing_ends_on_composition() {
        let b = vec![
            ("a/order.md".into(),
             "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- composes [OrderLine](./order-line.md)\n".into()),
            ("a/order-line.md".into(),
             "---\ntype: uml.Class\ntitle: OrderLine\n---\n# OrderLine\n".into()),
        ];
        let d = validate(&b);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].code, DiagCode::MalformedRelationship);
        assert!(d[0].message.contains("requires"));
    }

    #[test]
    fn flags_malformed_attribute() {
        let b = vec![("a/x.md".into(),
            "---\ntype: uml.Class\ntitle: X\n---\n# X\n\n## Attributes\n- bad line without colon\n".into())];
        let d = validate(&b);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].code, DiagCode::MalformedAttribute);
    }

    #[test]
    fn flags_duplicate_slug() {
        let b = vec![
            ("a/order.md".into(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".into()),
            ("b/order.md".into(), "---\ntype: uml.Class\ntitle: Order2\n---\n# Order2\n".into()),
        ];
        let d = validate(&b);
        assert_eq!(d.iter().filter(|x| x.code == DiagCode::DuplicateSlug).count(), 2);
    }

    #[test]
    fn unresolved_member_is_only_a_warning() {
        let b = vec![("d/dia.md".into(),
            "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Members\n- [Ghost](./ghost.md)\n".into())];
        let d = validate(&b);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].code, DiagCode::UnresolvedTarget);
        assert_eq!(d[0].severity, Severity::Warning);
    }

    #[test]
    fn flags_frontmatter_that_is_not_a_metadata_block() {
        // A missing closing fence breaks metadata-block recognition (pulldown-cmark
        // 0.12.2 tolerates a leading blank line, but not an unterminated block).
        let b = vec![("a/x.md".into(),
            "---\ntype: uml.Class\ntitle: X\n# X\n".into())];
        let d = validate(&b);
        assert!(d.iter().any(|x| x.code == DiagCode::FrontmatterNotClean));
    }

    #[test]
    fn warns_on_unknown_type() {
        let b = vec![("a/x.md".into(),
            "---\ntype: bpmn.Task\ntitle: X\n---\n# X\n".into())];
        let d = validate(&b);
        let w = d.iter().find(|x| x.code == DiagCode::UnknownType).unwrap();
        assert_eq!(w.severity, crate::diagnostic::Severity::Warning);
        assert_eq!(w.line, 2);
    }

    #[test]
    fn clean_document_has_no_diagnostics() {
        let b = vec![("a/x.md".into(),
            "---\ntype: uml.Class\ntitle: X\n---\n# X\n\n## Attributes\n- id: XId\n".into())];
        assert!(validate(&b).is_empty());
    }
}
