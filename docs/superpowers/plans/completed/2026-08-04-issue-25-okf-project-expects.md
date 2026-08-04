# Issue 25 — Content-reachable expects in okf::project; dead project_document

## Context

`crates/waml/src/okf.rs` carries two projection helpers with panicking `expect`s:

- `project_document` (okf.rs:392-412) — **dead**: no callers anywhere in the
  workspace (the only grep hits besides its definition are the unrelated
  `project_document_header` in `crates/waml-editor/src/app/shell.rs`). Carries
  two `expect`s (okf.rs:406, 408).
- `project(path, src) -> Concept` (okf.rs:414-430) — three `expect`s
  (okf.rs:421, 423, 427). The last, `.expect("non-reserved projection produces
  one concept")`, is false under the quarantine design:
  - `Bundle::parse` (okf.rs:270) delegates to `analyze_okf`, and
    `analyze_okf_inner` **quarantines** shell-failed documents instead of
    erroring (crates/waml/src/analysis.rs:1194, 1211, 1296). A document that
    fails the shell (e.g. `SourceTooLarge`, syntax-authority failure) yields
    `Ok(bundle)` with **zero concepts**, and the expect panics.
  - A reserved filename (`index.md` / `log.md`) is routed away from the
    concepts vec (crates/waml/src/okf/shell.rs:216-223), so
    `project("index.md", ...)` also yields zero concepts and panics.

### Verdict evidence (triage)

- Reachability today is **test-only**: every real caller of `okf::project` is a
  test or `#[cfg(test)]` support fn —
  - crates/waml/src/uml.rs:58 (inside `#[cfg(test)] mod tests`)
  - crates/waml/src/model.rs:1344 (test `model_looks_up_nodes_by_key`)
  - crates/waml/tests/serde_shape.rs:174, 199
  - crates/waml/tests/parser_platform_properties.rs:549, 643, 652
  - okf.rs's own `#[cfg(test)]` tests (:638, 664, 718, 727, 733, 744, 780)
  This matches docs/reviews/2026-08-04/resilience.md:134. So "poisons wasm" is
  latent, not currently live — but the API is `pub` in a headless crate and one
  new production caller re-arms the panic.
- Recent commits touching okf.rs (1a7ff781, adf8cdc2, 76de276e, 5dada457) did
  not change either function; the issue is current.

## Design decisions

1. **Delete `project_document`.** Zero callers, two expects, and its
   reserved-filename check duplicates shell.rs logic. No deprecation dance for
   dead private-in-practice code.
2. **`project` returns `Option<Concept>`.** `None` when the projection yields
   zero concepts (reserved filename, quarantined/shell-failed source). Option
   over Result: callers are tests that will `.unwrap()` at the call site, and
   the interesting distinction (reserved vs quarantined) is not needed by any
   caller; a Result with a bespoke error enum would be speculative API.
3. **Keep the two path-construction `expect`s inside `project` only if they are
   truly caller-error** — `try_from_pairs` failure (bad bundle-relative path)
   and `DuplicateConceptId` (impossible for a single document) become part of
   the `None` path instead: fold them via `.ok()?` so no `expect` remains in
   the function. Zero panics reachable through `project`.

### Delete project_document

- File: crates/waml/src/okf.rs — remove `pub fn project_document`
  (currently :392-412) and, if `SourceDocument` import becomes unused in
  okf.rs, drop the import.
- Gate: `cargo test --workspace` (also proves no hidden caller — the gate runs
  clippy `-D warnings`, which would flag a now-unused import).

### Make project return Option<Concept>

- File: crates/waml/src/okf.rs — change signature to
  `pub fn project(path: &str, src: &str) -> Option<Concept>`; replace all three
  `expect`s with `?`/`.ok()?`; on success set `concept.id = id_of(path)` and
  return `Some(concept)`.
- Update callers to unwrap at the call site (all tests/support):
  - crates/waml/src/uml.rs:58 — `.unwrap()` in the `concept` helper.
  - crates/waml/src/model.rs:1344 — `.unwrap()`.
  - crates/waml/tests/serde_shape.rs:174, 199 — `.unwrap()`.
  - crates/waml/tests/parser_platform_properties.rs:549, 643, 652 — `.unwrap()`.
  - crates/waml/src/okf.rs test module (:638, 664, 718, 727, 733, 744, 780) —
    `.unwrap()`.
  - Update the doc comment at crates/waml/src/model.rs:975 if its wording
    ("Populated from `crate::okf::project`") needs the Option noted.
- New tests in okf.rs tests module:
  - `project_returns_none_for_reserved_filename` —
    `project("index.md", "# X\n").is_none()` and same for `log.md`.
  - `project_returns_none_for_quarantined_source` — a source the shell rejects
    (e.g. oversized document mirroring
    analysis.rs `shell_failed_document_is_quarantined_not_fatal` at :1637)
    returns `None` instead of panicking.
- Gate: `cargo test --workspace` plus the vscode extension test/lint/build.
