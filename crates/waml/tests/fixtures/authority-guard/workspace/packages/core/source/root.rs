mod nested;
mod lib_authority;

#[path = "../shared/path_module.rs"]
mod path_module;

include!("../shared/included.rs");
include!(concat!(env!("OUT_DIR"), "/generated.rs"));

fn body_include_host() {
    include!("../shared/body_included.rs");
}

fn body_expression_host(text: SourceText) {
    include!("../shared/body_expression.rs");
}

fn lib_route(input: &[u8]) -> LayoutStatement {
    let _ = input;
    crate::lib_authority::decode("")
}
