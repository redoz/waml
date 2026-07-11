import type { Diagram } from "@mc/okf";

interface DiagramTabsProps {
  diagrams: Diagram[];
  activeKey: string;
  onSelect: (key: string) => void;
  onCreate: () => void;
}

export function DiagramTabs({ diagrams, activeKey, onSelect, onCreate }: DiagramTabsProps) {
  return (
    <div data-dock className="absolute top-[12px] left-1/2 -translate-x-1/2 z-[6] flex items-center gap-1 rounded-xl bg-white/95 px-1.5 py-1 shadow-[0_1px_4px_rgba(15,23,42,0.12)]">
      {diagrams.map(d => (
        <button key={d.key} onClick={() => onSelect(d.key)}
          className={`px-3 py-[5px] rounded-lg text-[12px] font-[600] whitespace-nowrap ${d.key === activeKey ? "bg-[#e6f1fb] text-[#1e88e5]" : "text-slate-600 hover:bg-[#f1f3f7]"}`}>
          {d.title}
        </button>
      ))}
      <button onClick={onCreate} title="New diagram from the current nodes"
        className="px-2 py-[5px] rounded-lg text-[13px] text-slate-500 hover:bg-[#f1f3f7]">+</button>
    </div>
  );
}
