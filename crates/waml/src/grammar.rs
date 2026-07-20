use regex::Regex;
use std::sync::LazyLock;

use crate::diagnostic::DiagCode;
use crate::model::{Attribute, FlowNodeKind, RelEnd, RelationshipKind, TypeRef, Visibility};
use crate::multiplicity::Multiplicity;
use crate::syntax::{
    ErrorNode, FlowBlock, FlowBullet, FlowNodeSyntax, FlowTargetRef, FlowTransition,
    InlineInstance, LifelineLine, Line, LinkRef, MemberGroup, MemberItem, MemberLine, MembersBlock,
    MessagesBlock, ParsedMessage, ParsedName, ParsedRel, ParsedSlot, SeqItemSyntax,
    SeqOperandSyntax, SlotValue,
};

static ATTR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- (?:([+\-#~]) )?([A-Za-z_][A-Za-z0-9_]*): (.+)$").unwrap());
static LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\[([^\]]+)\]\((?:\./)?((?:\.\./)*.+?)\.md\)$").unwrap());
static MULT_TAIL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.*?)\s+\{([^{}]*)\}$").unwrap());
static VALUE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^- (\S.*)$").unwrap());
static SLOT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- ([A-Za-z_][A-Za-z0-9_]*): (.+)$").unwrap());
static INLINE_INSTANCE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^- instance of \[([^\]]+)\]\(\./(.+?)\.md\) as ([A-Za-z_][A-Za-z0-9_]*)(?: with (.+))?$",
    )
    .unwrap()
});
static SLOT_ASSIGN_RE: LazyLock<Regex> = LazyLock::new(|| {
    // one `<name> set to <value>` assignment, value = quoted | link | bare token,
    // with the remaining clause (after ` and `) captured for the next iteration.
    Regex::new(
        r#"^([A-Za-z_][A-Za-z0-9_]*) set to ("[^"]*"|\[[^\]]+\]\(\./.+?\.md\)|\S+)(?: and (.*))?$"#,
    )
    .unwrap()
});
// verb · target-title · target-slug · name-label · name-link-title · name-link-slug · ends
static REL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"^- (associates|aggregates|composes|specializes|implements|depends|includes|extends|instance of|links) ",
        r"\[([^\]]+)\]\((?:\./)?((?:\.\./)*.+?)\.md\)",
        r#"(?: as (?:"([^"]*)"|\[([^\]]+)\]\((?:\./)?((?:\.\./)*.+?)\.md\)))?"#,
        r"(?:\s*:\s*(.+))?$",
    ))
    .unwrap()
});
static END_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\S+)(?:\s+([A-Za-z][A-Za-z0-9_]*))?$").unwrap());
static LIFELINE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^- \[([^\]]+)\]\(\./(.+?)\.md\)(?: as ([A-Za-z][A-Za-z0-9_]*))?$").unwrap()
});
static MESSAGE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^- (.+?) (calls|sends|replies|creates|destroys) (.+?)(?::\s*`([^`]+)`)?$").unwrap()
});
static SEQ_FRAGMENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- (alt|opt|loop)$").unwrap());
static SEQ_OPERAND_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- (?:when `([^`]+)`|else)$").unwrap());
static MEMBER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- \[([^\]]*)\]\(\./(.+?)\.md\)\s*$").unwrap());
static STRAY_BRACKET_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\[\](){}]").unwrap());
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
    const OTHER: [&str; 6] = [
        "specializes",
        "implements",
        "depends",
        "includes",
        "extends",
        "links",
    ];
    let verb = line
        .trim_start_matches("- ")
        .split_whitespace()
        .next()
        .unwrap_or("");
    let has_ends = has_multiplicity_ends(line);
    if ENDED.contains(&verb) && !has_ends {
        format!("'{verb}' requires ': <near> to <far>' multiplicity ends")
    } else if OTHER.contains(&verb) && has_ends {
        format!("'{verb}' does not take multiplicity ends")
    } else if verb == "annotates" {
        "note anchors ('annotates') are not supported yet".to_string()
    } else if !ENDED.contains(&verb)
        && !OTHER.contains(&verb)
        && verb != "associates"
        && verb != "instance"
    {
        format!("unknown relationship verb '{verb}'")
    } else {
        "malformed relationship line".to_string()
    }
}

