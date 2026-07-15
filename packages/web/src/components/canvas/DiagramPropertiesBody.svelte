<script lang="ts">
  // The active diagram's display controls, extracted from Dock's popover so the
  // central edit panel host can render the identical set. Display toggles only — no
  // title/profile. Each control emits a single changed field via onChange.
  import type { DiagramDisplay, Diagram } from "@waml/okf";

  let { display, diagram, candidateStereotypes, editable, onChange, onUpdateDiagram }: {
    display: DiagramDisplay;
    diagram: Diagram;
    candidateStereotypes: string[];
    editable: boolean;
    onChange: (patch: Partial<DiagramDisplay>) => void;
    onUpdateDiagram: (patch: Partial<Diagram>) => void;
  } = $props();

  function patch(p: Partial<DiagramDisplay>) {
    onChange(p);
  }

  let disabledAll = $derived(!editable);

  function commitTitle(v: string) {
    const t = v.trim();
    if (t && t !== diagram.title) onUpdateDiagram({ title: t });
  }
  function commitNote(v: string) {
    if (v !== (diagram.description ?? "")) onUpdateDiagram({ description: v });
  }
</script>

<!-- A labelled on/off toggle row inside the properties flyout. -->
{#snippet toggleRow(label: string, checked: boolean, onToggle: () => void, disabled = false)}
  <button
    type="button"
    role="switch"
    aria-checked={checked}
    aria-label={label}
    disabled={disabled}
    onclick={() => { if (!disabled) onToggle(); }}
    class="flex w-full items-center justify-between gap-3 rounded-lg px-2 py-1.5 text-left transition-colors {disabled
      ? 'opacity-40 cursor-not-allowed'
      : 'hover:bg-[#f1f3f7]'}"
  >
    <span class="text-[13px] font-medium text-slate-800">{label}</span>
    <span
      class="relative inline-flex h-[18px] w-[32px] flex-shrink-0 items-center rounded-full transition-colors {checked
        ? 'bg-[#1e88e5]'
        : 'bg-slate-300'}"
    >
      <span
        class="inline-block h-[14px] w-[14px] rounded-full bg-white shadow transition-transform {checked
          ? 'translate-x-[16px]'
          : 'translate-x-[2px]'}"
      ></span>
    </span>
  </button>
{/snippet}

