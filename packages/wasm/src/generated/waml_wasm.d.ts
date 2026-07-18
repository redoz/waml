/* tslint:disable */
/* eslint-disable */
/**
 * A `uml.Note` anchor. Three forms, per the spec.
 */
export type NoteAnchor = { targetKey: string } | { sourceKey: string; name: string } | { sourceKey: string; kind: RelationshipKind; targetKey: string };

/**
 * A behavior flow element in the shared model-level pool (design spec §3): an
 * `Element`, NOT a classifier. Each activity/state-machine node lives here and
 * is referenced from its owning behavior\'s view (`FlowDoc.nodes`) by `key` —
 * exactly as a class `Diagram` references pooled classifiers by `members`.
 */
export interface ActivityNode {
    /**
     * Global pool identity: `\"{behavior}#{id}\"` (unique across the model).
     */
    key: string;
    /**
     * Local heading identity (unique within the owning behavior): the display
     * name and the name local transitions resolve against.
     */
    id: string;
    /**
     * Owning behavior document key.
     */
    behavior: string;
    kind: FlowNodeKind;
    /**
     * Resolved key of an `object` node\'s typing classifier.
     */
    objectRef?: string;
    partition?: string;
    entry?: string;
    do?: string;
    exit?: string;
    /**
     * Resolved key of the flow document this composite/call-behavior refines.
     */
    refines?: string;
    notes?: string[];
}

/**
 * A citation: a link to an external source backing a claim, listed under a
 * `# Citations` heading (OKF §8).
 */
export interface Citation {
    text: string;
    href: string;
}

/**
 * A diagram\'s authored render settings — a PARTIAL. Only keys present in the
 * file are `Some`/non-empty; TS `resolveDisplay` fills the rest from
 * `DEFAULT_DISPLAY`. Serde `rename_all=\"camelCase\"` matches the TS keys.
 */
export interface DiagramDisplay {
    showAttributes?: boolean;
    showType?: boolean;
    showAttributeVisibility?: boolean;
    showAttributeMultiplicity?: boolean;
    maxAttributes?: number;
    showRoles?: boolean;
    showCardinality?: boolean;
    showLabels?: boolean;
    showStereotype?: boolean;
    /**
     * `None` ⇒ key absent ⇒ show all; `Some(vec)` ⇒ allowlist (empty ⇒ show none).
     */
    stereotypeFilter?: string[];
    /**
     * Opaque `\"name:#rrggbb\"` pairs; empty ⇒ key absent.
     */
    stereotypeColors?: string[];
}

/**
 * A flow node\'s closed kind set (heading keyword). `Plain` = no keyword →
 * action (activity) or state (state machine).
 */
export type FlowNodeKind = "initial" | "final" | "decision" | "merge" | "fork" | "join" | "object" | "plain";

/**
 * A fully-specified display block on the wire. Mirrors `waml::ops::DiagramDisplaySet`.
 */
export interface DisplayDto {
    showAttributes: boolean;
    showType: boolean;
    showAttributeVisibility: boolean;
    showAttributeMultiplicity: boolean;
    maxAttributes?: number | undefined;
    showRoles: boolean;
    showCardinality: boolean;
    showLabels: boolean;
    showStereotype: boolean;
    stereotypeFilter?: string[] | undefined;
    stereotypeColors?: string[];
}

/**
 * A message reference or a nested-fragment reference inside an ordered
 * interaction stream (the interaction root, or a fragment operand). Document
 * order within the list is time order (design spec §6). `edge`/`node` are ids
 * into `SequenceDoc.edges` / `SequenceDoc.nodes`.
 */
export type SeqChild = { item: "message"; edge: string } | { item: "fragment"; node: string };

/**
 * A message: an interaction-LOCAL, ORDERED edge (design spec §6). It is NOT a
 * reusable pool edge and NOT an Association — its identity is bound to this
 * interaction\'s time axis. `from`/`to` are lifeline node ids (a lifeline\'s
 * handle: its alias, else its title).
 */
export interface SeqEdge {
    /**
     * Doc-unique id (`m0`, `m1`, … in document/time order), referenced by a
     * container\'s ordered `items`.
     */
    id: string;
    from: string;
    verb: MessageVerb;
    to: string;
    signature?: string;
}

