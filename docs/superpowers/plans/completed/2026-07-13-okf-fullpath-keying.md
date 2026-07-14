# OKF/UAML — full-path node keying (tier-refactor Task 5, standalone track)

> **Rigor:** tdd-per-task

## Context

Last deferred piece of the okf/uaml tier refactor (`docs/superpowers/plans/2026-07-12-okf-uaml-tier-refactor.md`
Task 5). Units 1-6 of that plan (okf tier, `uml.*`→`uaml.*`, `Node` wraps `Concept`, wasm wire, package
forest/index.md, package ops) are landed on `origin/main`. Node/edge/diagram keys are still the bare
filename slug (`doc_slug`/`validate.rs`'s own `slug_of`, both: last path segment minus `.md`). Two docs
with the same basename in different directories (`tables/order.md`, `shop/order.md`) collide on one key.

`okf::id_of` (`crates/uaml/src/okf.rs:145`) already computes the target shape: full bundle-relative path,
backslashes normalized, `.md` stripped (`"tables/orders.md"` → `"tables/orders"`). This plan switches
node/edge/diagram keying from `doc_slug` to `okf::id_of`, and teaches target resolution (relationships,
diagram members, attribute type refs) to resolve a written href against the *referring* document's
directory instead of a bare basename lookup.

**Confirmed by investigation (do not re-derive):**
- Grammar regexes (`grammar.rs` `LINK_RE`/`REL_RE`/`MEMBER_RE`) already only accept single-segment
  `./slug.md` hrefs, then run every capture through `grammar.rs::basename()` which strips any directory
  prefix down to the bare stem. This basename-stripping is what has to go — the href capture itself
  doesn't need to change, only what happens to it after capture.
- `Link`/`Citation.href` (`okf.rs`) are stored raw/unresolved from the body regex — untouched by this
  plan (only relationship/attribute-ref/diagram-member targets are in scope; they're the ones that
  become node keys).
- `validate.rs` has its own copy-pasted `slug_of` (`validate.rs:9`), independent of `parse::doc_slug` and
  `okf::id_of`. This drift is exactly the kind of divergence unit 4's Files list warned about — collapse
  it onto one full-path id source.
- No dir-relative-join / `..`-normalize logic exists anywhere in the crate today.

## Target behavior

- Node/edge/diagram `key` = `okf::id_of(&doc.path)` (the *referring* doc's own key — trivial, no href
  resolution needed, `path` is already the full bundle path).
- A relationship/attribute-ref/diagram-member target written as `./x.md` in doc at path `tables/index.md`
  resolves against `tables/` (the referring doc's directory) → id `tables/x`. A `../` or nested
  `./sub/x.md` href resolves the same way (directory join + `.`/`..` normalize), even though today's
  fixtures only ever write single-segment hrefs — the resolver should not assume single-segment.
- `DuplicateSlug` (keep the diagnostic code name — trivial rename not worth the diff) fires only when two
  docs project to the *same full id*, not the same basename.
- `UnresolvedTarget` fires when a resolved href doesn't match any doc's full id.

## Cross-task gotchas

- serde is NOT a default cargo feature — gate with `cargo test --workspace`, not `-p uaml`.
- `RTK_DISABLE=1` on all gate + git commands; read files in <200-line ranges.
- Gate for this track (matches unit 3-6 relaxed gate): `cargo test --workspace && pnpm --filter @uaml/okf build`.
  `packages/web`/`packages/core` stay excluded — separate rewrite track, may be non-compiling.
- Regenerate goldens by inspecting diffs, never blind-accept — a diff that touches anything beyond
  slug→full-path key strings is a bug, not an expected regen.

---

### Task 1: `okf::resolve_href` — directory-relative href resolver

Add a pure function next to `id_of` that turns a written href into a full bundle-relative id, resolved
against the referring document's directory.

- `pub fn resolve_href(referring_path: &str, href: &str) -> String` in `crates/uaml/src/okf.rs`. Strip a
  leading `./`, join against `referring_path`'s parent directory, normalize `..` segments, normalize `\`
  to `/`, strip `.md` (reuse `id_of` for the final strip so the two stay in lockstep).
- Cover with unit tests colocated in `okf.rs`: same-dir link (`tables/index.md` + `./orders.md` →
  `tables/orders`), root-level referring doc with no directory prefix (`readme.md` + `./x.md` → `x`),
  nested multi-segment href (`tables/index.md` + `./sub/x.md` → `tables/sub/x`), and a `../` case
  (`tables/orders.md` + `../shop/order.md` → `shop/order`).
- No callers yet — this task is additive and independently testable.

**Files:** crates/uaml/src/okf.rs

### Task 2: `parse.rs` — full-path node/edge/diagram keying + target resolution

- Compute each `ParsedDoc`'s own key via `okf::id_of(&p.path)` (add a field, e.g. `id: String`, set in
  `parse_bundle`). `build_model`'s `keyset` becomes `HashSet<&str>` over full ids, not `p.slug`.
- `build_node` (`Node.key`), `build_edges` (`from`), `resolve_group`/`build_diagrams` (`Diagram.key`),
  and the package `docs` list (`build_model` ~601) all switch from `p.slug` to the new full-path id.
- Target resolution — `resolve_attr`'s `ty.ref_`, `build_edges`' `r.target_slug`, `resolve_group`'s
  `m.slug` — are currently bare stems captured by `grammar.rs::basename()`. Stop basename-stripping in
  `grammar.rs` (keep the raw captured href, or capture it separately) and resolve each target at the
  `parse.rs` call site via `okf::resolve_href(&referring_doc.path, &raw_href)` against the *referring*
  document's own path, then look the result up in the full-id `keyset`. `resolve_attr`/`build_edges`/
  `resolve_group` all need the referring doc's `path` threaded in alongside `keyset` (they currently only
  take `keyset: &HashSet<&str>`).
