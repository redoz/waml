# UAML Packages + Model Navigator (Spec 1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the model first-class **packages** (UML namespaces backed by OKF directories + `index.md`) and grow the TopBar diagram switcher into a searchable, type-filterable **navigator sheet** for browsing and reorganizing packages, classifiers, and diagrams.

**Architecture:** Part A (package model + serialization) lands in the Rust core (`crates/uaml`), which owns parse/serialize/ops and is the SOLE source of truth surfaced to the frontend through the inlined WASM build. Part B (navigator sheet) is Svelte + the TS state layer (`packages/core/src/state`, `packages/web`), consuming the enriched model and routing package mutations back through `apply_ops`.

**Tech Stack:** Rust (serde, pulldown-cmark, wasm-bindgen), TypeScript, Svelte 5 (runes), Vitest + @testing-library/svelte, pnpm workspaces.

## Global Constraints

- Package manager is **pnpm**. Green gate for TS: `pnpm build:wasm && pnpm -r test && pnpm lint && pnpm build`. Green gate for Rust: `cargo test` (workspace). Rust changes DO NOT reach TS tests until `pnpm build:wasm` regenerates `packages/okf/src/generated/*` (requires `wasm-pack`).
- Commits: **NEVER** add a `Co-Authored-By: Claude` trailer or any Claude footer. Conventional commits, `feat(uaml):` / `docs(uaml):` style.
- The Bundle (`[path, markdown][]`) is the source of truth; the `Model` is a derived read-model (`build_model`). Every edit is an `OpDto[]` realized via `apply_ops`, then re-derived.
- Keys stay the primary reference (rename-safe). Classifier/diagram keys = filename slug (global, as today). Package keys = relative directory path (POSIX `/`). Root package key = `""`, title = `ModelGraph.path`.
- No word "classifier" in the UI. `index.md` / `log.md` are reserved names; no package or classifier may slug to them.
- Where the spec's prose and the mockup pixels disagree, prose wins.

## Design reconciliations (spec was written against retired code — honor these)

1. The spec names `packages/okf/src/serialize.ts` and `migrate.ts`; those TS bodies are **retired**. All parse/serialize/migrate logic lives in **Rust** (`crates/uaml/src/{parse,serialize,ops}.rs`). Migration = `build_model` naturally wrapping a flat bundle in a single root package; there is no TS `migrateGraph`.
2. Packages are surfaced by `build_model` as a dedicated `Model.packages: Node[]` list (Node gains `members`) plus `Model.path` — NOT mixed into `Model.nodes` — so existing canvas/inspector consumers are untouched.
3. `index.md` is a real, frontmatter-less bundle doc (reserved name), managed by Rust ops and read by `build_model`; excluded from `nodes`/`packages`.
4. **Global** slug uniqueness is retained (references are by filename slug). The spec's "per-directory uniqueness" is deferred — see Risks.
5. Empty packages = frontend-only **ghosts** held in the store (not in the bundle); materialize on first child via an op, prune on last child out.
6. Diagram create/rename/remove remain Stage-1b **no-op stubs**; the navigator reuses the existing `onSelectDiagram`/`onRenameDiagram`/`onCreateDiagram` handlers unchanged. Making diagram edits persist is out of scope.

## Risks / open questions (flagged)

- **R1 — per-directory slug uniqueness:** current core uses global filename-slug keys with global references; true per-directory collision handling needs a `slugByKey` indirection not present in the core. Spec 1 keeps global uniqueness. Revisit if two packages must hold same-named docs.
- **R2 — `Model.path` source:** derived from the root `index.md` H1 title if present, else `""`; the navigator root crumb falls back to `loadModelName()` for display.
- **R3 — packages in `Model.packages` vs `Model.nodes`:** deviates from a strict reading of "packages reuse ModelNode" but is functionally equivalent and lower-risk.

---

## Phase 1 — Rust core: package model in `build_model` (discovery)

### Task 1: Model gains `path` + `packages`; Node gains `members`

**Files:**
- Modify: `crates/uaml/src/model.rs` (`Node`, `Model`)
- Test: `crates/uaml/tests/serde_shape.rs`

**Interfaces:**
- Produces: `Node.members: Vec<String>` (serde `members`, `skip_serializing_if = "Vec::is_empty"`, `default`); `Model.path: String` (serde `path`, `default`); `Model.packages: Vec<Node>` (serde `packages`, `skip_serializing_if = "Vec::is_empty"`, `default`). Package nodes carry `ty = ClassifierType::Uml(UmlMetaclass::Package)`.

- [ ] **Step 1: Write the failing test**

Add to `crates/uaml/tests/serde_shape.rs`:

```rust
#[test]
fn package_node_and_model_path_serialize_to_expected_json() {
    use uaml::model::{ClassifierType, Model, Node, UmlMetaclass};
    let pkg = Node {
        key: "sales".into(),
        ty: ClassifierType::Uml(UmlMetaclass::Package),
        title: "sales".into(),
        stereotypes: vec![],
        abstract_: false,
        description: Some("Sales bounded context.".into()),
        attributes: vec![],
        values: vec![],
        body: None,
        annotates: vec![],
        members: vec!["order".into(), "customer".into()],
    };
    let model = Model { nodes: vec![], edges: vec![], diagrams: vec![], path: "acme-model".into(), packages: vec![pkg] };
    let json = serde_json::to_string(&model).unwrap();
    assert!(json.contains("\"path\":\"acme-model\""));
    assert!(json.contains("\"members\":[\"order\",\"customer\"]"));
    // A classifier with no members must omit the field entirely.
    let bare = Node { key: "order".into(), ty: ClassifierType::Uml(UmlMetaclass::Class), title: "Order".into(),
        stereotypes: vec![], abstract_: false, description: None, attributes: vec![], values: vec![], body: None,
        annotates: vec![], members: vec![] };
    let bj = serde_json::to_string(&bare).unwrap();
    assert!(!bj.contains("members"), "empty members must be omitted: {bj}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml --test serde_shape package_node_and_model_path`
Expected: FAIL — `Node` has no field `members`; `Model` has no `path`/`packages`.

- [ ] **Step 3: Write minimal implementation**

In `crates/uaml/src/model.rs`, add to `struct Node` (after `annotates`):

```rust
    /// Owned member keys (classifiers, diagrams, sub-packages), in progressive-
    /// disclosure order. Meaningful only on `uml.Package` nodes; empty elsewhere.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub members: Vec<String>,
```

Replace `struct Model` with:

```rust
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Model {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub diagrams: Vec<Diagram>,
    /// Bundle/root name (root `index.md` H1); "" when absent. Export label + root crumb.
    #[cfg_attr(feature = "serde", serde(default))]
    pub path: String,
    /// Discovered `uml.Package` nodes (root + nested). Kept out of `nodes` so
    /// classifier consumers are unaffected.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub packages: Vec<Node>,
}
```

Make `UmlMetaclass` `pub` if not already (`pub enum UmlMetaclass`). Fix every `Node { .. }` literal in `model.rs`/`parse.rs`/`ops` tests to add `members: vec![]` (compiler lists them).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p uaml`
Expected: PASS (all existing tests still green after adding `members: vec![]`).

- [ ] **Step 5: Commit**

```bash
git add crates/uaml/src/model.rs crates/uaml/tests/serde_shape.rs
git commit -m "feat(uaml): add Model.path/packages and Node.members to core model"
```

### Task 2: `build_model` discovers packages from directory structure

**Files:**
- Modify: `crates/uaml/src/parse.rs` (`build_model`)
- Test: `crates/uaml/src/parse.rs` (inline `#[cfg(test)]`)

**Interfaces:**
- Consumes: Task 1 `Model.{path,packages}`, `Node.members`.
- Produces: `build_model(bundle)` now populates `Model.packages` (one `uml.Package` Node per directory, root key `""`), `Node.members` (directory children: classifier/diagram keys + sub-package keys, `index.md`/`log.md` excluded), and `Model.path`. Default member order = A–Z (title). Directory-derived package key = the doc's dir path with forward slashes; child classifier/diagram keys = filename slug (unchanged).

- [ ] **Step 1: Write the failing test**

Add to `parse.rs` tests:

```rust
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
    assert!(sales.members.contains(&"order".to_string()));
    assert!(sales.members.contains(&"sales/orders".to_string()));
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml --lib build_model_discovers build_model_flat`
Expected: FAIL — `m.packages` empty.

- [ ] **Step 3: Write minimal implementation**

In `parse.rs`, add a helper and call it at the end of `build_model` (which already builds `nodes`, `edges`, `diagrams`). It needs each doc's full path + resolved title. Compute a `(dir, key, title)` list from the parsed docs (reuse the existing `ParsedDoc { slug, ty, doc }` plus path; thread the path through if not already available), then:

```rust
/// Directory of a bundle path ("" for root). Forward-slash normalized.
fn dir_of(path: &str) -> String {
    let p = path.replace('\\', "/");
    match p.rfind('/') { Some(i) => p[..i].to_string(), None => String::new() }
}

/// Build the package forest from the bundle's directory structure.
/// `docs` = (full_path, key, title) for every NON-index concept/diagram doc.
fn build_packages(docs: &[(String, String, String)]) -> (String, Vec<Node>) {
    use std::collections::{BTreeMap, BTreeSet};
    // Every directory that contains a doc, plus all ancestor dirs, is a package.
    let mut dirs: BTreeSet<String> = BTreeSet::new();
    dirs.insert(String::new());
    for (path, _, _) in docs {
        let mut d = dir_of(path);
        loop { dirs.insert(d.clone()); if d.is_empty() { break; } d = dir_of(&d); }
    }
    // members: (title, key) per dir so we can sort A–Z by title/segment name.
    let mut members: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for d in &dirs { members.entry(d.clone()).or_default(); }
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
    let packages = dirs.iter().map(|d| {
        let mut ms = members.get(d).cloned().unwrap_or_default();
        ms.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()).then(a.1.cmp(&b.1)));
        let title = if d.is_empty() { String::new() } else { d.rsplit('/').next().unwrap_or(d).to_string() };
        Node {
            key: d.clone(),
            ty: ClassifierType::Uml(crate::model::UmlMetaclass::Package),
            title,
            stereotypes: vec![], abstract_: false, description: None,
            attributes: vec![], values: vec![], body: None, annotates: vec![],
            members: ms.into_iter().map(|(_, k)| k).collect(),
        }
    }).collect();
    (String::new(), packages) // path filled in Task 3 from root index.md
}
```

