import { memo } from "react";
import { BaseEdge, getStraightPath, useInternalNode, type EdgeProps } from "@xyflow/react";
import { getEdgeParams } from "./floating";

function AnchorEdgeInner({ id, source, target }: EdgeProps) {
  const s = useInternalNode(source);
  const t = useInternalNode(target);
  if (!s || !t) return null;
  // Floating endpoints, but the dashed connector stays a straight line.
  const { sx, sy, tx, ty } = getEdgeParams(s, t);
  const [path] = getStraightPath({ sourceX: sx, sourceY: sy, targetX: tx, targetY: ty });
  return <BaseEdge id={id} path={path} style={{ stroke: "#94a3b8", strokeWidth: 1.2, strokeDasharray: "4 3" }} />;
}
export const AnchorEdge = memo(AnchorEdgeInner);
