<script lang="ts">
  // Compact, selectable rendering of the package tree for the New Package
  // dialog's placement footer. Read-only: no context menus, no drag/drop. The
  // project root (empty path) is a valid, selectable target.
  let { packages, projectName, selected, onSelect }: {
    packages: { key: string }[];
    projectName: string;
    selected: string;
    onSelect: (path: string) => void;
  } = $props();

  // Depth by slash count drives the indent; packages are already keyed by full
  // path, so a lexicographic sort keeps children under their parent.
  const sorted = $derived([...packages].map((p) => p.key).sort());
  const label = (key: string) => key.slice(key.lastIndexOf("/") + 1);
  const depth = (key: string) => key.split("/").length;

  function rowClass(isSelected: boolean): string {
    return isSelected
      ? "bg-[#e6f1fb] text-[#1e88e5] font-[550]"
      : "text-slate-800 hover:bg-[#f1f3f7]";
  }
</script>

<div class="max-h-40 overflow-auto rounded-lg border border-[#e6e9f0] bg-white p-1 text-[13px]">
  <button
    type="button"
    onclick={() => onSelect("")}
    class="w-full text-left px-2 py-[6px] rounded-md cursor-pointer {rowClass(selected === '')}"
  >
    {projectName}
  </button>
  {#each sorted as key (key)}
    <button
      type="button"
      onclick={() => onSelect(key)}
      class="w-full text-left px-2 py-[6px] rounded-md cursor-pointer {rowClass(selected === key)}"
      style="padding-left: {depth(key) * 14 + 8}px"
    >
      {label(key)}
    </button>
  {/each}
</div>