/**
 * A node of an interaction\'s flat model: a participant lifeline, a combined
 * fragment, or a fragment operand. These are interaction-LOCAL (design spec
 * §6) — not members of the shared Element pool. Containment is preserved by id
 * reference: a fragment lists its operand ids; an operand lists its ordered
 * items (message edges + nested fragment nodes).
 */
export type SeqNode = { node: "lifeline"; id: string; title: string; alias?: string; ref?: string } | { node: "fragment"; id: string; kind: FragmentKind; operands: string[] } | { node: "operand"; id: string; guard?: string; items: SeqChild[] };

/**
 * A resolved membership group in a diagram (heading text + resolved keys).
 */
export interface DiagramGroup {
    name: string;
    members: string[];
    children: DiagramGroup[];
}

/**
 * A slot value on an `InstanceSpecification` (design spec §3.2): a named value
 * that stands in for a classifier attribute, rather than declaring one. Mirrors
 * `Attribute` for serde/tsify.
 */
export interface Slot {
    name: string;
    value: string;
    /**
     * Set when the slot value resolves to another pool element (an
     * instance-valued slot); a display token otherwise.
     */
    ref?: string;
}

/**
 * A typed control/object flow edge (design spec §3): a model-level pool member,
 * referenced from its owning behavior\'s view (`FlowDoc.edges`) by `key`.
 */
export interface FlowEdge {
    /**
     * Global pool identity: `\"{behavior}#e{n}\"`.
     */
    key: string;
    kind: FlowEdgeKind;
    /**
     * Owning behavior document key.
     */
    behavior: string;
    /**
     * Source activity-node pool key (always a node in `behavior`).
     */
    from: string;
    /**
     * Target activity-node pool key for a LOCAL target; the link title for a
     * cross-document target (matches no local node key → not drawn, mirroring
     * the class-diagram edge rule).
     */
    to: string;
    /**
     * Resolved key of the target *behavior document* when the target was a
     * cross-document link.
     */
    toRef?: string;
    trigger?: string;
    guard?: string;
    /**
     * Decision default branch (`else transitions to …`).
     */
    else?: boolean;
    effect?: string;
    /**
     * Resolved key of the carried object type (`carries <link>` object flow).
     */
    carries?: string;
}

/**
 * An attribute\'s type: a display token, optionally resolved to another classifier\'s slug.
 */
export interface TypeRef {
    name: string;
    ref?: string;
}

/**
 * An untyped OKF link (`[text](href)`) drawn from a concept\'s body (OKF §5.3).
 */
export interface Link {
    text: string;
    href: string;
}

/**
 * Combined-fragment keyword. `par` deferred (open question in spec).
 */
export type FragmentKind = "alt" | "opt" | "loop";

/**
 * Every document projects to exactly one `Concept`; a `Bundle` stays flat.
 */
export interface Bundle {
    concepts: Concept[];
}

/**
 * Flow flavor: tunes rendering only — one grammar for both.
 */
export type FlowFlavor = "activity" | "stateMachine";

/**
 * One behavior document as a **view** (design spec §4): it no longer owns its
 * nodes/edges inline — it references pooled `ActivityNode`s and `FlowEdge`s by
 * key, exactly as a class `Diagram` references pooled classifiers by `members`.
 */
export interface FlowDoc {
    key: string;
    title: string;
    flavor: FlowFlavor;
    /**
     * Resolved key of the entity this behavior describes (frontmatter link).
     */
    describes?: string;
    /**
     * Pool keys of this behavior\'s `ActivityNode`s (view → pool reference).
     */
    nodes: string[];
    /**
     * Pool keys of this behavior\'s `FlowEdge`s (view → pool reference).
     */
    edges: string[];
}

/**
 * One interaction (`uml.Sequence`): a flat, interaction-local model of
 * lifelines/fragments/operands (`nodes`) and ordered messages (`edges`), with
 * containment preserved via `items` (the root stream) plus each fragment\'s
 * operand ids and each operand\'s item stream. This is the RUNTIME view; the
 * on-disk `## Lifelines`/`## Messages` form (nested) is a separate storage
 * shape (design spec §9 — storage/runtime need not be 1:1).
 */
