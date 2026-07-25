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

### Task 4: Token and request guards

The access controls, still as pure functions with no server around them. Every rule from the spec's Security section that can be decided from a token string plus a few headers lives here and is tested here.

**Files:**
- Create: `crates/waml-cli/src/serve/guard.rs`
- Modify: `crates/waml-cli/src/serve/mod.rs` (add `pub mod guard;`)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub struct Token(String)` with `pub fn generate() -> Token`, `pub fn from_raw(s: String) -> Token`, `pub fn as_str(&self) -> &str`, `pub fn matches(&self, presented: &str) -> bool`
  - `pub struct Guard { pub token: Token, pub origin: String, pub port: u16, pub bind_all: bool }`
  - `pub enum Deny { Unauthorized, Forbidden(String) }`
  - `pub struct ReqFacts<'a> { pub bearer: Option<&'a str>, pub query_token: Option<&'a str>, pub origin: Option<&'a str>, pub host: Option<&'a str>, pub client_header: Option<&'a str>, pub mutating: bool }`
  - `pub fn check(g: &Guard, req: &ReqFacts) -> Result<(), Deny>`

- [ ] **Step 1: Write the failing tests**

Create `crates/waml-cli/src/serve/guard.rs` containing only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn guard() -> Guard {
        Guard {
            token: Token::from_raw("secret".into()),
            origin: "http://127.0.0.1:8099".into(),
            port: 8099,
            bind_all: false,
        }
    }

    fn facts() -> ReqFacts<'static> {
        ReqFacts {
            bearer: Some("secret"),
            query_token: None,
            origin: None,
            host: Some("127.0.0.1:8099"),
            client_header: None,
            mutating: false,
        }
    }

    #[test]
    fn a_generated_token_is_long_and_unique() {
        let a = Token::generate();
        let b = Token::generate();
        assert!(a.as_str().len() >= 43, "got {}", a.as_str().len());
        assert_ne!(a.as_str(), b.as_str());
        assert!(a
            .as_str()
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn accepts_the_token_in_either_position() {
        assert!(check(&guard(), &facts()).is_ok());
        let f = ReqFacts { bearer: None, query_token: Some("secret"), ..facts() };
        assert!(check(&guard(), &f).is_ok());
    }

    #[test]
    fn rejects_a_missing_or_wrong_token() {
        let f = ReqFacts { bearer: None, ..facts() };
        assert!(matches!(check(&guard(), &f), Err(Deny::Unauthorized)));
        let f = ReqFacts { bearer: Some("secre"), ..facts() };
        assert!(matches!(check(&guard(), &f), Err(Deny::Unauthorized)));
        let f = ReqFacts { bearer: Some("secretx"), ..facts() };
        assert!(matches!(check(&guard(), &f), Err(Deny::Unauthorized)));
    }

    #[test]
    fn rejects_a_foreign_origin() {
        let f = ReqFacts { origin: Some("https://redoz.github.io"), ..facts() };
        assert!(matches!(check(&guard(), &f), Err(Deny::Forbidden(_))));
        let f = ReqFacts { origin: Some("http://127.0.0.1:8099"), ..facts() };
        assert!(check(&guard(), &f).is_ok());
    }

    #[test]
    fn rejects_a_rebound_host() {
        let f = ReqFacts { host: Some("evil.example.com:8099"), ..facts() };
        assert!(matches!(check(&guard(), &f), Err(Deny::Forbidden(_))));
        let f = ReqFacts { host: Some("[::1]:8099"), ..facts() };
        assert!(check(&guard(), &f).is_ok());
        let f = ReqFacts { host: Some("127.0.0.1:9999"), ..facts() };
        assert!(matches!(check(&guard(), &f), Err(Deny::Forbidden(_))));
    }

    #[test]
    fn mutating_requests_need_the_client_header() {
        let f = ReqFacts { mutating: true, ..facts() };
        assert!(matches!(check(&guard(), &f), Err(Deny::Forbidden(_))));
        let f = ReqFacts { mutating: true, client_header: Some("1"), ..facts() };
        assert!(check(&guard(), &f).is_ok());
    }

    #[test]
    fn bind_all_relaxes_only_the_host_check() {
        let g = Guard { bind_all: true, ..guard() };
        let f = ReqFacts { host: Some("192.168.1.5:8099"), ..facts() };
        assert!(check(&g, &f).is_ok());
        let f = ReqFacts { host: Some("192.168.1.5:8099"), bearer: None, ..facts() };
        assert!(matches!(check(&g, &f), Err(Deny::Unauthorized)));
    }
}
```

