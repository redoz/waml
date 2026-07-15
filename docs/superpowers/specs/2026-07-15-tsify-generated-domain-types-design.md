# Tsify-generated domain types — design

## Goal

Eliminate hand-mirrored TypeScript domain types that duplicate Rust wire
shapes. Rust (`crates/waml`) is the single source of truth for parsing,
validation, and all domain logic; the TypeScript side should not maintain a
second, hand-typed copy of every struct that crosses the wasm boundary.

## Background

`crates/waml-wasm` already establishes this pattern for one surface:
`Diagnostic`, `Severity`, `DiagCode`, `Solved`, `SolveConfig`, and friends are
defined in Rust with `#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]`
and `tsify(into_wasm_abi, from_wasm_abi)`, and `packages/wasm/src/index.ts`
re-exports the generated `.d.ts` types with the comment "Types generated from
the Rust structs by Tsify (single source of truth)." The `solve()` wasm export
already returns the typed `SolveResult` directly.

What was never finished:
- The `validate()` wasm export still returns raw `JsValue` (`any` in the
  generated `.d.ts`) even though `Diagnostic` itself is already tsify'd.
- `Model`, `Node`, `Edge`, `Diagram`, `DiagramGroup`, `DiagramDisplay`,
  `FlowDoc`, `SequenceDoc` and their nested types (`crates/waml/src/model.rs`)
  have no tsify derive at all.
- `Concept`, `Bundle`, `Link`, `Citation`, `FmValue`, `ConceptRole`
  (`crates/waml/src/okf.rs`) have no tsify derive.
- `OpDto` (`crates/waml-ops-dto`) has no tsify derive.

These are exactly the types hand-mirrored today in
`packages/core/src/state/overlay.ts` (`RustModel`/`RustNode`/`RustEdge`/
`RustDiagram`/`RustDiagramGroup`/`RustDiagramDisplay`) and
`packages/okf/src/types.ts` (`Diagram`, `DiagramDisplay`, `FlowDoc`,
`SequenceDoc`, `Concept`, `Bundle`, etc.), consumed via unsafe
`build_model(bundle) as unknown as RustModel` casts.

## Architecture

Finish the pattern already established for `Diagnostic`/`Solved`:

1. Add `#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]` +
   `tsify(into_wasm_abi, from_wasm_abi)` to the remaining wire types in
   `crates/waml/src/model.rs`, `crates/waml/src/okf.rs`, and
   `crates/waml-ops-dto`.
2. Change the wasm-bindgen fn signatures in `crates/waml-wasm/src/lib.rs`
   (`build_model`, `build_bundle`, `validate`, `apply_ops`, `fmt`, `reindex`,
   `split_bundle`) from `JsValue`/`String` to the real typed values.
3. Regenerate `packages/wasm/src/generated/*` via
   `node scripts/build-wasm.mjs` (wasm-pack), which now emits concrete
   TypeScript types instead of `any`.
4. Delete the hand-mirrored TS interfaces; import the generated types from
   `@waml/wasm` instead, re-exported from `@waml/okf` under their existing
   public names where no fusion happens.

No new tooling, no new crate dependency beyond `tsify-next` (already a
dependency of both `waml` and `waml-wasm`).

## Type-by-type: what moves, what stays

### Moves to generated types

These are 1:1 wire mirrors today — the TS side does no transformation on
them, only pass-through or direct field copies:

- `Node`, `Edge`, `RelEnd`, `TypeRef`, `Attribute`, `RelationshipKind`,
  `NoteAnchor`, `DiagramGroup`, `DiagramDisplay` (wire partial), `Diagram`
  (wire shape, `groups` forest), `Model`
- `FlowDoc`, `FlowNode`, `FlowEdge`, `FlowFlavor`, `FlowNodeKind` — confirmed
  zero-transformation pass-through in `overlay.ts`
  (`flows: model.flows ?? []`)
- `SequenceDoc`, `Lifeline`, `SeqItem`, `SeqOperand`, `MessageVerb`,
  `FragmentKind`
- `Concept`, `Bundle`, `Link`, `Citation`, `FmValue`, `ConceptRole`
- `OpDto` (see Open Risks — the `v` field needs verification before this one
  ships)

### Stays hand-written in `@waml/okf`

These are UI-fused shapes produced by `toModelGraph`, not wire mirrors:

- `ModelNode`, `ModelEdge`, `ModelGraph` — add canvas-only fields (position,
  synthetic edge id, edge handles) that never exist on the wire
- `Diagram` (UI-facing) — flattens the wire `groups` forest into
  `members: string[]`, adds `hints`
- The resolved `DiagramDisplay` + `DEFAULT_DISPLAY` + `resolveDisplay()` —
  business logic applying defaults over the wire partial; Rust only ships the
  partial, never resolves it
- `DiagramHints` — pure canvas concept, no Rust counterpart

These hand-written types reference the *generated* leaf types (`Attribute`,
`RelEnd`, `NoteAnchor`, `RelationshipKind`, `Concept`) instead of redeclaring
them.

### Needs an explicit `#[tsify(type = "...")]` override

