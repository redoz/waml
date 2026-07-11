# UAML Rust Tooling — Plan 2: Document Tier (parse ↔ serialize)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the per-file fidelity tier of `uaml`: a `Document` AST, a `pulldown-cmark`-driven parser (`parse_document`), the line grammar (attribute/value/relationship/member/hint parsers + renderers), a bundle splitter, and a canonical serializer (`serialize_document`) — enough to implement `fmt` end-to-end (minus CLI I/O).

**Architecture:** `pulldown-cmark` parses the markdown *body* (after frontmatter is split off by Plan 1's `parse_frontmatter`). Its event stream gives us the H1 title and H2 section boundaries — respecting code fences, which naive line-splitting would not. Within a recognized section, items are single lines by our grammar, so we line-split the section's source and apply the line grammar. Unrecognized sections are preserved verbatim (graceful degradation). The serializer renders a `Document` back to canonical markdown, matching the TypeScript reference's canonical form (e.g. a default `[1]` multiplicity is omitted).

**Tech Stack:** Rust 2021, `regex`, `pulldown-cmark`. Builds on Plan 1 (`slug`, `frontmatter`, `multiplicity`, `model`).

## Global Constraints

- All Global Constraints from Plan 1 apply (edition 2021, MSRV 1.80, WASM-friendly, UML-only, no OWOX branding/frontmatter, `regex` only + now `pulldown-cmark`).
- **`pulldown-cmark` is added to the core crate.** It is pure Rust and `wasm32`-buildable, preserving the WASM-friendly constraint.
- **Canonical form matches `packages/okf/src/grammar.ts` / `serialize.ts`** so `fmt` output agrees with the web app's serializer: a `[1]` multiplicity is omitted; relationship names render as `as "label"` or `as [Title](./slug.md)`; ended relationships render `: <from> to <to>`.
- Parsing is **best-effort and infallible**: a line that doesn't match the grammar is dropped from the typed section (it will be reported by the Plan 3 validator, which re-reads the source). `parse_document` never panics.

---

### Task 1: `syntax` module — the `Document` AST

**Files:**
- Create: `crates/uaml/src/syntax.rs`
- Modify: `crates/uaml/src/lib.rs`

**Interfaces:**
- Consumes: `crate::model::{Attribute, RelationshipKind, RelEnd}`, `crate::frontmatter::Frontmatter`.
- Produces:
  - `Document { frontmatter: Frontmatter, title: String, sections: Vec<Section> }`
  - `enum Section { Attributes(Vec<Attribute>), Values(Vec<String>), Relationships(Vec<ParsedRel>), Body(String), Notes(Vec<String>), Members(Vec<MemberLine>), RenderHints(Vec<HintLine>), Unknown { title: String, raw: String } }`
  - `enum ParsedName { Label(String), Ref { title: String, slug: String } }`
  - `struct ParsedRel { kind, target_title, target_slug, name: Option<ParsedName>, from_end: RelEnd, to_end: RelEnd }`
  - `struct MemberLine { title: String, slug: String, position: Option<(f64, f64)> }`
  - `enum HintLine { Emphasize(Vec<String>), Collapse { title: String, slug: String } }`

These are plain data types; the tests just confirm they compile and are constructible. The grammar (Task 2) produces `ParsedRel`/`MemberLine`/`HintLine`; the parser (Task 3) produces `Document`.

- [ ] **Step 1: Write the failing test**

`crates/uaml/src/syntax.rs`:
```rust
use crate::frontmatter::Frontmatter;
use crate::model::{Attribute, RelEnd, RelationshipKind};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_is_constructible() {
        let doc = Document {
            frontmatter: Frontmatter::default(),
            title: "Order".to_string(),
            sections: vec![Section::Relationships(vec![ParsedRel {
                kind: RelationshipKind::Composes,
                target_title: "OrderLine".to_string(),
                target_slug: "order-line".to_string(),
                name: None,
                from_end: RelEnd::default(),
                to_end: RelEnd::default(),
            }])],
        };
        assert_eq!(doc.title, "Order");
        assert_eq!(doc.sections.len(), 1);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p uaml syntax`
Expected: FAIL — types not found.

- [ ] **Step 3: Implement the types**

Prepend to `crates/uaml/src/syntax.rs`:
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub frontmatter: Frontmatter,
    pub title: String,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Section {
    Attributes(Vec<Attribute>),
    Values(Vec<String>),
    Relationships(Vec<ParsedRel>),
    Body(String),
    Notes(Vec<String>),
    Members(Vec<MemberLine>),
    RenderHints(Vec<HintLine>),
    /// An unrecognized `## Section`, preserved verbatim (graceful degradation).
    Unknown { title: String, raw: String },
}

/// A relationship's optional `as …` name, as written in one document.
#[derive(Debug, Clone, PartialEq)]
pub enum ParsedName {
    Label(String),
    Ref { title: String, slug: String },
}

/// One `## Relationships` bullet, parsed but not yet resolved against the bundle.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedRel {
    pub kind: RelationshipKind,
    pub target_title: String,
    pub target_slug: String,
    pub name: Option<ParsedName>,
    pub from_end: RelEnd,
    pub to_end: RelEnd,
}