`Guard` must therefore derive nothing special, but it does need to be constructible field-by-field with struct-update syntax — keep all fields `pub`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-cli serve::guard`
Expected: FAIL to compile — `cannot find struct 'Guard' in this scope`.

- [ ] **Step 3: Implement**

Above the test module in the same file:

```rust
//! Access control for `waml serve`.
//!
//! The token is the control. Origin, Host and the custom-header requirement
//! are defence in depth against a browser being made to act as a confused
//! deputy; none of them is trusted on its own, and loopback is not treated as
//! authentication at all.

use rand::RngCore;
use subtle::ConstantTimeEq;

/// A per-invocation access token. Never persisted, and never printed except in
/// the single startup URL.
#[derive(Clone)]
pub struct Token(String);

impl Token {
    /// 256 bits from the OS CSPRNG, URL-safe base64 without padding so it can
    /// ride in a query string untouched.
    pub fn generate() -> Token {
        let mut raw = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut raw);
        Token(b64url(&raw))
    }

    /// Adopt an existing string. Used by tests and by the router, which is
    /// handed the same token `run` printed.
    pub fn from_raw(s: String) -> Token {
        Token(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Constant-time compare. Length is compared in the clear because the
    /// token length is public — it is always 43 characters.
    pub fn matches(&self, presented: &str) -> bool {
        let a = self.0.as_bytes();
        let b = presented.as_bytes();
        a.len() == b.len() && bool::from(a.ct_eq(b))
    }
}

/// URL-safe base64, no padding. Hand-rolled rather than adding a dependency
/// for twelve lines.
fn b64url(bytes: &[u8]) -> String {
    const ALPHA: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        for i in 0..chunk.len() + 1 {
            out.push(ALPHA[((n >> (18 - 6 * i)) & 0x3f) as usize] as char);
        }
    }
    out
}

/// Server-side facts every check is made against.
pub struct Guard {
    pub token: Token,
    /// The server's own origin, e.g. `http://127.0.0.1:8099`.
    pub origin: String,
    pub port: u16,
    pub bind_all: bool,
}

/// Request-side facts, read from headers before any body is touched.
pub struct ReqFacts<'a> {
    pub bearer: Option<&'a str>,
    pub query_token: Option<&'a str>,
    pub origin: Option<&'a str>,
    pub host: Option<&'a str>,
    pub client_header: Option<&'a str>,
    /// True for routes that write. Only these require the custom header.
    pub mutating: bool,
}

#[derive(Debug)]
pub enum Deny {
    Unauthorized,
    Forbidden(String),
}

/// Run every check. All must pass; the order only decides which error the
/// caller is told about first.
pub fn check(g: &Guard, req: &ReqFacts) -> Result<(), Deny> {
    let presented = req.bearer.or(req.query_token).ok_or(Deny::Unauthorized)?;
    if !g.token.matches(presented) {
        return Err(Deny::Unauthorized);
    }
    if let Some(origin) = req.origin {
        if origin != g.origin {
            return Err(Deny::Forbidden(format!("origin `{origin}` not allowed")));
        }
    }
    let host = req
        .host
        .ok_or_else(|| Deny::Forbidden("no Host header".into()))?;
    if !host_ok(g, host) {
        return Err(Deny::Forbidden(format!("host `{host}` not allowed")));
    }
    if req.mutating && req.client_header != Some("1") {
        return Err(Deny::Forbidden("missing X-Waml-Client: 1".into()));
    }
    Ok(())
}

/// A loopback host on the bound port, or — under `--bind-all` — any host on
/// that port.
///
/// The port comparison is what makes this an anti-rebinding control: a
/// hostile name that resolves to 127.0.0.1 still arrives with its own name in
/// `Host`, and is refused here.
fn host_ok(g: &Guard, host: &str) -> bool {
    let Some((name, port)) = split_host(host) else {
        return false;
    };
    if port != g.port {
        return false;
    }
    if g.bind_all {
        return true;
    }
    matches!(name, "127.0.0.1" | "localhost" | "[::1]" | "::1")
}

