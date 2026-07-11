# UAML Rust Tooling — Plan 5: `uaml-cli` (`check` + `fmt`)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the `uaml` binary: `uaml check` (parse + validate a bundle, print diagnostics as human text or JSON, exit non-zero on errors) and `uaml fmt` (canonicalize documents in place, with `--check` and `--stdout` modes), refusing to rewrite any file that has validation errors.

**Architecture:** A second workspace crate `crates/uaml-cli` producing the binary `uaml`. It is the **only** crate that touches the filesystem, `stdin`, and terminal output — the core `uaml` library stays pure and WASM-friendly. The CLI is split into a thin filesystem shell (`io.rs`, `main.rs`) over pure functions (`render.rs`, `commands.rs`) that are unit-tested without any I/O.

**Tech Stack:** Rust 2021, `clap` (derive), `serde` + `serde_json`, `uaml` (path dep). Builds on Plans 1–4.

## Global Constraints

- All Global Constraints from Plans 1–4 apply to the `uaml` core library. The **CLI crate may use the filesystem, stdin, and stdout** (that is its job) and may depend on non-WASM crates.
- **Exit codes:** `check` exits `1` if any `Error`-severity diagnostic is present (warnings alone exit `0`). `fmt --check` exits `1` if any file is not already canonical or was skipped for errors; otherwise `0`.
- **`fmt` never loses data:** a file with any `Error`-severity diagnostic located in it is **skipped** (left byte-for-byte untouched) and reported; only clean files are rewritten. This is what keeps deferred/unsupported constructs safe.
- **No OWOX branding** in any output.
- `fmt` treats each physical `.md` file as a single document (no blob-marker splitting); `check` accepts concatenated blobs.

---

### Task 1: scaffold `uaml-cli` with clap subcommands

**Files:**
- Create: `crates/uaml-cli/Cargo.toml`
- Create: `crates/uaml-cli/src/main.rs`
- Modify: `Cargo.toml` (workspace members + deps)

**Interfaces:**
- Produces: a `uaml` binary whose `--help` lists `check` and `fmt`; a `Cli` type parseable via `clap`.

- [ ] **Step 1: Register the crate and shared deps**

In the root `Cargo.toml`, extend the members list and add workspace dependencies:
```toml
[workspace]
resolver = "2"
members = ["crates/uaml", "crates/uaml-cli"]
```
Add under `[workspace.dependencies]`:
```toml
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

- [ ] **Step 2: Create the CLI crate manifest**

`crates/uaml-cli/Cargo.toml`:
```toml
[package]
name = "uaml-cli"
version = "0.0.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "Command-line tools (check, fmt) for UAML documents."

[[bin]]
name = "uaml"
path = "src/main.rs"

[dependencies]
uaml = { path = "../uaml" }
clap = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
```

- [ ] **Step 3: Write the failing test (argument parsing)**

`crates/uaml-cli/src/main.rs`:
```rust
use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "uaml", about = "Tools for UAML documents")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Parse and validate documents, reporting diagnostics.
    Check {
        /// Files or directories to check.
        paths: Vec<PathBuf>,
        /// Read a single document/bundle from stdin instead.
        #[arg(long)]
        stdin: bool,
        /// Output format.
        #[arg(long, value_enum, default_value_t = Format::Human)]
        format: Format,
    },
    /// Rewrite documents in canonical form.
    Fmt {
        /// Files or directories to format.
        paths: Vec<PathBuf>,
        /// Do not write; exit non-zero if any file is not already formatted.
        #[arg(long)]
        check: bool,
        /// Write the formatted result to stdout instead of the file.
        #[arg(long)]
        stdout: bool,
    },
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum Format {
    Human,
    Json,
}

