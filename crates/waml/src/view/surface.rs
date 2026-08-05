//! `SurfaceId`: the name table an extension's editor half contributes rows
//! and containers into. One name table with middleware — not a second
//! namespace.

/// A surface name contributed by an extension's editor half.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SurfaceId(pub String);
