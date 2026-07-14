# OKF core / UAML profile — tier refactor (units 3-6)

> **Rigor:** tdd-per-task

## Context

Splitting `crates/uaml`'s single UML-only type-model into a genuine two-tier model: a domain-agnostic **OKF** foundation (`okf::Concept`/`Bundle`, lossless projection of every markdown doc) with the **UAML** tier (`model::Node`) refined on top, plus renaming the family token `uml.*` → `uaml.*`.

**Units 1-2 already landed on origin/main (commit `2f38282`)** — the additive `crate::okf` module (`Concept`, `Bundle`, `Link`, `Citation`, `ConceptRole`, `okf::project`, `okf::build_bundle`) + serde on `Frontmatter`/`FmValue` + `lib.rs` `pub mod okf`. Do NOT redo these; the branch forks off origin/main which already contains them. This plan is the **remaining 4 units** that layer the UAML tier over OKF and wire it out.

### Gate is DELIBERATELY relaxed (user-approved)

The green gate is **`cargo test --workspace && pnpm --filter @uaml/okf build`**. The front-end TS stack — **`packages/web` AND `packages/core`** — is EXCLUDED: both consume the exact wire shape units 4-6 redesign (flat slug keys → nested `concept` + full-path keys), and they are on a separate rewrite track. They may be left **non-compiling on origin/main** until that track catches up. This is intentional; do not re-add `pnpm -r test` / `pnpm build` / `pnpm lint` to the gate.

### Cross-unit gotchas (learned the hard way — read before starting any unit)

- **serde is NOT a default cargo feature.** `cargo test -p uaml` alone compiles WITHOUT serde and silently skips serde_shape + okf serde tests. `cargo test --workspace` turns serde on via `uaml-wasm` feature unification — always gate with `--workspace`.
- **`uml.*` → `uaml.*` must flip ATOMICALLY.** Any fixture/template/test-bundle string left as `type: uml.X` or `profile: uml-domain` after the parser changes becomes `Unknown` and fires the `UnknownType` warning in `parse.rs` `scan_frontmatter_and_preamble` — "clean fixture" tests go red. Grep `uml\.` and `uml-domain` across all Rust crates + tests before committing the rename.
- **The wasm blob is PREBUILT.** `packages/okf/src/generated/wasm-inline.ts` is a committed artifact NOT rebuilt by `pnpm build`. Regenerate it with `node scripts/build-wasm.mjs` (root script `build:wasm`) in the wire unit; otherwise Rust wire changes never reach TS. For units 3-5 the stale blob is fine (okf still type-checks against the current `types.ts`).
- **RTK proxy mangles large tool outputs.** Set `$env:RTK_DISABLE=1` for gate commands and read files in <200-line ranges.

## Target shapes (reference)

`Node` wraps `okf::Concept` (foundation fields live on `concept`), keeping UML-only extension fields:

```rust
pub struct Node {
    pub concept: okf::Concept,   // id/title/description/resource/tags/timestamp/body/links/citations/role/extra
    pub ty: ClassifierType,      // uaml.Metaclass refinement of concept.ty (concept.ty stays free-text String)
    pub stereotypes: Vec<String>,
    pub abstract_: bool,
    pub attributes: Vec<Attribute>,
    pub values: Vec<String>,
    pub note_body: Option<String>,
    pub annotates: Vec<NoteAnchor>,
}
```

`RelationshipKind` stays **closed** (no `Unknown` arm). `okf` must NOT import any UML type — one-way dep (UML → okf) so a later crate split is mechanical.

---

### Task 3: Rename `uml.*` → `uaml.*` (mechanical, atomic)

Rename the family token and profile default everywhere, in one green unit.

- `model.rs`: `ClassifierType::parse` family check `"uml"` → `"uaml"`; `UmlMetaclass::as_str`/parse emit and accept `uaml.{name}`.
- `parse.rs` (~line 524): `Diagram.profile` default `"uml-domain"` → `"uaml-domain"`.
- Flip EVERY `type: uml.X` → `uaml.X` and `profile: uml-domain` → `uaml-domain` string ATOMICALLY: inline fixtures in `crates/uaml/src/**`, all of `crates/uaml/tests/**`, `crates/uaml-wasm/tests/native.rs`, and (for consistency, though gate-excluded) `packages/core/src/templates/*` + `packages/core/src/profiles/umlDomain.ts`. The okf tests (`okf.rs`) use `uml.Class` — free-text so optional, but flip for consistency.
- Verify: grep `uml\.`/`uml-domain` returns nothing meaningful; `cargo test --workspace` green.

