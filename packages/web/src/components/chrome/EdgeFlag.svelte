<script lang="ts">
  // A vertical "flag" tab pinned to the right edge of the canvas at mid-height.
  // Reused for both the Inspect toggle (button) and the Feedback link (anchor).
  // Multiple flags stack cleanly via the `offset` prop (pixels from vertical
  // centre); the parent passes a distinct offset per flag.
  import type { Snippet } from "svelte";

  let {
    label,
    href,
    onClick,
    offset = 0,
    rightOffset = 0,
    active = false,
    icon,
  }: {
    label: string;
    href?: string;
    onClick?: () => void;
    /** Vertical offset in px from the mid-height centre, for stacking. */
    offset?: number;
    /** Horizontal offset in px from the right edge — lets the parent slide the
     *  flags clear of an open panel so they don't obscure it. */
    rightOffset?: number;
    /** Pressed/active state (e.g. the Inspect flag while the inspector is open). */
    active?: boolean;
    icon?: Snippet;
  } = $props();

  // Anchored to the right edge (offset inward by rightOffset), translated to
  // mid-height plus the stacking offset.
  const style = $derived(`top: 50%; right: ${rightOffset}px; transform: translateY(calc(-50% + ${offset}px));`);

  // Rests behind the floating Inspector (z-[16]); hover/focus lifts it above.
  const base =
    "group absolute z-[12] hover:z-30 focus-visible:z-30 flex w-[34px] flex-col items-center justify-center gap-[6px] rounded-l-xl border border-r-0 border-[#d8dee8] bg-white py-3 text-[11px] font-[600] text-slate-500 shadow-[-3px_0_12px_rgba(15,23,42,0.08)] cursor-pointer transition-[right,background-color,color] duration-200 hover:bg-[#f1f3f7] hover:text-[#1e88e5] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#1e88e5]/60";
  const cls = $derived(base + (active ? " bg-[#e6f1fb] text-[#1e88e5]" : ""));

  // Vertical label reading bottom-to-top, sitting beneath the icon.
  const labelStyle = "writing-mode: vertical-rl; transform: rotate(180deg);";
</script>

{#if href}
  <a {href} target="_blank" rel="noreferrer" title={label} aria-label={label} class={cls} {style}>
    {@render icon?.()}
    <span class="tracking-[0.02em]" style={labelStyle}>{label}</span>
  </a>
{:else}
  <button type="button" onclick={onClick} title={label} aria-label={label} aria-pressed={active} class={cls} {style}>
    {@render icon?.()}
    <span class="tracking-[0.02em]" style={labelStyle}>{label}</span>
  </button>
{/if}
