# UAML → WASM frontend: design spec

**Status:** design locked, staged for implementation. Stage 1 has a plan
(`docs/superpowers/plans/2026-07-11-uaml-wasm-ops-editing.md`); later stages are
roadmapped here and get their own plans when reached.

## Problem

Parser / model / editing / layout logic exists **twice**: canonical Rust in
`crates/uaml` (parse, model, `ops`, serialize, validate) and a hand-maintained
TypeScript port in `packages/okf` (`parse.ts`, `serialize.ts`, `grammar.ts`,
`migrate.ts`, `slug.ts`) plus editing/layout logic in `packages/core` and
`packages/web`. Every change must be written twice and kept in sync. `@uaml/okf`
is imported by 60+ sites across `core` + `web`.

## Goal

The frontend calls the Rust crate compiled to **WASM** for parsing, model building,
validation, **editing (ops)**, and eventually **all diagram layout** — retiring the
TS reimplementation. The web app keeps only **rendering** (drawing). One source of
truth: Rust.

## Locked architectural decisions

### 1. Editing = ops on the AST; `Model` is a derived view

- **Source of truth for editing = the markdown bundle + per-file AST.** Editing is
  **ops-based** (`uaml::ops::apply(bundle, ops)`), which already parses each target
  doc to a `Document` AST, mutates it, and re-serializes.
- The resolved graph **`Model` (nodes/edges/diagrams) is a derived, read-only view**
  (`build_model`) for rendering/queries. We do **not** build a `Model → bundle`
  regenerator (that layer is lossy).
