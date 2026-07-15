// Projects the relationships touching a model node into flat rows for the
// object inspector's read-only Associations list. Editing an association still
// lives in the RelationshipInspector — this is a summary from the node's side.
import type { ModelNode, ModelEdge, RelationshipKind } from "@waml/okf";

export interface AssociationRow {
  id: string;
  kind: RelationshipKind;
  /** True when the selected node is the edge's `from` end. */
  outgoing: boolean;
  /** Title of the node at the other end of the relationship. */
  otherTitle: string;
  /** The far end's multiplicity, when the kind carries ends. */
  multiplicity?: string;
  /** The far end's role name, when set. */
  role?: string;
}

/** Relationships touching `node`, in model order. `annotates` (a uml.Note
 *  anchor, not a real association) is skipped. */
export function nodeAssociations(
  node: ModelNode,
  edges: ModelEdge[],
  nodes: ModelNode[],
): AssociationRow[] {
  const titleOf = (key: string) =>
    nodes.find((n) => n.key === key)?.concept.title?.trim() || key;
  const rows: AssociationRow[] = [];
  for (const edge of edges) {
    if (edge.kind === "annotates") continue;
    const outgoing = edge.from === node.key;
    const incoming = edge.to === node.key;
    if (!outgoing && !incoming) continue;
    const farEnd = outgoing ? edge.toEnd : edge.fromEnd;
    rows.push({
      id: edge.id,
      kind: edge.kind,
      outgoing,
      otherTitle: titleOf(outgoing ? edge.to : edge.from),
      multiplicity: farEnd.multiplicity,
      role: farEnd.role,
    });
  }
  return rows;
}
