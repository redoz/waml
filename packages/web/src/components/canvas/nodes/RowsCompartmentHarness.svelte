<script lang="ts">
  import RowsCompartment from "./RowsCompartment.svelte";
  let { rows, max }: { rows: number; max?: number } = $props();
</script>

<RowsCompartment {rows} {max}>
  {#snippet render(i: number)}
    <div data-row={i}>row {i}</div>
  {/snippet}
</RowsCompartment>
