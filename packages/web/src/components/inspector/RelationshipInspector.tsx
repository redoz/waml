import type { ModelEdge, ModelNode, RelationshipKind, RelEnd } from "@mc/okf";
import { RELATIONSHIP_KINDS, ENDED_KINDS } from "@mc/okf";
import { InfoTip } from "./InfoTip";

interface RelationshipInspectorProps {
  edge: ModelEdge;
  fromNode: ModelNode | undefined;
  toNode: ModelNode | undefined;
  onUpdate: (patch: Partial<ModelEdge>) => void;
}

const KIND_HELP: Record<RelationshipKind, string> = {
  associates: "Plain association — solid line, arrowhead on navigable end(s).",
  aggregates: "Shared aggregation — hollow diamond on this (whole) end.",
  composes: "Composition — filled diamond on this (whole) end; parts live and die with the whole.",
  specializes: "Generalization — hollow triangle at the parent (near→far reads child→parent).",
  implements: "Realization — dashed line, hollow triangle at the interface.",
  depends: "Dependency — dashed open arrow at the target.",
  annotates: "Note anchor — uml.Note only; never selectable here.",
};

// `annotates` is a uml.Note-only verb (anchors live on the note node, not on edges) — hide it from the edge verb select.
const EDGE_KINDS = RELATIONSHIP_KINDS.filter(k => k !== "annotates");

const inputCls = "w-full text-[13px] px-[10px] py-[8px] border border-[#d8dee8] rounded-lg text-slate-900 focus:outline-none focus:border-[#1e88e5] focus:ring-2 focus:ring-[#e6f1fb]";

function EndEditor({ title, end, onChange }: { title: string; end: RelEnd; onChange: (end: RelEnd) => void }) {
  return (
    <div className="flex gap-[6px]">
      <label className="flex-1 text-[11px] text-slate-500">
        {title} multiplicity
        <input aria-label={`${title} multiplicity`} type="text" value={end.multiplicity ?? ""} placeholder="1, 0..1, *"
          onChange={e => onChange({ ...end, multiplicity: e.target.value || undefined })} className={inputCls} />
      </label>
      <label className="flex-1 text-[11px] text-slate-500">
        {title} role
        <input aria-label={`${title} role`} type="text" value={end.role ?? ""} placeholder="role"
          onChange={e => onChange({ ...end, role: e.target.value || undefined })} className={inputCls} />
      </label>
    </div>
  );
}

export function RelationshipInspector({ edge, fromNode, toNode, onUpdate }: RelationshipInspectorProps) {
  const fromTitle = fromNode?.title ?? "Source";
  const toTitle = toNode?.title ?? "Target";
  const hasEnds = ENDED_KINDS.has(edge.kind);
  return (
    <div className="flex flex-col gap-[15px]">
      <div className="text-[13px] text-slate-500">
        <strong className="text-slate-900">{fromTitle}</strong>{" → "}<strong className="text-slate-900">{toTitle}</strong>
      </div>
      <div>
        <label htmlFor="rel-kind" className="flex items-center gap-[5px] text-[11px] font-semibold text-slate-500 uppercase tracking-[0.3px] mb-[6px]">
          Kind <InfoTip text={KIND_HELP[edge.kind]} />
        </label>
        <select id="rel-kind" aria-label="Kind" value={edge.kind}
          onChange={e => onUpdate({ kind: e.target.value as RelationshipKind })} className={inputCls}>
          {EDGE_KINDS.map(k => <option key={k} value={k}>{k}</option>)}
        </select>
      </div>
      {hasEnds && (
        <div className="flex flex-col gap-[10px]">
          <EndEditor title={fromTitle} end={edge.fromEnd} onChange={fromEnd => onUpdate({ fromEnd })} />
          <EndEditor title={toTitle} end={edge.toEnd} onChange={toEnd => onUpdate({ toEnd })} />
        </div>
      )}
      {edge.kind === "associates" && (
        <label className="flex items-start gap-[9px] cursor-pointer">
          <input type="checkbox" checked={edge.bidirectional}
            onChange={e => onUpdate({
              bidirectional: e.target.checked,
              fromEnd: { ...edge.fromEnd, navigable: e.target.checked ? true : undefined },
              toEnd: { ...edge.toEnd, navigable: true },
            })}
            className="w-4 h-4 mt-[1px] accent-[#1e88e5] cursor-pointer" />
          <span className="text-[12.5px]">
            <strong className="text-[13px]">Bidirectional</strong>
            <span className="text-slate-500 mt-[2px] leading-[1.4] block">Both ends navigable — arrowheads on both ends.</span>
          </span>
        </label>
      )}
    </div>
  );
}
