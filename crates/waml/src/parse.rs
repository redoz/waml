use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::diagnostic::{DiagCode, Diagnostic};
use crate::frontmatter::parse_frontmatter;
use crate::grammar::{
    bullet_range, parse_attribute_line, parse_relationship_line, parse_value_line,
};
use crate::syntax::{Document, ErrorNode, LayoutItem, Line, Section};

use std::collections::{HashMap, HashSet};

use crate::model::{
    Attribute, BehaviorKind, ClassifierType, Diagram, DiagramDisplay, DiagramGroup, Edge, FlowDoc,
    FlowEdge, FlowFlavor, FlowNode, Model, Node,
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
        "nodes" => Section::Nodes(crate::grammar::parse_flow_block(content, content_abs_start, src)),
        "lifelines" => Section::Lifelines(walk_bullets(
            content, content_abs_start, src, DiagCode::MalformedLifeline,
            |line, ln| {
                crate::grammar::parse_lifeline_line(line).map(|mut l| {
                    l.line = ln;
                    l.span = Some(find_link_span(line, &l.link.title, &l.link.slug));
                    l
                })
            },
        )),
        "messages" => Section::Messages(crate::grammar::parse_messages_block(content, content_abs_start, src)),
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

/// Push a `## Messages` block's `Line::Error` nodes as diagnostics, recursing
/// into fragment operands (and each fragment's own misplaced-line errors).
fn push_seq_errors(items: &[crate::syntax::Line<crate::syntax::SeqItemSyntax>], out: &mut Vec<Diagnostic>) {
    for it in items {
        match it {
            crate::syntax::Line::Error(e) => {
                out.push(Diagnostic::new(e.code, e.message.clone(), "", e.line).with_span(e.span));
            }
            crate::syntax::Line::Parsed(crate::syntax::SeqItemSyntax::Fragment { operands, errors, .. }) => {
                for e in errors {
                    out.push(Diagnostic::new(e.code, e.message.clone(), "", e.line).with_span(e.span));
                }
                for op in operands {
                    push_seq_errors(&op.items, out);
                }
            }
            crate::syntax::Line::Parsed(crate::syntax::SeqItemSyntax::Message(_)) => {}
        }
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
            Section::Lifelines(v) => push_line_errors(v, &mut out),
            Section::Messages(block) => push_seq_errors(&block.items, &mut out),
            Section::Members(block) => {
                for g in &block.groups {
                    push_group_errors(g, &mut out);
                }
            }
            Section::Nodes(block) => {
                for e in &block.preamble_errors {
                    out.push(Diagnostic::new(e.code, e.message.clone(), "", e.line).with_span(e.span));
                }
                for n in &block.nodes {
                    push_line_errors(&n.bullets, &mut out);
                    push_line_errors(&n.notes, &mut out);
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
    /// Bundle path of the source document (forward-slash normalized on use);
    /// carries the directory the doc lives in, for package discovery.
    path: String,
    /// Bare filename slug — used only for the reserved-role checks (`index`,
    /// `log`); NOT the node key (see `id`).
    slug: String,
    /// Full bundle-relative id (`okf::id_of(&path)`) — the node/edge/diagram key.
    id: String,
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
            ParsedDoc { path: path.clone(), slug: doc_slug(path), id: crate::okf::id_of(path), ty, doc, concept }
        })
        .collect()
}

fn resolve_attr(attr: &Attribute, referring_path: &str, keyset: &HashSet<&str>) -> Attribute {
    let mut a = attr.clone();
    if let Some(raw_href) = &a.ty.ref_ {
        let resolved = crate::okf::resolve_href(referring_path, raw_href);
        a.ty.ref_ = keyset.contains(resolved.as_str()).then_some(resolved); // else degrade to a bare token
    }
    a
}

fn build_node(p: &ParsedDoc, keyset: &HashSet<&str>) -> Node {
    let fm = &p.doc.frontmatter;
    let mut attributes = Vec::new();
    let mut values = Vec::new();
    let mut body = None;
    for s in &p.doc.sections {
        match s {
            Section::Attributes(a) => attributes = a.iter().filter_map(Line::parsed).map(|x| resolve_attr(x, &p.path, keyset)).collect(),
            Section::Values(v) => values = v.iter().filter_map(Line::parsed).cloned().collect(),
            Section::Body(b) => body = Some(b.clone()),
            _ => {}
        }
    }
    // title/description/verbatim body now live only on `concept` (single source,
    // resolved in `okf::project`). `note_body` carries the `## Body` prose.
    Node {
        concept: p.concept.clone(),
        key: p.id.clone(),
        ty: p.ty.clone(),
        stereotypes: fm.get_string_list("stereotype"),
        abstract_: fm.get_bool("abstract") == Some(true),
        attributes,
        values,
        note_body: body, // uml.Note prose (`## Body`)
        annotates: Vec::new(), // deferred: uml.Note anchors
        members: Vec::new(),    // classifiers own no members
    }
}

/// Resolved title of a doc (frontmatter `title`, else H1, else its slug).
fn doc_title(p: &ParsedDoc) -> String {
    p.doc.frontmatter.get_str("title").map(String::from).unwrap_or_else(|| {
        if p.doc.title.is_empty() { p.slug.clone() } else { p.doc.title.clone() }
    })
}

/// Directory of a bundle path ("" for root). Forward-slash normalized.
fn dir_of(path: &str) -> String {
    let p = path.replace('\\', "/");
    match p.rfind('/') { Some(i) => p[..i].to_string(), None => String::new() }
}

/// Parsed shape of a frontmatter-less `index.md`.
struct IndexDoc {
    intro: Option<String>,
    order: Vec<String>,
    h1: String,
}

/// Parse a frontmatter-less index.md: H1, intro prose (before the first bullet),
/// and `* [Title](url) - blurb` entries. `url` maps to a member key: `sub/` ->
/// the dir-relative sub-package key; `./slug.md` -> the full id, resolved
/// against this index.md's own directory (same as any other href target).
fn parse_index(dir: &str, text: &str) -> IndexDoc {
    let mut h1 = String::new();
    let mut intro_lines: Vec<&str> = vec![];
    let mut order = vec![];
    let re = regex::Regex::new(r"^\s*[*-]\s*\[[^\]]*\]\(([^)]+)\)(?:\s*-\s*(.*))?$").unwrap();
    let mut seen_bullet = false;
    let referring = if dir.is_empty() { "index.md".to_string() } else { format!("{dir}/index.md") };
    for line in text.lines() {
        if let Some(c) = re.captures(line) {
            seen_bullet = true;
            let url = c.get(1).unwrap().as_str();
            let key = if let Some(sub) = url.strip_suffix('/') {
                let seg = sub.trim_start_matches("./").trim_end_matches('/');
                if dir.is_empty() { seg.to_string() } else { format!("{dir}/{seg}") }
            } else {
                crate::okf::resolve_href(&referring, url)
            };
            order.push(key);
        } else if !seen_bullet {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("# ") {
                h1 = rest.trim().to_string();
            } else if !t.is_empty() {
                intro_lines.push(t);
            }
        }
    }
    IndexDoc {
        intro: (!intro_lines.is_empty()).then(|| intro_lines.join(" ")),
        order,
        h1,
    }
}

/// Build the package forest from the bundle's directory structure.
/// `docs` = (full_path, key, title) for every NON-index concept/diagram doc.
/// `indexes` = raw `index.md` text keyed by its directory; reconciles member
/// order + package description, and the root entry sets `model_path` (its H1).
/// Returns `(model_path, packages)`.
fn build_packages(
    docs: &[(String, String, String)],
    indexes: &std::collections::BTreeMap<String, String>,
) -> (String, Vec<Node>) {
    use std::collections::{BTreeMap, BTreeSet};
    // Every directory that contains a doc, plus all ancestor dirs, is a package.
    let mut dirs: BTreeSet<String> = BTreeSet::new();
    dirs.insert(String::new());
    for (path, _, _) in docs {
        let mut d = dir_of(path);
        loop {
            dirs.insert(d.clone());
            if d.is_empty() { break; }
            d = dir_of(&d);
        }
    }
    // members: (title, key) per dir so we can sort A–Z by title/segment name.
    let mut members: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for d in &dirs {
        members.entry(d.clone()).or_default();
    }
    // child docs
    for (path, key, title) in docs {
        members.get_mut(&dir_of(path)).unwrap().push((title.clone(), key.clone()));
    }
    // child sub-packages: each non-root dir is a member of its parent, sorted by last segment.
    for d in &dirs {
        if d.is_empty() { continue; }
        let parent = dir_of(d);
        let seg = d.rsplit('/').next().unwrap_or(d).to_string();
        members.get_mut(&parent).unwrap().push((seg, d.clone()));
    }
    let packages = dirs
        .iter()
        .map(|d| {
            let mut ms = members.get(d).cloned().unwrap_or_default();
            ms.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()).then(a.1.cmp(&b.1)));
            // Discovered member keys, already in A–Z order.
            let discovered: Vec<String> = ms.into_iter().map(|(_, k)| k).collect();
            let title = if d.is_empty() {
                String::new()
            } else {
                d.rsplit('/').next().unwrap_or(d).to_string()
            };
            let index_path =
                if d.is_empty() { "index.md".to_string() } else { format!("{d}/index.md") };

            // Reconcile against a real index.md when present: listed survivors
            // keep their order, unlisted discovered members are appended A–Z,
            // listed-but-absent entries are silently dropped. Otherwise A–Z.
            let (members, intro, index_src) = match indexes.get(d) {
                Some(text) => {
                    let idx = parse_index(d, text);
                    let mut ordered: Vec<String> = vec![];
                    for k in &idx.order {
                        if discovered.contains(k) && !ordered.contains(k) {
                            ordered.push(k.clone());
                        }
                    }
                    for k in &discovered {
                        if !ordered.contains(k) {
                            ordered.push(k.clone());
                        }
                    }
                    (ordered, idx.intro, text.clone())
                }
                // No index.md: synthesize one so `concept` is always populated.
                None => (discovered, None, format!("# {title}\n")),
            };

            // Title/description now live on `concept` (single source). Pin the
            // package title to its directory segment and route the index intro
            // into `concept.description`.
            let mut concept = crate::okf::project(&index_path, &index_src);
            concept.title = (!title.is_empty()).then(|| title.clone());
            concept.description = intro;

            Node {
                concept,
                key: d.clone(),
                ty: ClassifierType::Uml(crate::model::UmlMetaclass::Package),
                stereotypes: vec![],
                abstract_: false,
                attributes: vec![],
                values: vec![],
                note_body: None,
                annotates: vec![],
                members,
            }
        })
        .collect();

    // Model path = the ROOT index.md's H1 title (else "").
    let path = indexes
        .get("")
        .map(|text| parse_index("", text).h1)
        .unwrap_or_default();
    (path, packages)
}

