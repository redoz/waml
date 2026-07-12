# Straighten Edges Within a Shared Border Band — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Draw a RelEdge/AnchorEdge as a straight line (not a smooth-step jog) whenever a single straight line can hit both nodes' facing border strips head-on, and snap back to smooth-step when the nodes slide out of that shared band — on both axes.

**Architecture:** Add one pure helper `straightPort` to `floating.ts` that, given the two node rects, the source side, and the source slot, decides whether the two 0.72 slot-strips overlap enough to seat the edge's fan and returns the shared coordinate. `RelEdge.svelte` calls it in its `geometry` derived and switches `getSmoothStepPath` → `getStraightPath` when it fires; `AnchorEdge.svelte` (already straight) calls it to align its endpoints head-on.

**Tech Stack:** Svelte 5 (runes), `@xyflow/svelte` v1 (`getSmoothStepPath`, `getStraightPath`, `Position`), Vitest, TypeScript.

## Global Constraints

- Worktree: run all commands from `C:/dev/uaml/.claude/worktrees/floating-inspector`.
- Spec: `docs/superpowers/specs/2026-07-12-straighten-edges-shared-band-design.md`.
- Slot band constant is `0.72` (matches existing `portPoint`); ports occupy the central 72% of each border, i.e. fraction range `[0.14, 0.86]`.
- Straighten threshold: overlap height `>= max(8, groupSpan)` where `groupSpan = gap*(count-1)` — folds the "minimum overlap" and "match stepped spacing" decisions into one rule. No compression, no hysteresis.
- Fan seating uses the **source** slot only (a hub-to-hub pair shares edges, so distinct source slots already give distinct parallel coords). Do not add target-slot averaging.
- No Co-Authored-By Claude trailer on commits.
- Gate each task with `pnpm --filter @uaml/web test` (unit) and `pnpm --filter @uaml/web check` (svelte-check) as specified per task.

---

### Task 1: `straightPort` helper in `floating.ts`

**Files:**
- Modify: `packages/web/src/components/canvas/floating.ts:86-99` (extract `0.72` to a module `BAND` const; add `slotStrip` + `straightPort` + `StraightResult` below `portPoint`)
- Test: `packages/web/src/components/canvas/floating.test.ts` (append a `describe("straightPort ...")` block)

**Interfaces:**
- Consumes: existing `Rect` (`{x,y,w,h}`), `Slot` (`{index,count}`), `Position` from `@xyflow/svelte`.
- Produces:
  - `export type StraightResult = { straight: true; coord: number } | { straight: false };`
  - `export function straightPort(srcRect: Rect, tgtRect: Rect, side: Position, slot?: Slot): StraightResult;`
    — `side` is the **source** side; the target side is assumed opposite. `coord` is the shared **y** for `Left`/`Right` pairs, shared **x** for `Top`/`Bottom` pairs. Solo edge → `coord` is the overlap-band midpoint.

- [ ] **Step 1: Write the failing tests**

Append to `packages/web/src/components/canvas/floating.test.ts`:

```ts
import { getEdgeParams, portPoint, straightPort } from "./floating";

describe("straightPort (straighten within shared slot band)", () => {
  // 100x100 rects; slot strip is the central 72% → y in [14, 86] for a Left/Right border.
  const rect = (x: number, y: number, w = 100, h = 100) => ({ x, y, w, h });

  it("aligned side-by-side nodes straighten at the overlap midpoint", () => {
    const r = straightPort(rect(0, 0), rect(200, 0), Position.Right);
    expect(r.straight).toBe(true);
    if (r.straight) expect(r.coord).toBe(50); // both strips [14,86] → mid 50
  });

  it("stacked nodes straighten on the X axis (Top/Bottom pair)", () => {
    const r = straightPort(rect(0, 0), rect(0, 200), Position.Bottom);
    expect(r.straight).toBe(true);
    if (r.straight) expect(r.coord).toBe(50); // shared x
  });

  it("a fan seats parallel ports at stepped spacing, centered, in slot order", () => {
    // count 3, 100-tall source: gap = 100*0.72/4 = 18, groupSpan = 36, overlap 72 >= 36 → straight.
    const a = straightPort(rect(0, 0), rect(200, 0), Position.Right, { index: 0, count: 3 });
    const b = straightPort(rect(0, 0), rect(200, 0), Position.Right, { index: 1, count: 3 });
    const c = straightPort(rect(0, 0), rect(200, 0), Position.Right, { index: 2, count: 3 });
    expect(a.straight && b.straight && c.straight).toBe(true);
    if (a.straight && b.straight && c.straight) {
      expect(a.coord).toBe(32); // 50 - 18
      expect(b.coord).toBe(50); // center
      expect(c.coord).toBe(68); // 50 + 18
    }
  });

  it("grazing overlap below 8px stays stepped", () => {
    // tgt shifted down 65 → strip [79,151]; overlap with [14,86] is [79,86] = 7px < 8.
    expect(straightPort(rect(0, 0), rect(200, 65), Position.Right).straight).toBe(false);
  });

  it("a fan too tall for the overlap stays stepped", () => {
    // count 3 needs groupSpan 36; tgt shifted to y=42 → strip [56,128], overlap [56,86] = 30 < 36.
    expect(straightPort(rect(0, 0), rect(200, 42), Position.Right, { index: 0, count: 3 }).straight).toBe(false);
  });

  it("no strip overlap stays stepped", () => {
    // tgt far below → strip [314,386], no overlap with [14,86].
    expect(straightPort(rect(0, 0), rect(200, 300), Position.Right).straight).toBe(false);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `pnpm --filter @uaml/web test -- floating`
Expected: FAIL — `straightPort is not a function` (not yet exported).

- [ ] **Step 3: Add `BAND` const + `slotStrip` + `straightPort` to `floating.ts`**

Replace the `portPoint` block (lines 86-99) so the `0.72` literal becomes a shared module constant, then add the new helper directly beneath it:

```ts
// The central fraction of each border that ports occupy (rounded corners stay
// clear). Shared by portPoint and the straighten test so both agree on the strip.
const BAND = 0.72;

