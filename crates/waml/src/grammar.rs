use std::sync::LazyLock;
use regex::Regex;

use crate::diagnostic::DiagCode;
use crate::model::{Attribute, FlowNodeKind, RelEnd, RelationshipKind, TypeRef, Visibility};
use crate::multiplicity::Multiplicity;
use crate::syntax::{
    ErrorNode, FlowBlock, FlowBullet, FlowNodeSyntax, FlowTargetRef, FlowTransition, Line, LinkRef,
    MemberGroup, MemberLine, MembersBlock, ParsedName, ParsedRel,
};

static ATTR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- (?:([+\-#~]) )?([A-Za-z_][A-Za-z0-9_]*): (.+)$").unwrap());
static LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\[([^\]]+)\]\(\./(.+?)\.md\)$").unwrap());
static MULT_TAIL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.*?)\s+\{([^{}]*)\}$").unwrap());
static VALUE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- (\S.*)$").unwrap());
// verb · target-title · target-slug · name-label · name-link-title · name-link-slug · ends
static REL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"^- (associates|aggregates|composes|specializes|implements|depends|includes|extends) ",
        r"\[([^\]]+)\]\(\./(.+?)\.md\)",
        r#"(?: as (?:"([^"]*)"|\[([^\]]+)\]\(\./(.+?)\.md\)))?"#,
        r"(?:\s*:\s*(.+))?$",
    )).unwrap()
});
static END_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\S+)(?:\s+([A-Za-z][A-Za-z0-9_]*))?$").unwrap());
static MEMBER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- \[([^\]]*)\]\(\./(.+?)\.md\)\s*$").unwrap());
static STRAY_BRACKET_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\[\](){}]").unwrap());
static FLOW_TRANSITION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"^- ",
        r"(?:on `([^`]+)` )?",
        r"(?:when `([^`]+)` |(else) )?",
        r"transitions to (.+?)",
        r"(?: carries \[([^\]]+)\]\(\./(.+?)\.md\))?",
        r"(?::\s*`([^`]+)`)?$",
    ))
    .unwrap()
});
static FLOW_INTERNAL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- (entry|do|exit):\s*`([^`]+)`$").unwrap());
static FLOW_REFINES_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- refines \[([^\]]+)\]\(\./(.+?)\.md\)$").unwrap());
static FLOW_PARTITION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- partition:\s*(\S.*)$").unwrap());

/// A line-parse failure: a byte range within the input line plus a message.
#[derive(Debug, Clone, PartialEq)]
pub struct LineError {
    pub range: (usize, usize),
    pub message: String,
}

/// Whole-bullet byte range: first to last non-whitespace byte of `line`.
pub fn bullet_range(line: &str) -> (usize, usize) {
    let start = line.find(|c: char| !c.is_whitespace()).unwrap_or(0);
    let end = line.trim_end().len();
    (start, end.max(start))
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

/// Human-readable message for a malformed `## Relationships` bullet.
pub fn rel_error_message(line: &str) -> String {
    const ENDED: [&str; 2] = ["aggregates", "composes"];
    const OTHER: [&str; 5] = ["specializes", "implements", "depends", "includes", "extends"];
    let verb = line.trim_start_matches("- ").split_whitespace().next().unwrap_or("");
    let has_ends = has_multiplicity_ends(line);
    if ENDED.contains(&verb) && !has_ends {
        format!("'{verb}' requires ': <near> to <far>' multiplicity ends")
    } else if OTHER.contains(&verb) && has_ends {
        format!("'{verb}' does not take multiplicity ends")
    } else if verb == "annotates" {
        "note anchors ('annotates') are not supported yet".to_string()
    } else if !ENDED.contains(&verb) && !OTHER.contains(&verb) && verb != "associates" {
        format!("unknown relationship verb '{verb}'")
    } else {
        "malformed relationship line".to_string()
    }
}

