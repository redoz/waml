import type { Node } from "@xyflow/svelte";
import type { ModelNode } from "@uaml/okf";
import type { ViewMode } from "@uaml/core/state/viewMode";
import type { OkfNodeData } from "./nodes/types";

// NB (SvelteFlow-specific): clone `position` — SvelteFlow mutates node.position in
// place while dragging (bind:nodes), so sharing the model's position object would
// silently mutate the store. React Flow applied immutable changes, so it cloned via
// change objects; here we clone up front.
export function toRFNode(n: ModelNode, viewMode: ViewMode, profileName: string, collapsed = false): Node {
  return {
    id: n.key,
    type: "okf",
    position: { x: n.position.x, y: n.position.y },
    data: { ...n, _viewMode: viewMode, _profile: profileName, _collapsed: collapsed } as unknown as OkfNodeData & Record<string, unknown>,
  };
}
