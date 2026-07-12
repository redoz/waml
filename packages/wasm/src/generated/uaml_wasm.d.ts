/* tslint:disable */
/* eslint-disable */
/**
 * Result of solving one diagram: absolute rects + any layout diagnostics.
 * Tsify emits its TypeScript type; under `wasm` it crosses the boundary as a
 * plain JS object.
 */
export interface SolveResult {
    solved: Solved;
    diagnostics: Diagnostic[];
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

export interface FlagSet {
    emphasized: boolean;
    collapsed: boolean;
}

export interface Rect {
    x: number;
    y: number;
    w: number;
    h: number;
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

export type DiagCode = "duplicate-slug" | "frontmatter-not-clean" | "unknown-type" | "malformed-attribute" | "malformed-relationship" | "unresolved-target" | "droppable-content" | "malformed-layout" | "unresolved-layout-ref" | "layout-cycle" | "layout-conflict";

export type Severity = "error" | "warning";

export type Shape = "Frame" | "Box" | "Shrink";


/**
 * `bundle`: a `[path, markdown][]`; `ops`: an `OpDto[]`. Returns the edited bundle.
 */
export function apply_ops(bundle: any, ops: any): any;

/**
 * `bundle`: a `[path, markdown][]`. Returns the resolved OKF `Bundle` (one
 * `Concept` per document). Additive to [`build_model`]; the UML surface is
 * untouched. `Concept.extra` (frontmatter) serializes as a plain JS object —
 * `serialize_maps_as_objects` matches its JSON semantics and the TS
 * `Record<string, FmValue>` type, not a `Map`.
 */
export function build_bundle(bundle: any): any;

/**
 * `bundle`: a `[path, markdown][]` (array of pairs). Returns the resolved `Model`.
 */
export function build_model(bundle: any): any;

/**
 * `bundle`: a `[path, markdown][]`. Returns the canonicalized bundle.
 */
export function fmt(bundle: any): any;

export function init_panic_hook(): void;

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
export function validate(bundle: any): any;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly apply_ops: (a: any, b: any) => [number, number, number];
    readonly build_bundle: (a: any) => [number, number, number];
    readonly build_model: (a: any) => [number, number, number];
    readonly fmt: (a: any) => [number, number, number];
    readonly reindex: (a: any) => [number, number, number];
    readonly solve: (a: any, b: number, c: number, d: any, e: any) => [number, number, number];
    readonly split_bundle: (a: number, b: number) => [number, number, number];
    readonly validate: (a: any) => [number, number, number];
    readonly init_panic_hook: () => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
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
