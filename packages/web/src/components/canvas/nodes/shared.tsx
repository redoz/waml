import { useState } from "react";
import { Handle, Position } from "@xyflow/react";
import { ChevronDown, ChevronRight } from "lucide-react";
import type { Attribute, ModelNode } from "@mc/okf";
import type { ViewMode } from "@mc/core/state/viewMode";
import { ERD_COLLAPSED_ROWS } from "../layoutSize";
import { getProfile, stereotypeStyle } from "../../../profiles";

export type OkfNodeData = ModelNode & { _viewMode?: ViewMode; _profile?: string; _collapsed?: boolean };
export interface OkfNodeProps { data: OkfNodeData }

export const NODE_FONT = "-apple-system, BlinkMacSystemFont, 'Segoe UI', Inter, system-ui, sans-serif";

// Connection ports — a small dot at each side, revealed on node hover/selection
// (.mart-handle in canvas.css). They're the drag-to-connect affordance and the
// reconnect drop target; ConnectionMode.Loose lets any source handle also receive.
// Floating edges compute their own border attach point, so which dot a connection
// lands on never affects routing — the four dots just give the user a port to grab
// on whichever side faces the target. They sit ABOVE the content (zIndex 10) so a
// drag that starts on a dot begins a connection, while a drag on the node body
// still moves the node and clicks on rows/buttons still register.
export function NodePorts() {
  // Centre each dot exactly on its border midpoint (half in / half out) with an
  // explicit transform, overriding React Flow's per-side default offsets so all
  // four straddle the edge symmetrically and stay grabbable from just inside.
  const dot = {
    width: 11, height: 11, borderRadius: "50%",
    background: "#fff", border: "2px solid #1e88e5",
    opacity: 0, transition: "opacity 0.12s", zIndex: 10,
    transform: "translate(-50%, -50%)",
  } as const;
  return (
    <>
      <Handle type="source" position={Position.Left} id="l" isConnectable className="mart-handle" style={{ ...dot, left: 0, top: "50%" }} />
      <Handle type="source" position={Position.Right} id="r" isConnectable className="mart-handle" style={{ ...dot, left: "100%", top: "50%" }} />
      <Handle type="source" position={Position.Top} id="t" isConnectable className="mart-handle" style={{ ...dot, left: "50%", top: 0 }} />
      <Handle type="source" position={Position.Bottom} id="b" isConnectable className="mart-handle" style={{ ...dot, left: "50%", top: "100%" }} />
    </>
  );
}

export function StereotypeRow({ stereotypes, keyword }: { stereotypes: string[]; keyword?: string }) {
  if (!keyword && stereotypes.length === 0) return null;
  return (
    <div className="px-3 pt-[7px] text-center text-[10.5px] leading-tight text-slate-500">
      {keyword && <span className="block">{`«${keyword}»`}</span>}
      {stereotypes.map(s => <span key={s} className="mr-1">{`«${s}»`}</span>)}
    </div>
  );
}

export function AttributeRow({ a, showVisibility }: { a: Attribute; showVisibility?: boolean }) {
  return (
    <div className="relative flex items-center gap-2 px-3 py-[5px] text-[11.5px] border-b border-[#f3f5f8] last:border-b-0">
      {showVisibility && a.visibility && <span className="text-slate-400 font-mono">{a.visibility}</span>}
      <span className="flex-1 text-slate-800 truncate" title={a.name}>{a.name}</span>
      <span className="text-slate-400 font-mono text-[10.5px] truncate">
        {a.type.name}{a.multiplicity !== "1" ? ` [${a.multiplicity}]` : ""}
      </span>
    </div>
  );
}

// Attribute compartment with the collapsed/expand toggle (ERD_COLLAPSED_ROWS).
export function RowsCompartment({ rows, render }: { rows: number; render: (i: number) => React.ReactNode }) {
  const [expanded, setExpanded] = useState(false);
  if (rows === 0) return null;
  const visible = expanded ? rows : Math.min(rows, ERD_COLLAPSED_ROWS);
  const hidden = rows - ERD_COLLAPSED_ROWS;
  return (
    <div className="border-t border-[#eef1f5]">
      {Array.from({ length: visible }, (_, i) => render(i))}
      {hidden > 0 && (
        <button onClick={e => { e.stopPropagation(); setExpanded(v => !v); }}
          className="w-full flex items-center justify-center gap-1 px-3 py-[5px] text-[11px] font-medium text-[#1e88e5] hover:bg-[#f1f5fb] border-t border-[#f3f5f8]">
          {expanded ? <><ChevronDown size={12} /> Show less</> : <><ChevronRight size={12} /> +{hidden} more</>}
        </button>
      )}
    </div>
  );
}

export function ClassifierBox({ data, keyword, header }: { data: OkfNodeData; keyword?: string; header?: React.ReactNode }) {
  const profile = getProfile(data._profile);
  const st = stereotypeStyle(profile, data.stereotypes);
  const isDetailed = (data._viewMode ?? "compact") === "erd";
  const showVisibility = !profile.hide.includes("visibility");
  const boxStyle: React.CSSProperties = {
    fontFamily: NODE_FONT,
    ...(st.header ? { borderTopColor: st.header, borderTopWidth: 4 } : {}),
    ...(st.border === "thick" ? { borderColor: st.header ?? "#334155", borderWidth: 2.5 } : {}),
    ...(st.shape === "hexagon" ? { clipPath: "polygon(8% 0, 92% 0, 100% 50%, 92% 100%, 8% 100%, 0 50%)", borderRadius: 0 } : {}),
  };
  return (
    <div data-stereotyped={Object.keys(st).length > 0 || undefined}
      className="relative bg-white border-[1.5px] border-[#d8dee8] rounded-xl shadow-[0_2px_8px_rgba(15,23,42,0.05)] cursor-grab hover:border-[#c2cad8] select-none w-[230px]"
      style={boxStyle}>
      <NodePorts />
      <div className="relative z-[1]">
        {header}
        <StereotypeRow stereotypes={data.stereotypes} keyword={keyword} />
        <div className={`px-3 pb-[9px] pt-[3px] text-center text-[13.5px] font-semibold text-slate-900 ${data.abstract ? "italic" : ""}`}>
          {data.title}
        </div>
        {isDetailed && data.values && data.values.length > 0 && (
          <RowsCompartment rows={data.values.length}
            render={i => (
              <div key={data.values![i]} className="px-3 py-[5px] text-[11.5px] text-slate-800 border-b border-[#f3f5f8] last:border-b-0">
                {data.values![i]}
              </div>
            )} />
        )}
        {isDetailed && !data.values && (
          <RowsCompartment rows={data.attributes.length}
            render={i => <AttributeRow key={data.attributes[i].name + i} a={data.attributes[i]} showVisibility={showVisibility} />} />
        )}
        {!isDetailed && (
          <div className="px-3 pb-[10px] text-center text-[11px] text-slate-500">
            {data.values ? `${data.values.length} values` : `${data.attributes.length} attribute${data.attributes.length === 1 ? "" : "s"}`}
          </div>
        )}
      </div>
    </div>
  );
}