pub fn parse_attribute_line(line: &str) -> Result<Attribute, LineError> {
    let err = |msg: &str| LineError { range: bullet_range(line), message: msg.to_string() };
    let trimmed = line.trim_end_matches('\r').trim();
    let caps = ATTR_RE.captures(trimmed).ok_or_else(|| err("malformed attribute line"))?;
    let visibility = caps.get(1).and_then(|m| Visibility::from_marker(m.as_str().chars().next()?));
    let name = caps[2].to_string();
    let mut rest = caps[3].trim().to_string();
    let mut multiplicity = Multiplicity::default();
    if let Some(mm) = MULT_TAIL_RE.captures(&rest) {
        // A trailing `{…}` token must hold a valid multiplicity; anything else
        // (malformed braces) makes the whole line not an attribute.
        multiplicity = Multiplicity::parse(&mm[2]).ok_or_else(|| err("malformed attribute line"))?;
        rest = mm[1].trim().to_string();
    }
    let ty = if let Some(link) = LINK_RE.captures(&rest) {
        // Raw captured href stem (dir prefix intact, `.md` already stripped by
        // the regex) — resolution against the referring doc's directory
        // happens downstream in `parse::resolve_attr`.
        TypeRef { name: link[1].to_string(), ref_: Some(link[2].to_string()) }
    } else {
        if rest.is_empty() || STRAY_BRACKET_RE.is_match(&rest) {
            return Err(err("malformed attribute line")); // malformed link / stray brackets
        }
        TypeRef { name: rest, ref_: None }
    };
    Ok(Attribute { name, ty, multiplicity, visibility, description: None })
}

pub fn parse_value_line(line: &str) -> Result<String, LineError> {
    let trimmed = line.trim_end_matches('\r').trim();
    VALUE_RE
        .captures(trimmed)
        .map(|c| c[1].trim().to_string())
        .ok_or_else(|| LineError { range: bullet_range(line), message: "malformed value line".to_string() })
}

fn parse_end(part: &str) -> Option<RelEnd> {
    let em = END_RE.captures(part.trim())?;
    let multiplicity = Multiplicity::parse(&em[1])?;
    Some(RelEnd {
        multiplicity: Some(multiplicity),
        role: em.get(2).map(|m| m.as_str().to_string()),
        navigable: None,
    })
}

/// Parse a `<near> to <far>` ends clause into two `RelEnd`s. `None` if it is
/// not exactly two ` to `-separated, individually-valid ends.
pub fn parse_ends(raw: &str) -> Option<(RelEnd, RelEnd)> {
    let parts: Vec<&str> = raw.split(" to ").collect();
    if parts.len() != 2 {
        return None;
    }
    Some((parse_end(parts[0])?, parse_end(parts[1])?))
}

pub fn parse_relationship_line(line: &str) -> Result<ParsedRel, LineError> {
    let err = || LineError {
        range: bullet_range(line),
        message: rel_error_message(line.trim_end_matches('\r').trim()),
    };
    let trimmed = line.trim_end_matches('\r').trim();
    let m = REL_RE.captures(trimmed).ok_or_else(err)?;
    let kind = RelationshipKind::parse(&m[1]).ok_or_else(err)?;
    let ends_raw = m.get(7).map(|x| x.as_str());
    // Ends: required for aggregates/composes; OPTIONAL for associates (bare =
    // actor↔use-case communication link, enforced cross-doc in validate::link);
    // forbidden for all non-ended verbs.
    match (ends_raw.is_some(), kind) {
        (true, k) if !k.is_ended() => return Err(err()),
        (false, k) if k.is_ended() && k != RelationshipKind::Associates => return Err(err()),
        _ => {}
    }
    let name = if let Some(label) = m.get(4) {
        Some(ParsedName::Label(label.as_str().to_string()))
    } else if let (Some(t), Some(s)) = (m.get(5), m.get(6)) {
        Some(ParsedName::Ref { title: t.as_str().to_string(), slug: s.as_str().to_string() })
    } else {
        None
    };
    let (from_end, to_end) = match ends_raw {
        Some(raw) => parse_ends(raw).ok_or_else(err)?,
        None => (RelEnd::default(), RelEnd::default()),
    };
    Ok(ParsedRel {
        kind,
        target_title: m[2].to_string(),
        // Raw captured href stem (dir prefix intact); resolved against the
        // referring doc's directory downstream in `parse::build_edges`.
        target_slug: m[3].to_string(),
        name,
        from_end,
        to_end,
        line: 0,
        span: None,
    })
}

pub fn parse_member_line(line: &str) -> Result<MemberLine, LineError> {
    let trimmed = line.trim_end_matches('\r').trim();
    let m = MEMBER_RE.captures(trimmed).ok_or_else(|| LineError {
        range: bullet_range(line),
        message: "malformed member line".to_string(),
    })?;
    Ok(MemberLine {
        title: m[1].to_string(),
        // Raw captured href stem (dir prefix intact); resolved against the
        // referring diagram's directory downstream in `parse::resolve_group`.
        slug: m[2].to_string(),
        line: 0,
        span: None,
    })
}

