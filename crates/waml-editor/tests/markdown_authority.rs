use std::{fs, path::Path};

use quote::ToTokens;
use syn::{
    visit_mut::{self, VisitMut},
    Attribute, ForeignItem, ImplItem, Item, TraitItem,
};

const FORBIDDEN_MARKDOWN_AUTHORITIES: &[&str] = &[
    "MarkdownRef",
    "MarkdownAction",
    "as_markdown (",
    "makepad_widgets :: Markdown",
    "pulldown_cmark ::",
    "regex ::",
];

fn forbidden_markdown_authorities(source: &str) -> Vec<&'static str> {
    FORBIDDEN_MARKDOWN_AUTHORITIES
        .iter()
        .copied()
        .filter(|forbidden| source.contains(forbidden))
        .collect()
}

fn production_source(contents: &str) -> String {
    let mut file = syn::parse_file(contents).expect("production Rust source must parse");
    StripTestItems.visit_file_mut(&mut file);
    file.into_token_stream().to_string()
}

fn markdown_editor_mount_count(source: &str) -> usize {
    const WIDGET: &str = "MarkdownEditor";
    source
        .match_indices(WIDGET)
        .filter(|(start, _)| {
            source[..*start].trim_end().ends_with(":=")
                && source[*start + WIDGET.len()..]
                    .chars()
                    .next()
                    .map_or(true, |next| !(next.is_ascii_alphanumeric() || next == '_'))
        })
        .count()
}

struct StripTestItems;

impl VisitMut for StripTestItems {
    fn visit_file_mut(&mut self, file: &mut syn::File) {
        file.items
            .retain(|item| !has_cfg_test(item_attributes(item)));
        visit_mut::visit_file_mut(self, file);
    }

    fn visit_item_mod_mut(&mut self, item: &mut syn::ItemMod) {
        if let Some((_, items)) = &mut item.content {
            items.retain(|nested| !has_cfg_test(item_attributes(nested)));
        }
        visit_mut::visit_item_mod_mut(self, item);
    }

    fn visit_item_impl_mut(&mut self, item: &mut syn::ItemImpl) {
        item.items
            .retain(|nested| !has_cfg_test(impl_item_attributes(nested)));
        visit_mut::visit_item_impl_mut(self, item);
    }

    fn visit_item_trait_mut(&mut self, item: &mut syn::ItemTrait) {
        item.items
            .retain(|nested| !has_cfg_test(trait_item_attributes(nested)));
        visit_mut::visit_item_trait_mut(self, item);
    }

    fn visit_item_foreign_mod_mut(&mut self, item: &mut syn::ItemForeignMod) {
        item.items
            .retain(|nested| !has_cfg_test(foreign_item_attributes(nested)));
        visit_mut::visit_item_foreign_mod_mut(self, item);
    }

    fn visit_block_mut(&mut self, block: &mut syn::Block) {
        block.stmts.retain(|statement| {
            !matches!(statement, syn::Stmt::Item(item) if has_cfg_test(item_attributes(item)))
        });
        visit_mut::visit_block_mut(self, block);
    }
}

fn has_cfg_test(attributes: &[Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute.path().is_ident("cfg")
            && attribute
                .parse_args::<syn::Path>()
                .is_ok_and(|path| path.is_ident("test"))
    })
}

fn item_attributes(item: &Item) -> &[Attribute] {
    match item {
        Item::Const(item) => &item.attrs,
        Item::Enum(item) => &item.attrs,
        Item::ExternCrate(item) => &item.attrs,
        Item::Fn(item) => &item.attrs,
        Item::ForeignMod(item) => &item.attrs,
        Item::Impl(item) => &item.attrs,
        Item::Macro(item) => &item.attrs,
        Item::Mod(item) => &item.attrs,
        Item::Static(item) => &item.attrs,
        Item::Struct(item) => &item.attrs,
        Item::Trait(item) => &item.attrs,
        Item::TraitAlias(item) => &item.attrs,
        Item::Type(item) => &item.attrs,
        Item::Union(item) => &item.attrs,
        Item::Use(item) => &item.attrs,
        _ => &[],
    }
}

fn impl_item_attributes(item: &ImplItem) -> &[Attribute] {
    match item {
        ImplItem::Const(item) => &item.attrs,
        ImplItem::Fn(item) => &item.attrs,
        ImplItem::Macro(item) => &item.attrs,
        ImplItem::Type(item) => &item.attrs,
        _ => &[],
    }
}

fn trait_item_attributes(item: &TraitItem) -> &[Attribute] {
    match item {
        TraitItem::Const(item) => &item.attrs,
        TraitItem::Fn(item) => &item.attrs,
        TraitItem::Macro(item) => &item.attrs,
        TraitItem::Type(item) => &item.attrs,
        _ => &[],
    }
}

fn foreign_item_attributes(item: &ForeignItem) -> &[Attribute] {
    match item {
        ForeignItem::Fn(item) => &item.attrs,
        ForeignItem::Macro(item) => &item.attrs,
        ForeignItem::Static(item) => &item.attrs,
        ForeignItem::Type(item) => &item.attrs,
        _ => &[],
    }
}

fn production_rust_sources(root: &Path) -> String {
    let mut paths = fs::read_dir(root)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", root.display()))
        .map(|entry| entry.expect("production source directory entry"))
        .collect::<Vec<_>>();
    paths.sort_by_key(|entry| entry.path());

    let mut source = String::new();
    for entry in paths {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "tests") {
                continue;
            }
            source.push_str(&production_rust_sources(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            source.push_str(&format!("\n// FILE: {}\n", path.display()));
            let contents = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
            source.push_str(&production_source(&contents));
        }
    }
    source
}