/// One `## Members` bullet in a diagram document.
#[derive(Debug, Clone, PartialEq)]
pub struct MemberLine {
    pub title: String,
    pub slug: String,
    pub position: Option<(f64, f64)>,
}

/// One `## Render hints` bullet.
#[derive(Debug, Clone, PartialEq)]
pub enum HintLine {
    Emphasize(Vec<String>),
    Collapse { title: String, slug: String },
}
```

- [ ] **Step 4: Wire the module in**

In `crates/uaml/src/lib.rs`, add:
```rust
pub mod syntax;
```

- [ ] **Step 5: Run to verify passing**

Run: `cargo test -p uaml syntax`
Expected: PASS — `1 passed`.

- [ ] **Step 6: Commit**

```bash
git add crates/uaml/src/syntax.rs crates/uaml/src/lib.rs
git commit -m "feat(uaml): Document AST types"
```

---

### Task 2: `grammar` module — line parsers and renderers

**Files:**
- Create: `crates/uaml/src/grammar.rs`
- Modify: `crates/uaml/src/lib.rs`

**Interfaces:**
- Consumes: `crate::model::{Attribute, TypeRef, Visibility, RelationshipKind, RelEnd}`, `crate::multiplicity::Multiplicity`, `crate::syntax::{ParsedRel, ParsedName, MemberLine, HintLine}`.
- Produces:
  - `parse_attribute_line(&str) -> Option<Attribute>`
  - `parse_value_line(&str) -> Option<String>`
  - `parse_relationship_line(&str) -> Option<ParsedRel>`
  - `parse_member_line(&str) -> Option<MemberLine>`
  - `parse_hint_line(&str) -> Option<HintLine>`
  - `render_attribute_line(&Attribute) -> String`
  - `render_relationship_line(&ParsedRel) -> String`
  - `render_member_line(&MemberLine) -> String`
  - `render_hint_line(&HintLine) -> String`

Regexes mirror `packages/okf/src/grammar.ts` and `parse.ts`. At the Document tier a type/target link's slug is stored **as written** (unresolved): `TypeRef.ref_ = Some(slug)`; resolution against the bundle happens in Plan 3.

- [ ] **Step 1: Write the failing tests**

`crates/uaml/src/grammar.rs`:
```rust
use std::sync::LazyLock;
use regex::Regex;

use crate::model::{Attribute, RelEnd, RelationshipKind, TypeRef, Visibility};
use crate::multiplicity::Multiplicity;
use crate::syntax::{HintLine, MemberLine, ParsedName, ParsedRel};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_attribute_with_link_and_multiplicity() {
        let a = parse_attribute_line("- status: [OrderStatus](./order-status.md) [0..1]").unwrap();
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
        assert!(parse_attribute_line("- x: [Broken]").is_none());
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
        assert!(parse_relationship_line("- specializes [Animal](./animal.md): 1 to 1").is_none());
        assert!(parse_relationship_line("- composes [OrderLine](./order-line.md)").is_none());
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
    fn parses_and_renders_member_with_position() {
        let m = parse_member_line("- [Order](./order.md) at 40,80").unwrap();
        assert_eq!(m.slug, "order");
        assert_eq!(m.position, Some((40.0, 80.0)));
        assert_eq!(render_member_line(&m), "- [Order](./order.md) at 40,80");
    }

    #[test]
    fn parses_hint_lines() {
        assert_eq!(parse_hint_line("- emphasize: order, customer"),
            Some(HintLine::Emphasize(vec!["order".to_string(), "customer".to_string()])));
        assert_eq!(parse_hint_line("- collapse [Pricing](./pricing-service.md)"),
            Some(HintLine::Collapse { title: "Pricing".to_string(), slug: "pricing-service".to_string() }));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p uaml grammar`
Expected: FAIL — functions not found.

- [ ] **Step 3: Implement the grammar**

Prepend to `crates/uaml/src/grammar.rs`:
```rust
static ATTR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- (?:([+\-#~]) )?([A-Za-z_][A-Za-z0-9_]*): (.+)$").unwrap());
static LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\[([^\]]+)\]\(\./(.+?)\.md\)$").unwrap());
static MULT_TAIL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.*?)\s+\[([^\]]+)\]$").unwrap());
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
static MEMBER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^- \[([^\]]*)\]\(\./(.+?)\.md\)(?:\s+at\s+(-?\d+)\s*,\s*(-?\d+))?\s*$").unwrap()
});
static EMPHASIZE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- emphasize:\s*(.+)$").unwrap());
static COLLAPSE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- collapse \[([^\]]*)\]\(\./(.+?)\.md\)\s*$").unwrap());
static STRAY_BRACKET_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\[\]()]").unwrap());

