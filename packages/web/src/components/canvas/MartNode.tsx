import { memo, useState } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";
import { ChevronDown, ChevronRight } from "lucide-react";
import type { ModelNode, Attribute } from "@mc/okf";
import type { ViewMode } from "../../state/viewMode";
import { DataMartIcon } from "../../lib/icons";
import { ERD_COLLAPSED_ROWS } from "./layoutSize";

export type MartNodeData = ModelNode & { _viewMode?: ViewMode };

// Node-level connectable ports (the only way to draw a new relationship).
function NodePorts() {
  const common = {
    width: 13, height: 13, borderRadius: "50%",
    background: "#fff", border: "2px solid #1e88e5",
    top: 24, opacity: 0, transition: "opacity 0.12s",
  } as const;
  return (
    <>
      <Handle type="source" position={Position.Left} id="left" style={{ ...common, left: -7 }} className="mart-handle" />
      <Handle type="source" position={Position.Right} id="right" style={{ ...common, right: -7 }} className="mart-handle" />
    </>
  );
}

function MartHeader({ node, color }: { node: MartNodeData; color: string }) {
  return (
    <div className="flex items-center gap-2 px-3 pt-[11px] pb-2">
      <span className="w-1 self-stretch min-h-[18px] rounded-sm flex-shrink-0" style={{ background: color }} />
      <DataMartIcon size={15} className="text-slate-400 flex-shrink-0" />
      <span className="text-[13.5px] font-semibold flex-1 leading-tight pr-3 text-slate-900 line-clamp-2">
        {node.title}
      </span>
    </div>
  );
}

function FieldRow({ a }: { a: Attribute }) {
  return (
    <div className="relative flex items-center gap-2 px-3 py-[5px] text-[11.5px] border-b border-[#f3f5f8] last:border-b-0">
      <span className="flex-1 text-slate-800 truncate" title={a.name}>{a.name}</span>
      <span className="text-slate-400 font-mono text-[10.5px] truncate">{a.type.name}{a.multiplicity !== "1" ? ` [${a.multiplicity}]` : ""}</span>
    </div>
  );
}

// ERD body shows at most ERD_COLLAPSED_ROWS attributes by default so dense nodes
// stay readable; the rest hide behind a "+N more" toggle.
function ErdBody({ node }: { node: MartNodeData }) {
  const [expanded, setExpanded] = useState(false);
  const ordered = node.attributes;
  if (ordered.length === 0) {
    return <div className="px-3 pb-[10px] text-[11px] text-slate-400">no attributes</div>;
  }

  const visible = expanded ? ordered : ordered.slice(0, ERD_COLLAPSED_ROWS);
  const hidden = ordered.length - ERD_COLLAPSED_ROWS;

  return (
    <div className="border-t border-[#eef1f5]">
      {visible.map(a => <FieldRow key={a.name} a={a} />)}
      {hidden > 0 && (
        <button
          onClick={e => { e.stopPropagation(); setExpanded(v => !v); }}
          className="w-full flex items-center justify-center gap-1 px-3 py-[5px] text-[11px] font-medium text-[#1e88e5] hover:bg-[#f1f5fb] border-t border-[#f3f5f8]"
        >
          {expanded
            ? <><ChevronDown size={12} /> Show less</>
            : <><ChevronRight size={12} /> +{hidden} more field{hidden > 1 ? "s" : ""}</>}
        </button>
      )}
    </div>
  );
}

function MartNodeInner({ data }: NodeProps) {
  const node = data as unknown as MartNodeData;
  const viewMode = node._viewMode ?? "compact";
  const color = "#94a3b8";
  const isErd = viewMode === "erd";
  const fieldCount = node.attributes?.length ?? 0;
  const fieldText = fieldCount > 0 ? `${fieldCount} field${fieldCount > 1 ? "s" : ""}` : "no fields";

  return (
    <div
      className={`relative bg-white border-[1.5px] border-[#d8dee8] rounded-xl shadow-[0_2px_8px_rgba(15,23,42,0.05)] cursor-grab hover:border-[#c2cad8] select-none ${isErd ? "w-[250px]" : "w-[200px]"}`}
      style={{ fontFamily: "-apple-system, BlinkMacSystemFont, 'Segoe UI', Inter, system-ui, sans-serif" }}
    >
      <MartHeader node={node} color={color} />

      {/* Meta row: type chip + (compact) field count */}
      <div className="flex items-center gap-2 px-3 pb-[10px]">
        <span
          className="text-[10.5px] font-[650] uppercase tracking-[0.3px] px-[7px] py-[2px] rounded-full text-white"
          style={{ background: color }}
        >
          {node.type}
        </span>
        {!isErd && <span className="text-[11px] text-slate-500">{fieldText}</span>}
      </div>

      {isErd && <ErdBody node={node} />}

      <NodePorts />
    </div>
  );
}

export const MartNode = memo(MartNodeInner);