fn heading_depth(line: &str) -> Option<(u8, String)> {
    if !line.starts_with("###") {
        return None; // `##` is the section itself; groups start at `###`
    }
    let hashes = line.chars().take_while(|&c| c == '#').count();
    let name = line[hashes..].trim().to_string();
    Some((hashes as u8, name))
}

/// Parse the raw text under `## Members` into a group forest. `content_abs_start`
/// is the byte offset of `content`'s first byte within `src`, used to fill each
/// member's 1-based `line` and link `span`. A stray non-heading, non-member line
/// is preserved as a positioned `Line::Error` (never silently dropped).
pub fn parse_members_block(content: &str, content_abs_start: usize, src: &str) -> MembersBlock {
    fn close_to(stack: &mut Vec<MemberGroup>, groups: &mut Vec<MemberGroup>, depth: u8) {
        while let Some(top) = stack.last() {
            if top.depth >= depth {
                let g = stack.pop().unwrap();
                match stack.last_mut() {
                    Some(parent) => parent.children.push(g),
                    None => groups.push(g),
                }
            } else {
                break;
            }
        }
    }

    let mut groups: Vec<MemberGroup> = Vec::new();
    let mut implicit = MemberGroup { name: String::new(), depth: 0, members: vec![], children: vec![] };
    let mut stack: Vec<MemberGroup> = Vec::new();
    let mut fence: Option<char> = None;
    let mut offset = 0usize;

    for raw in content.split('\n') {
        let line_start = offset;
        offset += raw.len() + 1; // + 1 for the consumed '\n'
        let line = raw.trim_end_matches('\r');
        let t = line.trim_start();

        if let Some(marker) = fence {
            let delim = if marker == '`' { "```" } else { "~~~" };
            if t.starts_with(delim) {
                fence = None;
            }
            continue;
        }
        if t.starts_with("```") {
            fence = Some('`');
            continue;
        }
        if t.starts_with("~~~") {
            fence = Some('~');
            continue;
        }
        if t.is_empty() {
            continue;
        }

        if let Some((depth, name)) = heading_depth(t) {
            close_to(&mut stack, &mut groups, depth);
            stack.push(MemberGroup { name, depth, members: vec![], children: vec![] });
            continue;
        }

        let line_no = crate::parse::line_at(src, content_abs_start + line_start);
        let node = match parse_member_line(raw) {
            Ok(mut m) => {
                m.line = line_no;
                m.span = Some(crate::parse::find_link_span(raw, &m.title, &m.slug));
                Line::Parsed(m)
            }
            // A non-heading, non-member line would be silently dropped by
            // serialize — preserve it as a positioned droppable-content error.
            Err(_) => Line::Error(ErrorNode {
                raw: raw.to_string(),
                line: line_no,
                span: bullet_range(raw),
                code: DiagCode::DroppableContent,
                message: crate::parse::DROPPABLE_MSG.to_string(),
            }),
        };
        match stack.last_mut() {
            Some(g) => g.members.push(node),
            None => implicit.members.push(node),
        }
    }
    close_to(&mut stack, &mut groups, 0);

    if !implicit.members.is_empty() {
        groups.insert(0, implicit);
    } else if groups.is_empty() {
        groups.push(implicit); // empty `## Members` yields one empty implicit group
    }
    MembersBlock { groups }
}

/// Render a members block, heading included, as valid `## Members` Markdown.
pub fn render_members_block(block: &MembersBlock) -> String {
    fn render_group(out: &mut String, g: &MemberGroup) {
        if g.depth > 0 {
            out.push_str(&format!("\n\n{} {}", "#".repeat(g.depth as usize), g.name));
        }
        for m in &g.members {
            out.push('\n');
            match m {
                crate::syntax::Line::Parsed(ml) => out.push_str(&render_member_line(ml)),
                crate::syntax::Line::Error(e) => out.push_str(&e.raw),
            }
        }
        for c in &g.children {
            render_group(out, c);
        }
    }
    let mut out = String::from("## Members");
    for g in &block.groups {
        render_group(&mut out, g);
    }
    out
}

