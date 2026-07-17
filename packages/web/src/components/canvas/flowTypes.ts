import type { NodeTypes, EdgeTypes } from "@xyflow/svelte";
import OkfNode from "./nodes/OkfNode.svelte";
import GroupFrame from "./nodes/GroupFrame.svelte";
import RelEdge from "./RelEdge.svelte";
import AnchorEdge from "./AnchorEdge.svelte";

export const nodeTypes: NodeTypes = { okf: OkfNode, "group-frame": GroupFrame };
export const edgeTypes: EdgeTypes = { rel: RelEdge, anchor: AnchorEdge };
