use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Attribute, FnArg, ItemFn, Pat, ReturnType};

#[proc_macro_attribute]
pub fn waml_ui_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    match expand_waml_ui_test(attr.into(), item.into()) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

fn expand_waml_ui_test(
    attr: proc_macro2::TokenStream,
    item: proc_macro2::TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
    let workspace = parse_workspace(attr)?;
    let mut function: ItemFn = syn::parse2(item)?;

    validate_function(&function)?;

    let mut wrapper_attributes = vec![quote!(#[test])];
    let mut inner_attributes = Vec::new();
    for attribute in function.attrs {
        if is_test_wrapper_attribute(&attribute) {
            wrapper_attributes.push(quote!(#attribute));
        } else {
            inner_attributes.push(attribute);
        }
    }
    function.attrs = inner_attributes;
    function.vis = syn::Visibility::Inherited;

    let test_ident = function.sig.ident.clone();
    let test_name = test_ident.to_string();
    let inner_name = format_ident!("__waml_ui_test_{}", test_ident);
    function.sig.ident = inner_name.clone();

    Ok(quote! {
        #function

        #(#wrapper_attributes)*
        fn #test_ident() {
            ::waml_ui_test::__private::run_catalog_test(
                env!("CARGO_MANIFEST_DIR"),
                env!("CARGO_PKG_NAME"),
                module_path!(),
                #test_name,
                ::waml_ui_test::WorkspaceFixture::#workspace,
                #inner_name,
            );
        }
    })
}

fn parse_workspace(attr: proc_macro2::TokenStream) -> syn::Result<syn::Ident> {
    let argument: syn::MetaNameValue = syn::parse2(attr.clone()).map_err(|_| {
        syn::Error::new_spanned(
            attr,
            "#[waml_ui_test] requires exactly `workspace = <Fixture>`",
        )
    })?;

    if !argument.path.is_ident("workspace") {
        return Err(syn::Error::new_spanned(
            argument.path,
            "#[waml_ui_test] only accepts `workspace`",
        ));
    }

    match argument.value {
        syn::Expr::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 => {
            Ok(path.path.segments[0].ident.clone())
        }
        value => Err(syn::Error::new_spanned(
            value,
            "`workspace` must be a fixture identifier",
        )),
    }
}

fn validate_function(function: &ItemFn) -> syn::Result<()> {
    if function.sig.asyncness.is_some() {
        return Err(syn::Error::new_spanned(
            function.sig.asyncness,
            "#[waml_ui_test] does not support async functions",
        ));
    }
    if !function.sig.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &function.sig.generics,
            "#[waml_ui_test] does not support generic functions",
        ));
    }
    if !matches!(function.sig.output, ReturnType::Default) {
        return Err(syn::Error::new_spanned(
            &function.sig.output,
            "#[waml_ui_test] functions must not return a value",
        ));
    }
    if function.sig.inputs.len() != 1 {
        return Err(syn::Error::new_spanned(
            &function.sig.inputs,
            "#[waml_ui_test] requires one identifier argument",
        ));
    }
    match function.sig.inputs.first() {
        Some(FnArg::Typed(argument)) if matches!(*argument.pat, Pat::Ident(_)) => Ok(()),
        _ => Err(syn::Error::new_spanned(
            &function.sig.inputs,
            "#[waml_ui_test] requires one identifier argument",
        )),
    }
}

fn is_test_wrapper_attribute(attribute: &Attribute) -> bool {
    attribute.path().is_ident("ignore") || attribute.path().is_ident("should_panic")
}

#[cfg(test)]
mod tests {
    use quote::quote;

    use super::expand_waml_ui_test;

    #[test]
    fn expands_a_catalog_test_wrapper() {
        let expanded = expand_waml_ui_test(
            quote!(workspace = Mini),
            quote! {
                fn navigation(mut app: WamlApp) {
                    app.expect_workspace_open();
                }
            },
        )
        .unwrap()
        .to_string();

        assert!(expanded.contains("# [test]"));
        assert!(expanded.contains("run_catalog_test"));
        assert!(expanded.contains("env ! (\"CARGO_MANIFEST_DIR\")"));
        assert!(expanded.contains("env ! (\"CARGO_PKG_NAME\")"));
        assert!(expanded.contains("module_path ! ()"));
        assert!(expanded.contains("\"navigation\""));
        assert!(expanded.contains("WorkspaceFixture :: Mini"));
        assert!(expanded.contains("__waml_ui_test_navigation"));
    }

    #[test]
    fn keeps_ignore_and_should_panic_on_the_wrapper() {
        let expanded = expand_waml_ui_test(
            quote!(workspace = Mini),
            quote! {
                #[ignore]
                #[should_panic]
                fn navigation(app: WamlApp) {}
            },
        )
        .unwrap()
        .to_string();

        assert_eq!(expanded.matches("ignore").count(), 1);
        assert_eq!(expanded.matches("should_panic").count(), 1);
        assert!(expanded.contains("# [test] # [ignore] # [should_panic] fn navigation"));
    }

    #[test]
    fn makes_generated_inner_function_private() {
        let expanded = expand_waml_ui_test(
            quote!(workspace = Mini),
            quote! {
                pub fn navigation(app: WamlApp) {}
            },
        )
        .unwrap()
        .to_string();

        assert!(expanded.contains("fn __waml_ui_test_navigation"));
        assert!(!expanded.contains("pub fn __waml_ui_test_navigation"));
    }

    #[test]
    fn rejects_invalid_catalog_test_signatures() {
        for (attribute, function) in [
            (
                quote!(),
                quote!(
                    fn navigation(app: WamlApp) {}
                ),
            ),
            (
                quote!(fixture = Mini),
                quote!(
                    fn navigation(app: WamlApp) {}
                ),
            ),
            (
                quote!(workspace = Mini),
                quote!(
                    async fn navigation(app: WamlApp) {}
                ),
            ),
            (
                quote!(workspace = Mini),
                quote!(
                    fn navigation<T>(app: WamlApp) {}
                ),
            ),
            (
                quote!(workspace = Mini),
                quote!(
                    fn navigation(app: WamlApp) -> Result<(), ()> {}
                ),
            ),
            (
                quote!(workspace = Mini),
                quote!(
                    fn navigation() {}
                ),
            ),
            (
                quote!(workspace = Mini),
                quote!(
                    fn navigation(first: WamlApp, second: WamlApp) {}
                ),
            ),
            (
                quote!(workspace = Mini),
                quote!(
                    fn navigation(_: WamlApp) {}
                ),
            ),
            (
                quote!(workspace = Mini),
                quote!(impl Screen { fn navigation(app: WamlApp) {} }),
            ),
        ] {
            assert!(expand_waml_ui_test(attribute, function).is_err());
        }
    }
}