#[test]
fn production_editor_has_one_markdown_authority() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let editor_root = manifest_dir.join("src");
    let markdown_editor_root = manifest_dir.join("../waml-markdown-editor/src");

    for (crate_name, root) in [
        ("waml-editor", editor_root.as_path()),
        ("waml-markdown-editor", markdown_editor_root.as_path()),
    ] {
        let source = production_rust_sources(root);
        let forbidden = forbidden_markdown_authorities(&source);
        assert!(
            forbidden.is_empty(),
            "{crate_name} production source contains forbidden Markdown authorities: {forbidden:?}"
        );
    }

    assert!(!editor_root.join("markdown_surface.rs").exists());
}

#[test]
fn production_scan_excludes_nested_tests_directories() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "waml-markdown-authority-{}-{unique}",
        std::process::id()
    ));
    let feature_root = root.join("feature");
    let tests_root = feature_root.join("tests");
    fs::create_dir_all(&tests_root).expect("nested test source directory");
    fs::write(
        feature_root.join("production.rs"),
        "fn neighboring_production_item() {}",
    )
    .expect("neighboring production source");
    fs::write(
        tests_root.join("navigation.rs"),
        "fn test_only_item() { let _: MarkdownAction; }",
    )
    .expect("nested test source");

    let source = production_rust_sources(&root);
    fs::remove_dir_all(&root).expect("temporary source tree cleanup");

    assert!(source.contains("neighboring_production_item"));
    assert!(!source.contains("test_only_item"));
    assert!(!source.contains("MarkdownAction"));
}

#[test]
fn one_shared_waml_widget_serves_source_and_read_only_generic_views() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let app_source =
        fs::read_to_string(manifest_dir.join("src/app.rs")).expect("application source");
    let source_view_source =
        fs::read_to_string(manifest_dir.join("src/source_view.rs")).expect("source view source");
    let generic_source = fs::read_to_string(manifest_dir.join("src/generic_okf_view.rs"))
        .expect("generic OKF view source");
    let app = production_source(&app_source);
    let source = production_source(&source_view_source);
    let generic = production_source(&generic_source);

    assert_eq!(markdown_editor_mount_count(&app), 1);
    assert!(source.contains("session : MarkdownDocumentSession"));
    assert!(generic.contains("source : SourceView"));
    assert!(generic.contains("SourceView :: new_read_only"));
    assert!(generic.contains("set_read_only"));
    assert!(generic.contains("outcome . source_edit = None ;"));
    assert_eq!(generic.matches("source_edit").count(), 1);
}

#[test]
fn authority_policy_rejects_legacy_api_variants() {
    let source = r##"
        use pulldown_cmark :: Options ;
        use regex :: Captures ;
        fn legacy () { let widget : MarkdownRef = scope . as_markdown (cx) ; }
    "##;

    let forbidden = forbidden_markdown_authorities(source);

    assert!(forbidden.contains(&"pulldown_cmark ::"));
    assert!(forbidden.contains(&"regex ::"));
    assert!(forbidden.contains(&"MarkdownRef"));
    assert!(forbidden.contains(&"as_markdown ("));
}

#[test]
fn production_scan_keeps_items_after_test_only_helpers() {
    let source = r##"
        #[cfg(test)]
        mod viewport_tests { use regex::Captures; }
        fn first_production_item() {}
        #[cfg(test)]
        fn helper() { let _: MarkdownRef; }
        fn second_production_item() {}
        #[cfg(test)]
        mod syntax_facade_tests { use pulldown_cmark::Parser; }
        fn third_production_item() {}
        const ATTRIBUTE_TEXT: &str = "#[cfg(test)] fn not_an_item() {}";
        #[allow(dead_code)] #[cfg(test)] fn same_line_test_helper() { let _: MarkdownAction; }
        fn fourth_production_item() {}
        #[cfg(test)]
        fn helper() -> Foo<{ 1 }> { let _: MarkdownRef; }
        fn production_after_const_generic_helper() {}
        #[cfg(test)]
        const LESS: bool = 1 < 2;
        fn production_after_comparison_const() {}
    "##;

    let production = production_source(source);

    assert!(production.contains("first_production_item"));
    assert!(production.contains("second_production_item"));
    assert!(production.contains("third_production_item"));
    assert!(production.contains("ATTRIBUTE_TEXT"));
    assert!(production.contains("fourth_production_item"));
    assert!(production.contains("production_after_const_generic_helper"));
    assert!(production.contains("production_after_comparison_const"));
    assert!(!production.contains("viewport_tests"));
    assert!(!production.contains("MarkdownRef"));
    assert!(!production.contains("syntax_facade_tests"));
    assert!(!production.contains("same_line_test_helper"));
    assert!(!production.contains("fn helper() -> Foo<{ 1 }>"));
    assert!(!production.contains("const LESS"));
}

#[test]
fn markdown_editor_mount_count_ignores_field_names_and_whitespace() {
    let source = r#"
        primary := MarkdownEditor{}
        secondary:=MarkdownEditor {}
        tertiary :=
            MarkdownEditor{ }
    "#;

    assert_eq!(markdown_editor_mount_count(source), 3);
}
