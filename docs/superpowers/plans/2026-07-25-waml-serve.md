# `waml serve` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `waml serve` subcommand that serves the embedded web editor over loopback HTTP and exposes an ops API over a local directory, filling the native `save_backend` stub.

**Architecture:** A new `serve` module inside `waml-cli`, laid out like the existing `lsp` module. An axum server owns a `ServeState` (bundle in memory + monotonic revision + served root). Writes arrive as `OpDto[]`, go through `waml::ops::apply`, and are written back with the existing `io::write_back`. Reads project the same state three ways: raw bundle, `Model`, `Diagnostic[]`. The web artifact is brotli-compressed at build time by a `build.rs` and served from a static byte table.

**Tech Stack:** Rust, clap 4, tokio 1 (already a `waml-cli` dep), axum 0.8, brotli 8, rand 0.9, subtle 2.6; dev-only reqwest 0.13 + tempfile 3. Browser verification via playwright-core against ms-playwright `chromium-1228`.

**Source spec:** `docs/superpowers/specs/2026-07-25-waml-serve-design.md`

## Global Constraints

- **Rust edition/rust-version come from the workspace.** Never write `edition = "..."` into a crate manifest; use `edition.workspace = true` like every existing crate.
- **All new dependencies go in the root `Cargo.toml` `[workspace.dependencies]`** and are referenced as `{ workspace = true }` from crate manifests. That is the established pattern (`clap`, `serde`, `tokio`, `tower-lsp`).
- **Every dependency named in this plan is already in the local cargo registry cache** at the exact version pinned here. Do not bump versions — an unpinned bump may require network access the gate does not have.
- **`cargo test --workspace` must pass without a wasm toolchain and without a built web artifact.** Nothing in this plan may make the default build depend on `target/makepad-wasm-app/`.
- **Clippy runs with `-D warnings`.** `dead_code` is therefore a hard error, not a lint. Do not land a helper before the task that calls it; if a task must define an unused item, it also defines the test that uses it.
- **Token/auth code:** compare secrets with `subtle::ConstantTimeEq`, never `==`.
- **Default bind is `127.0.0.1`.** Loopback is never treated as authentication.
- **Port default is `8099`.**
- **Wire version fields:** every `OpDto` variant carries `v: u32` defaulting to `1` via `#[serde(default = "one")]` and is validated by `check_v`. New variants follow that exactly.
- **Commit messages** use the repo's conventional-commit style (`feat(serve): …`, `fix(cli): …`) and end with:
  ```
  Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
  ```

## Deviations from the spec, decided here

The spec left three things unresolved that implementation cannot avoid. They are recorded here rather than silently chosen inside a task.

1. **HTTP framework: axum 0.8.** `waml-cli` already pulls `tokio` and `tower` (via `tower-lsp`), so axum adds a router and extractors rather than a runtime. Rejected: hand-rolling on `hyper` (more code, same dependency tree).
2. **Artifact embedding is build-time and optional.** `build.rs` reads the artifact directory from `WAML_WEB_ARTIFACT`, falling back to `target/makepad-wasm-app/release/waml-editor`. If that directory does not exist, it emits an **empty** asset table and `serve` behaves as if `--api-only` were passed, printing a one-line notice. This is what keeps the Global Constraint about `cargo test --workspace` true. A release build sets `WAML_WEB_ARTIFACT` explicitly.
3. **`Op::PlaceSet` / `Op::PlaceRm` have no DTO.** `waml-ops-dto/src/lib.rs:765` currently panics (`unreachable!("place.set no web DTO yet (native-only)")`) for exactly the two ops the editor emits. The ops wire cannot carry an editor edit until that is fixed, so Task 2 fixes it before anything depends on it.

## File Structure

**New — `crates/waml-cli/src/serve/`** (mirrors the existing `src/lsp/` layout):

| File | Responsibility |
| --- | --- |
| `mod.rs` | `pub fn run(args: ServeArgs) -> i32`; builds state, router, binds, prints the URL, runs tokio |
| `paths.rs` | Bundle-path confinement. Pure functions, no I/O |
| `guard.rs` | Token generation/compare, Origin/Host/custom-header checks. Pure functions |
| `state.rs` | `ServeState`: bundle, revision, root dir; load, read projections, apply ops, write back |
| `routes.rs` | axum router, extractors, status-code mapping |
| `embed.rs` | Access to the build-time asset table; brotli bytes + content types |

**New — elsewhere:**

