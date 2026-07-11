<script lang="ts">
  // Mirrors packages/web/src/components/rail/RightRail.tsx.
  import { PanelRight, Share2 } from "lucide-svelte";
  import type { RightPanelId } from "./rightPanel.svelte";

  type Item = { id: RightPanelId; label: string };

  const ITEMS: Item[] = [
    { id: "inspect", label: "Inspect" },
    { id: "share", label: "Share" },
  ];

  const railBtn = (on: boolean) =>
    `w-full flex flex-col items-center gap-1 py-[9px] px-1 rounded-lg text-[11px] font-medium border ${
      on
        ? "bg-white text-slate-900 shadow-[0_1px_3px_rgba(15,23,42,0.08)] border-[#d8dee8]"
        : "border-transparent text-slate-500 hover:bg-[#f1f3f7] hover:text-slate-900"
    }`;

  let { active, onOpen }: {
    active: RightPanelId | null;
    onOpen: (id: RightPanelId) => void;
  } = $props();
</script>

<nav class="w-[60px] flex-shrink-0 border-l border-[#d8dee8] bg-[#fafafa] flex flex-col items-center gap-1 py-[14px] px-[4px] z-20">
  {#each ITEMS as it (it.id)}
    {@const on = it.id === active}
    <button
      onclick={() => onOpen(it.id)}
      aria-current={on ? "true" : undefined}
      class={railBtn(on)}
    >
      {#if it.id === "inspect"}
        <PanelRight size={20} />
      {:else}
        <Share2 size={20} />
      {/if}
      {it.label}
    </button>
  {/each}
</nav>
