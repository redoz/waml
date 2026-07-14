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
export const RELATIONSHIP_KINDS = ["associates", "aggregates", "composes", "specializes", "implements", "depends", "includes", "extends", "annotates"] as const;
export type RelationshipKind = (typeof RELATIONSHIP_KINDS)[number];

/** Verbs that may take `: <near> to <far>` ends. Required for aggregates/composes;
 *  optional for associates (bare = actor↔use-case communication link, enforced
 *  cross-doc by the Rust validate layer); forbidden for everything else. */
export const ENDED_KINDS: ReadonlySet<RelationshipKind> = new Set(["associates", "aggregates", "composes"]);

export interface RelEnd { multiplicity?: string; role?: string; navigable?: boolean }

/** A uml.Note anchor: a classifier, a NAMED association, or an association addressed by its endpoint (unnamed). */
export type NoteAnchor =
  | { targetKey: string }
  | { sourceKey: string; name: string }
  | { sourceKey: string; kind: RelationshipKind; targetKey: string };

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
  /** How much of each attribute row shows: just the name, or name + type. */
  attributeDetail: "name-only" | "name-type";
  /** Diagram-level gate on the +/-/#/~ visibility marker per attribute row. */
  showAttributeVisibility: boolean;
  /** Independent gate on the {mult} suffix per attribute row. */
  showAttributeMultiplicity: boolean;
  /** Cap on attribute rows drawn per box; excess folded as "+K more". Absent ⇒ unlimited. */
  maxAttributes?: number;
  /** Whether association edges carry their multiplicity/role labels. */
  associationLabels: "all" | "hidden";
  /** Visually emphasize multiplicity on association labels. */
  emphasizeMultiplicity: boolean;
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
  attributeDetail: "name-type",
  showAttributeVisibility: true,
  showAttributeMultiplicity: true,
  // maxAttributes omitted ⇒ undefined ⇒ unlimited
  associationLabels: "all",
  emphasizeMultiplicity: false,
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
}

// ── Flow substrate (activity / state-machine behavior documents) ────────────
// Mirrors the Rust `model::Flow*` shapes (crates/waml/src/model.rs). A flow doc
// is a self-rendering directed graph: one per behavior classifier, resolved
// from its `## Nodes` section by `build_flows`. Additive — absent on graphs
// with no behavior classifiers.

/** Flow flavor: tunes rendering only — one grammar for both. */
export type FlowFlavor = "activity" | "stateMachine";

/** A flow node's closed kind set (heading keyword). `"plain"` = no keyword →
 *  action (activity) or state (state machine). */
export type FlowNodeKind = "initial" | "final" | "decision" | "merge" | "fork" | "join" | "object" | "plain";

/** A resolved node of a flow document. */
export interface FlowNode {
  /** Heading text minus the kind keyword — the name transitions resolve against. */
  id: string;
  kind: FlowNodeKind;
  /** Resolved key of an `object` node's typing classifier. */
  objectRef?: string;
  partition?: string;
  entry?: string;
  do?: string;
  exit?: string;
  /** Resolved key of the flow document this composite/call-behavior refines. */
  refines?: string;
  notes?: string[];
}

/** A resolved transition (flow edge). Source/target are node identities. */
export interface FlowEdge {
  from: string;
  /** Local node identity, or the link title for a cross-document target. */
  to: string;
  /** Resolved key when the target was a cross-document link. */
  toRef?: string;
  trigger?: string;
  guard?: string;
  /** Decision default branch (`else transitions to …`). */
  else?: boolean;
  effect?: string;
  /** Resolved key of the carried object type (`carries <link>` object flow). */
  carries?: string;
}

/** One flow document: one self-rendering directed graph (model AND view). */
export interface FlowDoc {
  key: string;
  title: string;
  flavor: FlowFlavor;
  /** Resolved key of the entity this behavior describes (frontmatter link). */
  describes?: string;
  nodes: FlowNode[];
  edges: FlowEdge[];
}

/** Split "family.Metaclass". Null for opaque/legacy tokens. */
export function splitType(type: string): { family: string; metaclass: string } | null {
  const m = /^([a-z][a-z0-9]*)\.([A-Za-z][A-Za-z0-9]*)$/.exec(type);
  return m ? { family: m[1], metaclass: m[2] } : null;
}

// ── OKF tier (domain-agnostic substrate beneath the UML profile) ─────────────
// The lossless projection of a bundle: one `Concept` per markdown document,
// carrying every OKF field verbatim. Additive to the UML `Model*` types above —
// mirrors the Rust `okf::` shapes (see crates/waml/src/okf.rs) that
// `build_bundle` returns over the wasm wire. These do NOT replace `ModelNode` /
// `ModelGraph`; both surfaces coexist.

/** A frontmatter scalar or (recursively) list, mirroring Rust `okf`'s `FmValue`. */
export type FmValue = string | boolean | number | FmValue[];

/** Reserved-file role of a document. Absent on the wire ⇒ `"concept"`. */
export type ConceptRole = "concept" | "index" | "log";

/** An untyped OKF link (`[text](href)`) drawn from a concept's body (OKF §5.3). */
export interface Link { text: string; href: string }

/** A citation: a link to an external source backing a claim (OKF §8). */
export interface Citation { text: string; href: string }

/** The domain-agnostic projection of one markdown document. Round-trips every
 *  OKF field losslessly. Fields that are empty/default are omitted on the wire
 *  (serde `skip_serializing_if`), hence optional here. */
export interface Concept {
  /** Concept ID = full path minus the `.md` suffix (OKF §2). */
  id: string;
  /** The free-text `type` frontmatter field (NOT the UML classifier token). */
  type: string;
  title?: string;
  description?: string;
  resource?: string;
  tags?: string[];
  timestamp?: string;
  /** The full markdown body (everything after the frontmatter), verbatim. */
  body: string;
  links?: Link[];
  citations?: Citation[];
  /** Absent ⇒ `"concept"`. */
  role?: ConceptRole;
  /** Producer-specific frontmatter keys with no dedicated field above. */
  extra?: Record<string, FmValue>;
}

/** One `Concept` per document; a Bundle stays flat. */
export interface Bundle {
  concepts: Concept[];
}