// A point on `side` of `rect`, offset along that side by the slot. A single edge
// (count 1) sits at the midpoint; N edges divide the central `BAND` fraction of
// the side into N evenly-spaced ports, leaving the rounded corners clear.
export function portPoint(rect: Rect, side: Position, slot: Slot = { index: 0, count: 1 }): { x: number; y: number } {
  const f = slot.count > 1 ? (slot.index + 1) / (slot.count + 1) : 0.5;
  const t = 0.5 + (f - 0.5) * BAND;
  switch (side) {
    case Position.Left: return { x: rect.x, y: rect.y + rect.h * t };
    case Position.Right: return { x: rect.x + rect.w, y: rect.y + rect.h * t };
    case Position.Top: return { x: rect.x + rect.w * t, y: rect.y };
    default: return { x: rect.x + rect.w * t, y: rect.y + rect.h }; // Bottom
  }
}

// ── Straighten within a shared band ──────────────────────────────────────────
// When a single straight line can hit both nodes' facing slot-strips head-on we
// draw straight instead of smooth-step. `straightPort` reports whether that line
// exists for this edge and, if so, the shared coordinate (y for Left/Right pairs,
// x for Top/Bottom pairs), seating a hub's fan as parallel straights by slot.
export type StraightResult = { straight: true; coord: number } | { straight: false };

// The [lo, hi] range along the varying axis (y for a vertical border, x for a
// horizontal one) that portPoint confines this rect's ports to.
function slotStrip(rect: Rect, vertical: boolean): [number, number] {
  const lo = vertical ? rect.y : rect.x;
  const len = vertical ? rect.h : rect.w;
  const half = (len * BAND) / 2;
  const mid = lo + len / 2;
  return [mid - half, mid + half];
}

