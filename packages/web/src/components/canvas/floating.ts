import { Position } from "@xyflow/react";

// Standard React Flow floating-edge math: each edge computes its own attach point
// on a node's border facing the other node, so a hub's many edges fan out across
// all four borders instead of stacking on one fixed port.
export type NodeGeom = {
  measured?: { width?: number; height?: number };
  internals: { positionAbsolute: { x: number; y: number } };
};

function getNodeIntersection(intersectionNode: NodeGeom, targetNode: NodeGeom) {
  const w = (intersectionNode.measured?.width ?? 0) / 2;
  const h = (intersectionNode.measured?.height ?? 0) / 2;
  const x2 = intersectionNode.internals.positionAbsolute.x + w;
  const y2 = intersectionNode.internals.positionAbsolute.y + h;
  const x1 = targetNode.internals.positionAbsolute.x + (targetNode.measured?.width ?? 0) / 2;
  const y1 = targetNode.internals.positionAbsolute.y + (targetNode.measured?.height ?? 0) / 2;
  const xx1 = (x1 - x2) / (2 * w) - (y1 - y2) / (2 * h);
  const yy1 = (x1 - x2) / (2 * w) + (y1 - y2) / (2 * h);
  const a = 1 / (Math.abs(xx1) + Math.abs(yy1));
  const xx3 = a * xx1;
  const yy3 = a * yy1;
  const x = w * (xx3 + yy3) + x2;
  const y = h * (-xx3 + yy3) + y2;
  return { x, y };
}

function getEdgePosition(node: NodeGeom, point: { x: number; y: number }): Position {
  const nx = Math.round(node.internals.positionAbsolute.x);
  const ny = Math.round(node.internals.positionAbsolute.y);
  const nw = node.measured?.width ?? 0;
  const nh = node.measured?.height ?? 0;
  const px = Math.round(point.x);
  const py = Math.round(point.y);
  if (px <= nx + 1) return Position.Left;
  if (px >= nx + nw - 1) return Position.Right;
  if (py <= ny + 1) return Position.Top;
  if (py >= ny + nh - 1) return Position.Bottom;
  return Position.Top;
}

// A node with no measured size yet would divide by zero and produce NaN — fall
// back to the node centre and a sane horizontal side so the path is still valid.
function hasSize(n: NodeGeom): boolean {
  return !!n.measured?.width && !!n.measured?.height;
}
function nodeCenter(n: NodeGeom) {
  return {
    x: n.internals.positionAbsolute.x + (n.measured?.width ?? 0) / 2,
    y: n.internals.positionAbsolute.y + (n.measured?.height ?? 0) / 2,
  };
}

export function getEdgeParams(source: NodeGeom, target: NodeGeom) {
  const okSource = hasSize(source);
  const okTarget = hasSize(target);
  const sp = okSource ? getNodeIntersection(source, target) : nodeCenter(source);
  const tp = okTarget ? getNodeIntersection(target, source) : nodeCenter(target);
  return {
    sx: sp.x,
    sy: sp.y,
    tx: tp.x,
    ty: tp.y,
    sourcePos: okSource ? getEdgePosition(source, sp) : Position.Right,
    targetPos: okTarget ? getEdgePosition(target, tp) : Position.Left,
  };
}