export interface SequenceDoc {
    key: string;
    title: string;
    describes?: string;
    /**
     * Lifelines + fragments + operands; resolve by `id`. Lifelines appear first,
     * in declaration order (participant column order).
     */
    nodes: SeqNode[];
    /**
     * Messages, ORDERED (document order = time order); interaction-local.
     */
    edges: SeqEdge[];
    /**
     * The interaction root\'s ordered item stream (message/fragment refs).
     */
    items: SeqChild[];
}

/**
 * Reserved-file role. Every document lands in the bundle regardless of role;
 * `index.md`/`log.md` are flagged so consumers can treat them specially.
 */
export type ConceptRole = "concept" | "index" | "log";

/**
 * Result of solving one diagram: absolute rects + any layout diagnostics.
 * Tsify emits its TypeScript type; under `wasm` it crosses the boundary as a
 * plain JS object.
 */
export interface SolveResult {
    solved: Solved;
    diagnostics: Diagnostic[];
}

/**
 * The domain-agnostic projection of one markdown document. Round-trips every
 * OKF field losslessly — nothing a producer wrote is dropped: known fields are
 * promoted, the raw markdown body is retained verbatim, and any remaining
 * frontmatter survives in [`Concept::extra`].
 */
export interface Concept {
    /**
     * Concept ID = full path minus the `.md` suffix (OKF §2).
     */
    id: string;
    /**
     * The free-text `type` frontmatter field (NOT the UML `ElementType`).
     */
    type: string;
    title?: string;
    description?: string;
    resource?: string;
    tags?: string[];
    timestamp?: string;
    /**
     * The full markdown body (everything after the frontmatter), verbatim.
     */
    body: string;
    links?: Link[];
    citations?: Citation[];
    role?: ConceptRole;
    /**
     * Producer-specific frontmatter keys with no dedicated field above.
     */
    extra?: Record<string, FmValue>;
}

/**
 * The kind of a pooled activity edge (design spec §3). Not flattened into
 * `Association`; each kind keeps its own semantics.
 */
export type FlowEdgeKind = "controlFlow" | "objectFlow";

/**
 * The message kind: fixes line and arrowhead (interaction substrate).
 */
export type MessageVerb = "calls" | "sends" | "replies" | "creates" | "destroys";

export interface Attribute {
    name: string;
    type: TypeRef;
    multiplicity: string;
    visibility?: "+" | "-" | "#" | "~";
    description?: string;
}

export interface Diagnostic {
    severity: Severity;
    code: DiagCode;
    message: string;
    file: string;
    line: number;
    /**
     * Byte range within `line`, if the diagnostic pins a precise column span.
     */
    span: [number, number] | undefined;
}

export interface Diagram {
    key: string;
    title: string;
    profile: string;
    description?: string;
    groups: DiagramGroup[];
    layout: unknown[];
    display?: DiagramDisplay;
}

export interface Edge {
    from: string;
    to: string;
    kind: RelationshipKind;
    name?: string | { ref: string };
    fromEnd: RelEnd;
    toEnd: RelEnd;
    /**
     * True when a reciprocal `associates` was declared from both ends; both
     * ends are then navigable. Set during Model resolution (Plan 3).
     */
    bidirectional: boolean;
}

export interface FlagSet {
    emphasized: boolean;
    collapsed: boolean;
}

export interface Model {
    nodes: Node[];
    edges: Edge[];
    diagrams: Diagram[];
    /**
     * Bundle/root name (root `index.md` H1); \"\" when absent. Export label + root crumb.
     */
    path?: string;
    /**
     * Discovered `uml.Package` nodes (root + nested). Kept out of `nodes` so
     * classifier consumers are unaffected.
     */
    packages?: Node[];
    /**
     * Flow-substrate behavior documents (uml.Activity / uml.StateMachine).
     */
    flows?: FlowDoc[];
    /**
     * Model-level pool of behavior flow elements (activity/state-machine nodes),
     * referenced by `FlowDoc.nodes`. Design spec §3/§4.
     */
    activityNodes?: ActivityNode[];
    /**
     * Model-level pool of typed control/object flow edges, referenced by
     * `FlowDoc.edges`. Design spec §3/§4.
     */
    flowEdges?: FlowEdge[];
    /**
     * Interaction-substrate behavior documents (uml.Sequence).
     */
    interactions?: SequenceDoc[];
}

