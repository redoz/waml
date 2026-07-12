use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::diagnostic::{DiagCode, Diagnostic};
use crate::frontmatter::parse_frontmatter;
use crate::grammar::{
    bullet_range, parse_attribute_line, parse_relationship_line, parse_value_line,
};
use crate::syntax::{Document, ErrorNode, LayoutItem, Line, Section};

use std::collections::{HashMap, HashSet};

use crate::model::{
    Attribute, ClassifierType, Diagram, DiagramGroup, Edge, Model, Node,
};

struct Head {
    title: String,
    heading_start: usize,
    content_start: usize,
}

/// 1-based line number of byte offset `byte` within `src`.
pub(crate) fn line_at(src: &str, byte: usize) -> usize {
    1 + src[..byte.min(src.len())].bytes().filter(|&b| b == b'\n').count()
}

/// Byte range of `[Title](./slug.md)` within `line`, or the whole bullet.
pub(crate) fn find_link_span(line: &str, title: &str, slug: &str) -> (usize, usize) {
    let needle = format!("[{title}](./{slug}.md)");
    match line.find(&needle) {
        Some(s) => (s, s + needle.len()),
        None => bullet_range(line),
    }
}

/// Does `text` open with a clean CommonMark YAML metadata block? (A private
/// copy of `validate.rs`'s check; `validate.rs` keeps its own until Task 6.)
fn has_metadata_block(text: &str) -> bool {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);
    Parser::new_ext(text, opts).any(|e| matches!(e, Event::Start(Tag::MetadataBlock(_))))
}

pub(crate) const DROPPABLE_MSG: &str =
    "content here is outside the recognized document structure and would be silently dropped by fmt";

/// Walk a bullet section's content into `Line` nodes: a well-formed `- ` bullet
/// becomes `Line::Parsed`, a malformed bullet or stray non-bullet line becomes a
/// preserved `Line::Error` (never dropped). `content_abs_start` is the byte
/// offset of `content`'s first byte within `src`; `malformed_code` is the code
/// a failed bullet parse yields.
fn walk_bullets<T>(
    content: &str,
    content_abs_start: usize,
    src: &str,
    malformed_code: DiagCode,
    mut parse_one: impl FnMut(&str, usize) -> Result<T, crate::grammar::LineError>,
) -> Vec<Line<T>> {
    let mut out = Vec::new();
    let mut fence: Option<char> = None;
    let mut offset = 0usize;
    for raw_line in content.split('\n') {
        let line_start = offset;
        offset += raw_line.len() + 1; // + 1 for the consumed '\n'
        let trimmed = raw_line.trim_end_matches('\r').trim();

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
        if trimmed.is_empty() {
            continue;
        }

        let line_no = line_at(src, content_abs_start + line_start);
        if trimmed.starts_with("- ") {
            match parse_one(raw_line, line_no) {
                Ok(v) => out.push(Line::Parsed(v)),
                Err(e) => out.push(Line::Error(ErrorNode {
                    raw: raw_line.to_string(),
                    line: line_no,
                    span: e.range,
                    code: malformed_code,
                    message: e.message,
                })),
            }
        } else {
            out.push(Line::Error(ErrorNode {
                raw: raw_line.to_string(),
                line: line_no,
                span: bullet_range(raw_line),
                code: DiagCode::DroppableContent,
                message: DROPPABLE_MSG.to_string(),
            }));
        }
    }
    out
}