Wire it: after diagrams are built in `build_model`, collect `docs` = every parsed doc whose slug is not `index`/`log`, as `(path, slug, resolved_title)`, call `build_packages`, and set `model.packages`. Set `model.path` = `String::new()` for now (Task 3 fills it). Reference `UmlMetaclass` via `crate::model::UmlMetaclass`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p uaml`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/uaml/src/parse.rs
git commit -m "feat(uaml): discover package forest from directory structure in build_model"
```

### Task 3: `build_model` reads & reconciles `index.md` (order, blurbs, intro → `path`/description)

**Files:**
- Modify: `crates/uaml/src/parse.rs`
- Test: `crates/uaml/src/parse.rs` (inline)

**Interfaces:**
- Consumes: Task 2 `build_packages`.
- Produces: for each package with a `<dir>/index.md`, member order follows the `index.md` listing (survivors keep listed order + blurbs; docs present on disk but absent from `index.md` are appended A–Z; listed-but-absent entries are dropped — never an error). The package `description` = the index intro prose (text above the first list item). `Model.path` = the ROOT `index.md` H1 title (else `""`). `index.md` docs are excluded from `nodes`.

- [ ] **Step 1: Write the failing test**

```rust
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
    assert_eq!(sales.description.as_deref(), Some("Sales bounded context."));
    // listed order first (customer, order), then unlisted appended (invoice)
    assert_eq!(sales.members, vec!["customer".to_string(), "order".to_string(), "invoice".to_string()]);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml --lib build_model_honors_index_md`
Expected: FAIL — order is A–Z, `description` None, `path` empty, and `index` leaks into nodes.

- [ ] **Step 3: Write minimal implementation**

Exclude `index`/`log` slugs when building `nodes` (filter in the node-collection loop). Add an index parser and a reconcile step:

```rust
struct IndexDoc { intro: Option<String>, order: Vec<String>, blurbs: std::collections::HashMap<String, String>, h1: String }

/// Parse a frontmatter-less index.md: H1, intro prose (before first bullet),
/// and `* [Title](url) - blurb` entries. `url` maps to a key: `sub/` -> the
/// dir-relative sub-package key; `./slug.md` -> slug.
fn parse_index(dir: &str, text: &str) -> IndexDoc {
    let mut h1 = String::new();
    let mut intro_lines: Vec<&str> = vec![];
    let mut order = vec![]; let mut blurbs = std::collections::HashMap::new();
    let re = regex::Regex::new(r"^\s*[*-]\s*\[[^\]]*\]\(([^)]+)\)(?:\s*-\s*(.*))?$").unwrap();
    let mut seen_bullet = false;
    for line in text.lines() {
        if let Some(c) = re.captures(line) {
            seen_bullet = true;
            let url = c.get(1).unwrap().as_str();
            let key = if let Some(sub) = url.strip_suffix('/') {
                let seg = sub.trim_start_matches("./").trim_end_matches('/');
                if dir.is_empty() { seg.to_string() } else { format!("{dir}/{seg}") }
            } else {
                url.rsplit('/').next().unwrap_or(url).strip_suffix(".md").unwrap_or(url).to_string()
            };
            if let Some(b) = c.get(2) { blurbs.insert(key.clone(), b.as_str().trim().to_string()); }
            order.push(key);
        } else if !seen_bullet {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("# ") { h1 = rest.trim().to_string(); }
            else if !t.is_empty() { intro_lines.push(t); }
        }
    }
    IndexDoc { intro: (!intro_lines.is_empty()).then(|| intro_lines.join(" ")), order, blurbs, h1 }
}
```

