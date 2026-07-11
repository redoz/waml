# UAML Rust Tooling — Plan 1: Core Foundations

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up the `/crates` Cargo workspace and the `uaml` core library's primitive modules — slug, frontmatter, multiplicity, and the resolved data model types — with full unit-test coverage.

**Architecture:** A native, WASM-friendly Rust library crate `uaml` at `crates/uaml`. This plan builds only the leaf primitives that later plans (parser, serializer, validator) depend on. No markdown parsing, no filesystem, no I/O yet. Regexes mirror the TypeScript reference in `packages/okf/src/` so behavior stays in parity.

**Tech Stack:** Rust (edition 2021), `regex` crate, `std::sync::LazyLock`. Design doc: `docs/superpowers/specs/2026-07-11-uaml-rust-tooling-design.md`.

## Global Constraints

- Rust edition **2021**; MSRV **1.80** (for `std::sync::LazyLock`).
- Core crate `uaml` dependencies limited to **`regex`** only (WASM-compatible). No `serde`, no `tokio`, no I/O crates.
- **No `std::fs`, no threads, no OS calls** anywhere in `uaml` — it must stay `wasm32`-buildable.
- **UML-only.** No OWOX "Data Mart" legacy, no Google OKF v0.1 fallback.
- **No OWOX branding** and **no `owox.*` frontmatter** support. A node's key is always its filename slug; positions come only from diagram `## Members`.
- Reference implementation for parity: `packages/okf/src/{slug,grammar,types}.ts`.

---

### Task 1: Cargo workspace + `uaml` crate skeleton

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `crates/uaml/Cargo.toml`
- Create: `crates/uaml/src/lib.rs`
- Modify: `.gitignore`

**Interfaces:**
- Consumes: nothing.
- Produces: a buildable `uaml` library crate; `cargo test` runs from repo root.

- [ ] **Step 1: Create the workspace manifest**

`Cargo.toml`:
```toml
[workspace]
resolver = "2"
members = ["crates/uaml"]

[workspace.package]
edition = "2021"
rust-version = "1.80"
license = "Apache-2.0"

[workspace.dependencies]
regex = "1"
```

- [ ] **Step 2: Create the crate manifest**

`crates/uaml/Cargo.toml`:
```toml
[package]
name = "uaml"
version = "0.0.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "Core library for parsing, serializing, and validating UAML documents."

[dependencies]
regex.workspace = true
```

- [ ] **Step 3: Create the crate root with a smoke test**

`crates/uaml/src/lib.rs`:
```rust
//! Core library for UAML: a UML-profile authoring format layered on CommonMark.

#[cfg(test)]
mod smoke {
    #[test]
    fn workspace_builds() {
        assert_eq!(2 + 2, 4);
    }
}
```

- [ ] **Step 4: Ignore the Cargo target directory**

Append to `.gitignore`:
```
/target
```

- [ ] **Step 5: Build and test**

