/* tslint:disable */
/* eslint-disable */
/**
 * A `uml.Note` anchor. Three forms, per the spec.
 */
export type NoteAnchor = { targetKey: string } | { sourceKey: string; name: string } | { sourceKey: string; kind: RelationshipKind; targetKey: string };

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
 * A genuine UML Classifier node\'s payload (design spec §3.1).
 */
export interface Classifier {
    kind: ClassifierKind;
    stereotypes: string[];
    abstract?: boolean;
    attributes: Attribute[];
    values?: string[];
}

/**
 * A resolved membership group in a diagram (heading text + resolved keys).
 */
export interface DiagramGroup {
    name: string;
    members: string[];
    children: DiagramGroup[];
}

/**
 * A resolved node of a flow document.
 */
export interface FlowNode {
    /**
     * Heading text minus the kind keyword — the name transitions resolve against.
     */
    id: string;
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
 * A resolved transition (flow edge). Source/target are node identities.
 */
export interface FlowEdge {
    from: string;
    /**
     * Local node identity, or the link title for a cross-document target.
     */
    to: string;
    /**
     * Resolved key when the target was a cross-document link.
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
 * A sequence participant node. Constructed by slice 3.
 */
export interface Lifeline {
    ref?: string;
    alias?: string;
}

/**
 * A sequence participant: IS Class or Actor, referenced by link.
 */
export interface Lifeline {
    title: string;
    alias?: string;
    /**
     * Resolved key of the classifier this lifeline is; None when unresolved.
     */
    ref?: string;
}

/**
 * Activity/state-machine control pseudostates. Slice 2.
 */
export type PseudostateKind = "Initial" | "Final" | "Decision" | "Merge" | "Fork" | "Join";

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
 * Behavior/interaction node payloads. Constructed by slices 2–3.
 */
export type BehaviorElement = { Action: FlowBody } | { State: FlowBody } | { Pseudostate: PseudostateKind } | { ObjectNode: { object_ref?: string } } | { Fragment: { kind: FragmentKind } } | { Operand: { guard?: string } };

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
 * Flat diagram DTO. `members` is flattened from the object model\'s `groups` in
 * Rust (moves `overlay.ts::flattenGroups` into Rust). `display`/`layout` unchanged.
 */
export interface WireDiagram {
    key: string;
    title: string;
    profile: string;
    description?: string;
    members: string[];
    display?: DiagramDisplay;
    layout: unknown[];
}

/**
 * Flat node DTO == today\'s `ModelNode` minus `position` (position is TS overlay
 * state). Field names/serde match the pre-reshape `Node` exactly.
 */
export interface WireNode {
    concept: Concept;
    key: string;
    type: string;
    stereotypes: string[];
    abstract?: boolean;
    attributes: Attribute[];
    values?: string[];
    note_body?: string;
    annotates?: NoteAnchor[];
    members?: string[];
}

/**
 * Flow flavor: tunes rendering only — one grammar for both.
 */
export type FlowFlavor = "activity" | "stateMachine";

/**
 * Flow transition edge payload (design spec §3.2). Constructed by slice 2.
 */
export interface Transition {
    trigger?: string;
    guard?: string;
    else?: boolean;
    effect?: string;
    carries?: string;
    toRef?: string;
}

/**
 * Interaction message edge payload (design spec §3.2). Constructed by slice 3.
 */
export interface Message {
    verb: MessageVerb;
    signature?: string;
    seq: number;
}

/**
 * Non-classifier structural elements: packages and notes/comments (spec §3.1).
 */
export type Structural = { Package: { members?: string[] } } | { Note: { body?: string; annotates?: NoteAnchor[] } };

/**
 * One flow document: one self-rendering directed graph (model AND view).
 */
export interface FlowDoc {
    key: string;
    title: string;
    flavor: FlowFlavor;
    /**
     * Resolved key of the entity this behavior describes (frontmatter link).
     */
    describes?: string;
    nodes: FlowNode[];
    edges: FlowEdge[];
}

/**
 * One operand of a combined fragment. `guard: None` = the `else` operand.
 */
export interface SeqOperand {
    guard?: string;
    items: SeqItem[];
}

/**
 * One ordered interaction item: document order is time order.
 */
export type SeqItem = { item: "message"; from: string; verb: MessageVerb; to: string; signature?: string } | { item: "fragment"; kind: FragmentKind; operands: SeqOperand[] };

/**
 * One sequence document: lifelines + ordered messages (model AND view).
 */
export interface SequenceDoc {
    key: string;
    title: string;
    describes?: string;
    lifelines: Lifeline[];
    messages: SeqItem[];
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
 * Shared behavior-node body (activity action / state-machine state). Slice 2.
 */
export interface FlowBody {
    partition?: string;
    entry?: string;
    do?: string;
    exit?: string;
    refines?: string;
}

/**
 * Structural relationship edge payload (design spec §3.2). Absorbs the old
 * `Edge` association fields.
 */
export interface Relationship {
    kind: RelationshipKind;
    name?: string | { ref: string };
    fromEnd: RelEnd;
    toEnd: RelEnd;
    bidirectional: boolean;
}

/**
 * The classifier subset of the UML metaclass set (design spec §3.1). `Package`
 * and `Note` are NOT here — they are `Structural`, not classifiers.
 */
export type ClassifierKind = "Class" | "Interface" | "Enum" | "DataType" | "Association" | "Actor" | "UseCase";

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
 * The message kind: fixes line and arrowhead (interaction substrate).
 */
export type MessageVerb = "calls" | "sends" | "replies" | "creates" | "destroys";

/**
 * The ontology discriminator for a substrate `Node`. `Uml(..)` isolates every
 * UML concept behind one arm (design spec §2); a future ontology is a new arm +
 * a new module. `Unknown(String)` keeps graceful degradation at the ontology
 * layer, carrying the opaque `type` token.
 */
export type NodeKind = { Uml: UmlNode } | { Unknown: string };

/**
 * The ontology-agnostic substrate node (design spec §2): identity (`key`),
 * render label (`label`), and its ontology payload (`kind`). The OKF `Concept`
 * is NOT here — it is a parse-time projection of storage, kept on
 * `Model.concepts` and re-joined only on the Rust wire projection
 * (`crate::wire`). UML-specific data lives behind `kind` in `crate::uml`;
 * callers reach it via the accessors below (never a raw field/variant match).
 */
export interface Node {
    key: string;
    label: string;
    kind: NodeKind;
}

/**
 * UML diagram render payload (design spec §3.3): a flavor tag plus the render
 * fields moved off the substrate. `profile`/`description` are retained here (spec
 * §3.3 under-specifies them; keeping them avoids a lossy round-trip).
 */
export interface UmlDiagram {
    flavor: UmlDiagramFlavor;
    profile: string;
    description?: string;
    groups: DiagramGroup[];
    display?: DiagramDisplay;
    layout: unknown[];
}

/**
 * UML node payload, grouped by metamodel category (design spec §3.1). An ENUM:
 * the OKF `Concept` does NOT ride here (spec §2). The grouping — not a runtime
 * table — decides `is_classifier`.
 */
export type UmlNode = { Classifier: Classifier } | { Structural: Structural } | { Behavior: BehaviorElement } | { Lifeline: Lifeline };

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
     * Interaction-substrate behavior documents (uml.Sequence).
     */
    interactions?: SequenceDoc[];
    /**
     * Parse-time OKF projection of each node\'s source document (design spec §2:
     * Concept is a projection of storage, off the object-model `Node`). Keyed by
     * `node.key` (packages key by their dir; the `Concept.id` inside keeps its
     * natural path). `build_wire` re-joins it onto the wire; native readers use
     * `Model::concept`. Not part of the wire — the wire re-flattens it.
     */
    concepts?: Record<string, Concept>;
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

export interface WireEdge {
    from: string;
    to: string;
    kind: RelationshipKind;
    name?: string | { ref: string };
    fromEnd: RelEnd;
    toEnd: RelEnd;
    bidirectional: boolean;
}

export interface WireGraph {
    nodes: WireNode[];
    edges: WireEdge[];
    diagrams: WireDiagram[];
    path?: string;
    packages?: WireNode[];
    flows?: FlowDoc[];
    interactions?: SequenceDoc[];
}

export type DiagCode = "duplicate-slug" | "frontmatter-not-clean" | "unknown-type" | "malformed-attribute" | "malformed-relationship" | "malformed-flow-bullet" | "duplicate-flow-node" | "unresolved-target" | "droppable-content" | "malformed-layout" | "unresolved-layout-ref" | "layout-cycle" | "layout-conflict" | "malformed-message" | "malformed-lifeline";

export type DiagramKind = { Uml: UmlDiagram } | { Unknown: string };

export type EdgeKind = { Uml: UmlEdge } | { Unknown: string };

export type FmValue = string | boolean | number | FmValue[];

export type OpDto = { op: "node.new"; v?: number; slug: string; dir?: string; ty: string; title: string; stereotype?: string[]; desc?: string | undefined; abstract?: boolean } | { op: "node.rename"; v?: number; from: string; to: string } | { op: "node.set"; v?: number; slug: string; title?: string | undefined; desc?: string | undefined; stereotype?: string[] | undefined; abstract?: boolean | undefined; ty?: string | undefined } | { op: "node.rm"; v?: number; slug: string; cascade?: boolean } | { op: "attr.add"; v?: number; node: string; name: string; ty: string; mult?: string | undefined; vis?: string | undefined } | { op: "attr.set"; v?: number; node: string; name: string; ty?: string | undefined; mult?: string | undefined; vis?: string | undefined; rename?: string | undefined } | { op: "attr.rm"; v?: number; node: string; name: string } | { op: "value.add"; v?: number; node: string; literal: string } | { op: "value.rm"; v?: number; node: string; literal: string } | { op: "rel.add"; v?: number; source: string; kind: string; target: string; as?: string | undefined; as_ref?: string | undefined; ends?: string | undefined } | { op: "rel.set"; v?: number; source: string; kind?: string | undefined; target?: string | undefined; as?: string | undefined; ends?: string | undefined; set_as?: string | undefined; set_as_ref?: string | undefined } | { op: "rel.rm"; v?: number; source: string; kind?: string | undefined; target?: string | undefined; as?: string | undefined } | { op: "pkg.move"; v?: number; slug: string; to_dir: string } | { op: "pkg.rename"; v?: number; from: string; to: string } | { op: "pkg.delete"; v?: number; path: string; cascade?: boolean } | { op: "pkg.reorder"; v?: number; path: string; order?: string[] } | { op: "pkg.sort"; v?: number; path: string } | { op: "pkg.retitle"; v?: number; path: string; title: string } | { op: "pkg.insert"; v?: number; parent_path: string; name: string; docs?: [string, string][] } | { op: "diagram.set"; v?: number; key: string; title?: string | undefined; desc?: string | undefined; display?: DisplayDto | undefined };

export type RelationshipKind = "associates" | "aggregates" | "composes" | "specializes" | "implements" | "depends" | "annotates" | "includes" | "extends";

export type Severity = "error" | "warning";

export type Shape = "Frame" | "Box" | "Shrink";

export type UmlDiagramFlavor = "Class" | "Activity" | "StateMachine" | "Sequence" | "UseCase";

export type UmlEdge = { Relationship: Relationship } | { Transition: Transition } | { Message: Message } | "Containment";


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
 * `bundle`: a `[path, markdown][]` (array of pairs). Returns the resolved `WireGraph`.
 */
export function build_model(bundle: any): WireGraph;

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