pub fn parse_attribute_line(line: &str) -> Result<Attribute, LineError> {
    let err = |msg: &str| LineError {
        range: bullet_range(line),
        message: msg.to_string(),
    };
    let trimmed = line.trim_end_matches('\r').trim();
    let caps = ATTR_RE
        .captures(trimmed)
        .ok_or_else(|| err("malformed attribute line"))?;
    let visibility = caps
        .get(1)
        .and_then(|m| Visibility::from_marker(m.as_str().chars().next()?));
    let name = caps[2].to_string();
    let mut rest = caps[3].trim().to_string();
    let mut multiplicity = Multiplicity::default();
    if let Some(mm) = MULT_TAIL_RE.captures(&rest) {
        // A trailing `{…}` token must hold a valid multiplicity; anything else
        // (malformed braces) makes the whole line not an attribute.
        multiplicity =
            Multiplicity::parse(&mm[2]).ok_or_else(|| err("malformed attribute line"))?;
        rest = mm[1].trim().to_string();
    }
    let ty = if let Some(link) = LINK_RE.captures(&rest) {
        // Raw captured href stem (dir prefix intact, `.md` already stripped by
        // the regex) — resolution against the referring doc's directory
        // happens downstream in `parse::resolve_attr`.
        TypeRef {
            name: link[1].to_string(),
            ref_: Some(link[2].to_string()),
        }
    } else {
        if rest.is_empty() || STRAY_BRACKET_RE.is_match(&rest) {
            return Err(err("malformed attribute line")); // malformed link / stray brackets
        }
        TypeRef {
            name: rest,
            ref_: None,
        }
    };
    Ok(Attribute {
        name,
        ty,
        multiplicity,
        visibility,
        description: None,
    })
}

pub fn parse_value_line(line: &str) -> Result<String, LineError> {
    let trimmed = line.trim_end_matches('\r').trim();
    VALUE_RE
        .captures(trimmed)
        .map(|c| c[1].trim().to_string())
        .ok_or_else(|| LineError {
            range: bullet_range(line),
            message: "malformed value line".to_string(),
        })
}

/// Classify a slot value's surface form. `None` if it is not a valid value.
pub fn classify_slot_value(raw: &str) -> Option<SlotValue> {
    let raw = raw.trim();
    if let Some(inner) = raw.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
        Some(SlotValue::Quoted(inner.to_string()))
    } else if let Some(l) = parse_link_ref(raw) {
        Some(SlotValue::Link(l))
    } else if raw.is_empty() || raw.contains(char::is_whitespace) || STRAY_BRACKET_RE.is_match(raw)
    {
        // A bare value must be a single token with no whitespace / stray brackets.
        None
    } else {
        Some(SlotValue::Bare(raw.to_string()))
    }
}

/// Render a slot value's surface form (exact inverse of `classify_slot_value`).
pub fn render_slot_value(v: &SlotValue) -> String {
    match v {
        SlotValue::Quoted(s) => format!("\"{s}\""),
        SlotValue::Bare(s) => s.clone(),
        SlotValue::Link(l) => format!("[{}](./{}.md)", l.title, l.slug),
    }
}

/// Parse a `<n> set to <v> and <n2> set to <v2> …` clause into ordered slots.
fn parse_slot_clause(clause: &str, whole: &str) -> Result<Vec<ParsedSlot>, LineError> {
    let err = || LineError {
        range: bullet_range(whole),
        message: "malformed instance slot clause — expected '<name> set to <value>[ and …]'"
            .to_string(),
    };
    let mut out = Vec::new();
    let mut rest = clause.trim().to_string();
    while !rest.is_empty() {
        let caps = SLOT_ASSIGN_RE.captures(&rest).ok_or_else(err)?;
        let name = caps[1].to_string();
        let value = classify_slot_value(&caps[2]).ok_or_else(err)?;
        out.push(ParsedSlot {
            name,
            value,
            line: 0,
            span: None,
        });
        rest = caps
            .get(3)
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default();
    }
    Ok(out)
}

/// Parse `- instance of [Classifier](./c.md) as <name>[ with <clause>]`.
pub fn parse_inline_instance(line: &str) -> Result<InlineInstance, LineError> {
    let err = || {
        LineError {
        range: bullet_range(line),
        message: "malformed inline instance — expected '- instance of [Title](./slug.md) as <name>[ with <n> set to <v> and …]'".to_string(),
    }
    };
    let trimmed = line.trim_end_matches('\r').trim();
    let caps = INLINE_INSTANCE_RE.captures(trimmed).ok_or_else(err)?;
    let classifier = LinkRef {
        title: caps[1].to_string(),
        slug: caps[2].to_string(),
    };
    let name = caps[3].to_string();
    let slots = match caps.get(4) {
        Some(clause) => parse_slot_clause(clause.as_str(), trimmed)?,
        None => Vec::new(),
    };
    Ok(InlineInstance {
        classifier,
        name,
        slots,
        line: 0,
        span: None,
    })
}

/// Exact inverse of `parse_inline_instance` (canonical ` and `-joined clause).
pub fn render_inline_instance(i: &InlineInstance) -> String {
    let mut s = format!(
        "- instance of [{}](./{}.md) as {}",
        i.classifier.title, i.classifier.slug, i.name
    );
    if !i.slots.is_empty() {
        let clause = i
            .slots
            .iter()
            .map(|sl| format!("{} set to {}", sl.name, render_slot_value(&sl.value)))
            .collect::<Vec<_>>()
            .join(" and ");
        s.push_str(&format!(" with {clause}"));
    }
    s
}

