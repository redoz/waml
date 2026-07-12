use std::sync::LazyLock;
use regex::Regex;

use crate::model::{Attribute, RelEnd, RelationshipKind, TypeRef, Visibility};
use crate::multiplicity::Multiplicity;
use crate::syntax::{MemberGroup, MemberLine, MembersBlock, ParsedName, ParsedRel};

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
        r"^- (associates|aggregates|composes|specializes|implements|depends) ",
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

/// Strip a directory prefix and the `.md` suffix from a link path.
fn basename(path: &str) -> &str {
    let after_slash = path.rsplit(['/', '\\']).next().unwrap_or(path);
    after_slash.strip_suffix(".md").unwrap_or(after_slash)
}

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
        TypeRef { name: link[1].to_string(), ref_: Some(basename(&link[2]).to_string()) }
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
    if kind.is_ended() != ends_raw.is_some() {
        return Err(err()); // ends required XOR forbidden
    }
    let name = if let Some(label) = m.get(4) {
        Some(ParsedName::Label(label.as_str().to_string()))
    } else if let (Some(t), Some(s)) = (m.get(5), m.get(6)) {
        Some(ParsedName::Ref { title: t.as_str().to_string(), slug: basename(s.as_str()).to_string() })
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
        target_slug: basename(&m[3]).to_string(),
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
        slug: basename(&m[2]).to_string(),
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

/// Parse the raw text under `## Members` into a group forest.
pub fn parse_members_block(content: &str) -> MembersBlock {
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

    for raw in content.lines() {
        let line = raw.trim_end_matches('\r');
        let t = line.trim_start();
        if let Some((depth, name)) = heading_depth(t) {
            close_to(&mut stack, &mut groups, depth);
            stack.push(MemberGroup { name, depth, members: vec![], children: vec![] });
        } else if let Ok(m) = parse_member_line(t) {
            match stack.last_mut() {
                Some(g) => g.members.push(crate::syntax::Line::Parsed(m)),
                None => implicit.members.push(crate::syntax::Line::Parsed(m)),
            }
        }
        // blank / unrecognized lines are ignored here (validate flags droppable content)
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
    if !r.kind.is_ended() {
        format!("- {} {link}{name}", r.kind.as_str())
    } else {
        format!("- {} {link}{name}: {} to {}", r.kind.as_str(), render_end(&r.from_end), render_end(&r.to_end))
    }
}

pub fn render_member_line(m: &MemberLine) -> String {
    format!("- [{}](./{}.md)", m.title, m.slug)
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
        let block = parse_members_block(content);
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
        let block = parse_members_block(content);
        assert_eq!(block.groups.len(), 1);
        assert_eq!(block.groups[0].name, "");
        assert_eq!(block.groups[0].depth, 0);
        assert_eq!(block.groups[0].members.len(), 2);

        let rendered = render_members_block(&block);
        let reparsed = parse_members_block(rendered.strip_prefix("## Members\n").unwrap());
        assert_eq!(block, reparsed);
    }

    #[test]
    fn member_line_has_no_position() {
        let m = parse_member_line("- [Order](./order.md)").unwrap();
        assert_eq!(m.slug, "order");
        assert_eq!(render_member_line(&m), "- [Order](./order.md)");
    }
}
