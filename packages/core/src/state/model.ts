// Bundle-as-truth model store. The in-memory source of truth is the OKF `bundle`
// (a `[path, markdown][]` array of pairs). The `Model` is a derived, read-only
// view (`build_model`); every edit is translated to `OpDto[]` (via the ops
// adapter) and realized with `apply_ops`, then the model is re-derived. Canvas-
// only data (node positions, edge handles, synthetic ids) lives in the `Overlay`
// and never touches the bundle.
//
// Requires `initWasm()` to have resolved before a store is constructed or mutated
// (`build_model`/`apply_ops` are sync only after init). `bootstrap.ts` awaits it;
// tests `await initWasm()` in `beforeAll`.
//
// Error surface: a failed `apply_ops` (e.g. renaming onto an existing slug) never
// throws out of a Svelte handler. The store keeps its prior state (no partial
// edit) and reports the error via the optional `onError` callback — mutator
// return types are unchanged so the ~13 call sites in `CanvasInner.svelte` and the
// details panel stay untouched.
import type { ModelGraph, ModelNode, ModelEdge, Diagram, RelationshipKind } from "@uaml/okf";
import { build_model, apply_ops } from "@uaml/okf";
import {
  toModelGraph,
  emptyOverlay,
  edgeKey,
  type Overlay,
  type RustModel,
} from "./overlay";
import { updateNodeOps, nodeNewOps, nodeRmOps, edgeAddOps, edgeRmOps, edgeSetOps } from "./ops-adapter";

export type Bundle = [string, string][];

export interface CreateStoreOptions {
  /** Called with a human-readable reason when an `apply_ops` edit is rejected. */
  onError?: (error: string) => void;
}

function derive(bundle: Bundle): RustModel {
  return build_model(bundle) as unknown as RustModel;
}

