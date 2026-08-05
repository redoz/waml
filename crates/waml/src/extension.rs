//! Declares what the core (headless) half of waml ships: middleware names
//! and profiles. An extension is a pure list -- it must never grow behavior
//! hooks like `on_open`/`handle_event`. Nothing resolves *through* an
//! extension: [`crate::view::chain::Chain`] looks middleware up by name
//! exactly as before; an extension is only what put the name in the table.
//! See `docs/superpowers/specs/2026-08-05-folder-view-middleware-design.md`.

use std::sync::Arc;

use crate::profile::ProfileDef;
use crate::view::projection::Projection;
use crate::view::root::RootView;

/// A middleware factory: called once per chain build, since a
/// [`Projection`] may not be `Clone`. Matches
/// [`crate::view::chain::MiddlewareRegistry::register`]'s factory shape.
pub type MiddlewareFactory = Arc<dyn Fn() -> Box<dyn Projection> + Send + Sync>;

pub trait CoreExtension {
    fn name(&self) -> &str;
    fn middleware(&self) -> Vec<(&'static str, MiddlewareFactory)>;
    fn profiles(&self) -> Vec<ProfileDef>;
}

/// The one registered extension today: the `index` (root view) middleware,
/// plus the shipped `okf` and `uml-domain` profiles. Compiled-in; no
/// dynamic loading.
pub struct CoreExt;

impl CoreExtension for CoreExt {
    fn name(&self) -> &str {
        "core"
    }

    fn middleware(&self) -> Vec<(&'static str, MiddlewareFactory)> {
        vec![(
            "index",
            Arc::new(|| Box::new(RootView) as Box<dyn Projection>),
        )]
    }

    fn profiles(&self) -> Vec<ProfileDef> {
        crate::profile::shipped_profiles()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontmatter::Frontmatter;
    use crate::okf;
    use crate::view::chain::{Chain, ChainLimits, MiddlewareRegistry};
    use crate::view::decl::{ViewDecl, ViewEntry};
    use crate::view::projection::ProjectionCtx;

    fn dir() -> okf::Directory {
        okf::Directory {
            address: okf::DirectoryAddress::parse("/sales").unwrap(),
            parent: Some(okf::DirectoryAddress::parse("/").unwrap()),
            child_directories: Vec::new(),
            concepts: Vec::new(),
        }
    }

    fn index() -> okf::Index {
        okf::Index {
            directory: okf::DirectoryAddress::parse("/sales").unwrap(),
            title: None,
            description: None,
            members: Vec::new(),
            body: None,
            authored: true,
            profile: None,
            view: None,
            extra: Frontmatter::default(),
        }
    }

    fn decl_one(name: &str) -> ViewDecl {
        ViewDecl {
            entries: vec![ViewEntry {
                raw: name.to_string(),
                line: 1,
            }],
        }
    }

    fn run_ids(registry: &MiddlewareRegistry) -> Vec<String> {
        let (chain, diagnostics) = Chain::build(&decl_one("index"), registry, &index());
        assert!(
            diagnostics.is_empty(),
            "the `index` name must resolve through a registry built from CoreExt"
        );

        let directory = dir();
        let bundle = okf::Bundle::default();
        let params = Frontmatter::default();
        let descend = |_: &okf::Directory| Chain::default();
        let ctx = ProjectionCtx {
            dir: &directory,
            bundle: &bundle,
            params: &params,
            descend: &descend,
        };
        chain
            .run(&ctx, ChainLimits::default())
            .rows
            .into_iter()
            .map(|row| row.id.path.as_str().to_string())
            .collect()
    }

    #[test]
    fn core_extension_loads_and_projects_with_no_editor_present() {
        // `waml` cannot even name a `DocView` -- this test proves the core
        // half stands entirely on its own.
        let registry =
            MiddlewareRegistry::from_extensions(&[&CoreExt]).expect("core has no duplicates");
        let rows = run_ids(&registry);
        // The fixture directory is empty, so the root view legitimately
        // yields no rows -- what matters is that `index` resolved and ran
        // instead of falling back to the unknown-name diagnostic path.
        assert!(rows.is_empty());
    }

    #[test]
    fn registry_from_extensions_matches_the_hand_built_registry() {
        let from_ext =
            MiddlewareRegistry::from_extensions(&[&CoreExt]).expect("core has no duplicates");

        let mut hand_built = MiddlewareRegistry::new();
        hand_built.register("index", || Box::new(RootView) as Box<dyn Projection>);

        assert_eq!(run_ids(&from_ext), run_ids(&hand_built));
    }

    struct DupA;
    impl CoreExtension for DupA {
        fn name(&self) -> &str {
            "dup-a"
        }
        fn middleware(&self) -> Vec<(&'static str, MiddlewareFactory)> {
            vec![(
                "dup",
                Arc::new(|| Box::new(RootView) as Box<dyn Projection>),
            )]
        }
        fn profiles(&self) -> Vec<ProfileDef> {
            Vec::new()
        }
    }

    struct DupB;
    impl CoreExtension for DupB {
        fn name(&self) -> &str {
            "dup-b"
        }
        fn middleware(&self) -> Vec<(&'static str, MiddlewareFactory)> {
            vec![(
                "dup",
                Arc::new(|| Box::new(RootView) as Box<dyn Projection>),
            )]
        }
        fn profiles(&self) -> Vec<ProfileDef> {
            Vec::new()
        }
    }

    #[test]
    fn duplicate_middleware_names_across_extensions_are_a_build_error() {
        let result = MiddlewareRegistry::from_extensions(&[&DupA, &DupB]);
        assert!(result.is_err());
    }
}
