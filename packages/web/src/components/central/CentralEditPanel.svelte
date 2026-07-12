<script lang="ts">
  // Presentational chrome for the central edit panel: a centered, enlarged card
  // over a dismissing scrim. It hosts an arbitrary body snippet and knows nothing
  // about what that body edits. Dismissal: close button, scrim click, or Esc.
  // Esc is two-stage — if a text field inside the panel is focused, the first Esc
  // blurs it (so a stray keypress can't discard an in-progress edit) and only a
  // second Esc closes the panel.
  import type { Snippet } from "svelte";

  let { title, onClose, children }: {
    title: string;
    onClose: () => void;
    children: Snippet;
  } = $props();

  let card = $state<HTMLDivElement | null>(null);

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
</script>

<!-- Scrim: dims whatever is rendered behind (neutral app or the live diagram) and
     dismisses on click. -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  data-testid="central-scrim"
  onclick={onClose}
  class="fixed inset-0 z-[60] bg-slate-900/30 flex items-center justify-center p-8"
  style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Inter, system-ui, sans-serif;"
>
  <!-- Card: stops propagation so clicks inside never reach the scrim. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    bind:this={card}
    role="dialog"
    aria-modal="true"
    aria-label={title}
    onclick={(e) => e.stopPropagation()}
    class="w-full max-w-[560px] max-h-[85vh] flex flex-col rounded-2xl border border-[#d8dee8] bg-white shadow-[0_16px_48px_rgba(15,23,42,0.22)]"
  >
    <div class="px-5 py-[15px] border-b border-[#d8dee8] flex items-center gap-2 flex-shrink-0">
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
    <div class="px-5 py-5 overflow-y-auto flex-1 min-h-0">
      {@render children()}
    </div>
  </div>
</div>
