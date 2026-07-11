import type { NodeTypes, EdgeTypes } from "@xyflow/svelte";
import OkfNode from "./nodes/OkfNode.svelte";
import RelEdge from "./RelEdge.svelte";
import AnchorEdge from "./AnchorEdge.svelte";

export const nodeTypes: NodeTypes = { okf: OkfNode };
export const edgeTypes: EdgeTypes = { rel: RelEdge, anchor: AnchorEdge };
