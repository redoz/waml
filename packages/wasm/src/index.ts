// Frontend-facing entry to the Rust WAML core, compiled to wasm and inlined
// (no runtime .wasm fetch). Call `initWasm()` once before any other export.
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
import { wasmBytes } from "./generated/wasm-inline";

// Types generated from the Rust structs by Tsify (single source of truth).
export type {
  Size,
  Rect,
  FlagSet,
  Shape,
  SolvedGroup,
  Solved,
  SolveConfig,
  Diagnostic,
  DiagCode,
  Severity,
  SolveResult,
  Model,
  Node,
  Edge,
  RelEnd,
  TypeRef,
  Attribute,
  RelationshipKind,
  NoteAnchor,
  DiagramGroup,
  DiagramDisplay,
  Diagram,
  FlowDoc,
  ActivityNode,
  FlowEdge,
  FlowEdgeKind,
  FlowFlavor,
  FlowNodeKind,
  SequenceDoc,
  Lifeline,
  SeqItem,
  SeqOperand,
  MessageVerb,
  FragmentKind,
  Concept,
  Bundle,
  Link,
  Citation,
  FmValue,
  ConceptRole,
  OpDto,
  // Not in the original spec's type list, but a real generated wire type
  // consumed transitively via `OpDto`'s `diagram.set` variant — keep it.
  DisplayDto,
} from "./generated/waml_wasm.js";

import type { Size, SolveConfig, SolveResult } from "./generated/waml_wasm.js";

let ready: Promise<void> | undefined;

/** Instantiate the inlined wasm exactly once. Safe to await repeatedly. */
export function initWasm(): Promise<void> {
  if (!ready) {
    ready = init({ module_or_path: wasmBytes() }).then(() => {
      init_panic_hook();
    });
  }
  return ready;
}

export { apply_ops, build_bundle, build_model, fmt, new_diagram_doc, reindex, split_bundle, validate };

/**
 * Solve one diagram's layout. `bundle` is the OKF bundle, `diagramKey` the
 * `Diagram.key`, `sizes` maps node key → intrinsic size, `cfg` is optional.
 * Throws if `diagramKey` matches no diagram.
 */
export function solve(
  bundle: [string, string][],
  diagramKey: string,
  sizes: Record<string, Size>,
  cfg?: SolveConfig,
): SolveResult {
  return solveRaw(bundle, diagramKey, sizes, cfg ?? undefined);
}
