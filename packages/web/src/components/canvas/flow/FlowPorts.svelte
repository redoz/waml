<script lang="ts">
  import { Handle, Position } from "@xyflow/svelte";

  // Flow views are read-only, so these ports are never grabbable — they exist
  // purely so SvelteFlow initialises each node's handleBounds. Without a source
  // AND a target handle, getEdgePosition() returns null and the transition edge
  // is dropped before it ever reaches TransitionEdge (see @xyflow/system
  // isNodeInitialized). TransitionEdge computes its own floating attach point, so
  // the handle positions here don't affect routing.
  const hidden = "width:1px;height:1px;opacity:0;pointer-events:none;border:0;min-width:0;min-height:0;";
</script>

<Handle type="target" position={Position.Top} isConnectable={false} style={hidden} />
<Handle type="source" position={Position.Bottom} isConnectable={false} style={hidden} />
