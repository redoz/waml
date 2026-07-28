#[test]
fn uml_lowering_call_path_has_no_legacy_parser_or_serializer_authority() {
    for (path, source) in [
        (
            "uml/lower.rs",
            include_str!("../src/uml/lower.rs") as &'static str,
        ),
        ("uml/ops.rs", include_str!("../src/uml/ops.rs")),
        ("uml/rename.rs", include_str!("../src/uml/rename.rs")),
        ("compat.rs", include_str!("../src/compat.rs")),
    ] {
        for prohibited in [
            "parse_document",
            "serialize_document",
            "crate::syntax::Document",
            "Line<",
            "uml::ops::lower_one",
        ] {
            assert!(
                !source.contains(prohibited),
                "{path} retains prohibited UML lowering authority `{prohibited}`"
            );
        }
    }
}
