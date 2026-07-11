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

/// Whether a relationship line supplies multiplicity ends (`: <near> to <far>`).
/// Only a `:` that appears AFTER the target link's closing `)` counts — a `:`
/// inside the link's `[Title]` (e.g. `[OrderLine: v2]`) must not be misread
/// as the ends separator.
fn has_multiplicity_ends(line: &str) -> bool {
    match line.find("](") {
        Some(link_start) => match line[link_start..].find(')') {
            Some(close_offset) => line[link_start + close_offset + 1..].contains(':'),
            None => line.contains(':'), // no closing paren found; fall back to whole line
        },
        None => line.contains(':'), // no target link found; fall back to whole line
    }
}

fn rel_error_message(line: &str) -> String {
    const ENDED: [&str; 3] = ["associates", "aggregates", "composes"];
    const OTHER: [&str; 3] = ["specializes", "implements", "depends"];
    let verb = line.trim_start_matches("- ").split_whitespace().next().unwrap_or("");
    let has_ends = has_multiplicity_ends(line);
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
    let mut fence: Option<char> = None;
    let mut section = String::new();
    let mut seen_section = false;

    for (i, raw) in text.lines().enumerate() {
        let n = i + 1;
        let trimmed = raw.trim_end_matches('\r').trim();

        if !in_fm && !fm_done && trimmed == "---" {
            in_fm = true;
            continue;
        }
        if in_fm && (trimmed == "---" || trimmed == "...") {
            in_fm = false;
            fm_done = true;
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
        if let Some(marker) = fence {
            let delim = if marker == '`' { "```" } else { "~~~" };
            if trimmed.starts_with(delim) {
                fence = None;
            }
            continue;
        }
        if trimmed.starts_with("```") {
            fence = Some('`');
            continue;
        }
        if trimmed.starts_with("~~~") {
            fence = Some('~');
            continue;
        }
        if let Some(h) = trimmed.strip_prefix("## ") {
            section = h.trim().to_lowercase();
            seen_section = true;
            continue;
        }

        // Content the parse→serialize round-trip would silently drop:
        // non-blank, non-H1 lines before the first `## ` section, and
        // non-bullet lines inside the five bullet-list sections (their
        // parsers use `filter_map`, which drops anything that doesn't match).
        if !trimmed.is_empty() {
            let is_h1 = trimmed.starts_with('#') && !trimmed.starts_with("##");
            if !is_h1 {
                let in_bullet_section = matches!(
                    section.as_str(),
                    "attributes" | "values" | "relationships" | "members" | "render hints"
                );
                if (!seen_section || in_bullet_section) && !trimmed.starts_with("- ") {
                    diags.push(Diagnostic::new(
                        DiagCode::DroppableContent,
                        "content here is outside the recognized document structure and would be silently dropped by fmt",
                        path,
                        n,
                    ));
                }
            }
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

    #[test]
    fn flags_malformed_line_after_yaml_dots_close() {
        // Frontmatter opens with `---` but closes with `...` (both are valid
        // YAML-style metadata block closers). The body after it must still be
        // scanned — it must not be silently skipped.
        let b = vec![("a/x.md".into(),
            "---\ntype: uml.Class\ntitle: X\n...\n# X\n\n## Attributes\n- bad line without colon\n".into())];
        let d = validate(&b);
        assert!(d.iter().any(|x| x.code == DiagCode::MalformedAttribute));
    }

    #[test]
    fn stray_comment_does_not_hide_unresolved_target() {
        // Reproduces the reported bug: an unresolved relationship target
        // appears BEFORE a later, unrelated HTML comment (e.g. a review
        // note). Pre-fix, `split_bundle` treats that stray comment as a
        // bundle marker and — because it only keeps the tail of the blob
        // starting at the marker — silently discards everything before it,
        // including the frontmatter and the Relationships section. That
        // makes `check` report no problems for a document that actually
        // has an Error-severity diagnostic.
        let text = "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- depends [Ghost](./ghost.md)\n\n<!-- reviewed: needs follow-up -->\n\nTrailing note.\n";
        let bundle = crate::parse::split_bundle(text);
        let d = validate(&bundle);
        assert!(
            d.iter().any(|x| x.code == DiagCode::UnresolvedTarget),
            "unresolved-target must still be reported (content before a stray comment must not be discarded), got: {d:?}"
        );
    }

    #[test]
    fn flags_prose_before_first_section() {
        // A non-blank prose line after the frontmatter and the H1 title but
        // before the first `## ` section is silently dropped by parse/serialize
        // today. It must be flagged as an Error so `fmt` skips the file.
        let b = vec![("a/x.md".into(),
            "---\ntype: uml.Class\ntitle: X\n---\n# X\n\nSome stray prose here.\n\n## Attributes\n- id: XId\n".into())];
        let d = validate(&b);
        assert_eq!(d.len(), 1, "expected exactly one diagnostic, got: {d:?}");
        assert_eq!(d[0].code, DiagCode::DroppableContent);
        assert_eq!(d[0].severity, Severity::Error);
    }

    #[test]
    fn flags_non_bullet_line_in_attributes() {
        // A stray non-bullet line inside a bullet section (e.g. `## Attributes`)
        // is neither preserved nor flagged today — `filter_map` just drops it.
        let b = vec![("a/x.md".into(),
            "---\ntype: uml.Class\ntitle: X\n---\n# X\n\n## Attributes\n- id: XId\nA stray comment line.\n".into())];
        let d = validate(&b);
        assert!(
            d.iter().any(|x| x.code == DiagCode::DroppableContent),
            "expected a DroppableContent diagnostic, got: {d:?}"
        );
    }

    #[test]
    fn allows_prose_in_body_and_unknown_sections() {
        // Free prose in `## Body` and in an unrecognized `## Provenance` section
        // is preserved verbatim by serialize — it must never be flagged.
        let b = vec![("a/x.md".into(),
            "---\ntype: uml.Class\ntitle: X\n---\n# X\n\n## Body\nSome free prose.\nMore prose.\n\n## Provenance\nHand-authored. Keep me.\n".into())];
        let d = validate(&b);
        assert!(
            !d.iter().any(|x| x.code == DiagCode::DroppableContent),
            "prose in Body/unknown sections must never be flagged, got: {d:?}"
        );
    }

    #[test]
    fn rel_error_message_ignores_colon_inside_link_title() {
        // A malformed `composes` line with no multiplicity ends, but whose
        // bracketed target title happens to contain a colon, must still get
        // the "requires ends" message — the colon inside `[Title]` must not
        // be misread as the ends separator by a whole-line colon scan.
        let b = vec![("a/x.md".into(),
            "---\ntype: uml.Class\ntitle: X\n---\n# X\n\n## Relationships\n- composes [OrderLine: v2](./order-line.md)\n".into())];
        let d = validate(&b);
        let rel = d.iter().find(|x| x.code == DiagCode::MalformedRelationship).unwrap();
        assert!(rel.message.contains("requires"), "got: {}", rel.message);
    }

    #[test]
    fn mismatched_fence_styles_do_not_hide_diagnostics() {
        // A `~~~`-fenced block containing a literal ``` line must not desync
        // the fence tracker: only a matching `~~~` should close it, and the
        // malformed line after the real close must still be flagged.
        let b = vec![("a/x.md".into(),
            "---\ntype: uml.Class\ntitle: X\n---\n# X\n\n## Attributes\n~~~\nsome code\n```\nmore code\n~~~\n- bad line without colon\n".into())];
        let d = validate(&b);
        assert!(d.iter().any(|x| x.code == DiagCode::MalformedAttribute));
    }
}
