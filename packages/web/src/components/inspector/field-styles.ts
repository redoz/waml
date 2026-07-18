// Shared Tailwind class strings for inspector form fields, so the object,
// relationship, and diagram inspectors render identical inputs and section
// labels from one source instead of drifting copies. AttributeEditor keeps its
// own denser variant on purpose.
export const inputCls =
  "w-full text-[13px] px-[10px] py-2 border border-[color:var(--hair)] rounded-[var(--round-chip)] text-[color:var(--ink)] focus:outline-none focus:border-[color:rgb(var(--accent))] focus:ring-2 focus:ring-[color:rgba(var(--accent),.20)]";

export const labelCls =
  "block text-[11px] font-semibold text-[color:rgb(var(--ink-faint))] uppercase tracking-[0.3px] mb-[6px]";