/// Build a `Section` from its heading title and content, wiring bullet sections
/// with in-tree `Line::Error` nodes. `content_abs_start` is the byte offset of
/// `content`'s first byte within `src`.
fn walk_section(title: &str, content: &str, content_abs_start: usize, src: &str, raw_full: &str) -> Section {
    match title.to_lowercase().as_str() {
        "attributes" => Section::Attributes(walk_bullets(
            content, content_abs_start, src, DiagCode::MalformedAttribute,
            |line, _ln| parse_attribute_line(line),
        )),
        "values" => Section::Values(walk_bullets(
            content, content_abs_start, src, DiagCode::DroppableContent,
            |line, _ln| parse_value_line(line),
        )),
        "relationships" => Section::Relationships(walk_bullets(
            content, content_abs_start, src, DiagCode::MalformedRelationship,
            |line, ln| {
                parse_relationship_line(line).map(|mut r| {
                    r.line = ln;
                    r.span = Some(find_link_span(line, &r.target_title, &r.target_slug));
                    r
                })
            },
        )),
        "members" => Section::Members(crate::grammar::parse_members_block(content, content_abs_start, src)),
        "body" => Section::Body(content.trim().to_string()),
        "notes" => Section::Notes(walk_bullets(
            content, content_abs_start, src, DiagCode::DroppableContent,
            |line, _ln| parse_value_line(line),
        )),
        "layout" => Section::Layout(walk_bullets(
            content, content_abs_start, src, DiagCode::MalformedLayout,
            |line, ln| crate::layout::parse_layout_line(line).map(|stmt| LayoutItem { line: ln, stmt }),
        )),
        _ => Section::Unknown { title: title.to_string(), raw: raw_full.trim_end().to_string() },
    }
}

/// Push a bullet section's `Line::Error` nodes as diagnostics.
fn push_line_errors<T>(lines: &[Line<T>], out: &mut Vec<Diagnostic>) {
    for l in lines {
        if let Line::Error(e) = l {
            out.push(Diagnostic::new(e.code, e.message.clone(), "", e.line).with_span(e.span));
        }
    }
}

fn push_group_errors(g: &crate::syntax::MemberGroup, out: &mut Vec<Diagnostic>) {
    push_line_errors(&g.members, out);
    for c in &g.children {
        push_group_errors(c, out);
    }
}

/// Derive bullet-level syntactic diagnostics by walking the tree's `Line::Error`
/// nodes — the single source of truth for per-line syntax errors.
pub fn diagnostics_of(doc: &Document) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for s in &doc.sections {
        match s {
            Section::Attributes(v) => push_line_errors(v, &mut out),
            Section::Values(v) => push_line_errors(v, &mut out),
            Section::Notes(v) => push_line_errors(v, &mut out),
            Section::Relationships(v) => push_line_errors(v, &mut out),
            Section::Layout(v) => push_line_errors(v, &mut out),
            Section::Members(block) => {
                for g in &block.groups {
                    push_group_errors(g, &mut out);
                }
            }
            _ => {}
        }
    }
    out
}

/// Scan the frontmatter region and pre-first-section preamble of `src` for the
/// diagnostic codes that have no bullet-node home: `UnknownType`,
/// `FrontmatterNotClean`, and `DroppableContent` for prose before the first
/// `## ` section. `file` is left `""` (the caller sets the path).
fn scan_frontmatter_and_preamble(src: &str) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    if src.trim_start().starts_with("---") && !has_metadata_block(src) {
        diags.push(Diagnostic::new(
            DiagCode::FrontmatterNotClean,
            "frontmatter is not a clean CommonMark metadata block (would render as a thematic break + heading)",
            "",
            1,
        ));
    }

    let mut in_fm = false;
    let mut fm_done = false;
    let mut fence: Option<char> = None;
    for (i, raw) in src.lines().enumerate() {
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
                        "",
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
        // The first `## ` section ends the preamble — its content is handled by
        // the in-tree content walk.
        if trimmed.starts_with("## ") {
            break;
        }
        // Non-blank, non-H1 prose before the first section would be silently
        // dropped by parse→serialize.
        if !trimmed.is_empty() {
            let is_h1 = trimmed.starts_with('#') && !trimmed.starts_with("##");
            if !is_h1 {
                diags.push(Diagnostic::new(DiagCode::DroppableContent, DROPPABLE_MSG, "", n));
            }
        }
    }
    diags
}

