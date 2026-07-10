import type { ReactNode } from "react";
import { PanelRight, Share2 } from "lucide-react";
import type { RightPanelId } from "./useRightPanel";

type Item = { id: RightPanelId; label: string; icon: ReactNode };

const ITEMS: Item[] = [
  { id: "inspect", label: "Inspect", icon: <PanelRight size={20} /> },
  { id: "share", label: "Share", icon: <Share2 size={20} /> },
];

const railBtn = (on: boolean) =>
  `w-full flex flex-col items-center gap-1 py-[9px] px-1 rounded-lg text-[11px] font-medium border ${
    on ? "bg-white text-slate-900 shadow-[0_1px_3px_rgba(15,23,42,0.08)] border-[#d8dee8]"
       : "border-transparent text-slate-500 hover:bg-[#f1f3f7] hover:text-slate-900"}`;

export function RightRail({ active, onOpen }: {
  active: RightPanelId | null; onOpen: (id: RightPanelId) => void;
}) {
  return (
    <nav className="w-[60px] flex-shrink-0 border-l border-[#d8dee8] bg-[#fafafa] flex flex-col items-center gap-1 py-[14px] px-[4px] z-20">
      {ITEMS.map(it => {
        const on = it.id === active;
        return (
          <button key={it.id} onClick={() => onOpen(it.id)} aria-current={on ? "true" : undefined} className={railBtn(on)}>
            {it.icon}{it.label}
          </button>
        );
      })}
    </nav>
  );
}
