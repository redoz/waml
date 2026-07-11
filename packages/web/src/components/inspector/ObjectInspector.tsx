import type { ModelNode, Attribute } from "@mc/okf";
import { AttributeEditor } from "./AttributeEditor";
import { InfoTip } from "./InfoTip";
import { getProfile } from "../../profiles";

interface ObjectInspectorProps {
  node: ModelNode;
  onUpdate: (patch: Partial<ModelNode>) => void;
  profileName?: string;
}

const inputCls = "w-full text-[13px] px-[10px] py-2 border border-[#d8dee8] rounded-lg text-slate-900 focus:outline-none focus:border-[#1e88e5] focus:ring-2 focus:ring-[#e6f1fb]";
const labelCls = "block text-[11px] font-semibold text-slate-500 uppercase tracking-[0.3px] mb-[6px]";

export function ObjectInspector({ node, onUpdate, profileName }: ObjectInspectorProps) {
  const palette = getProfile(profileName).palette;
  const isEnum = node.type === "uml.Enum";
  return (
    <div className="flex flex-col gap-[15px]">
      <div>
        <label className={labelCls}>Title</label>
        <input type="text" value={node.title} onChange={e => onUpdate({ title: e.target.value })} className={inputCls} />
      </div>
      <div>
        <label className={labelCls}>Description</label>
        <textarea value={node.description ?? ""} rows={3}
          onChange={e => onUpdate({ description: e.target.value || undefined })}
          className={`${inputCls} resize-y min-h-[60px]`} />
      </div>
      <div className="flex gap-[10px]">
        <div className="flex-1">
          <label className={`${labelCls} flex items-center gap-[5px]`}>
            Type <InfoTip text="family.Metaclass dispatch key (e.g. uml.Class). Unknown values render as a generic box — never an error." />
          </label>
          <input type="text" list="okf-metaclasses" value={node.type}
            onChange={e => onUpdate({ type: e.target.value })} className={inputCls} />
          <datalist id="okf-metaclasses">{palette.metaclasses.map(t => <option key={t} value={t} />)}</datalist>
        </div>
        <label className="flex items-end gap-[7px] pb-[9px] cursor-pointer text-[12.5px] text-slate-700">
          <input type="checkbox" checked={node.abstract ?? false}
            onChange={e => onUpdate({ abstract: e.target.checked || undefined })}
            className="w-4 h-4 accent-[#1e88e5] cursor-pointer" />
          abstract
        </label>
      </div>
      <div>
        <label className={`${labelCls} flex items-center gap-[5px]`}>
          Stereotypes <InfoTip text="Comma-separated, open set: entity, valueObject, aggregateRoot, service, domainEvent — invent any. Rendered as «guillemets»." />
        </label>
        <input type="text" list="okf-stereotypes" value={node.stereotypes.join(", ")}
          onChange={e => onUpdate({ stereotypes: e.target.value.split(",").map(s => s.trim()).filter(Boolean) })}
          placeholder="aggregateRoot, entity" className={inputCls} />
        <datalist id="okf-stereotypes">{palette.stereotypes.map(s => <option key={s} value={s} />)}</datalist>
      </div>
      {isEnum ? (
        <div>
          <label className={labelCls}>Values (one per line)</label>
          <textarea value={(node.values ?? []).join("\n")} rows={5}
            onChange={e => onUpdate({ values: e.target.value.split("\n").map(v => v.trim()).filter(Boolean) })}
            className={`${inputCls} font-mono resize-y`} />
        </div>
      ) : (
        <div>
          <label className={labelCls}>Attributes</label>
          <AttributeEditor attributes={node.attributes} onChange={(attributes: Attribute[]) => onUpdate({ attributes })} />
        </div>
      )}
    </div>
  );
}