/// Parse `src` into a `Document` (with in-tree `Line::Error` nodes) plus the
/// syntactic diagnostics derived from those nodes and the frontmatter/preamble
/// scan. Diagnostic `file` is `""` (the caller sets the path); `line` is 1-based
/// over `src` and `span` is a line-relative byte range where known.
pub fn parse(src: &str) -> (Document, Vec<Diagnostic>) {
    let (frontmatter, body) = parse_frontmatter(src);
    let body_offset = src.len() - body.len();
    let parser = Parser::new_ext(&body, Options::empty()).into_offset_iter();

    let mut title = String::new();
    let mut in_h1 = false;
    let mut in_h2 = false;
    let mut cur_title = String::new();
    let mut pending_start = 0usize;
    let mut pending_heading_start = 0usize;
    let mut heads: Vec<Head> = Vec::new();

    for (ev, range) in parser {
        match ev {
            Event::Start(Tag::Heading { level: HeadingLevel::H1, .. }) => in_h1 = true,
            Event::End(TagEnd::Heading(HeadingLevel::H1)) => in_h1 = false,
            Event::Start(Tag::Heading { level: HeadingLevel::H2, .. }) => {
                in_h2 = true;
                cur_title = String::new();
                pending_heading_start = range.start;
                pending_start = range.end;
            }
            Event::End(TagEnd::Heading(HeadingLevel::H2)) => {
                in_h2 = false;
                heads.push(Head {
                    title: cur_title.trim().to_string(),
                    heading_start: pending_heading_start,
                    content_start: pending_start,
                });
            }
            Event::Text(t) | Event::Code(t) => {
                if in_h1 {
                    title.push_str(&t);
                } else if in_h2 {
                    cur_title.push_str(&t);
                }
            }
            _ => {}
        }
    }

    let mut sections = Vec::new();
    for (i, head) in heads.iter().enumerate() {
        let end = heads.get(i + 1).map(|h| h.heading_start).unwrap_or(body.len());
        let raw_slice = &body[head.content_start..end];
        let lead = raw_slice.len() - raw_slice.trim_start().len();
        let content = raw_slice.trim();
        let content_abs_start = body_offset + head.content_start + lead;
        let raw_full = &body[head.heading_start..end];
        sections.push(walk_section(&head.title, content, content_abs_start, src, raw_full));
    }

    let doc = Document { frontmatter, title: title.trim().to_string(), sections };
    let mut diags = diagnostics_of(&doc);
    diags.extend(scan_frontmatter_and_preamble(src));
    (doc, diags)
}

pub fn parse_document(src: &str) -> Document {
    parse(src).0
}

static MARKER_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?m)^<!--\s*(.+?\.md)\s*-->[ \t]*\n").unwrap());

/// Split a concatenated bundle blob into `(path, content)` pairs on
/// `<!-- path/slug.md -->` markers. An unmarked blob is a single document.
pub fn split_bundle(text: &str) -> Vec<(String, String)> {
    let mut marks: Vec<(usize, usize, String)> = Vec::new(); // (marker_start, content_start, path)
    for m in MARKER_RE.captures_iter(text) {
        let whole = m.get(0).unwrap();
        let path = m[1].to_string();
        marks.push((whole.start(), whole.end(), path));
    }
    if marks.is_empty() {
        return vec![("pasted/doc.md".to_string(), text.to_string())];
    }
    let mut out = Vec::new();
    for (i, (_, content_start, path)) in marks.iter().enumerate() {
        let end = marks.get(i + 1).map(|m| m.0).unwrap_or(text.len());
        out.push((path.clone(), text[*content_start..end].to_string()));
    }
    out
}

/// A classifier's filename slug (the node key): last path segment, `.md` stripped.
fn doc_slug(path: &str) -> String {
    let seg = path.rsplit(['/', '\\']).next().unwrap_or(path);
    seg.strip_suffix(".md").unwrap_or(seg).to_string()
}

struct ParsedDoc {
    slug: String,
    ty: ClassifierType,
    doc: Document,
    /// Lossless OKF projection of the source document (single source of the
    /// nested `Node::concept`; never re-derived by hand).
    concept: crate::okf::Concept,
}

fn parse_bundle(bundle: &[(String, String)]) -> Vec<ParsedDoc> {
    bundle
        .iter()
        .map(|(path, text)| {
            let doc = parse_document(text);
            let ty = ClassifierType::parse(doc.frontmatter.get_str("type").unwrap_or("uml.Class"));
            let concept = crate::okf::project(path, text);
            ParsedDoc { slug: doc_slug(path), ty, doc, concept }
        })
        .collect()
}

