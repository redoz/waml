<script lang="ts">
  // Mirrors packages/web/src/components/canvas/DiagramTabs.tsx.
  import type { Diagram } from "@uaml/okf";

  let {
    diagrams,
    activeKey,
    onSelect,
    onCreate,
  }: {
    diagrams: Diagram[];
    activeKey: string;
    onSelect: (key: string) => void;
    onCreate: () => void;
  } = $props();
</script>

<div
  data-dock
  class="absolute top-[12px] left-1/2 -translate-x-1/2 z-[6] flex items-center gap-1 rounded-xl bg-white/95 px-1.5 py-1 shadow-[0_1px_4px_rgba(15,23,42,0.12)]"
>
  {#each diagrams as d (d.key)}
    <button
      onclick={() => onSelect(d.key)}
      class="px-3 py-[5px] rounded-lg text-[12px] font-[600] whitespace-nowrap {d.key === activeKey
        ? 'bg-[#e6f1fb] text-[#1e88e5]'
        : 'text-slate-600 hover:bg-[#f1f3f7]'}"
    >
      {d.title}
    </button>
  {/each}
  <button
    onclick={onCreate}
    title="New diagram from the current nodes"
    class="px-2 py-[5px] rounded-lg text-[13px] text-slate-500 hover:bg-[#f1f3f7]"
  >
    +
  </button>
</div>
