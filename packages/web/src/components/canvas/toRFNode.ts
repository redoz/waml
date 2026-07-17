import type { Node } from "@xyflow/svelte";
import type { ModelNode, DiagramDisplay } from "@waml/okf";
import type { SolvedGroup } from "@waml/wasm";
import type { OkfNodeData } from "./nodes/types";

// NB (SvelteFlow-specific): clone `position` — SvelteFlow mutates node.position in
// place while dragging (bind:nodes), so sharing the model's position object would
// silently mutate the store. React Flow applied immutable changes, so it cloned via
// change objects; here we clone up front.
//
// `_display` is the ACTIVE diagram's resolved DiagramDisplay — it replaces the old
// global `_viewMode` flag and drives whether/how attributes + stereotypes render.
export function toRFNode(n: ModelNode, display: DiagramDisplay, profileName: string, collapsed = false): Node {
  return {
    id: n.key,
    type: "okf",
    position: { x: n.position.x, y: n.position.y },
    data: { ...n, _display: display, _profile: profileName, _collapsed: collapsed } as unknown as OkfNodeData & Record<string, unknown>,
  };
}

// A solved layout group → a non-interactive SvelteFlow backdrop node. Only
// `shape === "Frame"` renders chrome; `Box`/`Shrink` shaped the layout but draw
// nothing, so they map to null. The `"__group__" + index` id never collides with
// a model node key, so the canvas's selection/drag/delete handlers ignore these
// pseudo-nodes without any special-casing.
export function toGroupNode(group: SolvedGroup, index: number): Node | null {
  if (group.shape !== "Frame") return null;
  return {
    id: "__group__" + index,
    type: "group-frame",
    position: { x: group.rect.x, y: group.rect.y },
    data: { title: group.title, width: group.rect.w, height: group.rect.h } as unknown as Record<string, unknown>,
    width: group.rect.w,
    height: group.rect.h,
    style: `width:${group.rect.w}px;height:${group.rect.h}px;`,
    selectable: false,
    draggable: false,
    deletable: false,
    zIndex: group.depth - 1000,
  } as Node;
}
