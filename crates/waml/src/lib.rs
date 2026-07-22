//! Core library for WAML: a UML-profile authoring format layered on CommonMark.

pub mod adornment;
pub mod diagnostic;
pub mod frontmatter;
pub mod grammar;
pub mod index_md;
pub mod layout;
pub mod model;
pub mod multiplicity;
pub mod okf;
pub mod ops;
pub mod parse;
pub mod seed;
pub mod serialize;
pub mod slug;
pub mod solve;
pub mod syntax;
pub mod validate;

#[cfg(test)]
mod smoke {
    #[test]
    fn workspace_builds() {
        assert_eq!(2 + 2, 4);
    }
}