Run: `cargo test -p uaml`
Expected: compiles; `1 passed`.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/uaml/Cargo.toml crates/uaml/src/lib.rs .gitignore
git commit -m "feat(uaml): scaffold Cargo workspace and core crate"
```

---

### Task 2: `slug` module — slugify

**Files:**
- Create: `crates/uaml/src/slug.rs`
- Modify: `crates/uaml/src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pub fn slugify(text: &str, fallback: &str) -> String`.

Behavior copied verbatim from `packages/okf/src/slug.ts:1-9`: split camelCase and acronym boundaries, lowercase, collapse non-alphanumerics to `-`, trim leading/trailing `-`, fall back if empty.

- [ ] **Step 1: Write the failing tests**

`crates/uaml/src/slug.rs`:
```rust
use std::sync::LazyLock;
use regex::Regex;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowercases_and_hyphenates_spaces() {
        assert_eq!(slugify("Order Line", ""), "order-line");
    }

    #[test]
    fn splits_camel_case_boundary() {
        assert_eq!(slugify("OrderStatus", ""), "order-status");
    }

    #[test]
    fn splits_acronym_boundary() {
        assert_eq!(slugify("HTTPServer", ""), "http-server");
    }

    #[test]
    fn trims_and_collapses_punctuation() {
        assert_eq!(slugify("  Order__Line!! ", ""), "order-line");
    }

    #[test]
    fn empty_uses_fallback() {
        assert_eq!(slugify("---", "doc"), "doc");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p uaml slug`
Expected: FAIL — `cannot find function slugify`.

- [ ] **Step 3: Implement `slugify`**

Prepend to `crates/uaml/src/slug.rs` (above the `#[cfg(test)]` block):
```rust
static CAMEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([a-z0-9])([A-Z])").unwrap());
static ACRONYM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([A-Z]+)([A-Z][a-z])").unwrap());
static NON_ALNUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-z0-9]+").unwrap());
static EDGE_DASH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^-+|-+$").unwrap());

/// Slugify a title. Mirrors packages/okf/src/slug.ts.
pub fn slugify(text: &str, fallback: &str) -> String {
    let s = CAMEL.replace_all(text, "$1-$2");
    let s = ACRONYM.replace_all(&s, "$1-$2");
    let s = s.to_lowercase();
    let s = NON_ALNUM.replace_all(&s, "-");
    let s = EDGE_DASH.replace_all(&s, "").into_owned();
    if s.is_empty() { fallback.to_string() } else { s }
}
```

- [ ] **Step 4: Wire the module into the crate**

In `crates/uaml/src/lib.rs`, add near the top:
```rust
pub mod slug;
```

- [ ] **Step 5: Run to verify passing**

Run: `cargo test -p uaml slug`
Expected: PASS — `5 passed`.

- [ ] **Step 6: Commit**

```bash
git add crates/uaml/src/slug.rs crates/uaml/src/lib.rs
git commit -m "feat(uaml): slugify with camelCase and acronym boundaries"
```

---

### Task 3: `frontmatter` module — parse & render mini-YAML

**Files:**
- Create: `crates/uaml/src/frontmatter.rs`
- Modify: `crates/uaml/src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub enum FmValue { Str(String), Bool(bool), Num(f64), List(Vec<FmValue>) }`
  - `pub struct Frontmatter { pub entries: Vec<(String, FmValue)> }` with `get(&self, &str) -> Option<&FmValue>`, `get_str(&self, &str) -> Option<&str>`, `get_string_list(&self, &str) -> Vec<String>`, `get_bool(&self, &str) -> Option<bool>`.
  - `pub fn parse_frontmatter(text: &str) -> (Frontmatter, String)` — returns `(fm, body)`; if no `---` block, `(empty, text)`.
  - `pub fn render_frontmatter(fm: &Frontmatter) -> String` — order-preserving, no trailing newline.

Mirrors the flat subset of `packages/okf/src/slug.ts:29-56` (scalars, `[a, b]` lists, quoted strings, numbers, booleans). Nested-object frontmatter is intentionally unsupported (it only existed for dropped `owox.*` keys). Order is preserved for round-trip fidelity.

- [ ] **Step 1: Write the failing tests**

`crates/uaml/src/frontmatter.rs`:
```rust
use std::sync::LazyLock;
use regex::Regex;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scalars_lists_and_body() {
        let text = "---\ntype: uml.Class\ntitle: Order\nstereotype: [aggregateRoot, entity]\nabstract: true\n---\n# Order\n\nbody text";
        let (fm, body) = parse_frontmatter(text);
        assert_eq!(fm.get_str("type"), Some("uml.Class"));
        assert_eq!(fm.get_str("title"), Some("Order"));
        assert_eq!(fm.get_string_list("stereotype"), vec!["aggregateRoot", "entity"]);
        assert_eq!(fm.get_bool("abstract"), Some(true));
        assert_eq!(body, "# Order\n\nbody text");
    }

    #[test]
    fn no_frontmatter_returns_whole_text_as_body() {
        let (fm, body) = parse_frontmatter("# Just markdown");
        assert!(fm.entries.is_empty());
        assert_eq!(body, "# Just markdown");
    }

    #[test]
    fn parses_quoted_string_with_escapes() {
        let (fm, _) = parse_frontmatter("---\ndescription: \"A \\\"placed\\\" order.\"\n---\n");
        assert_eq!(fm.get_str("description"), Some("A \"placed\" order."));
    }

    #[test]
    fn render_round_trips_order() {
        let text = "---\ntype: uml.Class\nstereotype: [a, b]\ntitle: Order\n---\nbody";
        let (fm, _) = parse_frontmatter(text);
        assert_eq!(
            render_frontmatter(&fm),
            "type: \"uml.Class\"\nstereotype: [\"a\", \"b\"]\ntitle: \"Order\""
        );
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p uaml frontmatter`
Expected: FAIL — types/functions not found.

- [ ] **Step 3: Implement the module**

Prepend to `crates/uaml/src/frontmatter.rs`:
```rust
static BLOCK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)^---\n(.*?)\n---\n?(.*)$").unwrap());
static NUM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^-?\d+(\.\d+)?$").unwrap());

#[derive(Debug, Clone, PartialEq)]
pub enum FmValue {
    Str(String),
    Bool(bool),
    Num(f64),
    List(Vec<FmValue>),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Frontmatter {
    pub entries: Vec<(String, FmValue)>,
}

impl Frontmatter {
    pub fn get(&self, key: &str) -> Option<&FmValue> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
    pub fn get_str(&self, key: &str) -> Option<&str> {
        match self.get(key) {
            Some(FmValue::Str(s)) => Some(s.as_str()),
            _ => None,
        }
    }
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.get(key) {
            Some(FmValue::Bool(b)) => Some(*b),
            _ => None,
        }
    }
    /// A `stereotype` may be a scalar or a list; normalize to a Vec of strings.
    pub fn get_string_list(&self, key: &str) -> Vec<String> {
        match self.get(key) {
            Some(FmValue::List(items)) => items
                .iter()
                .filter_map(|v| match v {
                    FmValue::Str(s) => Some(s.clone()),
                    _ => None,
                })
                .collect(),
            Some(FmValue::Str(s)) if !s.is_empty() => vec![s.clone()],
            _ => Vec::new(),
        }
    }
}

fn parse_value(s: &str) -> FmValue {
    if let Some(inner) = s.strip_prefix('[').and_then(|x| x.strip_suffix(']')) {
        let items = inner
            .split(',')
            .map(|x| parse_value(x.trim()))
            .filter(|v| !matches!(v, FmValue::Str(s) if s.is_empty()))
            .collect();
        return FmValue::List(items);
    }
    if let Some(inner) = s.strip_prefix('"').and_then(|x| x.strip_suffix('"')) {
        return FmValue::Str(inner.replace("\\\"", "\"").replace("\\\\", "\\"));
    }
    if NUM_RE.is_match(s) {
        if let Ok(n) = s.parse::<f64>() {
            return FmValue::Num(n);
        }
    }
    match s {
        "true" => FmValue::Bool(true),
        "false" => FmValue::Bool(false),
        other => FmValue::Str(other.to_string()),
    }
}

pub fn parse_frontmatter(text: &str) -> (Frontmatter, String) {
    let caps = match BLOCK_RE.captures(text) {
        Some(c) => c,
        None => return (Frontmatter::default(), text.to_string()),
    };
    let mut entries = Vec::new();
    for raw in caps[1].split('\n') {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let Some(ci) = line.find(':') else { continue };
        let key = line[..ci].trim().to_string();
        let rest = line[ci + 1..].trim();
        if rest.is_empty() {
            continue; // nested-object frontmatter unsupported (UML-only, flat)
        }
        entries.push((key, parse_value(rest)));
    }
    (Frontmatter { entries }, caps[2].to_string())
}

fn scalar(v: &FmValue) -> String {
    match v {
        FmValue::Num(n) => {
            if n.fract() == 0.0 {
                format!("{}", *n as i64)
            } else {
                format!("{n}")
            }
        }
        FmValue::Bool(b) => b.to_string(),
        FmValue::Str(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
        FmValue::List(_) => unreachable!("lists are not scalars"),
    }
}

pub fn render_frontmatter(fm: &Frontmatter) -> String {
    fm.entries
        .iter()
        .map(|(k, v)| match v {
            FmValue::List(items) => {
                let inner = items.iter().map(scalar).collect::<Vec<_>>().join(", ");
                format!("{k}: [{inner}]")
            }
            other => format!("{k}: {}", scalar(other)),
        })
        .collect::<Vec<_>>()
        .join("\n")
}
```

- [ ] **Step 4: Wire the module in**

In `crates/uaml/src/lib.rs`, add:
```rust
pub mod frontmatter;
```

- [ ] **Step 5: Run to verify passing**

Run: `cargo test -p uaml frontmatter`
Expected: PASS — `4 passed`.

- [ ] **Step 6: Commit**

```bash
git add crates/uaml/src/frontmatter.rs crates/uaml/src/lib.rs
git commit -m "feat(uaml): mini-YAML frontmatter parse and render"
```

---

### Task 4: `multiplicity` module

**Files:**
- Create: `crates/uaml/src/multiplicity.rs`
- Modify: `crates/uaml/src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pub struct Multiplicity(String)` with `parse(&str) -> Option<Multiplicity>`, `as_str(&self) -> &str`, and `Default` = `"1"`.

BNF from `packages/okf/src/grammar.ts:4-12`: `bound | lower..bound`, `lower ::= 0 | posint`, `bound ::= posint | "*"`; a numeric range must satisfy `lower <= upper`.

- [ ] **Step 1: Write the failing tests**

`crates/uaml/src/multiplicity.rs`:
```rust
use std::sync::LazyLock;
use regex::Regex;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_forms() {
        for s in ["1", "0..1", "*", "1..*", "2..5", "10"] {
            assert!(Multiplicity::parse(s).is_some(), "{s} should be valid");
        }
    }

    #[test]
    fn rejects_invalid_forms() {
        for s in ["0", "", "1..", "..5", "-1", "1..2..3", "a", "5..2"] {
            assert!(Multiplicity::parse(s).is_none(), "{s} should be invalid");
        }
    }

    #[test]
    fn round_trips_the_source_string() {
        assert_eq!(Multiplicity::parse("1..*").unwrap().as_str(), "1..*");
    }

    #[test]
    fn default_is_one() {
        assert_eq!(Multiplicity::default().as_str(), "1");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p uaml multiplicity`
Expected: FAIL — `Multiplicity` not found.

- [ ] **Step 3: Implement the type**

Prepend to `crates/uaml/src/multiplicity.rs`:
```rust
static MULT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:[1-9]\d*|\*|(?:0|[1-9]\d*)\.\.(?:[1-9]\d*|\*))$").unwrap()
});
static RANGE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d+)\.\.(\d+)$").unwrap());

/// A UML multiplicity, validated against the BNF and stored in canonical string form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Multiplicity(String);

impl Multiplicity {
    pub fn parse(s: &str) -> Option<Multiplicity> {
        if !MULT_RE.is_match(s) {
            return None;
        }
        if let Some(c) = RANGE_RE.captures(s) {
            let lo: u64 = c[1].parse().ok()?;
            let hi: u64 = c[2].parse().ok()?;
            if lo > hi {
                return None;
            }
        }
        Some(Multiplicity(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Multiplicity {
    fn default() -> Self {
        Multiplicity("1".to_string())
    }
}
```

- [ ] **Step 4: Wire the module in**

In `crates/uaml/src/lib.rs`, add:
```rust
pub mod multiplicity;
```

- [ ] **Step 5: Run to verify passing**

Run: `cargo test -p uaml multiplicity`
Expected: PASS — `4 passed`.

- [ ] **Step 6: Commit**

```bash
git add crates/uaml/src/multiplicity.rs crates/uaml/src/lib.rs
git commit -m "feat(uaml): validated Multiplicity type"
```

---

### Task 5: `model` module — resolved data-model types

**Files:**
- Create: `crates/uaml/src/model.rs`
- Modify: `crates/uaml/src/lib.rs`

**Interfaces:**
- Consumes: `crate::multiplicity::Multiplicity`.
- Produces the resolved graph types used by every later plan:
  - `Visibility`, `TypeRef`, `Attribute`
  - `RelationshipKind` (with `as_str`, `parse`, `is_ended`), `RelEnd`, `AssocName`, `Edge`
  - `UmlMetaclass`, `ClassifierType` (with `parse`, `as_str`), `NoteAnchor`, `Node`
  - `Member`, `RenderHints`, `Diagram`
  - `Model` (with `node(&str) -> Option<&Node>`)

Mirrors `packages/okf/src/types.ts`. All model types derive `Debug, Clone, PartialEq` (not `Eq`, because positions are `f64`).

- [ ] **Step 1: Write the failing tests**

`crates/uaml/src/model.rs`:
```rust
use crate::multiplicity::Multiplicity;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relationship_kind_round_trips() {
        for k in [
            RelationshipKind::Associates,
            RelationshipKind::Aggregates,
            RelationshipKind::Composes,
            RelationshipKind::Specializes,
            RelationshipKind::Implements,
            RelationshipKind::Depends,
            RelationshipKind::Annotates,
        ] {
            assert_eq!(RelationshipKind::parse(k.as_str()), Some(k));
        }
        assert_eq!(RelationshipKind::parse("nope"), None);
    }

    #[test]
    fn only_association_family_takes_ends() {
        assert!(RelationshipKind::Associates.is_ended());
        assert!(RelationshipKind::Aggregates.is_ended());
        assert!(RelationshipKind::Composes.is_ended());
        assert!(!RelationshipKind::Specializes.is_ended());
        assert!(!RelationshipKind::Implements.is_ended());
        assert!(!RelationshipKind::Depends.is_ended());
        assert!(!RelationshipKind::Annotates.is_ended());
    }

    #[test]
    fn classifier_type_parses_known_and_unknown() {
        assert_eq!(
            ClassifierType::parse("uml.Class"),
            ClassifierType::Uml(UmlMetaclass::Class)
        );
        assert_eq!(ClassifierType::parse("Diagram"), ClassifierType::Diagram);
        assert_eq!(
            ClassifierType::parse("bpmn.Task"),
            ClassifierType::Unknown("bpmn.Task".to_string())
        );
        assert_eq!(
            ClassifierType::parse("LegacyToken"),
            ClassifierType::Unknown("LegacyToken".to_string())
        );
    }

    #[test]
    fn classifier_type_round_trips_to_string() {
        assert_eq!(ClassifierType::Uml(UmlMetaclass::Enum).as_str(), "uml.Enum");
        assert_eq!(ClassifierType::Diagram.as_str(), "Diagram");
        assert_eq!(
            ClassifierType::Unknown("x.Y".to_string()).as_str(),
            "x.Y"
        );
    }

    #[test]
    fn model_looks_up_nodes_by_key() {
        let node = Node {
            key: "order".to_string(),
            ty: ClassifierType::Uml(UmlMetaclass::Class),
            title: "Order".to_string(),
            stereotypes: vec![],
            abstract_: false,
            description: None,
            attributes: vec![],
            values: vec![],
            body: None,
            annotates: vec![],
        };
        let model = Model { nodes: vec![node], edges: vec![], diagrams: vec![] };
        assert_eq!(model.node("order").map(|n| n.title.as_str()), Some("Order"));
        assert!(model.node("missing").is_none());
    }

    #[test]
    fn attribute_defaults_multiplicity_to_one() {
        let a = Attribute {
            name: "id".to_string(),
            ty: TypeRef { name: "OrderId".to_string(), ref_: None },
            multiplicity: Multiplicity::default(),
            visibility: None,
            description: None,
        };
        assert_eq!(a.multiplicity.as_str(), "1");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p uaml model`
Expected: FAIL — model types not found.

- [ ] **Step 3: Implement the types**

Prepend to `crates/uaml/src/model.rs`:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Private,
    Protected,
    Package,
}

