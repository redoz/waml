//! The op wire contract (`OpDto`) now lives in the shared `uaml-ops-dto` crate so
//! the WASM bindings can reuse it. Re-exported here to keep `crate::ops_dto::*`
//! paths in `main.rs` unchanged.
pub use uaml_ops_dto::*;
