//! Core library for WAML: a UML-profile authoring format layered on CommonMark.

pub mod action;
pub mod adornment;
pub mod analysis;
pub mod bundle_envelope;
pub mod diagnostic;
pub mod edit;
pub mod frontmatter;
pub mod host;
pub mod index_md;
pub mod layout;
pub mod model;
pub mod multiplicity;
pub mod okf;
pub mod ops;
pub mod seed;
pub mod share;
pub mod slug;
pub mod solve;
pub mod source;
pub mod uml;
pub mod validate;

#[doc(hidden)]
pub mod compat;

#[cfg(test)]
mod smoke {
    #[test]
    fn workspace_builds() {
        assert_eq!(2 + 2, 4);
    }
}
