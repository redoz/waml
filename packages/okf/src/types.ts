// ── Profile-agnostic modeling core ───────────────────────────────────────────
// Nodes are classifiers dispatched on `type` = "family.Metaclass"; everything
// domain-specific rides as data (stereotypes). Unknown types render generically.

export type Visibility = "+" | "-" | "#" | "~";

/** An attribute's type: a display token, optionally resolved to another classifier. */
export interface TypeRef { name: string; ref?: string }

export interface Attribute {
  name: string;
  type: TypeRef;
  /** UML multiplicity string as authored ("1", "0..1", "*", "1..*", "2..5"). Parser defaults to "1". */
  multiplicity: string;
  visibility?: Visibility;
  description?: string;
}

// "annotates" is a uml.Note-only verb; it never produces a ModelEdge (anchors live on the note node).
export const RELATIONSHIP_KINDS = ["associates", "aggregates", "composes", "specializes", "implements", "depends", "annotates"] as const;
export type RelationshipKind = (typeof RELATIONSHIP_KINDS)[number];

/** Verbs that take `: <near> to <far>` ends. The rest forbid them. */
export const ENDED_KINDS: ReadonlySet<RelationshipKind> = new Set(["associates", "aggregates", "composes"]);

export interface RelEnd { multiplicity?: string; role?: string; navigable?: boolean }

/** A uml.Note anchor: a classifier, a NAMED association, or an association addressed by its endpoint (unnamed). */
export type NoteAnchor =
  | { targetKey: string }
  | { sourceKey: string; name: string }
  | { sourceKey: string; kind: RelationshipKind; targetKey: string };

export interface ModelNode {
  key: string;
  /** Structured dispatch key "family.Metaclass" (e.g. "uml.Class") or an opaque legacy token. */
  type: string;
  title: string;
  stereotypes: string[];
  abstract?: boolean;
  description?: string;
  attributes: Attribute[];
  /** uml.Enum literals. */
  values?: string[];
  /** uml.Note markdown body (from ## Body). */
  body?: string;
  /** uml.Note anchor targets; the ## Notes shorthand desugars into a self-anchored note. */
  annotates?: NoteAnchor[];
  position: { x: number; y: number };
  /** Raw markdown of unrecognized ## sections — carried through round-trip, never dropped. */
  extra?: string;
}

export interface ModelEdge {
  id: string;
  kind: RelationshipKind;
  /** Declaring/near end: whole for aggregates/composes, child for specializes/implements, dependent for depends. */
  from: string;
  /** Far end: part / parent / interface / dependency target. */
  to: string;
  /** Optional UML association name: a string reading-label (also the note anchor handle) OR
   *  a ref to a uml.Association node key (association class). Rendered near the line midpoint. */
  name?: string | { ref: string };
  fromEnd: RelEnd;
  toEnd: RelEnd;
  /** Derived from reciprocity: both docs declared the association. */
  bidirectional: boolean;
  // Canvas-only hints for which ports the edge attaches to (not encoded in OKF).
  sourceHandle?: string | null;
  targetHandle?: string | null;
}

export interface DiagramHints {
  emphasize?: string[];
  /** Node keys drawn as collapsed ref chips instead of full boxes. */
  collapse?: string[];
}

/** A curated, profiled view over nodes — not a classifier. */
export interface Diagram {
  key: string;
  title: string;
  profile: string;
  members: string[];
  hints?: DiagramHints;
}

export interface ModelGraph {
  nodes: ModelNode[];
  edges: ModelEdge[];
  /** Empty array ⇒ the canvas shows one implicit diagram containing every node. */
  diagrams: Diagram[];
}

/** Split "family.Metaclass". Null for opaque/legacy tokens. */
export function splitType(type: string): { family: string; metaclass: string } | null {
  const m = /^([a-z][a-z0-9]*)\.([A-Za-z][A-Za-z0-9]*)$/.exec(type);
  return m ? { family: m[1], metaclass: m[2] } : null;
}
