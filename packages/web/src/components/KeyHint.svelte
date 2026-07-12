<script lang="ts">
  let { keys }: { keys: string[] } = $props();
</script>

<!-- Self-positions just outside the top-right corner of the nearest positioned
     ancestor (equal top/right overhang), out of the content flow. Opacity is
     driven by ancestor CSS (see app.css): shown under [data-show-shortcuts] or
     on `.group:hover` — the hover reveal delayed ~0.5s so it only appears on a
     deliberate, lingering hover. -->
<span
  class="keyhint pointer-events-none absolute -top-1.5 -right-1.5 z-10 flex items-center gap-0.5 opacity-0"
  aria-hidden="true"
>
  {#each keys as k}
    <kbd
      class="inline-flex min-w-[18px] items-center justify-center rounded border border-slate-300 bg-slate-50 px-1.5 text-[11px] font-semibold leading-[17px] text-slate-600 shadow-[0_1px_0_rgba(15,23,42,0.08)]"
      >{k}</kbd
    >
  {/each}
</span>
