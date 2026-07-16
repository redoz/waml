// Shared Tailwind class strings for inspector form fields, so the object,
// relationship, and diagram inspectors render identical inputs and section
// labels from one source instead of drifting copies. AttributeEditor keeps its
// own denser variant on purpose.
export const inputCls =
  "w-full text-[13px] px-[10px] py-2 border border-[#d8dee8] rounded-lg text-slate-900 focus:outline-none focus:border-[#1e88e5] focus:ring-2 focus:ring-[#e6f1fb]";

export const labelCls =
  "block text-[11px] font-semibold text-slate-500 uppercase tracking-[0.3px] mb-[6px]";