export function createModelStore(initial?: Bundle, opts: CreateStoreOptions = {}) {
  let bundle: Bundle = initial ? initial.map(([p, m]) => [p, m] as [string, string]) : [];
  let model = derive(bundle);
  let overlay: Overlay = emptyOverlay();

  const subs = new Set<() => void>();
  const emit = () => subs.forEach((f) => f());

  /** Apply ops to the bundle. On success replace bundle + re-derive + emit and
   *  return true; on failure keep prior state, surface the error, return false. */
  function run(ops: ReturnType<typeof updateNodeOps>): boolean {
    if (ops.length === 0) return true;
    try {
      const next = apply_ops(bundle, ops) as unknown as Bundle;
      bundle = next;
      model = derive(bundle);
      emit();
      return true;
    } catch (e) {
      opts.onError?.(String((e as { message?: string })?.message ?? e));
      return false;
    }
  }

  const graph = (): ModelGraph => toModelGraph(model, overlay);
  const findNode = (key: string): ModelNode | undefined => graph().nodes.find((n) => n.key === key);
  const findEdge = (id: string): ModelEdge | undefined => graph().edges.find((e) => e.id === id);

  /** A fresh node slug not colliding with any existing document. */
  function freshSlug(): string {
    const used = new Set(model.nodes.map((n) => n.key));
    let i = model.nodes.length + 1;
    let slug = `n${i}`;
    while (used.has(slug)) slug = `n${++i}`;
    return slug;
  }

  return {
    /** Derived, read-only `ModelGraph` (Rust model fused with the canvas overlay). */
    get: (): ModelGraph => graph(),
    /** The underlying bundle (the true source), copied so callers can't mutate it. */
    getBundle: (): Bundle => bundle.map(([p, m]) => [p, m] as [string, string]),
    subscribe(f: () => void) {
      subs.add(f);
      return () => subs.delete(f);
    },

    /** Replace the whole model with a new bundle (import replace / template /
     *  share / clear). Resets the overlay — the web layer re-runs dagre and feeds
     *  positions back via `updateNode({position})`. */
    load(next: Bundle): void {
      bundle = next.map(([p, m]) => [p, m] as [string, string]);
      model = derive(bundle);
      overlay = emptyOverlay();
      emit();
    },

    addNode(position: { x: number; y: number }, _diagramKey?: string): ModelNode {
      // Diagram membership is derived-only in Stage 1b (no membership ops), so the
      // diagram hint is accepted and dropped — the node lands in the implicit view.
      const slug = freshSlug();
      const ok = run(nodeNewOps({ slug, title: "New object", type: "uml.Class" }));
      overlay.nodes.set(slug, { position });
      if (!ok) emit();
      return findNode(slug) ?? { key: slug, type: "uml.Class", title: "New object", stereotypes: [], attributes: [], position };
    },

    updateNode(key: string, patch: Partial<ModelNode>): void {
      // Position is canvas-only → overlay, no op, but still notify subscribers.
      if (patch.position) {
        overlay.nodes.set(key, { ...overlay.nodes.get(key), position: patch.position });
        emit();
      }
      const prev = findNode(key);
      if (!prev) return;
      run(updateNodeOps(prev, patch));
    },

    removeNode(key: string): void {
      overlay.nodes.delete(key);
      run(nodeRmOps(key, true));
    },

    addEdge(from: string, to: string, sourceHandle?: string | null, targetHandle?: string | null): ModelEdge | null {
      if (from === to) return null;
      const pair = [from, to].sort().join("|");
      const existing = graph().edges.find((e) => [e.from, e.to].sort().join("|") === pair);
      if (existing) {
        // A reciprocal association makes the derived edge bidirectional (both docs
        // declare it). Add the reverse `associates` unless it already reads both ways.
        if (!existing.bidirectional) run(edgeAddOps(existing.to, existing.from, "associates"));
        return findEdge(existing.id) ?? existing;
      }
      if (!run(edgeAddOps(from, to, "associates"))) return null;
      overlay.edges.set(edgeKey({ from, to, kind: "associates" }), { sourceHandle, targetHandle });
      emit();
      const created = graph().edges.find((e) => e.from === from && e.to === to && e.kind === "associates");
      return created ?? null;
    },

    updateEdge(id: string, patch: Partial<ModelEdge>): void {
      const prev = findEdge(id);
      if (!prev) return;
      const newFrom = patch.from ?? prev.from;
      const newTo = patch.to ?? prev.to;
      const newKind: RelationshipKind = patch.kind ?? prev.kind;
      // Canvas-only handle hints ride the overlay (keyed by the — possibly new — triple).
      if (patch.sourceHandle !== undefined || patch.targetHandle !== undefined) {
        const key = edgeKey({ from: newFrom, to: newTo, kind: newKind });
        const cur = overlay.edges.get(edgeKey(prev)) ?? {};
        overlay.edges.set(key, {
          ...cur,
          ...(patch.sourceHandle !== undefined ? { sourceHandle: patch.sourceHandle } : {}),
          ...(patch.targetHandle !== undefined ? { targetHandle: patch.targetHandle } : {}),
        });
        emit();
      }
      run(edgeSetOps(prev, patch));
    },

    removeEdge(id: string): void {
      const prev = findEdge(id);
      if (!prev) return;
      overlay.edges.delete(edgeKey(prev));
      run(edgeRmOps(prev));
    },

    // ── diagrams: derived-only in Stage 1b (no diagram/membership ops) ──────────
    // Signatures preserved so `CanvasInner.svelte` compiles; mutations are no-ops
    // (diagram editing returns in Stage 1c). `addDiagram`/`addDiagramFromMembers`
    // return an unpersisted stub so callers can read its `.key`.
    addDiagram(title: string): Diagram {
      return { key: `d-${title}`, title, profile: "uml-domain", members: [] };
    },
    addDiagramFromMembers(title: string, _members: string[]): Diagram {
      return { key: `d-${title}`, title, profile: "uml-domain", members: [] };
    },
    updateDiagram(_key: string, _patch: Partial<Diagram>): void {
      /* no-op in 1b */
    },
    removeDiagram(_key: string): void {
      /* no-op in 1b */
    },
  };
}

export type ModelStore = ReturnType<typeof createModelStore>;
