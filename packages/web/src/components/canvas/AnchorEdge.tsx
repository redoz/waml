import { memo } from "react";
import { BaseEdge, getStraightPath, type EdgeProps } from "@xyflow/react";

function AnchorEdgeInner({ id, sourceX, sourceY, targetX, targetY }: EdgeProps) {
  const [path] = getStraightPath({ sourceX, sourceY, targetX, targetY });
  return <BaseEdge id={id} path={path} style={{ stroke: "#94a3b8", strokeWidth: 1.2, strokeDasharray: "4 3" }} />;
}
export const AnchorEdge = memo(AnchorEdgeInner);
