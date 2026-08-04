//! `waml serve`: the web editor plus an ops API over one local directory.
//!
//! Laid out like `crate::lsp`: this module owns transport and process
//! lifetime, and delegates every semantic decision to `waml` proper.

use std::path::PathBuf;

// Consumed by Tasks 4-6 (apply_ops/apply_documents confinement, routes).
#[allow(dead_code)]
pub mod paths;

// Consumed by Task 6 (the routes' guard layer).
#[allow(dead_code)]
pub mod guard;

// Consumed by Task 6 (routes hold a ServeState behind a Mutex).
#[allow(dead_code)]
pub mod state;

/// Everything `run` needs, decoupled from clap so tests can build one.
///
/// `bind_all`/`api_only`/`no_open` are wired up by later tasks; `#[allow]`
/// here rather than deferring the fields keeps the struct's shape — and this
/// task's parse tests — final from Task 1 onward.
#[derive(Debug, Clone)]
#[allow(dead_code)]
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