impl Visibility {
    pub fn from_marker(c: char) -> Option<Visibility> {
        match c {
            '+' => Some(Visibility::Public),
            '-' => Some(Visibility::Private),
            '#' => Some(Visibility::Protected),
            '~' => Some(Visibility::Package),
            _ => None,
        }
    }
    pub fn marker(self) -> char {
        match self {
            Visibility::Public => '+',
            Visibility::Private => '-',
            Visibility::Protected => '#',
            Visibility::Package => '~',
        }
    }
}

/// An attribute's type: a display token, optionally resolved to another classifier's slug.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeRef {
    pub name: String,
    pub ref_: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Attribute {
    pub name: String,
    pub ty: TypeRef,
    pub multiplicity: Multiplicity,
    pub visibility: Option<Visibility>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationshipKind {
    Associates,
    Aggregates,
    Composes,
    Specializes,
    Implements,
    Depends,
    Annotates,
}

impl RelationshipKind {
    pub fn as_str(self) -> &'static str {
        match self {
            RelationshipKind::Associates => "associates",
            RelationshipKind::Aggregates => "aggregates",
            RelationshipKind::Composes => "composes",
            RelationshipKind::Specializes => "specializes",
            RelationshipKind::Implements => "implements",
            RelationshipKind::Depends => "depends",
            RelationshipKind::Annotates => "annotates",
        }
    }
    pub fn parse(s: &str) -> Option<RelationshipKind> {
        match s {
            "associates" => Some(RelationshipKind::Associates),
            "aggregates" => Some(RelationshipKind::Aggregates),
            "composes" => Some(RelationshipKind::Composes),
            "specializes" => Some(RelationshipKind::Specializes),
            "implements" => Some(RelationshipKind::Implements),
            "depends" => Some(RelationshipKind::Depends),
            "annotates" => Some(RelationshipKind::Annotates),
            _ => None,
        }
    }
    /// associates/aggregates/composes require `: near to far` ends; the rest forbid them.
    pub fn is_ended(self) -> bool {
        matches!(
            self,
            RelationshipKind::Associates
                | RelationshipKind::Aggregates
                | RelationshipKind::Composes
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RelEnd {
    pub multiplicity: Option<Multiplicity>,
    pub role: Option<String>,
    pub navigable: Option<bool>,
}

/// A relationship's optional `as …` name: a plain label, or a link to a
/// `uml.Association` document (an association class), stored by its resolved slug.
#[derive(Debug, Clone, PartialEq)]
pub enum AssocName {
    Label(String),
    Assoc(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub kind: RelationshipKind,
    pub name: Option<AssocName>,
    pub from_end: RelEnd,
    pub to_end: RelEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UmlMetaclass {
    Class,
    Interface,
    Enum,
    DataType,
    Package,
    Note,
    Association,
}

impl UmlMetaclass {
    fn parse(metaclass: &str) -> Option<UmlMetaclass> {
        match metaclass {
            "Class" => Some(UmlMetaclass::Class),
            "Interface" => Some(UmlMetaclass::Interface),
            "Enum" => Some(UmlMetaclass::Enum),
            "DataType" => Some(UmlMetaclass::DataType),
            "Package" => Some(UmlMetaclass::Package),
            "Note" => Some(UmlMetaclass::Note),
            "Association" => Some(UmlMetaclass::Association),
            _ => None,
        }
    }
    fn name(self) -> &'static str {
        match self {
            UmlMetaclass::Class => "Class",
            UmlMetaclass::Interface => "Interface",
            UmlMetaclass::Enum => "Enum",
            UmlMetaclass::DataType => "DataType",
            UmlMetaclass::Package => "Package",
            UmlMetaclass::Note => "Note",
            UmlMetaclass::Association => "Association",
        }
    }
}

/// A classifier's `type`. Graceful degradation is a type-level guarantee: any
/// unrecognized token becomes `Unknown` and renders as a generic labelled box.
#[derive(Debug, Clone, PartialEq)]
pub enum ClassifierType {
    Uml(UmlMetaclass),
    Diagram,
    Unknown(String),
}

impl ClassifierType {
    pub fn parse(s: &str) -> ClassifierType {
        if s == "Diagram" {
            return ClassifierType::Diagram;
        }
        if let Some((family, metaclass)) = s.split_once('.') {
            if family == "uml" {
                if let Some(mc) = UmlMetaclass::parse(metaclass) {
                    return ClassifierType::Uml(mc);
                }
            }
        }
        ClassifierType::Unknown(s.to_string())
    }
    pub fn as_str(&self) -> String {
        match self {
            ClassifierType::Uml(mc) => format!("uml.{}", mc.name()),
            ClassifierType::Diagram => "Diagram".to_string(),
            ClassifierType::Unknown(s) => s.clone(),
        }
    }
}

/// A `uml.Note` anchor. Three forms, per the spec.
#[derive(Debug, Clone, PartialEq)]
pub enum NoteAnchor {
    Classifier { target_key: String },
    NamedAssoc { source_key: String, name: String },
    EndpointAssoc {
        source_key: String,
        kind: RelationshipKind,
        target_key: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub key: String,
    pub ty: ClassifierType,
    pub title: String,
    pub stereotypes: Vec<String>,
    pub abstract_: bool,
    pub description: Option<String>,
    pub attributes: Vec<Attribute>,
    pub values: Vec<String>,
    pub body: Option<String>,
    pub annotates: Vec<NoteAnchor>,
}

/// A diagram member: a classifier slug and an optional `x,y` position.
#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    pub key: String,
    pub position: Option<(f64, f64)>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RenderHints {
    pub emphasize: Vec<String>,
    pub collapse: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Diagram {
    pub key: String,
    pub title: String,
    pub profile: String,
    pub members: Vec<Member>,
    pub hints: RenderHints,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub diagrams: Vec<Diagram>,
}

impl Model {
    pub fn node(&self, key: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.key == key)
    }
}
```

- [ ] **Step 4: Wire the module in**

In `crates/uaml/src/lib.rs`, add:
```rust
pub mod model;
```

- [ ] **Step 5: Run to verify passing**

Run: `cargo test -p uaml model`
Expected: PASS — `6 passed`.

- [ ] **Step 6: Run the full crate test suite**

Run: `cargo test -p uaml`
Expected: PASS — all tests from Tasks 1-5 green.

- [ ] **Step 7: Commit**

```bash
git add crates/uaml/src/model.rs crates/uaml/src/lib.rs
git commit -m "feat(uaml): resolved data-model types"
```

---

## Self-Review

- **Spec coverage (this plan's slice):** workspace under `/crates` ✔ (Task 1); `Multiplicity` as a validated type ✔ (Task 4); `ClassifierType::Unknown` for graceful degradation ✔ (Task 5); mini-YAML frontmatter mirroring `slug.ts`, flat subset only, no `owox.*` ✔ (Task 3); slugify with camelCase/acronym boundaries ✔ (Task 2). Parser, serializer, validator, CLI, fixtures — intentionally deferred to Plans 2 and 3.
- **Placeholder scan:** none — every step has concrete code and exact commands.
- **Type consistency:** `Multiplicity::parse/as_str/default`, `RelationshipKind::{as_str,parse,is_ended}`, `ClassifierType::{parse,as_str}`, and `Frontmatter::{get,get_str,get_bool,get_string_list}` are used consistently and are the same names Plan 2 will consume. `Node`/`Edge`/`Diagram`/`Member` field names are fixed here and referenced verbatim by later plans.