/// Parse `- name: value` where value is a quoted string, a `[Label](./slug.md)`
/// link, or a bare identifier/number. The value's surface form is preserved in
/// `SlotValue` for byte-identical round-trip.
pub fn parse_slot_line(line: &str) -> Result<ParsedSlot, LineError> {
    let err = || {
        LineError {
        range: bullet_range(line),
        message: "malformed slot — expected '- name: value' (value = \"quoted\", bare token, or [Label](./slug.md))".to_string(),
    }
    };
    let trimmed = line.trim_end_matches('\r').trim();
    let caps = SLOT_RE.captures(trimmed).ok_or_else(err)?;
    let name = caps[1].to_string();
    let value = classify_slot_value(caps[2].trim()).ok_or_else(err)?;
    Ok(ParsedSlot {
        name,
        value,
        line: 0,
        span: None,
    })
}

/// Exact inverse of `parse_slot_line`.
pub fn render_slot_line(s: &ParsedSlot) -> String {
    let v = render_slot_value(&s.value);
    format!("- {}: {v}", s.name)
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
        Some(ParsedName::Ref {
            title: t.as_str().to_string(),
            slug: s.as_str().to_string(),
        })
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
    let mut implicit = MemberGroup {
        name: String::new(),
        depth: 0,
        members: vec![],
        children: vec![],
    };
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
            stack.push(MemberGroup {
                name,
                depth,
                members: vec![],
                children: vec![],
            });
            continue;
        }

        let line_no = crate::parse::line_at(src, content_abs_start + line_start);
        let node = match parse_member_line(raw) {
            Ok(mut m) => {
                m.line = line_no;
                m.span = Some(crate::parse::find_link_span(raw, &m.title, &m.slug));
                Line::Parsed(MemberItem::Member(m))
            }
            // A non-heading, non-member line would be silently dropped by
            // serialize — preserve it as a positioned droppable-content error,
            // unless it is an inline instance (design spec §4.2).
            Err(_) => match parse_inline_instance(raw) {
                Ok(mut inst) => {
                    inst.line = line_no;
                    inst.span = Some(crate::parse::find_link_span(
                        raw,
                        &inst.classifier.title,
                        &inst.classifier.slug,
                    ));
                    Line::Parsed(MemberItem::Instance(inst))
                }
                Err(_) => Line::Error(ErrorNode {
                    raw: raw.to_string(),
                    line: line_no,
                    span: bullet_range(raw),
                    code: DiagCode::DroppableContent,
                    message: crate::parse::DROPPABLE_MSG.to_string(),
                }),
            },
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
                crate::syntax::Line::Parsed(MemberItem::Member(ml)) => {
                    out.push_str(&render_member_line(ml))
                }
                crate::syntax::Line::Parsed(MemberItem::Instance(i)) => {
                    out.push_str(&render_inline_instance(i))
                }
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

/// Render a link href from a stored slug stem. Bare same-dir slugs regain the
/// `./` prefix; parent-relative stems (`../…`) already carry their prefix and
/// are emitted verbatim — the inverse of the `(?:\./)?((?:\.\./)*…)` capture.
pub(crate) fn emit_href(slug: &str) -> String {
    if slug.starts_with("..") {
        format!("{slug}.md")
    } else {
        format!("./{slug}.md")
    }
}

pub fn render_attribute_line(a: &Attribute) -> String {
    let vis = a
        .visibility
        .map(|v| format!("{} ", v.marker()))
        .unwrap_or_default();
    let ty = match &a.ty.ref_ {
        Some(slug) => format!("[{}]({})", a.ty.name, emit_href(slug)),
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
    let link = format!("[{}]({})", r.target_title, emit_href(&r.target_slug));
    let name = match &r.name {
        None => String::new(),
        Some(ParsedName::Label(s)) => format!(" as \"{s}\""),
        Some(ParsedName::Ref { title, slug }) => {
            format!(" as [{title}]({})", emit_href(slug))
        }
    };
    let has_ends = r.from_end.multiplicity.is_some() || r.to_end.multiplicity.is_some();
    if !r.kind.is_ended() || !has_ends {
        format!("- {} {link}{name}", r.kind.as_str())
    } else {
        format!(
            "- {} {link}{name}: {} to {}",
            r.kind.as_str(),
            render_end(&r.from_end),
            render_end(&r.to_end)
        )
    }
}

pub fn render_member_line(m: &MemberLine) -> String {
    format!("- [{}](./{}.md)", m.title, m.slug)
}

/// Whole-string `[Title](./slug.md)` reference, or `None`.
pub fn parse_link_ref(s: &str) -> Option<LinkRef> {
    LINK_RE.captures(s.trim()).map(|c| LinkRef {
        title: c[1].to_string(),
        slug: c[2].to_string(),
    })
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
                (Some(t), Some(s)) => Some(LinkRef {
                    title: t.as_str().to_string(),
                    slug: s.as_str().to_string(),
                }),
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
        return Ok(FlowBullet::Refines(LinkRef {
            title: m[1].to_string(),
            slug: m[2].to_string(),
        }));
    }
    if let Some(m) = FLOW_PARTITION_RE.captures(trimmed) {
        return Ok(FlowBullet::Partition(m[1].trim().to_string()));
    }
    Err(LineError {
        range: bullet_range(line),
        message: flow_error_message(trimmed),
    })
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
            if n.identity == kw {
                format!("### {kw}")
            } else {
                format!("### {kw} {}", n.identity)
            }
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
            nodes.push(FlowNodeSyntax {
                kind,
                identity,
                object_ref,
                bullets: vec![],
                notes: vec![],
                line: line_no,
            });
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
    FlowBlock {
        nodes,
        preamble_errors,
    }
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

pub fn parse_lifeline_line(line: &str) -> Result<LifelineLine, LineError> {
    let trimmed = line.trim_end_matches('\r').trim();
    let m = LIFELINE_RE.captures(trimmed).ok_or_else(|| LineError {
        range: bullet_range(line),
        message: "malformed lifeline — expected '- [Title](./slug.md)[ as alias]' (a lifeline IS a Class or Actor, so it is a link)".to_string(),
    })?;
    Ok(LifelineLine {
        link: LinkRef {
            title: m[1].to_string(),
            slug: m[2].to_string(),
        },
        alias: m.get(3).map(|x| x.as_str().to_string()),
        line: 0,
        span: None,
    })
}

fn message_error_message(line: &str) -> String {
    let first = line
        .trim_start_matches("- ")
        .split_whitespace()
        .next()
        .unwrap_or("");
    if first == "par" {
        "'par' fragments are deferred — supported fragments are alt, opt, loop".to_string()
    } else {
        "malformed message — expected '<sender> <verb> <receiver>[: `signature`]' with verb one of calls/sends/replies/creates/destroys".to_string()
    }
}

pub fn parse_message_line(line: &str) -> Result<ParsedMessage, LineError> {
    let trimmed = line.trim_end_matches('\r').trim();
    let m = MESSAGE_RE.captures(trimmed).ok_or_else(|| LineError {
        range: bullet_range(line),
        message: message_error_message(trimmed),
    })?;
    Ok(ParsedMessage {
        from: m[1].trim().to_string(),
        verb: crate::model::MessageVerb::parse(&m[2])
            .expect("regex alternation is the closed verb set"),
        to: m[3].trim().to_string(),
        signature: m.get(4).map(|x| x.as_str().to_string()),
        line: 0,
    })
}

pub fn render_lifeline_line(l: &LifelineLine) -> String {
    match &l.alias {
        Some(a) => format!("- [{}](./{}.md) as {a}", l.link.title, l.link.slug),
        None => format!("- [{}](./{}.md)", l.link.title, l.link.slug),
    }
}

fn render_message_line(m: &ParsedMessage) -> String {
    match &m.signature {
        Some(sig) => format!("- {} {} {}: `{sig}`", m.from, m.verb.as_str(), m.to),
        None => format!("- {} {} {}", m.from, m.verb.as_str(), m.to),
    }
}

/// Parse the raw text under `## Messages`. Nesting is by indentation (two
/// spaces per level): a fragment owns operands one level deeper; an operand's
/// items nest one level deeper again. Malformed/misplaced lines are preserved
/// as positioned error nodes — nothing is dropped.
pub fn parse_messages_block(content: &str, content_abs_start: usize, src: &str) -> MessagesBlock {
    use crate::model::FragmentKind;

    enum Open {
        Fragment {
            kind: FragmentKind,
            operands: Vec<SeqOperandSyntax>,
            errors: Vec<ErrorNode>,
            line: usize,
            level: usize,
        },
        Operand {
            guard: Option<String>,
            items: Vec<Line<SeqItemSyntax>>,
            line: usize,
            level: usize,
        },
    }
    fn level_of(o: &Open) -> usize {
        match o {
            Open::Fragment { level, .. } | Open::Operand { level, .. } => *level,
        }
    }
    fn close_one(stack: &mut Vec<Open>, top: &mut Vec<Line<SeqItemSyntax>>) {
        match stack.pop().expect("close_one on non-empty stack") {
            Open::Operand {
                guard, items, line, ..
            } => match stack.last_mut() {
                Some(Open::Fragment { operands, .. }) => {
                    operands.push(SeqOperandSyntax { guard, items, line })
                }
                _ => unreachable!("an operand only ever opened under a fragment"),
            },
            Open::Fragment {
                kind,
                operands,
                errors,
                line,
                ..
            } => {
                let item = Line::Parsed(SeqItemSyntax::Fragment {
                    kind,
                    operands,
                    errors,
                    line,
                });
                match stack.last_mut() {
                    Some(Open::Operand { items, .. }) => items.push(item),
                    None => top.push(item),
                    Some(Open::Fragment { .. }) => {
                        unreachable!("a fragment is never opened under a fragment")
                    }
                }
            }
        }
    }

    let mut top: Vec<Line<SeqItemSyntax>> = Vec::new();
    let mut stack: Vec<Open> = Vec::new();
    let mut fence: Option<char> = None;
    let mut offset = 0usize;

    for raw in content.split('\n') {
        let line_start = offset;
        offset += raw.len() + 1;
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

        let line_no = crate::parse::line_at(src, content_abs_start + line_start);
        let level = (line.len() - t.len()) / 2;
        while stack.last().map(|o| level_of(o) >= level).unwrap_or(false) {
            close_one(&mut stack, &mut top);
        }

        let in_fragment = matches!(stack.last(), Some(Open::Fragment { .. }));

        let mk_err = |code: DiagCode, message: String| ErrorNode {
            raw: raw.to_string(),
            line: line_no,
            span: bullet_range(raw),
            code,
            message,
        };

        if !t.starts_with("- ") {
            let e = mk_err(
                DiagCode::DroppableContent,
                crate::parse::DROPPABLE_MSG.to_string(),
            );
            match stack.last_mut() {
                Some(Open::Operand { items, .. }) => items.push(Line::Error(e)),
                Some(Open::Fragment { errors, .. }) => errors.push(e),
                None => top.push(Line::Error(e)),
            }
            continue;
        }

        if let Some(m) = SEQ_OPERAND_RE.captures(t) {
            if in_fragment {
                stack.push(Open::Operand {
                    guard: m.get(1).map(|x| x.as_str().to_string()),
                    items: vec![],
                    line: line_no,
                    level,
                });
            } else {
                let e = mk_err(
                    DiagCode::MalformedMessage,
                    "'when'/'else' operand outside an alt/opt/loop fragment".to_string(),
                );
                match stack.last_mut() {
                    Some(Open::Operand { items, .. }) => items.push(Line::Error(e)),
                    _ => top.push(Line::Error(e)),
                }
            }
            continue;
        }
        if let Some(m) = SEQ_FRAGMENT_RE.captures(t) {
            let kind = crate::model::FragmentKind::parse(&m[1])
                .expect("regex alternation is the closed set");
            if in_fragment {
                let e = mk_err(
                    DiagCode::MalformedMessage,
                    "a nested fragment must sit inside a 'when'/'else' operand".to_string(),
                );
                if let Some(Open::Fragment { errors, .. }) = stack.last_mut() {
                    errors.push(e);
                }
            } else {
                stack.push(Open::Fragment {
                    kind,
                    operands: vec![],
                    errors: vec![],
                    line: line_no,
                    level,
                });
            }
            continue;
        }
        match parse_message_line(t) {
            Ok(mut msg) => {
                msg.line = line_no;
                if in_fragment {
                    let e = mk_err(
                        DiagCode::MalformedMessage,
                        "expected a 'when `guard`' or 'else' operand before messages inside a fragment".to_string(),
                    );
                    if let Some(Open::Fragment { errors, .. }) = stack.last_mut() {
                        errors.push(e);
                    }
                } else {
                    let item = Line::Parsed(SeqItemSyntax::Message(msg));
                    match stack.last_mut() {
                        Some(Open::Operand { items, .. }) => items.push(item),
                        _ => top.push(item),
                    }
                }
            }
            Err(le) => {
                let e = mk_err(DiagCode::MalformedMessage, le.message);
                match stack.last_mut() {
                    Some(Open::Operand { items, .. }) => items.push(Line::Error(e)),
                    Some(Open::Fragment { errors, .. }) => errors.push(e),
                    None => top.push(Line::Error(e)),
                }
            }
        }
    }
    while !stack.is_empty() {
        close_one(&mut stack, &mut top);
    }
    MessagesBlock { items: top }
}

/// Render a messages block, `## Messages` heading included.
pub fn render_messages_block(block: &MessagesBlock) -> String {
    fn render_items(out: &mut String, items: &[Line<SeqItemSyntax>], depth: usize) {
        for it in items {
            out.push('\n');
            match it {
                Line::Error(e) => out.push_str(&e.raw),
                Line::Parsed(SeqItemSyntax::Message(m)) => {
                    out.push_str(&"  ".repeat(depth));
                    out.push_str(&render_message_line(m));
                }
                Line::Parsed(SeqItemSyntax::Fragment {
                    kind,
                    operands,
                    errors,
                    ..
                }) => {
                    out.push_str(&"  ".repeat(depth));
                    out.push_str(&format!("- {}", kind.as_str()));
                    for e in errors {
                        out.push('\n');
                        out.push_str(&e.raw);
                    }
                    for op in operands {
                        out.push('\n');
                        out.push_str(&"  ".repeat(depth + 1));
                        match &op.guard {
                            Some(g) => out.push_str(&format!("- when `{g}`")),
                            None => out.push_str("- else"),
                        }
                        render_items(out, &op.items, depth + 2);
                    }
                }
            }
        }
    }
    let mut out = String::from("## Messages");
    render_items(&mut out, &block.items, 0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_lines_round_trip_all_three_value_forms() {
        for line in [
            "- id: \"ORD-42\"",
            "- status: PLACED",
            "- qty: 3",
            "- customer: [Ann](./ann.md)",
        ] {
            let s = parse_slot_line(line).unwrap();
            assert_eq!(
                render_slot_line(&s),
                line,
                "slot line must round-trip byte-identically"
            );
        }
    }

    #[test]
    fn slot_value_classifies_quoted_bare_and_link() {
        use crate::syntax::SlotValue;
        assert!(
            matches!(parse_slot_line("- id: \"ORD-42\"").unwrap().value, SlotValue::Quoted(v) if v == "ORD-42")
        );
        assert!(
            matches!(parse_slot_line("- status: PLACED").unwrap().value, SlotValue::Bare(v) if v == "PLACED")
        );
        let SlotValue::Link(l) = parse_slot_line("- customer: [Ann](./ann.md)")
            .unwrap()
            .value
        else {
            panic!()
        };
        assert_eq!((l.title.as_str(), l.slug.as_str()), ("Ann", "ann"));
    }

    #[test]
    fn inline_instance_lines_round_trip() {
        for line in [
            "- instance of [Order](./order.md) as order42",
            "- instance of [Order](./order.md) as order42 with id set to \"ORD-42\" and status set to PLACED",
            "- instance of [Order](./order.md) as o with owner set to [Ann](./ann.md)",
        ] {
            let i = parse_inline_instance(line).unwrap();
            assert_eq!(
                render_inline_instance(&i),
                line,
                "inline instance must round-trip byte-identically"
            );
        }
        let i = parse_inline_instance("- instance of [Order](./order.md) as order42 with id set to \"ORD-42\" and status set to PLACED").unwrap();
        assert_eq!(
            (
                i.classifier.title.as_str(),
                i.classifier.slug.as_str(),
                i.name.as_str()
            ),
            ("Order", "order", "order42")
        );
        assert_eq!(i.slots.len(), 2);
        assert_eq!(i.slots[0].name, "id");
        assert_eq!(i.slots[1].name, "status");
    }

    #[test]
    fn parses_attribute_with_link_and_multiplicity() {
        let a = parse_attribute_line("- status: [OrderStatus](./order-status.md) {0..1}").unwrap();
        assert_eq!(a.name, "status");
        assert_eq!(
            a.ty,
            TypeRef {
                name: "OrderStatus".to_string(),
                ref_: Some("order-status".to_string())
            }
        );
        assert_eq!(a.multiplicity.as_str(), "0..1");
        assert_eq!(a.visibility, None);
    }

    #[test]
    fn parses_and_round_trips_parent_relative_attribute_link() {
        // A cross-package attribute type link that steps up a directory.
        let line = "- trigger: [Trigger](../vocabularies/trigger.md)";
        let a = parse_attribute_line(line).unwrap();
        assert_eq!(
            a.ty,
            TypeRef {
                name: "Trigger".to_string(),
                ref_: Some("../vocabularies/trigger".to_string())
            }
        );
        assert_eq!(render_attribute_line(&a), line);
    }

    #[test]
    fn same_dir_attribute_link_still_round_trips_with_dot_slash() {
        // Regression: bare same-dir slugs keep the `./` prefix on the way out.
        let line = "- status: [OrderStatus](./order-status.md) {0..1}";
        let a = parse_attribute_line(line).unwrap();
        assert_eq!(a.ty.ref_.as_deref(), Some("order-status"));
        assert_eq!(render_attribute_line(&a), line);
    }

    #[test]
    fn parses_and_round_trips_parent_relative_relationship_target() {
        let line = "- associates [Pipeline Step Run](../execution/pipeline-step-run.md) as \"attributedTo\": 0..* to 1";
        let r = parse_relationship_line(line).unwrap();
        assert_eq!(r.target_slug, "../execution/pipeline-step-run");
        assert_eq!(render_relationship_line(&r), line);
    }

    #[test]
    fn parses_and_round_trips_parent_relative_relationship_name_link() {
        let line = "- associates [B](./b.md) as [Assoc](../assoc/link.md): 1 to 1";
        let r = parse_relationship_line(line).unwrap();
        match &r.name {
            Some(ParsedName::Ref { slug, .. }) => assert_eq!(slug, "../assoc/link"),
            other => panic!("expected a name link, got {other:?}"),
        }
        assert_eq!(render_relationship_line(&r), line);
    }

    #[test]
    fn parses_attribute_with_visibility_and_bare_type() {
        let a = parse_attribute_line("- - id: OrderId").unwrap();
        assert_eq!(a.visibility, Some(Visibility::Private));
        assert_eq!(a.name, "id");
        assert_eq!(
            a.ty,
            TypeRef {
                name: "OrderId".to_string(),
                ref_: None
            }
        );
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
        let r = parse_relationship_line(
            "- associates [Customer](./customer.md): 1 order to 1 customer",
        )
        .unwrap();
        assert_eq!(r.kind, RelationshipKind::Associates);
        assert_eq!(r.target_slug, "customer");
        assert_eq!(
            r.from_end,
            RelEnd {
                multiplicity: Multiplicity::parse("1"),
                role: Some("order".to_string()),
                navigable: None
            }
        );
        assert_eq!(r.to_end.role.as_deref(), Some("customer"));
    }

    #[test]
    fn parses_unended_relationship_with_named_link() {
        let r = parse_relationship_line(
            "- specializes [Animal](./animal.md) as [Kinship](./kinship.md)",
        )
        .unwrap();
        assert_eq!(r.kind, RelationshipKind::Specializes);
        assert_eq!(
            r.name,
            Some(ParsedName::Ref {
                title: "Kinship".to_string(),
                slug: "kinship".to_string()
            })
        );
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
            ty: TypeRef {
                name: "OrderId".to_string(),
                ref_: None,
            },
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
    fn instance_of_and_links_relationships_round_trip() {
        for line in [
            "- instance of [Order](./order.md)",
            "- links [order42-line](./order42-line.md) as [Order→OrderLine](./order-orderline-assoc.md)",
        ] {
            let r = parse_relationship_line(line).unwrap();
            assert_eq!(render_relationship_line(&r), line, "must round-trip byte-identically");
        }
        assert_eq!(
            parse_relationship_line("- instance of [Order](./order.md)")
                .unwrap()
                .kind,
            RelationshipKind::InstanceOf
        );
        let links = parse_relationship_line("- links [l](./l.md) as [A](./a.md)").unwrap();
        assert_eq!(links.kind, RelationshipKind::Links);
        assert!(matches!(links.name, Some(ParsedName::Ref { .. })));
    }

    #[test]
    fn parses_nested_member_groups() {
        let content = "### Users\n- [Customer](./customer.md)\n\n#### VIP\n- [Platinum](./platinum.md)\n\n### Orders\n- [Order](./order.md)";
        let block = parse_members_block(content, 0, content);
        assert_eq!(block.groups.len(), 2);
        assert_eq!(block.groups[0].name, "Users");
        assert_eq!(block.groups[0].depth, 3);
        let MemberItem::Member(m) = block.groups[0].members[0].parsed().unwrap() else {
            panic!("expected a plain member")
        };
        assert_eq!(m.slug, "customer");
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
        let FlowBullet::Transition(t) = parse_flow_bullet(
            "- on `ship` when `paid` transitions to Shipped carries [Order](./order.md): `notify`",
        )
        .unwrap() else {
            panic!("expected a transition")
        };
        assert_eq!(t.trigger.as_deref(), Some("ship"));
        assert_eq!(t.guard.as_deref(), Some("paid"));
        assert!(!t.is_else);
        assert_eq!(t.target, FlowTargetRef::Local("Shipped".to_string()));
        assert_eq!(t.carries.as_ref().unwrap().slug, "order");
        assert_eq!(t.effect.as_deref(), Some("notify"));
    }

    #[test]
    fn parses_completion_else_and_link_target_transitions() {
        let FlowBullet::Transition(t) = parse_flow_bullet("- transitions to final").unwrap() else {
            panic!()
        };
        assert_eq!(t.target, FlowTargetRef::Local("final".to_string()));
        assert!(t.trigger.is_none() && t.guard.is_none() && !t.is_else);

        let FlowBullet::Transition(t) = parse_flow_bullet("- else transitions to Hold").unwrap()
        else {
            panic!()
        };
        assert!(t.is_else);

        let FlowBullet::Transition(t) =
            parse_flow_bullet("- transitions to [Fulfilment](./fulfilment.md)").unwrap()
        else {
            panic!()
        };
        assert!(matches!(t.target, FlowTargetRef::Link(ref l) if l.slug == "fulfilment"));
    }

    #[test]
    fn parses_internals_refines_and_partition() {
        assert_eq!(
            parse_flow_bullet("- entry: `reserveStock`").unwrap(),
            FlowBullet::Entry("reserveStock".to_string())
        );
        assert_eq!(
            parse_flow_bullet("- do: `poll`").unwrap(),
            FlowBullet::Do("poll".to_string())
        );
        assert_eq!(
            parse_flow_bullet("- exit: `release`").unwrap(),
            FlowBullet::Exit("release".to_string())
        );
        assert!(
            matches!(parse_flow_bullet("- refines [SubFlow](./sub.md)").unwrap(), FlowBullet::Refines(ref l) if l.slug == "sub")
        );
        assert_eq!(
            parse_flow_bullet("- partition: Warehouse").unwrap(),
            FlowBullet::Partition("Warehouse".to_string())
        );
        assert!(parse_flow_bullet("- goes to X").is_err());
        assert!(
            parse_flow_bullet("- when paid transitions to X").is_err(),
            "guards must be backticked"
        );
    }

    #[test]
    fn parses_flow_headings() {
        assert_eq!(
            parse_flow_heading("Draft"),
            (FlowNodeKind::Plain, "Draft".to_string(), None)
        );
        assert_eq!(
            parse_flow_heading("initial"),
            (FlowNodeKind::Initial, "initial".to_string(), None)
        );
        assert_eq!(
            parse_flow_heading("decision Ready to ship?"),
            (FlowNodeKind::Decision, "Ready to ship?".to_string(), None)
        );
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
            let n = FlowNodeSyntax {
                kind,
                identity,
                object_ref,
                bullets: Vec::new(),
                notes: Vec::new(),
                line: 0,
            };
            assert_eq!(render_flow_heading(&n), format!("### {heading}"));
        }
    }

    use crate::model::{FragmentKind, MessageVerb};
    use crate::syntax::SeqItemSyntax;

    #[test]
    fn parses_lifeline_lines() {
        let l = parse_lifeline_line("- [Order](./order.md) as order").unwrap();
        assert_eq!(l.link.slug, "order");
        assert_eq!(l.alias.as_deref(), Some("order"));
        let l = parse_lifeline_line("- [Customer](./customer.md)").unwrap();
        assert_eq!(l.alias, None);
        assert!(
            parse_lifeline_line("- Customer").is_err(),
            "a lifeline IS a link"
        );
    }

    #[test]
    fn parses_message_lines() {
        let m = parse_message_line("- Customer calls order: `place(items)`").unwrap();
        assert_eq!(m.from, "Customer");
        assert_eq!(m.verb, MessageVerb::Calls);
        assert_eq!(m.to, "order");
        assert_eq!(m.signature.as_deref(), Some("place(items)"));
        let m = parse_message_line("- order replies Customer: `confirmation`").unwrap();
        assert_eq!(m.verb, MessageVerb::Replies);
        assert!(parse_message_line("- Customer shouts order").is_err());
        assert!(parse_message_line("- par").is_err(), "par is deferred");
    }

    #[test]
    fn parses_nested_fragments_in_messages_block() {
        let content = "- Customer calls order: `place(items)`\n- alt\n  - when `paid`\n    - order calls wh: `ship()`\n  - else\n    - order sends Customer: `paymentFailed()`\n- order replies Customer: `confirmation`";
        let block = parse_messages_block(content, 0, content);
        assert_eq!(block.items.len(), 3);
        let SeqItemSyntax::Fragment { kind, operands, .. } = block.items[1].parsed().unwrap()
        else {
            panic!("expected a fragment")
        };
        assert_eq!(*kind, FragmentKind::Alt);
        assert_eq!(operands.len(), 2);
        assert_eq!(operands[0].guard.as_deref(), Some("paid"));
        assert_eq!(operands[1].guard, None); // else
        let SeqItemSyntax::Message(m) = operands[0].items[0].parsed().unwrap() else {
            panic!()
        };
        assert_eq!(m.to, "wh");
    }

    #[test]
    fn messages_block_round_trips() {
        let content = "- Customer calls order: `place(items)`\n- alt\n  - when `paid`\n    - order calls wh: `ship()`\n  - else\n    - order sends Customer: `paymentFailed()`\n- order replies Customer: `confirmation`";
        let block = parse_messages_block(content, 0, content);
        let rendered = render_messages_block(&block);
        let body = rendered.strip_prefix("## Messages\n").unwrap();
        assert_eq!(parse_messages_block(body, 0, body), block);
        assert_eq!(body, content);
    }

    #[test]
    fn misplaced_operand_and_unknown_fragment_degrade_to_error_lines() {
        let content = "- when `paid`\n- par\n- Customer calls order";
        let block = parse_messages_block(content, 0, content);
        assert_eq!(block.items.len(), 3);
        assert!(
            block.items[0].parsed().is_none(),
            "operand outside a fragment is an error line"
        );
        assert!(
            block.items[1].parsed().is_none(),
            "'par' is deferred and degrades"
        );
        assert!(block.items[2].parsed().is_some());
    }
}