<!-- A two-option segmented control (radio group) inside the properties flyout. -->
{#snippet segmented(label: string, options: { value: string; label: string }[], value: string, onPick: (v: string) => void, disabled = false)}
  <div class="px-2 py-1.5 {disabled ? 'opacity-40' : ''}">
    <div class="mb-1 text-[13px] font-medium text-slate-800">{label}</div>
    <div role="radiogroup" aria-label={label} class="flex gap-1 rounded-lg bg-[#f1f3f7] p-0.5">
      {#each options as opt (opt.value)}
        {@const selected = opt.value === value}
        <button
          type="button"
          role="radio"
          aria-checked={selected}
          aria-label={opt.label}
          disabled={disabled}
          onclick={() => { if (!disabled) onPick(opt.value); }}
          class="flex-1 rounded-md px-2 py-1 text-[12px] font-semibold transition-colors {disabled
            ? 'cursor-not-allowed'
            : 'cursor-pointer'} {selected ? 'bg-white text-[#1e88e5] shadow-sm' : 'text-slate-500 hover:text-slate-800'}"
        >
          {opt.label}
        </button>
      {/each}
    </div>
  </div>
{/snippet}

<div>
  {#if !editable}
    <div
      role="note"
      class="mx-1 mb-2 rounded-lg bg-[#fff7ed] px-3 py-2 text-[12px] leading-snug text-[#9a3412]"
    >
      Display and note settings save to a diagram. The <strong>All</strong> view can't store them — create
      a diagram to customize.
    </div>
  {/if}

  <div class="px-2 py-1.5">
    <label class="block">
      <span class="mb-1 block text-[13px] font-medium text-slate-800">Title</span>
      <input
        type="text"
        aria-label="Diagram title"
        value={diagram.title}
        disabled={disabledAll}
        onblur={(e) => commitTitle((e.currentTarget as HTMLInputElement).value)}
        onkeydown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            (e.currentTarget as HTMLInputElement).blur();
          }
        }}
        class="w-full rounded-md border border-slate-300 px-2 py-1 text-[13px] disabled:opacity-40"
      />
    </label>
    <label class="mt-2 block">
      <span class="mb-1 block text-[13px] font-medium text-slate-800">Note</span>
      <textarea
        aria-label="Diagram note"
        rows="3"
        disabled={disabledAll}
        placeholder="Notes about this diagram (not shown on canvas)."
        value={diagram.description ?? ""}
        onblur={(e) => commitNote((e.currentTarget as HTMLTextAreaElement).value)}
        class="w-full resize-y rounded-md border border-slate-300 px-2 py-1 text-[13px] disabled:opacity-40"
      ></textarea>
    </label>
  </div>

  <div class="h-px bg-[#eef1f5] mx-1 my-1"></div>

  {@render toggleRow("Show attributes", display.showAttributes, () =>
    patch({ showAttributes: !display.showAttributes }), disabledAll,
  )}
  {@render segmented(
    "Attribute detail",
    [
      { value: "name-only", label: "Name only" },
      { value: "name-type", label: "Name + type" },
    ],
    display.attributeDetail,
    (v) => patch({ attributeDetail: v as DiagramDisplay["attributeDetail"] }),
    !display.showAttributes || disabledAll,
  )}
  {@render toggleRow("Show visibility", display.showAttributeVisibility, () =>
    patch({ showAttributeVisibility: !display.showAttributeVisibility }), !display.showAttributes || disabledAll,
  )}
  {@render toggleRow("Show multiplicity", display.showAttributeMultiplicity, () =>
    patch({ showAttributeMultiplicity: !display.showAttributeMultiplicity }), !display.showAttributes || disabledAll,
  )}
  <div class="px-2 py-1.5 {(!display.showAttributes || disabledAll) ? 'opacity-40' : ''}">
    <div class="mb-1 text-[13px] font-medium text-slate-800">Max attributes</div>
    <div class="flex items-center gap-2">
      <input
        type="number"
        min="1"
        aria-label="Max attributes"
        placeholder="∞"
        value={display.maxAttributes ?? ""}
        disabled={!display.showAttributes || disabledAll}
        oninput={(e) => {
          const n = Number((e.currentTarget as HTMLInputElement).value);
          if (Number.isFinite(n) && n >= 1) patch({ maxAttributes: Math.floor(n) });
        }}
        class="w-16 rounded-md border border-slate-300 px-2 py-1 text-[13px] disabled:opacity-40"
      />
      <button
        type="button"
        aria-label="Unlimited attributes"
        disabled={!display.showAttributes || disabledAll}
        onclick={() => {
          if (display.showAttributes && !disabledAll) patch({ maxAttributes: undefined });
        }}
        class="rounded-md px-2 py-1 text-[12px] font-semibold {display.maxAttributes === undefined
          ? 'bg-white text-[#1e88e5] shadow-sm'
          : 'text-slate-500'}"
      >
        Unlimited
      </button>
    </div>
  </div>

  <div class="h-px bg-[#eef1f5] mx-1 my-1"></div>

  {@render segmented(
    "Associations",
    [
      { value: "all", label: "Show labels" },
      { value: "hidden", label: "Hide labels" },
    ],
    display.associationLabels,
    (v) => patch({ associationLabels: v as DiagramDisplay["associationLabels"] }),
    disabledAll,
  )}
  {@render toggleRow("Emphasize multiplicity", display.emphasizeMultiplicity, () =>
    patch({ emphasizeMultiplicity: !display.emphasizeMultiplicity }), disabledAll,
  )}

  <div class="h-px bg-[#eef1f5] mx-1 my-1"></div>

  {@render toggleRow("Show stereotype", display.showStereotype, () =>
    patch({ showStereotype: !display.showStereotype }), disabledAll,
  )}
</div>