pub fn render_attribute_line(a: &Attribute) -> String {
    let vis = a.visibility.map(|v| format!("{} ", v.marker())).unwrap_or_default();
    let ty = match &a.ty.ref_ {
        Some(slug) => format!("[{}](./{}.md)", a.ty.name, slug),
        None => a.ty.name.clone(),
    };
    let mult = if a.multiplicity.as_str() == "1" {
        String::new()
    } else {
        format!(" {{{}}}", a.multiplicity.as_str())
    };
    format!("- {vis}{}: {ty}{mult}", a.name)
}

fn render_end(e: &RelEnd) -> String {
    let m = e.multiplicity.as_ref().map(|m| m.as_str()).unwrap_or("1");
    match &e.role {
        Some(role) => format!("{m} {role}"),
        None => m.to_string(),
    }
}

/// Render a `<near> to <far>` ends clause (inverse of `parse_ends`).
pub fn render_ends(from: &RelEnd, to: &RelEnd) -> String {
    format!("{} to {}", render_end(from), render_end(to))
}

pub fn render_relationship_line(r: &ParsedRel) -> String {
    let link = format!("[{}](./{}.md)", r.target_title, r.target_slug);
    let name = match &r.name {
        None => String::new(),
        Some(ParsedName::Label(s)) => format!(" as \"{s}\""),
        Some(ParsedName::Ref { title, slug }) => format!(" as [{title}](./{slug}.md)"),
    };
    let has_ends = r.from_end.multiplicity.is_some() || r.to_end.multiplicity.is_some();
    if !r.kind.is_ended() || !has_ends {
        format!("- {} {link}{name}", r.kind.as_str())
    } else {
        format!("- {} {link}{name}: {} to {}", r.kind.as_str(), render_end(&r.from_end), render_end(&r.to_end))
    }
}

pub fn render_member_line(m: &MemberLine) -> String {
    format!("- [{}](./{}.md)", m.title, m.slug)
}

/// Whole-string `[Title](./slug.md)` reference, or `None`.
pub fn parse_link_ref(s: &str) -> Option<LinkRef> {
    LINK_RE
        .captures(s.trim())
        .map(|c| LinkRef { title: c[1].to_string(), slug: c[2].to_string() })
}

/// Human-readable message for a malformed flow bullet.
fn flow_error_message(line: &str) -> String {
    if line.contains("transitions") {
        "malformed transition — expected '[on `trigger`] [when `guard`|else] transitions to <target> [carries <link>] [: `effect`]' (expressions must be backticked)".to_string()
    } else {
        "unrecognized flow bullet — expected a transition, 'entry|do|exit: `effect`', 'refines <link>', or 'partition: <name>'".to_string()
    }
}

pub fn parse_flow_bullet(line: &str) -> Result<FlowBullet, LineError> {
    let trimmed = line.trim_end_matches('\r').trim();
    if let Some(m) = FLOW_TRANSITION_RE.captures(trimmed) {
        let raw_target = m[4].trim().to_string();
        let target = match parse_link_ref(&raw_target) {
            Some(l) => FlowTargetRef::Link(l),
            None => FlowTargetRef::Local(raw_target),
        };
        return Ok(FlowBullet::Transition(FlowTransition {
            trigger: m.get(1).map(|x| x.as_str().to_string()),
            guard: m.get(2).map(|x| x.as_str().to_string()),
            is_else: m.get(3).is_some(),
            target,
            carries: match (m.get(5), m.get(6)) {
                (Some(t), Some(s)) => Some(LinkRef { title: t.as_str().to_string(), slug: s.as_str().to_string() }),
                _ => None,
            },
            effect: m.get(7).map(|x| x.as_str().to_string()),
            line: 0,
        }));
    }
    if let Some(m) = FLOW_INTERNAL_RE.captures(trimmed) {
        let e = m[2].to_string();
        return Ok(match &m[1] {
            "entry" => FlowBullet::Entry(e),
            "do" => FlowBullet::Do(e),
            _ => FlowBullet::Exit(e),
        });
    }
    if let Some(m) = FLOW_REFINES_RE.captures(trimmed) {
        return Ok(FlowBullet::Refines(LinkRef { title: m[1].to_string(), slug: m[2].to_string() }));
    }
    if let Some(m) = FLOW_PARTITION_RE.captures(trimmed) {
        return Ok(FlowBullet::Partition(m[1].trim().to_string()));
    }
    Err(LineError { range: bullet_range(line), message: flow_error_message(trimmed) })
}