/// Split `host:port`, tolerating a bracketed IPv6 literal.
fn split_host(host: &str) -> Option<(&str, u16)> {
    let (name, port) = if let Some(rest) = host.strip_prefix('[') {
        let end = rest.find(']')?;
        (&host[..end + 2], rest[end + 1..].strip_prefix(':')?)
    } else {
        let i = host.rfind(':')?;
        (&host[..i], &host[i + 1..])
    };
    Some((name, port.parse().ok()?))
}
```

`localhost` is accepted because browsers resolve it to loopback and it makes the printed URL friendlier. It does not weaken the control: the port must still match a socket this process bound.

- [ ] **Step 4: Declare the module**

In `crates/waml-cli/src/serve/mod.rs`, next to `pub mod paths;`, add `pub mod guard;`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p waml-cli serve::guard`
Expected: PASS, 7 tests.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-cli/src/serve
git commit -m "feat(serve): token, origin, host and client-header guards

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: Serve state — bundle, revision, apply, write-back

The semantic core, with no HTTP in it. Because `ServeState` is directly testable, the route tasks that follow only have to test wiring.

**Files:**
- Create: `crates/waml-cli/src/serve/state.rs`
- Modify: `crates/waml-cli/src/serve/mod.rs` (add `pub mod state;`)

**Interfaces:**
- Consumes: `crate::io::{read_files, write_back}` (`crates/waml-cli/src/io.rs:121` and `:150`), `crate::serve::paths::is_safe_rel`, `waml::ops::{apply, OpError}` (`crates/waml/src/ops/mod.rs:183`, error fields `index`/`op`/`selector`/`reason`), `waml::parse::build_model` (`crates/waml/src/parse.rs:822`), `waml::validate::validate`, `waml_ops_dto::OpDto`.
- Produces:
  - `pub struct ServeState` with `pub fn revision(&self) -> u64`, `pub fn bundle(&self) -> &[(String, String)]`, `pub fn root(&self) -> &Path`, `pub fn model(&self) -> waml::model::Model`, `pub fn diagnostics(&self) -> Vec<waml::diagnostic::Diagnostic>`, `pub fn apply_ops(&mut self, at: u64, dtos: &[OpDto]) -> Result<Vec<(String, String)>, ApplyFailure>`
  - `pub fn load(root: &Path) -> std::io::Result<ServeState>`
  - `pub enum ApplyFailure { Stale { current: u64 }, Op { index: usize, reason: String }, Io(String) }`

- [ ] **Step 1: Write the failing tests**

Create `crates/waml-cli/src/serve/state.rs` containing only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal on-disk bundle: one package index plus one class document.
    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::write(
            root.join("index.md"),
            "---\ntype: uml.Package\n---\n\n# Shop\n",
        )
        .unwrap();
        std::fs::write(
            root.join("order.md"),
            "---\ntype: uml.Class\n---\n\n# Order\n",
        )
        .unwrap();
        (dir, root)
    }

    fn attr_add() -> OpDto {
        serde_json::from_str(
            r#"{"op":"attr.add","node":"order","name":"total","tyToken":"Money"}"#,
        )
        .unwrap()
    }

    #[test]
    fn load_reads_the_directory_and_starts_at_revision_zero() {
        let (_d, root) = fixture();
        let st = load(&root).unwrap();
        assert_eq!(st.revision(), 0);
        assert_eq!(st.bundle().len(), 2);
        assert!(st.bundle().iter().any(|(p, _)| p.ends_with("order.md")));
    }

    #[test]
    fn the_model_projection_equals_calling_waml_directly() {
        let (_d, root) = fixture();
        let st = load(&root).unwrap();
        let direct = waml::parse::build_model(st.bundle());
        assert_eq!(
            serde_json::to_string(&st.model()).unwrap(),
            serde_json::to_string(&direct).unwrap()
        );
        let _ = st.diagnostics();
    }

    #[test]
    fn apply_bumps_the_revision_and_returns_only_changed_files() {
        let (_d, root) = fixture();
        let mut st = load(&root).unwrap();
        let before_index = std::fs::read_to_string(root.join("index.md")).unwrap();
        let changed = st.apply_ops(0, &[attr_add()]).unwrap();
        assert_eq!(st.revision(), 1);
        assert_eq!(changed.len(), 1, "only order.md changed: {changed:?}");
        assert!(changed[0].0.ends_with("order.md"));
        assert!(std::fs::read_to_string(root.join("order.md"))
            .unwrap()
            .contains("total"));
        assert_eq!(
            std::fs::read_to_string(root.join("index.md")).unwrap(),
            before_index,
            "an untouched file must not be rewritten"
        );
    }

    #[test]
    fn a_stale_revision_is_rejected_without_writing() {
        let (_d, root) = fixture();
        let mut st = load(&root).unwrap();
        st.apply_ops(0, &[attr_add()]).unwrap();
        let before = std::fs::read_to_string(root.join("order.md")).unwrap();
        match st.apply_ops(0, &[attr_add()]) {
            Err(ApplyFailure::Stale { current }) => assert_eq!(current, 1),
            other => panic!("expected Stale, got {other:?}"),
        }
        assert_eq!(
            std::fs::read_to_string(root.join("order.md")).unwrap(),
            before
        );
    }

    #[test]
    fn a_failing_op_writes_nothing_and_names_its_index() {
        let (_d, root) = fixture();
        let mut st = load(&root).unwrap();
        let bad: OpDto = serde_json::from_str(
            r#"{"op":"attr.add","node":"no-such-node","name":"x","tyToken":"Money"}"#,
        )
        .unwrap();
        let before = std::fs::read_to_string(root.join("order.md")).unwrap();
        match st.apply_ops(0, &[attr_add(), bad]) {
            Err(ApplyFailure::Op { index, .. }) => assert_eq!(index, 1),
            other => panic!("expected Op failure, got {other:?}"),
        }
        assert_eq!(st.revision(), 0, "a failed batch must not bump the revision");
        assert_eq!(
            std::fs::read_to_string(root.join("order.md")).unwrap(),
            before
        );
    }
}
```

If the `attr.add` JSON above does not match the real DTO field names, read the enum at `crates/waml-ops-dto/src/lib.rs:15` and fix the **test** to the real spelling. Never reshape the DTO to fit a guess in this plan.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-cli serve::state`
Expected: FAIL to compile — `cannot find function 'load' in this scope`.

