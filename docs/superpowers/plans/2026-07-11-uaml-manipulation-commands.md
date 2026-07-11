# UAML Manipulation Commands Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a composable command set to the `uaml` CLI (create / inspect / edit / rename nodes, attributes, enum values, relationships) driven by a pure, versioned op-engine that serves a human, an LLM agent, and (later) the web canvas from one mutation source of truth.

**Architecture:** A pure `ops` module in the core `uaml` crate defines a `Selector` (element addressing built on the note-anchor vocabulary), an `Op` enum (one variant per action), and `apply(bundle, &[Op]) -> Result<Bundle, OpError>` that folds ops over a whole-bundle working copy atomically (any op fails → nothing changes). The CLI adds noun-verb subcommands that each build one `Op` and call `apply`, an `OpDto` serde layer that maps NDJSON op-log lines ⇄ core `Op`, and read-only `show`/`refs` queries over the resolved `Model`.

**Tech Stack:** Rust (edition/rust-version from workspace), `regex` + `pulldown-cmark` (core only), `clap` + `serde` + `serde_json` (CLI only). Existing crates: `uaml` (core lib), `uaml-cli` (the `uaml` binary).

## Global Constraints

- Core crate `uaml` stays pure/WASM-friendly: dependencies are **only** `regex` + `pulldown-cmark`. No `std::fs`, threads, OS, `serde`, or `clap` in core. All I/O and all serde live in `uaml-cli`.
- The mutation API is text→text: `apply` takes `&[(String, String)]` (path, content) and returns a new `Vec<(String, String)>`. `apply` never touches the filesystem.
- Never lose data: no op drops an `Unknown` section, a note `## Body`, unknown frontmatter keys, or an unknown classifier type. Refuse rather than corrupt.
- Atomic: a command or batch is all-or-nothing. On any `OpError`, the caller writes zero files.
- Refuse corruption, allow forward references: refuse dup attr name, dup enum literal, ends-on-forbidden-verb / missing-on-required-verb, rename slug collision, editing a missing element. Allow a relationship/type-ref to a not-yet-created node.
- Canonical output: every touched file is re-emitted via `serialize_document` (default `[1]` omitted, links `[Title](./slug.md)`, ended rels `: <near> to <far>`). Round-trip is a semantic fixpoint, not byte-identity.
- Node key = filename slug (last path segment, `.md` stripped). `Document` (per-file fidelity) is the edit tier; `Model` (resolved graph) backs `show`/`refs` and the rename referrer sweep.
- Server-ready: `apply` and `OpDto` reach no CLI-only state, so a future `uaml serve` can call them unchanged. Keep `OpDto` in its own importable module.
- Every serialized op carries a `"v"` (version, currently `1`) and an `"op"` discriminator (e.g. `attr.add`). Versioning is per-op, not a log-wide header.
- Exit codes (match existing `check`/`fmt`): `0` success, `1` op refused / dry-run drift, `2` I/O error.
- Spec of record: `docs/superpowers/specs/2026-07-11-uaml-manipulation-commands-design.md`.

## File Structure

**Core (`crates/uaml/src/`):**
- `lib.rs` — add `pub mod ops;`.
- `ops/mod.rs` — `Bundle` alias, `Op`, `OpError`, `apply`, and the shared internal helpers (`slug_of`, `find_doc`, `edit_doc`, section get-or-create, `resolve_type`). Dispatches each op to a per-family applier.
- `ops/selector.rs` — `Selector`, `RelBy`, `parse_selector` (anchor link grammar, future-notes-compatible), `render_selector` (compact display form).
- `ops/rename.rs` — the cross-file `NodeRename` referrer sweep (isolated because it is the riskiest op; golden-tested).
- `grammar.rs` — add `pub fn parse_ends(&str) -> Option<(RelEnd, RelEnd)>` (extracted from `parse_relationship_line`) so rel ops parse an ends clause without building a whole line.

**CLI (`crates/uaml-cli/src/`):**
- `main.rs` — new `clap` subcommands (`node`, `attr`, `value`, `rel`, `show`, `refs`, `list`, `apply`) and their arms.
- `ops_dto.rs` — `OpDto` (serde) with `to_op(&self) -> Result<Op, String>` and `from_op(&Op) -> OpDto`; the only place serde meets ops. Rejects unknown `v`/`op`.
- `commands.rs` — pure helpers added: `render_diff(before, after) -> String` (unified-ish per-file diff for `--dry-run`), `apply_outcome` shaping. Existing `render_human`/`render_json`/`plan_fmt` untouched.
- `io.rs` — helpers added: `write_back(old, new)` (write only changed/added entries, no-op unchanged), `read_ndjson(path_or_stdin) -> Vec<(usize, String)>` (line number + raw line, blanks skipped).

## Shared Foundations (defined in Task 1–2, reused everywhere)

These signatures are produced by early tasks and consumed by later ones; every later task relies on them:

```rust
// ops/mod.rs
pub type Bundle = Vec<(String, String)>;

#[derive(Debug, Clone, PartialEq)]
pub struct OpError {
    pub index: usize,          // which op in the batch (filled by `apply`)
    pub op: String,            // discriminator, e.g. "attr.add"
    pub selector: Option<String>,
    pub reason: String,
}

pub fn apply(bundle: &[(String, String)], ops: &[Op]) -> Result<Bundle, OpError>;
```

---

### Task 1: `Selector` type + anchor grammar

The shared addressing type. Node/Rel variants mirror `NoteAnchor::{Classifier, NamedAssoc, EndpointAssoc}` so future `annotates` resolution reuses `parse_selector`. Attr/Value are extensions (notes never reach a sub-element). `parse_selector` reads the anchor **link** form (what appears in a note line); `render_selector` emits a compact display form for error messages (it does not round-trip to the link form — the Selector stores slugs/kinds/names only, no titles).

**Files:**
- Create: `crates/uaml/src/ops/selector.rs`
- Create: `crates/uaml/src/ops/mod.rs` (module stub this task; fleshed out in Task 2)
- Modify: `crates/uaml/src/lib.rs` (add `pub mod ops;` after `pub mod model;`)

**Interfaces:**
- Consumes: `crate::model::RelationshipKind` (has `parse(&str) -> Option<Self>`, `as_str(&self) -> &'static str`).
- Produces:
  ```rust
  pub enum Selector {
      Node(String),                            // slug
      Rel { source: String, by: RelBy },
      Attr { node: String, name: String },
      Value { node: String, literal: String },
  }
  pub enum RelBy {
      Named(String),                           // as "name"
      Endpoint { kind: RelationshipKind, target: String },
  }
  pub fn parse_selector(s: &str) -> Option<Selector>;
  pub fn render_selector(sel: &Selector) -> String;
  ```

- [ ] **Step 1: Add the module declarations**

In `crates/uaml/src/lib.rs`, add after `pub mod model;`:
```rust
pub mod ops;
```
In a new `crates/uaml/src/ops/mod.rs`:
```rust
pub mod selector;
pub use selector::{parse_selector, render_selector, RelBy, Selector};
```

- [ ] **Step 2: Write the failing tests**

Create `crates/uaml/src/ops/selector.rs` with only a test module first:
```rust
use crate::model::RelationshipKind;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_classifier_anchor_as_node() {
        assert_eq!(parse_selector("[Order](./order.md)"), Some(Selector::Node("order".into())));
    }

    #[test]
    fn parses_endpoint_anchor_as_rel() {
        let got = parse_selector("[Order](./order.md) composes [OrderLine](./order-line.md)");
        assert_eq!(got, Some(Selector::Rel {
            source: "order".into(),
            by: RelBy::Endpoint { kind: RelationshipKind::Composes, target: "order-line".into() },
        }));
    }

    #[test]
    fn parses_named_anchor_as_rel() {
        let got = parse_selector("[Order](./order.md) as \"places\"");
        assert_eq!(got, Some(Selector::Rel { source: "order".into(), by: RelBy::Named("places".into()) }));
    }

    #[test]
    fn parses_attr_and_value_extensions() {
        assert_eq!(parse_selector("[Order](./order.md) attr total"),
                   Some(Selector::Attr { node: "order".into(), name: "total".into() }));
        assert_eq!(parse_selector("[OrderStatus](./order-status.md) value PLACED"),
                   Some(Selector::Value { node: "order-status".into(), literal: "PLACED".into() }));
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(parse_selector("not a selector"), None);
        assert_eq!(parse_selector("[Order](./order.md) frobnicates [X](./x.md)"), None);
    }

    #[test]
    fn renders_compact_display_form() {
        assert_eq!(render_selector(&Selector::Node("order".into())), "order");
        assert_eq!(render_selector(&Selector::Rel {
            source: "order".into(),
            by: RelBy::Endpoint { kind: RelationshipKind::Composes, target: "order-line".into() },
        }), "order composes order-line");
        assert_eq!(render_selector(&Selector::Rel { source: "order".into(), by: RelBy::Named("places".into()) }),
                   "order as \"places\"");
        assert_eq!(render_selector(&Selector::Attr { node: "order".into(), name: "total".into() }), "order attr total");
        assert_eq!(render_selector(&Selector::Value { node: "order-status".into(), literal: "PLACED".into() }),
                   "order-status value PLACED");
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p uaml selector`
Expected: FAIL to compile — `Selector`, `RelBy`, `parse_selector`, `render_selector` not defined.

- [ ] **Step 4: Implement the type + grammar**

Prepend to `crates/uaml/src/ops/selector.rs` (above the test module):
```rust
use std::sync::LazyLock;
use regex::Regex;

/// Addressing for a model element. Node/Rel mirror `NoteAnchor`; Attr/Value extend it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selector {
    Node(String),
    Rel { source: String, by: RelBy },
    Attr { node: String, name: String },
    Value { node: String, literal: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelBy {
    Named(String),
    Endpoint { kind: RelationshipKind, target: String },
}

// A leading `[Title](./slug.md)` link, capturing the slug; then an optional tail.
static SEL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\[[^\]]+\]\(\./(?P<src>[^)]+?)\.md\)(?P<tail>.*)$").unwrap()
});
// Tail: ` <verb> [Title](./slug.md)`
static ENDPOINT_TAIL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^\s+(?P<verb>\w+)\s+\[[^\]]+\]\(\./(?P<tgt>[^)]+?)\.md\)$"#).unwrap()
});
// Tail: ` as "name"`
static NAMED_TAIL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s+as\s+"(?P<name>[^"]*)"$"#).unwrap());
// Tail: ` attr <name>`  /  ` value <literal>`
static ATTR_TAIL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s+attr\s+(?P<name>\S.*)$").unwrap());
static VALUE_TAIL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s+value\s+(?P<lit>\S.*)$").unwrap());

fn basename(slug: &str) -> String {
    slug.rsplit(['/', '\\']).next().unwrap_or(slug).to_string()
}

pub fn parse_selector(s: &str) -> Option<Selector> {
    let s = s.trim();
    let caps = SEL_RE.captures(s)?;
    let src = basename(&caps["src"]);
    let tail = &caps["tail"];
    if tail.is_empty() {
        return Some(Selector::Node(src));
    }
    if let Some(t) = ENDPOINT_TAIL.captures(tail) {
        let kind = RelationshipKind::parse(&t["verb"])?;
        return Some(Selector::Rel {
            source: src,
            by: RelBy::Endpoint { kind, target: basename(&t["tgt"]) },
        });
    }
    if let Some(t) = NAMED_TAIL.captures(tail) {
        return Some(Selector::Rel { source: src, by: RelBy::Named(t["name"].to_string()) });
    }
    if let Some(t) = ATTR_TAIL.captures(tail) {
        return Some(Selector::Attr { node: src, name: t["name"].trim().to_string() });
    }
    if let Some(t) = VALUE_TAIL.captures(tail) {
        return Some(Selector::Value { node: src, literal: t["lit"].trim().to_string() });
    }
    None
}

pub fn render_selector(sel: &Selector) -> String {
    match sel {
        Selector::Node(s) => s.clone(),
        Selector::Rel { source, by: RelBy::Endpoint { kind, target } } => {
            format!("{source} {} {target}", kind.as_str())
        }
        Selector::Rel { source, by: RelBy::Named(name) } => format!("{source} as \"{name}\""),
        Selector::Attr { node, name } => format!("{node} attr {name}"),
        Selector::Value { node, literal } => format!("{node} value {literal}"),
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p uaml selector`
Expected: PASS (6 tests).

- [ ] **Step 6: Clippy + commit**

Run: `cargo clippy -p uaml --all-targets`
Expected: clean.
```bash
git add crates/uaml/src/lib.rs crates/uaml/src/ops/mod.rs crates/uaml/src/ops/selector.rs
git commit -m "feat(uaml): Selector type + anchor grammar (shared with future notes)"
```

---

### Task 2: Op-engine scaffolding + `AttrAdd` (the vertical slice)

Builds `Bundle`, `OpError`, `apply`, and the shared internal helpers (`slug_of`, `find_doc`, `edit_doc`, `attrs_mut`, `resolve_type`), then implements the first op end-to-end to prove atomicity and canonicalization. The `Op` enum starts with only `AttrAdd`; later tasks add variants and match arms.

