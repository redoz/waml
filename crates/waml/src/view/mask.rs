//! The projection mask: which declared middleware stages are switched OFF for
//! this session.
//!
//! Lives in `waml`, not `waml-editor`, because the CLI and the vscode server
//! run the same chain path and must be able to describe the same state.
//!
//! Presentational reachability ONLY. A row a chain declined to emit is not
//! protected by anything; masking a stage asks for the listing without it. It
//! is never a permission boundary.
//!
//! Session-only by construction: nothing here serializes, and no caller writes
//! it to `.waml/editor.json`. Raw is a deliberate act, not a preference, so
//! every launch starts empty and an author's declared `view:` is what a reader
//! sees unless they ask otherwise.

use std::collections::BTreeSet;

/// A set of disabled middleware names. Empty (the default) is exactly today's
/// behaviour: every declared stage runs.
///
/// `BTreeSet` rather than `HashSet`: `PartialEq` is used as a cache key by the
/// editor's nav-tree memo, and `names()` feeds a popup whose row order must not
/// wobble between frames.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectionMask {
    disabled: BTreeSet<String>,
}

impl ProjectionMask {
    pub fn from_names(names: impl IntoIterator<Item = impl Into<String>>) -> ProjectionMask {
        ProjectionMask {
            disabled: names.into_iter().map(Into::into).collect(),
        }
    }

    /// Is `name` switched off? `Chain::build` asks this per declared entry.
    pub fn is_masked(&self, name: &str) -> bool {
        self.disabled.contains(name)
    }

    pub fn set_masked(&mut self, name: &str, masked: bool) {
        if masked {
            self.disabled.insert(name.to_string());
        } else {
            self.disabled.remove(name);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.disabled.is_empty()
    }

    /// The disabled names, sorted. Deterministic for the popup and for tests.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.disabled.iter().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_mask_disables_nothing() {
        let mask = ProjectionMask::default();
        assert!(mask.is_empty());
        assert!(!mask.is_masked("hide"));
        assert!(!mask.is_masked("uml"));
    }

    #[test]
    fn set_masked_adds_and_removes_one_name_without_touching_siblings() {
        let mut mask = ProjectionMask::default();
        mask.set_masked("hide", true);
        assert!(mask.is_masked("hide"));
        assert!(!mask.is_masked("uml"));

        mask.set_masked("uml", true);
        mask.set_masked("hide", false);
        assert!(!mask.is_masked("hide"));
        assert!(
            mask.is_masked("uml"),
            "unmasking one name must not clear the set"
        );
        assert!(!mask.is_empty());
    }

    #[test]
    fn masks_built_from_the_same_names_in_any_order_are_equal() {
        let a = ProjectionMask::from_names(["uml", "hide"]);
        let b = ProjectionMask::from_names(["hide", "uml"]);
        assert_eq!(a, b, "the nav-tree cache key compares masks by value");
        assert_eq!(a.names().collect::<Vec<_>>(), vec!["hide", "uml"]);
    }
}