- [ ] **Step 3: Implement**

Above the test module in the same file:

```rust
//! The served bundle, its revision, and the only path that mutates it.
//!
//! Deliberately free of HTTP concepts: this is where "the server owns
//! validation and canonical formatting" is actually true, and it is tested
//! without a socket.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use waml_ops_dto::OpDto;

use crate::io;
use crate::serve::paths::is_safe_rel;

pub struct ServeState {
    root: PathBuf,
    bundle: Vec<(String, String)>,
    /// Bumped on every successful apply. Clients echo the revision they hold;
    /// a mismatch means they are working from a stale read.
    revision: u64,
}

#[derive(Debug)]
pub enum ApplyFailure {
    /// The client's revision is behind. It must re-read before retrying.
    Stale { current: u64 },
    /// An op was rejected. Nothing was written.
    Op { index: usize, reason: String },
    /// Filesystem or path-confinement failure.
    Io(String),
}

/// Read every markdown document under `root` into memory.
pub fn load(root: &Path) -> std::io::Result<ServeState> {
    let bundle = io::read_files(std::slice::from_ref(&root.to_path_buf()))?;
    Ok(ServeState {
        root: root.to_path_buf(),
        bundle,
        revision: 0,
    })
}

impl ServeState {
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn bundle(&self) -> &[(String, String)] {
        &self.bundle
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The resolved model. Identical to calling `waml::parse::build_model` on
    /// `bundle()` — that equivalence is the entire justification for the
    /// `/api/model` projection, so it is asserted in the tests.
    pub fn model(&self) -> waml::model::Model {
        waml::parse::build_model(&self.bundle)
    }

    pub fn diagnostics(&self) -> Vec<waml::diagnostic::Diagnostic> {
        waml::validate::validate(&self.bundle)
    }

    /// Apply `dtos` as one all-or-nothing batch.
    ///
    /// `at` is the revision the client believes it holds. On success the new
    /// bundle is written to disk, the revision is bumped, and only the
    /// documents whose bytes actually changed are returned.
    pub fn apply_ops(
        &mut self,
        at: u64,
        dtos: &[OpDto],
    ) -> Result<Vec<(String, String)>, ApplyFailure> {
        if at != self.revision {
            return Err(ApplyFailure::Stale {
                current: self.revision,
            });
        }
        let mut ops = Vec::with_capacity(dtos.len());
        for (i, dto) in dtos.iter().enumerate() {
            ops.push(
                dto.to_op()
                    .map_err(|reason| ApplyFailure::Op { index: i, reason })?,
            );
        }
        let next = waml::ops::apply(&self.bundle, &ops).map_err(|e| ApplyFailure::Op {
            index: e.index,
            reason: e.reason,
        })?;

        // Confinement runs over the RESULT, so an op that invents a path is
        // caught even though no op names a path directly. Checked before any
        // write, so a rejected batch leaves the disk untouched.
        for (p, _) in &next {
            is_safe_rel(p).map_err(ApplyFailure::Io)?;
        }

        io::write_back(&self.bundle, &next).map_err(|e| ApplyFailure::Io(e.to_string()))?;

        let old: BTreeMap<&str, &str> = self
            .bundle
            .iter()
            .map(|(p, c)| (p.as_str(), c.as_str()))
            .collect();
        let changed: Vec<(String, String)> = next
            .iter()
            .filter(|(p, c)| old.get(p.as_str()) != Some(&c.as_str()))
            .cloned()
            .collect();

        self.bundle = next;
        self.revision += 1;
        Ok(changed)
    }
}
```

