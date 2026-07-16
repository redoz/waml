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

  let { templates, packages, projectName, onAdd, onClose }: {
    templates: Template[];
    packages: { key: string }[];
    projectName: string;
    onAdd: (p: NewPackagePayload) => void;
    onClose: () => void;
  } = $props();

  // The starter's payload shape - what pkg.insert receives when this item is
  // picked. Empty and the diagram kinds are synthetic; templates map straight in.
  type Make =
    | { tier: "empty" }
    | { tier: "diagram"; kind: DiagramKind }
    | { tier: "template"; bundle: [string, string][] };
  type Item = { id: string; name: string; description: string; make: Make };

  const KIND_LABELS: Record<DiagramKind, string> = {
    class: "Domain model",
    usecase: "Use-case",
    activity: "Activity",
    sequence: "Sequence",
  };
  const KIND_DESC: Record<DiagramKind, string> = {
    class: "Blank domain model",
    usecase: "Blank use-case diagram",
    activity: "Blank activity diagram",
    sequence: "Blank sequence diagram",
  };
  const KINDS = Object.keys(KIND_LABELS) as DiagramKind[];

  function cleanTemplateName(n: string): string {
    return n.replace(/\s*\(UML\)\s*$/i, "").trim();
  }

  // One flat starter list: a blank package, the four empty-diagram kinds, then
  // the committed templates. Rendered as uniform cards.
  const items = $derived<Item[]>([
    { id: "empty", name: "Empty package", description: "No diagram - materializes on first child", make: { tier: "empty" } },
    ...KINDS.map((k) => ({ id: `diagram:${k}`, name: KIND_LABELS[k], description: KIND_DESC[k], make: { tier: "diagram" as const, kind: k } })),
    ...templates.map((t) => ({ id: `template:${t.id}`, name: t.name, description: t.description, make: { tier: "template" as const, bundle: t.bundle } })),
  ]);

  let selectedId = $state("empty");
  let parentPath = $state("");
  let name = $state("New package");
  // Tracks whether the user has hand-edited the name; if not, the name follows
  // the selected starter's default.
  let nameDirty = $state(false);

  const selected = $derived(items.find((it) => it.id === selectedId) ?? items[0]);

  // The default name for the selected starter: a generic name for a blank
  // package, the kind label for a diagram, the cleaned template name otherwise.
  const defaultName = $derived(
    selected.make.tier === "empty"
      ? "New package"
      : selected.make.tier === "diagram"
        ? KIND_LABELS[selected.make.kind]
        : cleanTemplateName(selected.name),
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
  const canAdd = $derived(name.trim().length > 0 && !collision);

  // Placement targets as flat <select> options: project root plus every package,
  // sorted by full path (keeps children under parents) and indented by depth.
  const placeOptions = $derived(
    [...packages]
      .map((p) => p.key)
      .sort()
      .map((key) => ({
        key,
        label: " ".repeat((key.split("/").length - 1) * 2) + key.slice(key.lastIndexOf("/") + 1),
      })),
  );

  function selectItem(id: string) {
    selectedId = id;
    nameDirty = false;
  }

  function submit() {
    if (!canAdd) return;
    const trimmed = name.trim();
    const m = selected.make;
    if (m.tier === "empty") onAdd({ tier: "empty", parentPath, name: trimmed });
    else if (m.tier === "diagram") onAdd({ tier: "diagram", parentPath, name: trimmed, kind: m.kind });
    else onAdd({ tier: "template", parentPath, name: trimmed, bundle: m.bundle });
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

    <!-- Name + placement, on top -->
    <div class="flex flex-col gap-3">
      <label class="flex flex-col gap-1 text-[12px] font-medium text-slate-500">
        Name
        <input
          aria-label="Package name"
          bind:value={name}
          oninput={() => (nameDirty = true)}
          placeholder={defaultName}
          class="text-[13px] px-2 py-[7px] border border-[#d8dee8] rounded-md text-slate-900 focus:outline-none focus:border-[#1e88e5] focus:ring-2 focus:ring-[#e6f1fb]"
        />
      </label>
      {#if collision}
        <p class="text-[12px] text-[#d93025] -mt-1">name already used here</p>
      {/if}

      <label class="flex flex-col gap-1 text-[12px] font-medium text-slate-500">
        Place in
        <select
          aria-label="Place in"
          bind:value={parentPath}
          class="text-[13px] px-2 py-[7px] border border-[#d8dee8] rounded-md text-slate-900 bg-white cursor-pointer focus:outline-none focus:border-[#1e88e5] focus:ring-2 focus:ring-[#e6f1fb]"
        >
          <option value="">{projectName}</option>
          {#each placeOptions as o (o.key)}
            <option value={o.key}>{o.label}</option>
          {/each}
        </select>
      </label>
    </div>

    <!-- Starter list -->
    <div class="flex flex-col gap-1.5 border-t border-slate-100 pt-3">
      <span class="text-[12px] font-medium text-slate-500">Start from</span>
      <div class="flex flex-col gap-1.5 max-h-64 overflow-auto">
      {#each items as it (it.id)}
        <button
          type="button"
          onclick={() => selectItem(it.id)}
          class="text-left rounded-lg border px-3 py-2 cursor-pointer {selectedId === it.id ? 'border-[#1e88e5] bg-[#e6f1fb]' : 'border-[#d8dee8] hover:bg-[#f1f3f7]'}"
        >
          <div class="text-[13px] font-[550] text-slate-900">{it.name}</div>
          <div class="text-[12px] text-slate-500">{it.description}</div>
        </button>
      {/each}
      </div>
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