fn resolve_attr(attr: &Attribute, keyset: &HashSet<&str>) -> Attribute {
    let mut a = attr.clone();
    if let Some(slug) = &a.ty.ref_ {
        if !keyset.contains(slug.as_str()) {
            a.ty.ref_ = None; // degrade to a bare token
        }
    }
    a
}

fn build_node(p: &ParsedDoc, keyset: &HashSet<&str>) -> Node {
    let fm = &p.doc.frontmatter;
    let title = fm.get_str("title").map(String::from).unwrap_or_else(|| {
        if p.doc.title.is_empty() { "Untitled".to_string() } else { p.doc.title.clone() }
    });
    let mut attributes = Vec::new();
    let mut values = Vec::new();
    let mut body = None;
    for s in &p.doc.sections {
        match s {
            Section::Attributes(a) => attributes = a.iter().filter_map(Line::parsed).map(|x| resolve_attr(x, keyset)).collect(),
            Section::Values(v) => values = v.iter().filter_map(Line::parsed).cloned().collect(),
            Section::Body(b) => body = Some(b.clone()),
            _ => {}
        }
    }
    Node {
        concept: p.concept.clone(),
        key: p.slug.clone(),
        ty: p.ty.clone(),
        title,
        stereotypes: fm.get_string_list("stereotype"),
        abstract_: fm.get_bool("abstract") == Some(true),
        description: fm.get_str("description").map(String::from),
        attributes,
        values,
        body,
        annotates: Vec::new(), // deferred: uml.Note anchors
        members: Vec::new(),    // classifiers own no members
    }
}

pub fn build_model(bundle: &[(String, String)]) -> Model {
    let parsed = parse_bundle(bundle);
    let classifiers: Vec<&ParsedDoc> =
        parsed.iter().filter(|p| p.ty != ClassifierType::Diagram).collect();
    let keyset: HashSet<&str> = classifiers.iter().map(|p| p.slug.as_str()).collect();

    let nodes = classifiers.iter().map(|p| build_node(p, &keyset)).collect();
    let edges: Vec<Edge> = build_edges(&classifiers, &keyset);
    let diagrams: Vec<Diagram> = build_diagrams(&parsed, &keyset);

    Model { nodes, edges, diagrams, path: String::new(), packages: Vec::new() }
}

use crate::model::{AssocName, RelationshipKind};
use crate::syntax::ParsedName;

fn build_edges(classifiers: &[&ParsedDoc], keyset: &HashSet<&str>) -> Vec<Edge> {
    let mut edges: Vec<Edge> = Vec::new();
    let mut assoc_pair: HashMap<(String, String), usize> = HashMap::new();
    let mut seen_other: HashSet<(String, String, String)> = HashSet::new();

    for p in classifiers {
        let from = &p.slug;
        for s in &p.doc.sections {
            let Section::Relationships(rels) = s else { continue };
            for r in rels.iter().filter_map(Line::parsed) {
                let to = &r.target_slug;
                if !keyset.contains(to.as_str()) || to == from {
                    continue;
                }
                let name = match &r.name {
                    None => None,
                    Some(ParsedName::Label(l)) => Some(AssocName::Label(l.clone())),
                    Some(ParsedName::Ref { slug, .. }) => {
                        keyset.contains(slug.as_str()).then(|| AssocName::Assoc(slug.clone()))
                    }
                };

                if r.kind == RelationshipKind::Associates {
                    let mut pair = [from.clone(), to.clone()];
                    pair.sort();
                    let key = (pair[0].clone(), pair[1].clone());
                    if let Some(&idx) = assoc_pair.get(&key) {
                        let e = &mut edges[idx];
                        e.bidirectional = true;
                        e.from_end.navigable = Some(true);
                        e.to_end.navigable = Some(true);
                        if e.name.is_none() && name.is_some() {
                            e.name = name;
                        }
                        continue;
                    }
                    let mut to_end = r.to_end.clone();
                    to_end.navigable = Some(true);
                    edges.push(Edge {
                        source: from.clone(),
                        target: to.clone(),
                        kind: RelationshipKind::Associates,
                        name,
                        from_end: r.from_end.clone(),
                        to_end,
                        bidirectional: false,
                    });
                    assoc_pair.insert(key, edges.len() - 1);
                } else {
                    let dedup = (r.kind.as_str().to_string(), from.clone(), to.clone());
                    if !seen_other.insert(dedup) {
                        continue;
                    }
                    edges.push(Edge {
                        source: from.clone(),
                        target: to.clone(),
                        kind: r.kind,
                        name,
                        from_end: r.from_end.clone(),
                        to_end: r.to_end.clone(),
                        bidirectional: false,
                    });
                }
            }
        }
    }
    edges
}