| File | Responsibility |
| --- | --- |
| `crates/waml-cli/build.rs` | Walk the artifact dir, brotli-compress, emit `assets.rs` into `OUT_DIR` |
| `crates/waml-cli/tests/serve_e2e.rs` | Integration tests against a real ephemeral-port server |
| `scripts/serve-browser-check.mjs` | Playwright verification of the served UI |

**Modified:**

| File | Change |
| --- | --- |
| `Cargo.toml` (root) | Add axum, brotli, rand, subtle, reqwest, tempfile to `[workspace.dependencies]` |
| `crates/waml-cli/Cargo.toml` | New deps; enable `waml`'s `serde` feature; `[build-dependencies]`; `[dev-dependencies]` |
| `crates/waml-cli/src/main.rs` | `mod serve;`, `Command::Serve` variant, dispatch arm, parse test |
| `crates/waml-ops-dto/src/lib.rs` | `place.set` / `place.rm` DTO variants, both directions |
| `crates/waml-editor/src/app.rs` | Op log at the two `waml::ops::apply` sites; `Backend::Http` save arm |
| `crates/waml-editor/Cargo.toml` | Depend on `waml-ops-dto` |

---

### Task 1: Dependencies and the `serve` command skeleton

Adds the subcommand and its arguments with no server behind it yet. Ends with `waml serve --help` working and the argument surface locked, so later tasks never renegotiate flag names.

**Files:**
- Modify: `Cargo.toml` (workspace deps)
- Modify: `crates/waml-cli/Cargo.toml`
- Modify: `crates/waml-cli/src/main.rs` (the `Command` enum near line 20, the dispatch `match` near line 297, the parse tests near line 812)
- Create: `crates/waml-cli/src/serve/mod.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub struct ServeArgs { pub dir: PathBuf, pub port: u16, pub bind_all: bool, pub api_only: bool, pub no_open: bool }` in `serve::mod`
  - `pub fn run(args: ServeArgs) -> i32` in `serve::mod`

- [ ] **Step 1: Add the workspace dependencies**

In the root `Cargo.toml`, inside `[workspace.dependencies]`, append:

```toml
axum = { version = "0.8", default-features = false, features = ["http1", "json", "tokio", "query"] }
brotli = "8"
rand = "0.9"
subtle = "2.6"
reqwest = { version = "0.13", default-features = false, features = ["json"] }
tempfile = "3"
```

- [ ] **Step 2: Wire them into `waml-cli`**

In `crates/waml-cli/Cargo.toml`, change the `waml` dependency line to enable model serialization, and add the rest:

```toml
[dependencies]
waml = { path = "../waml", features = ["serde"] }
waml-ops-dto = { path = "../waml-ops-dto" }
clap = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
tower-lsp = { workspace = true }
axum = { workspace = true }
rand = { workspace = true }
subtle = { workspace = true }

[build-dependencies]
brotli = { workspace = true }

[dev-dependencies]
reqwest = { workspace = true }
tempfile = { workspace = true }
```

`tokio`'s workspace feature list is `["rt-multi-thread", "io-std", "macros"]`, which is sufficient for axum's `http1` server — do not add `full`.

- [ ] **Step 3: Write the failing parse test**

In `crates/waml-cli/src/main.rs`, in the existing `#[cfg(test)] mod tests` block (the one containing `parses_lsp_stdio` near line 828), add:

