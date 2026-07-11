//! Core library for UAML: a UML-profile authoring format layered on CommonMark.

pub mod frontmatter;
pub mod multiplicity;
pub mod slug;

#[cfg(test)]
mod smoke {
    #[test]
    fn workspace_builds() {
        assert_eq!(2 + 2, 4);
    }
}
