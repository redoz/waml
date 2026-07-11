import { memo } from "react";
import { BaseEdge, EdgeLabelRenderer, getBezierPath, type EdgeProps } from "@xyflow/react";
import type { ModelEdge, RelEnd, RelationshipKind } from "@mc/okf";
import type { RelLabelMode } from "../../state/relLabels";

export type RelEdgeData = Pick<ModelEdge, "kind" | "fromEnd" | "toEnd" | "bidirectional"> & {
  relLabelMode?: RelLabelMode;
  modelEdgeId?: string;
};

const DASHED: ReadonlySet<RelationshipKind> = new Set(["implements", "depends"]);

function RelEdgeInner(props: EdgeProps) {
  const { id, sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, data, selected } = props;
  const d = data as unknown as RelEdgeData | undefined;
  const kind: RelationshipKind = d?.kind ?? "associates";
  const fromEnd: RelEnd = d?.fromEnd ?? {};
  const toEnd: RelEnd = d?.toEnd ?? {};
  const mode: RelLabelMode = d?.relLabelMode ?? "all";

  const [edgePath] = getBezierPath({ sourceX, sourceY, sourcePosition, targetX, targetY, targetPosition });
  const stroke = selected ? "#1e88e5" : "#64748b";
  const strokeWidth = selected ? 2.5 : 1.8;

  // Verb → end adornments (spec table).
  let markerStart: string | undefined;
  let markerEnd: string | undefined;
  const defs: React.ReactNode[] = [];
  const diamond = (fill: string, mid: string) => (
    <marker key={mid} id={`${mid}-${id}`} markerWidth="14" markerHeight="10" refX="1" refY="5" orient="auto" markerUnits="userSpaceOnUse">
      <path d="M1,5 L7,1 L13,5 L7,9 z" fill={fill} stroke={stroke} strokeWidth="1" />
    </marker>
  );
  const triangle = (
    <marker key="triangle" id={`triangle-${id}`} markerWidth="14" markerHeight="12" refX="12" refY="6" orient="auto" markerUnits="userSpaceOnUse">
      <path d="M1,1 L12,6 L1,11 z" fill="#fff" stroke={stroke} strokeWidth="1.2" />
    </marker>
  );
  const arrow = (key: string, flip: boolean) => (
    <marker key={key} id={`${key}-${id}`} markerWidth="12" markerHeight="12" refX={flip ? 1 : 10} refY="6" orient="auto" markerUnits="userSpaceOnUse">
      <path d={flip ? "M10,1 L1,6 L10,11" : "M1,1 L10,6 L1,11"} fill="none" stroke={stroke} strokeWidth="1.5" />
    </marker>
  );

  if (kind === "composes") { defs.push(diamond(stroke, "diamond-filled")); markerStart = `url(#diamond-filled-${id})`; }
  else if (kind === "aggregates") { defs.push(diamond("#fff", "diamond-hollow")); markerStart = `url(#diamond-hollow-${id})`; }
  else if (kind === "specializes" || kind === "implements") { defs.push(triangle); markerEnd = `url(#triangle-${id})`; }
  else if (kind === "depends") { defs.push(arrow("dep-arrow", false)); markerEnd = `url(#dep-arrow-${id})`; }
  else { // associates: arrowhead on navigable end(s)
    if (toEnd.navigable) { defs.push(arrow("nav-end", false)); markerEnd = `url(#nav-end-${id})`; }
    if (fromEnd.navigable) { defs.push(arrow("nav-start", true)); markerStart = `url(#nav-start-${id})`; }
  }

  const endText = (e: RelEnd) => [e.multiplicity, e.role].filter(Boolean).join(" ");
  const showLabels = mode !== "hidden";
  const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
  const labels: { x: number; y: number; text: string }[] = [];
  if (showLabels) {
    const ft = endText(fromEnd); const tt = endText(toEnd);
    if (ft) labels.push({ x: lerp(sourceX, targetX, 0.18), y: lerp(sourceY, targetY, 0.18) - 10, text: ft });
    if (tt) labels.push({ x: lerp(sourceX, targetX, 0.82), y: lerp(sourceY, targetY, 0.82) - 10, text: tt });
  }

  return (
    <>
      <defs>{defs}</defs>
      <BaseEdge id={id} path={edgePath} markerStart={markerStart} markerEnd={markerEnd}
        style={{ stroke, strokeWidth, ...(DASHED.has(kind) ? { strokeDasharray: "6 4" } : {}) }} />
      {labels.length > 0 && (
        <EdgeLabelRenderer>
          {labels.map((l, i) => (
            <div key={i} className="nodrag nopan"
              style={{ position: "absolute", transform: `translate(-50%, -50%) translate(${l.x}px,${l.y}px)`,
                background: "rgba(255,255,255,0.9)", borderRadius: 4, padding: "0 4px",
                fontSize: 10.5, fontWeight: 600, color: "#334155", pointerEvents: "all", whiteSpace: "nowrap" }}>
              {l.text}
            </div>
          ))}
        </EdgeLabelRenderer>
      )}
    </>
  );
}

export const RelEdge = memo(RelEdgeInner);
