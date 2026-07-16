# Rename the internal "model" graph type (stub spec)

**Date:** 2026-07-16
**Status:** Draft / stub — deferred. Do not implement yet. Sibling of
`2026-07-16-root-package-name-design.md`, which purges the *user-facing* "model"
concept. This one purges the remaining *internal* "model" naming.

## Why

After the root-package-name work, the only user-facing concept is the package.
But the codebase still names the parsed-bundle graph "model" everywhere. Goal:
no "model" vocabulary left in the code either — the top-level parsed structure
gets a package/graph-centric name.

## Scope (identifiers to rename)

The "model" here means **the graph derived from a bundle** (nodes + edges +
diagrams + packages + flows + interactions + path) — NOT a single package.

- Rust (`crates/waml`): `struct Model`, `fn build_model`, `Model::node`, the
  wasm export `build_model`.
- TS types (`packages/okf`): `ModelGraph`, `ModelNode`, `ModelEdge`.
- Core store (`packages/core/src/state/model.ts`): `createModelStore`, the
  exported `model` store, `model.svelte.ts`, the `$model` usages across web.
- Any `RustModel` alias in `overlay.ts`.

Out of scope: the `Bundle` (`[path, markdown][]`) type — that name is already
correct and stays.

## The one open decision: target name

The container-of-everything is not a package, so it can't just become
`Package`. Candidates:

- **`Graph` / `build_graph` / `GraphNode` / `GraphEdge` / `createGraphStore`
  (recommended).** Shortest, and `ModelGraph` is already half-named this. Reads
  well: the parsed graph of a bundle.
- `BundleGraph` — more explicit about what it's a graph *of*; pairs with
  `Bundle`. Verbose.
- `Workspace` — frames it as the whole working set. Broader connotation than a
  pure graph.

Recommendation: `Graph`. Resolve this before implementing.

## Approach

Mechanical, type-driven rename — one identifier at a time, leaning on the
compiler/tsc to find every site. No behavior change. Land after the
root-package-name feature so the two don't fight over the same files. Gate:
`cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`.

## Not doing

- No semantic/structural change to the graph.
- No `Bundle` rename.
- No user-facing string changes (those belong to the sibling spec).
