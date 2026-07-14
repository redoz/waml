import type { Node } from "@xyflow/svelte";
import type { ModelNode, DiagramDisplay } from "@waml/okf";
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
