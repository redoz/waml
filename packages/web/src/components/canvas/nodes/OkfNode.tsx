import { memo } from "react";
import type { NodeProps } from "@xyflow/react";
import { resolveNodeRenderer } from "./registry";
import { NodePorts, type OkfNodeData } from "./shared";

function OkfNodeInner({ data }: NodeProps) {
  const node = data as unknown as OkfNodeData;
  // A collapsed diagram member renders as a compact ref chip (a "drawn as ref chip"
  // hint), keeping off-focus classifiers present but small.
  if (node._collapsed) {
    return (
      <div className="relative rounded-full border border-[#d8dee8] bg-white px-3 py-[6px] text-[12px] font-[600] text-slate-600 shadow-sm">
        <NodePorts />
        <span className="relative z-[1]">{node.title}</span>
      </div>
    );
  }
  const Renderer = resolveNodeRenderer(node.type);
  return <Renderer data={node} />;
}

export const OkfNode = memo(OkfNodeInner);
