// Pure translation of a requested store change into the `OpDto[]` that realizes
// it against the bundle. NO wasm calls here — the store (Task 3) feeds these ops
// to `apply_ops` and re-derives. This is where the array diffing lives.
//
// Wire tag → Rust variant map (crates/waml-ops-dto/src/lib.rs, #[serde(tag="op")]):
//   "node.new"  → NodeNew   { slug, ty, title, stereotype[], desc?, abstract? }
//   "node.set"  → NodeSet   { slug, title?, desc?, stereotype?, abstract?, ty? }
//   "node.rm"   → NodeRm    { slug, cascade? }
//   "node.rename" → NodeRename { from, to }
//   "attr.add"  → AttrAdd   { node, name, ty, mult?, vis? }
//   "attr.set"  → AttrSet   { node, name, ty?, mult?, vis?, rename? }
//   "attr.rm"   → AttrRm    { node, name }
//   "value.add" → ValueAdd  { node, literal }
//   "value.rm"  → ValueRm   { node, literal }
//   "rel.add"   → RelAdd    { source, kind, target, as?(label), as_ref?, ends? }
//   "rel.set"   → RelSet    { source, kind?, target? (selector), as? (selector),
//                            ends?, set_as? (new label), set_as_ref? (new ref) }
//   "rel.rm"    → RelRm     { source, kind?, target? (selector), as? (selector) }
//
// Matching rules (documented, tested):
//  • Attributes match by NAME. A name present in both is a "kept" attribute; a
//    changed field (type/mult/vis) on it emits `attr.set` with ONLY the changed
//    fields. Names only in `prev` are removals, only in `next` are additions; the
//    leftover removals and additions are PAIRED in order and each pair emits a
//    rename `attr.set` (old name → new name, carrying the new spec). Unpaired
//    leftovers fall back to `attr.rm` / `attr.add`.
//  • Values match by exact string equality.
//  • Edges are selected by their `source` + endpoint (`kind`+`target`) triple.
//    Changing an edge's kind or endpoints is not expressible as `rel.set` (they
//    are the selector), so it becomes `rel.rm` + `rel.add`. Only ends/name changes
//    on the same triple use `rel.set`. Canvas-only fields (handles) emit nothing.
import type { ModelNode, ModelEdge, Attribute, RelEnd, RelationshipKind, Visibility } from "@waml/okf";
import { ENDED_KINDS } from "@waml/okf";
// `OpDto` (and its nested `DisplayDto`) is generated from the Rust
// `waml-ops-dto` crate via Tsify (single source of truth); see
// crates/waml-ops-dto/src/lib.rs. The generated union is a superset of the
// old hand-written one here (adds the `diagram.set` variant and an optional
// `v?: number` version tag on every variant), so existing narrowings by `op`
// tag remain valid.
import type { OpDto } from "@waml/wasm";
export type { OpDto };

type EdgeName = string | { ref: string };

// ── helpers ──────────────────────────────────────────────────────────────────

function renderEnd(e: RelEnd): string {
  return `${e.multiplicity ?? "1"}${e.role ? ` ${e.role}` : ""}`;
}
function renderEnds(from: RelEnd, to: RelEnd): string {
  return `${renderEnd(from)} to ${renderEnd(to)}`;
}
function endEq(a: RelEnd, b: RelEnd): boolean {
  return (a.multiplicity ?? "1") === (b.multiplicity ?? "1") && (a.role ?? "") === (b.role ?? "");
}
function arrEq(a: string[], b: string[]): boolean {
  return a.length === b.length && a.every((x, i) => x === b[i]);
}
function nameEq(a: EdgeName | undefined, b: EdgeName | undefined): boolean {
  if (a === undefined || b === undefined) return a === b;
  if (typeof a === "string" || typeof b === "string") return a === b;
  return a.ref === b.ref;
}
function visMarker(v: Visibility | undefined): string | undefined {
  return v ?? undefined;
}
function isEnded(kind: RelationshipKind): boolean {
  return ENDED_KINDS.has(kind);
}

// ── nodes ──────────────────────────────────────────────────────────────────

export interface NewNodeFields {
  slug: string;
  dir?: string;
  type?: string;
  title?: string;
  stereotypes?: string[];
  description?: string;
  abstract?: boolean;
}

export function nodeNewOps(f: NewNodeFields): OpDto[] {
  return [
    {
      op: "node.new",
      slug: f.slug,
      ...(f.dir ? { dir: f.dir } : {}),
      ty: f.type ?? "uml.Class",
      title: f.title ?? "New object",
      ...(f.stereotypes && f.stereotypes.length ? { stereotype: f.stereotypes } : {}),
      ...(f.description ? { desc: f.description } : {}),
      ...(f.abstract ? { abstract: true } : {}),
    },
  ];
}

