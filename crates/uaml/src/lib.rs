//! Core library for UAML: a UML-profile authoring format layered on CommonMark.

pub mod diagnostic;
pub mod frontmatter;
pub mod model;
pub mod multiplicity;
pub mod slug;
pub mod grammar;
pub mod parse;
pub mod serialize;
pub mod syntax;
pub mod validate;

#[cfg(test)]
mod smoke {
    #[test]
    fn workspace_builds() {
        assert_eq!(2 + 2, 4);
    }
}
