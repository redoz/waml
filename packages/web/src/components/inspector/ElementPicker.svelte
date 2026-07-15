<script module lang="ts">
  // The inspector's element switcher, shared by the floating InspectorPanel
  // header and the CentralEditPanel (edit dialog) header so both offer the same
  // diagram/object/association picker. Entries span three kinds; the kind drives
  // the row icon and how the caller routes a selection back to the canvas.
  import { Box, Spline, Frame } from "lucide-svelte";

  export type Kind = "diagram" | "node" | "edge";
  export const KIND_ICON = { diagram: Frame, node: Box, edge: Spline };
</script>

<script lang="ts">
  import { ChevronDown, Check } from "lucide-svelte";

  let {
    options,
    selectedKey,
    onSelect,
    placeholder = "Select an element…",
  }: {
    options: { key: string; label: string; kind: Kind }[];
    selectedKey: string | null;
    onSelect: (key: string, kind: Kind) => void;
    placeholder?: string;
  } = $props();

  // Custom listbox (not a native <select>) so the option list carries the same
  // styling as the diagram/object switcher (Navigator). The menu is
  // position:fixed so it escapes the panel's overflow-hidden clip; coordinates
  // are measured off the trigger when it opens.
  let open = $state(false);
  let highlighted = $state(-1);
  let triggerEl: HTMLButtonElement | undefined;
  let menuStyle = $state("");

  const selectedLabel = $derived(options.find((o) => o.key === selectedKey)?.label);

  function openMenu() {
    if (triggerEl) {
      const r = triggerEl.getBoundingClientRect();
      menuStyle = `left: ${r.left}px; top: ${r.bottom + 6}px; min-width: ${r.width}px;`;
    }
    highlighted = Math.max(0, options.findIndex((o) => o.key === selectedKey));
    open = true;
  }

  function closeMenu() {
    open = false;
  }

  function toggleMenu() {
    if (open) closeMenu();
    else openMenu();
  }

  function choose(key: string, kind: Kind) {
    onSelect(key, kind);
    closeMenu();
    triggerEl?.focus();
  }

  function onTriggerKeydown(e: KeyboardEvent) {
    if (!open) {
      if (e.key === "ArrowDown" || e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        openMenu();
      }
      return;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      closeMenu();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      highlighted = Math.min(options.length - 1, highlighted + 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      highlighted = Math.max(0, highlighted - 1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (highlighted >= 0 && options[highlighted])
        choose(options[highlighted].key, options[highlighted].kind);
    }
  }
</script>

<button
  bind:this={triggerEl}
  type="button"
  role="combobox"
  aria-label="Select element"
  aria-haspopup="listbox"
  aria-controls="inspector-element-listbox"
  aria-expanded={open}
  onclick={toggleMenu}
  onkeydown={onTriggerKeydown}
  class="w-full flex items-center gap-1.5 min-w-0 text-[14px] rounded-md py-1 px-1.5 cursor-pointer transition-colors hover:bg-[#f1f3f7] focus:outline-none focus:ring-2 focus:ring-[#e6f1fb]"
>
  <span
    class={`flex-1 truncate text-left ${selectedLabel ? "font-semibold text-slate-900" : "font-medium text-slate-400"}`}
  >
    {selectedLabel ?? placeholder}
  </span>
  <ChevronDown
    size={15}
    class={`flex-none text-slate-400 transition-transform duration-150 ${open ? "rotate-180" : ""}`}
  />
</button>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="fixed inset-0 z-[59]" onclick={closeMenu}></div>
  <div
    id="inspector-element-listbox"
    role="listbox"
    aria-label="Select element"
    tabindex="-1"
    style={menuStyle}
    class="fixed z-[60] max-h-[280px] overflow-y-auto rounded-lg border border-[#d8dee8] bg-white shadow-[0_8px_24px_rgba(15,23,42,0.18)] py-1"
  >
    {#if options.length === 0}
      <div class="px-3 py-2 text-[13px] text-slate-400">No elements in this diagram</div>
    {/if}
    {#each options as opt, i (opt.key)}
      {@const RowIcon = KIND_ICON[opt.kind]}
      <button
        type="button"
        role="option"
        aria-selected={opt.key === selectedKey}
        onclick={() => choose(opt.key, opt.kind)}
        onmouseenter={() => (highlighted = i)}
        class={`w-full text-left px-3 py-2 text-[13px] cursor-pointer flex items-center gap-[7px] ${
          i === highlighted ? "bg-[#f1f3f7]" : ""
        } ${opt.key === selectedKey ? "text-[#1e88e5] font-[600]" : "text-slate-900"}`}
      >
        <RowIcon
          size={14}
          class={`flex-shrink-0 ${opt.key === selectedKey ? "text-[#1e88e5]" : "text-slate-400"}`}
        />
        <span class="truncate">{opt.label}</span>
        {#if opt.key === selectedKey}
          <Check size={14} class="ml-auto flex-shrink-0 text-[#1e88e5]" />
        {/if}
      </button>
    {/each}
  </div>
{/if}