fn main() {
    let cli = Cli::parse();
    let code = match cli.command {
        Command::Check { .. } => 0,
        Command::Fmt { .. } => 0,
    };
    std::process::exit(code);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_check_with_json_flag() {
        let cli = Cli::try_parse_from(["uaml", "check", "a.md", "--format", "json"]).unwrap();
        match cli.command {
            Command::Check { paths, format, stdin } => {
                assert_eq!(paths.len(), 1);
                assert_eq!(format, Format::Json);
                assert!(!stdin);
            }
            _ => panic!("expected check"),
        }
    }

    #[test]
    fn parses_fmt_check() {
        let cli = Cli::try_parse_from(["uaml", "fmt", "--check", "docs/"]).unwrap();
        assert!(matches!(cli.command, Command::Fmt { check: true, .. }));
    }
}
```

- [ ] **Step 4: Build and test**

Run: `cargo test -p uaml-cli`
Expected: compiles; `2 passed`.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/uaml-cli/Cargo.toml crates/uaml-cli/src/main.rs
git commit -m "feat(uaml-cli): scaffold binary with check and fmt subcommands"
```

---

### Task 2: `io` — assemble a bundle from paths / stdin

**Files:**
- Create: `crates/uaml-cli/src/io.rs`
- Modify: `crates/uaml-cli/src/main.rs`

**Interfaces:**
- Produces:
  - `pub fn read_bundle(paths: &[PathBuf], stdin: bool) -> std::io::Result<Vec<(String, String)>>` — for `check`: expands directories to `*.md`, splits a blob file on markers, reads stdin as one blob.
  - `pub fn read_files(paths: &[PathBuf]) -> std::io::Result<Vec<(String, String)>>` — for `fmt`: each `.md` file as a single `(path, content)` entry (no blob splitting); directories expanded to `*.md`.
  - `pub fn collect_md(paths: &[PathBuf]) -> std::io::Result<Vec<PathBuf>>` — recursively gather `.md` files.

- [ ] **Step 1: Write the failing tests**

`crates/uaml-cli/src/io.rs`:
```rust
use std::path::PathBuf;

use uaml::parse::split_bundle;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_blob_text_into_docs() {
        let blob = "<!-- a/one.md -->\n# One\n\n<!-- a/two.md -->\n# Two\n";
        let docs = expand_text("stdin", blob);
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].0, "a/one.md");
    }

    #[test]
    fn plain_text_uses_its_own_path() {
        let docs = expand_text("shop/order.md", "# Order\n");
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].0, "shop/order.md");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p uaml-cli io`
Expected: FAIL — `expand_text` not found.

- [ ] **Step 3: Implement `io`**

Prepend to `crates/uaml-cli/src/io.rs`:
```rust
use std::fs;
use std::io::Read;

/// Turn one file's text into `(path, content)` docs: split on `<!-- path -->`
/// markers if present, otherwise a single doc keyed by `display_path`.
pub fn expand_text(display_path: &str, text: &str) -> Vec<(String, String)> {
    if text.contains("<!--") {
        let parts = split_bundle(text);
        // split_bundle returns "pasted/doc.md" for unmarked text; only trust it when markers existed.
        if parts.len() > 1 || parts.first().map(|(p, _)| p != "pasted/doc.md").unwrap_or(false) {
            return parts;
        }
    }
    vec![(display_path.to_string(), text.to_string())]
}

/// Recursively collect `.md` files under the given files/directories.
pub fn collect_md(paths: &[PathBuf]) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for p in paths {
        if p.is_dir() {
            for entry in fs::read_dir(p)? {
                let path = entry?.path();
                out.extend(collect_md(&[path])?);
            }
        } else if p.extension().and_then(|e| e.to_str()) == Some("md") {
            out.push(p.clone());
        }
    }
    out.sort();
    Ok(out)
}

fn path_key(p: &PathBuf) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// For `check`: expand dirs to `*.md`, split blob files, or read stdin as one blob.
pub fn read_bundle(paths: &[PathBuf], stdin: bool) -> std::io::Result<Vec<(String, String)>> {
    if stdin {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        return Ok(expand_text("stdin", &buf));
    }
    let mut out = Vec::new();
    for file in collect_md(paths)? {
        let text = fs::read_to_string(&file)?;
        out.extend(expand_text(&path_key(&file), &text));
    }
    Ok(out)
}

/// For `fmt`: each physical `.md` file is one document (no blob splitting).
pub fn read_files(paths: &[PathBuf]) -> std::io::Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    for file in collect_md(paths)? {
        let text = fs::read_to_string(&file)?;
        out.push((path_key(&file), text));
    }
    Ok(out)
}
```

