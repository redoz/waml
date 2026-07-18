<script lang="ts">
  let { stereotypes, keyword }: { stereotypes: string[]; keyword?: string } = $props();
</script>

{#if keyword || stereotypes.length > 0}
  <div class="node-stereo">
    {#if keyword}<span>{`«${keyword}»`}</span>{/if}
    {#each stereotypes as s (s)}<span>{`«${s}»`}</span>{/each}
  </div>
{/if}