- `doc_slug` may stay for any remaining non-keying uses (check callers before deleting); if it has no
  callers left after this task, delete it rather than leave dead code.

**Files:** crates/uaml/src/parse.rs, crates/uaml/src/grammar.rs

### Task 3: `validate.rs` — `DuplicateSlug` + `UnresolvedTarget` on full path

- Delete `validate.rs`'s standalone `slug_of` (`validate.rs:9`); source the id the same way `parse.rs`
  now does — `okf::id_of(path)` for a doc's own key.
- `DuplicateSlug` detection (`validate.rs` `link()`, ~130-149): key `keyset`/`slug_count` on full id.
  Regression to prove: `tables/order.md` and `shop/order.md` no longer collide.
- `UnresolvedTarget` for relationships (~164-181) and diagram members (`check_group_members`, ~98-122):
  resolve each raw target href via `okf::resolve_href` against the referring doc's path (same approach as
  Task 2) before checking membership in the full-id keyset.
- Regression to prove: a link `./order.md` written in `tables/index.md` resolves to `tables/order`, not
  to a same-named doc elsewhere in the bundle.

**Files:** crates/uaml/src/validate.rs

### Task 4: `solve/resolve.rs` — target resolution on full id

- `Builder.node_keys: BTreeSet<String>` and `BoxId::Node(String)` switch to full-id keys, populated from
  `DiagramGroup.members` (which, after Task 2, already carry full ids — this task is mostly propagation +
  rename, not new resolution logic).
- `resolve_ref`'s `NameRef::Link{slug,..}` arm looks up the (now full-id) `slug` directly in `node_keys`
  — should fall out once the upstream `DiagramGroup` member ids are full-path.
- `NameRef::Bare(name)` arm re-derives a key via `crate::slug::slugify(name, "")` — this is a
  same-diagram informal name reference, not an href, and is out of scope for href-resolution semantics;
  decide (and note inline why) whether it now needs the diagram's own directory prefixed to stay
  resolvable against the full-id keyset, and implement whichever keeps existing diagrams working.

**Files:** crates/uaml/src/solve/resolve.rs

### Task 5: Regenerate goldens

- Regenerate `golden.rs`, `ops_golden.rs`, `solver_golden.rs`, `serde_shape.rs`, and
  `crates/uaml-wasm/tests/native.rs` against the new full-path keys.
- Inspect every diff by hand: only slug→full-path key strings (and the `DuplicateSlug`/`UnresolvedTarget`
  fixtures/messages if they embed a key) should change. Any structural or unrelated diff is a bug
  introduced in Tasks 1-4, not an expected regen — stop and fix the root cause rather than accepting it.

**Files:** crates/uaml/tests/golden.rs, crates/uaml/tests/ops_golden.rs, crates/uaml/tests/solver_golden.rs, crates/uaml/tests/serde_shape.rs, crates/uaml-wasm/tests/native.rs

## Verification

- Each task: `cargo test --workspace && pnpm --filter @uaml/okf build` green before commit.
- Headline: two docs with identical basename in different directories (`tables/order.md`, `shop/order.md`)
  get distinct keys and neither trips `DuplicateSlug`.
- A relationship/diagram-member link `./order.md` written in `tables/index.md` resolves to `tables/order`,
  not to a same-named doc elsewhere in the bundle.
- Goldens reflect full-path keys; diffs reviewed, not blind-accepted.
- `packages/web` / `packages/core` are NOT verified here (separate rewrite track, gate excludes them).