- [ ] **Step 4: Wire the module in and run**

In `crates/uaml-cli/src/main.rs`, add near the top (after the `use` lines):
```rust
mod io;
```

Run: `cargo test -p uaml-cli io`
Expected: PASS — `2 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/uaml-cli/src/io.rs crates/uaml-cli/src/main.rs
git commit -m "feat(uaml-cli): assemble bundles from paths and stdin"
```

---

### Task 3: `check` — render diagnostics + exit code

**Files:**
- Create: `crates/uaml-cli/src/commands.rs`
- Modify: `crates/uaml-cli/src/main.rs`

**Interfaces:**
- Produces (pure functions):
  - `pub fn render_human(diags: &[Diagnostic]) -> String`
  - `pub fn render_json(diags: &[Diagnostic]) -> String`
  - `pub fn check_exit_code(diags: &[Diagnostic]) -> i32`
- Wires `Command::Check` in `main` to `read_bundle` → `validate` → render → exit.

- [ ] **Step 1: Write the failing tests**

`crates/uaml-cli/src/commands.rs`:
```rust
use serde::Serialize;
use uaml::diagnostic::{Diagnostic, Severity};

#[cfg(test)]
mod tests {
    use super::*;
    use uaml::diagnostic::DiagCode;

    fn sample() -> Vec<Diagnostic> {
        vec![
            Diagnostic::new(DiagCode::UnresolvedTarget, "no doc './ghost.md'", "a/order.md", 8),
            Diagnostic::warn(DiagCode::UnknownType, "unknown type 'bpmn.Task'", "a/x.md", 2),
        ]
    }

    #[test]
    fn human_output_has_file_line_and_code() {
        let out = render_human(&sample());
        assert!(out.contains("a/order.md:8: error[unresolved-target]: no doc './ghost.md'"));
        assert!(out.contains("a/x.md:2: warning[unknown-type]:"));
    }

    #[test]
    fn json_output_is_an_array_of_diagnostics() {
        let out = render_json(&sample());
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 2);
        assert_eq!(v[0]["code"], "unresolved-target");
        assert_eq!(v[0]["line"], 8);
    }

    #[test]
    fn exit_code_is_one_with_errors_zero_with_only_warnings() {
        assert_eq!(check_exit_code(&sample()), 1);
        let only_warn = vec![Diagnostic::warn(DiagCode::UnknownType, "w", "a.md", 1)];
        assert_eq!(check_exit_code(&only_warn), 0);
        assert_eq!(check_exit_code(&[]), 0);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p uaml-cli commands`
Expected: FAIL — functions not found.

- [ ] **Step 3: Implement the render + exit functions**

Prepend to `crates/uaml-cli/src/commands.rs`:
```rust
#[derive(Serialize)]
struct DiagDto<'a> {
    severity: &'a str,
    code: &'a str,
    message: &'a str,
    file: &'a str,
    line: usize,
}

fn severity_str(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}

fn sorted(diags: &[Diagnostic]) -> Vec<&Diagnostic> {
    let mut v: Vec<&Diagnostic> = diags.iter().collect();
    v.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    v
}

pub fn render_human(diags: &[Diagnostic]) -> String {
    if diags.is_empty() {
        return "No problems found.".to_string();
    }
    let mut lines = Vec::new();
    for d in sorted(diags) {
        lines.push(format!(
            "{}:{}: {}[{}]: {}",
            d.file,
            d.line,
            severity_str(d.severity),
            d.code.as_str(),
            d.message
        ));
    }
    let errors = diags.iter().filter(|d| d.severity == Severity::Error).count();
    let warnings = diags.len() - errors;
    lines.push(format!("\n{errors} error(s), {warnings} warning(s)."));
    lines.join("\n")
}

pub fn render_json(diags: &[Diagnostic]) -> String {
    let dtos: Vec<DiagDto> = sorted(diags)
        .into_iter()
        .map(|d| DiagDto {
            severity: severity_str(d.severity),
            code: d.code.as_str(),
            message: &d.message,
            file: &d.file,
            line: d.line,
        })
        .collect();
    serde_json::to_string_pretty(&dtos).unwrap_or_else(|_| "[]".to_string())
}

pub fn check_exit_code(diags: &[Diagnostic]) -> i32 {
    if diags.iter().any(|d| d.severity == Severity::Error) {
        1
    } else {
        0
    }
}
```

