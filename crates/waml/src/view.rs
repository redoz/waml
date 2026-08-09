//! A folder's view as a middleware chain over its contents. Headless: no
//! editor dependency, no makepad, no window. See
//! `docs/superpowers/specs/2026-08-05-folder-view-middleware-design.md`.

pub mod chain;
pub mod decl;
pub(crate) mod hide;
pub mod kind;
pub mod mask;
pub mod projection;
pub(crate) mod root;
pub mod row;
pub mod surface;
pub mod uml;

pub use root::{ROOT_VIEW_NAME, ROOT_VIEW_OWNER};
