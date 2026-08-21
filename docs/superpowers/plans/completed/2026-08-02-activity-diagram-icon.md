# Activity Diagram Icon Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the heartbeat activity glyph with a compact Lucide-style UML activity-flow glyph.

**Architecture:** Keep the existing `Icon::Activity` catalog identity and replace only its SVG source and generated Makepad SDF body. Use the single-icon generator so no catalog ordering or unrelated glyph changes occur.

**Tech Stack:** SVG 1.1, Python 3 icon generator, Makepad `Sdf2d`, Rust, Cargo.

## Global Constraints

- Use Lucide's 24 by 24 view box, two-unit stroke, round caps, and round joins.
- Keep the outer geometry inside the icon view box.
- Use only a start circle, rounded action node, decision diamond, end ring, and centered transition line.
- Do not change the public `Icon::Activity` identity or catalog order.
- Do not run `scripts/gen-all-icons.py`; use `scripts/gen-icon.py` for this glyph only.
- Preserve all unrelated workspace changes.

---

### Task 1: Replace and Validate the Activity Glyph

**Files:**
- Modify: `crates/waml-editor/resources/icons/activity.svg`
- Modify: `crates/waml-editor/src/icons.rs:3070`
- Verify: `crates/waml-editor/src/icons.rs:4575`

**Interfaces:**
- Consumes: `python scripts/gen-icon.py <path-to.svg>`, which prints a Makepad `Sdf2d` shader body.
- Produces: the unchanged `Icon::Activity` catalog entry backed by the new `mod.draw.IconActivity` glyph.

- [ ] **Step 1: Run the focused catalog tests as a baseline**

Run:

```powershell
rtk cargo test -p waml-editor icons::tests --lib
```

Expected: PASS before the visual change.

- [ ] **Step 2: Replace the SVG source**

Use this complete SVG:

```svg
<svg
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <circle cx="12" cy="3" r="1" />
  <path d="M12 4v2.5" />
  <rect x="8" y="6.5" width="8" height="4" rx="1.5" />
  <path d="M12 10.5v2M12 12.5l3.5 2.75L12 18l-3.5-2.75L12 12.5ZM12 18v1.5" />
  <circle cx="12" cy="21" r="1.5" />
</svg>
```

The one-unit start circle renders as a solid-looking UML initial node at small sizes because its two-unit outline closes the center. The larger end circle remains visually distinct as a ring.

- [ ] **Step 3: Generate the Makepad SDF body**

Run:

```powershell
rtk python scripts/gen-icon.py crates/waml-editor/resources/icons/activity.svg
```

Expected: output starts with `let w = s * 0.068`, emits the five SVG elements in document order, and ends with `return sdf.result`.

- [ ] **Step 4: Replace only the `IconActivity` shader body**

Keep this wrapper and replace the statements between `let s` and the closing braces with the exact generator output:

```rust
// Activity: a compact UML start-action-decision-end flow.
// Faithful port of resources/icons/activity.svg via scripts/gen-icon.py.
mod.draw.IconActivity = mod.draw.DrawColor{
    pixel: fn() {
        let s = self.rect_size.x
        let w = s * 0.068
        let sdf = Sdf2d.viewport(self.pos * self.rect_size)
        sdf.move_to(s * 0.5417, s * 0.1250)
        sdf.arc_to(s * 0.5000, s * 0.1250, s * 0.0417, 0.0000, 3.1416)
        sdf.arc_to(s * 0.5000, s * 0.1250, s * 0.0417, 3.1416, 6.2832)
        sdf.close_path()
        sdf.stroke(self.color, w)
        sdf.move_to(s * 0.5000, s * 0.1667)
        sdf.line_to(s * 0.5000, s * 0.2708)
        sdf.stroke(self.color, w)
        sdf.move_to(s * 0.3958, s * 0.2708)
        sdf.line_to(s * 0.6042, s * 0.2708)
        sdf.arc_to(s * 0.6042, s * 0.3333, s * 0.0625, -1.5708, 0.0000)
        sdf.line_to(s * 0.6667, s * 0.3750)
        sdf.arc_to(s * 0.6042, s * 0.3750, s * 0.0625, 0.0000, 1.5708)
        sdf.line_to(s * 0.3958, s * 0.4375)
        sdf.arc_to(s * 0.3958, s * 0.3750, s * 0.0625, 1.5708, 3.1416)
        sdf.line_to(s * 0.3333, s * 0.3333)
        sdf.arc_to(s * 0.3958, s * 0.3333, s * 0.0625, 3.1416, 4.7124)
        sdf.close_path()
        sdf.stroke(self.color, w)
        sdf.move_to(s * 0.5000, s * 0.4375)
        sdf.line_to(s * 0.5000, s * 0.5208)
        sdf.move_to(s * 0.5000, s * 0.5208)
        sdf.line_to(s * 0.6458, s * 0.6354)
        sdf.line_to(s * 0.5000, s * 0.7500)
        sdf.line_to(s * 0.3542, s * 0.6354)
        sdf.line_to(s * 0.5000, s * 0.5208)
        sdf.close_path()
        sdf.move_to(s * 0.5000, s * 0.7500)
        sdf.line_to(s * 0.5000, s * 0.8125)
        sdf.stroke(self.color, w)
        sdf.move_to(s * 0.5625, s * 0.8750)
        sdf.arc_to(s * 0.5000, s * 0.8750, s * 0.0625, 0.0000, 3.1416)
        sdf.arc_to(s * 0.5000, s * 0.8750, s * 0.0625, 3.1416, 6.2832)
        sdf.close_path()
        sdf.stroke(self.color, w)
        return sdf.result
    }
}
```

Do not change `IconSet`, `Icon::ALL`, `Icon::label`, or their ordering.

- [ ] **Step 5: Run formatting and focused tests**

Run:

```powershell
rtk cargo fmt --check
rtk cargo test -p waml-editor icons::tests --lib
```

Expected: both commands PASS.

- [ ] **Step 6: Inspect the generated icon in the harness**

Run the existing icon harness, capture it at native pixels, and inspect the `activity` tile:

```powershell
rtk cargo run -p waml-editor --bin icon_harness
pwsh -File scripts/capture-window.ps1 -Out activity-icon-harness.png -Process icon_harness
```

Expected: the vertical flow is centered, unclipped, and visually consistent with adjacent Lucide glyphs. The start node looks solid, the end node looks like a ring, and the action and decision shapes stay distinct.

- [ ] **Step 7: Review the final diff**

Run:

```powershell
rtk git diff --check
rtk git diff -- crates/waml-editor/resources/icons/activity.svg crates/waml-editor/src/icons.rs
```

Expected: only the activity SVG, the `IconActivity` shader body, and its description change for the implementation.

- [ ] **Step 8: Commit the implementation**

```powershell
git add crates/waml-editor/resources/icons/activity.svg crates/waml-editor/src/icons.rs
git commit -m "feat(icons): clarify activity diagram glyph"
```
