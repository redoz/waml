<script lang="ts">
  // The active diagram's display controls, extracted from Dock's popover so the
  // central edit panel host can render the identical set. Display toggles only — no
  // title/profile. Each control emits a single changed field via onChange.
  import type { DiagramDisplay } from "@uaml/okf";

  let { display, onChange }: {
    display: DiagramDisplay;
    onChange: (patch: Partial<DiagramDisplay>) => void;
  } = $props();

  function patch(p: Partial<DiagramDisplay>) {
    onChange(p);
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
  {@render toggleRow("Show attributes", display.showAttributes, () =>
    patch({ showAttributes: !display.showAttributes }),
  )}
  {@render segmented(
    "Attribute detail",
    [
      { value: "name-only", label: "Name only" },
      { value: "name-type", label: "Name + type" },
    ],
    display.attributeDetail,
    (v) => patch({ attributeDetail: v as DiagramDisplay["attributeDetail"] }),
    !display.showAttributes,
  )}

  <div class="h-px bg-[#eef1f5] mx-1 my-1"></div>

  {@render segmented(
    "Associations",
    [
      { value: "all", label: "Show labels" },
      { value: "hidden", label: "Hide labels" },
    ],
    display.associationLabels,
    (v) => patch({ associationLabels: v as DiagramDisplay["associationLabels"] }),
  )}
  {@render toggleRow("Emphasize multiplicity", display.emphasizeMultiplicity, () =>
    patch({ emphasizeMultiplicity: !display.emphasizeMultiplicity }),
  )}

  <div class="h-px bg-[#eef1f5] mx-1 my-1"></div>

  {@render toggleRow("Show stereotype", display.showStereotype, () =>
    patch({ showStereotype: !display.showStereotype }),
  )}
</div>