export interface Node {
    /**
     * Lossless OKF projection of this node\'s source document (OKF tier) and the
     * single authoritative source for `title`/`description`/verbatim `body` (read
     * via `concept.title`/`concept.description`/`concept.body`) plus the non-UML
     * OKF fields (`tags`/`resource`/`timestamp`/`links`/`citations`/`role`/`extra`).
     * Populated from `crate::okf::project` (single source).
     */
    concept: Concept;
    key: string;
    type: string;
    stereotypes: string[];
    abstract?: boolean;
    attributes: Attribute[];
    values?: string[];
    /**
     * A `uml.Note`\'s markdown prose (from its `## Body` section). Distinct from
     * the generic verbatim `concept.body`: this is the Note-specific rendered
     * prose. Sole reader is the note node renderer. Title/description/verbatim
     * body now live only on `concept` (the single authoritative source).
     */
    note_body?: string;
    annotates?: NoteAnchor[];
    /**
     * Owned member keys (classifiers, diagrams, sub-packages), in progressive-
     * disclosure order. Meaningful only on `uml.Package` nodes; empty elsewhere.
     */
    members?: string[];
    /**
     * Slot values on an `InstanceSpecification` node (design spec §3.3). Empty
     * on every non-instance node.
     */
    slots?: Slot[];
}

export interface Rect {
    x: number;
    y: number;
    w: number;
    h: number;
}

export interface RelEnd {
    multiplicity?: string;
    role?: string;
    navigable?: boolean;
}

export interface Size {
    w: number;
    h: number;
}

export interface SolveConfig {
    margin_px: [number, number, number, number];
    chip: Size;
}

export interface Solved {
    nodes: Record<string, Rect>;
    groups: SolvedGroup[];
    flags: Record<string, FlagSet>;
}

export interface SolvedGroup {
    rect: Rect;
    shape: Shape;
    title: string | undefined;
    depth: number;
}

export type DiagCode = "duplicate-slug" | "frontmatter-not-clean" | "unknown-type" | "malformed-attribute" | "malformed-relationship" | "malformed-flow-bullet" | "duplicate-flow-node" | "unresolved-target" | "droppable-content" | "malformed-layout" | "unresolved-layout-ref" | "layout-cycle" | "layout-conflict" | "malformed-message" | "malformed-lifeline";

export type FmValue = string | boolean | number | FmValue[];

export type OpDto = { op: "node.new"; v?: number; slug: string; dir?: string; ty: string; title: string; stereotype?: string[]; desc?: string | undefined; abstract?: boolean } | { op: "node.rename"; v?: number; from: string; to: string } | { op: "node.set"; v?: number; slug: string; title?: string | undefined; desc?: string | undefined; stereotype?: string[] | undefined; abstract?: boolean | undefined; ty?: string | undefined } | { op: "node.rm"; v?: number; slug: string; cascade?: boolean } | { op: "attr.add"; v?: number; node: string; name: string; ty: string; mult?: string | undefined; vis?: string | undefined } | { op: "attr.set"; v?: number; node: string; name: string; ty?: string | undefined; mult?: string | undefined; vis?: string | undefined; rename?: string | undefined } | { op: "attr.rm"; v?: number; node: string; name: string } | { op: "value.add"; v?: number; node: string; literal: string } | { op: "value.rm"; v?: number; node: string; literal: string } | { op: "rel.add"; v?: number; source: string; kind: string; target: string; as?: string | undefined; as_ref?: string | undefined; ends?: string | undefined } | { op: "rel.set"; v?: number; source: string; kind?: string | undefined; target?: string | undefined; as?: string | undefined; ends?: string | undefined; set_as?: string | undefined; set_as_ref?: string | undefined } | { op: "rel.rm"; v?: number; source: string; kind?: string | undefined; target?: string | undefined; as?: string | undefined } | { op: "pkg.move"; v?: number; slug: string; to_dir: string } | { op: "pkg.rename"; v?: number; from: string; to: string } | { op: "pkg.delete"; v?: number; path: string; cascade?: boolean } | { op: "pkg.reorder"; v?: number; path: string; order?: string[] } | { op: "pkg.sort"; v?: number; path: string } | { op: "pkg.retitle"; v?: number; path: string; title: string } | { op: "pkg.insert"; v?: number; parent_path: string; name: string; docs?: [string, string][] } | { op: "diagram.set"; v?: number; key: string; title?: string | undefined; desc?: string | undefined; display?: DisplayDto | undefined };

