<script module lang="ts">
  export type DiagramKind = "class" | "usecase" | "activity" | "sequence";
  export type NewPackagePayload =
    | { tier: "empty"; parentPath: string; name: string }
    | { tier: "diagram"; parentPath: string; name: string; kind: DiagramKind }
    | { tier: "template"; parentPath: string; name: string; bundle: [string, string][] };
</script>

<script lang="ts">
  import type { Template } from "@waml/core/templates";
  import { slugify } from "@waml/okf";
  import PackageTreePicker from "./PackageTreePicker.svelte";

  let { templates, packages, projectName, onAdd, onClose }: {
    templates: Template[];
    packages: { key: string }[];
    projectName: string;
    onAdd: (p: NewPackagePayload) => void;
    onClose: () => void;
  } = $props();

  type Tier = "empty" | "diagram" | "template";
  const KIND_LABELS: Record<DiagramKind, string> = {
    class: "Class / Domain",
    usecase: "Use-case",
    activity: "Activity",
    sequence: "Sequence",
  };

  let tier = $state<Tier>("empty");
  let kind = $state<DiagramKind>("class");
  let templateId = $state<string | null>(null);
  let parentPath = $state("");
  let name = $state("New package");
  // Tracks whether the user has hand-edited the name; if not, the name follows
  // the tier/kind/template default.
  let nameDirty = $state(false);

  const selectedTemplate = $derived(templates.find((t) => t.id === templateId) ?? null);

  function cleanTemplateName(n: string): string {
    return n.replace(/\s*\(UML\)\s*$/i, "").trim();
  }

  // The default name for the current tier/selection.
  const defaultName = $derived(
    tier === "empty"
      ? "New package"
      : tier === "diagram"
        ? KIND_LABELS[kind]
        : selectedTemplate
          ? cleanTemplateName(selectedTemplate.name)
          : "New package",
  );

  // Keep the name in sync with the default until the user edits it.
  $effect(() => {
    if (!nameDirty) name = defaultName;
  });

  // Collision: does <parentPath>/<slug> already exist as a package path?
  const targetPath = $derived(
    (() => {
      const s = slugify(name);
      return parentPath ? `${parentPath}/${s}` : s;
    })(),
  );
  const collision = $derived(name.trim().length > 0 && packages.some((p) => p.key === targetPath));
  const canAdd = $derived(name.trim().length > 0 && !collision && (tier !== "template" || selectedTemplate !== null));

  function selectTier(t: Tier) {
    tier = t;
    nameDirty = false;
  }
  function selectKind(k: DiagramKind) {
    kind = k;
    nameDirty = false;
  }
  function selectTemplate(id: string) {
    templateId = id;
    nameDirty = false;
  }

  function submit() {
    if (!canAdd) return;
    const trimmed = name.trim();
    if (tier === "empty") onAdd({ tier: "empty", parentPath, name: trimmed });
    else if (tier === "diagram") onAdd({ tier: "diagram", parentPath, name: trimmed, kind });
    else if (selectedTemplate) onAdd({ tier: "template", parentPath, name: trimmed, bundle: selectedTemplate.bundle });
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="fixed inset-0 z-[60] flex items-center justify-center bg-black/40"
  onclick={(e) => { if (e.target === e.currentTarget) onClose(); }}
>
  <div class="bg-white rounded-xl shadow-xl w-[480px] max-w-[95vw] p-6 flex flex-col gap-4">
    <div class="flex items-center justify-between">
      <h2 class="text-[15px] font-semibold text-slate-900">New package</h2>
      <button onclick={onClose} class="text-slate-400 hover:text-slate-700 text-xl leading-none px-1">✕</button>
    </div>

    <!-- Tier selector -->
    <div class="grid grid-cols-3 gap-2">
      {#each [["empty", "Empty"], ["diagram", "Diagram"], ["template", "Template"]] as [t, lbl] (t)}
        <button
          type="button"
          onclick={() => selectTier(t as Tier)}
          class="rounded-lg border px-3 py-2 text-[13px] cursor-pointer {tier === t ? 'border-[#1e88e5] bg-[#e6f1fb] text-[#1e88e5] font-[550]' : 'border-[#d8dee8] text-slate-800 hover:bg-[#f1f3f7]'}"
        >
          {lbl}
        </button>
      {/each}
    </div>

    <!-- Contextual middle -->
    {#if tier === "diagram"}
      <div class="grid grid-cols-2 gap-2 border-t border-slate-100 pt-3">
        {#each Object.entries(KIND_LABELS) as [k, lbl] (k)}
          <button
            type="button"
            onclick={() => selectKind(k as DiagramKind)}
            class="rounded-lg border px-3 py-2 text-[13px] cursor-pointer {kind === k ? 'border-[#1e88e5] bg-[#e6f1fb] text-[#1e88e5] font-[550]' : 'border-[#d8dee8] text-slate-800 hover:bg-[#f1f3f7]'}"
          >
            {lbl}
          </button>
        {/each}
      </div>
    {:else if tier === "template"}
      <div class="flex flex-col gap-1.5 border-t border-slate-100 pt-3 max-h-48 overflow-auto">
        {#each templates as t (t.id)}
          <button
            type="button"
            onclick={() => selectTemplate(t.id)}
            class="text-left rounded-lg border px-3 py-2 cursor-pointer {templateId === t.id ? 'border-[#1e88e5] bg-[#e6f1fb]' : 'border-[#d8dee8] hover:bg-[#f1f3f7]'}"
          >
            <div class="text-[13px] font-[550] text-slate-900">{t.name}</div>
            <div class="text-[12px] text-slate-500">{t.description}</div>
          </button>
        {/each}
      </div>
    {/if}

    <!-- Placement footer -->
    <div class="flex flex-col gap-2 border-t border-slate-100 pt-3">
      <span class="text-[12px] font-medium text-slate-500">Place under</span>
      <PackageTreePicker {packages} {projectName} selected={parentPath} onSelect={(p) => (parentPath = p)} />
      <label class="flex flex-col gap-1 text-[12px] font-medium text-slate-500">
        Name
        <input
          aria-label="Package name"
          bind:value={name}
          oninput={() => (nameDirty = true)}
          class="text-[13px] px-2 py-[7px] border border-[#d8dee8] rounded-md text-slate-900 focus:outline-none focus:border-[#1e88e5] focus:ring-2 focus:ring-[#e6f1fb]"
        />
      </label>
      {#if collision}
        <p class="text-[12px] text-[#d93025]">name already used here</p>
      {/if}
    </div>

    <div class="flex gap-2 justify-end">
      <button
        onclick={onClose}
        class="text-[13px] font-[550] border border-[#d8dee8] bg-white text-slate-900 rounded-lg px-4 py-[7px] cursor-pointer hover:bg-[#f1f3f7]"
      >
        Cancel
      </button>
      <button
        onclick={submit}
        disabled={!canAdd}
        class="text-[13px] font-[550] bg-[#1e88e5] text-white border border-[#1e88e5] rounded-lg px-4 py-[7px] cursor-pointer hover:bg-[#1976d2] disabled:opacity-50 disabled:cursor-not-allowed"
      >
        Add
      </button>
    </div>
  </div>
</div>