fn resolve_group(g: &crate::syntax::MemberGroup, keyset: &HashSet<&str>) -> DiagramGroup {
    DiagramGroup {
        name: g.name.clone(),
        members: g
            .members
            .iter()
            .filter_map(Line::parsed)
            .filter(|m| keyset.contains(m.slug.as_str()))
            .map(|m| m.slug.clone())
            .collect(),
        children: g.children.iter().map(|c| resolve_group(c, keyset)).collect(),
    }
}

fn build_diagrams(parsed: &[ParsedDoc], keyset: &HashSet<&str>) -> Vec<Diagram> {
    let mut out = Vec::new();
    for p in parsed.iter().filter(|p| p.ty == ClassifierType::Diagram) {
        let fm = &p.doc.frontmatter;
        let title = fm.get_str("title").map(String::from).unwrap_or_else(|| "Untitled diagram".to_string());
        let profile = fm
            .get_str("profile")
            .filter(|s| !s.is_empty())
            .unwrap_or("uml-domain")
            .to_string();

        let mut groups = Vec::new();
        let mut layout = Vec::new();
        for s in &p.doc.sections {
            match s {
                Section::Members(block) => {
                    groups = block.groups.iter().map(|g| resolve_group(g, keyset)).collect();
                }
                Section::Layout(items) => {
                    layout = items.iter().filter_map(Line::parsed).map(|it| it.stmt.clone()).collect();
                }
                _ => {}
            }
        }
        out.push(Diagram { key: p.slug.clone(), title, profile, groups, layout });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::RelationshipKind;

    const ORDER: &str = "---\ntype: uml.Class\nstereotype: [aggregateRoot, entity]\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n- status: [OrderStatus](./order-status.md) {0..1}\n\n## Relationships\n- composes [OrderLine](./order-line.md): 1 to 1..* lines\n\n## Provenance\nHand-authored. Keep me.\n";

    #[test]
    fn parses_frontmatter_title_and_known_sections() {
        let doc = parse_document(ORDER);
        assert_eq!(doc.frontmatter.get_str("title"), Some("Order"));
        assert_eq!(doc.title, "Order");
        let attrs = doc.sections.iter().find_map(|s| match s {
            Section::Attributes(a) => Some(a),
            _ => None,
        }).unwrap();
        assert_eq!(attrs.len(), 2);
        assert_eq!(attrs[1].parsed().unwrap().ty.ref_.as_deref(), Some("order-status"));
        let rels = doc.sections.iter().find_map(|s| match s {
            Section::Relationships(r) => Some(r),
            _ => None,
        }).unwrap();
        assert_eq!(rels[0].parsed().unwrap().kind, RelationshipKind::Composes);
    }

    #[test]
    fn parse_reports_malformed_attribute_with_span_and_line() {
        let src = "---\ntype: uml.Class\ntitle: X\n---\n# X\n\n## Attributes\n- bad line without colon\n";
        let (_doc, diags) = parse(src);
        let d = diags.iter().find(|d| d.code == DiagCode::MalformedAttribute).unwrap();
        assert_eq!(d.line, 8);
        let span = d.span.expect("malformed attribute must carry a span");
        assert!(span.0 < span.1);
    }

    #[test]
    fn parse_reports_unknown_type_on_frontmatter_line() {
        let src = "---\ntype: bpmn.Task\ntitle: X\n---\n# X\n";
        let (_doc, diags) = parse(src);
        let d = diags.iter().find(|d| d.code == DiagCode::UnknownType).unwrap();
        assert_eq!(d.line, 2);
        assert_eq!(d.severity, crate::diagnostic::Severity::Warning);
    }

    #[test]
    fn parse_of_a_clean_doc_has_no_diagnostics() {
        let src = "---\ntype: uml.Class\ntitle: X\n---\n# X\n\n## Attributes\n- id: XId\n";
        let (_doc, diags) = parse(src);
        assert!(diags.is_empty(), "got: {diags:?}");
    }

    #[test]
    fn malformed_line_is_preserved_as_error_node_not_dropped() {
        use crate::syntax::{Line, Section};
        let src = "---\ntype: uml.Class\ntitle: X\n---\n# X\n\n## Attributes\n- id: XId\n- bad line without colon\n";
        let (doc, _diags) = parse(src);
        let attrs = doc.sections.iter().find_map(|s| match s {
            Section::Attributes(a) => Some(a), _ => None }).unwrap();
        assert_eq!(attrs.len(), 2, "the malformed line must be kept as an error node, not dropped");
        let err = attrs.iter().find_map(|l| match l { Line::Error(e) => Some(e), _ => None }).unwrap();
        assert!(err.raw.contains("bad line without colon"));
        // Diagnostics are derived from the same error node.
        let (_d, diags) = parse(src);
        assert!(diags.iter().any(|d| d.code == DiagCode::MalformedAttribute));
    }

    #[test]
    fn preserves_unknown_section_verbatim() {
        let doc = parse_document(ORDER);
        let unknown = doc.sections.iter().find_map(|s| match s {
            Section::Unknown { title, raw } => Some((title.clone(), raw.clone())),
            _ => None,
        }).unwrap();
        assert_eq!(unknown.0, "Provenance");
        assert!(unknown.1.contains("Hand-authored. Keep me."));
        assert!(unknown.1.starts_with("## Provenance"));
    }

    #[test]
    fn ignores_headings_inside_code_fences() {
        let src = "# Doc\n\n## Body\n```\n## Not a section\n```\n";
        let doc = parse_document(src);
        // The fenced `## Not a section` must not open a section.
        assert_eq!(doc.sections.len(), 1);
        assert!(matches!(doc.sections[0], Section::Body(_)));
    }

    #[test]
    fn splits_blob_on_markers() {
        let blob = "<!-- shop/order.md -->\n# Order\n\n<!-- shop/customer.md -->\n# Customer\n";
        let parts = split_bundle(blob);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].0, "shop/order.md");
        assert!(parts[0].1.contains("# Order"));
        assert_eq!(parts[1].0, "shop/customer.md");
    }

    #[test]
    fn unmarked_blob_is_a_single_doc() {
        let parts = split_bundle("# Just one doc\n");
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].0, "pasted/doc.md");
    }

    #[test]
    fn stray_html_comment_is_not_a_bundle_marker() {
        // A lone HTML comment that is NOT a `.md` path marker (e.g. an
        // author's review note) must not be treated as a bundle split
        // point: it belongs to the surrounding document, not a new one.
        let blob = "# Order\n\nSome intro text.\n\n<!-- reviewed: TODO -->\n\nMore text after the comment.\n";
        let parts = split_bundle(blob);
        assert_eq!(parts.len(), 1, "a stray non-.md comment must not split the blob");
        assert_eq!(parts[0].0, "pasted/doc.md");
        assert!(parts[0].1.contains("Some intro text."), "content before the comment must be kept");
        assert!(parts[0].1.contains("More text after the comment."), "content after the comment must be kept");

        // A genuine `.md` marker must still split the blob.
        let real = "<!-- shop/x.md -->\n# X\n";
        let real_parts = split_bundle(real);
        assert_eq!(real_parts.len(), 1);
        assert_eq!(real_parts[0].0, "shop/x.md");
    }
}

