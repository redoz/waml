//! `uaml lsp` — stdio language server. Server code lives here so the core
//! crate (`uaml`) stays LSP-free.

pub mod bundle;
pub mod map;
mod server;

/// Entry point for `uaml lsp --stdio`. Implemented in Task 11.
pub fn run() -> i32 {
    server::serve_stdio();
    0
}
