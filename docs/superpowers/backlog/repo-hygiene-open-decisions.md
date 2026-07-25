# Repo hygiene — open decisions (backlog)

Salvaged 2026-07-25 from `.worktrees/icon-remap/issues.md`, the scratch report of an overnight
codebase sweep (2026-07-20, base `main` @ `0cf6b90`). That worktree was merged and is being
removed; these were the only items in it still unresolved. Everything else in the report
(clippy fixes, the "web is broken" stale-dist scare) has since landed or was a false alarm.

GitHub issues are disabled for `redoz/waml`, so this file is the record.

Each item is a **decision for redoz@**, not a queued task — nothing here is a proven bug.
Status re-verified against `main` @ `40ffc75` on 2026-07-25; all five still open.

---

## 1. `.prettierignore` the generated wasm bindings — **M**

`packages/wasm/src/generated/{waml_wasm.d.ts, waml_wasm.js, wasm-inline.ts}` are flagged by
`prettier --check` even though they're machine-generated (`wasm-inline.ts` is 2.5 MB). They're
already ESLint-ignored.

Fix = add the path to `.prettierignore`; do **not** reformat them. Current `.prettierignore`
covers `dist`, so the built copies are excluded but the committed `src/generated` ones are not.

Low-risk, mechanical.

## 2. `.svelte` files linted by neither tool — **L**

61 `.svelte` files under `packages/web/src` are checked by no formatter or linter:
`prettier-plugin-svelte` is not installed, and `eslint.config.mjs` defers `.svelte` to
`svelte-check`. That's a coverage gap, not a bug.

Decision: install the plugin or accept the gap.

## 3. Mass `prettier --write` of the web sources — **L, hold**

92 unformatted source files in the web packages (278 repo-wide). Reformatting produces a huge
noise diff for little value and was deliberately **not** auto-run. Decision: reformat + enforce
in CI, or leave alone.

Depends on 1 and 2 — settle the ignore list and the Svelte plugin first, otherwise the sweep
churns files twice.

## 4. Build-ordering gap makes stale `dist` recur — **M, durable**

`check` does not build workspace deps first, so `@waml/wasm` / `@waml/okf` `dist` can lag `src`
and `tsc` resolves the stale `types` entry — surfacing as a cascade of phantom "no exported
member" errors that look like a source bug. This has already cost one full misdiagnosis
(see the `stale-dist-phantom-typecheck` note); same class as the New-Package-flow dist gotcha.

Fix options:
- Topological build before check — turbo `dependsOn: ["^build"]`, or `pnpm -r --sort build`
  ahead of check. There is currently no root `check` script; the root `build` script hardcodes
  the dep order (`wasm → okf → core → web`), which is the same ordering knowledge, unshared.
- And/or point `@waml/wasm` and `@waml/okf` `types` at `src` rather than `dist`.

Highest-value item here: it removes a recurring false alarm rather than a lint nit.

## 5. Two panic-capable sites on arbitrary input — **L, optional**

The sweep found 68 panic-capable calls in Rust, but ~52 are compile-constant
`Regex::new(...).unwrap()` in `LazyLock`, or CLI. **Do not mass-convert to `Result`** — big
risky refactor with no proven bug behind it. Only two sites take arbitrary input:

- `crates/waml/src/parse.rs:718,728` — `members.get_mut(&dir_of(path)).unwrap()` and
  `get_mut(&parent).unwrap()` in the package-tree build. Assumes every directory was pre-seeded
  by the ancestor walk above. Confirm malformed input can't reach an unseeded dir.
- `crates/waml/src/solve/geometry.rs` — `v.iter().min().unwrap()` on a component vec, panics if
  empty. **The line has moved since the sweep and no `min().unwrap()` matches in that file
  today**; re-locate it (it may have been refactored away) before spending time here.

Also noted, no action: `waml-editor` card/start_screen `.expect("taffy…")` (7) and list-item
unwraps (2) are UI-render invariants; `waml-wasm/src/lib.rs:9,17,21` serde `.unwrap()` sits on
the WASM boundary. Low risk.

---

## Not carried over

- **makepad fork dup-package warnings** (`cfg-if`, `bitflags`) — benign, fix lives in the
  redoz/makepad fork, not this repo. Already tracked separately.
- `plan/sequence-flat-model` — superseded, its flatten commit landed as `760614c`.
