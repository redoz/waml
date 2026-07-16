# New Package Flow Implementation Plan

> **STATUS: PAUSED (2026-07-16).** Blocked on a separate profile-system redesign
> spec. User directive: profiles must cover EVERY diagram kind and dictate both
> the toolbox (node types) and the allowed association/relationship kinds — a
> distinct subsystem being brainstormed on its own. The seed profile strings in
> Task 1 (`new_diagram_doc`) are PROVISIONAL (use-case currently resolves to
> `uml-domain`); revisit Task 1's `kind_frontmatter` once the profile spec lands
> so each kind seeds its real profile name. Everything else in this plan
> (`pkg.insert`, dialog, tree picker, create-new, retiring `mergeBundles`) is
> independent of that redesign and stands as written.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the "apply template" (Replace/Merge) flow with a unified **New Package** dialog whose Empty / Diagram / Template tiers all seed a package at a user-chosen tree location and name, backed by a new Rust `pkg.insert` op and a Rust diagram-seed generator.

**Architecture:** All bundle manipulation moves into the Rust core, where identity is full-path correct. A new `Op::PkgInsert { parent_path, name, docs }` re-roots incoming docs under `parent_path/name/` and appends them (full-path identity, so same-basename docs across packages coexist). A pure `new_diagram_doc(kind, name)` emits an empty diagram document per kind. The TS layer only orchestrates: a `pkgInsertOps` adapter, a `store.insertPackage` method, a slim `PackageTreePicker`, and a repurposed `NewPackageDialog`. The TS `mergeBundles` global-basename-dedup helper is deleted and OKF-import merge is re-pointed at `pkg.insert`.

**Tech Stack:** Rust (`waml`, `waml-ops-dto`, `waml-wasm` crates; serde; tsify-next; wasm-bindgen), TypeScript (`@waml/core`, `@waml/wasm`), Svelte 5 (runes) + Tailwind, Vitest + @testing-library/svelte.

## Global Constraints

- Identity is full bundle-path (`okf::id_of`): `Sales/order` and `Billing/order` are distinct. Never dedup or collide by bare basename.
- NO cross-package linking by bare document name — the core resolves links directory-relative (`resolve_href`); do not add name-based cross-package references.
- Windows repo, CRLF files: write files with the Write tool using ASCII hyphens only — never em-dashes in Rust/TS source, and no shell heredocs with unicode punctuation (they corrupt).
- Re-rooting strips the incoming doc's top-level folder segment, then prepends `parent_path/name/`. `./`-relative links inside re-rooted docs stay valid and are never rewritten.
- The project root (empty `parent_path`) is a valid insert target.
- Full gate (run from repo root before every commit): `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`. `pnpm build` (or `./build.ps1`) regenerates `packages/wasm/src/generated/*` from the Rust wasm surface — any new `#[wasm_bindgen]` export or `OpDto` variant requires it before TS can see the binding.
- New op variants must thread through both directions of `OpDto` (`to_op` and `from_op`) and pass the existing `every_op_survives_a_wire_round_trip` test.

---

### Task 1: Rust diagram-seed generator `new_diagram_doc`

Emits the markdown for one empty diagram document per UML diagram kind. Pure, native-tested; the wasm wrapper comes in Task 4.

**Files:**
- Create: `crates/waml/src/seed.rs`
- Modify: `crates/waml/src/lib.rs` (add `pub mod seed;` after `pub mod parse;`)
- Test: inline `#[cfg(test)] mod tests` in `crates/waml/src/seed.rs`

**Interfaces:**
- Consumes: `crate::frontmatter::{FmValue, Frontmatter}`, `crate::serialize::serialize_document`, `crate::syntax::Document`.
- Produces: `pub fn new_diagram_doc(kind: &str, name: &str) -> String`. Kind tokens: `"class"` / `"domain"` (both -> `type: Diagram`, `profile: uml-domain`), `"usecase"` (-> `type: Diagram`, `profile: uml-domain`), `"activity"` (-> `type: uml.Activity`), `"sequence"` (-> `type: uml.Sequence`). Any other token falls back to the class/domain form (never panics).

- [ ] **Step 1: Write the failing test**

Create `crates/waml/src/seed.rs` with only the tests first:

```rust
//! Seed generators for freshly created documents (the New Package flow's
//! "Diagram" tier). Emits one empty diagram document per UML diagram kind, with
//! the title set but no members, so the user starts from a blank, valid diagram.

#[cfg(test)]
mod tests {
    use super::new_diagram_doc;
    use crate::parse::build_model;

    #[test]
    fn class_kind_emits_diagram_type_and_uml_domain_profile() {
        let md = new_diagram_doc("class", "My Domain");
        assert!(md.contains("type: \"Diagram\""), "got: {md}");
        assert!(md.contains("profile: \"uml-domain\""), "got: {md}");
        assert!(md.contains("title: \"My Domain\""), "got: {md}");
        assert!(md.contains("# My Domain"), "H1 present: {md}");
    }

    #[test]
    fn usecase_kind_emits_diagram_type_and_uml_domain_profile() {
        let md = new_diagram_doc("usecase", "Actors");
        assert!(md.contains("type: \"Diagram\""), "got: {md}");
        assert!(md.contains("profile: \"uml-domain\""), "got: {md}");
        assert!(md.contains("title: \"Actors\""), "got: {md}");
    }

    #[test]
    fn activity_kind_emits_uml_activity_type_and_no_profile() {
        let md = new_diagram_doc("activity", "Checkout");
        assert!(md.contains("type: \"uml.Activity\""), "got: {md}");
        assert!(!md.contains("profile:"), "activity has no profile: {md}");
        assert!(md.contains("title: \"Checkout\""), "got: {md}");
    }

    #[test]
    fn sequence_kind_emits_uml_sequence_type() {
        let md = new_diagram_doc("sequence", "Place Order");
        assert!(md.contains("type: \"uml.Sequence\""), "got: {md}");
        assert!(md.contains("title: \"Place Order\""), "got: {md}");
    }

    #[test]
    fn unknown_kind_falls_back_to_class_form() {
        let md = new_diagram_doc("nonsense", "X");
        assert!(md.contains("type: \"Diagram\""), "got: {md}");
        assert!(md.contains("profile: \"uml-domain\""), "got: {md}");
    }

    #[test]
    fn seeded_diagram_is_a_valid_document_that_build_model_ingests() {
        // A seeded class diagram at some path must project without panicking and
        // parse as a diagram doc (empty members list is fine).
        let md = new_diagram_doc("class", "My Domain");
        let bundle = vec![("pkg/my-domain.md".to_string(), md)];
        let model = build_model(&bundle);
        assert!(model.diagrams.iter().any(|d| d.title == "My Domain"), "diagram present: {:?}", model.diagrams);
    }
}
```

Add `pub mod seed;` to `crates/waml/src/lib.rs` immediately after the `pub mod parse;` line.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml seed`
Expected: FAIL to compile with "cannot find function `new_diagram_doc`".

- [ ] **Step 3: Write minimal implementation**

Prepend to `crates/waml/src/seed.rs` (above the test module):

```rust
use crate::frontmatter::{FmValue, Frontmatter};
use crate::serialize::serialize_document;
use crate::syntax::Document;

