# OKF migrate step 1 — repoint `description` readers flat → `concept.description`

> **Rigor:** tdd-per-task

## Context

Second phase of the OKF two-tier `Node` un-flattening: **migrate** (expand already
landed — `864ce6a` — so every `ModelNode` now carries BOTH the flat `description` field
and the nested `concept.description`, identical values).

This is the FIRST migrate unit and deliberately the safest possible one: `description`
is the ONE flat field whose value is byte-identical to its `concept.*` counterpart
(both are exactly the frontmatter `description`, an `Option<String>`). Other flat fields
are NOT equal and are explicitly out of scope:
- `title` — flat = `fm.title ?? H1 ?? "Untitled"`; `concept.title` = fm only (fallback would regress)
- `body` — flat = the Section body; `concept.body` = the full verbatim markdown
- `ty`/`type` — flat = parsed `ClassifierType` enum; `concept.ty` = free-text `String`

So this unit moves ONLY the `description` **reads** of stored `ModelNode` state onto
`concept.description`. It establishes the migrate pattern with zero semantic risk.

**Out of scope (do NOT do):** migrating title/body/ty; deleting the flat `description`
field (that is a later CONTRACT step, once every reader has moved); changing the write
path / patch shape / ops-adapter op emission; full-path keying; the `uml.*` rename.

### Gate — FULL, must stay green

`cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`

Do NOT relax it. This is a TS/Svelte-only change (no Rust edit), but the whole tree is gated.

### Scope facts (verified — do not re-derive)

The only two readers of stored `ModelNode.description` are:
- `packages/web/src/components/inspector/ObjectInspector.svelte:29` — `value={node.description ?? ""}` (display).
- `packages/core/src/state/ops-adapter.ts:140` — `prev.description` in the "did description change?" compare inside `updateNodeOps`.

Everything else matching `.description` is unrelated: template `t.description`
(Welcome/LibraryDialog), attribute `a.description` (AttributeEditor / ops-adapter:121,
which is Attribute, NOT Node), and generated `uaml_wasm.js`. Leave all of those alone.

The mutation API is unchanged: `updateNode(key, { description })` still takes a flat
`Partial<ModelNode>` patch; `ops-adapter` still compares `patch.description` (the write
intent) — only the STORED-state read `prev.description` moves to `prev.concept.description`.
Both sides remain the same frontmatter value, so the compare result is identical.

---

### Task 1: Move the two `ModelNode.description` reads onto `concept.description`

- **`packages/web/src/components/inspector/ObjectInspector.svelte`** (~line 29): change the
  display read `node.description ?? ""` → `node.concept.description ?? ""`. Do NOT touch the
  `onUpdate({ description: ... })` write — the patch stays flat.
- **`packages/core/src/state/ops-adapter.ts`** (~line 140): change the stored-state read
  `prev.description` → `prev.concept.description` in the `updateNodeOps` compare. Keep
  `patch.description` (write intent) as-is; keep the emitted `desc` op unchanged. The
  attribute-description code at line 121 (`f.description`) is UNRELATED — do not touch it.
- Keep the flat `description` field on `ModelNode` in place (no contract this unit).
- Update any test that asserts on these two readers so it exercises the `concept.description`
  source (e.g. `ObjectInspector.test.ts`, and any `ops-adapter` test seeding a `prev` node —
  the `prev` fixture must carry `concept.description`, which the real `build_model` output
  already provides). VERIFY each test still asserts the SAME observable behavior (the
  inspector shows the same text; the same `desc` op is/ isn't emitted) — the source field is
  the only thing that changed.
- **Behavior check (must hold):** open the inspector on a node with a frontmatter
  `description` → the field shows it (now sourced from `concept.description`); edit it →
  a single `desc` op is emitted exactly as before; a node with no description → empty field,
  no spurious op.

**Files:** packages/web/src/components/inspector/ObjectInspector.svelte, packages/core/src/state/ops-adapter.ts, packages/web/src/components/inspector/ObjectInspector.test.ts, packages/core/src/state/ops-adapter.test.ts
