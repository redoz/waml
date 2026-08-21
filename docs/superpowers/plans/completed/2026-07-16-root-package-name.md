# Root Package Name Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Delete the phantom `modelName` string and make the bundle's one true name its root package title (`$model.path`), editable inline from the top bar via a new generic `pkg.retitle { path, title }` op.

**Architecture:** A new bundle op `pkg.retitle` threads through every layer (Rust `Op` → `waml-ops-dto` DTO → `waml-wasm` regeneration → core ops-adapter → store → web UI), generic over any package key. It reuses the existing `write_package_index` machinery to write/create `<path>/index.md` (root → `index.md`) with an H1, preserving intro prose and member listing. `render_index` is made title-aware so custom H1s survive reorder/sort/reindex. The top bar wires the root package (key `""`); everything downstream (export filename, image name, share) reads `$model.path`. All `modelName` / `&n=` share-name plumbing is deleted.

**Tech Stack:** Rust (crates/waml, waml-ops-dto, waml-wasm), wasm-pack + tsify (generated TS types), TypeScript (packages/core), Svelte 5 runes (packages/web), Vitest, @testing-library/svelte.

## Global Constraints

- **Guiding principle — "a package is a package":** The op, DTO, wasm mapping, core ops-adapter fn, and store method are GENERIC over any package key. Root (key `""`) is just the instance the top bar wires. The ONLY root special-casing allowed is the index-file path arithmetic: root → `index.md`, else → `<key>/index.md`.
- **No migration** of `mc.modelName.v1` — the value is ignored and deleted.
- **Op version tag:** every DTO variant carries `#[serde(default = "one")] v: u32` and `check_v(*v, "<op>")` in `to_op`; `from_op` sets `v: 1`. Mirror this exactly.
- **Rust op error pattern:** `OpError::at("<op.name>", "<reason>")`; the op name string matches the wire tag (`pkg.retitle`).
- **Wasm regeneration is mandatory** after any `waml-ops-dto` change: run `pnpm build:wasm` and commit the regenerated `packages/wasm/src/generated/{waml_wasm.js,waml_wasm.d.ts,wasm-inline.ts}`. The generated `OpDto` union is the single source of truth consumed by TS (`import type { OpDto } from "@waml/wasm"`).
- **Full monorepo gate** (run before declaring a task done / final commit): `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`. Individual steps run focused tests; the gate is the last line of defense.
- **Existing uncommitted working-tree edits** already did the DISPLAY half partially (`TopBar.svelte` shows `WAML` text + a `rootPackageName` prop; `CanvasInner.svelte` derives `rootPackageName`; `TopBar.test.ts` updated). This plan adjusts those and adds the rename UI + removals. Do NOT revert the working-tree display changes; build on them.
- Windows dev environment: use `pnpm` scripts; `cargo test --workspace` from repo root.

---

## File / Responsibility Map

- `crates/waml/src/index_md.rs` — `render_index` becomes title-aware; `reindex_bundle` sources the root title from `model.path`, else `concept.title`.
- `crates/waml/src/ops/pkg.rs` — `write_package_index` gains a `MemberOrder` enum + `title_override`; new `op_pkg_retitle`; fix root index-path arithmetic (`""` → `index.md`).
- `crates/waml/src/ops/mod.rs` — `Op::PkgRetitle` variant + `apply_one` dispatch.
- `crates/waml-ops-dto/src/lib.rs` — `OpDto::PkgRetitle` variant + `to_op`/`from_op` + round-trip test.
- `packages/wasm/src/generated/*` — regenerated (do not hand-edit).
- `packages/core/src/state/ops-adapter.ts` — `retitlePackageOps(key, title)`.
- `packages/core/src/state/model.ts` — store `retitlePackage(key, title)`.
- `packages/web/src/components/TopBar.svelte` — inline rename UI (hover pencil, input, Enter/blur/Esc), `onRenameRoot` prop, `Untitled` placeholder.
- `packages/web/src/components/canvas/CanvasInner.svelte` — wire `onRenameRoot`, simplify `rootPackageName`, purge `modelName` (state, effect, imageName, export title, share url, template seeding).
- `packages/core/src/state/modelName.ts` — DELETED.
- `packages/core/src/share/url.ts` + `packages/web/src/state/bootstrap.ts` — drop `&n=` / `readSharedName` / `sharedModelName`.

---

## Task 1: Title-aware `render_index` + title-preserving reindex/reorder/sort (Rust)