**Files:**
- Modify: `crates/uaml/src/ops/mod.rs`
- Test: same file (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `parse_document`, `serialize_document`, `Document`, `Section`, `Attribute`, `TypeRef`, `Visibility`, `Multiplicity`.
- Produces (relied on by every later core task):
  ```rust
  pub type Bundle = Vec<(String, String)>;
  pub struct OpError { pub index: usize, pub op: String, pub selector: Option<String>, pub reason: String }
  pub enum Op { AttrAdd { node: String, name: String, ty_token: String, multiplicity: Multiplicity, visibility: Option<Visibility> } /* extended later */ }
  pub fn apply(bundle: &[(String, String)], ops: &[Op]) -> Result<Bundle, OpError>;
  // pub(crate): slug_of, find_doc, edit_doc, attrs_mut, resolve_type
  ```

- [ ] **Step 1: Write the failing tests**

Append to `crates/uaml/src/ops/mod.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::multiplicity::Multiplicity;

    fn attr_add(node: &str, name: &str, ty: &str) -> Op {
        Op::AttrAdd { node: node.into(), name: name.into(), ty_token: ty.into(),
                      multiplicity: Multiplicity::default(), visibility: None }
    }

    #[test]
    fn attr_add_appends_a_bare_attribute() {
        let b = vec![("shop/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n".to_string())];
        let out = apply(&b, &[attr_add("order", "total", "Money")]).unwrap();
        assert!(out[0].1.contains("- total: Money"));
        assert!(out[0].1.contains("- id: OrderId"), "existing attr kept");
    }

    #[test]
    fn attr_add_links_a_known_slug() {
        let b = vec![
            ("a/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
            ("a/money.md".to_string(), "---\ntype: uml.DataType\ntitle: Money\n---\n# Money\n".to_string()),
        ];
        let out = apply(&b, &[attr_add("order", "total", "money")]).unwrap();
        assert!(out[0].1.contains("- total: [Money](./money.md)"), "known slug links with target title");
    }

    #[test]
    fn attr_add_refuses_a_duplicate_name() {
        let b = vec![("a/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n".to_string())];
        let err = apply(&b, &[attr_add("order", "id", "X")]).unwrap_err();
        assert_eq!(err.index, 0);
        assert_eq!(err.op, "attr.add");
        assert!(err.reason.contains("already exists"));
    }

    #[test]
    fn attr_add_on_missing_node_errors() {
        let b: Bundle = vec![];
        let err = apply(&b, &[attr_add("ghost", "x", "Y")]).unwrap_err();
        assert!(err.reason.contains("no document 'ghost'"));
    }

    #[test]
    fn apply_is_atomic_on_a_later_failure() {
        let b = vec![("a/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n".to_string())];
        let ops = vec![attr_add("order", "total", "Money"), attr_add("order", "id", "X")]; // 2nd is a dup
        let err = apply(&b, &ops).unwrap_err();
        assert_eq!(err.index, 1, "failing op index reported");
        assert!(!b[0].1.contains("total"), "input bundle untouched; caller writes nothing");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p uaml ops::tests`
Expected: FAIL to compile — `Op`, `apply`, `Bundle` not defined.

- [ ] **Step 3: Implement the scaffolding + `AttrAdd`**

Prepend to `crates/uaml/src/ops/mod.rs` (above `pub mod selector;`):
```rust
use crate::model::{Attribute, TypeRef, Visibility};
use crate::multiplicity::Multiplicity;
use crate::parse::parse_document;
use crate::serialize::serialize_document;
use crate::syntax::{Document, Section};

pub type Bundle = Vec<(String, String)>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpError {
    pub index: usize,
    pub op: String,
    pub selector: Option<String>,
    pub reason: String,
}

impl OpError {
    pub(crate) fn at(op: &str, reason: impl Into<String>) -> OpError {
        OpError { index: 0, op: op.to_string(), selector: None, reason: reason.into() }
    }
}

/// One mutation. One variant per sugar command; grows task by task.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    AttrAdd {
        node: String,
        name: String,
        ty_token: String,
        multiplicity: Multiplicity,
        visibility: Option<Visibility>,
    },
}

pub fn apply(bundle: &[(String, String)], ops: &[Op]) -> Result<Bundle, OpError> {
    let mut work: Bundle = bundle.to_vec();
    for (i, op) in ops.iter().enumerate() {
        apply_one(&mut work, op).map_err(|mut e| {
            e.index = i;
            e
        })?;
    }
    Ok(work)
}

fn apply_one(work: &mut Bundle, op: &Op) -> Result<(), OpError> {
    match op {
        Op::AttrAdd { node, name, ty_token, multiplicity, visibility } => {
            op_attr_add(work, node, name, ty_token, multiplicity, *visibility)
        }
    }
}

// ---- shared helpers (reused by every later op) ----

pub(crate) fn slug_of(path: &str) -> String {
    let seg = path.rsplit(['/', '\\']).next().unwrap_or(path);
    seg.strip_suffix(".md").unwrap_or(seg).to_string()
}

pub(crate) fn find_doc(work: &Bundle, slug: &str, op: &str) -> Result<usize, OpError> {
    work.iter()
        .position(|(p, _)| slug_of(p) == slug)
        .ok_or_else(|| OpError::at(op, format!("no document '{slug}'")))
}

/// Parse the target file, mutate via `f`, re-serialize canonically.
pub(crate) fn edit_doc<F>(work: &mut Bundle, slug: &str, op: &str, f: F) -> Result<(), OpError>
where
    F: FnOnce(&mut Document) -> Result<(), OpError>,
{
    let i = find_doc(work, slug, op)?;
    let mut doc = parse_document(&work[i].1);
    f(&mut doc)?;
    work[i].1 = serialize_document(&doc);
    Ok(())
}

/// Get the `## Attributes` list, creating an empty section if absent
/// (canonical serialize re-orders sections, so append position is irrelevant).
pub(crate) fn attrs_mut(doc: &mut Document) -> &mut Vec<Attribute> {
    if !doc.sections.iter().any(|s| matches!(s, Section::Attributes(_))) {
        doc.sections.push(Section::Attributes(Vec::new()));
    }
    doc.sections
        .iter_mut()
        .find_map(|s| match s {
            Section::Attributes(a) => Some(a),
            _ => None,
        })
        .expect("attributes section just ensured")
}

/// Forward-ref-safe: a token matching an existing doc slug links to it (using
/// that doc's title); otherwise it is a bare type token. Mirrors build_model.
pub(crate) fn resolve_type(work: &Bundle, token: &str) -> TypeRef {
    if let Some((_, text)) = work.iter().find(|(p, _)| slug_of(p) == token) {
        let title = parse_document(text)
            .frontmatter
            .get_str("title")
            .map(String::from)
            .unwrap_or_else(|| token.to_string());
        TypeRef { name: title, ref_: Some(token.to_string()) }
    } else {
        TypeRef { name: token.to_string(), ref_: None }
    }
}

fn op_attr_add(
    work: &mut Bundle,
    node: &str,
    name: &str,
    ty_token: &str,
    multiplicity: &Multiplicity,
    visibility: Option<Visibility>,
) -> Result<(), OpError> {
    let ty = resolve_type(work, ty_token);
    edit_doc(work, node, "attr.add", |doc| {
        let attrs = attrs_mut(doc);
        if attrs.iter().any(|a| a.name == name) {
            return Err(OpError::at("attr.add", format!("attribute '{name}' already exists in {node}")));
        }
        attrs.push(Attribute {
            name: name.to_string(),
            ty,
            multiplicity: multiplicity.clone(),
            visibility,
            description: None,
        });
        Ok(())
    })
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p uaml ops::tests`
Expected: PASS (5 tests).

- [ ] **Step 5: Clippy + commit**

Run: `cargo clippy -p uaml --all-targets`
Expected: clean.
```bash
git add crates/uaml/src/ops/mod.rs
git commit -m "feat(uaml): op-engine core (apply, atomic, helpers) + attr.add"
```

---

### Task 3: `AttrSet` + `AttrRm`

Edit and remove attributes, reusing `edit_doc` / `attrs_mut`. `AttrSet` sets only the fields present (`Some`); a `rename` that collides is refused; editing/removing a missing attr is refused.

**Files:**
- Modify: `crates/uaml/src/ops/mod.rs` (add 2 `Op` variants, 2 arms, 2 appliers, tests)

**Interfaces:**
- Consumes: `edit_doc`, `attrs_mut`, `resolve_type`, `OpError` (from Task 2).
- Produces:
  ```rust
  Op::AttrSet { node: String, name: String, ty_token: Option<String>,
                multiplicity: Option<Multiplicity>, visibility: Option<Visibility>, rename: Option<String> }
  Op::AttrRm { node: String, name: String }
  ```

- [ ] **Step 1: Write the failing tests**

Add inside the existing `mod tests` in `crates/uaml/src/ops/mod.rs`:
```rust
    #[test]
    fn attr_set_changes_type_and_multiplicity() {
        let b = vec![("a/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n".to_string())];
        let out = apply(&b, &[Op::AttrSet {
            node: "order".into(), name: "id".into(),
            ty_token: Some("String".into()),
            multiplicity: Some(Multiplicity::parse("0..1").unwrap()),
            visibility: Some(crate::model::Visibility::Private),
            rename: None,
        }]).unwrap();
        assert!(out[0].1.contains("- id: String [0..1]"));
    }

    #[test]
    fn attr_set_renames_and_refuses_collision() {
        let b = vec![("a/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n- total: Money\n".to_string())];
        let ok = apply(&b, &[Op::AttrSet { node:"order".into(), name:"id".into(), ty_token:None, multiplicity:None, visibility:None, rename: Some("orderId".into()) }]).unwrap();
        assert!(ok[0].1.contains("- orderId: OrderId"));
        let err = apply(&b, &[Op::AttrSet { node:"order".into(), name:"id".into(), ty_token:None, multiplicity:None, visibility:None, rename: Some("total".into()) }]).unwrap_err();
        assert!(err.reason.contains("already exists"));
    }

    #[test]
    fn attr_set_on_missing_attr_errors() {
        let b = vec![("a/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string())];
        let err = apply(&b, &[Op::AttrSet { node:"order".into(), name:"ghost".into(), ty_token:Some("X".into()), multiplicity:None, visibility:None, rename:None }]).unwrap_err();
        assert!(err.reason.contains("no attribute 'ghost'"));
    }

    #[test]
    fn attr_rm_removes_and_refuses_missing() {
        let b = vec![("a/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n- total: Money\n".to_string())];
        let out = apply(&b, &[Op::AttrRm { node:"order".into(), name:"total".into() }]).unwrap();
        assert!(!out[0].1.contains("total"));
        assert!(out[0].1.contains("- id: OrderId"));
        let err = apply(&b, &[Op::AttrRm { node:"order".into(), name:"ghost".into() }]).unwrap_err();
        assert!(err.reason.contains("no attribute 'ghost'"));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p uaml ops::tests`
Expected: FAIL to compile — no `Op::AttrSet` / `Op::AttrRm`.

- [ ] **Step 3: Extend `Op` + `apply_one` + add appliers**

Add these variants to `enum Op` (after `AttrAdd`):
```rust
    AttrSet {
        node: String,
        name: String,
        ty_token: Option<String>,
        multiplicity: Option<Multiplicity>,
        visibility: Option<Visibility>,
        rename: Option<String>,
    },
    AttrRm { node: String, name: String },
```
Add arms to `apply_one`'s match:
```rust
        Op::AttrSet { node, name, ty_token, multiplicity, visibility, rename } => {
            op_attr_set(work, node, name, ty_token, multiplicity, *visibility, rename)
        }
        Op::AttrRm { node, name } => op_attr_rm(work, node, name),
```
Add the appliers (after `op_attr_add`):
```rust
#[allow(clippy::too_many_arguments)]
fn op_attr_set(
    work: &mut Bundle,
    node: &str,
    name: &str,
    ty_token: &Option<String>,
    multiplicity: &Option<Multiplicity>,
    visibility: Option<Visibility>,
    rename: &Option<String>,
) -> Result<(), OpError> {
    let ty = ty_token.as_ref().map(|t| resolve_type(work, t));
    edit_doc(work, node, "attr.set", |doc| {
        let attrs = attrs_mut(doc);
        if let Some(new) = rename {
            if new != name && attrs.iter().any(|a| a.name == *new) {
                return Err(OpError::at("attr.set", format!("attribute '{new}' already exists in {node}")));
            }
        }
        let a = attrs
            .iter_mut()
            .find(|a| a.name == name)
            .ok_or_else(|| OpError::at("attr.set", format!("no attribute '{name}' in {node}")))?;
        if let Some(t) = ty {
            a.ty = t;
        }
        if let Some(m) = multiplicity {
            a.multiplicity = m.clone();
        }
        if let Some(v) = visibility {
            a.visibility = Some(v);
        }
        if let Some(new) = rename {
            a.name = new.clone();
        }
        Ok(())
    })
}

fn op_attr_rm(work: &mut Bundle, node: &str, name: &str) -> Result<(), OpError> {
    edit_doc(work, node, "attr.rm", |doc| {
        let attrs = attrs_mut(doc);
        let before = attrs.len();
        attrs.retain(|a| a.name != name);
        if attrs.len() == before {
            return Err(OpError::at("attr.rm", format!("no attribute '{name}' in {node}")));
        }
        Ok(())
    })
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p uaml ops::tests`
Expected: PASS.

- [ ] **Step 5: Clippy + commit**

Run: `cargo clippy -p uaml --all-targets`
Expected: clean.
```bash
git add crates/uaml/src/ops/mod.rs
git commit -m "feat(uaml): attr.set + attr.rm"
```

---

### Task 4: `ValueAdd` + `ValueRm`

Enum literal editing. Structural refusal on a duplicate literal (add) or a missing literal (rm). No type-gating (a literal on a non-enum is a semantic issue `check` reports, not a corruption).

**Files:**
- Modify: `crates/uaml/src/ops/mod.rs` (add `values_mut` helper, 2 variants, 2 arms, 2 appliers, tests)

**Interfaces:**
- Consumes: `edit_doc`, `Section::Values`.
- Produces:
  ```rust
  Op::ValueAdd { node: String, literal: String }
  Op::ValueRm { node: String, literal: String }
  pub(crate) fn values_mut(doc: &mut Document) -> &mut Vec<String>
  ```

- [ ] **Step 1: Write the failing tests**

Add inside `mod tests`:
```rust
    #[test]
    fn value_add_appends_and_refuses_duplicate() {
        let b = vec![("a/order-status.md".to_string(),
            "---\ntype: uml.Enum\ntitle: OrderStatus\n---\n# OrderStatus\n\n## Values\n- DRAFT\n".to_string())];
        let out = apply(&b, &[Op::ValueAdd { node:"order-status".into(), literal:"PLACED".into() }]).unwrap();
        assert!(out[0].1.contains("- DRAFT"));
        assert!(out[0].1.contains("- PLACED"));
        let err = apply(&b, &[Op::ValueAdd { node:"order-status".into(), literal:"DRAFT".into() }]).unwrap_err();
        assert!(err.reason.contains("already"));
    }

    #[test]
    fn value_rm_removes_and_refuses_missing() {
        let b = vec![("a/order-status.md".to_string(),
            "---\ntype: uml.Enum\ntitle: OrderStatus\n---\n# OrderStatus\n\n## Values\n- DRAFT\n- PLACED\n".to_string())];
        let out = apply(&b, &[Op::ValueRm { node:"order-status".into(), literal:"DRAFT".into() }]).unwrap();
        assert!(!out[0].1.contains("DRAFT"));
        assert!(out[0].1.contains("- PLACED"));
        let err = apply(&b, &[Op::ValueRm { node:"order-status".into(), literal:"GONE".into() }]).unwrap_err();
        assert!(err.reason.contains("no value 'GONE'"));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p uaml ops::tests`
Expected: FAIL to compile.

- [ ] **Step 3: Add helper + variants + arms + appliers**

Add the helper (near `attrs_mut`):
```rust
pub(crate) fn values_mut(doc: &mut Document) -> &mut Vec<String> {
    if !doc.sections.iter().any(|s| matches!(s, Section::Values(_))) {
        doc.sections.push(Section::Values(Vec::new()));
    }
    doc.sections
        .iter_mut()
        .find_map(|s| match s {
            Section::Values(v) => Some(v),
            _ => None,
        })
        .expect("values section just ensured")
}
```
Add to `enum Op`:
```rust
    ValueAdd { node: String, literal: String },
    ValueRm { node: String, literal: String },
```
Add to `apply_one`:
```rust
        Op::ValueAdd { node, literal } => op_value_add(work, node, literal),
        Op::ValueRm { node, literal } => op_value_rm(work, node, literal),
```
Add appliers:
```rust
fn op_value_add(work: &mut Bundle, node: &str, literal: &str) -> Result<(), OpError> {
    edit_doc(work, node, "value.add", |doc| {
        let values = values_mut(doc);
        if values.iter().any(|v| v == literal) {
            return Err(OpError::at("value.add", format!("value '{literal}' already in {node}")));
        }
        values.push(literal.to_string());
        Ok(())
    })
}

fn op_value_rm(work: &mut Bundle, node: &str, literal: &str) -> Result<(), OpError> {
    edit_doc(work, node, "value.rm", |doc| {
        let values = values_mut(doc);
        let before = values.len();
        values.retain(|v| v != literal);
        if values.len() == before {
            return Err(OpError::at("value.rm", format!("no value '{literal}' in {node}")));
        }
        Ok(())
    })
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p uaml ops::tests`
Expected: PASS.

- [ ] **Step 5: Clippy + commit**

Run: `cargo clippy -p uaml --all-targets`
Expected: clean.
```bash
git add crates/uaml/src/ops/mod.rs
git commit -m "feat(uaml): value.add + value.rm"
```

---

### Task 5: Relationship ops (`RelAdd` / `RelSet` / `RelRm`) + `grammar::parse_ends`

Relationship editing. `RelAdd` builds a `ParsedRel` and pushes it into `## Relationships`, enforcing the ends-XOR-verb rule and refusing an exact duplicate. `RelSet` / `RelRm` locate the rel via the shared `Selector` (`Rel` variant). First extract `parse_ends` in `grammar.rs` so ops can parse an ends clause without assembling a whole line.

**Files:**
- Modify: `crates/uaml/src/grammar.rs` (add `pub fn parse_ends`, rewire `parse_relationship_line` to use it)
- Modify: `crates/uaml/src/ops/mod.rs` (add `NameSpec`, `rels_mut`, `resolve_title`, `rel_matches`, `rel_target`, `OpError::with_sel`, 3 variants, 3 arms, 3 appliers, tests)

**Interfaces:**
- Consumes: `ParsedRel`, `ParsedName` (`syntax`), `RelEnd`, `RelationshipKind` (`model`, `is_ended(&self) -> bool`), `parse_end` (existing private in grammar), `Selector`/`RelBy`/`render_selector` (Task 1).
- Produces:
  ```rust
  pub fn crate::grammar::parse_ends(raw: &str) -> Option<(RelEnd, RelEnd)>
  pub enum NameSpec { Label(String), Ref(String) }   // Ref carries a target slug
  Op::RelAdd { source: String, kind: RelationshipKind, target: String, name: Option<NameSpec>, ends: Option<(RelEnd, RelEnd)> }
  Op::RelSet { selector: Selector, ends: Option<(RelEnd, RelEnd)>, name: Option<NameSpec> }
  Op::RelRm { selector: Selector }
  ```

- [ ] **Step 1: Extract `parse_ends` in grammar.rs (refactor, existing tests guard it)**

Add to `crates/uaml/src/grammar.rs` (near `parse_relationship_line`):
```rust
/// Parse a `<near> to <far>` ends clause into two `RelEnd`s. `None` if it is
/// not exactly two ` to `-separated, individually-valid ends.
pub fn parse_ends(raw: &str) -> Option<(RelEnd, RelEnd)> {
    let parts: Vec<&str> = raw.split(" to ").collect();
    if parts.len() != 2 {
        return None;
    }
    Some((parse_end(parts[0])?, parse_end(parts[1])?))
}
```
Then in `parse_relationship_line`, replace the inline ends block that computes `(from_end, to_end)` from `ends_raw` with:
```rust
    let (from_end, to_end) = match ends_raw {
        Some(raw) => parse_ends(raw)?,
        None => (RelEnd::default(), RelEnd::default()),
    };
```
Run `cargo test -p uaml grammar` — the existing `renders_relationship_round_trip` and rel-parse tests must still PASS (the refactor is behavior-preserving). If your "before" text differs slightly from the tree, adapt the match — the tests are the gate.

- [ ] **Step 2: Write the failing ops tests**

Add inside `mod tests` in `ops/mod.rs`:
```rust
    use crate::ops::selector::{RelBy, Selector};
    use crate::model::RelationshipKind;
    use crate::grammar::parse_ends;

    #[test]
    fn rel_add_composes_with_ends() {
        let b = vec![
            ("a/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
            ("a/order-line.md".to_string(), "---\ntype: uml.Class\ntitle: OrderLine\n---\n# OrderLine\n".to_string()),
        ];
        let out = apply(&b, &[Op::RelAdd {
            source: "order".into(), kind: RelationshipKind::Composes, target: "order-line".into(),
            name: None, ends: parse_ends("1 to 1..* lines"),
        }]).unwrap();
        assert!(out[0].1.contains("- composes [OrderLine](./order-line.md): 1 to 1..* lines"));
    }

    #[test]
    fn rel_add_enforces_ends_xor_verb() {
        let b = vec![("a/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string())];
        // composes requires ends
        let e1 = apply(&b, &[Op::RelAdd { source:"order".into(), kind:RelationshipKind::Composes, target:"x".into(), name:None, ends:None }]).unwrap_err();
        assert!(e1.reason.contains("requires ends"));
        // depends forbids ends
        let e2 = apply(&b, &[Op::RelAdd { source:"order".into(), kind:RelationshipKind::Depends, target:"x".into(), name:None, ends: parse_ends("1 to 1") }]).unwrap_err();
        assert!(e2.reason.contains("does not take ends"));
    }

    #[test]
    fn rel_add_allows_forward_ref_and_refuses_duplicate() {
        let b = vec![("a/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string())];
        // forward ref (ghost.md absent) is allowed; title falls back to the slug
        let out = apply(&b, &[Op::RelAdd { source:"order".into(), kind:RelationshipKind::Depends, target:"ghost".into(), name:None, ends:None }]).unwrap();
        assert!(out[0].1.contains("- depends [ghost](./ghost.md)"));
        let dup = apply(&out, &[Op::RelAdd { source:"order".into(), kind:RelationshipKind::Depends, target:"ghost".into(), name:None, ends:None }]).unwrap_err();
        assert!(dup.reason.contains("already exists"));
    }

    #[test]
    fn rel_set_updates_ends_and_rel_rm_removes() {
        let b = vec![
            ("a/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- composes [OrderLine](./order-line.md): 1 to 1..* lines\n".to_string()),
            ("a/order-line.md".to_string(), "---\ntype: uml.Class\ntitle: OrderLine\n---\n# OrderLine\n".to_string()),
        ];
        let sel = Selector::Rel { source:"order".into(), by: RelBy::Endpoint { kind: RelationshipKind::Composes, target:"order-line".into() } };
        let set = apply(&b, &[Op::RelSet { selector: sel.clone(), ends: parse_ends("1 to *"), name: None }]).unwrap();
        assert!(set[0].1.contains(": 1 to *"));
        let rm = apply(&b, &[Op::RelRm { selector: sel }]).unwrap();
        assert!(!rm[0].1.contains("composes"));
    }

    #[test]
    fn rel_set_on_missing_rel_errors() {
        let b = vec![("a/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string())];
        let sel = Selector::Rel { source:"order".into(), by: RelBy::Named("nope".into()) };
        let err = apply(&b, &[Op::RelRm { selector: sel }]).unwrap_err();
        assert!(err.reason.contains("no relationship"));
        assert!(err.selector.is_some());
    }
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p uaml ops::tests`
Expected: FAIL to compile — missing `Op::RelAdd`/`RelSet`/`RelRm`, `NameSpec`.

- [ ] **Step 4: Add `with_sel`, helpers, variants, arms, appliers**

Extend `impl OpError`:
```rust
    pub(crate) fn with_sel(mut self, sel: String) -> OpError {
        self.selector = Some(sel);
        self
    }
```
Add the name spec + helpers to `ops/mod.rs`:
```rust
use crate::syntax::{ParsedName, ParsedRel};
use crate::model::{RelEnd, RelationshipKind};
use crate::ops::selector::{render_selector, RelBy, Selector};

/// How a relationship name is given to an op (title of a `Ref` is resolved at apply time).
#[derive(Debug, Clone, PartialEq)]
pub enum NameSpec {
    Label(String),
    Ref(String), // target slug
}

pub(crate) fn rels_mut(doc: &mut Document) -> &mut Vec<ParsedRel> {
    if !doc.sections.iter().any(|s| matches!(s, Section::Relationships(_))) {
        doc.sections.push(Section::Relationships(Vec::new()));
    }
    doc.sections
        .iter_mut()
        .find_map(|s| match s {
            Section::Relationships(r) => Some(r),
            _ => None,
        })
        .expect("relationships section just ensured")
}

pub(crate) fn resolve_title(work: &Bundle, slug: &str) -> String {
    work.iter()
        .find(|(p, _)| slug_of(p) == slug)
        .and_then(|(_, t)| parse_document(t).frontmatter.get_str("title").map(String::from))
        .unwrap_or_else(|| slug.to_string())
}

fn build_name(work: &Bundle, spec: &Option<NameSpec>) -> Option<ParsedName> {
    match spec {
        None => None,
        Some(NameSpec::Label(l)) => Some(ParsedName::Label(l.clone())),
        Some(NameSpec::Ref(slug)) => {
            Some(ParsedName::Ref { title: resolve_title(work, slug), slug: slug.clone() })
        }
    }
}

fn rel_matches(r: &ParsedRel, by: &RelBy) -> bool {
    match by {
        RelBy::Endpoint { kind, target } => r.kind == *kind && r.target_slug == *target,
        RelBy::Named(name) => match &r.name {
            Some(ParsedName::Label(l)) => l == name,
            Some(ParsedName::Ref { title, .. }) => title == name,
            None => false,
        },
    }
}

fn rel_target<'a>(sel: &'a Selector, op: &str) -> Result<(&'a str, &'a RelBy), OpError> {
    match sel {
        Selector::Rel { source, by } => Ok((source.as_str(), by)),
        _ => Err(OpError::at(op, "selector does not address a relationship")),
    }
}
```
Add to `enum Op`:
```rust
    RelAdd {
        source: String,
        kind: RelationshipKind,
        target: String,
        name: Option<NameSpec>,
        ends: Option<(RelEnd, RelEnd)>,
    },
    RelSet { selector: Selector, ends: Option<(RelEnd, RelEnd)>, name: Option<NameSpec> },
    RelRm { selector: Selector },
```
Add to `apply_one`:
```rust
        Op::RelAdd { source, kind, target, name, ends } => {
            op_rel_add(work, source, *kind, target, name, ends)
        }
        Op::RelSet { selector, ends, name } => op_rel_set(work, selector, ends, name),
        Op::RelRm { selector } => op_rel_rm(work, selector),
```
Add the appliers:
```rust
fn op_rel_add(
    work: &mut Bundle,
    source: &str,
    kind: RelationshipKind,
    target: &str,
    name: &Option<NameSpec>,
    ends: &Option<(RelEnd, RelEnd)>,
) -> Result<(), OpError> {
    if kind.is_ended() != ends.is_some() {
        let msg = if kind.is_ended() {
            format!("'{}' requires ends", kind.as_str())
        } else {
            format!("'{}' does not take ends", kind.as_str())
        };
        return Err(OpError::at("rel.add", msg));
    }
    let target_title = resolve_title(work, target);
    let name = build_name(work, name);
    let ends = ends.clone();
    edit_doc(work, source, "rel.add", |doc| {
        let rels = rels_mut(doc);
        if rels.iter().any(|r| r.kind == kind && r.target_slug == target) {
            return Err(OpError::at(
                "rel.add",
                format!("relationship '{} {target}' already exists in {source}", kind.as_str()),
            ));
        }
        let (from_end, to_end) = ends.unwrap_or_default();
        rels.push(ParsedRel {
            kind,
            target_title,
            target_slug: target.to_string(),
            name,
            from_end,
            to_end,
        });
        Ok(())
    })
}

fn op_rel_set(
    work: &mut Bundle,
    selector: &Selector,
    ends: &Option<(RelEnd, RelEnd)>,
    name: &Option<NameSpec>,
) -> Result<(), OpError> {
    let (source, by) = rel_target(selector, "rel.set")?;
    let (source, by) = (source.to_string(), by.clone());
    let new_name = name.as_ref().map(|spec| build_name(work, &Some(spec.clone())));
    let ends = ends.clone();
    let disp = render_selector(selector);
    edit_doc(work, &source, "rel.set", |doc| {
        let rels = rels_mut(doc);
        let r = rels
            .iter_mut()
            .find(|r| rel_matches(r, &by))
            .ok_or_else(|| OpError::at("rel.set", format!("no relationship '{disp}'")).with_sel(disp.clone()))?;
        if let Some((f, t)) = ends {
            if !r.kind.is_ended() {
                return Err(OpError::at("rel.set", format!("'{}' does not take ends", r.kind.as_str())));
            }
            r.from_end = f;
            r.to_end = t;
        }
        if let Some(n) = new_name {
            r.name = n;
        }
        Ok(())
    })
}

fn op_rel_rm(work: &mut Bundle, selector: &Selector) -> Result<(), OpError> {
    let (source, by) = rel_target(selector, "rel.rm")?;
    let (source, by) = (source.to_string(), by.clone());
    let disp = render_selector(selector);
    edit_doc(work, &source, "rel.rm", |doc| {
        let rels = rels_mut(doc);
        let before = rels.len();
        rels.retain(|r| !rel_matches(r, &by));
        if rels.len() == before {
            return Err(OpError::at("rel.rm", format!("no relationship '{disp}'")).with_sel(disp.clone()));
        }
        Ok(())
    })
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p uaml`
Expected: PASS (ops + grammar + everything).

- [ ] **Step 6: Clippy + commit**

Run: `cargo clippy -p uaml --all-targets`
Expected: clean.
```bash
git add crates/uaml/src/grammar.rs crates/uaml/src/ops/mod.rs
git commit -m "feat(uaml): rel.add/set/rm via Selector + grammar::parse_ends"
```

---

### Task 6: Node ops (`NodeNew` / `NodeSet` / `NodeRm`) + `referrers` scan

Whole-document ops. `NodeNew` adds a `<slug>.md` entry with canonical frontmatter (refusing a slug collision). `NodeSet` upserts frontmatter keys in place (it never moves the file — slug changes go through `node rename` in Task 7). `NodeRm` refuses to delete a referenced node unless `--cascade` (cascade = proceed and let the dangling refs become `check` warnings; it does not strip referrers). The `referrers(work, slug)` helper is shared with `refs` (Task 10) and rename (Task 7).

**Files:**
- Modify: `crates/uaml/src/ops/mod.rs` (imports, `fm_set`, `referrers`, 3 variants, 3 arms, 3 appliers, tests)

**Interfaces:**
- Consumes: `Frontmatter`, `FmValue` (`crate::frontmatter`), `ClassifierType` (`as_str(&self) -> String`), `HintLine`, `ParsedName`.
- Produces:
  ```rust
  Op::NodeNew { slug: String, ty: ClassifierType, title: String, stereotype: Vec<String>, description: Option<String>, abstract_: bool }
  Op::NodeSet { slug: String, title: Option<String>, description: Option<String>, stereotype: Option<Vec<String>>, abstract_: Option<bool>, ty: Option<ClassifierType> }
  Op::NodeRm { slug: String, cascade: bool }
  pub(crate) fn referrers(work: &Bundle, slug: &str) -> Vec<String>   // referring doc slugs, sorted+deduped
  ```

- [ ] **Step 1: Write the failing tests**

Add inside `mod tests`:
```rust
    use crate::model::ClassifierType;

    #[test]
    fn node_new_writes_frontmatter_and_title_and_refuses_dup() {
        let b: Bundle = vec![];
        let out = apply(&b, &[Op::NodeNew {
            slug: "order".into(), ty: ClassifierType::parse("uml.Class"), title: "Order".into(),
            stereotype: vec!["entity".into()], description: Some("An order.".into()), abstract_: false,
        }]).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, "order.md");
        assert!(out[0].1.contains("type: \"uml.Class\""));
        assert!(out[0].1.contains("title: \"Order\""));
        assert!(out[0].1.contains("# Order"));
        let dup = apply(&out, &[Op::NodeNew { slug:"order".into(), ty: ClassifierType::parse("uml.Class"), title:"X".into(), stereotype: vec![], description: None, abstract_: false }]).unwrap_err();
        assert!(dup.reason.contains("already exists"));
    }

    #[test]
    fn node_set_updates_title_frontmatter_in_place() {
        let b = vec![("a/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string())];
        let out = apply(&b, &[Op::NodeSet {
            slug: "order".into(), title: Some("Sales Order".into()), description: None,
            stereotype: Some(vec!["aggregateRoot".into()]), abstract_: None, ty: None,
        }]).unwrap();
        assert_eq!(out[0].0, "a/order.md", "node.set never moves the file");
        assert!(out[0].1.contains("title: \"Sales Order\""));
        assert!(out[0].1.contains("# Sales Order"));
        assert!(out[0].1.contains("stereotype: [\"aggregateRoot\"]"));
    }

    #[test]
    fn node_rm_refuses_referenced_then_allows_cascade() {
        let b = vec![
            ("a/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- depends [Money](./money.md)\n".to_string()),
            ("a/money.md".to_string(), "---\ntype: uml.DataType\ntitle: Money\n---\n# Money\n".to_string()),
        ];
        let err = apply(&b, &[Op::NodeRm { slug:"money".into(), cascade: false }]).unwrap_err();
        assert!(err.reason.contains("referenced by"));
        assert!(err.reason.contains("order"));
        let out = apply(&b, &[Op::NodeRm { slug:"money".into(), cascade: true }]).unwrap();
        assert!(out.iter().all(|(p, _)| slug_of(p) != "money"));
    }

    #[test]
    fn node_rm_deletes_unreferenced() {
        let b = vec![("a/lonely.md".to_string(), "---\ntype: uml.Class\ntitle: Lonely\n---\n# Lonely\n".to_string())];
        let out = apply(&b, &[Op::NodeRm { slug:"lonely".into(), cascade: false }]).unwrap();
        assert!(out.is_empty());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p uaml ops::tests`
Expected: FAIL to compile.

- [ ] **Step 3: Add imports, helpers, variants, arms, appliers**

Add imports near the top of `ops/mod.rs`:
```rust
use crate::frontmatter::{FmValue, Frontmatter};
use crate::model::ClassifierType;
use crate::syntax::HintLine;
```
Add the frontmatter upsert + referrer scan:
```rust
fn fm_set(fm: &mut Frontmatter, key: &str, val: FmValue) {
    if let Some(e) = fm.entries.iter_mut().find(|(k, _)| k == key) {
        e.1 = val;
    } else {
        fm.entries.push((key.to_string(), val));
    }
}

fn str_list(items: &[String]) -> FmValue {
    FmValue::List(items.iter().map(|s| FmValue::Str(s.clone())).collect())
}

/// Slugs of every document that references `slug` (rel target, attribute
/// type-ref, `as [Ref]` name, diagram member/hint). Sorted, deduped.
pub(crate) fn referrers(work: &Bundle, slug: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (p, text) in work {
        let s = slug_of(p);
        if s == slug {
            continue;
        }
        let doc = parse_document(text);
        let hit = doc.sections.iter().any(|sec| match sec {
            Section::Attributes(attrs) => attrs.iter().any(|a| a.ty.ref_.as_deref() == Some(slug)),
            Section::Relationships(rels) => rels.iter().any(|r| {
                r.target_slug == slug
                    || matches!(&r.name, Some(ParsedName::Ref { slug: rs, .. }) if rs == slug)
            }),
            Section::Members(ms) => ms.iter().any(|m| m.slug == slug),
            Section::RenderHints(hs) => hs.iter().any(|h| match h {
                HintLine::Emphasize(list) => list.iter().any(|x| x == slug),
                HintLine::Collapse { slug: cs, .. } => cs == slug,
            }),
            _ => false,
        });
        if hit {
            out.push(s);
        }
    }
    out.sort();
    out.dedup();
    out
}
```
Add to `enum Op`:
```rust
    NodeNew {
        slug: String,
        ty: ClassifierType,
        title: String,
        stereotype: Vec<String>,
        description: Option<String>,
        abstract_: bool,
    },
    NodeSet {
        slug: String,
        title: Option<String>,
        description: Option<String>,
        stereotype: Option<Vec<String>>,
        abstract_: Option<bool>,
        ty: Option<ClassifierType>,
    },
    NodeRm { slug: String, cascade: bool },
```
Add to `apply_one`:
```rust
        Op::NodeNew { slug, ty, title, stereotype, description, abstract_ } => {
            op_node_new(work, slug, ty, title, stereotype, description, *abstract_)
        }
        Op::NodeSet { slug, title, description, stereotype, abstract_, ty } => {
            op_node_set(work, slug, title, description, stereotype, abstract_, ty)
        }
        Op::NodeRm { slug, cascade } => op_node_rm(work, slug, *cascade),
```
Add the appliers:
```rust
#[allow(clippy::too_many_arguments)]
fn op_node_new(
    work: &mut Bundle,
    slug: &str,
    ty: &ClassifierType,
    title: &str,
    stereotype: &[String],
    description: &Option<String>,
    abstract_: bool,
) -> Result<(), OpError> {
    if work.iter().any(|(p, _)| slug_of(p) == slug) {
        return Err(OpError::at("node.new", format!("document '{slug}' already exists")));
    }
    let mut entries: Vec<(String, FmValue)> = vec![("type".into(), FmValue::Str(ty.as_str()))];
    if !stereotype.is_empty() {
        entries.push(("stereotype".into(), str_list(stereotype)));
    }
    if abstract_ {
        entries.push(("abstract".into(), FmValue::Bool(true)));
    }
    entries.push(("title".into(), FmValue::Str(title.to_string())));
    if let Some(d) = description {
        entries.push(("description".into(), FmValue::Str(d.clone())));
    }
    let doc = Document {
        frontmatter: Frontmatter { entries },
        title: title.to_string(),
        sections: Vec::new(),
    };
    work.push((format!("{slug}.md"), serialize_document(&doc)));
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn op_node_set(
    work: &mut Bundle,
    slug: &str,
    title: &Option<String>,
    description: &Option<String>,
    stereotype: &Option<Vec<String>>,
    abstract_: &Option<bool>,
    ty: &Option<ClassifierType>,
) -> Result<(), OpError> {
    edit_doc(work, slug, "node.set", |doc| {
        if let Some(t) = title {
            fm_set(&mut doc.frontmatter, "title", FmValue::Str(t.clone()));
            doc.title = t.clone();
        }
        if let Some(d) = description {
            fm_set(&mut doc.frontmatter, "description", FmValue::Str(d.clone()));
        }
        if let Some(list) = stereotype {
            fm_set(&mut doc.frontmatter, "stereotype", str_list(list));
        }
        if let Some(a) = abstract_ {
            fm_set(&mut doc.frontmatter, "abstract", FmValue::Bool(*a));
        }
        if let Some(t) = ty {
            fm_set(&mut doc.frontmatter, "type", FmValue::Str(t.as_str()));
        }
        Ok(())
    })
}

fn op_node_rm(work: &mut Bundle, slug: &str, cascade: bool) -> Result<(), OpError> {
    let i = find_doc(work, slug, "node.rm")?;
    if !cascade {
        let refs = referrers(work, slug);
        if !refs.is_empty() {
            return Err(OpError::at(
                "node.rm",
                format!("'{slug}' is referenced by: {} (use --cascade)", refs.join(", ")),
            ));
        }
    }
    work.remove(i);
    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p uaml ops::tests`
Expected: PASS.

- [ ] **Step 5: Clippy + commit**

Run: `cargo clippy -p uaml --all-targets`
Expected: clean.
```bash
git add crates/uaml/src/ops/mod.rs
git commit -m "feat(uaml): node.new/set/rm + referrers scan"
```

---

### Task 7: Cross-file rename (`NodeRename`)

The riskiest op — isolated in its own file. Re-keys `from.md` → `to.md` and structurally rewrites every referrer's slug references (rel targets, attribute type-refs, `as [Ref]` name links, diagram members, collapse hints, and bare-slug emphasize lists), preserving all titles. Refuses if `to` already exists. Structured (parse → mutate → re-serialize) so it cannot corrupt a referrer, and only referring docs are touched.

**Files:**
- Create: `crates/uaml/src/ops/rename.rs`
- Modify: `crates/uaml/src/ops/mod.rs` (add `pub mod rename;`, `Op::NodeRename` variant + arm)

**Interfaces:**
- Consumes: `Bundle`, `OpError`, `find_doc`, `slug_of` (pub(crate) from Task 2), `parse_document`, `serialize_document`, `Document`, `Section`, `ParsedName`, `HintLine`.
- Produces:
  ```rust
  Op::NodeRename { from: String, to: String }
  pub(crate) fn crate::ops::rename::op_node_rename(work: &mut Bundle, from: &str, to: &str) -> Result<(), OpError>
  ```

- [ ] **Step 1: Write the failing test**

Create `crates/uaml/src/ops/rename.rs` with the test module first:
```rust
#[cfg(test)]
mod tests {
    use crate::ops::{apply, slug_of, Op};

    fn bundle() -> Vec<(String, String)> {
        vec![
            // the doc being renamed
            ("shop/order-line.md".to_string(),
             "---\ntype: uml.Class\ntitle: OrderLine\n---\n# OrderLine\n".to_string()),
            // a referrer: rel target + attribute type-ref + as-ref name link
            ("shop/order.md".to_string(),
             "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- first: [OrderLine](./order-line.md)\n\n## Relationships\n- composes [OrderLine](./order-line.md) as [OrderLine](./order-line.md): 1 to 1..* lines\n".to_string()),
            // a diagram referrer: member + emphasize (bare slug) + collapse (link)
            ("shop/diagram.md".to_string(),
             "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Members\n- [OrderLine](./order-line.md) at 10,20\n\n## Render hints\n- emphasize: order-line, order\n- collapse [OrderLine](./order-line.md)\n".to_string()),
        ]
    }

    #[test]
    fn rename_rewrites_every_referrer_and_rekeys_the_file() {
        let out = apply(&bundle(), &[Op::NodeRename { from: "order-line".into(), to: "line-item".into() }]).unwrap();

        // file re-keyed, directory preserved
        assert!(out.iter().any(|(p, _)| p == "shop/line-item.md"));
        assert!(out.iter().all(|(p, _)| slug_of(p) != "order-line"));

        let order = &out.iter().find(|(p, _)| p == "shop/order.md").unwrap().1;
        assert!(order.contains("(./line-item.md)"), "links repointed");
        assert!(!order.contains("(./order-line.md)"), "no stale link left");
        assert!(order.contains("[OrderLine]"), "titles preserved");

        let diagram = &out.iter().find(|(p, _)| p == "shop/diagram.md").unwrap().1;
        assert!(diagram.contains("(./line-item.md)"), "member + collapse repointed");
        assert!(diagram.contains("emphasize: line-item, order"), "bare-slug emphasize repointed");
    }

    #[test]
    fn rename_refuses_a_slug_collision() {
        let mut b = bundle();
        b.push(("shop/line-item.md".to_string(), "---\ntype: uml.Class\ntitle: LineItem\n---\n# LineItem\n".to_string()));
        let err = apply(&b, &[Op::NodeRename { from: "order-line".into(), to: "line-item".into() }]).unwrap_err();
        assert!(err.reason.contains("already exists"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml rename`
Expected: FAIL to compile — `Op::NodeRename` not defined, `rename` module absent.

- [ ] **Step 3: Implement the rename**

Prepend to `crates/uaml/src/ops/rename.rs`:
```rust
use super::{find_doc, slug_of, Bundle, OpError};
use crate::parse::parse_document;
use crate::serialize::serialize_document;
use crate::syntax::{Document, HintLine, ParsedName, Section};

/// Swap the basename of `path` to `to.md`, preserving any directory prefix.
fn replace_basename(path: &str, to: &str) -> String {
    match path.rfind(['/', '\\']) {
        Some(i) => format!("{}/{}.md", &path[..i], to),
        None => format!("{to}.md"),
    }
}

/// Repoint every `from`-slug reference inside one document to `to`. Titles are
/// left untouched. Returns whether anything changed.
fn rename_in_doc(doc: &mut Document, from: &str, to: &str) -> bool {
    let mut changed = false;
    for sec in &mut doc.sections {
        match sec {
            Section::Attributes(attrs) => {
                for a in attrs {
                    if a.ty.ref_.as_deref() == Some(from) {
                        a.ty.ref_ = Some(to.to_string());
                        changed = true;
                    }
                }
            }
            Section::Relationships(rels) => {
                for r in rels {
                    if r.target_slug == from {
                        r.target_slug = to.to_string();
                        changed = true;
                    }
                    if let Some(ParsedName::Ref { slug, .. }) = &mut r.name {
                        if slug == from {
                            *slug = to.to_string();
                            changed = true;
                        }
                    }
                }
            }
            Section::Members(ms) => {
                for m in ms {
                    if m.slug == from {
                        m.slug = to.to_string();
                        changed = true;
                    }
                }
            }
            Section::RenderHints(hs) => {
                for h in hs {
                    match h {
                        HintLine::Emphasize(list) => {
                            for x in list.iter_mut() {
                                if x == from {
                                    *x = to.to_string();
                                    changed = true;
                                }
                            }
                        }
                        HintLine::Collapse { slug, .. } => {
                            if slug == from {
                                *slug = to.to_string();
                                changed = true;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    changed
}

pub(crate) fn op_node_rename(work: &mut Bundle, from: &str, to: &str) -> Result<(), OpError> {
    let idx = find_doc(work, from, "node.rename")?;
    if work.iter().any(|(p, _)| slug_of(p) == to) {
        return Err(OpError::at("node.rename", format!("target slug '{to}' already exists")));
    }
    for (p, text) in work.iter_mut() {
        if slug_of(p) == from {
            continue; // the renamed doc's own body doesn't reference itself
        }
        let mut doc = parse_document(text);
        if rename_in_doc(&mut doc, from, to) {
            *text = serialize_document(&doc);
        }
    }
    work[idx].0 = replace_basename(&work[idx].0, to);
    Ok(())
}
```
Note: `OpError::at` is `pub(crate)` from Task 2 — reachable here as `super::OpError` already imports the type; call it as `OpError::at(...)`. If the compiler complains about visibility, mark `OpError::at` `pub(crate)` (it already is).

Wire into `crates/uaml/src/ops/mod.rs`:
```rust
pub mod rename;   // add near `pub mod selector;`
```
Add to `enum Op`:
```rust
    NodeRename { from: String, to: String },
```
Add to `apply_one`:
```rust
        Op::NodeRename { from, to } => rename::op_node_rename(work, from, to),
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p uaml`
Expected: PASS (rename + all prior).

- [ ] **Step 5: Clippy + commit**

Run: `cargo clippy -p uaml --all-targets`
Expected: clean.
```bash
git add crates/uaml/src/ops/rename.rs crates/uaml/src/ops/mod.rs
git commit -m "feat(uaml): cross-file node.rename (structured referrer rewrite)"
```

---

### Task 8: `OpDto` — the NDJSON wire layer (CLI)

The only place serde meets ops. An internally-tagged (`op`) enum, each variant self-versioned (`v`, default 1). `to_op` maps a parsed line to a core `Op` (rejecting an unknown `v` and validating markers/multiplicities/ends); `from_op` renders a core `Op` back to the wire form for `--emit`. Requires a `grammar::render_ends` (public inverse of `parse_ends`) so ends round-trip.

**Files:**
- Modify: `crates/uaml/src/grammar.rs` (add `pub fn render_ends`)
- Create: `crates/uaml-cli/src/ops_dto.rs`
- Modify: `crates/uaml-cli/src/main.rs` (add `mod ops_dto;`)

**Interfaces:**
- Consumes: core `Op`, `NameSpec`, `Selector`, `RelBy` (public from core `ops`), `Multiplicity`, `Visibility`, `ClassifierType`, `RelationshipKind`, `RelEnd`, `parse_ends`, `render_ends`.
- Produces:
  ```rust
  #[serde(tag = "op")] pub enum OpDto { /* one variant per op */ }
  impl OpDto { pub fn to_op(&self) -> Result<Op, String>; pub fn from_op(op: &Op) -> OpDto; }
  pub fn crate::grammar::render_ends(from: &RelEnd, to: &RelEnd) -> String
  ```

- [ ] **Step 1: Add `render_ends` to grammar.rs**

The private `render_end` already exists. Add:
```rust
/// Render a `<near> to <far>` ends clause (inverse of `parse_ends`).
pub fn render_ends(from: &RelEnd, to: &RelEnd) -> String {
    format!("{} to {}", render_end(from), render_end(to))
}
```
Run `cargo test -p uaml grammar` — still green.

- [ ] **Step 2: Write the failing tests**

Create `crates/uaml-cli/src/ops_dto.rs` with the test module first:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use uaml::ops::Op;

    fn round_trip(line: &str) -> Op {
        let dto: OpDto = serde_json::from_str(line).unwrap();
        dto.to_op().unwrap()
    }

    #[test]
    fn parses_attr_add_line() {
        let op = round_trip(r#"{"v":1,"op":"attr.add","node":"order","name":"total","ty":"Money","mult":"0..1"}"#);
        match op {
            Op::AttrAdd { node, name, ty_token, multiplicity, .. } => {
                assert_eq!((node.as_str(), name.as_str(), ty_token.as_str()), ("order", "total", "Money"));
                assert_eq!(multiplicity.as_str(), "0..1");
            }
            _ => panic!("wrong op"),
        }
    }

    #[test]
    fn version_defaults_to_one_and_rejects_unknown() {
        // absent v defaults to 1
        let _ = round_trip(r#"{"op":"value.add","node":"e","literal":"X"}"#);
        // explicit unknown v is rejected
        let dto: OpDto = serde_json::from_str(r#"{"v":2,"op":"value.add","node":"e","literal":"X"}"#).unwrap();
        assert!(dto.to_op().is_err());
    }

    #[test]
    fn rel_add_line_parses_kind_and_ends() {
        let op = round_trip(r#"{"v":1,"op":"rel.add","source":"order","kind":"composes","target":"order-line","ends":"1 to 1..* lines"}"#);
        assert!(matches!(op, Op::RelAdd { .. }));
    }

    #[test]
    fn from_op_round_trips_through_to_op() {
        // build a core Op, render to a DTO line, parse back, expect equality
        let op = Op::AttrAdd {
            node: "order".into(), name: "total".into(), ty_token: "Money".into(),
            multiplicity: uaml::multiplicity::Multiplicity::parse("0..1").unwrap(), visibility: None,
        };
        let line = serde_json::to_string(&OpDto::from_op(&op)).unwrap();
        let back = { let dto: OpDto = serde_json::from_str(&line).unwrap(); dto.to_op().unwrap() };
        assert_eq!(op, back);
    }
}
```

- [ ] **Step 3: Implement `OpDto`**

Prepend to `crates/uaml-cli/src/ops_dto.rs`:
```rust
use serde::{Deserialize, Serialize};
use uaml::grammar::{parse_ends, render_ends};
use uaml::model::{ClassifierType, RelEnd, RelationshipKind, Visibility};
use uaml::multiplicity::Multiplicity;
use uaml::ops::{NameSpec, Op, RelBy, Selector};

fn one() -> u32 { 1 }

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum OpDto {
    #[serde(rename = "node.new")]
    NodeNew { #[serde(default = "one")] v: u32, slug: String, ty: String, title: String,
              #[serde(default)] stereotype: Vec<String>, #[serde(default)] desc: Option<String>,
              #[serde(default, rename = "abstract")] abstract_: bool },
    #[serde(rename = "node.rename")]
    NodeRename { #[serde(default = "one")] v: u32, from: String, to: String },
    #[serde(rename = "node.set")]
    NodeSet { #[serde(default = "one")] v: u32, slug: String, #[serde(default)] title: Option<String>,
              #[serde(default)] desc: Option<String>, #[serde(default)] stereotype: Option<Vec<String>>,
              #[serde(default, rename = "abstract")] abstract_: Option<bool>, #[serde(default)] ty: Option<String> },
    #[serde(rename = "node.rm")]
    NodeRm { #[serde(default = "one")] v: u32, slug: String, #[serde(default)] cascade: bool },
    #[serde(rename = "attr.add")]
    AttrAdd { #[serde(default = "one")] v: u32, node: String, name: String, ty: String,
              #[serde(default)] mult: Option<String>, #[serde(default)] vis: Option<String> },
    #[serde(rename = "attr.set")]
    AttrSet { #[serde(default = "one")] v: u32, node: String, name: String, #[serde(default)] ty: Option<String>,
              #[serde(default)] mult: Option<String>, #[serde(default)] vis: Option<String>, #[serde(default)] rename: Option<String> },
    #[serde(rename = "attr.rm")]
    AttrRm { #[serde(default = "one")] v: u32, node: String, name: String },
    #[serde(rename = "value.add")]
    ValueAdd { #[serde(default = "one")] v: u32, node: String, literal: String },
    #[serde(rename = "value.rm")]
    ValueRm { #[serde(default = "one")] v: u32, node: String, literal: String },
    #[serde(rename = "rel.add")]
    RelAdd { #[serde(default = "one")] v: u32, source: String, kind: String, target: String,
             #[serde(default, rename = "as")] as_label: Option<String>, #[serde(default)] as_ref: Option<String>,
             #[serde(default)] ends: Option<String> },
    #[serde(rename = "rel.set")]
    RelSet { #[serde(default = "one")] v: u32, source: String, #[serde(default)] kind: Option<String>,
             #[serde(default)] target: Option<String>, #[serde(default, rename = "as")] as_sel: Option<String>,
             #[serde(default)] ends: Option<String>, #[serde(default, rename = "set_as")] set_label: Option<String>,
             #[serde(default)] set_as_ref: Option<String> },
    #[serde(rename = "rel.rm")]
    RelRm { #[serde(default = "one")] v: u32, source: String, #[serde(default)] kind: Option<String>,
            #[serde(default)] target: Option<String>, #[serde(default, rename = "as")] as_sel: Option<String> },
}

fn check_v(v: u32, op: &str) -> Result<(), String> {
    if v != 1 { return Err(format!("unsupported version {v} for op '{op}'")); }
    Ok(())
}
fn mult_req(s: &Option<String>) -> Result<Multiplicity, String> {
    match s { None => Ok(Multiplicity::default()),
              Some(m) => Multiplicity::parse(m).ok_or_else(|| format!("bad multiplicity '{m}'")) }
}
fn mult_opt(s: &Option<String>) -> Result<Option<Multiplicity>, String> {
    s.as_ref().map(|m| Multiplicity::parse(m).ok_or_else(|| format!("bad multiplicity '{m}'"))).transpose()
}
fn vis_opt(s: &Option<String>) -> Result<Option<Visibility>, String> {
    match s { None => Ok(None),
              Some(c) => { let ch = c.chars().next().ok_or("empty visibility")?;
                           Visibility::from_marker(ch).map(Some).ok_or_else(|| format!("bad visibility '{c}'")) } }
}
fn ends_opt(s: &Option<String>) -> Result<Option<(RelEnd, RelEnd)>, String> {
    s.as_ref().map(|e| parse_ends(e).ok_or_else(|| format!("bad ends '{e}'"))).transpose()
}
fn kind_req(s: &str) -> Result<RelationshipKind, String> {
    RelationshipKind::parse(s).ok_or_else(|| format!("unknown verb '{s}'"))
}
fn name_of(label: &Option<String>, as_ref: &Option<String>) -> Option<NameSpec> {
    label.as_ref().map(|l| NameSpec::Label(l.clone()))
        .or_else(|| as_ref.as_ref().map(|s| NameSpec::Ref(s.clone())))
}
fn rel_sel(source: &str, kind: &Option<String>, target: &Option<String>, as_sel: &Option<String>) -> Result<Selector, String> {
    let by = match (kind, target, as_sel) {
        (Some(k), Some(t), _) => RelBy::Endpoint { kind: kind_req(k)?, target: t.clone() },
        (_, _, Some(n)) => RelBy::Named(n.clone()),
        _ => return Err("relationship selector needs kind+target or as".into()),
    };
    Ok(Selector::Rel { source: source.to_string(), by })
}

impl OpDto {
    pub fn to_op(&self) -> Result<Op, String> {
        match self {
            OpDto::NodeNew { v, slug, ty, title, stereotype, desc, abstract_ } => { check_v(*v, "node.new")?;
                Ok(Op::NodeNew { slug: slug.clone(), ty: ClassifierType::parse(ty), title: title.clone(),
                    stereotype: stereotype.clone(), description: desc.clone(), abstract_: *abstract_ }) }
            OpDto::NodeRename { v, from, to } => { check_v(*v, "node.rename")?;
                Ok(Op::NodeRename { from: from.clone(), to: to.clone() }) }
            OpDto::NodeSet { v, slug, title, desc, stereotype, abstract_, ty } => { check_v(*v, "node.set")?;
                Ok(Op::NodeSet { slug: slug.clone(), title: title.clone(), description: desc.clone(),
                    stereotype: stereotype.clone(), abstract_: *abstract_, ty: ty.as_ref().map(|t| ClassifierType::parse(t)) }) }
            OpDto::NodeRm { v, slug, cascade } => { check_v(*v, "node.rm")?;
                Ok(Op::NodeRm { slug: slug.clone(), cascade: *cascade }) }
            OpDto::AttrAdd { v, node, name, ty, mult, vis } => { check_v(*v, "attr.add")?;
                Ok(Op::AttrAdd { node: node.clone(), name: name.clone(), ty_token: ty.clone(),
                    multiplicity: mult_req(mult)?, visibility: vis_opt(vis)? }) }
            OpDto::AttrSet { v, node, name, ty, mult, vis, rename } => { check_v(*v, "attr.set")?;
                Ok(Op::AttrSet { node: node.clone(), name: name.clone(), ty_token: ty.clone(),
                    multiplicity: mult_opt(mult)?, visibility: vis_opt(vis)?, rename: rename.clone() }) }
            OpDto::AttrRm { v, node, name } => { check_v(*v, "attr.rm")?;
                Ok(Op::AttrRm { node: node.clone(), name: name.clone() }) }
            OpDto::ValueAdd { v, node, literal } => { check_v(*v, "value.add")?;
                Ok(Op::ValueAdd { node: node.clone(), literal: literal.clone() }) }
            OpDto::ValueRm { v, node, literal } => { check_v(*v, "value.rm")?;
                Ok(Op::ValueRm { node: node.clone(), literal: literal.clone() }) }
            OpDto::RelAdd { v, source, kind, target, as_label, as_ref, ends } => { check_v(*v, "rel.add")?;
                Ok(Op::RelAdd { source: source.clone(), kind: kind_req(kind)?, target: target.clone(),
                    name: name_of(as_label, as_ref), ends: ends_opt(ends)? }) }
            OpDto::RelSet { v, source, kind, target, as_sel, ends, set_label, set_as_ref } => { check_v(*v, "rel.set")?;
                Ok(Op::RelSet { selector: rel_sel(source, kind, target, as_sel)?,
                    ends: ends_opt(ends)?, name: name_of(set_label, set_as_ref) }) }
            OpDto::RelRm { v, source, kind, target, as_sel } => { check_v(*v, "rel.rm")?;
                Ok(Op::RelRm { selector: rel_sel(source, kind, target, as_sel)? }) }
        }
    }

    pub fn from_op(op: &Op) -> OpDto {
        let ends_str = |e: &Option<(RelEnd, RelEnd)>| e.as_ref().map(|(f, t)| render_ends(f, t));
        let name_parts = |n: &Option<NameSpec>| match n {
            Some(NameSpec::Label(l)) => (Some(l.clone()), None),
            Some(NameSpec::Ref(s)) => (None, Some(s.clone())),
            None => (None, None),
        };
        match op {
            Op::NodeNew { slug, ty, title, stereotype, description, abstract_ } =>
                OpDto::NodeNew { v: 1, slug: slug.clone(), ty: ty.as_str(), title: title.clone(),
                    stereotype: stereotype.clone(), desc: description.clone(), abstract_: *abstract_ },
            Op::NodeRename { from, to } => OpDto::NodeRename { v: 1, from: from.clone(), to: to.clone() },
            Op::NodeSet { slug, title, description, stereotype, abstract_, ty } =>
                OpDto::NodeSet { v: 1, slug: slug.clone(), title: title.clone(), desc: description.clone(),
                    stereotype: stereotype.clone(), abstract_: *abstract_, ty: ty.as_ref().map(|t| t.as_str()) },
            Op::NodeRm { slug, cascade } => OpDto::NodeRm { v: 1, slug: slug.clone(), cascade: *cascade },
            Op::AttrAdd { node, name, ty_token, multiplicity, visibility } =>
                OpDto::AttrAdd { v: 1, node: node.clone(), name: name.clone(), ty: ty_token.clone(),
                    mult: Some(multiplicity.as_str().to_string()), vis: visibility.map(|x| x.marker().to_string()) },
            Op::AttrSet { node, name, ty_token, multiplicity, visibility, rename } =>
                OpDto::AttrSet { v: 1, node: node.clone(), name: name.clone(), ty: ty_token.clone(),
                    mult: multiplicity.as_ref().map(|m| m.as_str().to_string()),
                    vis: visibility.map(|x| x.marker().to_string()), rename: rename.clone() },
            Op::AttrRm { node, name } => OpDto::AttrRm { v: 1, node: node.clone(), name: name.clone() },
            Op::ValueAdd { node, literal } => OpDto::ValueAdd { v: 1, node: node.clone(), literal: literal.clone() },
            Op::ValueRm { node, literal } => OpDto::ValueRm { v: 1, node: node.clone(), literal: literal.clone() },
            Op::RelAdd { source, kind, target, name, ends } => {
                let (as_label, as_ref) = name_parts(name);
                OpDto::RelAdd { v: 1, source: source.clone(), kind: kind.as_str().to_string(), target: target.clone(),
                    as_label, as_ref, ends: ends_str(ends) }
            }
            Op::RelSet { selector, ends, name } => {
                let (source, kind, target, as_sel) = sel_parts(selector);
                let (set_label, set_as_ref) = name_parts(name);
                OpDto::RelSet { v: 1, source, kind, target, as_sel, ends: ends_str(ends), set_label, set_as_ref }
            }
            Op::RelRm { selector } => {
                let (source, kind, target, as_sel) = sel_parts(selector);
                OpDto::RelRm { v: 1, source, kind, target, as_sel }
            }
        }
    }
}

/// Decompose a `Selector::Rel` into wire fields for `from_op`.
fn sel_parts(sel: &Selector) -> (String, Option<String>, Option<String>, Option<String>) {
    match sel {
        Selector::Rel { source, by: RelBy::Endpoint { kind, target } } =>
            (source.clone(), Some(kind.as_str().to_string()), Some(target.clone()), None),
        Selector::Rel { source, by: RelBy::Named(n) } => (source.clone(), None, None, Some(n.clone())),
        // node/attr/value selectors never reach a rel op; render source-only as a defensive default
        Selector::Node(s) => (s.clone(), None, None, None),
        Selector::Attr { node, .. } | Selector::Value { node, .. } => (node.clone(), None, None, None),
    }
}
```
Note: `NameSpec`, `RelBy`, `Selector`, `Op` must be re-exported from `uaml::ops` — confirm `ops/mod.rs` has `pub use selector::{RelBy, Selector};` (Task 1) and `pub enum NameSpec` / `pub enum Op` (public). Add `mod ops_dto;` to `crates/uaml-cli/src/main.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p uaml-cli ops_dto`
Expected: PASS (4 tests).

- [ ] **Step 5: Clippy + commit**

Run: `cargo clippy -p uaml-cli --all-targets`
Expected: clean.
```bash
git add crates/uaml/src/grammar.rs crates/uaml-cli/src/ops_dto.rs crates/uaml-cli/src/main.rs
git commit -m "feat(uaml-cli): OpDto NDJSON wire layer + grammar::render_ends"
```

---

### Task 9: Mutating CLI subcommands (`node` / `attr` / `value` / `rel`)

Wire the sugar commands to the engine. Each arm builds an `OpDto` from clap args, then `run_mutation` runs the shared pipeline: `--emit` prints the NDJSON line (no I/O); otherwise `to_op()` → load bundle from `--dir` → `apply` → `--stdout` blob / `--dry-run` diff / write-back. Building an `OpDto` (not a core `Op`) means the CLI reuses all of `OpDto::to_op`'s parsing/validation, and `--emit` is exact.

**Files:**
- Modify: `crates/uaml-cli/src/main.rs` (subcommands + arms + `run_mutation` + helpers)
- Modify: `crates/uaml-cli/src/commands.rs` (add `render_diff` + `diff_lines`, tested)
- Modify: `crates/uaml-cli/src/io.rs` (add `write_back`)

**Interfaces:**
- Consumes: `OpDto` (Task 8), `uaml::ops::apply`, `uaml::ops::OpError`, `io::read_files` (existing).
- Produces:
  ```rust
  fn run_mutation(common: &Common, dto: OpDto) -> i32
  pub fn commands::render_diff(old: &[(String,String)], new: &[(String,String)]) -> String
  pub fn io::write_back(old: &[(String,String)], new: &[(String,String)]) -> std::io::Result<Vec<String>>
  ```

- [ ] **Step 1: Write the failing tests (pure diff + clap parse)**

Add to `crates/uaml-cli/src/commands.rs` `mod tests`:
```rust
    #[test]
    fn render_diff_shows_added_changed_deleted() {
        let old = vec![("a.md".to_string(), "x\ny\n".to_string()), ("gone.md".to_string(), "z\n".to_string())];
        let new = vec![("a.md".to_string(), "x\nY\n".to_string()), ("new.md".to_string(), "q\n".to_string())];
        let d = render_diff(&old, &new);
        assert!(d.contains("a.md"));
        assert!(d.contains("-y"));
        assert!(d.contains("+Y"));
        assert!(d.contains("new.md"));   // added
        assert!(d.contains("gone.md"));  // deleted
    }
```
Add to `crates/uaml-cli/src/main.rs` `mod tests`:
```rust
    #[test]
    fn parses_attr_add() {
        let cli = Cli::try_parse_from(["uaml", "attr", "add", "order", "total", "Money", "--mult", "0..1"]).unwrap();
        assert!(matches!(cli.command, Command::Attr { .. }));
    }

    #[test]
    fn parses_rel_add_with_ends() {
        let cli = Cli::try_parse_from(["uaml", "rel", "add", "order", "composes", "order-line", "--ends", "1 to 1..* lines"]).unwrap();
        assert!(matches!(cli.command, Command::Rel { .. }));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p uaml-cli`
Expected: FAIL to compile — `render_diff`, `Command::Attr`, `Command::Rel` missing.

- [ ] **Step 3: Add `diff_lines` + `render_diff` to commands.rs**

```rust
fn diff_lines(a: &str, b: &str) -> String {
    let al: Vec<&str> = a.lines().collect();
    let bl: Vec<&str> = b.lines().collect();
    let mut s = 0;
    while s < al.len() && s < bl.len() && al[s] == bl[s] { s += 1; }
    let (mut ea, mut eb) = (al.len(), bl.len());
    while ea > s && eb > s && al[ea - 1] == bl[eb - 1] { ea -= 1; eb -= 1; }
    let mut out = String::new();
    for l in &al[s..ea] { out.push_str(&format!("-{l}\n")); }
    for l in &bl[s..eb] { out.push_str(&format!("+{l}\n")); }
    out
}

pub fn render_diff(old: &[(String, String)], new: &[(String, String)]) -> String {
    use std::collections::BTreeMap;
    let om: BTreeMap<&str, &str> = old.iter().map(|(p, c)| (p.as_str(), c.as_str())).collect();
    let nm: BTreeMap<&str, &str> = new.iter().map(|(p, c)| (p.as_str(), c.as_str())).collect();
    let mut out = String::new();
    for (p, c) in &nm {
        match om.get(p) {
            Some(old_c) if old_c == c => {}
            Some(old_c) => { out.push_str(&format!("~ {p}\n")); out.push_str(&diff_lines(old_c, c)); }
            None => { out.push_str(&format!("+ {p} (new)\n")); out.push_str(&diff_lines("", c)); }
        }
    }
    for p in om.keys() {
        if !nm.contains_key(p) { out.push_str(&format!("- {p} (deleted)\n")); }
    }
    if out.is_empty() { out.push_str("no changes\n"); }
    out
}
```

- [ ] **Step 4: Add `write_back` to io.rs**

```rust
use std::collections::BTreeMap;

/// Write only changed/added entries; delete entries dropped from the bundle.
/// Returns a human list of what happened.
pub fn write_back(old: &[(String, String)], new: &[(String, String)]) -> std::io::Result<Vec<String>> {
    let om: BTreeMap<&str, &str> = old.iter().map(|(p, c)| (p.as_str(), c.as_str())).collect();
    let nm: BTreeMap<&str, &str> = new.iter().map(|(p, c)| (p.as_str(), c.as_str())).collect();
    let mut touched = Vec::new();
    for (p, c) in &nm {
        if om.get(p) != Some(c) {
            fs::write(p, c)?;
            touched.push(format!("wrote {p}"));
        }
    }
    for p in om.keys() {
        if !nm.contains_key(p) {
            let _ = fs::remove_file(p);
            touched.push(format!("deleted {p}"));
        }
    }
    Ok(touched)
}
```

- [ ] **Step 5: Add subcommands, `Common`, arms, `run_mutation` to main.rs**

Add `use clap::Args;` and, alongside the existing enum, the shared flags + subcommand action enums:
```rust
#[derive(Args)]
struct Common {
    /// Bundle root; recursively collects *.md. Default: current directory.
    #[arg(long, default_value = ".")]
    dir: PathBuf,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    stdout: bool,
    #[arg(long)]
    emit: bool,
    #[arg(long, value_enum, default_value_t = Format::Human)]
    format: Format,
}

#[derive(Subcommand)]
enum NodeCmd {
    New { slug: String, #[arg(long)] r#type: String, #[arg(long)] title: String,
          #[arg(long, value_delimiter = ',')] stereotype: Vec<String>,
          #[arg(long)] desc: Option<String>, #[arg(long)] r#abstract: bool },
    Rename { from: String, to: String },
    Set { slug: String, #[arg(long)] title: Option<String>, #[arg(long)] desc: Option<String>,
          #[arg(long, value_delimiter = ',')] stereotype: Option<Vec<String>>,
          #[arg(long)] r#abstract: Option<bool>, #[arg(long)] r#type: Option<String> },
    Rm { slug: String, #[arg(long)] cascade: bool },
}

#[derive(Subcommand)]
enum AttrCmd {
    Add { node: String, name: String, r#type: String, #[arg(long)] mult: Option<String>, #[arg(long)] vis: Option<String> },
    Set { node: String, name: String, #[arg(long)] r#type: Option<String>, #[arg(long)] mult: Option<String>,
          #[arg(long)] vis: Option<String>, #[arg(long)] rename: Option<String> },
    Rm { node: String, name: String },
}

#[derive(Subcommand)]
enum ValueCmd {
    Add { node: String, literal: String },
    Rm { node: String, literal: String },
}

#[derive(Subcommand)]
enum RelCmd {
    Add { source: String, verb: String, target: String, #[arg(long)] ends: Option<String>,
          #[arg(long = "as")] as_label: Option<String>, #[arg(long)] as_ref: Option<String> },
    Set { source: String, #[arg(long)] verb: Option<String>, #[arg(long)] target: Option<String>,
          #[arg(long = "as")] as_sel: Option<String>, #[arg(long)] ends: Option<String>,
          #[arg(long = "set-as")] set_label: Option<String>, #[arg(long)] set_as_ref: Option<String> },
    Rm { source: String, #[arg(long)] verb: Option<String>, #[arg(long)] target: Option<String>, #[arg(long = "as")] as_sel: Option<String> },
}
```
Add variants to `enum Command`:
```rust
    Node { #[command(subcommand)] action: NodeCmd, #[command(flatten)] common: Common },
    Attr { #[command(subcommand)] action: AttrCmd, #[command(flatten)] common: Common },
    Value { #[command(subcommand)] action: ValueCmd, #[command(flatten)] common: Common },
    Rel { #[command(subcommand)] action: RelCmd, #[command(flatten)] common: Common },
```
Add arms to `main`'s match (each builds an `OpDto`, then `run_mutation`):
```rust
        Command::Node { action, common } => run_mutation(&common, node_dto(action)),
        Command::Attr { action, common } => run_mutation(&common, attr_dto(action)),
        Command::Value { action, common } => run_mutation(&common, value_dto(action)),
        Command::Rel { action, common } => run_mutation(&common, rel_dto(action)),
```
Add the `*_dto` builders + `run_mutation` + helpers (use `crate::ops_dto::OpDto`):
```rust
use crate::ops_dto::OpDto;

fn node_dto(a: NodeCmd) -> OpDto {
    match a {
        NodeCmd::New { slug, r#type, title, stereotype, desc, r#abstract } =>
            OpDto::NodeNew { v: 1, slug, ty: r#type, title, stereotype, desc, abstract_: r#abstract },
        NodeCmd::Rename { from, to } => OpDto::NodeRename { v: 1, from, to },
        NodeCmd::Set { slug, title, desc, stereotype, r#abstract, r#type } =>
            OpDto::NodeSet { v: 1, slug, title, desc, stereotype, abstract_: r#abstract, ty: r#type },
        NodeCmd::Rm { slug, cascade } => OpDto::NodeRm { v: 1, slug, cascade },
    }
}
fn attr_dto(a: AttrCmd) -> OpDto {
    match a {
        AttrCmd::Add { node, name, r#type, mult, vis } => OpDto::AttrAdd { v: 1, node, name, ty: r#type, mult, vis },
        AttrCmd::Set { node, name, r#type, mult, vis, rename } => OpDto::AttrSet { v: 1, node, name, ty: r#type, mult, vis, rename },
        AttrCmd::Rm { node, name } => OpDto::AttrRm { v: 1, node, name },
    }
}
fn value_dto(a: ValueCmd) -> OpDto {
    match a {
        ValueCmd::Add { node, literal } => OpDto::ValueAdd { v: 1, node, literal },
        ValueCmd::Rm { node, literal } => OpDto::ValueRm { v: 1, node, literal },
    }
}
fn rel_dto(a: RelCmd) -> OpDto {
    match a {
        RelCmd::Add { source, verb, target, ends, as_label, as_ref } =>
            OpDto::RelAdd { v: 1, source, kind: verb, target, as_label, as_ref, ends },
        RelCmd::Set { source, verb, target, as_sel, ends, set_label, set_as_ref } =>
            OpDto::RelSet { v: 1, source, kind: verb, target, as_sel, ends, set_label, set_as_ref },
        RelCmd::Rm { source, verb, target, as_sel } => OpDto::RelRm { v: 1, source, kind: verb, target, as_sel },
    }
}

fn to_blob(bundle: &[(String, String)]) -> String {
    bundle.iter().map(|(p, c)| format!("<!-- {p} -->\n{c}")).collect::<Vec<_>>().join("\n")
}

fn run_mutation(common: &Common, dto: OpDto) -> i32 {
    if [common.emit, common.stdout, common.dry_run].iter().filter(|x| **x).count() > 1 {
        eprintln!("uaml: --emit, --stdout, --dry-run are mutually exclusive");
        return 2;
    }
    if common.emit {
        match serde_json::to_string(&dto) {
            Ok(line) => { println!("{line}"); return 0; }
            Err(e) => { eprintln!("uaml: {e}"); return 2; }
        }
    }
    let op = match dto.to_op() {
        Ok(o) => o,
        Err(e) => { eprintln!("uaml: {e}"); return 1; }
    };
    let bundle = match io::read_files(&[common.dir.clone()]) {
        Ok(b) => b,
        Err(e) => { eprintln!("uaml: {e}"); return 2; }
    };
    match uaml::ops::apply(&bundle, std::slice::from_ref(&op)) {
        Ok(new) => {
            if common.stdout {
                print!("{}", to_blob(&new));
                0
            } else if common.dry_run {
                print!("{}", commands::render_diff(&bundle, &new));
                0
            } else {
                match io::write_back(&bundle, &new) {
                    Ok(touched) => { for t in touched { println!("uaml: {t}"); } 0 }
                    Err(e) => { eprintln!("uaml: {e}"); 2 }
                }
            }
        }
        Err(e) => {
            let sel = e.selector.as_ref().map(|s| format!(" [{s}]")).unwrap_or_default();
            eprintln!("uaml: op {}: {}{sel}", e.index, e.reason);
            1
        }
    }
}
```

- [ ] **Step 5b: Run tests to verify they pass**

Run: `cargo test -p uaml-cli`
Expected: PASS.

- [ ] **Step 6: Clippy + commit**

Run: `cargo clippy -p uaml-cli --all-targets`
Expected: clean.
```bash
git add crates/uaml-cli/src/main.rs crates/uaml-cli/src/commands.rs crates/uaml-cli/src/io.rs
git commit -m "feat(uaml-cli): node/attr/value/rel subcommands via OpDto + dry-run/stdout/emit"
```

---

### Task 10: `apply` (NDJSON batch) + `show` / `refs` / `list` queries

The op-log consumer and the read-only inspectors. `apply` streams NDJSON (one self-versioned op per line, blanks skipped), maps each to a core `Op` (unknown `v`/bad line → error naming the line number), and runs the whole batch atomically through the same output handling as the sugar commands. `show`/`refs`/`list` build the resolved `Model` and print — no mutation.

**Files:**
- Modify: `crates/uaml/src/ops/mod.rs` (make `referrers` `pub`)
- Modify: `crates/uaml-cli/src/io.rs` (add `read_ndjson`)
- Modify: `crates/uaml-cli/src/main.rs` (add `Apply`/`Show`/`Refs`/`List` subcommands + `run_apply`/`run_batch`/query fns)

**Interfaces:**
- Consumes: `OpDto::to_op`, `uaml::ops::apply`, `uaml::ops::referrers`, `uaml::parse::build_model`, `uaml::model::Model`.
- Produces:
  ```rust
  pub fn io::read_ndjson(src: &str) -> std::io::Result<Vec<(usize, String)>>   // (line_no, trimmed non-blank line)
  fn run_apply(ops_src: &str, common: &Common) -> i32
  fn run_show(slug: &str, q: &QueryArgs) -> i32
  fn run_refs(slug: &str, q: &QueryArgs) -> i32
  fn run_list(ty: &Option<String>, q: &QueryArgs) -> i32
  ```

- [ ] **Step 1: Make `referrers` public**

In `crates/uaml/src/ops/mod.rs`, change `pub(crate) fn referrers` to `pub fn referrers`. Run `cargo test -p uaml` — still green.

- [ ] **Step 2: Write the failing tests**

Add to `crates/uaml-cli/src/io.rs` `mod tests`:
```rust
    #[test]
    fn read_ndjson_skips_blanks_and_numbers_lines() {
        // write a temp file
        let dir = std::env::temp_dir().join(format!("uaml_ndjson_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("ops.ndjson");
        std::fs::write(&f, "{\"op\":\"a\"}\n\n{\"op\":\"b\"}\n").unwrap();
        let lines = read_ndjson(f.to_str().unwrap()).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].0, 1);
        assert_eq!(lines[1].0, 3, "blank line 2 skipped, numbering preserved");
    }
```
Add to `crates/uaml-cli/src/main.rs` `mod tests`:
```rust
    #[test]
    fn parses_apply_and_show() {
        assert!(matches!(Cli::try_parse_from(["uaml", "apply", "ops.ndjson"]).unwrap().command, Command::Apply { .. }));
        assert!(matches!(Cli::try_parse_from(["uaml", "show", "order"]).unwrap().command, Command::Show { .. }));
    }
```

- [ ] **Step 3: Add `read_ndjson` to io.rs**

```rust
/// Read an NDJSON op-log: `(line_number, trimmed_line)` for each non-blank line.
/// `src == "-"` reads stdin.
pub fn read_ndjson(src: &str) -> std::io::Result<Vec<(usize, String)>> {
    let text = if src == "-" {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        buf
    } else {
        fs::read_to_string(src)?
    };
    Ok(text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim().to_string()))
        .filter(|(_, l)| !l.is_empty())
        .collect())
}
```

- [ ] **Step 4: Add subcommands, `QueryArgs`, arms, and run fns to main.rs**

Add to `enum Command`:
```rust
    Apply { /// NDJSON op-log file, or `-` for stdin.
            ops: String, #[command(flatten)] common: Common },
    Show { slug: String, #[command(flatten)] query: QueryArgs },
    Refs { slug: String, #[command(flatten)] query: QueryArgs },
    List { #[arg(long)] r#type: Option<String>, #[command(flatten)] query: QueryArgs },
```
Add:
```rust
#[derive(Args)]
struct QueryArgs {
    #[arg(long, default_value = ".")]
    dir: PathBuf,
    #[arg(long, value_enum, default_value_t = Format::Human)]
    format: Format,
}
```
Add arms to `main`:
```rust
        Command::Apply { ops, common } => run_apply(&ops, &common),
        Command::Show { slug, query } => run_show(&slug, &query),
        Command::Refs { slug, query } => run_refs(&slug, &query),
        Command::List { r#type, query } => run_list(&r#type, &query),
```
Add the run functions:
```rust
fn run_batch(common: &Common, ops: Vec<uaml::ops::Op>) -> i32 {
    let bundle = match io::read_files(&[common.dir.clone()]) {
        Ok(b) => b,
        Err(e) => { eprintln!("uaml: {e}"); return 2; }
    };
    match uaml::ops::apply(&bundle, &ops) {
        Ok(new) => {
            if common.stdout {
                print!("{}", to_blob(&new));
                0
            } else if common.dry_run {
                print!("{}", commands::render_diff(&bundle, &new));
                0
            } else {
                match io::write_back(&bundle, &new) {
                    Ok(touched) => { for t in touched { println!("uaml: {t}"); } 0 }
                    Err(e) => { eprintln!("uaml: {e}"); 2 }
                }
            }
        }
        Err(e) => {
            let sel = e.selector.as_ref().map(|s| format!(" [{s}]")).unwrap_or_default();
            eprintln!("uaml: op {}: {}{sel}", e.index, e.reason);
            1
        }
    }
}

fn run_apply(ops_src: &str, common: &Common) -> i32 {
    let lines = match io::read_ndjson(ops_src) {
        Ok(l) => l,
        Err(e) => { eprintln!("uaml: {e}"); return 2; }
    };
    let mut ops = Vec::new();
    for (n, line) in lines {
        let dto: OpDto = match serde_json::from_str(&line) {
            Ok(d) => d,
            Err(e) => { eprintln!("uaml: line {n}: {e}"); return 1; }
        };
        match dto.to_op() {
            Ok(o) => ops.push(o),
            Err(e) => { eprintln!("uaml: line {n}: {e}"); return 1; }
        }
    }
    run_batch(common, ops)
}

fn run_show(slug: &str, q: &QueryArgs) -> i32 {
    let bundle = match io::read_files(&[q.dir.clone()]) {
        Ok(b) => b,
        Err(e) => { eprintln!("uaml: {e}"); return 2; }
    };
    let model = uaml::parse::build_model(&bundle);
    let Some(node) = model.node(slug) else {
        eprintln!("uaml: no classifier '{slug}'");
        return 1;
    };
    match q.format {
        Format::Human => {
            println!("{} ({})", node.title, node.ty.as_str());
            for a in &node.attributes {
                println!("  - {}: {} [{}]", a.name, a.ty.name, a.multiplicity.as_str());
            }
            for v in &node.values {
                println!("  = {v}");
            }
            for e in model.edges.iter().filter(|e| e.source == slug || e.target == slug) {
                println!("  {} {} {}", e.source, e.kind.as_str(), e.target);
            }
            0
        }
        Format::Json => {
            let refs = uaml::ops::referrers(&bundle, slug);
            let dto = serde_json::json!({
                "slug": slug, "title": node.title, "type": node.ty.as_str(),
                "attributes": node.attributes.iter().map(|a| serde_json::json!({
                    "name": a.name, "type": a.ty.name, "ref": a.ty.ref_, "multiplicity": a.multiplicity.as_str()
                })).collect::<Vec<_>>(),
                "values": node.values,
                "referrers": refs,
            });
            println!("{}", serde_json::to_string_pretty(&dto).unwrap_or_else(|_| "{}".into()));
            0
        }
    }
}

fn run_refs(slug: &str, q: &QueryArgs) -> i32 {
    let bundle = match io::read_files(&[q.dir.clone()]) {
        Ok(b) => b,
        Err(e) => { eprintln!("uaml: {e}"); return 2; }
    };
    let refs = uaml::ops::referrers(&bundle, slug);
    match q.format {
        Format::Human => {
            if refs.is_empty() { println!("no referrers of '{slug}'"); }
            else { for r in refs { println!("{r}"); } }
        }
        Format::Json => println!("{}", serde_json::to_string(&refs).unwrap_or_else(|_| "[]".into())),
    }
    0
}

fn run_list(ty: &Option<String>, q: &QueryArgs) -> i32 {
    let bundle = match io::read_files(&[q.dir.clone()]) {
        Ok(b) => b,
        Err(e) => { eprintln!("uaml: {e}"); return 2; }
    };
    let model = uaml::parse::build_model(&bundle);
    for n in &model.nodes {
        if ty.as_deref().map(|t| t == n.ty.as_str()).unwrap_or(true) {
            match q.format {
                Format::Human => println!("{}\t{}\t{}", n.key, n.ty.as_str(), n.title),
                Format::Json => println!("{}", serde_json::json!({"slug": n.key, "type": n.ty.as_str(), "title": n.title})),
            }
        }
    }
    0
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p uaml-cli`
Expected: PASS.

- [ ] **Step 6: Clippy + commit**

Run: `cargo clippy -p uaml-cli --all-targets`
Expected: clean.
```bash
git add crates/uaml/src/ops/mod.rs crates/uaml-cli/src/io.rs crates/uaml-cli/src/main.rs
git commit -m "feat(uaml-cli): apply (NDJSON batch) + show/refs/list queries"
```

---

### Task 11: End-to-end golden + command↔op parity + full gate

Integration proof: a rename on the real orders-domain fixture, a wire round-trip parity check over every op variant, and a CLI binary smoke test that writes real files. Then the whole-workspace gate.

**Files:**
- Create: `crates/uaml/tests/ops_golden.rs`
- Create: `crates/uaml-cli/tests/cli_e2e.rs`
- Modify: `crates/uaml-cli/src/ops_dto.rs` (add the parity test to its existing `mod tests`)

**Interfaces:**
- Consumes: `uaml::parse::split_bundle`, `uaml::ops::{apply, Op}`. `slug_of` is `pub(crate)`, so integration tests use a local basename helper. `OpDto` is private to the CLI binary (binary crates expose no importable lib), so the parity test lives **inside** `ops_dto.rs`'s `mod tests`, not a separate integration file.

- [ ] **Step 1: Golden rename on the orders-domain fixture**

Create `crates/uaml/tests/ops_golden.rs`:
```rust
use uaml::ops::{apply, Op};
use uaml::parse::split_bundle;

fn base(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).strip_suffix(".md").unwrap_or(path).to_string()
}

#[test]
fn rename_on_orders_domain_fixture_rewrites_all_referrers() {
    let blob = include_str!("fixtures/orders-domain.md");
    let bundle = split_bundle(blob);
    // Pick a slug the fixture actually defines and references. `order-line` is
    // composed by `order` and appears in the diagram. If the fixture uses a
    // different slug, target that one instead.
    assert!(bundle.iter().any(|(p, _)| base(p) == "order-line"), "fixture must define order-line");

    let out = apply(&bundle, &[Op::NodeRename { from: "order-line".into(), to: "line-item".into() }]).unwrap();

    assert!(out.iter().any(|(p, _)| base(p) == "line-item"), "file re-keyed");
    assert!(out.iter().all(|(p, _)| base(p) != "order-line"), "old key gone");
    for (_, text) in &out {
        assert!(!text.contains("(./order-line.md)"), "no stale link anywhere");
    }
    // a referrer now points at the new slug
    assert!(out.iter().any(|(_, t)| t.contains("(./line-item.md)")), "referrers repointed");
}
```

- [ ] **Step 2: Command↔op parity over every op variant**

The parity check pins the wire round-trip for every variant: a core `Op` → `from_op` → NDJSON line → parse → `to_op` must equal the original. Since the sugar commands and `--emit` both go through `OpDto`, this proves running a command directly and replaying its `--emit` line produce identical `Op`s (hence identical bundles). It lives inside `ops_dto.rs`'s existing `mod tests` (no separate integration file — the binary exposes no importable lib).

Add to `crates/uaml-cli/src/ops_dto.rs` `mod tests`:
```rust
    #[test]
    fn every_op_survives_a_wire_round_trip() {
        use uaml::ops::{NameSpec, Op, RelBy, Selector};
        use uaml::model::{ClassifierType, RelationshipKind};
        use uaml::multiplicity::Multiplicity;
        use uaml::grammar::parse_ends;

        let ops = vec![
            Op::NodeNew { slug: "order".into(), ty: ClassifierType::parse("uml.Class"), title: "Order".into(),
                          stereotype: vec!["entity".into()], description: Some("x".into()), abstract_: true },
            Op::NodeRename { from: "a".into(), to: "b".into() },
            Op::NodeSet { slug: "order".into(), title: Some("O".into()), description: None,
                          stereotype: Some(vec!["e".into()]), abstract_: Some(false), ty: None },
            Op::NodeRm { slug: "x".into(), cascade: true },
            Op::AttrAdd { node: "order".into(), name: "total".into(), ty_token: "Money".into(),
                          multiplicity: Multiplicity::parse("0..1").unwrap(), visibility: Some(uaml::model::Visibility::Private) },
            Op::AttrSet { node: "order".into(), name: "total".into(), ty_token: Some("Cash".into()),
                          multiplicity: Some(Multiplicity::default()), visibility: None, rename: Some("amount".into()) },
            Op::AttrRm { node: "order".into(), name: "total".into() },
            Op::ValueAdd { node: "e".into(), literal: "PLACED".into() },
            Op::ValueRm { node: "e".into(), literal: "DRAFT".into() },
            Op::RelAdd { source: "order".into(), kind: RelationshipKind::Composes, target: "order-line".into(),
                         name: Some(NameSpec::Label("has".into())), ends: parse_ends("1 to 1..* lines") },
            Op::RelSet { selector: Selector::Rel { source: "order".into(), by: RelBy::Endpoint { kind: RelationshipKind::Composes, target: "order-line".into() } },
                         ends: parse_ends("1 to *"), name: None },
            Op::RelRm { selector: Selector::Rel { source: "order".into(), by: RelBy::Named("has".into()) } },
        ];
        for op in &ops {
            let line = serde_json::to_string(&OpDto::from_op(op)).unwrap();
            let back: OpDto = serde_json::from_str(&line).unwrap();
            assert_eq!(&back.to_op().unwrap(), op, "wire round-trip changed op: {line}");
        }
    }
```

- [ ] **Step 3: CLI binary smoke test (real files)**

Create `crates/uaml-cli/tests/cli_e2e.rs`:
```rust
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_uaml"))
}

fn tmp() -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("uaml_e2e_{}_{}", std::process::id(), line!()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn attr_add_writes_the_file() {
    let d = tmp();
    std::fs::write(d.join("order.md"), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n").unwrap();
    let status = bin()
        .args(["attr", "add", "order", "total", "Money", "--dir"])
        .arg(&d)
        .status()
        .unwrap();
    assert!(status.success());
    let text = std::fs::read_to_string(d.join("order.md")).unwrap();
    assert!(text.contains("- total: Money"));
}

#[test]
fn emit_prints_an_op_line_without_writing() {
    let d = tmp();
    std::fs::write(d.join("order.md"), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n").unwrap();
    let out = bin()
        .args(["attr", "add", "order", "total", "Money", "--emit", "--dir"])
        .arg(&d)
        .output()
        .unwrap();
    assert!(out.status.success());
    let line = String::from_utf8(out.stdout).unwrap();
    assert!(line.contains("\"op\":\"attr.add\""));
    // file untouched
    assert!(!std::fs::read_to_string(d.join("order.md")).unwrap().contains("total"));
}

#[test]
fn duplicate_attr_exits_1() {
    let d = tmp();
    std::fs::write(d.join("order.md"), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n").unwrap();
    let status = bin().args(["attr", "add", "order", "id", "X", "--dir"]).arg(&d).status().unwrap();
    assert_eq!(status.code(), Some(1));
}
```

- [ ] **Step 4: Run the new integration tests**

Run: `cargo test -p uaml --test ops_golden`
Run: `cargo test -p uaml-cli`
Expected: PASS. If the golden fails because the fixture does not define `order-line`, retarget the test to a slug the fixture does define (inspect `crates/uaml/tests/fixtures/orders-domain.md`).

- [ ] **Step 5: Full workspace gate**

Run: `cargo test`
Expected: PASS (entire workspace, including the existing `golden.rs`).
Run: `cargo clippy --all-targets`
Expected: no warnings.
Run: `cargo build --release`
Expected: builds `uaml` binary.

- [ ] **Step 6: Commit**

```bash
git add crates/uaml/tests/ops_golden.rs crates/uaml-cli/tests/cli_e2e.rs crates/uaml-cli/src/ops_dto.rs
git commit -m "test(uaml): rename golden, op wire-parity, CLI e2e smoke"
```

---

## Notes for the implementer

- **Enum growth:** `Op` and `apply_one`'s match grow across Tasks 2–7. Each task adds only its variants/arms — the match stays exhaustive at every commit, so every task compiles and passes on its own.
- **Borrow ordering:** appliers that resolve against the bundle (`resolve_type`, `resolve_title`, `build_name`) compute their owned result *before* calling `edit_doc`, which then takes `&mut work`. Keep that order.
- **No serde in core:** if you reach for `#[derive(Serialize)]` in `crates/uaml/`, stop — it belongs in `OpDto` (`crates/uaml-cli/src/ops_dto.rs`).
- **Canonicalization is intentional:** every edited file is re-emitted by `serialize_document`, so an edit also normalizes that file (default `[1]` dropped, etc.). Assertions use `contains`, never byte-equality, to stay robust to trailing-newline conventions.
- **New-node placement:** `node new` writes `<slug>.md` at the bundle root (cwd-relative). Placing it under `--dir` is a later polish, not part of this cut.
- **Cleaner op-log (optional):** the `OpDto` optional/`Vec` fields serialize as `null`/`[]` when empty. Correctness and parity don't need it, but for tidier `--emit` lines and git diffs you may add `#[serde(default, skip_serializing_if = "Option::is_none")]` to the `Option` fields and `skip_serializing_if = "Vec::is_empty"` to `stereotype`. Keep `default` so absent fields still deserialize.