pub fn build_model(bundle: &[(String, String)]) -> Model {
    let parsed = parse_bundle(bundle);
    // `index.md`/`log.md` are reserved package files, never classifiers.
    // Behavior docs (`uml.Activity`/`StateMachine`/`Sequence`) are the document
    // AND view for their own substrate — they never become classifier nodes.
    let classifiers: Vec<&ParsedDoc> = parsed
        .iter()
        .filter(|p| {
            p.ty != ClassifierType::Diagram
                && !matches!(p.ty, ClassifierType::Behavior(_))
                && p.slug != "index"
                && p.slug != "log"
        })
        .collect();
    let keyset: HashSet<&str> = classifiers.iter().map(|p| p.id.as_str()).collect();

    let nodes = classifiers.iter().map(|p| build_node(p, &keyset)).collect();
    let edges: Vec<Edge> = build_edges(&classifiers, &keyset);
    let diagrams: Vec<Diagram> = build_diagrams(&parsed, &keyset);

    // Discover the package forest from directory structure (index/log excluded).
    let docs: Vec<(String, String, String)> = parsed
        .iter()
        .filter(|p| p.slug != "index" && p.slug != "log")
        .map(|p| (p.path.clone(), p.id.clone(), doc_title(p)))
        .collect();
    // Raw index.md text keyed by directory, for member/description reconciliation.
    let indexes: std::collections::BTreeMap<String, String> = bundle
        .iter()
        .filter(|(path, _)| doc_slug(path) == "index")
        .map(|(path, text)| (dir_of(path), text.clone()))
        .collect();
    let (path, packages) = build_packages(&docs, &indexes);

    let flows = build_flows(&parsed, &keyset);

    Model { nodes, edges, diagrams, path, packages, flows, ..Default::default() }
}