```rust
#[test]
fn parses_serve_defaults() {
    let cli = Cli::parse_from(["waml", "serve"]);
    match cli.command {
        Command::Serve {
            ref dir,
            port,
            bind_all,
            api_only,
            no_open,
        } => {
            assert_eq!(dir, std::path::Path::new("."));
            assert_eq!(port, 8099);
            assert!(!bind_all);
            assert!(!api_only);
            assert!(!no_open);
        }
        _ => panic!("expected Serve"),
    }
}

#[test]
fn parses_serve_flags() {
    let cli = Cli::parse_from([
        "waml", "serve", "docs", "--port", "0", "--bind-all", "--api-only", "--no-open",
    ]);
    match cli.command {
        Command::Serve {
            ref dir,
            port,
            bind_all,
            api_only,
            no_open,
        } => {
            assert_eq!(dir, std::path::Path::new("docs"));
            assert_eq!(port, 0);
            assert!(bind_all && api_only && no_open);
        }
        _ => panic!("expected Serve"),
    }
}
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `cargo test -p waml-cli parses_serve -- --nocapture`
Expected: FAIL — `no variant named 'Serve' found for enum 'Command'`.

- [ ] **Step 5: Add the `Serve` variant**

In `crates/waml-cli/src/main.rs`, inside `enum Command`, immediately after the `Lsp { .. }` variant:

```rust
    /// Serve the web editor and an ops API over a local directory.
    ///
    /// Binds loopback by default and mints a fresh access token per run; the
    /// token is printed once, in the URL, and is required on every request.
    /// Loopback is not access control — see the token flag docs.
    Serve {
        /// Directory to serve and edit.
        #[arg(default_value = ".")]
        dir: PathBuf,
        /// Port to bind. `0` picks an ephemeral port and prints it.
        #[arg(long, default_value_t = 8099)]
        port: u16,
        /// Bind 0.0.0.0 instead of 127.0.0.1. Exposes the API to your network.
        #[arg(long)]
        bind_all: bool,
        /// Serve only the API, without the embedded web editor.
        #[arg(long)]
        api_only: bool,
        /// Do not open a browser on start.
        #[arg(long)]
        no_open: bool,
    },
```

- [ ] **Step 6: Create the module with a stub `run`**

Create `crates/waml-cli/src/serve/mod.rs`:

```rust
//! `waml serve`: the web editor plus an ops API over one local directory.
//!
//! Laid out like `crate::lsp`: this module owns transport and process
//! lifetime, and delegates every semantic decision to `waml` proper.

use std::path::PathBuf;

/// Everything `run` needs, decoupled from clap so tests can build one.
#[derive(Debug, Clone)]
pub struct ServeArgs {
    pub dir: PathBuf,
    pub port: u16,
    pub bind_all: bool,
    pub api_only: bool,
    pub no_open: bool,
}

/// Process exit code, matching the rest of the CLI: 0 ok, 2 I/O failure.
pub fn run(args: ServeArgs) -> i32 {
    eprintln!(
        "waml serve: not implemented yet (dir {}, port {})",
        args.dir.display(),
        args.port
    );
    2
}
```

- [ ] **Step 7: Declare the module and dispatch to it**

In `crates/waml-cli/src/main.rs`, next to the existing `mod lsp;`, add `mod serve;`. In the dispatch `match`, next to the `Command::Lsp` arm (near line 376):

```rust
        Command::Serve {
            dir,
            port,
            bind_all,
            api_only,
            no_open,
        } => serve::run(serve::ServeArgs {
            dir,
            port,
            bind_all,
            api_only,
            no_open,
        }),
```

- [ ] **Step 8: Run the tests to verify they pass**

Run: `cargo test -p waml-cli parses_serve`
Expected: PASS, 2 tests.

- [ ] **Step 9: Verify the help text and the clean gate**

Run: `cargo run -p waml-cli -- serve --help`
Expected: usage lists `[DIR]`, `--port`, `--bind-all`, `--api-only`, `--no-open`.

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 10: Commit**

```bash
git add Cargo.toml Cargo.lock crates/waml-cli/Cargo.toml crates/waml-cli/src/main.rs crates/waml-cli/src/serve/mod.rs
git commit -m "feat(serve): add the waml serve command surface

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: `place.set` / `place.rm` wire DTOs

`OpDto::from_op` panics on the only two ops the editor emits, so the ops API cannot carry an editor edit until this exists. Independent of the server; do it first so nothing later is blocked.

**Files:**
- Modify: `crates/waml-ops-dto/src/lib.rs` (the `OpDto` enum, `to_op` at line 338, `from_op` at line 571 including the `unreachable!` arms at 765–770)

**Interfaces:**
- Consumes: `waml::ops::Op::PlaceSet { diagram, subject_title, subject_slug, reference_title, reference_slug, directions }`, `waml::ops::Op::PlaceRm { diagram, subject_slug, reference_slug }`, `waml::syntax::Direction` (**note: `syntax`, not `model`** — variants are `LeftOf`, `RightOf`, `Above`, `Below`, `AboveLeft`, `AboveRight`, `BelowLeft`, `BelowRight`, declared at `crates/waml/src/syntax.rs:187`).
- Produces: `OpDto::PlaceSet { v, diagram, subject_title, subject_slug, reference_title, reference_slug, directions: Vec<String> }` and `OpDto::PlaceRm { v, diagram, subject_slug, reference_slug }`, tagged `place.set` / `place.rm`. `Direction` crosses the wire as a camelCase name (`"leftOf"`, `"aboveRight"`, …).

