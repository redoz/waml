<script lang="ts">
  import { Handle, Position } from "@xyflow/svelte";

  // Connection ports — a small dot at each side, revealed on node hover/selection
  // (.node-handle in canvas.css). They're the drag-to-connect affordance and the
  // reconnect drop target; ConnectionMode.Loose lets any source handle also receive.
  // Floating edges compute their own border attach point, so which dot a connection
  // lands on never affects routing — the four dots just give the user a port to grab
  // on whichever side faces the target. They sit ABOVE the content (zIndex 10) so a
  // drag that starts on a dot begins a connection, while a drag on the node body
  // still moves the node and clicks on rows/buttons still register.
  //
  // Centre each dot exactly on its border midpoint (half in / half out) with an
  // explicit transform, overriding SvelteFlow's per-side default offsets so all
  // four straddle the edge symmetrically and stay grabbable from just inside.
  // Handle's `style` prop is a plain CSS string (unlike React Flow's style object).
  // Per-side offsets only; visual style comes from the global `.node-port` class
  // (accent-themed, revealed on hover/select via canvas.css .node-handle rule).
</script>

<Handle type="source" position={Position.Left} id="l" isConnectable class="node-handle node-port" style="left:0;top:50%;" />
<Handle type="source" position={Position.Right} id="r" isConnectable class="node-handle node-port" style="left:100%;top:50%;" />
<Handle type="source" position={Position.Top} id="t" isConnectable class="node-handle node-port" style="left:50%;top:0;" />
<Handle type="source" position={Position.Bottom} id="b" isConnectable class="node-handle node-port" style="left:50%;top:100%;" />