/// Resolve a frontmatter `describes: [T](./t.md)` link against the classifier keyset.
fn resolve_describes(p: &ParsedDoc, keyset: &HashSet<&str>) -> Option<String> {
    p.doc
        .frontmatter
        .get_str("describes")
        .and_then(crate::grammar::parse_link_ref)
        .map(|l| crate::okf::resolve_href(&p.path, &l.slug))
        .filter(|k| keyset.contains(k.as_str()))
}

/// Scan all parsed docs for behavior docs in the flow substrate
/// (`uml.Activity` / `uml.StateMachine` — NOT `uml.Sequence`, which is the
/// separate interaction substrate) and resolve their `## Nodes` block into a
/// `FlowDoc`. A behavior doc with no `## Nodes` section (or an empty one)
/// yields a `FlowDoc` with empty `nodes`/`edges` — never a panic.
fn build_flows(parsed: &[ParsedDoc], keyset: &HashSet<&str>) -> Vec<FlowDoc> {
    use crate::syntax::{FlowBullet, FlowTargetRef};
    let flow_keys: HashSet<String> = parsed
        .iter()
        .filter(|p| matches!(p.ty, ClassifierType::Behavior(BehaviorKind::Activity | BehaviorKind::StateMachine)))
        .map(|p| p.id.clone())
        .collect();
    let mut out = Vec::new();
    for p in parsed {
        let flavor = match p.ty {
            ClassifierType::Behavior(BehaviorKind::Activity) => FlowFlavor::Activity,
            ClassifierType::Behavior(BehaviorKind::StateMachine) => FlowFlavor::StateMachine,
            _ => continue,
        };
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        for s in &p.doc.sections {
            let Section::Nodes(block) = s else { continue };
            for n in &block.nodes {
                let mut fnode = FlowNode {
                    id: n.identity.clone(),
                    kind: n.kind,
                    object_ref: n
                        .object_ref
                        .as_ref()
                        .map(|l| crate::okf::resolve_href(&p.path, &l.slug))
                        .filter(|k| keyset.contains(k.as_str())),
                    partition: None,
                    entry: None,
                    do_: None,
                    exit: None,
                    refines: None,
                    notes: n.notes.iter().filter_map(Line::parsed).cloned().collect(),
                };
                for b in n.bullets.iter().filter_map(Line::parsed) {
                    match b {
                        FlowBullet::Transition(t) => {
                            let (to, to_ref) = match &t.target {
                                FlowTargetRef::Local(name) => (name.clone(), None),
                                FlowTargetRef::Link(l) => {
                                    let r = crate::okf::resolve_href(&p.path, &l.slug);
                                    (l.title.clone(), flow_keys.contains(&r).then_some(r))
                                }
                            };
                            edges.push(FlowEdge {
                                from: n.identity.clone(),
                                to,
                                to_ref,
                                trigger: t.trigger.clone(),
                                guard: t.guard.clone(),
                                is_else: t.is_else,
                                effect: t.effect.clone(),
                                carries: t
                                    .carries
                                    .as_ref()
                                    .map(|l| crate::okf::resolve_href(&p.path, &l.slug))
                                    .filter(|k| keyset.contains(k.as_str())),
                            });
                        }
                        FlowBullet::Entry(e) => fnode.entry = Some(e.clone()),
                        FlowBullet::Do(e) => fnode.do_ = Some(e.clone()),
                        FlowBullet::Exit(e) => fnode.exit = Some(e.clone()),
                        FlowBullet::Refines(l) => {
                            let r = crate::okf::resolve_href(&p.path, &l.slug);
                            fnode.refines = flow_keys.contains(&r).then_some(r);
                        }
                        FlowBullet::Partition(name) => fnode.partition = Some(name.clone()),
                    }
                }
                nodes.push(fnode);
            }
        }
        out.push(FlowDoc {
            key: p.id.clone(),
            title: doc_title(p),
            flavor,
            describes: resolve_describes(p, keyset),
            nodes,
            edges,
        });
    }
    out
}