export function straightPort(srcRect: Rect, tgtRect: Rect, side: Position, slot: Slot = { index: 0, count: 1 }): StraightResult {
  const vertical = side === Position.Left || side === Position.Right;
  const [sLo, sHi] = slotStrip(srcRect, vertical);
  const [tLo, tHi] = slotStrip(tgtRect, vertical);
  const lo = Math.max(sLo, tLo);
  const hi = Math.min(sHi, tHi);
  const overlap = hi - lo;
  if (overlap <= 0) return { straight: false };

  // Stepped-fan gap between adjacent ports, driven off the source strip so the
  // straight fan matches the smooth-step fan's spacing.
  const gap = ((vertical ? srcRect.h : srcRect.w) * BAND) / (slot.count + 1);
  const groupSpan = gap * (slot.count - 1); // 0 for a solo edge
  if (overlap < Math.max(8, groupSpan)) return { straight: false };

  // Seat the N ports at stepped spacing, group centered in the overlap band.
  const mid = (lo + hi) / 2;
  const coord = mid - groupSpan / 2 + slot.index * gap;
  return { straight: true, coord };
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `pnpm --filter @uaml/web test -- floating`
Expected: PASS — all `straightPort` cases plus the pre-existing `portPoint`/`getEdgeParams` cases green (the `BAND` refactor is behavior-preserving).

- [ ] **Step 5: Typecheck**

Run: `pnpm --filter @uaml/web check`
Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add packages/web/src/components/canvas/floating.ts packages/web/src/components/canvas/floating.test.ts
git commit -m "feat(web): add straightPort — straighten test for shared border band

Reports whether a single straight line can hit both nodes' 0.72 slot
strips head-on and the shared coordinate, seating a fan by source slot.
Extracts BAND constant shared with portPoint."
```

---

### Task 2: Wire the straight branch into `RelEdge.svelte`

**Files:**
- Modify: `packages/web/src/components/canvas/RelEdge.svelte:3` (import `getStraightPath`), `:5` (import `straightPort`), `:51-75` (`geometry` + `edgePath` deriveds)

**Interfaces:**
- Consumes: `straightPort(srcRect, tgtRect, side, slot)` from Task 1; existing `rectOf`, `portPoint`, `d.sourceSide`/`d.sourceSlot`/`d.targetSide`/`d.targetSlot`.
- Produces: `geometry` now carries an optional `straight?: boolean`; `edgePath` renders `getStraightPath` when `geometry.straight`, else `getSmoothStepPath` (unchanged).

- [ ] **Step 1: Add imports**

Edit line 3 to add `getStraightPath` and promote `Position` from a type-only import to a **value** import (needed to compare against `Position.Left`/`Position.Right`; `@xyflow/svelte`'s `Position` is a string enum, so a raw `=== "left"` comparison fails svelte-check with a no-overlap error):

```ts
  import { BaseEdge, EdgeLabel, EdgeReconnectAnchor, getSmoothStepPath, getStraightPath, Position, useInternalNode, type EdgeProps } from "@xyflow/svelte";
```

Edit line 5 to add `straightPort`:

```ts
  import { getEdgeParams, portPoint, straightPort, type NodeGeom, type Rect, type Slot } from "./floating";
```

- [ ] **Step 2: Replace the `geometry` derived (lines 51-61) with the straighten-aware version**

```ts
  const geometry = $derived.by(() => {
    if (!sourceNode || !targetNode) return undefined;
    const measured =
      !!sourceNode.measured?.width && !!sourceNode.measured?.height && !!targetNode.measured?.width && !!targetNode.measured?.height;
    if (measured && d?.sourceSide && d?.targetSide) {
      const sRect = rectOf(sourceNode);
      const tRect = rectOf(targetNode);
      const sp = portPoint(sRect, d.sourceSide, d.sourceSlot);
      const tp = portPoint(tRect, d.targetSide, d.targetSlot);
      // When one straight line can hit both facing strips head-on, snap both
      // endpoints to the shared coordinate and draw straight; else keep stepped.
      const st = straightPort(sRect, tRect, d.sourceSide, d.sourceSlot);
      if (st.straight) {
        const vertical = d.sourceSide === Position.Left || d.sourceSide === Position.Right;
        if (vertical) { sp.y = st.coord; tp.y = st.coord; }
        else { sp.x = st.coord; tp.x = st.coord; }
        return { sx: sp.x, sy: sp.y, tx: tp.x, ty: tp.y, sourcePos: d.sourceSide, targetPos: d.targetSide, straight: true };
      }
      return { sx: sp.x, sy: sp.y, tx: tp.x, ty: tp.y, sourcePos: d.sourceSide, targetPos: d.targetSide, straight: false };
    }
    return getEdgeParams(sourceNode, targetNode);
  });
```

Note: the `getEdgeParams` fallback return (last line) has no `straight` field, so `geometry.straight` is `undefined` there → falsy → smooth-step, preserving current behavior for the unmeasured path.

- [ ] **Step 3: Replace the `edgePath` derived (lines 63-75) to branch on `straight`**

```ts
  const edgePath = $derived.by(() => {
    if (!geometry) return undefined;
    if (geometry.straight) {
      const [p] = getStraightPath({
        sourceX: geometry.sx,
        sourceY: geometry.sy,
        targetX: geometry.tx,
        targetY: geometry.ty,
      });
      return p;
    }
    const [p] = getSmoothStepPath({
      sourceX: geometry.sx,
      sourceY: geometry.sy,
      sourcePosition: geometry.sourcePos,
      targetX: geometry.tx,
      targetY: geometry.ty,
      targetPosition: geometry.targetPos,
      borderRadius: 8,
    });
    return p;
  });
```

Note: the `getEdgeParams` fallback return has no `straight` field → `geometry.straight` is `undefined` → falsy → smooth-step, preserving current behavior for the unmeasured path.

- [ ] **Step 4: Typecheck**

Run: `pnpm --filter @uaml/web check`
Expected: no errors. (If the `"left"`/`"right"` literal comparison errors, apply the value-import fix from Step 2's note and re-run.)

- [ ] **Step 5: Unit tests still green**

Run: `pnpm --filter @uaml/web test`
Expected: PASS (no RelEdge-specific unit test; this confirms nothing regressed).

- [ ] **Step 6: Commit**

```bash
git add packages/web/src/components/canvas/RelEdge.svelte
git commit -m "feat(web): straighten RelEdge when facing strips align head-on

geometry snaps both endpoints to straightPort's shared coordinate and
edgePath draws getStraightPath; smooth-step preserved otherwise."
```

---

### Task 3: Align `AnchorEdge.svelte` endpoints head-on

**Files:**
- Modify: `packages/web/src/components/canvas/AnchorEdge.svelte:3-4` (imports), `:19-25` (`path` derived)

**Interfaces:**
- Consumes: `getEdgeParams` (already used — returns `sourcePos`), `straightPort` (Task 1), `Rect`/`NodeGeom` from `floating`, `Position` from `@xyflow/svelte`.
- Produces: no exported surface change; the dashed straight edge now snaps to a head-on shared coordinate when the facing strips overlap.

- [ ] **Step 1: Update imports (lines 3-4)**

`Position` is imported as a **value** (string enum) so the side comparison typechecks — same reason as Task 2:

```ts
  import { BaseEdge, getStraightPath, Position, useInternalNode, type EdgeProps } from "@xyflow/svelte";
  import { getEdgeParams, straightPort, type NodeGeom, type Rect } from "./floating";
```

- [ ] **Step 2: Replace the `path` derived (lines 19-25) to snap endpoints via `straightPort`**

```ts
  const rectOf = (n: NodeGeom): Rect => ({
    x: n.internals.positionAbsolute.x,
    y: n.internals.positionAbsolute.y,
    w: n.measured?.width ?? 0,
    h: n.measured?.height ?? 0,
  });

  // Floating endpoints, straight line. When the facing slot-strips overlap, snap
  // both ends to the shared coordinate so the anchor points at its target head-on
  // (anchor edges carry no slot → treated as a solo edge, count 1).
  const path = $derived.by(() => {
    if (!sourceNode || !targetNode) return undefined;
    const p = getEdgeParams(sourceNode, targetNode);
    let { sx, sy, tx, ty } = p;
    const st = straightPort(rectOf(sourceNode), rectOf(targetNode), p.sourcePos);
    if (st.straight) {
      const vertical = p.sourcePos === Position.Left || p.sourcePos === Position.Right;
      if (vertical) { sy = st.coord; ty = st.coord; }
      else { sx = st.coord; tx = st.coord; }
    }
    const [d] = getStraightPath({ sourceX: sx, sourceY: sy, targetX: tx, targetY: ty });
    return d;
  });
```

- [ ] **Step 3: Typecheck**

Run: `pnpm --filter @uaml/web check`
Expected: no errors.

- [ ] **Step 4: Unit tests still green**

Run: `pnpm --filter @uaml/web test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/canvas/AnchorEdge.svelte
git commit -m "feat(web): snap AnchorEdge endpoints head-on within shared band

Reuses straightPort to align the dashed anchor's endpoints to a shared
coordinate when the facing strips overlap."
```

---

### Task 4: End-to-end visual verification

**Files:** none (verification only).

**Interfaces:** none.

- [ ] **Step 1: Rebuild dists if the branch/deps changed since last dev run**

Only if the app white-pages with "Cannot read properties of undefined (reading 'build_model')" — a stale dist. Rebuild in order:

```bash
pnpm --filter @uaml/wasm build && pnpm --filter @uaml/okf build && pnpm --filter @uaml/core build
```

- [ ] **Step 2: Start the dev server**

Run: `pnpm --filter @uaml/web dev`
Expected: serves on a local port (e.g. http://localhost:5175/); canvas renders.

- [ ] **Step 3: Verify horizontal straighten + snap-back**

In the browser: drag two connected nodes side-by-side so their vertical extents overlap → the connector becomes a **straight horizontal line**. Drag one node up/down until the extents no longer share ≥ the fan height → it **snaps back to stepped**.
Expected: both transitions happen live during the drag (geometry is `$derived`).

- [ ] **Step 4: Verify vertical straighten + hub fan**

Stack two connected nodes with overlapping horizontal extents → **straight vertical line**. On a hub with several edges to aligned targets → the edges render as **parallel straight lines**, evenly spaced, non-crossing, and the dashed **AnchorEdge** (association-class / note) points head-on within the band.
Expected: matches the spec's Testing section.

- [ ] **Step 5: Final gate**

Run: `pnpm --filter @uaml/web test && pnpm --filter @uaml/web check`
Expected: all PASS, no type errors.

---

## Notes for the implementer

- The `getEdgeParams` fallback path (unmeasured nodes) intentionally stays smooth-step; only the measured, side-assigned path can straighten.
- Do **not** reintroduce any `optimizeDeps` change in `vite.config.ts` — a prior wrong guess, already reverted. White-page issues are stale dists (Task 4 Step 1), not vite config.
- `dist/` artifacts are not committed.