/// Split a `###` heading's text into (kind, identity, object link). The
/// identity is the text minus the leading kind keyword; a keyword-only heading
/// uses the keyword itself; an `object` node's identity is its link title.
pub fn parse_flow_heading(text: &str) -> (FlowNodeKind, String, Option<LinkRef>) {
    let t = text.trim();
    let (kw, rest) = match t.split_once(' ') {
        Some((a, b)) => (a, b.trim()),
        None => (t, ""),
    };
    match FlowNodeKind::from_keyword(kw) {
        None => (FlowNodeKind::Plain, t.to_string(), None),
        Some(k) if rest.is_empty() => (k, kw.to_string(), None),
        Some(FlowNodeKind::Object) => match parse_link_ref(rest) {
            Some(l) => (FlowNodeKind::Object, l.title.clone(), Some(l)),
            None => (FlowNodeKind::Object, rest.to_string(), None),
        },
        Some(k) => (k, rest.to_string(), None),
    }
}

pub fn render_flow_heading(n: &FlowNodeSyntax) -> String {
    match n.kind {
        FlowNodeKind::Plain => format!("### {}", n.identity),
        FlowNodeKind::Object => match &n.object_ref {
            Some(l) => format!("### object [{}](./{}.md)", l.title, l.slug),
            None => format!("### object {}", n.identity),
        },
        k => {
            let kw = k.keyword().expect("non-plain kinds have a keyword");
            if n.identity == kw { format!("### {kw}") } else { format!("### {kw} {}", n.identity) }
        }
    }
}

pub fn render_flow_bullet(b: &FlowBullet) -> String {
    match b {
        FlowBullet::Transition(t) => {
            let mut s = String::from("- ");
            if let Some(x) = &t.trigger {
                s.push_str(&format!("on `{x}` "));
            }
            if let Some(g) = &t.guard {
                s.push_str(&format!("when `{g}` "));
            } else if t.is_else {
                s.push_str("else ");
            }
            s.push_str("transitions to ");
            match &t.target {
                FlowTargetRef::Local(n) => s.push_str(n),
                FlowTargetRef::Link(l) => s.push_str(&format!("[{}](./{}.md)", l.title, l.slug)),
            }
            if let Some(c) = &t.carries {
                s.push_str(&format!(" carries [{}](./{}.md)", c.title, c.slug));
            }
            if let Some(e) = &t.effect {
                s.push_str(&format!(": `{e}`"));
            }
            s
        }
        FlowBullet::Entry(e) => format!("- entry: `{e}`"),
        FlowBullet::Do(e) => format!("- do: `{e}`"),
        FlowBullet::Exit(e) => format!("- exit: `{e}`"),
        FlowBullet::Refines(l) => format!("- refines [{}](./{}.md)", l.title, l.slug),
        FlowBullet::Partition(p) => format!("- partition: {p}"),
    }
}

/// Parse the raw text under `## Nodes` into a flow graph block. Each `###`
/// heading opens a node; `#### Notes` opens the current node's notes; bullets
/// parse via `parse_flow_bullet`. Malformed or stray lines are preserved as
/// positioned `Line::Error`s (never dropped).
pub fn parse_flow_block(content: &str, content_abs_start: usize, src: &str) -> FlowBlock {
    let mut nodes: Vec<FlowNodeSyntax> = Vec::new();
    let mut preamble_errors: Vec<ErrorNode> = Vec::new();
    let mut in_notes = false;
    let mut fence: Option<char> = None;
    let mut offset = 0usize;

    for raw in content.split('\n') {
        let line_start = offset;
        offset += raw.len() + 1;
        let line = raw.trim_end_matches('\r');
        let t = line.trim();

        if let Some(marker) = fence {
            let delim = if marker == '`' { "```" } else { "~~~" };
            if t.starts_with(delim) {
                fence = None;
            }
            continue;
        }
        if t.starts_with("```") {
            fence = Some('`');
            continue;
        }
        if t.starts_with("~~~") {
            fence = Some('~');
            continue;
        }
        if t.is_empty() {
            continue;
        }

        let line_no = crate::parse::line_at(src, content_abs_start + line_start);

        if let Some(rest) = t.strip_prefix("### ") {
            let (kind, identity, object_ref) = parse_flow_heading(rest);
            nodes.push(FlowNodeSyntax { kind, identity, object_ref, bullets: vec![], notes: vec![], line: line_no });
            in_notes = false;
            continue;
        }
        if let Some(rest) = t.strip_prefix("#### ") {
            if rest.trim().eq_ignore_ascii_case("notes") && !nodes.is_empty() {
                in_notes = true;
                continue;
            }
            // Unrecognized sub-heading → preserved droppable line.
        }

        let droppable = || ErrorNode {
            raw: raw.to_string(),
            line: line_no,
            span: bullet_range(raw),
            code: DiagCode::DroppableContent,
            message: crate::parse::DROPPABLE_MSG.to_string(),
        };
        let Some(node) = nodes.last_mut() else {
            preamble_errors.push(droppable());
            continue;
        };
        if in_notes {
            match parse_value_line(raw) {
                Ok(v) => node.notes.push(Line::Parsed(v)),
                Err(_) => node.notes.push(Line::Error(droppable())),
            }
        } else if t.starts_with("- ") {
            match parse_flow_bullet(raw) {
                Ok(mut b) => {
                    if let FlowBullet::Transition(ref mut tr) = b {
                        tr.line = line_no;
                    }
                    node.bullets.push(Line::Parsed(b));
                }
                Err(e) => node.bullets.push(Line::Error(ErrorNode {
                    raw: raw.to_string(),
                    line: line_no,
                    span: e.range,
                    code: DiagCode::MalformedFlowBullet,
                    message: e.message,
                })),
            }
        } else {
            node.bullets.push(Line::Error(droppable()));
        }
    }
    FlowBlock { nodes, preamble_errors }
}