- [ ] **Step 4: Wire `check` into `main`**

In `crates/uaml-cli/src/main.rs`: add `mod commands;` near the top, and replace the `Command::Check { .. } => 0,` arm with:
```rust
        Command::Check { paths, stdin, format } => {
            let bundle = match io::read_bundle(&paths, stdin) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("uaml: {e}");
                    std::process::exit(2);
                }
            };
            let diags = uaml::validate::validate(&bundle);
            let out = match format {
                Format::Human => commands::render_human(&diags),
                Format::Json => commands::render_json(&diags),
            };
            println!("{out}");
            commands::check_exit_code(&diags)
        }
```

- [ ] **Step 5: Run to verify passing**

Run: `cargo test -p uaml-cli`
Expected: PASS — parsing + io + commands tests green.

- [ ] **Step 6: Manual smoke test**

Run: `printf -- '---\ntype: uml.Class\ntitle: A\n---\n# A\n\n## Relationships\n- depends [Ghost](./ghost.md)\n' | cargo run -q -p uaml-cli -- check --stdin`
Expected: prints `stdin:8: error[unresolved-target]: ...` and the process exits `1`.

- [ ] **Step 7: Commit**

```bash
git add crates/uaml-cli/src/commands.rs crates/uaml-cli/src/main.rs
git commit -m "feat(uaml-cli): check command with human/json output and exit codes"
```

---

### Task 4: `fmt` — canonicalize, skip files with errors

**Files:**
- Modify: `crates/uaml-cli/src/commands.rs`, `crates/uaml-cli/src/main.rs`

**Interfaces:**
- Produces (pure):
  - `pub struct FmtResult { pub path: String, pub formatted: String, pub changed: bool, pub skipped: bool }`
  - `pub fn plan_fmt(files: &[(String, String)]) -> Vec<FmtResult>` — validates the whole bundle for context, formats each file whose own lines have no `Error` diagnostic, and marks the rest `skipped`.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `crates/uaml-cli/src/commands.rs`:
```rust
    #[test]
    fn formats_a_clean_file_and_detects_change() {
        // A default `[1]` is dropped by canonical form → the file changes.
        let files = vec![("x/a.md".to_string(),
            "---\ntype: uml.Class\ntitle: A\n---\n# A\n\n## Attributes\n- id: AId [1]\n".to_string())];
        let plan = plan_fmt(&files);
        assert_eq!(plan.len(), 1);
        assert!(!plan[0].skipped);
        assert!(plan[0].changed);
        assert!(plan[0].formatted.contains("- id: AId\n"));
        assert!(!plan[0].formatted.contains("[1]"));
    }

    #[test]
    fn skips_a_file_with_errors() {
        let files = vec![("x/a.md".to_string(),
            "---\ntype: uml.Class\ntitle: A\n---\n# A\n\n## Attributes\n- broken line\n".to_string())];
        let plan = plan_fmt(&files);
        assert!(plan[0].skipped);
        assert!(!plan[0].changed);
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p uaml-cli commands`
Expected: FAIL — `plan_fmt` not found.

- [ ] **Step 3: Implement `plan_fmt`**

