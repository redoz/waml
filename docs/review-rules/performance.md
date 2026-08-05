# Performance

Evaluate whether the code uses resources appropriately and will scale.

## Checklist

### Hot Paths
- Are there hot paths doing unnecessary work? (redundant lookups, repeated serialization, re-parsing unchanged source)
- Are there O(n^2) or worse patterns on data that can grow? (nodes per diagram, edges per node, documents per session)
- For the parse path specifically: does an incremental edit cost time proportional to the edit, not to the file?
- For the layout and edge-routing solver: does cost stay bounded as the diagram grows?

### Draw & Frame Budget
- Does the change add work to `draw_walk` or `handle_event` that runs every frame rather than on change?
- Does it force text re-rasterization? Text is rasterized per zoom-scaled size — new sizes on a continuous gesture are the expensive case.
- Are redraws scoped to the affected area, or does the change trigger a full-tree redraw for a local update?
- Is expensive work cached, and is the cache keyed on everything that invalidates it?

### Allocation
- Is there unnecessary cloning or allocation in a per-frame or per-event path? (cloning the model to read one field, rebuilding a `String` per row)
- Are large structures moved or borrowed rather than copied?
- Does a collection get rebuilt each frame where a retained buffer would do?

### Web & Boot
- Does the change grow the wasm artifact or add work to the boot path? Boot cost is measured, not guessed — see `scripts/measure-web-boot.mjs`.
- Does it add a shader program or a font that must be compiled or fetched before the first frame?

## Scope Guidance

- **Full evaluation**: Measure startup, web boot, document open, full parse, layout solve, and zoom or pan interaction.
- **Change review**: Focus on whether the change adds per-frame work, introduces an O(n^2) pattern, allocates in a hot loop, or grows the boot path.
