#[path = "support/authority_guard.rs"]
mod authority_guard;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use authority_guard::{analyze_sources, analyze_workspace};

fn reasons(source: &str) -> Vec<String> {
    analyze_sources([("crates/waml/src/compat.rs", source)])
        .into_iter()
        .map(|violation| violation.reason)
        .collect()
}

fn fixture_workspace(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("authority-guard")
        .join(name)
}

fn compile_external(source: &str) -> std::process::Output {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after Unix epoch")
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("waml-authority-api-{}-{nonce}", std::process::id()));
    let src = root.join("src");
    fs::create_dir_all(&src).expect("create external compile fixture");
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .to_string_lossy()
        .replace('\\', "/");
    fs::write(
        root.join("Cargo.toml"),
        format!(
            "[package]\nname = \"authority-api-fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[dependencies]\nwaml = {{ path = \"{manifest_dir}\" }}\n"
        ),
    )
    .expect("write external fixture manifest");
    fs::write(src.join("main.rs"), source).expect("write external fixture source");

    Command::new(env!("CARGO"))
        .args(["check", "--quiet", "--offline", "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .env(
            "CARGO_TARGET_DIR",
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(Path::parent)
                .expect("workspace root")
                .join("target")
                .join("authority-api-fixtures"),
        )
        .output()
        .expect("run external cargo check")
}

