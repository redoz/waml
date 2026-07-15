<script lang="ts">
  // Presentational chrome for the central edit panel: a centered, enlarged card
  // over a dismissing scrim. It hosts an arbitrary body snippet and knows nothing
  // about what that body edits. Dismissal: close button, scrim click, or Esc.
  // Esc is two-stage — if a text field inside the panel is focused, the first Esc
  // blurs it (so a stray keypress can't discard an in-progress edit) and only a
  // second Esc closes the panel.
  import type { Snippet } from "svelte";
  import { fade } from "svelte/transition";

  let { title, onClose, fullHeight = false, showPreview = false, previewEl = $bindable(null), children }: {
    title: string;
    onClose: () => void;
    fullHeight?: boolean;
    /** Renders an empty cutout strip (transparent, no background) so the
     *  real canvas behind the scrim shows through it, panned/zoomed to
     *  frame the focal element — a "magnifying glass" over the live diagram
     *  rather than a separate rendered preview. */
    showPreview?: boolean;
    previewEl?: HTMLDivElement | null;
    children: Snippet;
  } = $props();

  let card = $state<HTMLDivElement | null>(null);

  // Punch a real hole in the scrim at the cutout's screen rect, instead of
  // dimming everything then leaving the cutout transparent (which only
  // reveals the scrim's own dark tint sitting behind it) — this way the
  // live canvas paints through at its normal, undimmed colors.
  let holeRect = $state<{ left: number; top: number; width: number; height: number } | null>(
    null,
  );
  $effect(() => {
    if (!showPreview || !previewEl) {
      holeRect = null;
      return;
    }
    const el = previewEl;
    const update = () => {
      const r = el.getBoundingClientRect();
      holeRect = { left: r.left, top: r.top, width: r.width, height: r.height };
    };
    update();
    window.addEventListener("resize", update);
    return () => window.removeEventListener("resize", update);
  });

  $effect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key !== "Escape") return;
      const active = document.activeElement as HTMLElement | null;
      const editing =
        !!active &&
        !!card?.contains(active) &&
        ["INPUT", "TEXTAREA", "SELECT"].includes(active.tagName);
      if (editing) {
        active!.blur();      // first Esc: protect the in-progress edit
        e.stopPropagation();
      } else {
        onClose();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  // Move focus into the dialog on open so screen readers announce it
  // (otherwise focus stays on the now-hidden trigger behind the scrim).
  $effect(() => {
    card?.focus();
  });
</script>

<!-- Scrim: dims whatever is rendered behind (neutral app or the live diagram) and
     dismisses on click. -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  data-testid="central-scrim"
  onclick={onClose}
  out:fade={{ duration: 200 }}
  class={`fixed inset-0 z-[60] flex items-center justify-center ${holeRect ? "" : "bg-slate-900/30"} ${fullHeight ? "p-4" : "p-8"}`}
  style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Inter, system-ui, sans-serif;"
>
  {#if holeRect}
    <!-- Hole-punch: box-shadow spread fills the whole viewport except this
         element's own box, which is positioned exactly over the cutout. -->
    <div
      class="fixed pointer-events-none"
      style={`left:${holeRect.left}px; top:${holeRect.top}px; width:${holeRect.width}px; height:${holeRect.height}px; box-shadow: 0 0 0 100vmax rgba(15,23,42,0.3);`}
    ></div>
  {/if}
  <!-- Card: stops propagation so clicks inside never reach the scrim. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    bind:this={card}
    role="dialog"
    aria-modal="true"
    aria-label={title}
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    out:fade={{ duration: 150 }}
    class={`relative w-full max-w-[620px] ${fullHeight ? "h-[95vh] max-h-[95vh]" : "max-h-[85vh]"} flex flex-col rounded-2xl border border-[#d8dee8] shadow-[0_16px_48px_rgba(15,23,42,0.22)] overflow-hidden`}
  >
    <div class="px-5 py-[15px] border-b border-[#d8dee8] flex items-center gap-2 flex-shrink-0 bg-white">
      <h2 class="text-[15px] font-[650] flex-1 text-slate-900 truncate">{title}</h2>
      <button
        onclick={onClose}
        aria-label="Close"
        title="Close"
        class="cursor-pointer text-slate-500 bg-transparent border-none text-[20px] leading-none hover:text-slate-900 transition-colors p-0"
      >
        ×
      </button>
    </div>
    {#if showPreview}
      <!-- Transparent cutout: no background, so the scrim-dimmed live
           canvas behind this dialog shows through, panned/zoomed by the
           host to frame the focal node/edge. -->
      <div bind:this={previewEl} data-testid="central-preview" class="h-[220px] flex-shrink-0"></div>
    {/if}
    <div class="px-5 py-5 overflow-y-auto flex-1 min-h-0 bg-white">
      {@render children()}
    </div>
  </div>
</div>