**Files:** crates/uaml/src/model.rs, crates/uaml/src/parse.rs, crates/uaml/src/okf.rs, crates/uaml/tests/golden.rs, crates/uaml/tests/ops_golden.rs, crates/uaml/tests/solver_golden.rs, crates/uaml/tests/serde_shape.rs, crates/uaml-wasm/tests/native.rs, packages/core/src/templates/*, packages/core/src/profiles/umlDomain.ts

### Task 4: Refactor `model::Node` to WRAP `okf::Concept`

- Give `Node` a `concept: okf::Concept` field carrying all generic fields; keep only UML-only extension fields alongside it (see Target shapes). `concept.ty` stays free-text `String`; `Node.ty` remains `ClassifierType`.
- `build_model`: project each doc via `okf::project` → `Concept`, then build each UML `Node` by wrapping its `Concept` and re-reading the UML `Section`s for extension fields. **Reconcile the double-parse:** `okf::project` currently re-parses raw markdown (its own frontmatter + regex link/citation extraction) rather than projecting from `syntax::Document`. Have `build_model` source generic fields from the `Concept` (do not re-drop `tags`/`resource`/`timestamp`/citations/links for non-UML docs); keep `okf::project` as the single generic-field source so the two paths can't diverge.
- Non-UML concepts stay in `bundle.concepts` and get NO `Node`. Diagrams stay UML-tier-only.
- Regenerate `serde_shape.rs`, `golden.rs`, `ops_golden.rs`, `solver_golden.rs`, and `uaml-wasm/tests/native.rs` to the new `Node` shape (verify diffs, don't blindly accept). UML semantics behavior-preserving apart from the additive `concept` nesting.

**Files:** crates/uaml/src/model.rs, crates/uaml/src/parse.rs, crates/uaml/tests/golden.rs, crates/uaml/tests/serde_shape.rs, crates/uaml/tests/ops_golden.rs, crates/uaml/tests/solver_golden.rs, crates/uaml-wasm/tests/native.rs

### Task 5: Full-path node keying

- Node key = `concept.id` (full path, e.g. `tables/orders`) instead of the current slug. `parse.rs` `build_node`/`build_model` keyset + `keyset: &HashSet<&str>` become full-path based.
- `validate.rs` `DuplicateSlug` detection keys on full path (keep the `DuplicateSlug` code name unless the rename is trivially clean). Relationship + diagram-member target resolution (`solve/resolve.rs`, `validate.rs` `UnresolvedTarget`) resolve against full-path IDs.
- Decide + implement link-href → id normalization: `id = path minus ".md"`; a link `href` is resolved relative to the referring doc's directory (so `./order.md` from `tables/index.md` → `tables/order`). Add/confirm an `okf::id_of`/`resolve_href` helper and use it consistently in projection, keying, and resolution.
- Regenerate goldens (keys change slug → full path); verify the diffs are exactly the key changes.

**Files:** crates/uaml/src/parse.rs, crates/uaml/src/validate.rs, crates/uaml/src/solve/resolve.rs, crates/uaml/src/okf.rs, crates/uaml/tests/golden.rs, crates/uaml/tests/ops_golden.rs, crates/uaml/tests/solver_golden.rs, crates/uaml/tests/serde_shape.rs

### Task 6: Wire — wasm `build_bundle` + regenerate TS `types.ts` + wasm blob

- `crates/uaml-wasm/src/lib.rs`: add `build_bundle` (and native `build_bundle_json` mirroring `build_model_json`). `build_model` output now carries the `Node`-wraps-`Concept` shape.
- Regenerate the prebuilt wasm blob: run `node scripts/build-wasm.mjs`, refreshing `packages/okf/src/generated/wasm-inline.ts` and the wasm glue.
- Regenerate `packages/okf/src/types.ts`: `Concept`/`Link`/`Citation`/`ConceptRole`/`Bundle` + nested-`concept` `Node`. Drop legacy `position`/`members`/`hints`/`display` (front-end re-derives on its own track).
- Update `packages/okf/src/wasm/smoke.test.ts` to the new shape if it asserts the old wire.
- Gate: `pnpm --filter @uaml/okf build` must compile the regenerated types against the fresh blob.

**Files:** crates/uaml-wasm/src/lib.rs, packages/okf/src/types.ts, packages/okf/src/generated/wasm-inline.ts, packages/okf/src/wasm/smoke.test.ts

## Verification

- Each unit: `cargo test --workspace && pnpm --filter @uaml/okf build` green before commit.
- **Headline (already provable via okf tier, re-confirm after Task 4):** an OKF `Playbook` (non-`uaml.*` type with `tags`/`resource`/`timestamp`/citations/links) → `build_bundle` → every field survives on the `Concept`; and via `build_model` a non-UML doc gets NO `Node` but keeps its `Concept` intact in the bundle.
- Round-trip: `fmt`/`apply_ops` on a mixed (uaml + non-uaml) bundle stays byte-lossless (tiers are read-only projections over `Document`).
- `serde_shape.rs` regenerated to the new `Node`/`Concept` field names; goldens reflect full-path keys and `uaml.*` tokens.
- `packages/web` / `packages/core` are NOT verified here (separate rewrite track).