- **Roundtrip guarantee:** `serialize(apply(parse(x)))` is **canonical-idempotent** —
  `serialize_document` *is* `fmt`, so the first save may reformat, but re-serializing
  is stable. **Not** byte-exact (intentional; it's a formatter).
- **The CLI (`crates/uaml-cli`) is left unchanged** — it keeps its minimal-diff
  guarantee (only rewrites files an op touched; untouched files byte-identical;
  `index.md`/non-node files passed through). It already calls `ops::apply`.

Why not edit the resolved `Model` and regenerate files: `build_model` is lossy —
it drops `index.md`/non-node files, unknown `## Sections`, most frontmatter, merges
reciprocal `associates` into one `Edge` (no stable edge id), and resolves/degrades
refs. Reconstructing files from it loses data. The AST (`Document`/`Section`) is
already lossless (keeps `Unknown`, `Notes`, full ordered frontmatter), so editing
there is roundtrip-safe by construction.

### 2. The canvas ↔ model divergence and the (interim) overlay

The Rust/OKF model and the TS canvas model differ in load-bearing ways:

| Concern | Rust / OKF | TS canvas (`packages/okf/src/types.ts`) |
|---|---|---|
| node identity | slug (`Node.key`) | synthetic `n#` (`data.owox.key` persists it) |
| node position | per-diagram `Member.position` (`at x,y`) | node-level `position {x,y}` |
| edge identity | none (synthesized from `## Relationships`) | synthetic `e#` (`ModelEdge.id`) |
| edge handles | absent — "not encoded in OKF" | `sourceHandle` / `targetHandle` |
| display | `RenderHints{emphasize,collapse}` only | also `DiagramDisplay{showAttributes,…}` |

Positions **do** round-trip via diagram-member `at x,y` and `data.owox.position`
frontmatter; handles and synthetic ids do not.

- **Interim (stage 1):** a thin **TS overlay** keyed by slug holds canvas-only data
  (position, synthetic `n#`/`e#`, handles). Semantic edits go through WASM ops;
  pure-canvas edits (drag, handle) touch only the overlay. `toModelGraph(model,
  overlay)` assembles the xyflow-shaped graph the UI expects.
- **End state (stage 3):** fold canvas concerns into the model/format (positions via
  diagram members; **encode edge-handle side-overrides in the format**; drop
  synthetic ids for slug keys) and delete the overlay → true single source of truth.

### 3. serde on the core crate (feature-gated)

`crates/uaml` has zero serde today. Add optional `serde` + `[features] serde`, and
`#[cfg_attr(feature="serde", derive(Serialize, Deserialize))]` on the ~20 types
reachable from `Model` and `Diagnostic`:
`Model, Node, Edge, Diagram, Member, RenderHints, Attribute, TypeRef, RelEnd,
AssocName, NoteAnchor, ClassifierType, UmlMetaclass, RelationshipKind, Visibility,
Multiplicity, Diagnostic, Severity, DiagCode`.

**Pin serde renames to the TS `packages/okf/src/types.ts` field names** so the JS
side needs no remapping: `ty → type`, `abstract_ → abstract`, `Edge.source/target →
from/to`, flat-string for `ClassifierType`/`RelationshipKind`, untagged unions for
`AssocName`/`NoteAnchor`, `Multiplicity` via `try_from="String"` (re-validate on
deserialize). A `bundle → build_model → serialize → equal` fixture test guards drift.

### 4. Ops wire contract — shared `OpDto`

`crates/uaml-cli/src/ops_dto.rs` already defines `OpDto` — an internally-tagged
(`#[serde(tag="op")]`) enum mirroring `ops::Op`, with `rename="abstract"` etc. and a
round-trip test suite. Move it to a shared crate `crates/uaml-ops-dto` so both the
CLI and the WASM layer use one wire format. CLI re-exports it (otherwise unchanged).

### 5. WASM inlining — no runtime fetch

`wasm-pack build --target web` emits an ES module whose `init(input)` accepts
`BufferSource`. Base64-embed the `.wasm` bytes into a generated TS module and call
`init(bytes)` → **fully inlined, no separate `.wasm` request**. No `vite-plugin-wasm`
needed. Async `initWasm()` runs once and is awaited before the app mounts.

### 6. de-OWOX / licensing

This de-fork feeds the planned Apache-2.0 → AGPL-3.0 relicense (removing
OWOX-derived core). Constraints:
- **No OWOX branding in any new Rust/WASM output.** The OWOX watermark/logo in
  `packages/core/src/share/exportImage.ts` is **not** carried into a future CLI
  renderer.
- The `owox.key` / `owox.position` frontmatter fields (read by
  `packages/okf/src/parse.ts:68,116,148,188`) get renamed as part of this work.
  `build_model` must read node identity + position from these (renamed) fields so the
  render view and positions survive.

## Layout engine (stage 2 — summary)

Move all diagram **layout** (geometry) into Rust so the web UI and a future CLI
`render → svg/png` share one engine; web keeps only rendering.

- **No text metrics anywhere today** — node width is a fixed constant, height is
  row-count driven (`packages/core/src/canvas/layoutSize.ts`). Fully headless-portable.
- Port targets: `erdAwareNodeSize` (sizing, trivial); `runDagreLayout` (dagre config
  `rankdir:LR, nodesep:60, ranksep:150` → **a Rust layered/Sugiyama layout — the crux,
  start with a crate-vs-handroll spike**); `planPlacements` (edge side + fan-out slot,
  `packages/web/src/components/canvas/edges.ts`); `floating.ts` port geometry.
- New crate `crates/uaml-layout` (headless, no browser deps). Output contract
  `LaidOutDiagram { nodes:[{key,x,y,width,height,collapsed}], edges:[{id,source,
  target,source_side,target_side,source_slot,target_slot,points}], bounds }`, serde.
- Rust model must gain `DiagramDisplay` (`showAttributes` drives sizing).
- dagre parity is **semantic, not pixel-exact** (web adopts Rust output).
- Debug harness `uaml layout <dir> [--json|--svg]` exercises it standalone and seeds
  the future CLI renderer.

## Roadmap (staged; each stage ships independently)

1. **WASM ops-editing in the frontend** — plan:
   `docs/superpowers/plans/2026-07-11-uaml-wasm-ops-editing.md`. serde feature,
   `uaml-ops-dto` crate, `uaml-wasm` crate (`build_model`/`validate`/`apply_ops`/
   `fmt`/`split_bundle`), inline build, frontend rewire (ops + overlay), retire TS
   editing/parse/serialize bodies. Layout stays on dagre. **← current**
2. **Rust layout engine** — `crates/uaml-layout` standalone, then swap web off dagre.
3. **Single source of truth** — fold canvas data into model/format, encode
   edge-handle overrides, drop the overlay, retire remaining `@uaml/okf`.
4. **Format-preserving edits** — lossless CST + byte-span in-place patching replacing
   `serialize_document` as the edit path; CLI + WASM inherit.

## Open decisions (recorded; resolve at the stage that needs them)

- **Render WASM delivery** (Render's static build has no Rust toolchain): (a) commit a
  generated base64 artifact + CI staleness check, or (b) install rustup+wasm-pack in
  `render.yaml`. Keep `scripts/build-wasm.mjs` idempotent so both stay viable. Lean: (a).
- **Package layout:** replace `@uaml/okf` internals in place (zero import churn) vs a
  new `@uaml/wasm` package + shim. Lean: in-place.

## Non-goals

- Byte-exact file preservation (canonical `fmt` normalizes) — until stage 4.
- `Model → bundle` regenerator — never (editing is ops-on-AST).
- Porting rendering (xyflow/HTML drawing) to Rust — stays in web.
- Pixel-exact dagre parity.