`Direction` does derive serde under `waml`'s `serde` feature, but `waml-ops-dto` does not enable that feature and this task does not add it: an explicit string mapping keeps the wire spelling a decision of the DTO crate rather than a side effect of a feature flag somewhere else.

- [ ] **Step 1: Write the failing round-trip test**

At the end of `crates/waml-ops-dto/src/lib.rs`, in the existing `#[cfg(test)] mod tests` block (create one if absent, `use super::*;`):

```rust
#[test]
fn place_set_round_trips_through_the_dto() {
    use waml::syntax::Direction;
    let op = Op::PlaceSet {
        diagram: "d/index.md".into(),
        subject_title: "Order".into(),
        subject_slug: "order".into(),
        reference_title: "Customer".into(),
        reference_slug: "customer".into(),
        directions: vec![Direction::LeftOf, Direction::AboveRight],
    };
    let dto = OpDto::from_op(&op);
    let json = serde_json::to_string(&dto).unwrap();
    assert!(json.contains(r#""op":"place.set""#), "{json}");
    assert!(json.contains(r#""aboveRight""#), "{json}");
    let back: OpDto = serde_json::from_str(&json).unwrap();
    assert_eq!(format!("{:?}", back.to_op().unwrap()), format!("{op:?}"));
}

#[test]
fn place_rm_round_trips_through_the_dto() {
    let op = Op::PlaceRm {
        diagram: "d/index.md".into(),
        subject_slug: "order".into(),
        reference_slug: "customer".into(),
    };
    let dto = OpDto::from_op(&op);
    let back: OpDto = serde_json::from_str(&serde_json::to_string(&dto).unwrap()).unwrap();
    assert_eq!(format!("{:?}", back.to_op().unwrap()), format!("{op:?}"));
}

#[test]
fn an_unknown_direction_is_an_error_not_a_panic() {
    let json = r#"{"op":"place.rm","diagram":"d/index.md","subjectSlug":"a","referenceSlug":"b"}"#;
    let dto: OpDto = serde_json::from_str(json).unwrap();
    assert!(dto.to_op().is_ok());
    let bad = r#"{"op":"place.set","diagram":"d/index.md","subjectTitle":"A","subjectSlug":"a",
                  "referenceTitle":"B","referenceSlug":"b","directions":["sideways"]}"#;
    let dto: OpDto = serde_json::from_str(bad).unwrap();
    assert!(dto.to_op().unwrap_err().contains("sideways"));
}
```

If `serde_json` is not yet a dev-dependency of `waml-ops-dto`, add `serde_json = { workspace = true }` under `[dev-dependencies]` in `crates/waml-ops-dto/Cargo.toml`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-ops-dto place_`
Expected: FAIL — the current `from_op` hits `unreachable!("place.set no web DTO yet (native-only)")`.

- [ ] **Step 3: Add the DTO variants**

In the `OpDto` enum, alongside the other variants (note `#[serde(rename_all = "camelCase")]` matches how the neighbouring variants spell multi-word fields — copy whatever the adjacent variants do rather than inventing a convention):

```rust
    #[serde(rename = "place.set", rename_all = "camelCase")]
    PlaceSet {
        #[serde(default = "one")]
        v: u32,
        diagram: String,
        subject_title: String,
        subject_slug: String,
        reference_title: String,
        reference_slug: String,
        /// Direction names as `Direction::wire_name` spells them.
        directions: Vec<String>,
    },
    #[serde(rename = "place.rm", rename_all = "camelCase")]
    PlaceRm {
        #[serde(default = "one")]
        v: u32,
        diagram: String,
        subject_slug: String,
        reference_slug: String,
    },
```

- [ ] **Step 4: Add direction name mapping**

Near the other small helpers (`mult_opt`, `vis_opt`) in the same file:

```rust
fn dir_name(d: waml::syntax::Direction) -> &'static str {
    use waml::syntax::Direction as D;
    match d {
        D::LeftOf => "leftOf",
        D::RightOf => "rightOf",
        D::Above => "above",
        D::Below => "below",
        D::AboveLeft => "aboveLeft",
        D::AboveRight => "aboveRight",
        D::BelowLeft => "belowLeft",
        D::BelowRight => "belowRight",
    }
}

fn dir_from_name(s: &str) -> Result<waml::syntax::Direction, String> {
    use waml::syntax::Direction as D;
    Ok(match s {
        "leftOf" => D::LeftOf,
        "rightOf" => D::RightOf,
        "above" => D::Above,
        "below" => D::Below,
        "aboveLeft" => D::AboveLeft,
        "aboveRight" => D::AboveRight,
        "belowLeft" => D::BelowLeft,
        "belowRight" => D::BelowRight,
        other => return Err(format!("unknown direction `{other}`")),
    })
}
```

