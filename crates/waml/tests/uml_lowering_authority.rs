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
            concat!("parse_", "document"),
            concat!("serialize_", "document"),
            "crate::layout::Document",
            concat!("Line", "<"),
            "uml::ops::lower_one",
        ] {
            assert!(
                !source.contains(prohibited),
                "{path} retains prohibited UML lowering authority `{prohibited}`"
            );
        }
    }
}
