# OKF Node un-flatten — enrich, migrate readers, contract

> **Rigor:** tdd-per-task

## Context

`model::Node` (Rust, `crates/uaml/src/model.rs`) and `ModelNode` (TS, `packages/okf/src/types.ts`)
currently carry BOTH flat fields (`title`, `description`, `body`) AND a nested `concept: okf::Concept`
that duplicates them. This plan removes the flat/nested duplication: enrich the tier so `concept.*`
is the single authoritative source, migrate every reader off the flat fields onto `concept.*`, then
delete the flat fields and reshape the write path.

**End-state `Node` shape** (target, plan-of-record):
`{ concept, key, ty, stereotypes, abstract_, attributes, values, note_body, annotates, members }`
— flat `title`/`description`/`body` DELETED. `ty`/`stereotypes`/`abstract_`/`attributes`/`values`/
`members` STAY on Node (UML-tier refinements, NOT migrated). Only `title`/`description`/`body` move.

### Locked decisions

1. **Title fallback → enrich tier.** Flat `title = fm.title ?? first-H1 ?? "Untitled"` (parse.rs
   `build_node`). `concept.title` is currently frontmatter-only (`Option`). Enrich `okf::project` so
   `concept.title = fm.title ?? first-H1` — it becomes the single resolved source. The `"Untitled"`
   literal is presentation: readers render `concept.title ?? "Untitled"`. Only after this can title
   readers migrate to `concept.title` without regressing the H1 fallback.
2. **body → note_body, kept distinct.** Flat `body` = `Section::Body` prose only. Rename to a
   uml.Note-specific `note_body: Option<String>` (byte-identical value). The sole reader
   (`UmlNoteNode`) reads `note_body`. Generic full-verbatim body stays on `concept.body`.
3. **ty NOT migrated.** `node.type`/`node.ty` readers stay flat — `ty: ClassifierType` refines
   `concept.ty` (free-text String) and is kept on Node.

### Gate (FULL, never relax)

`cargo test --workspace && pnpm -r test && pnpm lint && pnpm build` green before every commit.
Baseline confirmed green on `origin/main` @ `6052939`.

### Gotchas (read before any unit)

- **serde is NOT a default cargo feature** — always `cargo test --workspace` (turns it on via
  `uaml-wasm` feature unification); `cargo test -p uaml` silently skips serde_shape/okf serde tests.
- **wasm blob is PREBUILT** at `packages/wasm/src/generated/wasm-inline.ts` (moved there in the
  recent @uaml/okf→@uaml/wasm restructure). Regenerate with `node scripts/build-wasm.mjs` (root
  `build:wasm`) whenever the Rust wire shape OR wire VALUES change. `pnpm build` does NOT rebuild it.
- **serde/golden gap:** `serde_shape.rs` + `golden.rs` pin Node JSON field-by-field but historically
  did NOT assert `concept` (only `uaml-wasm/tests/native.rs` headline covers it). When contracting,
  extend these goldens so the delete-flat-field diff is gold-locked.
- **RTK_DISABLE=1** on all gate + git commands (RTK mangles large output).
- Read/write asymmetry is intentional mid-migration: reads move to `concept.*` while writes stay flat
  until contract. Safe ONLY because `build_model` keeps both byte-identical and stored Nodes always
  come from a fresh `build_model` derive (no optimistic flat-only mutation).

---

### Task 1: Enrich tier — resolve `concept.title`, add `note_body` (Rust, additive, FIRST)

Additive only; NO flat field dropped, NO reader changed. After this, `concept.title` carries the
H1-resolved title and `note_body` mirrors flat `body`, so readers can migrate.

- `crates/uaml/src/okf.rs` `project`: resolve `title = frontmatter title ?? first level-1 heading
  (H1)` instead of frontmatter-only. Stays `Option` — `None` only when neither present. Do NOT bake
  in the `"Untitled"` literal (that is render-side).
- `crates/uaml/src/model.rs`: add `note_body: Option<String>` to `Node` (serde `default`,
  `skip_serializing_if = "Option::is_none"`).
- `crates/uaml/src/parse.rs` `build_node`: populate `note_body` from `Section::Body` (same source the
  flat `body` reads today — byte-identical). Leave flat `title`/`description`/`body` untouched.
- `packages/okf/src/types.ts`: add `note_body?: string` to `ModelNode`.
- Regenerate wasm blob: `node scripts/build-wasm.mjs` (concept.title values change + new field).
- Regenerate goldens (`golden.rs`, `serde_shape.rs`, `ops_golden.rs`, `solver_golden.rs`,
  `uaml-wasm/tests/native.rs`, `okf.rs` tests) — verify diffs are exactly the H1-title change +
  additive `note_body`; do NOT blindly accept.

**Files:** crates/uaml/src/okf.rs, crates/uaml/src/model.rs, crates/uaml/src/parse.rs, crates/uaml/tests/golden.rs, crates/uaml/tests/serde_shape.rs, crates/uaml/tests/ops_golden.rs, crates/uaml/tests/solver_golden.rs, crates/uaml-wasm/tests/native.rs, packages/okf/src/types.ts, packages/wasm/src/generated/wasm-inline.ts

### Task 2: Migrate title + body readers onto `concept.*` / `note_body` (front-end)