// ── packages ─────────────────────────────────────────────────────────────────

export function moveNodeOps(slug: string, toDir: string): OpDto[] {
  return [{ op: "pkg.move", slug, to_dir: toDir }];
}
export function renamePackageOps(from: string, to: string): OpDto[] {
  return from === to ? [] : [{ op: "pkg.rename", from, to }];
}
export function deletePackageOps(path: string, cascade: boolean): OpDto[] {
  return [{ op: "pkg.delete", path, cascade }];
}
export function reorderMembersOps(path: string, order: string[]): OpDto[] {
  return [{ op: "pkg.reorder", path, order }];
}
export function sortPackageOps(path: string): OpDto[] {
  return [{ op: "pkg.sort", path }];
}

export function nodeRenameOps(from: string, to: string): OpDto[] {
  return from === to ? [] : [{ op: "node.rename", from, to }];
}

export function nodeRmOps(slug: string, cascade = true): OpDto[] {
  return [{ op: "node.rm", slug, cascade }];
}

/** Scalar node fields only (title/description/stereotypes/abstract/type). Emits a
 *  single `node.set` carrying just the fields that actually changed, or `[]`. */
export function nodeSetOps(prev: ModelNode, patch: Partial<ModelNode>): OpDto[] {
  const set: Omit<Extract<OpDto, { op: "node.set" }>, "op" | "slug"> = {};
  // Title/description edits ride the concept (the single authoritative source);
  // the emitted node.set still mutates doc frontmatter, which build_model
  // re-derives back into concept.*.
  if (patch.concept?.title !== undefined && patch.concept.title !== prev.concept.title) set.title = patch.concept.title;
  if (patch.concept?.description !== undefined && patch.concept.description !== prev.concept.description) set.desc = patch.concept.description;
  if (patch.stereotypes !== undefined && !arrEq(patch.stereotypes, prev.stereotypes)) set.stereotype = patch.stereotypes;
  if (patch.abstract !== undefined && !!patch.abstract !== !!prev.abstract) set.abstract = !!patch.abstract;
  if (patch.type !== undefined && patch.type !== prev.type) set.ty = patch.type;
  return Object.keys(set).length ? [{ op: "node.set", slug: prev.key, ...set }] : [];
}

// ── attributes (array diff) ──────────────────────────────────────────────────

function attrAddOp(node: string, a: Attribute): Extract<OpDto, { op: "attr.add" }> {
  return {
    op: "attr.add",
    node,
    name: a.name,
    ty: a.type.name,
    ...(a.multiplicity && a.multiplicity !== "1" ? { mult: a.multiplicity } : {}),
    ...(visMarker(a.visibility) ? { vis: visMarker(a.visibility) } : {}),
  };
}

/** Fields of a kept attribute that changed (type token / multiplicity / visibility). */
function attrFieldChanges(prev: Attribute, next: Attribute): { ty?: string; mult?: string; vis?: string } {
  const out: { ty?: string; mult?: string; vis?: string } = {};
  if (next.type.name !== prev.type.name) out.ty = next.type.name;
  if ((next.multiplicity ?? "1") !== (prev.multiplicity ?? "1")) out.mult = next.multiplicity ?? "1";
  if (visMarker(next.visibility) !== visMarker(prev.visibility)) out.vis = visMarker(next.visibility);
  return out;
}

export function attrDiffOps(node: string, prev: Attribute[], next: Attribute[]): OpDto[] {
  const ops: OpDto[] = [];
  const prevByName = new Map(prev.map((a) => [a.name, a]));
  const nextByName = new Map(next.map((a) => [a.name, a]));

  // kept (same name): a changed field → attr.set with only the changed fields.
  for (const a of next) {
    const p = prevByName.get(a.name);
    if (!p) continue;
    const changes = attrFieldChanges(p, a);
    if (Object.keys(changes).length) ops.push({ op: "attr.set", node, name: a.name, ...changes });
  }

  const removed = prev.filter((a) => !nextByName.has(a.name));
  const added = next.filter((a) => !prevByName.has(a.name));
  const pairs = Math.min(removed.length, added.length);
  // paired leftovers = renames: old name → new name, carrying the new spec.
  for (let i = 0; i < pairs; i++) {
    const r = removed[i];
    const a = added[i];
    ops.push({
      op: "attr.set",
      node,
      name: r.name,
      rename: a.name,
      ty: a.type.name,
      mult: a.multiplicity ?? "1",
      ...(visMarker(a.visibility) ? { vis: visMarker(a.visibility) } : {}),
    });
  }
  for (let i = pairs; i < removed.length; i++) ops.push({ op: "attr.rm", node, name: removed[i].name });
  for (let i = pairs; i < added.length; i++) ops.push(attrAddOp(node, added[i]));
  return ops;
}

