<script lang="ts">
  let { keys }: { keys: string[] } = $props();
</script>

<!-- Visibility is driven by ancestor CSS (see app.css):
     shown on `.group:hover` or under `[data-show-shortcuts]`. -->
<span
  class="keyhint pointer-events-none inline-flex items-center gap-0.5 opacity-0 transition-opacity"
  aria-hidden="true"
>
  {#each keys as k}
    <kbd
      class="inline-flex min-w-[16px] items-center justify-center rounded border border-slate-300 bg-slate-50 px-1 text-[10px] font-semibold leading-[15px] text-slate-600 shadow-[0_1px_0_rgba(15,23,42,0.08)]"
      >{k}</kbd
    >
  {/each}
</span>