/// Render a flow block, `## Nodes` heading included, as canonical Markdown.
pub fn render_flow_block(block: &FlowBlock) -> String {
    let mut out = String::from("## Nodes");
    for e in &block.preamble_errors {
        out.push('\n');
        out.push_str(&e.raw);
    }
    for n in &block.nodes {
        out.push_str("\n\n");
        out.push_str(&render_flow_heading(n));
        for b in &n.bullets {
            out.push('\n');
            match b {
                Line::Parsed(x) => out.push_str(&render_flow_bullet(x)),
                Line::Error(e) => out.push_str(&e.raw),
            }
        }
        if !n.notes.is_empty() {
            out.push_str("\n\n#### Notes");
            for m in &n.notes {
                out.push('\n');
                match m {
                    Line::Parsed(v) => out.push_str(&format!("- {v}")),
                    Line::Error(e) => out.push_str(&e.raw),
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_attribute_with_link_and_multiplicity() {
        let a = parse_attribute_line("- status: [OrderStatus](./order-status.md) {0..1}").unwrap();
        assert_eq!(a.name, "status");
        assert_eq!(a.ty, TypeRef { name: "OrderStatus".to_string(), ref_: Some("order-status".to_string()) });
        assert_eq!(a.multiplicity.as_str(), "0..1");
        assert_eq!(a.visibility, None);
    }

    #[test]
    fn parses_attribute_with_visibility_and_bare_type() {
        let a = parse_attribute_line("- - id: OrderId").unwrap();
        assert_eq!(a.visibility, Some(Visibility::Private));
        assert_eq!(a.name, "id");
        assert_eq!(a.ty, TypeRef { name: "OrderId".to_string(), ref_: None });
        assert_eq!(a.multiplicity.as_str(), "1");
    }

    #[test]
    fn rejects_bare_type_with_stray_brackets() {
        assert!(parse_attribute_line("- x: [Broken]").is_err());
    }

    #[test]
    fn rejects_legacy_bracket_multiplicity() {
        // Hard migration: `[…]` attribute multiplicity is no longer accepted.
        assert!(parse_attribute_line("- id: OrderId [1]").is_err());
        assert!(parse_attribute_line("- status: [OrderStatus](./order-status.md) [0..1]").is_err());
    }

    #[test]
    fn rejects_malformed_brace_multiplicity() {
        assert!(parse_attribute_line("- id: OrderId {nope}").is_err());
        assert!(parse_attribute_line("- id: OrderId {}").is_err());
    }

    #[test]
    fn parses_ended_relationship_with_roles() {
        let r = parse_relationship_line("- associates [Customer](./customer.md): 1 order to 1 customer").unwrap();
        assert_eq!(r.kind, RelationshipKind::Associates);
        assert_eq!(r.target_slug, "customer");
        assert_eq!(r.from_end, RelEnd { multiplicity: Multiplicity::parse("1"), role: Some("order".to_string()), navigable: None });
        assert_eq!(r.to_end.role.as_deref(), Some("customer"));
    }

    #[test]
    fn parses_unended_relationship_with_named_link() {
        let r = parse_relationship_line("- specializes [Animal](./animal.md) as [Kinship](./kinship.md)").unwrap();
        assert_eq!(r.kind, RelationshipKind::Specializes);
        assert_eq!(r.name, Some(ParsedName::Ref { title: "Kinship".to_string(), slug: "kinship".to_string() }));
    }

    #[test]
    fn rejects_ends_on_forbidden_kind_and_missing_ends_on_ended() {
        assert!(parse_relationship_line("- specializes [Animal](./animal.md): 1 to 1").is_err());
        assert!(parse_relationship_line("- composes [OrderLine](./order-line.md)").is_err());
    }

    #[test]
    fn renders_attribute_omitting_default_multiplicity() {
        let a = Attribute {
            name: "id".to_string(),
            ty: TypeRef { name: "OrderId".to_string(), ref_: None },
            multiplicity: Multiplicity::default(),
            visibility: None,
            description: None,
        };
        assert_eq!(render_attribute_line(&a), "- id: OrderId");
    }

    #[test]
    fn renders_relationship_round_trip() {
        let line = "- composes [OrderLine](./order-line.md): 1 to 1..* lines";
        let r = parse_relationship_line(line).unwrap();
        assert_eq!(render_relationship_line(&r), line);
    }

    #[test]
    fn parses_nested_member_groups() {
        let content = "### Users\n- [Customer](./customer.md)\n\n#### VIP\n- [Platinum](./platinum.md)\n\n### Orders\n- [Order](./order.md)";
        let block = parse_members_block(content, 0, content);
        assert_eq!(block.groups.len(), 2);
        assert_eq!(block.groups[0].name, "Users");
        assert_eq!(block.groups[0].depth, 3);
        assert_eq!(block.groups[0].members[0].parsed().unwrap().slug, "customer");
        assert_eq!(block.groups[0].children[0].name, "VIP");
        assert_eq!(block.groups[0].children[0].depth, 4);
        assert_eq!(block.groups[1].name, "Orders");
    }

    #[test]
    fn flat_list_is_one_implicit_group_and_round_trips() {
        let content = "- [Order](./order.md)\n- [Customer](./customer.md)";
        let block = parse_members_block(content, 0, content);
        assert_eq!(block.groups.len(), 1);
        assert_eq!(block.groups[0].name, "");
        assert_eq!(block.groups[0].depth, 0);
        assert_eq!(block.groups[0].members.len(), 2);

        let rendered = render_members_block(&block);
        let body = rendered.strip_prefix("## Members\n").unwrap();
        let reparsed = parse_members_block(body, 0, body);
        assert_eq!(block, reparsed);
    }

    #[test]
    fn member_line_has_no_position() {
        let m = parse_member_line("- [Order](./order.md)").unwrap();
        assert_eq!(m.slug, "order");
        assert_eq!(render_member_line(&m), "- [Order](./order.md)");
    }

    #[test]
    fn parses_includes_and_extends_without_ends() {
        let r = parse_relationship_line("- includes [Authenticate](./authenticate.md)").unwrap();
        assert_eq!(r.kind, RelationshipKind::Includes);
        assert_eq!(r.target_slug, "authenticate");
        let r = parse_relationship_line("- extends [Apply Coupon](./apply-coupon.md)").unwrap();
        assert_eq!(r.kind, RelationshipKind::Extends);
        assert!(parse_relationship_line("- includes [A](./a.md): 1 to 1").is_err());
    }

    #[test]
    fn associates_without_ends_parses_as_bare_communication_link() {
        let r = parse_relationship_line("- associates [Customer](./customer.md)").unwrap();
        assert_eq!(r.kind, RelationshipKind::Associates);
        assert_eq!(r.from_end, RelEnd::default());
        assert_eq!(r.to_end, RelEnd::default());
    }

    #[test]
    fn renders_endless_associates_and_use_case_verbs_round_trip() {
        for line in [
            "- associates [Customer](./customer.md)",
            "- includes [Authenticate](./authenticate.md)",
            "- extends [Apply Coupon](./apply-coupon.md)",
        ] {
            let r = parse_relationship_line(line).unwrap();
            assert_eq!(render_relationship_line(&r), line);
        }
    }

    use crate::model::FlowNodeKind;
    use crate::syntax::{FlowBullet, FlowNodeSyntax, FlowTargetRef};

    #[test]
    fn parses_full_transition_bullet() {
        let FlowBullet::Transition(t) =
            parse_flow_bullet("- on `ship` when `paid` transitions to Shipped carries [Order](./order.md): `notify`").unwrap()
        else { panic!("expected a transition") };
        assert_eq!(t.trigger.as_deref(), Some("ship"));
        assert_eq!(t.guard.as_deref(), Some("paid"));
        assert!(!t.is_else);
        assert_eq!(t.target, FlowTargetRef::Local("Shipped".to_string()));
        assert_eq!(t.carries.as_ref().unwrap().slug, "order");
        assert_eq!(t.effect.as_deref(), Some("notify"));
    }

    #[test]
    fn parses_completion_else_and_link_target_transitions() {
        let FlowBullet::Transition(t) = parse_flow_bullet("- transitions to final").unwrap() else { panic!() };
        assert_eq!(t.target, FlowTargetRef::Local("final".to_string()));
        assert!(t.trigger.is_none() && t.guard.is_none() && !t.is_else);

        let FlowBullet::Transition(t) = parse_flow_bullet("- else transitions to Hold").unwrap() else { panic!() };
        assert!(t.is_else);

        let FlowBullet::Transition(t) =
            parse_flow_bullet("- transitions to [Fulfilment](./fulfilment.md)").unwrap() else { panic!() };
        assert!(matches!(t.target, FlowTargetRef::Link(ref l) if l.slug == "fulfilment"));
    }

    #[test]
    fn parses_internals_refines_and_partition() {
        assert_eq!(parse_flow_bullet("- entry: `reserveStock`").unwrap(), FlowBullet::Entry("reserveStock".to_string()));
        assert_eq!(parse_flow_bullet("- do: `poll`").unwrap(), FlowBullet::Do("poll".to_string()));
        assert_eq!(parse_flow_bullet("- exit: `release`").unwrap(), FlowBullet::Exit("release".to_string()));
        assert!(matches!(parse_flow_bullet("- refines [SubFlow](./sub.md)").unwrap(), FlowBullet::Refines(ref l) if l.slug == "sub"));
        assert_eq!(parse_flow_bullet("- partition: Warehouse").unwrap(), FlowBullet::Partition("Warehouse".to_string()));
        assert!(parse_flow_bullet("- goes to X").is_err());
        assert!(parse_flow_bullet("- when paid transitions to X").is_err(), "guards must be backticked");
    }

    #[test]
    fn parses_flow_headings() {
        assert_eq!(parse_flow_heading("Draft"), (FlowNodeKind::Plain, "Draft".to_string(), None));
        assert_eq!(parse_flow_heading("initial"), (FlowNodeKind::Initial, "initial".to_string(), None));
        assert_eq!(parse_flow_heading("decision Ready to ship?"), (FlowNodeKind::Decision, "Ready to ship?".to_string(), None));
        let (k, id, obj) = parse_flow_heading("object [Order](./order.md)");
        assert_eq!(k, FlowNodeKind::Object);
        assert_eq!(id, "Order");
        assert_eq!(obj.unwrap().slug, "order");
    }

    #[test]
    fn flow_bullets_and_headings_round_trip() {
        for line in [
            "- on `place` when `items > 0` transitions to Placed",
            "- transitions to Deliver carries [Order](./order.md)",
            "- else transitions to Hold",
            "- transitions to Shipped: `notify`",
            "- entry: `reserveStock`",
            "- do: `pollCarrier`",
            "- exit: `releaseStock`",
            "- refines [SubFlow](./sub.md)",
            "- partition: Warehouse",
        ] {
            let b = parse_flow_bullet(line).unwrap();
            assert_eq!(render_flow_bullet(&b), line);
        }
    }

    #[test]
    fn renders_flow_headings_round_trip() {
        for heading in [
            "Draft",
            "initial",
            "decision Ready to ship?",
            "object [Order](./order.md)",
        ] {
            let (kind, identity, object_ref) = parse_flow_heading(heading);
            let n = FlowNodeSyntax { kind, identity, object_ref, bullets: Vec::new(), notes: Vec::new(), line: 0 };
            assert_eq!(render_flow_heading(&n), format!("### {heading}"));
        }
    }
}
