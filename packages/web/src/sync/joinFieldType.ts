import type { ModelNode, ModelEdge } from "@mc/okf";

// When a join key references a field that doesn't exist yet, we have to create
// it. Defaulting to STRING breaks the join if the other side is e.g. an INTEGER
// primary key (OWOX rejects "Incompatible types"). Instead, infer the new
// field's type from the field on the OTHER side of the same join key. Falls
// back to STRING when the counterpart is unknown too.
export function joinFieldType(
  nodes: ModelNode[],
  edges: ModelEdge[],
  nodeKey: string,
  fieldName: string,
): string {
  if (!fieldName) return "STRING";
  for (const e of edges) {
    for (const k of e.keys) {
      let otherKey: string | undefined;
      let otherField: string | undefined;
      if (e.from === nodeKey && k.left === fieldName) { otherKey = e.to; otherField = k.right; }
      else if (e.to === nodeKey && k.right === fieldName) { otherKey = e.from; otherField = k.left; }
      if (otherKey && otherField) {
        const t = nodes.find(n => n.key === otherKey)?.schema.find(f => f.name === otherField)?.type;
        if (t) return t;
      }
    }
  }
  return "STRING";
}
