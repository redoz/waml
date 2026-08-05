# Correctness

Evaluate whether the code does what it is supposed to do and handles failure.

## Checklist

### Logic
- Are there logic bugs? (wrong conditions, off-by-one, missing cases)
- Are `match` arms exhaustive on real domain enums, or does a catch-all `_ =>` hide a new variant?
- Are `Option`/`Result` unwrapped only where the invariant is proven? Is a `.unwrap()` on a path that untrusted input can reach?
- Does incremental work agree with a full recompute? (waml-syntax reparse vs full parse, layout resolve vs re-solve)

### Widget & Event Logic
- Does hit-testing gate on the same verdict as drawing? A widget drawn under a condition must not stay clickable when that condition is false.
- Are draw-time rects and event-time positions in the same coordinate space? (aligned or `Size::Fill` parents shift children after the child cached its rect)
- Is widget state consistent across every handler that mutates it, including the early-return paths?
- Is every `hover_in` matched by a `hover_out`, and every armed drag cleared on `FingerUp`?

### Error Handling
- Is error handling consistent? Are `Result`, `panic!`, and `expect` used where each belongs?
- Does a recoverable parse or load failure produce a diagnostic instead of a panic?
- Are there unhandled edge cases at system boundaries? (malformed `.waml` source, absent file, truncated bundle, hostile frontmatter)

### Platform Boundaries
- Does the change hold on **both** native and wasm? `SystemTime::now()` panics on wasm; file and clock APIs differ.
- Are `cfg` branches for wasm and native kept behaviourally equivalent, or does one silently do less?
- Is input validated at the DSL, LSP, and bundle boundaries before it reaches the model?

## Scope Guidance

- **Full evaluation**: Review the model and analysis paths, all parse and reparse logic, every error path, and every native/wasm split.
- **Change review**: Focus on new branches, new match arms (exhaustive?), new widget state transitions, and whether error cases are handled rather than unwrapped.