// ── values (array diff) ──────────────────────────────────────────────────────

export function valueDiffOps(node: string, prev: string[], next: string[]): OpDto[] {
  const ops: OpDto[] = [];
  const prevSet = new Set(prev);
  const nextSet = new Set(next);
  for (const v of next) if (!prevSet.has(v)) ops.push({ op: "value.add", node, literal: v });
  for (const v of prev) if (!nextSet.has(v)) ops.push({ op: "value.rm", node, literal: v });
  return ops;
}

// ── composite: an `updateNode(key, patch)` → ops ─────────────────────────────

/** Translate a whole `updateNode` patch. Position/extra and any other canvas-only
 *  field contribute no ops (the store keeps those in the overlay). */
export function updateNodeOps(prev: ModelNode, patch: Partial<ModelNode>): OpDto[] {
  const ops = nodeSetOps(prev, patch);
  if (patch.attributes) ops.push(...attrDiffOps(prev.key, prev.attributes, patch.attributes));
  if (patch.values) ops.push(...valueDiffOps(prev.key, prev.values ?? [], patch.values));
  return ops;
}

// ── edges ────────────────────────────────────────────────────────────────────

function nameAddFields(name: EdgeName | undefined): { as?: string; as_ref?: string } {
  if (name === undefined) return {};
  return typeof name === "string" ? { as: name } : { as_ref: name.ref };
}
function nameSetFields(name: EdgeName | undefined): { set_as?: string; set_as_ref?: string } {
  if (name === undefined) return {};
  return typeof name === "string" ? { set_as: name } : { set_as_ref: name.ref };
}

function relAddOp(
  source: string,
  kind: RelationshipKind,
  target: string,
  name: EdgeName | undefined,
  fromEnd: RelEnd,
  toEnd: RelEnd,
): Extract<OpDto, { op: "rel.add" }> {
  return {
    op: "rel.add",
    source,
    kind,
    target,
    ...nameAddFields(name),
    ...(isEnded(kind) ? { ends: renderEnds(fromEnd, toEnd) } : {}),
  };
}

/** A brand-new edge (store.addEdge defaults to an `associates` with `1 to 1` ends). */
export function edgeAddOps(from: string, to: string, kind: RelationshipKind = "associates"): OpDto[] {
  return [relAddOp(from, kind, to, undefined, {}, { navigable: true })];
}

export function edgeRmOps(prev: ModelEdge): OpDto[] {
  return [{ op: "rel.rm", source: prev.from, kind: prev.kind, target: prev.to }];
}

/** Translate an `updateEdge(id, patch)`. Kind/endpoint changes → rm+add; ends/name
 *  changes on the same triple → `rel.set`; handles-only → `[]`. */
export function edgeSetOps(prev: ModelEdge, patch: Partial<ModelEdge>): OpDto[] {
  const newKind = patch.kind ?? prev.kind;
  const newFrom = patch.from ?? prev.from;
  const newTo = patch.to ?? prev.to;
  const newName = "name" in patch ? patch.name : prev.name;
  const newFromEnd = patch.fromEnd ?? prev.fromEnd;
  const newToEnd = patch.toEnd ?? prev.toEnd;

  if (newKind !== prev.kind || newFrom !== prev.from || newTo !== prev.to) {
    return [...edgeRmOps(prev), relAddOp(newFrom, newKind, newTo, newName, newFromEnd, newToEnd)];
  }

  const set: Extract<OpDto, { op: "rel.set" }> = { op: "rel.set", source: prev.from, kind: prev.kind, target: prev.to };
  let changed = false;
  if (isEnded(prev.kind) && (patch.fromEnd !== undefined || patch.toEnd !== undefined)) {
    if (!endEq(newFromEnd, prev.fromEnd) || !endEq(newToEnd, prev.toEnd)) {
      set.ends = renderEnds(newFromEnd, newToEnd);
      changed = true;
    }
  }
  if ("name" in patch && !nameEq(patch.name, prev.name)) {
    Object.assign(set, nameSetFields(patch.name));
    changed = true;
  }
  return changed ? [set] : [];
}
