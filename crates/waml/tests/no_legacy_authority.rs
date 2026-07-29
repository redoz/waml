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
        violations
            .iter()
            .any(|reason| reason.contains("`concealed` defines raw-text grammar")),
        "{violations:#?}"
    );
    assert!(
        violations.iter().any(|reason| {
            reason.contains("`route_through_helper` reaches shadow authority `concealed`")
        }),
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
