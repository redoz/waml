<script lang="ts">
  import type { SequenceDoc } from "@waml/okf";
  import { layoutSequence, type SeqRow } from "../../../canvas/sequenceLayout";

  let { doc }: { doc: SequenceDoc } = $props();

  const layout = $derived(layoutSequence(doc));

  type MessageRow = Extract<SeqRow, { kind: "message" }>;
  // Solid+filled = calls (sync); solid+open = sends (async);
  // dashed+open = replies; dashed→new lifeline = creates; →✕ = destroys.
  const dashed = (r: MessageRow) => r.edge.verb === "replies" || r.edge.verb === "creates";
  const filled = (r: MessageRow) => r.edge.verb === "calls";
  const destroyed = (r: MessageRow) => r.edge.verb === "destroys";
  // Picks the arrow marker for a row: filled > destroys-✕ > default open.
  const markerFor = (r: MessageRow) => (filled(r) ? "url(#seq-arrow-filled)" : destroyed(r) ? "url(#seq-arrow-x)" : "url(#seq-arrow-open)");
</script>

<!-- A self-rendering behavior view: read-only, plain SVG. The lifelines and
     document-ordered messages ARE the layout — no solver is involved. -->
<div class="h-full w-full overflow-auto" style="background: var(--canvas-bg);" data-sequence-view>
  <svg width={layout.width} height={layout.height} class="block">
    <defs>
      <marker id="seq-arrow-filled" markerWidth="12" markerHeight="12" refX="9" refY="5" orient="auto">
        <path d="M0,0 L10,5 L0,10 z" fill="rgb(var(--ink-faint))" />
      </marker>
      <marker id="seq-arrow-open" markerWidth="12" markerHeight="12" refX="9" refY="5" orient="auto">
        <path d="M0,0 L10,5 L0,10" fill="none" stroke="rgb(var(--ink-faint))" stroke-width="1.5" />
      </marker>
      <marker id="seq-arrow-x" markerWidth="14" markerHeight="14" refX="10" refY="6" orient="auto">
        <path d="M4,2 L10,10 M10,2 L4,10" fill="none" stroke="rgb(var(--ink-faint))" stroke-width="1.5" />
      </marker>
    </defs>

    {#each layout.lifelines as lane (lane.key)}
      <line x1={lane.x} y1={44} x2={lane.x} y2={layout.height - 10} stroke="rgba(var(--ink-faint), .5)" stroke-width="1.5" stroke-dasharray="4 3" />
      <rect x={lane.x - 60} y={10} width="120" height="30" rx="var(--round-chip)" fill="var(--panel-fill)" stroke="rgba(var(--accent), .3)" stroke-width="1.5" />
      <text x={lane.x} y={30} text-anchor="middle" font-family="var(--font-ui)" font-size="12" font-weight="600" fill="var(--ink)">{lane.handle}</text>
    {/each}

    {#each layout.rows as row (row.y + row.kind)}
      {#if row.kind === "message"}
        {#if row.self}
          <path d={`M${row.fromX},${row.y} h30 v18 h-30`} fill="none" stroke="rgb(var(--ink-faint))" stroke-width="1.5" stroke-dasharray={dashed(row) ? "5 3" : undefined} marker-end={markerFor(row)} />
        {:else}
          <line x1={row.fromX} y1={row.y} x2={row.toX} y2={row.y} stroke="rgb(var(--ink-faint))" stroke-width="1.5" stroke-dasharray={dashed(row) ? "5 3" : undefined} marker-end={markerFor(row)} />
        {/if}
        {#if row.edge.signature}
          <text x={(row.fromX + row.toX) / 2} y={row.y - 6} text-anchor="middle" font-family="var(--font-mono)" font-size="11" fill="var(--ink-dim)">{row.edge.signature}</text>
        {/if}
      {:else if row.kind === "fragmentStart"}
        <rect x={row.x0} y={row.y} width={row.x1 - row.x0} height={layout.height - row.y - 20} fill="none" stroke="rgba(var(--ink-faint), .7)" stroke-width="1.2" />
        <path d={`M${row.x0},${row.y} h34 v14 l-8,8 h-26 z`} fill="var(--canvas-bg)" stroke="rgba(var(--ink-faint), .7)" stroke-width="1.2" />
        <text x={row.x0 + 6} y={row.y + 15} font-family="var(--font-mono)" font-size="10.5" font-weight="700" fill="var(--ink-dim)">{row.label}</text>
      {:else if row.kind === "operandDivider"}
        <line x1={row.x0} y1={row.y} x2={row.x1} y2={row.y} stroke="rgba(var(--ink-faint), .7)" stroke-width="1" stroke-dasharray="3 3" />
        {#if row.label}<text x={row.x0 + 6} y={row.y + 13} font-family="var(--font-mono)" font-size="10" font-style="italic" fill="rgb(var(--ink-faint))">[{row.label}]</text>{/if}
      {/if}
    {/each}
  </svg>
</div>