`io::write_back` opens exactly the path strings that appear in the bundle, and `io::read_files` produced those from the served root — so `is_safe_rel` above is checking the same strings that will be opened. If `read_files` turns out to yield paths that are absolute or otherwise not root-relative, store them relative in `load` and join `root` at the write site. Do **not** relax `is_safe_rel` to accommodate it.

- [ ] **Step 4: Declare the module**

In `crates/waml-cli/src/serve/mod.rs`, add `pub mod state;`. `tempfile` and `serde_json` are already available from Task 1.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p waml-cli serve::state`
Expected: PASS, 5 tests.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-cli/src/serve
git commit -m "feat(serve): bundle state, revisions and all-or-nothing op apply

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 6: Read routes and the running server

The first task with a socket. Brings up axum, wires the guard into every handler, serves the three read projections, and turns `serve::run` from a stub into a real server.

**Files:**
- Create: `crates/waml-cli/src/lib.rs`
- Create: `crates/waml-cli/src/serve/routes.rs`
- Create: `crates/waml-cli/tests/serve_e2e.rs`
- Modify: `crates/waml-cli/Cargo.toml` (add a `[lib]` target)
- Modify: `crates/waml-cli/src/main.rs` (use the library's modules instead of declaring its own)
- Modify: `crates/waml-cli/src/serve/mod.rs` (real `run`)

**Interfaces:**
- Consumes: `serve::state::{load, ServeState, ApplyFailure}`, `serve::guard::{check, Guard, Deny, ReqFacts, Token}`.
- Produces:
  - `pub struct App { pub state: Arc<Mutex<ServeState>>, pub guard: Arc<Guard>, pub api_only: bool }`
  - `pub fn App::new(state: ServeState, token: Token, port: u16, bind_all: bool, api_only: bool) -> App`
  - `pub fn router(app: App) -> axum::Router`
  - `pub async fn serve_on(listener: tokio::net::TcpListener, app: App) -> std::io::Result<()>`
  - Response shapes: `{"revision":N,"files":[[path,md],…]}`, `{"revision":N,"model":{…}}`, `{"revision":N,"diagnostics":[…]}`

- [ ] **Step 1: Give the crate a library target**

The integration test drives the server in-process, so the modules must be reachable from outside the binary.

Add to `crates/waml-cli/Cargo.toml`, above the existing `[[bin]]`:

```toml
[lib]
name = "waml_cli"
path = "src/lib.rs"
```

Create `crates/waml-cli/src/lib.rs`:

```rust
//! Library face of the CLI, so integration tests can drive the server
//! in-process instead of shelling out to the binary.

pub mod commands;
pub mod io;
pub mod lsp;
pub mod ops_dto;
pub mod serve;
```

In `crates/waml-cli/src/main.rs`, delete the `mod commands;` / `mod io;` / `mod lsp;` / `mod ops_dto;` / `mod serve;` declarations and replace them with:

```rust
use waml_cli::{commands, io, lsp, ops_dto, serve};
```

A binary can name its own package's library directly; no extra dependency entry is needed. If any of those modules referred to a sibling as `crate::io`, that still resolves inside the library — only `main.rs` changes.

Run: `cargo test -p waml-cli`
Expected: the existing suite still passes with no duplicated modules.

- [ ] **Step 2: Write the failing integration test**

Create `crates/waml-cli/tests/serve_e2e.rs`:

```rust
//! Drives a real server on an ephemeral port. No mocks — the point is that the
//! guard, the router and the state agree with each other.

use std::net::SocketAddr;

use waml_cli::serve::{self, guard::Token, routes};

fn fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    std::fs::write(root.join("index.md"), "---\ntype: uml.Package\n---\n\n# Shop\n").unwrap();
    std::fs::write(root.join("order.md"), "---\ntype: uml.Class\n---\n\n# Order\n").unwrap();
    (dir, root)
}

