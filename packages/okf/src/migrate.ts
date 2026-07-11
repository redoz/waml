import type { Attribute, ModelEdge, ModelGraph, ModelNode, RelEnd } from "./types";

// Pre-UML (data-mart era) shapes as found in old localStorage payloads and
// shared URLs. They exist only here; the rest of the codebase never sees them.
interface LegacyField { name: string; type: string; pk?: boolean; alias?: string; description?: string }
interface LegacyNode {
  key: string; title?: string; description?: string;
  schema?: LegacyField[]; position?: { x: number; y: number };
  [k: string]: unknown; // inputSource/status/owoxId/definition/… — dropped
}
interface LegacyEdge {
  id: string; from: string; to: string;
  bidirectional?: boolean; cardinality?: string;
  sourceHandle?: string | null; targetHandle?: string | null;
  [k: string]: unknown; // keys/existing — dropped
}

const MULT = (t: string) => (t === "N" ? "*" : "1");

/** Legacy "X:Y" cardinality → per-end multiplicities + navigability. */
export function endsFromCardinality(
  cardinality: string | undefined,
  bidirectional: boolean,
): { fromEnd: RelEnd; toEnd: RelEnd } {
  const fromEnd: RelEnd = {};
  const toEnd: RelEnd = { navigable: true };
  if (bidirectional) fromEnd.navigable = true;
  if (cardinality) {
    const [l, r] = cardinality.split(":");
    fromEnd.multiplicity = MULT(l);
    toEnd.multiplicity = MULT(r);
  }
  return { fromEnd, toEnd };
}

function migrateNode(n: LegacyNode): ModelNode {
  const attributes: Attribute[] = (n.schema ?? []).map(f => ({
    name: f.name,
    type: { name: f.type },
    multiplicity: "1",
    ...(f.description ? { description: f.description } : {}),
  }));
  return {
    key: n.key,
    type: "uml.Class",
    title: n.title ?? "Untitled",
    stereotypes: [],
    ...(n.description ? { description: n.description } : {}),
    attributes,
    position: n.position ?? { x: 0, y: 0 },
  };
}

function migrateEdge(e: LegacyEdge): ModelEdge {
  const bidirectional = e.bidirectional === true;
  return {
    id: e.id,
    kind: "associates",
    from: e.from,
    to: e.to,
    ...endsFromCardinality(typeof e.cardinality === "string" ? e.cardinality : undefined, bidirectional),
    bidirectional,
    ...(e.sourceHandle !== undefined ? { sourceHandle: e.sourceHandle } : {}),
    ...(e.targetHandle !== undefined ? { targetHandle: e.targetHandle } : {}),
  };
}

/** A payload predates the UML model when it has no `diagrams` array. */
export function isLegacyGraph(g: unknown): boolean {
  const x = g as { nodes?: unknown; diagrams?: unknown };
  return !!x && Array.isArray(x.nodes) && !Array.isArray(x.diagrams);
}

/** Normalize any persisted/shared payload (legacy or current) to a ModelGraph. Null = unusable. */
export function migrateGraph(raw: unknown): ModelGraph | null {
  const g = raw as { nodes?: unknown[]; edges?: unknown[]; diagrams?: unknown[] };
  if (!g || !Array.isArray(g.nodes) || !Array.isArray(g.edges)) return null;
  if (!isLegacyGraph(g)) {
    return {
      nodes: g.nodes as ModelNode[],
      edges: g.edges as ModelEdge[],
      diagrams: (g.diagrams ?? []) as ModelGraph["diagrams"],
    };
  }
  return {
    nodes: (g.nodes as LegacyNode[]).map(migrateNode),
    edges: (g.edges as LegacyEdge[]).map(migrateEdge),
    diagrams: [],
  };
}
