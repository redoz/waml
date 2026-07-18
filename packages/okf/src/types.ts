// ── Profile-agnostic modeling core ───────────────────────────────────────────
// Nodes are classifiers dispatched on `type` = "family.Metaclass"; everything
// domain-specific rides as data (stereotypes). Unknown types render generically.

export type Visibility = "+" | "-" | "#" | "~";

export type {
  TypeRef,
  Attribute,
  Slot,
  RelEnd,
  RelationshipKind,
  NoteAnchor,
  FlowFlavor,
  FlowNodeKind,
  ActivityNode,
  FlowEdge,
  FlowEdgeKind,
  FlowDoc,
  MessageVerb,
  FragmentKind,
  SeqEdge,
  SeqNode,
  SeqChild,
  SequenceDoc,
  FmValue,
  ConceptRole,
  Link,
  Citation,
  Concept,
  Bundle,
} from "@waml/wasm";

import type {
  Attribute,
  Slot,
  RelEnd,
  RelationshipKind,
  NoteAnchor,
  ActivityNode,
  FlowEdge,
  FlowDoc,
  SequenceDoc,
  Concept,
} from "@waml/wasm";

// "annotates" is a uml.Note-only verb; it never produces a ModelEdge (anchors live on the note node).
export const RELATIONSHIP_KINDS = ["associates", "aggregates", "composes", "specializes", "implements", "depends", "includes", "extends", "annotates"] as const;

/** Verbs that may take `: <near> to <far>` ends. Required for aggregates/composes;
 *  optional for associates (bare = actor↔use-case communication link, enforced
 *  cross-doc by the Rust validate layer); forbidden for everything else. */
export const ENDED_KINDS: ReadonlySet<RelationshipKind> = new Set(["associates", "aggregates", "composes"]);

export interface ModelNode {
  /** Lossless OKF projection of this node's source document (OKF tier) and the
   *  single authoritative source for title/description/verbatim body (read via
   *  `concept.title` / `concept.description` / `concept.body`) plus the non-UML
   *  OKF fields (tags/resource/timestamp/links/citations/role/extra). */
  concept: Concept;
  key: string;
  /** Structured dispatch key "family.Metaclass" (e.g. "uml.Class") or an opaque legacy token. */
  type: string;
  stereotypes: string[];
  abstract?: boolean;
  attributes: Attribute[];
  /** Slot values on a uml.InstanceSpecification node (design spec §3.3). Absent on non-instances. */
  slots?: Slot[];
  /** uml.Enum literals. */
  values?: string[];
  /** uml.Note markdown prose (from ## Body). Distinct from the generic verbatim
   *  `concept.body`; sole reader is the note node renderer. */
  note_body?: string;
  /** uml.Note anchor targets; the ## Notes shorthand desugars into a self-anchored note. */
  annotates?: NoteAnchor[];
  /** Ordered member keys (classifiers, diagrams, sub-packages). Meaningful only
   *  on uml.Package nodes; absent elsewhere. */
  members?: string[];
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

/** Per-diagram render settings — how the ACTIVE diagram draws its classifiers and
 *  associations. Persisted on the diagram (in the model / OKF), NOT per-browser.
 *  Absent ⇒ resolves to DEFAULT_DISPLAY (see resolveDisplay), so existing OKF
 *  files without a `display` block stay valid and round-trip unchanged. */
export interface DiagramDisplay {
  /** Show attribute rows inside class boxes (vs. a collapsed attribute count). */
  showAttributes: boolean;
  /** Whether each attribute row shows its type name (true) or name only (false). */
  showType: boolean;
  /** Diagram-level gate on the +/-/#/~ visibility marker per attribute row. */
  showAttributeVisibility: boolean;
  /** Independent gate on the {mult} suffix per attribute row. */
  showAttributeMultiplicity: boolean;
  /** Cap on attribute rows drawn per box; excess folded as "+K more". Absent ⇒ unlimited. */
  maxAttributes?: number;
  /** Show each association end's role name. */
  showRoles: boolean;
  /** Show each association end's cardinality (multiplicity). */
  showCardinality: boolean;
  /** Show the association's reading-label name at the line midpoint. */
  showLabels: boolean;
  /** Show the «stereotype» / keyword row on class boxes. */
  showStereotype: boolean;
  /** Allowlist of stereotype tag names to render. Absent ⇒ show all; [] ⇒ show none. */
  stereotypeFilter?: string[];
  /** Per-stereotype-name color override. */
  stereotypeColors: Record<string, string>;
}

/** Defaults applied when a diagram has no `display` block (keeps legacy OKF valid). */
export const DEFAULT_DISPLAY: DiagramDisplay = {
  showAttributes: true,
  showType: true,
  showAttributeVisibility: true,
  showAttributeMultiplicity: true,
  // maxAttributes omitted ⇒ undefined ⇒ unlimited
  showRoles: true,
  showCardinality: true,
  showLabels: true,
  showStereotype: true,
  // stereotypeFilter omitted ⇒ undefined ⇒ show all
  stereotypeColors: {},
};

/** Resolve a diagram's (possibly absent/partial) display to a full DiagramDisplay. */
export function resolveDisplay(display?: Partial<DiagramDisplay>): DiagramDisplay {
  return { ...DEFAULT_DISPLAY, ...display };
}

/** A curated, profiled view over nodes — not a classifier. */
export interface Diagram {
  key: string;
  title: string;
  profile: string;
  members: string[];
  hints?: DiagramHints;
  /** Free-text reviewer note. */
  description?: string;
  /** The raw STORED partial (only authored keys); always fed through resolveDisplay before use. */
  display?: Partial<DiagramDisplay>;
}

export interface ModelGraph {
  nodes: ModelNode[];
  edges: ModelEdge[];
  /** Empty array ⇒ the canvas shows one implicit diagram containing every node. */
  diagrams: Diagram[];
  /** Bundle/root name (export label + navigator root crumb). */
  path: string;
  /** Discovered uml.Package nodes (root has key ""), carrying ordered `members`. */
  packages: ModelNode[];
  /** Flow-substrate behavior documents (self-rendering; absent on legacy graphs). */
  flows?: FlowDoc[];
  /** Model-level pool of behavior flow elements, referenced by `FlowDoc.nodes` (design spec §3/§4). */
  activityNodes?: ActivityNode[];
  /** Model-level pool of typed control/object flow edges, referenced by `FlowDoc.edges`. */
  flowEdges?: FlowEdge[];
  /** Interaction-substrate behavior documents (self-rendering; absent on legacy graphs). */
  interactions?: SequenceDoc[];
}

/** Split "family.Metaclass". Null for opaque/legacy tokens. */
export function splitType(type: string): { family: string; metaclass: string } | null {
  const m = /^([a-z][a-z0-9]*)\.([A-Za-z][A-Za-z0-9]*)$/.exec(type);
  return m ? { family: m[1], metaclass: m[2] } : null;
}