use crate::model::{AssocName, RelationshipKind};
use crate::syntax::ParsedName;

fn build_edges(classifiers: &[&ParsedDoc], keyset: &HashSet<&str>) -> Vec<Edge> {
    let mut edges: Vec<Edge> = Vec::new();
    let mut assoc_pair: HashMap<(String, String), usize> = HashMap::new();
    let mut seen_other: HashSet<(String, String, String)> = HashSet::new();

    for p in classifiers {
        let from = &p.id;
        for s in &p.doc.sections {
            let Section::Relationships(rels) = s else { continue };
            for r in rels.iter().filter_map(Line::parsed) {
                let to = crate::okf::resolve_href(&p.path, &r.target_slug);
                if !keyset.contains(to.as_str()) || &to == from {
                    continue;
                }
                let name = match &r.name {
                    None => None,
                    Some(ParsedName::Label(l)) => Some(AssocName::Label(l.clone())),
                    Some(ParsedName::Ref { slug, .. }) => {
                        let resolved = crate::okf::resolve_href(&p.path, slug);
                        keyset.contains(resolved.as_str()).then_some(AssocName::Assoc(resolved))
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

fn resolve_group(g: &crate::syntax::MemberGroup, referring_path: &str, keyset: &HashSet<&str>) -> DiagramGroup {
    DiagramGroup {
        name: g.name.clone(),
        members: g
            .members
            .iter()
            .filter_map(Line::parsed)
            .filter_map(|m| {
                let resolved = crate::okf::resolve_href(referring_path, &m.slug);
                keyset.contains(resolved.as_str()).then_some(resolved)
            })
            .collect(),
        children: g.children.iter().map(|c| resolve_group(c, referring_path, keyset)).collect(),
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
                    groups = block.groups.iter().map(|g| resolve_group(g, &p.path, keyset)).collect();
                }
                Section::Layout(items) => {
                    layout = items.iter().filter_map(Line::parsed).map(|it| it.stmt.clone()).collect();
                }
                _ => {}
            }
        }
        let description = fm.get_str("description").map(String::from);

        let max_attributes = match fm.get("maxAttributes") {
            Some(crate::frontmatter::FmValue::Num(n)) if *n > 0.0 => Some(*n as u32),
            _ => None,
        };
        let stereotype_filter = fm.get("stereotypeFilter").map(|_| fm.get_string_list("stereotypeFilter"));

        let display = DiagramDisplay {
            show_attributes: fm.get_bool("showAttributes"),
            attribute_detail: fm.get_str("attributeDetail").map(String::from),
            show_attribute_visibility: fm.get_bool("showAttributeVisibility"),
            show_attribute_multiplicity: fm.get_bool("showAttributeMultiplicity"),
            max_attributes,
            association_labels: fm.get_str("associationLabels").map(String::from),
            emphasize_multiplicity: fm.get_bool("emphasizeMultiplicity"),
            show_stereotype: fm.get_bool("showStereotype"),
            stereotype_filter,
            stereotype_colors: fm.get_string_list("stereotypeColors"),
        };

        out.push(Diagram { key: p.id.clone(), title, profile, description, groups, layout, display });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::RelationshipKind;

    fn diagram_bundle(fm_body: &str) -> Vec<(String, String)> {
        vec![(
            "d.md".to_string(),
            format!("---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n{fm_body}---\n# D\n"),
        )]
    }

    #[test]
    fn build_diagrams_reads_all_display_keys() {
        let b = diagram_bundle(
            "description: \"Notes\"\nshowAttributes: false\nattributeDetail: name-only\n\
             showAttributeVisibility: false\nshowAttributeMultiplicity: false\nmaxAttributes: 6\n\
             associationLabels: hidden\nemphasizeMultiplicity: true\nshowStereotype: false\n\
             stereotypeFilter: [entity, valueObject]\nstereotypeColors: [\"entity:#ffedd5\"]\n",
        );
        let m = build_model(&b);
        let d = &m.diagrams[0];
        assert_eq!(d.description.as_deref(), Some("Notes"));
        let x = &d.display;
        assert_eq!(x.show_attributes, Some(false));
        assert_eq!(x.attribute_detail.as_deref(), Some("name-only"));
        assert_eq!(x.show_attribute_visibility, Some(false));
        assert_eq!(x.show_attribute_multiplicity, Some(false));
        assert_eq!(x.max_attributes, Some(6));
        assert_eq!(x.association_labels.as_deref(), Some("hidden"));
        assert_eq!(x.emphasize_multiplicity, Some(true));
        assert_eq!(x.show_stereotype, Some(false));
        assert_eq!(x.stereotype_filter, Some(vec!["entity".to_string(), "valueObject".to_string()]));
        assert_eq!(x.stereotype_colors, vec!["entity:#ffedd5".to_string()]);
    }

    #[test]
    fn build_diagrams_distinguishes_absent_vs_empty_stereotype_filter() {
        let present = build_model(&diagram_bundle("stereotypeFilter: []\n"));
        assert_eq!(present.diagrams[0].display.stereotype_filter, Some(vec![]));
        let absent = build_model(&diagram_bundle(""));
        assert_eq!(absent.diagrams[0].display.stereotype_filter, None);
    }

    #[test]
    fn build_diagrams_maps_max_attributes_floor() {
        assert_eq!(build_model(&diagram_bundle("maxAttributes: 6\n")).diagrams[0].display.max_attributes, Some(6));
        assert_eq!(build_model(&diagram_bundle("maxAttributes: 0\n")).diagrams[0].display.max_attributes, None);
        assert_eq!(build_model(&diagram_bundle("")).diagrams[0].display.max_attributes, None);
    }

    #[test]
    fn build_diagrams_legacy_doc_has_no_description_and_empty_display() {
        let m = build_model(&diagram_bundle(""));
        assert_eq!(m.diagrams[0].description, None);
        assert!(m.diagrams[0].display.is_empty());
    }

    #[test]
    fn build_model_discovers_nested_packages_from_directories() {
        let b = vec![
            ("sales/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
            ("sales/orders/order-line.md".to_string(), "---\ntype: uml.Class\ntitle: OrderLine\n---\n# OrderLine\n".to_string()),
            ("billing/invoice.md".to_string(), "---\ntype: uml.Class\ntitle: Invoice\n---\n# Invoice\n".to_string()),
        ];
        let m = build_model(&b);
        // classifiers remain flat in `nodes`
        assert_eq!(m.nodes.len(), 3);
        // packages: root "", "sales", "sales/orders", "billing"
        let keys: std::collections::HashSet<_> = m.packages.iter().map(|p| p.key.as_str()).collect();
        assert!(keys.contains("") && keys.contains("sales") && keys.contains("sales/orders") && keys.contains("billing"));
        let root = m.packages.iter().find(|p| p.key.is_empty()).unwrap();
        assert_eq!(root.members, vec!["billing".to_string(), "sales".to_string()]); // A–Z sub-packages
        let sales = m.packages.iter().find(|p| p.key == "sales").unwrap();
        // members = child classifier "order" + sub-package "sales/orders", A–Z by title/name
        assert!(sales.members.contains(&"sales/order".to_string()));
        assert!(sales.members.contains(&"sales/orders".to_string()));
    }

    #[test]
    fn build_model_honors_index_md_order_blurbs_and_intro() {
        let b = vec![
            ("sales/index.md".to_string(),
             "# Sales\n\nSales bounded context.\n\n* [Customer](./customer.md) - a buyer\n* [Order](./order.md) - an order\n".to_string()),
            ("sales/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
            ("sales/customer.md".to_string(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string()),
            // present on disk but NOT listed -> appended after listed ones
            ("sales/invoice.md".to_string(), "---\ntype: uml.Class\ntitle: Invoice\n---\n# Invoice\n".to_string()),
            ("index.md".to_string(), "# acme-model\n\n* [sales](sales/)\n".to_string()),
        ];
        let m = build_model(&b);
        assert_eq!(m.path, "acme-model");
        // index.md docs are not classifiers
        assert!(m.nodes.iter().all(|n| n.key != "index"));
        let sales = m.packages.iter().find(|p| p.key == "sales").unwrap();
        assert_eq!(sales.concept.description.as_deref(), Some("Sales bounded context."));
        // listed order first (customer, order), then unlisted appended (invoice)
        assert_eq!(
            sales.members,
            vec!["sales/customer".to_string(), "sales/order".to_string(), "sales/invoice".to_string()]
        );
    }

    #[test]
    fn build_model_flat_bundle_yields_single_root_package() {
        let b = vec![
            ("order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
            ("customer.md".to_string(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string()),
        ];
        let m = build_model(&b);
        assert_eq!(m.packages.len(), 1);
        let root = &m.packages[0];
        assert_eq!(root.key, "");
        assert_eq!(root.members, vec!["customer".to_string(), "order".to_string()]);
    }

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

    const LIFECYCLE: &str = "---\ntype: uml.StateMachine\ntitle: Order Lifecycle\ndescribes: [Order](./order.md)\n---\n# Order Lifecycle\n\n## Nodes\n\n### initial\n- transitions to Draft\n\n### Draft\n- on `place` when `items > 0` transitions to Placed\n- on `cancel` transitions to Cancelled\n\n#### Notes\n- Auto-expires after 24h.\n\n### Placed\n- entry: `reserveStock`\n- on `ship` transitions to Shipped: `notify`\n\n### Shipped\n- on `deliver` transitions to final\n\n### Cancelled\n- transitions to final\n\n### final\n";

    #[test]
    fn parses_flow_nodes_section() {
        use crate::model::FlowNodeKind;
        use crate::syntax::{FlowBullet, Section};
        let doc = parse_document(LIFECYCLE);
        let block = doc.sections.iter().find_map(|s| match s {
            Section::Nodes(b) => Some(b),
            _ => None,
        }).expect("## Nodes must parse into Section::Nodes");
        // NOTE: the fixture has 6 `###` node headings (initial, Draft, Placed,
        // Shipped, Cancelled, final); the plan's brief asserted 7 / index 6,
        // an off-by-one against its own literal fixture text — corrected here.
        assert_eq!(block.nodes.len(), 6);
        assert_eq!(block.nodes[0].kind, FlowNodeKind::Initial);
        assert_eq!(block.nodes[1].identity, "Draft");
        assert_eq!(block.nodes[1].bullets.len(), 2);
        assert_eq!(block.nodes[1].notes.iter().filter_map(crate::syntax::Line::parsed).next().unwrap(), "Auto-expires after 24h.");
        assert!(matches!(block.nodes[2].bullets[0].parsed().unwrap(), FlowBullet::Entry(e) if e == "reserveStock"));
        assert_eq!(block.nodes[5].kind, FlowNodeKind::Final);
    }

    #[test]
    fn malformed_flow_bullet_is_preserved_and_diagnosed() {
        let src = "---\ntype: uml.Activity\ntitle: A\n---\n# A\n\n## Nodes\n\n### Ship\n- goes to Deliver\n";
        let (doc, diags) = parse(src);
        let d = diags.iter().find(|d| d.code == DiagCode::MalformedFlowBullet).unwrap();
        assert_eq!(d.line, 10);
        // preserved, not dropped
        use crate::syntax::{Line, Section};
        let block = doc.sections.iter().find_map(|s| match s { Section::Nodes(b) => Some(b), _ => None }).unwrap();
        assert!(matches!(&block.nodes[0].bullets[0], Line::Error(e) if e.raw.contains("goes to Deliver")));
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
        let order = m.node("shop/order").unwrap();
        assert_eq!(order.concept.title.as_deref(), Some("Order"));
        assert_eq!(order.ty, ClassifierType::Uml(UmlMetaclass::Class));
        assert_eq!(order.stereotypes, vec!["aggregateRoot", "entity"]);
        assert_eq!(order.attributes.len(), 3);
    }

    #[test]
    fn behavior_docs_are_not_classifier_nodes() {
        let b = vec![
            ("m/order.md".into(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".into()),
            ("m/lifecycle.md".into(), "---\ntype: uml.StateMachine\ntitle: Order Lifecycle\n---\n# Order Lifecycle\n".into()),
        ];
        let m = build_model(&b);
        assert_eq!(m.nodes.len(), 1, "a behavior doc must not become a classifier node");
        assert!(m.node("m/lifecycle").is_none());
    }

    #[test]
    fn resolves_and_degrades_attribute_refs() {
        let m = build_model(&bundle());
        let order = m.node("shop/order").unwrap();
        // resolvable link keeps its ref, resolved to the full id
        assert_eq!(order.attributes[1].ty.ref_.as_deref(), Some("shop/order-status"));
        // unresolvable link degrades to a bare token (ref dropped), name preserved
        assert_eq!(order.attributes[2].ty.name, "Missing");
        assert_eq!(order.attributes[2].ty.ref_, None);
    }

    #[test]
    fn collects_enum_values() {
        let m = build_model(&bundle());
        assert_eq!(m.node("shop/order-status").unwrap().values, vec!["DRAFT", "PLACED"]);
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
        assert_eq!(comp.source, "a/order");
        assert_eq!(comp.target, "a/order-line");
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

    #[test]
    fn builds_flow_doc_with_resolved_links_and_edges() {
        use crate::model::{FlowFlavor, FlowNodeKind};
        let b = vec![
            ("m/order.md".into(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".into()),
            ("m/sub.md".into(), "---\ntype: uml.Activity\ntitle: Sub\n---\n# Sub\n\n## Nodes\n\n### initial\n- transitions to final\n\n### final\n".into()),
            ("m/lifecycle.md".into(),
             "---\ntype: uml.StateMachine\ntitle: Order Lifecycle\ndescribes: [Order](./order.md)\n---\n# Order Lifecycle\n\n## Nodes\n\n### initial\n- transitions to Draft\n\n### Draft\n- on `place` when `items > 0` transitions to Placed: `reserve`\n- partition: Sales\n\n### Placed\n- entry: `reserveStock`\n- refines [Sub](./sub.md)\n- transitions to Ship carries [Order](./order.md)\n\n### Ship\n- transitions to final\n\n### final\n".into()),
        ];
        let m = build_model(&b);
        assert_eq!(m.flows.len(), 2);
        let f = m.flows.iter().find(|f| f.key == "m/lifecycle").unwrap();
        assert_eq!(f.flavor, FlowFlavor::StateMachine);
        assert_eq!(f.describes.as_deref(), Some("m/order"));
        assert_eq!(f.nodes.len(), 5);
        assert_eq!(f.nodes[0].kind, FlowNodeKind::Initial);
        assert_eq!(f.nodes[1].partition.as_deref(), Some("Sales"));
        assert_eq!(f.nodes[2].entry.as_deref(), Some("reserveStock"));
        assert_eq!(f.nodes[2].refines.as_deref(), Some("m/sub"));
        assert_eq!(f.edges.len(), 4);
        let placed = f.edges.iter().find(|e| e.to == "Placed").unwrap();
        assert_eq!(placed.trigger.as_deref(), Some("place"));
        assert_eq!(placed.guard.as_deref(), Some("items > 0"));
        assert_eq!(placed.effect.as_deref(), Some("reserve"));
        let ship = f.edges.iter().find(|e| e.to == "Ship").unwrap();
        assert_eq!(ship.carries.as_deref(), Some("m/order"));
    }

    #[test]
    fn excludes_sequence_docs_from_flows() {
        let b = vec![
            ("m/onboarding.md".into(),
             "---\ntype: uml.Activity\ntitle: Onboarding\n---\n# Onboarding\n\n## Nodes\n\n### initial\n- transitions to final\n\n### final\n".into()),
            ("m/login.md".into(), "---\ntype: uml.Sequence\ntitle: Login Flow\n---\n# Login Flow\n".into()),
        ];
        let m = build_model(&b);
        assert_eq!(m.flows.len(), 1, "Sequence doc must not become a flow");
        assert_eq!(m.flows[0].key, "m/onboarding");
        assert!(m.flows.iter().all(|f| f.key != "m/login"));
    }
}
