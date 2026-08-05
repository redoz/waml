//! Static table of known UML profiles. A profile names a document dialect
//! (`uml-domain`, `okf`) and optionally contributes a default `view:` chain
//! for folders that declare that profile but no `view:` of their own.
//!
//! Adopted unchanged from the superseded plan's profile table, except
//! `default_view` is now `Option<ViewDecl>` to match the middleware-chain
//! design: `Chain`-shaped defaults, not a bespoke view enum.

use crate::view::decl::{ViewDecl, ViewEntry};

/// One known profile: its exact name and its optional default view chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileDef {
    pub name: &'static str,
    pub default_view: Option<ViewDecl>,
}

/// Core's shipped profiles -- the list `CoreExt` (`crate::extension`) hands
/// back from `profiles()`. A builder, not a `const`, since `ProfileDef` now
/// holds a `Vec` (`ViewDecl::entries`) via `default_view`.
pub(crate) fn shipped_profiles() -> Vec<ProfileDef> {
    vec![ProfileDef {
        name: "okf",
        default_view: None,
    }]
}

/// The `uml` extension's shipped profile: `uml-domain` folders default to the
/// `["uml"]` chain when they declare no `view:` of their own. `line: 0` marks
/// the entry as never authored in any file -- a diagnostic spanning an
/// inherited default must not point at a line the author never wrote.
pub(crate) fn uml_profiles() -> Vec<ProfileDef> {
    vec![ProfileDef {
        name: "uml-domain",
        default_view: Some(ViewDecl {
            entries: vec![ViewEntry {
                raw: "uml".to_string(),
                line: 0,
            }],
        }),
    }]
}

/// Look up a shipped profile by its exact name. No case folding: `"UML-Domain"`
/// does not match `"uml-domain"`. Searches the whole shipped set -- extension
/// order must never change whether a name resolves, since this is the
/// parse-time name check. Returns owned data: `default_view` can no longer be
/// handed back as `&'static` now that it holds a `Vec`.
pub fn profile(name: &str) -> Option<ProfileDef> {
    shipped_profiles()
        .into_iter()
        .chain(uml_profiles())
        .find(|p| p.name == name)
        .or_else(|| test_override(name))
}

// Test-only seam: lets a `resolved_view` test drive the "inherited profile
// default" step for real, without a fixture profile ever shipping in the
// builder functions above. Thread-local so parallel tests do not interfere
// with each other.
#[cfg(test)]
thread_local! {
    static TEST_OVERRIDES: std::cell::RefCell<std::collections::HashMap<String, ProfileDef>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

#[cfg(test)]
fn test_override(name: &str) -> Option<ProfileDef> {
    TEST_OVERRIDES.with(|overrides| overrides.borrow().get(name).cloned())
}

#[cfg(not(test))]
fn test_override(_name: &str) -> Option<ProfileDef> {
    None
}

/// Register a profile for the duration of the current test thread. Overwrites
/// a prior registration under the same name.
#[cfg(test)]
pub(crate) fn register_test_profile(def: ProfileDef) {
    TEST_OVERRIDES.with(|overrides| {
        overrides.borrow_mut().insert(def.name.to_string(), def);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_profiles_resolve_by_name() {
        let uml = profile("uml-domain").expect("uml-domain is shipped");
        assert_eq!(uml.name, "uml-domain");
        let chain_names: Vec<&str> = uml
            .default_view
            .as_ref()
            .expect("uml-domain has a default view")
            .entries
            .iter()
            .map(|e| e.raw.as_str())
            .collect();
        assert_eq!(chain_names, vec!["uml"]);

        let okf = profile("okf").expect("okf is shipped");
        assert_eq!(okf.name, "okf");
        assert_eq!(okf.default_view, None);
    }

    #[test]
    fn unknown_profiles_resolve_to_none() {
        assert!(profile("unknown-profile").is_none());
        assert!(profile("").is_none());
        // No case folding: exact name only.
        assert!(profile("UML-Domain").is_none());
        assert!(profile("OKF").is_none());
    }
}