/// Strip a directory prefix and the `.md` suffix from a link path.
fn basename(path: &str) -> &str {
    let after_slash = path.rsplit(['/', '\\']).next().unwrap_or(path);
    after_slash.strip_suffix(".md").unwrap_or(after_slash)
}

pub fn parse_attribute_line(line: &str) -> Option<Attribute> {
    let line = line.trim_end_matches('\r').trim();
    let caps = ATTR_RE.captures(line)?;
    let visibility = caps.get(1).and_then(|m| Visibility::from_marker(m.as_str().chars().next()?));
    let name = caps[2].to_string();
    let mut rest = caps[3].trim().to_string();
    let mut multiplicity = Multiplicity::default();
    if let Some(mm) = MULT_TAIL_RE.captures(&rest) {
        if let Some(m) = Multiplicity::parse(&mm[2]) {
            multiplicity = m;
            rest = mm[1].trim().to_string();
        }
    }
    let ty = if let Some(link) = LINK_RE.captures(&rest) {
        TypeRef { name: link[1].to_string(), ref_: Some(basename(&link[2]).to_string()) }
    } else {
        if rest.is_empty() || STRAY_BRACKET_RE.is_match(&rest) {
            return None; // malformed link / stray brackets → not an attribute
        }
        TypeRef { name: rest, ref_: None }
    };
    Some(Attribute { name, ty, multiplicity, visibility, description: None })
}

