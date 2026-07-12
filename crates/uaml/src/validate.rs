use std::collections::{HashMap, HashSet};

use pulldown_cmark::{Event, Options, Parser, Tag};

use crate::diagnostic::{DiagCode, Diagnostic};
use crate::frontmatter::parse_frontmatter;
use crate::grammar::{parse_attribute_line, parse_member_line, parse_relationship_line};
use crate::model::ClassifierType;
use crate::parse::parse_document;
use crate::syntax::{LayoutStatement, Line, MemberGroup, NameRef, Operand, OperandRef, Section};

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
            let is_member_group_heading = section == "members" && trimmed.starts_with("###");
            if !is_h1 && !is_member_group_heading {
                let in_bullet_section = matches!(
                    section.as_str(),
                    "attributes" | "values" | "relationships" | "members" | "layout"
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
                if parse_attribute_line(trimmed).is_err() {
                    diags.push(Diagnostic::new(DiagCode::MalformedAttribute, "malformed attribute line", path, n));
                }
            }
            "relationships" => match parse_relationship_line(trimmed) {
                Err(e) => diags.push(Diagnostic::new(DiagCode::MalformedRelationship, e.message, path, n)),
                Ok(r) => {
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
                if let Ok(m) = parse_member_line(trimmed) {
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
            "layout"
                if crate::layout::parse_layout_line(trimmed).is_err() => {
                    diags.push(Diagnostic::new(
                        DiagCode::MalformedLayout,
                        "malformed layout statement",
                        path,
                        n,
                    ));
                }
            _ => {}
        }
    }
}

/// Collect every group's heading name (recursively) into `names`.
fn collect_group_names(g: &MemberGroup, names: &mut HashSet<String>) {
    if !g.name.is_empty() {
        names.insert(g.name.clone());
    }
    for c in &g.children {
        collect_group_names(c, names);
    }
}

/// Walk an operand, reporting each `Name` ref that resolves to neither a
/// member key nor a declared group name.
fn check_operand_refs(
    op: &Operand,
    keyset: &HashSet<String>,
    group_names: &HashSet<String>,
    path: &str,
    line: usize,
    diags: &mut Vec<Diagnostic>,
) {
    match &op.ref_ {
        OperandRef::Name(name) => {
            let (label, resolved) = match name {
                NameRef::Link { slug, .. } => (slug.clone(), keyset.contains(slug)),
                NameRef::Bare(s) => (s.clone(), keyset.contains(s) || group_names.contains(s)),
            };
            if !resolved {
                diags.push(Diagnostic::warn(
                    DiagCode::UnresolvedLayoutRef,
                    format!("layout operand '{label}' resolves no member group"),
                    path,
                    line,
                ));
            }
        }
        OperandRef::InlineGroup { items, .. } => {
            for it in items {
                check_operand_refs(it, keyset, group_names, path, line, diags);
            }
        }
        OperandRef::Paren(inner) => check_operand_refs(inner, keyset, group_names, path, line, diags),
    }
}

/// A stable key for a named operand (its slug or bare name); `None` for an
/// anonymous inline group.
fn operand_key(op: &Operand) -> Option<String> {
    match &op.ref_ {
        OperandRef::Name(NameRef::Link { slug, .. }) => Some(slug.clone()),
        OperandRef::Name(NameRef::Bare(s)) => Some(s.clone()),
        OperandRef::Paren(inner) => operand_key(inner),
        OperandRef::InlineGroup { .. } => None,
    }
}

/// Depth-first cycle check over a directed adjacency map.
fn has_cycle(graph: &HashMap<String, Vec<String>>) -> bool {
    // 0 = unvisited, 1 = on stack, 2 = done
    fn dfs(node: &str, graph: &HashMap<String, Vec<String>>, state: &mut HashMap<String, u8>) -> bool {
        state.insert(node.to_string(), 1);
        if let Some(succs) = graph.get(node) {
            for s in succs {
                match state.get(s).copied().unwrap_or(0) {
                    1 => return true,
                    0
                        if dfs(s, graph, state) => {
                            return true;
                        }
                    _ => {}
                }
            }
        }
        state.insert(node.to_string(), 2);
        false
    }
    let mut state: HashMap<String, u8> = HashMap::new();
    for node in graph.keys() {
        if state.get(node).copied().unwrap_or(0) == 0 && dfs(node, graph, &mut state) {
            return true;
        }
    }
    false
}

fn validate_diagram_refs(path: &str, text: &str, keyset: &HashSet<String>, diags: &mut Vec<Diagnostic>) {
    if doc_type(text) != "Diagram" {
        return;
    }
    let doc = parse_document(text);
    let mut group_names = HashSet::new();
    let mut layout: Vec<&LayoutStatement> = Vec::new();
    for s in &doc.sections {
        match s {
            Section::Members(block) => {
                for g in &block.groups {
                    collect_group_names(g, &mut group_names);
                }
            }
            Section::Layout(stmts) => {
                layout = stmts.iter().filter_map(Line::parsed).map(|it| &it.stmt).collect();
            }
            _ => {}
        }
    }
    // Line number is approximate (the layout statement's exact position within
    // the doc is not tracked here); use the `## Layout` heading line as anchor.
    let layout_line = text.lines().position(|l| l.trim().to_lowercase() == "## layout").map(|i| i + 1).unwrap_or(1);
    for &stmt in &layout {
        let ops: Vec<&Operand> = match stmt {
            LayoutStatement::Standalone(op) => vec![op],
            LayoutStatement::Placement { operands, .. } => operands.iter().collect(),
            LayoutStatement::Alignment { left, right } => vec![&left.operand, &right.operand],
        };
        for op in ops {
            check_operand_refs(op, keyset, &group_names, path, layout_line, diags);
        }
    }

    use crate::syntax::Direction;
    let mut horizontal: HashMap<String, Vec<String>> = HashMap::new();
    let mut vertical: HashMap<String, Vec<String>> = HashMap::new();
    for &stmt in &layout {
        if let LayoutStatement::Placement { operands, directions } = stmt {
            for (i, dir) in directions.iter().enumerate() {
                let (a, b) = (operand_key(&operands[i]), operand_key(&operands[i + 1]));
                let (Some(a), Some(b)) = (a, b) else { continue };
                // Edge points from the operand that must come first to the one after it.
                let (graph, from, to) = match dir {
                    Direction::LeftOf => (&mut horizontal, a, b),
                    Direction::RightOf => (&mut horizontal, b, a),
                    Direction::Above => (&mut vertical, a, b),
                    Direction::Below => (&mut vertical, b, a),
                };
                graph.entry(from).or_default().push(to);
            }
        }
    }
    if has_cycle(&horizontal) || has_cycle(&vertical) {
        let layout_line = text.lines().position(|l| l.trim().to_lowercase() == "## layout").map(|i| i + 1).unwrap_or(1);
        diags.push(Diagnostic::new(
            DiagCode::LayoutCycle,
            "layout placement constraints form a cycle (contradictory ordering)",
            path,
            layout_line,
        ));
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
        validate_diagram_refs(path, text, &keyset, &mut diags);
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
    fn malformed_layout_line_is_an_error() {
        let bundle = vec![(
            "d.md".to_string(),
            "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Layout\n- Users nonsense Orders\n".to_string(),
        )];
        let diags = validate(&bundle);
        assert!(diags.iter().any(|d| d.code == DiagCode::MalformedLayout));
    }

    #[test]
    fn member_subheading_is_not_droppable() {
        let bundle = vec![(
            "d.md".to_string(),
            "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Members\n\n### Users\n- [Customer](./customer.md)\n".to_string(),
        )];
        let diags = validate(&bundle);
        assert!(!diags.iter().any(|d| d.code == DiagCode::DroppableContent),
            "### group heading must not be flagged droppable");
    }

    #[test]
    fn unknown_layout_ref_is_a_warning() {
        let bundle = vec![
            ("customer.md".to_string(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string()),
            ("d.md".to_string(),
             "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Members\n\n### Users\n- [Customer](./customer.md)\n\n## Layout\n- Users left of Ghosts\n".to_string()),
        ];
        let diags = validate(&bundle);
        // "Users" is a declared group, "Ghosts" resolves to nothing -> one warning.
        let refs: Vec<_> = diags.iter().filter(|d| d.code == DiagCode::UnresolvedLayoutRef).collect();
        assert_eq!(refs.len(), 1);
        assert!(refs[0].message.contains("Ghosts"));
    }

    #[test]
    fn contradictory_placement_is_a_cycle_error() {
        let bundle = vec![(
            "d.md".to_string(),
            "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Layout\n- A left of B\n- B left of A\n".to_string(),
        )];
        let diags = validate(&bundle);
        assert!(diags.iter().any(|d| d.code == DiagCode::LayoutCycle));
    }

    #[test]
    fn consistent_placement_has_no_cycle() {
        let bundle = vec![(
            "d.md".to_string(),
            "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Layout\n- A left of B left of C\n- A above D\n".to_string(),
        )];
        let diags = validate(&bundle);
        assert!(!diags.iter().any(|d| d.code == DiagCode::LayoutCycle));
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