Three types have hand-written `Serialize`/`Deserialize` impls that tsify
cannot introspect automatically:

- `Visibility` (custom `into`/`try_from String`) → override to
  `"+" | "-" | "#" | "~"`
- `AssocName` (manually implemented untagged serialization) → override to
  `string | { ref: string }`
- `ClassifierType` (custom `into`/`try_from String`, dynamic
  `"family.Metaclass"` dispatch) → override to plain `string` (matches how
  `ModelNode.type` is already typed today)

## `@waml/okf` dependency change

`@waml/okf` currently has zero dependencies by design (see
`packages/okf/package.json`). To re-export the generated wire types under
their existing public names — so nothing importing `FlowDoc`, `SequenceDoc`,
`Concept`, `Bundle`, etc. from `@waml/okf` needs to change — it gains a new
`@waml/wasm` dependency, used only for type-only re-exports
(`export type { FlowDoc, ... } from "@waml/wasm"`).

This is the one real architectural tradeoff of this design: it couples a
previously-independent package to the wasm build. The alternative — forcing
every `web`/`core` call site to import these types from `@waml/wasm`
directly — is pure churn with no benefit, so the re-export is preferred.

## `overlay.ts` changes

`RustModel`, `RustNode`, `RustEdge`, `RustDiagram`, `RustDiagramGroup`,
`RustDiagramDisplay` interfaces are deleted. Replaced with aliased imports:

```ts
import type {
  Model as RustModel,
  Node as RustNode,
  Edge as RustEdge,
  Diagram as RustDiagram,
  DiagramGroup as RustDiagramGroup,
  DiagramDisplay as RustDiagramDisplay,
} from "@waml/wasm";
```

The rest of `overlay.ts` already uses the `RustX` names throughout, so the
diff is contained to the import block; `toModelGraph`'s body is unchanged.

## Rollout order (risk-ascending)

1. **`validate()` wiring** — `Diagnostic` is already tsify'd; just flip the
   fn signature to `Result<Vec<Diagnostic>, JsValue>`. Zero Rust type work,
   proves the pipeline end-to-end.
2. **Model surface** — `Node`/`Edge`/`RelEnd`/`TypeRef`/`Attribute`/
   `RelationshipKind`/`NoteAnchor`/`DiagramGroup`/`DiagramDisplay`/`Diagram`/
   `Model` + `build_model`.
3. **Flow/Sequence surface** — pass-through types; `build_model` already
   carries them (same fn, additive, no separate wasm export).
4. **OKF surface** — `Concept`/`Bundle`/`Link`/`Citation`/`FmValue`/
   `ConceptRole` + `build_bundle`. Requires verifying tsify emits
   `Record<string, FmValue>` correctly for the `HashMap` that
   `Concept::extra` uses today via a custom `serialize_maps_as_objects`
   serializer — that custom serializer goes away if tsify's own map handling
   matches.
5. **`fmt`/`reindex`/`split_bundle`** — trivial, all return
   `Vec<(String, String)>` tuples; ride along with whichever step touches
   the same file.
6. **`OpDto` + `apply_ops`** — last, after resolving the open risk below.

Each step: `cargo test --workspace`, `node scripts/build-wasm.mjs`,
`pnpm -r test`, `pnpm build` before moving to the next step.
`crates/waml/tests/serde_shape.rs` pins the wire shape and should stay green
throughout every step (only the derivation mechanism changes, never the
shape itself).

## Open risks

- **`OpDto.v` field.** The Rust `OpDto` enum has a `v: u32` version field on
  every variant with `#[serde(default = "one")]`. The hand-written TS
  `OpDto` in `packages/core/src/state/ops-adapter.ts` deliberately omits `v`
  — callers never send it, Rust defaults it to `1`. It is not yet confirmed
  whether tsify marks a `#[serde(default)]` field as optional in the
  generated TS type, or emits it as required `v: number`. Before wiring
  `apply_ops` to the generated `OpDto`, spike this one field in isolation.
  If tsify emits `v` as required, options are: add `#[tsify(optional)]` (if
  supported by tsify-next 0.5.6), or leave `OpDto` hand-written a while
  longer and revisit.
- **`Concept.extra: HashMap<String, FmValue>` → `Record<string, FmValue>`.**
  Today this crosses the wire via a hand-rolled
  `serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true)`
  call in `build_bundle`, specifically to avoid JS `Map` semantics. Need to
  confirm tsify's `into_wasm_abi` path produces the same plain-object
  representation without that custom serializer.

## Testing

- Rust: `cargo test --workspace` after each rollout step; `serde_shape.rs`
  wire-shape pins must stay green unchanged.
- TypeScript: `packages/core/src/state/overlay.test.ts`,
  `packages/core/src/state/model.test.ts`,
  `packages/core/src/state/ops-adapter.test.ts`,
  `packages/core/src/templates/templates.test.ts` exercise `build_model`/
  `apply_ops`/`toModelGraph` today and must keep passing with generated types
  substituted in — these are the regression net for the whole migration.
- Full gate per step: `pnpm -r test && pnpm build && cargo test --workspace`.
