<script lang="ts">
  // The active diagram's display controls, extracted from Dock's popover so the
  // central edit panel host can render the identical set. Display toggles only — no
  // title/profile. Each control emits a single changed field via onChange.
  import type { DiagramDisplay, Diagram } from "@waml/okf";
  import { inputCls, labelCls } from "../inspector/field-styles";

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
  let attrDisabled = $derived(!display.showAttributes || disabledAll);
  let stereoDisabled = $derived(!display.showStereotype || disabledAll);

  function toggleFilter(name: string) {
    if (stereoDisabled) return;
    const current = display.stereotypeFilter === undefined ? [...candidateStereotypes] : [...display.stereotypeFilter];
    const idx = current.indexOf(name);
    if (idx >= 0) current.splice(idx, 1);
    else current.push(name);
    patch({ stereotypeFilter: current });
  }
  function setColor(name: string, hex: string) {
    if (stereoDisabled) return;
    patch({ stereotypeColors: { ...display.stereotypeColors, [name]: hex } });
  }
  function clearColor(name: string) {
    if (stereoDisabled) return;
    const next = { ...display.stereotypeColors };
    delete next[name];
    patch({ stereotypeColors: next });
  }

  function commitTitle(v: string) {
    const t = v.trim();
    if (t && t !== diagram.title) onUpdateDiagram({ title: t });
  }
  function commitNote(v: string) {
    if (v !== (diagram.description ?? "")) onUpdateDiagram({ description: v });
  }

  // Title and Note field labels — one weight below the uppercase section
  // headers so the hierarchy reads at a glance.
  const fieldLabelCls = "mb-1 block text-[13px] font-medium text-slate-800";
</script>

