<script lang="ts">
  // Dedicated host for the Inspector. Chosen over the generic ModelSheet because
  // the pin + translucent/hover-opaque behaviour is inspector-specific state that
  // would otherwise leak into the shared sheet (which still hosts the Share
  // panel). Provides its own resizable chrome, a pin toggle, and the
  // translucency logic.
  import type { Snippet } from "svelte";
  import { Pin, PinOff, X } from "lucide-svelte";

  const MIN_WIDTH = 320;

  let {
    open,
    pinned = false,
    title,
    onTogglePin,
    onClose,
    hideDelay = 250,
    width = $bindable(380),
    children,
  }: {
    open: boolean;
    pinned?: boolean;
    title: string;
    onTogglePin: () => void;
    onClose: () => void;
    /** Delay (ms) before re-dimming after the pointer leaves — avoids flicker. */
    hideDelay?: number;
    width?: number;
    children?: Snippet;
  } = $props();

  // "engaged" = pointer over the panel or focus is inside it. A pinned panel is
  // translucent only while idle (not engaged); hover/focus fades it back opaque.
  let engaged = $state(false);
  let hideTimer: ReturnType<typeof setTimeout> | undefined;

  const translucent = $derived(pinned && !engaged);

  function engage() {
    if (hideTimer) {
      clearTimeout(hideTimer);
      hideTimer = undefined;
    }
    engaged = true;
  }

  function disengage() {
    if (hideTimer) clearTimeout(hideTimer);
    // Short delay so brushing past the edge doesn't flicker the panel.
    hideTimer = setTimeout(() => {
      engaged = false;
      hideTimer = undefined;
    }, hideDelay);
  }

  // ── Resize (left-edge drag), mirrors ModelSheet/Inspector behaviour ──────────
  let resizing = false;
  let startX = 0;
  let startWidth = 0;

  function onResizeMouseDown(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    resizing = true;
    startX = e.clientX;
    startWidth = width;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
  }

  $effect(() => {
    function onMouseMove(e: MouseEvent) {
      if (!resizing) return;
      const delta = startX - e.clientX;
      width = Math.min(window.innerWidth * 0.6, Math.max(MIN_WIDTH, startWidth + delta));
    }
    function onMouseUp() {
      if (!resizing) return;
      resizing = false;
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    }
    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
    return () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };
  });
</script>

{#if open}
  <aside
    aria-label={title}
    style={`width: ${width}px`}
    class={`absolute top-0 bottom-0 right-0 max-w-full bg-white border-l border-[#d8dee8]
            shadow-[0_10px_15px_-3px_rgba(0,0,0,0.1)] z-[16] flex flex-col transition-opacity duration-200 ${translucent ? "opacity-40" : "opacity-100"}`}
    onpointerenter={engage}
    onpointerleave={disengage}
    onfocusin={engage}
    onfocusout={disengage}
  >
    <!-- Left-edge drag handle to resize the panel -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      onmousedown={onResizeMouseDown}
      title="Drag to resize"
      class="absolute left-0 top-0 bottom-0 w-[6px] -ml-[3px] cursor-col-resize z-[17] hover:bg-[#1e88e5]/20"
    ></div>

    <div class="flex items-center justify-between gap-2 p-4 border-b border-[#d8dee8]">
      <h2 class="m-0 text-[15px] font-semibold text-slate-900 flex-1 min-w-0 truncate">{title}</h2>
      <button
        onclick={onTogglePin}
        aria-label={pinned ? "Unpin inspector" : "Pin inspector"}
        aria-pressed={pinned}
        title={pinned ? "Unpin inspector" : "Pin inspector"}
        class={`w-[30px] h-[30px] flex items-center justify-center rounded-md transition-colors ${pinned ? "text-[#1e88e5] bg-[#e6f1fb]" : "text-slate-500 hover:bg-[#f1f3f7]"}`}
      >
        {#if pinned}
          <PinOff size={16} />
        {:else}
          <Pin size={16} />
        {/if}
      </button>
      <button
        onclick={onClose}
        aria-label="Close inspector"
        title="Close inspector"
        class="w-[30px] h-[30px] flex items-center justify-center rounded-md text-slate-500 hover:bg-[#f1f3f7]"
      >
        <X size={18} />
      </button>
    </div>
    <div class="p-4 overflow-y-auto flex-1 min-h-0">{@render children?.()}</div>
  </aside>
{/if}
