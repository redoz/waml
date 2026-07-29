#[path = "support/authority_guard.rs"]
mod authority_guard;

use std::path::PathBuf;

use authority_guard::{analyze_sources, analyze_workspace};

fn reasons(source: &str) -> Vec<String> {
    analyze_sources([("crates/waml/src/compat.rs", source)])
        .into_iter()
        .map(|violation| violation.reason)
        .collect()
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
