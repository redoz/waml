import type { ModelGraph, ModelNode, ModelEdge, Attribute } from "@mc/okf";
import { endsFromCardinality } from "@mc/okf";

// ── tiny authoring helpers ─────────────────────────────────────────────────
// Signatures kept from the mart era so the 22 template files stay untouched:
// `pk`, `inputSource` and join fields are accepted and dropped (data-profile
// concerns, deferred); cardinality maps onto per-end multiplicities.
export const f = (name: string, type: string, _pk = false, description?: string): Attribute =>
  ({ name, type: { name: type }, multiplicity: "1", ...(description ? { description } : {}) });

export const mart = (
  key: string,
  title: string,
  _inputSource: string,
  attributes: Attribute[],
  description?: string,
): ModelNode =>
  ({ key, title, type: "uml.Class", stereotypes: [], ...(description ? { description } : {}), attributes, position: { x: 0, y: 0 } });

export const rel = (
  id: string,
  from: string,
  to: string,
  _left: string,
  _right: string,
  cardinality: "1:1" | "1:N" | "N:1" | "N:N" = "N:1",
  bidirectional = false,
): ModelEdge => ({ id, kind: "associates", from, to, ...endsFromCardinality(cardinality, bidirectional), bidirectional });

export interface Template {
  id: string;                    // immutable — ?template=<id> deep links are public CTAs
  nicheId: string | null;
  category: "industry" | "dataset";
  name: string;
  description: string;
  graph: ModelGraph;
}

// ── UML-profile authoring helpers (stage 6+) ───────────────────────────────
export const attr = (
  name: string,
  type: string | { name: string; ref: string },
  opts: { mult?: string; vis?: Attribute["visibility"]; desc?: string } = {},
): Attribute => ({
  name,
  type: typeof type === "string" ? { name: type } : type,
  multiplicity: opts.mult ?? "1",
  ...(opts.vis ? { visibility: opts.vis } : {}),
  ...(opts.desc ? { description: opts.desc } : {}),
});

export const cls = (
  key: string,
  title: string,
  opts: { type?: string; stereotypes?: string[]; abstract?: boolean; description?: string; attributes?: Attribute[] } = {},
): ModelNode => ({
  key, title,
  type: opts.type ?? "uml.Class",
  stereotypes: opts.stereotypes ?? [],
  ...(opts.abstract ? { abstract: true } : {}),
  ...(opts.description ? { description: opts.description } : {}),
  attributes: opts.attributes ?? [],
  position: { x: 0, y: 0 },
});

export const enumOf = (key: string, title: string, values: string[], description?: string): ModelNode =>
  ({ key, title, type: "uml.Enum", stereotypes: [], ...(description ? { description } : {}), attributes: [], values, position: { x: 0, y: 0 } });

export const edge = (
  id: string,
  kind: ModelEdge["kind"],
  from: string,
  to: string,
  fromEnd: ModelEdge["fromEnd"] = {},
  toEnd: ModelEdge["toEnd"] = {},
): ModelEdge => ({
  id, kind, from, to, fromEnd,
  toEnd: kind === "associates" ? { navigable: true, ...toEnd } : toEnd,
  bidirectional: false,
});