export type RelationshipKind = "associates" | "aggregates" | "composes" | "specializes" | "implements" | "depends" | "annotates" | "includes" | "extends" | "instanceof" | "links";

export type Severity = "error" | "warning";

export type Shape = "Frame" | "Box" | "Shrink";


/**
 * `bundle`: a `[path, markdown][]`; `ops`: an `OpDto[]` (Tsify-generated union;
 * see `packages/wasm/src/generated/waml_wasm.d.ts`). Returns the edited bundle.
 */
export function apply_ops(bundle: any, ops: OpDto[]): any;

/**
 * `bundle`: a `[path, markdown][]`. Returns the resolved OKF `Bundle` (one
 * `Concept` per document). Additive to [`build_model`]; the UML surface is
 * untouched. `Concept.extra` (frontmatter) serializes as a plain JS object —
 * `serialize_maps_as_objects` matches its JSON semantics and the TS
 * `Record<string, FmValue>` type, not a `Map`.
 * Spike B (see docs/superpowers/plans/notes/2026-07-15-tsify-spike-findings.md) found
 * tsify's `into_wasm_abi` renders this shape as a JS `Map`, so the return stays `JsValue`
 * with this serializer rather than the now-Tsify'd `waml::okf::Bundle`.
 */
export function build_bundle(bundle: any): any;

/**
 * `bundle`: a `[path, markdown][]` (array of pairs). Returns the resolved `Model`.
 */
export function build_model(bundle: any): Model;

/**
 * `bundle`: a `[path, markdown][]`. Returns the canonicalized bundle.
 */
export function fmt(bundle: any): any;

export function init_panic_hook(): void;

/**
 * Markdown for one empty diagram document of `kind` (`"class"`/`"domain"`,
 * `"usecase"`, `"activity"`, `"sequence"`), titled `name`. The seed for the
 * New Package flow's Diagram tier.
 */
export function new_diagram_doc(kind: string, name: string): string;

/**
 * `bundle`: a `[path, markdown][]`. Returns the bundle with every
 * `<dir>/index.md` regenerated from the package forest.
 */
export function reindex(bundle: any): any;

/**
 * `bundle`: `[path, markdown][]`; `diagram_key`: which diagram to solve;
 * `sizes`: `Record<string, {w, h}>`; `cfg`: `SolveConfig | null | undefined`.
 * Returns `{ solved, diagnostics }`.
 */
export function solve(bundle: any, diagram_key: string, sizes: any, cfg: any): SolveResult;

/**
 * Split a multi-document bundle string into `[path, markdown][]`.
 */
export function split_bundle(text: string): any;

/**
 * `bundle`: a `[path, markdown][]`. Returns a `Diagnostic[]`.
 */
export function validate(bundle: any): Diagnostic[];

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly apply_ops: (a: any, b: number, c: number) => [number, number, number];
    readonly build_bundle: (a: any) => [number, number, number];
    readonly build_model: (a: any) => [number, number, number];
    readonly fmt: (a: any) => [number, number, number];
    readonly new_diagram_doc: (a: number, b: number, c: number, d: number) => [number, number];
    readonly reindex: (a: any) => [number, number, number];
    readonly solve: (a: any, b: number, c: number, d: any, e: any) => [number, number, number];
    readonly split_bundle: (a: number, b: number) => [number, number, number];
    readonly validate: (a: any) => [number, number, number, number];
    readonly init_panic_hook: () => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __externref_drop_slice: (a: number, b: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
