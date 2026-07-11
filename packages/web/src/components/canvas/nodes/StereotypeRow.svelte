<script lang="ts">
  let { stereotypes, keyword }: { stereotypes: string[]; keyword?: string } = $props();
</script>

{#if keyword || stereotypes.length > 0}
  <div class="px-3 pt-[7px] text-center text-[10.5px] leading-tight text-slate-500">
    {#if keyword}<span class="block">{`«${keyword}»`}</span>{/if}
    {#each stereotypes as s (s)}<span class="mr-1">{`«${s}»`}</span>{/each}
  </div>
{/if}