If `waml::syntax::Direction` has gained variants beyond these eight since this plan was written, cover every one — a non-exhaustive `match` will not compile, which is the intended safety net.

- [ ] **Step 5: Implement both directions**

In `to_op`, add arms:

```rust
            OpDto::PlaceSet {
                v,
                diagram,
                subject_title,
                subject_slug,
                reference_title,
                reference_slug,
                directions,
            } => {
                check_v(*v, "place.set")?;
                Ok(Op::PlaceSet {
                    diagram: diagram.clone(),
                    subject_title: subject_title.clone(),
                    subject_slug: subject_slug.clone(),
                    reference_title: reference_title.clone(),
                    reference_slug: reference_slug.clone(),
                    directions: directions
                        .iter()
                        .map(|d| dir_from_name(d))
                        .collect::<Result<Vec<_>, _>>()?,
                })
            }
            OpDto::PlaceRm {
                v,
                diagram,
                subject_slug,
                reference_slug,
            } => {
                check_v(*v, "place.rm")?;
                Ok(Op::PlaceRm {
                    diagram: diagram.clone(),
                    subject_slug: subject_slug.clone(),
                    reference_slug: reference_slug.clone(),
                })
            }
```

In `from_op`, replace both `unreachable!` arms:

```rust
            Op::PlaceSet {
                diagram,
                subject_title,
                subject_slug,
                reference_title,
                reference_slug,
                directions,
            } => OpDto::PlaceSet {
                v: 1,
                diagram: diagram.clone(),
                subject_title: subject_title.clone(),
                subject_slug: subject_slug.clone(),
                reference_title: reference_title.clone(),
                reference_slug: reference_slug.clone(),
                directions: directions.iter().copied().map(dir_name).map(str::to_string).collect(),
            },
            Op::PlaceRm {
                diagram,
                subject_slug,
                reference_slug,
            } => OpDto::PlaceRm {
                v: 1,
                diagram: diagram.clone(),
                subject_slug: subject_slug.clone(),
                reference_slug: reference_slug.clone(),
            },
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p waml-ops-dto`
Expected: PASS, including the three new tests.

- [ ] **Step 7: Regenerate the TypeScript types and check the workspace**