Make `render_index` emit the package's current title (H1) verbatim instead of always deriving it from the directory basename, falling back to the basename only when no title is set. Root's title lives in `model.path` (the root `index.md` H1), other packages carry it on `concept.title`. This fixes the latent clobber where reorder/sort/reindex silently reset a custom title. Also introduce the `MemberOrder` enum and `title_override` parameter on `write_package_index` (the override is unused by reorder/sort here — always `None` — but is the seam Task 2's `pkg.retitle` uses), and fix the root index-file path arithmetic (`""` currently produces the buggy `/index.md`).

**Files:**
- Modify: `crates/waml/src/index_md.rs` (`render_index` signature+body lines 23-44; `reindex_bundle` lines 84-90; existing tests lines 98-125)
- Modify: `crates/waml/src/ops/pkg.rs` (`write_package_index` lines 85-122; `op_pkg_reorder`/`op_pkg_sort` lines 124-129; tests)

**Interfaces:**
- Produces (consumed by Task 2):
  - `pub fn render_index(dir: &str, title: Option<&str>, description: Option<&str>, members: &[IndexEntry]) -> String` — `title` (when `Some(non-empty)`) is emitted verbatim as the H1; otherwise falls back to the dir basename (`"index"` for root).
  - `enum MemberOrder<'a> { Explicit(&'a [String]), Sort }` in `pkg.rs` (Task 2 adds a `Keep` variant).
  - `fn write_package_index(work: &mut Bundle, path: &str, order: MemberOrder<'_>, title_override: Option<&str>) -> Result<(), OpError>` — sources the current title (root → `model.path`, else → `pkg.concept.title`), lets `title_override` win, writes `index.md` for root / `<path>/index.md` otherwise.

- [ ] **Step 1: Update the two existing `render_index` unit tests to the new signature and add a title-emitted test**

In `crates/waml/src/index_md.rs`, replace the `render_index_emits_intro_and_listing` test (lines 98-110) and add a title test:

```rust
    #[test]
    fn render_index_emits_intro_and_listing() {
        let members = vec![
            IndexEntry { key: "sales/orders".into(), title: "orders".into(), blurb: None, is_package: true },
            IndexEntry { key: "customer".into(), title: "Customer".into(), blurb: Some("a buyer".into()), is_package: false },
        ];
        // title None => fall back to the dir basename.
        let out = render_index("sales", None, Some("Sales bounded context."), &members);
        assert!(out.starts_with("# sales\n"));
        assert!(out.contains("Sales bounded context."));
        assert!(out.contains("* [orders](orders/)"));
        assert!(out.contains("* [Customer](./customer.md) - a buyer"));
        assert!(!out.contains("---")); // frontmatter-less
    }

    #[test]
    fn render_index_emits_a_custom_title_verbatim() {
        let out = render_index("sales", Some("Sales Domain"), None, &[]);
        assert!(out.starts_with("# Sales Domain\n"), "custom title must be the H1: {out}");
    }

    #[test]
    fn render_index_root_uses_title_over_index_fallback() {
        // Root ("" dir): a Some title wins; None falls back to "index".
        assert!(render_index("", Some("My Domain"), None, &[]).starts_with("# My Domain\n"));
        assert!(render_index("", None, None, &[]).starts_with("# index\n"));
    }
```

- [ ] **Step 2: Run the tests to verify they fail to compile (signature mismatch)**

Run: `cargo test -p waml render_index`
Expected: FAIL — compile error, `render_index` takes 3 args not 4.

- [ ] **Step 3: Make `render_index` title-aware**

In `crates/waml/src/index_md.rs`, replace the `render_index` function (lines 23-44):

```rust
pub fn render_index(dir: &str, title: Option<&str>, description: Option<&str>, members: &[IndexEntry]) -> String {
    let fallback = if dir.is_empty() { "index" } else { dir.rsplit('/').next().unwrap_or(dir) };
    // A custom title (parsed from the existing H1, or set by pkg.retitle) is
    // emitted verbatim; only an absent/blank title falls back to the basename.
    let heading = title.map(str::trim).filter(|t| !t.is_empty()).unwrap_or(fallback);
    let mut out = format!("# {heading}\n");
    if let Some(d) = description.filter(|d| !d.trim().is_empty()) {
        out.push('\n');
        out.push_str(d.trim());
        out.push('\n');
    }
    if !members.is_empty() {
        out.push('\n');
    }
    for e in members {
        let url = member_url(dir, e);
        match &e.blurb {
            Some(b) if !b.trim().is_empty() => {
                out.push_str(&format!("* [{}]({url}) - {}\n", e.title, b.trim()))
            }
            _ => out.push_str(&format!("* [{}]({url})\n", e.title)),
        }
    }
    out
}
```

- [ ] **Step 4: Make `reindex_bundle` source the root title from `model.path`**

In `crates/waml/src/index_md.rs`, replace the `out.push(...)` call inside the `for pkg in &model.packages` loop (lines 84-90):

```rust
        let path = if pkg.key.is_empty() {
            "index.md".to_string()
        } else {
            format!("{}/index.md", pkg.key)
        };
        // Root's name is the model path (root index.md H1); nested packages carry
        // it on concept.title. Preserve either verbatim instead of resetting to
        // the dir basename.
        let title: Option<&str> = if pkg.key.is_empty() {
            (!model.path.is_empty()).then_some(model.path.as_str())
        } else {
            pkg.concept.title.as_deref()
        };
        out.push((path, render_index(&pkg.key, title, pkg.concept.description.as_deref(), &entries)));
```

- [ ] **Step 5: Add a `reindex_bundle` golden test that a custom root H1 survives**

In `crates/waml/src/index_md.rs` tests module, add:

```rust
    #[test]
    fn reindex_preserves_a_custom_root_index_title() {
        let b = vec![
            ("index.md".to_string(), "# My Domain\n\n* [Order](./order.md)\n".to_string()),
            ("order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
        ];
        let out = reindex_bundle(&b);
        let root = &out.iter().find(|(p, _)| p == "index.md").unwrap().1;
        assert!(root.starts_with("# My Domain\n"), "root H1 must survive reindex, got: {root}");
    }
```

- [ ] **Step 6: Introduce `MemberOrder` + `title_override` in `write_package_index` (pkg.rs)**

In `crates/waml/src/ops/pkg.rs`, add the enum above `write_package_index` and replace `write_package_index` (lines 82-122) plus the two callers (lines 124-129):

```rust
/// How a rewritten index.md orders its members. `Sort` = A–Z by title; `Explicit`
/// = a caller-supplied order (unknown keys ignored, missing keys appended).
enum MemberOrder<'a> {
    Explicit(&'a [String]),
    Sort,
}

/// Write/replace `<path>/index.md` (root → `index.md`) with a listing in the
/// requested order, preserving intro prose + blurbs. The H1 title comes from
/// `title_override` when set, else the package's current title (root →
/// `model.path`, else `concept.title`), else the dir basename.
fn write_package_index(
    work: &mut Bundle,
    path: &str,
    order: MemberOrder<'_>,
    title_override: Option<&str>,
) -> Result<(), OpError> {
    let model = build_model(work);
    let pkg = model
        .packages
        .iter()
        .find(|p| p.key == path)
        .ok_or_else(|| OpError::at("pkg.index", format!("no package '{path}'")))?;
    // desired order
    let mut keys: Vec<String> = match order {
        MemberOrder::Explicit(o) => {
            let mut v: Vec<String> = o.iter().filter(|k| pkg.members.contains(k)).cloned().collect();
            for m in &pkg.members {
                if !v.contains(m) {
                    v.push(m.clone());
                }
            }
            v
        }
        MemberOrder::Sort => {
            let mut v = pkg.members.clone();
            v.sort_by_key(|k| member_title(&model, k).to_lowercase());
            v
        }
    };
    let entries: Vec<IndexEntry> = keys
        .drain(..)
        .map(|k| {
            let (title, is_pkg, blurb) = model
                .nodes
                .iter()
                .find(|n| n.key == k)
                .map(|n| {
                    (
                        n.concept.title.clone().unwrap_or_else(|| k.clone()),
                        false,
                        n.concept.description.as_ref().map(|d| d.lines().next().unwrap_or("").to_string()),
                    )
                })
                .or_else(|| model.diagrams.iter().find(|d| d.key == k).map(|d| (d.title.clone(), false, None)))
                .or_else(|| {
                    model
                        .packages
                        .iter()
                        .find(|p| p.key == k)
                        .map(|p| (p.concept.title.clone().unwrap_or_else(|| k.clone()), true, None))
                })
                .unwrap_or((k.clone(), false, None));
            IndexEntry { key: k, title, is_package: is_pkg, blurb }
        })
        .collect();
    // Current title: root's name lives on model.path (the root index.md H1);
    // other packages carry it on concept.title. An explicit override wins.
    let current_title = if path.is_empty() {
        (!model.path.is_empty()).then(|| model.path.clone())
    } else {
        pkg.concept.title.clone()
    };
    let title_for_index = title_override.map(str::to_string).or(current_title);
    let text = render_index(path, title_for_index.as_deref(), pkg.concept.description.as_deref(), &entries);
    // Root special-case is ONLY the index-file path arithmetic.
    let idx_path = if path.is_empty() { "index.md".to_string() } else { format!("{path}/index.md") };
    match work.iter_mut().find(|(p, _)| *p == idx_path) {
        Some(slot) => slot.1 = text,
        None => work.push((idx_path, text)),
    }
    Ok(())
}

pub(crate) fn op_pkg_reorder(work: &mut Bundle, path: &str, order: &[String]) -> Result<(), OpError> {
    write_package_index(work, path, MemberOrder::Explicit(order), None)
}
pub(crate) fn op_pkg_sort(work: &mut Bundle, path: &str) -> Result<(), OpError> {
    write_package_index(work, path, MemberOrder::Sort, None)
}
```

- [ ] **Step 7: (no nested-sort test)**

> **NOTE — scope guard:** A nested package's `concept.title` is ALWAYS pinned to
> its directory segment by `build_packages` (`crates/waml/src/parse.rs:608`), never
> a free-form H1. So a custom nested H1 is NOT durable across reorder/sort — that
> is out of scope for this feature (the top bar only retitles ROOT, and root IS
> durable because its name lives in `model.path`). The previously-planned
> `sort_preserves_a_custom_package_title` test asserted a false premise and is
> deliberately omitted. The Task 1 golden coverage that matters is Step 5's
> `reindex_preserves_a_custom_root_index_title`.

- [ ] **Step 8: Run the Rust tests for this crate**

Run: `cargo test -p waml index_md && cargo test -p waml pkg`
Expected: PASS — all `render_index`, `reindex`, and `pkg` tests green.

- [ ] **Step 9: Commit**

```bash
git add crates/waml/src/index_md.rs crates/waml/src/ops/pkg.rs
git commit -m "feat(index): title-aware render_index; preserve custom H1 on reindex/reorder/sort"
```

---

## Task 2: `Op::PkgRetitle { path, title }` core op (Rust)

Add the generic retitle op. It rejects an empty/whitespace title, then reuses `write_package_index` with `MemberOrder::Keep` (preserving the current member listing) and `title_override = Some(title)` to write/create the index H1 while keeping intro prose + members. Works identically for a nested key and for root `""` (build_model always synthesizes a root package, so root exists even for an index-less or empty bundle).

**Files:**
- Modify: `crates/waml/src/ops/pkg.rs` (add `MemberOrder::Keep`; add `op_pkg_retitle`; tests)
- Modify: `crates/waml/src/ops/mod.rs` (`Op::PkgRetitle` variant lines ~105-109; `apply_one` dispatch lines ~153-157)

**Interfaces:**
- Consumes: `write_package_index`, `MemberOrder` (Task 1).
- Produces (consumed by Task 3):
  - `Op::PkgRetitle { path: String, title: String }` in `crates/waml/src/ops/mod.rs`.
  - `pub(crate) fn op_pkg_retitle(work: &mut Bundle, path: &str, title: &str) -> Result<(), OpError>`.

- [ ] **Step 1: Write the failing tests**

In `crates/waml/src/ops/pkg.rs` tests module, add:

```rust
    #[test]
    fn retitle_creates_root_index_when_absent() {
        let b = vec![("order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string())];
        let out = apply(&b, &[Op::PkgRetitle { path: "".into(), title: "Acme".into() }]).unwrap();
        let idx = &out.iter().find(|(p, _)| p == "index.md").expect("root index.md created").1;
        assert!(idx.starts_with("# Acme\n"), "root H1: {idx}");
        assert!(idx.contains("./order.md"), "member listing preserved: {idx}");
    }

    #[test]
    fn retitle_preserves_intro_and_members_for_a_nested_package() {
        let b = vec![
            ("sales/index.md".to_string(), "# Old\n\nIntro prose.\n\n* [order](./order.md)\n".to_string()),
            ("sales/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
            ("sales/customer.md".to_string(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string()),
        ];
        let out = apply(&b, &[Op::PkgRetitle { path: "sales".into(), title: "Sales Domain".into() }]).unwrap();
        let idx = &out.iter().find(|(p, _)| p == "sales/index.md").unwrap().1;
        assert!(idx.starts_with("# Sales Domain\n"), "new H1: {idx}");
        assert!(idx.contains("Intro prose."), "intro preserved: {idx}");
        assert!(idx.contains("./order.md") && idx.contains("./customer.md"), "members preserved: {idx}");
    }

    #[test]
    fn retitle_rejects_an_empty_title() {
        let b = vec![("order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string())];
        let err = apply(&b, &[Op::PkgRetitle { path: "".into(), title: "   ".into() }]).unwrap_err();
        assert_eq!(err.op, "pkg.retitle");
        assert!(err.reason.contains("empty"), "reason: {}", err.reason);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml retitle`
Expected: FAIL — `Op::PkgRetitle` variant does not exist (compile error).

- [ ] **Step 3: Add the `Op::PkgRetitle` enum variant + dispatch**

In `crates/waml/src/ops/mod.rs`, add the variant after `PkgSort { path: String },` (line 109):

```rust
    PkgRetitle { path: String, title: String },
```

And in `apply_one`, add after the `Op::PkgSort` arm (line 157):

```rust
        Op::PkgRetitle { path, title } => pkg::op_pkg_retitle(work, path, title),
```

- [ ] **Step 4: Add `MemberOrder::Keep` and `op_pkg_retitle` (pkg.rs)**

In `crates/waml/src/ops/pkg.rs`, extend the `MemberOrder` enum with a `Keep` variant and handle it in `write_package_index`'s match:

```rust
enum MemberOrder<'a> {
    Explicit(&'a [String]),
    Sort,
    /// Keep the package's current (reconciled) member order — used by retitle,
    /// which must not reshuffle the listing.
    Keep,
}
```

Add a match arm in `write_package_index` (alongside `Explicit`/`Sort`):

```rust
        MemberOrder::Keep => pkg.members.clone(),
```

Add the op function below `op_pkg_sort`:

```rust
/// Set a package's display title by writing its index.md H1, creating the file
/// (root → `index.md`, else `<path>/index.md`) when absent. Preserves the intro
/// prose and member listing. Empty/whitespace titles are rejected. Generic over
/// any package key; root ("") is just one instance.
pub(crate) fn op_pkg_retitle(work: &mut Bundle, path: &str, title: &str) -> Result<(), OpError> {
    if title.trim().is_empty() {
        return Err(OpError::at("pkg.retitle", "title cannot be empty"));
    }
    write_package_index(work, path, MemberOrder::Keep, Some(title))
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p waml retitle`
Expected: PASS — all three retitle tests green.

- [ ] **Step 6: Run the whole core crate**

Run: `cargo test -p waml`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/waml/src/ops/pkg.rs crates/waml/src/ops/mod.rs
git commit -m "feat(ops): add generic pkg.retitle op (writes package index H1)"
```

---

## Task 3: `OpDto::PkgRetitle` DTO variant + wasm regeneration (Rust + generated TS)

Add the wire DTO variant with the `v` version tag, wire `to_op`/`from_op`, extend the round-trip test, then regenerate the wasm/TS surface so the generated `OpDto` union gains `pkg.retitle`.

**Files:**
- Modify: `crates/waml-ops-dto/src/lib.rs` (enum variant ~line 190; `to_op` ~line 416; `from_op` ~line 509; `every_op_survives_a_wire_round_trip` test ~line 656)
- Regenerate: `packages/wasm/src/generated/waml_wasm.js`, `waml_wasm.d.ts`, `wasm-inline.ts`

**Interfaces:**
- Consumes: `Op::PkgRetitle` (Task 2).
- Produces (consumed by Task 4): generated TS union member `{ op: "pkg.retitle"; v?: number; path: string; title: string }` in `packages/wasm/src/generated/waml_wasm.d.ts`, re-exported as `OpDto` from `@waml/wasm`.

- [ ] **Step 1: Add the failing round-trip test case**

In `crates/waml-ops-dto/src/lib.rs`, add to the `ops` vec inside `every_op_survives_a_wire_round_trip` (after the `Op::PkgSort` entry, ~line 656):

```rust
            Op::PkgRetitle { path: "sales".into(), title: "Sales Domain".into() },
```

- [ ] **Step 2: Run the test to verify it fails to compile**

Run: `cargo test -p waml-ops-dto every_op_survives`
Expected: FAIL — compile error: no `to_op`/`from_op`/DTO variant for `Op::PkgRetitle` (non-exhaustive match).

- [ ] **Step 3: Add the `OpDto::PkgRetitle` variant**

In `crates/waml-ops-dto/src/lib.rs`, add after the `PkgSort` variant (line 190, before the `DiagramSet` variant):

```rust
    #[serde(rename = "pkg.retitle")]
    PkgRetitle {
        #[serde(default = "one")]
        v: u32,
        path: String,
        title: String,
    },
```

- [ ] **Step 4: Wire `to_op` and `from_op`**

In `to_op`, add after the `OpDto::PkgSort` arm (line 419):

```rust
            OpDto::PkgRetitle { v, path, title } => {
                check_v(*v, "pkg.retitle")?;
                Ok(Op::PkgRetitle { path: path.clone(), title: title.clone() })
            }
```

In `from_op`, add after the `Op::PkgSort` arm (line 509):

```rust
            Op::PkgRetitle { path, title } => OpDto::PkgRetitle { v: 1, path: path.clone(), title: title.clone() },
```

- [ ] **Step 5: Run the DTO tests to verify they pass**

Run: `cargo test -p waml-ops-dto`
Expected: PASS — `every_op_survives_a_wire_round_trip` and the rest green.

- [ ] **Step 6: Run the full workspace to confirm nothing else broke**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 7: Regenerate the wasm / TS surface**

Run: `pnpm build:wasm`
Expected: wasm-pack rebuilds `crates/waml-wasm` (with the `wasm` feature on `waml-ops-dto`, which derives tsify `OpDto`), and `scripts/build-wasm.mjs` copies `waml_wasm.js` + `waml_wasm.d.ts` and rewrites `wasm-inline.ts` into `packages/wasm/src/generated/`.

- [ ] **Step 8: Verify the generated union gained `pkg.retitle`**

Run: `grep -c 'pkg.retitle' packages/wasm/src/generated/waml_wasm.d.ts`
Expected: `1` (the `OpDto` union now contains `{ op: "pkg.retitle"; v?: number; path: string; title: string }`). If it is `0`, the tsify feature did not pick up the new variant — reconfirm the `waml-ops-dto = { ..., features = ["wasm"] }` dep in `crates/waml-wasm/Cargo.toml` and rerun `pnpm build:wasm`.

- [ ] **Step 9: Commit (source + regenerated artifacts together)**

```bash
git add crates/waml-ops-dto/src/lib.rs packages/wasm/src/generated/waml_wasm.js packages/wasm/src/generated/waml_wasm.d.ts packages/wasm/src/generated/wasm-inline.ts
git commit -m "feat(dto): add pkg.retitle wire DTO; regenerate wasm/TS surface"
```

---

## Task 4: `retitlePackageOps(key, title)` core adapter (TypeScript)

Add the pure op-builder mirroring `renamePackageOps` / `reorderMembersOps`. Emits `[]` for a blank title (defense-in-depth; the Rust op also rejects it).

**Files:**
- Modify: `packages/core/src/state/ops-adapter.ts` (package builders block, lines 98-114)
- Modify: `packages/core/src/state/ops-adapter.test.ts` (`package op builders` describe, lines 221-229)

**Interfaces:**
- Consumes: generated `OpDto` (Task 3).
- Produces (consumed by Task 5): `export function retitlePackageOps(key: string, title: string): OpDto[]`.

- [ ] **Step 1: Write the failing test**

In `packages/core/src/state/ops-adapter.test.ts`, add `retitlePackageOps` to the import list (lines 13-22) and add assertions inside the `emit the expected wire shapes` test (after line 228):

```ts
    expect(retitlePackageOps("", "Acme")).toEqual([{ op: "pkg.retitle", path: "", title: "Acme" }]);
    expect(retitlePackageOps("sales", "Sales Domain")).toEqual([{ op: "pkg.retitle", path: "sales", title: "Sales Domain" }]);
    expect(retitlePackageOps("", "   ")).toEqual([]);
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `pnpm --filter @waml/core test -- ops-adapter`
Expected: FAIL — `retitlePackageOps is not a function` (or TS import error).

- [ ] **Step 3: Add the op builder**

In `packages/core/src/state/ops-adapter.ts`, add after `sortPackageOps` (line 114):

```ts
export function retitlePackageOps(key: string, title: string): OpDto[] {
  // Blank titles are rejected server-side too; short-circuit here so an accidental
  // empty edit never produces a no-op round-trip.
  return title.trim() ? [{ op: "pkg.retitle", path: key, title }] : [];
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `pnpm --filter @waml/core test -- ops-adapter`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/core/src/state/ops-adapter.ts packages/core/src/state/ops-adapter.test.ts
git commit -m "feat(core): add retitlePackageOps op builder"
```

---

## Task 5: store `retitlePackage(key, title)` method (TypeScript)

Add the store mutator mirroring `renamePackage`. Routes through `run(retitlePackageOps(...))`, which feeds `apply_ops` and re-derives; `$model.path` updates when the root is retitled.

**Files:**
- Modify: `packages/core/src/state/model.ts` (import block lines 26-39; packages methods block lines 248-260)
- Modify: `packages/core/src/state/model.test.ts` (add a round-trip test)

**Interfaces:**
- Consumes: `retitlePackageOps` (Task 4).
- Produces (consumed by Task 7): `store.retitlePackage(key: string, title: string): void`.

- [ ] **Step 1: Write the failing test**

In `packages/core/src/state/model.test.ts`, add inside the `describe("bundle-as-truth model store", ...)` block:

```ts
  it("retitlePackage writes the root index.md H1 and updates path", () => {
    const s = createModelStore([
      ["order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"],
    ] as Bundle);
    s.retitlePackage("", "Acme Domain");
    expect(s.get().path).toBe("Acme Domain");
    const idx = s.getBundle().find(([p]) => p === "index.md");
    expect(idx?.[1]).toContain("# Acme Domain");
  });
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `pnpm --filter @waml/core test -- model`
Expected: FAIL — `s.retitlePackage is not a function`.

- [ ] **Step 3: Add the store method**

In `packages/core/src/state/model.ts`, add `retitlePackageOps` to the ops-adapter import (line 38, next to `sortPackageOps`):

```ts
  sortPackageOps,
  retitlePackageOps,
```

Add the method after `sortPackage` (line 260):

```ts
    retitlePackage(key: string, title: string): void {
      run(retitlePackageOps(key, title));
    },
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `pnpm --filter @waml/core test -- model`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/core/src/state/model.ts packages/core/src/state/model.test.ts
git commit -m "feat(core): add store retitlePackage method"
```

---

## Task 6: TopBar inline rename UI (Svelte)

Add the hover pencil + inline text input to the brand. The working tree already renders `WAML` text, a `/` separator, and `{rootPackageName}`. Extend that: show a muted `Untitled` placeholder when the name is blank; reveal an edit pencil on hover; clicking the name or pencil opens an input seeded with the current name; Enter or blur commits (calls `onRenameRoot` with the trimmed value only when non-empty and changed); Esc cancels without firing.

**Files:**
- Modify: `packages/web/src/components/TopBar.svelte` (props block lines 13-44; brand markup lines 89-105; add rename state + handlers in `<script>`)
- Modify: `packages/web/src/components/TopBar.test.ts` (add rename tests)

**Interfaces:**
- Consumes: `rootPackageName` prop (already present).
- Produces (consumed by Task 7): `onRenameRoot?: (title: string) => void` prop on `TopBar`.

- [ ] **Step 1: Write the failing tests**

In `packages/web/src/components/TopBar.test.ts`, add (the file already imports `render, screen, fireEvent` and `vi`):

```ts
test("shows a muted Untitled placeholder when the root name is blank", () => {
  const { container } = render(TopBar, { props: { rootPackageName: "" } });
  expect(container.textContent).toContain("Untitled");
});

test("clicking the pencil opens an input seeded with the current name", async () => {
  render(TopBar, { props: { rootPackageName: "Acme Package" } });
  await fireEvent.click(screen.getByRole("button", { name: /rename package/i }));
  const input = screen.getByRole("textbox", { name: /package name/i }) as HTMLInputElement;
  expect(input.value).toBe("Acme Package");
});

test("Enter in the rename input fires onRenameRoot with the new value", async () => {
  const onRenameRoot = vi.fn();
  render(TopBar, { props: { rootPackageName: "Acme Package", onRenameRoot } });
  await fireEvent.click(screen.getByRole("button", { name: /rename package/i }));
  const input = screen.getByRole("textbox", { name: /package name/i });
  await fireEvent.input(input, { target: { value: "Beta Package" } });
  await fireEvent.keyDown(input, { key: "Enter" });
  expect(onRenameRoot).toHaveBeenCalledWith("Beta Package");
});

test("Escape cancels the rename without firing onRenameRoot", async () => {
  const onRenameRoot = vi.fn();
  render(TopBar, { props: { rootPackageName: "Acme Package", onRenameRoot } });
  await fireEvent.click(screen.getByRole("button", { name: /rename package/i }));
  const input = screen.getByRole("textbox", { name: /package name/i });
  await fireEvent.input(input, { target: { value: "Discarded" } });
  await fireEvent.keyDown(input, { key: "Escape" });
  expect(onRenameRoot).not.toHaveBeenCalled();
});
```

**Query determinism:** the pencil is the ONLY `<button>` labeled `Rename package`
(the name itself is a plain clickable `<span>`, not a button), so
`getByRole("button", { name: /rename package/i })` matches exactly one element.
The input's accessible name is `Package name`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm --filter @waml/web test -- TopBar`
Expected: FAIL — no `rename model` button / no textbox / `onRenameRoot` never called.

- [ ] **Step 3: Add the `onRenameRoot` prop**

In `packages/web/src/components/TopBar.svelte`, add to the destructured props (after `rootPackageName = "",` line 26):

```ts
    onRenameRoot,
```

And to the props type (after `rootPackageName?: string;` line 43):

```ts
    // Commit a new root package title (inline rename from the brand).
    onRenameRoot?: (title: string) => void;
```

- [ ] **Step 4: Add rename state + handlers in `<script>`**

In `packages/web/src/components/TopBar.svelte`, add near the other `$state` declarations (after `let switcherOpen = $state(false);` line 52):

```ts
  // ── Inline root-package rename ─────────────────────────────────────────────
  // Clicking the name (or the hover pencil) swaps it for a text input seeded with
  // the current name; Enter/blur commit a non-blank change, Esc cancels.
  let renaming = $state(false);
  let renameDraft = $state("");

  function startRename() {
    renameDraft = rootPackageName;
    renaming = true;
  }
  function commitRename() {
    if (!renaming) return;
    renaming = false;
    const next = renameDraft.trim();
    if (next && next !== rootPackageName) onRenameRoot?.(next);
  }
  function cancelRename() {
    renaming = false;
  }
  function onRenameKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      commitRename();
    } else if (e.key === "Escape") {
      e.preventDefault();
      cancelRename();
    }
  }
```

- [ ] **Step 5: Replace the brand name markup**

In `packages/web/src/components/TopBar.svelte`, replace the two `<span>` lines for the separator + name (lines 103-104) with the group-hover pencil + name/input:

```svelte
    <span class="text-slate-300 font-normal">/</span>
    {#if renaming}
      <!-- svelte-ignore a11y_autofocus -->
      <input
        aria-label="Package name"
        class="font-[550] text-slate-800 max-w-[240px] px-1 py-0.5 rounded border border-[#d8dee8] outline-none focus:border-[#1e88e5]"
        value={renameDraft}
        autofocus
        oninput={(e) => (renameDraft = e.currentTarget.value)}
        onkeydown={onRenameKey}
        onblur={commitRename}
      />
    {:else}
      <div class="group flex items-center gap-1">
        <!-- The name is a plain clickable span (NOT a button) so the pencil is the
             one and only "Rename package" button — keeps the test query
             deterministic. Both open the same inline editor. -->
        <span
          role="button"
          tabindex="0"
          onclick={startRename}
          onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); startRename(); } }}
          title="Rename package"
          class="font-[550] text-slate-600 max-w-[240px] truncate cursor-text hover:text-slate-900"
        >
          {#if rootPackageName}{rootPackageName}{:else}<span class="text-slate-400 italic">Untitled</span>{/if}
        </span>
        <button
          type="button"
          onclick={startRename}
          title="Rename package"
          aria-label="Rename package"
          class="opacity-0 group-hover:opacity-100 transition-opacity text-slate-400 hover:text-slate-700"
        >
          <Pencil size={13} />
        </button>
      </div>
    {/if}
```

(`Pencil` is already imported on line 3.)

- [ ] **Step 6: Run the tests to verify they pass**

Run: `pnpm --filter @waml/web test -- TopBar`
Expected: PASS — the existing display test plus the four new rename tests green. Exactly ONE button carries `aria-label="Rename package"` (the pencil; the name is a `<span role="button">` with a `title` but no matching accessible name via the label query used), so `getByRole("button", { name: /rename package/i })` resolves deterministically.

- [ ] **Step 7: Commit**

```bash
git add packages/web/src/components/TopBar.svelte packages/web/src/components/TopBar.test.ts
git commit -m "feat(topbar): inline root package rename with hover pencil"
```

---

## Task 7: Wire rename + purge `modelName` from CanvasInner (Svelte)

Wire the TopBar `onRenameRoot` to `store.retitlePackage("", title)`, simplify the `rootPackageName` derivation to `$model.path`, and remove every `modelName` usage: the `$state`, the persist effect, the template-name seeding, and the export/image/share derivations (now `$model.path`, fallback `"waml-model"`). Delete `modelName.ts`. Also drop the now-unused `sharedModelName` import (the export itself is removed in Task 8).

**Files:**
- Modify: `packages/web/src/components/canvas/CanvasInner.svelte` (imports lines 21, 65, 68; state line 99; `rootPackageName` derived ~line 167; `imageName` line 222; effect lines 272-275; `handleExport` line 486; `clearCanvas` line 497; template handlers lines 566, 579; TopBar usage ~line 600; ShareDialog usage line 689)
- Delete: `packages/core/src/state/modelName.ts`

**Interfaces:**
- Consumes: `store.retitlePackage` (Task 5), `TopBar` `onRenameRoot` (Task 6).

- [ ] **Step 1: Simplify the `rootPackageName` derivation**

In `packages/web/src/components/canvas/CanvasInner.svelte`, replace the working-tree `rootPackageName` derived (the block added at ~line 164-168) with:

```ts
  // Root package name = the bundle's root index.md H1 (ModelGraph.path). Blank
  // when unnamed; TopBar renders the "Untitled" placeholder for the empty case.
  const rootPackageName = $derived(($model.path ?? "").trim());
```

- [ ] **Step 2: Remove the `modelName` state and the `sharedModelName` import**

Replace the import on line 21:

```ts
  import { isFirstVisit, onStoreError } from "../../state/bootstrap";
```

Delete the `modelName` import line (line 65):

```ts
  import { loadModelName, persistModelName, DEFAULT_MODEL_NAME, templateModelName } from "@waml/core/state/modelName";
```

Delete the state declaration (line 99):

```ts
  let modelName = $state(sharedModelName ?? loadModelName());
```

- [ ] **Step 3: Repoint `imageName`, export title, and share url to `$model.path`**

Replace `imageName` (line 222):

```ts
  const imageName = $derived(($model.path ?? "").trim() || "untitled-package");
```

Replace the `handleExport` title (line 486):

```ts
    const title = ($model.path ?? "").trim() || "untitled-package";
```

Replace the `ShareDialog` `shareUrl` prop (line 689):

```svelte
      shareUrl={buildShareUrl(store.getBundle())}
```

- [ ] **Step 4: Delete the persist-model-name effect and the template/clear seeding**

Delete the effect (lines 272-275):

```ts
  // 4) Persist the model name on change.
  $effect(() => {
    persistModelName(modelName);
  });
```

Delete the `modelName = DEFAULT_MODEL_NAME;` line inside `clearCanvas` (line 497).

Delete `modelName = templateModelName(name);` inside `handleUseTemplate` (line 566) and the `if (mode === "replace") modelName = templateModelName(pendingTemplate.name);` line inside `handleTemplateApplyConfirm` (line 579). Leave the surrounding `applyTemplate(...)` calls intact.

- [ ] **Step 5: Wire `onRenameRoot` on the TopBar usage**

In the `<TopBar ... />` invocation (around line 595-605, where `rootPackageName={rootPackageName}` already is), add:

```svelte
    onRenameRoot={(title) => store.retitlePackage("", title)}
```

- [ ] **Step 6: Delete `modelName.ts`**

```bash
git rm packages/core/src/state/modelName.ts
```

- [ ] **Step 7: Typecheck + run the web tests**

Run: `pnpm --filter @waml/web test && pnpm --filter @waml/web build`
Expected: PASS / clean build — no remaining references to `modelName`, `loadModelName`, `persistModelName`, `DEFAULT_MODEL_NAME`, `templateModelName`, or `sharedModelName` in `CanvasInner.svelte`. (If the build flags `sharedModelName` as still-exported-but-unused, that is fine until Task 8 removes the export.)

- [ ] **Step 8: Commit**

```bash
git add packages/web/src/components/canvas/CanvasInner.svelte
git commit -m "refactor(canvas): read model name from \$model.path; delete modelName.ts"
```

---

## Task 8: Drop the `&n=` share-name param + `sharedModelName` (TypeScript)

Remove the separate share-link name param. The bundle now carries its own name in the root `index.md`, so a recipient reads it directly. Old `&n=` links open as `Untitled` (no back-compat kept).

**Files:**
- Modify: `packages/core/src/share/url.ts` (`NAME_KEY` line 16; `buildShareUrl` lines 62-68; `readSharedName` lines 76-85)
- Modify: `packages/core/src/share/url.test.ts` (import line 3; name tests lines 32-44)
- Modify: `packages/web/src/state/bootstrap.ts` (import line 4; `sharedModelName` export line 22)
- Modify: `packages/web/src/state/bootstrap.test.ts` (assertion line 27)

**Interfaces:**
- Produces: `buildShareUrl(bundle: Bundle): string` (no `name` param); `readSharedName` and `sharedModelName` removed.

- [ ] **Step 1: Update the failing tests first**

In `packages/core/src/share/url.test.ts`, change the import (line 3) to drop `readSharedName`:

```ts
import { encodeModel, decodeModel, buildShareUrl, SHARE_URL_HASH_CEILING } from "./url";
```

Delete the two name tests (`"carries the model name in the link..."` and `"omits the name param..."`, lines 32-44) and replace with a single test asserting the name param is gone:

```ts
  it("does not carry a separate name param (the bundle owns its name)", () => {
    const url = buildShareUrl(bundle);
    expect(url).not.toContain("&n=");
    expect(url).toContain("#m=");
  });
```

In `packages/web/src/state/bootstrap.test.ts`, delete the `expect(mod.sharedModelName).toBeNull();` line (line 27).

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm --filter @waml/core test -- url && pnpm --filter @waml/web test -- bootstrap`
Expected: FAIL — `readSharedName` still exported / `buildShareUrl` still appends `&n=` when passed a name (the url test's import now errors or `mod.sharedModelName` is still defined).

- [ ] **Step 3: Remove the name plumbing from `url.ts`**

In `packages/core/src/share/url.ts`, delete the `NAME_KEY` const (line 16). Replace `buildShareUrl` (lines 62-68):

```ts
/** Full shareable URL for the current page that reopens `bundle`. The bundle
 *  carries its own name (root index.md H1), so no separate name param is needed. */
export function buildShareUrl(bundle: Bundle): string {
  return `${location.origin}${location.pathname}#${HASH_KEY}=${encodeModel(bundle)}`;
}
```

Delete the entire `readSharedName` function (lines 76-85).

- [ ] **Step 4: Remove `sharedModelName` from `bootstrap.ts`**

In `packages/web/src/state/bootstrap.ts`, change the import (line 4) to drop `readSharedName`:

```ts
import { readSharedModel, clearSharedModelFromUrl } from "@waml/core/share/url";
```

Delete the export (line 22):

```ts
export const sharedModelName = readSharedName(); // name carried alongside a shared link, if any
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `pnpm --filter @waml/core test -- url && pnpm --filter @waml/web test -- bootstrap`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add packages/core/src/share/url.ts packages/core/src/share/url.test.ts packages/web/src/state/bootstrap.ts packages/web/src/state/bootstrap.test.ts
git commit -m "refactor(share): drop &n= name param and sharedModelName"
```

---

## Task 9: Full monorepo verification gate

Confirm the whole feature is green end-to-end and no stragglers reference the deleted symbols.

**Files:** none (verification only).

- [ ] **Step 1: Grep for any surviving references to deleted symbols**

Run: `grep -rn "modelName\|readSharedName\|sharedModelName\|templateModelName\|DEFAULT_MODEL_NAME\|loadModelName\|persistModelName\|NAME_KEY" packages/`
Expected: no matches under `packages/` (docs under `docs/` may still mention them — those are historical specs/plans, out of scope; note the unimplemented `docs/superpowers/plans/2026-07-16-new-package-flow.md` still references `DEFAULT_MODEL_NAME` and will need reconciliation when that plan is executed).

- [ ] **Step 2: Run the full gate**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: PASS — all Rust tests, all TS package tests, eslint clean, all packages build.

- [ ] **Step 3: Final commit (only if the gate surfaced fixes)**

```bash
git add -A
git commit -m "chore: root-package-name feature verification pass"
```

---

## Self-Review

**Spec coverage:**
- "Delete `modelName` entirely / everything reads from `$model.path`" → Tasks 7, 8 (state, imageName, export, share, template seeding, `modelName.ts` deletion).
- "New op `pkg.retitle { path, title }` generic over any key" → Tasks 2 (Rust), 3 (DTO+wasm), 4 (ops-adapter), 5 (store). Root = key `""`, wired by TopBar (Task 6) → CanvasInner (Task 7).
- "Reuse `write_package_index`, preserve intro + members, create when absent" → Task 2 (`MemberOrder::Keep`, `title_override`), tested.
- "Empty title rejected" → Task 2 Rust guard; Task 4 adapter short-circuit.
- "Fix `render_index` to be title-aware; fallback to basename; golden tests; byte-identical when no custom title" → Task 1 (both callers, root via `model.path`, reindex + sort golden tests).
- "Root index-file path arithmetic (`""` → `index.md`)" → Task 1 (`write_package_index` idx_path fix — previously produced the buggy `/index.md`).
- "Top-bar subtitle = `$model.path`; `Untitled` placeholder; hover pencil; click/Enter/blur/Esc" → Tasks 6, 7.
- "Drop `&n=`, `readSharedName`, `sharedModelName`, `templateModelName`; no migration" → Tasks 7, 8.
- "Wasm/TS regeneration" → Task 3 (`pnpm build:wasm` + commit generated artifacts + verify union).
- Testing matrix (Rust retitle create/preserve/nested+root/empty; core adapter + store; TopBar; CanvasInner export/image/share) → Tasks 1-8.

**Placeholder scan:** No TBDs; every code step shows the actual code and exact commands.

**Type consistency:** `render_index(dir, title: Option<&str>, description, members)` used identically in Tasks 1-2. `MemberOrder { Explicit, Sort, Keep }` introduced in Task 1 (Explicit/Sort), extended in Task 2 (Keep). `Op::PkgRetitle { path, title }` consistent across mod.rs/pkg.rs/DTO. Wire tag `"pkg.retitle"` consistent in DTO, generated union, `retitlePackageOps`. `retitlePackageOps(key, title)` → `store.retitlePackage(key, title)` → `store.retitlePackage("", title)` at the TopBar call site — names align.

**Decisions made (flagged for reviewer):**
1. **Root title source = `model.path`, not `concept.title`.** `build_packages` pins the root package's `concept.title` to `None` (its title lives in `model.path` = root H1). So `write_package_index` and `reindex_bundle` source the current title from `model.path` for root and `concept.title` for nested packages. Without this, reorder/sort/reindex would reset a named root back to `index`.
2. **`MemberOrder` enum** replaces the previous `order: Option<&[String]>` param. `None` used to mean "sort A–Z"; retitle needs "keep current order", which `None` could not express — hence the explicit `Explicit`/`Sort`/`Keep` tri-state. Reorder/sort semantics are unchanged.
3. **Root always exists in `build_model`** (`build_packages` seeds `dirs` with `""`), so retitling root on an empty/index-less bundle finds the package and creates `index.md` — no separate empty-bundle branch needed.
4. **`Untitled` placeholder lives in `TopBar`**, so `CanvasInner` passes the raw (possibly empty) `$model.path`; the export/image fallback is `"untitled-package"` (no user-facing "model" wording — replaces the old `"model"` / `"model-okf"` / `"waml-model"`).
