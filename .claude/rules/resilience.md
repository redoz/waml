# Resilience

Evaluate whether the system degrades gracefully and recovers from failure.

waml holds work the user has not saved. One bad document, one bad node, or one
bad frame must not cost the session.

## Checklist

### Blast Radius
- If one document is malformed, does it only affect that document, or does it break the workspace, the tab bar, or the other open documents?
- If one node or edge cannot be laid out, does the rest of the diagram still draw?
- Can one panel's failure blank the whole window? An unbalanced Turtle — begun and never ended — takes out siblings that did nothing wrong.

### Failure Instead of Panic
- Is a panic used where a `Result` and a diagnostic belong? A panic kills the native process and poisons the wasm instance for the rest of the session.
- Is `unwrap`/`expect` reachable from document content, file content, or user gesture order?
- Does the code assume a lookup succeeds because it usually does?

### State Recovery
- After a failed load or a failed parse, does the editor return to a usable state, or does it stay wedged?
- What is lost when a document fails to open? Is that acceptable, and is it said?
- Does a partial or stale result get presented as if it were complete?

### Resource Bounds
- If the editor runs for hours, does anything grow without bound? Caches, per-frame allocations, retained document history, accumulated overlays.
- Does a cache have an eviction rule, or does it only ever grow?

### Degraded Environments
- Does the web build survive a slow network, a missing font, or a shader that fails to compile — or does it show a blank canvas with no explanation?
- Does the editor behave when the window is tiny, the display is HiDPI, or a resize arrives mid-frame?
- If an optional feature is unavailable on a platform, does the rest still work?

### Self-Correction
- If a cached rect, a hover state, or a drag arm goes stale, does the next event correct it, or does it stay wrong until restart?
- Is there a state the UI can enter that no gesture can leave?

## Scope Guidance

- **Full evaluation**: Review failure isolation between documents and panels, panic reachability, cache growth, and the web degraded paths.
- **Change review**: Focus on whether the change adds a panic path, a new failure mode, an unbounded resource, or a state with no exit.
