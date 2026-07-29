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
        "packages/core/source/nested.rs",
        "packages/core/shared/path_module.rs",
        "packages/core/shared/included.rs",
        "packages/core/targets/custom_example.rs",
        "packages/core/tools/custom_build.rs",
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
        violations.iter().any(|violation| {
            violation
                .reason
                .contains("forbidden workspace dependency direction")
        }),
        "workspace dependency direction was not enforced: {violations:#?}"
    );
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

        impl ToString for Model {
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