In `build_packages` (or a post-pass), for each package dir with an `index.md` in the bundle: reconcile `members` = listed keys that still exist (in listed order), then remaining discovered members appended A–Z; set `description` = `intro`. Set `model.path` from the ROOT `index.md`'s `h1`. Pass the raw `index.md` texts (keyed by dir) into `build_packages`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p uaml`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/uaml/src/parse.rs
git commit -m "feat(uaml): reconcile package members and intro from index.md on load"
```

## Phase 2 — Rust core: `index.md` serialization + round-trip

### Task 4: Generate `index.md` for a package directory + `reindex_bundle` pass + wasm entry

**Files:**
- Create: `crates/uaml/src/index_md.rs`
- Modify: `crates/uaml/src/lib.rs` (add `pub mod index_md;`), `crates/uaml-wasm/src/lib.rs` (add `reindex` entry), `packages/okf/src/wasm/index.ts` + `packages/okf/src/index.ts` (export `reindex`)
- Test: `crates/uaml/src/index_md.rs` (inline)

**Interfaces:**
- Produces: `index_md::render_index(dir: &str, description: Option<&str>, members: &[IndexEntry]) -> String` where `IndexEntry { key, title, blurb, is_package }`; `index_md::reindex_bundle(bundle: &[(String,String)]) -> Vec<(String,String)>` — rebuilds every `<dir>/index.md` from the current directory contents + member order (reading any existing `index.md` for order/blurbs), leaving concept docs untouched. Wasm `reindex(bundle) -> bundle`; TS `reindex(bundle: [string,string][]): [string,string][]`.

- [ ] **Step 1: Write the failing test**

```rust
// crates/uaml/src/index_md.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_index_emits_intro_and_listing() {
        let members = vec![
            IndexEntry { key: "sales/orders".into(), title: "orders".into(), blurb: None, is_package: true },
            IndexEntry { key: "customer".into(), title: "Customer".into(), blurb: Some("a buyer".into()), is_package: false },
        ];
        let out = render_index("sales", Some("Sales bounded context."), &members);
        assert!(out.starts_with("# sales\n"));
        assert!(out.contains("Sales bounded context."));
        assert!(out.contains("* [orders](orders/)"));
        assert!(out.contains("* [Customer](./customer.md) - a buyer"));
        assert!(!out.contains("---")); // frontmatter-less
    }

    #[test]
    fn reindex_bundle_creates_index_for_each_directory() {
        let b = vec![
            ("sales/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
            ("sales/orders/line.md".to_string(), "---\ntype: uml.Class\ntitle: Line\n---\n# Line\n".to_string()),
        ];
        let out = reindex_bundle(&b);
        assert!(out.iter().any(|(p, _)| p == "index.md"));
        assert!(out.iter().any(|(p, _)| p == "sales/index.md"));
        assert!(out.iter().any(|(p, _)| p == "sales/orders/index.md"));
        // concept docs untouched
        assert_eq!(out.iter().find(|(p, _)| p == "sales/order.md").unwrap().1,
                   b.iter().find(|(p, _)| p == "sales/order.md").unwrap().1);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml --lib index_md`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Write minimal implementation**

Create `crates/uaml/src/index_md.rs`:

```rust
use crate::parse::build_model;

pub struct IndexEntry { pub key: String, pub title: String, pub blurb: Option<String>, pub is_package: bool }

/// Relative URL for a member, from its containing dir. Sub-packages -> `seg/`,
/// concept docs -> `./slug.md`.
fn member_url(dir: &str, e: &IndexEntry) -> String {
    if e.is_package {
        let seg = e.key.rsplit('/').next().unwrap_or(&e.key);
        format!("{seg}/")
    } else {
        format!("./{}.md", e.key)
    }
}

pub fn render_index(dir: &str, description: Option<&str>, members: &[IndexEntry]) -> String {
    let title = if dir.is_empty() { "index" } else { dir.rsplit('/').next().unwrap_or(dir) };
    let mut out = format!("# {title}\n");
    if let Some(d) = description.filter(|d| !d.trim().is_empty()) {
        out.push('\n'); out.push_str(d.trim()); out.push('\n');
    }
    if !members.is_empty() { out.push('\n'); }
    for e in members {
        let url = member_url(dir, e);
        match &e.blurb {
            Some(b) if !b.trim().is_empty() => out.push_str(&format!("* [{}]({url}) - {}\n", e.title, b.trim())),
            _ => out.push_str(&format!("* [{}]({url})\n", e.title)),
        }
    }
    out
}

/// Rebuild every directory's index.md from the current model's package forest.
pub fn reindex_bundle(bundle: &[(String, String)]) -> Vec<(String, String)> {
    let model = build_model(bundle);
    // key -> (title, is_package, blurb-from-description-first-line)
    let mut meta = std::collections::HashMap::new();
    for n in &model.nodes { meta.insert(n.key.clone(), (n.title.clone(), false, n.description.clone())); }
    for d in &model.diagrams { meta.insert(d.key.clone(), (d.title.clone(), false, None)); }
    for p in &model.packages { meta.insert(p.key.clone(), (p.title.clone(), true, None)); }
    // start from concept/diagram docs (drop existing index.md), then append fresh indexes
    let mut out: Vec<(String, String)> = bundle.iter()
        .filter(|(p, _)| !p.rsplit(['/', '\\']).next().unwrap_or(p).eq_ignore_ascii_case("index.md"))
        .cloned().collect();
    for pkg in &model.packages {
        let entries: Vec<IndexEntry> = pkg.members.iter().filter_map(|k| {
            meta.get(k).map(|(title, is_pkg, desc)| IndexEntry {
                key: k.clone(), title: title.clone(), is_package: *is_pkg,
                blurb: desc.as_ref().map(|d| d.lines().next().unwrap_or("").to_string()),
            })
        }).collect();
        let path = if pkg.key.is_empty() { "index.md".to_string() } else { format!("{}/index.md", pkg.key) };
        out.push((path, render_index(&pkg.key, pkg.description.as_deref(), &entries)));
    }
    out
}
```

Add `pub mod index_md;` to `crates/uaml/src/lib.rs`. In `crates/uaml-wasm/src/lib.rs` add a pure core + wasm shell mirroring `fmt`:

```rust
pub fn reindex_bundle_core(bundle: &[(String, String)]) -> Vec<(String, String)> {
    uaml::index_md::reindex_bundle(bundle)
}

#[wasm_bindgen]
pub fn reindex(bundle: JsValue) -> Result<JsValue, JsValue> {
    let b: Vec<(String, String)> = serde_wasm_bindgen::from_value(bundle)?;
    Ok(serde_wasm_bindgen::to_value(&uaml::index_md::reindex_bundle(&b))?)
}
```

In `packages/okf/src/wasm/index.ts` add `reindex` to the import list and re-export; in `packages/okf/src/index.ts` add `reindex` to the WASM re-export line.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p uaml` then `pnpm build:wasm`
Expected: PASS; wasm rebuilds with `reindex` exported.

- [ ] **Step 5: Commit**

```bash
git add crates/uaml/src/index_md.rs crates/uaml/src/lib.rs crates/uaml-wasm/src/lib.rs packages/okf/src/wasm/index.ts packages/okf/src/index.ts packages/okf/src/generated
git commit -m "feat(uaml): generate index.md per package directory (reindex pass + wasm entry)"
```

### Task 5: Round-trip test — model → bundle → model with nested packages

**Files:**
- Test: `crates/uaml/tests/golden.rs` (new test) or `crates/uaml/src/index_md.rs` (inline)

**Interfaces:**
- Consumes: Tasks 3 + 4 (`build_model`, `reindex_bundle`).

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn nested_packages_round_trip_through_reindex() {
    use uaml::index_md::reindex_bundle;
    use uaml::parse::build_model;
    let b = vec![
        ("sales/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
        ("sales/customer.md".to_string(), "---\ntype: uml.Class\ntitle: Customer\ndescription: A buyer.\n---\n# Customer\n".to_string()),
        ("sales/orders/line.md".to_string(), "---\ntype: uml.Class\ntitle: Line\n---\n# Line\n".to_string()),
    ];
    let m1 = build_model(&b);
    let bundle2 = reindex_bundle(&b);
    let m2 = build_model(&bundle2);
    // packages + members stable across the round-trip
    let names = |m: &uaml::model::Model| {
        let mut v: Vec<_> = m.packages.iter().map(|p| (p.key.clone(), p.members.clone())).collect();
        v.sort(); v
    };
    assert_eq!(names(&m1), names(&m2));
    // blurb from description survived into sales/index.md
    let idx = bundle2.iter().find(|(p, _)| p == "sales/index.md").unwrap();
    assert!(idx.1.contains("[Customer](./customer.md) - A buyer."));
    // second reindex is a fixpoint
    let bundle3 = reindex_bundle(&bundle2);
    assert_eq!(bundle2.iter().find(|(p,_)| p=="sales/index.md").unwrap().1,
               bundle3.iter().find(|(p,_)| p=="sales/index.md").unwrap().1);
}
```

- [ ] **Step 2: Run test to verify it fails (or passes immediately)**

Run: `cargo test -p uaml nested_packages_round_trip`
Expected: PASS if Tasks 3–4 correct; if not, the assertion pinpoints the reconcile/format gap — fix in `parse.rs`/`index_md.rs`.

- [ ] **Step 3: Fix any gap surfaced**

Adjust `reconcile`/`render_index` (e.g. trailing-newline normalization, blurb first-line) until green. No new code shape.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p uaml`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/uaml/tests/golden.rs crates/uaml/src/index_md.rs
git commit -m "test(uaml): round-trip nested packages through reindex is a fixpoint"
```

## Phase 3 — Rust core: package mutation ops

All package ops mutate the Bundle (paths) and let `build_model`/`reindex` re-derive. Keys are filename slugs (stable across dir moves); references need no rewrite when only a directory prefix changes. New ops live in `crates/uaml/src/ops/pkg.rs` (mirroring `rename.rs`).

### Task 6: `node.new` gains a target directory

**Files:**
- Modify: `crates/uaml/src/ops/mod.rs` (`Op::NodeNew`, `op_node_new`), `crates/uaml-ops-dto/src/lib.rs` (`OpDto::NodeNew`)
- Test: `crates/uaml/src/ops/mod.rs` (inline)

**Interfaces:**
- Produces: `Op::NodeNew` gains `dir: String` (`""` = root). File written at `<dir>/<slug>.md`. DTO gains `#[serde(default)] dir: String`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn node_new_writes_into_target_directory() {
    let out = apply(&vec![], &[Op::NodeNew {
        slug: "order".into(), dir: "sales".into(), ty: ClassifierType::parse("uml.Class"),
        title: "Order".into(), stereotype: vec![], description: None, abstract_: false,
    }]).unwrap();
    assert_eq!(out[0].0, "sales/order.md");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml --lib node_new_writes_into_target`
Expected: FAIL — `NodeNew` has no `dir`.

- [ ] **Step 3: Write minimal implementation**

Add `dir: String` to `Op::NodeNew`. In `op_node_new`, change the push target:

```rust
    let path = if dir.is_empty() { format!("{slug}.md") } else { format!("{dir}/{slug}.md") };
    work.push((path, serialize_document(&doc)));
```

Add `#[serde(default)] dir: String` to `OpDto::NodeNew` and pass it through `to_op`/`from_op`. Update the existing `node_new_writes_frontmatter...` test literal + every `Op::NodeNew {..}` construction to add `dir: String::new()` (or `dir: "".into()`).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p uaml -p uaml-ops-dto`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/uaml/src/ops/mod.rs crates/uaml-ops-dto/src/lib.rs
git commit -m "feat(uaml): node.new accepts a target package directory"
```

### Task 7: `pkg.move` — move a node into another package directory

**Files:**
- Create: `crates/uaml/src/ops/pkg.rs`
- Modify: `crates/uaml/src/ops/mod.rs` (`Op::PkgMove`, dispatch, `pub mod pkg;`)
- Test: `crates/uaml/src/ops/pkg.rs` (inline)

**Interfaces:**
- Produces: `Op::PkgMove { slug: String, to_dir: String }` — changes a concept/diagram doc's directory prefix, keeping its basename (key). References (by slug) are unaffected. Errors if the doc is missing or a same-key doc already lives in `to_dir`.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use crate::ops::{apply, Op};
    #[test]
    fn move_changes_directory_keeps_basename() {
        let b = vec![("sales/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string())];
        let out = apply(&b, &[Op::PkgMove { slug: "order".into(), to_dir: "billing".into() }]).unwrap();
        assert!(out.iter().any(|(p, _)| p == "billing/order.md"));
        assert!(out.iter().all(|(p, _)| p != "sales/order.md"));
    }
    #[test]
    fn move_to_root_uses_bare_filename() {
        let b = vec![("sales/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string())];
        let out = apply(&b, &[Op::PkgMove { slug: "order".into(), to_dir: "".into() }]).unwrap();
        assert!(out.iter().any(|(p, _)| p == "order.md"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml --lib move_changes_directory`
Expected: FAIL — `Op::PkgMove` undefined.

- [ ] **Step 3: Write minimal implementation**

Create `crates/uaml/src/ops/pkg.rs`:

```rust
use super::{find_doc, slug_of, Bundle, OpError};

fn join(dir: &str, slug: &str) -> String {
    if dir.is_empty() { format!("{slug}.md") } else { format!("{dir}/{slug}.md") }
}

pub(crate) fn op_pkg_move(work: &mut Bundle, slug: &str, to_dir: &str) -> Result<(), OpError> {
    let idx = find_doc(work, slug, "pkg.move")?;
    let dest = join(to_dir, slug);
    if work.iter().enumerate().any(|(i, (p, _))| i != idx && *p == dest) {
        return Err(OpError::at("pkg.move", format!("'{dest}' already exists")));
    }
    work[idx].0 = dest;
    Ok(())
}
```

In `ops/mod.rs`: add `Op::PkgMove { slug: String, to_dir: String }`, dispatch it to `pkg::op_pkg_move`, and add `pub mod pkg;` (already has `pub mod rename;`). `slug_of` import in pkg.rs is used by later tasks.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p uaml`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/uaml/src/ops/pkg.rs crates/uaml/src/ops/mod.rs
git commit -m "feat(uaml): pkg.move relocates a doc between package directories"
```

### Task 8: `pkg.rename` — rename a package directory

**Files:**
- Modify: `crates/uaml/src/ops/pkg.rs`, `crates/uaml/src/ops/mod.rs` (`Op::PkgRename`, dispatch)
- Test: `crates/uaml/src/ops/pkg.rs` (inline)

**Interfaces:**
- Produces: `Op::PkgRename { from: String, to: String }` — `from`/`to` are package keys (relative dir paths). Rewrites the directory-path prefix of every doc under `from/` to `to/`. Slugs (keys) and references are unchanged. Errors if `to` already exists as a directory prefix.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn rename_package_rewrites_child_paths_only() {
    let b = vec![
        ("sales/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- depends [Customer](./customer.md)\n".to_string()),
        ("sales/customer.md".to_string(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string()),
    ];
    let out = apply(&b, &[Op::PkgRename { from: "sales".into(), to: "commerce".into() }]).unwrap();
    assert!(out.iter().any(|(p, _)| p == "commerce/order.md"));
    assert!(out.iter().any(|(p, _)| p == "commerce/customer.md"));
    assert!(out.iter().all(|(p, _)| !p.starts_with("sales/")));
    // slug-based references untouched
    let order = &out.iter().find(|(p, _)| p == "commerce/order.md").unwrap().1;
    assert!(order.contains("(./customer.md)"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml --lib rename_package_rewrites`
Expected: FAIL — `Op::PkgRename` undefined.

- [ ] **Step 3: Write minimal implementation**

Add to `pkg.rs`:

```rust
pub(crate) fn op_pkg_rename(work: &mut Bundle, from: &str, to: &str) -> Result<(), OpError> {
    if from.is_empty() { return Err(OpError::at("pkg.rename", "cannot rename the root package")); }
    let from_pfx = format!("{from}/");
    let to_pfx = format!("{to}/");
    if work.iter().any(|(p, _)| p.replace('\\', "/").starts_with(&to_pfx)) {
        return Err(OpError::at("pkg.rename", format!("directory '{to}' already exists")));
    }
    let mut hit = false;
    for (p, _) in work.iter_mut() {
        let norm = p.replace('\\', "/");
        if let Some(rest) = norm.strip_prefix(&from_pfx) {
            *p = format!("{to_pfx}{rest}");
            hit = true;
        }
    }
    if !hit { return Err(OpError::at("pkg.rename", format!("no package '{from}'"))); }
    Ok(())
}
```

Add `Op::PkgRename { from: String, to: String }` + dispatch in `ops/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p uaml`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/uaml/src/ops/pkg.rs crates/uaml/src/ops/mod.rs
git commit -m "feat(uaml): pkg.rename repaths a package directory (keys unchanged)"
```

### Task 9: `pkg.delete` — cascade or reparent

**Files:**
- Modify: `crates/uaml/src/ops/pkg.rs`, `crates/uaml/src/ops/mod.rs` (`Op::PkgDelete`, dispatch)
- Test: `crates/uaml/src/ops/pkg.rs` (inline)

**Interfaces:**
- Produces: `Op::PkgDelete { path: String, cascade: bool }`. `cascade=true` removes every doc under `path/` (incl. its `index.md`). `cascade=false` = move-to-parent: every DIRECT-and-nested child doc has its `path/` prefix stripped to the parent, then the (now empty) directory disappears naturally. Root cannot be deleted.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn delete_package_cascade_removes_subtree() {
    let b = vec![
        ("sales/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
        ("sales/orders/line.md".to_string(), "---\ntype: uml.Class\ntitle: Line\n---\n# Line\n".to_string()),
        ("billing/invoice.md".to_string(), "---\ntype: uml.Class\ntitle: Invoice\n---\n# Invoice\n".to_string()),
    ];
    let out = apply(&b, &[Op::PkgDelete { path: "sales".into(), cascade: true }]).unwrap();
    assert!(out.iter().all(|(p, _)| !p.starts_with("sales")));
    assert!(out.iter().any(|(p, _)| p == "billing/invoice.md"));
}
#[test]
fn delete_package_reparent_moves_children_up() {
    let b = vec![
        ("sales/orders/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
    ];
    let out = apply(&b, &[Op::PkgDelete { path: "sales/orders".into(), cascade: false }]).unwrap();
    assert!(out.iter().any(|(p, _)| p == "sales/order.md"));
    assert!(out.iter().all(|(p, _)| !p.contains("orders")));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml --lib delete_package`
Expected: FAIL — `Op::PkgDelete` undefined.

- [ ] **Step 3: Write minimal implementation**

Add to `pkg.rs`:

```rust
fn parent_of(dir: &str) -> String {
    match dir.rfind('/') { Some(i) => dir[..i].to_string(), None => String::new() }
}

pub(crate) fn op_pkg_delete(work: &mut Bundle, path: &str, cascade: bool) -> Result<(), OpError> {
    if path.is_empty() { return Err(OpError::at("pkg.delete", "cannot delete the root package")); }
    let pfx = format!("{path}/");
    if cascade {
        let before = work.len();
        work.retain(|(p, _)| !p.replace('\\', "/").starts_with(&pfx));
        if work.len() == before { return Err(OpError::at("pkg.delete", format!("no package '{path}'"))); }
    } else {
        let parent = parent_of(path);
        let parent_pfx = if parent.is_empty() { String::new() } else { format!("{parent}/") };
        for (p, _) in work.iter_mut() {
            let norm = p.replace('\\', "/");
            if let Some(rest) = norm.strip_prefix(&pfx) {
                // strip only the deleted segment, keep any deeper nesting
                *p = format!("{parent_pfx}{rest}");
            }
        }
    }
    Ok(())
}
```

Add `Op::PkgDelete { path: String, cascade: bool }` + dispatch.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p uaml`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/uaml/src/ops/pkg.rs crates/uaml/src/ops/mod.rs
git commit -m "feat(uaml): pkg.delete supports cascade and move-to-parent"
```

### Task 10: `pkg.reorder` + `pkg.sort` — persist member order to `index.md`

**Files:**
- Modify: `crates/uaml/src/ops/pkg.rs`, `crates/uaml/src/ops/mod.rs`
- Test: `crates/uaml/src/ops/pkg.rs` (inline)

**Interfaces:**
- Produces: `Op::PkgReorder { path: String, order: Vec<String> }` and `Op::PkgSort { path: String }`. Both write/replace `<path>/index.md` with a listing in the requested (or A–Z) order, preserving intro prose + blurbs from the existing index (via `index_md::render_index` + a metadata lookup from `build_model`). `order` keys not currently in the package are ignored; missing ones are appended in existing order.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn reorder_writes_index_md_in_requested_order() {
    let b = vec![
        ("sales/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
        ("sales/customer.md".to_string(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string()),
    ];
    let out = apply(&b, &[Op::PkgReorder { path: "sales".into(), order: vec!["order".into(), "customer".into()] }]).unwrap();
    let idx = &out.iter().find(|(p, _)| p == "sales/index.md").unwrap().1;
    let oi = idx.find("order.md").unwrap();
    let ci = idx.find("customer.md").unwrap();
    assert!(oi < ci, "order must precede customer in index.md");
}
#[test]
fn sort_writes_index_md_alphabetically() {
    let b = vec![
        ("sales/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
        ("sales/customer.md".to_string(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string()),
    ];
    let out = apply(&b, &[Op::PkgSort { path: "sales".into() }]).unwrap();
    let idx = &out.iter().find(|(p, _)| p == "sales/index.md").unwrap().1;
    assert!(idx.find("customer.md").unwrap() < idx.find("order.md").unwrap());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml --lib reorder_writes sort_writes`
Expected: FAIL — ops undefined.

- [ ] **Step 3: Write minimal implementation**

Add to `pkg.rs` (reuse `index_md`):

```rust
use crate::index_md::{render_index, IndexEntry};
use crate::parse::build_model;

fn write_package_index(work: &mut Bundle, path: &str, order: Option<&[String]>) -> Result<(), OpError> {
    let model = build_model(work);
    let pkg = model.packages.iter().find(|p| p.key == path)
        .ok_or_else(|| OpError::at("pkg.order", format!("no package '{path}'")))?;
    // desired order
    let mut keys: Vec<String> = match order {
        Some(o) => {
            let mut v: Vec<String> = o.iter().filter(|k| pkg.members.contains(k)).cloned().collect();
            for m in &pkg.members { if !v.contains(m) { v.push(m.clone()); } }
            v
        }
        None => {
            let mut v = pkg.members.clone();
            let title_of = |k: &str| model.nodes.iter().find(|n| n.key == k).map(|n| n.title.clone())
                .or_else(|| model.diagrams.iter().find(|d| d.key == k).map(|d| d.title.clone()))
                .or_else(|| model.packages.iter().find(|p| &p.key == k).map(|p| p.title.clone()))
                .unwrap_or_else(|| k.to_string());
            v.sort_by_key(|k| title_of(k).to_lowercase());
            v
        }
    };
    let entries: Vec<IndexEntry> = keys.drain(..).map(|k| {
        let (title, is_pkg, blurb) = model.nodes.iter().find(|n| n.key == k)
            .map(|n| (n.title.clone(), false, n.description.as_ref().map(|d| d.lines().next().unwrap_or("").to_string())))
            .or_else(|| model.diagrams.iter().find(|d| d.key == k).map(|d| (d.title.clone(), false, None)))
            .or_else(|| model.packages.iter().find(|p| p.key == k).map(|p| (p.title.clone(), true, None)))
            .unwrap_or((k.clone(), false, None));
        IndexEntry { key: k, title, is_package: is_pkg, blurb }
    }).collect();
    let text = render_index(path, pkg.description.as_deref(), &entries);
    let idx_path = format!("{path}/index.md");
    match work.iter_mut().find(|(p, _)| *p == idx_path) {
        Some(slot) => slot.1 = text,
        None => work.push((idx_path, text)),
    }
    Ok(())
}

pub(crate) fn op_pkg_reorder(work: &mut Bundle, path: &str, order: &[String]) -> Result<(), OpError> {
    write_package_index(work, path, Some(order))
}
pub(crate) fn op_pkg_sort(work: &mut Bundle, path: &str) -> Result<(), OpError> {
    write_package_index(work, path, None)
}
```

Add `Op::PkgReorder`/`Op::PkgSort` + dispatch.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p uaml`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/uaml/src/ops/pkg.rs crates/uaml/src/ops/mod.rs
git commit -m "feat(uaml): pkg.reorder and pkg.sort persist member order to index.md"
```

### Task 11: Wire package ops through OpDto + wasm; native round-trip

**Files:**
- Modify: `crates/uaml-ops-dto/src/lib.rs` (DTO variants + `to_op`/`from_op` + round-trip test)
- Test: `crates/uaml-ops-dto/src/lib.rs` (`every_op_survives_a_wire_round_trip`)

**Interfaces:**
- Produces wire tags: `pkg.move { slug, to_dir }`, `pkg.rename { from, to }`, `pkg.delete { path, cascade }`, `pkg.reorder { path, order[] }`, `pkg.sort { path }`. Each carries `#[serde(default="one")] v: u32`.

- [ ] **Step 1: Write the failing test**

Extend the `ops` vec in `every_op_survives_a_wire_round_trip` with one of each new `Op` variant (e.g. `Op::PkgMove { slug: "order".into(), to_dir: "billing".into() }`, `Op::PkgDelete { path: "sales".into(), cascade: false }`, `Op::PkgReorder { path: "sales".into(), order: vec!["a".into()] }`, `Op::PkgSort { path: "sales".into() }`, `Op::PkgRename { from: "a".into(), to: "b".into() }`).

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml-ops-dto every_op_survives`
Expected: FAIL — no `OpDto` arm for the new ops.

- [ ] **Step 3: Write minimal implementation**

Add five DTO variants (mirroring existing `#[serde(rename="...")]` + `#[serde(default="one")] v`) and their `to_op`/`from_op` arms. No wasm-side change needed (the wasm `apply_ops` already round-trips any `OpDto`).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p uaml -p uaml-ops-dto` then `pnpm build:wasm`
Expected: PASS; wasm rebuilt so TS can emit the new ops.

- [ ] **Step 5: Commit**

```bash
git add crates/uaml-ops-dto/src/lib.rs packages/okf/src/generated
git commit -m "feat(uaml): expose package ops (move/rename/delete/reorder/sort) on the op wire"
```

## Phase 4 — TS: types, ops-adapter, store mutators, navigator tree/search model

### Task 12: TS model shape — `members`, `path`, `packages` through the overlay

**Files:**
- Modify: `packages/okf/src/types.ts` (`ModelNode`, `ModelGraph`), `packages/core/src/state/overlay.ts` (`RustNode`, `RustModel`, `toModelGraph`)
- Test: `packages/core/src/state/overlay.test.ts`

**Interfaces:**
- Produces: `ModelNode` gains `members?: string[]`; `ModelGraph` gains `path: string` and `packages: ModelNode[]`. `toModelGraph` copies `model.path` (default `""`), maps `model.packages` to `ModelNode[]` (carrying `members`), and leaves `nodes`/`edges`/`diagrams` unchanged.

- [ ] **Step 1: Write the failing test**

Add to `overlay.test.ts`:

```ts
test("toModelGraph carries path and packages with members", () => {
  const rust = {
    nodes: [{ key: "order", type: "uml.Class", title: "Order", stereotypes: [], attributes: [] }],
    edges: [],
    diagrams: [],
    path: "acme-model",
    packages: [{ key: "", type: "uml.Package", title: "", stereotypes: [], attributes: [], members: ["order"] }],
  } as unknown as RustModel;
  const g = toModelGraph(rust, emptyOverlay());
  expect(g.path).toBe("acme-model");
  expect(g.packages).toHaveLength(1);
  expect(g.packages[0].members).toEqual(["order"]);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter @uaml/core test -- overlay`
Expected: FAIL — `g.path`/`g.packages` undefined; type errors on `RustModel.path`.

- [ ] **Step 3: Write minimal implementation**

In `types.ts`, add `members?: string[]` to `ModelNode`, and to `ModelGraph`:

```ts
  /** Bundle/root name (export label + navigator root crumb). */
  path: string;
  /** Discovered uml.Package nodes (root has key ""), carrying ordered `members`. */
  packages: ModelNode[];
```

In `overlay.ts`: add `members?: string[]` to `RustNode`; add `path: string` and `packages: RustNode[]` to `RustModel`; in `toModelGraph`, build `packages` by mapping `model.packages` through the same field-copy used for `nodes` plus `...(p.members ? { members: p.members } : {})`, and return `{ nodes, edges, diagrams, path: model.path ?? "", packages }`. Existing `ModelGraph` construction sites that omit `path`/`packages` (tests, `effectiveDiagrams` callers) must be updated — add `path: ""`, `packages: []` where a literal graph is built.

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter @uaml/core test && pnpm --filter @uaml/okf test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/okf/src/types.ts packages/core/src/state/overlay.ts packages/core/src/state/overlay.test.ts
git commit -m "feat(uaml): surface path and packages (with members) in ModelGraph"
```

### Task 13: ops-adapter — package OpDto builders

**Files:**
- Modify: `packages/core/src/state/ops-adapter.ts`
- Test: `packages/core/src/state/ops-adapter.test.ts`

**Interfaces:**
- Produces: `OpDto` union gains `pkg.move`/`pkg.rename`/`pkg.delete`/`pkg.reorder`/`pkg.sort` arms, plus `dir?: string` on `node.new`. Builders: `nodeNewOps(f)` gains an optional `dir`; `moveNodeOps(slug, toDir)`, `renamePackageOps(from, to)`, `deletePackageOps(path, cascade)`, `reorderMembersOps(path, order)`, `sortPackageOps(path)`.

- [ ] **Step 1: Write the failing test**

```ts
test("package op builders emit the expected wire shapes", () => {
  expect(moveNodeOps("order", "billing")).toEqual([{ op: "pkg.move", slug: "order", to_dir: "billing" }]);
  expect(renamePackageOps("sales", "commerce")).toEqual([{ op: "pkg.rename", from: "sales", to: "commerce" }]);
  expect(deletePackageOps("sales", true)).toEqual([{ op: "pkg.delete", path: "sales", cascade: true }]);
  expect(reorderMembersOps("sales", ["a", "b"])).toEqual([{ op: "pkg.reorder", path: "sales", order: ["a", "b"] }]);
  expect(sortPackageOps("sales")).toEqual([{ op: "pkg.sort", path: "sales" }]);
  expect(nodeNewOps({ slug: "order", type: "uml.Class", title: "Order", dir: "sales" })[0]).toMatchObject({ op: "node.new", dir: "sales" });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter @uaml/core test -- ops-adapter`
Expected: FAIL — builders undefined.

- [ ] **Step 3: Write minimal implementation**

Extend the `OpDto` union with the five package arms and `dir?: string` on `node.new`. Add `dir` to `NewNodeFields` and spread `...(f.dir ? { dir: f.dir } : {})` in `nodeNewOps`. Add the five builders:

```ts
export function moveNodeOps(slug: string, toDir: string): OpDto[] { return [{ op: "pkg.move", slug, to_dir: toDir }]; }
export function renamePackageOps(from: string, to: string): OpDto[] { return from === to ? [] : [{ op: "pkg.rename", from, to }]; }
export function deletePackageOps(path: string, cascade: boolean): OpDto[] { return [{ op: "pkg.delete", path, cascade }]; }
export function reorderMembersOps(path: string, order: string[]): OpDto[] { return [{ op: "pkg.reorder", path, order }]; }
export function sortPackageOps(path: string): OpDto[] { return [{ op: "pkg.sort", path }]; }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter @uaml/core test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/core/src/state/ops-adapter.ts packages/core/src/state/ops-adapter.test.ts
git commit -m "feat(uaml): package OpDto builders in the ops adapter"
```

### Task 14: store mutators + ghost-package state

**Files:**
- Modify: `packages/core/src/state/model.ts`
- Test: `packages/core/src/state/model.test.ts`

**Interfaces:**
- Consumes: Task 13 builders; Task 12 `graph().packages`.
- Produces store methods:
  - `createGhostPackage(parentKey: string, name: string): string` — registers an empty ghost (no op) and returns its key `parentKey ? "${parentKey}/${slug}" : slug`.
  - `moveNode(slug, toDir)`, `renamePackage(from, to)`, `deletePackage(path, cascade)`, `reorderMembers(path, order)`, `sortPackage(path)` — route through `run(...Ops(...))`.
  - `createNodeInPackage(dir, type, title): string` — `run(nodeNewOps({slug, dir, type, title}))`, materializing a ghost `dir`.
  - `graph().packages` includes ghost packages fused in (ghosts pruned when they gain a real child or lose their last one).

- [ ] **Step 1: Write the failing test**

```ts
test("ghost package appears then materializes on first child", async () => {
  await initWasm();
  const store = createModelStore();
  store.load([["order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"]]);
  const key = store.createGhostPackage("", "Sales"); // -> "sales"
  expect(store.get().packages.some(p => p.key === "sales")).toBe(true);
  // ghost is NOT in the bundle yet
  expect(store.getBundle().some(([p]) => p.startsWith("sales/"))).toBe(false);
  // add first child -> materialized in the bundle
  store.createNodeInPackage("sales", "uml.Class", "Customer");
  expect(store.getBundle().some(([p]) => p.startsWith("sales/"))).toBe(true);
});

test("moveNode relocates a doc via pkg.move", async () => {
  await initWasm();
  const store = createModelStore();
  store.load([["sales/order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"]]);
  store.moveNode("order", "billing");
  expect(store.getBundle().some(([p]) => p === "billing/order.md")).toBe(true);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter @uaml/core test -- model`
Expected: FAIL — methods undefined.

- [ ] **Step 3: Write minimal implementation**

Add a `ghosts: Set<string>` closure var. Fuse ghosts into the returned graph by wrapping `graph()`: after `toModelGraph`, append any ghost key not already a real package as an empty `uml.Package` ModelNode, and add it to its parent's `members`; prune ghosts that now exist for real. Add the mutators, each `run(...)` returning nothing (mirroring existing method style). `createGhostPackage` slugifies `name` (import `slugify` from `@uaml/okf`), records the key in `ghosts`, calls `emit()`, returns the key. `createNodeInPackage` deletes the dir from `ghosts` (it's materializing) and runs `nodeNewOps({ slug: freshSlug()-or-slugify(title), dir, type, title })`.

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter @uaml/core test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/core/src/state/model.ts packages/core/src/state/model.test.ts
git commit -m "feat(uaml): store mutators for packages incl. ghost materialization"
```

### Task 15: navigator tree builder (float diagrams, members order, scope)

**Files:**
- Create: `packages/core/src/nav/tree.ts`
- Test: `packages/core/src/nav/tree.test.ts`

**Interfaces:**
- Produces:
  - `type NavRow = { key: string; title: string; kind: "package" | "diagram" | "note" | "classifier"; depth: number; members?: string[] };`
  - `buildNavTree(graph: ModelGraph, scopeKey: string): NavRow[]` — rows for the scope package's subtree, fully expanded. Within each package: **diagrams first** (in members order), then the rest (in members order); recurses into sub-packages (diagrams float at every level). `depth` reflects nesting for indentation. Row `kind` derives from node `type` (`uml.Package`→package, `Diagram`→diagram, `uml.Note`→note, else classifier).
  - `packageOf(graph, key): string` — the package key owning a member (for context-menu targeting).

- [ ] **Step 1: Write the failing test**

```ts
import { buildNavTree } from "./tree";
const g = {
  path: "acme", nodes: [
    { key: "order", type: "uml.Class", title: "Order", stereotypes: [], attributes: [] },
    { key: "billing-rules", type: "uml.Note", title: "Billing rules", stereotypes: [], attributes: [] },
  ],
  edges: [],
  diagrams: [{ key: "overview", title: "Sales overview", profile: "uml-domain", members: [] }],
  packages: [
    { key: "", type: "uml.Package", title: "", stereotypes: [], attributes: [], members: ["sales"] },
    { key: "sales", type: "uml.Package", title: "sales", stereotypes: [], attributes: [],
      members: ["order", "billing-rules", "overview"] },
  ],
} as any;

test("diagrams float to the top of a package regardless of members order", () => {
  const rows = buildNavTree(g, "sales");
  expect(rows.map(r => r.key)).toEqual(["overview", "order", "billing-rules"]);
  expect(rows[0].kind).toBe("diagram");
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter @uaml/core test -- nav/tree`
Expected: FAIL — module missing.

- [ ] **Step 3: Write minimal implementation**

```ts
import type { ModelGraph, ModelNode } from "@uaml/okf";
export type NavKind = "package" | "diagram" | "note" | "classifier";
export interface NavRow { key: string; title: string; kind: NavKind; depth: number; members?: string[]; }

function kindOf(type: string): NavKind {
  if (type === "uml.Package") return "package";
  if (type === "Diagram") return "diagram";
  if (type === "uml.Note") return "note";
  return "classifier";
}
export function buildNavTree(graph: ModelGraph, scopeKey: string): NavRow[] {
  const byKey = new Map<string, { title: string; type: string; members?: string[] }>();
  for (const n of graph.nodes) byKey.set(n.key, { title: n.title, type: n.type });
  for (const d of graph.diagrams) byKey.set(d.key, { title: d.title, type: "Diagram" });
  for (const p of graph.packages) byKey.set(p.key, { title: p.title || graph.path, type: "uml.Package", members: p.members });
  const rows: NavRow[] = [];
  const emitPackage = (pkgKey: string, depth: number) => {
    const pkg = byKey.get(pkgKey);
    const members = pkg?.members ?? [];
    const diagrams = members.filter(k => byKey.get(k)?.type === "Diagram");
    const rest = members.filter(k => byKey.get(k)?.type !== "Diagram");
    for (const k of [...diagrams, ...rest]) {
      const m = byKey.get(k); if (!m) continue;
      rows.push({ key: k, title: m.title, kind: kindOf(m.type), depth, members: m.members });
      if (m.type === "uml.Package") emitPackage(k, depth + 1);
    }
  };
  emitPackage(scopeKey, 0);
  return rows;
}
export function packageOf(graph: ModelGraph, key: string): string {
  return graph.packages.find(p => p.members?.includes(key))?.key ?? "";
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter @uaml/core test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/core/src/nav/tree.ts packages/core/src/nav/tree.test.ts
git commit -m "feat(uaml): navigator tree builder with per-package diagram float"
```

### Task 16: navigator search (filtered tree, 3 states, highlight)

**Files:**
- Create: `packages/core/src/nav/search.ts`
- Test: `packages/core/src/nav/search.test.ts`

**Interfaces:**
- Consumes: Task 15 `buildNavTree`, `NavRow`.
- Produces:
  - `type SearchResult = { inScope: NavRow[]; elsewhere: NavRow[]; state: "matches" | "empty-scope" | "empty-all" };`
  - `filterNav(graph, scopeKey, query, typeFilter): SearchResult` — case-insensitive substring match on title, filtered against `typeFilter` (`"all"` or a metaclass token). Matching rows are kept **in structure**: ancestor packages of any match are retained (full strength, not highlighted); non-matching siblings pruned. `inScope` = filtered subtree of `scopeKey`. If empty in scope but matches exist elsewhere → `state="empty-scope"`, `elsewhere` = filtered whole-model tree. If none anywhere → `state="empty-all"`. Empty query → all rows, `state="matches"`.
  - `matchSpan(title, query): [number, number] | null` — for `<mark>` highlighting in the component.

- [ ] **Step 1: Write the failing test**

```ts
import { filterNav } from "./search";
// (reuse a nested `g` fixture: scope "sales" has no "payment"; "billing" pkg has "Payment")
test("zero in scope but matches elsewhere -> empty-scope state with elsewhere tree", () => {
  const r = filterNav(g, "sales", "payment", "all");
  expect(r.state).toBe("empty-scope");
  expect(r.inScope).toHaveLength(0);
  expect(r.elsewhere.some(row => row.title.toLowerCase().includes("payment"))).toBe(true);
  // ancestor package kept full-strength
  expect(r.elsewhere.some(row => row.kind === "package")).toBe(true);
});
test("no matches anywhere -> empty-all", () => {
  expect(filterNav(g, "sales", "zzzzz", "all").state).toBe("empty-all");
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter @uaml/core test -- nav/search`
Expected: FAIL — module missing.

- [ ] **Step 3: Write minimal implementation**

Build the full tree via `buildNavTree(graph, scopeOrRoot)`, mark rows matching `query`+`typeFilter`, then keep a row iff it matches OR a descendant matches (packages) — compute by walking rows with their `depth` to know ancestry, retaining any package that has a kept descendant. Return the three-state result. `matchSpan` does a `toLowerCase().indexOf`.

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter @uaml/core test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/core/src/nav/search.ts packages/core/src/nav/search.test.ts
git commit -m "feat(uaml): navigator filtered-tree search with three empty states"
```

## Phase 5 — Navigator sheet component (Svelte)

The navigator is a new `Navigator.svelte` grown from the switcher markup in `TopBar.svelte`. It is a prop-driven presentational component (all mutations are callbacks) so it is unit-testable exactly like `TopBar.test.ts` (@testing-library/svelte + vitest, jsdom). Icons use `lucide-svelte` (placeholder glyphs in the mockup map to lucide names); app colors `#1e88e5` / `#d8dee8`.

Shared prop contract (declared in Task 17, reused by all Phase 5 tasks):

```ts
let {
  graph,                         // ModelGraph (nodes/edges/diagrams/path/packages)
  scopeKey = "",                 // active package key ("" = root)
  activeDiagramKey = "",
  palette,                       // readonly string[] of metaclass tokens from the active profile
  onScope,                       // (key: string) => void   — drill in / breadcrumb pop
  onSelectDiagram, onRenameDiagram, onCreateDiagram,        // reuse TopBar handlers
  onAddToNewDiagram,             // (classifierKey: string) => void
  onViewInDiagram,               // (classifierKey: string, diagramKey: string) => void
  onEditProperties,              // (classifierKey: string) => void  — Inspector stub
  onCreatePackage,               // (parentKey: string, name: string) => void
  onCreateNode,                  // (dir: string, metaclass: string) => void
  onRename,                      // (key: string, kind: NavKind, title: string) => void
  onReorder,                     // (pkgKey: string, order: string[]) => void
  onSort,                        // (pkgKey: string) => void
  onDelete,                      // (key: string, kind: NavKind, mode: "cascade"|"reparent"|"single") => void
}: NavigatorProps = $props();
```

### Task 17: Navigator skeleton — search row, type chip, scope breadcrumb, tree render

**Files:**
- Create: `packages/web/src/components/Navigator.svelte`
- Test: `packages/web/src/components/Navigator.test.ts`

**Interfaces:**
- Consumes: `@uaml/core` `buildNavTree`, `filterNav`, `NavRow`, `NavKind`.
- Produces: renders a `role="menu"` sheet: search `<input aria-label="Search model">`, a type chip button `aria-label="Filter by type"` showing `All ▾`, a breadcrumb of clickable crumbs (`role="button"` per crumb, root crumb = `graph.path || "model"`), and one tree row per `buildNavTree(graph, scopeKey)` result. Row shows a kind icon and title; the active diagram row is checkmarked; indentation from `depth`. Package rows call `onScope(key)`; diagram rows call `onSelectDiagram(key)`.

- [ ] **Step 1: Write the failing test**

```ts
import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import Navigator from "./Navigator.svelte";

const graph = {
  path: "acme-model", nodes: [{ key: "customer", type: "uml.Class", title: "Customer", stereotypes: [], attributes: [] }],
  edges: [],
  diagrams: [{ key: "overview", title: "Sales overview", profile: "uml-domain", members: [] }],
  packages: [
    { key: "", type: "uml.Package", title: "", stereotypes: [], attributes: [], members: ["sales"] },
    { key: "sales", type: "uml.Package", title: "sales", stereotypes: [], attributes: [], members: ["overview", "customer"] },
  ],
};
const props = (over = {}) => ({ graph, scopeKey: "sales", activeDiagramKey: "overview", palette: ["uml.Class"],
  onScope: vi.fn(), onSelectDiagram: vi.fn(), ...over });

test("renders scope breadcrumb and floated diagram first", () => {
  render(Navigator, { props: props() });
  expect(screen.getByText("acme-model")).toBeTruthy();
  const rows = screen.getAllByRole("treeitem");
  expect(rows[0].textContent).toContain("Sales overview");
});
test("clicking a diagram row selects it; package crumb rescopes", async () => {
  const onSelectDiagram = vi.fn(); const onScope = vi.fn();
  render(Navigator, { props: props({ onSelectDiagram, onScope }) });
  await fireEvent.click(screen.getByRole("treeitem", { name: /Sales overview/ }));
  expect(onSelectDiagram).toHaveBeenCalledWith("overview");
  await fireEvent.click(screen.getByRole("button", { name: "acme-model" }));
  expect(onScope).toHaveBeenCalledWith("");
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter @uaml/web test -- Navigator`
Expected: FAIL — component missing.

- [ ] **Step 3: Write minimal implementation**

Create `Navigator.svelte`: `<script lang="ts">` importing `buildNavTree`, `filterNav`, types from `@uaml/core`; declare the shared props (Task 17 subset needed now). Derive `rows = $derived(query ? filterNav(...).inScope : buildNavTree(graph, scopeKey))`; derive `crumbs` by splitting `scopeKey` on `/` (root label `graph.path || "model"`). Markup: search input (`bind:value={query}`), chip button, breadcrumb (`{#each crumbs}` → `role="button"` calling `onScope(crumbKey)`), tree (`{#each rows}` → `role="treeitem"` with `style="padding-left:{8+row.depth*16}px"`, kind icon, title, checkmark when `row.kind==='diagram' && row.key===activeDiagramKey`; click dispatches package→`onScope`, diagram→`onSelectDiagram`).

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter @uaml/web test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/Navigator.svelte packages/web/src/components/Navigator.test.ts
git commit -m "feat(uaml): navigator sheet skeleton (search, breadcrumb, floated tree)"
```

### Task 18: type chip rotation (Ctrl-T) + drag-grip reorder

**Files:**
- Modify: `packages/web/src/components/Navigator.svelte`
- Test: `packages/web/src/components/Navigator.test.ts`

**Interfaces:**
- Produces: the type chip cycles `["all", ...palette]` on click and on `Ctrl-T` (no inline key hint rendered); the chip label de-prefixes the token (`uml.Class`→`Class`, `all`→`All`). On-hover drag grips per row; dropping a row within its package calls `onReorder(pkgKey, newOrder)` with the reordered member keys.

- [ ] **Step 1: Write the failing test**

```ts
test("Ctrl-T rotates the type chip through palette without an inline hint", async () => {
  render(Navigator, { props: props({ palette: ["uml.Class", "uml.Interface"] }) });
  const chip = screen.getByRole("button", { name: /Filter by type/ });
  expect(chip.textContent).toContain("All");
  await fireEvent.keyDown(window, { key: "t", ctrlKey: true });
  expect(chip.textContent).toContain("Class");
  expect(chip.textContent).not.toMatch(/Ctrl/i);
});
test("dropping a reordered row persists new member order", async () => {
  const onReorder = vi.fn();
  render(Navigator, { props: props({ onReorder }) });
  // simulate the component's reorder callback path (drag events on grips)
  const rows = screen.getAllByRole("treeitem");
  await fireEvent.dragStart(rows[1]); // customer
  await fireEvent.drop(rows[0]);      // above overview's slot
  expect(onReorder).toHaveBeenCalled();
  const [pkgKey, order] = onReorder.mock.calls[0];
  expect(pkgKey).toBe("sales");
  expect(order).toContain("customer");
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter @uaml/web test -- Navigator`
Expected: FAIL.

- [ ] **Step 3: Write minimal implementation**

Add `typeFilter = $state("all")`, `chipOptions = $derived(["all", ...palette])`, a `rotateChip()` and a `$effect` binding a `keydown` listener that on `ctrlKey && key==='t'` (preventDefault) calls `rotateChip()`. Chip label helper `deprefix(t)`. Add `draggable` grips (`<span aria-hidden>⠿</span>`), `dragstart`/`dragover`/`drop` handlers tracking source/target index within the same package; on drop compute the new member key order for that package (from `packageOf(graph, key)` + current members) and call `onReorder(pkgKey, order)`. Wire the derived `rows` to pass `typeFilter` into `filterNav` when searching.

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter @uaml/web test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/Navigator.svelte packages/web/src/components/Navigator.test.ts
git commit -m "feat(uaml): navigator type-chip rotation and drag reorder"
```

### Task 19: action menu (left-click classifier)

**Files:**
- Modify: `packages/web/src/components/Navigator.svelte`
- Test: `packages/web/src/components/Navigator.test.ts`

**Interfaces:**
- Produces: left-clicking a classifier/note row opens an action menu: **View in diagram** — if the classifier is a member of exactly one diagram, calls `onViewInDiagram(key, dKey)`; if several, opens a submenu of those diagrams; if none, the item reads **Add to new diagram** and calls `onAddToNewDiagram(key)`. **Add to new diagram** → `onAddToNewDiagram(key)`. **View / edit properties** → `onEditProperties(key)`. Which diagrams "contain" a classifier = `graph.diagrams.filter(d => d.members.includes(key))`.

- [ ] **Step 1: Write the failing test**

```ts
test("classifier with one containing diagram jumps; edit-props calls stub", async () => {
  const onViewInDiagram = vi.fn(); const onEditProperties = vi.fn();
  const g2 = structuredClone(graph); g2.diagrams[0].members = ["customer"];
  render(Navigator, { props: props({ graph: g2, onViewInDiagram, onEditProperties }) });
  await fireEvent.click(screen.getByRole("treeitem", { name: /Customer/ }));
  await fireEvent.click(screen.getByRole("menuitem", { name: /View in diagram/ }));
  expect(onViewInDiagram).toHaveBeenCalledWith("customer", "overview");
  await fireEvent.click(screen.getByRole("treeitem", { name: /Customer/ }));
  await fireEvent.click(screen.getByRole("menuitem", { name: /View \/ edit properties/ }));
  expect(onEditProperties).toHaveBeenCalledWith("customer");
});
test("classifier in no diagram shows Add to new diagram", async () => {
  const onAddToNewDiagram = vi.fn();
  render(Navigator, { props: props({ onAddToNewDiagram }) });
  await fireEvent.click(screen.getByRole("treeitem", { name: /Customer/ }));
  await fireEvent.click(screen.getByRole("menuitem", { name: /Add to new diagram/ }));
  expect(onAddToNewDiagram).toHaveBeenCalledWith("customer");
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter @uaml/web test -- Navigator`
Expected: FAIL.

- [ ] **Step 3: Write minimal implementation**

Track `actionMenu = $state<{key:string}|null>(null)`; open it on a classifier/note row left-click (packages/diagrams keep their scope/select behavior). Compute `containing = graph.diagrams.filter(d => d.members.includes(key))`. Render menu items per the interface (submenu when `containing.length>1`). Each item is `role="menuitem"`. Close on select / outside click (reuse the `fixed inset-0` overlay pattern from TopBar).

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter @uaml/web test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/Navigator.svelte packages/web/src/components/Navigator.test.ts
git commit -m "feat(uaml): navigator classifier action menu (view/add/edit-stub)"
```

### Task 20: context menu (right-click) + create vocabulary from palette

**Files:**
- Modify: `packages/web/src/components/Navigator.svelte`
- Test: `packages/web/src/components/Navigator.test.ts`

**Interfaces:**
- Produces: right-click on any row opens a context menu: **New package** (`onCreatePackage(targetPkgKey, name)` via an inline name input), **New \<Metaclass\>** items — one per `palette` token, labelled de-prefixed (`uml.Class`→`New Class`) — calling `onCreateNode(targetPkgKey, token)`, **New diagram** (`onCreateDiagram(name)`), **Rename** (`onRename(key, kind, title)`), **Sort A–Z** (`onSort(targetPkgKey)`), **Delete…** (Task 22). `targetPkgKey` = the row's key if it's a package, else `packageOf(graph, key)`.

- [ ] **Step 1: Write the failing test**

```ts
test("context menu lists de-prefixed metaclasses and creates under the package", async () => {
  const onCreateNode = vi.fn(); const onSort = vi.fn();
  render(Navigator, { props: props({ palette: ["uml.Class", "uml.Interface"], onCreateNode, onSort }) });
  await fireEvent.contextMenu(screen.getByRole("treeitem", { name: /Customer/ }));
  expect(screen.getByRole("menuitem", { name: "New Class" })).toBeTruthy();
  expect(screen.getByRole("menuitem", { name: "New Interface" })).toBeTruthy();
  expect(screen.queryByText(/classifier/i)).toBeNull();
  await fireEvent.click(screen.getByRole("menuitem", { name: "New Class" }));
  expect(onCreateNode).toHaveBeenCalledWith("sales", "uml.Class");
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter @uaml/web test -- Navigator`
Expected: FAIL.

- [ ] **Step 3: Write minimal implementation**

Add `ctxMenu = $state<{key:string; kind:NavKind}|null>(null)` opened via `oncontextmenu={(e)=>{e.preventDefault(); ctxMenu={...}}}`. Compute `targetPkg = kind==='package' ? key : packageOf(graph, key)`. Render items per interface; the metaclass items map `palette` with `deprefix`. "New package"/"New diagram"/"Rename" reveal inline inputs (mirroring TopBar's `newMode` pattern — never `window.prompt`). Ensure no literal "classifier" string anywhere in the component.

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter @uaml/web test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/Navigator.svelte packages/web/src/components/Navigator.test.ts
git commit -m "feat(uaml): navigator context menu with palette-driven create vocabulary"
```

### Task 21: search filtered tree UI (three states + highlight + rescope-clears)

**Files:**
- Modify: `packages/web/src/components/Navigator.svelte`
- Test: `packages/web/src/components/Navigator.test.ts`

**Interfaces:**
- Consumes: Task 16 `filterNav`, `matchSpan`.
- Produces: while `query` is non-empty, the tree renders `filterNav(...)` output — `state==="matches"` → `inScope` rows with the matched substring wrapped in `<mark>`; `state==="empty-scope"` → a centered "No matches in \<scope\>" line, an "Elsewhere in model" divider, then `elsewhere` rows (their own filtered tree, clickable); `state==="empty-all"` → a bare centered "No matches found". Clicking a package row in results calls `onScope(key)` **and clears `query`**.

- [ ] **Step 1: Write the failing test**

```ts
test("zero-in-scope shows No matches in <scope> + Elsewhere divider with results", async () => {
  const g2 = structuredClone(graph);
  g2.nodes.push({ key: "payment", type: "uml.Class", title: "Payment", stereotypes: [], attributes: [] });
  g2.packages.push({ key: "billing", type: "uml.Package", title: "billing", stereotypes: [], attributes: [], members: ["payment"] });
  g2.packages[0].members = ["sales", "billing"];
  render(Navigator, { props: props({ graph: g2 }) });
  await fireEvent.input(screen.getByLabelText("Search model"), { target: { value: "payment" } });
  expect(screen.getByText(/No matches in/).textContent).toContain("sales");
  expect(screen.getByText(/Elsewhere in model/)).toBeTruthy();
  expect(screen.getByText("Payment").querySelector("mark")).toBeTruthy();
});
test("clicking a package in results rescopes and clears the query", async () => {
  const onScope = vi.fn();
  render(Navigator, { props: props({ onScope }) });
  const input = screen.getByLabelText("Search model") as HTMLInputElement;
  await fireEvent.input(input, { target: { value: "sal" } });
  // (root scope surfaces the "sales" package) — click it
  await fireEvent.click(screen.getByRole("treeitem", { name: /sales/ }));
  expect(onScope).toHaveBeenCalled();
  expect(input.value).toBe("");
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter @uaml/web test -- Navigator`
Expected: FAIL.

- [ ] **Step 3: Write minimal implementation**

Derive `search = $derived(query ? filterNav(graph, scopeKey, query, typeFilter) : null)`. When `search`, branch the tree render on `search.state`; render `inScope`/`elsewhere` with a `{#snippet}` row renderer that splits the title on `matchSpan` into `text<mark>match</mark>text`. Package-row click handler: `onScope(key); query = ""`. Divider + empty lines styled per the mockup (`.divider`, `.empty`).

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter @uaml/web test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/Navigator.svelte packages/web/src/components/Navigator.test.ts
git commit -m "feat(uaml): navigator filtered-tree search UI with three empty states"
```

### Task 22: delete non-empty package prompt (cascade / move-to-parent / cancel)

**Files:**
- Modify: `packages/web/src/components/Navigator.svelte`
- Test: `packages/web/src/components/Navigator.test.ts`

**Interfaces:**
- Produces: choosing **Delete…** on a package row: if the package is empty (a ghost / no members) → immediate `onDelete(key, "package", "single")`, no prompt. If non-empty → an inline prompt with three choices: **Delete children too** → `onDelete(key, "package", "cascade")`, **Move to parent** → `onDelete(key, "package", "reparent")`, **Cancel** → dismiss. Non-package rows delete via `onDelete(key, kind, "single")`.

- [ ] **Step 1: Write the failing test**

```ts
test("deleting a non-empty package prompts all three branches", async () => {
  const onDelete = vi.fn();
  render(Navigator, { props: props({ onDelete }) }); // "sales" has members
  await fireEvent.contextMenu(screen.getByRole("treeitem", { name: /^\s*sales/ }) ?? screen.getByText("sales"));
  await fireEvent.click(screen.getByRole("menuitem", { name: /Delete/ }));
  expect(screen.getByRole("button", { name: /Delete children too/ })).toBeTruthy();
  await fireEvent.click(screen.getByRole("button", { name: /Move to parent/ }));
  expect(onDelete).toHaveBeenCalledWith("sales", "package", "reparent");
});
test("deleting an empty ghost package does not prompt", async () => {
  const onDelete = vi.fn();
  const g2 = structuredClone(graph);
  g2.packages.push({ key: "empty", type: "uml.Package", title: "empty", stereotypes: [], attributes: [], members: [] });
  g2.packages[0].members = ["sales", "empty"];
  render(Navigator, { props: props({ graph: g2, scopeKey: "", onDelete }) });
  await fireEvent.contextMenu(screen.getByText("empty"));
  await fireEvent.click(screen.getByRole("menuitem", { name: /Delete/ }));
  expect(onDelete).toHaveBeenCalledWith("empty", "package", "single");
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter @uaml/web test -- Navigator`
Expected: FAIL.

- [ ] **Step 3: Write minimal implementation**

Add `deletePrompt = $state<{key:string}|null>(null)`. The context-menu Delete item: if `kind!=='package'` → `onDelete(key, kind, "single")`; else look up the package's members (`graph.packages.find(p=>p.key===key)?.members ?? []`) — empty → `onDelete(key,"package","single")`, non-empty → `deletePrompt = {key}`. Render the prompt (three `role="button"`s) when set; wire the two destructive ones to `onDelete(...,"cascade"|"reparent")` and Cancel to clear.

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter @uaml/web test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/Navigator.svelte packages/web/src/components/Navigator.test.ts
git commit -m "feat(uaml): navigator delete prompt (cascade/move-to-parent/cancel)"
```

## Phase 6 — Integration & wiring

### Task 23: mount the Navigator in place of the switcher; wire handlers end-to-end

**Files:**
- Modify: `packages/web/src/components/TopBar.svelte` (replace the switcher dropdown body with `<Navigator … />`, keeping the center trigger button), and the TopBar's parent (`packages/web/src/components/canvas/CanvasInner.svelte` — the only site that already renders TopBar and holds the store + Inspector selection).
- Test: `packages/web/src/components/TopBar.test.ts` (extend), plus a focused `CanvasInner` wiring assertion where practical.

**Interfaces:**
- Consumes: Task 14 store mutators (`createGhostPackage`, `createNodeInPackage`, `moveNode`, `renamePackage`, `deletePackage`, `reorderMembers`, `sortPackage`), Task 15/16 nav model, the profile palette (`getProfile(profileName).palette.metaclasses` from `@uaml/core`), and the existing diagram handlers.
- Produces: the center title button still opens the sheet, but the sheet is now `<Navigator>`. The parent passes `graph = $model`, `scopeKey` (new local `$state`, default `""`), `activeDiagramKey`, `palette`, and maps every navigator callback to a store mutator or an existing handler. `onEditProperties(key)` sets the canvas selection to `{ type:"node", id:key }` (opening the existing Inspector). `onScope` updates the local scope state.

- [ ] **Step 1: Write the failing test**

Add to `TopBar.test.ts`:

```ts
test("the center switcher opens the Navigator sheet (not the old inline list)", async () => {
  render(TopBar, { props: {
    diagrams: [diagram("d1", "Overview")], activeDiagramKey: "d1",
    graph: { path: "acme-model", nodes: [], edges: [],
      diagrams: [{ key: "d1", title: "Overview", profile: "uml-domain", members: [] }],
      packages: [{ key: "", type: "uml.Package", title: "", stereotypes: [], attributes: [], members: ["d1"] }] },
    palette: ["uml.Class"],
    onSelectDiagram: vi.fn(), onScope: vi.fn(),
  } });
  await fireEvent.click(screen.getByRole("button", { name: /switch diagram/i }));
  // Navigator's search field is the tell that the new sheet mounted.
  expect(screen.getByLabelText("Search model")).toBeTruthy();
  expect(screen.getByText("acme-model")).toBeTruthy();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter @uaml/web test -- TopBar`
Expected: FAIL — no "Search model" field (old inline list still there).

- [ ] **Step 3: Write minimal implementation**

In `TopBar.svelte`: add the new props (`graph`, `scopeKey`, `palette`, and the navigator callbacks) alongside the existing ones. Replace the `{#if switcherOpen}` dropdown *body* (the diagram list / rename / new-diagram form) with `<Navigator {graph} {scopeKey} {activeDiagramKey} {palette} {onScope} {onSelectDiagram} {onRenameDiagram} {onCreateDiagram} … />`, keeping the trigger button and the outside-click overlay. Remove the now-dead inline switcher state that Navigator supersedes (keep `onRename/onCreate` handlers — Navigator calls them). In `CanvasInner.svelte`: add `let scopeKey = $state("")`; compute `palette = getProfile(profileName).palette.metaclasses`; pass `graph={$model}` and map callbacks: `onCreatePackage:(parent,name)=>store.createGhostPackage(parent,name)`, `onCreateNode:(dir,mc)=>store.createNodeInPackage(dir,mc, deprefix(mc))`, `onReorder:store.reorderMembers`, `onSort:store.sortPackage`, `onRename:(key,kind,title)=> kind==='package'?store.renamePackage(key, reslug(key,title)) : store.updateNode(key,{title})`, `onDelete:(key,kind,mode)=> kind==='package'? store.deletePackage(key, mode==='cascade') /* reparent→cascade:false */ : store.removeNode(key)`, `onScope:(k)=>scopeKey=k`, `onEditProperties:(key)=> selection = { type:"node", id:key }`, and reuse the existing diagram handlers + `onAddToNewDiagram`/`onViewInDiagram` (existing create-diagram-from-members + select-diagram paths). Keep the existing `TopBar.test.ts` cases green (the diagram switcher handlers still exist as props).

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm build:wasm && pnpm -r test && pnpm lint && pnpm build`
Expected: PASS (full green gate).

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/TopBar.svelte packages/web/src/components/canvas/CanvasInner.svelte packages/web/src/components/TopBar.test.ts
git commit -m "feat(uaml): mount navigator sheet in the top bar and wire package ops"
```

---

## Self-review notes

- **Spec coverage:** Part A model (Tasks 1–3), serialization/index.md (4–5), ops incl. delete-cascade/reparent + reorder/sort (6–11); Part B tree/search model (15–16), sheet + two menus + create vocab + search states + delete prompt (17–22), wiring + Inspector stub + palette-driven vocab (23). Ghost lifecycle: Task 14. Migration: Task 2 (flat→root package). Reserved names: Tasks 3–4.
- **Deferred (out of scope, per spec):** classifier document editor / LSP (Parts C/D), package profiles/kinds (Part E). The create menu is palette-driven only.
- **Known deviations (see Risks R1–R3):** global (not per-directory) slug uniqueness; `Model.packages` separate from `Model.nodes`; `path` from root `index.md` H1.
