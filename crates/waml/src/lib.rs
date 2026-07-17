//! Core library for WAML: a UML-profile authoring format layered on CommonMark.

pub mod diagnostic;
pub mod frontmatter;
pub mod model;
pub mod okf;
pub mod ops;
pub mod multiplicity;
pub mod slug;
pub mod solve;
pub mod grammar;
pub mod index_md;
pub mod layout;
pub mod parse;
pub mod seed;
pub mod serialize;
pub mod syntax;
pub mod uml;
pub mod validate;

#[cfg(test)]
mod smoke {
    #[test]
    fn workspace_builds() {
        assert_eq!(2 + 2, 4);
    }
}