pub fn parse_value_line(line: &str) -> Option<String> {
    let line = line.trim_end_matches('\r').trim();
    VALUE_RE.captures(line).map(|c| c[1].trim().to_string())
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

pub fn parse_relationship_line(line: &str) -> Option<ParsedRel> {
    let line = line.trim_end_matches('\r').trim();
    let m = REL_RE.captures(line)?;
    let kind = RelationshipKind::parse(&m[1])?;
    let ends_raw = m.get(7).map(|x| x.as_str());
    if kind.is_ended() != ends_raw.is_some() {
        return None; // ends required XOR forbidden
    }
    let name = if let Some(label) = m.get(4) {
        Some(ParsedName::Label(label.as_str().to_string()))
    } else if let (Some(t), Some(s)) = (m.get(5), m.get(6)) {
        Some(ParsedName::Ref { title: t.as_str().to_string(), slug: basename(s.as_str()).to_string() })
    } else {
        None
    };
    let (from_end, to_end) = if let Some(raw) = ends_raw {
        let parts: Vec<&str> = raw.split(" to ").collect();
        if parts.len() != 2 {
            return None;
        }
        (parse_end(parts[0])?, parse_end(parts[1])?)
    } else {
        (RelEnd::default(), RelEnd::default())
    };
    Some(ParsedRel {
        kind,
        target_title: m[2].to_string(),
        target_slug: basename(&m[3]).to_string(),
        name,
        from_end,
        to_end,
    })
}

pub fn parse_member_line(line: &str) -> Option<MemberLine> {
    let line = line.trim_end_matches('\r').trim();
    let m = MEMBER_RE.captures(line)?;
    let position = match (m.get(3), m.get(4)) {
        (Some(x), Some(y)) => Some((x.as_str().parse().ok()?, y.as_str().parse().ok()?)),
        _ => None,
    };
    Some(MemberLine { title: m[1].to_string(), slug: basename(&m[2]).to_string(), position })
}

pub fn parse_hint_line(line: &str) -> Option<HintLine> {
    let line = line.trim_end_matches('\r').trim();
    if let Some(m) = EMPHASIZE_RE.captures(line) {
        let items = m[1].split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        return Some(HintLine::Emphasize(items));
    }
    if let Some(m) = COLLAPSE_RE.captures(line) {
        return Some(HintLine::Collapse { title: m[1].to_string(), slug: basename(&m[2]).to_string() });
    }
    None
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
        format!(" [{}]", a.multiplicity.as_str())
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
    let at = match m.position {
        Some((x, y)) => format!(" at {},{}", x.round() as i64, y.round() as i64),
        None => String::new(),
    };
    format!("- [{}](./{}.md){at}", m.title, m.slug)
}

pub fn render_hint_line(h: &HintLine) -> String {
    match h {
        HintLine::Emphasize(items) => format!("- emphasize: {}", items.join(", ")),
        HintLine::Collapse { title, slug } => format!("- collapse [{title}](./{slug}.md)"),
    }
}
```

- [ ] **Step 4: Wire the module in**

In `crates/uaml/src/lib.rs`, add:
```rust
pub mod grammar;
```

- [ ] **Step 5: Run to verify passing**

Run: `cargo test -p uaml grammar`
Expected: PASS — `10 passed`.

- [ ] **Step 6: Commit**

```bash
git add crates/uaml/src/grammar.rs crates/uaml/src/lib.rs
git commit -m "feat(uaml): line grammar parsers and renderers"
```

---

### Task 3: `parse` module — markdown body → `Document`

**Files:**
- Create: `crates/uaml/src/parse.rs`
- Modify: `crates/uaml/src/lib.rs`, `Cargo.toml`, `crates/uaml/Cargo.toml`

**Interfaces:**
- Consumes: `pulldown-cmark`, `crate::frontmatter::parse_frontmatter`, `crate::grammar::*`, `crate::syntax::{Document, Section}`.
- Produces: `pub fn parse_document(src: &str) -> Document`.

`pulldown-cmark` parses the body (after frontmatter split). The first `#` heading is the title; each `##` heading opens a section whose content runs to the next `##` (or end). Headings inside code fences are correctly ignored by the parser. Known sections parse their `-` lines via the grammar; unknown sections keep their raw source.

- [ ] **Step 1: Add the dependency**

In root `Cargo.toml` under `[workspace.dependencies]`:
```toml
pulldown-cmark = { version = "0.12", default-features = false }
```

In `crates/uaml/Cargo.toml` under `[dependencies]`:
```toml
pulldown-cmark = { workspace = true }
```

- [ ] **Step 2: Write the failing tests**

`crates/uaml/src/parse.rs`:
```rust
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::frontmatter::parse_frontmatter;
use crate::grammar::{
    parse_attribute_line, parse_hint_line, parse_member_line, parse_relationship_line,
    parse_value_line,
};
use crate::syntax::{Document, Section};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::RelationshipKind;

    const ORDER: &str = "---\ntype: uml.Class\nstereotype: [aggregateRoot, entity]\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n- status: [OrderStatus](./order-status.md) [0..1]\n\n## Relationships\n- composes [OrderLine](./order-line.md): 1 to 1..* lines\n\n## Provenance\nHand-authored. Keep me.\n";

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
        assert_eq!(attrs[1].ty.ref_.as_deref(), Some("order-status"));
        let rels = doc.sections.iter().find_map(|s| match s {
            Section::Relationships(r) => Some(r),
            _ => None,
        }).unwrap();
        assert_eq!(rels[0].kind, RelationshipKind::Composes);
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
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p uaml parse`
Expected: FAIL — `parse_document` not found.

- [ ] **Step 4: Implement `parse_document`**

Prepend to `crates/uaml/src/parse.rs`:
```rust
struct Head {
    title: String,
    heading_start: usize,
    content_start: usize,
}

fn classify(title: &str, content: &str, raw_full: &str) -> Section {
    let lines = |c: &str| c.lines().map(|l| l.to_string()).collect::<Vec<_>>();
    match title.to_lowercase().as_str() {
        "attributes" => {
            Section::Attributes(lines(content).iter().filter_map(|l| parse_attribute_line(l)).collect())
        }
        "values" => {
            Section::Values(lines(content).iter().filter_map(|l| parse_value_line(l)).collect())
        }
        "relationships" => {
            Section::Relationships(lines(content).iter().filter_map(|l| parse_relationship_line(l)).collect())
        }
        "members" => {
            Section::Members(lines(content).iter().filter_map(|l| parse_member_line(l)).collect())
        }
        "render hints" => {
            Section::RenderHints(lines(content).iter().filter_map(|l| parse_hint_line(l)).collect())
        }
        "body" => Section::Body(content.trim().to_string()),
        "notes" => {
            Section::Notes(lines(content).iter().filter_map(|l| parse_value_line(l)).collect())
        }
        _ => Section::Unknown { title: title.to_string(), raw: raw_full.trim_end().to_string() },
    }
}

pub fn parse_document(src: &str) -> Document {
    let (frontmatter, body) = parse_frontmatter(src);
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
        let content = body[head.content_start..end].trim();
        let raw_full = &body[head.heading_start..end];
        sections.push(classify(&head.title, content, raw_full));
    }

    Document { frontmatter, title: title.trim().to_string(), sections }
}
```

- [ ] **Step 5: Run to verify passing**

Run: `cargo test -p uaml parse`
Expected: PASS — `3 passed`.

- [ ] **Step 6: Wire the module in and commit**

In `crates/uaml/src/lib.rs`, add:
```rust
pub mod parse;
```

```bash
git add Cargo.toml crates/uaml/Cargo.toml crates/uaml/src/parse.rs crates/uaml/src/lib.rs
git commit -m "feat(uaml): parse markdown body into Document on pulldown-cmark"
```

---

### Task 4: bundle splitter

**Files:**
- Modify: `crates/uaml/src/parse.rs`

**Interfaces:**
- Produces: `pub fn split_bundle(text: &str) -> Vec<(String, String)>` — split a concatenated blob on `<!-- path/slug.md -->` markers into `(path, content)` pairs; a blob with no markers returns a single `("pasted/doc.md", text)`.

Mirrors `packages/web/src/okf/io.ts:62-68`.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `crates/uaml/src/parse.rs`:
```rust
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
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p uaml parse::tests::splits_blob_on_markers`
Expected: FAIL — `split_bundle` not found.

- [ ] **Step 3: Implement `split_bundle`**

Add to `crates/uaml/src/parse.rs` (above the test module):
```rust
static MARKER_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?m)^<!--\s*(.+?)\s*-->[ \t]*\n").unwrap());

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
```

- [ ] **Step 4: Run to verify passing**

Run: `cargo test -p uaml parse`
Expected: PASS — `5 passed` (3 from Task 3 + 2 here).

- [ ] **Step 5: Commit**

```bash
git add crates/uaml/src/parse.rs
git commit -m "feat(uaml): split concatenated bundle blobs on path markers"
```

---

### Task 5: `serialize` module — `Document` → canonical markdown

**Files:**
- Create: `crates/uaml/src/serialize.rs`
- Modify: `crates/uaml/src/lib.rs`

**Interfaces:**
- Consumes: `crate::frontmatter::render_frontmatter`, `crate::grammar::*`, `crate::syntax::{Document, Section}`.
- Produces: `pub fn serialize_document(doc: &Document) -> String`.

Canonical order (matching `serialize.ts` `renderNode`, generalized over classifier and diagram docs): Body, Attributes, Values, Relationships, Notes, Members, Render hints, then any Unknown sections in their original relative order. Frontmatter is rendered from the document's own (order-preserving) `Frontmatter`; the title comes from `doc.title`.

- [ ] **Step 1: Write the failing tests**

`crates/uaml/src/serialize.rs`:
```rust
use crate::frontmatter::render_frontmatter;
use crate::grammar::{
    render_attribute_line, render_hint_line, render_member_line, render_relationship_line,
};
use crate::syntax::{Document, Section};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_document;

    const ORDER: &str = "---\ntype: uml.Class\nstereotype: [aggregateRoot, entity]\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n- status: [OrderStatus](./order-status.md) [0..1]\n\n## Relationships\n- composes [OrderLine](./order-line.md): 1 to 1..* lines\n\n## Provenance\nHand-authored. Keep me.\n";

    #[test]
    fn serialize_is_a_semantic_fixpoint() {
        let once = serialize_document(&parse_document(ORDER));
        let twice = serialize_document(&parse_document(&once));
        assert_eq!(once, twice);
    }

    #[test]
    fn serialize_preserves_unknown_section() {
        let out = serialize_document(&parse_document(ORDER));
        assert!(out.contains("## Provenance\nHand-authored. Keep me."));
    }

    #[test]
    fn serialize_omits_default_multiplicity() {
        let out = serialize_document(&parse_document(ORDER));
        assert!(out.contains("- id: OrderId\n"));
        assert!(out.contains("- status: [OrderStatus](./order-status.md) [0..1]"));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p uaml serialize`
Expected: FAIL — `serialize_document` not found.

- [ ] **Step 3: Implement `serialize_document`**

Prepend to `crates/uaml/src/serialize.rs`:
```rust
fn section_order(s: &Section) -> u8 {
    match s {
        Section::Body(_) => 0,
        Section::Attributes(_) => 1,
        Section::Values(_) => 2,
        Section::Relationships(_) => 3,
        Section::Notes(_) => 4,
        Section::Members(_) => 5,
        Section::RenderHints(_) => 6,
        Section::Unknown { .. } => 7,
    }
}

fn render_section(s: &Section) -> String {
    match s {
        Section::Body(body) => format!("## Body\n{body}"),
        Section::Attributes(attrs) => {
            let body = attrs.iter().map(render_attribute_line).collect::<Vec<_>>().join("\n");
            format!("## Attributes\n{body}")
        }
        Section::Values(values) => {
            let body = values.iter().map(|v| format!("- {v}")).collect::<Vec<_>>().join("\n");
            format!("## Values\n{body}")
        }
        Section::Relationships(rels) => {
            let body = rels.iter().map(render_relationship_line).collect::<Vec<_>>().join("\n");
            format!("## Relationships\n{body}")
        }
        Section::Notes(notes) => {
            let body = notes.iter().map(|n| format!("- {n}")).collect::<Vec<_>>().join("\n");
            format!("## Notes\n{body}")
        }
        Section::Members(members) => {
            let body = members.iter().map(render_member_line).collect::<Vec<_>>().join("\n");
            format!("## Members\n{body}")
        }
        Section::RenderHints(hints) => {
            let body = hints.iter().map(render_hint_line).collect::<Vec<_>>().join("\n");
            format!("## Render hints\n{body}")
        }
        Section::Unknown { raw, .. } => raw.trim_end().to_string(),
    }
}

pub fn serialize_document(doc: &Document) -> String {
    let mut ordered: Vec<&Section> = doc.sections.iter().collect();
    // Stable sort keeps Unknown sections in their original relative order.
    ordered.sort_by_key(|s| section_order(s));

    let fm = render_frontmatter(&doc.frontmatter);
    let mut out = format!("---\n{fm}\n---\n\n# {}\n", doc.title);
    for s in ordered {
        out.push('\n');
        out.push_str(&render_section(s));
        out.push('\n');
    }
    out
}
```

- [ ] **Step 4: Wire the module in**

In `crates/uaml/src/lib.rs`, add:
```rust
pub mod serialize;
```

- [ ] **Step 5: Run to verify passing**

Run: `cargo test -p uaml serialize`
Expected: PASS — `3 passed`.

- [ ] **Step 6: Run the whole crate suite**

Run: `cargo test -p uaml`
Expected: PASS — all Plan 1 + Plan 2 tests green.

- [ ] **Step 7: Commit**

```bash
git add crates/uaml/src/serialize.rs crates/uaml/src/lib.rs
git commit -m "feat(uaml): serialize Document to canonical markdown"
```

---

## Self-Review

- **Spec coverage (this plan's slice):** two-tier AST — `Document` tier built here ✔ (Tasks 1, 3); grammar mirroring the reference ✔ (Task 2); parse pipeline `markdown events → Document` on `pulldown-cmark` ✔ (Task 3); bundle blob split ✔ (Task 4); serialize with canonical form + **unknown sections preserved verbatim** (graceful degradation) ✔ (Tasks 3, 5); markdown-clean structure comes for free because `pulldown-cmark` (not line-splitting) finds sections, correctly ignoring code fences ✔ (Task 3 test). The `Model` tier, edge/bidirectional resolution, diagrams, note desugaring, the golden bundle fixture, validation, and the CLI are deferred to Plans 3 and 4.
- **Placeholder scan:** none — every step has concrete code and exact commands.
- **Type consistency:** `parse_document`/`split_bundle` (Plan 2) feed `Document`/`Section`/`ParsedRel` (Task 1) which the grammar (Task 2) and serializer (Task 5) consume with matching signatures. `Attribute`/`RelEnd`/`Multiplicity`/`Visibility`/`RelationshipKind` names match Plan 1 exactly. `serialize_document` and `parse_document` are the entry points Plan 4's `fmt` will call.
- **Canonical-form note:** the round-trip is a *semantic fixpoint* (`serialize(parse(x))` is idempotent), not byte-identity with the hand-authored `okf-format.md` example, because canonical form omits a default `[1]` — consistent with the TS serializer. Verified by `serialize_is_a_semantic_fixpoint`.
