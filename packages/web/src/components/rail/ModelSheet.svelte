<script lang="ts">
  // Mirrors packages/web/src/components/rail/ModelSheet.tsx.
  import type { Snippet } from "svelte";
  import { X } from "lucide-svelte";
  import type { RightPanelId } from "./rightPanel.svelte";

  const MIN_WIDTH = 320;
  const DEFAULT_WIDTH = 380;

  let {
    active,
    modal = true,
    title,
    onClose,
    children,
  }: {
    active: RightPanelId | null;
    modal?: boolean;
    title: string;
    onClose: () => void;
    children?: Snippet;
  } = $props();

  // Width is user-resizable via the left-edge drag handle (restores the old
  // Inspector behaviour). Runs unconditionally, same as the React hooks.
  let width = $state(DEFAULT_WIDTH);
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
      // Sheet is anchored to the right, so dragging its left edge leftwards widens it.
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

{#if active}
  <!-- overlay covers the canvas only for modal panels — inspect keeps canvas interactive -->
  {#if modal}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="absolute inset-0 bg-black/50 z-[15]" onclick={onClose}></div>
  {/if}
  <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
  <aside
    role="dialog"
    aria-label={title}
    style={`width: ${width}px`}
    class="absolute top-0 bottom-0 right-[60px] max-w-[calc(100%-60px)] bg-white border-l border-[#d8dee8]
                   shadow-[0_10px_15px_-3px_rgba(0,0,0,0.1)] z-[16] flex flex-col"
  >
    <!-- Left-edge drag handle to resize the sheet -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      onmousedown={onResizeMouseDown}
      title="Drag to resize"
      class="absolute left-0 top-0 bottom-0 w-[6px] -ml-[3px] cursor-col-resize z-[17] hover:bg-[#1e88e5]/20"
    ></div>
    <div class="flex items-center justify-between gap-2 p-4 border-b border-[#d8dee8]">
      <h2 class="m-0 text-[17px] font-semibold text-slate-900">{title}</h2>
      <button
        onclick={onClose}
        aria-label="Close"
        class="w-[30px] h-[30px] flex items-center justify-center rounded-md text-slate-500 hover:bg-[#f1f3f7]"
      >
        <X size={18} />
      </button>
    </div>
    <div class="p-4 overflow-y-auto">{@render children?.()}</div>
  </aside>
{/if}