#[cfg(test)]
mod model_tests {
    use super::*;
    use crate::model::{ClassifierType, UmlMetaclass};

    fn bundle() -> Vec<(String, String)> {
        vec![
            ("shop/order.md".into(),
             "---\ntype: uml.Class\nstereotype: [aggregateRoot, entity]\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n- status: [OrderStatus](./order-status.md) {0..1}\n- ghost: [Missing](./missing.md)\n".into()),
            ("shop/order-status.md".into(),
             "---\ntype: uml.Enum\ntitle: OrderStatus\n---\n# OrderStatus\n\n## Values\n- DRAFT\n- PLACED\n".into()),
        ]
    }

    #[test]
    fn builds_classifier_nodes() {
        let m = build_model(&bundle());
        assert_eq!(m.nodes.len(), 2);
        let order = m.node("order").unwrap();
        assert_eq!(order.title, "Order");
        assert_eq!(order.ty, ClassifierType::Uml(UmlMetaclass::Class));
        assert_eq!(order.stereotypes, vec!["aggregateRoot", "entity"]);
        assert_eq!(order.attributes.len(), 3);
    }

    #[test]
    fn resolves_and_degrades_attribute_refs() {
        let m = build_model(&bundle());
        let order = m.node("order").unwrap();
        // resolvable link keeps its ref
        assert_eq!(order.attributes[1].ty.ref_.as_deref(), Some("order-status"));
        // unresolvable link degrades to a bare token (ref dropped), name preserved
        assert_eq!(order.attributes[2].ty.name, "Missing");
        assert_eq!(order.attributes[2].ty.ref_, None);
    }

