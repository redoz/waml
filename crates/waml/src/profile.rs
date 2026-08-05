//! Static table of known UML profiles. A profile names a document dialect
//! (`uml-domain`, `okf`) and optionally contributes a default `view:` chain
//! for folders that declare that profile but no `view:` of their own.
//!
//! Adopted unchanged from the superseded plan's profile table, except
//! `default_view` is now `Option<ViewDecl>` to match the middleware-chain
//! design: `Chain`-shaped defaults, not a bespoke view enum.

use crate::view::decl::ViewDecl;

/// One known profile: its exact name and its optional default view chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileDef {
    pub name: &'static str,
    pub default_view: Option<ViewDecl>,
}

const PROFILES: &[ProfileDef] = &[
    ProfileDef {
        name: "uml-domain",
        default_view: None,
    },
    ProfileDef {
        name: "okf",
        default_view: None,
    },
];

/// All statically shipped profiles, in table order -- the list `CoreExt`
/// (`crate::extension`) hands back from `profiles()`. Kept separate from
/// [`profile`] so it never accidentally picks up a test-only override: this
/// is the real, compiled-in table, nothing else.
pub(crate) fn shipped_profiles() -> Vec<ProfileDef> {
    PROFILES.to_vec()
}

/// Look up a shipped profile by its exact name. No case folding: `"UML-Domain"`
/// does not match `"uml-domain"`.
pub fn profile(name: &str) -> Option<&'static ProfileDef> {
    PROFILES
        .iter()
        .find(|p| p.name == name)
        .or_else(|| test_override(name))
}

// Test-only seam: lets a `resolved_view` test drive the "inherited profile
// default" step for real, without a fixture profile ever shipping in
// `PROFILES`. Registrations are leaked (test-scoped, never freed) and
// thread-local so parallel tests do not interfere with each other.
#[cfg(test)]
thread_local! {
    static TEST_OVERRIDES: std::cell::RefCell<std::collections::HashMap<&'static str, &'static ProfileDef>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

#[cfg(test)]
fn test_override(name: &str) -> Option<&'static ProfileDef> {
    TEST_OVERRIDES.with(|overrides| overrides.borrow().get(name).copied())
}

#[cfg(not(test))]
fn test_override(_name: &str) -> Option<&'static ProfileDef> {
    None
}

/// Register a profile for the duration of the current test thread. Overwrites
/// a prior registration under the same name.
#[cfg(test)]
pub(crate) fn register_test_profile(def: ProfileDef) {
    let leaked: &'static ProfileDef = Box::leak(Box::new(def));
    TEST_OVERRIDES.with(|overrides| {
        overrides.borrow_mut().insert(leaked.name, leaked);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_profiles_resolve_by_name_and_default_to_no_view() {
        let uml = profile("uml-domain").expect("uml-domain is shipped");
        assert_eq!(uml.name, "uml-domain");
        assert_eq!(uml.default_view, None);

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