Add to `crates/uaml-cli/src/commands.rs` (imports at top; body below the render functions):
```rust
use uaml::parse::parse_document;
use uaml::serialize::serialize_document;
use uaml::validate::validate;
```
```rust
pub struct FmtResult {
    pub path: String,
    pub formatted: String,
    pub changed: bool,
    pub skipped: bool,
}

pub fn plan_fmt(files: &[(String, String)]) -> Vec<FmtResult> {
    let diags = validate(files);
    let mut out = Vec::new();
    for (path, text) in files {
        let has_error = diags
            .iter()
            .any(|d| d.file == *path && d.severity == Severity::Error);
        if has_error {
            out.push(FmtResult { path: path.clone(), formatted: text.clone(), changed: false, skipped: true });
            continue;
        }
        let formatted = serialize_document(&parse_document(text));
        let changed = formatted != *text;
        out.push(FmtResult { path: path.clone(), formatted, changed, skipped: false });
    }
    out
}
```

- [ ] **Step 4: Wire `fmt` into `main`**

In `crates/uaml-cli/src/main.rs`, replace the `Command::Fmt { .. } => 0,` arm with:
```rust
        Command::Fmt { paths, check, stdout } => {
            let files = match io::read_files(&paths) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("uaml: {e}");
                    std::process::exit(2);
                }
            };
            let plan = commands::plan_fmt(&files);
            let mut exit = 0;
            for r in &plan {
                if r.skipped {
                    eprintln!("uaml: skipped {} (has errors; run `uaml check`)", r.path);
                    exit = 1;
                    continue;
                }
                if stdout {
                    println!("{}", r.formatted);
                } else if check {
                    if r.changed {
                        eprintln!("uaml: {} is not formatted", r.path);
                        exit = 1;
                    }
                } else if r.changed {
                    if let Err(e) = std::fs::write(&r.path, &r.formatted) {
                        eprintln!("uaml: failed to write {}: {e}", r.path);
                        std::process::exit(2);
                    }
                    println!("uaml: formatted {}", r.path);
                }
            }
            exit
        }
```

- [ ] **Step 5: Run to verify passing**

Run: `cargo test -p uaml-cli`
Expected: PASS — all CLI tests green.

- [ ] **Step 6: Final full-workspace gate**

Run: `cargo test`
Expected: PASS — every test in `uaml` and `uaml-cli`.

Run: `cargo build --release`
Expected: builds the `uaml` binary.

- [ ] **Step 7: Manual smoke test**

Run:
```bash
mkdir -p /tmp/uaml-demo && printf -- '---\ntype: uml.Class\ntitle: A\n---\n# A\n\n## Attributes\n- id: AId [1]\n' > /tmp/uaml-demo/a.md
cargo run -q -p uaml-cli -- fmt --check /tmp/uaml-demo
```
Expected: reports `a.md is not formatted` and exits `1`. Re-running without `--check` rewrites it (dropping `[1]`); a subsequent `--check` exits `0`.

- [ ] **Step 8: Commit**

```bash
git add crates/uaml-cli/src/commands.rs crates/uaml-cli/src/main.rs
git commit -m "feat(uaml-cli): fmt command that skips files with errors"
```

---

## Self-Review

- **Spec coverage (this plan's slice):** `uaml` binary with `check` and `fmt` ✔ (Tasks 1, 3, 4); input forms — directory, single file, blob (check), stdin ✔ (Task 2); `check` human + JSON output, exit `1` on errors ✔ (Task 3); `fmt` in-place / `--check` / `--stdout`, canonical form, **skips files with errors so nothing is lost** ✔ (Task 4); core library never does I/O — the CLI owns it all ✔; no OWOX branding ✔. Zip input remains deferred (matches scope).
- **Placeholder scan:** none — every step has concrete code and commands; the `main` match arms are stubbed in Task 1 and explicitly replaced in Tasks 3–4.
- **Type consistency:** `validate`/`parse_document`/`serialize_document`/`split_bundle` signatures match Plans 2–4. `Diagnostic`/`Severity`/`DiagCode::as_str` match Plan 4. `read_bundle` (check) and `read_files` (fmt) return the same `Vec<(String, String)>` bundle shape the library consumes.
- **Whole-project sequencing:** this completes the five-plan arc — Plan 1 (foundations) → 2 (Document tier / `fmt` core) → 3 (Model tier) → 4 (validation) → 5 (CLI). Implement in order; each plan's tests are green before the next begins.