    #[test]
    fn collects_enum_values() {
        let m = build_model(&bundle());
        assert_eq!(m.node("order-status").unwrap().values, vec!["DRAFT", "PLACED"]);
    }

    fn rel_bundle() -> Vec<(String, String)> {
        vec![
            ("a/order.md".into(),
             "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- composes [OrderLine](./order-line.md): 1 to 1..* lines\n- associates [Customer](./customer.md): 1 to 1\n".into()),
            ("a/order-line.md".into(),
             "---\ntype: uml.Class\ntitle: OrderLine\n---\n# OrderLine\n".into()),
            ("a/customer.md".into(),
             "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n\n## Relationships\n- associates [Order](./order.md): 1 to 1\n".into()),
        ]
    }

    #[test]
    fn builds_composition_edge() {
        let m = build_model(&rel_bundle());
        let comp = m.edges.iter().find(|e| e.kind == crate::model::RelationshipKind::Composes).unwrap();
        assert_eq!(comp.source, "order");
        assert_eq!(comp.target, "order-line");
        assert_eq!(comp.to_end.role.as_deref(), Some("lines"));
        assert!(!comp.bidirectional);
    }

    #[test]
    fn reciprocal_associates_collapse_to_one_bidirectional_edge() {
        let m = build_model(&rel_bundle());
        let assocs: Vec<_> = m.edges.iter().filter(|e| e.kind == crate::model::RelationshipKind::Associates).collect();
        assert_eq!(assocs.len(), 1, "reciprocal associates must collapse to one edge");
        assert!(assocs[0].bidirectional);
        assert_eq!(assocs[0].from_end.navigable, Some(true));
        assert_eq!(assocs[0].to_end.navigable, Some(true));
    }

    #[test]
    fn skips_unresolved_targets() {
        let b = vec![("x/a.md".into(),
            "---\ntype: uml.Class\ntitle: A\n---\n# A\n\n## Relationships\n- depends [Ghost](./ghost.md)\n".into())];
        let m = build_model(&b);
        assert!(m.edges.is_empty());
    }

    #[test]
    fn builds_diagram_groups_and_layout() {
        let diagram = "---\ntype: Diagram\ntitle: Orders\nprofile: uml-domain\n---\n# Orders\n\n## Members\n\n### Users\n- [Customer](./customer.md)\n\n### Orders\n- [Order](./order.md)\n\n## Layout\n- Users left of Orders\n";
        let bundle = vec![
            ("customer.md".to_string(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string()),
            ("order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
            ("orders.md".to_string(), diagram.to_string()),
        ];
        let model = build_model(&bundle);
        let d = model.diagrams.iter().find(|d| d.key == "orders").unwrap();
        assert_eq!(d.groups.len(), 2);
        assert_eq!(d.groups[0].name, "Users");
        assert_eq!(d.groups[0].members, vec!["customer".to_string()]);
        assert_eq!(d.layout.len(), 1);
        assert!(matches!(d.layout[0], crate::syntax::LayoutStatement::Placement { .. }));
    }
}
