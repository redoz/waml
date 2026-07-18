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
  type="button" role="combobox" aria-label="Select element"
  aria-haspopup="listbox" aria-controls="inspector-element-listbox" aria-expanded={open}
  onclick={toggleMenu} onkeydown={onTriggerKeydown}
  class="ep-trigger"
>
  <span class={`ep-label ${selectedLabel ? "" : "ep-label--empty"}`}>{selectedLabel ?? placeholder}</span>
  <ChevronDown size={15} class="ep-chev" style={open ? "transform:rotate(180deg)" : ""} />
</button>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="ep-scrim" onclick={closeMenu}></div>
  <div id="inspector-element-listbox" role="listbox" aria-label="Select element" tabindex="-1" style={menuStyle} class="ep-menu">
    {#if options.length === 0}
      <div class="ep-empty">No elements in this diagram</div>
    {/if}
    {#each options as opt, i (opt.key)}
      {@const RowIcon = KIND_ICON[opt.kind]}
      <button
        type="button" role="option" aria-selected={opt.key === selectedKey}
        onclick={() => choose(opt.key, opt.kind)} onmouseenter={() => (highlighted = i)}
        class={`ep-opt ${i === highlighted ? "is-hi" : ""} ${opt.key === selectedKey ? "is-sel" : ""}`}
      >
        <RowIcon size={14} style="flex:none" />
        <span style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap">{opt.label}</span>
        {#if opt.key === selectedKey}<Check size={14} style="margin-left:auto;flex:none" />{/if}
      </button>
    {/each}
  </div>
{/if}

<style>
  .ep-trigger {
    width: 100%; display: flex; align-items: center; gap: 6px; min-width: 0;
    font: 500 14px/1 var(--font-ui); border: 0; background: transparent;
    border-radius: 2px; padding: 4px 6px; cursor: pointer;
  }
  .ep-trigger:hover { background: rgba(var(--accent), .10); }
  .ep-label { flex: 1; text-align: left; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--ink); font-weight: 600; }
  .ep-label--empty { color: rgb(var(--ink-faint)); font-weight: 500; }
  .ep-chev { flex: none; color: rgb(var(--ink-faint)); transition: transform .15s ease; }
  .ep-scrim { position: fixed; inset: 0; z-index: 59; }
  .ep-menu {
    position: fixed; z-index: 60; max-height: 280px; overflow-y: auto; padding: 6px;
    background: linear-gradient(180deg, rgba(255,255,255,.95), rgba(255,255,255,.82)), rgba(var(--accent), .06);
    box-shadow: 0 12px 30px rgba(40,70,110,.20), 0 0 calc(14px * var(--glow)) rgba(var(--accent), calc(.16 * var(--glow)));
  }
  .ep-empty { padding: 8px 12px; font: 500 13px/1 var(--font-ui); color: rgb(var(--ink-faint)); }
  .ep-opt {
    width: 100%; text-align: left; display: flex; align-items: center; gap: 7px;
    padding: 8px 11px; border: 0; background: transparent; border-radius: 2px; cursor: pointer;
    font: 500 13px/1 var(--font-ui); color: var(--ink);
  }
  .ep-opt.is-hi { background: rgba(var(--accent), .12); }
  .ep-opt.is-sel { color: rgb(var(--accent)); font-weight: 600; }
</style>
