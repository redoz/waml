import { memo } from "react";
import type { NodeProps } from "@xyflow/react";
import { resolveNodeRenderer } from "./registry";
import type { OkfNodeData } from "./shared";

function OkfNodeInner({ data }: NodeProps) {
  const node = data as unknown as OkfNodeData;
  const Renderer = resolveNodeRenderer(node.type);
  return <Renderer data={node} />;
}

export const OkfNode = memo(OkfNodeInner);
