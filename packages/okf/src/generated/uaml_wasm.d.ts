/* tslint:disable */
/* eslint-disable */

/**
 * `bundle`: a `[path, markdown][]`; `ops`: an `OpDto[]`. Returns the edited bundle.
 */
export function apply_ops(bundle: any, ops: any): any;

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
    readonly build_model: (a: any) => [number, number, number];
    readonly fmt: (a: any) => [number, number, number];
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