<!-- A labelled on/off toggle row inside the properties flyout. -->
{#snippet toggleRow(label: string, checked: boolean, onToggle: () => void, disabled = false, ariaLabel = label)}
  <button
    type="button"
    role="switch"
    aria-checked={checked}
    aria-label={ariaLabel}
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

<div class="flex flex-col gap-4 py-1">
  {#if !editable}
    <div
      role="note"
      class="mx-1 rounded-lg bg-[#fff7ed] px-3 py-2 text-[12px] leading-snug text-[#9a3412]"
    >
      Display and note settings save to a diagram. The <strong>All</strong> view can't store them — create
      a diagram to customize.
    </div>
  {/if}

  <section class="flex flex-col">
    <h3 class="px-2 {labelCls}">Identity</h3>
    <div class="px-2">
      <label class="block">
        <span class={fieldLabelCls}>Title</span>
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
          class={`${inputCls} disabled:opacity-40`}
        />
      </label>
      <label class="mt-2 block">
        <span class={fieldLabelCls}>Note</span>
        <textarea
          aria-label="Diagram note"
          rows="3"
          disabled={disabledAll}
          placeholder="Notes about this diagram (not shown on canvas)."
          value={diagram.description ?? ""}
          onblur={(e) => commitNote((e.currentTarget as HTMLTextAreaElement).value)}
          class={`${inputCls} resize-y disabled:opacity-40`}
        ></textarea>
      </label>
    </div>
  </section>

  <section class="flex flex-col border-t border-[#d8dee8] pt-4">
    <h3 class="px-2 {labelCls}">Attributes</h3>
    <div>
      {@render toggleRow("Show attributes", display.showAttributes, () =>
        patch({ showAttributes: !display.showAttributes }), disabledAll,
      )}
      {@render toggleRow("Show type", display.showType, () =>
        patch({ showType: !display.showType }), attrDisabled,
      )}
      {@render toggleRow("Show visibility", display.showAttributeVisibility, () =>
        patch({ showAttributeVisibility: !display.showAttributeVisibility }), attrDisabled,
      )}
      {@render toggleRow("Show cardinality", display.showAttributeMultiplicity, () =>
        patch({ showAttributeMultiplicity: !display.showAttributeMultiplicity }), attrDisabled, "Attribute cardinality",
      )}
      <div class="px-2 py-1.5 {attrDisabled ? 'opacity-40' : ''}">
        <div class="mb-1 text-[13px] font-medium text-slate-800">Max attributes</div>
        <div class="flex items-center gap-2">
          <input
            type="number"
            min="1"
            aria-label="Max attributes"
            placeholder="∞"
            value={display.maxAttributes ?? ""}
            disabled={attrDisabled}
            oninput={(e) => {
              const n = Number((e.currentTarget as HTMLInputElement).value);
              if (Number.isFinite(n) && n >= 1) patch({ maxAttributes: Math.floor(n) });
            }}
            class="w-16 rounded-md border border-slate-300 px-2 py-1 text-[13px] disabled:opacity-40"
          />
          <button
            type="button"
            aria-label="Unlimited attributes"
            disabled={attrDisabled}
            onclick={() => {
              if (!attrDisabled) patch({ maxAttributes: undefined });
            }}
            class="rounded-md px-2 py-1 text-[12px] font-semibold {display.maxAttributes === undefined
              ? 'bg-white text-[#1e88e5] shadow-sm'
              : 'text-slate-500'}"
          >
            Unlimited
          </button>
        </div>
      </div>
    </div>
  </section>

  <section class="flex flex-col border-t border-[#d8dee8] pt-4">
    <h3 class="px-2 {labelCls}">Relationships</h3>
    <div>
      {@render toggleRow("Show roles", display.showRoles, () =>
        patch({ showRoles: !display.showRoles }), disabledAll,
      )}
      {@render toggleRow("Show cardinality", display.showCardinality, () =>
        patch({ showCardinality: !display.showCardinality }), disabledAll,
      )}
      {@render toggleRow("Show labels", display.showLabels, () =>
        patch({ showLabels: !display.showLabels }), disabledAll,
      )}
    </div>
  </section>

  <section class="flex flex-col border-t border-[#d8dee8] pt-4">
    <h3 class="px-2 {labelCls}">Stereotypes</h3>
    <div>
      {@render toggleRow("Show stereotype", display.showStereotype, () =>
        patch({ showStereotype: !display.showStereotype }), disabledAll,
      )}

      <div class="px-2 py-1.5 {stereoDisabled ? 'opacity-40' : ''}">
        <div class="mb-1 text-[13px] font-medium text-slate-800">Stereotype filter</div>
        {#if candidateStereotypes.length === 0}
          <div class="text-[12px] text-slate-400">No stereotypes on this diagram's members yet.</div>
        {:else}
          <label class="flex items-center gap-2 py-0.5 text-[12px] text-slate-700">
            <input
              type="checkbox" aria-label="Show all stereotypes"
              checked={display.stereotypeFilter === undefined} disabled={stereoDisabled}
              onchange={() => { if (!stereoDisabled) patch({ stereotypeFilter: undefined }); }} />
            <span>Show all</span>
          </label>
          {#each candidateStereotypes as name (name)}
            {@const checked = display.stereotypeFilter === undefined ? true : display.stereotypeFilter.includes(name)}
            <label class="flex items-center gap-2 py-0.5 text-[12px] text-slate-700">
              <input type="checkbox" aria-label={name} checked={checked} disabled={stereoDisabled} onchange={() => toggleFilter(name)} />
              <span>{name}</span>
            </label>
          {/each}
        {/if}
      </div>

      <div class="px-2 py-1.5 {stereoDisabled ? 'opacity-40' : ''}">
        <div class="mb-1 text-[13px] font-medium text-slate-800">Stereotype colors</div>
        {#if candidateStereotypes.length === 0}
          <div class="text-[12px] text-slate-400">No stereotypes on this diagram's members yet.</div>
        {:else}
          {#each candidateStereotypes as name (name)}
            <div class="flex items-center gap-2 py-0.5 text-[12px] text-slate-700">
              <input
                type="color" aria-label={`Color for ${name}`} disabled={stereoDisabled}
                value={display.stereotypeColors[name] ?? "#dddddd"}
                oninput={(e) => setColor(name, (e.currentTarget as HTMLInputElement).value)}
                class="h-6 w-8 rounded border border-slate-300" />
              <span class="flex-1">{name}</span>
              {#if display.stereotypeColors[name]}
                <button
                  type="button" aria-label={`Clear color for ${name}`} disabled={stereoDisabled}
                  onclick={() => clearColor(name)}
                  class="text-slate-400 hover:text-slate-700">Clear</button>
              {/if}
            </div>
          {/each}
        {/if}
      </div>
    </div>
  </section>
</div>
