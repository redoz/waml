# Repo hygiene — open decisions (backlog)

Salvaged 2026-07-25 from an overnight codebase sweep. Retired web, generated
binding, and workspace build-order findings were resolved when those product
trees were removed. GitHub issues are disabled for `redoz/waml`, so this file
keeps the remaining decision.

## Panic-capable sites on arbitrary input — low priority

Most panic-capable calls are compile-constant regex construction, CLI
invariants, or native UI rendering invariants. Do not mass-convert them to
`Result`.

Two sites originally accepted arbitrary input:

- `crates/waml/src/parse.rs` used `get_mut(...).unwrap()` while constructing
  directory membership. Confirm malformed bundle input cannot reach an
  unseeded directory.
- `crates/waml/src/solve/geometry.rs` previously used
  `v.iter().min().unwrap()` on a component vector. Re-locate it before spending
  time; it may already have been removed.

No action without a reproducible failure.

## Not carried over

- Makepad fork duplicate-package warnings are tracked in the Makepad fork.
- `plan/sequence-flat-model` was superseded by commit `760614c`.