#[test]
fn production_sources_have_one_parser_and_serializer_authority() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|crates| crates.parent())
        .expect("workspace root")
        .to_path_buf();
    let violations = analyze_workspace(&workspace).expect("analyze workspace Rust sources");

    assert!(
        violations.is_empty(),
        "legacy or shadow parser/serializer authority remains:\n{}",
        violations
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn cargo_metadata_and_module_graph_cover_every_production_source_shape() {
    let violations =
        analyze_workspace(&fixture_workspace("workspace")).expect("analyze fixture workspace");
    let paths = violations
        .iter()
        .map(|violation| violation.path.as_str())
        .collect::<Vec<_>>();

    for expected in [
        "packages/core/source/root.rs",
        "packages/core/source/nested.rs",
        "packages/core/source/lib_authority.rs",
        "packages/core/shared/path_module.rs",
        "packages/core/shared/included.rs",
        "packages/core/shared/body_included.rs",
        "packages/core/shared/body_expression.rs",
        "packages/core/targets/custom_bin.rs",
        "packages/core/targets/bin_authority.rs",
        "packages/core/targets/custom_example.rs",
        "packages/core/targets/example_authority.rs",
        "packages/core/tools/custom_build.rs",
        "packages/core/tools/build_authority.rs",
        "../outside-member/custom/lib_entry.rs",
    ] {
        assert!(
            paths.iter().any(|path| path.ends_with(expected)),
            "Cargo/module discovery missed {expected}: {violations:#?}"
        );
    }
    assert!(
        violations
            .iter()
            .any(|violation| violation.reason.contains("generated Rust include")),
        "dynamic/generated include policy was not enforced: {violations:#?}"
    );
    assert!(
        violations
            .iter()
            .any(|violation| violation.reason.contains("body_included_shadow")),
        "function-body include authority escaped: {violations:#?}"
    );
    assert!(
        violations.iter().any(|violation| {
            violation
                .path
                .ends_with("packages/core/shared/body_expression.rs")
                && violation
                    .reason
                    .contains("context-dependent function-body include")
        }),
        "context-dependent body include did not fail closed: {violations:#?}"
    );
    assert!(
        violations.iter().any(|violation| {
            violation
                .reason
                .contains("forbidden workspace dependency direction")
        }),
        "workspace dependency direction was not enforced: {violations:#?}"
    );

    for (path, caller) in [
        ("packages/core/source/root.rs", "lib_route"),
        ("packages/core/targets/custom_bin.rs", "bin_route"),
        ("packages/core/targets/custom_example.rs", "example_route"),
        ("packages/core/tools/custom_build.rs", "build_route"),
    ] {
        assert!(
            violations.iter().any(|violation| {
                violation.path.ends_with(path)
                    && violation.reason.contains(caller)
                    && violation.reason.contains("reaches shadow authority")
            }),
            "target-scoped `crate::` call for {caller} was not propagated: {violations:#?}"
        );
    }
}

#[test]
fn cargo_target_crate_roots_preserve_cross_module_crate_resolution() {
    let violations =
        analyze_workspace(&fixture_workspace("workspace")).expect("analyze fixture workspace");

    for (path, caller) in [
        ("packages/core/source/root.rs", "lib_route"),
        ("packages/core/targets/custom_bin.rs", "bin_route"),
        ("packages/core/targets/custom_example.rs", "example_route"),
        ("packages/core/tools/custom_build.rs", "build_route"),
    ] {
        assert!(
            violations.iter().any(|violation| {
                violation.path.ends_with(path)
                    && violation.reason.contains(caller)
                    && violation.reason.contains("reaches shadow authority")
            }),
            "target-scoped `crate::` call for {caller} was not propagated: {violations:#?}"
        );
    }
}

#[test]
fn cargo_target_type_identities_isolate_same_named_receivers() {
    let violations =
        analyze_workspace(&fixture_workspace("workspace")).expect("analyze fixture workspace");

    for expected in ["bin_unsafe_field", "bin_unsafe_chain"] {
        assert!(
            violations.iter().any(|violation| {
                violation
                    .path
                    .ends_with("packages/core/targets/custom_bin.rs")
                    && violation.reason.contains(expected)
                    && violation.reason.contains("model-to-source reparse")
            }),
            "target-qualified receiver edge for `{expected}` escaped: {violations:#?}"
        );
    }
    for control in ["lib_safe_field", "lib_safe_chain"] {
        assert!(
            violations
                .iter()
                .all(|violation| !violation.reason.contains(control)),
            "same-named receiver leaked across Cargo targets into `{control}`: {violations:#?}"
        );
    }
}

#[test]
fn raw_authority_entry_is_not_an_external_api() {
    let output = compile_external(
        r#"
        fn main() {
            let _raw_parser = waml::uml::syntax::parser::parse;
        }
        "#,
    );

    assert!(
        !output.status.success(),
        "raw syntax parser unexpectedly remained public"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("private") || stderr.contains("E0603"),
        "expected a visibility failure, got:\n{stderr}"
    );
}

#[test]
fn compat_shadow_parser_is_rejected_without_name_conventions() {
    let violations = reasons(
        r#"
        mod compatibility {
            fn decode(raw: &str) -> Option<LayoutStatement> {
                let words = raw.split_whitespace().collect::<Vec<_>>();
                words.is_empty().then(|| unimplemented!())
            }
        }
        "#,
    );

    assert!(
        violations
            .iter()
            .any(|reason| reason.contains("raw-text grammar")),
        "{violations:#?}"
    );
}

#[test]
fn differently_named_and_pub_super_helpers_are_rejected() {
    let differently_named = reasons(
        r#"
        fn decipher(input: &str) -> LayoutStatement {
            let _parts = input.split_once(" of ");
            unimplemented!()
        }
        "#,
    );
    let restricted_visibility = reasons(
        r#"
        pub(super) fn internalize(input: String) -> Result<LayoutStatement, Error> {
            let _parts = input.trim().split(',').collect::<Vec<_>>();
            unimplemented!()
        }
        "#,
    );

    assert!(
        differently_named
            .iter()
            .any(|reason| reason.contains("raw-text grammar")),
        "{differently_named:#?}"
    );
    assert!(
        restricted_visibility
            .iter()
            .any(|reason| reason.contains("raw-text grammar")),
        "{restricted_visibility:#?}"
    );
}

#[test]
fn real_syntax_tree_authority_signatures_and_builders_are_rejected() {
    let violations = analyze_sources([
        (
            "crates/waml/src/types.rs",
            r#"
            type SharedInput<'a> = &'a SourceText;
            type SharedRaw<'a> = &'a str;
            type ProtectedTree = Arc<SyntaxTree<UmlLanguage>>;
            type WrappedTree = Result<Box<ProtectedTree>, Error>;
            type TreeSink = Option<ProtectedTree>;
            struct TreeBuilder<L>(core::marker::PhantomData<L>);

            mod sibling_types {
                pub type Tree = Arc<SyntaxTree<UmlLanguage>>;

                pub struct Slot {
                    pub tree: Tree,
                }
            }

            mod left_types {
                pub type Tree = Arc<SyntaxTree<UmlLanguage>>;

                pub struct Slot {
                    pub tree: Tree,
                }
            }

            mod right_types {
                pub type Tree = Arc<SyntaxTree<UmlLanguage>>;

                pub struct Slot {
                    pub tree: Tree,
                }
            }
            "#,
        ),
        (
            "crates/waml/src/uml/compat.rs",
            r#"
            use crate::types::{SharedInput, SharedRaw, TreeBuilder, TreeSink, WrappedTree};

            fn direct(text: SourceText) -> Arc<SyntaxTree<UmlLanguage>> {
                let _ = text;
                unimplemented!()
            }

            fn direct_raw(text: &str) -> Arc<SyntaxTree<UmlLanguage>> {
                let _ = text;
                unimplemented!()
            }

            fn imported_alias(text: SharedInput<'_>) -> WrappedTree {
                let _ = text;
                unimplemented!()
            }

            fn raw_alias(text: SharedRaw<'_>) -> WrappedTree {
                let _ = text;
                unimplemented!()
            }

            fn output_parameter(text: &SourceText, output: &mut TreeSink) {
                let _ = (text, output);
            }

            fn builder_output(text: SourceText, builder: &mut TreeBuilder<UmlLanguage>) {
                let _ = (text, builder);
            }

            fn constructed(text: SourceText) -> Opaque {
                let _ = text;
                let _tree = SyntaxTree::<UmlLanguage>::new(unimplemented!());
                Opaque
            }

            fn annotated_local(raw: &str) -> Opaque {
                let _tree: Arc<SyntaxTree<UmlLanguage>> = external_factory(raw);
                Opaque
            }

            fn cast_local(raw: &str) -> Opaque {
                let _tree = external_factory(raw) as *const SyntaxTree<UmlLanguage>;
                Opaque
            }

            struct Shadow;
            impl Shadow {
                fn cached_tree_near_match(
                    &self,
                    bundle: &SourceBundle,
                    raw: &str,
                ) -> Arc<SyntaxTree<UmlLanguage>> {
                    let _ = bundle;
                    external_factory(raw)
                }
            }

            struct TreeSlot {
                tree: Arc<SyntaxTree<UmlLanguage>>,
            }
            struct TreePair<'a>(&'a mut Arc<SyntaxTree<UmlLanguage>>);
            struct TreeNamed<'a> {
                slot: &'a mut Arc<SyntaxTree<UmlLanguage>>,
            }
            impl TreeSlot {
                fn parse_into(&mut self, raw: &str) {
                    self.tree = external_factory(raw);
                }

                fn parse_alias(&mut self, raw: &str) {
                    let slot = &mut self.tree;
                    *slot = external_factory(raw);
                }

                fn parse_selected(&mut self, raw: &str, choose: bool) {
                    let slot = if choose {
                        &mut self.tree
                    } else {
                        &mut self.tree
                    };
                    *slot = external_factory(raw);
                }

                fn parse_matched(&mut self, raw: &str, choose: bool) {
                    let slot = match choose {
                        true => &mut self.tree,
                        false => &mut self.tree,
                    };
                    *slot = external_factory(raw);
                }

                fn parse_blocked(&mut self, raw: &str) {
                    let slot = { &mut self.tree };
                    *slot = external_factory(raw);
                }

                fn parse_destructured(&mut self, raw: &str) {
                    let (slot,) = (&mut self.tree,);
                    *slot = external_factory(raw);
                }

                fn parse_tuple_structured(&mut self, raw: &str) {
                    let TreePair(slot) = TreePair(&mut self.tree);
                    *slot = external_factory(raw);
                }

                fn parse_named_structured(&mut self, raw: &str) {
                    let TreeNamed { slot } = TreeNamed {
                        slot: &mut self.tree,
                    };
                    *slot = external_factory(raw);
                }

                fn parse_sliced(&mut self, raw: &str) {
                    let [slot] = [&mut self.tree];
                    *slot = external_factory(raw);
                }
            }

            fn parse_sibling(slot: &mut crate::types::sibling_types::Slot, raw: &str) {
                slot.tree = external_factory(raw);
            }

            fn parse_sibling_selected(
                left: &mut crate::types::left_types::Slot,
                right: &mut crate::types::right_types::Slot,
                raw: &str,
                choose: bool,
            ) {
                let slot = if choose {
                    &mut left.tree
                } else {
                    &mut right.tree
                };
                *slot = external_factory(raw);
            }

            struct TreeSlots {
                trees: Vec<Arc<SyntaxTree<UmlLanguage>>>,
            }
            impl TreeSlots {
                fn parse_indexed(&mut self, raw: &str) {
                    self.trees[0] = external_factory(raw);
                }
            }

            fn parse_dereferenced(slot: &mut TreeSlot, raw: &str) {
                (*slot).tree = external_factory(raw);
            }

            pub trait ShadowParser {
                fn trait_entry(text: SharedInput<'_>) -> WrappedTree;
                fn trait_raw_entry(text: SharedRaw<'_>) -> WrappedTree;

                fn trait_constructed(text: SourceText) -> Opaque {
                    let _ = text;
                    let _tree = SyntaxTree::<UmlLanguage>::new(unimplemented!());
                    Opaque
                }
            }
            "#,
        ),
        (
            "crates/waml/src/uml/lower.rs",
            r#"
            trait Reparse {
                fn reparse(&mut self, raw: &str);
            }

            struct UmlLoweringState {
                touched_islands: BTreeMap<BundlePath, Arc<SyntaxTree<UmlLanguage>>>,
            }
            impl UmlLoweringState {
                fn tree(
                    &mut self,
                    candidate: &SourceBundle,
                    target: &str,
                    op: &str,
                    parser: &mut dyn Reparse,
                ) -> Result<(BundlePath, Arc<SyntaxTree<UmlLanguage>>), EditError> {
                    let path = self
                        .path(target)
                        .cloned()
                        .ok_or_else(|| EditError::at(op, "missing concept"))?;
                    if !self.touched_islands.contains_key(&path) {
                        parser.reparse(target);
                    }
                    Ok((
                        path.clone(),
                        self.touched_islands
                            .get(&path)
                            .expect("cached tree")
                            .clone(),
                    ))
                }
            }

            mod shadow {
                struct UmlLoweringState;

                impl UmlLoweringState {
                    fn tree(
                        &self,
                        bundle: &SourceBundle,
                        raw: &str,
                    ) -> Arc<SyntaxTree<UmlLanguage>> {
                        let _ = bundle;
                        external_factory(raw)
                    }
                }
            }
            "#,
        ),
    ]);

    for expected in [
        "direct",
        "direct_raw",
        "imported_alias",
        "raw_alias",
        "output_parameter",
        "builder_output",
        "constructed",
        "annotated_local",
        "cast_local",
        "cached_tree_near_match",
        "parse_into",
        "parse_alias",
        "parse_selected",
        "parse_matched",
        "parse_blocked",
        "parse_destructured",
        "parse_tuple_structured",
        "parse_named_structured",
        "parse_sliced",
        "parse_sibling",
        "parse_sibling_selected",
        "parse_indexed",
        "parse_dereferenced",
        "trait_entry",
        "trait_raw_entry",
        "trait_constructed",
    ] {
        assert!(
            violations
                .iter()
                .any(|violation| violation.reason.contains(expected)),
            "real authority bypass `{expected}` escaped: {violations:#?}"
        );
    }
    assert!(
        violations.iter().any(|violation| {
            violation
                .reason
                .contains("waml::uml::lower::shadow::<UmlLoweringState>::tree")
        }),
        "same-file nested cache-owner collision escaped: {violations:#?}"
    );
    assert!(
        violations.iter().any(|violation| {
            violation
                .reason
                .contains("waml::uml::lower::<UmlLoweringState>::tree")
        }),
        "behavior-changing edit to the exact cache accessor escaped: {violations:#?}"
    );
}

#[test]
fn exact_cache_accessor_rejects_shadowed_format_macro() {
    let violations = analyze_sources([(
        "crates/waml/src/uml/lower.rs",
        r#"
        macro_rules! format {
            ($literal:literal) => {{
                perform_side_effect();
                String::new()
            }};
        }

        struct UmlLoweringState {
            touched_islands: BTreeMap<BundlePath, Arc<SyntaxTree<UmlLanguage>>>,
        }

        impl UmlLoweringState {
            fn tree(
                &mut self,
                candidate: &SourceBundle,
                target: &str,
                op: &str,
            ) -> Result<(BundlePath, Arc<SyntaxTree<UmlLanguage>>), EditError> {
                let path = self
                    .path(target)
                    .cloned()
                    .ok_or_else(|| EditError::at(op, format!("missing '{target}'")))?;
                if !self.touched_islands.contains_key(&path) {
                    self.reparse(candidate, &path, op)?;
                }
                Ok((
                    path.clone(),
                    self.touched_islands
                        .get(&path)
                        .expect("cached tree")
                        .clone(),
                ))
            }
        }
        "#,
    )]);

    assert!(
        violations.iter().any(|violation| {
            violation
                .reason
                .contains("waml::uml::lower::<UmlLoweringState>::tree")
        }),
        "shadowed `format!` kept the exact cache exception: {violations:#?}"
    );
}

#[test]
fn closure_and_call_indirection_are_rejected() {
    let violations = reasons(
        r#"
        fn concealed(input: &str) -> LayoutStatement {
            let tokenize = |text: &str| text.split(',').collect::<Vec<_>>();
            let _tokens = tokenize(input);
            unimplemented!()
        }

        fn route_through_helper(input: &str) -> LayoutStatement {
            concealed(input)
        }
        "#,
    );

    assert!(
        violations.iter().any(|reason| {
            reason.contains("concealed") && reason.contains("defines raw-text grammar")
        }),
        "{violations:#?}"
    );
    assert!(
        violations
            .iter()
            .any(|reason| reason.contains("route_through_helper")),
        "{violations:#?}"
    );
}

#[test]
fn standalone_raw_authority_closures_are_rejected() {
    let violations = reasons(
        r#"
        fn install() {
            let parser = |raw: &str| -> Arc<SyntaxTree<UmlLanguage>> {
                let _ = raw;
                unimplemented!()
            };
            let _ = parser;
        }
        "#,
    );

    assert!(
        violations
            .iter()
            .any(|reason| { reason.contains("install") && reason.contains("closure") }),
        "standalone raw authority closure escaped: {violations:#?}"
    );
}

#[test]
fn call_edges_propagate_reparse_through_resolved_and_unresolved_dispatch() {
    let violations = reasons(
        r#"
        fn raw_authority(text: SourceText) -> Arc<SyntaxTree<UmlLanguage>> {
            let _ = text;
            unimplemented!()
        }

        fn free_dispatch(raw: String) -> Analysis {
            let _tree = raw_authority(SourceText::from(raw));
            Analysis
        }

        struct Decoder;
        impl Decoder {
            fn decode(&self, raw: String) -> Analysis {
                let _tree = raw_authority(SourceText::from(raw));
                Analysis
            }
        }

        trait Decode {
            fn route(&self, raw: String) -> Analysis;
        }
        impl Decode for Decoder {
            fn route(&self, raw: String) -> Analysis {
                let _tree = raw_authority(SourceText::from(raw));
                Analysis
            }
        }

        type Parser = fn(String) -> Analysis;
        struct Services {
            decoder: Decoder,
            callable: Parser,
        }
        impl Services {
            fn decoder(&self) -> &Decoder {
                &self.decoder
            }
        }

        struct Renderer;
        impl Renderer {
            fn render(&self, model: &Model) -> String {
                model.to_string()
            }
        }
        struct RenderServices {
            renderer: Renderer,
        }
        impl RenderServices {
            fn renderer(&self) -> &Renderer {
                &self.renderer
            }
        }

        fn render_model(model: &Model) -> String {
            model.to_string()
        }

        fn function_pointer(model: &Model) -> Analysis {
            let rendered = model.to_string();
            let callable: Parser = free_dispatch;
            callable(rendered)
        }

        fn field_receiver(services: &Services, model: &Model) -> Analysis {
            let rendered = model.to_string();
            services.decoder.decode(rendered)
        }

        fn trait_receiver(decoder: &dyn Decode, model: &Model) -> Analysis {
            let rendered = model.to_string();
            decoder.route(rendered)
        }

        fn callable_field(services: &Services, model: &Model) -> Analysis {
            let rendered = model.to_string();
            (services.callable)(rendered)
        }

        fn chained_dispatch(services: &Services, model: &Model) -> Analysis {
            let rendered = model.to_string();
            services.decoder().decode(rendered)
        }

        fn helper_return_pointer(model: &Model, callable: Parser) -> Analysis {
            let rendered = render_model(model);
            callable(rendered)
        }

        fn helper_method_pointer(
            renderer: &Renderer,
            model: &Model,
            callable: Parser,
        ) -> Analysis {
            let rendered = renderer.render(model);
            callable(rendered)
        }

        fn helper_chain_pointer(
            services: &RenderServices,
            model: &Model,
            callable: Parser,
        ) -> Analysis {
            let rendered = services.renderer().render(model);
            callable(rendered)
        }

        struct Vec;
        impl Vec {
            fn push(&self, raw: String) -> Analysis {
                free_dispatch(raw)
            }
        }

        fn custom_vec_push(sink: &Vec, model: &Model) -> Analysis {
            sink.push(render_model(model))
        }

        mod unresolved_vec_spoof {
            enum Vec {
                Sink,
            }
            impl external::Push for Vec {}

            fn unresolved_vec_push(sink: &Vec, model: &Model) {
                let rendered = model.to_string();
                sink.push(rendered);
            }
        }

        fn unrelated_callable(callable: fn(&str) -> String, label: &str) -> String {
            callable(label)
        }

        fn unrelated_domain_helper(model: &Model) -> Analysis {
            inspect_model(model)
        }
        "#,
    );

    for expected in [
        "function_pointer",
        "field_receiver",
        "trait_receiver",
        "chained_dispatch",
    ] {
        let matching = violations
            .iter()
            .filter(|reason| reason.contains(expected))
            .collect::<Vec<_>>();
        assert!(
            !matching.is_empty(),
            "dispatch bypass `{expected}` escaped: {violations:#?}"
        );
        assert!(
            matching
                .iter()
                .any(|reason| reason.contains("model-to-source reparse")),
            "`{expected}` was not rejected through resolved call-edge capability propagation: {matching:#?}"
        );
    }
    let callable_field = violations
        .iter()
        .filter(|reason| reason.contains("callable_field"))
        .collect::<Vec<_>>();
    assert!(
        callable_field
            .iter()
            .any(|reason| reason.contains("unresolved callable dispatch")),
        "callable-field model text was not rejected through the unresolved-call edge: {callable_field:#?}"
    );
    for expected in [
        "helper_return_pointer",
        "helper_method_pointer",
        "helper_chain_pointer",
    ] {
        assert!(
            violations.iter().any(|reason| {
                reason.contains(expected) && reason.contains("unresolved callable dispatch")
            }),
            "helper-return model text escaped unresolved dispatch in `{expected}`: {violations:#?}"
        );
    }
    assert!(
        violations.iter().any(|reason| {
            reason.contains("custom_vec_push") && reason.contains("model-to-source reparse")
        }),
        "user-defined `Vec::push` was mistaken for a harmless standard collection call: {violations:#?}"
    );
    assert!(
        violations.iter().any(|reason| {
            reason.contains("unresolved_vec_push")
                && reason.contains("unresolved callable dispatch")
        }),
        "unresolved enum `Vec::push` was mistaken for a harmless standard collection call: {violations:#?}"
    );
    for control in ["unrelated_callable", "unrelated_domain_helper"] {
        assert!(
            violations.iter().all(|reason| !reason.contains(control)),
            "legitimate control `{control}` was rejected: {violations:#?}"
        );
    }
}

#[test]
fn benign_model_inspection_does_not_taint_unrelated_reparse() {
    let violations = reasons(
        r#"
        fn inspect_and_parse_constant(model: &Model) -> Analysis {
            let _count = model.nodes.len();
            crate::analysis::prepare_candidate("constant")
        }

        fn standard_collection_controls(model: &Model) {
            let mut std_rows = std::vec::Vec::new();
            std_rows.push(model.to_string());

            let mut alloc_rows = alloc::vec::Vec::new();
            alloc_rows.push(model.to_string());

            let present: std::collections::HashSet<String> = Default::default();
            let _ = present.contains(&model.to_string());
        }
        "#,
    );

    assert!(
        violations.is_empty(),
        "benign model inspection was treated as serialization: {violations:#?}"
    );
}

#[test]
fn structural_call_bypasses_are_rejected_conservatively() {
    let violations = reasons(
        r#"
        fn assemble(raw: &str) -> LayoutStatement {
            let _ = raw;
            LayoutStatement { parts: Vec::new() }
        }

        fn decode(raw: &str) -> LayoutStatement {
            let _parts = raw.split_whitespace().collect::<Vec<_>>();
            unimplemented!()
        }

        fn callable_local(raw: &str) -> LayoutStatement {
            let callable: fn(&str) -> LayoutStatement = decode;
            callable(raw)
        }

        trait Decoder {
            fn dispatch(&self, raw: &str) -> LayoutStatement;
        }

        struct Holder;
        impl Decoder for Holder {
            fn dispatch(&self, raw: &str) -> LayoutStatement {
                decode(raw)
            }
        }

        struct Services {
            decoder: Holder,
        }

        fn field_receiver(services: &Services, raw: &str) -> LayoutStatement {
            services.decoder.dispatch(raw)
        }

        fn trait_receiver(decoder: &dyn Decoder, raw: &str) -> LayoutStatement {
            decoder.dispatch(raw)
        }

        fn qualified_reparse(model: &Model, structure: &MarkdownStructureMap) -> Analysis {
            let rendered = model.to_string();
            let text = SourceText::from(rendered);
            crate::uml::syntax::parser::parse(text, structure);
            unimplemented!()
        }
        "#,
    );

    for expected in [
        "assemble",
        "callable_local",
        "field_receiver",
        "trait_receiver",
        "qualified_reparse",
    ] {
        assert!(
            violations.iter().any(|reason| reason.contains(expected)),
            "missing structural bypass {expected}: {violations:#?}"
        );
    }
}

#[test]
fn macro_generated_shadow_authority_is_rejected() {
    let violations = reasons(
        r#"
        macro_rules! shadow_authority {
            () => {
                fn generated(raw: &str) -> LayoutStatement {
                    let _parts = raw.split_whitespace().collect::<Vec<_>>();
                    unimplemented!()
                }
            };
        }
        shadow_authority!();
        "#,
    );

    assert!(
        violations
            .iter()
            .any(|reason| reason.contains("opaque macro")),
        "{violations:#?}"
    );
}

#[test]
fn body_macros_and_external_proc_macros_cannot_expand_authority() {
    let body_macro_violations = reasons(
        r#"
        fn statement_macro(text: SourceText) -> Opaque {
            external_authority!(text);
            Opaque
        }

        fn expression_macro(text: SourceText) -> Opaque {
            let _expanded = external_authority!(text);
            Opaque
        }

        macro_rules! expands_real_authority {
            () => {
                fn generated(text: SourceText) -> Arc<SyntaxTree<UmlLanguage>> {
                    let _ = text;
                    unimplemented!()
                }
            };
        }
        expands_real_authority!();
        "#,
    );
    for expected in [
        "statement_macro",
        "expression_macro",
        "expands_real_authority",
    ] {
        assert!(
            body_macro_violations
                .iter()
                .any(|reason| reason.contains(expected)),
            "body/macro-expanded authority `{expected}` escaped: {body_macro_violations:#?}"
        );
    }

    let proc_macro_violations = analyze_sources([
        (
            "crates/waml/src/uml/syntax/attribute_generated.rs",
            r#"
            #[external::authority]
            fn generated(text: SourceText) -> Arc<SyntaxTree<UmlLanguage>> {
                let _ = text;
                unimplemented!()
            }
            "#,
        ),
        (
            "crates/waml/src/uml/syntax/derive_generated.rs",
            r#"
            #[derive(Clone, external::Authority)]
            struct Generated;
            "#,
        ),
        (
            "crates/waml/src/uml/syntax/function_generated.rs",
            r#"
            external::generate_authority! {
                fn generated(text: SourceText) -> Arc<SyntaxTree<UmlLanguage>>;
            }

            fn host(text: SourceText) {
                let _ = external::parse_authority!(text);
            }
            "#,
        ),
        (
            "crates/waml/src/uml/analysis_generated.rs",
            r#"
            #[external::compat_authority]
            fn ordinary_helper() {}

            external::generate_hidden_authority!();
            "#,
        ),
    ]);
    for expected in [
        "external::authority",
        "external::Authority",
        "external::generate_authority",
        "external::parse_authority",
        "external::compat_authority",
        "external::generate_hidden_authority",
    ] {
        assert!(
            proc_macro_violations
                .iter()
                .any(|violation| violation.reason.contains(expected)),
            "external authority proc macro `{expected}` escaped: {proc_macro_violations:#?}"
        );
    }

    let harmless = analyze_sources([(
        "crates/waml/src/uml/syntax/harmless.rs",
        r#"
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        struct Harmless;

        #[allow(dead_code)]
        fn helper() {}
        "#,
    )]);
    assert!(
        harmless.is_empty(),
        "harmless authority attributes were rejected: {harmless:#?}"
    );
}

#[test]
fn qualified_duplicate_names_cross_file_and_method_calls_do_not_hide_edges() {
    let sources = [
        (
            "crates/waml/src/codec.rs",
            r#"
            type Tree = LayoutStatement;
            struct Decoder;
            impl Decoder {
                fn decode(&self, input: &str) -> Tree {
                    let _parts = input.split_whitespace().collect::<Vec<_>>();
                    unimplemented!()
                }
            }
            fn route(decoder: &Decoder, input: &[u8]) -> Tree {
                let _ = input;
                decoder.decode("")
            }
            fn associated(input: &[u8]) -> Tree {
                let _ = input;
                Decoder::decode(&Decoder, "")
            }
            fn local_receiver(input: &[u8]) -> Tree {
                let _ = input;
                let decoder = Decoder;
                decoder.decode("")
            }
            fn hidden(input: &str) -> Tree {
                let _parts = input.split(',').collect::<Vec<_>>();
                unimplemented!()
            }
            "#,
        ),
        (
            "crates/waml/src/other.rs",
            r#"
            fn decode(input: &str) -> String {
                input.trim().to_owned()
            }
            "#,
        ),
        (
            "crates/waml/src/entry.rs",
            r#"
            use crate::codec::hidden as imported;
            fn enter(decoder: &Decoder, input: &[u8]) -> Tree {
                crate::codec::route(decoder, input)
            }
            fn imported_entry(input: &[u8]) -> Tree {
                let _ = input;
                imported("")
            }
            "#,
        ),
    ];
    let violations = analyze_sources(sources);

    assert!(
        violations.iter().any(|violation| {
            violation.reason.contains("waml::codec::<Decoder>::decode")
                && violation.reason.contains("raw-text grammar")
        }),
        "{violations:#?}"
    );
    assert!(
        violations.iter().any(|violation| {
            violation.reason.contains("waml::entry::enter")
                && violation.reason.contains("waml::codec::route")
        }),
        "{violations:#?}"
    );
    for caller in ["associated", "local_receiver", "imported_entry"] {
        assert!(
            violations
                .iter()
                .any(|violation| violation.reason.contains(caller)),
            "missing call edge for {caller}: {violations:#?}"
        );
    }
    assert!(
        violations
            .iter()
            .all(|violation| !violation.reason.contains("waml::other::decode")),
        "{violations:#?}"
    );
}

#[test]
fn deleted_module_paths_and_model_source_reparse_are_rejected() {
    let deleted_import = reasons(
        r#"
        use crate::grammar::DocumentParser;
        "#,
    );
    let deleted_module =
        analyze_sources([("crates/waml/src/lib.rs", "pub(super) mod serialize {}")]);
    let model_reparse = reasons(
        r#"
        fn rebuild(model: &Model) -> Analysis {
            let authored = model.render();
            parse_document(&authored)
        }
        "#,
    );

    assert!(
        deleted_import
            .iter()
            .any(|reason| reason.contains("deleted authority path")),
        "{deleted_import:#?}"
    );
    assert!(
        deleted_module
            .iter()
            .any(|violation| violation.reason.contains("deleted authority path")),
        "{deleted_module:#?}"
    );
    assert!(
        model_reparse
            .iter()
            .any(|reason| reason.contains("model-to-source reparse")),
        "{model_reparse:#?}"
    );
}

#[test]
fn split_model_serialization_and_reparse_capabilities_are_propagated() {
    let violations = reasons(
        r#"
        fn authored(model: &Model) -> String {
            model.render()
        }
        fn decode(authored: &str) -> Analysis {
            parse_document(authored)
        }
        fn rebuild(model: &Model) -> Analysis {
            let source = authored(model);
            decode(&source)
        }
        "#,
    );

    assert!(
        violations.iter().any(|reason| {
            reason.contains("waml::compat::rebuild") && reason.contains("model-to-source reparse")
        }),
        "{violations:#?}"
    );
}

#[test]
fn visible_model_to_source_capability_is_rejected_but_private_rendering_is_allowed() {
    let visible = reasons(
        r#"
        pub(super) fn export_model(model: &Model) -> String {
            model.to_string()
        }
        "#,
    );
    let private = reasons(
        r#"
        fn render_label(model: &Model) -> String {
            model.to_string()
        }
        "#,
    );

    assert!(
        visible
            .iter()
            .any(|reason| reason.contains("visible model-to-source capability")),
        "{visible:#?}"
    );
    assert!(private.is_empty(), "{private:#?}");
}

#[test]
fn visible_semantic_model_self_and_concrete_owner_surfaces_are_rejected() {
    let violations = reasons(
        r#"
        impl Model {
            pub fn export(&self) -> String {
                String::new()
            }
        }
        impl Diagram {
            pub(super) fn export_diagram(&self) -> Box<str> {
                unimplemented!()
            }
            fn private_label(&self) -> String {
                String::new()
            }
        }
        impl Node {
            pub fn export_node(&self) -> Arc<String> {
                unimplemented!()
            }
        }
        impl Edge {
            pub fn export_edge(&self) -> Result<String, Error> {
                unimplemented!()
            }
        }
        impl Attribute {
            pub fn export_attribute(&self) -> String {
                String::new()
            }
        }
        impl Slot {
            pub fn export_slot(&self) -> String {
                String::new()
            }
        }
        impl ActivityNode {
            pub fn export_activity(&self) -> String {
                String::new()
            }
        }
        impl FlowEdge {
            pub fn export_flow_edge(&self) -> String {
                String::new()
            }
        }
        impl SequenceDoc {
            pub fn export_sequence(&self) -> String {
                String::new()
            }
        }
        impl RelEnd {
            pub fn export_relation_end(&self) -> String {
                String::new()
            }
        }

        pub fn export_typed_diagram(diagram: &Diagram) -> String {
            let _ = diagram;
            String::new()
        }

        impl std::string::ToString for Model {
            fn to_string(&self) -> String {
                String::new()
            }
        }

        pub trait ExportModel {
            fn export_trait(&self) -> String;
        }
        impl ExportModel for Model {
            fn export_trait(&self) -> String {
                String::new()
            }
        }

        pub trait DefaultExportModel {
            fn default_export_trait(&self) -> String {
                String::new()
            }
        }
        impl DefaultExportModel for Model {}
        "#,
    );

    for expected in [
        "export",
        "export_diagram",
        "export_node",
        "export_edge",
        "export_attribute",
        "export_slot",
        "export_activity",
        "export_flow_edge",
        "export_sequence",
        "export_relation_end",
        "export_typed_diagram",
        "export_trait",
        "default_export_trait",
    ] {
        assert!(
            violations.iter().any(|reason| {
                reason.contains(expected) && reason.contains("visible model-to-source capability")
            }),
            "visible semantic owner `{expected}` escaped: {violations:#?}"
        );
    }
    for control in ["private_label", "to_string"] {
        assert!(
            violations.iter().all(|reason| !reason.contains(control)),
            "legitimate semantic text control `{control}` was rejected: {violations:#?}"
        );
    }
}

#[test]
fn imported_external_to_string_is_not_allowlisted() {
    let violations = reasons(
        r#"
        use external::ToString;

        impl ToString for Model {
            fn to_string(&self) -> String {
                export_model_source(self)
            }
        }
        "#,
    );

    assert!(
        violations.iter().any(|reason| {
            reason.contains("to_string") && reason.contains("visible model-to-source capability")
        }),
        "imported external `ToString` escaped the visible serializer guard: {violations:#?}"
    );
}

#[test]
fn local_standard_root_names_do_not_acquire_external_trust() {
    let alloc_violations = reasons(
        r#"
        mod alloc {
            pub mod vec {
                pub enum Vec {
                    Sink,
                }
            }
            pub mod string {
                pub trait ToString {
                    fn to_string(&self) -> String;
                }
            }
        }

        impl external::Push for alloc::vec::Vec {}
        impl alloc::string::ToString for Model {
            fn to_string(&self) -> String {
                export_model_source(self)
            }
        }

        fn alloc_vec_leak(sink: &alloc::vec::Vec, model: &Model) {
            let rendered = model.to_string();
            sink.push(rendered);
        }
        "#,
    );
    let std_violations = reasons(
        r#"
        #![no_std]

        mod std {
            pub mod vec {
                pub enum Vec {
                    Sink,
                }
            }
            pub mod string {
                pub trait ToString {
                    fn to_string(&self) -> String;
                }
            }
        }

        impl external::Push for std::vec::Vec {}
        impl std::string::ToString for Model {
            fn to_string(&self) -> String {
                export_model_source(self)
            }
        }

        fn std_vec_leak(sink: &std::vec::Vec, model: &Model) {
            let rendered = model.to_string();
            sink.push(rendered);
        }
        "#,
    );
    let extern_alias_violations = reasons(
        r#"
        extern crate external as alloc;

        impl external::Push for alloc::vec::Vec {}
        impl alloc::string::ToString for Model {
            fn to_string(&self) -> String {
                export_model_source(self)
            }
        }

        fn extern_alias_vec_leak(sink: &alloc::vec::Vec, model: &Model) {
            let rendered = model.to_string();
            sink.push(rendered);
        }
        "#,
    );
    let block_local_violations = reasons(
        r#"
        struct Model;
        impl Model {
            fn to_string(&self) -> String {
                String::new()
            }
        }

        fn block_local_vec_leak(model: &Model) {
            mod alloc {
                pub mod vec {
                    pub struct Vec;
                    impl Vec {
                        pub fn push(&self, _: String) {}
                    }
                }
            }

            let sink: alloc::vec::Vec = alloc::vec::Vec;
            sink.push(model.to_string());
        }
        "#,
    );

    for (root, violations) in [
        ("alloc", alloc_violations),
        ("std", std_violations),
        ("extern_alias", extern_alias_violations),
    ] {
        assert!(
            violations.iter().any(|reason| {
                reason.contains(&format!("{root}_vec_leak"))
                    && reason.contains("unresolved callable dispatch")
            }),
            "local `{root}` collection root acquired external trust: {violations:#?}"
        );
        assert!(
            violations.iter().any(|reason| {
                reason.contains("to_string")
                    && reason.contains("visible model-to-source capability")
            }),
            "local `{root}::string::ToString` acquired external trust: {violations:#?}"
        );
    }
    assert!(
        block_local_violations.iter().any(|reason| {
            reason.contains("block_local_vec_leak")
                && reason.contains("unresolved callable dispatch")
        }),
        "block-local `alloc` collection root acquired external trust: {block_local_violations:#?}"
    );
}

#[test]
fn imported_and_qualified_arbitrary_aliases_are_rejected() {
    let violations = analyze_sources([
        ("crates/waml/src/types.rs", "type Tree = LayoutStatement;"),
        (
            "crates/waml/src/alias_entry.rs",
            r#"
            use crate::types::Tree;
            fn imported(input: &str) -> Tree {
                let _parts = input.split(',').collect::<Vec<_>>();
                unimplemented!()
            }
            fn qualified(input: &str) -> crate::types::Tree {
                let _parts = input.split_whitespace().collect::<Vec<_>>();
                unimplemented!()
            }
            "#,
        ),
    ]);

    for function in ["imported", "qualified"] {
        assert!(
            violations
                .iter()
                .any(|violation| violation.reason.contains(function)),
            "missing alias violation for {function}: {violations:#?}"
        );
    }
}

#[test]
fn allowlist_is_qualified_not_inherited_by_nested_same_name() {
    let violations = analyze_sources([(
        "crates/waml-editor/src/cli.rs",
        r#"
        mod shadow {
            fn parse_hex(input: &str) -> LayoutStatement {
                let _parts = input.split(',').collect::<Vec<_>>();
                unimplemented!()
            }
        }
        "#,
    )]);

    assert!(
        violations
            .iter()
            .any(|violation| violation.reason.contains("shadow::parse_hex")),
        "{violations:#?}"
    );
}

#[test]
fn cfg_test_shadow_authority_is_not_a_production_violation() {
    let violations = reasons(
        r#"
        #[cfg(test)]
        mod tests {
            fn fake(input: &str) -> LayoutStatement {
                let _parts = input.split_whitespace().collect::<Vec<_>>();
                unimplemented!()
            }
        }
        "#,
    );

    assert!(violations.is_empty(), "{violations:#?}");
}

#[test]
fn legitimate_raw_text_helper_is_accepted() {
    let violations = reasons(
        r#"
        fn render_label(label: &str) -> String {
            label.trim().to_uppercase()
        }
        "#,
    );

    assert!(violations.is_empty(), "{violations:#?}");
}

#[test]
fn legitimate_domain_and_text_helpers_are_not_name_false_positives() {
    let violations = reasons(
        r#"
        struct Label(String);
        struct Analysis;
        struct Model;

        impl std::string::ToString for Model {
            fn to_string(&self) -> String {
                String::new()
            }
        }

        fn label(input: &str) -> Label {
            Label(input.trim().to_owned())
        }

        fn render(model: &Model) -> String {
            model.to_string()
        }

        fn analyze(input: &str) -> Analysis {
            let _trimmed = input.trim();
            Analysis
        }

        fn trim(input: &str) -> String {
            input.trim().to_owned()
        }

        fn to_string(input: &str) -> String {
            input.to_string()
        }
        "#,
    );

    assert!(violations.is_empty(), "{violations:#?}");
}
