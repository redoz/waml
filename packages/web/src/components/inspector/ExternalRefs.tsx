import type { Diagram, ModelEdge, ModelNode } from "@mc/okf";

interface ExternalRefsProps {
  nodeKey: string;
  nodes: ModelNode[];
  edges: ModelEdge[];
  members: string[];
  diagrams: Diagram[];
  onNavigate: (diagramKey: string, nodeKey: string) => void;
}

// The spec's "isolate a domain, still see other sources" behavior: relationships
// whose other end is off-diagram surface here as navigable chips.
export function ExternalRefs({ nodeKey, nodes, edges, members, diagrams, onNavigate }: ExternalRefsProps) {
  const memberSet = new Set(members);
  const byKey = new Map(nodes.map(n => [n.key, n]));
  const refs: { key: string; label: string; other: string }[] = [];
  for (const e of edges) {
    if (e.from === nodeKey && !memberSet.has(e.to) && byKey.has(e.to)) {
      refs.push({ key: e.id, label: `${e.kind} → ${byKey.get(e.to)!.title}`, other: e.to });
    } else if (e.to === nodeKey && !memberSet.has(e.from) && byKey.has(e.from)) {
      refs.push({ key: e.id, label: `${byKey.get(e.from)!.title} → ${e.kind}`, other: e.from });
    }
  }
  if (refs.length === 0) return null;
  const diagramFor = (k: string) => diagrams.find(d => d.members.includes(k))?.key;
  return (
    <div>
      <label className="block text-[11px] font-semibold text-slate-500 uppercase tracking-[0.3px] mb-[6px]">
        External references
      </label>
      <div className="flex flex-wrap gap-[6px]">
        {refs.map(r => {
          const target = diagramFor(r.other);
          return (
            <button key={r.key} disabled={!target}
              onClick={() => target && onNavigate(target, r.other)}
              title={target ? "Open the diagram containing this node" : "Not on any diagram"}
              className="rounded-full border border-[#d8dee8] bg-white px-[10px] py-[4px] text-[11.5px] text-slate-600 hover:border-[#1e88e5] hover:text-[#1e88e5] disabled:opacity-50">
              {r.label}
            </button>
          );
        })}
      </div>
    </div>
  );
}
