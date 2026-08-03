# Maintainability

Evaluate whether the code is easy to understand, modify, and extend.

## Checklist

### Abstractions & Boundaries
- Are abstractions intact? The headless crates (`waml`, `waml-syntax`) must not depend on the editor, on makepad, or on a window.
- Are boundaries respected? No layer reaching into another's internals, no circular dependencies between crates.
- Is there one implementation of a rule, shared by every frontend, rather than a native copy and a web copy that will drift?
- Are there leaky abstractions where rendering details bleed into the model, or model details into a widget?

### Placement & Structure
- Is logic placed where it naturally belongs, or where it was convenient to reach?
- Are there modules or types that duplicate the same concept under different names?
- Are types defined in the crate that owns them?
- Does a new widget belong in the module it was added to?

### Design Quality
- Are enums and message types well designed? Right granularity, no variant that means "several unrelated things"?
- Is naming clear and consistent with the surrounding code?
- Is there dead code, commented-out code, or an obsolete module left behind?
- Does the change match the density and idiom of the code around it?

### Change Resistance
- Does the code resist change? Hardcoded assumptions, magic constants repeated in several places, a shape that must be edited in three files to add one case.
- Could a new requirement be added without a rewrite, or would it ripple across layers?
- Are there parts of the code everyone avoids touching? Why?

## Codebase Patterns

- **`script_mod!` registration**: a widget is invisible and dead unless its module is registered, and a child widget must register **before** its consumer. There is no glob import in `app.rs` — an unlisted widget is silently dropped with a green gate.
- **Immediate-mode drawing**: draw and hit-test are separate passes over the same condition. Both must gate on the same verdict, or a widget stays clickable after it stops being drawn.
- **Turtle layout**: layout is makepad's Turtle, not a retained tree. `Size::Fill` is resolved late, so a sibling that trails a `Fill` sibling can cache a pre-shift rect.
- **Headless core, thin frontends**: solving, parsing, and model rules live in the headless crates; the editor and the wasm build present them.

## Scope Guidance

- **Full evaluation**: Review the crate graph, module structure, and the boundary between headless logic and presentation.
- **Change review**: Focus on whether the change respects existing boundaries, adds coupling, duplicates an existing concept, or places code in the wrong crate.