/// Boot a server on port 0 and return its address plus the token it wants.
async fn boot(root: &std::path::Path) -> (SocketAddr, String) {
    let raw = "test-token".to_string();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = routes::App::new(
        serve::state::load(root).unwrap(),
        Token::from_raw(raw.clone()),
        addr.port(),
        false,
        true,
    );
    tokio::spawn(async move { routes::serve_on(listener, app).await.unwrap() });
    (addr, raw)
}

#[tokio::test]
async fn reads_require_a_token() {
    let (_d, root) = fixture();
    let (addr, _tok) = boot(&root).await;
    let res = reqwest::Client::new()
        .get(format!("http://127.0.0.1:{}/api/bundle", addr.port()))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn bundle_model_and_diagnostics_all_carry_the_revision() {
    let (_d, root) = fixture();
    let (addr, tok) = boot(&root).await;
    let c = reqwest::Client::new();
    for route in ["bundle", "model", "diagnostics"] {
        let v: serde_json::Value = c
            .get(format!("http://127.0.0.1:{}/api/{route}", addr.port()))
            .bearer_auth(&tok)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(v["revision"], 0, "{route}");
    }
    let v: serde_json::Value = c
        .get(format!("http://127.0.0.1:{}/api/bundle", addr.port()))
        .bearer_auth(&tok)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(v["files"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn the_token_may_ride_in_the_query_string() {
    let (_d, root) = fixture();
    let (addr, tok) = boot(&root).await;
    let res = reqwest::get(format!(
        "http://127.0.0.1:{}/api/bundle?token={tok}",
        addr.port()
    ))
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
}

#[tokio::test]
async fn a_foreign_origin_is_refused_even_with_the_token() {
    let (_d, root) = fixture();
    let (addr, tok) = boot(&root).await;
    let res = reqwest::Client::new()
        .get(format!("http://127.0.0.1:{}/api/bundle", addr.port()))
        .bearer_auth(&tok)
        .header("Origin", "https://redoz.github.io")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);
}

#[tokio::test]
async fn the_model_route_matches_calling_waml_directly() {
    let (_d, root) = fixture();
    let (addr, tok) = boot(&root).await;
    let v: serde_json::Value = reqwest::Client::new()
        .get(format!("http://127.0.0.1:{}/api/model", addr.port()))
        .bearer_auth(&tok)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let bundle = waml_cli::io::read_files(&[root.clone()]).unwrap();
    let direct = serde_json::to_value(waml::parse::build_model(&bundle)).unwrap();
    assert_eq!(v["model"], direct);
}
```

This test needs `tokio`'s `macros` feature (already in the workspace feature list) and `waml` as a dev-dependency of `waml-cli` — it is already a normal dependency, so nothing to add.

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p waml-cli --test serve_e2e`
Expected: FAIL to compile — `could not find 'routes' in 'serve'`.

- [ ] **Step 4: Implement the router**

Create `crates/waml-cli/src/serve/routes.rs`:

```rust
//! HTTP surface. Every semantic decision belongs to `state`; this module only
//! translates between it and status codes.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;
use tokio::net::TcpListener;

use crate::serve::guard::{check, Deny, Guard, ReqFacts, Token};
use crate::serve::state::ServeState;

#[derive(Clone)]
pub struct App {
    pub state: Arc<Mutex<ServeState>>,
    pub guard: Arc<Guard>,
    pub api_only: bool,
}

impl App {
    pub fn new(state: ServeState, token: Token, port: u16, bind_all: bool, api_only: bool) -> App {
        let host = if bind_all { "0.0.0.0" } else { "127.0.0.1" };
        App {
            state: Arc::new(Mutex::new(state)),
            guard: Arc::new(Guard {
                token,
                origin: format!("http://{host}:{port}"),
                port,
                bind_all,
            }),
            api_only,
        }
    }
}

/// Pull the guard's inputs out of a request, so handlers stay one line of
/// policy each.
fn facts<'a>(
    headers: &'a HeaderMap,
    q: &'a HashMap<String, String>,
    mutating: bool,
) -> ReqFacts<'a> {
    let hv = |k: &str| headers.get(k).and_then(|v| v.to_str().ok());
    ReqFacts {
        bearer: hv("authorization").and_then(|v| v.strip_prefix("Bearer ")),
        query_token: q.get("token").map(|s| s.as_str()),
        origin: hv("origin"),
        host: hv("host"),
        client_header: hv("x-waml-client"),
        mutating,
    }
}

fn deny_response(d: Deny) -> Response {
    match d {
        Deny::Unauthorized => (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "missing or invalid token"})),
        )
            .into_response(),
        Deny::Forbidden(why) => {
            (StatusCode::FORBIDDEN, Json(json!({"error": why}))).into_response()
        }
    }
}

/// Guard a handler, returning early on denial.
macro_rules! guarded {
    ($app:expr, $headers:expr, $q:expr, $mutating:expr) => {
        if let Err(d) = check(&$app.guard, &facts(&$headers, &$q, $mutating)) {
            return deny_response(d);
        }
    };
}

async fn get_bundle(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    guarded!(app, headers, q, false);
    let st = app.state.lock().unwrap();
    Json(json!({ "revision": st.revision(), "files": st.bundle() })).into_response()
}

async fn get_model(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    guarded!(app, headers, q, false);
    let st = app.state.lock().unwrap();
    Json(json!({ "revision": st.revision(), "model": st.model() })).into_response()
}

async fn get_diagnostics(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    guarded!(app, headers, q, false);
    let st = app.state.lock().unwrap();
    Json(json!({ "revision": st.revision(), "diagnostics": st.diagnostics() })).into_response()
}

pub fn router(app: App) -> Router {
    Router::new()
        .route("/api/bundle", get(get_bundle))
        .route("/api/model", get(get_model))
        .route("/api/diagnostics", get(get_diagnostics))
        .with_state(app)
}

pub async fn serve_on(listener: TcpListener, app: App) -> std::io::Result<()> {
    axum::serve(listener, router(app)).await
}
```

- [ ] **Step 5: Make `run` bring the server up**

In `crates/waml-cli/src/serve/mod.rs`, add `pub mod routes;` alongside the other module declarations and replace the stub body of `run`:

```rust
pub fn run(args: ServeArgs) -> i32 {
    let state = match state::load(&args.dir) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("waml: {}: {e}", args.dir.display());
            return 2;
        }
    };
    let token = guard::Token::generate();
    let host = if args.bind_all { "0.0.0.0" } else { "127.0.0.1" };

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("waml: {e}");
            return 2;
        }
    };
    rt.block_on(async move {
        let listener = match tokio::net::TcpListener::bind((host, args.port)).await {
            Ok(l) => l,
            Err(e) => {
                eprintln!("waml: bind {host}:{}: {e}", args.port);
                return 2;
            }
        };
        let port = listener.local_addr().map(|a| a.port()).unwrap_or(args.port);
        if args.bind_all {
            eprintln!(
                "waml serve: WARNING binding 0.0.0.0 — every host that can reach \
                 this machine can reach this API if it learns the token below"
            );
        }
        eprintln!(
            "waml serve  http://127.0.0.1:{port}/?token={}   (serving {})",
            token.as_str(),
            args.dir.display()
        );
        let app = routes::App::new(state, token, port, args.bind_all, args.api_only);
        match routes::serve_on(listener, app).await {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("waml: {e}");
                2
            }
        }
    })
}
```

`--no-open` is parsed and, for now, controls nothing: there is no browser launch yet. Do **not** add an `open`-style dependency to give it something to suppress; if a launch is added later, it reads this flag then.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p waml-cli --test serve_e2e`
Expected: PASS, 5 tests.

- [ ] **Step 7: Verify by hand**

In one shell: `cargo run -p waml-cli -- serve crates/waml-editor/tests/fixtures/mini --port 8123`
In another, using the token it printed:

```bash
curl -s "http://127.0.0.1:8123/api/model?token=TOKEN" | head -c 200
curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8123/api/model
```

Expected: JSON beginning `{"revision":0,"model":{`, then `401`.

- [ ] **Step 8: Commit**

```bash
git add crates/waml-cli
git commit -m "feat(serve): serve bundle, model and diagnostics over guarded HTTP

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 7: `POST /api/ops`

The write path, including the two failure modes that matter: a stale client and a rejected op.

**Files:**
- Modify: `crates/waml-cli/src/serve/routes.rs`
- Modify: `crates/waml-cli/tests/serve_e2e.rs`

**Interfaces:**
- Consumes: `ServeState::apply_ops`, `ApplyFailure`, `waml_ops_dto::OpDto`.
- Produces: `pub struct OpsRequest { pub revision: u64, pub ops: Vec<OpDto> }`; route `POST /api/ops` returning `{"revision":N+1,"changed":[[path,md],…]}`, or `409 {"error":"stale","current":N}`, or `422 {"index":I,"reason":"…"}`, or `500 {"error":"…"}`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/waml-cli/tests/serve_e2e.rs`:

```rust
fn attr_add() -> serde_json::Value {
    serde_json::json!({"op":"attr.add","node":"order","name":"total","tyToken":"Money"})
}

#[tokio::test]
async fn ops_write_the_file_and_return_only_what_changed() {
    let (_d, root) = fixture();
    let (addr, tok) = boot(&root).await;
    let res = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{}/api/ops", addr.port()))
        .bearer_auth(&tok)
        .header("X-Waml-Client", "1")
        .json(&serde_json::json!({"revision": 0, "ops": [attr_add()]}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let v: serde_json::Value = res.json().await.unwrap();
    assert_eq!(v["revision"], 1);
    let changed = v["changed"].as_array().unwrap();
    assert_eq!(changed.len(), 1);
    assert!(changed[0][0].as_str().unwrap().ends_with("order.md"));
    assert!(std::fs::read_to_string(root.join("order.md"))
        .unwrap()
        .contains("total"));
}

#[tokio::test]
async fn a_write_without_the_client_header_is_refused() {
    let (_d, root) = fixture();
    let (addr, tok) = boot(&root).await;
    let res = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{}/api/ops", addr.port()))
        .bearer_auth(&tok)
        .json(&serde_json::json!({"revision": 0, "ops": [attr_add()]}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);
    assert!(!std::fs::read_to_string(root.join("order.md"))
        .unwrap()
        .contains("total"));
}

#[tokio::test]
async fn a_stale_revision_gets_409_with_the_current_one() {
    let (_d, root) = fixture();
    let (addr, tok) = boot(&root).await;
    let c = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/api/ops", addr.port());
    c.post(&url)
        .bearer_auth(&tok)
        .header("X-Waml-Client", "1")
        .json(&serde_json::json!({"revision": 0, "ops": [attr_add()]}))
        .send()
        .await
        .unwrap();
    let res = c
        .post(&url)
        .bearer_auth(&tok)
        .header("X-Waml-Client", "1")
        .json(&serde_json::json!({"revision": 0, "ops": [attr_add()]}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 409);
    let v: serde_json::Value = res.json().await.unwrap();
    assert_eq!(v["current"], 1);
}

#[tokio::test]
async fn a_rejected_op_gets_422_naming_its_index() {
    let (_d, root) = fixture();
    let (addr, tok) = boot(&root).await;
    let bad =
        serde_json::json!({"op":"attr.add","node":"no-such-node","name":"x","tyToken":"Money"});
    let res = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{}/api/ops", addr.port()))
        .bearer_auth(&tok)
        .header("X-Waml-Client", "1")
        .json(&serde_json::json!({"revision": 0, "ops": [attr_add(), bad]}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 422);
    let v: serde_json::Value = res.json().await.unwrap();
    assert_eq!(v["index"], 1);
    assert!(!std::fs::read_to_string(root.join("order.md"))
        .unwrap()
        .contains("total"));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-cli --test serve_e2e ops`
Expected: FAIL — `405 Method Not Allowed`, since `/api/ops` has no route.

- [ ] **Step 3: Implement the route**

In `crates/waml-cli/src/serve/routes.rs`, add the imports, the request shape, and the handler:

```rust
use axum::routing::post;
use serde::Deserialize;
use waml_ops_dto::OpDto;

use crate::serve::state::ApplyFailure;

#[derive(Deserialize)]
pub struct OpsRequest {
    /// The revision the client believes it holds.
    pub revision: u64,
    pub ops: Vec<OpDto>,
}

async fn post_ops(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
    Json(body): Json<OpsRequest>,
) -> Response {
    guarded!(app, headers, q, true);
    let mut st = app.state.lock().unwrap();
    match st.apply_ops(body.revision, &body.ops) {
        Ok(changed) => {
            Json(json!({ "revision": st.revision(), "changed": changed })).into_response()
        }
        Err(ApplyFailure::Stale { current }) => (
            StatusCode::CONFLICT,
            Json(json!({ "error": "stale", "current": current })),
        )
            .into_response(),
        Err(ApplyFailure::Op { index, reason }) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "index": index, "reason": reason })),
        )
            .into_response(),
        Err(ApplyFailure::Io(why)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": why })),
        )
            .into_response(),
    }
}
```

and in `router`, add `.route("/api/ops", post(post_ops))`.

Extractor order is load-bearing: axum runs extractors left to right, and `Json(body)` consumes the request body. It is last in the argument list so the guard decides before any body is parsed. Do not move it earlier.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p waml-cli --test serve_e2e`
Expected: PASS, 9 tests.

- [ ] **Step 5: Full gate**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-cli
git commit -m "feat(serve): apply ops over HTTP with revision and error mapping

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---