Move every PRODUCTION read of flat `.title` / `.body` off a Node onto `concept.title` (with a
render-side `?? "Untitled"` where the flat read had a default) and `note_body`. Reads only — the
write path (ObjectInspector `onUpdate({ title })`, ops-adapter) STAYS flat until Task 3.
`toRFNode.ts:17` spreads the whole ModelNode into flow `data`, so `data.concept`/`data.note_body`
are already present at runtime; extend the `OkfNodeData` type if svelte-check needs it.

Title reads to migrate (`.title` → `.concept.title ?? <existing default>`):
- `CanvasInner.svelte:128` (`n.title.trim() || "Untitled"`)
- `LibraryDialog.svelte:71` (`title={n.title}`), `:80`/`:81` (`?.title ?? e.from/e.to`)
- `ClassifierBox.svelte:46` (`{data.title}`)
- `ExternalRefs.svelte:21`/`:23` (`.title`)
- `Inspector.svelte:40` (`selectedNode.title.trim() || "Untitled"`)
- `OkfNode.svelte:16` (`{node.title}`)
- `ObjectInspector.svelte:23` (input `value={node.title}` → `value={node.concept.title ?? ""}`;
  leave the `oninput`/`onUpdate({ title })` WRITE unchanged)
- `RelationshipInspector.svelte:28`/`:29` (`?.title ?? "Source"/"Target"`)

Body read to migrate:
- `UmlNoteNode.svelte:18` (`data.body ?? data.title` → `data.note_body ?? data.concept.title`)

Do NOT touch: `TopBar.svelte:58`/`:182` (Diagram `.title`, not a Node); tests reading flat `.title`
(they stay green — flat fields still populated until Task 3).

**Files:** packages/web/src/components/canvas/CanvasInner.svelte, packages/web/src/components/LibraryDialog.svelte, packages/web/src/components/canvas/nodes/ClassifierBox.svelte, packages/web/src/components/canvas/nodes/OkfNode.svelte, packages/web/src/components/canvas/nodes/UmlNoteNode.svelte, packages/web/src/components/inspector/ExternalRefs.svelte, packages/web/src/components/inspector/Inspector.svelte, packages/web/src/components/inspector/ObjectInspector.svelte, packages/web/src/components/inspector/RelationshipInspector.svelte, packages/web/src/components/canvas/toRFNode.ts


### Task 3 (LAST): Contract — delete flat fields, reshape write path, gold-lock (serial)

Run ONLY after Task 2 — no reader touches flat `title`/`description`/`body`.

- `crates/uaml/src/model.rs`: delete flat `title`, `description`, `body` from `Node` (keep
  `note_body`, `concept`, and the UML-tier fields).
- `crates/uaml/src/parse.rs` `build_node`: stop populating the deleted fields (the H1/`Section::Body`
  resolution now lives only in `concept.title`/`concept.body`/`note_body`).
- `packages/okf/src/types.ts`: delete flat `title`/`description`/`body` from `ModelNode`.
- Reshape the write path so title/description edits route to the concept-backed source instead of the
  deleted flat fields: `packages/core/src/state/ops-adapter.ts` `nodeNewOps` (line ~119 `title:
  f.title`, ~121 `desc: f.description`), `nodeSetOps` (~139 `set.title`, ~140 `set.desc`); and
  `packages/core/src/state/model.ts` `updateNode(key, patch: Partial<ModelNode>)` (~118) — `patch`
  can no longer name flat `title`/`description`. (These ops already emit doc-level `title`/`desc`
  frontmatter mutations that `build_model` re-derives into `concept.*`; confirm they stay correct
  once the flat mirror is gone.)
- Update tests that assert flat fields: `packages/core/src/state/overlay.test.ts:153`
  (`n.description`), `packages/web/src/components/canvas/Canvas.test.ts:85`/`:94`,
  `packages/web/src/state/model.svelte.test.ts:20`/`:24`/`:28`,
  `packages/core/src/state/model.test.ts:54`, `packages/core/src/state/ops-adapter.test.ts:51`/`:55`
  → read via `concept.*`.
- Regenerate wasm blob (`node scripts/build-wasm.mjs`).
- Extend/regenerate ALL Rust goldens (`golden.rs`, `serde_shape.rs`, `ops_golden.rs`,
  `solver_golden.rs`, `uaml-wasm/tests/native.rs`) to gold-lock the concept-only Node shape — assert
  `concept` explicitly so the delete-flat-field diff is locked (closes the serde/golden `concept` gap).
- Full gate green.

**Files:** crates/uaml/src/model.rs, crates/uaml/src/parse.rs, packages/okf/src/types.ts, packages/core/src/state/ops-adapter.ts, packages/core/src/state/model.ts, packages/core/src/state/overlay.test.ts, packages/core/src/state/model.test.ts, packages/core/src/state/ops-adapter.test.ts, packages/web/src/components/canvas/Canvas.test.ts, packages/web/src/state/model.svelte.test.ts, crates/uaml/tests/golden.rs, crates/uaml/tests/serde_shape.rs, crates/uaml/tests/ops_golden.rs, crates/uaml/tests/solver_golden.rs, crates/uaml-wasm/tests/native.rs, packages/wasm/src/generated/wasm-inline.ts

## Verification

- Each unit: full gate green before commit; workflow rebases onto `origin/main` + re-gates + ff-pushes.
- Title readers preserve the H1/`"Untitled"` fallback — a node whose title comes only from its H1
  still renders that title, never blank.
- Contract only after no reader touches a flat field; goldens gold-lock `concept`; write path reshaped.
