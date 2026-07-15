<script lang="ts">
  // Thin back-compat wrapper: the old fixed-width dropdown card around the
  // extracted NavigatorBody. Kept only until CanvasInner mounts NavigatorPanel
  // directly (then this file is removed).
  import NavigatorBody from "./NavigatorBody.svelte";
  import type { ModelGraph } from "@waml/okf";
  import type { NavKind } from "@waml/core/nav/tree";

  let props: {
    graph: ModelGraph;
    scopeKey?: string;
    activeDiagramKey?: string;
    palette?: string[];
    onScope?: (key: string) => void;
    onSelectDiagram?: (key: string) => void;
    onReorder?: (pkgKey: string, order: string[]) => void;
    onViewInDiagram?: (key: string, diagramKey: string) => void;
    onAddToNewDiagram?: (key: string) => void;
    onEditProperties?: (key: string) => void;
    onCreatePackage?: (parentKey: string, name: string) => void;
    onCreateNode?: (dir: string, metaclass: string) => void;
    onCreateDiagram?: (name: string) => void;
    onRename?: (key: string, kind: NavKind, title: string) => void;
    onSort?: (pkgKey: string) => void;
    onDelete?: (key: string, kind: NavKind, mode: "single" | "cascade" | "reparent") => void;
  } = $props();
</script>

<div class="w-[300px] h-[440px] rounded-lg border border-[#d8dee8] bg-white shadow-[0_8px_24px_rgba(15,23,42,0.18)] overflow-hidden">
  <NavigatorBody {...props} />
</div>
