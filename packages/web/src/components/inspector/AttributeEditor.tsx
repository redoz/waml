import { useState } from "react";
import { GripVertical } from "lucide-react";
import type { Attribute, Visibility } from "@mc/okf";
import { InfoTip } from "./InfoTip";

const VISIBILITIES: (Visibility | "")[] = ["", "+", "-", "#", "~"];

interface AttributeEditorProps {
  attributes: Attribute[];
  onChange: (attributes: Attribute[]) => void;
}

export function AttributeEditor({ attributes, onChange }: AttributeEditorProps) {
  const [dragIdx, setDragIdx] = useState<number | null>(null);
  const [overIdx, setOverIdx] = useState<number | null>(null);

  const update = (i: number, patch: Partial<Attribute>) =>
    onChange(attributes.map((a, idx) => idx === i ? { ...a, ...patch } : a));
  const remove = (i: number) => onChange(attributes.filter((_, idx) => idx !== i));
  const add = () => onChange([...attributes, { name: "", type: { name: "String" }, multiplicity: "1" }]);
  const move = (from: number, to: number) => {
    if (from === to || from < 0 || to < 0) return;
    const next = attributes.slice();
    const [moved] = next.splice(from, 1);
    next.splice(to, 0, moved);
    onChange(next);
  };

  const cols = "16px minmax(100px,1fr) minmax(90px,1fr) 62px 52px minmax(120px,1.3fr) 24px";
  const inputCls = "w-full text-[12.5px] px-[7px] py-[5px] border border-[#d8dee8] rounded-lg text-slate-900 focus:outline-none focus:border-[#1e88e5] focus:ring-2 focus:ring-[#e6f1fb]";

  return (
    <div className="border border-[#d8dee8] rounded-[10px] overflow-hidden">
      <div className="overflow-x-auto">
        <div className="min-w-[540px]">
          <div className="grid bg-[#f8fafc] px-[10px] py-[7px] text-[10.5px] font-semibold text-slate-500 uppercase tracking-[0.3px] border-b border-[#d8dee8] gap-[6px]" style={{ gridTemplateColumns: cols }}>
            <span />
            <span>Name</span>
            <span className="flex items-center gap-[3px]">Type <InfoTip text="A bare token (String, OrderId) or another classifier's title. Links to other docs survive import; editing the text keeps a plain token." /></span>
            <span className="flex items-center gap-[3px]">Mult <InfoTip text="UML multiplicity: 1, 0..1, *, 1..*, 2..5. Blank means 1." /></span>
            <span className="flex items-center gap-[3px]">Vis <InfoTip text="Visibility: + public, - private, # protected, ~ package. Optional; the uml-domain profile hides it on canvas." /></span>
            <span>Description</span>
            <span />
          </div>
          {attributes.map((a, i) => (
            <div key={i}
              onDragOver={e => { if (dragIdx === null) return; e.preventDefault(); if (overIdx !== i) setOverIdx(i); }}
              onDrop={e => { e.preventDefault(); if (dragIdx !== null) move(dragIdx, i); setDragIdx(null); setOverIdx(null); }}
              className={`grid px-[10px] py-[6px] border-b border-[#eef1f5] last:border-b-0 items-center gap-[6px] ${dragIdx === i ? "opacity-40" : ""} ${overIdx === i && dragIdx !== null && dragIdx !== i ? "bg-[#e6f1fb]" : ""}`}
              style={{ gridTemplateColumns: cols }}>
              <span draggable
                onDragStart={e => { setDragIdx(i); e.dataTransfer.effectAllowed = "move"; }}
                onDragEnd={() => { setDragIdx(null); setOverIdx(null); }}
                title="Drag to reorder"
                className="flex items-center justify-center text-slate-300 hover:text-slate-500 cursor-grab active:cursor-grabbing">
                <GripVertical size={13} />
              </span>
              <input type="text" value={a.name} placeholder="name" onChange={e => update(i, { name: e.target.value })} className={inputCls} />
              <input type="text" value={a.type.name} placeholder="String"
                onChange={e => update(i, { type: { name: e.target.value } })} className={inputCls} />
              <input type="text" value={a.multiplicity} placeholder="1"
                onChange={e => update(i, { multiplicity: e.target.value || "1" })} className={inputCls} />
              <select value={a.visibility ?? ""} aria-label="Visibility"
                onChange={e => update(i, { visibility: (e.target.value || undefined) as Visibility | undefined })}
                className="w-full text-[11.5px] px-[4px] py-[5px] border border-[#d8dee8] rounded-lg text-slate-900">
                {VISIBILITIES.map(v => <option key={v} value={v}>{v || "—"}</option>)}
              </select>
              <input type="text" value={a.description ?? ""} placeholder="description"
                onChange={e => update(i, { description: e.target.value || undefined })} className={inputCls} />
              <button onClick={() => remove(i)} title="Remove attribute"
                className="border-none bg-transparent text-slate-300 cursor-pointer text-[15px] p-0 hover:text-[#ef4444] flex items-center justify-center">×</button>
            </div>
          ))}
        </div>
      </div>
      <button onClick={add}
        className="w-full border-none bg-white px-2 py-[8px] text-[12.5px] font-semibold text-[#1e88e5] cursor-pointer hover:bg-[#f8fafc] transition-colors border-t border-[#eef1f5]">
        + Add attribute
      </button>
    </div>
  );
}