/// `(type, profile)` for a diagram kind token. Unknown tokens fall back to the
/// class/domain form. Kept as a small table so adding a kind is one line.
fn kind_frontmatter(kind: &str) -> (&'static str, Option<&'static str>) {
    match kind {
        "activity" => ("uml.Activity", None),
        "sequence" => ("uml.Sequence", None),
        // "class" | "domain" | "usecase" | anything else
        _ => ("Diagram", Some("uml-domain")),
    }
}

/// Markdown for one empty diagram document of `kind`, titled `name`. No members;
/// canonical formatting via `serialize_document`.
pub fn new_diagram_doc(kind: &str, name: &str) -> String {
    let (ty, profile) = kind_frontmatter(kind);
    let mut entries: Vec<(String, FmValue)> = vec![("type".into(), FmValue::Str(ty.to_string()))];
    if let Some(p) = profile {
        entries.push(("profile".into(), FmValue::Str(p.to_string())));
    }
    entries.push(("title".into(), FmValue::Str(name.to_string())));
    let doc = Document {
        frontmatter: Frontmatter { entries },
        title: name.to_string(),
        sections: Vec::new(),
    };
    serialize_document(&doc)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p waml seed`
Expected: PASS (6 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/waml/src/seed.rs crates/waml/src/lib.rs
git commit -m "feat(seed): new_diagram_doc emits empty diagram per kind"
```

---

### Task 2: Rust `pkg.insert` op

Re-roots incoming docs under `parent_path/name/` and appends them. Full-path identity: same-basename docs across packages coexist; the only collision is the target package path already existing.

**Files:**
- Modify: `crates/waml/src/ops/pkg.rs` (add `op_pkg_insert` + tests)
- Modify: `crates/waml/src/ops/mod.rs` (add `Op::PkgInsert` variant + `apply_one` arm)
- Test: inline tests in `crates/waml/src/ops/pkg.rs`

**Interfaces:**
- Consumes: nothing new; uses `super::{Bundle, OpError}`.
- Produces: `Op::PkgInsert { parent_path: String, name: String, docs: Vec<(String, String)> }` and `pub(crate) fn op_pkg_insert(work: &mut Bundle, parent_path: &str, name: &str, docs: &[(String, String)]) -> Result<(), OpError>`.

- [ ] **Step 1: Write the failing test**

Add these tests inside the existing `#[cfg(test)] mod tests` block at the bottom of `crates/waml/src/ops/pkg.rs` (they use the already-imported `crate::ops::{apply, Op}`):

```rust
    #[test]
    fn insert_reroots_docs_under_parent_and_name() {
        let b: crate::ops::Bundle = vec![];
        let docs = vec![
            ("orders-domain-uml/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
            ("orders-domain-uml/customer.md".to_string(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string()),
        ];
        let out = apply(&b, &[Op::PkgInsert { parent_path: "sales".into(), name: "orders".into(), docs }]).unwrap();
        assert!(out.iter().any(|(p, _)| p == "sales/orders/order.md"), "{out:?}");
        assert!(out.iter().any(|(p, _)| p == "sales/orders/customer.md"), "{out:?}");
        assert!(out.iter().all(|(p, _)| !p.starts_with("orders-domain-uml/")), "top folder stripped: {out:?}");
    }

    #[test]
    fn insert_at_root_uses_name_as_top_segment() {
        let b: crate::ops::Bundle = vec![];
        let docs = vec![("tmpl/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string())];
        let out = apply(&b, &[Op::PkgInsert { parent_path: "".into(), name: "orders".into(), docs }]).unwrap();
        assert!(out.iter().any(|(p, _)| p == "orders/order.md"), "{out:?}");
    }

    #[test]
    fn insert_preserves_same_directory_relative_links() {
        let b: crate::ops::Bundle = vec![];
        let docs = vec![
            ("t/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- depends [Customer](./customer.md)\n".to_string()),
            ("t/customer.md".to_string(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string()),
        ];
        let out = apply(&b, &[Op::PkgInsert { parent_path: "".into(), name: "orders".into(), docs }]).unwrap();
        let order = &out.iter().find(|(p, _)| p == "orders/order.md").unwrap().1;
        assert!(order.contains("(./customer.md)"), "relative link untouched: {order}");
    }

    #[test]
    fn insert_keeps_distinct_same_basename_docs_across_packages() {
        // The old TS mergeBundles bug: a same-basename doc in a different package
        // must NOT be dropped. Full-path identity keeps both.
        let b: crate::ops::Bundle = vec![("billing/order.md".to_string(), "---\ntype: uml.Class\ntitle: Invoice Order\n---\n# Invoice Order\n".to_string())];
        let docs = vec![("t/order.md".to_string(), "---\ntype: uml.Class\ntitle: Sales Order\n---\n# Sales Order\n".to_string())];
        let out = apply(&b, &[Op::PkgInsert { parent_path: "".into(), name: "sales".into(), docs }]).unwrap();
        assert!(out.iter().any(|(p, _)| p == "billing/order.md"), "existing kept: {out:?}");
        assert!(out.iter().any(|(p, _)| p == "sales/order.md"), "inserted kept: {out:?}");
        assert_eq!(out.len(), 2, "neither dropped: {out:?}");
    }

    #[test]
    fn insert_errors_when_target_package_already_exists() {
        let b: crate::ops::Bundle = vec![("sales/orders/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string())];
        let docs = vec![("t/thing.md".to_string(), "---\ntype: uml.Class\ntitle: Thing\n---\n# Thing\n".to_string())];
        let err = apply(&b, &[Op::PkgInsert { parent_path: "sales".into(), name: "orders".into(), docs }]).unwrap_err();
        assert_eq!(err.op, "pkg.insert");
        assert!(err.reason.contains("already exists"), "got: {}", err.reason);
    }

    #[test]
    fn insert_errors_on_empty_name() {
        let b: crate::ops::Bundle = vec![];
        let docs = vec![("t/x.md".to_string(), "---\ntype: uml.Class\ntitle: X\n---\n# X\n".to_string())];
        let err = apply(&b, &[Op::PkgInsert { parent_path: "".into(), name: "".into(), docs }]).unwrap_err();
        assert!(err.reason.contains("name"), "got: {}", err.reason);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml insert_`
Expected: FAIL to compile with "no variant named `PkgInsert`".

- [ ] **Step 3: Write minimal implementation**

In `crates/waml/src/ops/pkg.rs`, add the op function (place it just before the `#[cfg(test)]` module):

```rust
/// Insert a package: re-root every doc in `docs` under `<parent_path>/<name>/`
/// (or `<name>/` at root) and append. The incoming top-level folder segment is
/// stripped so a template's baked folder is replaced by the target prefix;
/// `./`-relative links stay valid untouched. Identity is the full path, so
/// distinct same-basename docs across packages coexist. Errors if the target
/// package path already exists or `name` is empty.
pub(crate) fn op_pkg_insert(
    work: &mut Bundle,
    parent_path: &str,
    name: &str,
    docs: &[(String, String)],
) -> Result<(), OpError> {
    if name.is_empty() {
        return Err(OpError::at("pkg.insert", "package name is required"));
    }
    let prefix = if parent_path.is_empty() {
        format!("{name}/")
    } else {
        format!("{parent_path}/{name}/")
    };
    if work.iter().any(|(p, _)| p.replace('\\', "/").starts_with(&prefix)) {
        return Err(OpError::at("pkg.insert", format!("package '{}' already exists", prefix.trim_end_matches('/'))));
    }
    for (path, text) in docs {
        let norm = path.replace('\\', "/");
        // strip the incoming top-level folder segment (if any)
        let rest = match norm.split_once('/') {
            Some((_, r)) => r,
            None => norm.as_str(),
        };
        work.push((format!("{prefix}{rest}"), text.clone()));
    }
    Ok(())
}
```

In `crates/waml/src/ops/mod.rs`, add the variant to the `Op` enum (after `PkgSort { path: String },`):

```rust
    PkgInsert { parent_path: String, name: String, docs: Vec<(String, String)> },
```

And add the match arm in `apply_one` (after the `Op::PkgSort` arm):

```rust
        Op::PkgInsert { parent_path, name, docs } => pkg::op_pkg_insert(work, parent_path, name, docs),
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p waml insert_`
Expected: PASS (6 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/waml/src/ops/pkg.rs crates/waml/src/ops/mod.rs
git commit -m "feat(ops): pkg.insert re-roots and appends a package's docs"
```

---

### Task 3: Wire `pkg.insert` through `OpDto`

Adds the wire variant so `apply_ops` accepts it from TS, and keeps the `from_op`/round-trip contract intact.

**Files:**
- Modify: `crates/waml-ops-dto/src/lib.rs` (add `OpDto::PkgInsert`, `to_op` arm, `from_op` arm, extend the round-trip test)

**Interfaces:**
- Consumes: `Op::PkgInsert { parent_path, name, docs }` from Task 2.
- Produces: wire shape `{ "op": "pkg.insert", "parent_path": string, "name": string, "docs": [string, string][] }` (Tsify emits field `parent_path`, `name`, `docs`).

- [ ] **Step 1: Write the failing test**

Add this test inside `crates/waml-ops-dto/src/tests` (the `mod tests` block at the bottom):

```rust
    #[test]
    fn pkg_insert_wire_round_trips() {
        let op = Op::PkgInsert {
            parent_path: "sales".into(),
            name: "orders".into(),
            docs: vec![("t/order.md".into(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".into())],
        };
        let line = serde_json::to_string(&OpDto::from_op(&op)).unwrap();
        assert!(line.contains("\"op\":\"pkg.insert\""), "wire tag: {line}");
        let back: OpDto = serde_json::from_str(&line).unwrap();
        assert_eq!(back.to_op().unwrap(), op, "round-trip changed op: {line}");
    }
```

Also add the `Op::PkgInsert` case to the `ops` vec in the existing `every_op_survives_a_wire_round_trip` test (append after the `Op::PkgSort { .. }` entry):

```rust
            Op::PkgInsert {
                parent_path: "sales".into(),
                name: "orders".into(),
                docs: vec![("t/order.md".into(), "body".into())],
            },
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml-ops-dto pkg_insert`
Expected: FAIL to compile ("no variant named `PkgInsert`" on `OpDto`).

- [ ] **Step 3: Write minimal implementation**

In `crates/waml-ops-dto/src/lib.rs`, add the DTO variant to the `OpDto` enum (after the `PkgSort` variant, before `DiagramSet`):

```rust
    #[serde(rename = "pkg.insert")]
    PkgInsert {
        #[serde(default = "one")]
        v: u32,
        parent_path: String,
        name: String,
        #[serde(default)]
        docs: Vec<(String, String)>,
    },
```

Add the `to_op` arm (in the `impl OpDto { pub fn to_op }` match, after `OpDto::PkgSort`):

```rust
            OpDto::PkgInsert { v, parent_path, name, docs } => {
                check_v(*v, "pkg.insert")?;
                Ok(Op::PkgInsert {
                    parent_path: parent_path.clone(),
                    name: name.clone(),
                    docs: docs.clone(),
                })
            }
```

Add the `from_op` arm (in the `pub fn from_op` match, after `Op::PkgSort`):

```rust
            Op::PkgInsert { parent_path, name, docs } => OpDto::PkgInsert {
                v: 1,
                parent_path: parent_path.clone(),
                name: name.clone(),
                docs: docs.clone(),
            },
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p waml-ops-dto`
Expected: PASS (including `pkg_insert_wire_round_trips` and `every_op_survives_a_wire_round_trip`).

- [ ] **Step 5: Commit**

```bash
git add crates/waml-ops-dto/src/lib.rs
git commit -m "feat(ops-dto): wire pkg.insert through OpDto both directions"
```

---

### Task 4: Expose `new_diagram_doc` on the wasm surface + regenerate bindings

Thin `#[wasm_bindgen]` wrapper over `waml::seed::new_diagram_doc`, then regenerate the inlined TS bindings and re-export from `@waml/wasm`.

**Files:**
- Modify: `crates/waml-wasm/src/lib.rs` (add `#[wasm_bindgen] pub fn new_diagram_doc`)
- Modify: `packages/wasm/src/index.ts` (import + re-export `new_diagram_doc`)
- Create: `packages/wasm/src/seed.test.ts`
- Regenerated (do not hand-edit): `packages/wasm/src/generated/*`

**Interfaces:**
- Consumes: `waml::seed::new_diagram_doc` (Task 1).
- Produces: `new_diagram_doc(kind: string, name: string): string` exported from `@waml/wasm`.

- [ ] **Step 1: Write the failing test**

Create `packages/wasm/src/seed.test.ts`:

```ts
import { describe, it, expect, beforeAll } from "vitest";
import { initWasm, new_diagram_doc } from "./index";

describe("new_diagram_doc", () => {
  beforeAll(async () => {
    await initWasm();
  });

  it("emits a uml.Activity doc for the activity kind", () => {
    const md = new_diagram_doc("activity", "Checkout");
    expect(md).toContain('type: "uml.Activity"');
    expect(md).toContain('title: "Checkout"');
  });

  it("emits a Diagram + uml-domain doc for the class kind", () => {
    const md = new_diagram_doc("class", "My Domain");
    expect(md).toContain('type: "Diagram"');
    expect(md).toContain('profile: "uml-domain"');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter @waml/wasm test seed`
Expected: FAIL — `new_diagram_doc` is not exported from `./index`.

- [ ] **Step 3: Write minimal implementation**

In `crates/waml-wasm/src/lib.rs`, add near the other `#[wasm_bindgen]` entries:

```rust
/// Markdown for one empty diagram document of `kind` (`"class"`/`"domain"`,
/// `"usecase"`, `"activity"`, `"sequence"`), titled `name`. The seed for the
/// New Package flow's Diagram tier.
#[wasm_bindgen]
pub fn new_diagram_doc(kind: &str, name: &str) -> String {
    waml::seed::new_diagram_doc(kind, name)
}
```

Regenerate the wasm bindings so `packages/wasm/src/generated/*` picks up the new export:

Run: `./build.ps1` (repo root; full build — regenerates `packages/wasm/src/generated/waml_wasm.js`, `.d.ts`, and the inlined bytes).

Then in `packages/wasm/src/index.ts`, add `new_diagram_doc` to the runtime import from `./generated/waml_wasm.js`:

```ts
import init, {
  apply_ops,
  build_bundle,
  build_model,
  fmt,
  init_panic_hook,
  new_diagram_doc,
  reindex,
  solve as solveRaw,
  split_bundle,
  validate,
} from "./generated/waml_wasm.js";
```

And add it to the re-export line near the bottom:

```ts
export { apply_ops, build_bundle, build_model, fmt, new_diagram_doc, reindex, split_bundle, validate };
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter @waml/wasm test seed`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/waml-wasm/src/lib.rs packages/wasm/src/index.ts packages/wasm/src/seed.test.ts packages/wasm/src/generated
git commit -m "feat(wasm): expose new_diagram_doc and re-export from @waml/wasm"
```

---

### Task 5: Core adapter `pkgInsertOps`

Pure translation of a package insert into the `OpDto[]` for `apply_ops`.

**Files:**
- Modify: `packages/core/src/state/ops-adapter.ts` (add `pkgInsertOps`)
- Test: `packages/core/src/state/ops-adapter.test.ts` (add a case)

**Interfaces:**
- Consumes: `OpDto` (imported already in this file).
- Produces: `export function pkgInsertOps(parentPath: string, name: string, docs: [string, string][]): OpDto[]`.

- [ ] **Step 1: Write the failing test**

Add to `packages/core/src/state/ops-adapter.test.ts`:

```ts
import { pkgInsertOps } from "./ops-adapter";

describe("pkgInsertOps", () => {
  it("builds a single pkg.insert op carrying parent, name and docs", () => {
    const docs: [string, string][] = [["t/order.md", "# Order"]];
    expect(pkgInsertOps("sales", "orders", docs)).toEqual([
      { op: "pkg.insert", parent_path: "sales", name: "orders", docs },
    ]);
  });
});
```

(If the file already has a top-level `import` from `./ops-adapter`, add `pkgInsertOps` to that import instead of a new line.)

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter @waml/core test ops-adapter`
Expected: FAIL — `pkgInsertOps` is not exported.

- [ ] **Step 3: Write minimal implementation**

In `packages/core/src/state/ops-adapter.ts`, add under the `// ── packages ──` section (near `moveNodeOps`):

```ts
/** Insert a package: re-root `docs` under `<parentPath>/<name>/` (or `<name>/`
 *  at root) and append. `name` is the final path segment (already slugified by
 *  the caller); `docs` is the source `[path, markdown][]` whose top-level folder
 *  is replaced by the target prefix. */
export function pkgInsertOps(parentPath: string, name: string, docs: [string, string][]): OpDto[] {
  return [{ op: "pkg.insert", parent_path: parentPath, name, docs }];
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter @waml/core test ops-adapter`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/core/src/state/ops-adapter.ts packages/core/src/state/ops-adapter.test.ts
git commit -m "feat(core): pkgInsertOps adapter for the New Package flow"
```

---

### Task 6: Store `insertPackage` method

Runs `pkg.insert` against the live bundle via the store's `run()` (surfaces the collision error through `onError`, returns success).

**Files:**
- Modify: `packages/core/src/state/model.ts` (import `pkgInsertOps`; add `insertPackage`)
- Test: `packages/core/src/state/model.test.ts` (add cases in the "package mutators" describe)

**Interfaces:**
- Consumes: `pkgInsertOps` (Task 5), the store's private `run`.
- Produces: `insertPackage(parentPath: string, name: string, docs: [string, string][]): boolean` on the store.

- [ ] **Step 1: Write the failing test**

Add to `packages/core/src/state/model.test.ts` inside the `describe("package mutators + ghost state", ...)` block:

```ts
  it("insertPackage re-roots docs under the target path", () => {
    const store = createModelStore([]);
    const ok = store.insertPackage("", "orders", [
      ["t/order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"],
    ]);
    expect(ok).toBe(true);
    expect(store.getBundle().some(([p]) => p === "orders/order.md")).toBe(true);
  });

  it("insertPackage returns false and surfaces an error on a path collision", () => {
    const errors: string[] = [];
    const store = createModelStore(
      [["sales/orders/order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"]],
      { onError: (e) => errors.push(e) },
    );
    const ok = store.insertPackage("sales", "orders", [["t/x.md", "---\ntype: uml.Class\ntitle: X\n---\n# X\n"]]);
    expect(ok).toBe(false);
    expect(errors.join()).toContain("already exists");
  });
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter @waml/core test model.test`
Expected: FAIL — `store.insertPackage` is not a function.

- [ ] **Step 3: Write minimal implementation**

In `packages/core/src/state/model.ts`, add `pkgInsertOps` to the existing import from `./ops-adapter` (the module already imports `nodeNewOps`, `moveNodeOps`, etc.). Then add the method inside the returned object, next to `createGhostPackage` (after line ~234):

```ts
    /** Insert a package: re-root `docs` under `<parentPath>/<name>/` and append,
     *  via the Rust `pkg.insert` op. Returns true on success; on a path collision
     *  keeps prior state, surfaces the error, and returns false. Positions are
     *  the web layer's job (re-run layout after a successful insert). */
    insertPackage(parentPath: string, name: string, docs: [string, string][]): boolean {
      return run(pkgInsertOps(parentPath, name, docs));
    },
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter @waml/core test model.test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/core/src/state/model.ts packages/core/src/state/model.test.ts
git commit -m "feat(core): store.insertPackage runs pkg.insert"
```

---

### Task 7: Retire `mergeBundles`; re-point OKF-import merge at `pkg.insert`

Deletes the global-basename-dedup helper. The only production caller is `CanvasInner.applyMergeWithLayout` (OKF import merge; the template-merge caller is removed in Task 10). Re-point import-merge at `store.insertPackage`, using the incoming bundle's top-level folder as the package name so structure is preserved and full-path identity fixes the drop-on-duplicate bug. `mergeGraphs` is reviewed and left untouched (it remaps by fresh generated keys, has no production caller, and does not share the basename defect).

**Files:**
- Modify: `packages/core/src/sync/merge.ts` (remove `mergeBundles`; keep `mergeGraphs` + a one-line note)
- Modify: `packages/core/src/sync/merge.test.ts` (remove the `mergeBundles` test if present; keep `mergeGraphs` tests)
- Modify: `packages/web/src/components/canvas/CanvasInner.svelte` (drop the `mergeBundles` import; rewrite `applyMergeWithLayout`)
- Test: covered by Task 2's Rust regression (`insert_keeps_distinct_same_basename_docs_across_packages`) + existing `mergeGraphs` tests.

**Interfaces:**
- Consumes: `store.insertPackage` (Task 6).
- Produces: none new.

- [ ] **Step 1: Establish current green, then remove `mergeBundles`**

Confirm the `mergeBundles`-specific test (if any) in `packages/core/src/sync/merge.test.ts`. Run: `pnpm --filter @waml/core test merge`. Note which tests reference `mergeBundles`.

- [ ] **Step 2: Delete `mergeBundles` and add the review note**

In `packages/core/src/sync/merge.ts`, delete the `mergeBundles` function (lines defining it and its doc comment) and the now-unused `type Bundle` if nothing else uses it. Above `mergeGraphs`, add:

```ts
// NOTE: the former `mergeBundles` (global-basename dedup) is retired — bundle
// merging is now the Rust `pkg.insert` op (full-path identity). `mergeGraphs`
// below is graph-level remap by fresh generated keys (not basename dedup); it has
// no production caller and is kept for its unit test only.
```

Remove any `mergeBundles` import/test in `packages/core/src/sync/merge.test.ts`.

- [ ] **Step 3: Re-point import-merge in CanvasInner**

In `packages/web/src/components/canvas/CanvasInner.svelte`, remove the import line:

```ts
import { mergeBundles } from "@waml/core/sync/merge";
```

Replace the body of `applyMergeWithLayout` (currently `store.load(mergeBundles(store.getBundle(), bundle)); layoutAll();`) with:

```ts
  // Merge an incoming OKF bundle: insert it as a package (full-path identity via
  // the Rust pkg.insert op), named after its own top-level folder so structure is
  // preserved, then re-layout. Replaces the retired global-basename mergeBundles.
  function applyMergeWithLayout(bundle: Bundle) {
    const top = bundle[0]?.[0]?.replace(/[\\/].*$/, "") ?? "";
    const name = top || "imported";
    if (store.insertPackage("", name, bundle)) layoutAll();
  }
```

- [ ] **Step 4: Run the gates**

Run: `pnpm --filter @waml/core test && pnpm --filter @waml/web test && pnpm lint`
Expected: PASS; no remaining references to `mergeBundles` (grep to confirm: `git grep mergeBundles` returns nothing).

- [ ] **Step 5: Commit**

```bash
git add packages/core/src/sync/merge.ts packages/core/src/sync/merge.test.ts packages/web/src/components/canvas/CanvasInner.svelte
git commit -m "refactor: retire mergeBundles; import-merge uses pkg.insert"
```

---

### Task 8: `PackageTreePicker.svelte` — slim selectable package tree

A compact, read-only tree of the current packages with the project root as a selectable top node. No context menus, no DnD.

**Files:**
- Create: `packages/web/src/components/PackageTreePicker.svelte`
- Test: `packages/web/src/components/PackageTreePicker.test.ts`

**Interfaces:**
- Consumes: `ModelNode[]` package list (each has `key`, `members`), the project name.
- Produces: props `{ packages: { key: string }[]; projectName: string; selected: string; onSelect: (path: string) => void }`. `selected` is the chosen parent path (`""` = project root). Emits `onSelect` with the package key (or `""` for root).

- [ ] **Step 1: Write the failing test**

Create `packages/web/src/components/PackageTreePicker.test.ts`:

```ts
import { render, fireEvent } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";
import PackageTreePicker from "./PackageTreePicker.svelte";

const packages = [{ key: "sales" }, { key: "sales/orders" }, { key: "billing" }];

describe("PackageTreePicker", () => {
  it("renders the project root and every package", () => {
    const { getByText } = render(PackageTreePicker, {
      props: { packages, projectName: "My Project", selected: "", onSelect: () => {} },
    });
    expect(getByText("My Project")).toBeTruthy();
    expect(getByText("orders")).toBeTruthy();
    expect(getByText("billing")).toBeTruthy();
  });

  it("selecting a package reports its key; selecting root reports empty string", async () => {
    const onSelect = vi.fn();
    const { getByText } = render(PackageTreePicker, {
      props: { packages, projectName: "My Project", selected: "", onSelect },
    });
    await fireEvent.click(getByText("orders"));
    expect(onSelect).toHaveBeenCalledWith("sales/orders");
    await fireEvent.click(getByText("My Project"));
    expect(onSelect).toHaveBeenCalledWith("");
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter @waml/web test PackageTreePicker`
Expected: FAIL — component file does not exist.

- [ ] **Step 3: Write minimal implementation**

Create `packages/web/src/components/PackageTreePicker.svelte`:

```svelte
<script lang="ts">
  // Compact, selectable rendering of the package tree for the New Package
  // dialog's placement footer. Read-only: no context menus, no drag/drop. The
  // project root (empty path) is a valid, selectable target.
  let { packages, projectName, selected, onSelect }: {
    packages: { key: string }[];
    projectName: string;
    selected: string;
    onSelect: (path: string) => void;
  } = $props();

  // Depth by slash count drives the indent; packages are already keyed by full
  // path, so a lexicographic sort keeps children under their parent.
  const sorted = $derived([...packages].map((p) => p.key).sort());
  const label = (key: string) => key.slice(key.lastIndexOf("/") + 1);
  const depth = (key: string) => key.split("/").length;

  function rowClass(isSelected: boolean): string {
    return isSelected
      ? "bg-[#e6f1fb] text-[#1e88e5] font-[550]"
      : "text-slate-800 hover:bg-[#f1f3f7]";
  }
</script>

<div class="max-h-40 overflow-auto rounded-lg border border-[#e6e9f0] bg-white p-1 text-[13px]">
  <button
    type="button"
    onclick={() => onSelect("")}
    class="w-full text-left px-2 py-[6px] rounded-md cursor-pointer {rowClass(selected === '')}"
  >
    {projectName}
  </button>
  {#each sorted as key (key)}
    <button
      type="button"
      onclick={() => onSelect(key)}
      class="w-full text-left px-2 py-[6px] rounded-md cursor-pointer {rowClass(selected === key)}"
      style="padding-left: {depth(key) * 14 + 8}px"
    >
      {label(key)}
    </button>
  {/each}
</div>
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter @waml/web test PackageTreePicker`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/PackageTreePicker.svelte packages/web/src/components/PackageTreePicker.test.ts
git commit -m "feat(web): PackageTreePicker slim selectable package tree"
```

---

### Task 9: `NewPackageDialog.svelte` — the unified modal

Repurposes `TemplateApplyDialog` into the three-tier New Package modal: tier selector, contextual middle (kind chooser / template gallery), placement footer (tree picker + name + Add), collision block, and name defaults.

**Files:**
- Create: `packages/web/src/components/NewPackageDialog.svelte`
- Delete: `packages/web/src/components/TemplateApplyDialog.svelte` (its role is fully replaced; removal happens in Task 10 when the last usage is gone — this task only adds the new component)
- Test: `packages/web/src/components/NewPackageDialog.test.ts`

**Interfaces:**
- Consumes: `Template[]` (from `@waml/core`), `PackageTreePicker` (Task 8), `slugify` (`@waml/okf`).
- Produces: props `{ templates: Template[]; packages: { key: string }[]; projectName: string; onAdd: (p: NewPackagePayload) => void; onClose: () => void }` where

```ts
export type DiagramKind = "class" | "usecase" | "activity" | "sequence";
export type NewPackagePayload =
  | { tier: "empty"; parentPath: string; name: string }
  | { tier: "diagram"; parentPath: string; name: string; kind: DiagramKind }
  | { tier: "template"; parentPath: string; name: string; bundle: [string, string][] };
```

(Export these types from the component's `<script module lang="ts">` so Task 10 can import them.)

- [ ] **Step 1: Write the failing test**

Create `packages/web/src/components/NewPackageDialog.test.ts`:

```ts
import { render, fireEvent } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";
import NewPackageDialog from "./NewPackageDialog.svelte";

const templates = [
  { id: "t1", nicheId: null, category: "dataset", name: "Orders Domain (UML)", description: "d", bundle: [["orders-domain-uml/order.md", "# Order"]] },
];
const packages = [{ key: "sales" }];

function props(overrides = {}) {
  return { templates, packages, projectName: "My Project", onAdd: vi.fn(), onClose: vi.fn(), ...overrides };
}

describe("NewPackageDialog", () => {
  it("defaults to Empty tier with name 'New package'", () => {
    const { getByLabelText } = render(NewPackageDialog, { props: props() });
    expect((getByLabelText("Package name") as HTMLInputElement).value).toBe("New package");
  });

  it("switching to Diagram shows the 4 kind choices and defaults the name to the kind", async () => {
    const { getByText, getByLabelText } = render(NewPackageDialog, { props: props() });
    await fireEvent.click(getByText("Diagram"));
    expect(getByText("Class / Domain")).toBeTruthy();
    expect(getByText("Use-case")).toBeTruthy();
    expect(getByText("Activity")).toBeTruthy();
    expect(getByText("Sequence")).toBeTruthy();
    await fireEvent.click(getByText("Activity"));
    expect((getByLabelText("Package name") as HTMLInputElement).value).toBe("Activity");
  });

  it("blocks Add on a name collision with an inline message", async () => {
    // 'sales' already exists at root; typing it must disable Add.
    const { getByLabelText, getByText, getByRole } = render(NewPackageDialog, { props: props() });
    const input = getByLabelText("Package name") as HTMLInputElement;
    await fireEvent.input(input, { target: { value: "Sales" } });
    expect(getByText("name already used here")).toBeTruthy();
    expect((getByRole("button", { name: "Add" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("Empty tier emits an empty-tier payload with the selected parent and name", async () => {
    const onAdd = vi.fn();
    const { getByLabelText, getByRole } = render(NewPackageDialog, { props: props({ onAdd }) });
    await fireEvent.input(getByLabelText("Package name"), { target: { value: "Fresh" } });
    await fireEvent.click(getByRole("button", { name: "Add" }));
    expect(onAdd).toHaveBeenCalledWith({ tier: "empty", parentPath: "", name: "Fresh" });
  });

  it("Template tier emits the chosen template's bundle", async () => {
    const onAdd = vi.fn();
    const { getByText, getByRole } = render(NewPackageDialog, { props: props({ onAdd }) });
    await fireEvent.click(getByText("Template"));
    await fireEvent.click(getByText("Orders Domain (UML)"));
    await fireEvent.click(getByRole("button", { name: "Add" }));
    expect(onAdd).toHaveBeenCalledWith(expect.objectContaining({ tier: "template", parentPath: "", bundle: templates[0].bundle }));
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter @waml/web test NewPackageDialog`
Expected: FAIL — component file does not exist.

- [ ] **Step 3: Write minimal implementation**

Create `packages/web/src/components/NewPackageDialog.svelte`:

```svelte
<script module lang="ts">
  export type DiagramKind = "class" | "usecase" | "activity" | "sequence";
  export type NewPackagePayload =
    | { tier: "empty"; parentPath: string; name: string }
    | { tier: "diagram"; parentPath: string; name: string; kind: DiagramKind }
    | { tier: "template"; parentPath: string; name: string; bundle: [string, string][] };
</script>

<script lang="ts">
  import type { Template } from "@waml/core";
  import { slugify } from "@waml/okf";
  import PackageTreePicker from "./PackageTreePicker.svelte";

  let { templates, packages, projectName, onAdd, onClose }: {
    templates: Template[];
    packages: { key: string }[];
    projectName: string;
    onAdd: (p: NewPackagePayload) => void;
    onClose: () => void;
  } = $props();

  type Tier = "empty" | "diagram" | "template";
  const KIND_LABELS: Record<DiagramKind, string> = {
    class: "Class / Domain",
    usecase: "Use-case",
    activity: "Activity",
    sequence: "Sequence",
  };

  let tier = $state<Tier>("empty");
  let kind = $state<DiagramKind>("class");
  let templateId = $state<string | null>(null);
  let parentPath = $state("");
  let name = $state("New package");
  // Tracks whether the user has hand-edited the name; if not, the name follows
  // the tier/kind/template default.
  let nameDirty = $state(false);

  const selectedTemplate = $derived(templates.find((t) => t.id === templateId) ?? null);

  function cleanTemplateName(n: string): string {
    return n.replace(/\s*\(UML\)\s*$/i, "").trim();
  }

  // The default name for the current tier/selection.
  const defaultName = $derived(
    tier === "empty"
      ? "New package"
      : tier === "diagram"
        ? KIND_LABELS[kind]
        : selectedTemplate
          ? cleanTemplateName(selectedTemplate.name)
          : "New package",
  );

  // Keep the name in sync with the default until the user edits it.
  $effect(() => {
    if (!nameDirty) name = defaultName;
  });

  // Collision: does <parentPath>/<slug> already exist as a package path?
  const targetPath = $derived(
    (() => {
      const s = slugify(name);
      return parentPath ? `${parentPath}/${s}` : s;
    })(),
  );
  const collision = $derived(name.trim().length > 0 && packages.some((p) => p.key === targetPath));
  const canAdd = $derived(name.trim().length > 0 && !collision && (tier !== "template" || selectedTemplate !== null));

  function selectTier(t: Tier) {
    tier = t;
    nameDirty = false;
  }
  function selectKind(k: DiagramKind) {
    kind = k;
    nameDirty = false;
  }
  function selectTemplate(id: string) {
    templateId = id;
    nameDirty = false;
  }

  function submit() {
    if (!canAdd) return;
    const trimmed = name.trim();
    if (tier === "empty") onAdd({ tier: "empty", parentPath, name: trimmed });
    else if (tier === "diagram") onAdd({ tier: "diagram", parentPath, name: trimmed, kind });
    else if (selectedTemplate) onAdd({ tier: "template", parentPath, name: trimmed, bundle: selectedTemplate.bundle });
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="fixed inset-0 z-[60] flex items-center justify-center bg-black/40"
  onclick={(e) => { if (e.target === e.currentTarget) onClose(); }}
>
  <div class="bg-white rounded-xl shadow-xl w-[480px] max-w-[95vw] p-6 flex flex-col gap-4">
    <div class="flex items-center justify-between">
      <h2 class="text-[15px] font-semibold text-slate-900">New package</h2>
      <button onclick={onClose} class="text-slate-400 hover:text-slate-700 text-xl leading-none px-1">✕</button>
    </div>

    <!-- Tier selector -->
    <div class="grid grid-cols-3 gap-2">
      {#each [["empty", "Empty"], ["diagram", "Diagram"], ["template", "Template"]] as [t, lbl] (t)}
        <button
          type="button"
          onclick={() => selectTier(t as Tier)}
          class="rounded-lg border px-3 py-2 text-[13px] cursor-pointer {tier === t ? 'border-[#1e88e5] bg-[#e6f1fb] text-[#1e88e5] font-[550]' : 'border-[#d8dee8] text-slate-800 hover:bg-[#f1f3f7]'}"
        >
          {lbl}
        </button>
      {/each}
    </div>

    <!-- Contextual middle -->
    {#if tier === "diagram"}
      <div class="grid grid-cols-2 gap-2 border-t border-slate-100 pt-3">
        {#each Object.entries(KIND_LABELS) as [k, lbl] (k)}
          <button
            type="button"
            onclick={() => selectKind(k as DiagramKind)}
            class="rounded-lg border px-3 py-2 text-[13px] cursor-pointer {kind === k ? 'border-[#1e88e5] bg-[#e6f1fb] text-[#1e88e5] font-[550]' : 'border-[#d8dee8] text-slate-800 hover:bg-[#f1f3f7]'}"
          >
            {lbl}
          </button>
        {/each}
      </div>
    {:else if tier === "template"}
      <div class="flex flex-col gap-1.5 border-t border-slate-100 pt-3 max-h-48 overflow-auto">
        {#each templates as t (t.id)}
          <button
            type="button"
            onclick={() => selectTemplate(t.id)}
            class="text-left rounded-lg border px-3 py-2 cursor-pointer {templateId === t.id ? 'border-[#1e88e5] bg-[#e6f1fb]' : 'border-[#d8dee8] hover:bg-[#f1f3f7]'}"
          >
            <div class="text-[13px] font-[550] text-slate-900">{t.name}</div>
            <div class="text-[12px] text-slate-500">{t.description}</div>
          </button>
        {/each}
      </div>
    {/if}

    <!-- Placement footer -->
    <div class="flex flex-col gap-2 border-t border-slate-100 pt-3">
      <span class="text-[12px] font-medium text-slate-500">Place under</span>
      <PackageTreePicker {packages} {projectName} selected={parentPath} onSelect={(p) => (parentPath = p)} />
      <label class="flex flex-col gap-1 text-[12px] font-medium text-slate-500">
        Name
        <input
          aria-label="Package name"
          bind:value={name}
          oninput={() => (nameDirty = true)}
          class="text-[13px] px-2 py-[7px] border border-[#d8dee8] rounded-md text-slate-900 focus:outline-none focus:border-[#1e88e5] focus:ring-2 focus:ring-[#e6f1fb]"
        />
      </label>
      {#if collision}
        <p class="text-[12px] text-[#d93025]">name already used here</p>
      {/if}
    </div>

    <div class="flex gap-2 justify-end">
      <button
        onclick={onClose}
        class="text-[13px] font-[550] border border-[#d8dee8] bg-white text-slate-900 rounded-lg px-4 py-[7px] cursor-pointer hover:bg-[#f1f3f7]"
      >
        Cancel
      </button>
      <button
        onclick={submit}
        disabled={!canAdd}
        class="text-[13px] font-[550] bg-[#1e88e5] text-white border border-[#1e88e5] rounded-lg px-4 py-[7px] cursor-pointer hover:bg-[#1976d2] disabled:opacity-50 disabled:cursor-not-allowed"
      >
        Add
      </button>
    </div>
  </div>
</div>
```

Note on the collision test: `slugify("Sales")` must equal `"sales"` to match the existing `sales` package key — confirm `@waml/okf`'s `slugify` lowercases. If it does not, the test's expectation and the dialog remain correct (collision is computed the same way the store keys paths); adjust the test's seed package key to match `slugify`'s output.

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter @waml/web test NewPackageDialog`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/NewPackageDialog.svelte packages/web/src/components/NewPackageDialog.test.ts
git commit -m "feat(web): NewPackageDialog unified three-tier modal"
```

---

### Task 10: Wire the New Package flow into CanvasInner; remove the old template-apply path

Replaces the template Replace/Merge handlers with the New Package dialog. Empty tier -> `createGhostPackage`; Diagram tier -> `new_diagram_doc` seed + `insertPackage`; Template tier -> `insertPackage(template.bundle)`. Deletes `TemplateApplyDialog.svelte` and its usage.

**Files:**
- Modify: `packages/web/src/components/canvas/CanvasInner.svelte`
- Delete: `packages/web/src/components/TemplateApplyDialog.svelte`
- Test: `packages/web/src/components/canvas/CanvasInner.*` (extend existing canvas tests if present; otherwise the dialog/store units in Tasks 6/9 cover behavior and this task is verified by the full gate + a manual run)

**Interfaces:**
- Consumes: `NewPackageDialog` + `NewPackagePayload` (Task 9), `store.insertPackage` (Task 6), `store.createGhostPackage`, `new_diagram_doc` (Task 4), `slugify` (`@waml/okf`).
- Produces: none new.

- [ ] **Step 1: Add imports and state**

In `packages/web/src/components/canvas/CanvasInner.svelte`:

Add imports (near the other component/util imports):

```ts
import NewPackageDialog, { type NewPackagePayload } from "../NewPackageDialog.svelte";
import { new_diagram_doc } from "@waml/wasm";
import { slugify } from "@waml/okf";
```

Add state near `let showLibrary` / `let showClear`:

```ts
  let showNewPackage = $state(false);
```

- [ ] **Step 2: Add the New Package handler**

Add near `handleUseTemplate` (which will be removed in Step 3):

```ts
  // Realize a New Package dialog choice. Empty -> ghost package (materializes on
  // first child). Diagram/Template -> pkg.insert (Rust re-roots + appends), then
  // re-layout so the freshly added nodes leave the origin.
  function handleNewPackageAdd(p: NewPackagePayload) {
    if (p.tier === "empty") {
      store.createGhostPackage(p.parentPath, p.name);
    } else {
      const slug = slugify(p.name);
      const docs: Bundle =
        p.tier === "diagram"
          ? [[`${slug}.md`, new_diagram_doc(p.kind, p.name)]]
          : p.bundle;
      if (store.insertPackage(p.parentPath, slug, docs)) layoutAll();
    }
    showNewPackage = false;
  }
```

- [ ] **Step 3: Remove the old template-apply path**

Delete these functions from CanvasInner: `applyTemplate`, `handleUseTemplate`, `handleTemplateApplyConfirm`, and the `pendingTemplate` state. Repoint the template library and welcome dialog to open the New Package dialog instead:

- `LibraryDialog`'s `onUse={handleUseTemplate}` -> `onUse={() => (showNewPackage = true)}` (the New Package dialog carries the template gallery itself; the standalone LibraryDialog may be retired in a follow-up, out of scope here — keep it opening the new dialog).
- `WelcomeDialog`'s `onUseTemplate={(g, name) => { handleUseTemplate(g, name); showWelcome = false; }}` -> `onUseTemplate={() => { showWelcome = false; showNewPackage = true; }}`.

Remove the `{#if pendingTemplate}<TemplateApplyDialog .../>{/if}` block and the `import TemplateApplyDialog` line. Then render the new dialog (near the other `{#if show...}` dialog blocks):

```svelte
  {#if showNewPackage}
    <NewPackageDialog
      templates={TEMPLATES}
      packages={$model.packages ?? store.get().packages}
      projectName={modelName}
      onAdd={handleNewPackageAdd}
      onClose={() => (showNewPackage = false)}
    />
  {/if}
```

Ensure `TEMPLATES` is imported from `@waml/core` (LibraryDialog previously sourced it; import it here if not already). The `packages` prop should be the live package list — use `store.get().packages` (each entry has `.key`).

Add a TopBar entry point if the library button no longer fits: keep `onLibrary={() => (showNewPackage = true)}` so the existing "New / Library" button opens the unified dialog.

- [ ] **Step 4: Delete TemplateApplyDialog and run the gate**

```bash
git rm packages/web/src/components/TemplateApplyDialog.svelte
```

Run: `pnpm --filter @waml/web test && pnpm lint && pnpm build`
Expected: PASS; `git grep TemplateApplyDialog` returns nothing.

- [ ] **Step 5: Manually verify the flow, then commit**

Launch the web app (per the project run skill) and confirm: opening New Package shows the three tiers; Diagram tier -> pick Activity, choose a parent, Add -> a new package with an empty Activity diagram appears at the chosen location; Template tier -> Add -> the template's package appears re-rooted under the chosen name; a colliding name disables Add.

```bash
git add packages/web/src/components/canvas/CanvasInner.svelte
git commit -m "feat(web): wire New Package dialog; drop template Replace/Merge"
```

---

### Task 11: Create-new project action

A TopBar "Create new" that opens a confirm dialog and, on confirm, resets to an empty project.

**Files:**
- Modify: `packages/web/src/components/canvas/CanvasInner.svelte` (state, handler, confirm dialog, TopBar wiring)
- Modify: `packages/web/src/components/TopBar.svelte` (add `onCreateNew` prop + button) — confirm exact prop plumbing against the existing TopBar surface
- Test: reuse the existing clear-canvas confirm dialog component; behavior mirrors `clearCanvas`

**Interfaces:**
- Consumes: `store.load`, `DEFAULT_MODEL_NAME`, the existing confirm-dialog component (`ClearCanvasDialog` or the generic confirm used at `CanvasInner.svelte:649`).
- Produces: none new.

- [ ] **Step 1: Add state and handler**

In `CanvasInner.svelte`, add:

```ts
  let showCreateNew = $state(false);

  // Create a new project: close the current one (everything autosaves) and reset
  // to an empty model with the default name. Same primitive as clearCanvas; the
  // separate confirm guards the context switch.
  function createNewProject() {
    store.load([]);
    selectionSet = EMPTY_SELECTION;
    modelName = DEFAULT_MODEL_NAME;
    activeDiagramKey = defaultDiagramKey(store.get());
    showCreateNew = false;
  }
```

- [ ] **Step 2: Render the confirm dialog**

Add near the other dialog blocks. Reuse the existing confirm dialog component (the one rendered for `showClear`). If it is specific to clearing, add a minimal confirm using the same component with copy "This will close the current project - your work is saved.":

```svelte
  {#if showCreateNew}
    <ConfirmDialog
      title="Create a new project"
      message="This will close the current project - your work is saved."
      confirmLabel="Create new"
      onConfirm={createNewProject}
      onClose={() => (showCreateNew = false)}
    />
  {/if}
```

If no generic `ConfirmDialog` exists, reuse `ClearCanvasDialog`'s pattern with project-appropriate copy, or extract the shared confirm markup — check what `CanvasInner.svelte:649` renders and follow that component's prop shape exactly (do not invent a new dialog if one fits).

- [ ] **Step 3: Wire the TopBar button**

In `TopBar.svelte`, add an `onCreateNew?: () => void` prop and a "Create new" menu/button item alongside Import/Export. In `CanvasInner.svelte`'s `<TopBar ... />`, pass `onCreateNew={() => (showCreateNew = true)}`.

- [ ] **Step 4: Run the gate and manually verify**

Run: `pnpm --filter @waml/web test && pnpm lint && pnpm build`
Then launch the app: "Create new" -> confirm -> the canvas empties and the name resets to the default; Cancel leaves the project intact.

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/canvas/CanvasInner.svelte packages/web/src/components/TopBar.svelte
git commit -m "feat(web): Create-new project action with confirm gate"
```

---

## Self-Review

**Spec coverage:**
- Section 1 (New Package dialog: tier selector, contextual middle, placement footer, name defaults, collision block) -> Tasks 8 + 9.
- Section 2 (`pkg.insert` op + `new_diagram_doc` seed + Empty tier via existing ghost op) -> Tasks 1, 2, 3, 4; Empty tier reuses `createGhostPackage` in Task 10.
- Section 3 (remove TS merge path; re-point OKF import; review `mergeGraphs`) -> Task 7.
- Section 4 (Create-new project) -> Task 11.
- Data flow (build op input per tier; `apply_ops`; `store.load` / layout) -> Task 10 (`insertPackage` runs the op and re-derives; `layoutAll` positions new nodes).
- Error handling (collision disables Add + op backstop; empty name disabled; profile fallback never errors) -> dialog `canAdd`/`collision` (Task 9), `op_pkg_insert` collision + empty-name errors (Task 2), `kind_frontmatter` fallback (Task 1).
- Testing bullets -> Rust re-root/link/collision/same-basename/regression + per-kind frontmatter (Tasks 1-2); Svelte tier switching / tree selection / collision / per-tier payload (Tasks 8-9); create-new gate (Task 11).
- Open implementation details: use-case profile = `uml-domain` (Task 1, documented); `pkg.insert` is a new `OpDto` variant (Task 3); `new_diagram_doc` is a standalone wasm export (Task 4); tree-picker is a slim purpose-built component (Task 8).

**Placeholder scan:** No "TBD"/"add error handling"/"write tests for the above" — every code step carries concrete code; every test step carries assertions. Two spots defer to codebase reality with explicit instructions (Task 9's `slugify` casing note; Task 11's confirm-dialog component identity) rather than inventing unverifiable APIs.

**Type consistency:** `new_diagram_doc(kind, name)` identical across Rust core (Task 1), wasm export (Task 4), and TS callsite (Task 10). `pkg.insert` fields `parent_path` / `name` / `docs` consistent across `Op::PkgInsert` (Task 2), `OpDto::PkgInsert` (Task 3), `pkgInsertOps` (Task 5), `insertPackage` (Task 6). `NewPackagePayload` defined once in Task 9 and imported in Task 10. `DiagramKind` tokens (`class`/`usecase`/`activity`/`sequence`) match between the dialog (Task 9) and `kind_frontmatter` (Task 1).

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-16-new-package-flow.md`. Two execution options:

1. **Subagent-Driven (recommended)** - a fresh subagent per task, review between tasks, fast iteration.
2. **Inline Execution** - execute tasks in this session using executing-plans, batch execution with checkpoints.

Which approach?