Run: `node scripts/build-wasm.mjs` (or the repo's usual wasm build) and confirm `packages/wasm/src/generated/waml_wasm.d.ts` gains `place.set` / `place.rm` members. If the wasm toolchain is unavailable in this environment, skip the regeneration, say so explicitly in the commit body, and do **not** hand-edit the generated file.

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/waml-ops-dto packages/wasm/src/generated
git commit -m "feat(ops-dto): give place.set and place.rm wire DTOs

from_op panicked on both, which are the only ops the editor emits, so
no wire could carry an editor edit.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: Bundle-path confinement

Pure functions, no I/O, no server. This is the control that stops a crafted op from writing outside the served directory, so it is tested exhaustively and lands before anything can call it.

**Files:**
- Create: `crates/waml-cli/src/serve/paths.rs`
- Modify: `crates/waml-cli/src/serve/mod.rs` (add `pub mod paths;`)

**Interfaces:**
- Consumes: nothing.
- Produces: `pub fn safe_join(root: &Path, rel: &str) -> Result<PathBuf, String>` — returns the absolute path for a bundle-relative path, or an error string naming the reason. `pub fn is_safe_rel(rel: &str) -> Result<(), String>` — the pure syntactic half, usable without touching the filesystem.

- [ ] **Step 1: Write the failing tests**

Create `crates/waml-cli/src/serve/paths.rs` containing only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_ordinary_relative_paths() {
        assert!(is_safe_rel("index.md").is_ok());
        assert!(is_safe_rel("pkg/order/index.md").is_ok());
    }

    #[test]
    fn rejects_traversal_and_absolutes() {
        for bad in [
            "../secrets.md",
            "pkg/../../secrets.md",
            "/etc/passwd",
            "C:\\Windows\\win.ini",
            "\\\\server\\share\\x.md",
            "",
            ".",
        ] {
            assert!(is_safe_rel(bad).is_err(), "should reject {bad:?}");
        }
    }

    #[test]
    fn rejects_nul_and_windows_device_names() {
        assert!(is_safe_rel("a\0b.md").is_err());
        for bad in ["CON", "con.md", "nul", "PRN.txt", "COM1", "lpt9.md", "AUX"] {
            assert!(is_safe_rel(bad).is_err(), "should reject {bad:?}");
        }
        assert!(is_safe_rel("console.md").is_ok(), "CON prefix is not CON");
    }

    #[test]
    fn normalises_separators() {
        assert!(is_safe_rel("pkg\\order\\index.md").is_ok());
    }

    #[test]
    fn safe_join_stays_under_root() {
        let root = std::env::temp_dir().join("waml_paths_test_root");
        std::fs::create_dir_all(&root).unwrap();
        let ok = safe_join(&root, "a/b.md").unwrap();
        assert!(ok.starts_with(&root), "{ok:?} escaped {root:?}");
        assert!(safe_join(&root, "../b.md").is_err());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-cli serve::paths`
Expected: FAIL to compile — `cannot find function 'is_safe_rel'`.

- [ ] **Step 3: Implement**

Above the test module in the same file:

```rust
//! Bundle-path confinement.
//!
//! Every path that reaches this module came off the wire, so it is treated as
//! hostile input: the syntactic check runs first and cheaply, and only a path
//! that survives it is ever joined onto the served root.

use std::path::{Component, Path, PathBuf};

/// Windows reserved device names. Opening one of these does not touch the
/// filesystem at all, so they are rejected on every platform to keep bundle
/// contents portable and behaviour identical across hosts.
const DEVICE_NAMES: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Syntactic half of the check: is this string usable as a bundle path at all?
pub fn is_safe_rel(rel: &str) -> Result<(), String> {
    if rel.is_empty() {
        return Err("empty path".into());
    }
    if rel.contains('\0') {
        return Err("path contains NUL".into());
    }
    let unified = rel.replace('\\', "/");
    if unified.starts_with('/') || unified.starts_with("//") {
        return Err(format!("absolute path `{rel}`"));
    }
    // `C:` or any other drive-letter prefix.
    let b = unified.as_bytes();
    if b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic() {
        return Err(format!("drive-qualified path `{rel}`"));
    }
    for seg in unified.split('/') {
        if seg.is_empty() {
            return Err(format!("empty segment in `{rel}`"));
        }
        if seg == "." || seg == ".." {
            return Err(format!("relative segment `{seg}` in `{rel}`"));
        }
        let stem = seg.split('.').next().unwrap_or(seg).to_ascii_lowercase();
        if DEVICE_NAMES.contains(&stem.as_str()) {
            return Err(format!("reserved device name `{seg}`"));
        }
    }
    Ok(())
}

/// Resolve `rel` under `root`, refusing anything that lands outside it.
///
/// The syntactic check alone is not enough: a symlink inside the root can
/// point out of it, so the joined path is canonicalized when it exists and
/// its parent is canonicalized when it does not (a file about to be created).
pub fn safe_join(root: &Path, rel: &str) -> Result<PathBuf, String> {
    is_safe_rel(rel)?;
    let root_real = root
        .canonicalize()
        .map_err(|e| format!("served directory {}: {e}", root.display()))?;
    let joined = root_real.join(rel.replace('\\', "/"));

    let probe = if joined.exists() {
        joined.clone()
    } else {
        joined
            .parent()
            .ok_or_else(|| format!("no parent for `{rel}`"))?
            .to_path_buf()
    };
    let real = probe
        .canonicalize()
        .map_err(|e| format!("`{rel}`: {e}"))?;
    if !real.starts_with(&root_real) {
        return Err(format!("`{rel}` escapes the served directory"));
    }
    // Reject anything that reintroduced traversal after normalisation.
    if joined.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(format!("`{rel}` escapes the served directory"));
    }
    Ok(joined)
}
```

- [ ] **Step 4: Declare the module**

In `crates/waml-cli/src/serve/mod.rs`, above `ServeArgs`:

```rust
pub mod paths;
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p waml-cli serve::paths`
Expected: PASS, 5 tests.

Note on `safe_join`'s `..` test: it fails at `is_safe_rel`, which is intentional — the two layers overlap, and overlapping is the point.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-cli/src/serve
git commit -m "feat(serve): confine bundle paths to the served directory

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---
